#!/usr/bin/env python3
"""Extract embedded BGS protobuf metadata from a Blizzard client binary.

The generated Blizzard protobuf implementation embeds ordinary serialized
``google.protobuf.FileDescriptorProto`` messages. This tool needs no protobuf
runtime: it recognizes the descriptor wire schema, writes a standard
``FileDescriptorSet``, and emits a compact JSON service/method manifest.

SC2's descriptor names begin with ``bgs/low/pb/client/``. Newer Client SDK
builds, including the one shipped with Warcraft III: Reforged, use ``bnet/``;
select those with ``--path-prefix bnet/``.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path


DEFAULT_PATH_PREFIX = "bgs/low/pb/client/"
FIELD_TYPES = {
    1: "double",
    2: "float",
    3: "int64",
    4: "uint64",
    5: "int32",
    6: "fixed64",
    7: "fixed32",
    8: "bool",
    9: "string",
    10: "group",
    11: "message",
    12: "bytes",
    13: "uint32",
    14: "enum",
    15: "sfixed32",
    16: "sfixed64",
    17: "sint32",
    18: "sint64",
}
FIELD_LABELS = {1: "optional", 2: "required", 3: "repeated"}
FILE_DESCRIPTOR_FIELDS = {
    (1, 2),   # name
    (2, 2),   # package
    (3, 2),   # dependency
    (4, 2),   # message_type
    (5, 2),   # enum_type
    (6, 2),   # service
    (7, 2),   # extension
    (8, 2),   # options
    (9, 2),   # source_code_info
    (10, 0),  # public_dependency
    (11, 0),  # weak_dependency
    (12, 2),  # syntax
    (14, 0),  # edition
}


def read_varint(data: bytes, position: int) -> tuple[int, int]:
    value = 0
    shift = 0
    for _ in range(10):
        if position >= len(data):
            raise EOFError("truncated varint")
        byte = data[position]
        position += 1
        value |= (byte & 0x7F) << shift
        if byte < 0x80:
            return value, position
        shift += 7
    raise ValueError("varint exceeds 64 bits")


def encode_varint(value: int) -> bytes:
    if value < 0:
        raise ValueError("varint must be non-negative")
    output = bytearray()
    while value >= 0x80:
        output.append((value & 0x7F) | 0x80)
        value >>= 7
    output.append(value)
    return bytes(output)


def parse_fields(
    data: bytes,
    start: int = 0,
    end: int | None = None,
    allowed: set[tuple[int, int]] | None = None,
):
    """Yield ``(number, wire_type, value, start, end)`` protobuf fields."""

    limit = len(data) if end is None else end
    position = start
    while position < limit:
        field_start = position
        tag, position = read_varint(data, position)
        number, wire_type = tag >> 3, tag & 7
        if number == 0 or wire_type not in (0, 1, 2, 5):
            raise ValueError(f"invalid field tag at offset {field_start:#x}")
        if allowed is not None and (number, wire_type) not in allowed:
            raise ValueError(f"unexpected descriptor field {number}/{wire_type}")

        if wire_type == 0:
            value, position = read_varint(data, position)
        elif wire_type == 1:
            if position + 8 > limit:
                raise EOFError("truncated fixed64")
            value = data[position : position + 8]
            position += 8
        elif wire_type == 5:
            if position + 4 > limit:
                raise EOFError("truncated fixed32")
            value = data[position : position + 4]
            position += 4
        else:
            length, position = read_varint(data, position)
            if position + length > limit:
                raise EOFError("truncated length-delimited field")
            value = data[position : position + length]
            position += length
        yield number, wire_type, value, field_start, position


def _candidate_start(data: bytes, name_offset: int) -> int | None:
    for distance in range(2, 7):
        start = name_offset - distance
        if start < 0 or data[start] != 0x0A:
            continue
        try:
            length, value_start = read_varint(data, start + 1)
        except (EOFError, ValueError):
            continue
        if value_start == name_offset and length > 0:
            return start
    return None


def _descriptor_at(
    data: bytes, start: int, path_prefix: bytes
) -> tuple[bytes, list[tuple]] | None:
    fields = []
    position = start
    while position < len(data):
        try:
            field = next(
                parse_fields(
                    data,
                    position,
                    min(len(data), start + 512 * 1024),
                    FILE_DESCRIPTOR_FIELDS,
                )
            )
        except (EOFError, StopIteration, ValueError):
            break
        fields.append(field)
        position = field[4]
    if not fields or fields[0][0] != 1:
        return None
    try:
        name = fields[0][2].decode("utf-8")
    except UnicodeDecodeError:
        return None
    if not name.startswith(path_prefix.decode()) or not name.endswith(".proto"):
        return None
    return data[start:position], fields


def find_descriptors(data: bytes, path_prefix: str = DEFAULT_PATH_PREFIX) -> dict[str, bytes]:
    encoded_prefix = path_prefix.encode("utf-8")
    descriptors: dict[str, bytes] = {}
    position = 0
    while True:
        name_offset = data.find(encoded_prefix, position)
        if name_offset < 0:
            break
        position = name_offset + 1
        start = _candidate_start(data, name_offset)
        if start is None:
            continue
        candidate = _descriptor_at(data, start, encoded_prefix)
        if candidate is None:
            continue
        descriptor, fields = candidate
        name = fields[0][2].decode("utf-8")
        if len(descriptor) > len(descriptors.get(name, b"")):
            descriptors[name] = descriptor
    return descriptors


def _message_fields(data: bytes) -> list[tuple]:
    return list(parse_fields(data))


def _text_field(fields: list[tuple], number: int, default: str = "") -> str:
    for field_number, wire_type, value, _start, _end in fields:
        if field_number == number and wire_type == 2:
            return value.decode("utf-8", "replace")
    return default


def _varint_field(fields: list[tuple], number: int, default=None):
    for field_number, wire_type, value, _start, _end in fields:
        if field_number == number and wire_type == 0:
            return value
    return default


def _enum_summary(data: bytes) -> dict:
    fields = _message_fields(data)
    values = []
    for number, wire_type, value, _start, _end in fields:
        if number != 2 or wire_type != 2:
            continue
        value_fields = _message_fields(value)
        number_value = _varint_field(value_fields, 2, 0)
        if number_value >= 1 << 31:
            number_value -= 1 << 64
        values.append(
            {"name": _text_field(value_fields, 1), "number": number_value}
        )
    return {"name": _text_field(fields, 1), "values": values}


def _field_summary(data: bytes) -> dict:
    fields = _message_fields(data)
    type_number = _varint_field(fields, 5)
    label_number = _varint_field(fields, 4)
    result = {
        "name": _text_field(fields, 1),
        "number": _varint_field(fields, 3),
        "label": FIELD_LABELS.get(label_number, f"unknown_{label_number}"),
        "type": FIELD_TYPES.get(type_number, f"unknown_{type_number}"),
    }
    for key, number in (
        ("type_name", 6),
        ("default_value", 7),
        ("json_name", 10),
        ("extendee", 2),
    ):
        value = _text_field(fields, number)
        if value:
            result[key] = value
    oneof_index = _varint_field(fields, 9)
    if oneof_index is not None:
        result["oneof_index"] = oneof_index
    return result


def _message_summary(data: bytes) -> dict:
    fields = _message_fields(data)
    return {
        "name": _text_field(fields, 1),
        "fields": [
            _field_summary(value)
            for number, wire_type, value, _start, _end in fields
            if number == 2 and wire_type == 2
        ],
        "nested_messages": [
            _message_summary(value)
            for number, wire_type, value, _start, _end in fields
            if number == 3 and wire_type == 2
        ],
        "enums": [
            _enum_summary(value)
            for number, wire_type, value, _start, _end in fields
            if number == 4 and wire_type == 2
        ],
        "oneofs": [
            _text_field(_message_fields(value), 1)
            for number, wire_type, value, _start, _end in fields
            if number == 8 and wire_type == 2
        ],
    }


def _custom_option_id(option_data: bytes, extension_number: int) -> int | None:
    for number, wire_type, value, _start, _end in _message_fields(option_data):
        if number != extension_number or wire_type != 2:
            continue
        nested = _message_fields(value)
        identifier = _varint_field(nested, 1)
        if identifier is not None:
            return identifier
    return None


def _custom_option_name(option_data: bytes, extension_number: int) -> str:
    for number, wire_type, value, _start, _end in _message_fields(option_data):
        if number != extension_number or wire_type != 2:
            continue
        name = _text_field(_message_fields(value), 1)
        if name:
            return name
    return ""


def descriptor_summary(descriptor: bytes) -> dict:
    fields = _message_fields(descriptor)
    services = []
    for number, wire_type, value, _start, _end in fields:
        if number != 6 or wire_type != 2:
            continue
        service_fields = _message_fields(value)
        methods = []
        for method_number, method_wire, method_value, _ms, _me in service_fields:
            if method_number != 2 or method_wire != 2:
                continue
            method_fields = _message_fields(method_value)
            flags = {
                number: bool(value)
                for number, wire, value, _fs, _fe in method_fields
                if wire == 0 and number in (5, 6)
            }
            method = {
                    "name": _text_field(method_fields, 1),
                    "input_type": _text_field(method_fields, 2),
                    "output_type": _text_field(method_fields, 3),
                    "client_streaming": flags.get(5, False),
                    "server_streaming": flags.get(6, False),
                }
            method_options = next(
                (
                    option_value
                    for option_number, option_wire, option_value, _os, _oe
                    in method_fields
                    if option_number == 4 and option_wire == 2
                ),
                b"",
            )
            method_id = _custom_option_id(method_options, 90000)
            if method_id is None:
                method_id = _custom_option_id(method_options, 91000)
            if method_id is not None:
                method["method_id"] = method_id
            methods.append(method)
        service = {"name": _text_field(service_fields, 1), "methods": methods}
        service_options = next(
            (
                option_value
                for option_number, option_wire, option_value, _os, _oe
                in service_fields
                if option_number == 3 and option_wire == 2
            ),
            b"",
        )
        full_name = _custom_option_name(service_options, 90000)
        if not full_name:
            full_name = _custom_option_name(service_options, 91000)
        if full_name:
            service["full_name"] = full_name
        services.append(service)

    return {
        "name": _text_field(fields, 1),
        "package": _text_field(fields, 2),
        "syntax": _text_field(fields, 12),
        "dependencies": [
            value.decode("utf-8", "replace")
            for number, wire_type, value, _start, _end in fields
            if number == 3 and wire_type == 2
        ],
        "messages": [
            _message_summary(value)
            for number, wire_type, value, _start, _end in fields
            if number == 4 and wire_type == 2
        ],
        "enums": [
            _enum_summary(value)
            for number, wire_type, value, _start, _end in fields
            if number == 5 and wire_type == 2
        ],
        "extensions": [
            _field_summary(value)
            for number, wire_type, value, _start, _end in fields
            if number == 7 and wire_type == 2
        ],
        "services": services,
        "byte_length": len(descriptor),
    }


def write_outputs(descriptors: dict[str, bytes], output_directory: Path) -> None:
    output_directory.mkdir(parents=True, exist_ok=True)
    descriptor_set = bytearray()
    summaries = []
    for name, descriptor in sorted(descriptors.items()):
        descriptor_set.append(0x0A)
        descriptor_set.extend(encode_varint(len(descriptor)))
        descriptor_set.extend(descriptor)
        summaries.append(descriptor_summary(descriptor))

    descriptor_path = output_directory / "bgs-descriptors.pb"
    descriptor_path.write_bytes(descriptor_set)
    os.chmod(descriptor_path, 0o644)
    manifest_path = output_directory / "bgs-descriptors.json"
    manifest_path.write_text(
        json.dumps(
            {"descriptor_count": len(summaries), "files": summaries},
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("executable", type=Path, help="client binary or analysis copy")
    parser.add_argument("output_directory", type=Path)
    parser.add_argument(
        "--path-prefix",
        default=DEFAULT_PATH_PREFIX,
        help=f"embedded .proto path prefix (default: {DEFAULT_PATH_PREFIX})",
    )
    args = parser.parse_args()

    data = args.executable.read_bytes()
    descriptors = find_descriptors(data, args.path_prefix)
    if not descriptors:
        parser.error("no embedded BGS descriptors found")
    write_outputs(descriptors, args.output_directory)
    print(
        f"Extracted {len(descriptors)} descriptors to "
        f"{args.output_directory.resolve()}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
