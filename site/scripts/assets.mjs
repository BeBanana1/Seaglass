// Copies into the site the files it shares with the rest of the repository, so they are never
// duplicated in git: the README screenshots (docs/images/*.webp, into src/assets/screenshots/ so
// that astro:assets builds their responsive sizes), the app icon (favicon and logo), and the
// link preview image (og.jpg, cropped from a screenshot, #48). The copies are gitignored;
// `pnpm dev` and `pnpm build` run this first.
import { copyFileSync, mkdirSync, readdirSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const site = join(dirname(fileURLToPath(import.meta.url)), "..");
const root = join(site, "..");

function copy(from, to) {
  mkdirSync(dirname(to), { recursive: true });
  copyFileSync(from, to);
}

const images = join(root, "docs", "images");
for (const name of readdirSync(images).filter((n) => n.endsWith(".webp"))) {
  copy(join(images, name), join(site, "src", "assets", "screenshots", name));
}
// Served from public/images/ before #48: an old copy left there would still be published.
rmSync(join(site, "public", "images"), { recursive: true, force: true });

copy(join(root, "src-tauri", "icons", "64x64.png"), join(site, "public", "favicon.png"));
copy(join(root, "src-tauri", "icons", "app-icon.svg"), join(site, "src", "assets", "seaglass.svg"));

// Link preview (og:image, astro.config.mjs): 1200×630, the size Discord, Mastodon and the
// others expect, from the top of the playlist screenshot (16:10, so the bottom is cropped).
await sharp(join(images, "playlist-seaglass.webp"))
  .resize(1200, 630, { fit: "cover", position: "top" })
  .jpeg({ quality: 85, mozjpeg: true })
  .toFile(join(site, "public", "og.jpg"));
