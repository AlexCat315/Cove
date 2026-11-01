//! Translation of `Primitives/Data/DataTable/{DataTableHosts,EditorPresence}.swift`.
//!
//! Row, cell and input hosts, plus the editor-presence probe.
//!
//! Behaviour checklist:
//! - row fill is accent when selected, a half-strength stripe on odd rows, and
//!   the plain background otherwise — selection wins over striping
//! - a row's update is skipped when nothing changed, so scrolling a large table
//!   does not restyle every visible row
//! - a frozen-lane row announces itself as a group, a scrolling row as a row
//! - a cell is a cell with a selected/enabled state, and only an enabled cell
//!   exposes the activate action
//! - an active cell draws a focus-ring border
//! - the editor-presence probe reports whether the detached editor still lives
//!   inside the focus root

use cove_core::Color;

use super::data_table_geometry::DataTableColumnGeometry;

/// What a row host needs to repaint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowHostUpdate {
    pub index: usize,
    pub is_selected: bool,
    pub is_striped: bool,
    pub is_enabled: bool,
    pub is_frozen_lane: bool,
}

impl RowHostUpdate {
    /// Whether the row should restyle at all.
    ///
    /// Every layout pass constructs a new update; comparing first is what keeps
    /// a scroll from restyling every visible row.
    pub fn changed_from(&self, previous: Option<&RowHostUpdate>) -> bool {
        previous != Some(self)
    }

    /// Whether this row carries the alternating stripe.
    ///
    /// Striping is on odd indices only, and a selected row reads as selected
    /// rather than striped.
    pub fn is_striped_row(&self) -> bool {
        self.is_striped && self.index % 2 == 1
    }

    /// The row fill.
    pub fn background(&self, accent: Color, surface: Color, background: Color) -> Color {
        if self.is_selected {
            accent
        } else if self.is_striped_row() {
            surface.multiplied_alpha(0.6)
        } else {
            background
        }
    }

    /// The accessibility role: a frozen lane is a group, a scrolling row a row.
    pub fn accessibility_role(&self) -> RowAccessibilityRole {
        if self.is_frozen_lane {
            RowAccessibilityRole::Group
        } else {
            RowAccessibilityRole::Row
        }
    }

    /// The row's announced label.
    pub fn accessibility_label(&self) -> String {
        format!("Row {}", self.index + 1)
    }
}

/// The accessibility role a row announces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowAccessibilityRole {
    Group,
    Row,
}

/// What a cell host needs to repaint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellHostUpdate<'a> {
    pub title: &'a str,
    pub is_active: bool,
    pub is_enabled: bool,
}

impl<'a> CellHostUpdate<'a> {
    pub fn changed_from(&self, previous: Option<&CellHostUpdate<'a>>) -> bool {
        previous != Some(self)
    }

    /// The focus-ring border width: one point while this is the active cell.
    pub fn border_width(&self) -> f32 {
        if self.is_active { 1.0 } else { 0.0 }
    }

    /// Whether the cell exposes the activate accessibility action.
    pub fn exposes_activate(&self) -> bool {
        self.is_enabled
    }
}

/// The persistence rule for a datatable cell edit.
///
/// A cell's editor is a detached window that may move between the frozen and
/// scrolling panes. That reconciliation must not be read as a user committing a
/// draft — only a real blur outside the still-present editor commits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorBlurOutcome {
    /// The editor is still inside the focus root; keep the draft open.
    KeepDraft,
    /// The editor left the tree; commit the draft.
    Commit,
}

/// Decides what a blur means for a detached editor.
pub fn editor_blur_outcome(editor_still_present_inside_root: bool) -> EditorBlurOutcome {
    if editor_still_present_inside_root {
        EditorBlurOutcome::KeepDraft
    } else {
        EditorBlurOutcome::Commit
    }
}

/// The horizontal extent of a header drag, in header coordinates.
///
/// A drop indicator inside the frozen prefix is not offset by the horizontal
/// scroll position, because that region does not scroll.
pub fn drop_indicator_x(
    boundary: f32,
    geometry: &DataTableColumnGeometry,
    horizontal_offset: f32,
) -> f32 {
    if boundary <= geometry.frozen_width() {
        boundary
    } else {
        boundary - horizontal_offset
    }
}
