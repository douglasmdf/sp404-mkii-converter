"""Writes app-icon.png (1024x1024): a dark rounded square with a 4x4 pad grid."""
import struct
import sys
import zlib

SIZE = 1024
MARGIN, RADIUS = 64, 180
BODY, PAD, ACCENT = (29, 29, 31, 255), (72, 72, 76, 255), (255, 106, 19, 255)
GRID_START, CELL, GAP = 200, 140, 28


def inside_rounded(x, y):
    lo, hi = MARGIN, SIZE - MARGIN
    if not (lo <= x < hi and lo <= y < hi):
        return False
    cx = min(max(x, lo + RADIUS), hi - RADIUS)
    cy = min(max(y, lo + RADIUS), hi - RADIUS)
    return (x - cx) ** 2 + (y - cy) ** 2 <= RADIUS ** 2


def pixel(x, y):
    if not inside_rounded(x, y):
        return (0, 0, 0, 0)
    gx, gy = x - GRID_START, y - GRID_START
    if 0 <= gx < 4 * (CELL + GAP) and 0 <= gy < 4 * (CELL + GAP):
        col, row = gx // (CELL + GAP), gy // (CELL + GAP)
        if gx % (CELL + GAP) < CELL and gy % (CELL + GAP) < CELL:
            return ACCENT if (col, row) == (0, 3) else PAD
    return BODY


def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)


rows = bytearray()
for y in range(SIZE):
    rows.append(0)
    for x in range(SIZE):
        rows.extend(pixel(x, y))
png = (
    b"\x89PNG\r\n\x1a\n"
    + chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0))
    + chunk(b"IDAT", zlib.compress(bytes(rows), 9))
    + chunk(b"IEND", b"")
)
with open(sys.argv[1] if len(sys.argv) > 1 else "app-icon.png", "wb") as f:
    f.write(png)
