#!/bin/zsh
set -euo pipefail

root=${0:A:h:h}
bundle="$root/build/Superiority.app"
feed_dir="$root/build/update-feed"
bucket=superiority-releases
pages_project=superiority-sc2-updates
feed_url="https://${pages_project}.pages.dev/appcast.xml"
site="$root/site"
wrangler="$site/node_modules/.bin/wrangler"
live="$root/live"
live_wrangler="$live/node_modules/.bin/wrangler"
key_account=com.superiority.sc2-chat
update_public_key='IVqqIejocXACpzUqr/W4FpT8qkuJidILS7UqPZ7x7xE='
pages_dir=""

cleanup() {
  if [[ -n "$pages_dir" ]]; then
    /bin/rm -rf "$pages_dir"
  fi
}
trap cleanup EXIT

# Release builds carry the bare patch as their build number; without this,
# builds are stamped as dev and must never be published.
export SUPERIORITY_RELEASE_BUILD=1

# Exercise the exact HTTPS stack used by the shipped updater before creating
# or uploading any release artifact. This is intentionally a live smoke test:
# a TLS-provider mismatch must stop the release here, not reach users.
cargo test \
  --release \
  --manifest-path "$root/updater/Cargo.toml" \
  --lib \
  download::tests::fetches_live_appcast \
  -- \
  --ignored \
  --exact

if [[ ! -x "$wrangler" ]]; then
  print -u2 "Site dependencies are missing. Run npm install in $site."
  exit 1
fi
if [[ ! -x "$live_wrangler" ]]; then
  print -u2 "Live dependencies are missing. Run npm install in $live."
  exit 1
fi

npm --prefix "$site" run check
npm --prefix "$live" test
"$root/scripts/package-macos.zsh" >/dev/null

version=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' \
  "$bundle/Contents/Info.plist")
build=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' \
  "$bundle/Contents/Info.plist")
release_notes="$root/release-notes/${version}.md"
archive_name="Superiority-${version}-${build}.zip"
archive="$feed_dir/$archive_name"
disk_image="$root/dist/Superiority-${version}-universal.dmg"

if [[ ! -f "$release_notes" ]]; then
  print -u2 "missing release notes: $release_notes"
  exit 1
fi

# A build number that is already in the live feed must never ship again —
# identical numbers are exactly how a stale install becomes undetectable.
if published_feed=$(/usr/bin/curl -fsS "$feed_url" 2>/dev/null); then
  if print -r -- "$published_feed" | /usr/bin/grep -q "<sparkle:version>${build}</sparkle:version>"; then
    print -u2 "build ${build} (${version}) is already published in ${feed_url}"
    print -u2 "bump the version in app/Cargo.toml before releasing"
    exit 1
  fi
else
  print "could not fetch ${feed_url}; skipping the already-published check"
fi
"$wrangler" whoami >/dev/null
"$live_wrangler" whoami >/dev/null
/usr/bin/install -d "$feed_dir"

live_appcast="$feed_dir/appcast.live.xml"
if /usr/bin/curl -fsSL "$feed_url" -o "$live_appcast"; then
  /bin/mv -f "$live_appcast" "$feed_dir/appcast.xml"
elif [[ ! -f "$feed_dir/appcast.xml" ]]; then
  /usr/bin/touch "$feed_dir/appcast.xml"
fi

/usr/bin/ditto -c -k --keepParent "$bundle" "$archive"
/usr/bin/install -m 644 "$release_notes" "$feed_dir/${archive_name:r}.md"

release_base_url=${SUPERIORITY_RELEASE_BASE_URL:-}
if [[ -z "$release_base_url" ]]; then
  r2_status=$(NO_COLOR=1 "$wrangler" r2 bucket dev-url get "$bucket")
  release_base_url=$(print -r -- "$r2_status" | /usr/bin/grep -Eo 'https://[^[:space:]]+\.r2\.dev' | /usr/bin/head -1)
fi
if [[ -z "$release_base_url" ]]; then
  print -u2 "R2 public URL was not found. Run $root/scripts/setup-cloudflare.zsh first."
  exit 1
fi
release_base_url=${release_base_url%/}

cargo build --release \
  --manifest-path "$root/updater/Cargo.toml" \
  --bin superiority-appcast
appcast_tool="$root/target/release/superiority-appcast"
mac_signature=$("$appcast_tool" \
  --sign-file "$archive" \
  --keychain-account "$key_account" \
  --public-key "$update_public_key")
published_at=$(LC_ALL=C /bin/date '+%a, %d %b %Y %H:%M:%S %z')
"$appcast_tool" \
  --mode publish-macos \
  --input "$feed_dir/appcast.xml" \
  --output "$feed_dir/appcast.xml" \
  --feed-url "$feed_url" \
  --title "$version" \
  --version "$version" \
  --build "$build" \
  --published-at "$published_at" \
  --minimum-system-version 14.0 \
  --notes-file "$release_notes" \
  --url "$release_base_url/releases/$archive_name" \
  --file "$archive" \
  --signature "$mac_signature" \
  --maximum-releases 3

# A Windows release is optional so macOS-only hotfixes keep the existing
# workflow. When supplied, it is added to the same release item using a
# namespaced element that old Sparkle clients ignore.
windows_archive=${SUPERIORITY_WINDOWS_UPDATE_ARCHIVE:-}
windows_install_archive=${SUPERIORITY_WINDOWS_INSTALL_ARCHIVE:-}
windows_platform=${SUPERIORITY_WINDOWS_UPDATE_PLATFORM:-windows-x86_64}
windows_download_url=
if [[ -n "$windows_archive" ]]; then
  windows_archive=${windows_archive:A}
  if [[ ! -f "$windows_archive" ]]; then
    print -u2 "missing Windows update archive: $windows_archive"
    exit 1
  fi
  if [[ -z "$windows_install_archive" || ! -f "$windows_install_archive" ]]; then
    print -u2 "SUPERIORITY_WINDOWS_INSTALL_ARCHIVE must name the signed full Windows archive"
    exit 1
  fi
  windows_install_archive=${windows_install_archive:A}
  case "$windows_platform" in
    windows-x86_64|windows-aarch64) ;;
    *)
      print -u2 "unsupported Windows update platform: $windows_platform"
      exit 1
      ;;
  esac
  windows_archive_name=${windows_archive:t}
  expected_windows_name="Superiority-${version}-${build}-${windows_platform%-*}-${windows_platform#*-}.zip"
  windows_install_name=${windows_install_archive:t}
  expected_windows_install_name="Superiority-${version}-${windows_platform%-*}-${windows_platform#*-}.zip"
  if [[ "$windows_archive_name" != "$expected_windows_name" ]]; then
    print -u2 "unexpected Windows archive name: $windows_archive_name"
    print -u2 "expected: $expected_windows_name"
    exit 1
  fi
  if [[ "$windows_install_name" != "$expected_windows_install_name" ]]; then
    print -u2 "unexpected Windows install archive name: $windows_install_name"
    print -u2 "expected: $expected_windows_install_name"
    exit 1
  fi
  windows_signature=$("$appcast_tool" \
    --sign-file "$windows_archive" \
    --keychain-account "$key_account" \
    --public-key "$update_public_key")
  "$appcast_tool" \
    --input "$feed_dir/appcast.xml" \
    --output "$feed_dir/appcast.xml" \
    --build "$build" \
    --platform "$windows_platform" \
    --url "$release_base_url/releases/$windows_archive_name" \
    --file "$windows_archive" \
    --signature "$windows_signature"
  if [[ "$windows_platform" == "windows-x86_64" ]]; then
    windows_stable_name=Superiority-Windows.zip
  else
    windows_stable_name=Superiority-Windows-arm64.zip
  fi
  windows_download_url="$release_base_url/releases/$windows_stable_name"
fi

if [[ ! -f "$disk_image" ]]; then
  print -u2 "missing disk image: $disk_image"
  exit 1
fi

SUPERIORITY_RELEASE_BASE_URL="$release_base_url" \
  SUPERIORITY_WINDOWS_DOWNLOAD_URL="$windows_download_url" \
  npm --prefix "$site" run build

# Live, the desktop release, and the landing page advance through this one
# release command. Pages is deployed last, so the appcast only advertises
# artifacts after Live and every download are available.
(cd "$live" && npm run migrate:remote && "$live_wrangler" deploy)

"$wrangler" r2 object put "$bucket/releases/$archive_name" \
  --remote \
  --file "$archive" \
  --content-type application/zip \
  --cache-control 'public, max-age=31536000, immutable'

if [[ -n "$windows_archive" ]]; then
  "$wrangler" r2 object put "$bucket/releases/$windows_archive_name" \
    --remote \
    --file "$windows_archive" \
    --content-type application/zip \
    --cache-control 'public, max-age=31536000, immutable'
  "$wrangler" r2 object put "$bucket/releases/$windows_stable_name" \
    --remote \
    --file "$windows_install_archive" \
    --content-type application/zip \
    --cache-control 'public, max-age=300'
fi

# Someone who has never installed Superiority needs the disk image under a
# stable URL. Its short cache lets new releases replace it promptly.
"$wrangler" r2 object put "$bucket/releases/Superiority.dmg" \
  --remote \
  --file "$disk_image" \
  --content-type application/x-apple-diskimage \
  --cache-control 'public, max-age=300'

pages_dir=$(/usr/bin/mktemp -d /tmp/superiority-appcast.XXXXXX)
/usr/bin/ditto "$site/dist" "$pages_dir"
/usr/bin/install -m 644 "$feed_dir/appcast.xml" "$pages_dir/appcast.xml"
"$wrangler" pages deploy "$pages_dir" \
  --project-name "$pages_project" \
  --branch main \
  --commit-dirty=true

print "Published Superiority $version ($build)"
print "Site: https://${pages_project}.pages.dev/"
print "Live: https://live.superioritybot.com/"
print "Appcast: $feed_url"
print "Archive: $release_base_url/releases/$archive_name"
if [[ -n "$windows_archive" ]]; then
  print "Windows archive: $release_base_url/releases/$windows_archive_name"
  print "Windows install: $release_base_url/releases/$windows_stable_name"
fi
print "Disk image: $release_base_url/releases/Superiority.dmg"
