//! Translation of the column model in Primitives/Data/Lists/Table.swift and of
//! Primitives/Data/Columns/Visibility.swift via the `TableColumnPlan` type below.
//!
//! What a `DataTable` column is: its cell, width, visibility, comparison and text
//! editing.
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::AnyView;
use crate::primitives::layout::grid::{ColumnVisibilityPlan, ColumnVisibilityTrack};
use cove_core::TextAlignment;

/// Which way a column is sorted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TableSortDirection {
    #[default]
    Ascending,
    Descending,
}

/// The active sort: which column, and which way.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TableSort {
    pub column_id: String,
    pub direction: TableSortDirection,
}

impl TableSort {
    pub fn new(column_id: impl Into<String>) -> Self {
        Self {
            column_id: column_id.into(),
            direction: TableSortDirection::Ascending,
        }
    }

    pub fn descending(mut self) -> Self {
        self.direction = TableSortDirection::Descending;
        self
    }
}

/// Per-column width, alignment and resizability.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TableColumnLayout {
    pub width: f32,
    pub min_width: f32,
    pub max_width: f32,
    pub alignment: TextAlignment,
    pub is_resizable: bool,
}

impl Default for TableColumnLayout {
    fn default() -> Self {
        Self {
            width: 160.0,
            min_width: 60.0,
            max_width: 800.0,
            alignment: TextAlignment::Leading,
            is_resizable: true,
        }
    }
}

impl TableColumnLayout {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn constrained(&self, value: f32) -> f32 {
        let minimum = if self.min_width.is_finite() {
            self.min_width.max(24.0)
        } else {
            60.0
        };
        let maximum = if self.max_width.is_finite() {
            self.max_width.max(minimum)
        } else {
            800.0
        };
        maximum.min(minimum.max(if value.is_finite() { value } else { minimum }))
    }
}

/// Whether a column must be shown, must be hidden, or is automatic.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ColumnVisibility {
    /// At least one column must stay visible.
    #[default]
    Required,
    /// Hidden when it does not fit, but shown if asked.
    Optional,
    /// Never shown unless explicitly overridden.
    Hidden,
}

/// Sort ordering between two records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComparisonResult {
    Ascending,
    Same,
    Descending,
}

/// How a column's cell becomes editable text.
///
/// The table owns only the draft lifecycle; reading the current value and
/// writing the accepted one stay with the caller, so a record type needs no
/// editor protocol of its own.
pub struct TableTextEditing<Record> {
    pub text: Rc<dyn Fn(&Record) -> String>,
    pub update: Rc<dyn Fn(&mut Record, &str)>,
    /// A message rejects the draft and leaves the authored row untouched.
    pub validate: Option<Rc<dyn Fn(&str) -> Option<String>>>,
}

impl<Record> Clone for TableTextEditing<Record> {
    fn clone(&self) -> Self {
        Self {
            text: self.text.clone(),
            update: self.update.clone(),
            validate: self.validate.clone(),
        }
    }
}

impl<Record> TableTextEditing<Record> {
    pub fn new(
        text: impl Fn(&Record) -> String + 'static,
        update: impl Fn(&mut Record, &str) + 'static,
    ) -> Self {
        Self {
            text: Rc::new(text),
            update: Rc::new(update),
            validate: None,
        }
    }

    pub fn validate(mut self, validate: impl Fn(&str) -> Option<String> + 'static) -> Self {
        self.validate = Some(Rc::new(validate));
        self
    }

    pub fn text_of(&self, record: &Record) -> String {
        (self.text)(record)
    }

    pub fn apply(&self, record: &mut Record, value: &str) {
        (self.update)(record, value);
    }

    pub fn rejection(&self, value: &str) -> Option<String> {
        self.validate.as_ref().and_then(|validate| validate(value))
    }
}

/// One column: title, layout, sort comparison and cell content.
pub struct TableColumn<Record> {
    pub id: String,
    pub title: String,
    pub layout: TableColumnLayout,
    pub visibility: ColumnVisibility,
    /// Equal comparisons retain source order in both sort directions.
    pub compare: Option<Rc<dyn Fn(&Record, &Record) -> ComparisonResult>>,
    /// Present when the column can be edited in place.
    pub text_editing: Option<TableTextEditing<Record>>,
    content: Rc<dyn Fn(&Record) -> AnyView>,
}

impl<Record> Clone for TableColumn<Record> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            title: self.title.clone(),
            layout: self.layout,
            visibility: self.visibility,
            compare: self.compare.clone(),
            text_editing: self.text_editing.clone(),
            content: self.content.clone(),
        }
    }
}

impl<Record> TableColumn<Record> {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        content: impl Fn(&Record) -> AnyView + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            layout: TableColumnLayout::default(),
            visibility: ColumnVisibility::Required,
            compare: None,
            text_editing: None,
            content: Rc::new(content),
        }
    }

    pub fn layout(mut self, layout: TableColumnLayout) -> Self {
        self.layout = layout;
        self
    }

    pub fn visibility(mut self, visibility: ColumnVisibility) -> Self {
        self.visibility = visibility;
        self
    }

    pub fn compare(
        mut self,
        compare: impl Fn(&Record, &Record) -> ComparisonResult + 'static,
    ) -> Self {
        self.compare = Some(Rc::new(compare));
        self
    }

    pub fn text_editing(mut self, editing: TableTextEditing<Record>) -> Self {
        self.text_editing = Some(editing);
        self
    }

    pub fn cell(&self, record: &Record) -> AnyView {
        (self.content)(record)
    }
}

/// Row and header density.
#[derive(Clone, Debug, PartialEq)]
pub struct TableLayout {
    /// Authored logical height, before text scaling and the touch density floor.
    pub row_height: f32,
    /// Authored logical height, before text scaling and the touch density floor.
    pub header_height: f32,
    pub is_striped: bool,
    pub empty_title: String,
}

impl Default for TableLayout {
    fn default() -> Self {
        Self {
            row_height: 36.0,
            header_height: 38.0,
            is_striped: false,
            empty_title: "No records".to_string(),
        }
    }
}

impl TableLayout {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate(&mut self) {
        self.row_height = if self.row_height.is_finite() {
            self.row_height.max(24.0)
        } else {
            36.0
        };
        self.header_height = if self.header_height.is_finite() {
            self.header_height.max(24.0)
        } else {
            38.0
        };
    }

    pub fn validated(mut self) -> Self {
        self.validate();
        self
    }

    /// Row and header heights after text scale and the touch density floor.
    pub fn resolved(&self, text_scale: f32, touch: bool, minimum_extent: f32) -> Self {
        let mut result = self.clone();
        result.row_height *= text_scale;
        result.header_height *= text_scale;
        if touch {
            result.row_height = result.row_height.max(minimum_extent);
            result.header_height = result.header_height.max(minimum_extent);
        }
        result
    }
}

/// Resolves which table columns are shown for a given available width.
///
/// Translation of `Primitives/Data/Columns/Visibility.swift`. The generic
/// `ColumnVisibilityPlan` (shared with `Grid`) decides *which* columns fit; this
/// adapter supplies the table-specific inputs: measured widths, the pinned and
/// frozen prefixes, and the rule that a frozen prefix wider than the viewport
/// stops being frozen.
#[derive(Clone, Debug, PartialEq)]
pub struct TableColumnPlan {
    /// Indices of the columns to show, in authored order.
    pub indices: Vec<usize>,
    /// Columns that must stay available regardless of width.
    pub required_ids: std::collections::HashSet<String>,
    /// Leading columns kept out of horizontal scrolling.
    pub frozen_count: usize,
    pub available_width: f32,
}

/// The table's three-way visibility maps onto the shared plan's two-way one.
///
/// `Hidden` is handled by the table before planning (the column is filtered out
/// entirely), so only `Required` and `Optional` reach the resolver. Optional
/// columns all share priority 0, which keeps them in authored order.
fn table_visibility(
    visibility: ColumnVisibility,
) -> crate::primitives::layout::grid::ColumnVisibility {
    match visibility {
        ColumnVisibility::Required => crate::primitives::layout::grid::ColumnVisibility::Required,
        ColumnVisibility::Optional | ColumnVisibility::Hidden => {
            crate::primitives::layout::grid::ColumnVisibility::Automatic { priority: 0 }
        }
    }
}

impl TableColumnPlan {
    /// Widths may be user-resized; anything absent falls back to the authored
    /// width, constrained to the column's own min/max.
    pub fn tracks<Record>(
        columns: &[TableColumn<Record>],
        widths: &BTreeMap<String, f32>,
    ) -> Vec<ColumnVisibilityTrack> {
        columns
            .iter()
            .map(|column| {
                let authored = widths
                    .get(&column.id)
                    .copied()
                    .unwrap_or(column.layout.width);
                ColumnVisibilityTrack {
                    id: column.id.clone(),
                    title: column.title.clone(),
                    visibility: table_visibility(column.visibility),
                    width: column.layout.constrained(authored),
                }
            })
            .collect()
    }

    pub fn resolve<Record>(
        columns: &[TableColumn<Record>],
        widths: &BTreeMap<String, f32>,
        overrides: &std::collections::HashMap<String, bool>,
        available_width: f32,
        frozen_count: usize,
        pinned_ids: &std::collections::HashSet<String>,
    ) -> Self {
        let frozen_count = frozen_count.min(columns.len());
        let tracks = Self::tracks(columns, widths);

        // Pinned columns and the frozen prefix can never be dropped.
        let mut required: std::collections::HashSet<String> = pinned_ids.clone();
        required.extend(
            columns
                .iter()
                .take(frozen_count)
                .map(|column| column.id.clone()),
        );

        let plan =
            ColumnVisibilityPlan::resolve(&tracks, overrides, available_width, 0.0, &required);

        // A frozen prefix that fills the viewport cannot stay frozen: there would
        // be no scrollable region left for the remaining columns.
        let frozen_width: f32 = tracks
            .iter()
            .take(frozen_count)
            .map(|track| track.width)
            .sum();
        let frozen_count = if frozen_width >= available_width.max(0.0) {
            0
        } else {
            frozen_count
        };

        Self {
            indices: plan.indices,
            required_ids: plan.required_ids,
            frozen_count,
            available_width: available_width.max(0.0),
        }
    }
}
