// copies the shipped chrome and fonts into superiority live
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const sc2 = join(here, "..", "..");
const images = join(sc2, "app", "macos", "resources", "images");
const fonts = join(sc2, "assets", "fonts", "game");
const output = join(sc2, "live-app", "www", "public");

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
  [join(images, "backgrounds", "deep-nebula.png"), "ui/deep-nebula.png"],
  [join(images, "nine-patch", "controls", "button-idle.png"), "ui/button-idle.png"],
  [join(images, "nine-patch", "controls", "button-active.png"), "ui/button-active.png"],
  [join(images, "icons", "app-icon.png"), "ui/app-icon.png"],
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
];

for (const [source, target] of copies) {
  const destination = join(output, target);
  mkdirSync(dirname(destination), { recursive: true });
  copyFileSync(source, destination);
}
console.log(`synced ${copies.length} app resources into superiority live`);
