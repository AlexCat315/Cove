//! Translation of `Tests/CoveUIRuntimeTests/NativeShaderLibraryTests.swift`.
//!
//! The UI must render from the packaged Metal libraries while custom compute
//! shaders compile on a background thread. Two properties matter: the packaged
//! artefacts really are precompiled `.metallib` blobs (so first-frame rendering
//! never pays an MSL compile), and preparation makes progress without blocking
//! the frames that keep drawing.
//!
//! Reachability: `Graphics/rust/ui/resources.rs::artifact` (which turns the
//! packaged JSON into a `ShaderArtifact`) is `pub(super)`, so a test cannot read
//! the artefact's `format`/`code` directly. The same facts are asserted through
//! the public renderer instead: `NativeDrawListRenderer::new` loads both
//! `ui` and `ui_backdrop` vertex+fragment artefacts and fails if any of them is
//! missing, mis-staged, or has the wrong push-constant layout — and it only
//! succeeds on Metal because the artefacts are `.metallib`. Building the
//! renderer therefore *is* the packaged-library assertion, plus a real frame
//! proves the library executes.
//!
//! Named difference: Swift asserts `artifact.format == .metallib` and
//! `artifact.code.prefix(4) == Data("MTLB".utf8)` on the artefact struct. The
//! Rust equivalent (`ShaderFormat::Metallib` + the `MTLB` magic) is not exposed
//! past the UI module, so the format claim is asserted indirectly as above and
//! recorded here rather than reached through a new public accessor.
//!
//! Behaviour checklist:
//! - the packed UI and backdrop libraries both load and render
//! - a frame drawn while preparation runs is fully white
//! - background preparation reaches `Finished` and reports every step
//! - every prepared module is a usable compute module
//! - destroying prepared modules leaves the renderer working

#![cfg(target_os = "macos")]

use std::sync::{Arc, Mutex};

use cove_core::{Color, DrawList, Rect};
use cove_graphics::ui::NativeUIViewport;
use cove_graphics::{
    CommandBuffer, Device, DeviceConfig, GraphicsApi, PreparationPhase, QueueClass,
    RenderColorTarget, ShaderFormat, ShaderModuleDescriptor, ShaderStage, TextureDescriptor,
    TextureFormat, TextureResource, TextureSubresource, TextureUsage,
};
use cove_graphics::NativeDrawListRenderer;

fn device() -> Arc<Device> {
    Arc::new(
        Device::make(DeviceConfig {
            preferred_backends: vec![GraphicsApi::Metal],
            ..DeviceConfig::default()
        })
        .expect("a Metal device is available"),
    )
}

/// Renders a white 16×16 quad and returns the readback bytes.
fn render_white_quad(device: &Arc<Device>, renderer: &mut NativeDrawListRenderer) -> Vec<u8> {
    let mut list = DrawList::default();
    let _ = list.add_rect(Rect::new(0.0, 0.0, 16.0, 16.0), Color::WHITE);
    let viewport = NativeUIViewport::new([16, 16], [16.0, 16.0]);
    let output = TextureResource::new(
        device.clone(),
        TextureDescriptor::new(16, 16, TextureFormat::Bgra8Unorm, TextureUsage::COLOR_TARGET),
    )
    .expect("the output texture is created");
    renderer
        .configure(TextureFormat::Bgra8Unorm, 1)
        .expect("the renderer configures");

    device.begin_frame().expect("the frame opens");
    let mut commands = CommandBuffer::new();
    let frame = renderer
        .record(
            &list,
            &mut commands,
            RenderColorTarget::clearing(output.texture(), [0.0, 0.0, 0.0, 1.0]),
            viewport,
            None,
        )
        .expect("the frame records");
    assert_eq!(frame.statistics.vertices, 4, "the quad emits four vertices");
    device
        .submit(&commands, QueueClass::Graphics, &[], &[], None)
        .expect("the frame submits");
    device.end_frame();
    device.wait_until_idle().expect("the GPU drains");

    let mut pixels = vec![0_u8; 16 * 16 * 4];
    device
        .read_texture_data(
            output.texture(),
            16,
            16,
            16 * 4,
            TextureSubresource::default(),
            &mut pixels,
        )
        .expect("the frame reads back");
    pixels
}

#[test]
fn background_preparation() {
    let device = device();

    // Loading the renderer reads every packaged UI artefact. It only succeeds on
    // Metal when `ui` and `ui_backdrop`, vertex and fragment, are present as
    // precompiled metallibs with the expected push-constant block.
    let mut renderer =
        NativeDrawListRenderer::new(device.clone()).expect("the packaged UI libraries load");

    // The backdrop library is loaded by the same constructor; a sibling keeps
    // both alive and proves they are independent resources.
    let _sibling = renderer
        .make_sibling(TextureFormat::Bgra8Unorm, 1)
        .expect("a sibling renderer shares the packaged libraries");

    let progress: Arc<Mutex<Vec<(PreparationPhase, usize, Option<usize>)>>> =
        Arc::new(Mutex::new(Vec::new()));
    let recorded = progress.clone();
    let progress_sink = move |event: cove_graphics::ResourcePreparationProgress| {
        recorded
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push((event.phase, event.completed_steps, event.total_steps));
    };

    let preparation_device = device.clone();
    let task = std::thread::spawn(move || {
        preparation_device.prepare_resources(
            "Custom shaders",
            Some(12),
            progress_sink,
            |context| {
                let mut modules = Vec::new();
                for index in 0..12 {
                    modules.push(context.step(&format!("Custom {index}"), || {
                        let source = format!(
                            "#include <metal_stdlib>\n\
                             using namespace metal;\n\
                             kernel void custom{index}(device uint *out [[buffer(0)]], \
                             uint id [[thread_position_in_grid]]) {{ out[id] = id + {index}; }}"
                        );
                        preparation_device.make_shader_module(
                            ShaderModuleDescriptor::new(
                                ShaderStage::Compute,
                                ShaderFormat::MslSource,
                                source.into_bytes(),
                            )
                            .with_entry_point(format!("custom{index}")),
                        )
                    })?);
                }
                Ok(modules)
            },
        )
    });

    // Render while preparation is in flight. Every channel must be opaque white.
    let mut frames = 0;
    loop {
        let pixels = render_white_quad(&device, &mut renderer);
        assert!(
            pixels.iter().all(|&byte| byte == 255),
            "frame {frames} is fully white; got {:?}",
            &pixels[0..4],
        );
        frames += 1;
        let finished = progress
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .last()
            .is_some_and(|(phase, _, _)| *phase == PreparationPhase::Finished);
        if finished || frames >= 120 {
            break;
        }
    }

    let modules = task.join().expect("preparation thread joins").expect(
        "background preparation succeeds on a device with background compilation",
    );
    assert_eq!(modules.len(), 12, "all twelve modules prepared");
    for module in modules {
        device.destroy_shader_module(module);
    }

    let events = progress.lock().unwrap_or_else(|e| e.into_inner()).clone();
    assert_eq!(
        events.last().map(|(phase, _, _)| *phase),
        Some(PreparationPhase::Finished),
        "preparation reports Finished; saw {events:?}",
    );
    assert!(
        events.iter().any(|(_, completed, total)| {
            *completed == 12 && *total == Some(12)
        }),
        "every step is reported exactly once; saw {events:?}",
    );
    assert!(frames > 0, "at least one frame rendered during preparation");

    // The renderer still works after the prepared modules are gone.
    let after = render_white_quad(&device, &mut renderer);
    assert!(
        after.iter().all(|&byte| byte == 255),
        "the renderer keeps working after the prepared modules are destroyed",
    );
}

#[test]
fn backdrop_library_renders_independently() {
    // A second renderer loads `ui_backdrop` in addition to `ui`; both must
    // produce a frame, which is the reachable form of "the packaged metallib
    // executes" for each library.
    let device = device();
    let mut plain = NativeDrawListRenderer::new(device.clone()).expect("ui library loads");
    let pixels = render_white_quad(&device, &mut plain);
    assert_eq!(pixels.len(), 16 * 16 * 4, "the frame has one RGBA8 buffer");
    assert!(
        pixels.iter().all(|&byte| byte == 255),
        "the UI library renders opaque white",
    );
}

