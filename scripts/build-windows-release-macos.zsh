#!/bin/zsh
set -euo pipefail

root=${0:A:h:h}
architecture=${1:-x86_64}
case "$architecture" in
  x86_64|aarch64) ;;
  *)
    print -u2 "usage: ${0:t} [x86_64|aarch64]"
    exit 1
    ;;
esac

target="${architecture}-pc-windows-msvc"
version=$(/usr/bin/awk -F'"' '/^version = "/ { print $2; exit }' "$root/app/Cargo.toml")
if [[ ! "$version" =~ '^[0-9]+\.[0-9]+\.[0-9]+$' ]]; then
  print -u2 "the application version must have three numeric components: $version"
  exit 1
fi

for command in cargo rustup; do
  if ! command -v "$command" >/dev/null; then
    print -u2 "missing required command: $command"
    exit 1
  fi
done
if ! cargo xwin --version >/dev/null 2>&1; then
  print -u2 "cargo-xwin is required; install it with: cargo install cargo-xwin"
  exit 1
fi
if ! rustup target list --installed | /usr/bin/grep -qx "$target"; then
  print -u2 "missing Rust target $target; install it with: rustup target add $target"
  exit 1
fi

SUPERIORITY_APP_VERSION="$version" cargo xwin build \
  --release \
  --locked \
  --manifest-path "$root/app/Cargo.toml" \
  --bin superiority \
  --features rust-updater \
  --target "$target"
SUPERIORITY_APP_VERSION="$version" cargo xwin build \
  --release \
  --locked \
  --manifest-path "$root/updater/Cargo.toml" \
  --bin superiority-updater-agent \
  --bin superiority-launcher \
  --target "$target"

stage="$root/build/windows/$architecture/unsigned"
if [[ "$stage" != "$root/build/windows/"*"/unsigned" ]]; then
  print -u2 "refusing to replace an unexpected Windows staging path: $stage"
  exit 1
fi
/bin/rm -rf "$stage"
/usr/bin/install -d "$stage"
target_dir="$root/target/$target/release"
/usr/bin/install -m 755 "$target_dir/superiority.exe" "$stage/superiority-app.exe"
/usr/bin/install -m 755 "$target_dir/superiority-updater-agent.exe" "$stage/superiority-updater-agent.exe"
/usr/bin/install -m 755 "$target_dir/superiority-launcher.exe" "$stage/Superiority.exe"
/usr/bin/ditto "$root/app/macos/resources" "$stage/resources"

print "Built unsigned Superiority $version for Windows $architecture"
print "Staging directory: $stage"
print "Sign and package it on Windows with:"
print -r -- "  powershell -ExecutionPolicy Bypass -File .\scripts\sign-package-windows-release.ps1 -Architecture $architecture"
