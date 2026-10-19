"""Regenerate the committed NSIS bitmaps with Pillow; not needed for normal builds.

Uses the application's existing icon and system Noto Sans fonts. No downloaded
artwork, additional installer plugins or application runtime dependencies.
"""
from pathlib import Path
import math
import subprocess
from PIL import Image, ImageDraw, ImageFont

root = Path(__file__).resolve().parents[1]
destination = root / 'src-tauri/installer'
destination.mkdir(exist_ok=True)
scale = 4


def font(size, bold=False):
    pattern = 'Noto Sans:style=Bold' if bold else 'Noto Sans'
    path = subprocess.check_output(['fc-match', '-f', '%{file}', pattern], text=True)
    return ImageFont.truetype(path, round(size * scale))


def text(draw, xy, value, size, color, bold=False):
    draw.text(tuple(round(v * scale) for v in xy), value, font=font(size, bold), fill=color)


def save(image, name):
    # Twice the logical NSIS dimensions so Windows scaling does not magnify
    # a low-resolution logo. MUI fits the bitmap into the existing page control.
    image.resize((image.width * 2 // scale, image.height * 2 // scale), Image.Resampling.LANCZOS).convert('RGB').save(destination / name)


sidebar = Image.new('RGB', (164 * scale, 314 * scale))
pixels = sidebar.load()
for y in range(sidebar.height):
    for x in range(sidebar.width):
        glow = math.exp(-(((x / scale - 45) / 120) ** 2 + ((y / scale - 180) / 130) ** 2))
        pixels[x, y] = (round(18 + 13 * glow), round(17 + 9 * glow), round(24 + 24 * glow))
draw = ImageDraw.Draw(sidebar)
text(draw, (20, 20), 'neisora.', 21, '#eee8fa', True)
text(draw, (21, 52), 'DESKTOP MUSIC PLAYER', 6.8, '#ada0c4')
# Quiet orbital lines echo the expanded player's visualizer.
for radius in [67, 84, 101, 118]:
    cx, cy = 83 * scale, 190 * scale
    r = radius * scale
    draw.ellipse((cx - r, cy - r, cx + r, cy + r), outline='#393047', width=scale)
icon = Image.open(root / 'src-tauri/icons/icon.png').convert('RGBA')
icon.thumbnail((88 * scale, 88 * scale), Image.Resampling.LANCZOS)
sidebar.paste(icon, ((sidebar.width - icon.width) // 2, 145 * scale), icon)
draw = ImageDraw.Draw(sidebar)
draw.line((21 * scale, 272 * scale, 43 * scale, 272 * scale), fill='#d1bbfb', width=2 * scale)
text(draw, (21, 284), 'WINDOWS', 6.5, '#b5a9c7')
save(sidebar, 'sidebar.bmp')

header = Image.new('RGB', (150 * scale, 57 * scale), '#ffffff')
draw = ImageDraw.Draw(header)
text(draw, (9, 13), 'neisora.', 21, '#292332', True)
draw.ellipse((127 * scale, 25 * scale, 134 * scale, 32 * scale), fill='#cbb4ef')
save(header, 'header.bmp')
print('NSIS artwork: sidebar 328x628, header 300x114, RGB BMP (2x)')
