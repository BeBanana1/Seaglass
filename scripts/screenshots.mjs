// README screenshots (#507): headless Edge over the DevTools protocol, on the UI preview
// without Tauri (mock backend, catalog pages only: no account data). No dependency: Node's
// built-in WebSocket and fetch.
//
//   pnpm vite --port 1420 --strictPort      (in another terminal)
//   node scripts/screenshots.mjs            (writes docs/images/*.webp)
import { spawn } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const EDGE =
  process.env.SEAGLASS_EDGE ?? "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe";
const PREVIEW = "http://localhost:1420/";
const OUT = join(import.meta.dirname, "..", "docs", "images");
const DEBUG_PORT = 9333;
// `eggs=off`: the Interstellar album of the fixtures is a soundtrack (letterbox easter egg).
const COMMON = "lang=en&eggs=off";
const SHOTS = [
  { name: "album-estampe", query: "theme=estampe&route=album", width: 1440, height: 900 },
  { name: "playlist-seaglass", query: "theme=seaglass&route=playlist", width: 1440, height: 900 },
  {
    name: "now-playing-cassette",
    query: "theme=cassette&route=album&nowplaying=1",
    width: 1440,
    height: 900,
  },
  {
    name: "android-midnight",
    query: "theme=midnight&route=album&platform=android",
    width: 412,
    height: 915,
    mobile: true,
  },
];
// Lets the page load its covers and settle its entrance animations before the capture.
const SETTLE_MS = 4000;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Images of the viewport still loading or failed, after waiting up to 10 s for them. */
async function brokenImages(send) {
  const { result } = await send("Runtime.evaluate", {
    awaitPromise: true,
    returnByValue: true,
    expression: `(async () => {
      const visible = () => [...document.images].filter((i) => {
        const r = i.getBoundingClientRect();
        return r.width > 0 && r.bottom > 0 && r.top < innerHeight;
      });
      for (let i = 0; i < 50 && visible().some((img) => !img.complete); i++)
        await new Promise((r) => setTimeout(r, 200));
      return visible().filter((img) => !img.complete || img.naturalWidth === 0).length;
    })()`,
  });
  return result.value;
}

const profile = mkdtempSync(join(tmpdir(), "seaglass-shots-"));
const edge = spawn(EDGE, [
  "--headless=new",
  `--remote-debugging-port=${DEBUG_PORT}`,
  `--user-data-dir=${profile}`,
  "--hide-scrollbars",
  "--force-color-profile=srgb",
  "about:blank",
]);

try {
  let targets;
  for (let i = 0; i < 50 && !targets; i++) {
    try {
      targets = await (await fetch(`http://127.0.0.1:${DEBUG_PORT}/json/list`)).json();
    } catch {
      await sleep(200);
    }
  }
  const page = targets?.find((t) => t.type === "page");
  if (!page) throw new Error("headless Edge did not start");
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => {
    ws.addEventListener("open", resolve);
    ws.addEventListener("error", reject);
  });
  let lastId = 0;
  const pending = new Map();
  ws.addEventListener("message", (e) => {
    const msg = JSON.parse(e.data);
    pending.get(msg.id)?.(msg);
    pending.delete(msg.id);
  });
  const send = (method, params = {}) =>
    new Promise((resolve, reject) => {
      const id = ++lastId;
      pending.set(id, (m) => (m.error ? reject(new Error(m.error.message)) : resolve(m.result)));
      ws.send(JSON.stringify({ id, method, params }));
    });

  try {
    await fetch(PREVIEW);
  } catch {
    throw new Error(`no preview at ${PREVIEW}: run pnpm vite --port 1420 --strictPort`);
  }
  await send("Page.enable");
  for (const shot of SHOTS) {
    const mobile = shot.mobile === true;
    await send("Emulation.setDeviceMetricsOverride", {
      width: shot.width,
      height: shot.height,
      deviceScaleFactor: mobile ? 3 : 1.5,
      mobile,
    });
    await send("Emulation.setTouchEmulationEnabled", { enabled: mobile });
    // A cover from the image CDN sometimes fails: one more load before giving up.
    let broken = 0;
    for (let attempt = 0; attempt < 3; attempt++) {
      await send("Page.navigate", { url: `${PREVIEW}?${shot.query}&${COMMON}` });
      await sleep(SETTLE_MS);
      broken = await brokenImages(send);
      if (broken === 0) break;
    }
    if (broken > 0) throw new Error(`${shot.name}: ${broken} image(s) did not load`);
    // Freezes the ambient animations so the frame does not depend on timing.
    await send("Runtime.evaluate", {
      expression: `document.head.append(Object.assign(document.createElement("style"), {
        textContent: "*,*::before,*::after{animation-play-state:paused!important;transition:none!important;caret-color:transparent!important}",
      }))`,
    });
    await sleep(300);
    const { data } = await send("Page.captureScreenshot", { format: "webp", quality: 82 });
    writeFileSync(join(OUT, `${shot.name}.webp`), Buffer.from(data, "base64"));
    console.log(`docs/images/${shot.name}.webp`);
  }
  ws.close();
} finally {
  edge.kill();
  await sleep(500);
  rmSync(profile, { recursive: true, force: true, maxRetries: 5 });
}
