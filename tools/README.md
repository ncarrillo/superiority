# Tools

| Tool | Purpose |
| --- | --- |
| `extract-bgs-metadata.py` | Extract BGS protobuf descriptors from SC2. |
| `extract-bsn-metadata/` | Extract BSN metadata from SC2. |
| `bsn-schema-generator/` | Generate the Rust BSN schema. |

```sh
./tools/extract-bgs-metadata.py research/.analysis/SC2 protocol/bgs

cargo run --release -p extract-bsn-metadata -- \
  /path/to/SC2.exe \
  protocol/bsn/sc2-metadata.bin

cargo run --release -p bsn-schema-generator
```
