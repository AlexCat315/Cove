//! Translation of Android/CoveAndroidProbe/Transfer.swift.

use super::runner::{require, ProbeContext, ProbeFailure};
use cove_graphics::rhi::{
    BindingResource, BindingSetDescriptor, BindingSetEntry, BufferDescriptor, BufferUsage,
    CommandBuffer, ComputePipelineDescriptor, PipelineLayoutDescriptor, QueueClass, TextureDescriptor,
    TextureFormat, TextureSubresource, TextureUploadRegion, TextureUsage,
};

impl ProbeContext {
    /// Uploads a byte pattern, copies it between textures on the transfer queue
    /// and reads it back unchanged.
    pub fn transfer(&self) -> Result<String, ProbeFailure> {
        let fail = |error: cove_graphics::rhi::RhiError| ProbeFailure::new(error.to_string());
        let input = self
            .device
            .make_texture(TextureDescriptor::new(
                8,
                8,
                TextureFormat::Rgba8Unorm,
                TextureUsage::TRANSFER_SOURCE | TextureUsage::TRANSFER_DESTINATION,
            ))
            .map_err(fail)?;
        let output = self
            .device
            .make_texture(TextureDescriptor::new(
                8,
                8,
                TextureFormat::Rgba8Unorm,
                TextureUsage::TRANSFER_SOURCE | TextureUsage::TRANSFER_DESTINATION,
            ))
            .map_err(fail)?;
        let bytes = (0_u16..256).map(|value| value as u8).collect::<Vec<_>>();
        self.device
            .upload_texture_data(
                input,
                &bytes,
                TextureUploadRegion::new(8, 8),
                32,
                TextureSubresource::default(),
            )
            .map_err(fail)?;
        let mut commands = CommandBuffer::new();
        commands.copy_pass(|pass| pass.copy_texture(input, output, 8, 8));
        let result = self.submit(&commands, QueueClass::Transfer);
        self.device.destroy_texture(input);
        self.device.destroy_texture(output);
        result?;
        let read = self.read(output, 8, 8);
        let read = match read {
            Ok(read) => read,
            Err(error) => return Err(error),
        };
        require(read == bytes, "texture upload/copy/readback mismatch")?;
        Ok("all 256 bytes match after upload, transfer queue copy and readback".to_owned())
    }

    /// Dispatches a storage-buffer compute kernel indirectly and verifies the
    /// uploaded results.
    pub fn compute(&self) -> Result<String, ProbeFailure> {
        let fail = |error: cove_graphics::rhi::RhiError| ProbeFailure::new(error.to_string());
        let artifact = self.shader("compute")?;
        let module = self
            .device
            .make_shader_module(&artifact.module_descriptor(&[]).map_err(fail)?)
            .map_err(fail)?;
        let binding = self
            .device
            .make_binding_layout(&artifact.binding_layout_descriptor(0).map_err(fail)?)
            .map_err(fail)?;
        let layout = self
            .device
            .make_pipeline_layout(&PipelineLayoutDescriptor::new(vec![binding]))
            .map_err(fail)?;
        let pipeline = self
            .device
            .make_compute_pipeline(&ComputePipelineDescriptor::new(layout, module))
            .map_err(fail)?;
        let buffer = self
            .device
            .make_buffer(BufferDescriptor::new(
                64,
                BufferUsage::STORAGE_WRITE | BufferUsage::TRANSFER_SOURCE,
            ))
            .map_err(fail)?;
        let indirect = self
            .device
            .make_buffer(BufferDescriptor::new(12, BufferUsage::INDIRECT))
            .map_err(fail)?;
        let groups = [2_u32, 1, 1];
        let bytes: &[u8] = unsafe {
            // SAFETY: three initialized `u32` values are contiguous and the
            // borrow ends before the buffers are destroyed.
            std::slice::from_raw_parts(groups.as_ptr().cast(), std::mem::size_of_val(&groups))
        };
        self.device.upload_buffer_data(indirect, 0, bytes).map_err(fail)?;
        let texture = self
            .device
            .make_texture(TextureDescriptor::new(
                16,
                1,
                TextureFormat::R32Uint,
                TextureUsage::TRANSFER_SOURCE | TextureUsage::TRANSFER_DESTINATION,
            ))
            .map_err(fail)?;
        let set = self
            .device
            .make_binding_set(
                binding,
                &BindingSetDescriptor::new(vec![BindingSetEntry::new(
                    0,
                    BindingResource::storage_buffer(buffer),
                )]),
            )
            .map_err(fail)?;
        let mut commands = CommandBuffer::new();
        commands.compute_pass(|pass| {
            pass.set_pipeline(pipeline);
            pass.set_binding_set(set, 0, Vec::new());
            pass.dispatch_indirect(indirect, 0);
        });
        commands.copy_pass(|pass| {
            pass.upload_buffer_to_texture(cove_graphics::rhi::TextureBufferUpload::new(
                buffer,
                64,
                texture,
                TextureUploadRegion::new(16, 1),
            ));
        });
        let result = self.submit(&commands, QueueClass::Compute);
        let read = if result.is_ok() {
            let mut values = vec![0_u8; 16 * 4];
            self.device
                .read_texture_data(texture, 16, 1, 64, TextureSubresource::default(), &mut values)
                .map(|()| values)
                .map_err(fail)
        } else {
            Err(ProbeFailure::new(String::new()))
        };
        self.device.destroy_buffer(buffer);
        self.device.destroy_buffer(indirect);
        self.device.destroy_texture(texture);
        self.device.destroy_compute_pipeline(pipeline);
        self.device.destroy_shader_module(module);
        result?;
        let values = read?;
        let decoded = values
            .chunks_exact(4)
            .map(|word| u32::from_ne_bytes([word[0], word[1], word[2], word[3]]))
            .collect::<Vec<_>>();
        let expected = (7_u32..23).collect::<Vec<_>>();
        require(
            decoded == expected,
            &format!("compute storage buffer mismatch: {decoded:?}"),
        )?;
        Ok("16 indirect-dispatch compute results match on the compute queue".to_owned())
    }
}
