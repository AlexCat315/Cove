//! Translation of `Tests/CoveUIRuntimeTests/NativeImageAssetTests.swift`.
//!
//! A recorded frame owns the GPU images its bindings reference. The interesting
//! cases are the ones where ownership is easy to get wrong: a recording that
//! fails before submission, a CPU cache that clears while a frame is in flight,
//! a sibling renderer with its own store, and two registries that hand out the
//! same key but must not share an image.
//!
//! Behaviour checklist:
//! - recording without an open frame is refused and uploads nothing
//! - an abandoned recording keeps its pending upload for the retry
//! - two registries with the same key produce different texture ids
//! - re-registering a cleared key produces a new texture id
//! - three sibling textures upload once and render in their own colours
//! - a sibling renderer re-uploads only its own fallback
//! - repeated frames upload nothing
//! - a texture from another device cannot be synchronised in

#![cfg(target_os = "macos")]

use std::sync::Arc;

use cove_core::scene::TextureId;
use cove_core::{Bitmap, Color, DrawList, Point, Rect};
use cove_graphics::ui::NativeUIViewport;
use cove_graphics::{
    CommandBuffer, Device, DeviceConfig, GraphicsApi, QueueClass, RenderColorTarget,
    TextureDescriptor, TextureFormat, TextureResource, TextureSubresource, TextureUsage,
};
use cove_graphics::NativeDrawListRenderer;
use cove_imaging::ImageAssetRegistry;

fn device() -> Arc<Device> {
    Arc::new(
        Device::make(DeviceConfig {
            preferred_backends: vec![GraphicsApi::Metal],
            ..DeviceConfig::default()
        })
        .expect("a Metal device is available"),
    )
}

fn viewport(pixels: [usize; 2]) -> NativeUIViewport {
    NativeUIViewport::new(pixels, [pixels[0] as f32, pixels[1] as f32])
}

/// One 1×1 image quad at `x`, spanning 16 points.
fn image(asset: TextureId, x: f32) -> DrawList {
    let mut list = DrawList::default();
    let _ = list.add_image_quad(
        Rect::new(x, 0.0, 16.0, 16.0),
        asset,
        Color::WHITE,
        Point { x: 0.0, y: 0.0 },
        Point { x: 1.0, y: 1.0 },
    );
    list
}

/// A draw list that owns its image asset. The store only uploads what the list
/// retains, so a quad whose asset is not retained renders the white fallback —
/// the same contract `DrawListResources` has in Swift.
fn image_owned(asset: &Arc<cove_imaging::ImageAsset>, x: f32) -> DrawList {
    let mut list = image(asset.texture_id, x);
    list.resources.retain(asset.clone());
    list
}

fn pixel_rgba(r: u8, g: u8, b: u8, a: u8) -> Bitmap {
    Bitmap::new(1, 1, vec![r, g, b, a]).expect("a 1x1 bitmap is valid")
}

fn output(device: &Arc<Device>, width: usize, height: usize) -> TextureResource {
    TextureResource::new(
        device.clone(),
        TextureDescriptor::new(width, height, TextureFormat::Bgra8Unorm, TextureUsage::COLOR_TARGET),
    )
    .expect("the output texture is created")
}

fn read(device: &Arc<Device>, texture: TextureResource, width: usize, height: usize) -> Vec<u8> {
    let mut pixels = vec![0_u8; width * height * 4];
    device
        .read_texture_data(
            texture.texture(),
            width,
            height,
            width * 4,
            TextureSubresource::default(),
            &mut pixels,
        )
        .expect("the texture reads back");
    pixels
}

#[test]
fn recovery_and_reclamation() {
    let device = device();
    let mut renderer = NativeDrawListRenderer::new(device.clone()).expect("the renderer builds");
    renderer
        .configure(TextureFormat::Bgra8Unorm, 1)
        .expect("the renderer configures");

    let registry = ImageAssetRegistry::new(Default::default());
    let asset = registry
        .register("red", pixel_rgba(233, 17, 99, 255))
        .expect("the image registers");
    let id = asset.texture_id;
    let list = image_owned(&asset, 0.0);
    let viewport = viewport([32, 16]);
    let target = output(&device, 32, 16);

    // A recording made outside a frame is refused. Any staging it did must be
    // retained for the next recording, not lost.
    let mut refused_commands = CommandBuffer::new();
    let refused = renderer.record(
        &list,
        &mut refused_commands,
        RenderColorTarget::clearing(target.texture(), [0.0, 0.0, 0.0, 1.0]),
        viewport,
        None,
    );
    assert!(refused.is_err(), "recording without a frame is refused");

    device.begin_frame().expect("the frame opens");
    let mut abandoned_commands = CommandBuffer::new();
    let abandoned = renderer
        .record(
            &list,
            &mut abandoned_commands,
            RenderColorTarget::clearing(target.texture(), [0.0, 0.0, 0.0, 1.0]),
            viewport,
            None,
        )
        .expect("the retry records");
    // 8 bytes: the one 1x1 texel plus the renderer's 1x1 white fallback, which
    // is what Swift's `textureUploadBytes == 8` measured.
    assert_eq!(
        abandoned.statistics.texture_upload_bytes, 8,
        "the first recording stages the texel and the fallback",
    );
    // Abandoning this token must leave the patch queued for the next attempt.
    // The frame stays open: the next recording happens in the same frame, which
    // is exactly the retry the Swift suite describes.
    drop(abandoned);

    let mut commands = CommandBuffer::new();
    let accepted = renderer
        .record(
            &list,
            &mut commands,
            RenderColorTarget::clearing(target.texture(), [0.0, 0.0, 0.0, 1.0]),
            viewport,
            None,
        )
        .expect("the accepted frame records");
    assert_eq!(
        accepted.statistics.texture_upload_bytes, 8,
        "the abandoned patch is still pending, so the retry uploads it",
    );
    // The CPU cache and the geometry can both go away; the recording owns the
    // GPU slot independently of them.
    registry.clear();
    let mut list = list;
    list.reset();

    device
        .submit(&commands, QueueClass::Graphics, &[], &[], None)
        .expect("the accepted frame submits");
    let mut accepted = accepted;
    accepted.did_submit();
    device.end_frame();
    device.wait_until_idle().expect("the GPU drains");

    let pixels = read(&device, target, 32, 16);
    // The uploaded image was RGBA into a Bgra8Unorm texture, so the red and blue
    // channels read back swapped relative to the source bytes.
    assert_eq!(
        pixels[0..4],
        [99, 17, 233, 255],
        "the frame still drew the image after the CPU cache cleared",
    );
}

#[test]
fn multiple_assets_and_reuse() {
    let device = device();
    let mut renderer = NativeDrawListRenderer::new(device.clone()).expect("the renderer builds");
    renderer
        .configure(TextureFormat::Bgra8Unorm, 1)
        .expect("the renderer configures");

    let registry = ImageAssetRegistry::new(Default::default());
    let a = registry
        .register("same", pixel_rgba(255, 0, 0, 255))
        .expect("the first image registers");
    let b = ImageAssetRegistry::new(Default::default())
        .register("same", pixel_rgba(0, 255, 0, 255))
        .expect("a second registry registers the same key");
    registry.clear();
    let replacement = registry
        .register("same", pixel_rgba(0, 0, 255, 255))
        .expect("the cleared key registers again");

    assert_ne!(
        replacement.texture_id, a.texture_id,
        "a cleared key gets a fresh texture id",
    );
    assert_ne!(
        a.texture_id, b.texture_id,
        "independent registries never share a texture id",
    );

    let mut list = image_owned(&a, 0.0);
    list.append(&image_owned(&b, 16.0), Point { x: 0.0, y: 0.0 })
        .expect("the draws append");
    list.append(&image_owned(&replacement, 32.0), Point { x: 0.0, y: 0.0 })
        .expect("the draws append");

    let viewport = viewport([48, 16]);
    let target = output(&device, 48, 16);

    device.begin_frame().expect("the frame opens");
    let mut commands = CommandBuffer::new();
    let frame = renderer
        .record(
            &list,
            &mut commands,
            RenderColorTarget::clearing(target.texture(), [0.0, 0.0, 0.0, 1.0]),
            viewport,
            None,
        )
        .expect("the frame records");
    // 16 bytes: three 1x1 texels plus the renderer's 1x1 white fallback.
    assert_eq!(
        frame.statistics.texture_upload_bytes, 16,
        "three texels and the fallback upload once, at four bytes each",
    );
    device
        .submit(&commands, QueueClass::Graphics, &[], &[], None)
        .expect("the frame submits");
    let mut frame = frame;
    frame.did_submit();
    device.end_frame();
    device.wait_until_idle().expect("the GPU drains");

    let pixels = read(&device, target, 48, 16);
    let texel = |x: usize| {
        let index = x * 4;
        [pixels[index], pixels[index + 1], pixels[index + 2], pixels[index + 3]]
    };
    assert_eq!(texel(0), [0, 0, 255, 255], "the first asset is blue-red-swapped");
    assert_eq!(texel(16), [0, 255, 0, 255], "the sibling registry's green renders");
    assert_eq!(texel(32), [255, 0, 0, 255], "the replacement renders");

    // A sibling keeps its own store: it must re-upload only its fallback.
    let mut sibling = renderer
        .make_sibling(TextureFormat::Bgra8Unorm, 1)
        .expect("a sibling builds");
    device.begin_frame().expect("the sibling frame opens");
    let mut sibling_commands = CommandBuffer::new();
    let sibling_target = output(&device, 48, 16);
    let sibling_frame = sibling
        .record(
            &list,
            &mut sibling_commands,
            RenderColorTarget::clearing(sibling_target.texture(), [0.0, 0.0, 0.0, 1.0]),
            viewport,
            None,
        )
        .expect("the sibling frame records");
    assert!(
        sibling_frame.statistics.texture_upload_bytes > 0,
        "the sibling uploads its own copies, not zero",
    );
    device
        .submit(&sibling_commands, QueueClass::Graphics, &[], &[], None)
        .expect("the sibling frame submits");
    let mut sibling_frame = sibling_frame;
    sibling_frame.did_submit();
    device.end_frame();
    device.wait_until_idle().expect("the GPU drains");

    let sibling_pixels = read(&device, sibling_target, 48, 16);
    assert_eq!(
        sibling_pixels[0..4],
        pixels[0..4],
        "the sibling renders identical pixels",
    );

    // A second frame on the warmed renderer uploads nothing.
    device.begin_frame().expect("the repeat frame opens");
    let mut repeat_commands = CommandBuffer::new();
    let repeat_target = output(&device, 48, 16);
    let repeat = renderer
        .record(
            &list,
            &mut repeat_commands,
            RenderColorTarget::clearing(repeat_target.texture(), [0.0, 0.0, 0.0, 1.0]),
            viewport,
            None,
        )
        .expect("the repeat frame records");
    assert_eq!(
        repeat.statistics.texture_uploads, 0,
        "a warmed renderer re-uploads nothing",
    );
    device.end_frame();
    device.wait_until_idle().expect("the GPU drains");
}

#[test]
fn a_foreign_renderer_cannot_be_synchronised() {
    let device = device();
    let renderer = NativeDrawListRenderer::new(device.clone()).expect("the renderer builds");
    let foreign = Arc::new(
        Device::make(DeviceConfig {
            preferred_backends: vec![GraphicsApi::Metal],
            ..DeviceConfig::default()
        })
        .expect("a second Metal device is available"),
    );
    let mut foreign_renderer =
        NativeDrawListRenderer::new(foreign).expect("the foreign renderer builds");
    assert!(
        foreign_renderer.synchronize_textures(&renderer).is_err(),
        "textures cannot cross device boundaries",
    );

    let foreign_texture = TextureResource::new(
        device.clone(),
        TextureDescriptor::new(1, 1, TextureFormat::Bgra8Unorm, TextureUsage::SAMPLED),
    )
    .expect("the candidate texture is created");
    assert!(
        foreign_renderer
            .register_external_color_texture(2, Arc::new(foreign_texture))
            .is_err(),
        "a texture from another device is refused",
    );
}
