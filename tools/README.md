# Useful tools

- `bsn-schema-generator/` generates the checked-in Rust BSN schema from extracted BSN metadata
- `extract-bgs-metadata.py` extracts the embedded BGS protobuf descriptors and a readable JSON manifest from an SC2 executable.
- `extract-bsn-metadata/` extracts the BSN metadata block from an SC2 executable.


```console
./tools/extract-bgs-metadata.py research/.analysis/SC2 protocol/bgs
cargo run -p extract-bsn-metadata -- \
  /path/to/SC2-x64-97364.exe \
  protocol/bsn/sc2-97364-metadata.bin
cargo run -p bsn-schema-generator
```