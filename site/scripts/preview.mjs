// Local preview of the built site (`pnpm build` then `pnpm preview`) under the server's real
// headers (Caddy block in docs/release.md), so a CSP violation shows up in the browser console
// before publishing. /updates/* is fetched from the live server: the download page shows the
// published versions.
import { createServer } from "node:http";
import { readFile, stat } from "node:fs/promises";
import { dirname, extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const dist = join(dirname(fileURLToPath(import.meta.url)), "..", "dist");
const port = Number(process.env.PORT ?? 4321);
const LIVE = "https://seaglass.bebanana.ch";
const CSP =
  "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self'; " +
  "base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

const types = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript",
  ".css": "text/css",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".webp": "image/webp",
  ".json": "application/json",
  ".xml": "application/xml",
};

async function file(path) {
  const info = await stat(path).catch(() => null);
  if (info?.isDirectory()) return file(join(path, "index.html"));
  return info ? path : null;
}

createServer(async (req, res) => {
  res.setHeader("Content-Security-Policy", CSP);
  res.setHeader("X-Content-Type-Options", "nosniff");
  const url = new URL(req.url ?? "/", "http://localhost");
  if (url.pathname.startsWith("/updates/")) {
    const live = await fetch(LIVE + url.pathname).catch(() => null);
    res.writeHead(live?.status ?? 502, { "Content-Type": "application/json" });
    res.end(live ? Buffer.from(await live.arrayBuffer()) : "");
    return;
  }
  const path = await file(
    join(dist, normalize(decodeURIComponent(url.pathname)).replace(/^(\.\.[/\\])+/, "")),
  );
  if (!path) {
    res.writeHead(404, { "Content-Type": types[".html"] });
    res.end(await readFile(join(dist, "404.html")));
    return;
  }
  res.writeHead(200, { "Content-Type": types[extname(path)] ?? "application/octet-stream" });
  res.end(await readFile(path));
}).listen(port, () => console.log(`preview on http://localhost:${port}/`));
