// Copies into the site the files it shares with the rest of the repository, so they are never
// duplicated in git: the README screenshots (docs/images/*.webp, served at /images/), the app
// icon (favicon and logo). The copies are gitignored; `pnpm dev` and `pnpm build` run this first.
import { copyFileSync, mkdirSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const site = join(dirname(fileURLToPath(import.meta.url)), "..");
const root = join(site, "..");

function copy(from, to) {
  mkdirSync(dirname(to), { recursive: true });
  copyFileSync(from, to);
}

const images = join(root, "docs", "images");
for (const name of readdirSync(images).filter((n) => n.endsWith(".webp"))) {
  copy(join(images, name), join(site, "public", "images", name));
}
copy(join(root, "src-tauri", "icons", "64x64.png"), join(site, "public", "favicon.png"));
copy(join(root, "src-tauri", "icons", "app-icon.svg"), join(site, "src", "assets", "seaglass.svg"));
