"""Writes the Urban Sprawl app icon: a copy of the drawn mark at 1024 or 256 pixels (no alpha
channel, so it is valid for the App Store). Standard library only.

    python3 make-icon.py <out.png> [size]

Refresh all artwork by running python3 packaging/icon/draw-logo.py.
"""
import pathlib
import shutil
import sys

size = int(sys.argv[2]) if len(sys.argv) > 2 else 256
if size not in (1024, 256):
    sys.exit(f"make-icon.py: only 1024 and 256 are drawn, not {size}")
shutil.copyfile(pathlib.Path(__file__).parent / "icon" / f"icon-{size}.png", sys.argv[1])
