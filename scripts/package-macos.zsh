#!/bin/zsh
set -euo pipefail

root=${0:A:h:h}
bundle="$root/build/Superiority.app"
dist="$root/dist"
install_note="$root/app/macos/INSTALL.txt"
signing_identity=${SUPERIORITY_CODESIGN_IDENTITY:--}
notary_profile=${SUPERIORITY_NOTARY_PROFILE:-}

"$root/scripts/build-macos-app.zsh"

version=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' \
  "$bundle/Contents/Info.plist")
dmg="$dist/Superiority-$version-universal.dmg"
staging=$(/usr/bin/mktemp -d /tmp/superiority-distribution.XXXXXX)
trap '/bin/rm -rf "$staging"' EXIT

/usr/bin/codesign --verify --deep --strict --verbose=2 "$bundle"
/usr/bin/install -d "$dist" "$staging"
/usr/bin/ditto "$bundle" "$staging/Superiority.app"
/bin/ln -s /Applications "$staging/Applications"
/usr/bin/install -m 644 "$install_note" "$staging/Read Me.txt"

/usr/bin/hdiutil create \
  -volname "Superiority" \
  -srcfolder "$staging" \
  -format UDZO \
  -ov \
  "$dmg"

if [[ "$signing_identity" != "-" ]]; then
  /usr/bin/codesign --force --timestamp --sign "$signing_identity" "$dmg"
fi

if [[ -n "$notary_profile" ]]; then
  if [[ "$signing_identity" == "-" ]]; then
    print -u2 "notarization requires SUPERIORITY_CODESIGN_IDENTITY"
    exit 1
  fi
  /usr/bin/xcrun notarytool submit \
    "$dmg" \
    --keychain-profile "$notary_profile" \
    --wait
  /usr/bin/xcrun stapler staple "$dmg"
  /usr/sbin/spctl --assess --type open --context context:primary-signature -vv "$dmg"
fi

/usr/bin/shasum -a 256 "$dmg"
print "$dmg"
