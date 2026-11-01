//! Translation of NativeRHI/Validation/{ResourceValidation,
//! RenderAttachmentValidation}.swift: byte-range, texture-transfer and render
//! attachment validation.
//!
//! Every check returns `RhiError::InvalidArgument` rather than trapping, so a
//! bad descriptor reaches the caller as a value.
use super::{
    RenderPassDescriptor, RhiResult, Texture, TextureDescriptor, TextureDimension, TextureFormat,
    TextureSubresource, TextureUploadRegion, TextureUsage, rhi_require,
};

/// Validates a byte range inside a resource of `capacity` bytes.
pub fn rhi_byte_range(offset: usize, size: usize, capacity: usize) -> RhiResult<()> {
    rhi_require(
        size <= capacity && offset <= capacity - size,
        "resource byte range is out of bounds",
    )
}

/// Narrows a signed count to the `u32` the GPU APIs accept.
pub fn rhi_count(value: i64) -> RhiResult<u32> {
    u32::try_from(value)
        .map_err(|_| super::RhiError::invalid_argument("GPU count cannot be represented as UInt32"))
}

/// Total bytes described by a color transfer, validating the row pitch and
/// bounds against the texture extent and buffer capacity.
pub fn rhi_texture_transfer_bytes(
    width: usize,
    height: usize,
    row_bytes: usize,
    format: TextureFormat,
    texture_width: usize,
    texture_height: usize,
    capacity: usize,
) -> RhiResult<usize> {
    let texel_bytes = format.bytes_per_texel();
    let minimum_row = width.checked_mul(texel_bytes);
    let bytes = row_bytes.checked_mul(height);
    let (Some(minimum_row), Some(bytes)) = (minimum_row, bytes) else {
        return Err(super::RhiError::invalid_argument(
            "invalid color texture transfer bounds or row pitch",
        ));
    };
    rhi_require(
        !format.is_depth()
            && texel_bytes > 0
            && width > 0
            && height > 0
            && width <= texture_width
            && height <= texture_height
            && row_bytes >= minimum_row
            && row_bytes.is_multiple_of(texel_bytes)
            && bytes <= capacity,
        "invalid color texture transfer bounds or row pitch",
    )?;
    Ok(bytes)
}

/// The mip extent of `subresource`, validating that the mip and layer exist.
pub fn rhi_texture_subresource_extent(
    subresource: TextureSubresource,
    width: usize,
    height: usize,
    mip_levels: usize,
    layers: usize,
) -> RhiResult<(usize, usize)> {
    rhi_require(
        subresource.mip_level < mip_levels
            && subresource.mip_level < usize::BITS as usize
            && subresource.layer < layers,
        "texture subresource is out of bounds",
    )?;
    Ok((
        (width >> subresource.mip_level).max(1),
        (height >> subresource.mip_level).max(1),
    ))
}

/// Bytes described by a texture upload region, validating its origin against the
/// texture extent.
pub fn rhi_texture_upload_bytes(
    region: TextureUploadRegion,
    row_bytes: usize,
    format: TextureFormat,
    texture_width: usize,
    texture_height: usize,
    capacity: usize,
) -> RhiResult<usize> {
    let (origin_x, origin_y) = (region.origin[0], region.origin[1]);
    rhi_require(
        origin_x >= 0
            && origin_y >= 0
            && origin_x as usize <= texture_width
            && origin_y as usize <= texture_height,
        "texture upload origin is out of bounds",
    )?;
    rhi_texture_transfer_bytes(
        region.width as usize,
        region.height as usize,
        row_bytes,
        format,
        texture_width - origin_x as usize,
        texture_height - origin_y as usize,
        capacity,
    )
}

/// A texture copy extent together with the source and destination formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureCopyExtent {
    pub width: usize,
    pub height: usize,
    pub format: TextureFormat,
}

/// Validates a top-left color texture copy.
pub fn rhi_color_texture_copy_extent(
    width: usize,
    height: usize,
    source: TextureCopyExtent,
    destination: TextureCopyExtent,
) -> RhiResult<()> {
    rhi_require(
        width > 0
            && height > 0
            && width <= source.width.min(destination.width)
            && height <= source.height.min(destination.height)
            && source.format == destination.format
            && source.format.bytes_per_texel() > 0
            && !source.format.is_depth(),
        "invalid color texture copy extent or format",
    )
}

// MARK: - Render attachments

pub fn rhi_validate_sample_count(count: usize) -> RhiResult<()> {
    rhi_require(
        matches!(count, 1 | 2 | 4 | 8 | 16 | 32 | 64),
        "invalid sample count",
    )
}

pub fn rhi_validate_texture_samples(descriptor: &TextureDescriptor) -> RhiResult<()> {
    rhi_validate_sample_count(descriptor.sample_count)?;
    if descriptor.transient_attachment {
        let attachment_only =
            descriptor.usage & !(TextureUsage::COLOR_TARGET | TextureUsage::DEPTH_STENCIL_TARGET);
        rhi_require(
            descriptor.sample_count > 1 && attachment_only.is_empty(),
            "transient textures require multisample attachment-only usage",
        )?;
    }
    if descriptor.sample_count <= 1 {
        return Ok(());
    }
    rhi_require(
        descriptor.dimension == TextureDimension::Texture2D
            && descriptor.depth == 1
            && descriptor.layers == 1
            && descriptor.mip_levels == 1,
        "multisampled textures require one 2D layer and one mip",
    )?;
    rhi_require(
        !descriptor.usage.contains(TextureUsage::STORAGE_WRITE)
            && !descriptor.usage.contains(TextureUsage::STORAGE_READ)
            && !descriptor.usage.contains(TextureUsage::SAMPLED)
            && !descriptor.usage.contains(TextureUsage::PRESENT),
        "multisampled shader bindings or presentation textures are unsupported; resolve before sampling",
    )?;
    rhi_require(
        descriptor.usage.contains(TextureUsage::COLOR_TARGET)
            || descriptor
                .usage
                .contains(TextureUsage::DEPTH_STENCIL_TARGET),
        "multisampled textures require attachment usage",
    )
}

/// Actual backend resource metadata, rather than caller-provided render
/// extents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderTextureInfo<Format> {
    pub supports_color_resolve: bool,
    pub extent: [usize; 2],
    pub sample_count: usize,
    pub format: Format,
    pub usage: TextureUsage,
    pub single_layer_2d: bool,
    pub is_depth: bool,
}
impl<Format> RenderTextureInfo<Format> {
    pub fn new(extent: [usize; 2], sample_count: usize, format: Format) -> Self {
        Self {
            supports_color_resolve: true,
            extent,
            sample_count,
            format,
            usage: TextureUsage::EMPTY,
            single_layer_2d: true,
            is_depth: false,
        }
    }
}

/// The attachment shape a pipeline was compiled against.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderPassSignature<Format> {
    pub extent: [usize; 2],
    pub sample_count: usize,
    pub colors: Vec<Format>,
    pub depth: Option<Format>,
}
impl<Format: PartialEq> RenderPassSignature<Format> {
    pub fn validate(
        &self,
        colors: &[Format],
        depth: Option<&Format>,
        samples: usize,
    ) -> RhiResult<()> {
        rhi_require(
            self.colors == colors && self.depth.as_ref() == depth,
            "pipeline formats do not match render attachments",
        )?;
        rhi_require(
            self.sample_count == samples,
            "pipeline sample count does not match render attachments",
        )
    }
}

/// Builds and validates the attachment signature of one render pass.
pub fn rhi_render_pass_signature<Format: PartialEq>(
    pass: &RenderPassDescriptor,
    describe: impl Fn(Texture) -> RhiResult<RenderTextureInfo<Format>>,
) -> RhiResult<RenderPassSignature<Format>> {
    let mut sources: Vec<Texture> = pass
        .color_targets
        .iter()
        .map(|target| target.texture)
        .collect();
    if let Some(depth) = pass.depth_target {
        sources.push(depth.texture);
    }
    let resolves: Vec<Texture> = pass
        .color_targets
        .iter()
        .filter_map(|target| target.resolve_texture)
        .collect();

    rhi_require(
        !sources.is_empty() && pass.color_targets.len() <= 8,
        "render pass requires attachments and at most eight color targets",
    )?;

    let mut attachment_ids: Vec<u32> = sources
        .iter()
        .chain(resolves.iter())
        .map(|texture| texture.id())
        .collect();
    let total = attachment_ids.len();
    attachment_ids.sort_unstable();
    attachment_ids.dedup();
    rhi_require(
        attachment_ids.len() == total,
        "render attachments and resolve destinations must be distinct",
    )?;

    let first = describe(sources[0])?;
    for texture in &sources {
        let info = describe(*texture)?;
        rhi_require(
            info.extent == first.extent && info.sample_count == first.sample_count,
            "render attachments have different dimensions or sample counts",
        )?;
        rhi_require(
            info.single_layer_2d,
            "render attachments require single-layer 2D textures",
        )?;
    }

    let mut colors = Vec::with_capacity(pass.color_targets.len());
    for target in &pass.color_targets {
        let source = describe(target.texture)?;
        rhi_require(
            source.usage.contains(TextureUsage::COLOR_TARGET) && !source.is_depth,
            "invalid render color attachment",
        )?;
        if let Some(resolve_texture) = target.resolve_texture {
            let resolve = describe(resolve_texture)?;
            rhi_require(
                source.sample_count > 1
                    && resolve.sample_count == 1
                    && resolve.extent == source.extent
                    && resolve.format == source.format
                    && resolve.single_layer_2d
                    && resolve.usage.contains(TextureUsage::COLOR_TARGET)
                    && !resolve.is_depth,
                "resolve requires matching multisample and single-sample color targets",
            )?;
            rhi_require(
                source.supports_color_resolve,
                "color format does not support average multisample resolve",
            )?;
        }
        colors.push(source.format);
    }

    let depth = match pass.depth_target {
        Some(target) => {
            let info = describe(target.texture)?;
            rhi_require(
                info.usage.contains(TextureUsage::DEPTH_STENCIL_TARGET) && info.is_depth,
                "invalid render depth attachment",
            )?;
            Some(info.format)
        }
        None => None,
    };

    Ok(RenderPassSignature {
        extent: first.extent,
        sample_count: first.sample_count,
        colors,
        depth,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_ranges_reject_overflow() {
        assert!(rhi_byte_range(0, 16, 16).is_ok());
        assert!(rhi_byte_range(16, 0, 16).is_ok());
        assert!(rhi_byte_range(1, 16, 16).is_err());
        assert!(rhi_byte_range(0, 17, 16).is_err());
        assert_eq!(rhi_count(4), Ok(4));
        assert!(rhi_count(-1).is_err());
        assert!(rhi_count(i64::MAX).is_err());
    }

    #[test]
    fn texture_transfers_validate_pitch_and_depth() {
        assert_eq!(
            rhi_texture_transfer_bytes(4, 4, 16, TextureFormat::Rgba8Unorm, 8, 8, 64),
            Ok(64)
        );
        // Row pitch narrower than one texel row.
        assert!(rhi_texture_transfer_bytes(4, 4, 8, TextureFormat::Rgba8Unorm, 8, 8, 64).is_err());
        // Depth formats are not valid color transfer sources.
        assert!(
            rhi_texture_transfer_bytes(4, 4, 16, TextureFormat::Depth32Float, 8, 8, 64).is_err()
        );
        // Buffer too small.
        assert!(rhi_texture_transfer_bytes(4, 4, 16, TextureFormat::Rgba8Unorm, 8, 8, 63).is_err());
    }

    #[test]
    fn subresource_extents_halve_per_mip() {
        assert_eq!(
            rhi_texture_subresource_extent(TextureSubresource::new(0, 0), 64, 32, 4, 1),
            Ok((64, 32))
        );
        assert_eq!(
            rhi_texture_subresource_extent(TextureSubresource::new(2, 0), 64, 32, 4, 1),
            Ok((16, 8))
        );
        assert_eq!(
            rhi_texture_subresource_extent(TextureSubresource::new(3, 0), 8, 8, 4, 1),
            Ok((1, 1))
        );
        assert!(rhi_texture_subresource_extent(TextureSubresource::new(4, 0), 8, 8, 4, 1).is_err());
        assert!(rhi_texture_subresource_extent(TextureSubresource::new(0, 1), 8, 8, 4, 1).is_err());
    }

    #[test]
    fn sample_counts_are_power_of_two_limits() {
        for count in [1, 2, 4, 8, 16, 32, 64] {
            assert!(rhi_validate_sample_count(count).is_ok());
        }
        assert!(rhi_validate_sample_count(3).is_err());
        assert!(rhi_validate_sample_count(0).is_err());
    }

    #[test]
    fn multisampled_textures_must_be_attachment_only() {
        let mut descriptor =
            TextureDescriptor::new(8, 8, TextureFormat::Rgba8Unorm, TextureUsage::COLOR_TARGET)
                .with_sample_count(4);
        assert!(rhi_validate_texture_samples(&descriptor).is_ok());
        descriptor.usage |= TextureUsage::SAMPLED;
        assert!(rhi_validate_texture_samples(&descriptor).is_err());
        descriptor.usage = TextureUsage::COLOR_TARGET;
        descriptor.mip_levels = 2;
        assert!(rhi_validate_texture_samples(&descriptor).is_err());
    }

    #[test]
    fn render_passes_require_distinct_attachments() {
        let pass = RenderPassDescriptor::new(vec![
            super::super::RenderColorTarget::new(Texture::new(1)),
            super::super::RenderColorTarget::new(Texture::new(1)),
        ]);
        let describe = |_texture: Texture| {
            let mut info = RenderTextureInfo::new([8, 8], 1, TextureFormat::Rgba8Unorm);
            info.usage = TextureUsage::COLOR_TARGET;
            Ok(info)
        };
        assert!(rhi_render_pass_signature(&pass, describe).is_err());

        let empty = RenderPassDescriptor::new(Vec::new());
        assert!(rhi_render_pass_signature(&empty, describe).is_err());
    }
}
