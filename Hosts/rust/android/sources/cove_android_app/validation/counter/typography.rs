//! Translation of Android/CoveAndroidApp/Validation/Counter/Typography.swift.
//!
//! Android system faces use the same FreeType, HarfBuzz and atlas as the other
//! hosts, so this module owns the platform face discovery, the per-frame glyph
//! statistics and the atlas occupancy accounting. Painting a text run is
//! delegated through [`TypographyDriver`]'s painter so the counter product keeps
//! ownership of its text styling.

use crate::android::sources::cove_android_app::content::{
    TypographyDriver, TypographyStatistics,
};
use cove_core::{Color, DrawList, Point};
use cove_text::{FontAtlas, FontCollection};

/// System faces Android always ships, in preference order. Roboto covers Latin
/// and Noto Sans CJK covers the CJK fallback chain the gallery content uses.
const SYSTEM_FACES: [&str; 2] =
    ["/system/fonts/Roboto-Regular.ttf", "/system/fonts/NotoSansCJK-Regular.ttc"];

/// One authored counter text run.
#[derive(Clone, Debug, PartialEq)]
pub struct CounterTextRun {
    pub text: String,
    pub size: f32,
    pub inset: f32,
    pub color: Color,
}

/// Android validation typography backed by the shared text stack.
pub struct CounterTypography {
    fonts: FontCollection,
    statistics: TypographyStatistics,
}

impl CounterTypography {
    /// Loads every system face that exists. The Swift initializer threw
    /// `CocoaError(.fileReadNoSuchFile)` when none could be opened, which is the
    /// empty-collection case here.
    pub fn new() -> Result<Self, String> {
        let mut fonts = FontCollection::new(2048, 2048);
        for path in SYSTEM_FACES {
            let Ok(bytes) = std::fs::read(path) else { continue };
            let _ = fonts.append(bytes);
        }
        if fonts.font_count() == 0 {
            return Err(format!(
                "no Android system face is readable (looked for {})",
                SYSTEM_FACES.join(", ")
            ));
        }
        Ok(Self {
            fonts,
            statistics: TypographyStatistics {
                fonts: fonts.font_count(),
                ..TypographyStatistics::default()
            },
        })
    }

    pub fn font_count(&self) -> usize {
        self.fonts.font_count()
    }

    /// Paints one run, charging glyph and missing-glyph counts to this frame.
    pub fn paint(&mut self, run: &CounterTextRun, list: &mut DrawList, origin: Point, scale: f32) {
        self.fonts.configure(run.size, scale);
        let glyphs = self.fonts.shape(&run.text);
        self.statistics.glyphs += glyphs.len();
        self.statistics.missing_glyphs +=
            glyphs.iter().filter(|glyph| glyph.glyph_id == 0).count();
        let point = Point {
            x: origin.x + run.inset,
            y: origin.y + if run.inset > 0.0 { 10.0 } else { 2.0 },
        };
        let _ = self.fonts.draw(&run.text, run.size, scale, list, point, run.color, 1);
    }

    /// Occupancy changes only when glyph pixels change. The cached count is kept
    /// across frames so input and diagnostic snapshots also remain cheap.
    pub fn finish_frame(&mut self) {
        let atlas = self.font_collection_atlas();
        if !atlas.is_dirty() {
            return;
        }
        self.statistics.atlas_pixels =
            atlas.alpha_pixels().iter().filter(|&&value| value > 0).count();
    }

    fn font_collection_atlas(&self) -> FontAtlas {
        self.fonts.atlas.clone()
    }
}

impl TypographyDriver for CounterTypography {
    fn begin_frame(&mut self) {
        self.statistics.fonts = self.fonts.font_count();
        self.statistics.glyphs = 0;
        self.statistics.missing_glyphs = 0;
    }

    fn finish_frame(&mut self) {
        CounterTypography::finish_frame(self);
    }

    fn statistics(&self) -> TypographyStatistics {
        self.statistics
    }

    fn atlas(&self) -> Option<FontAtlas> {
        Some(self.fonts.atlas.clone())
    }

    fn atlas_is_full(&self) -> bool {
        self.fonts.atlas.is_full()
    }
}
