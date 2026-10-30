//! Translation of Android/CoveAndroidProbe/UI.swift.

use super::runner::{require, ProbeContext, ProbeFailure};
use cove_core::{Color, DrawList, Rect, TextureId};
use cove_graphics::rhi::{
    CommandBuffer, QueueClass, RenderColorTarget, TextureDescriptor, TextureFormat,
    TextureUploadRegion, TextureUsage,
};
use cove_graphics::ui::NativeDrawListRenderer;
use cove_graphics::ui::NativeUIViewport;

/// A one-pixel magenta texture retained by the probe draw list.
struct ProbeImage {
    texture_id: TextureId,
    pixels: Vec<u8>,
}

impl ProbeImage {
    fn new() -> Self {
        Self { texture_id: 1024, pixels: vec![255, 0, 255, 255] }
    }
}

impl ProbeContext {
    /// Exercises the production DrawList renderer: solid fills, clipping,
    /// retained image upload/sampling and `samples`-sample resolve.
    pub fn ui(&self, samples: usize) -> Result<String, ProbeFailure> {
        let fail = |error: cove_graphics::rhi::RhiError| ProbeFailure::new(error.to_string());
        let mut renderer = NativeDrawListRenderer::new(self.device.clone()).map_err(fail)?;
        renderer.configure(TextureFormat::Rgba8Unorm, samples).map_err(fail)?;

        let mut list = DrawList::default();
        list.add_rect(
            Rect::new(8.0, 8.0, 48.0, 24.0),
            Color { r: 0.0, g: 0.5, b: 1.0, a: 1.0 },
        )
        .map_err(|error: cove_core::DrawError| ProbeFailure::new(error.to_string()))?;
        list.push_clip_rect(Rect::new(20.0, 40.0, 24.0, 16.0));
        list.add_rect(
            Rect::new(0.0, 36.0, 64.0, 24.0),
            Color { r: 1.0, g: 0.5, b: 0.0, a: 1.0 },
        )
        .map_err(|error: cove_core::DrawError| ProbeFailure::new(error.to_string()))?;
        list.pop_clip_rect();
        let image = ProbeImage::new();
        renderer
            .register_texture(
                image.texture_id,
                image.pixels.clone(),
                [1, 1],
                TextureUploadRegion::new(1, 1),
                TextureFormat::Rgba8Unorm,
            )
            .map_err(|error: cove_core::DrawError| ProbeFailure::new(error.to_string()))?;
        list.add_image_quad(
            Rect::new(48.0, 40.0, 8.0, 16.0),
            image.texture_id,
            Color::WHITE,
            cove_core::Point { x: 0.0, y: 0.0 },
            cove_core::Point { x: 1.0, y: 1.0 },
        )
        .map_err(|error: cove_core::DrawError| ProbeFailure::new(error.to_string()))?;

        let target = self
            .device
            .make_texture(TextureDescriptor::new(
                64,
                64,
                TextureFormat::Rgba8Unorm,
                TextureUsage::COLOR_TARGET | TextureUsage::TRANSFER_SOURCE,
            ))
            .map_err(|error: cove_core::DrawError| ProbeFailure::new(error.to_string()))?;
        let (color, extra_color) = if samples == 1 {
            (target, None)
        } else {
            let color = self
                .device
                .make_texture(
                    TextureDescriptor::new(64, 64, TextureFormat::Rgba8Unorm, TextureUsage::COLOR_TARGET)
                        .with_sample_count(samples),
                )
                .map_err(|error: cove_core::DrawError| ProbeFailure::new(error.to_string()))?;
            (color, Some(color))
        };

        let mut commands = CommandBuffer::new();
        let mut attachment = RenderColorTarget::clearing(color, [0.0, 0.0, 0.0, 1.0]);
        if samples != 1 {
            attachment.resolve_texture = Some(target);
        }
        let viewport = NativeUIViewport::new([64, 64], [64.0, 64.0]);
        let record = self.device.with_frame_session(|| {
            self.device.begin_frame()?;
            let frame = renderer.record(&list, &mut commands, attachment, viewport, None)?;
            let submitted = self.device.submit(&commands, QueueClass::Graphics, &[], &[], None);
            let mut frame = frame;
            if submitted.is_ok() {
                frame.did_submit();
            }
            self.device.end_frame();
            self.device.wait_until_idle()?;
            submitted
        });
        let pixels = match record {
            Ok(()) => self.read(target, 64, 64),
            Err(error) => Err(ProbeFailure::new(error.to_string())),
        };

        if let Some(color) = extra_color {
            self.device.destroy_texture(color);
        }
        self.device.destroy_texture(target);

        let pixels = pixels?;
        let pixel = |x: usize, y: usize| -> [u8; 4] {
            let start = (y * 64 + x) * 4;
            [pixels[start], pixels[start + 1], pixels[start + 2], pixels[start + 3]]
        };
        require(
            pixel(16, 16) == [0, 128, 255, 255],
            &format!("UI color mismatch: {:?}", pixel(16, 16)),
        )?;
        require(pixel(32, 48) == [255, 128, 0, 255], "UI clipped fill mismatch")?;
        require(pixel(52, 48) == [255, 0, 255, 255], "UI retained image/sampled texture mismatch")?;
        require(pixel(8, 48) == [0, 0, 0, 255], "UI scissor mismatch")?;
        self.write_image(&pixels, 64, 64, &format!("coveui-{samples}x"))?;
        Ok(format!(
            "production CoveUI DrawList + NativeDrawListRenderer: colors, clipping, retained image upload/sampling, {samples}x samples"
        ))
    }
}
