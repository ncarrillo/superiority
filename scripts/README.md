# Operational scripts


## Build and packaging

- `build-macos-app.zsh` builds the Apple silicon and Intel binaries, assembles
  `build/Superiority.app`, embeds the Rust updater agent and resources, and
  signs the bundle.
- `build-windows-release-macos.zsh` cross-compiles the production Windows
  binaries on macOS without signing them.
- `package-macos.zsh` builds the app and packages it as a distributable DMG,
  with optional Developer ID signing and notarization.

- `prepare-app-assets.zsh` converts the extracted SC2 and WC3 asset trees into
  the curated resources embedded by the desktop clients.
- `stage-stimpak-macos.zsh` builds, signs, and stages the current macOS Stimpak
  core and optional in-process authentication runtimes for NuGet packaging.
- `build-stimpak-windows-macos.zsh` cross-compiles unsigned Windows Stimpak and
  Stimpak.Auth runtimes on the macOS host.
- `sign-stage-stimpak-windows-macos.zsh` Authenticode-signs those DLL/EXE files
  with `osslsigncode` and stages them, without a Windows VM or SignTool.
- `package-stimpak-nuget.zsh` tests and packs both NuGet packages, rejecting any
  staged Windows DLL whose Authenticode signature does not verify.

## Publishing and infrastructure

- `publish-update-macos.zsh` builds a release, preserves the existing Sparkle
  macOS enclosure, uploads the update archive and DMG to R2, and deploys the
  update site to Pages. Set `SUPERIORITY_WINDOWS_UPDATE_ARCHIVE` to a signed
  Windows update ZIP, set `SUPERIORITY_WINDOWS_INSTALL_ARCHIVE` to the matching
  full install ZIP, and optionally set `SUPERIORITY_WINDOWS_UPDATE_PLATFORM`
  to add Windows to that same appcast release. Existing Sparkle clients ignore
  this namespaced extension.
- `publish-update-windows.zsh` adds the signed Windows artifact to the current
  appcast release, uploads the stable Windows install archive, and deploys the
  appcast and landing page without rebuilding the macOS release.
- `setup-cloudflare.zsh` provisions the update-hosting Pages project and R2
  bucket together with the Live backend queues and D1 database, then applies
  migrations and deploys the Worker.
