"""Images de l'installeur NSIS (#194), tirées du logo.

    python -I scripts/installer-images.py

Écrit `src-tauri/nsis/sidebar.bmp` (164 x 314, pages d'accueil et de fin) et
`src-tauri/nsis/header.bmp` (150 x 57, en-tête des autres pages), en BMP 24 bits comme
l'exige NSIS. À relancer après un changement du logo (`src-tauri/icons/icon.png`,
généré par `pnpm tauri icon`). Dessiné en x4 puis réduit, pour des bords lissés.
Dépendance : Pillow.
"""

import math
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
ICON = ROOT / "src-tauri" / "icons" / "icon.png"
OUT = ROOT / "src-tauri" / "nsis"
SCALE = 4

# Couleurs du logo (`app-icon.svg`).
NIGHT = (17, 20, 23)
SEA = (19, 168, 158)
SUN = (255, 181, 71)
FONT = Path("C:/Windows/Fonts/segoeuib.ttf")


def mix(a, b, t):
    return tuple(round(x + (y - x) * t) for x, y in zip(a, b))


def font(size):
    try:
        return ImageFont.truetype(str(FONT), size)
    except OSError:
        return ImageFont.load_default()


def waves(draw, width, top, height, count):
    """Bandes de mer ondulées, de plus en plus claires vers le bas."""
    for i in range(count):
        t = i / max(count - 1, 1)
        color = mix(mix(NIGHT, SEA, 0.35), SEA, t)
        base = top + height * t
        amp = height / (count * 2.2)
        points = [(0, base + height)]
        for x in range(0, width + 1, SCALE * 2):
            y = base + amp * math.sin(x / width * 2 * math.pi * 1.5 + i * 1.3)
            points.append((x, y))
        points.append((width, base + height))
        draw.polygon(points, fill=color)


def sidebar():
    w, h = 164 * SCALE, 314 * SCALE
    img = Image.new("RGB", (w, h), NIGHT)
    draw = ImageDraw.Draw(img)
    # Ciel : nuit en haut, lueur du soleil vers l'horizon.
    horizon = int(h * 0.72)
    for y in range(h):
        t = min(y / horizon, 1) ** 2.2
        draw.line([(0, y), (w, y)], fill=mix(NIGHT, mix(NIGHT, SUN, 0.45), t))
    # Soleil couchant posé sur l'horizon, puis la mer par-dessus.
    r = w * 0.26
    draw.ellipse([w / 2 - r, horizon - r * 0.8, w / 2 + r, horizon + r * 1.2], fill=SUN)
    waves(draw, w, horizon, h - horizon, 5)
    # Logo et nom dans le ciel.
    icon = Image.open(ICON).convert("RGBA").resize((int(w * 0.62),) * 2, Image.LANCZOS)
    img.paste(icon, ((w - icon.width) // 2, int(h * 0.12)), icon)
    f = font(19 * SCALE)
    text = "Seaglass"
    tw = draw.textlength(text, font=f)
    draw.text(((w - tw) / 2, int(h * 0.12) + icon.height + 6 * SCALE), text, font=f, fill=(255, 255, 255))
    return img.resize((164, 314), Image.LANCZOS)


def header():
    # Fond blanc : celui de l'en-tête des pages de NSIS (MUI2).
    w, h = 150 * SCALE, 57 * SCALE
    img = Image.new("RGB", (w, h), (255, 255, 255))
    icon = Image.open(ICON).convert("RGBA").resize((int(h * 0.82),) * 2, Image.LANCZOS)
    img.paste(icon, (w - icon.width - 6 * SCALE, (h - icon.height) // 2), icon)
    return img.resize((150, 57), Image.LANCZOS)


def main():
    OUT.mkdir(exist_ok=True)
    sidebar().save(OUT / "sidebar.bmp")
    header().save(OUT / "header.bmp")
    print(f"écrit : {OUT / 'sidebar.bmp'}, {OUT / 'header.bmp'}")


if __name__ == "__main__":
    main()
