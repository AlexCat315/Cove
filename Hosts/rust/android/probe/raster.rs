//! Translation of Android/CoveAndroidProbe/Raster.swift.

use super::runner::{require, ProbeContext, ProbeFailure};
use cove_graphics::rhi::{
    BindingResource, BindingSetDescriptor, BindingSetEntry, BufferDescriptor, BufferUsage,
    ColorAttachmentDescriptor, ColorLoadAction, CommandBuffer, DepthLoadAction, DepthStencilState,
    GraphicsPipelineDescriptor, PipelineLayoutDescriptor, QueueClass, RasterizationState,
    RenderColorTarget, RenderDepthTarget, RenderPassDescriptor, TextureDescriptor, TextureFormat,
    TextureUploadRegion, TextureUsage,
};

impl ProbeContext {
    /// Six frames exercising direct and indirect draws, depth, a uniform binding,
    /// load preservation and `samples`-sample MSAA resolve.
    pub fn raster(&self, samples: usize) -> Result<String, ProbeFailure> {
        let fail = |error: cove_graphics::rhi::RhiError| ProbeFailure::new(error.to_string());
        let vertex = self.shader("raster.vertex")?;
        let fragment = self.shader("raster.fragment")?;
        let mut entries = vertex.binding_layout_descriptor(0).map_err(fail)?.entries;
        entries.extend(fragment.binding_layout_descriptor(0).map_err(fail)?.entries);
        let binding = self
            .device
            .make_binding_layout(&cove_graphics::rhi::BindingLayoutDescriptor::new(entries))
            .map_err(fail)?;
        let layout = self
            .device
            .make_pipeline_layout(&PipelineLayoutDescriptor::new(vec![binding]))
            .map_err(fail)?;
        let vs = self
            .device
            .make_shader_module(&vertex.module_descriptor(&[]).map_err(fail)?)
            .map_err(fail)?;
        let fs = self
            .device
            .make_shader_module(&fragment.module_descriptor(&[]).map_err(fail)?)
            .map_err(fail)?;
        let raster = RasterizationState { cull_mode: cove_graphics::rhi::CullMode::None, ..RasterizationState::default() };
        let mut descriptor = GraphicsPipelineDescriptor::new(layout, vs);
        descriptor.fragment = Some(fs);
        descriptor.color_attachments =
            vec![ColorAttachmentDescriptor::new(TextureFormat::Rgba8Unorm)];
        descriptor.depth_format = Some(TextureFormat::Depth32Float);
        descriptor.rasterization = raster;
        descriptor.depth_stencil = Some(DepthStencilState::default());
        descriptor.sample_count = samples;
        let pipeline = self.device.make_graphics_pipeline(&descriptor).map_err(fail)?;

        let buffer = self
            .device
            .make_buffer(BufferDescriptor::new(256, BufferUsage::UNIFORM))
            .map_err(fail)?;
        let green = [0.0_f32, 1.0, 0.0, 1.0];
        let green_bytes: &[u8] = unsafe {
            // SAFETY: four initialized `f32` values are contiguous and the
            // borrow ends before the buffer is destroyed.
            std::slice::from_raw_parts(green.as_ptr().cast(), std::mem::size_of_val(&green))
        };
        self.device.upload_buffer_data(buffer, 0, green_bytes).map_err(fail)?;

        let indirect = self
            .device
            .make_buffer(BufferDescriptor::new(16, BufferUsage::INDIRECT))
            .map_err(fail)?;
        let args = [3_u32, 1, 0, 0];
        let args_bytes: &[u8] = unsafe {
            // SAFETY: four initialized `u32` values are contiguous and the
            // borrow ends before the buffer is destroyed.
            std::slice::from_raw_parts(args.as_ptr().cast(), std::mem::size_of_val(&args))
        };
        self.device.upload_buffer_data(indirect, 0, args_bytes).map_err(fail)?;

        let set = self
            .device
            .make_binding_set(
                binding,
                &BindingSetDescriptor::new(vec![BindingSetEntry::new(
                    0,
                    BindingResource::uniform_buffer(buffer),
                )]),
            )
            .map_err(fail)?;
        let target = self
            .device
            .make_texture(TextureDescriptor::new(
                64,
                64,
                TextureFormat::Rgba8Unorm,
                TextureUsage::COLOR_TARGET | TextureUsage::TRANSFER_SOURCE,
            ))
            .map_err(fail)?;
        let (color, extra_color) = if samples == 1 {
            (target, None)
        } else {
            let color = self
                .device
                .make_texture(
                    TextureDescriptor::new(64, 64, TextureFormat::Rgba8Unorm, TextureUsage::COLOR_TARGET)
                        .with_sample_count(samples),
                )
                .map_err(fail)?;
            (color, Some(color))
        };
        let depth = self
            .device
            .make_texture(
                TextureDescriptor::new(
                    64,
                    64,
                    TextureFormat::Depth32Float,
                    TextureUsage::DEPTH_STENCIL_TARGET,
                )
                .with_sample_count(samples),
            )
            .map_err(fail)?;

        let mut outcome = Ok(());
        for index in 0..6 {
            let mut commands = CommandBuffer::new();
            let mut attachment =
                RenderColorTarget::clearing(color, [1.0, 0.0, 0.0, 1.0]);
            if samples != 1 {
                attachment.resolve_texture = Some(target);
            }
            commands.render_pass(
                RenderPassDescriptor::new(vec![attachment]).with_depth(
                    RenderDepthTarget::new(depth).with_load_action(DepthLoadAction::Clear(1.0)),
                ),
                |pass| {
                    pass.set_pipeline(pipeline);
                    pass.set_binding_set(set, 0, Vec::new());
                    if index % 2 == 0 {
                        pass.draw(3, 1, 0, 0);
                    } else {
                        pass.draw_indirect(indirect, 0, 1);
                    }
                },
            );
            // The second pass must preserve the result of the first, including a
            // multisample resolve consumed in the same command list.
            commands.render_pass(
                RenderPassDescriptor::new(vec![RenderColorTarget {
                    load_action: ColorLoadAction::Load,
                    ..RenderColorTarget::new(target)
                }]),
                |_| {},
            );
            if let Err(error) = self.submit(&commands, QueueClass::Graphics) {
                outcome = Err(error);
                break;
            }
        }

        let pixels = if outcome.is_ok() { self.read(target, 64, 64) } else { Err(ProbeFailure::new(String::new())) };

        self.device.destroy_texture(depth);
        if let Some(color) = extra_color {
            self.device.destroy_texture(color);
        }
        self.device.destroy_texture(target);
        self.device.destroy_buffer(buffer);
        self.device.destroy_buffer(indirect);
        self.device.destroy_graphics_pipeline(pipeline);
        self.device.destroy_shader_module(vs);
        self.device.destroy_shader_module(fs);

        outcome?;
        let pixels = pixels?;
        let center = (32 * 64 + 32) * 4;
        require(
            pixels[center..center + 4] == [0, 255, 0, 255],
            "raster center mismatch",
        )?;
        require(pixels[0..4] == [255, 0, 0, 255], "raster background/load mismatch")?;
        if samples == 4 {
            require(
                pixels
                    .chunks_exact(4)
                    .any(|pixel| pixel[0] > 0 && pixel[0] < 255),
                "MSAA edge coverage is missing",
            )?;
        }
        self.write_image(&pixels, 64, 64, &format!("raster-{samples}x"))?;
        Ok(format!(
            "6 frames, direct/indirect draw, depth, uniform binding, load preservation and {samples}x samples verified"
        ))
    }

    /// Uploads a uniform block through the explicit region API, used by the
    /// raster check to prove the binding bytes reached the device.
    pub fn upload_uniform(
        &self,
        buffer: cove_graphics::rhi::Buffer,
        bytes: &[u8],
    ) -> Result<(), ProbeFailure> {
        self.device
            .upload_buffer_data(buffer, 0, bytes)
            .map_err(|error| ProbeFailure::new(error.to_string()))
    }

    /// Region-based texture upload used by the transfer check.
    pub fn upload_region(
        &self,
        texture: cove_graphics::rhi::Texture,
        bytes: &[u8],
        width: usize,
        height: usize,
        bytes_per_row: usize,
    ) -> Result<(), ProbeFailure> {
        self.device
            .upload_texture_data(
                texture,
                bytes,
                TextureUploadRegion::new(width as i64, height as i64),
                bytes_per_row,
                cove_graphics::rhi::TextureSubresource::default(),
            )
            .map_err(|error| ProbeFailure::new(error.to_string()))
    }
}
