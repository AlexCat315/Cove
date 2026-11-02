//! Process-wide font identity.
//!
//! A font ID names one face of one font file, with its variation coordinates,
//! at one point size and raster scale. IDs are issued once per process, so a
//! glyph shaped on any thread refers to the same font everywhere.
//!
//! Font files are read once and shared between threads while any face uses
//! them. FreeType faces are not thread-safe, so every thread opens its own
//! instance of a font from the shared bytes: [`instance`] returns the one a
//! font provider on this thread already registered, and [`open`] makes a
//! plain FreeType instance for shaping on a worker.
use crate::{FontAxisId, FontFace, FontFaceSelector, ManagedFont};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    sync::{Arc, LazyLock, Mutex, MutexGuard, PoisonError},
};

/// IDs below this are left to fonts registered by hand, such as an
/// environment's primary face at 0 or a [`crate::FontCollection`]'s faces.
const FIRST_ID: i64 = 256;

/// Everything that decides the glyphs a face produces.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FontKey {
    path: PathBuf,
    selector: FontFaceSelector,
    coordinates: Vec<(FontAxisId, u64)>,
    size: u32,
    scale: u32,
}

impl FontKey {
    pub fn new(
        path: PathBuf,
        selector: FontFaceSelector,
        coordinates: &BTreeMap<FontAxisId, f64>,
        size: f32,
        scale: f32,
    ) -> Self {
        Self {
            path,
            selector,
            coordinates: coordinates
                .iter()
                .map(|(axis, value)| (*axis, value.to_bits()))
                .collect(),
            size: size.to_bits(),
            scale: scale.to_bits(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn selector(&self) -> FontFaceSelector {
        self.selector
    }

    pub fn coordinates(&self) -> BTreeMap<FontAxisId, f64> {
        self.coordinates
            .iter()
            .map(|(axis, bits)| (*axis, f64::from_bits(*bits)))
            .collect()
    }

    pub fn size(&self) -> f32 {
        f32::from_bits(self.size)
    }

    pub fn scale(&self) -> f32 {
        f32::from_bits(self.scale)
    }
}

#[derive(Default)]
struct Registry {
    files: HashMap<PathBuf, std::sync::Weak<[u8]>>,
    selectors: HashMap<(PathBuf, String), Option<FontFaceSelector>>,
    ids: HashMap<FontKey, i64>,
    keys: HashMap<i64, FontKey>,
}

static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(Mutex::default);

fn registry() -> MutexGuard<'static, Registry> {
    REGISTRY.lock().unwrap_or_else(PoisonError::into_inner)
}

thread_local! {
    /// Fonts registered by font providers on this thread.
    static INSTANCES: RefCell<HashMap<i64, Weak<ManagedFont>>> = RefCell::new(HashMap::new());
    /// Plain FreeType instances opened for shaping on this thread.
    static OPENED: RefCell<HashMap<i64, Rc<ManagedFont>>> = RefCell::new(HashMap::new());
}

/// The contents of the font file at `path`, read at most once while any
/// face built from it is alive.
pub fn file(path: &Path) -> std::io::Result<Arc<[u8]>> {
    if let Some(bytes) = registry().files.get(path).and_then(std::sync::Weak::upgrade) {
        return Ok(bytes);
    }
    let bytes: Arc<[u8]> = std::fs::read(path)?.into();
    let mut registry = registry();
    // Another thread may have read the file meanwhile; keep one copy.
    if let Some(existing) = registry.files.get(path).and_then(std::sync::Weak::upgrade) {
        return Ok(existing);
    }
    registry
        .files
        .insert(path.to_owned(), Arc::downgrade(&bytes));
    Ok(bytes)
}

/// The face in the file at `path` whose PostScript name is `name`.
pub fn selector(path: &Path, name: &str, bytes: &[u8]) -> Option<FontFaceSelector> {
    let key = (path.to_owned(), name.to_owned());
    if let Some(selector) = registry().selectors.get(&key) {
        return *selector;
    }
    let selector = FontFace::probe_selector(name, bytes);
    registry().selectors.insert(key, selector);
    selector
}

/// The process-wide ID of `key`, issued on first use.
pub fn id(key: &FontKey) -> i64 {
    let mut registry = registry();
    if let Some(id) = registry.ids.get(key) {
        return *id;
    }
    let id = FIRST_ID
        .checked_add(registry.ids.len() as i64)
        .expect("font identity space exhausted");
    registry.ids.insert(key.clone(), id);
    registry.keys.insert(id, key.clone());
    id
}

/// The key that `id` was issued for.
pub fn key(id: i64) -> Option<FontKey> {
    registry().keys.get(&id).cloned()
}

/// The font a provider on this thread registered under `id`.
pub fn instance(id: i64) -> Option<Rc<ManagedFont>> {
    INSTANCES.with(|instances| instances.borrow().get(&id).and_then(Weak::upgrade))
}

/// Makes `font` this thread's instance of its ID.
pub(crate) fn remember(font: &Rc<ManagedFont>) {
    INSTANCES.with(|instances| {
        let mut instances = instances.borrow_mut();
        if instances.len() >= 1024 {
            instances.retain(|_, font| font.strong_count() > 0);
        }
        instances.insert(font.id, Rc::downgrade(font));
    });
}

/// This thread's instance of `id`: the one a provider registered, or a
/// FreeType face opened from the shared file bytes. Fonts whose glyphs come
/// from a platform color font are only available where they were registered.
pub fn open(id: i64) -> Option<Rc<ManagedFont>> {
    if let Some(font) = instance(id) {
        return Some(font);
    }
    if let Some(font) = OPENED.with(|opened| opened.borrow().get(&id).cloned()) {
        return Some(font);
    }
    let key = key(id)?;
    let face = FontFace::from_bytes(
        file(key.path()).ok()?,
        key.size(),
        key.scale(),
        key.selector(),
        key.coordinates(),
    )
    .ok()?;
    let font = Rc::new(ManagedFont {
        id,
        post_script_name: face.post_script_name().to_owned(),
        point_size: key.size(),
        raster_scale: key.scale(),
        face,
    });
    OPENED.with(|opened| {
        let mut opened = opened.borrow_mut();
        if opened.len() >= 64 {
            opened.clear();
        }
        opened.insert(id, font.clone());
    });
    Some(font)
}
