// Recap summary as a shareable image (#16): a square PNG with the period's key numbers and
// its top 5 tracks, drawn like the "Now playing" card (`shareCard.ts`: same theme palette,
// background, text fitting and PNG export), copied to the clipboard. The text is built by a
// pure function (tested); the drawing only runs in the WebView.

import type { Recap } from "./ipc";
import { artistNames, coverSmall, fullTitle } from "./format";
import {
  CARD_SIZE,
  cardPng,
  context2d,
  copyPng,
  fitText,
  loadImage,
  paintBackground,
  roundRect,
  themePalette,
  type Palette,
} from "./shareCard";

const MARGIN = 80;
const SITE = "seaglass.bebanana.ch";
export const TOP = 5;

export interface RecapLabels {
  kicker: string;
  time: (hours: number, minutes: number) => string;
  timeLabel: string;
  playsLabel: string;
  daysLabel: string;
  newArtistsLabel: string;
  topLabel: string;
  plays: (n: number) => string;
  num: (n: number) => string;
}

export interface RecapStat {
  value: string;
  label: string;
}

export interface RecapRow {
  title: string;
  artists: string;
  plays: string;
  cover: string | null;
}

export interface RecapCardText {
  kicker: string;
  period: string;
  stats: RecapStat[];
  topLabel: string;
  top: RecapRow[];
  footer: string;
}

export function recapCardText(recap: Recap, period: string, l: RecapLabels): RecapCardText {
  const minutes = Math.round(recap.seconds / 60);
  return {
    kicker: l.kicker,
    period,
    stats: [
      { value: l.time(Math.floor(minutes / 60), minutes % 60), label: l.timeLabel },
      { value: l.num(recap.plays), label: l.playsLabel },
      { value: l.num(recap.days), label: l.daysLabel },
      { value: l.num(recap.newArtistCount), label: l.newArtistsLabel },
    ],
    topLabel: l.topLabel,
    top: recap.tracks.slice(0, TOP).map((c) => ({
      title: fullTitle(c.item),
      artists: artistNames(c.item),
      plays: l.plays(c.plays),
      cover: coverSmall(c.item.album),
    })),
    footer: `Seaglass · ${SITE}`,
  };
}

/** Largest font size from `max` down to `min` at which `text` fits in `width`. */
function shrink(
  ctx: CanvasRenderingContext2D,
  text: string,
  font: (size: number) => string,
  max: number,
  min: number,
  width: number,
): string {
  let size = max;
  ctx.font = font(size);
  while (size > min && ctx.measureText(text).width > width) {
    size -= 2;
    ctx.font = font(size);
  }
  return ctx.font;
}

const ROW = 100;
const THUMB = 72;

function draw(
  canvas: HTMLCanvasElement,
  text: RecapCardText,
  p: Palette,
  covers: (HTMLImageElement | null)[],
) {
  const ctx = context2d(canvas);
  const S = CARD_SIZE;
  const width = S - 2 * MARGIN;
  const fit = (s: string, w: number) => fitText(s, w, (v) => ctx.measureText(v).width);
  paintBackground(ctx, p);
  ctx.textBaseline = "alphabetic";

  // Header: kicker, then the period name.
  ctx.textAlign = "left";
  ctx.font = `600 26px ${p.ui}`;
  ctx.fillStyle = p.accent;
  ctx.fillText(fit(text.kicker.toUpperCase(), width), MARGIN, 130);
  ctx.font = shrink(
    ctx,
    text.period,
    (s) => `${p.displayWeight} ${s}px ${p.display}`,
    68,
    48,
    width,
  );
  ctx.fillStyle = p.text;
  ctx.fillText(fit(text.period, width), MARGIN, 204);

  // Four numbers in a row, each shrinking to its column.
  const col = width / text.stats.length;
  text.stats.forEach((stat, i) => {
    const x = MARGIN + i * col;
    ctx.font = shrink(ctx, stat.value, (s) => `700 ${s}px ${p.display}`, 50, 30, col - 20);
    ctx.fillStyle = p.text;
    ctx.fillText(fit(stat.value, col - 20), x, 300);
    ctx.font = `400 22px ${p.ui}`;
    ctx.fillStyle = p.muted;
    ctx.fillText(fit(stat.label, col - 20), x, 336);
  });

  ctx.fillStyle = p.muted;
  ctx.globalAlpha = 0.35;
  ctx.fillRect(MARGIN, 382, width, 2);
  ctx.globalAlpha = 1;

  ctx.font = `600 26px ${p.ui}`;
  ctx.fillStyle = p.accent;
  ctx.fillText(fit(text.topLabel.toUpperCase(), width), MARGIN, 440);

  // Top tracks: rank, cover, title and artists, plays on the right.
  text.top.forEach((row, i) => {
    const top = 470 + i * ROW;
    const mid = top + THUMB / 2;
    ctx.textAlign = "left";
    ctx.font = `600 30px ${p.mono}`;
    ctx.fillStyle = p.muted;
    ctx.fillText(String(i + 1), MARGIN, mid + 11);

    const x = MARGIN + 48;
    roundRect(ctx, x, top, THUMB, 10);
    ctx.fillStyle = p.accent;
    ctx.fill();
    const cover = covers[i];
    if (cover) {
      ctx.save();
      roundRect(ctx, x, top, THUMB, 10);
      ctx.clip();
      ctx.drawImage(cover, x, top, THUMB, THUMB);
      ctx.restore();
    }

    ctx.font = `500 24px ${p.mono}`;
    const plays = row.plays;
    const playsWidth = ctx.measureText(plays).width;
    ctx.textAlign = "right";
    ctx.fillStyle = p.muted;
    ctx.fillText(plays, S - MARGIN, mid + 9);

    const textX = x + THUMB + 24;
    const textWidth = S - MARGIN - playsWidth - 32 - textX;
    ctx.textAlign = "left";
    ctx.font = `600 30px ${p.ui}`;
    ctx.fillStyle = p.text;
    ctx.fillText(fit(row.title, textWidth), textX, mid - 4);
    ctx.font = `400 24px ${p.ui}`;
    ctx.fillStyle = p.muted;
    ctx.fillText(fit(row.artists, textWidth), textX, mid + 28);
  });

  ctx.textAlign = "center";
  ctx.font = `400 22px ${p.ui}`;
  ctx.fillStyle = p.muted;
  ctx.fillText(text.footer, S / 2, S - 40);
}

export async function renderRecapCard(text: RecapCardText): Promise<Blob> {
  const [covers] = await Promise.all([
    Promise.all(text.top.map((row) => loadImage(row.cover))),
    document.fonts.ready,
  ]);
  const palette = themePalette("--accent");
  return cardPng(
    (canvas, withImages) => draw(canvas, text, palette, withImages ? covers : []),
    covers.some((c) => c !== null),
  );
}

/** Copies the recap card to the clipboard. */
export function copyRecapCard(text: RecapCardText): Promise<void> {
  return copyPng(renderRecapCard(text));
}
