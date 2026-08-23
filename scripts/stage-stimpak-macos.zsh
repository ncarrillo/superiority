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

cargo build --release --locked -p stimpak -p stimpak-auth-window

destination="$root/stimpak/csharp/artifacts/runtimes/$rid/native"
/usr/bin/install -d "$destination"
/usr/bin/install -m 755 "$root/target/release/libstimpak.dylib" "$destination/libstimpak.dylib"
/usr/bin/install -m 755 "$root/target/release/stimpak-auth-window" "$destination/stimpak-auth-window"

identity=${STIMPAK_MACOS_SIGN_IDENTITY:--}
sign_arguments=(--force --options runtime --sign "$identity")
if [[ "$identity" != - ]]; then
  sign_arguments+=(--timestamp)
fi
/usr/bin/codesign "${sign_arguments[@]}" "$destination/libstimpak.dylib"
/usr/bin/codesign "${sign_arguments[@]}" "$destination/stimpak-auth-window"

print "Staged $rid under $destination"
