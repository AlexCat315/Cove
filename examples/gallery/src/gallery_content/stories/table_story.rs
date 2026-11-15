//! Data story: the warehouse data table and its project tree.
//!
//! Swift source: `GalleryContent/Stories/Data/DataTableStory.swift`.
//!
//! Up to 200,000 rows across 35 columns, with a frozen first column, per-column
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
use std::collections::{BTreeSet, HashSet};
use std::rc::Rc;

use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_core::{State, TextAlignment};
use cove_ui_compose::modifiers::layout::{LayoutFrameExt, PaddingExt};
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::controls::{Button, Toggle};
use cove_ui_compose::primitives::data::lists::Selection;
use cove_ui_compose::primitives::data::lists::tree::{
    Tree, TreeLayout, TreeOptions, TreeSelection,
};
use cove_ui_compose::primitives::data::table::table_column::{
    ColumnVisibility, ComparisonResult, TableColumn, TableColumnLayout, TableLayout,
    TableTextEditing,
};
use cove_ui_compose::primitives::data::table::{Table, TableModel, TableNavigation, TableOptions};
use cove_ui_compose::primitives::data::{TreeTable, TreeTableColumn, TreeTableOptions};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::{SemanticColorRef, SemanticFontRef, ThemeMetric};
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View};

use super::feedback::StorySection;
use crate::gallery_content::catalog::{CowSize, GalleryOptions};

/// The row counts the loader offers.
pub const SMALL_LOAD: usize = 1_000;
pub const LARGE_LOAD: usize = 200_000;
/// The metric column count beyond the five authored columns.
pub const METRIC_COLUMNS: usize = 30;
/// The total column count, matching the story's title.
pub const TOTAL_COLUMNS: usize = 5 + METRIC_COLUMNS;
/// The category rotation.
pub const CATEGORIES: [&str; 3] = ["Texture", "Material", "Mesh"];

/// One warehouse row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WarehouseRow {
    pub id: usize,
    pub name: String,
    pub category: &'static str,
    pub owner: &'static str,
    pub updated: String,
    pub quantity: usize,
}

/// Who the asset is assigned to, rotating by row id.
const OWNERS: [&str; 4] = ["Nia Park", "Kai Ito", "Rin Vos", "Ada Bell"];

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
            owner: OWNERS[id % 4],
            updated: format!("2026-11-{:02}", (id % 28) + 1),
            quantity: (id * 17) % 250,
        }
    }

    /// The metric at column `index` (0-based beyond the authored five).
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

/// `TableStory`: the warehouse table plus the project tree.
pub struct TableStory {
    pub options: GalleryOptions,
    model: Rc<TableModel<WarehouseRow, usize>>,
    selection: State<BTreeSet<usize>>,
    status: State<String>,
    is_frozen: State<bool>,
    navigation: State<Navigation>,
    order: State<Vec<String>>,
    expanded: State<HashSet<String>>,
    tree_expanded: State<BTreeSet<String>>,
    tree_selected: State<Option<String>>,
}

/// The table's focused cell plus a request token.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Navigation {
    pub row: Option<usize>,
    pub request_id: u64,
}

impl TableStory {
    pub fn new(options: GalleryOptions) -> Self {
        // A data table demo opens on a populated table: an empty grid is a
        // preview of the empty state, not of how the table behaves under load.
        let rows: Vec<WarehouseRow> = (0..SMALL_LOAD).map(WarehouseRow::generate).collect();
        Self {
            options,
            model: TableModel::try_new(rows, |row: &WarehouseRow| row.id)
                .expect("warehouse generator assigns one unique id per row"),
            selection: State::new(BTreeSet::new()),
            status: State::new("Ready".to_owned()),
            is_frozen: State::new(true),
            navigation: State::new(Navigation::default()),
            order: State::new(Vec::new()),
            // The sample tree opens with `engine` expanded.
            expanded: State::new(HashSet::from(["engine".to_owned()])),
            tree_expanded: State::new(BTreeSet::from(["scene".to_owned(), "lights".to_owned()])),
            tree_selected: State::new(Some("camera".to_owned())),
        }
    }

    pub fn row_count(&self) -> usize {
        self.model.count()
    }

    pub fn rows(&self) -> Vec<WarehouseRow> {
        (0..self.model.count())
            .filter_map(|index| self.model.record_at(index))
            .collect()
    }

    pub fn status(&self) -> String {
        self.status.get()
    }

    pub fn selection(&self) -> Vec<usize> {
        self.selection.get().into_iter().collect()
    }

    pub fn is_frozen(&self) -> bool {
        self.is_frozen.get()
    }

    pub fn order(&self) -> Vec<String> {
        self.order.get()
    }

    pub fn navigation(&self) -> Navigation {
        self.navigation.get()
    }

    /// Replaces the dataset, clears the selection and focuses the first row.
    pub fn load(&self, count: usize) {
        let rows: Vec<WarehouseRow> = (0..count).map(WarehouseRow::generate).collect();
        if let Err(error) = self.model.replace_rows(rows) {
            cove_ui_compose::core::Logger::new("com.cove.gallery.table")
                .error(format!("Could not load warehouse rows: {error}"));
            self.status.set("Could not load rows".to_owned());
            return;
        }
        self.selection.set(BTreeSet::new());
        // The card's toolbar owns the row count, so this strip reports events
        // only; a second copy of the same number 400pt below it is noise.
        self.status.set("Loaded".to_owned());
        self.navigate_to(0);
    }

    /// Moves focus to a row, leaving the column on the name column.
    pub fn navigate_to(&self, index: usize) {
        self.navigation.update(|navigation| {
            navigation.row = self.model.row_id_at(index);
            navigation.request_id = navigation.request_id.wrapping_add(1);
        });
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
            .model
            .index_for(&row_id)
            .and_then(|index| self.model.record_at(index))
            .map(|row| row.name)
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
        self.expanded.get().into_iter().collect()
    }

    pub fn visible_tree(&self) -> Vec<(String, usize)> {
        visible_tree_rows(&self.expanded.get())
    }

    /// Toggles a branch's disclosure.
    pub fn toggle_branch(&self, id: &str) {
        self.expanded.update(|current| {
            if !current.remove(id) {
                current.insert(id.to_owned());
            }
        });
    }

    /// The frozen column count the table uses.
    pub fn frozen_column_count(&self) -> usize {
        if self.is_frozen.get() { 1 } else { 0 }
    }

    fn handle(&self) -> Self {
        Self {
            options: self.options.clone(),
            model: self.model.clone(),
            selection: self.selection.clone(),
            status: self.status.clone(),
            is_frozen: self.is_frozen.clone(),
            navigation: self.navigation.clone(),
            order: self.order.clone(),
            expanded: self.expanded.clone(),
            tree_expanded: self.tree_expanded.clone(),
            tree_selected: self.tree_selected.clone(),
        }
    }

    /// The authored columns: name is required and editable, the rest optional.
    pub fn columns(&self) -> Vec<TableColumn<WarehouseRow>> {
        let sized = |width: f32| {
            let mut layout = TableColumnLayout::default();
            layout.width = width;
            layout
        };
        let mut columns = vec![
            TableColumn::new("name", "Asset", |row: &WarehouseRow| {
                AnyView::new(Text::new(row.name.clone()))
            })
            .layout(TableColumnLayout {
                is_flex: true,
                ..sized(184.0)
            })
            .text_editing(
                TableTextEditing::new(
                    |row: &WarehouseRow| row.name.clone(),
                    |row: &mut WarehouseRow, value: &str| row.name = value.to_owned(),
                )
                .validate(|value| validate_name(value).map(str::to_owned)),
            )
            .compare(|left: &WarehouseRow, right: &WarehouseRow| {
                compare_names(&left.name, &right.name)
            }),
            TableColumn::new("category", "Category", |row: &WarehouseRow| {
                AnyView::new(Text::new(row.category))
            })
            .layout(sized(136.0))
            .visibility(ColumnVisibility::Optional)
            .compare(|left: &WarehouseRow, right: &WarehouseRow| {
                compare_names(&left.category, &right.category)
            }),
            TableColumn::new("owner", "Owner", |row: &WarehouseRow| {
                AnyView::new(Text::new(row.owner))
            })
            .layout(sized(150.0))
            .visibility(ColumnVisibility::Optional)
            .compare(|left: &WarehouseRow, right: &WarehouseRow| {
                compare_names(&left.owner, &right.owner)
            }),
            TableColumn::new("updated", "Updated", |row: &WarehouseRow| {
                AnyView::new(Text::new(row.updated.clone()))
            })
            .layout({
                let mut layout = sized(140.0);
                layout.alignment = TextAlignment::Trailing;
                layout
            })
            .visibility(ColumnVisibility::Optional)
            .compare(|left: &WarehouseRow, right: &WarehouseRow| {
                compare_names(&left.updated, &right.updated)
            }),
            TableColumn::new("quantity", "Quantity", |row: &WarehouseRow| {
                AnyView::new(Text::new(row.quantity.to_string()))
            })
            .layout({
                let mut layout = sized(100.0);
                layout.alignment = TextAlignment::Trailing;
                layout
            })
            .visibility(ColumnVisibility::Optional)
            .priority(20)
            .compare(|left: &WarehouseRow, right: &WarehouseRow| {
                compare_quantities(left.quantity, right.quantity)
            })
            .text_editing(
                TableTextEditing::new(
                    |row: &WarehouseRow| row.quantity.to_string(),
                    apply_quantity,
                )
                .validate(|value| validate_quantity(value).map(str::to_owned)),
            ),
        ];
        columns.extend((0..METRIC_COLUMNS).map(|metric| {
            TableColumn::new(
                format!("metric-{}", metric + 1),
                format!("Metric {}", metric + 1),
                move |row: &WarehouseRow| AnyView::new(Text::new(row.metric(metric).to_string())),
            )
            .layout(TableColumnLayout {
                alignment: TextAlignment::Trailing,
                ..sized(100.0)
            })
            .visibility(ColumnVisibility::Hidden)
            .compare(move |left: &WarehouseRow, right: &WarehouseRow| {
                compare_quantities(left.metric(metric), right.metric(metric))
            })
        }));
        columns
    }
}

fn table_layout(size: CowSize) -> TableLayout {
    let (row_height, header_height) = match size {
        CowSize::Mini => (28.0, 28.0),
        CowSize::Small => (32.0, 30.0),
        CowSize::Regular => (36.0, 32.0),
        CowSize::Large => (44.0, 40.0),
    };
    TableLayout {
        row_height,
        header_height,
        ..Default::default()
    }
}

impl View for TableStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled = self.options.is_enabled;
        let navigation = self.navigation();
        let activate = self.handle();
        let table = AnyView::new(
            Table::try_new(self.model.clone(), self.columns())
                .expect("the gallery table columns are static with unique ids")
                .options(TableOptions {
                    is_enabled: enabled,
                    layout: table_layout(self.options.size),
                    frozen_column_count: self.frozen_column_count(),
                    column_order: Some(self.order.binding()),
                    selection: Some(Selection::multiple(self.selection.binding())),
                    navigation: TableNavigation {
                        request_id: navigation.row.map(|_| navigation.request_id),
                        row_id: navigation.row,
                        column_id: Some("name".to_owned()),
                    },
                    ..Default::default()
                })
                .on_activate(move |row: &WarehouseRow| activate.activate(row.id))
                .frame_height(322.0)
                .debug_name("data-table"),
        );

        let small = self.handle();
        let large = self.handle();
        let middle = self.handle();
        let last = self.handle();
        let controls = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::Wrap,
            8.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    Button::title("Load 1,000", move || small.load(SMALL_LOAD)).enabled(enabled),
                ),
                AnyView::new(
                    Button::title("Load 200,000", move || large.load(LARGE_LOAD)).enabled(enabled),
                ),
                AnyView::new(
                    Button::title("Middle row", move || {
                        middle.navigate_middle();
                    })
                    .enabled(enabled),
                ),
                AnyView::new(
                    Button::title("Last row", move || {
                        last.navigate_last();
                    })
                    .enabled(enabled),
                ),
                AnyView::new(
                    Toggle::labeled("Freeze Asset", self.is_frozen.binding()).enabled(enabled),
                ),
            ])),
        ));

        let mut warehouse = vec![controls, table];
        let status = self.status();
        if status != "Ready" && status != "Loaded" {
            warehouse.push(AnyView::new(
                Text::new(status)
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted())
                    .debug_name("gallery-data-table"),
            ));
        }

        let project = Tree::new(
            ProjectNode::sample(),
            |node: &ProjectNode| node.id.to_owned(),
            |node: &ProjectNode| node.children.clone(),
            |node: &ProjectNode, _| AnyView::new(Text::new(node.name).truncating_tail()),
        )
        .options(TreeOptions {
            is_enabled: enabled,
            selection: TreeSelection {
                expanded: Some(self.expanded.binding()),
                ..TreeSelection::local([])
            },
            layout: TreeLayout::default()
                .row_height(table_layout(self.options.size).row_height)
                .leading_padding(8.0)
                .trailing_padding(8.0),
            ..Default::default()
        });

        AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            24.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(StorySection::plain(
                    "Warehouse",
                    "Double-click Asset or Quantity, or press F2, to edit.",
                    AnyView::new(BoxView::new(
                        FlexDirection::Column,
                        Align::Stretch,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        8.0,
                        AnyView::new(TupleView::new(warehouse)),
                    )),
                )),
                AnyView::new(StorySection::plain(
                    "Project tree",
                    "",
                    AnyView::new(
                        project
                            .frame_height(184.0)
                            .padding_edges(0.0, 4.0)
                            .semantic_background(SemanticColorRef::surface_variant())
                            .corner_radius_metric(ThemeMetric::control_radius())
                            .debug_name("project-tree"),
                    ),
                )),
                AnyView::new(StorySection::plain(
                    "Tree table",
                    "",
                    AnyView::new(TreeTableSample {
                        options: self.options.clone(),
                        expanded: self.tree_expanded.clone(),
                        selected: self.tree_selected.clone(),
                    }),
                )),
            ])),
        ))
    }

    fn tag(&self) -> &'static str {
        "TableStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; status, is_frozen, navigation)
    }
}

// MARK: - Tree table sample

/// One asset in the tree-table sample.
#[derive(Clone, Debug, PartialEq)]
pub struct AssetNode {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub size: String,
    children: Vec<AssetNode>,
}

impl AssetNode {
    fn branch(id: &str, name: &str, kind: &str, children: Vec<AssetNode>) -> Self {
        Self {
            id: id.to_owned(),
            name: name.to_owned(),
            kind: kind.to_owned(),
            size: String::new(),
            children,
        }
    }

    fn leaf(id: &str, name: &str, kind: &str, size: &str) -> Self {
        Self {
            id: id.to_owned(),
            name: name.to_owned(),
            kind: kind.to_owned(),
            size: size.to_owned(),
            children: Vec::new(),
        }
    }

    /// The authored scene graph, three levels deep.
    pub fn sample() -> Vec<AssetNode> {
        vec![AssetNode::branch(
            "scene",
            "Scene",
            "Folder",
            vec![
                AssetNode::leaf("camera", "Camera", "Camera", "1 KB"),
                AssetNode::branch(
                    "lights",
                    "Lights",
                    "Folder",
                    vec![
                        AssetNode::leaf("sun", "Sun", "Directional", "2 KB"),
                        AssetNode::leaf("fill", "Fill", "Point", "2 KB"),
                    ],
                ),
                AssetNode::branch(
                    "meshes",
                    "Meshes",
                    "Folder",
                    vec![AssetNode::leaf("character", "Character", "Mesh", "18 MB")],
                ),
            ],
        )]
    }
}

/// The tree-table demonstration: real `TreeTable` bound to the story's own
/// expansion and selection state.
pub struct TreeTableSample {
    options: GalleryOptions,
    expanded: State<BTreeSet<String>>,
    selected: State<Option<String>>,
}

impl TreeTableSample {
    pub fn expanded(&self) -> BTreeSet<String> {
        self.expanded.get()
    }

    pub fn selected(&self) -> Option<String> {
        self.selected.get()
    }
}

impl View for TreeTableSample {
    fn body(&self, _cx: &Scope) -> AnyView {
        let options = TreeTableOptions {
            expanded: Some(self.expanded.binding()),
            selection: Some(Selection::single(self.selected.binding())),
            layout: table_layout(self.options.size),
            is_enabled: self.options.is_enabled,
            ..Default::default()
        };

        AnyView::new(
            TreeTable::try_new(
                AssetNode::sample(),
                |node: &AssetNode| node.id.clone(),
                |node: &AssetNode| (!node.children.is_empty()).then(|| node.children.clone()),
                TreeTableColumn::new("name", "Name", |node: &AssetNode| {
                    AnyView::new(Text::new(node.name.clone()).truncating_tail())
                })
                .layout(TableColumnLayout {
                    width: 208.0,
                    is_flex: true,
                    ..Default::default()
                }),
                [
                    TreeTableColumn::new("kind", "Type", |node: &AssetNode| {
                        AnyView::new(Text::new(node.kind.clone()).truncating_tail())
                    }),
                    TreeTableColumn::new("size", "Size", |node: &AssetNode| {
                        AnyView::new(Text::new(node.size.clone()))
                    })
                    .layout(TableColumnLayout {
                        width: 104.0,
                        alignment: TextAlignment::Trailing,
                        ..Default::default()
                    }),
                ],
            )
            .expect("the gallery tree-table columns are static with unique ids")
            .options(options)
            .frame_height(288.0)
            .debug_name("tree-table"),
        )
    }
}
