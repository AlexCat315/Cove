use std::{any::Any, sync::Arc};
use crate::{ImageDecodeError, PixelSize};
use cove_core::{Size, TextureId, image_lod};
pub trait TextureImageBacking: Any + Send + Sync {}
impl<T: Any + Send + Sync> TextureImageBacking for T {}
/// Backend resource lifetime is tied to this retained image, including frame snapshots.
pub struct TextureImage { pub texture_id: TextureId, pub pixel_size: PixelSize, pub backing: Arc<dyn TextureImageBacking>, pub mipmaps: Vec<Arc<Self>> }
impl TextureImage {
    pub fn new(backing: Arc<dyn TextureImageBacking>, pixel_size: PixelSize, mipmaps: Vec<Arc<Self>>) -> Result<Arc<Self>, ImageDecodeError> {
        PixelSize::new(pixel_size.width, pixel_size.height)?;
        Ok(Arc::new(Self { texture_id: super::image_asset_registry::allocate_id()?, pixel_size, backing, mipmaps }))
    }
    pub fn resolution(self: &Arc<Self>, target: Size, source: Option<Size>) -> Arc<Self> {
        let source = source.unwrap_or(Size { width: self.pixel_size.width as f32, height: self.pixel_size.height as f32 });
        let level = image_lod(source, target, self.mipmaps.len());
        if level == 0 { self.clone() } else { self.mipmaps[level - 1].clone() }
    }
}
impl PartialEq for TextureImage { fn eq(&self, other: &Self) -> bool { std::ptr::eq(self, other) } }
impl Eq for TextureImage {}
