//! Translation of CoveUICore/DrawListResources.swift.
use std::{
    any::Any,
    collections::HashSet,
    hash::{BuildHasherDefault, Hasher},
    sync::Arc,
};

type Resource = Arc<dyn Any + Send + Sync>;

/// Hashes an `Arc` address: already unique, so one multiply spreads it.
#[derive(Default)]
struct PointerHasher(u64);
impl Hasher for PointerHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = (self.0.rotate_left(5) ^ u64::from(byte)).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        }
    }
    fn write_usize(&mut self, value: usize) {
        self.0 = (value as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
}
type IdentitySet = HashSet<usize, BuildHasherDefault<PointerHasher>>;

fn identity(resource: &Resource) -> usize {
    Arc::as_ptr(resource) as *const () as usize
}

/// Resources a draw list keeps alive until the GPU retires its frame.
///
/// Composing a frame appends many cached layer lists. Each list's own
/// resources live behind an `Arc`, so appending shares them instead of
/// re-inserting every entry; cloning a list is likewise O(1). Identity
/// de-duplication across appended chunks happens lazily, when the set is
/// iterated or counted (on the render thread, once per frame).
#[derive(Clone, Default)]
pub struct DrawListResources {
    own: Arc<Vec<Resource>>,
    identities: Arc<IdentitySet>,
    appended: Vec<Arc<Vec<Resource>>>,
}
impl std::fmt::Debug for DrawListResources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DrawListResources")
            .field("count", &self.len())
            .finish()
    }
}
impl DrawListResources {
    /// Distinct retained resources.
    pub fn len(&self) -> usize {
        let mut count = 0;
        self.visit(|_| count += 1);
        count
    }
    pub fn is_empty(&self) -> bool {
        self.own.is_empty() && self.appended.iter().all(|chunk| chunk.is_empty())
    }
    pub fn retain<T: Any + Send + Sync>(&mut self, resource: Arc<T>) {
        self.retain_erased(resource);
    }
    pub fn retain_erased(&mut self, resource: Resource) {
        if Arc::make_mut(&mut self.identities).insert(identity(&resource)) {
            Arc::make_mut(&mut self.own).push(resource);
        }
    }
    /// Shares `other`'s resources; O(chunks), not O(resources).
    pub fn append(&mut self, other: &Self) {
        if !other.own.is_empty() {
            self.appended.push(other.own.clone());
        }
        self.appended.extend(
            other
                .appended
                .iter()
                .filter(|chunk| !chunk.is_empty())
                .cloned(),
        );
    }
    /// Each distinct resource once, own resources first, then appended ones in
    /// append order.
    fn visit(&self, mut visit: impl FnMut(&Resource)) {
        for resource in self.own.iter() {
            visit(resource);
        }
        if self.appended.is_empty() {
            return;
        }
        let mut seen: IdentitySet = (*self.identities).clone();
        for chunk in &self.appended {
            for resource in chunk.iter() {
                if seen.insert(identity(resource)) {
                    visit(resource);
                }
            }
        }
    }
    pub fn for_each<T: Any>(&self, mut visit: impl FnMut(&T)) {
        self.visit(|resource| {
            if let Some(value) = resource.downcast_ref::<T>() {
                visit(value);
            }
        });
    }
    pub fn for_each_retained<T: Any + Send + Sync>(&self, mut visit: impl FnMut(Arc<T>)) {
        self.visit(|resource| {
            if let Ok(value) = resource.clone().downcast::<T>() {
                visit(value);
            }
        });
    }
    pub fn retain_texture_asset(&mut self, asset: Arc<dyn TextureAsset>) {
        let identity = Arc::as_ptr(&asset) as *const () as usize;
        if Arc::make_mut(&mut self.identities).insert(identity) {
            Arc::make_mut(&mut self.own).push(Arc::new(RetainedTextureAsset(asset)));
        }
    }
    pub fn reset(&mut self) {
        // Reuse the buffers when this list is their only owner.
        match Arc::get_mut(&mut self.own) {
            Some(own) => own.clear(),
            None => self.own = Arc::default(),
        }
        match Arc::get_mut(&mut self.identities) {
            Some(identities) => identities.clear(),
            None => self.identities = Arc::default(),
        }
        self.appended.clear();
    }
}
pub type TextureId = u32;
pub fn color_glyph_atlas_id(texture_id: TextureId) -> TextureId {
    assert!(texture_id > 0 && texture_id < 0x8000_0000);
    texture_id | 0x8000_0000
}
pub trait TextureAsset: Any + Send + Sync {
    fn texture_id(&self) -> TextureId;
    fn image(&self) -> &crate::Bitmap;
}

/// Type-erased sampled CPU image, retaining the original owner through frame retirement.
pub struct RetainedTextureAsset(pub Arc<dyn TextureAsset>);
