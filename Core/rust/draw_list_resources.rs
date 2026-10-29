//! Translation of CoveUICore/DrawListResources.swift.
use std::{any::Any, collections::HashSet, sync::Arc};
#[derive(Clone, Default)]
pub struct DrawListResources { retained: Vec<Arc<dyn Any + Send + Sync>>, identities: HashSet<usize> }
impl std::fmt::Debug for DrawListResources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.debug_struct("DrawListResources").field("count", &self.len()).finish() }
}
impl DrawListResources {
    pub fn len(&self) -> usize { self.retained.len() }
    pub fn is_empty(&self) -> bool { self.retained.is_empty() }
    pub fn retain<T: Any + Send + Sync>(&mut self, resource: Arc<T>) { self.retain_erased(resource); }
    pub fn retain_erased(&mut self, resource: Arc<dyn Any + Send + Sync>) {
        let identity = Arc::as_ptr(&resource) as *const () as usize;
        if self.identities.insert(identity) { self.retained.push(resource); }
    }
    pub fn append(&mut self, other: &Self) { for resource in &other.retained { self.retain_erased(resource.clone()); } }
    pub fn for_each<T: Any>(&self, mut visit: impl FnMut(&T)) {
        for resource in &self.retained { if let Some(value) = resource.downcast_ref::<T>() { visit(value); } }
    }
    pub fn for_each_retained<T: Any + Send + Sync>(&self, mut visit: impl FnMut(Arc<T>)) {
        for resource in &self.retained { if let Ok(value) = resource.clone().downcast::<T>() { visit(value); } }
    }
    pub fn retain_texture_asset(&mut self, asset: Arc<dyn TextureAsset>) {
        let identity = Arc::as_ptr(&asset) as *const () as usize;
        if self.identities.insert(identity) { self.retained.push(Arc::new(RetainedTextureAsset(asset))); }
    }
    pub fn reset(&mut self) { self.retained.clear(); self.identities.clear(); }
}
pub type TextureId = u32;
pub fn color_glyph_atlas_id(texture_id: TextureId) -> TextureId {
    assert!(texture_id > 0 && texture_id < 0x8000_0000); texture_id | 0x8000_0000
}
pub trait TextureAsset: Any + Send + Sync { fn texture_id(&self) -> TextureId; fn image(&self) -> &crate::Bitmap; }

/// Type-erased sampled CPU image, retaining the original owner through frame retirement.
pub struct RetainedTextureAsset(pub Arc<dyn TextureAsset>);
