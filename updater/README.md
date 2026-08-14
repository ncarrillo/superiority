# Superiority updater

`superiority-updater` is the signed, cross-platform updater used by the desktop
app. It preserves the application's existing update dialog and consumes the
same appcast URL and Ed25519 key as the previous macOS updater.

## Compatibility contract

- macOS releases remain ordinary Sparkle RSS items with an unqualified
  `<enclosure>`. Already-installed Sparkle versions continue to update from
  them. The macOS bundle retains only the inert `SUPublicEDKey` plist value,
  because Sparkle rejects a transition update that removes the old key.
- Windows artifacts use a namespaced `<superiority:artifact>` child on the
  same item. Sparkle ignores this extension; the Rust client selects it by
  architecture.
- Event JSON deliberately matches the old custom user-driver bridge, so the
  checking, release-notes, download, extraction, ready-to-relaunch, error, and
  current-version screens are unchanged.

## Installation flow

1. Fetch and parse the bounded appcast.
2. Download the exact declared byte length into a persistent cache.
3. Verify the Ed25519 signature and, for extension artifacts, SHA-256.
4. Extract into a random staging directory and validate the platform code
   signature and product identity.
5. Prepare a one-shot updater agent before asking the app to quit.
6. Wait for the app process, replace the application, and relaunch it. A native
   progress window appears only if the post-quit install takes long enough to
   be visible.

On macOS, a writable bundle is swapped atomically. A bundle whose directory or
ownership cannot be preserved without root is staged again in a root-owned
location and installed through an Authorization Services launchd job; the
administrator prompt happens before the app exits.

On Windows, the stable `Superiority.exe` launcher reads `current.json` and
starts `versions/<build>/superiority-app.exe`. An update installs a complete
new version, advances the manifest with a durable atomic replacement, and
retains the previous executable for launch rollback. The signed launcher and
updater agent are updated alongside the application. A Program Files install
uses the standard UAC `runas` prompt before the app exits.

## Release tooling

The `superiority-appcast` binary signs archives, prepends and prunes macOS
items, and adds Windows artifacts. On macOS it can read the existing update
seed from the `com.superiority.sc2-chat` Keychain account. CI can instead set
`SUPERIORITY_UPDATE_PRIVATE_KEY` or pass `--key-file`.

The public release scripts are:

- `scripts/build-macos-app.zsh`
- `scripts/build-windows-app.ps1 -Release`
- `scripts/publish-update-macos.zsh`

Pass the Windows update ZIP to the publisher through
`SUPERIORITY_WINDOWS_UPDATE_ARCHIVE` and the full ZIP through
`SUPERIORITY_WINDOWS_INSTALL_ARCHIVE`. The publisher uploads both artifacts and
deploys one appcast.
