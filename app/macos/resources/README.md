# Client resources

This tree contains the assets Superiority can ship and load directly.

- `fonts/` contains the app-shell and game fonts registered by the application bundle.
- `images/icons/` contains interface glyphs used by the chat shell.
- `images/dialogs/` contains the centered SC2 honeycomb title band shared by modal headers.
- `images/backgrounds/` contains the eight 4:3 chat-pane crops exposed through Superiority's app menu.
- `images/portraits/` contains the local portrait set.
- `images/nine-patch/controls/` contains the original SC2 button slices.
- `images/curated/controls/` contains the layered Battle.net top-navigation background, divider, selection, glow, and underline textures used by the channel tabs. These are hand-recovered copies: the retail DDS sources are additive-blend art with an all-zero alpha channel, so the extract/prepare pipeline would regenerate them as fully transparent PNGs. `scripts/prepare-app-assets.zsh` intentionally never writes into `images/curated/`.
- `images/nine-patch/dialogs/` contains SC2's scalable blue dialog frame used by the Settings overlay.
- `images/nine-patch/portraits/` contains the frame used around account and roster portraits.

Run `./scripts/prepare-app-assets.zsh` after refreshing the raw extraction.
