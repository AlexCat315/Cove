//! Data story: the warehouse data table and its project tree.
//!
//! Swift source: `GalleryContent/Stories/Data/DataTableStory.swift`.
//!
//! Up to 200,000 rows across 33 columns, with a frozen first column, per-column
//! widths and visibility, in-cell editing with validation, and a tree whose
//! disclosure state drives what is visible.
//!
//! Behaviour checklist:
//! - loading rows regenerates them deterministically and clears the selection
//! - quantity cycles 0…249 through a fixed stride, so the data is reproducible
//! - the metric columns derive from the row id, not the display order
//! - the name column is required; quantity must be a nonnegative integer
//! - a tree flattens to exactly the rows whose ancestors are all expanded
//! - navigating to a row sets both row and column and bumps the request id

use cove_composition::Scope;
use std::cell::RefCell;
use std::rc::Rc;

use cove_core::State;
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::data::data_table::table_column::ComparisonResult;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, LayoutDebugExt, TupleView, View, ViewExt};

use super::feedback::StorySection;
use crate::gallery_content::catalog::GalleryOptions;

/// The row counts the loader offers.
pub const SMALL_LOAD: usize = 1_000;
pub const LARGE_LOAD: usize = 200_000;
/// The metric column count beyond the three authored columns.
pub const METRIC_COLUMNS: usize = 30;
/// The total column count, matching the story's title.
pub const TOTAL_COLUMNS: usize = 3 + METRIC_COLUMNS;
/// The category rotation.
pub const CATEGORIES: [&str; 3] = ["Texture", "Material", "Mesh"];

/// One warehouse row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WarehouseRow {
    pub id: usize,
    pub name: String,
    pub category: &'static str,
    pub quantity: usize,
}

impl WarehouseRow {
    /// The row at `id`, generated from the same formulas as the story.
    ///
    /// Quantity steps by 17 and wraps at 250, so the column looks irregular but
    /// is fully reproducible.
    pub fn generate(id: usize) -> Self {
        Self {
            id,
            name: format!("Asset {}", id + 1),
            category: CATEGORIES[id % 3],
            quantity: (id * 17) % 250,
        }
    }

    /// The metric at column `index` (0-based beyond the authored three).
    pub fn metric(&self, index: usize) -> usize {
        (self.id * (index + 3)) % 10_000
    }
}

/// What a cell edit produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CellEdit {
    /// The value is acceptable; write it.
    Accept(String),
    /// The value is rejected, with the message the field shows.
    Reject(&'static str),
}

/// Validates an asset name: it may not be blank.
pub fn validate_name(value: &str) -> Option<&'static str> {
    if value.trim().is_empty() {
        Some("Asset name is required")
    } else {
        None
    }
}

/// Validates a quantity: a nonnegative integer.
pub fn validate_quantity(value: &str) -> Option<&'static str> {
    match value.trim().parse::<i64>() {
        Ok(parsed) if parsed >= 0 => None,
        _ => Some("Quantity must be a nonnegative integer"),
    }
}

/// Applies a quantity edit, keeping the old value when it does not parse.
pub fn apply_quantity(row: &mut WarehouseRow, value: &str) {
    row.quantity = value.trim().parse::<usize>().unwrap_or(row.quantity);
}

/// Compares two names the way the name column sorts.
pub fn compare_names(left: &str, right: &str) -> ComparisonResult {
    match left.cmp(right) {
        std::cmp::Ordering::Less => ComparisonResult::Ascending,
        std::cmp::Ordering::Greater => ComparisonResult::Descending,
        std::cmp::Ordering::Equal => ComparisonResult::Same,
    }
}

/// Compares two quantities numerically, not by their display text.
pub fn compare_quantities(left: usize, right: usize) -> ComparisonResult {
    match left.cmp(&right) {
        std::cmp::Ordering::Less => ComparisonResult::Ascending,
        std::cmp::Ordering::Greater => ComparisonResult::Descending,
        std::cmp::Ordering::Equal => ComparisonResult::Same,
    }
}

/// A node of the project tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectNode {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: &'static str,
    pub size: &'static str,
    pub children: Vec<ProjectNode>,
}

impl ProjectNode {
    /// The authored sample tree.
    pub fn sample() -> Vec<ProjectNode> {
        let leaf = |id, name, size| ProjectNode {
            id,
            name,
            kind: "Source",
            size,
            children: Vec::new(),
        };
        vec![
            ProjectNode {
                id: "engine",
                name: "Engine",
                kind: "Package",
                size: "—",
                children: vec![
                    ProjectNode {
                        id: "render",
                        name: "Render",
                        kind: "Module",
                        size: "—",
                        children: vec![
                            leaf("metal", "MetalBackend.swift", "42 KB"),
                            leaf("vulkan", "VulkanBackend.swift", "51 KB"),
                        ],
                    },
                    ProjectNode {
                        id: "physics",
                        name: "Physics",
                        kind: "Module",
                        size: "—",
                        children: Vec::new(),
                    },
                ],
            },
            ProjectNode {
                id: "editor",
                name: "Editor",
                kind: "Package",
                size: "—",
                children: vec![leaf("inspector", "Inspector.swift", "18 KB")],
            },
            leaf("readme", "README.md", "6 KB"),
        ]
    }

    /// The children that make a node expandable.
    pub fn is_expandable(&self) -> bool {
        !self.children.is_empty()
    }
}

/// Flattens the tree to the rows currently visible, with each row's depth.
///
/// A node appears only when every ancestor is expanded, which is why a collapsed
/// branch hides its whole subtree rather than just its first level.
pub fn flatten(
    nodes: &[ProjectNode],
    expanded: &std::collections::HashSet<String>,
    depth: usize,
    out: &mut Vec<(String, usize)>,
) {
    for node in nodes {
        out.push((node.id.to_owned(), depth));
        if node.is_expandable() && expanded.contains(node.id) {
            flatten(&node.children, expanded, depth + 1, out);
        }
    }
}

/// The visible tree rows for a set of expanded ids.
pub fn visible_tree_rows(expanded: &std::collections::HashSet<String>) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    flatten(&ProjectNode::sample(), expanded, 0, &mut out);
    out
}

/// The sort bookkeeping the status line reports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SortStats {
    pub completed_builds: u64,
    pub cache_hits: u64,
    pub is_pending: bool,
}

impl SortStats {
    /// The state word in the status line.
    pub fn state_label(&self) -> &'static str {
        if self.is_pending {
            "Sorting…"
        } else {
            "Ready"
        }
    }

    /// The status line, with grouped row counts.
    pub fn status_line(&self, row_count: usize, selected: usize) -> String {
        format!(
            "{} rows · {selected} selected · {} · {} sort builds · {} cache hits",
            group_thousands(row_count),
            self.state_label(),
            self.completed_builds,
            self.cache_hits
        )
    }
}

/// Groups digits with thousands separators.
pub fn group_thousands(value: usize) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(character);
    }
    out
}

/// `DataTableStory`: the warehouse table plus the project tree.
pub struct DataTableStory {
    pub options: GalleryOptions,
    rows: Rc<RefCell<Vec<WarehouseRow>>>,
    selection: State<Vec<usize>>,
    status: State<String>,
    is_frozen: State<bool>,
    navigation: Rc<RefCell<Navigation>>,
    order: State<Vec<String>>,
    expanded: State<Vec<String>>,
}

/// The table's focused cell plus a request token.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Navigation {
    pub row: Option<usize>,
    pub request_id: u64,
}

impl DataTableStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            rows: Rc::new(RefCell::new(Vec::new())),
            selection: State::new(Vec::new()),
            status: State::new("Load a dataset to begin".to_owned()),
            is_frozen: State::new(true),
            navigation: Rc::new(RefCell::new(Navigation::default())),
            order: State::new(Vec::new()),
            // The sample tree opens with `engine` expanded.
            expanded: State::new(vec!["engine".to_owned()]),
        }
    }

    pub fn row_count(&self) -> usize {
        self.rows.borrow().len()
    }

    pub fn rows(&self) -> Vec<WarehouseRow> {
        self.rows.borrow().clone()
    }

    pub fn status(&self) -> String {
        self.status.get()
    }

    pub fn selection(&self) -> Vec<usize> {
        self.selection.get()
    }

    pub fn is_frozen(&self) -> bool {
        self.is_frozen.get()
    }

    pub fn order(&self) -> Vec<String> {
        self.order.get()
    }

    pub fn navigation(&self) -> Navigation {
        *self.navigation.borrow()
    }

    /// Replaces the dataset, clears the selection and focuses the first row.
    pub fn load(&self, count: usize) {
        let rows: Vec<WarehouseRow> = (0..count).map(WarehouseRow::generate).collect();
        *self.rows.borrow_mut() = rows;
        self.selection.set(Vec::new());
        self.status
            .set(format!("{} rows loaded", group_thousands(count)));
        self.navigate_to(0);
    }

    /// Moves focus to a row, leaving the column on the name column.
    pub fn navigate_to(&self, index: usize) {
        let mut navigation = self.navigation.borrow_mut();
        navigation.row = Some(index);
        navigation.request_id = navigation.request_id.wrapping_add(1);
    }

    /// Jumps to the middle row.
    pub fn navigate_middle(&self) -> bool {
        let count = self.row_count();
        if count == 0 {
            return false;
        }
        self.navigate_to(count / 2);
        true
    }

    /// Jumps to the last row.
    pub fn navigate_last(&self) -> bool {
        let count = self.row_count();
        if count == 0 {
            return false;
        }
        self.navigate_to(count - 1);
        true
    }

    /// Records the activation the table reported.
    pub fn activate(&self, row_id: usize) {
        let name = self
            .rows
            .borrow()
            .iter()
            .find(|row| row.id == row_id)
            .map(|row| row.name.clone())
            .unwrap_or_else(|| format!("Asset {}", row_id + 1));
        self.status.set(format!("Opened {name}"));
    }

    /// The order caption: authored until a reorder happens.
    pub fn order_caption(&self) -> String {
        let order = self.order.get();
        if order.is_empty() {
            "Order: authored".to_owned()
        } else {
            let head: Vec<String> = order.iter().take(4).cloned().collect();
            format!("Order: {}", head.join(", "))
        }
    }

    pub fn expanded(&self) -> Vec<String> {
        self.expanded.get()
    }

    pub fn visible_tree(&self) -> Vec<(String, usize)> {
        let set: std::collections::HashSet<String> = self.expanded.get().into_iter().collect();
        visible_tree_rows(&set)
    }

    /// Toggles a branch's disclosure.
    pub fn toggle_branch(&self, id: &str) {
        self.expanded.update(|current| {
            if let Some(position) = current.iter().position(|value| value == id) {
                current.remove(position);
            } else {
                current.push(id.to_owned());
            }
        });
    }

    /// The frozen column count the table uses.
    pub fn frozen_column_count(&self) -> usize {
        if self.is_frozen.get() { 1 } else { 0 }
    }
}

impl View for DataTableStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let caption = Text::new("Return commits · Escape cancels · Tab moves to the next editable cell · Arrows / Page Up / Page Down / Home / End navigate".to_string())
        .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
        .semantic_foreground(SemanticColorRef::on_surface_muted());

        let status = Text::new(self.status())
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted());

        AnyView::new(TupleView::new(vec![
            AnyView::new(StorySection::new(
                "Warehouse · up to 200,000 rows × 33 columns",
                "Load rows once. Narrow tables hide lower-priority columns. Use Columns to reveal them or restore Automatic columns; explicit choices scroll horizontally while Asset remains frozen. Double-click Asset or Quantity, or press F2, to edit.",
                AnyView::new(AnyView::new(status).debug_name("gallery-data-table")),
            )),
            AnyView::new(caption),
        ]))
    }

    fn tag(&self) -> &'static str {
        "DataTableStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; selection, status, is_frozen, order, expanded)
    }
}
