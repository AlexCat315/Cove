//! Translation of `Tests/CoveUIRuntimeTests/Rendering/TransientMSAATests.swift`.
//!
//! A transient 4x MSAA attachment may live entirely in tile memory on Apple
//! GPUs. It must resolve to exactly the pixels a persistent attachment would
//! produce, and the two passes that make tile-only storage impossible — loading
//! prior samples, or storing the multisample contents — must be rejected before
//! anything is encoded.
//!
//! Behaviour checklist:
//! - transient attachments preserve resolved UI pixels, at both densities
//! - memoryless attachments reject loading or preserving prior samples
//! - transient storage only accepts multisample attachments
//!
//! Difference from Swift: Swift reads `TextureAllocation` off the
//! `TextureResource` it created. Rust keeps the same information on
//! `TextureResource::allocation()`.

#![cfg(target_os = "macos")]

use std::sync::Arc;

use cove_core::{Color, DrawList, Rect};
use cove_graphics::rhi::{
    ColorLoadAction, CommandBuffer, Device, DeviceConfig, GraphicsApi, QueueClass,
    RenderColorTarget, TextureDescriptor, TextureFormat, TextureResource, TextureSubresource,
    TextureUsage,
};
use cove_graphics::ui::NativeUIViewport;
use cove_graphics::NativeDrawListRenderer;

fn device() -> Option<Arc<Device>> {
    Device::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Metal],
        enable_validation: false,
        ..DeviceConfig::default()
    })
    .ok()
    .map(Arc::new)
}

/// The published scene: a rounded fill, a rounded stroke, a path stroke, a
/// masked rect and a triangle, all at fractional coordinates.
fn scene(scale: f32) -> DrawList {
    let mut list = DrawList::default();
    list.pixel_scale = scale;
    let _ = list.add_rounded_rect(
        Rect::new(1.3, 2.7, 54.2, 29.8),
        8.2,
        Color {
            r: 0.7,
            g: 0.2,
            b: 0.5,
            a: 0.6,
        },
    );
    let _ = list.add_rounded_rect_stroke(
        Rect::new(3.7, 4.2, 48.4, 25.3),
        6.1,
        1.2,
        Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 0.7,
        },
    );
    let _ = list.add_path_stroke(
        &[
            cove_core::Point { x: 2.3, y: 52.8 },
            cove_core::Point { x: 37.6, y: 36.2 },
            cove_core::Point { x: 64.1, y: 59.3 },
        ],
        1.1,
        Color::WHITE,
        false,
    );
    list.push_mask(cove_core::RoundedRect::new(
        Rect::new(59.3, 2.8, 31.4, 28.2),
        7.2,
    ));
    let _ = list.add_rect(
        Rect::new(54.0, 0.0, 42.0, 36.0),
        Color {
            r: 0.1,
            g: 0.8,
            b: 0.5,
            a: 0.8,
        },
    );
    list.pop_mask();
    let _ = list.add_triangle(
        cove_core::Point { x: 67.2, y: 37.3 },
        cove_core::Point { x: 93.7, y: 60.1 },
        cove_core::Point { x: 70.4, y: 62.6 },
        Color {
            r: 0.8,
            g: 0.2,
            b: 0.4,
            a: 1.0,
        },
    );
    list
}

fn target(device: &Arc<Device>, extent: [usize; 2], transient: bool) -> TextureResource {
    let mut descriptor = TextureDescriptor::new(
        extent[0],
        extent[1],
        TextureFormat::Bgra8UnormSrgb,
        TextureUsage::COLOR_TARGET,
    );
    descriptor.sample_count = 4;
    descriptor.transient_attachment = transient;
    TextureResource::new(device.clone(), descriptor).expect("a multisample target")
}

fn resolve(device: &Arc<Device>, extent: [usize; 2]) -> TextureResource {
    TextureResource::new(
        device.clone(),
        TextureDescriptor::new(
            extent[0],
            extent[1],
            TextureFormat::Bgra8UnormSrgb,
            TextureUsage::COLOR_TARGET | TextureUsage::TRANSFER_SOURCE,
        ),
    )
    .expect("a resolve target")
}

fn render(
    device: &Arc<Device>,
    list: &DrawList,
    samples: &TextureResource,
    output: &TextureResource,
    viewport: NativeUIViewport,
) -> Vec<u8> {
    let mut renderer = NativeDrawListRenderer::new(device.clone()).expect("a renderer");
    renderer
        .configure(TextureFormat::Bgra8UnormSrgb, 4)
        .expect("configuration");
    let mut target = RenderColorTarget {
        texture: samples.texture(),
        load_action: ColorLoadAction::Clear([0.07, 0.09, 0.12, 1.0]),
        store: false,
        resolve_texture: Some(output.texture()),
    };
    target.resolve_texture = Some(output.texture());

    device.begin_frame().expect("begin frame");
    let mut commands = CommandBuffer::new();
    let mut frame = renderer
        .record(list, &mut commands, target, viewport, None)
        .expect("record");
    device
        .submit(&commands, QueueClass::Graphics, &[], &[], None)
        .expect("submit");
    frame.did_submit();
    device.end_frame();
    device.wait_until_idle().expect("idle");

    let [width, height] = viewport.pixels;
    let mut pixels = vec![0u8; width * height * 4];
    device
        .read_texture_data(
            output.texture(),
            width,
            height,
            width * 4,
            TextureSubresource::default(),
            &mut pixels,
        )
        .expect("read back");
    pixels
}

#[test]
fn transient_attachments_preserve_resolved_ui_pixels() {
    let Some(device) = device() else {
        eprintln!("no Metal device available; skipping the transient MSAA test");
        return;
    };

    for scale in [1usize, 2] {
        let extent = [96 * scale, 64 * scale];
        let viewport = NativeUIViewport::new(extent, [96.0, 64.0]);
        let list = scene(scale as f32);

        let persistent_samples = target(&device, extent, false);
        let persistent_output = resolve(&device, extent);
        let persistent = render(
            &device,
            &list,
            &persistent_samples,
            &persistent_output,
            viewport,
        );

        let transient_samples = target(&device, extent, true);
        let transient_output = resolve(&device, extent);
        let transient = render(
            &device,
            &list,
            &transient_samples,
            &transient_output,
            viewport,
        );

        assert_eq!(
            persistent, transient,
            "scale {scale}: tile-only storage resolves to the same pixels",
        );

        let normal = persistent_samples
            .allocation()
            .expect("the persistent attachment reports an allocation");
        let temporary = transient_samples
            .allocation()
            .expect("the transient attachment reports an allocation");
        assert!(
            normal.allocated_bytes > 0 && !normal.is_memoryless,
            "the persistent attachment owns device memory",
        );
        if temporary.is_memoryless {
            assert_eq!(temporary.allocated_bytes, 0, "tile-only storage allocates nothing");
        }
        assert!(
            temporary.is_memoryless,
            "Apple GPUs back a transient attachment with tile memory",
        );
    }
}

#[test]
fn memoryless_attachments_reject_loading_or_preserving_prior_samples() {
    let Some(device) = device() else {
        eprintln!("no Metal device available; skipping the memoryless rejection test");
        return;
    };
    let extent = [32usize, 32];
    let viewport = NativeUIViewport::new(extent, [32.0, 32.0]);
    let samples = target(&device, extent, true);
    if samples.allocation().is_none_or(|allocation| !allocation.is_memoryless) {
        return;
    }
    let output = resolve(&device, extent);

    let mut list = DrawList::default();
    let _ = list.add_rect(Rect::new(0.0, 0.0, 32.0, 32.0), Color::WHITE);

    for (load_action, store) in [
        (ColorLoadAction::Load, false),
        (ColorLoadAction::Clear([0.0, 0.0, 0.0, 0.0]), true),
    ] {
        let mut renderer = NativeDrawListRenderer::new(device.clone()).expect("a renderer");
        renderer
            .configure(TextureFormat::Bgra8Unorm, 4)
            .expect("configuration");
        let target = RenderColorTarget {
            texture: samples.texture(),
            load_action,
            store,
            resolve_texture: Some(output.texture()),
        };

        device.begin_frame().expect("begin frame");
        let mut commands = CommandBuffer::new();
        let recorded = renderer.record(&list, &mut commands, target, viewport, None);
        let submitted = recorded.and_then(|mut frame| {
            let result = device.submit(&commands, QueueClass::Graphics, &[], &[], None);
            frame.did_submit();
            result
        });
        device.end_frame();
        assert!(
            submitted.is_err(),
            "a memoryless attachment cannot load ({load_action:?}) or store ({store}) samples",
        );
    }
}

#[test]
fn transient_storage_only_accepts_multisample_attachments() {
    let Some(device) = device() else {
        eprintln!("no Metal device available; skipping the transient texture test");
        return;
    };

    // A single-sample target has nothing to discard, so tile-only storage is a
    // contradiction.
    let mut single = TextureDescriptor::new(
        32,
        32,
        TextureFormat::Bgra8Unorm,
        TextureUsage::COLOR_TARGET,
    );
    single.transient_attachment = true;
    assert!(
        TextureResource::new(device.clone(), single).is_err(),
        "transient storage requires multisampling",
    );

    // Nor may a transient attachment be read back later.
    let mut transferable = TextureDescriptor::new(
        32,
        32,
        TextureFormat::Bgra8Unorm,
        TextureUsage::COLOR_TARGET | TextureUsage::TRANSFER_SOURCE,
    );
    transferable.sample_count = 4;
    transferable.transient_attachment = true;
    assert!(
        TextureResource::new(device.clone(), transferable).is_err(),
        "transient storage is attachment-only",
    );
}
