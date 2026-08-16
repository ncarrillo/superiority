#!/bin/zsh
set -euo pipefail

root=${0:A:h:h}
site="$root/site"
wrangler="$site/node_modules/.bin/wrangler"
bucket=superiority-releases
pages_project=superiority-sc2-updates
feed_url="https://${pages_project}.pages.dev/appcast.xml"
key_account=com.superiority.sc2-chat
update_public_key='IVqqIejocXACpzUqr/W4FpT8qkuJidILS7UqPZ7x7xE='
architecture=${1:-x86_64}
case "$architecture" in
  x86_64|aarch64) ;;
  *)
    print -u2 "usage: ${0:t} [x86_64|aarch64]"
    exit 1
    ;;
esac

version=$(/usr/bin/awk -F'"' '/^version = "/ { print $2; exit }' "$root/app/Cargo.toml")
build=${version##*.}
platform="windows-${architecture}"
update_archive="$root/dist/Superiority-${version}-${build}-windows-${architecture}.zip"
install_archive="$root/dist/Superiority-${version}-windows-${architecture}.zip"
if [[ ! -f "$update_archive" || ! -f "$install_archive" ]]; then
  print -u2 "missing signed Windows archives; run sign-package-windows-release.ps1 first"
  exit 1
fi
if [[ ! -x "$wrangler" ]]; then
  print -u2 "site dependencies are missing; run npm install in $site"
  exit 1
fi

temporary=$(/usr/bin/mktemp -d /tmp/superiority-windows-publish.XXXXXX)
cleanup() {
  /bin/rm -rf "$temporary"
}
trap cleanup EXIT

for archive in "$update_archive" "$install_archive"; do
  if ! /usr/bin/unzip -tq "$archive" >/dev/null; then
    print -u2 "invalid Windows archive: $archive"
    exit 1
  fi
  if /usr/bin/unzip -Z1 "$archive" | /usr/bin/grep -Eqi 'test[- ](root|signing|certificate)|trust-superiority'; then
    print -u2 "the public Windows archive contains test-certificate material: $archive"
    exit 1
  fi
done

"$wrangler" whoami >/dev/null
release_base_url=${SUPERIORITY_RELEASE_BASE_URL:-}
if [[ -z "$release_base_url" ]]; then
  r2_status=$(NO_COLOR=1 "$wrangler" r2 bucket dev-url get "$bucket")
  release_base_url=$(print -r -- "$r2_status" | /usr/bin/grep -Eo 'https://[^[:space:]]+\.r2\.dev' | /usr/bin/head -1)
fi
if [[ -z "$release_base_url" ]]; then
  print -u2 "R2 public URL was not found; run $root/scripts/setup-cloudflare.zsh first"
  exit 1
fi
release_base_url=${release_base_url%/}
update_name=${update_archive:t}
stable_name=Superiority-Windows.zip
if [[ "$architecture" == aarch64 ]]; then
  stable_name=Superiority-Windows-arm64.zip
fi
update_url="$release_base_url/releases/$update_name"
stable_url="$release_base_url/releases/$stable_name"

upload_update=1
remote_update="$temporary/remote-update.zip"
if /usr/bin/curl -fsSL "$update_url" -o "$remote_update" 2>/dev/null; then
  if ! /usr/bin/cmp -s "$update_archive" "$remote_update"; then
    print -u2 "the immutable Windows release already exists with different bytes: $update_url"
    exit 1
  fi
  upload_update=0
fi

appcast="$temporary/appcast.xml"
/usr/bin/curl -fsSL "$feed_url" -o "$appcast"
if ! /usr/bin/grep -q "<sparkle:version>${build}</sparkle:version>" "$appcast"; then
  print -u2 "the live appcast does not contain build $build"
  exit 1
fi

cargo build --release --locked \
  --manifest-path "$root/updater/Cargo.toml" \
  --bin superiority-appcast
appcast_tool="$root/target/release/superiority-appcast"
signature=$("$appcast_tool" \
  --sign-file "$update_archive" \
  --keychain-account "$key_account" \
  --public-key "$update_public_key")
"$appcast_tool" \
  --input "$appcast" \
  --output "$appcast" \
  --build "$build" \
  --platform "$platform" \
  --url "$update_url" \
  --file "$update_archive" \
  --signature "$signature"

SUPERIORITY_RELEASE_BASE_URL="$release_base_url" \
  SUPERIORITY_WINDOWS_DOWNLOAD_URL="$stable_url" \
  npm --prefix "$site" run build
pages="$temporary/pages"
/usr/bin/ditto "$site/dist" "$pages"
/usr/bin/install -m 644 "$appcast" "$pages/appcast.xml"

if (( upload_update )); then
  "$wrangler" r2 object put "$bucket/releases/$update_name" \
    --remote \
    --file "$update_archive" \
    --content-type application/zip \
    --cache-control 'public, max-age=31536000, immutable'
fi
"$wrangler" r2 object put "$bucket/releases/$stable_name" \
  --remote \
  --file "$install_archive" \
  --content-type application/zip \
  --cache-control 'public, max-age=300'
"$wrangler" pages deploy "$pages" \
  --project-name "$pages_project" \
  --branch main \
  --commit-dirty=true

/usr/bin/install -d "$root/build/update-feed"
/usr/bin/install -m 644 "$appcast" "$root/build/update-feed/appcast.xml"
print "Published Superiority $version for Windows $architecture"
print "Windows install: $stable_url"
print "Appcast: $feed_url"
print "Site: https://${pages_project}.pages.dev/"
