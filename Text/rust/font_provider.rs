//! Translation of CoveUIText/FontProvider.swift, including all platform branches.
#[cfg(any(target_os = "macos", target_os = "ios"))]
use crate::native_color_glyph_font::AppleFont;
use crate::{ColorGlyphSource, FontAtlas, FontFace, FontFaceSelector, TextDirection};
use cove_core::{FontWeight, ShapedGlyph};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    rc::Rc,
};
use unicode_segmentation::UnicodeSegmentation;

pub struct ManagedFont {
    pub id: i64,
    pub post_script_name: String,
    pub point_size: f32,
    pub raster_scale: f32,
    pub face: FontFace,
}
impl ManagedFont {
    pub fn color_source(&self) -> Option<ColorGlyphSource> {
        self.face.color_source()
    }
}
#[derive(Clone)]
pub struct FontRun {
    pub font: Rc<ManagedFont>,
    pub text: String,
    pub utf8_offset: u32,
    pub is_right_to_left: Option<bool>,
}
pub struct FontProvider {
    fonts: HashMap<String, Rc<ManagedFont>>,
    next_id: i64,
    size: f32,
    scale: f32,
    primary_name: Option<String>,
    fallback_chain: Vec<Rc<ManagedFont>>,
    tried_paths: HashSet<PathBuf>,
    fallback_paths: Vec<PathBuf>,
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    primary_native: Option<AppleFont>,
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    styled_fallbacks: Option<Vec<AppleFont>>,
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fallback_names: HashMap<String, String>,
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    failed_names: HashSet<String>,
    weight: FontWeight,
    italic: bool,
}
impl FontProvider {
    pub fn new(size: f32, raster_scale: f32, id_base: i64) -> Self {
        Self {
            fonts: HashMap::new(),
            next_id: id_base,
            size,
            scale: raster_scale.max(1.0),
            primary_name: None,
            fallback_chain: vec![],
            tried_paths: HashSet::new(),
            fallback_paths: system_fallback_paths(),
            weight: FontWeight::Regular,
            italic: false,
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            primary_native: None,
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            styled_fallbacks: None,
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            fallback_names: HashMap::new(),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            failed_names: HashSet::new(),
        }
    }
    pub fn primary_font(&self) -> Option<Rc<ManagedFont>> {
        self.fonts.get(self.primary_name.as_ref()?).cloned()
    }
    pub fn all_fonts(&self) -> Vec<Rc<ManagedFont>> {
        let mut seen = HashSet::new();
        self.fonts
            .values()
            .filter(|font| seen.insert(font.id))
            .cloned()
            .collect()
    }
    pub fn register_all_fonts(&self, atlas: &FontAtlas) {
        for font in self.all_fonts() {
            if let Some(source) = font.color_source() {
                atlas.register_color_source(source, font.id);
            } else {
                atlas.register_face(font.face.clone(), font.id);
            }
        }
    }
    fn register(
        &mut self,
        face: FontFace,
        requested: String,
        aliases: &[String],
    ) -> Rc<ManagedFont> {
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("font identity space exhausted");
        let resolved = if face.post_script_name().is_empty() {
            requested.clone()
        } else {
            face.post_script_name().to_owned()
        };
        let font = Rc::new(ManagedFont {
            id,
            post_script_name: resolved.clone(),
            point_size: self.size,
            raster_scale: self.scale,
            face,
        });
        self.fonts.insert(requested, font.clone());
        self.fonts.insert(resolved.clone(), font.clone());
        for alias in aliases {
            self.fonts.insert(alias.clone(), font.clone());
        }
        if self.primary_name.is_none() {
            self.primary_name = Some(resolved);
        }
        font
    }
    pub fn load_primary_font(
        &mut self,
        name: &str,
        weight: FontWeight,
        italic: bool,
    ) -> Option<Rc<ManagedFont>> {
        self.weight = weight;
        self.italic = italic;
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            self.failed_names.clear();
            let native = AppleFont::new(Some(name), self.size, weight, italic)?;
            let font = self.load_apple_font(native.clone())?;
            self.primary_name = Some(font.post_script_name.clone());
            self.primary_native = Some(native);
            self.styled_fallbacks = None;
            self.fallback_names.clear();
            Some(font)
        }
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        {
            self.load_primary_direct(name)
        }
    }
    pub fn resolve_runs(&mut self, text: &str) -> Vec<FontRun> {
        if text.is_empty() {
            return vec![];
        }
        let Some(primary) = self.primary_font() else {
            return vec![];
        };
        let mut result: Vec<FontRun> = Vec::new();
        let mut utf16_location = 0;
        for (offset, cluster) in text.grapheme_indices(true) {
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            let font = self.covering_apple_font(
                cluster,
                text,
                utf16_location,
                cluster.encode_utf16().count(),
                &primary,
            );
            #[cfg(not(any(target_os = "macos", target_os = "ios")))]
            let font = self.fallback_font(cluster, &primary);
            if let Some(last) = result.last_mut().filter(|last| last.font.id == font.id) {
                last.text.push_str(cluster);
            } else if let Ok(offset) = u32::try_from(offset) {
                result.push(FontRun {
                    font,
                    text: cluster.into(),
                    utf8_offset: offset,
                    is_right_to_left: None,
                });
            }
            utf16_location += cluster.encode_utf16().count();
        }
        let _ = utf16_location;
        result
    }
    pub fn shape_run(&self, run: &FontRun) -> Vec<ShapedGlyph> {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        if let Some(color) = run.font.face.native_color_font() {
            return color.shape(&run.text, run.font.id, run.utf8_offset);
        }
        run.font.face.shape(
            &run.text,
            run.is_right_to_left.map(|rtl| {
                if rtl {
                    TextDirection::Rtl
                } else {
                    TextDirection::Ltr
                }
            }),
            None,
            None,
            run.utf8_offset,
            run.font.id,
        )
    }
    fn load_direct(&mut self, path: &Path, alias: &str) -> Option<Rc<ManagedFont>> {
        if let Some(font) = self.fonts.get(alias) {
            return Some(font.clone());
        }
        let face = FontFace::from_path(
            path,
            self.size,
            self.scale,
            FontFaceSelector::default(),
            Default::default(),
        )
        .ok()?;
        Some(self.register(face, alias.into(), &[]))
    }
    fn load_primary_direct(&mut self, name: &str) -> Option<Rc<ManagedFont>> {
        if Path::new(name).is_absolute()
            && let Some(font) = self.load_direct(Path::new(name), name)
        {
            return Some(font);
        }
        for directory in [
            PathBuf::from("/system/fonts"),
            windows_font_directory(),
            PathBuf::from("/usr/share/fonts"),
            PathBuf::from("/usr/share/fonts/truetype/dejavu"),
            PathBuf::from("/usr/share/fonts/TTF"),
            PathBuf::from("/usr/local/share/fonts"),
        ] {
            for extension in ["ttc", "ttf", "otf"] {
                if let Some(font) =
                    self.load_direct(&directory.join(format!("{name}.{extension}")), name)
                {
                    return Some(font);
                }
            }
        }
        for path in system_primary_paths() {
            if let Some(font) =
                self.load_direct(&path, if name.is_empty() { "system-ui" } else { name })
            {
                return Some(font);
            }
        }
        None
    }
    fn fallback_font(&mut self, cluster: &str, primary: &Rc<ManagedFont>) -> Rc<ManagedFont> {
        if primary.face.can_render(cluster) {
            return primary.clone();
        }
        if let Some(font) = self
            .fallback_chain
            .iter()
            .find(|font| font.face.can_render(cluster))
        {
            return font.clone();
        }
        for path in self.fallback_paths.clone() {
            if !self.tried_paths.insert(path.clone()) {
                continue;
            }
            let alias = format!("fallback:{}", path.display());
            if let Some(font) = self.load_direct(&path, &alias) {
                self.fallback_chain.push(font.clone());
                if font.face.can_render(cluster) {
                    return font;
                }
            }
        }
        primary.clone()
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn load_apple_font(&mut self, native: AppleFont) -> Option<Rc<ManagedFont>> {
        let name = native.name(false);
        if let Some(font) = self.fonts.get(&name) {
            return Some(font.clone());
        }
        if self.failed_names.contains(&name) {
            return None;
        }
        if let Some(font) = self.make_apple_font(&native, &[]) {
            return Some(font);
        }
        let normalized = name
            .trim_start_matches('.')
            .replace("PingFangUIText", "PingFang")
            .replace("PingFangUI", "PingFang");
        let family = native
            .name(true)
            .trim_start_matches('.')
            .replace("PingFang UI", "PingFang ")
            .replace(" UI ", " ")
            .replace(" UI", " ")
            .replace(' ', "");
        let style = native.subfamily().replace(' ', "");
        let family_candidate = if style.is_empty() {
            family
        } else {
            format!("{family}-{style}")
        };
        let mut seen = HashSet::new();
        for candidate in [normalized, family_candidate] {
            if candidate.is_empty() || !seen.insert(candidate.clone()) {
                continue;
            }
            if let Some(font) = self.fonts.get(&candidate).cloned() {
                self.fonts.insert(name.clone(), font.clone());
                return Some(font);
            }
            if let Some(candidate_native) =
                AppleFont::new(Some(&candidate), self.size, FontWeight::Regular, false)
                && let Some(font) =
                    self.make_apple_font(&candidate_native, &[name.clone(), candidate])
            {
                return Some(font);
            }
        }
        self.failed_names.insert(name);
        None
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn make_apple_font(
        &mut self,
        native: &AppleFont,
        aliases: &[String],
    ) -> Option<Rc<ManagedFont>> {
        let name = native.name(false);
        if let Some(font) = self.fonts.get(&name).cloned() {
            for alias in aliases {
                self.fonts.insert(alias.clone(), font.clone());
            }
            return Some(font);
        }
        let bytes = std::fs::read(native.path()?).ok()?;
        let selector = FontFace::probe_selector(&name, &bytes).unwrap_or_default();
        let face =
            FontFace::from_bytes(bytes, self.size, self.scale, selector, native.variations())
                .ok()?;
        if face.has_color_glyphs() {
            if let Some(color) =
                crate::NativeColorGlyphFont::new(native.clone(), self.size, self.scale)
            {
                face.set_native_color_font(color);
            }
        }
        Some(self.register(face, name, aliases))
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn covering_apple_font(
        &mut self,
        cluster: &str,
        text: &str,
        location: usize,
        length: usize,
        primary: &Rc<ManagedFont>,
    ) -> Rc<ManagedFont> {
        if primary.face.can_render(cluster) {
            return primary.clone();
        }
        if let Some(name) = self.fallback_names.get(cluster)
            && let Some(font) = self.fonts.get(name)
        {
            return font.clone();
        }
        if self.styled_fallbacks.is_none() {
            self.styled_fallbacks = Some(
                crate::SystemFontDefaults::FONT_STACK
                    .iter()
                    .filter_map(|name| {
                        AppleFont::new(Some(name), self.size, self.weight, self.italic)
                    })
                    .collect(),
            );
        }
        let mut seen = HashSet::new();
        for native in self.styled_fallbacks.clone().unwrap_or_default() {
            if !seen.insert(native.name(false)) {
                continue;
            }
            if let Some(font) = self.load_apple_font(native)
                && font.face.can_render(cluster)
            {
                if cluster.len() <= 256 {
                    if self.fallback_names.len() >= 1024 {
                        self.fallback_names.clear();
                    }
                    self.fallback_names
                        .insert(cluster.into(), font.post_script_name.clone());
                }
                return font;
            }
        }
        if let Some(native) = self
            .primary_native
            .clone()
            .and_then(|native| native.fallback(text, location, length))
            && let Some(font) = self.load_apple_font(native)
            && font.face.can_render(cluster)
        {
            return font;
        }
        primary.clone()
    }
}
fn windows_font_directory() -> PathBuf {
    std::env::var_os("WINDIR")
        .or_else(|| std::env::var_os("SystemRoot"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\Windows"))
        .join("Fonts")
}
fn system_primary_paths() -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        return ["segoeui.ttf", "tahoma.ttf", "arial.ttf"]
            .iter()
            .map(|name| windows_font_directory().join(name))
            .collect();
    }
    #[cfg(target_os = "android")]
    {
        return [
            "/system/fonts/Roboto-Regular.ttf",
            "/system/fonts/NotoSans-Regular.ttf",
        ]
        .map(PathBuf::from)
        .to_vec();
    }
    #[cfg(target_os = "linux")]
    {
        return [
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
        ]
        .map(PathBuf::from)
        .to_vec();
    }
    #[cfg(not(any(target_os = "windows", target_os = "android", target_os = "linux")))]
    Vec::new()
}
fn system_fallback_paths() -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        return [
            "msyh.ttc",
            "msyhl.ttc",
            "simsun.ttc",
            "msjh.ttc",
            "msgothic.ttc",
            "yugothm.ttc",
            "malgun.ttf",
            "seguiemj.ttf",
            "arial.ttf",
        ]
        .iter()
        .map(|name| windows_font_directory().join(name))
        .collect();
    }
    #[cfg(target_os = "android")]
    {
        let mut paths = [
            "/system/fonts/NotoSansCJK-Regular.ttc",
            "/system/fonts/NotoSansSymbols-Regular-Subsetted.ttf",
            "/system/fonts/NotoSansSymbols-Regular-Subsetted2.ttf",
            "/system/fonts/NotoSansArabic-Regular.ttf",
            "/system/fonts/NotoSansDevanagari-Regular.ttf",
        ]
        .map(PathBuf::from)
        .to_vec();
        if let Some(resources) = std::env::var_os("PACKAGE_RESOURCE_BUNDLE_PATH") {
            paths.push(PathBuf::from(resources).join("Fonts/NotoEmoji.ttf"));
        }
        return paths;
    }
    #[cfg(target_os = "linux")]
    {
        return [
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSerifCJK-Regular.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
            "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
            "/usr/share/fonts/truetype/twemoji/TwemojiMozilla.ttf",
            "/usr/share/fonts/twemoji/TwemojiMozilla.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ]
        .map(PathBuf::from)
        .to_vec();
    }
    #[cfg(not(any(target_os = "windows", target_os = "android", target_os = "linux")))]
    Vec::new()
}
