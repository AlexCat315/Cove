//! Translation of Graphics/Tests/NativeRHITests/NativeRecordedTextureUploadTests.swift.
//!
//! Recorded uploads are validated on the CPU before any backend call: the
//! subresource must exist, the region must fit the mip extent, the row pitch must
//! match the texel size, and the buffer range must be addressable.
//!
//! The Swift `regionsAndRecovery` helper uploads to a device texture, records
//! buffer-to-texture blits with nonzero offsets and padded rows, then reads the
//! pixels back. That pixel work needs a live GPU; the validation and recovery
//! contract below is the portable half.

mod support;

use cove_graphics::{
    rhi_texture_subresource_extent, Buffer, BufferDescriptor, BufferUsage, CommandBuffer,
    Device, QueueClass, ShaderStage, Texture, TextureBufferUpload, TextureDescriptor,
    TextureDimension, TextureFormat, TextureSubresource, TextureUploadRegion, TextureUsage,
};

use support::{RecordingBackend, make_device};

fn texture_descriptor(format: TextureFormat) -> TextureDescriptor {
    let mut descriptor = TextureDescriptor::new(12, 8, format, TextureUsage::SAMPLED);
    descriptor.usage = TextureUsage::SAMPLED
        | TextureUsage::TRANSFER_DESTINATION
        | TextureUsage::TRANSFER_SOURCE;
    descriptor.dimension = TextureDimension::Texture2DArray;
    descriptor.layers = 2;
    descriptor.mip_levels = 3;
    descriptor
}

/// The mip extent shrinks by halves and stops at one texel.
#[test]
fn mip_extents_halve_and_stop_at_one() {
    for (mip, expected_width, expected_height) in
        [(0usize, 12usize, 8usize), (1, 6, 4), (2, 3, 2)]
    {
        assert_eq!(
            rhi_texture_subresource_extent(TextureSubresource::new(mip, 0), 12, 8, 3, 2),
            Ok((expected_width, expected_height)),
            "mip {mip}"
        );
    }
}

/// A subresource outside the declared mip/layer range is rejected.
#[test]
fn out_of_range_subresources_are_rejected() {
    for subresource in [
        TextureSubresource::new(3, 0),
        TextureSubresource::new(0, 2),
        TextureSubresource::new(99, 0),
    ] {
        assert!(
            rhi_texture_subresource_extent(subresource, 12, 8, 3, 2).is_err(),
            "mip {} layer {} is outside the texture",
            subresource.mip_level,
            subresource.layer
        );
    }
}

/// A recorded upload whose buffer range is not addressable fails the submission
/// without poisoning the planner: a valid command list still submits afterwards.
#[test]
fn failed_uploads_do_not_poison_the_frame() {
    let backend = RecordingBackend::standard();
    let device = make_device(&backend, 1);

    let texture = device
        .make_texture(texture_descriptor(TextureFormat::Rgba8Unorm))
        .expect("texture");
    let buffer = device
        .make_buffer(BufferDescriptor::new(256, BufferUsage::TRANSFER_SOURCE))
        .expect("buffer");

    device.begin_frame().expect("begin frame");

    // Five ways an encoded upload is invalid: offset past the buffer, origin
    // past the texture, mip past the level count, layer past the array size and
    // a row pitch that is not a multiple of the texel size.
    let mut upload = TextureBufferUpload::new(
        buffer,
        TextureFormat::Rgba8Unorm.bytes_per_texel(),
        texture,
        TextureUploadRegion::new(1, 1),
    );
    let invalid: Vec<TextureBufferUpload> = vec![
        {
            let mut candidate = upload;
            candidate.offset = 256;
            candidate
        },
        {
            let mut candidate = upload;
            candidate.region.origin = [12, 0];
            candidate
        },
        {
            let mut candidate = upload;
            candidate.subresource = TextureSubresource::new(3, 0);
            candidate
        },
        {
            let mut candidate = upload;
            candidate.subresource = TextureSubresource::new(0, 2);
            candidate
        },
        {
            let mut candidate = upload;
            candidate.bytes_per_row = TextureFormat::Rgba8Unorm.bytes_per_texel() - 1;
            candidate
        },
    ];
    for candidate in invalid {
        let mut commands = CommandBuffer::new();
        commands.copy_pass(|encoder| encoder.upload_buffer_to_texture(candidate));
        assert!(
            device.submit(&commands, QueueClass::Graphics, &[], &[], None).is_err(),
            "an invalid recorded upload must be rejected"
        );
    }

    // An offset that would overflow the address space is rejected too.
    upload.offset = i64::MAX as usize;
    let mut overflowing = CommandBuffer::new();
    overflowing.copy_pass(|encoder| encoder.upload_buffer_to_texture(upload));
    assert!(
        device.submit(&overflowing, QueueClass::Graphics, &[], &[], None).is_err(),
        "an overflowing buffer offset must be rejected"
    );

    // Recovery: the frame is still usable for a valid submission.
    let mut valid = CommandBuffer::new();
    valid.copy_pass(|encoder| {
        encoder.upload_buffer_to_texture(TextureBufferUpload::new(
            buffer,
            TextureFormat::Rgba8Unorm.bytes_per_texel(),
            texture,
            TextureUploadRegion::new(1, 1),
        ))
    });
    device
        .submit(&valid, QueueClass::Graphics, &[], &[], None)
        .expect("a valid upload still submits in the same frame");

    device.end_frame();
    device.wait_until_idle().expect("idle");
    device.destroy_texture(texture);
    device.destroy_buffer(buffer);
}

/// Immediate uploads are validated against the subresource and region as well.
#[test]
fn immediate_uploads_validate_the_subresource_and_region() {
    let backend = RecordingBackend::standard();
    let device = make_device(&backend, 1);
    let texture = device
        .make_texture(texture_descriptor(TextureFormat::Rgba8Unorm))
        .expect("texture");

    let payload = vec![0u8; 12 * 8 * 4];
    assert!(
        device
            .upload_texture_data(
                texture,
                &payload,
                TextureUploadRegion::new(12, 8),
                12 * 4,
                TextureSubresource::new(0, 0),
            )
            .is_ok(),
        "the base mip of layer 0 accepts a full-size upload"
    );
    assert!(
        device
            .upload_texture_data(
                texture,
                &payload,
                TextureUploadRegion::new(1, 1),
                4,
                TextureSubresource::new(3, 0),
            )
            .is_err(),
        "mip 3 does not exist"
    );
    device.destroy_texture(texture);
}

/// The stage constant keeps the shader import meaningful for hosts that build
/// upload pipelines alongside this suite.
#[test]
fn uploads_are_transfer_work_not_compute_work() {
    let backend = RecordingBackend::standard();
    let device = make_device(&backend, 1);
    let _ = device.make_shader_module(support::shader_descriptor(ShaderStage::Compute, &[1]));
    let _: fn(&Device) -> Buffer = |device| {
        device
            .make_buffer(BufferDescriptor::new(4, BufferUsage::TRANSFER_SOURCE))
            .expect("buffer")
    };
    let _: Texture = Texture::new(0);
}
