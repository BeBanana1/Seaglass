// "Now playing" card as a shareable image (#518): a square PNG drawn on a canvas, in the active
// theme's colors, copied to the clipboard. The layout helpers are pure (tested); the drawing
// part only runs in the WebView. The Recap summary PNG (#521) can reuse `themePalette`,
// `loadImage` and `fitText`.

import type { StreamFormat, Track } from "./ipc";
import { artistNames, coverLarge, formatStream, fullTitle, type SignalVerdict } from "./format";

export const CARD_SIZE = 1080;
const MARGIN = 80;
const COVER = 600;
const SITE = "seaglass.bebanana.ch";

/** Shortens `text` with an ellipsis until `measure` says it fits in `maxWidth`. */
export function fitText(text: string, maxWidth: number, measure: (s: string) => number): string {
  if (measure(text) <= maxWidth) return text;
  const chars = Array.from(text);
  let lo = 0;
  let hi = chars.length;
  // Longest prefix that still fits with the ellipsis (binary search: titles can be long).
  while (lo < hi) {
    const mid = Math.ceil((lo + hi) / 2);
    if (measure(chars.slice(0, mid).join("").trimEnd() + "…") <= maxWidth) lo = mid;
    else hi = mid - 1;
  }
  return chars.slice(0, lo).join("").trimEnd() + "…";
}

/** Played format, then "Bit-perfect" when the whole chain is: the other verdicts
 *  (normalization, software volume, resampling) are not worth showing off. */
export function formatLine(stream: StreamFormat | null, verdict: SignalVerdict): string | null {
  if (!stream) return null;
  const format = formatStream(stream);
  return verdict.tone === "ok" ? `${format} · ${verdict.label}` : format;
}

export interface CardText {
  title: string;
  artists: string;
  album: string;
  format: string | null;
  footer: string;
}

export function cardText(
  track: Track,
  stream: StreamFormat | null,
  verdict: SignalVerdict,
): CardText {
  return {
    title: fullTitle(track),
    artists: artistNames(track),
    album: track.album.title,
    format: formatLine(stream, verdict),
    footer: `Seaglass · ${SITE}`,
  };
}

export interface Palette {
  bg: string;
  text: string;
  muted: string;
  accent: string;
  tone: string;
  display: string;
  displayWeight: string;
  ui: string;
  mono: string;
}

/** Resolves a CSS value through a hidden element, then a 1×1 canvas readback for colors:
 *  theme tokens use `color-mix()`, which the canvas does not have to understand. */
export function themePalette(tone: string): Palette {
  const probe = document.createElement("span");
  probe.style.position = "absolute";
  probe.style.visibility = "hidden";
  document.body.append(probe);
  const read = (prop: "color" | "fontFamily" | "fontWeight", value: string) => {
    probe.style[prop] = value;
    return getComputedStyle(probe)[prop];
  };
  const pixel = document.createElement("canvas").getContext("2d", { willReadFrequently: true });
  const color = (token: string, fallback: string) => {
    const css = read("color", `var(${token}, ${fallback})`);
    if (!pixel) return css;
    pixel.clearRect(0, 0, 1, 1);
    pixel.fillStyle = fallback;
    pixel.fillStyle = css;
    pixel.fillRect(0, 0, 1, 1);
    const [r, g, b, a] = pixel.getImageData(0, 0, 1, 1).data;
    return `rgb(${r} ${g} ${b} / ${(a / 255).toFixed(3)})`;
  };
  const palette: Palette = {
    bg: color("--bg", "#061012"),
    text: color("--text", "#eef6f4"),
    muted: color("--text-muted", "#9fb0ad"),
    accent: color("--accent", "#7fd0e0"),
    tone: color(tone, "#7ad6a4"),
    display: read("fontFamily", "var(--display-family, var(--font-display, system-ui))"),
    displayWeight: read("fontWeight", "var(--display-weight, 700)"),
    ui: read("fontFamily", "var(--font-ui, system-ui)"),
    mono: read("fontFamily", "var(--font-mono, monospace)"),
  };
  probe.remove();
  return palette;
}

/** Image readable by the canvas, or null. The TIDAL image CDN answers CORS requests with `*`,
 *  but the cover is usually already cached from a plain `<img>`, without that header: the
 *  cached copy would then fail the CORS check. A query string the CDN ignores gives the CORS
 *  request its own cache entry (and keeps `img-src`, no `connect-src` needed). */
export function loadImage(url: string | null): Promise<HTMLImageElement | null> {
  if (!url) return Promise.resolve(null);
  return new Promise((resolve) => {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => resolve(img);
    img.onerror = () => resolve(null);
    img.src = corsUrl(url);
  });
}

export function corsUrl(url: string): string {
  return url.startsWith("data:") ? url : `${url}${url.includes("?") ? "&" : "?"}cors`;
}

function roundRect(ctx: CanvasRenderingContext2D, x: number, y: number, size: number, r: number) {
  ctx.beginPath();
  ctx.roundRect(x, y, size, size, r);
}

function draw(
  canvas: HTMLCanvasElement,
  text: CardText,
  p: Palette,
  cover: HTMLImageElement | null,
) {
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("canvas 2d context unavailable");
  const S = CARD_SIZE;
  const width = S - 2 * MARGIN;

  ctx.fillStyle = p.bg;
  ctx.fillRect(0, 0, S, S);
  // Accent glow from the top, fading into the theme background.
  const glow = ctx.createRadialGradient(S / 2, 0, 0, S / 2, 0, S);
  glow.addColorStop(0, p.accent);
  glow.addColorStop(1, "transparent");
  ctx.globalAlpha = 0.28;
  ctx.fillStyle = glow;
  ctx.fillRect(0, 0, S, S);
  ctx.globalAlpha = 1;

  const x = (S - COVER) / 2;
  const y = 90;
  ctx.save();
  ctx.shadowColor = "rgb(0 0 0 / 0.45)";
  ctx.shadowBlur = 50;
  ctx.shadowOffsetY = 18;
  roundRect(ctx, x, y, COVER, 24);
  ctx.fillStyle = p.accent;
  ctx.fill();
  ctx.restore();
  if (cover) {
    ctx.save();
    roundRect(ctx, x, y, COVER, 24);
    ctx.clip();
    ctx.drawImage(cover, x, y, COVER, COVER);
    ctx.restore();
  }

  ctx.textAlign = "center";
  ctx.textBaseline = "alphabetic";
  const line = (s: string, font: string, color: string, top: number) => {
    ctx.font = font;
    ctx.fillStyle = color;
    ctx.fillText(
      fitText(s, width, (v) => ctx.measureText(v).width),
      S / 2,
      top,
    );
  };
  // The title shrinks a little before being cut.
  let size = 62;
  ctx.font = `${p.displayWeight} ${size}px ${p.display}`;
  while (size > 46 && ctx.measureText(text.title).width > width) {
    size -= 2;
    ctx.font = `${p.displayWeight} ${size}px ${p.display}`;
  }
  line(text.title, ctx.font, p.text, 790);
  line(text.artists, `500 38px ${p.ui}`, p.text, 852);
  line(text.album, `400 30px ${p.ui}`, p.muted, 900);

  if (text.format) {
    ctx.font = `500 28px ${p.mono}`;
    const label = fitText(text.format, width - 40, (v) => ctx.measureText(v).width);
    const w = ctx.measureText(label).width;
    const left = S / 2 - (w + 28) / 2;
    ctx.fillStyle = p.tone;
    ctx.beginPath();
    ctx.arc(left + 6, 958, 7, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = p.accent;
    ctx.textAlign = "left";
    ctx.fillText(label, left + 28, 968);
    ctx.textAlign = "center";
  }

  line(text.footer, `400 22px ${p.ui}`, p.muted, S - 40);
}

function isTainted(canvas: HTMLCanvasElement): boolean {
  try {
    canvas.getContext("2d")?.getImageData(0, 0, 1, 1);
    return false;
  } catch {
    return true;
  }
}

/** Draws the card and returns it as a PNG. A cover the canvas cannot read back (no CORS
 *  answer) would make the export fail: the card is then drawn without it. */
export async function renderCard(
  track: Track,
  stream: StreamFormat | null,
  verdict: SignalVerdict,
): Promise<Blob> {
  const toneToken = {
    ok: "--ok",
    warn: "--warn",
    lossy: "--danger",
    idle: "--text-muted",
  }[verdict.tone];
  const [cover] = await Promise.all([loadImage(coverLarge(track.album)), document.fonts.ready]);
  const palette = themePalette(toneToken);
  const text = cardText(track, stream, verdict);
  let canvas = document.createElement("canvas");
  canvas.width = canvas.height = CARD_SIZE;
  draw(canvas, text, palette, cover);
  if (cover && isTainted(canvas)) {
    canvas = document.createElement("canvas");
    canvas.width = canvas.height = CARD_SIZE;
    draw(canvas, text, palette, null);
  }
  return new Promise((resolve, reject) =>
    canvas.toBlob((b) => (b ? resolve(b) : reject(new Error("PNG export failed"))), "image/png"),
  );
}

/** Copies the card to the clipboard. The blob is handed over as a promise so the clipboard
 *  write starts within the click (user activation) while the card is still being drawn. */
export async function copyCard(
  track: Track,
  stream: StreamFormat | null,
  verdict: SignalVerdict,
): Promise<void> {
  const png = renderCard(track, stream, verdict);
  await navigator.clipboard.write([new ClipboardItem({ "image/png": png })]);
}
