# Operational scripts


## Build and packaging

- `bootstrap-sparkle.zsh` downloads the pinned Sparkle framework, verifies its
  checksum, and caches it under `.dependencies/`.
- `build-macos-app.zsh` builds the Apple silicon and Intel binaries, assembles
  `build/Superiority.app`, embeds resources and Sparkle, and signs the bundle.
- `package-macos.zsh` builds the app and packages it as a distributable DMG,
  with optional Developer ID signing and notarization.
- `prepare-app-assets.zsh` converts the extracted SC2 asset tree into the
  curated resources embedded by the macOS client.

## Publishing and infrastructure

- `publish-update-macos.zsh` builds a release, generates the Sparkle appcast, uploads
  the update archive and DMG to R2, and deploys the update site to Pages.
- `setup-cloudflare.zsh` provisions the update-hosting Pages project and R2
  bucket together with the Live backend queues and D1 database, then applies
  migrations and deploys the Worker.