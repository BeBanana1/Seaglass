// Post-build step: the server's CSP (`script-src 'self'; style-src 'self'`, docs/release.md)
// blocks inline code, which Starlight emits in a few places. This moves it into files:
// - every inline <script> becomes a <script src> to a file named by its content hash (same
//   attributes, so a classic script still runs synchronously at the same place);
// - every style="…" attribute becomes data-sgs="<hash>", with one generated stylesheet holding
//   the matching rules, linked from each page.
// Fails if anything inline is left, so a Starlight update cannot break the site silently.
import { createHash } from "node:crypto";
import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const dist = join(dirname(fileURLToPath(import.meta.url)), "..", "dist");
const assets = join(dist, "_astro");

const hash = (text) => createHash("sha256").update(text).digest("hex").slice(0, 10);

function pages(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return pages(path);
    return name.endsWith(".html") ? [path] : [];
  });
}

const styles = new Map(); // declarations -> hash
const files = pages(dist);
const html = new Map();

for (const file of files) {
  let text = readFileSync(file, "utf8");
  text = text.replace(/<script(?![^>]*\ssrc=)([^>]*)>([\s\S]*?)<\/script>/g, (all, attrs, code) => {
    if (/type=["']?application\/(ld\+)?json/.test(attrs)) return all;
    if (!code.trim()) return "";
    const name = `inline.${hash(code)}.js`;
    writeFileSync(join(assets, name), code);
    return `<script${attrs} src="/_astro/${name}"></script>`;
  });
  text = text.replace(/\sstyle=(?:"([^"]*)"|'([^']*)')/g, (_, dq, sq) => {
    const decls = (dq ?? sq).trim();
    if (!styles.has(decls)) styles.set(decls, hash(decls));
    return ` data-sgs="${styles.get(decls)}"`;
  });
  html.set(file, text);
}

const css = [...styles].map(([decls, h]) => `[data-sgs="${h}"]{${decls}}`).join("\n");
const sheet = `inline-styles.${hash(css)}.css`;
writeFileSync(join(assets, sheet), css + "\n");

for (const [file, text] of html) {
  let out = text;
  if (styles.size) {
    out = out.replace("</head>", `<link rel="stylesheet" href="/_astro/${sheet}"></head>`);
  }
  const left =
    /<script(?![^>]*\ssrc=)(?![^>]*application\/(ld\+)?json)[^>]*>|<style[\s>]|\sstyle=|\son[a-z]+=/i;
  const found = out.match(left);
  if (found) {
    console.error(`inline code left in ${relative(dist, file)}: ${found[0]}`);
    process.exit(1);
  }
  writeFileSync(file, out);
}

console.log(`no-inline: ${files.length} pages, ${styles.size} style attributes moved to ${sheet}`);
