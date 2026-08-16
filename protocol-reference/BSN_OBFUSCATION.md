# SC2 BSN obfuscation: client-derived wire layout

Status: verified against the decrypted macOS x86_64 client for SC2
5.0.16.97563.

This note explains the BSN metadata bit named `Obfuscated`. It uses the
decrypted SC2 client as the source of truth. Captures verify concrete examples.

## Result

The `0x40` metadata bit marks a type as eligible for generated handling. A
separate per-type table supplies the wire layout.

For a structure, SC2 can use a separate layout with one entry per logical
field. Each entry says:

1. how many filler bits occur before the field; and
2. which logical field occurs next on the wire.

The client packs both values into one `u32`:

```rust
const FIELD_INDEX_MASK: u32 = 0x03ff_ffff;

#[derive(Clone, Copy)]
#[repr(transparent)]
struct ObfuscatedField(u32);

impl ObfuscatedField {
    fn field_index(self) -> u32 {
        self.0 & FIELD_INDEX_MASK
    }

    fn filler_bits(self) -> u8 {
        (self.0 >> 26) as u8
    }
}

struct ObfuscatedStruct<'a> {
    field_count: u64,
    fields_in_wire_order: &'a [ObfuscatedField],
}
```

The low 26 bits select a reflected field. The high 6 bits give the number of
filler bits immediately before that field. The array order is the wire order.

The wire remains a positional bitstream with field permutation and inserted
bits.

## Evidence source

The analyzed executable is:

```text
research/.analysis/SC2-runtime-decrypted-97563
```

The client contains source-path strings for BSNC 2.2.13, including:

```text
bsnc/2.2.13/noarch/src/Decoder.cpp
bsnc/2.2.13/noarch/src/Encoder.cpp
bsnc/2.2.13/noarch/src/Obfuscator.cpp
bsnc/2.2.13/noarch/src/ProtocolMeta.cpp
bsnc/2.2.13/noarch/src/SizeEncoder.cpp
```

All addresses in this note are image virtual addresses from that executable.

The main BSN metadata block starts at `0x10418e030`. It contains 9,982 types.
Of those types, 1,218 have the `0x40` bit. Every type in two other metadata
blocks, containing 89 and 1,854 types, has the bit clear.

The main metadata is initialized at `0x100e61f10`. That initializer passes:

```text
metadata bytes: 0x10418e030
metadata size:  0x72e76
code sidecar:   0x104722530
sidecar size:   0x8406 qwords (0x42030 bytes)
```

The code sidecar is separate from the metadata bytes. It contains native
object sizes and member offsets used by reflection. Generated reader and writer
functions supply the wire order for the examples below.

## Meaning of the metadata byte

The first byte of a BSN metadata dump record has this form:

```text
bit 7       indexed-type encoding flag
bit 6       Obfuscated
bits 0..5   BSN metadata type kind
```

`ProtocolMeta.cpp` at `0x10001e5b0` masks the byte with `0x3f` when it asks for
the type kind. The type-info constructor at `0x10001e630` also preserves the
complete byte. The generic encoder later tests bit `0x40` in that preserved
byte.

The bit is a type marker. Field indices and filler lengths come from the
separate generated layout.

## The separate layout table

`Obfuscator::GetStruct` is at `0x10001da80`. It receives a type ID and returns
one pointer from a type-indexed pointer table.

The returned memory has this form:

```text
offset  size                         value
0       8 bytes                      reflected field count
8       4 bytes * reflected count    packed field entries
```

The encoder, decoder, and size encoder all assert that the first value equals
the reflected field count.

The table entry is decoded as follows:

```rust
struct WireField {
    logical_field: u32, // entry & 0x03ff_ffff
    filler_before: u8,  // entry >> 26
}
```

The generic engine applies a supplied table independently of the metadata bit.
The table controls the transformation. BSNC typically generates tables for
marked types.

## Generic decoding

The generic structure decoder is at `0x10000cfc0` in `Decoder.cpp`.

Its behavior is equivalent to:

```rust
fn decode_struct(
    reader: &mut BitReader,
    reflected: &[Field],
    layout: Option<&[ObfuscatedField]>,
) -> Result<()> {
    for wire_position in 0..reflected.len() {
        let (field_index, filler_bits) = match layout {
            Some(entries) => {
                let entry = entries[wire_position];
                (entry.field_index() as usize, entry.filler_bits())
            }
            None => (wire_position, 0),
        };

        reader.skip(filler_bits as usize)?;
        decode_value(reader, &reflected[field_index])?;
    }
    Ok(())
}
```

The decoder advances over each filler run and ignores the skipped values.

When a per-type layout is absent, the decoder uses reflection order even when
metadata bit `0x40` is set.

## Generic encoding

The generic structure encoder is at `0x100013a70` in `Encoder.cpp`.

With a layout, it emits each filler run and then encodes the selected logical
field. With the layout absent, it encodes logical fields in reflection order.

The encoder generates deterministic filler values. A rolling `u32` starts at
the structure field count. Before a filler run, the encoder changes that value
using the current absolute bit position and bytes already written:

```rust
fn next_filler_value(
    mut state: u32,
    used_bits: usize,
    output: &[u8],
) -> u32 {
    let byte = used_bits / 8;

    state = match used_bits {
        0..=7 => !state,
        8..=15 => state.wrapping_add(output[byte - 1] as u32),
        16..=31 => {
            state.wrapping_add(u16::from_le_bytes([
                output[byte - 2],
                output[byte - 1],
            ]) as u32)
        }
        _ => {
            let previous_word = u32::from_le_bytes([
                output[byte - 4],
                output[byte - 3],
                output[byte - 2],
                output[byte - 1],
            ]);
            let previous_half = u16::from_le_bytes([
                output[byte - 2],
                output[byte - 1],
            ]) as u32;
            state.wrapping_add(previous_word).wrapping_add(previous_half)
        }
    };

    state.rotate_left(8)
}
```

For widths through 32, the encoder emits the requested low-width part of the
new state. It retains the new state for the next filler run. The width comes
from the high 6 bits of the packed layout entry, so the representation can
store widths from 0 through 63. Wider runs use the same 32-bit state and the
encoder's masked-shift behavior.

Generated SC2 writers call the helper at `0x100e64860`. That helper implements
the same state update and bit emission. Generated writers seed its state with
the structure field count.

The deterministic value is an encoder convention. The decrypted client
decoders examined here ignore the value. Battle.net server validation remains
an open question.

## Generic size calculation

The generic structure size encoder is at `0x100020bd0` in `SizeEncoder.cpp`.

For each layout entry, it:

1. adds `entry >> 26` to the bit count; and
2. sizes reflected field `entry & 0x03ff_ffff`.

The size encoder uses the same packed-entry format. The filler run contributes
to encoded size and contributes zero reflected fields.

## Behavior with an absent layout

The generic encoder has a setting named `obfuscatedTypesAllowed`. Its check is
narrower than the setting name suggests.

| Runtime state | Encoder behavior | Decoder behavior |
|---|---|---|
| `Obfuscator` absent; bit `0x40` clear | Reflection order | Reflection order |
| `Obfuscator` absent; bit `0x40` set; allow false | Assertion | Reflection order |
| `Obfuscator` absent; bit `0x40` set; allow true | Reflection order | Reflection order |
| `Obfuscator` exists; type entry is null | Reflection order | Reflection order |
| `Obfuscator` exists; type entry exists | Entry-defined order and filler | Entry-defined order and skip |

The assertion text is:

```text
obfuscatedTypesAllowed || !m.Obfuscated()
```

The assertion is at `0x100013a70`. It is evaluated when the encoder lacks an
`Obfuscator` object. A non-null `Obfuscator` with a null entry falls back to
reflection order directly.

The `0x40` bit marks eligibility for generated handling. Reflection-order
encoding of a marked type may require an explicit opt-in.

## Example 1: `Club::InviteAction`

Metadata type 2244 is `Battlenet::Club::InviteAction`. It is marked with
`0x40`. Its logical reflection order is:

```rust
struct InviteAction {
    club_id: ClubId,    // logical field 0; u32 on the wire
    member: ToonHandle, // logical field 1
    code: InviteCode,   // logical field 2
    result: ErrorCode,  // logical field 3; u16 on the wire
}
```

The generated writer is at `0x100f03c40`. The generated reader is at
`0x100f03eb0`. The size function is at `0x100f04120`.

The actual wire form is easier to understand as an explicit wire definition:

```rust
struct InviteActionWire {
    code: InviteCode,
    member: ToonHandle,
    club_id: ClubId,
    _filler: FillerBits<11>,
    result: ErrorCode,
}
```

The equivalent layout entries are:

```rust
[
    ObfuscatedField(0x0000_0002), // field 2, filler 0
    ObfuscatedField(0x0000_0001), // field 1, filler 0
    ObfuscatedField(0x0000_0000), // field 0, filler 0
    ObfuscatedField(0x2c00_0003), // field 3, 11 filler bits first
]
```

The generated reader advances 11 bits and ignores their values. The generated
writer seeds a local filler state with `4`, the number of logical fields, and
passes it to `0x100e64860` for the 11-bit run.

The generated size function adds 197 payload bits for a valid action. That
matches the generated field order and the 11-bit run.

## Example 2: `Friends::AccountBlockContainer`

Metadata type 2715 is `Battlenet::Friends::AccountBlockContainer`. It is also
marked with `0x40`. Its logical reflection order is:

```rust
struct AccountBlockContainer {
    account_id: AccountId,             // logical field 0; u32 on the wire
    full_name: Option<FullName>,        // logical field 1
    nickname: Option<AccountNickname>, // logical field 2
    role: RoleId,                       // logical field 3; u32 on the wire
}
```

The generated writer begins at `0x100f18720`. The generated reader begins at
`0x100f18bd0`.

The actual wire form is:

```rust
struct AccountBlockContainerWire {
    _filler_1: FillerBits<9>,
    account_id: AccountId,
    nickname: Option<AccountNickname>,
    _filler_2: FillerBits<20>,
    full_name: Option<FullName>,
    role: RoleId,
}
```

The equivalent layout entries are:

```rust
[
    ObfuscatedField(0x2400_0000), // field 0, 9 filler bits first
    ObfuscatedField(0x0000_0002), // field 2, filler 0
    ObfuscatedField(0x5000_0001), // field 1, 20 filler bits first
    ObfuscatedField(0x0000_0003), // field 3, filler 0
]
```

This example validates both field reordering and multiple filler runs. The
writer seeds the helper state with `4` and calls `0x100e64860` for both runs.
The reader advances 9 and 20 bits and ignores their values.

## Layer model

The BSN reflection metadata and the wire layout are different layers.

```text
reflection metadata
  names, logical fields, field types, ranges, optionality

wire layout
  field permutation, filler-run widths

generated codec
  compiled implementation of both layers
```

Document `0x40` as a type marker. Decoding requires an actual layout, generated
code, or build-specific proof that reflection order is correct.

## Recommended representation

The human-facing protocol specification should show logical and wire definitions
separately with readable structural notation.

For example:

```rust
// Logical API shape from reflection metadata.
struct InviteAction {
    club_id: ClubId,
    member: ToonHandle,
    code: InviteCode,
    result: ErrorCode,
}

// Build-specific wire shape from the generated client codec.
wire_struct! {
    build = 97563;
    type_id = 2244;

    struct InviteActionWire {
        code: field(InviteAction::code),
        member: field(InviteAction::member),
        club_id: field(InviteAction::club_id),
        _filler: filler(11),
        result: field(InviteAction::result),
    }
}
```

This form makes packet order visible. The logical structure remains ordinary.
The build-specific wire structure explains only what differs on the wire.

An implementation data model can stay smaller:

```rust
pub struct StructWireLayout {
    pub build: u32,
    pub type_id: u32,
    pub fields: &'static [WireField],
}

pub struct WireField {
    pub logical_field: u32,
    pub filler_before: u8,
}
```

For decoding, `filler_before` means “advance this many bits.” For encoding, it
means “emit this many bits using the client filler algorithm.”

## Extraction path

The decrypted client provides enough information for a build-specific
extractor:

1. Parse the version-7 metadata block for type names, field names, field types,
   and constraints.
2. Parse the code sidecar for native object sizes and logical member offsets.
3. Find each generated reader, writer, and size function.
4. Match native member accesses to logical fields by object offset.
5. Recognize calls to `0x100e64860` as encoder filler runs.
6. Recognize decoder loops that advance the bit cursor while skipping loads as
   filler runs.
7. Emit a packed field-layout table and a readable wire definition.
8. Validate the result by comparing generated-reader, generated-writer, and
   generated-size behavior.

The main protocol's code sidecar begins at `0x104722530`. For type 2244, it
describes a 40-byte native object with logical member offsets `0`, `8`, `32`,
and `36`. Those offsets let a static analyzer map the generated codec's object
accesses back to `club_id`, `member`, `code`, and `result`.

Reconstruct the example entry arrays by analyzing the retail generated native
functions or a live layout table.

## Proven facts, inferences, and unknowns

### Proven from the decrypted client

- `0x40` is named `Obfuscated` by the embedded encoder assertion.
- The bit is preserved separately from the low 6-bit type kind.
- A per-type layout pointer can be supplied out of band.
- A layout has a field count followed by packed `u32` entries.
- Entry bits 0 through 25 select a logical field.
- Entry bits 26 through 31 give the filler width before that field.
- The generic decoder ignores filler values.
- The generic size encoder counts filler widths.
- The generic and generated encoders use the same filler algorithm.
- The two generated examples match the reconstructed packed-entry model.
- A marked type can fall back to reflection order when its per-type entry is
  absent.

### Strong inference

The generated readers and writers are compiled forms of the same logical
layout model used by the generic `Obfuscator` path. They have identical
observable order, filler, and size behavior in the examples examined.

### Open questions

- Whether Battle.net servers validate filler values sent by clients.
- How stable a type's layout is across SC2 builds.
- Where and when every runtime `Obfuscator` table is populated.
- Whether every type marked `0x40` has a non-identity generated layout.
- Whether an unmarked type is ever given a non-null obfuscation entry.

## Documentation rules

Use these rules in the protocol reference:

1. Call `0x40` the **obfuscation marker**. Store wire layouts separately.
2. Show logical fields and wire order separately when they differ.
3. Render the wire order as an explicit definition.
4. Render inserted runs as `FillerBits<N>` or `filler(N)`.
5. State that the SC2 decoder ignores filler values.
6. Describe filler as inserted bits. Reserve integrity-check claims for
   independently proven server validation.
7. Attach the SC2 build number to every recovered generated layout.
8. Verify the generated layout of each marked type.
9. Treat a reflection-compatible capture as evidence for the observed values
   and paths.
