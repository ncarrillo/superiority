#!/bin/zsh
set -euo pipefail

root=${0:A:h:h}
architecture=${1:-x86_64}
case "$architecture" in
  x86_64)
    target=x86_64-pc-windows-msvc
    rid=win-x64
    ;;
  aarch64)
    target=aarch64-pc-windows-msvc
    rid=win-arm64
    ;;
  *)
    print -u2 "usage: ${0:t} [x86_64|aarch64]"
    exit 1
    ;;
esac

if [[ $(uname -s) != Darwin ]]; then
  print -u2 "Windows Stimpak releases are cross-compiled on macOS"
  exit 1
fi
if ! cargo xwin --version >/dev/null 2>&1; then
  print -u2 "cargo-xwin is required; install it with: cargo install cargo-xwin"
  exit 1
fi
if ! rustup target list --installed | /usr/bin/grep -qx "$target"; then
  print -u2 "missing Rust target $target; install it with: rustup target add $target"
  exit 1
fi

cargo xwin build --release --locked \
  -p stimpak \
  -p stimpak-auth-window \
  --target "$target"

destination="$root/build/stimpak/windows/$architecture/unsigned"
/usr/bin/install -d "$destination"
/usr/bin/install -m 755 "$root/target/$target/release/stimpak.dll" "$destination/stimpak.dll"
/usr/bin/install -m 755 "$root/target/$target/release/stimpak-auth-window.exe" \
  "$destination/stimpak-auth-window.exe"

print "Built unsigned $rid artifacts on macOS under $destination"
print "Sign and stage both files on this macOS host with:"
print "  scripts/sign-stage-stimpak-windows-macos.zsh $architecture"
