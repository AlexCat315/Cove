//! Paints a `TerminalFrame` into a draw list.
//!
//! Glyphs are laid out one cell at a time. Monospaced columns then stay exact
//! even where fallback fonts (CJK, emoji) have other advances, and the shared
//! layout cache is keyed by grapheme, not by ever-changing program output.

use crate::engine::{
    CellWidth, CursorShape, PLACEHOLDER_CODEPOINT, TerminalCell, TerminalCellAttributes,
    TerminalFrame, TerminalImagePlacement, TerminalRGB,
};
use crate::view::{box_drawing, cell_metrics::CellMetrics, url_detector::UrlMatch};
use cove_core::{Color, DrawList, Font, Point, Rect, TextureId};
use cove_text::{DrawListText, TextEnvironment};

pub struct TerminalPainter<'a> {
    pub frame: &'a TerminalFrame,
    pub metrics: &'a CellMetrics,
    pub environment: &'a TextEnvironment,
    pub is_focused: bool,
    pub opacity: f32,
    /// IME composition shown at the cursor until committed.
    pub preedit: String,
    /// False during the off phase of a blinking cursor.
    pub is_cursor_shown: bool,
    /// False during the off phase of SGR 5/6 blinking text.
    pub is_blinking_text_shown: bool,
    /// A detected plain-text URL under the pointer, underlined like a link.
    pub hovered_link: Option<UrlMatch>,
    /// Kitty graphics placements with their uploaded textures, in z order.
    pub images: Vec<(TerminalImagePlacement, TextureId)>,
    /// Cell size the engine measures image pixels in.
    pub cell_pixels: (usize, usize),
}

impl<'a> TerminalPainter<'a> {
    /// `bounds` is the whole view: the partial cell left over at the right and
    /// bottom edges still takes the terminal background.
    pub fn paint(&self, list: &mut DrawList, bounds: Rect) {
        let _ = list.add_rect(bounds, self.color(self.frame.background));
        for row in 0..self.frame.rows {
            self.paint_backgrounds(
                list,
                row,
                bounds.y + row as f32 * self.metrics.height,
                bounds.x,
            );
        }
        self.paint_images(list, true, bounds);
        for row in 0..self.frame.rows {
            self.paint_glyphs(
                list,
                row,
                bounds.y + row as f32 * self.metrics.height,
                bounds.x,
            );
        }
        self.paint_images(list, false, bounds);
        self.paint_cursor(list, bounds);
    }

    fn paint_backgrounds(&self, list: &mut DrawList, row: usize, top: f32, origin_x: f32) {
        let mut run_start = 0usize;
        let mut run_color: Option<TerminalRGB> = None;
        for column in 0..self.frame.columns {
            let cell = self.frame.cell(row, column);
            let fill = self.colors(cell).1;
            let next = if fill == self.frame.background {
                None
            } else {
                Some(fill)
            };
            if next != run_color {
                if let Some(color) = run_color
                    && column > run_start
                {
                    let _ = list.add_rect(
                        Rect::new(
                            origin_x + run_start as f32 * self.metrics.width,
                            top,
                            (column - run_start) as f32 * self.metrics.width,
                            self.metrics.height,
                        ),
                        self.color(color),
                    );
                }
                run_start = column;
                run_color = next;
            }
        }
        if let Some(color) = run_color
            && self.frame.columns > run_start
        {
            let _ = list.add_rect(
                Rect::new(
                    origin_x + run_start as f32 * self.metrics.width,
                    top,
                    (self.frame.columns - run_start) as f32 * self.metrics.width,
                    self.metrics.height,
                ),
                self.color(color),
            );
        }
    }

    fn paint_glyphs(&self, list: &mut DrawList, row: usize, top: f32, origin_x: f32) {
        for column in 0..self.frame.columns {
            let cell = self.frame.cell(row, column);
            if cell.width == CellWidth::Spacer
                || cell.attributes.contains(TerminalCellAttributes::INVISIBLE)
                || (!self.is_blinking_text_shown
                    && cell.attributes.contains(TerminalCellAttributes::BLINK))
            {
                continue;
            }
            let scalar = cell.text.chars().next().map(u32::from).unwrap_or(0);
            // Kitty placeholders stand in for image strips drawn separately.
            if scalar == PLACEHOLDER_CODEPOINT {
                continue;
            }
            let left = origin_x + column as f32 * self.metrics.width;
            let span = if cell.width == CellWidth::Wide {
                2.0
            } else {
                1.0
            } * self.metrics.width;
            let mut foreground = self.color(self.colors(cell).0);
            if cell.attributes.contains(TerminalCellAttributes::FAINT) {
                foreground = foreground.multiplied_alpha(0.6);
            }
            if box_drawing::handles(scalar) && cell.text.chars().count() == 1 {
                box_drawing::draw(
                    scalar,
                    Rect::new(left, top, span, self.metrics.height),
                    foreground,
                    list,
                );
            } else if !cell.text.is_empty() && cell.text != " " {
                let layout = self.environment.cached_layout(
                    &cell.text,
                    Some(&self.font(cell)),
                    Some(self.metrics.height),
                    f32::INFINITY,
                    cove_core::TextAlignment::Leading,
                    0.0,
                );
                let _ = list.add_text(
                    &layout,
                    Point::new(left, top),
                    foreground,
                    self.environment.atlas_texture_id,
                    Some(&self.environment.atlas),
                    None,
                );
            }
            let is_hovered_link = self.hovered_link.as_ref().is_some_and(|link| {
                link.row == row && (link.columns.0..=link.columns.1).contains(&column)
            });
            if cell.attributes.contains(TerminalCellAttributes::UNDERLINE)
                || cell.is_hyperlink
                || is_hovered_link
            {
                let _ = list.add_rect(
                    Rect::new(left, top + self.metrics.height - 2.0, span, 1.0),
                    foreground,
                );
            }
            if cell
                .attributes
                .contains(TerminalCellAttributes::STRIKETHROUGH)
            {
                let _ = list.add_rect(
                    Rect::new(left, top + (self.metrics.height / 2.0).round(), span, 1.0),
                    foreground,
                );
            }
        }
    }

    /// Kitty placements: `z < 0` sits between cell backgrounds and text.
    fn paint_images(&self, list: &mut DrawList, below_text: bool, bounds: Rect) {
        let scale_x = self.metrics.width / self.cell_pixels.0.max(1) as f32;
        let scale_y = self.metrics.height / self.cell_pixels.1.max(1) as f32;
        for (placement, texture) in &self.images {
            if placement.is_below_text != below_text {
                continue;
            }
            let size = placement.image_size;
            if size.0 == 0 || size.1 == 0 {
                continue;
            }
            let rect = Rect::new(
                bounds.x
                    + placement.column as f32 * self.metrics.width
                    + placement.offset.0 as f32 * scale_x,
                bounds.y
                    + placement.row as f32 * self.metrics.height
                    + placement.offset.1 as f32 * scale_y,
                placement.width as f32 * scale_x,
                placement.height as f32 * scale_y,
            );
            let source = placement.source;
            let min = Point::new(
                source.0 as f32 / size.0 as f32,
                source.1 as f32 / size.1 as f32,
            );
            let max = Point::new(
                (source.0 + source.2) as f32 / size.0 as f32,
                (source.1 + source.3) as f32 / size.1 as f32,
            );
            let tint = Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: self.opacity,
            };
            let _ = list.add_image_quad(rect, *texture, tint, min, max);
        }
    }

    fn paint_cursor(&self, list: &mut DrawList, bounds: Rect) {
        let cursor = &self.frame.cursor;
        if !cursor.is_visible
            || cursor.row >= self.frame.rows
            || cursor.column >= self.frame.columns
        {
            return;
        }
        let cell = self.frame.cell(cursor.row, cursor.column);
        let left = bounds.x + cursor.column as f32 * self.metrics.width;
        let top = bounds.y + cursor.row as f32 * self.metrics.height;
        let width = if cell.width == CellWidth::Wide {
            2.0
        } else {
            1.0
        } * self.metrics.width;
        let ink = self.color(self.colors(cell).0);

        if !self.preedit.is_empty() {
            let layout = self.environment.cached_layout(
                &self.preedit,
                Some(&self.metrics.font),
                Some(self.metrics.height),
                f32::INFINITY,
                cove_core::TextAlignment::Leading,
                0.0,
            );
            let _ = list.add_rect(
                Rect::new(
                    left,
                    top,
                    width.max(layout.total_width),
                    self.metrics.height,
                ),
                self.color(self.frame.background),
            );
            let _ = list.add_text(
                &layout,
                Point::new(left, top),
                ink,
                self.environment.atlas_texture_id,
                Some(&self.environment.atlas),
                None,
            );
            let _ = list.add_rect(
                Rect::new(
                    left,
                    top + self.metrics.height - 2.0,
                    layout.total_width,
                    1.0,
                ),
                ink,
            );
            return;
        }
        if !self.is_focused {
            let _ = list.add_rounded_rect_stroke(
                Rect::new(left, top, width, self.metrics.height),
                0.0,
                1.0,
                ink,
            );
            return;
        }
        if !self.is_cursor_shown {
            return;
        }
        match cursor.shape {
            CursorShape::Block => {
                // The block covers the glyph painted earlier; repaint it inverted.
                let _ = list.add_rect(Rect::new(left, top, width, self.metrics.height), ink);
                if !cell.text.is_empty() && cell.text != " " {
                    let layout = self.environment.cached_layout(
                        &cell.text,
                        Some(&self.font(cell)),
                        Some(self.metrics.height),
                        f32::INFINITY,
                        cove_core::TextAlignment::Leading,
                        0.0,
                    );
                    let _ = list.add_text(
                        &layout,
                        Point::new(left, top),
                        self.color(self.colors(cell).1),
                        self.environment.atlas_texture_id,
                        Some(&self.environment.atlas),
                        None,
                    );
                }
            }
            CursorShape::HollowBlock => {
                let _ = list.add_rounded_rect_stroke(
                    Rect::new(left, top, width, self.metrics.height),
                    0.0,
                    1.0,
                    ink,
                );
            }
            CursorShape::Bar => {
                let _ = list.add_rect(Rect::new(left, top, 2.0, self.metrics.height), ink);
            }
            CursorShape::Underline => {
                let _ = list.add_rect(
                    Rect::new(left, top + self.metrics.height - 2.0, width, 2.0),
                    ink,
                );
            }
        }
    }

    /// Effective colors after defaults, inverse video and selection.
    fn colors(&self, cell: &TerminalCell) -> (TerminalRGB, TerminalRGB) {
        let mut foreground = cell.foreground.unwrap_or(self.frame.foreground);
        let mut background = cell.background.unwrap_or(self.frame.background);
        if cell.attributes.contains(TerminalCellAttributes::INVERSE) != cell.is_selected {
            std::mem::swap(&mut foreground, &mut background);
        }
        (foreground, background)
    }

    fn font(&self, cell: &TerminalCell) -> Font {
        let mut font = self.metrics.font.clone();
        if cell.attributes.contains(TerminalCellAttributes::BOLD) {
            font = font.with_weight(cove_core::FontWeight::Bold);
        }
        if cell.attributes.contains(TerminalCellAttributes::ITALIC) {
            font = font.with_italic(true);
        }
        font
    }

    fn color(&self, rgb: TerminalRGB) -> Color {
        rgb.to_color().multiplied_alpha(self.opacity)
    }
}
