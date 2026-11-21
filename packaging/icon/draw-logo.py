"""Rasterize the Urban Sprawl skyline from hand-drawn 32 x 32 pixel artwork.

Run this file to refresh the SVG marks, packaging PNGs and desktop RGBA icon.
The PNGs are opaque RGB for App Store packaging. No external libraries are used.
"""

from pathlib import Path
import struct
import zlib

PALETTE = {
    ".": "101b30",  # night sky
    "s": "263953",  # horizon
    "a": "f2ae48",  # sun and road paint
    "p": "d56b45",  # sunset stripes
    "d": "18263d",  # dark trim
    "n": "284964",  # tower sides
    "b": "437a91",  # tower fronts
    "c": "91d5cc",  # glass
    "w": "ffe3a0",  # warm windows
    "g": "27645b",  # park shadow
    "m": "4a9269",  # park
    "r": "334355",  # asphalt
    "t": "a7b2ab",  # kerb
}
SIZE = 32
pixels = [["."] * SIZE for _ in range(SIZE)]


def paint(x, y, rows):
    for dy, row in enumerate(rows):
        for dx, colour in enumerate(row):
            if colour != " ":
                pixels[y + dy][x + dx] = colour


def fill(x, y, width, height, colour):
    paint(x, y, [colour * width] * height)


fill(2, 16, 28, 6, "s")
paint(16, 4, [
    "    aaaa",
    "  aaaaaaaa",
    " aaaaaaaaaa",
    "aaaaaaaaaaaa",
    "aaaaaaaaaaaa",
    "pppppppppppp",
    " aaaaaaaaaa",
    "  pppppppp",
    "   aaaaaa",
])
fill(2, 21, 28, 4, "m")
fill(2, 25, 28, 4, "g")
paint(9, 21, [
    "      trrt",
    "     trrrrt",
    "    trrarrt",
    "   trrrrrrt",
    "  trrrrrrrt",
    " trrrarrrrt",
    "trrrrarrrrt",
    "trrrrrrrrrt",
])
paint(4, 13, [
    " bbbb",
    "bccbn",
    "bbbnn",
    "bwwnn",
    "bbbnn",
    "bccnn",
    "bbbnn",
    "bwwnn",
    "dddnn",
])
paint(10, 6, [
    "   c",
    "   c",
    "  ccc",
    " bbbbb",
    "cccccbn",
    "bbbbbnn",
    "bcwcbnn",
    "bbbbbnn",
    "bcbcbnn",
    "bbbbbnn",
    "bwcwbnn",
    "bbbbbnn",
    "bcbcbnn",
    "bbbbbnn",
    "bwcwbnn",
    "dddddnn",
])
paint(21, 12, [
    " cccc",
    "ccccbb",
    "bbbbnn",
    "bcwbnn",
    "bbbbnn",
    "bwcbnn",
    "bbbbnn",
    "bccbnn",
    "bbbbnn",
    "ddddnn",
])
paint(24, 19, [
    "  pp",
    " pppp",
    "pppppp",
    "wbbbwn",
    "wbbbwn",
    "dddddn",
])
paint(3, 23, [
    "  m",
    " mmm",
    "mmmmm",
    " ggg",
    "  n",
])
paint(25, 25, [
    "  m",
    " mmm",
    "mmmmm",
    "  g",
])


def svg(rounded):
    radius = ' rx="5"' if rounded else ""
    parts = [
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" shape-rendering="crispEdges">',
        f'<rect width="32" height="32"{radius} fill="#{PALETTE["."]}"/>',
    ]
    for y, row in enumerate(pixels):
        x = 0
        while x < SIZE:
            end = x + 1
            while end < SIZE and row[end] == row[x]:
                end += 1
            if row[x] != ".":
                parts.append(
                    f'<rect x="{x}" y="{y}" width="{end - x}" height="1" fill="#{PALETTE[row[x]]}"/>'
                )
            x = end
    return "\n".join(parts + ["</svg>", ""])


def raster_rows(size, alpha=False):
    scale = size // SIZE
    for row in pixels:
        data = b"".join(
            (bytes.fromhex(PALETTE[colour]) + (b"\xff" if alpha else b"")) * scale
            for colour in row
        )
        yield from [data] * scale


def png(size):
    def chunk(kind, content):
        return (struct.pack(">I", len(content)) + kind + content
                + struct.pack(">I", zlib.crc32(kind + content)))

    return (b"\x89PNG\r\n\x1a\n"
            + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(b"".join(b"\0" + row for row in raster_rows(size)), 9))
            + chunk(b"IEND", b""))


if __name__ == "__main__":
    directory = Path(__file__).resolve().parent
    (directory / "mark.svg").write_text(svg(rounded=True), encoding="utf-8")
    (directory / "icon-square.svg").write_text(svg(rounded=False), encoding="utf-8")
    for size in (256, 1024):
        (directory / f"icon-{size}.png").write_bytes(png(size))
    window_icon = directory.parents[1] / "crates/us-app/src/window-icon-64.rgba"
    window_icon.write_bytes(b"".join(raster_rows(64, alpha=True)))
