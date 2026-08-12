#!/bin/zsh
set -euo pipefail

root=${0:A:h:h}
version=2.9.5
archive_name="Sparkle-${version}.tar.xz"
archive_url="https://github.com/sparkle-project/Sparkle/releases/download/${version}/${archive_name}"
archive_sha256=015336b601493e05c237964954bff6191370003d94edefe663724c88840d73cc
dependency_root="$root/.dependencies/sparkle-$version"
framework="$dependency_root/Sparkle.framework"

if [[ -d "$framework" ]]; then
  print "$framework"
  exit 0
fi

temporary=$(mktemp -d /tmp/superiority-sparkle.XXXXXX)
trap '/bin/rm -rf "$temporary"' EXIT

print "Downloading Sparkle $version..." >&2
/usr/bin/curl -fsSL "$archive_url" -o "$temporary/$archive_name"
actual_sha256=$(/usr/bin/shasum -a 256 "$temporary/$archive_name" | /usr/bin/awk '{print $1}')
if [[ "$actual_sha256" != "$archive_sha256" ]]; then
  print -u2 "Sparkle checksum mismatch: expected $archive_sha256, got $actual_sha256"
  exit 1
fi

/usr/bin/tar -xJf "$temporary/$archive_name" -C "$temporary"
/usr/bin/install -d "$dependency_root"
/usr/bin/ditto "$temporary/Sparkle.framework" "$framework"
/usr/bin/ditto "$temporary/bin" "$dependency_root/bin"
/usr/bin/install -m 644 "$temporary/LICENSE" "$dependency_root/LICENSE"

print "$framework"
