#!/bin/zsh
set -euo pipefail

root=${0:A:h:h}
if [[ $(uname -s) != Darwin ]]; then
  print -u2 "Stimpak release staging runs on macOS"
  exit 1
fi

case $(uname -m) in
  arm64) rid=osx-arm64 ;;
  x86_64) rid=osx-x64 ;;
  *)
    print -u2 "unsupported macOS architecture: $(uname -m)"
    exit 1
    ;;
esac

cargo build --release --locked -p stimpak -p stimpak-auth

core_destination="$root/stimpak/csharp/artifacts/runtimes/$rid/native"
auth_destination="$root/stimpak/csharp/Stimpak.Auth/artifacts/runtimes/$rid/native"
/usr/bin/install -d "$core_destination" "$auth_destination"
/usr/bin/install -m 755 "$root/target/release/libstimpak.dylib" \
  "$core_destination/libstimpak.dylib"
/usr/bin/install -m 755 "$root/target/release/libstimpak_auth.dylib" \
  "$auth_destination/libstimpak_auth.dylib"

identity=${STIMPAK_MACOS_SIGN_IDENTITY:--}
sign_arguments=(--force --options runtime --sign "$identity")
if [[ "$identity" != - ]]; then
  sign_arguments+=(--timestamp)
fi
/usr/bin/codesign "${sign_arguments[@]}" "$core_destination/libstimpak.dylib"
/usr/bin/codesign "${sign_arguments[@]}" "$auth_destination/libstimpak_auth.dylib"

print "Staged Stimpak and Stimpak.Auth $rid runtimes"
