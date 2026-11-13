#!/usr/bin/env python3
"""Decode a DevTools PNG and report the facts the acceptance harness checks.

The controller encodes captures as 8-bit RGB/RGBA PNGs. This helper decodes
the IHDR, validates the zlib stream and PNG filters, and prints one JSON
object with dimensions and pixel statistics. Unsupported PNG variants fail
instead of guessing.

Usage:
    png_probe.py CAPTURE.png

Exit status: 0 with JSON on stdout for a supported PNG, 2 for an invalid or
unsupported file.
"""

import json
import struct
import sys
import zlib

SIGNATURE = b"\x89PNG\r\n\x1a\n"
MAX_PIXELS = 40_000_000
MAX_COLORS = 65_536


def fail(message):
    print(json.dumps({"error": message}), file=sys.stderr)
    sys.exit(2)


def paeth(a, b, c):
    p = a + b - c
    pa = abs(p - a)
    pb = abs(p - b)
    pc = abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    if pb <= pc:
        return b
    return c


def main(path):
    try:
        with open(path, "rb") as file:
            data = file.read()
    except OSError as error:
        fail(f"cannot read {path}: {error}")

    if not data.startswith(SIGNATURE):
        fail("not a PNG file")

    header = None
    compressed = bytearray()
    offset = len(SIGNATURE)
    while offset + 12 <= len(data):
        length = struct.unpack(">I", data[offset : offset + 4])[0]
        kind = data[offset + 4 : offset + 8]
        end = offset + 8 + length
        if end + 4 > len(data):
            fail("truncated PNG chunk")
        payload = data[offset + 8 : end]
        stored_crc = struct.unpack(">I", data[end : end + 4])[0]
        actual_crc = zlib.crc32(kind + payload) & 0xFFFFFFFF
        if stored_crc != actual_crc:
            fail(f"invalid CRC for {kind!r} chunk")
        if kind == b"IHDR":
            if length != 13:
                fail("invalid IHDR length")
            header = struct.unpack(">IIBBBBB", payload)
        elif kind == b"IDAT":
            compressed.extend(payload)
        elif kind == b"IEND":
            break
        offset = end + 4

    if header is None:
        fail("missing IHDR")
    width, height, bit_depth, color_type, compression, filter_method, interlace = header
    if width == 0 or height == 0:
        fail("PNG has an empty image")
    if width * height > MAX_PIXELS:
        fail(f"PNG has {width * height} pixels, above the {MAX_PIXELS} pixel budget")
    if bit_depth != 8:
        fail(f"unsupported bit depth {bit_depth}; expected 8")
    if color_type not in (2, 6):
        fail(f"unsupported color type {color_type}; expected RGB or RGBA")
    if compression != 0 or filter_method != 0:
        fail("unsupported PNG compression or filter method")
    if interlace != 0:
        fail("interlaced PNGs are unsupported")
    if not compressed:
        fail("missing IDAT")

    channels = 4 if color_type == 6 else 3
    stride = width * channels
    try:
        raw = zlib.decompress(bytes(compressed))
    except zlib.error as error:
        fail(f"invalid zlib stream: {error}")
    expected = height * (stride + 1)
    if len(raw) != expected:
        fail(f"decompressed size {len(raw)} does not match expected {expected}")

    colors = set()
    opaque = 0
    transparent = 0
    semi = 0
    cursor = 0
    previous = bytearray(stride)
    for _ in range(height):
        filter_type = raw[cursor]
        cursor += 1
        line = bytearray(raw[cursor : cursor + stride])
        cursor += stride
        if filter_type == 0:
            pass
        elif filter_type == 1:
            for index in range(stride):
                left = line[index - channels] if index >= channels else 0
                line[index] = (line[index] + left) & 0xFF
        elif filter_type == 2:
            for index in range(stride):
                line[index] = (line[index] + previous[index]) & 0xFF
        elif filter_type == 3:
            for index in range(stride):
                left = line[index - channels] if index >= channels else 0
                up = previous[index]
                line[index] = (line[index] + ((left + up) >> 1)) & 0xFF
        elif filter_type == 4:
            for index in range(stride):
                left = line[index - channels] if index >= channels else 0
                up = previous[index]
                up_left = previous[index - channels] if index >= channels else 0
                line[index] = (line[index] + paeth(left, up, up_left)) & 0xFF
        else:
            fail(f"unknown PNG filter type {filter_type}")
        previous = line

        for pixel in range(width):
            index = pixel * channels
            red, green, blue = line[index], line[index + 1], line[index + 2]
            alpha = line[index + 3] if channels == 4 else 255
            if len(colors) < MAX_COLORS:
                colors.add((red << 24) | (green << 16) | (blue << 8) | alpha)
            if alpha == 255:
                opaque += 1
            elif alpha == 0:
                transparent += 1
            else:
                semi += 1

    total = width * height
    print(
        json.dumps(
            {
                "width": width,
                "height": height,
                "bitDepth": bit_depth,
                "colorType": "rgba" if color_type == 6 else "rgb",
                "uniqueColors": len(colors),
                "uniqueColorsCapped": len(colors) >= MAX_COLORS,
                "nonUniform": len(colors) > 1,
                "opaquePixels": opaque,
                "transparentPixels": transparent,
                "semiPixels": semi,
                "opaqueRatio": round(opaque / total, 6),
                "pixels": total,
                "bytes": len(data),
            }
        )
    )


if __name__ == "__main__":
    if len(sys.argv) != 2:
        fail("usage: png_probe.py CAPTURE.png")
    main(sys.argv[1])
