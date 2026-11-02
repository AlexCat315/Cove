//! Translation of CoveUIText/TextEnvironment.swift.
use crate::{
    FontAtlas, FontRun, PortableRun, SystemFontDefaults, TextCacheLimits, TextCacheSnapshot,
    TextFontResolver, TextShaper, text_cache::TextCache,
};
use cove_core::{
    Color, Font, FontWeight, ShapedGlyph, TextAlignment, TextLayoutCacheKey, TextLayoutOptions,
    TextLayoutResult, TextureId, layout_text,
};
use std::{
    cell::RefCell,
    collections::HashSet,
    mem::size_of,
    rc::Rc,
    sync::Arc,
};
/// Below this much text a batch is shaped on the calling thread; handing it
/// to workers would cost more than it saves.
const PARALLEL_SHAPING_BYTES: usize = 512;
#[derive(Clone, PartialEq, Eq, Hash)]
struct ShapeKey {
    text: String,
    font: Font,
}
#[derive(Clone)]
pub struct TextEnvironment {
    pub atlas: FontAtlas,
    pub shaper: TextShaper,
    pub atlas_texture_id: TextureId,
    pub default_line_height: f32,
    pub default_color: Color,
    pub default_font: Font,
    pub font_resolver: Option<TextFontResolver>,
    shape_cache: Arc<TextCache<ShapeKey, Arc<[ShapedGlyph]>>>,
    layout_cache: Arc<TextCache<TextLayoutCacheKey, Arc<TextLayoutResult>>>,
    pending_shapes: Rc<RefCell<Vec<ShapeRequest>>>,
}
/// A text to shape with the next batch, read when the batch runs so it sees
/// the font that layout will use. `None` when it no longer needs shaping.
type ShapeRequest = Box<dyn FnOnce(&TextEnvironment) -> Option<(String, Font)>>;
impl TextEnvironment {
    #[allow(
        clippy::too_many_arguments,
        reason = "translated environment constructor owns distinct font, atlas and cache configuration"
    )]
    pub fn new(
        atlas: FontAtlas,
        shaper: TextShaper,
        atlas_texture_id: TextureId,
        default_line_height: f32,
        default_color: Color,
        default_font: Option<Font>,
        font_resolver: Option<TextFontResolver>,
        limits: TextCacheLimits,
    ) -> Self {
        let fallback_size = if atlas.font_size() > 0.0 {
            atlas.font_size()
        } else {
            default_line_height.max(1.0)
        };
        Self {
            atlas,
            shaper,
            atlas_texture_id,
            default_line_height,
            default_color,
            default_font: default_font
                .unwrap_or_else(|| Font::system(fallback_size, FontWeight::Regular)),
            font_resolver,
            shape_cache: Arc::new(TextCache::new(limits)),
            layout_cache: Arc::new(TextCache::new(limits)),
            pending_shapes: Rc::default(),
        }
    }
    pub fn shaping_cache_snapshot(&self) -> TextCacheSnapshot {
        self.shape_cache.snapshot()
    }
    pub fn layout_cache_snapshot(&self) -> TextCacheSnapshot {
        self.layout_cache.snapshot()
    }
    pub fn trim_text_caches(&self) {
        self.pending_shapes.borrow_mut().clear();
        self.shape_cache.clear();
        self.layout_cache.clear();
    }
    pub fn bootstrapped(options: TextBootstrapOptions) -> Self {
        let scale = options.raster_scale.max(1.0);
        let edge = options
            .atlas_edge
            .unwrap_or_else(|| (1024.0 * scale).ceil().max(1024.0) as usize);
        let atlas = FontAtlas::new(edge, edge);
        let mut shaper = TextShaper::default();
        let resolver = TextFontResolver::new(options.primary_font_name, atlas.clone(), scale);
        if let Some(primary) = resolver.prepare_primary_font(&options.default_font) {
            shaper.set_face(primary.face.clone(), options.default_font.size());
            atlas.register_face(primary.face.clone(), 0);
        }
        Self::new(
            atlas,
            shaper,
            options.atlas_texture_id,
            options.default_line_height,
            options.default_color,
            Some(options.default_font),
            Some(resolver),
            options.cache_limits,
        )
    }
    pub fn resolved_font(&self, font: Option<&Font>) -> Font {
        font.unwrap_or(&self.default_font).clone()
    }
    pub fn resolved_line_height(&self, font: &Font, override_height: Option<f32>) -> f32 {
        override_height.unwrap_or_else(|| {
            if self.default_font.size() > 0.0 {
                self.default_line_height * font.size() / self.default_font.size()
            } else {
                self.default_line_height
            }
        })
    }
    pub fn shape(&self, text: &str, font: Option<&Font>) -> Arc<[ShapedGlyph]> {
        let font = self.resolved_font(font);
        let key = ShapeKey {
            text: text.into(),
            font: font.clone(),
        };
        if !self.pending_shapes.borrow().is_empty() && !self.shape_cache.contains(&key) {
            let requests = self.pending_shapes.take();
            let keys: HashSet<ShapeKey> = requests
                .into_iter()
                .filter_map(|request| request(self))
                .map(|(text, font)| ShapeKey { text, font })
                .chain([key.clone()])
                .collect();
            if let Some(glyphs) = self.shape_batch(keys, &key) {
                return glyphs;
            }
        }
        self.shape_cache.value(
            key,
            |glyphs| shape_cost(text.len(), glyphs),
            || self.shape_now(text, &font),
        )
    }
    /// Queues a text for the next shaping batch: the next [`shape`](Self::shape)
    /// that misses the cache first calls every queued `request` and shapes
    /// the texts they return together, running HarfBuzz on worker threads.
    pub fn request_shape(
        &self,
        request: impl FnOnce(&TextEnvironment) -> Option<(String, Font)> + 'static,
    ) {
        if self.font_resolver.is_none() {
            return;
        }
        // A larger batch would evict its own results before layout reads them.
        let mut pending = self.pending_shapes.borrow_mut();
        if pending.len() < self.shape_cache.limits().maximum_entries {
            pending.push(Box::new(request));
        }
    }
    fn shape_now(&self, text: &str, font: &Font) -> Arc<[ShapedGlyph]> {
        if let Some(resolver) = &self.font_resolver {
            let glyphs = resolver.shape(text, font);
            if !glyphs.is_empty() || text.is_empty() {
                return glyphs.into();
            }
        }
        self.shaper.shape_latin(text).into()
    }
    /// Shapes the uncached `keys`, caches them and returns the glyphs of
    /// `wanted`. Font fallback runs here, so every font is loaded and registered
    /// with this thread's atlas before a worker shapes with its ID.
    fn shape_batch(
        &self,
        keys: HashSet<ShapeKey>,
        wanted: &ShapeKey,
    ) -> Option<Arc<[ShapedGlyph]>> {
        let resolver = self.font_resolver.as_ref()?;
        let keys: Vec<ShapeKey> = keys
            .into_iter()
            .filter(|key| !key.text.is_empty() && !self.shape_cache.contains(key))
            .collect();
        let runs: Vec<Vec<FontRun>> = keys
            .iter()
            .map(|key| resolver.resolve_runs(&key.text, &key.font))
            .collect();
        let mut jobs = Vec::new();
        let slots: Vec<Option<usize>> = runs
            .iter()
            .flatten()
            .map(|run| {
                run.portable().map(|job| {
                    jobs.push(job);
                    jobs.len() - 1
                })
            })
            .collect();
        let bytes: usize = jobs.iter().map(|job| job.text.len()).sum();
        let mut shaped = if bytes >= PARALLEL_SHAPING_BYTES {
            cove_core::work::par_map(&jobs, PortableRun::shape)
        } else {
            jobs.iter().map(PortableRun::shape).collect()
        };
        let mut slots = slots.into_iter();
        let mut result = None;
        for (key, runs) in keys.into_iter().zip(&runs) {
            let mut glyphs = Vec::new();
            for run in runs {
                let worker = slots.next().flatten().and_then(|slot| shaped[slot].take());
                glyphs.extend(worker.unwrap_or_else(|| run.shape()));
            }
            if glyphs.is_empty() {
                glyphs = self.shaper.shape_latin(&key.text);
            }
            let glyphs: Arc<[ShapedGlyph]> = glyphs.into();
            let bytes = shape_cost(key.text.len(), &glyphs);
            if key == *wanted {
                result = Some(glyphs.clone());
            }
            self.shape_cache.value(key, |_| bytes, || glyphs);
        }
        result
    }
    pub fn cached_layout(
        &self,
        text: &str,
        font: Option<&Font>,
        line_height: Option<f32>,
        max_width: f32,
        alignment: TextAlignment,
        letter_spacing: f32,
    ) -> Arc<TextLayoutResult> {
        let font = self.resolved_font(font);
        let line_height = self.resolved_line_height(&font, line_height);
        let max_width = if max_width.is_finite() && max_width > 0.0 {
            max_width
        } else {
            f32::INFINITY
        };
        let letter_spacing = if letter_spacing.is_finite() {
            letter_spacing
        } else {
            0.0
        };
        let key = TextLayoutCacheKey {
            text: text.into(),
            font: font.clone(),
            line_height,
            alignment,
            max_width,
            letter_spacing,
            atlas_id: self.atlas.identity(),
        };
        self.layout_cache.value(
            key,
            |layout| {
                text.len()
                    .saturating_add(size_of::<TextLayoutCacheKey>())
                    .saturating_add(
                        layout
                            .lines
                            .len()
                            .saturating_mul(size_of::<cove_core::TextLine>()),
                    )
                    .saturating_add(
                        layout
                            .lines
                            .iter()
                            .map(|line| {
                                line.glyphs
                                    .len()
                                    .saturating_mul(size_of::<cove_core::PositionedGlyph>())
                            })
                            .fold(0usize, usize::saturating_add),
                    )
            },
            || {
                let glyphs = self.shape(text, Some(&font));
                Arc::new(layout_text(
                    &glyphs,
                    text,
                    &self.atlas,
                    TextLayoutOptions {
                        max_width,
                        line_height,
                        alignment,
                        letter_spacing,
                    },
                ))
            },
        )
    }
    pub fn prewarm_glyphs(&self, text: &str, fonts: &[Font]) {
        let mut seen = HashSet::new();
        for font in fonts {
            if seen.insert(font.clone()) {
                self.prewarm_font_glyphs(text, Some(font));
            }
        }
    }
    pub fn prewarm_font_glyphs(&self, text: &str, font: Option<&Font>) {
        for glyph in self.shape(text, font).iter() {
            self.atlas.rasterize_glyph(glyph.glyph_id, glyph.font_id);
        }
    }
}
fn shape_cost(text_bytes: usize, glyphs: &Arc<[ShapedGlyph]>) -> usize {
    text_bytes
        .saturating_add(size_of::<ShapeKey>())
        .saturating_add(glyphs.len().saturating_mul(size_of::<ShapedGlyph>()))
}
pub struct TextBootstrapOptions {
    pub atlas_texture_id: TextureId,
    pub primary_font_name: String,
    pub default_font: Font,
    pub default_line_height: f32,
    pub default_color: Color,
    pub raster_scale: f32,
    pub atlas_edge: Option<usize>,
    pub cache_limits: TextCacheLimits,
}
impl Default for TextBootstrapOptions {
    fn default() -> Self {
        Self {
            atlas_texture_id: 1,
            primary_font_name: SystemFontDefaults::primary_font_name(),
            default_font: Font::system(18.0, FontWeight::Regular),
            default_line_height: 22.0,
            default_color: Color::WHITE,
            raster_scale: 1.0,
            atlas_edge: None,
            cache_limits: TextCacheLimits::default(),
        }
    }
}
thread_local! { static CURRENT_TEXT: RefCell<Option<TextEnvironment>> = const { RefCell::new(None) }; }
pub struct TextEnvironmentHolder;
impl TextEnvironmentHolder {
    pub fn current() -> Option<TextEnvironment> {
        CURRENT_TEXT.with(|current| current.borrow().clone())
    }
    pub fn set_current(environment: Option<TextEnvironment>) {
        CURRENT_TEXT.with(|current| {
            current.replace(environment);
        });
    }
    pub fn with_current<R>(environment: Option<TextEnvironment>, body: impl FnOnce() -> R) -> R {
        let previous = CURRENT_TEXT.with(|current| current.replace(environment));
        struct Restore(Option<Option<TextEnvironment>>);
        impl Drop for Restore {
            fn drop(&mut self) {
                CURRENT_TEXT.with(|current| {
                    current.replace(self.0.take().unwrap());
                });
            }
        }
        let _restore = Restore(Some(previous));
        body()
    }
}
