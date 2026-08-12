#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
mode=${1:-release}

if [ "$mode" = "release" ]; then
  cargo_args="--release"
  profile=release
else
  cargo_args=""
  profile=debug
fi

cd "$project_dir"
cargo build --target wasm32-unknown-unknown --target-dir "$project_dir/target" $cargo_args
wasm-bindgen \
  "target/wasm32-unknown-unknown/$profile/superiority_live_app.wasm" \
  --out-dir www/src/wasm \
  --target web \
  --no-typescript
