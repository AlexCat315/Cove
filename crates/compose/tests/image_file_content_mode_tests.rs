//! Translation of `Tests/CoveUIComposeTests/ImageFileContentModeTests.swift`.
//!
//! Behaviour checklist:
//! - `Image::from_file` with `.fit` falls back to the decoded size when no
//!   registry is installed, and letterboxes a 200x100 source into a 100x100 box
//! - the same file with `.fill` covers the box and crops the source with UVs
//! - a missing file emits one `DecodeFailed` diagnostic and uses the empty
//!   texture instead of failing
//! - a missing bundle resource emits a `ResourceNotFound` diagnostic and
//!   reports `NO_TEXTURE`

mod support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::AnyView;
use cove_core::TextureId;
use cove_core::scene::Node;
use cove_core::{Color, DrawList, Point, Rect};
use cove_imaging::{BundleImageResource, ImageAssetRegistryHolder};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;

/// The empty texture id the fallback source paints.
const NO_TEXTURE: TextureId = 0;
use cove_ui_compose::primitives::media::image::Image;
use cove_ui_compose::primitives::media::image_file::{
    ImageLoadDiagnostic, ImageLoadDiagnostics, ImageLoadFailureReason,
};
use cove_ui_compose::primitives::media::image_geometry::{ImageContentMode, ImageRenderingMode};

use support::GlobalTestLock;

/// A minimal opaque RGBA PNG. Written by hand so the suite needs no image
/// encoder: one `IHDR`, one `IDAT` of uncompressed deflate, one `IEND`.
fn write_png(path: &std::path::Path, width: u32, height: u32) -> std::io::Result<()> {
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for byte in data {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
        !crc
    }
    fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut payload = kind.to_vec();
        payload.extend_from_slice(data);
        out.extend_from_slice(&payload);
        out.extend_from_slice(&crc32(&payload).to_be_bytes());
        out
    }

    // Raw scanlines: each row is a filter byte followed by RGBA pixels.
    let mut raw = Vec::with_capacity((width * height * 4 + height) as usize);
    for _ in 0..height {
        raw.push(0u8);
        for _ in 0..width {
            raw.extend_from_slice(&[255, 255, 255, 255]);
        }
    }
    // Uncompressed deflate blocks: BFINAL/BTYPE header, length, then literal bytes.
    let mut zlib = vec![0x78u8, 0x01];
    let mut cursor = 0usize;
    while cursor < raw.len() {
        let end = (cursor + 65_535).min(raw.len());
        let block = &raw[cursor..end];
        let final_block = end == raw.len();
        zlib.push(if final_block { 1 } else { 0 });
        let length = block.len() as u16;
        zlib.extend_from_slice(&length.to_le_bytes());
        zlib.extend_from_slice(&(!length).to_le_bytes());
        zlib.extend_from_slice(block);
        cursor = end;
    }
    let mut adler_a: u32 = 1;
    let mut adler_b: u32 = 0;
    for byte in &raw {
        adler_a = (adler_a + u32::from(*byte)) % 65_521;
        adler_b = (adler_b + adler_a) % 65_521;
    }
    zlib.extend_from_slice(&((adler_b << 16) | adler_a).to_be_bytes());

    let mut png = vec![0x89u8, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    png.extend(chunk(b"IHDR", &ihdr));
    png.extend(chunk(b"IDAT", &zlib));
    png.extend(chunk(b"IEND", &[]));
    std::fs::write(path, png)
}

fn temp_png(width: u32, height: u32) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "coveui-image-file-{}-{}.png",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    write_png(&path, width, height).expect("the fixture PNG is written");
    path
}

/// Draws `image` through its node draw hook and returns the recorded list.
fn draw_image(image: Image, width: f32, height: f32) -> DrawList {
    let graph = support::graph();
    graph.install_view(
        AnyView::new(AnyView::new(image).frame_size(width, height)),
        Default::default(),
    );
    graph.compute_layout(100.0, 100.0);
    let root = graph.tree.borrow().root().expect("root");
    let mut collected = vec![root.clone()];
    let mut index = 0;
    while index < collected.len() {
        collected.extend(collected[index].children());
        index += 1;
    }
    let paint = collected
        .iter()
        .find(|node| node.draw().is_some())
        .expect("the image node installs a draw hook");
    let mut list = DrawList::default();
    (paint.draw().expect("the image installs a draw hook"))(&mut list, Point::default());
    list
}

fn bounds(list: &DrawList) -> (f32, f32, f32, f32) {
    let xs: Vec<f32> = list.vertices().iter().map(|vertex| vertex.x).collect();
    let ys: Vec<f32> = list.vertices().iter().map(|vertex| vertex.y).collect();
    (
        xs.iter().copied().fold(f32::INFINITY, f32::min),
        xs.iter().copied().fold(f32::NEG_INFINITY, f32::max),
        ys.iter().copied().fold(f32::INFINITY, f32::min),
        ys.iter().copied().fold(f32::NEG_INFINITY, f32::max),
    )
}

#[test]
fn fit_uses_the_decoded_size_without_a_registry() {
    GlobalTestLock::locked(|| {
        let png = temp_png(200, 100);
        ImageAssetRegistryHolder::set_current(None);

        let list = draw_image(
            Image::from_file(
                &png,
                100.0,
                100.0,
                Color::WHITE,
                ImageContentMode::Fit,
                ImageRenderingMode::Color,
            ),
            100.0,
            100.0,
        );
        let _ = std::fs::remove_file(&png);

        assert!(
            list.batches()
                .first()
                .is_some_and(|batch| batch.texture_id != NO_TEXTURE),
            "a decoded file binds a real texture"
        );
        assert_eq!(list.resources.len(), 1, "one image resource is retained");
        assert_eq!(list.vertices().len(), 4, "a quad is four vertices");
        let (min_x, max_x, min_y, max_y) = bounds(&list);
        assert_eq!((min_x, max_x), (0.0, 100.0), "`.fit` spans the box width");
        assert_eq!(
            (min_y, max_y),
            (25.0, 75.0),
            "a 2:1 source letterboxes to the middle half of a square box"
        );
    });
}

#[test]
fn fill_covers_the_box_and_crops_the_source() {
    GlobalTestLock::locked(|| {
        let png = temp_png(200, 100);
        ImageAssetRegistryHolder::set_current(None);

        let list = draw_image(
            Image::from_file(
                &png,
                100.0,
                100.0,
                Color::WHITE,
                ImageContentMode::Fill,
                ImageRenderingMode::Color,
            ),
            100.0,
            100.0,
        );
        let _ = std::fs::remove_file(&png);

        assert!(
            list.batches()
                .first()
                .is_some_and(|batch| batch.texture_id != NO_TEXTURE),
            "a decoded file binds a real texture"
        );
        assert_eq!(list.resources.len(), 1, "one image resource is retained");
        assert_eq!(list.vertices().len(), 4, "a quad is four vertices");
        let (min_x, max_x, min_y, max_y) = bounds(&list);
        assert_eq!(
            (min_x, max_x, min_y, max_y),
            (0.0, 100.0, 0.0, 100.0),
            "`.fill` covers the whole box"
        );
        let us: Vec<f32> = list.vertices().iter().map(|vertex| vertex.u).collect();
        assert_eq!(
            (
                us.iter().copied().fold(f32::INFINITY, f32::min),
                us.iter().copied().fold(f32::NEG_INFINITY, f32::max)
            ),
            (10.25, 10.75),
            "filling a 2:1 source into a square crops to the middle half of the texture"
        );
    });
}

#[test]
fn a_decode_failure_emits_a_diagnostic_and_uses_the_empty_texture() {
    GlobalTestLock::locked(|| {
        let events: Rc<RefCell<Vec<ImageLoadDiagnostic>>> = Rc::new(RefCell::new(Vec::new()));
        let recorded = events.clone();
        ImageLoadDiagnostics::set_on_event(Some(Rc::new(move |event| {
            recorded.borrow_mut().push(event.clone());
        })));
        ImageAssetRegistryHolder::set_current(None);

        let image = Image::from_file(
            "/path/that/does/not/exist.png",
            32.0,
            32.0,
            Color::WHITE,
            ImageContentMode::Fit,
            ImageRenderingMode::Color,
        );
        ImageLoadDiagnostics::set_on_event(None);

        assert!(
            !events.borrow().is_empty(),
            "a failed decode reports an event"
        );
        assert!(
            events
                .borrow()
                .iter()
                .any(|event| event.reason == ImageLoadFailureReason::DecodeFailed),
            "the reason is a decode failure"
        );
        assert_eq!(
            image.source.texture_id(),
            NO_TEXTURE,
            "the fallback source paints the empty texture"
        );
    });
}

#[test]
fn a_missing_bundle_resource_emits_a_diagnostic_and_the_empty_texture() {
    GlobalTestLock::locked(|| {
        let events: Rc<RefCell<Vec<ImageLoadDiagnostic>>> = Rc::new(RefCell::new(Vec::new()));
        let recorded = events.clone();
        ImageLoadDiagnostics::set_on_event(Some(Rc::new(move |event| {
            recorded.borrow_mut().push(event.clone());
        })));

        let resource = BundleImageResource::svg(
            format!("definitely-missing-{}", std::process::id()),
            std::env::temp_dir(),
            None,
        );
        let image = Image::from_resource(
            &resource,
            24.0,
            24.0,
            Color::WHITE,
            ImageContentMode::Fit,
            ImageRenderingMode::Color,
        );
        ImageLoadDiagnostics::set_on_event(None);

        assert_eq!(
            image.source.texture_id(),
            NO_TEXTURE,
            "a missing resource falls back to the empty texture"
        );
        assert!(
            events
                .borrow()
                .iter()
                .any(|event| event.reason == ImageLoadFailureReason::ResourceNotFound),
            "the reason is a missing resource, got {:?}",
            events.borrow()
        );
    });
}

const _: fn() = || {
    let _ = Rect::new;
    let _: fn(&Node) = |_| {};
};
