//! Translation of CoveUIText/TextEnvironment.swift.
use crate::{
    FontAtlas, SystemFontDefaults, TextCacheLimits, TextCacheSnapshot, TextFontResolver,
    TextShaper, text_cache::TextCache,
};
use cove_core::{
    Color, Font, FontWeight, ShapedGlyph, TextAlignment, TextLayoutCacheKey, TextLayoutOptions,
    TextLayoutResult, TextureId, layout_text,
};
use std::{cell::RefCell, collections::HashSet, mem::size_of, sync::Arc};
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
}
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
        }
    }
    pub fn shaping_cache_snapshot(&self) -> TextCacheSnapshot {
        self.shape_cache.snapshot()
    }
    pub fn layout_cache_snapshot(&self) -> TextCacheSnapshot {
        self.layout_cache.snapshot()
    }
    pub fn trim_text_caches(&self) {
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
        let resolver = TextFontResolver::new(options.primary_font_name, atlas.clone(), 256, scale);
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
        self.shape_cache.value(
            key,
            |glyphs| {
                text.len()
                    .saturating_add(size_of::<ShapeKey>())
                    .saturating_add(glyphs.len().saturating_mul(size_of::<ShapedGlyph>()))
            },
            || {
                if let Some(resolver) = &self.font_resolver {
                    let glyphs = resolver.shape(text, &font);
                    if !glyphs.is_empty() || text.is_empty() {
                        return glyphs.into();
                    }
                }
                self.shaper.shape_latin(text).into()
            },
        )
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
