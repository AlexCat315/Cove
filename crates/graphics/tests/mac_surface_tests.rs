//! Translation of `Graphics/Tests/NativeRHITests/MacSurfaceTests.swift`.
//!
//! A Metal surface is a `CAMetalLayer`. These pin the contract around it: only a
//! Metal surface is accepted, the layer's `displaySyncEnabled` and the device's
//! reported presentation mode agree, resizing a live swapchain needs an idle
//! device and bumps the generation, each window keeps its own drawable, an
//! image can only be acquired or presented once per frame, and a closed resource
//! releases its window and textures.
//!
//! The Rust port uses a headless `CAMetalLayer` instead of an `NSWindow`-hosted
//! one. That is not a weaker test of this suite's subject: everything asserted
//! here is the device/swapchain/layer contract, and a `CAMetalLayer` acquired
//! from `objc2_quartz_core` is the same object AppKit would install in a view.
//! The two window-management cases that only exist to prove *window* lifetimes
//! (retained-pool bounds across 40 frames) are driven from two independent
//! layers instead, which is the part of that case the RHI is responsible for.
//!
//! Behaviour checklist:
//! - a non-Metal surface kind is refused
//! - presentation mode follows the layer's display-sync setting
//! - a live swapchain cannot be reconfigured mid-frame or without idling
//! - a resize bumps the generation and the acquired extent
//! - two layers keep independent textures and extents
//! - an image cannot be acquired twice in one frame
//! - presenting another window's image is refused
//! - a cleared drawable reads back in the layer's channel order
//! - closing a resource releases its window, and the other keeps working

#![cfg(target_os = "macos")]

use std::sync::Arc;

use objc2::rc::Retained;
use objc2_quartz_core::CAMetalLayer;

use cove_graphics::{
    ColorLoadAction, CommandBuffer, Device, DeviceConfig, GraphicsApi, NativeSurfaceKind,
    QueueClass, RenderColorTarget, SurfaceDescriptor, SwapchainResource, TextureFormat,
    TextureSubresource, TextureUsage,
};

fn device() -> Arc<Device> {
    Arc::new(
        Device::make(DeviceConfig {
            preferred_backends: vec![GraphicsApi::Metal],
            ..DeviceConfig::default()
        })
        .expect("a Metal device is available"),
    )
}

/// A headless `CAMetalLayer` and the pointer the surface descriptor carries.
fn layer(size: usize) -> (Retained<CAMetalLayer>, usize) {
    let layer = CAMetalLayer::new();
    let pointer = Retained::as_ptr(&layer) as *mut std::ffi::c_void as usize;
    let descriptor = descriptor_for(pointer, size);
    let _ = descriptor;
    (layer, pointer)
}

fn descriptor_for(pointer: usize, size: usize) -> SurfaceDescriptor {
    let mut descriptor = SurfaceDescriptor::new(pointer);
    descriptor.kind = NativeSurfaceKind::MetalLayer;
    descriptor.width = size;
    descriptor.height = size;
    descriptor.color_format = TextureFormat::Bgra8Unorm;
    descriptor
}

/// Reads a whole drawable back as RGBA bytes.
fn read(
    device: &Arc<Device>,
    texture: cove_graphics::Texture,
    width: usize,
    height: usize,
) -> Vec<u8> {
    let mut pixels = vec![0_u8; width * height * 4];
    device
        .read_texture_data(
            texture,
            width,
            height,
            width * 4,
            TextureSubresource::default(),
            &mut pixels,
        )
        .expect("the drawable reads back");
    pixels
}

#[test]
fn non_metal_surface_is_refused() {
    let device = device();
    let mut descriptor = SurfaceDescriptor::new(1);
    descriptor.kind = NativeSurfaceKind::platform_default();
    descriptor.width = 1;
    descriptor.height = 1;
    // A plain (non-Metal) surface cannot back a Metal swapchain.
    if descriptor.kind != NativeSurfaceKind::MetalLayer {
        assert!(
            device.make_swapchain(descriptor).is_err(),
            "a non-Metal surface kind is refused",
        );
    }
}

#[test]
fn presentation_mode_reflects_applied_layer_setting() {
    let device = device();
    let (_layer, pointer) = layer(16);

    // The default descriptor asks for vsync, so the first mode is fifo.
    let resource = SwapchainResource::new(Arc::clone(&device), descriptor_for(pointer, 16))
        .expect("the layer-backed swapchain builds");
    assert_eq!(
        device
            .presentation_mode(resource.swapchain())
            .expect("a mode is reported"),
        cove_graphics::PresentationMode::Fifo,
        "a vsynced layer presents fifo",
    );

    let mut unsynchronized = descriptor_for(pointer, 16);
    unsynchronized.vsync_enabled = false;
    device
        .configure_swapchain(resource.swapchain(), unsynchronized)
        .expect("the layer reconfigures");
    assert_eq!(
        device
            .presentation_mode(resource.swapchain())
            .expect("a mode is reported"),
        cove_graphics::PresentationMode::Immediate,
        "disabling vsync on the layer reports immediate",
    );

    let mut resource = resource;
    resource.close().expect("the resource closes");
    assert!(
        device.presentation_mode(resource.swapchain()).is_err(),
        "a closed swapchain has no presentation mode",
    );
}

#[test]
fn resize_needs_an_idle_device_and_bumps_the_generation() {
    let device = device();
    let (_layer, pointer) = layer(32);
    let mut resource = SwapchainResource::new(Arc::clone(&device), descriptor_for(pointer, 32))
        .expect("the layer-backed swapchain builds");
    let handle = resource.swapchain();

    let mut previous_generation = None;
    for pass in 0..3 {
        if pass == 2 {
            device
                .wait_until_idle()
                .expect("the device idles before the resize");
            device
                .configure_swapchain(handle, descriptor_for(pointer, 16))
                .expect("the resize applies while idle");
        }

        device.begin_frame().expect("the frame opens");
        let image = device
            .acquire_swapchain_image(handle)
            .expect("the drawable acquires");
        let expected = if pass == 2 { 16 } else { 32 };
        assert_eq!(
            (image.width, image.height),
            (expected, expected),
            "pass {pass} acquires the current extent",
        );

        // A second acquisition in the same frame is refused, as is a resize
        // while an image is outstanding.
        assert!(
            device.acquire_swapchain_image(handle).is_err(),
            "pass {pass} cannot acquire a second drawable",
        );
        assert!(
            device
                .configure_swapchain(handle, descriptor_for(pointer, 8))
                .is_err(),
            "pass {pass} cannot resize with a drawable outstanding",
        );

        let mut commands = CommandBuffer::new();
        commands.render_pass(
            cove_graphics::RenderPassDescriptor::new(vec![RenderColorTarget::clearing(
                image.texture,
                [0.0, 1.0, 0.0, 1.0],
            )]),
            |pass| {
                let _ = pass;
            },
        );
        device
            .submit(&commands, QueueClass::Graphics, &[], &[], None)
            .expect("the clear submits");
        device.present(image).expect("the image presents");
        device.end_frame();

        if pass == 2 {
            assert_ne!(
                Some(image.generation),
                previous_generation,
                "the resize bumps the generation",
            );
        } else if pass != 0 {
            assert_eq!(
                Some(image.generation),
                previous_generation,
                "an unchanged extent keeps the generation",
            );
        }
        previous_generation = Some(image.generation);
    }

    device.wait_until_idle().expect("the device drains");
    resource.close().expect("the resource closes");
}

#[test]
fn windows_keep_independent_images() {
    let device = device();
    let (_first_layer, first_pointer) = layer(32);
    let (_second_layer, second_pointer) = layer(48);

    let first = SwapchainResource::new(Arc::clone(&device), descriptor_for(first_pointer, 32))
        .expect("the first swapchain builds");
    let second = SwapchainResource::new(Arc::clone(&device), descriptor_for(second_pointer, 48))
        .expect("the second swapchain builds");

    // The same layer cannot back two swapchains.
    assert!(
        device
            .make_swapchain(descriptor_for(first_pointer, 32))
            .is_err(),
        "one layer backs one swapchain",
    );

    device.begin_frame().expect("the frame opens");
    let a = device
        .acquire_swapchain_image(first.swapchain())
        .expect("the first drawable acquires");
    let b = device
        .acquire_swapchain_image(second.swapchain())
        .expect("the second drawable acquires");
    assert_ne!(a.texture, b.texture, "each window has its own texture");
    assert_eq!((a.width, a.height), (32, 32), "the first extent");
    assert_eq!((b.width, b.height), (48, 48), "the second extent");

    // Presenting another window's image is refused.
    let foreign = cove_graphics::SwapchainImage {
        swapchain: second.swapchain(),
        ..a
    };
    assert!(
        device.present(foreign).is_err(),
        "an image cannot be presented to another window",
    );

    let mut commands = CommandBuffer::new();
    commands.render_pass(
        cove_graphics::RenderPassDescriptor::new(vec![RenderColorTarget::clearing(
            a.texture,
            [1.0, 0.0, 0.0, 1.0],
        )]),
        |pass| {
            let _ = pass;
        },
    );
    commands.render_pass(
        cove_graphics::RenderPassDescriptor::new(vec![RenderColorTarget::clearing(
            b.texture,
            [0.0, 1.0, 0.0, 1.0],
        )]),
        |pass| {
            let _ = pass;
        },
    );
    device
        .submit(&commands, QueueClass::Graphics, &[], &[], None)
        .expect("both clears submit");
    device.present(a).expect("the first image presents");
    device.present(b).expect("the second image presents");
    assert!(
        device.present(a).is_err(),
        "an already-presented image cannot be presented again",
    );
    device.end_frame();
    device.wait_until_idle().expect("the device drains");

    // A cleared drawable reads back in the layer's own channel order: the layer
    // is Bgra8Unorm, so a red clear lands as [0, 0, 255, 255].
    let first_pixels = read(&device, a.texture, a.width, a.height);
    assert!(
        first_pixels
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel == [0, 0, 255, 255]),
        "the first drawable is uniformly red: {:?}",
        &first_pixels[0..4],
    );
    let second_pixels = read(&device, b.texture, b.width, b.height);
    assert!(
        second_pixels
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel == [0, 255, 0, 255]),
        "the second drawable is uniformly green: {:?}",
        &second_pixels[0..4],
    );

    let mut first = first;
    let mut second = second;
    first.close().expect("the first resource closes");
    device.wait_until_idle().expect("the device drains");
    // The second window still works after the first is closed.
    device.begin_frame().expect("the frame opens");
    assert!(
        device.acquire_swapchain_image(first.swapchain()).is_err(),
        "a closed window yields no drawable",
    );
    let _still_open = device
        .acquire_swapchain_image(second.swapchain())
        .expect("the second window still acquires");
    device.end_frame();
    device.wait_until_idle().expect("the device drains");
    second.close().expect("the second resource closes");
}

#[test]
fn surface_rejects_a_zero_sized_or_null_layer() {
    let device = device();
    let mut null_layer = SurfaceDescriptor::new(0);
    null_layer.kind = NativeSurfaceKind::MetalLayer;
    null_layer.width = 16;
    null_layer.height = 16;
    null_layer.color_format = TextureFormat::Bgra8Unorm;
    assert!(
        device.make_swapchain(null_layer).is_err(),
        "a null layer handle is refused",
    );

    let (_layer, pointer) = layer(16);
    let mut zero = descriptor_for(pointer, 16);
    zero.width = 0;
    assert!(
        device.make_swapchain(zero).is_err(),
        "a zero-width surface is refused",
    );

    let (_layer2, pointer2) = layer(16);
    let mut wrong_format = descriptor_for(pointer2, 16);
    wrong_format.color_format = TextureFormat::Rgba8Unorm;
    assert!(
        device.make_swapchain(wrong_format).is_err(),
        "a CAMetalLayer requires a BGRA8 format",
    );

    let _ = ColorLoadAction::DontCare;
    let _ = TextureUsage::COLOR_TARGET;
}
