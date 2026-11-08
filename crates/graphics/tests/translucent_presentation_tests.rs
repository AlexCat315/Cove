//! A window that shows a platform material through clear pixels hands the
//! compositor premultiplied colour. The frame blends in linear light, so its
//! premultiplied colour is linear; an eight-bit sRGB target stores that value
//! encoded, and a compositor reading encoded premultiplied colour then sees a
//! partly covered edge whose colour exceeds its alpha — a light shape's
//! rounded corner composites past white over any backdrop.
//!
//! Behaviour checklist:
//! - copied into a linear float target, every pixel of a light rounded fill
//!   over a clear frame keeps colour at or below alpha, so compositing it over
//!   the brightest backdrop never exceeds white
//! - the eight-bit sRGB copy of the same frame breaks that at the corners,
//!   which is the defect the float target exists to avoid

#![cfg(target_os = "macos")]

use std::sync::Arc;

use cove_core::{Color, DrawList, Rect};
use cove_graphics::rhi::{
    ColorLoadAction, CommandBuffer, Device, DeviceConfig, GraphicsApi, QueueClass,
    RenderColorTarget, TextureDescriptor, TextureFormat, TextureResource, TextureSubresource,
    TextureUsage,
};
use cove_graphics::ui::{NativeDrawListRenderer, NativeUIViewport};

const SIZE: [usize; 2] = [96, 64];

fn device() -> Option<Arc<Device>> {
    match Device::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Metal],
        enable_validation: true,
        frames_in_flight: 3,
        ..DeviceConfig::default()
    }) {
        Ok(device) => Some(Arc::new(device)),
        Err(error) => {
            eprintln!("no Metal device available ({error}); skipping");
            None
        }
    }
}

fn texture(device: &Arc<Device>, format: TextureFormat, usage: TextureUsage) -> TextureResource {
    TextureResource::new(
        device.clone(),
        TextureDescriptor::new(SIZE[0], SIZE[1], format, usage),
    )
    .expect("texture")
}

/// Renders a light, nearly opaque rounded fill over a clear frame the way the
/// desktop host does — sRGB frame, 4x MSAA — then copies it into `target`.
fn present(format: TextureFormat) -> Vec<[f32; 4]> {
    let Some(device) = device() else {
        return Vec::new();
    };
    let mut renderer = NativeDrawListRenderer::new(device.clone()).expect("renderer");
    renderer
        .configure(TextureFormat::Bgra8UnormSrgb, 4)
        .expect("configure");
    let frame = texture(
        &device,
        TextureFormat::Bgra8UnormSrgb,
        TextureUsage::COLOR_TARGET | TextureUsage::SAMPLED,
    );
    let output = texture(
        &device,
        format,
        TextureUsage::COLOR_TARGET | TextureUsage::TRANSFER_SOURCE,
    );
    let mut list = DrawList::default();
    list.pixel_scale = 2.0;
    list.add_rounded_rect(
        Rect::new(4.25, 4.25, 38.0, 22.0),
        8.0,
        Color {
            r: 0.875,
            g: 0.875,
            b: 0.88,
            a: 0.97,
        },
    )
    .expect("fill");
    let viewport = NativeUIViewport::new(SIZE, [SIZE[0] as f32 / 2.0, SIZE[1] as f32 / 2.0]);
    device.begin_frame().expect("begin frame");
    let mut commands = CommandBuffer::new();
    let mut recorded = renderer
        .record_presentation(
            &list,
            &mut commands,
            frame.texture(),
            viewport,
            [0.0; 4],
            usize::MAX,
        )
        .expect("record");
    renderer
        .record_blit(
            frame.texture(),
            &mut commands,
            RenderColorTarget {
                resolve_texture: None,
                texture: output.texture(),
                load_action: ColorLoadAction::DontCare,
                store: true,
            },
            format,
            SIZE,
        )
        .expect("blit");
    device
        .submit(&commands, QueueClass::Graphics, &[], &[], None)
        .expect("submit");
    recorded.did_submit();
    device.end_frame();
    device.wait_until_idle().expect("idle");
    let bytes_per_pixel = if format == TextureFormat::Rgba16Float {
        8
    } else {
        4
    };
    let mut data = vec![0u8; SIZE[0] * SIZE[1] * bytes_per_pixel];
    device
        .read_texture_data(
            output.texture(),
            SIZE[0],
            SIZE[1],
            SIZE[0] * bytes_per_pixel,
            TextureSubresource::default(),
            &mut data,
        )
        .expect("read back");
    if format == TextureFormat::Rgba16Float {
        data.chunks(8)
            .map(|pixel| {
                let channel = |i: usize| half(u16::from_le_bytes([pixel[i * 2], pixel[i * 2 + 1]]));
                [channel(0), channel(1), channel(2), channel(3)]
            })
            .collect()
    } else {
        // BGRA bytes, read as the encoded premultiplied colour a compositor sees.
        data.chunks(4)
            .map(|pixel| {
                let unit = |value: u8| f32::from(value) / 255.0;
                [
                    unit(pixel[2]),
                    unit(pixel[1]),
                    unit(pixel[0]),
                    unit(pixel[3]),
                ]
            })
            .collect()
    }
}

fn half(bits: u16) -> f32 {
    let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exponent = i32::from((bits >> 10) & 0x1f);
    let mantissa = f32::from(bits & 0x3ff);
    sign * match exponent {
        0 => mantissa / 1024.0 * 2f32.powi(-14),
        31 => f32::INFINITY,
        _ => (1.0 + mantissa / 1024.0) * 2f32.powi(exponent - 15),
    }
}

/// Pixels that composite past white over a white backdrop: colour + (1 - a) > 1.
fn overflowing(pixels: &[[f32; 4]]) -> usize {
    pixels
        .iter()
        .filter(|pixel| (0..3).any(|channel| pixel[channel] > pixel[3] + 1.0 / 512.0))
        .count()
}

#[test]
fn a_float_presentation_keeps_translucent_edges_premultiplied() {
    let pixels = present(TextureFormat::Rgba16Float);
    if pixels.is_empty() {
        return;
    }
    let edges = pixels
        .iter()
        .filter(|pixel| pixel[3] > 0.02 && pixel[3] < 0.9)
        .count();
    assert!(
        edges > 0,
        "the rounded corners produce partly covered pixels"
    );
    assert_eq!(
        overflowing(&pixels),
        0,
        "no pixel's colour exceeds its alpha, so none composites past white"
    );
}

#[test]
fn an_srgb_presentation_overflows_at_translucent_edges() {
    let pixels = present(TextureFormat::Bgra8UnormSrgb);
    if pixels.is_empty() {
        return;
    }
    assert!(
        overflowing(&pixels) > 0,
        "encoded linear premultiplied colour exceeds alpha at the corners"
    );
}
