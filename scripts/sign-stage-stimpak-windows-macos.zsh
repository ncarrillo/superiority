#!/bin/zsh
set -euo pipefail

root=${0:A:h:h}
architecture=${1:-x86_64}
case "$architecture" in
  x86_64) rid=win-x64 ;;
  aarch64) rid=win-arm64 ;;
  *)
    print -u2 "usage: ${0:t} [x86_64|aarch64]"
    exit 1
    ;;
esac

if [[ $(uname -s) != Darwin ]]; then
  print -u2 "Windows Stimpak signing runs on macOS"
  exit 1
fi
for command in osslsigncode; do
  if ! command -v "$command" >/dev/null; then
    print -u2 "missing required command: $command"
    exit 1
  fi
done

certificate=${STIMPAK_WINDOWS_CERTIFICATE:-}
password_file=${STIMPAK_WINDOWS_PASSWORD_FILE:-}
timestamp_url=${STIMPAK_WINDOWS_TIMESTAMP_URL:-http://timestamp.digicert.com}
if [[ ! -f "$certificate" ]]; then
  print -u2 "STIMPAK_WINDOWS_CERTIFICATE must name a PKCS#12 certificate"
  exit 1
fi
if [[ ! -f "$password_file" ]]; then
  print -u2 "STIMPAK_WINDOWS_PASSWORD_FILE must name a protected password file"
  exit 1
fi

source_dir="$root/build/stimpak/windows/$architecture/unsigned"
destination="$root/stimpak/csharp/artifacts/runtimes/$rid/native"
for name in stimpak.dll stimpak-auth-window.exe; do
  if [[ ! -f "$source_dir/$name" ]]; then
    print -u2 "missing unsigned artifact: $source_dir/$name"
    exit 1
  fi
done

temporary=$(/usr/bin/mktemp -d /tmp/stimpak-windows-sign.XXXXXX)
cleanup() {
  /bin/rm -rf "$temporary"
}
trap cleanup EXIT

for name in stimpak.dll stimpak-auth-window.exe; do
  signed="$temporary/$name"
  osslsigncode sign \
    -pkcs12 "$certificate" \
    -readpass "$password_file" \
    -h sha256 \
    -n "Stimpak" \
    -i "https://github.com/ncarrillo/superiority" \
    -ts "$timestamp_url" \
    -in "$source_dir/$name" \
    -out "$signed"
  osslsigncode verify -in "$signed" >/dev/null
done

/usr/bin/install -d "$destination"
/usr/bin/install -m 755 "$temporary/stimpak.dll" "$destination/stimpak.dll"
/usr/bin/install -m 755 "$temporary/stimpak-auth-window.exe" \
  "$destination/stimpak-auth-window.exe"

print "Signed and staged $rid on macOS under $destination"
