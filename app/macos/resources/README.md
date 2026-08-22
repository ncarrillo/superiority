# Client resources

This tree contains the assets Superiority can ship and load directly.

- `fonts/` contains the app-shell and game fonts registered by the application bundle.
- `images/brand/` contains the baked Superiority mark (the cyan duotone adjutant) used by the games list lockup. `scripts/generate-brand-assets.zsh` regenerates it, `Superiority.icns`, and `app/windows/Superiority.ico` from `assets/png/brand/adjutant.png`.
- `images/icons/` contains interface glyphs used by the chat shell.
- `images/dialogs/` contains the centered SC2 honeycomb title band shared by modal headers.
- `images/backgrounds/` contains the SC2 and WC3:R 4:3 chat-pane crops exposed through Superiority's app menu.
- `images/portraits/` contains the local portrait set.
- `images/nine-patch/controls/` contains the original SC2 button slices.
- `images/curated/controls/` contains the layered Battle.net top-navigation background, divider, selection, glow, and underline textures used by the channel tabs. These are hand-recovered copies: the retail DDS sources are additive-blend art with an all-zero alpha channel, so the extract/prepare pipeline would regenerate them as fully transparent PNGs. The orange selection layers use the channel-swapped retail orange navigation palette; the pink layers preserve the same luminance and alpha structure with a synthesized party palette. `scripts/prepare-app-assets.zsh` only derives those palette variants and never replaces the recovered source layers.
- `images/nine-patch/dialogs/` contains SC2's scalable blue dialog frame used by the Settings overlay.
- `images/nine-patch/portraits/` contains the frame used around account and roster portraits.

Run `./scripts/extract-wc3-backgrounds.zsh` when refreshing the WC3:R source
art, then run `./scripts/prepare-app-assets.zsh` to rebuild the shipped crops.
