use crate::rhi::*;
use cove_imaging::{PixelSize, TextureImage};
use std::sync::Arc;
pub struct NativeTextureImageBacking {
    pub resource: Arc<TextureResource>,
}
pub fn texture_image(
    texture: Arc<TextureResource>,
    mipmaps: Vec<Arc<TextureResource>>,
) -> RhiResult<Arc<TextureImage>> {
    let d = texture.descriptor();
    if d.dimension != TextureDimension::Texture2D
        || d.sample_count != 1
        || !d.usage.contains(TextureUsage::SAMPLED)
        || !matches!(
            d.format,
            TextureFormat::Rgba8Unorm | TextureFormat::Bgra8Unorm
        )
    {
        return Err(RhiError::invalid_argument(
            "UI images require a sampled RGBA/BGRA 2D texture",
        ));
    }
    if !mipmaps
        .iter()
        .all(|m| Arc::ptr_eq(m.device(), texture.device()) && m.descriptor().format == d.format)
    {
        return Err(RhiError::invalid_argument(
            "image mip levels must use the same device and color format",
        ));
    }
    let pixel_size = PixelSize::new(
        u32::try_from(d.width).map_err(|_| RhiError::invalid_argument("image width overflow"))?,
        u32::try_from(d.height).map_err(|_| RhiError::invalid_argument("image height overflow"))?,
    )
    .map_err(|e| RhiError::invalid_argument(e.to_string()))?;
    let levels = mipmaps
        .into_iter()
        .map(|m| texture_image(m, Vec::new()))
        .collect::<RhiResult<Vec<_>>>()?;
    TextureImage::new(
        Arc::new(NativeTextureImageBacking { resource: texture }),
        pixel_size,
        levels,
    )
    .map_err(|e| RhiError::invalid_argument(e.to_string()))
}
