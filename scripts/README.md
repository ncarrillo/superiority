# Operational scripts


## Build and packaging

- `build-macos-app.zsh` builds the Apple silicon and Intel binaries, assembles
  `build/Superiority.app`, embeds the Rust updater agent and resources, and
  signs the bundle.
- `build-windows-app.ps1` builds the stable Windows launcher, versioned app,
  updater agent, full install archive, and update archive. Release packaging
  requires an Authenticode certificate through
  `SUPERIORITY_WINDOWS_CERTIFICATE_SHA1`.
- `package-macos.zsh` builds the app and packages it as a distributable DMG,
  with optional Developer ID signing and notarization.
- `prepare-app-assets.zsh` converts the extracted SC2 asset tree into the
  curated resources embedded by the macOS client.

## Publishing and infrastructure

- `publish-update-macos.zsh` builds a release, preserves the existing Sparkle
  macOS enclosure, uploads the update archive and DMG to R2, and deploys the
  update site to Pages. Set `SUPERIORITY_WINDOWS_UPDATE_ARCHIVE` to a signed
  Windows update ZIP, set `SUPERIORITY_WINDOWS_INSTALL_ARCHIVE` to the matching
  full install ZIP, and optionally set `SUPERIORITY_WINDOWS_UPDATE_PLATFORM`
  to add Windows to that same appcast release. Existing Sparkle clients ignore
  this namespaced extension.
- `setup-cloudflare.zsh` provisions the update-hosting Pages project and R2
  bucket together with the Live backend queues and D1 database, then applies
  migrations and deploys the Worker.
