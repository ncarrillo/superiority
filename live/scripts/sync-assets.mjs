// copies the shipped chrome and fonts into superiority live
import { copyFileSync, mkdirSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const sc2 = join(here, "..", "..");
const images = join(sc2, "app", "macos", "resources", "images");
const fonts = join(sc2, "assets", "fonts", "game");
const output = join(sc2, "live-app", "www", "public");

// per-product avatar catalogues: the viewer resolves a member's `avatar` id
// (SC:R `avatar_terran_marine`, WC3 `p126`) to one of these PNGs on demand, so
// the whole set is copied but nothing is on the critical path.
function productImages(relativeDir, targetPrefix) {
  const source = join(images, "products", relativeDir);
  return readdirSync(source)
    .filter((name) => name.endsWith(".png"))
    .map((name) => [join(source, name), `${targetPrefix}/${name}`]);
}

// portrait avatars render directly from the atlas cells
const atlases = Array.from({ length: 17 }, (_, index) => {
  const name = `atlas-${String(index).padStart(2, "0")}.png`;
  return [join(images, "portrait-atlases", name), `ui/portraits/${name}`];
});

const copies = [
  ...atlases,
  [join(images, "curated", "controls", "top-nav-background.png"), "ui/top-nav-background.png"],
  [join(images, "curated", "controls", "top-nav-divider.png"), "ui/top-nav-divider.png"],
  [join(images, "curated", "controls", "top-nav-hover.png"), "ui/top-nav-hover.png"],
  [join(images, "curated", "controls", "top-nav-selected.png"), "ui/top-nav-selected.png"],
  [join(images, "curated", "controls", "top-nav-selected-line.png"), "ui/top-nav-selected-line.png"],
  [join(images, "curated", "controls", "top-nav-selected-line-glow.png"), "ui/top-nav-selected-line-glow.png"],
  [join(images, "curated", "controls", "top-nav-selected-orange.png"), "ui/top-nav-selected-orange.png"],
  [join(images, "curated", "controls", "top-nav-selected-line-orange.png"), "ui/top-nav-selected-line-orange.png"],
  [join(images, "curated", "controls", "top-nav-selected-line-glow-orange.png"), "ui/top-nav-selected-line-glow-orange.png"],
  [join(images, "curated", "controls", "top-nav-selected-pink.png"), "ui/top-nav-selected-pink.png"],
  [join(images, "curated", "controls", "top-nav-selected-line-pink.png"), "ui/top-nav-selected-line-pink.png"],
  [join(images, "curated", "controls", "top-nav-selected-line-glow-pink.png"), "ui/top-nav-selected-line-glow-pink.png"],
  [join(images, "backgrounds", "deep-nebula.png"), "ui/deep-nebula.png"],
  // the game picker's card art, one per product
  [join(images, "backgrounds", "shakuras-nebula.png"), "ui/backgrounds/shakuras-nebula.png"],
  [join(images, "backgrounds", "swarm-horizon.png"), "ui/backgrounds/swarm-horizon.png"],
  [join(images, "backgrounds", "wc3-orc-standard.png"), "ui/backgrounds/wc3-orc-standard.png"],
  [join(images, "nine-patch", "controls", "button-idle.png"), "ui/button-idle.png"],
  [join(images, "nine-patch", "controls", "button-active.png"), "ui/button-active.png"],
  [join(images, "icons", "app-icon.png"), "ui/app-icon.png"],
  [join(images, "brand", "logo-tile.png"), "ui/brand/logo-tile.png"],
  [join(images, "icons", "friend-placeholder.png"), "ui/friend-placeholder.png"],
  [join(images, "nine-patch", "portraits", "frame.png"), "ui/portrait-frame.png"],
  [join(images, "settings", "tooltip-fill.png"), "ui/tooltip-fill.png"],
  [join(images, "icons", "status-available.png"), "ui/status-available.png"],
  [join(images, "icons", "status-away.png"), "ui/status-away.png"],
  [join(images, "icons", "status-busy.png"), "ui/status-busy.png"],
  [join(images, "icons", "status-in-game.png"), "ui/status-in-game.png"],
  [join(images, "icons", "status-offline.png"), "ui/status-offline.png"],
  [join(fonts, "eurostile-bol.otf"), "fonts/eurostile-bol.otf"],
  [join(fonts, "eurostile-reg.otf"), "fonts/eurostile-reg.otf"],
  [join(fonts, "eurostileext-med.otf"), "fonts/eurostileext-med.otf"],
  [join(fonts, "bl.ttf"), "fonts/blizzard-global.ttf"],
  // Friz Quadrata, Reforged's own UI face, extracted from the WC3:R client
  [join(fonts, "frizqt.ttf"), "fonts/frizqt.ttf"],
  ...productImages("scr/avatars", "ui/products/scr/avatars"),
  ...productImages("wc3/portraits", "ui/products/wc3/portraits"),
];

for (const [source, target] of copies) {
  const destination = join(output, target);
  mkdirSync(dirname(destination), { recursive: true });
  copyFileSync(source, destination);
}
console.log(`synced ${copies.length} app resources into superiority live`);
