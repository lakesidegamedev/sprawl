"""Writes the Urban Sprawl app icon: a copy of the drawn mark at 1024 or 256 pixels (no alpha
channel, so it is valid for the App Store). Standard library only.

    python3 make-icon.py <out.png> [size]

The PNGs in `icon/` are drawn from `icon/draw-logo.py`:
    python3 draw-logo.py icon-square.svg --square
    rsvg-convert -w 1024 -b '#13212F' icon-square.svg | magick - -alpha off icon-1024.png
"""
import pathlib
import shutil
import sys

size = int(sys.argv[2]) if len(sys.argv) > 2 else 256
if size not in (1024, 256):
    sys.exit(f"make-icon.py: only 1024 and 256 are drawn, not {size}")
shutil.copyfile(pathlib.Path(__file__).parent / "icon" / f"icon-{size}.png", sys.argv[1])
