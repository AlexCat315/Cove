//! Render-ready snapshot of the viewport: cells, cursor, colors and scroll.
//!
//! Captured on the UI thread after the engine has consumed output, then painted
//! without touching the engine again.

/// 8-bit sRGB color as resolved by the VT engine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TerminalRGB {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl TerminalRGB {
    pub const WHITE: Self = Self { r: 0xFF, g: 0xFF, b: 0xFF };
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Converts a theme color, whose channels are `0.0...1.0` floats.
    pub fn from_color(color: cove_core::Color) -> Self {
        fn channel(value: f32) -> u8 {
            if !value.is_finite() {
                return 0;
            }
            (value * 255.0).round().clamp(0.0, 255.0) as u8
        }
        Self { r: channel(color.r), g: channel(color.g), b: channel(color.b) }
    }

    pub fn to_color(self) -> cove_core::Color {
        cove_core::Color { r: self.r as f32 / 255.0, g: self.g as f32 / 255.0, b: self.b as f32 / 255.0, a: 1.0 }
    }
}

/// SGR attributes that change how a cell is painted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct TerminalCellAttributes(pub u8);

impl TerminalCellAttributes {
    pub const BOLD: Self = Self(1 << 0);
    pub const ITALIC: Self = Self(1 << 1);
    pub const FAINT: Self = Self(1 << 2);
    pub const UNDERLINE: Self = Self(1 << 3);
    pub const STRIKETHROUGH: Self = Self(1 << 4);
    pub const INVERSE: Self = Self(1 << 5);
    pub const INVISIBLE: Self = Self(1 << 6);
    /// SGR 5/6: the text blinks (the cell background does not).
    pub const BLINK: Self = Self(1 << 7);

    pub const fn empty() -> Self {
        Self(0)
    }
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for TerminalCellAttributes {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}
impl std::ops::BitOrAssign for TerminalCellAttributes {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// Grid column width of a cell.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CellWidth {
    #[default]
    Narrow,
    Wide,
    /// Trailing half of a wide glyph, so column indices equal grid positions.
    Spacer,
}

/// One grid column. A wide glyph occupies its own cell plus a following
/// `Spacer` cell.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TerminalCell {
    /// Full grapheme cluster; empty for blank cells.
    pub text: String,
    pub width: CellWidth,
    /// `None` means the terminal default color.
    pub foreground: Option<TerminalRGB>,
    pub background: Option<TerminalRGB>,
    pub attributes: TerminalCellAttributes,
    pub is_selected: bool,
    /// Part of an OSC 8 hyperlink.
    pub is_hyperlink: bool,
}

impl TerminalCell {
    pub const fn blank() -> Self {
        Self {
            text: String::new(),
            width: CellWidth::Narrow,
            foreground: None,
            background: None,
            attributes: TerminalCellAttributes::empty(),
            is_selected: false,
            is_hyperlink: false,
        }
    }

    /// Reuses an existing cell's text allocation for a blank cell, which keeps
    /// the per-frame clear from allocating once per column.
    pub fn clear(&mut self) {
        self.text.clear();
        self.width = CellWidth::Narrow;
        self.foreground = None;
        self.background = None;
        self.attributes = TerminalCellAttributes::empty();
        self.is_selected = false;
        self.is_hyperlink = false;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CursorShape {
    Bar,
    #[default]
    Block,
    Underline,
    HollowBlock,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TerminalCursor {
    /// False when hidden by the program (DECTCEM) or scrolled out of view.
    pub is_visible: bool,
    pub column: usize,
    pub row: usize,
    pub shape: CursorShape,
    pub is_blinking: bool,
}

impl Default for TerminalCursor {
    fn default() -> Self {
        Self { is_visible: true, column: 0, row: 0, shape: CursorShape::Block, is_blinking: false }
    }
}

/// Viewport position inside the scrollback, in rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TerminalScrollPosition {
    pub total: usize,
    pub offset: usize,
    pub visible: usize,
}

impl TerminalScrollPosition {
    pub fn is_at_bottom(&self) -> bool {
        self.offset + self.visible >= self.total
    }
}

/// Render-ready snapshot of the viewport.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TerminalFrame {
    pub columns: usize,
    pub rows: usize,
    /// Row-major cells, `columns * rows` long.
    pub cells: Vec<TerminalCell>,
    pub cursor: TerminalCursor,
    pub foreground: TerminalRGB,
    pub background: TerminalRGB,
    pub scroll: TerminalScrollPosition,
    /// Kitty graphics placements in draw order.
    pub images: Vec<TerminalImagePlacement>,
    /// Kitty Unicode placeholder cells; their images are part of `images`.
    pub placeholders: Vec<PlaceholderCell>,
}

impl TerminalFrame {
    pub fn new(columns: usize, rows: usize) -> Self {
        Self {
            columns,
            rows,
            cells: vec![TerminalCell::blank(); columns * rows],
            cursor: TerminalCursor::default(),
            foreground: TerminalRGB::WHITE,
            background: TerminalRGB::BLACK,
            scroll: TerminalScrollPosition::default(),
            images: Vec::new(),
            placeholders: Vec::new(),
        }
    }

    pub fn cell(&self, row: usize, column: usize) -> &TerminalCell {
        &self.cells[row * self.columns + column]
    }

    pub fn cell_mut(&mut self, row: usize, column: usize) -> &mut TerminalCell {
        &mut self.cells[row * self.columns + column]
    }

    pub fn row(&self, index: usize) -> &[TerminalCell] {
        &self.cells[index * self.columns..(index + 1) * self.columns]
    }

    /// Plain text of one row with trailing blanks trimmed.
    pub fn text_of_row(&self, index: usize) -> String {
        let mut result = String::with_capacity(self.columns);
        for cell in self.row(index) {
            if cell.width == CellWidth::Spacer {
                continue;
            }
            if cell.text.is_empty() {
                result.push(' ');
            } else {
                result.push_str(&cell.text);
            }
        }
        while result.ends_with(' ') {
            result.pop();
        }
        result
    }
}
