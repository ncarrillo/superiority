#!/bin/zsh
set -euo pipefail

root=${0:A:h:h}
bundle="$root/build/Superiority.app"
feed_dir="$root/build/update-feed"
bucket=superiority-releases
pages_project=superiority-sc2-updates
feed_url="https://${pages_project}.pages.dev/appcast.xml"
sparkle_root="$root/.dependencies/sparkle-2.9.5"
site="$root/site"
wrangler="$site/node_modules/.bin/wrangler"
key_account=com.superiority.sc2-chat

# Release builds carry the bare patch as their build number; without this,
# builds are stamped as dev and must never be published.
export SUPERIORITY_RELEASE_BUILD=1

"$root/scripts/build-macos-app.zsh"

version=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' \
  "$bundle/Contents/Info.plist")
build=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' \
  "$bundle/Contents/Info.plist")
release_notes="$root/release-notes/${version}.md"
archive_name="Superiority-${version}-${build}.zip"
archive="$feed_dir/$archive_name"

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
if [[ ! -x "$wrangler" ]]; then
  print -u2 "Site dependencies are missing. Run npm install in $site."
  exit 1
fi

"$wrangler" whoami >/dev/null
/usr/bin/install -d "$feed_dir"

if [[ ! -f "$feed_dir/appcast.xml" ]]; then
  /usr/bin/curl -fsSL "$feed_url" -o "$feed_dir/appcast.xml" || :
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

"$sparkle_root/bin/generate_appcast" \
  --account "$key_account" \
  --download-url-prefix "$release_base_url/releases/" \
  --embed-release-notes \
  --maximum-deltas 0 \
  "$feed_dir"

"$wrangler" r2 object put "$bucket/releases/$archive_name" \
  --remote \
  --file "$archive" \
  --content-type application/zip \
  --cache-control 'public, max-age=31536000, immutable'

# Sparkle updates an installed copy from the archive above; someone who has
# never installed it needs the disk image. It goes up under one unchanging
# name, so the download page can link to it once and never be rebuilt for a
# release — which means it is not immutable, and gets a cache short enough that
# a new release reaches people the same day.
"$root/scripts/package-macos.zsh" >/dev/null
disk_image="$root/dist/Superiority-${version}-universal.dmg"
if [[ ! -f "$disk_image" ]]; then
  print -u2 "missing disk image: $disk_image"
  exit 1
fi
"$wrangler" r2 object put "$bucket/releases/Superiority.dmg" \
  --remote \
  --file "$disk_image" \
  --content-type application/x-apple-diskimage \
  --cache-control 'public, max-age=300'

# The page carries no version, so this rebuilds nothing that changed — it runs
# because the page ships in the same Pages deploy as the appcast, and deploying
# a stale or missing dist would take the page down with it.
SUPERIORITY_RELEASE_BASE_URL="$release_base_url" npm --prefix "$site" run build

pages_dir=$(/usr/bin/mktemp -d /tmp/superiority-appcast.XXXXXX)
trap '/bin/rm -rf "$pages_dir"' EXIT
/usr/bin/ditto "$site/dist" "$pages_dir"
/usr/bin/install -m 644 "$feed_dir/appcast.xml" "$pages_dir/appcast.xml"
"$wrangler" pages deploy "$pages_dir" \
  --project-name "$pages_project" \
  --branch main \
  --commit-dirty=true

print "Published Superiority $version ($build)"
print "Site: https://${pages_project}.pages.dev/"
print "Appcast: $feed_url"
print "Archive: $release_base_url/releases/$archive_name"
print "Disk image: $release_base_url/releases/Superiority.dmg"
