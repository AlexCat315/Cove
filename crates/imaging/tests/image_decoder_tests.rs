//! Translation of `Tests/CoveUIRuntimeTests/ImageDecoderTests.swift`.
//!
//! Coverage for `ImageDecoder` — vector thumbnails, bounded raster thumbnails,
//! a bitmap round-trip through a real encoded file, and the error surface for
//! missing files and unsupported sources.
//!
//! The Swift suite encoded its PNG through ImageIO and decoded it through
//! `ImageDecoder.decode(url:)`. There is no encoder in this crate, so the round
//! trip writes the PNG container directly (IHDR/IDAT/IEND with a stored-deflate
//! zlib stream) and then decodes it through the same production path. That is
//! still a real round trip: nothing about the decoder is stubbed.
//!
//! Alpha note: Swift's fixture was premultiplied because it fed CoreGraphics,
//! and `ImageDecoder.swift` uses the CoreGraphics path for PDF/HEIC/TIFF. PNG
//! takes the stb_image path, which round-trips straight alpha, so the fixture
//! stores straight RGBA and the decoder is expected to return it unchanged —
//! the same straight-alpha contract the Swift assertion described.
//!
//! Behaviour checklist:
//! - vector thumbnails rasterize above their natural dimensions for HiDPI
//! - an SVG thumbnail rejects an oversized source budget
//! - an SVG thumbnail rejects out-of-range target bounds
//! - thumbnails preserve aspect ratio and reject oversized sources before allocation
//! - a PNG round-trip preserves size and straight-alpha pixels
//! - a missing file raises `FileNotFound`

use std::path::PathBuf;

use cove_imaging::{ImageDecodeError, ImageDecoder, PixelSize};

fn svg(width: u32, height: u32, body: &str) -> Vec<u8> {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\">{body}</svg>"
    )
    .into_bytes()
}

#[test]
fn vector_thumbnail_density() {
    let source = svg(24, 16, "<circle cx=\"12\" cy=\"8\" r=\"7\" fill=\"red\"/>");
    let image = ImageDecoder::decode_thumbnail(
        &source,
        "svg",
        PixelSize {
            width: 144,
            height: 144,
        },
        ImageDecoder::DEFAULT_SOURCE_PIXELS,
    )
    .expect("the vector decodes as a thumbnail");
    assert_eq!((image.width(), image.height()), (144, 96));

    let over_budget = ImageDecoder::decode_thumbnail(
        &source,
        "svg",
        PixelSize {
            width: 144,
            height: 144,
        },
        1_000,
    );
    assert!(
        matches!(over_budget, Err(ImageDecodeError::DecodeFailure(_))),
        "an oversized source budget is rejected: {over_budget:?}",
    );

    let over_bounds = ImageDecoder::decode_thumbnail(
        &source,
        "svg",
        PixelSize {
            width: 9_000,
            height: 144,
        },
        ImageDecoder::DEFAULT_SOURCE_PIXELS,
    );
    assert!(
        matches!(over_bounds, Err(ImageDecodeError::DecodeFailure(_))),
        "target bounds above the supported maximum are rejected: {over_bounds:?}",
    );
}

#[test]
fn bounded_thumbnail() {
    let source = svg(
        240,
        160,
        "<rect width=\"240\" height=\"160\" fill=\"red\"/>",
    );
    let image = ImageDecoder::decode_thumbnail(
        &source,
        "svg",
        PixelSize {
            width: 120,
            height: 120,
        },
        ImageDecoder::DEFAULT_SOURCE_PIXELS,
    )
    .expect("the bounded thumbnail decodes");
    assert_eq!((image.width(), image.height()), (120, 80));
    assert_eq!(
        image.pixels().len(),
        120 * 80 * 4,
        "the bitmap is exactly one RGBA8 buffer for its size",
    );

    assert!(
        ImageDecoder::decode_thumbnail(
            &source,
            "svg",
            PixelSize {
                width: 120,
                height: 120,
            },
            10,
        )
        .is_err(),
        "a source larger than the pixel budget is rejected before allocation",
    );
}

#[test]
fn png_round_trip() {
    const WIDTH: u32 = 4;
    const HEIGHT: u32 = 3;

    // Column 0 opaque red, column 1 opaque green, column 2 opaque blue, column 3
    // half-transparent yellow in straight alpha.
    let mut straight = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
    for y in 0..HEIGHT as usize {
        for x in 0..WIDTH as usize {
            let index = (y * WIDTH as usize + x) * 4;
            let pixel: [u8; 4] = match x {
                0 => [255, 0, 0, 255],
                1 => [0, 255, 0, 255],
                2 => [0, 0, 255, 255],
                _ => [255, 255, 0, 128],
            };
            straight[index..index + 4].copy_from_slice(&pixel);
        }
    }

    let path = temp_png(WIDTH, HEIGHT, &straight);
    let decoded = ImageDecoder::decode(&path, None).expect("the written PNG decodes");
    let _ = std::fs::remove_file(&path);

    assert_eq!(decoded.width(), WIDTH);
    assert_eq!(decoded.height(), HEIGHT);
    assert_eq!(decoded.pixels().len(), (WIDTH * HEIGHT * 4) as usize);

    assert_eq!(
        &decoded.pixels()[0..4],
        &[255, 0, 0, 255],
        "the opaque red pixel survives intact",
    );

    let yellow = 3 * 4;
    assert_eq!(
        decoded.pixels()[yellow + 3],
        128,
        "the half-transparent alpha survives",
    );
    for (channel, name) in [(0, "red"), (1, "green")] {
        let actual = decoded.pixels()[yellow + channel];
        assert!(
            (i32::from(actual) - 255).abs() <= 2,
            "the straight-alpha {name} channel is ~255, got {actual}",
        );
    }
    assert!(
        decoded.pixels()[yellow + 2] <= 2,
        "the straight-alpha blue channel is ~0, got {}",
        decoded.pixels()[yellow + 2],
    );
}

#[test]
fn missing_file_raises() {
    let path = std::env::temp_dir().join("this-file-should-not-exist-cove-image-decoder.png");
    let _ = std::fs::remove_file(&path);
    assert!(
        matches!(
            ImageDecoder::decode(&path, None),
            Err(ImageDecodeError::FileNotFound(_))
        ),
        "a missing file reports FileNotFound",
    );
}

// MARK: - PNG container writer
//
// `Bitmap` has no encoder, so the round trip needs a container. A stored-deflate
// zlib stream is valid PNG IDAT data and keeps the writer honest: anything the
// decoder accepts here it accepts from a real file on disk.

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0_u32; 256];
    for (index, slot) in table.iter_mut().enumerate() {
        let mut value = index as u32;
        for _ in 0..8 {
            value = if value & 1 != 0 {
                0xedb8_8320 ^ (value >> 1)
            } else {
                value >> 1
            };
        }
        *slot = value;
    }
    let mut crc = 0xffff_ffff_u32;
    for byte in data {
        crc = table[((crc ^ u32::from(*byte)) & 0xff) as usize] ^ (crc >> 8);
    }
    crc ^ 0xffff_ffff
}

fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 12);
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_input = kind.to_vec();
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
    out
}

fn zlib_stored(raw: &[u8]) -> Vec<u8> {
    let mut stream = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65_535).collect();
    for (index, block) in blocks.iter().enumerate() {
        stream.push(u8::from(index + 1 == blocks.len()));
        stream.extend_from_slice(&(block.len() as u16).to_le_bytes());
        stream.extend_from_slice(&(!(block.len() as u16)).to_le_bytes());
        stream.extend_from_slice(block);
    }
    let (mut low, mut high) = (1_u32, 0_u32);
    for byte in raw {
        low = (low + u32::from(*byte)) % 65_521;
        high = (high + low) % 65_521;
    }
    stream.extend_from_slice(&((high << 16) | low).to_be_bytes());
    stream
}

fn temp_png(width: u32, height: u32, rgba: &[u8]) -> PathBuf {
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    // 8-bit depth, colour type 6 (RGBA), deflate, adaptive filtering, no interlace.
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);

    let mut raw = Vec::with_capacity(((width as usize * 4) + 1) * height as usize);
    for row in 0..height as usize {
        raw.push(0);
        let start = row * width as usize * 4;
        raw.extend_from_slice(&rgba[start..start + width as usize * 4]);
    }

    let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    png.extend(chunk(b"IHDR", &ihdr));
    png.extend(chunk(b"IDAT", &zlib_stored(&raw)));
    png.extend(chunk(b"IEND", &[]));

    let directory = std::env::temp_dir().join(format!("cove-image-decoder-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("creates the temporary directory");
    let path = directory.join(format!("round-trip-{width}x{height}.png"));
    std::fs::write(&path, &png).expect("writes the temporary PNG");
    path
}
