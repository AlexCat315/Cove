//! Translation of CoveUIText/TextStyle.swift.
use crate::{FontAtlas, FontProvider, FontRun, ManagedFont};
use cove_core::{BidiParagraphs, Font, FontDesign, FontWeight, ShapedGlyph};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
pub trait FontWeightTextExt {
    fn core_text_weight(self) -> f32;
}
impl FontWeightTextExt for FontWeight {
    fn core_text_weight(self) -> f32 {
        match self {
            Self::Regular => 0.0,
            Self::Medium => 0.23,
            Self::Semibold => 0.3,
            Self::Bold => 0.4,
        }
    }
}
pub struct SystemFontDefaults;
impl SystemFontDefaults {
    pub const FONT_STACK: &'static [&'static str] = &[
        "Helvetica Neue",
        "Helvetica",
        "PingFang SC",
        "Hiragino Sans GB",
        "Microsoft YaHei",
        "微软雅黑",
        "Arial",
    ];
    pub fn primary_font_name() -> String {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            use crate::native_color_glyph_font::AppleFont;
            if let Some(font) = AppleFont::new(None, 13.0, FontWeight::Regular, false) {
                let family = font.name(true);
                if !family.is_empty() {
                    return family;
                }
                let ps = font.name(false);
                if !ps.is_empty() {
                    return ps;
                }
            }
            for &name in Self::FONT_STACK {
                if AppleFont::new(Some(name), 13.0, FontWeight::Regular, false)
                    .is_some_and(|font| font.name(true).eq_ignore_ascii_case(name))
                {
                    return name.into();
                }
            }
            return "Helvetica Neue".into();
        }
        #[cfg(target_os = "windows")]
        {
            return "Segoe UI".into();
        }
        #[cfg(target_os = "android")]
        {
            return "Roboto".into();
        }
        #[cfg(not(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "windows",
            target_os = "android"
        )))]
        "DejaVu Sans".into()
    }
    pub fn monospaced_font_name() -> &'static str {
        if cfg!(any(target_os = "macos", target_os = "ios")) {
            "Menlo"
        } else if cfg!(target_os = "windows") {
            "consola"
        } else if cfg!(target_os = "android") {
            "/system/fonts/DroidSansMono.ttf"
        } else {
            "DejaVuSansMono"
        }
    }
}
struct ResolverData {
    primary_name: String,
    atlas: FontAtlas,
    scale: f32,
    block: i64,
    next_base: i64,
    providers: HashMap<Font, FontProvider>,
}
#[derive(Clone)]
pub struct TextFontResolver(Rc<RefCell<ResolverData>>);
impl TextFontResolver {
    pub fn new(
        primary_name: String,
        atlas: FontAtlas,
        provider_id_block_size: i64,
        scale: f32,
    ) -> Self {
        let block = provider_id_block_size.max(32);
        Self(Rc::new(RefCell::new(ResolverData {
            primary_name,
            atlas,
            scale: scale.max(1.0),
            block,
            next_base: block.max(256),
            providers: HashMap::new(),
        })))
    }
    fn with_provider<R>(
        &self,
        font: &Font,
        use_provider: impl FnOnce(&mut FontProvider, &FontAtlas) -> R,
    ) -> R {
        let mut data = self.0.borrow_mut();
        if !data.providers.contains_key(font) {
            let mut provider = FontProvider::new(font.size(), data.scale, data.next_base);
            data.next_base = data
                .next_base
                .checked_add(data.block)
                .expect("font provider identity space exhausted");
            let name = font.family().unwrap_or_else(|| {
                if font.design() == FontDesign::Monospaced {
                    SystemFontDefaults::monospaced_font_name()
                } else {
                    &data.primary_name
                }
            });
            provider.load_primary_font(name, font.weight(), font.is_italic());
            provider.register_all_fonts(&data.atlas);
            data.providers.insert(font.clone(), provider);
        }
        let atlas = data.atlas.clone();
        use_provider(data.providers.get_mut(font).unwrap(), &atlas)
    }
    pub fn shape(&self, text: &str, font: &Font) -> Vec<ShapedGlyph> {
        if self.0.borrow().primary_name.is_empty() {
            return vec![];
        }
        self.with_provider(font, |provider, atlas| {
            let runs = if BidiParagraphs::contains_right_to_left(text) {
                Self::directional_runs(text, provider)
            } else {
                provider.resolve_runs(text)
            };
            provider.register_all_fonts(atlas);
            runs.iter()
                .flat_map(|run| provider.shape_run(run))
                .collect()
        })
    }
    fn directional_runs(text: &str, provider: &mut FontProvider) -> Vec<FontRun> {
        let bidi = BidiParagraphs::new(text, false);
        let mut result = Vec::new();
        for paragraph in &bidi.paragraphs {
            for (range, level) in bidi.level_runs(paragraph.range.clone()) {
                let Some(substring) = text.get(range.clone()) else {
                    continue;
                };
                for mut run in provider.resolve_runs(substring) {
                    if let Some(offset) = run.utf8_offset.checked_add(range.start as u32) {
                        run.utf8_offset = offset;
                        run.is_right_to_left = Some(level & 1 == 1);
                        result.push(run);
                    }
                }
            }
        }
        result
    }
    pub fn prepare_primary_font(&self, font: &Font) -> Option<Rc<ManagedFont>> {
        if self.0.borrow().primary_name.is_empty() {
            return None;
        }
        self.with_provider(font, |provider, _| provider.primary_font())
    }
}
