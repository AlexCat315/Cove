//! Translation of `Tests/CoveUIRuntimeTests/DrawBatchCoalescingTests.swift`.
//!
//! Two halves of the same contract. First, whether a batch may drop its scissor
//! at all: a batch whose triangle lies *mostly* inside the clip rect still needs
//! the scissor, and being one hundredth of a point out must not flip that. Then,
//! given that classification, coalescing same-texture runs must not merge across
//! a clip or a skipped index gap — the gap has to keep what the reference image
//! had there.
//!
//! Behaviour checklist:
//! - clip classification checks every edge and preserves its tolerance
//! - merged draws preserve real clips and skipped index gaps
//!
//! Difference from Swift: `NativeUIGeometry` is private to `cove_graphics::ui`,
//! so the classification cannot be called directly. These drive the real
//! renderer instead and read the resulting pixel and draw-call behaviour, which
//! is what the classification exists to produce.

#![cfg(target_os = "macos")]

use std::sync::Arc;

use cove_core::{Color, DrawList, Rect};
use cove_graphics::rhi::{
    ColorLoadAction, CommandBuffer, Device, DeviceConfig, GraphicsApi, QueueClass,
    RenderColorTarget, TextureDescriptor, TextureFormat, TextureResource, TextureSubresource,
    TextureUsage,
};
use cove_graphics::ui::NativeUIViewport;
use cove_graphics::{NativeDrawListRenderer, NativeUIDrawStatistics};

const RED: Color = Color {
    r: 1.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};
const GREEN: Color = Color {
    r: 0.0,
    g: 1.0,
    b: 0.0,
    a: 1.0,
};
const BLUE: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 1.0,
    a: 1.0,
};

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
}

impl Scene {
    fn new(device: Arc<Device>, pixels: [usize; 2], logical: [f32; 2]) -> Self {
        let mut renderer = NativeDrawListRenderer::new(device.clone()).expect("a renderer");
        renderer
            .configure(TextureFormat::Bgra8Unorm, 1)
            .expect("configuration");
        Self {
            device,
            renderer,
            viewport: NativeUIViewport::new(pixels, logical),
        }
    }

    /// Render `list` offscreen and return its pixels alongside the statistics.
    fn render(&mut self, list: &DrawList) -> (Vec<u8>, NativeUIDrawStatistics) {
        let [width, height] = self.viewport.pixels;
        let output = TextureResource::new(
            self.device.clone(),
            TextureDescriptor::new(
                width,
                height,
                TextureFormat::Bgra8Unorm,
                TextureUsage::COLOR_TARGET | TextureUsage::TRANSFER_SOURCE,
            ),
        )
        .expect("an offscreen target");

        self.device.begin_frame().expect("begin frame");
        let mut commands = CommandBuffer::new();
        let frame = self
            .renderer
            .record(
                list,
                &mut commands,
                RenderColorTarget {
                    texture: output.texture(),
                    load_action: ColorLoadAction::Clear([0.0, 0.0, 0.0, 0.0]),
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
        let mut frame = frame;
        frame.did_submit();
        self.device.end_frame();
        self.device.wait_until_idle().expect("idle");

        let mut pixels = vec![0u8; width * height * 4];
        self.device
            .read_texture_data(
                output.texture(),
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

fn texel(pixels: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
    let offset = (y * width + x) * 4;
    [
        pixels[offset],
        pixels[offset + 1],
        pixels[offset + 2],
        pixels[offset + 3],
    ]
}

#[test]
fn clip_classification_checks_every_edge_and_preserves_its_tolerance() {
    let Some(device) = device() else {
        eprintln!("no Metal device available; skipping the clip classification test");
        return;
    };
    // A 10x10 white fill clipped to its own 10x10 rect can drop the scissor;
    // nudging any corner just outside the boundary must restore it. The
    // tolerance is one hundredth of a point, so -0.009/10.009 still classify as
    // inside and -0.011/10.011 do not.
    let cases: [((f32, f32), bool); 9] = [
        ((5.0, 5.0), false),
        ((-0.009, 5.0), false),
        ((-0.011, 5.0), true),
        ((5.0, -0.009), false),
        ((5.0, -0.011), true),
        ((10.009, 5.0), false),
        ((10.011, 5.0), true),
        ((5.0, 10.009), false),
        ((5.0, 10.011), true),
    ];

    for (position, expected) in cases {
        for corner in [0usize, 5] {
            let mut geometry = DrawList::default();
            geometry.push_clip_rect(Rect::new(0.0, 0.0, 10.0, 10.0));
            let _ = geometry.add_rect(Rect::new(0.0, 0.0, 10.0, 10.0), Color::WHITE);
            let batch = geometry.batches()[0].clone();

            let mut list = DrawList::default();
            let vertices: Vec<cove_core::Vertex> = geometry
                .vertices()
                .iter()
                .copied()
                .enumerate()
                .map(|(index, mut vertex)| {
                    if index == corner {
                        vertex.x = position.0;
                        vertex.y = position.1;
                    }
                    vertex
                })
                .collect();
            list.load(
                vertices,
                geometry.indices().to_vec(),
                vec![batch],
                Default::default(),
            )
            .expect("the modified geometry still validates");

            // A batch that needs the scissor paints only inside the clip; one
            // that does not keeps the escaped corner outside it.
            let mut scene = Scene::new(device.clone(), [32, 24], [10.0, 10.0]);
            let (pixels, _) = scene.render(&list);
            let escaped = texel(&pixels, 32, 32 - 1, 24 - 1);
            if expected {
                assert_eq!(
                    escaped[3], 0,
                    "corner {corner} at {position:?} escapes the clip, so the scissor must apply",
                );
            } else {
                assert_ne!(
                    escaped[3], 0,
                    "corner {corner} at {position:?} stays inside the tolerance, so the scissor may drop",
                );
            }
        }
    }
}

#[test]
fn merged_draws_preserve_real_clips_and_skipped_index_gaps() {
    let Some(device) = device() else {
        eprintln!("no Metal device available; skipping the coalescing test");
        return;
    };
    let viewport = NativeUIViewport::new([32, 24], [32.0, 24.0]);
    let mut scene = Scene::new(device, [32, 24], [32.0, 24.0]);
    let (reference, _) = scene.render(&DrawList::default());

    for variant in 0..3 {
        let mut list = DrawList::default();
        list.push_clip_rect(Rect::new(0.0, 0.0, 8.0, 8.0));
        // Variant 1 draws a rect twice as wide as its clip, which forces the
        // scissor to survive coalescing.
        let _ = list.add_rect(
            Rect::new(0.0, 0.0, if variant == 1 { 16.0 } else { 8.0 }, 8.0),
            RED,
        );
        list.pop_clip_rect();
        if variant == 2 {
            // A zero-width clip contributes no draw and leaves an index gap.
            list.push_clip_rect(Rect::new(8.0, 0.0, 0.0, 8.0));
            let _ = list.add_rect(Rect::new(8.0, 0.0, 8.0, 8.0), BLUE);
            list.pop_clip_rect();
        }
        list.push_clip_rect(Rect::new(16.0, 0.0, 8.0, 8.0));
        let _ = list.add_rect(Rect::new(16.0, 0.0, 8.0, 8.0), GREEN);
        list.pop_clip_rect();

        let (pixels, statistics) = scene.render(&list);
        assert_eq!(
            statistics.draw_calls,
            if variant == 0 { 1 } else { 2 },
            "variant {variant}: coalescing merges only what shares a clip",
        );

        let width = viewport.pixels[0];
        assert_eq!(
            texel(&pixels, width, 4, 4),
            [0, 0, 255, 255],
            "variant {variant}: the red fill is clipped to eight pixels",
        );
        assert_eq!(
            texel(&pixels, width, 12, 4),
            texel(&reference, width, 12, 4),
            "variant {variant}: a skipped index gap keeps the reference pixels",
        );
        assert_eq!(
            texel(&pixels, width, 20, 4),
            [0, 255, 0, 255],
            "variant {variant}: the green fill paints its own clip",
        );
    }
}
