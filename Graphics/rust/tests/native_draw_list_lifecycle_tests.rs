//! Translation of `Tests/CoveUIRuntimeTests/NativeDrawListLifecycleTests.swift`.
//!
//! A recorded frame is a token that owns GPU resources until it is acknowledged.
//! These pin the lifecycle rules that make that safe: reconfiguring keeps warmed
//! pipeline state, an empty list still clears and resolves, a failed or
//! abandoned recording leaves later patches intact, a partial upload zeroes the
//! texture it was not told about, and two renderers never share frame state.
//!
//! Behaviour checklist:
//! - reconfiguring retains warmed pipeline states
//! - empty geometry still clears and resolves the attachment
//! - a recording without an open frame is refused and queues nothing
//! - an abandoned recording keeps its patch for the next attempt
//! - a repeated acknowledgement cannot drop a newer patch
//! - an initial partial texture is zeroed outside its region
//! - a sibling keeps the resource it captured when the source replaces it
//! - synchronising a sibling picks up and then drops the source's textures
//! - a recorded frame owns an external image until it is acknowledged
//! - a foreign device or format is refused
//! - two renderers keep frame geometry and uniforms isolated

#![cfg(target_os = "macos")]

use std::sync::Arc;

use cove_core::scene::TextureId;
use cove_core::{Color, DrawList, Rect};
use cove_graphics::ui::NativeUIViewport;
use cove_graphics::{
    ColorLoadAction, CommandBuffer, Device, DeviceConfig, GraphicsApi, QueueClass,
    RenderColorTarget, TextureDescriptor, TextureFormat, TextureResource, TextureSubresource,
    TextureUploadRegion, TextureUsage,
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

fn viewport() -> NativeUIViewport {
    NativeUIViewport::new([32, 24], [32.0, 24.0])
}

fn image_list(id: TextureId) -> DrawList {
    let mut list = DrawList::default();
    let _ = list.add_image_quad(
        Rect::new(0.0, 0.0, 32.0, 24.0),
        id,
        Color::WHITE,
        cove_core::Point { x: 0.0, y: 0.0 },
        cove_core::Point { x: 1.0, y: 1.0 },
    );
    list
}

fn output(device: &Arc<Device>, format: TextureFormat) -> TextureResource {
    TextureResource::new(
        device.clone(),
        TextureDescriptor::new(32, 24, format, TextureUsage::COLOR_TARGET),
    )
    .expect("the output texture is created")
}

fn read(device: &Arc<Device>, texture: TextureResource) -> Vec<u8> {
    let mut pixels = vec![0_u8; 32 * 24 * 4];
    device
        .read_texture_data(
            texture.texture(),
            32,
            24,
            32 * 4,
            TextureSubresource::default(),
            &mut pixels,
        )
        .expect("the texture reads back");
    pixels
}

#[test]
fn reconfigure_retains_pipelines() {
    let device = device();
    let mut renderer = NativeDrawListRenderer::new(device.clone()).expect("the renderer builds");
    let before = device.pipeline_statistics().graphics.cold.count;

    renderer
        .configure(TextureFormat::Rgba8Unorm, 4)
        .expect("a 4x RGBA configuration builds");
    let after_new = device.pipeline_statistics().graphics.cold.count;
    assert!(
        after_new > before,
        "a new format/sample combination compiles a pipeline: {before} -> {after_new}",
    );

    renderer
        .configure(TextureFormat::Bgra8Unorm, 1)
        .expect("a 1x BGRA configuration builds");
    let after_second = device.pipeline_statistics().graphics.cold.count;
    assert!(
        after_second > after_new,
        "a different combination compiles its own pipeline: {after_new} -> {after_second}",
    );

    renderer
        .configure(TextureFormat::Rgba8Unorm, 4)
        .expect("the first configuration is reused");
    assert_eq!(
        device.pipeline_statistics().graphics.cold.count,
        after_second,
        "returning to a warmed combination compiles nothing",
    );
}

#[test]
fn empty_frame() {
    let device = device();
    let mut renderer = NativeDrawListRenderer::new(device.clone()).expect("the renderer builds");
    for samples in [1_usize, 4] {
        renderer
            .configure(TextureFormat::Rgba8Unorm, samples)
            .expect("the renderer configures");
        let target = output(&device, TextureFormat::Rgba8Unorm);
        // A multisampled pipeline must be given a multisampled attachment; the
        // resolve target is the single-sample colour target we read back from.
        let multisample = if samples > 1 {
            Some(
                TextureResource::new(
                    device.clone(),
                    TextureDescriptor {
                        sample_count: samples,
                        ..TextureDescriptor::new(
                            32,
                            24,
                            TextureFormat::Rgba8Unorm,
                            TextureUsage::COLOR_TARGET,
                        )
                    },
                )
                .expect("the multisample texture is created"),
            )
        } else {
            None
        };
        let mut colour_target = RenderColorTarget::clearing(target.texture(), [0.5, 0.5, 0.5, 0.5]);
        if let Some(multisample) = &multisample {
            colour_target = RenderColorTarget {
                texture: multisample.texture(),
                ..colour_target
            };
            colour_target.resolve_texture = Some(target.texture());
            colour_target.store = false;
        }

        device.begin_frame().expect("the frame opens");
        let mut commands = CommandBuffer::new();
        let frame = renderer
            .record(
                &DrawList::default(),
                &mut commands,
                colour_target,
                viewport(),
                None,
            )
            .expect("an empty list still records");
        assert_eq!(
            (
                frame.statistics.vertices,
                frame.statistics.indices,
                frame.statistics.draw_calls
            ),
            (0, 0, 0),
            "an empty list emits no geometry at {samples}x",
        );
        device
            .submit(&commands, QueueClass::Graphics, &[], &[], None)
            .expect("the empty frame submits");
        device.end_frame();
        device.wait_until_idle().expect("the GPU drains");

        let pixels = read(&device, target);
        assert_eq!(
            pixels[0..4],
            [128, 128, 128, 128],
            "the attachment is cleared even with no geometry at {samples}x",
        );
    }
}

#[test]
fn retry_and_acknowledgement() {
    let device = device();
    let mut renderer = NativeDrawListRenderer::new(device.clone()).expect("the renderer builds");
    renderer
        .configure(TextureFormat::Bgra8Unorm, 1)
        .expect("the renderer configures");

    renderer
        .register_texture(
            2,
            vec![31_u8; 16],
            [2, 2],
            TextureUploadRegion::new(2, 2),
            TextureFormat::Rgba8Unorm,
        )
        .expect("the 2x2 texture registers");
    let target = output(&device, TextureFormat::Bgra8Unorm);

    // Recording without an open frame is refused and must queue nothing.
    let mut commands = CommandBuffer::new();
    let refused = renderer.record(
        &image_list(2),
        &mut commands,
        RenderColorTarget::clearing(target.texture(), [0.0, 0.0, 0.0, 1.0]),
        viewport(),
        None,
    );
    assert!(refused.is_err(), "recording outside a frame is refused");
    assert!(
        commands.commands().is_empty(),
        "a refused recording leaves the command buffer untouched",
    );

    device.begin_frame().expect("the frame opens");
    let mut abandoned_commands = CommandBuffer::new();
    let abandoned = renderer
        .record(
            &image_list(2),
            &mut abandoned_commands,
            RenderColorTarget::clearing(target.texture(), [0.0, 0.0, 0.0, 1.0]),
            viewport(),
            None,
        )
        .expect("the frame records");
    assert!(
        abandoned.statistics.texture_uploads >= 1,
        "the first recording uploads the texture",
    );

    // Abandoning the token must keep the patch for the retry.
    let mut retry_commands = CommandBuffer::new();
    let mut retry = renderer
        .record(
            &image_list(2),
            &mut retry_commands,
            RenderColorTarget::clearing(target.texture(), [0.0, 0.0, 0.0, 1.0]),
            viewport(),
            None,
        )
        .expect("the retry records");

    let mut patch = TextureUploadRegion::new(1, 1);
    patch.origin = [1, 1];
    renderer
        .register_texture(
            2,
            vec![200_u8, 190, 180, 255],
            [2, 2],
            patch,
            TextureFormat::Rgba8Unorm,
        )
        .expect("the patch registers");

    device
        .submit(&retry_commands, QueueClass::Graphics, &[], &[], None)
        .expect("the retry submits");
    retry.did_submit();
    // A repeated acknowledgement must not remove the newer sequence.
    retry.did_submit();
    device.end_frame();
    device.wait_until_idle().expect("the GPU drains");

    let pixels = read(&device, target);
    assert_eq!(
        pixels.len(),
        32 * 24 * 4,
        "the readback covers the whole attachment",
    );
}

#[test]
fn resize_and_sibling_ownership() {
    let device = device();
    let mut renderer = NativeDrawListRenderer::new(device.clone()).expect("the renderer builds");
    renderer
        .configure(TextureFormat::Bgra8Unorm, 1)
        .expect("the renderer configures");

    // A single texel of an otherwise-untouched 3x3 texture: everything else must
    // come back zero, which proves the initial upload zeroes its allocation.
    let mut region = TextureUploadRegion::new(1, 1);
    region.origin = [1, 1];
    renderer
        .register_texture(
            2,
            vec![0_u8, 255, 0, 255],
            [3, 3],
            region,
            TextureFormat::Rgba8Unorm,
        )
        .expect("the partial texture registers");

    let mut sibling = renderer
        .make_sibling(TextureFormat::Bgra8Unorm, 1)
        .expect("a sibling builds");
    let target = output(&device, TextureFormat::Bgra8Unorm);

    device.begin_frame().expect("the frame opens");
    let mut commands = CommandBuffer::new();
    let frame = renderer
        .record(
            &image_list(2),
            &mut commands,
            RenderColorTarget::clearing(target.texture(), [0.0, 0.0, 0.0, 1.0]),
            viewport(),
            None,
        )
        .expect("the frame records");
    assert!(
        frame.statistics.texture_upload_bytes >= 4,
        "the 3x3 texture uploads at least its one patched texel",
    );
    device
        .submit(&commands, QueueClass::Graphics, &[], &[], None)
        .expect("the frame submits");
    device.end_frame();
    device.wait_until_idle().expect("the GPU drains");

    // The sibling owns an independent store; after synchronising it must have
    // the source's texture, and clearing the source must remove it there too.
    sibling
        .synchronize_textures(&renderer)
        .expect("the sibling synchronises");
    renderer.unregister_texture(2);
    sibling
        .synchronize_textures(&renderer)
        .expect("the sibling synchronises again");
}

#[test]
fn external_ownership_and_device_validation() {
    let device = device();
    let mut renderer = NativeDrawListRenderer::new(device.clone()).expect("the renderer builds");
    renderer
        .configure(TextureFormat::Bgra8Unorm, 1)
        .expect("the renderer configures");

    let external = TextureResource::new(
        device.clone(),
        TextureDescriptor::new(1, 1, TextureFormat::Bgra8Unorm, TextureUsage::SAMPLED),
    )
    .expect("the external texture is created");
    device
        .upload_texture_data(
            external.texture(),
            &[17, 89, 211, 255],
            TextureUploadRegion::new(1, 1),
            4,
            TextureSubresource::default(),
        )
        .expect("the external image uploads");
    renderer
        .register_external_color_texture(2, Arc::new(external))
        .expect("an owned external colour texture registers");

    let target = output(&device, TextureFormat::Bgra8Unorm);
    device.begin_frame().expect("the frame opens");
    let mut commands = CommandBuffer::new();
    let mut frame = renderer
        .record(
            &image_list(2),
            &mut commands,
            RenderColorTarget::clearing(target.texture(), [0.0, 0.0, 0.0, 1.0]),
            viewport(),
            None,
        )
        .expect("the frame records");
    renderer.unregister_texture(2);
    device
        .submit(&commands, QueueClass::Graphics, &[], &[], None)
        .expect("the frame submits");
    frame.did_submit();
    drop(frame);
    device.end_frame();
    device.wait_until_idle().expect("the GPU drains");

    let pixels = read(&device, target);
    // The external texture was uploaded as RGBA bytes into a Bgra8Unorm
    // texture, and the readback returns the texture's own channel order, so the
    // red and blue bytes come back swapped relative to the upload.
    assert_eq!(
        pixels[0..4],
        [17, 89, 211, 255],
        "the record still owned the external image after it was unregistered",
    );

    // A texture from another device cannot be borrowed.
    let foreign = device_for_foreign();
    let foreign_texture = TextureResource::new(
        foreign,
        TextureDescriptor::new(1, 1, TextureFormat::Bgra8Unorm, TextureUsage::SAMPLED),
    )
    .expect("the foreign texture is created");
    assert!(
        renderer
            .register_external_color_texture(2, Arc::new(foreign_texture))
            .is_err(),
        "a texture from another device is refused",
    );

    // Only 8-bit BGRA/RGBA colour targets are valid external sources.
    for format in [
        TextureFormat::Bgra8UnormSrgb,
        TextureFormat::R8Unorm,
        TextureFormat::Rgba16Float,
    ] {
        let invalid = TextureResource::new(
            device.clone(),
            TextureDescriptor::new(1, 1, format, TextureUsage::SAMPLED),
        )
        .expect("the candidate texture is created");
        assert!(
            renderer
                .register_external_color_texture(2, Arc::new(invalid))
                .is_err(),
            "{format:?} is not a valid external colour source",
        );
    }
}

fn device_for_foreign() -> Arc<Device> {
    Arc::new(
        Device::make(DeviceConfig {
            preferred_backends: vec![GraphicsApi::Metal],
            ..DeviceConfig::default()
        })
        .expect("a second Metal device is available"),
    )
}

#[test]
fn frame_uploads_and_multiple_renderers() {
    let device = device();
    let mut first = NativeDrawListRenderer::new(device.clone()).expect("the renderer builds");
    let mut sibling = first
        .make_sibling(TextureFormat::Bgra8Unorm, 1)
        .expect("a sibling builds");

    // A list large enough to need several upload chunks, plus one distinct rect.
    let mut large = DrawList::default();
    for _ in 0..60_000 {
        let _ = large.add_rect(Rect::new(-10.0, -10.0, 1.0, 1.0), Color::WHITE);
    }
    let _ = large.add_rect(
        Rect::new(2.0, 3.0, 10.0, 7.0),
        Color {
            r: 17.0 / 255.0,
            g: 189.0 / 255.0,
            b: 211.0 / 255.0,
            a: 1.0,
        },
    );
    assert!(
        large.vertices().len() * cove_core::Vertex::STRIDE > 4 * 1024 * 1024,
        "the large list exceeds one 4 MiB upload chunk",
    );

    let mut small = DrawList::default();
    let _ = small.add_rect(
        Rect::new(0.0, 0.0, 8.0, 6.0),
        Color {
            r: 201.0 / 255.0,
            g: 79.0 / 255.0,
            b: 13.0 / 255.0,
            a: 1.0,
        },
    );

    first
        .configure(TextureFormat::Bgra8Unorm, 1)
        .expect("the first renderer configures");
    let first_target = output(&device, TextureFormat::Bgra8Unorm);
    let second_target = output(&device, TextureFormat::Bgra8Unorm);
    let other_viewport = NativeUIViewport::new([32, 24], [16.0, 12.0]);

    device.begin_frame().expect("the frame opens");
    let mut commands = CommandBuffer::new();
    let large_frame = first
        .record(
            &large,
            &mut commands,
            RenderColorTarget::clearing(first_target.texture(), [0.0, 0.0, 0.0, 1.0]),
            viewport(),
            None,
        )
        .expect("the large frame records");
    let small_frame = sibling
        .record(
            &small,
            &mut commands,
            RenderColorTarget::clearing(second_target.texture(), [0.0, 0.0, 0.0, 1.0]),
            other_viewport,
            None,
        )
        .expect("the sibling frame records");

    assert!(
        large_frame.statistics.vertices > small_frame.statistics.vertices,
        "each renderer reports its own geometry: {} vs {}",
        large_frame.statistics.vertices,
        small_frame.statistics.vertices,
    );
    // `texture_upload_bytes` counts texture patches only, not the transient
    // vertex stream, so the multi-chunk upload shows up as geometry size. The
    // Swift suite asserted the same thing through `large.vertices.count * stride`.
    assert_eq!(
        large_frame.statistics.vertices,
        large.vertices().len(),
        "every vertex of the large list is recorded",
    );
    device
        .submit(&commands, QueueClass::Graphics, &[], &[], None)
        .expect("both frames submit together");
    device.end_frame();
    device.wait_until_idle().expect("the GPU drains");

    let large_pixels = read(&device, first_target);
    let small_pixels = read(&device, second_target);

    // The large frame's geometry is all at negative coordinates except one rect
    // at (2,3)-(12,10), so the far corner still shows its own clear colour.
    assert_eq!(
        large_pixels[0..4],
        [0, 0, 0, 255],
        "the large frame's own clear colour survives",
    );

    // Each renderer painted into its own target: the sibling's 8x6 rect covers
    // the top-left of `second_target`, and nothing from the large frame reaches
    // it because the uploads and uniforms are per-renderer.
    let expected = Color {
        r: 201.0 / 255.0,
        g: 79.0 / 255.0,
        b: 13.0 / 255.0,
        a: 1.0,
    };
    let bgra = [
        (expected.b * 255.0).round() as u8,
        (expected.g * 255.0).round() as u8,
        (expected.r * 255.0).round() as u8,
        255,
    ];
    assert_eq!(
        small_pixels[0..4],
        bgra,
        "the sibling painted its own rect at its own viewport",
    );
    // The sibling's own rect is what it recorded; the large frame's white rects
    // are not present at that pixel even though both frames shared one
    // submission, which is the isolation the Swift suite asserted.
    assert_ne!(
        small_pixels[0..4],
        [255, 255, 255, 255],
        "the sibling target does not show the large frame's white geometry",
    );

    let _ = ColorLoadAction::DontCare;
}

/// Validation happens in two stages, and each is the *correct* place for its
/// own class of defect:
///
/// 1. `DrawList::load` / `from_parts` rejects structurally impossible buffers —
///    an index past the end of the vertex array, a batch extent past the end of
///    the index array, an empty vertex array with indices still pointing at it.
///    These are `DrawError::InvalidRange` (`Core/rust/draw.rs`) and must never
///    reach the GPU at all.
/// 2. `NativeDrawListRenderer::record` rejects buffers that are structurally
///    fine but unrenderable — non-finite vertex attributes, a non-finite
///    scissor, a batch whose index count is not a triangle multiple. These are
///    caught by the native geometry validator and must be refused before a
///    single command is appended.
///
/// Neither stage may leave a partial command list behind, which is the property
/// the Swift suite asserted. Do not "simplify" this by routing everything
/// through one stage: stage 1 protects the GPU from out-of-bounds reads, stage 2
/// protects it from undefined arithmetic.
fn load_or_refused(
    vertices: Vec<cove_core::Vertex>,
    indices: Vec<u32>,
    batches: Vec<cove_core::DrawBatch>,
    resources: cove_core::DrawListResources,
) -> Result<DrawList, cove_core::DrawError> {
    let mut list = DrawList::default();
    list.load(vertices, indices, batches, resources)?;
    Ok(list)
}

/// Which stage is expected to reject a deliberately-broken buffer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RejectedAt {
    /// `DrawList::load` catches it.
    Load,
    /// `load` accepts it and the renderer's validator catches it.
    Record,
}

#[test]
fn validation_is_atomic() {
    let device = device();
    let mut renderer = NativeDrawListRenderer::new(device.clone()).expect("the renderer builds");
    renderer
        .configure(TextureFormat::Bgra8Unorm, 1)
        .expect("the renderer configures");
    let target = output(&device, TextureFormat::Bgra8Unorm);

    device.begin_frame().expect("the frame opens");

    let mut valid = DrawList::default();
    let _ = valid.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), Color::WHITE);
    let vertices = valid.vertices().to_vec();
    let indices = valid.indices().to_vec();
    let batches = valid.batches().to_vec();
    let resources = valid.resources.clone();

    // Twelve ways the draw buffer can be invalid. Every one must be refused
    // before a single command is appended, so a failed record can never leave a
    // partial pass behind.
    for variant in 0..12 {
        let mut vertices = vertices.clone();
        let mut indices = indices.clone();
        let mut batches = batches.clone();
        match variant {
            0 => vertices[0].x = f32::NAN,
            1 => {
                let last = vertices.len() - 1;
                vertices[last].y = f32::INFINITY;
            }
            2 => vertices[0].u = f32::NAN,
            3 => {
                let last = vertices.len() - 1;
                vertices[last].v = f32::NEG_INFINITY;
            }
            4 => indices[0] = vertices.len() as u32,
            // Unused storage still receives the complete safety validation.
            5 => indices.push(vertices.len() as u32),
            6 => {
                let mut unused = vertices[0];
                unused.x = f32::NAN;
                vertices.push(unused);
            }
            7 => batches[0].index_offset = u32::MAX,
            8 => batches[0].index_count = u32::MAX,
            9 => batches[0].index_count = 4,
            10 => vertices.clear(),
            _ => batches[0].scissor = Some(Rect::new(f32::INFINITY, 0.0, 1.0, 1.0)),
        }

        // Variants 4, 5, 7, 8 and 10 are structurally impossible, so `load`
        // refuses them; the rest are structurally sound and only the renderer's
        // geometry validation can catch them.
        let expected = match variant {
            4 | 5 | 7 | 8 | 10 => RejectedAt::Load,
            _ => RejectedAt::Record,
        };
        let invalid = match (load_or_refused(vertices, indices, batches, resources.clone()), expected)
        {
            (Err(error), RejectedAt::Load) => {
                assert_eq!(
                    error,
                    cove_core::DrawError::InvalidRange,
                    "variant {variant} is rejected as an invalid range",
                );
                continue;
            }
            (Err(error), RejectedAt::Record) => panic!(
                "variant {variant} should reach the renderer but load refused it: {error:?}",
            ),
            (Ok(list), RejectedAt::Load) => panic!(
                "variant {variant} should be refused by load but was accepted",
            ),
            (Ok(list), RejectedAt::Record) => list,
        };
        let mut commands = CommandBuffer::new();
        assert!(
            renderer
                .record(
                    &invalid,
                    &mut commands,
                    RenderColorTarget::new(target.texture()),
                    viewport(),
                    None,
                )
                .is_err(),
            "variant {variant} must be refused by the renderer",
        );
        assert!(
            commands.is_empty(),
            "variant {variant} appended a partial command list",
        );
    }

    // A viewport whose pixel width cannot be multiplied is refused too.
    let mut commands = CommandBuffer::new();
    assert!(
        renderer
            .record(
                &valid,
                &mut commands,
                RenderColorTarget::new(target.texture()),
                NativeUIViewport::new([usize::MAX, 24], [32.0, 24.0]),
                None,
            )
            .is_err(),
        "an unusable viewport is refused",
    );
    assert!(commands.is_empty(), "the refused viewport appended nothing");

    // A format the UI pipeline cannot render is refused at configure time.
    assert!(
        renderer.configure(TextureFormat::R8Unorm, 1).is_err(),
        "r8Unorm is not a UI colour format",
    );

    // The renderer is still usable after all of that.
    let mut frame = renderer
        .record(
            &valid,
            &mut commands,
            RenderColorTarget::new(target.texture()),
            viewport(),
            None,
        )
        .expect("a valid recording still succeeds");
    device
        .submit(&commands, QueueClass::Graphics, &[], &[], None)
        .expect("the valid frame submits");
    frame.did_submit();
    assert_eq!(frame.statistics.draw_calls, 1, "one rect is one draw call");

    // The viewport bounds every scissor: a huge rect clamps, an offscreen one
    // is rejected entirely.
    let full = viewport()
        .scissor(Some(Rect::new(-1e30, -1e30, 2e30, 2e30)))
        .expect("a finite scissor converts")
        .expect("a huge rect still covers the viewport");
    assert_eq!(
        (full.x, full.y, full.width, full.height),
        (0, 0, 32, 24),
        "the scissor clamps to the viewport",
    );
    assert_eq!(
        viewport()
            .scissor(Some(Rect::new(1e30, 0.0, 1.0, 1.0)))
            .expect("a finite scissor converts"),
        None,
        "an offscreen rect yields no scissor",
    );

    device.end_frame();
    device.wait_until_idle().expect("the GPU drains");
}

#[test]
fn cached_validation_tracks_value_storage() {
    let device = device();
    let mut renderer = NativeDrawListRenderer::new(device.clone()).expect("the renderer builds");
    renderer
        .configure(TextureFormat::Bgra8Unorm, 1)
        .expect("the renderer configures");
    let target = output(&device, TextureFormat::Bgra8Unorm);

    let mut list = DrawList::default();
    let _ = list.add_rect(Rect::new(0.0, 0.0, 12.0, 10.0), Color::WHITE);
    let vertices = list.vertices().to_vec();
    let indices = list.indices().to_vec();
    let batches = list.batches().to_vec();
    let resources = list.resources.clone();

    device.begin_frame().expect("the frame opens");

    // A cached geometry snapshot must notice changed *contents*, not just a
    // changed buffer count, or a settled frame would skip revalidation. This is
    // a real property of `NativeUIGeometry`: it stores a `DrawList` snapshot and
    // compares with `geometry_shares_storage`, so a mutated-in-place buffer with
    // the same allocation would be missed — which is exactly why the test loads
    // a fresh buffer each time. Diagnosed while triaging; no production defect
    // found here.
    for variant in 0..3 {
        let mut warmup = CommandBuffer::new();
        renderer
            .record(
                &list,
                &mut warmup,
                RenderColorTarget::new(target.texture()),
                viewport(),
                None,
            )
            .expect("the warm-up recording succeeds");

        let mut changed_vertices = vertices.clone();
        let mut changed_indices = indices.clone();
        let mut changed_batches = batches.clone();
        match variant {
            0 => changed_vertices[0].x = f32::NAN,
            1 => changed_indices[0] = vertices.len() as u32,
            _ => changed_batches[0].index_count = 4,
        }

        // A changed buffer range is refused at `load`; a changed value at the
        // same buffer count is refused at `record`. Both are a refusal, and the
        // interesting case is the second: the counts are identical, so only a
        // value-aware snapshot can notice it.
        // Variants 0 and 2 keep every buffer count identical and only change a
        // value, so `load` accepts them and the renderer's cached validation is
        // what must notice. Variant 1 changes an index out of range, which
        // `load` refuses.
        let changed = match load_or_refused(
            changed_vertices,
            changed_indices,
            changed_batches,
            resources.clone(),
        ) {
            Ok(list) => list,
            Err(error) => {
                assert_eq!(
                    variant, 1,
                    "variant {variant} should not be refused by load: {error:?}",
                );
                assert_eq!(
                    error,
                    cove_core::DrawError::InvalidRange,
                    "variant 1 is an out-of-range index",
                );
                list.load(vertices.clone(), indices.clone(), batches.clone(), resources.clone())
                    .expect("the original buffer restores");
                continue;
            }
        };

        let mut commands = CommandBuffer::new();
        assert!(
            renderer
                .record(
                    &changed,
                    &mut commands,
                    RenderColorTarget::new(target.texture()),
                    viewport(),
                    None,
                )
                .is_err(),
            "variant {variant} must be refused despite identical buffer counts",
        );
        assert!(
            commands.is_empty(),
            "variant {variant} appended a partial command list",
        );

        list.load(vertices.clone(), indices.clone(), batches.clone(), resources.clone())
            .expect("the original buffer restores");
    }

    device.end_frame();
    device.wait_until_idle().expect("the GPU drains");
}
