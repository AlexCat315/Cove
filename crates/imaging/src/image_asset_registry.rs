use crate::{DecodedImage, ImageDecodeError, ImageDecoder, PixelSize};
use cove_core::{Size, TextureAsset, TextureId, image_lod};
use std::{
    cell::RefCell,
    collections::HashMap,
    path::{Component, Path, PathBuf},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicU32, Ordering},
    },
};
static NEXT_ID: AtomicU32 = AtomicU32::new(0x1000_0000);
/// A process-unique id for a texture whose pixels the caller supplies, such as a
/// streamed video surface that republishes new owners under one id.
pub fn allocate_texture_id() -> Result<TextureId, ImageDecodeError> {
    allocate_id()
}
pub(crate) fn allocate_id() -> Result<TextureId, ImageDecodeError> {
    NEXT_ID
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
            (next < 0x4000_0000).then_some(next + 1)
        })
        .map_err(|_| ImageDecodeError::DecodeFailure("image texture IDs exhausted".into()))
}
pub struct ImageAsset {
    pub texture_id: TextureId,
    pub image: DecodedImage,
    pub mipmaps: Vec<Arc<Self>>,
}
impl ImageAsset {
    pub fn new(image: DecodedImage) -> Result<Arc<Self>, ImageDecodeError> {
        if image.width() as u64 * image.height() as u64 > 64 * 1024 * 1024 {
            return Err(ImageDecodeError::DecodeFailure(
                "oversized RGBA image asset payload".into(),
            ));
        }
        let mipmaps = if let Some(reduced) = image.reduced_mip() {
            let next = Self::new(reduced)?;
            let mut chain = vec![next.clone()];
            chain.extend(next.mipmaps.iter().cloned());
            chain
        } else {
            Vec::new()
        };
        Ok(Arc::new(Self {
            texture_id: allocate_id()?,
            image,
            mipmaps,
        }))
    }
    pub fn retained_pixel_bytes(&self) -> usize {
        self.image.pixels().len()
            + self
                .mipmaps
                .iter()
                .map(|m| m.image.pixels().len())
                .sum::<usize>()
    }
    pub fn resolution(self: &Arc<Self>, target: Size, fraction: Size) -> Arc<Self> {
        let source = Size {
            width: self.image.width() as f32 * fraction.width,
            height: self.image.height() as f32 * fraction.height,
        };
        let level = image_lod(source, target, self.mipmaps.len());
        if level == 0 {
            self.clone()
        } else {
            self.mipmaps[level - 1].clone()
        }
    }
}
impl TextureAsset for ImageAsset {
    fn texture_id(&self) -> TextureId {
        self.texture_id
    }
    fn image(&self) -> &DecodedImage {
        &self.image
    }
}
impl PartialEq for ImageAsset {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}
impl Eq for ImageAsset {}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageCacheLimits {
    pub maximum_bytes: usize,
    pub maximum_count: usize,
}
impl Default for ImageCacheLimits {
    fn default() -> Self {
        Self {
            maximum_bytes: 256 * 1024 * 1024,
            maximum_count: 2048,
        }
    }
}
struct Entry {
    asset: Arc<ImageAsset>,
    last_use: u64,
}
struct Cache {
    entries: HashMap<String, Entry>,
    generation: u64,
    clock: u64,
    bytes: usize,
    limits: ImageCacheLimits,
}
pub struct ImageAssetRegistry(Mutex<Cache>);
impl Default for ImageAssetRegistry {
    fn default() -> Self {
        Self::new(ImageCacheLimits::default())
    }
}
impl ImageAssetRegistry {
    pub fn new(limits: ImageCacheLimits) -> Self {
        Self(Mutex::new(Cache {
            entries: HashMap::new(),
            generation: 0,
            clock: 0,
            bytes: 0,
            limits,
        }))
    }
    fn locked(&self) -> MutexGuard<'_, Cache> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub fn revision(&self) -> u64 {
        self.locked().generation
    }
    pub fn limits(&self) -> ImageCacheLimits {
        self.locked().limits
    }
    pub fn set_limits(&self, limits: ImageCacheLimits) {
        let mut cache = self.locked();
        cache.limits = limits;
        evict(&mut cache, None);
    }
    pub fn cached_bytes(&self) -> usize {
        self.locked().bytes
    }
    pub fn cached_count(&self) -> usize {
        self.locked().entries.len()
    }
    pub fn cached(&self, key: &str) -> Option<Arc<ImageAsset>> {
        let mut cache = self.locked();
        cache.clock = cache.clock.wrapping_add(1);
        let tick = cache.clock;
        let entry = cache.entries.get_mut(key)?;
        entry.last_use = tick;
        Some(entry.asset.clone())
    }
    pub fn texture(
        &self,
        path: impl AsRef<Path>,
        size: Option<PixelSize>,
    ) -> Result<Arc<ImageAsset>, ImageDecodeError> {
        let key = Self::key(path.as_ref(), size);
        if let Some(hit) = self.cached(&key) {
            return Ok(hit);
        }
        self.register(key, ImageDecoder::decode(path, size)?)
    }
    pub fn register(
        &self,
        key: impl Into<String>,
        image: DecodedImage,
    ) -> Result<Arc<ImageAsset>, ImageDecodeError> {
        let key = key.into();
        let mut cache = self.locked();
        cache.clock = cache.clock.wrapping_add(1);
        let tick = cache.clock;
        if let Some(entry) = cache.entries.get_mut(&key) {
            entry.last_use = tick;
            return Ok(entry.asset.clone());
        }
        let asset = ImageAsset::new(image)?;
        cache.bytes += asset.retained_pixel_bytes();
        cache.entries.insert(
            key.clone(),
            Entry {
                asset: asset.clone(),
                last_use: tick,
            },
        );
        evict(&mut cache, Some(&key));
        Ok(asset)
    }
    pub fn clear(&self) {
        let mut cache = self.locked();
        cache.entries.clear();
        cache.bytes = 0;
        cache.generation = cache.generation.wrapping_add(1);
    }
    pub fn key(path: &Path, size: Option<PixelSize>) -> String {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_default().join(path)
        };
        let mut normalized = PathBuf::new();
        for part in absolute.components() {
            match part {
                Component::CurDir => (),
                Component::ParentDir => {
                    normalized.pop();
                }
                other => normalized.push(other.as_os_str()),
            }
        }
        let path = normalized.to_string_lossy();
        size.map_or_else(
            || path.to_string(),
            |s| format!("{path}#{}x{}", s.width, s.height),
        )
    }
}
fn evict(cache: &mut Cache, keep: Option<&str>) {
    while cache.bytes > cache.limits.maximum_bytes
        || cache.entries.len() > cache.limits.maximum_count
    {
        let candidate = cache
            .entries
            .iter()
            .filter(|(key, _)| Some(key.as_str()) != keep)
            .min_by_key(|(_, entry)| entry.last_use)
            .map(|(key, _)| key.clone());
        let Some(candidate) = candidate else {
            break;
        };
        if let Some(entry) = cache.entries.remove(&candidate) {
            cache.bytes -= entry.asset.retained_pixel_bytes();
        }
    }
}
thread_local! { static CURRENT: RefCell<Option<Arc<ImageAssetRegistry>>> = const { RefCell::new(None) }; }
pub struct ImageAssetRegistryHolder;
impl ImageAssetRegistryHolder {
    pub fn current() -> Option<Arc<ImageAssetRegistry>> {
        CURRENT.with(|slot| slot.borrow().clone())
    }
    pub fn set_current(registry: Option<Arc<ImageAssetRegistry>>) {
        CURRENT.with(|slot| *slot.borrow_mut() = registry);
    }
    pub fn with_current<R>(registry: Arc<ImageAssetRegistry>, body: impl FnOnce() -> R) -> R {
        struct Restore(Option<Arc<ImageAssetRegistry>>);
        impl Drop for Restore {
            fn drop(&mut self) {
                ImageAssetRegistryHolder::set_current(self.0.take());
            }
        }
        let guard = Restore(Self::current());
        Self::set_current(Some(registry));
        let value = body();
        drop(guard);
        value
    }
}
