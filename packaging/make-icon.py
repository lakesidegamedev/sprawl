"""Writes a plain PNG placeholder icon (dark square, light road cross), 256x256 unless a size is
given. No alpha channel, so it is valid for the App Store. Standard library only.

    python3 make-icon.py <out.png> [size]
"""
import struct
import sys
import zlib

N = int(sys.argv[2]) if len(sys.argv) > 2 else 256
HALF = max(1, round(14 * N / 256))
rows = []
for y in range(N):
    row = bytearray([0])
    for x in range(N):
        road = abs(x - N // 2) < HALF or abs(y - N // 2) < HALF
        row += bytes((225, 225, 215) if road else (34, 40, 48))
    rows.append(bytes(row))


def chunk(tag, data):
    body = tag + data
    return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)


png = b"\x89PNG\r\n\x1a\n"
png += chunk(b"IHDR", struct.pack(">IIBBBBB", N, N, 8, 2, 0, 0, 0))
png += chunk(b"IDAT", zlib.compress(b"".join(rows), 9))
png += chunk(b"IEND", b"")
open(sys.argv[1], "wb").write(png)
