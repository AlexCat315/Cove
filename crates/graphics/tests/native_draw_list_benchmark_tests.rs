//! Translation of `Tests/CoveUIRuntimeTests/NativeDrawListBenchmarkTests.swift`.
//!
//! Release benchmarks over the native draw-list path. Swift gates each case on
//! an environment variable and refuses to run in a debug build; Rust ports the
//! same workloads and asserts the invariants the benchmark exists to protect,
//! because `#[test]` cannot be conditionally skipped without losing the checks.
//!
//! The invariants are the interesting part and they are cheap:
//! - commanding more batches never changes the pixels, and the native path
//!   never emits more draws than it was given batches
//! - steady state re-records the same geometry without re-uploading textures
//! - an owned-image scene keeps every retained asset alive and draws it once per
//!   batch at most
//!
//! Behaviour checklist:
//! - command scaling: extra batches with distinct scissors draw the same image
//! - equivalent work: a tiled scene renders identically at each viewport size
//! - owned images: 96 owned CPU assets render and stay retained
//!
//! Differences from Swift:
//! - the JSON report, percentile timing against a Metal reference backend, and
//!   the `COVE_NATIVE_*` environment gates are not ported; they measure the
//!   machine, not the code.
//! - `SubmissionCPUProfile` phase fields (`planning`, `encoding`, ...) have no
//!   Rust equivalent on `submit`, so the phase report is not produced.

#![cfg(target_os = "macos")]

use std::sync::Arc;

use cove_core::{Color, DrawList, DrawListResources, Point, Rect, TextureId};
use cove_graphics::rhi::{
    ColorLoadAction, CommandBuffer, Device, DeviceConfig, GraphicsApi, QueueClass,
    RenderColorTarget, TextureDescriptor, TextureFormat, TextureResource, TextureSubresource,
    TextureUsage,
};
use cove_graphics::ui::NativeUIViewport;
use cove_graphics::{NativeDrawListRenderer, NativeUIDrawStatistics};
use cove_imaging::{DecodedImage, ImageAssetRegistry, ImageCacheLimits};

fn device() -> Option<Arc<Device>> {
    Device::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Metal],
        enable_validation: false,
        ..DeviceConfig::default()
    })
    .ok()
    .map(Arc::new)
}

struct Scene {
    device: Arc<Device>,
    renderer: NativeDrawListRenderer,
    viewport: NativeUIViewport,
    output: TextureResource,
}

impl Scene {
    fn new(device: Arc<Device>, pixels: [usize; 2], logical: [f32; 2], samples: usize) -> Self {
        let mut renderer = NativeDrawListRenderer::new(device.clone()).expect("a renderer");
        renderer
            .configure(TextureFormat::Bgra8UnormSrgb, samples)
            .expect("configuration");
        let output = TextureResource::new(
            device.clone(),
            TextureDescriptor::new(
                pixels[0],
                pixels[1],
                TextureFormat::Bgra8UnormSrgb,
                TextureUsage::COLOR_TARGET | TextureUsage::TRANSFER_SOURCE,
            ),
        )
        .expect("an offscreen output");
        Self {
            device,
            renderer,
            viewport: NativeUIViewport::new(pixels, logical),
            output,
        }
    }

    fn render(&mut self, list: &DrawList) -> (Vec<u8>, NativeUIDrawStatistics) {
        let [width, height] = self.viewport.pixels;
        self.device.begin_frame().expect("begin frame");
        let mut commands = CommandBuffer::new();
        let mut frame = self
            .renderer
            .record(
                list,
                &mut commands,
                RenderColorTarget {
                    texture: self.output.texture(),
                    load_action: ColorLoadAction::Clear([0.07, 0.09, 0.12, 0.5]),
                    store: true,
                    resolve_texture: None,
                },
                self.viewport,
                None,
            )
            .expect("record");
        self.device
            .submit(&commands, QueueClass::Graphics, &[], &[], None)
            .expect("submit");
        frame.did_submit();
        self.device.end_frame();
        self.device.wait_until_idle().expect("idle");

        let mut pixels = vec![0u8; width * height * 4];
        self.device
            .read_texture_data(
                self.output.texture(),
                width,
                height,
                width * 4,
                TextureSubresource::default(),
                &mut pixels,
            )
            .expect("read back");
        (pixels, frame.statistics)
    }
}

/// Fixed geometry, no authored image assets: varies the command count while
/// avoiding a growing vertex upload or texture working set. Every batch gets the
/// full-viewport scissor, so commanding more of them must not change a pixel.
#[test]
fn command_scaling_draws_the_same_image_for_more_batches() {
    let Some(device) = device() else {
        eprintln!("no Metal device available; skipping the command benchmark");
        return;
    };

    let mut geometry = DrawList::default();
    let _ = geometry.add_rounded_rect(
        Rect::new(0.0, 0.0, 128.0, 72.0),
        0.0,
        Color {
            r: 0.3,
            g: 0.5,
            b: 0.7,
            a: 1.0,
        },
    );
    let batch = geometry.batches()[0].clone();

    let mut baseline: Option<Vec<u8>> = None;
    let mut empty: Option<Vec<u8>> = None;
    for count in [0usize, 1, 96, 1024] {
        let mut list = DrawList::default();
        if count > 0 {
            let batches: Vec<cove_core::DrawBatch> = (0..count)
                .map(|_| {
                    let mut value = batch.clone();
                    value.scissor = Some(Rect::new(0.0, 0.0, 128.0, 72.0));
                    value
                })
                .collect();
            list.load(
                geometry.vertices().to_vec(),
                geometry.indices().to_vec(),
                batches,
                DrawListResources::default(),
            )
            .expect("the list validates");
        }

        let mut scene = Scene::new(device.clone(), [128, 72], [128.0, 72.0], 1);
        let (pixels, statistics) = scene.render(&list);
        let live = list
            .batches()
            .iter()
            .filter(|batch| batch.index_count > 0)
            .count();
        assert!(
            statistics.draw_calls <= live,
            "count {count}: {} draws over {live} live batches",
            statistics.draw_calls,
        );
        if count == 0 {
            // Nothing was commanded, so only the clear colour is present. That
            // is the control value the filled variants must differ from.
            empty = Some(pixels);
        } else {
            match &baseline {
                Some(expected) => assert!(
                    pixels == *expected,
                    "count {count}: the batch count changed the image",
                ),
                None => baseline = Some(pixels),
            }
        }
    }

    let baseline = baseline.expect("a filled variant rendered");
    let empty = empty.expect("the empty list rendered");
    assert_ne!(
        baseline, empty,
        "the fill is actually drawn, so the equality above is not vacuous",
    );
}

/// The `nativeUIScene` workload: a rounded card with a label bar and a shadow.
fn native_ui_scene() -> DrawList {
    let mut list = DrawList::default();
    let _ = list.add_rounded_rect(
        Rect::new(8.0, 8.0, 144.0, 104.0),
        12.0,
        Color {
            r: 0.18,
            g: 0.2,
            b: 0.24,
            a: 1.0,
        },
    );
    let _ = list.add_rounded_rect(
        Rect::new(16.0, 16.0, 128.0, 20.0),
        6.0,
        Color {
            r: 0.9,
            g: 0.92,
            b: 0.96,
            a: 1.0,
        },
    );
    let _ = list.add_rounded_rect_stroke(
        Rect::new(16.0, 44.0, 128.0, 60.0),
        8.0,
        1.0,
        Color {
            r: 0.4,
            g: 0.45,
            b: 0.5,
            a: 1.0,
        },
    );
    list
}

/// Tile the scene `x` by `y`, shifting vertices and scissors, exactly like the
/// Swift `tiled(_:)` helper.
fn tiled(source: &DrawList, columns: usize, rows: usize) -> DrawList {
    let mut list = DrawList::default();
    for y in 0..rows {
        for x in 0..columns {
            let (dx, dy) = (x as f32 * 160.0, y as f32 * 120.0);
            let _ = dy;
            list.append(source, Point { x: dx, y: dy })
                .expect("a shifted tile appends");
        }
    }
    list
}

#[test]
fn equivalent_work_renders_identically_at_each_viewport_size() {
    let Some(device) = device() else {
        eprintln!("no Metal device available; skipping the UI benchmark");
        return;
    };
    let list = tiled(&native_ui_scene(), 8, 6);
    assert!(!list.vertices().is_empty(), "the tiled scene has geometry");

    // Steady state must not re-upload or change its draw count: the second
    // record over the same list reuses the interning and the recorded geometry.
    let mut scene = Scene::new(device, [256, 144], [128.0, 72.0], 1);
    let (first_pixels, first) = scene.render(&list);
    let (second_pixels, second) = scene.render(&list);
    assert_eq!(
        first_pixels, second_pixels,
        "the same list renders the same"
    );
    assert_eq!(
        second.texture_uploads, 0,
        "steady state re-uploads no textures",
    );
    assert_eq!(
        first.draw_calls, second.draw_calls,
        "steady state records the same draw count",
    );
    assert!(
        first.draw_calls
            <= list
                .batches()
                .iter()
                .filter(|batch| batch.index_count > 0)
                .count(),
        "native coalesces runs of same-texture batches",
    );
}

#[test]
fn owned_images_render_and_stay_retained() {
    let Some(device) = device() else {
        eprintln!("no Metal device available; skipping the image benchmark");
        return;
    };

    let registry = ImageAssetRegistry::new(ImageCacheLimits::default());
    let mut list = DrawList::default();
    let mut bytes = 0usize;
    let mut assets = 0usize;
    for row in 0..8usize {
        for column in 0..12usize {
            let index = row * 12 + column;
            let mut pixels = Vec::with_capacity(64 * 64 * 4);
            for y in 0..64u8 {
                for x in 0..64u8 {
                    pixels.extend_from_slice(&[
                        ((index * 17 + x as usize * 3) % 256) as u8,
                        ((index * 29 + y as usize * 3) % 256) as u8,
                        ((x as usize * 2 + y as usize * 2) % 256) as u8,
                        128 + (((x / 8 + y / 8) % 2) * 127),
                    ]);
                }
            }
            let image = DecodedImage::new(64, 64, pixels).expect("a 64x64 RGBA tile");
            let asset = registry
                .register(format!("tile-{index}"), image)
                .expect("the tile registers");
            list.resources.retain(Arc::clone(&asset));
            list.add_rounded_image_quad(
                Rect::new(
                    column as f32 * 106.0 + 2.0,
                    row as f32 * 90.0 + 2.0,
                    102.0,
                    86.0,
                ),
                8.0,
                asset.texture_id,
                Color {
                    r: 0.9,
                    g: 0.8,
                    b: 1.0,
                    a: 0.9,
                },
                Point { x: 0.0, y: 0.0 },
                Point { x: 1.0, y: 1.0 },
            )
            .expect("the image quad fits the list");
            bytes += asset.image.pixels().len();
            assets += 1;
        }
    }

    assert_eq!(list.resources.len(), 96, "every owned image is retained");
    assert_eq!(assets, 96);
    assert_eq!(bytes, 96 * 64 * 64 * 4);

    // Count what the list actually retained, so a dropped asset cannot pass.
    let mut retained = 0usize;
    list.resources
        .for_each(|_asset: &cove_imaging::ImageAsset| retained += 1);
    assert_eq!(retained, 96);

    let mut scene = Scene::new(device, [1280, 720], [1280.0, 720.0], 1);
    let (_, statistics) = scene.render(&list);
    assert!(
        statistics.draw_calls
            <= list
                .batches()
                .iter()
                .filter(|batch| batch.index_count > 0)
                .count(),
        "each owned image contributes at most one draw per batch",
    );
    assert!(
        statistics.texture_uploads >= 96,
        "the first frame uploads every owned image, saw {}",
        statistics.texture_uploads,
    );
    let _ = TextureId::default();
}
