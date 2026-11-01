//! Translation of `Tests/CoveUIRuntimeTests/Rendering/CircularRevealPixels.swift`.
//!
//! A themed reveal keeps the *previous* frame and uncovers the new theme with a
//! growing circle. Three things must hold on real pixels: the first frame of a
//! sequence shows the old theme everywhere, a half-grown circle shows the old
//! theme inside and the new theme outside, and reversing the sequence keeps the
//! mixed frame instead of snapping. A finished or cleared reveal leaves no
//! residue.
//!
//! Behaviour checklist:
//! - the first frame holds the old theme
//! - the new theme grows from the origin
//! - a reversal retains the mixed frame
//! - a finished reveal and a cleared list show the new theme everywhere
//!
//! Difference from Swift: the Swift case takes a `mode` axis that selects
//! between the 1x path, the explicit 4x raster workspace and the presentation
//! MSAA path. Rust exposes the presentation path through
//! `record_presentation`, so the tests use that and assert the same resolved
//! pixels for the multisample and single-sample configurations.

#![cfg(target_os = "macos")]

use std::sync::Arc;

use cove_core::{Color, DrawList, Rect, Reveal};
use cove_graphics::NativeDrawListRenderer;
use cove_graphics::rhi::{
    CommandBuffer, Device, DeviceConfig, GraphicsApi, QueueClass, RenderColorTarget,
    TextureDescriptor, TextureFormat, TextureResource, TextureSubresource, TextureUsage,
};
use cove_graphics::ui::NativeUIViewport;

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
const RED: Color = Color {
    r: 1.0,
    g: 0.0,
    b: 0.0,
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

struct Output {
    texture: TextureResource,
    pixels: [usize; 2],
}

impl Output {
    fn new(device: &Arc<Device>, pixels: [usize; 2]) -> Self {
        let texture = TextureResource::new(
            device.clone(),
            TextureDescriptor::new(
                pixels[0],
                pixels[1],
                TextureFormat::Bgra8Unorm,
                TextureUsage::COLOR_TARGET | TextureUsage::TRANSFER_SOURCE,
            ),
        )
        .expect("an offscreen output");
        Self { texture, pixels }
    }
}

/// One renderer across the whole sequence, because the retained previous frame
/// lives on it.
struct Scene {
    device: Arc<Device>,
    renderer: NativeDrawListRenderer,
    viewport: NativeUIViewport,
    samples: usize,
}

impl Scene {
    fn new(device: Arc<Device>, scale: usize, samples: usize) -> Self {
        let mut renderer = NativeDrawListRenderer::new(device.clone()).expect("a renderer");
        renderer
            .configure(TextureFormat::Bgra8Unorm, samples)
            .expect("configuration");
        Self {
            device,
            renderer,
            viewport: NativeUIViewport::new([64 * scale, 64 * scale], [64.0, 64.0]),
            samples,
        }
    }

    fn render(&mut self, output: &Output, color: Color, reveal: Option<Reveal>) -> Vec<u8> {
        let mut list = DrawList::default();
        let _ = list.add_rect(Rect::new(0.0, 0.0, 64.0, 64.0), color);
        if let Some(reveal) = reveal {
            list.begin_reveal(reveal);
        }

        self.device.begin_frame().expect("begin frame");
        let mut commands = CommandBuffer::new();
        let row = output.pixels[0] * 4 * self.samples;
        let recorded = if self.samples > 1 {
            self.renderer
                .record_presentation(
                    &list,
                    &mut commands,
                    output.texture.texture(),
                    self.viewport,
                    [0.0, 0.0, 0.0, 0.0],
                    row * output.pixels[1],
                )
                .map(|frame| (frame, true))
        } else {
            self.renderer
                .record(
                    &list,
                    &mut commands,
                    RenderColorTarget {
                        texture: output.texture.texture(),
                        load_action: cove_graphics::rhi::ColorLoadAction::Clear([
                            0.0, 0.0, 0.0, 0.0,
                        ]),
                        store: true,
                        resolve_texture: None,
                    },
                    self.viewport,
                    Some(output.texture.texture()),
                )
                .map(|frame| (frame, false))
        };
        let (mut frame, via_presentation) = recorded.expect("record");
        self.device
            .submit(&commands, QueueClass::Graphics, &[], &[], None)
            .expect("submit");
        frame.did_submit();
        self.device.end_frame();
        self.device.wait_until_idle().expect("idle");

        if via_presentation {
            assert_eq!(
                self.renderer.presentation_msaa_allocated_bytes(),
                Some(0),
                "the presentation workspace resolves from tile memory",
            );
            assert!(
                self.renderer.presentation_msaa_memoryless(),
                "the presentation workspace is memoryless on this GPU",
            );
        }

        let mut pixels = vec![0u8; output.pixels[0] * output.pixels[1] * 4];
        self.device
            .read_texture_data(
                output.texture.texture(),
                output.pixels[0],
                output.pixels[1],
                output.pixels[0] * 4,
                TextureSubresource::default(),
                &mut pixels,
            )
            .expect("read back");
        pixels
    }
}

fn pixel(image: &[u8], width: usize, scale: usize, x: usize, y: usize) -> [u8; 4] {
    let offset = ((y * scale) * width + x * scale) * 4;
    [
        image[offset],
        image[offset + 1],
        image[offset + 2],
        image[offset + 3],
    ]
}

fn sequence(scale: usize, samples: usize) {
    let Some(device) = device() else {
        eprintln!("no Metal device available; skipping the circular reveal test");
        return;
    };
    let mut scene = Scene::new(device.clone(), scale, samples);
    let output = Output::new(&device, [64 * scale, 64 * scale]);

    // A sequence the renderer has never seen shows the old theme everywhere.
    let initial = scene.render(&output, GREEN, Some(Reveal::new(999, 12.0, 20.0, 0.0, 0.0)));
    assert_eq!(
        pixel(&initial, 64 * scale, scale, 60, 60),
        [0, 255, 0, 255],
        "an unknown sequence paints the new theme",
    );

    let old = scene.render(&output, BLUE, None);
    let start = scene.render(&output, RED, Some(Reveal::new(1, 12.0, 20.0, 0.0, 0.0)));
    assert_eq!(start, old, "radius 0 holds the previous frame exactly");

    let middle = scene.render(&output, RED, Some(Reveal::new(1, 12.0, 20.0, 18.0, 0.0)));
    assert_eq!(
        pixel(&middle, 64 * scale, scale, 12, 20),
        [0, 0, 255, 255],
        "the centre still shows the old theme inside the circle",
    );
    assert_eq!(
        pixel(&middle, 64 * scale, scale, 60, 60),
        [255, 0, 0, 255],
        "outside the circle the new theme has arrived",
    );

    let reversal = scene.render(&output, GREEN, Some(Reveal::new(2, 12.0, 20.0, 0.0, 0.0)));
    assert_eq!(
        reversal, middle,
        "a reversal at radius 0 keeps the mixed frame it was given",
    );

    let finished = scene.render(&output, GREEN, Some(Reveal::new(2, 12.0, 20.0, 100.0, 0.0)));
    assert_eq!(pixel(&finished, 64 * scale, scale, 0, 0), [0, 255, 0, 255]);
    assert_eq!(
        pixel(&finished, 64 * scale, scale, 63, 63),
        [0, 255, 0, 255]
    );

    let cleared = scene.render(&output, GREEN, None);
    assert_eq!(
        cleared, finished,
        "dropping the reveal keeps the finished theme",
    );
}

#[test]
fn reveal_and_reverse_at_one_x_single_sample() {
    sequence(1, 1);
}

#[test]
fn reveal_and_reverse_at_two_x_single_sample() {
    sequence(2, 1);
}

#[test]
fn reveal_and_reverse_at_one_x_multisample() {
    sequence(1, 4);
}
