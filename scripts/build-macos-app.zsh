#!/bin/zsh
set -euo pipefail

root=${0:A:h:h}
app="$root/app"
bundle="$root/build/Superiority.app"
contents="$bundle/Contents"
resources="$app/macos/resources"
architectures=(aarch64-apple-darwin x86_64-apple-darwin)
signing_identity=${SUPERIORITY_CODESIGN_IDENTITY:--}

if [[ ! -d "$resources/images/nine-patch" || ! -d "$resources/fonts" ]]; then
  print -u2 "missing curated client resources: $resources"
  exit 1
fi

for architecture in $architectures; do
  if ! rustup target list --installed | /usr/bin/grep -qx "$architecture"; then
    print -u2 "missing Rust target: $architecture"
    print -u2 "install it with: rustup target add $architecture"
    exit 1
  fi
  cargo build \
    --manifest-path "$app/Cargo.toml" \
    --release \
    --bin superiority \
    --features rust-updater \
    --target "$architecture"
  cargo build \
    --manifest-path "$root/updater/Cargo.toml" \
    --release \
    --bin superiority-updater-agent \
    --target "$architecture"
done

if [[ "$bundle" != "$root/build/Superiority.app" ]]; then
  print -u2 "refusing to replace an unexpected bundle path"
  exit 1
fi

/bin/rm -rf "$bundle"
/usr/bin/install -d "$contents/MacOS" "$contents/Resources" "$contents/Helpers"
/usr/bin/lipo -create \
  "$root/target/aarch64-apple-darwin/release/superiority" \
  "$root/target/x86_64-apple-darwin/release/superiority" \
  -output "$contents/MacOS/superiority"
/bin/chmod 755 "$contents/MacOS/superiority"
/usr/bin/lipo -create \
  "$root/target/aarch64-apple-darwin/release/superiority-updater-agent" \
  "$root/target/x86_64-apple-darwin/release/superiority-updater-agent" \
  -output "$contents/Helpers/superiority-updater-agent"
/bin/chmod 755 "$contents/Helpers/superiority-updater-agent"
/usr/bin/install -m 644 "$app/macos/Info.plist" "$contents/Info.plist"

# one version source: app/Cargo.toml. a release build
# (scripts/publish-update-macos.zsh
# sets SUPERIORITY_RELEASE_BUILD) carries the bare patch number as its build
# number; every other build gets patch.epoch — the updater orders that as newer
# than the release it came from and older than the next one, so a locally
# installed build can never wear a published build's number and make its
# update invisible.
app_version=$(/usr/bin/sed -n 's/^version = "\(.*\)"/\1/p' "$app/Cargo.toml" | /usr/bin/head -1)
app_patch=${app_version##*.}
if [[ -n "${SUPERIORITY_RELEASE_BUILD:-}" ]]; then
  display_version=$app_version
  bundle_version=$app_patch
else
  display_version="${app_version}-dev"
  bundle_version="${app_patch}.$(/bin/date +%s)"
fi
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $display_version" "$contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $bundle_version" "$contents/Info.plist"
/usr/bin/ditto "$resources" "$contents/Resources"

if [[ "$signing_identity" == "-" ]]; then
  /usr/bin/codesign --force --sign - "$contents/Helpers/superiority-updater-agent"
  /usr/bin/codesign --force --sign - "$bundle"
else
  /usr/bin/codesign \
    --force \
    --options runtime \
    --timestamp \
    --sign "$signing_identity" \
    "$contents/Helpers/superiority-updater-agent"
  /usr/bin/codesign \
    --force \
    --options runtime \
    --timestamp \
    --sign "$signing_identity" \
    "$bundle"
fi

/usr/bin/codesign --verify --deep --strict --verbose=2 "$bundle"
/usr/bin/file "$contents/MacOS/superiority"

print "$bundle"
