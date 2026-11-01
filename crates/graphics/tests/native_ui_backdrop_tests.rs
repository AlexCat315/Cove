//! Translation of `Tests/CoveUIRuntimeTests/NativeUIBackdropTests.swift`.
//!
//! Frost is a rendering decision, so these tests read pixels: a hard striped
//! background must come out soft inside a frosted panel and stay sharp
//! everywhere else, including when the renderer has nothing to sample.
//!
//! Behaviour checklist:
//! - a frosted panel softens what sits behind it, while the stripes outside stay
//!   sharp
//! - frost fades with the panel's opacity and contributes exactly nothing at
//!   zero, with the half-opacity case sitting between the two
//! - a material panel still transmits the blurred backdrop through its own tint
//! - frost is visible over a flat backdrop too, on both a light and a dark field
//! - the same panel stays sharp when there is no source frame to sample
//! - a panel that never asked for a backdrop is left alone
//! - frost survives MSAA, where the panel samples the resolved frame
//! - a presentation renderer shares texture residency but owns its own effect
//!   scratch, and trimming frees the scratch without losing correctness
//! - a panel's own shadow does not grey the frost it samples
//! - the shadow falloff is smooth, with no ridge where the blur stack steps
//! - frost answers to what is behind it: a light and a dark field differ
//! - content just outside the panel still diffuses into it, and fades before the
//!   far side
//!
//! Named difference (`GAP — capture margin introspection`): Swift's
//! `NativeUIBackdropPass.captureMargin(blurRadius:scale:)` is `internal` and
//! directly asserted (28 at 14pt/1x, 56 at 14pt/2x, 12 at 4pt/1.5x, 0 for a zero
//! and a negative radius). In Rust the equivalent lives in
//! `crates/graphics/src/ui/backdrop_pass.rs` as the private free function
//! `capture_margin`, and `NativeUIBackdropPass` itself is `pub(super)`. Neither
//! is reachable from an integration test, so the margin is asserted through its
//! observable effect instead — the "content just outside the panel still
//! diffuses into it" case, which is exactly what the Swift margin case exists to
//! protect. Not fixed here; reported.
//!
//! `NativeUIDrawTestSupport.swift` is a support file, not a suite; its
//! `NativeUIDrawTestContext` is folded in below as [`Context`].

#![cfg(target_os = "macos")]

use std::sync::Arc;

use cove_core::scene::{Node, NodeRenderer, SurfaceShadow};
use cove_core::{Backdrop, Color, DrawList, Rect};
use cove_graphics::rhi::{
    ColorLoadAction, CommandBuffer, Device, DeviceConfig, GraphicsApi, QueueClass,
    RenderColorTarget, TextureDescriptor, TextureFormat, TextureResource, TextureSubresource,
    TextureUsage,
};
use cove_graphics::ui::{NativeDrawListRenderer, NativeUIDrawStatistics, NativeUIViewport};

const SIZE: usize = 64;

struct Context {
    device: Arc<Device>,
}

impl Context {
    fn new() -> Option<Self> {
        let device = match Device::make(DeviceConfig {
            preferred_backends: vec![GraphicsApi::Metal],
            enable_validation: true,
            frames_in_flight: 3,
            ..DeviceConfig::default()
        }) {
            Ok(device) => device,
            Err(error) => {
                eprintln!("no Metal device available ({error}); skipping");
                return None;
            }
        };
        Some(Self {
            device: Arc::new(device),
        })
    }

    /// A fresh renderer over this context's device, which is what the Swift
    /// helper's `renderer:` parameter does.
    fn renderer(&self) -> NativeDrawListRenderer {
        let mut renderer = NativeDrawListRenderer::new(self.device.clone()).expect("UI renderer");
        renderer
            .configure(TextureFormat::Bgra8Unorm, 1)
            .expect("configure");
        renderer
    }

    fn output(&self, pixels: [usize; 2]) -> TextureResource {
        TextureResource::new(
            self.device.clone(),
            TextureDescriptor::new(
                pixels[0],
                pixels[1],
                TextureFormat::Bgra8Unorm,
                TextureUsage::COLOR_TARGET
                    | TextureUsage::TRANSFER_SOURCE
                    | TextureUsage::TRANSFER_DESTINATION
                    | TextureUsage::SAMPLED,
            ),
        )
        .expect("colour target")
    }

    fn read(&self, resource: &TextureResource) -> Vec<u8> {
        let descriptor = resource.descriptor();
        let size = [descriptor.width, descriptor.height];
        let mut data = vec![0u8; size[0] * size[1] * 4];
        self.device
            .read_texture_data(
                resource.texture(),
                size[0],
                size[1],
                size[0] * 4,
                TextureSubresource::default(),
                &mut data,
            )
            .expect("read back");
        data
    }

    /// One offscreen pass through the given renderer.
    fn image(
        &mut self,
        renderer: &mut NativeDrawListRenderer,
        list: &DrawList,
        pixels: [usize; 2],
        samples: usize,
        backdrop_source: bool,
    ) -> (Vec<u8>, NativeUIDrawStatistics) {
        let viewport = NativeUIViewport::new(pixels, [pixels[0] as f32, pixels[1] as f32]);
        let output = self.output(pixels);
        renderer
            .configure(TextureFormat::Bgra8Unorm, samples)
            .expect("configure");
        let multisample = (samples > 1).then(|| {
            let mut descriptor = TextureDescriptor::new(
                pixels[0],
                pixels[1],
                TextureFormat::Bgra8Unorm,
                TextureUsage::COLOR_TARGET,
            );
            descriptor.sample_count = samples;
            TextureResource::new(self.device.clone(), descriptor).expect("multisample target")
        });

        self.device.begin_frame().expect("begin frame");
        let statistics = {
            let mut commands = CommandBuffer::new();
            let mut frame = renderer
                .record(
                    list,
                    &mut commands,
                    RenderColorTarget {
                        resolve_texture: multisample.as_ref().map(|_| output.texture()),
                        texture: multisample
                            .as_ref()
                            .map_or(output.texture(), TextureResource::texture),
                        load_action: ColorLoadAction::Clear([0.07, 0.09, 0.12, 0.5]),
                        store: multisample.is_none(),
                    },
                    viewport,
                    backdrop_source.then(|| output.texture()),
                )
                .expect("record");
            self.device
                .submit(&commands, QueueClass::Graphics, &[], &[], None)
                .expect("submit");
            frame.did_submit();
            frame.statistics
        };
        self.device.end_frame();
        self.device.wait_until_idle().expect("wait for idle");
        (self.read(&output), statistics)
    }
}

fn panel() -> Rect {
    Rect::new(16.0, 16.0, 32.0, 32.0)
}

/// 8px bars: wide enough that simply reading the frame at a lower resolution
/// still leaves a ripple, so only a real blur can flatten it.
fn striped_list(frosted: bool, fill: Color, opacity: f32) -> DrawList {
    let mut list = DrawList::default();
    let mut x = 0.0_f32;
    while x < SIZE as f32 {
        list.add_rect(
            Rect::new(x, 0.0, 8.0, SIZE as f32),
            Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
        )
        .expect("stripe");
        x += 16.0;
    }
    if frosted {
        let mut backdrop = Backdrop::new(panel());
        backdrop.corner_radius = 4.0;
        backdrop.blur_radius = 8.0;
        backdrop.opacity = opacity;
        list.begin_backdrop(backdrop);
    }
    list.add_rounded_rect(panel(), 4.0, fill)
        .expect("panel fill");
    list
}

/// Peak-to-peak luma across one scanline: 1.0 for the stripes, near 0 once they
/// have been blurred into each other.
fn contrast(pixels: &[u8], row: usize, from: usize, to: usize) -> f32 {
    let mut minimum = 1.0_f32;
    let mut maximum = 0.0_f32;
    for x in from..to {
        let offset = (row * SIZE + x) * 4;
        if offset + 2 >= pixels.len() {
            continue;
        }
        // The target is BGRA8; channel 2 is red, which is what a grey stripe
        // varies in.
        let luma = pixels[offset + 2] as f32 / 255.0;
        minimum = minimum.min(luma);
        maximum = maximum.max(luma);
    }
    maximum - minimum
}

fn clear() -> Color {
    Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.0,
    }
}

#[test]
fn frosted_panel_blurs_backdrop() {
    let Some(mut context) = Context::new() else {
        return;
    };
    let list = striped_list(true, Color::from_rgba8(255, 255, 255, 5), 1.0);
    let (pixels, _) = context.image(&mut context.renderer(), &list, [SIZE, SIZE], 1, true);
    let outside = contrast(&pixels, 8, 0, SIZE);
    let inside = contrast(&pixels, 32, 27, 37);
    assert!(
        outside > 0.8,
        "the stripes stay sharp outside, got {outside}"
    );
    assert!(
        inside < outside * 0.5,
        "frost flattens the stripes inside the panel: {inside} vs {outside}"
    );
}

#[test]
fn frost_composite_opacity() {
    let Some(mut context) = Context::new() else {
        return;
    };
    let mut contrasts = Vec::new();
    for opacity in [0.0_f32, 0.5, 1.0] {
        let list = striped_list(true, clear(), opacity);
        let mut renderer = context.renderer();
        let (pixels, _) = context.image(&mut renderer, &list, [SIZE, SIZE], 1, true);
        contrasts.push(contrast(&pixels, 32, 27, 37));
    }

    let plain_list = striped_list(false, clear(), 1.0);
    let mut renderer = context.renderer();
    let (plain, _) = context.image(&mut renderer, &plain_list, [SIZE, SIZE], 1, true);

    let (sharp, half, frosted) = (contrasts[0], contrasts[1], contrasts[2]);
    assert!(
        (sharp - contrast(&plain, 32, 27, 37)).abs() < 1e-6,
        "zero-opacity frost contributes no pixels: {sharp} vs {}",
        contrast(&plain, 32, 27, 37)
    );
    assert!(
        sharp > half && half > frosted,
        "contrast falls monotonically with opacity: {sharp} > {half} > {frosted}"
    );
    assert!(
        (half - (sharp + frosted) * 0.5).abs() < 0.03,
        "half opacity sits between: {half} vs {}",
        (sharp + frosted) * 0.5
    );
}

#[test]
fn material_panel_transmits_blurred_backdrop() {
    let Some(mut context) = Context::new() else {
        return;
    };
    let material = Color {
        r: 0x2B as f32 / 255.0,
        g: 0x2D as f32 / 255.0,
        b: 0x33 as f32 / 255.0,
        a: 0.5,
    };
    let measure = |context: &mut Context, frosted: bool, fill: Color| -> f32 {
        let list = striped_list(frosted, fill, 1.0);
        let mut renderer = context.renderer();
        let (pixels, _) = context.image(&mut renderer, &list, [SIZE, SIZE], 1, true);
        contrast(&pixels, 32, 27, 37)
    };
    let frosted = measure(&mut context, true, material);
    let plain = measure(&mut context, false, material);
    assert!(
        frosted < plain * 0.5,
        "a material panel still shows the blurred content: {frosted} vs {plain}"
    );
}

#[test]
fn frost_shows_over_a_flat_backdrop() {
    let Some(mut context) = Context::new() else {
        return;
    };
    let material = Color {
        r: 0x2B as f32 / 255.0,
        g: 0x2D as f32 / 255.0,
        b: 0x33 as f32 / 255.0,
        a: 0.5,
    };

    let delta = |context: &mut Context, level: f32| -> f32 {
        let centre = |context: &mut Context, frosted: bool| -> (f32, f32, f32) {
            let mut list = DrawList::default();
            list.add_rect(
                Rect::new(0.0, 0.0, SIZE as f32, SIZE as f32),
                Color {
                    r: level,
                    g: level,
                    b: level,
                    a: 1.0,
                },
            )
            .expect("flat field");
            if frosted {
                let mut backdrop = Backdrop::new(panel());
                backdrop.corner_radius = 4.0;
                backdrop.blur_radius = 8.0;
                list.begin_backdrop(backdrop);
            }
            list.add_rounded_rect(panel(), 4.0, material)
                .expect("panel fill");
            let mut renderer = context.renderer();
            let (pixels, _) = context.image(&mut renderer, &list, [SIZE, SIZE], 1, true);
            let offset = (32 * SIZE + 32) * 4;
            (
                pixels[offset + 2] as f32 / 255.0,
                pixels[offset + 1] as f32 / 255.0,
                pixels[offset] as f32 / 255.0,
            )
        };
        let frosted = centre(context, true);
        let plain = centre(context, false);
        (frosted.0 - plain.0).abs() + (frosted.1 - plain.1).abs() + (frosted.2 - plain.2).abs()
    };

    // Light and dark are the two a demo actually shows, and the two a one-way
    // lift can only ever get right on one of.
    let on_light = delta(&mut context, 0.94);
    let on_dark = delta(&mut context, 0.12);
    assert!(
        on_light > 0.02,
        "frost must change a light backdrop, got {on_light}"
    );
    assert!(
        on_dark > 0.02,
        "frost must change a dark backdrop, got {on_dark}"
    );
}

#[test]
fn without_source_frame_stays_sharp() {
    let Some(mut context) = Context::new() else {
        return;
    };
    let list = striped_list(true, Color::from_rgba8(255, 255, 255, 5), 1.0);
    let mut renderer = context.renderer();
    let (pixels, _) = context.image(&mut renderer, &list, [SIZE, SIZE], 1, false);
    let inside = contrast(&pixels, 32, 20, 44);
    assert!(
        inside > 0.8,
        "with no source the stripes the panel covers are still the stripes the frame drew, got {inside}"
    );
}

#[test]
fn unfrosted_panel_stays_sharp() {
    let Some(mut context) = Context::new() else {
        return;
    };
    let list = striped_list(false, Color::from_rgba8(255, 255, 255, 5), 1.0);
    let mut renderer = context.renderer();
    let (pixels, _) = context.image(&mut renderer, &list, [SIZE, SIZE], 1, true);
    let inside = contrast(&pixels, 32, 20, 44);
    assert!(
        inside > 0.8,
        "a panel that never asked for a backdrop is left alone, got {inside}"
    );
}

#[test]
fn frosted_panel_blurs_under_msaa() {
    let Some(mut context) = Context::new() else {
        return;
    };
    let list = striped_list(true, Color::from_rgba8(255, 255, 255, 5), 1.0);
    let mut renderer = context.renderer();
    let (pixels, _) = context.image(&mut renderer, &list, [SIZE, SIZE], 4, true);
    let outside = contrast(&pixels, 8, 0, SIZE);
    let inside = contrast(&pixels, 32, 27, 37);
    assert!(
        outside > 0.8,
        "outside stays sharp under MSAA, got {outside}"
    );
    assert!(
        inside < outside * 0.5,
        "frost survives MSAA, where the panel samples the resolved frame: {inside} vs {outside}"
    );
}

#[test]
fn presentation_ownership_is_independent() {
    let Some(mut context) = Context::new() else {
        return;
    };
    let base = context.renderer();
    let mut first = base
        .make_presentation_renderer(TextureFormat::Bgra8Unorm, 1)
        .expect("first presentation renderer");
    let mut second = base
        .make_presentation_renderer(TextureFormat::Bgra8Unorm, 1)
        .expect("second presentation renderer");

    let list = striped_list(true, Color::from_rgba8(255, 255, 255, 5), 1.0);
    let (first_image, _) = context.image(&mut first, &list, [SIZE, SIZE], 1, true);
    let (second_image, _) = context.image(&mut second, &list, [SIZE, SIZE], 1, true);
    assert_eq!(first_image, second_image, "two window renderers agree");

    let second_bytes = second.estimated_presentation_texture_bytes();
    assert!(
        first.estimated_presentation_texture_bytes() > 0,
        "the first renderer owns effect scratch"
    );
    assert!(second_bytes > 0, "the second renderer owns its own scratch");

    first.trim_presentation_resources();
    assert_eq!(
        first.estimated_presentation_texture_bytes(),
        0,
        "trimming frees only the first renderer's scratch"
    );
    assert_eq!(
        second.estimated_presentation_texture_bytes(),
        second_bytes,
        "the second renderer's scratch is untouched"
    );

    let (restored, _) = context.image(&mut first, &list, [SIZE, SIZE], 1, true);
    assert_eq!(
        restored, first_image,
        "a trimmed renderer reallocates and renders the same pixels"
    );
}

/// A panel every measurement below shares: frosted, and carrying the layered
/// shadow a floating surface gets.
fn floating_panel(shadow: bool, backdrop_color: Color) -> Vec<u8> {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 128.0, 128.0));
    root.set_background_color(Some(backdrop_color));
    let panel_node = Node::new();
    panel_node.set_frame(Rect::new(48.0, 40.0, 32.0, 32.0));
    panel_node.set_corner_radius(6.0);
    panel_node.set_backdrop_blur(8.0);
    panel_node.set_background_color(Some(Color {
        r: 0x2B as f32 / 255.0,
        g: 0x2D as f32 / 255.0,
        b: 0x33 as f32 / 255.0,
        a: 0.5,
    }));
    if shadow {
        panel_node.set_ambient_shadow(SurfaceShadow {
            color: Some(Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.3,
            }),
            offset_x: 0.0,
            offset_y: 3.0,
            blur: 16.0,
        });
        panel_node.set_shadow(SurfaceShadow {
            color: Some(Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.6,
            }),
            offset_x: 0.0,
            offset_y: 6.0,
            blur: 6.0,
        });
    }
    root.add_child(&panel_node);

    let mut list = DrawList::default();
    NodeRenderer.render(&root, &mut list);

    let Some(mut context) = Context::new() else {
        eprintln!("no Metal device available; skipping");
        return vec![0u8; 128 * 128 * 4];
    };
    let mut renderer = context.renderer();
    let (pixels, _) = context.image(&mut renderer, &list, [128, 128], 1, true);
    pixels
}

fn luma(pixels: &[u8], x: usize, y: usize, width: usize) -> f32 {
    let offset = (y * width + x) * 4;
    if offset + 2 >= pixels.len() {
        return 0.0;
    }
    pixels[offset + 2] as f32 / 255.0
}

fn panel_luma(pixels: &[u8]) -> f32 {
    let mut total = 0.0_f32;
    let mut count = 0_u32;
    for y in 44..68 {
        for x in 52..76 {
            total += luma(pixels, x, y, 128);
            count += 1;
        }
    }
    total / count as f32
}

#[test]
fn shadow_does_not_grey_the_frost() {
    let white = Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    let clean = panel_luma(&floating_panel(false, white));
    let shadowed = panel_luma(&floating_panel(true, white));
    // The shadow falls behind the panel, so the frost must not eat it. What is
    // measured is the leak scaled by how much of the frost the fill lets through,
    // so the bound moves with the fill's alpha.
    assert!(
        clean - shadowed < 0.04,
        "the shadow must stay out of the frost it samples, got {} vs {}",
        clean,
        shadowed
    );
}

#[test]
fn shadow_falloff_is_smooth() {
    let pixels = floating_panel(
        true,
        Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
    );
    let mut deltas = Vec::new();
    for y in 75..92 {
        deltas.push(luma(&pixels, 64, y, 128) - luma(&pixels, 64, y - 1, 128));
    }
    // A coarse stack puts a hard edge every few pixels and the falloff stutters;
    // its second difference peaks around 0.11 against 0.055 for a smooth one.
    let mut ridge = 0.0_f32;
    for index in 1..deltas.len() {
        ridge = ridge.max((deltas[index] - deltas[index - 1]).abs());
    }
    assert!(ridge < 0.07, "the shadow falloff has a ridge of {ridge}");
}

#[test]
fn frost_follows_the_backdrop() {
    let on_white = panel_luma(&floating_panel(
        false,
        Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
    ));
    let on_black = panel_luma(&floating_panel(
        false,
        Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
    ));
    assert!(
        on_white - on_black > 0.15,
        "what makes a panel read as glass is this swing, got {}",
        on_white - on_black
    );
}

/// A dark field with one bright bar ending right at the panel's left edge.
fn edge_content_list(blur_radius: f32) -> DrawList {
    let mut list = DrawList::default();
    list.add_rect(
        Rect::new(0.0, 0.0, SIZE as f32, SIZE as f32),
        Color {
            r: 0.1,
            g: 0.1,
            b: 0.1,
            a: 1.0,
        },
    )
    .expect("dark field");
    list.add_rect(
        Rect::new(10.0, 0.0, 6.0, SIZE as f32),
        Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
    )
    .expect("bright bar");
    let mut backdrop = Backdrop::new(panel());
    backdrop.corner_radius = 4.0;
    backdrop.blur_radius = blur_radius;
    list.begin_backdrop(backdrop);
    list.add_rounded_rect(panel(), 4.0, Color::from_rgba8(255, 255, 255, 5))
        .expect("panel fill");
    list
}

fn band_luma(pixels: &[u8], from: usize, to: usize) -> f32 {
    let mut total = 0.0_f32;
    let mut count = 0_u32;
    for x in from..to {
        total += luma(pixels, x, 32, SIZE);
        count += 1;
    }
    total / count as f32
}

#[test]
fn margin_lets_the_backdrop_reach_in() {
    let Some(mut context) = Context::new() else {
        return;
    };
    let list = edge_content_list(12.0);
    let mut renderer = context.renderer();
    let (pixels, _) = context.image(&mut renderer, &list, [SIZE, SIZE], 1, true);
    let near = band_luma(&pixels, 18, 24);
    let body = band_luma(&pixels, 27, 37);
    let far = band_luma(&pixels, 40, 46);
    // The bright bar ends at the panel's left edge, so the near band can only be
    // brighter if the capture carried it across the boundary.
    assert!(
        near > body + 0.02,
        "content outside the panel diffuses in: near {near} vs body {body}"
    );
    // And it stays a diffusion: the bar fades out well before the far side.
    assert!(
        body > far + 0.02,
        "and fades before the far side: body {body} vs far {far}"
    );
}
