#!/usr/bin/env python3
"""Generate the valid PNG app icon required by Tauri's Linux build."""
from pathlib import Path
import struct
import zlib

size = 256
rows = []
for y in range(size):
    row = bytearray([0])
    for x in range(size):
        edge = min(x, y, size - 1 - x, size - 1 - y)
        bg = (
            min(20 + x // 32, 28),
            min(112 + y // 12, 140),
            min(170 + x // 16, 195),
            255,
        )
        if edge < 10:
            bg = (18, 92, 148, 255)
        mark = (62 <= x <= 91 and 62 <= y <= 190) or (
            165 <= x <= 194 and 62 <= y <= 190
        )
        mark = mark or (
            62 <= x <= 194
            and abs(y - (190 - (x - 62) * 128 / 132)) <= 14
        )
        row.extend((245, 249, 255, 255) if mark else bg)
    rows.append(bytes(row))


def chunk(kind: bytes, data: bytes) -> bytes:
    return (
        struct.pack(">I", len(data))
        + kind
        + data
        + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
    )


png = b"\x89PNG\r\n\x1a\n"
png += chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
png += chunk(b"IDAT", zlib.compress(b"".join(rows), 9))
png += chunk(b"IEND", b"")
target = Path(__file__).resolve().parents[2] / "tauri-web/src-tauri/icons/icon.png"
target.parent.mkdir(parents=True, exist_ok=True)
target.write_bytes(png)
print(f"Generated valid {size}x{size} Tauri icon: {target}")
