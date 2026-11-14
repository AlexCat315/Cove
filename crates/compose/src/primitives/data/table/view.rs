//! Translation of `Primitives/Data/DataTable/DataTable.swift` (the view half).
//!
//! A persistent row model, fixed header, frozen leading columns and a two-axis
//! cell window. Only visible rows and columns — plus overscan — create view
//! nodes, so a 200,000 x 100 table mounts a few dozen cells.
//!
//! This file composes the rest of the module instead of re-deriving it:
//! [`DataTableColumnGeometry`] resolves column spans and the visible column
//! window, [`DataTableRowMetrics`] resolves row spans, [`TableColumnPlan`]
//! resolves which columns fit, [`CellEditing`] owns the draft lifecycle, and
//! the host/editing primitives carry the per-node update contract.
//!
//! Behaviour checklist:
//! - rows and columns outside the window never build a cell
//! - the frozen prefix lives in its own vertically offset lane, so a frozen
//!   cell stays column-aligned with the moving rows at every offset
//! - the header follows the horizontal offset but never the vertical one
//! - a frozen prefix wider than the viewport stops being frozen
//! - keyboard navigation walks rows and columns; Page Up/Down step by viewport,
//!   Home/End reach the ends, F2 and Return edit, Escape clears the selection
//! - sorting a column cycles ascending, descending, source order
//! - a navigation request reveals its cell once per request identity

use crate::Scope;
use crate::primitives::{DuplicateId, check_ids};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, AccessibilityState, Align,
    EventPhase, EventResult, FlexDirection, FocusChainHolder, HoverPhase, InputHandlerRoute,
    InteractionRegistryHolder, Justify, LayoutNode, Node, PointerCaptureHolder, PointerPhase, Wrap,
};
use cove_core::{Binding, Color, Point, Size, State, TextAlignment};
use cove_platform::{KeyEvent, KeyModifiers, MouseButton, SystemCursor};

use super::column_order::DataTableColumnOrder;
use super::data_table_geometry::DataTableColumnGeometry;
use super::editing_model::{CellEditing, DataTableEditingState, EditableColumn};
use super::model::DataTableModel;
use super::row_metrics::{
    DataTableRowMetrics, RowHeightKey, row_height_key_matches, variable_row_heights,
};
use super::selection_model::{DataTableNavigation, DataTableSelection, DataTableSelectionMode};
use super::table_column::{
    ColumnVisibility, TableColumn, TableColumnPlan, TableLayout, TableSort, TableSortDirection,
};
use crate::modifiers::layout::{EdgeInsets, FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::data::lists::selection::Selection;
use crate::primitives::feedback::EmptyState;
use crate::primitives::layout::{ScrollAxis, ScrollView, ScrollbarGutter};
use crate::primitives::text::text_buffer::TextBuffer;
use crate::primitives::text::text_field::{
    TextField, TextFieldBehavior, TextFieldEvents, TextFieldNavigationRequest,
};
use crate::primitives::text::{Divider, Text};
use crate::theme::{SemanticColorRef, SemanticFontRef, node_text_scaler, theme_for_node};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View, impl_primitive_view};
use cove_core::scene::ObservableObject;

const INPUT_MARKER: &str = "dataTable.input";
const ROW_INDEX_MARKER: &str = "dataTable.rowIndex";
const METRICS_CACHE: &str = "dataTable.metrics";
const COLUMN_DRAG: &str = "dataTable.columnDrag";
/// Pointer travel before a header press becomes a column drag.
const COLUMN_DRAG_SLOP: f32 = crate::primitives::interaction::DRAG_THRESHOLD;

/// The hovered row, painted directly on its nodes so entering a row recomposes
/// nothing. The frozen and moving lanes each mount a node for the same row.
#[derive(Default)]
pub struct DataTableRowHover {
    row: std::cell::Cell<Option<usize>>,
    nodes: RefCell<HashMap<usize, Vec<(cove_core::scene::NodeWeak, Color, bool)>>>,
}

impl DataTableRowHover {
    fn register(&self, row: usize, node: &Node, fill: Color, is_selected: bool) {
        let mut nodes = self.nodes.borrow_mut();
        let lane = nodes.entry(row).or_default();
        lane.retain(|(weak, _, _)| Node::upgrade(weak).is_some_and(|known| known != *node));
        lane.push((node.downgrade(), fill, is_selected));
    }

    fn paint(&self, row: usize, hovered: bool) {
        let mut nodes = self.nodes.borrow_mut();
        let Some(lane) = nodes.get_mut(&row) else {
            return;
        };
        lane.retain(|(weak, _, _)| Node::upgrade(weak).is_some());
        for (weak, fill, is_selected) in lane.iter() {
            let Some(node) = Node::upgrade(weak) else {
                continue;
            };
            let paint = if hovered && !*is_selected {
                theme_for_node(&node).colors.state_layer_hover
            } else {
                *fill
            };
            node.set_background_color(Some(paint));
        }
    }

    fn set(&self, row: Option<usize>) {
        let previous = self.row.replace(row);
        if previous == row {
            return;
        }
        if let Some(previous) = previous {
            self.paint(previous, false);
        }
        if let Some(row) = row {
            self.paint(row, true);
        }
    }

    fn current(&self) -> Option<usize> {
        self.row.get()
    }
}

/// Authored options for a `DataTable`.
pub struct DataTableOptions<Id: Ord + Clone> {
    /// The selected rows; session-local multiple selection when `None`.
    pub selection: Option<Selection<Id>>,
    /// The cell cursor, for a caller that observes or moves it. Its selected
    /// rows follow `selection`.
    pub cursor: Option<Binding<DataTableSelection<Id>>>,
    pub column_widths: Option<Binding<BTreeMap<String, f32>>>,
    pub column_visibility: Option<Binding<HashMap<String, bool>>>,
    /// Display order by column id; header drags and Alt+Left/Right write it.
    pub column_order: Option<Binding<Vec<String>>>,
    pub layout: TableLayout,
    pub frozen_column_count: usize,
    pub navigation: DataTableNavigation<Id>,
    pub is_enabled: bool,
}

impl<Id: Ord + Clone> Default for DataTableOptions<Id> {
    fn default() -> Self {
        Self {
            selection: None,
            cursor: None,
            column_widths: None,
            column_visibility: None,
            column_order: None,
            layout: TableLayout::default(),
            frozen_column_count: 1,
            navigation: DataTableNavigation::default(),
            is_enabled: true,
        }
    }
}

impl<Id: Ord + Clone> DataTableOptions<Id> {
    pub fn new() -> Self {
        Self::default()
    }
}

/// The mounted row and column window.
///
/// A pure function of the offset, viewport and metrics, so it needs no state
/// of its own: two builds over the same scroll position mount the same cells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DataTableWindows {
    pub row_start: usize,
    pub row_count: usize,
    pub column_start: usize,
    pub column_count: usize,
}

impl DataTableWindows {
    pub fn rows(&self, count: usize) -> std::ops::Range<usize> {
        let start = self.row_start.min(count);
        start..(start + self.row_count).min(count)
    }

    pub fn columns(&self, count: usize) -> std::ops::Range<usize> {
        let start = self.column_start.min(count);
        start..(start + self.column_count).min(count)
    }

    /// Rows intersecting the viewport, plus `overscan` on each side. `slack`
    /// trades rebuild frequency for a slightly larger pane.
    pub fn resolve(
        metrics: &DataTableRowMetrics,
        geometry: &DataTableColumnGeometry,
        offset: Point,
        moving_width: f32,
        viewport_height: f32,
        slack: usize,
    ) -> Self {
        let rows = metrics.visible_range(offset.y, viewport_height, slack);
        let columns = geometry.visible_columns(offset.x, moving_width.max(1.0));
        Self {
            row_start: rows.start,
            row_count: rows.len(),
            column_start: columns.start,
            column_count: columns.len(),
        }
    }
}

/// Session values that survive recomposition: the fallbacks for selection,
/// widths and visibility, plus the draft and the last applied navigation.
#[derive(Clone)]
pub struct DataTableSession<Id: Ord + Clone> {
    pub selection: DataTableSelection<Id>,
    pub widths: BTreeMap<String, f32>,
    pub visibility: HashMap<String, bool>,
    pub order: Vec<String>,
    pub editing: CellEditing<Id>,
    pub last_navigation: Option<u64>,
}

impl<Id: Ord + Clone> Default for DataTableSession<Id> {
    fn default() -> Self {
        Self {
            selection: DataTableSelection::default(),
            widths: BTreeMap::new(),
            visibility: HashMap::new(),
            order: Vec::new(),
            editing: CellEditing::new(),
            last_navigation: None,
        }
    }
}

/// A persistent row model, fixed header, frozen columns and two-axis window.
pub struct DataTable<
    Record: Clone + 'static,
    Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static,
> {
    pub model: Rc<DataTableModel<Record, Id>>,
    columns: Vec<TableColumn<Record>>,
    pub options: DataTableOptions<Id>,
    pub on_activate: Option<Rc<dyn Fn(&Record)>>,
    /// Per-record logical height, scaled with text. `None` keeps every row at
    /// `options.layout.row_height`, which also remains the minimum.
    pub row_height: Option<Rc<dyn Fn(&Record) -> f32>>,
    /// Hierarchy keys (`TreeTable`) get the first chance at row navigation.
    pub row_key_handler: Option<Rc<dyn Fn(&KeyEvent, &Id) -> bool>>,
    session: State<DataTableSession<Id>>,
    offset: State<Point>,
    /// Rows from `from_records`, poured into the model the view retains.
    records: Option<Rc<Vec<Record>>>,
}

impl<Record: Clone + 'static, Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static>
    DataTable<Record, Id>
{
    /// A table over plain records. The view keeps its own model, so sorting
    /// survives the parent passing a new array.
    ///
    /// Column and row IDs must be unique.
    pub fn from_records(
        records: Vec<Record>,
        row_id: impl Fn(&Record) -> Id + 'static,
        columns: Vec<TableColumn<Record>>,
    ) -> Result<Self, DuplicateId> {
        let model = DataTableModel::try_new(records.clone(), row_id)?;
        let mut table = Self::try_new(model, columns)?;
        table.records = Some(Rc::new(records));
        Ok(table)
    }

    pub fn try_new(
        model: Rc<DataTableModel<Record, Id>>,
        columns: Vec<TableColumn<Record>>,
    ) -> Result<Self, DuplicateId> {
        check_ids(columns.iter().map(|item| &item.id), "Data table column")?;
        let mut options = DataTableOptions::default();
        options.layout.validate();
        Ok(Self {
            model,
            columns,
            options,
            on_activate: None,
            row_height: None,
            row_key_handler: None,
            session: State::new(DataTableSession::default()),
            offset: State::new(Point::default()),
            records: None,
        })
    }

    pub fn options(mut self, options: DataTableOptions<Id>) -> Self {
        self.options = options;
        self.options.layout.validate();
        self
    }

    pub fn on_activate(mut self, handler: impl Fn(&Record) + 'static) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }

    pub fn row_height(mut self, resolver: impl Fn(&Record) -> f32 + 'static) -> Self {
        self.row_height = Some(Rc::new(resolver));
        self
    }

    fn controller(&self) -> DataTableController<Record, Id> {
        DataTableController {
            model: self.model.clone(),
            columns: Rc::new(self.columns.clone()),
            session: self.session.clone(),
            offset: self.offset.clone(),
            on_activate: self.on_activate.clone(),
            row_height: self.row_height.clone(),
            row_key_handler: self.row_key_handler.clone(),
            layout: self.options.layout.clone(),
            frozen_column_count: self.options.frozen_column_count,
            is_enabled: self.options.is_enabled,
            selection: self.options.selection.clone(),
            cursor: self.options.cursor.clone(),
            row_hover: Rc::default(),
            column_widths: self.options.column_widths.clone(),
            column_visibility: self.options.column_visibility.clone(),
            column_order: self.options.column_order.clone(),
            navigation: self.options.navigation.clone(),
        }
    }
}

impl<Record: Clone + 'static, Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static> View
    for DataTable<Record, Id>
{
    fn body(&self, cx: &Scope) -> AnyView {
        let mut controller = self.controller();
        controller.row_hover = cx.remember(Rc::<DataTableRowHover>::default);
        if let Some(records) = &self.records {
            let retained = cx.remember(|| self.model.clone());
            let poured = cx.remember(|| Rc::new(RefCell::new(None::<Rc<Vec<Record>>>)));
            let is_new = !poured
                .borrow()
                .as_ref()
                .is_some_and(|last| Rc::ptr_eq(last, records));
            if is_new && !Rc::ptr_eq(&retained, &self.model) {
                // `from_records` validated these IDs against the same rule.
                let _ = retained.replace_rows(records.as_ref().clone());
            }
            *poured.borrow_mut() = Some(records.clone());
            controller.model = retained;
        }
        // Sorting and row edits publish through the model.
        let revision = cx.state(|| 0_u64);
        let _ = revision.get();
        let subscription =
            cx.remember(|| Rc::new(RefCell::new(None::<ModelSubscription<Record, Id>>)));
        let is_subscribed = subscription
            .borrow()
            .as_ref()
            .is_some_and(|current| Rc::ptr_eq(&current.model, &controller.model));
        if !is_subscribed {
            *subscription.borrow_mut() = Some(ModelSubscription::new(
                controller.model.clone(),
                Rc::new(move || revision.set(revision.get_untracked().wrapping_add(1))),
            ));
        }
        let probe = controller.clone();
        AnyView::new(crate::adaptive::ContainerSizeReader::new(
            move |size: Size| DataTableSurface {
                controller: probe.clone(),
                size,
            },
        ))
    }

    fn dynamic_properties(&self) -> Vec<crate::DynamicField> {
        vec![
            crate::DynamicField::state("session", &self.session),
            crate::DynamicField::state("offset", &self.offset),
        ]
    }
}

/// Holds the table's observer on a model for as long as the view is mounted.
struct ModelSubscription<Record: 'static, Id: Eq + std::hash::Hash + Clone + 'static> {
    model: Rc<DataTableModel<Record, Id>>,
    token: u64,
}

impl<Record: 'static, Id: Eq + std::hash::Hash + Clone + 'static> ModelSubscription<Record, Id> {
    fn new(model: Rc<DataTableModel<Record, Id>>, handler: Rc<dyn Fn()>) -> Self {
        let token = model.register_observer(handler);
        Self { model, token }
    }
}

impl<Record: 'static, Id: Eq + std::hash::Hash + Clone + 'static> Drop
    for ModelSubscription<Record, Id>
{
    fn drop(&mut self) {
        self.model.unregister_observer(self.token);
    }
}

/// State and helpers shared by the header, the lanes and the keyboard host.
pub struct DataTableController<
    Record: 'static,
    Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static,
> {
    pub model: Rc<DataTableModel<Record, Id>>,
    pub columns: Rc<Vec<TableColumn<Record>>>,
    pub session: State<DataTableSession<Id>>,
    pub offset: State<Point>,
    pub on_activate: Option<Rc<dyn Fn(&Record)>>,
    pub row_height: Option<Rc<dyn Fn(&Record) -> f32>>,
    pub row_key_handler: Option<Rc<dyn Fn(&KeyEvent, &Id) -> bool>>,
    pub layout: TableLayout,
    pub frozen_column_count: usize,
    pub is_enabled: bool,
    pub selection: Option<Selection<Id>>,
    pub cursor: Option<Binding<DataTableSelection<Id>>>,
    row_hover: Rc<DataTableRowHover>,
    pub column_widths: Option<Binding<BTreeMap<String, f32>>>,
    pub column_visibility: Option<Binding<HashMap<String, bool>>>,
    pub column_order: Option<Binding<Vec<String>>>,
    pub navigation: DataTableNavigation<Id>,
}

impl<Record: Clone + 'static, Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static> Clone
    for DataTableController<Record, Id>
{
    fn clone(&self) -> Self {
        Self {
            model: self.model.clone(),
            columns: self.columns.clone(),
            session: self.session.clone(),
            offset: self.offset.clone(),
            on_activate: self.on_activate.clone(),
            row_height: self.row_height.clone(),
            row_key_handler: self.row_key_handler.clone(),
            layout: self.layout.clone(),
            frozen_column_count: self.frozen_column_count,
            is_enabled: self.is_enabled,
            selection: self.selection.clone(),
            cursor: self.cursor.clone(),
            row_hover: self.row_hover.clone(),
            column_widths: self.column_widths.clone(),
            column_visibility: self.column_visibility.clone(),
            column_order: self.column_order.clone(),
            navigation: self.navigation.clone(),
        }
    }
}

impl<Record: Clone + 'static, Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static>
    DataTableController<Record, Id>
{
    fn selection_mode(&self) -> DataTableSelectionMode {
        match self.selection.as_ref() {
            Some(Selection::Single(_)) => DataTableSelectionMode::Single,
            _ => DataTableSelectionMode::Multiple,
        }
    }

    /// The cursor from whichever store owns it, with the selected rows from the
    /// caller's selection.
    fn selection(&self, session: &DataTableSession<Id>) -> DataTableSelection<Id> {
        let mut state = match self.cursor.as_ref() {
            Some(binding) => binding.get(),
            None => session.selection.clone(),
        };
        if let Some(selection) = self.selection.as_ref() {
            state.selected_ids = selection.ids();
        }
        state
    }

    fn write_selection(&self, next: DataTableSelection<Id>) {
        if let Some(selection) = self.selection.as_ref() {
            selection.replace(next.selected_ids.iter().cloned());
        }
        match self.cursor.as_ref() {
            Some(binding) => binding.set(next),
            None => {
                let mut session = self.session.get();
                session.selection = next;
                self.session.set(session);
            }
        }
    }

    fn visibility(&self, session: &DataTableSession<Id>) -> HashMap<String, bool> {
        match self.column_visibility.as_ref() {
            Some(binding) => binding.get(),
            None => session.visibility.clone(),
        }
    }

    fn write_visibility(&self, id: &str, shown: bool) {
        match self.column_visibility.as_ref() {
            Some(binding) => {
                let mut next = binding.get();
                next.insert(id.to_string(), shown);
                binding.set(next);
            }
            None => {
                let mut session = self.session.get();
                session.visibility.insert(id.to_string(), shown);
                self.session.set(session);
            }
        }
    }

    /// The override map as a binding: the caller's when one is supplied,
    /// otherwise this mount's session, so an unbound choice resets on unmount.
    fn visibility_binding(&self) -> Binding<HashMap<String, bool>> {
        match self.column_visibility.as_ref() {
            Some(binding) => binding.clone(),
            None => {
                let session = self.session.clone();
                let read = session.clone();
                Binding::new(
                    move || read.get().visibility,
                    move |value| {
                        let mut next = session.get();
                        next.visibility = value;
                        session.set(next);
                    },
                )
            }
        }
    }

    fn stored_widths(&self) -> BTreeMap<String, f32> {
        match self.column_widths.as_ref() {
            Some(binding) => binding.get(),
            None => self.session.get().widths,
        }
    }

    fn width_of(&self, column: &TableColumn<Record>, stored: &BTreeMap<String, f32>) -> f32 {
        column.layout.constrained(
            stored
                .get(&column.id)
                .copied()
                .unwrap_or(column.layout.width),
        )
    }

    fn write_width(&self, column: &TableColumn<Record>, width: f32) {
        let constrained = column.layout.constrained(width);
        match self.column_widths.as_ref() {
            Some(binding) => {
                let mut next = binding.get();
                next.insert(column.id.clone(), constrained);
                binding.set(next);
            }
            None => {
                let mut session = self.session.get();
                session.widths.insert(column.id.clone(), constrained);
                self.session.set(session);
            }
        }
    }

    fn display_order(&self, session: &DataTableSession<Id>) -> Vec<String> {
        let authored: Vec<String> = self
            .columns
            .iter()
            .map(|column| column.id.clone())
            .collect();
        match self.column_order.as_ref() {
            Some(binding) => DataTableColumnOrder::apply(&binding.get(), &authored),
            None => DataTableColumnOrder::apply(&session.order, &authored),
        }
    }

    fn ordered_columns(&self, session: &DataTableSession<Id>) -> Vec<TableColumn<Record>> {
        let mut by_id: HashMap<String, TableColumn<Record>> = self
            .columns
            .iter()
            .map(|column| (column.id.clone(), column.clone()))
            .collect();
        self.display_order(session)
            .into_iter()
            .filter_map(|id| by_id.remove(&id))
            .collect()
    }

    /// Presented columns, their geometry and the frozen prefix for one width.
    fn plan(&self, session: &DataTableSession<Id>, available_width: f32) -> DataTablePlan<Record> {
        let ordered = self.ordered_columns(session);
        let stored = self.stored_widths();
        let widths: BTreeMap<String, f32> = ordered
            .iter()
            .map(|column| (column.id.clone(), self.width_of(column, &stored)))
            .collect();
        let overrides = self.visibility(session);
        // An open editor's column stays mounted even when it would not fit.
        let pinned: HashSet<String> = session
            .editing
            .draft
            .as_ref()
            .map(|draft| HashSet::from([draft.column_id.clone()]))
            .unwrap_or_default();
        let resolved = TableColumnPlan::resolve(
            &ordered,
            &widths,
            &overrides,
            available_width.max(0.0),
            self.frozen_column_count,
            &pinned,
        );
        let columns: Vec<TableColumn<Record>> = resolved
            .indices
            .iter()
            .filter_map(|index| ordered.get(*index).cloned())
            .collect();
        let mut presented: Vec<f32> = columns
            .iter()
            .map(|column| self.width_of(column, &stored))
            .collect();
        // `fills_width` only applies when nothing had to be dropped for space:
        // stretching the last column over slack that hidden columns would have
        // used would misreport the table's real width.
        if self.layout.fills_width
            && !presented.is_empty()
            && resolved.frozen_count < presented.len()
        {
            let visible: f32 = ordered
                .iter()
                .filter(|column| overrides.get(&column.id) != Some(&false))
                .map(|column| widths.get(&column.id).copied().unwrap_or(column.layout.width))
                .sum();
            let slack = available_width - visible;
            if slack > 0.5 {
                let target = columns
                    .iter()
                    .rposition(|column| column.layout.is_flex)
                    .unwrap_or(columns.len() - 1);
                if let Some(column) = columns.get(target) {
                    presented[target] = column.layout.constrained(presented[target] + slack);
                }
            }
        }
        let geometry = DataTableColumnGeometry::new(presented, resolved.frozen_count);
        DataTablePlan { columns, geometry }
    }

    /// Row geometry, rebuilt only when the revision, count, minimum height or
    /// text scale changes.
    fn metrics(&self, node: &Node) -> DataTableRowMetrics {
        let count = self.model.count();
        let surface = node.composition_value(&crate::adaptive::SurfaceEnvironment::key());
        let minimum = self
            .layout
            .resolved(
                node_text_scaler(node).factor(),
                surface.input.contains(cove_core::InputCapabilities::TOUCH),
                surface.minimum_control_extent(),
            )
            .row_height;
        let key = RowHeightKey {
            revision: self.model.revision(),
            count,
            minimum,
            scale: node_text_scaler(node).factor(),
        };
        if let Some(cached) = node
            .attachment::<Rc<RefCell<Option<(RowHeightKey, DataTableRowMetrics)>>>>(METRICS_CACHE)
            && let Some((cached_key, metrics)) = cached.borrow().as_ref()
            && row_height_key_matches(Some(*cached_key), key)
        {
            return metrics.clone();
        }
        let metrics = match self.row_height.as_ref() {
            Some(resolver) => {
                let heights = variable_row_heights(count, minimum, key.scale, |row| {
                    self.model
                        .record_at(row)
                        .map(|record| resolver(&record))
                        .unwrap_or(minimum)
                });
                let ids: Vec<usize> = (0..count).collect();
                let mut index = crate::primitives::layout::virtualization::VariableRowIndex::new();
                index.replace(&ids, &heights, 0.0);
                DataTableRowMetrics::variable(count, minimum, index)
            }
            None => DataTableRowMetrics::fixed(count, minimum),
        };
        let cache = node
            .attachment::<Rc<RefCell<Option<(RowHeightKey, DataTableRowMetrics)>>>>(METRICS_CACHE)
            .unwrap_or_else(|| {
                let cache = Rc::new(RefCell::new(None));
                node.set_attachment(METRICS_CACHE, cache.clone());
                cache
            });
        *cache.borrow_mut() = Some((key, metrics.clone()));
        metrics
    }

    /// Clamps an offset to the scrollable extent. Frozen columns do not scroll,
    /// so the horizontal maximum uses the moving span.
    fn clamped_offset(
        &self,
        next: Point,
        plan: &DataTablePlan<Record>,
        metrics: &DataTableRowMetrics,
        moving_width: f32,
        viewport_height: f32,
    ) -> Point {
        let maximum_x = (plan.geometry.moving_width() - moving_width).max(0.0);
        let maximum_y = (metrics.total() - viewport_height).max(0.0);
        Point::new(next.x.clamp(0.0, maximum_x), next.y.clamp(0.0, maximum_y))
    }

    fn reveal(
        &self,
        row: usize,
        column: Option<usize>,
        plan: &DataTablePlan<Record>,
        metrics: &DataTableRowMetrics,
        moving_width: f32,
        viewport_height: f32,
    ) {
        let mut offset = self.offset.get();
        let top = metrics.top(row);
        let bottom = top + metrics.height(row);
        let visible_height = metrics.height(row).max(viewport_height);
        if top < offset.y {
            offset.y = top;
        } else if bottom > offset.y + visible_height {
            offset.y = bottom - visible_height;
        }
        if let Some(column) = column
            && column >= plan.geometry.frozen_count
            && column < plan.geometry.widths.len()
        {
            let left = plan.geometry.starts()[column] - plan.geometry.frozen_width();
            let right = left + plan.geometry.widths[column];
            let visible_width = plan.geometry.widths[column].max(moving_width);
            if left < offset.x {
                offset.x = left;
            } else if right > offset.x + visible_width {
                offset.x = right - visible_width;
            }
        }
        self.offset
            .set(self.clamped_offset(offset, plan, metrics, moving_width, viewport_height));
    }

    fn begin_editing(&self, row: usize, column: usize, plan: &DataTablePlan<Record>) {
        if !self.is_enabled {
            return;
        }
        let Some(target) = plan.columns.get(column) else {
            return;
        };
        let Some(editing) = target.text_editing.clone() else {
            return;
        };
        let Some(row_id) = self.model.row_id_at(row) else {
            return;
        };
        let Some(record) = self.model.record_at(row) else {
            return;
        };
        let mut session = self.session.get();
        if session.editing.is_editing(&row_id, &target.id) {
            return;
        }
        session
            .editing
            .begin(row_id, target.id.clone(), editing.text_of(&record));
        self.session.set(session);
    }

    fn open_draft(
        &self,
        session: &DataTableSession<Id>,
        row_id: &Id,
        column_id: &str,
    ) -> Option<String> {
        let draft = session.editing.draft.as_ref()?;
        draft
            .is_for(row_id, column_id)
            .then(|| draft.buffer.clone())
    }

    fn set_draft_buffer(&self, value: String) {
        let mut session = self.session.get();
        session.editing.set_buffer(value);
        self.session.set(session);
    }

    fn cancel_editing(&self, input: &Rc<RefCell<Option<Node>>>) {
        let mut session = self.session.get();
        session.editing.cancel();
        self.session.set(session);
        focus_node(input);
    }

    /// Commits a draft the person left by clicking elsewhere. Focus already
    /// moved to whatever they clicked, so this path leaves it alone.
    fn commit_editing_without_focus(&self, input: &Rc<RefCell<Option<Node>>>) -> bool {
        let restore = false;
        self.commit_editing_inner(input, restore)
    }

    /// Validates and closes the open draft. Returns whether it closed.
    fn commit_editing(&self, input: &Rc<RefCell<Option<Node>>>) -> bool {
        self.commit_editing_inner(input, true)
    }

    fn commit_editing_inner(&self, input: &Rc<RefCell<Option<Node>>>, restore_focus: bool) -> bool {
        let mut session = self.session.get();
        let Some(draft) = session.editing.draft.clone() else {
            return true;
        };
        let Some(column) = self
            .ordered_columns(&session)
            .into_iter()
            .find(|column| column.id == draft.column_id)
        else {
            session.editing.cancel();
            self.session.set(session);
            return true;
        };
        let Some(editing) = column.text_editing.clone() else {
            session.editing.cancel();
            self.session.set(session);
            return true;
        };
        if let Some(message) = editing.rejection(&draft.buffer) {
            session.editing.reject(message);
            self.session.set(session);
            return false;
        }
        if self.model.index_for(&draft.row_id).is_none() {
            session.editing.cancel();
            self.session.set(session);
            return true;
        }
        let row_id = draft.row_id.clone();
        let original = draft.original.clone();
        let value = draft.buffer.clone();
        session.editing.cancel();
        self.session.set(session);
        if value != original {
            self.model
                .update_row(&row_id, |record| editing.apply(record, &value));
        }
        if restore_focus {
            focus_node(input);
        }
        true
    }

    /// Tab moves to the next editable column, skipping the rest.
    fn move_editor(
        &self,
        direction: i32,
        plan: &DataTablePlan<Record>,
        input: &Rc<RefCell<Option<Node>>>,
    ) {
        if !self.commit_editing(input) {
            return;
        }
        let session = self.session.get();
        let Some(draft) = session.editing.draft.as_ref() else {
            return;
        };
        let Some(row) = self.model.index_for(&draft.row_id) else {
            return;
        };
        let editable: Vec<EditableColumn> = self
            .ordered_columns(&session)
            .iter()
            .map(|column| EditableColumn {
                id: column.id.clone(),
                is_editable: column.text_editing.is_some(),
            })
            .collect();
        let Some(next_id) =
            DataTableEditingState::next_editable(&draft.column_id, direction, &editable)
        else {
            return;
        };
        let Some(next_column) = plan.columns.iter().position(|column| column.id == next_id) else {
            return;
        };
        self.begin_editing(row, next_column, plan);
    }

    fn cycle_sort(&self, column_id: &str) {
        let session = self.session.get();
        let Some(column) = self
            .ordered_columns(&session)
            .into_iter()
            .find(|column| column.id == column_id)
        else {
            return;
        };
        let Some(compare) = column.compare.clone() else {
            return;
        };
        let next = match self.model.sort() {
            Some(sort)
                if sort.column_id == column_id
                    && sort.direction == TableSortDirection::Ascending =>
            {
                Some(TableSort {
                    column_id: column_id.to_string(),
                    direction: TableSortDirection::Descending,
                })
            }
            Some(sort) if sort.column_id == column_id => None,
            _ => Some(TableSort::new(column_id.to_string())),
        };
        self.model.set_sort(next, Some(compare));
    }

    fn move_column(&self, plan: &DataTablePlan<Record>, index: usize, step: i32) -> bool {
        if !self.is_enabled {
            return false;
        }
        let target = index as i64 + step as i64;
        if !(0..plan.columns.len() as i64).contains(&target) {
            return false;
        }
        self.move_column_to(plan, index, target as usize);
        true
    }

    /// Writes `index` next to `target` in the display order. A column stays in
    /// its own lane, frozen or moving.
    fn move_column_to(&self, plan: &DataTablePlan<Record>, index: usize, target: usize) {
        let frozen = plan.geometry.frozen_count;
        if index == target || (index < frozen) != (target < frozen) {
            return;
        }
        let (Some(source), Some(destination)) = (plan.columns.get(index), plan.columns.get(target))
        else {
            return;
        };
        let session = self.session.get();
        let order = self.display_order(&session);
        let next = DataTableColumnOrder::move_id(&source.id, &destination.id, &order);
        match self.column_order.as_ref() {
            Some(binding) => binding.set(next),
            None => {
                let mut session = session;
                session.order = next;
                self.session.set(session);
            }
        }
    }

    /// Keyboard grid navigation. Returns whether the event was consumed.
    fn handle_key(
        &self,
        event: &KeyEvent,
        plan: &DataTablePlan<Record>,
        metrics: &DataTableRowMetrics,
        moving_width: f32,
        viewport_height: f32,
        _input: &Rc<RefCell<Option<Node>>>,
    ) -> bool {
        if !self.is_enabled || self.model.count() == 0 || plan.columns.is_empty() {
            return false;
        }
        let session = self.session.get();
        let mut selection = self.selection(&session);
        let current = selection
            .focused_id
            .as_ref()
            .and_then(|id| self.model.index_for(id));
        let column = selection
            .focused_column_id
            .as_ref()
            .and_then(|id| plan.columns.iter().position(|column| column.id == *id))
            .unwrap_or(0);

        if column == 0
            && let Some(current) = current
            && let Some(row_id) = self.model.row_id_at(current)
            && let Some(handler) = self.row_key_handler.clone()
            && handler(event, &row_id)
        {
            return true;
        }
        if event.modifiers.has_alt()
            && (event.scancode == crate::Scancode::ARROW_LEFT
                || event.scancode == crate::Scancode::ARROW_RIGHT)
        {
            let step = if event.scancode == crate::Scancode::ARROW_LEFT {
                -1
            } else {
                1
            };
            return self.move_column(plan, column, step);
        }
        if event.scancode == crate::Scancode::A
            && (event.modifiers.has_gui() || event.modifiers.has_ctrl())
            && self.selection_mode() == DataTableSelectionMode::Multiple
        {
            selection.select_all(&self.model);
            self.write_selection(selection);
            return true;
        }

        let (row, next_column) = match event.scancode {
            crate::Scancode::ARROW_DOWN => (
                (current.map_or(0, |index| index + 1)).min(self.model.count() - 1),
                column,
            ),
            crate::Scancode::ARROW_UP => (
                current.map_or(self.model.count() - 1, |index| index.saturating_sub(1)),
                column,
            ),
            crate::Scancode::HOME => (0, column),
            crate::Scancode::END => (self.model.count() - 1, column),
            crate::Scancode::PAGE_UP => {
                let from = current.unwrap_or(0);
                let target = metrics.row_at(metrics.top(from) - viewport_height);
                (target.min(from.saturating_sub(1)), column)
            }
            crate::Scancode::PAGE_DOWN => {
                let from = current.unwrap_or(0);
                let target = metrics.row_at(metrics.top(from) + viewport_height);
                (target.max((from + 1).min(self.model.count() - 1)), column)
            }
            crate::Scancode::ARROW_LEFT => (current.unwrap_or(0), column.saturating_sub(1)),
            crate::Scancode::ARROW_RIGHT => (
                current.unwrap_or(0),
                (column + 1).min(plan.columns.len() - 1),
            ),
            crate::Scancode::F2 => {
                let Some(current) = current else { return false };
                self.begin_editing(current, column, plan);
                return true;
            }
            crate::Scancode::RETURN | crate::Scancode::KEYPAD_ENTER => {
                let Some(current) = current else { return false };
                if plan.columns[column].text_editing.is_some() {
                    self.begin_editing(current, column, plan);
                } else if let Some(handler) = self.on_activate.as_ref()
                    && let Some(record) = self.model.record_at(current)
                {
                    handler(&record);
                }
                return true;
            }
            crate::Scancode::ESCAPE => {
                selection.clear();
                self.write_selection(selection);
                return true;
            }
            _ => return false,
        };

        selection.select(row, &self.model, self.selection_mode(), event.modifiers);
        selection.focused_column_id = Some(plan.columns[next_column].id.clone());
        self.write_selection(selection);
        self.reveal(
            row,
            Some(next_column),
            plan,
            metrics,
            moving_width,
            viewport_height,
        );
        true
    }

    /// Applies a pending navigation request once per request identity.
    fn apply_navigation(
        &self,
        plan: &DataTablePlan<Record>,
        metrics: &DataTableRowMetrics,
        moving_width: f32,
        viewport_height: f32,
    ) {
        let Some(request_id) = self.navigation.request_id else {
            return;
        };
        if self.session.get().last_navigation == Some(request_id) {
            return;
        }
        let Some(row_id) = self.navigation.row_id.clone() else {
            return;
        };
        let Some(row) = self.model.index_for(&row_id) else {
            return;
        };
        let session = self.session.get();
        let column = self
            .navigation
            .column_id
            .as_ref()
            .and_then(|id| plan.columns.iter().position(|column| column.id == *id));
        // A requested cell in a hidden column becomes visible.
        if let Some(column_id) = self.navigation.column_id.as_ref()
            && let Some(column) = self
                .ordered_columns(&session)
                .into_iter()
                .find(|column| column.id == *column_id)
            && column.visibility != ColumnVisibility::Required
            && self.visibility(&session).get(column_id) != Some(&true)
        {
            self.write_visibility(column_id, true);
        }
        let mut session = self.session.get();
        session.last_navigation = Some(request_id);
        self.session.set(session);
        let plan = self.plan(
            &self.session.get(),
            plan_available_width(plan, moving_width),
        );
        self.reveal(row, column, &plan, metrics, moving_width, viewport_height);
    }

    fn select_row(&self, row: usize) {
        let session = self.session.get();
        let mut selection = self.selection(&session);
        selection.select(
            row,
            &self.model,
            self.selection_mode(),
            KeyModifiers::default(),
        );
        self.write_selection(selection);
        self.activate(row);
    }

    fn select(
        &self,
        row: usize,
        column: usize,
        plan: &DataTablePlan<Record>,
        modifiers: KeyModifiers,
    ) {
        let session = self.session.get();
        let mut selection = self.selection(&session);
        selection.select(row, &self.model, self.selection_mode(), modifiers);
        if let Some(target) = plan.columns.get(column) {
            selection.focused_column_id = Some(target.id.clone());
        }
        self.write_selection(selection);
    }

    fn activate(&self, row: usize) {
        if let Some(handler) = self.on_activate.as_ref()
            && let Some(record) = self.model.record_at(row)
        {
            handler(&record);
        }
    }
}

/// The table's own tracks re-expressed through the shared columns contract, so
/// the picker and the planner read one visibility type.
fn plan_available_width<Record>(plan: &DataTablePlan<Record>, moving_width: f32) -> f32 {
    plan.geometry.frozen_width() + moving_width
}

fn focus_node(input: &Rc<RefCell<Option<Node>>>) {
    if let Some(node) = input.borrow().clone()
        && let Some(chain) = FocusChainHolder::current()
    {
        chain.focus(Some(&node), false);
    }
}

/// Presented columns plus their geometry.
pub struct DataTablePlan<Record> {
    pub columns: Vec<TableColumn<Record>>,
    pub geometry: DataTableColumnGeometry,
}

impl<Record> Clone for DataTablePlan<Record> {
    fn clone(&self) -> Self {
        Self {
            columns: self.columns.clone(),
            geometry: self.geometry.clone(),
        }
    }
}

/// Height of the card's toolbar row (the column picker lives here), in
/// unscaled points.
const TOOLBAR_HEIGHT: f32 = 44.0;

/// The measured surface: turns a size into a plan and a window.
struct DataTableSurface<Record: 'static, Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static>
{
    controller: DataTableController<Record, Id>,
    size: Size,
}

impl<Record: Clone + 'static, Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static> View
    for DataTableSurface<Record, Id>
{
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

impl<Record: Clone + 'static, Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static>
    PrimitiveView for DataTableSurface<Record, Id>
{
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let controller = &self.controller;
        let scale = node_text_scaler(node).factor();
        let surface = node.composition_value(&crate::adaptive::SurfaceEnvironment::key());
        let layout = controller.layout.clone().resolved(
            scale,
            surface.input.contains(cove_core::InputCapabilities::TOUCH),
            surface.minimum_control_extent(),
        );
        let session = controller.session.get();
        // The card sits under the column picker, so the lane viewport is the
        // card's interior: the whole measured surface minus the picker row and
        // the chrome. Ignoring the picker here makes the lanes taller than the
        // card and rows paint past its bottom edge.
        let has_picker = controller
            .ordered_columns(&session)
            .iter()
            .any(|column| column.visibility != ColumnVisibility::Required);
        let toolbar_height = if has_picker {
            TOOLBAR_HEIGHT * scale
        } else {
            0.0
        };
        // The toolbar and the header are both chrome inside the card, so the
        // lane viewport is the card's interior.
        let vertical_chrome = layout.header_height + 1.0 + toolbar_height;
        let viewport_height = (self.size.height - vertical_chrome).max(1.0);
        let plan = controller.plan(&session, self.size.width);
        let frozen_width = plan.geometry.frozen_width();
        let moving_width = (self.size.width - frozen_width).max(1.0);
        let metrics = controller.metrics(node);
        controller.apply_navigation(&plan, &metrics, moving_width, viewport_height);
        let offset = controller.offset.get();
        let window = DataTableWindows::resolve(
            &metrics,
            &plan.geometry,
            offset,
            moving_width,
            viewport_height,
            3,
        );
        let session = controller.session.get();
        // The picker reads the same session, before the lanes take ownership.
        let picker_session = controller.session.get();

        let input: Rc<RefCell<Option<Node>>> = Rc::new(RefCell::new(None));
        // The header stays mounted for an empty dataset, so the sort controls
        // and column titles remain reachable; only the body is replaced.
        let content = AnyView::new(DataTableLanes {
            controller: controller.clone(),
            plan: plan.clone(),
            metrics: metrics.clone(),
            window,
            offset,
            moving_width,
            viewport_height,
            layout,
            session,
            input: input.clone(),
        });

        // The picker sits above the table and shares the plan the body uses.
        let picker = if has_picker {
            let ordered = controller.ordered_columns(&picker_session);
            let stored = controller.stored_widths();
            let widths: BTreeMap<String, f32> = ordered
                .iter()
                .map(|column| (column.id.clone(), controller.width_of(column, &stored)))
                .collect();
            let overrides = controller.visibility(&picker_session);
            let resolved = TableColumnPlan::resolve(
                &ordered,
                &widths,
                &overrides,
                self.size.width,
                controller.frozen_column_count,
                &Default::default(),
            );
            let tracks = TableColumnPlan::tracks(&ordered, &widths);
            Some(AnyView::new(
                AnyView::new(crate::primitives::columns::ColumnVisibilityPicker::new(
                    crate::primitives::data::columns::picker_tracks(&tracks),
                    crate::primitives::columns::ColumnVisibilityPlan {
                        indices: resolved.indices.clone(),
                        required_ids: resolved.required_ids.clone(),
                    },
                    controller.visibility_binding(),
                    controller.is_enabled,
                ))
                .flex_with(0.0, 0.0, None),
            ))
        } else {
            None
        };
        let mut bands: Vec<AnyView> = Vec::new();
        if let Some(picker) = picker {
            // The picker is a toolbar action, not a caption above the table:
            // it sits on the card's first row, hard against the trailing edge,
            // and the rule below separates it from the column header.
            bands.push(AnyView::new(
                AnyView::new(
                    BoxView::new(
                        cove_core::scene::FlexDirection::Row,
                        cove_core::scene::Align::Center,
                        cove_core::scene::Justify::SpaceBetween,
                        cove_core::scene::Wrap::NoWrap,
                        8.0,
                        AnyView::new(TupleView::new(vec![
                            crate::primitives::data::lists::table::toolbar_count(
                                controller.model.count(),
                            ),
                            picker,
                        ])),
                    )
                    .padding_edges(20.0, 0.0)
                    .frame_height(TOOLBAR_HEIGHT * scale),
                )
                .flex_with(0.0, 0.0, None),
            ));
        }
        bands.push(content);
        vec![AnyView::new(DataTableChrome {
            content: AnyView::new(
                BoxView::new(
                    cove_core::scene::FlexDirection::Column,
                    cove_core::scene::Align::Stretch,
                    cove_core::scene::Justify::FlexStart,
                    cove_core::scene::Wrap::NoWrap,
                    0.0,
                    AnyView::new(TupleView::new(bands)),
                )
                .flex_with(1.0, 1.0, None),
            ),
            is_enabled: controller.is_enabled,
            header_height: self.size.height,
            model_rows: controller.model.count(),
            columns: controller.columns.len(),
        })]
    }
}

/// Outer border, corner radius and the table-level semantics.
struct DataTableChrome {
    content: AnyView,
    is_enabled: bool,
    header_height: f32,
    model_rows: usize,
    columns: usize,
}

impl PrimitiveView for DataTableChrome {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, node: &Node) {
        let theme = theme_for_node(node);
        // The card owns its fill, so the header and the rows paint the same
        // surface instead of leaning on whatever sits behind the table.
        node.set_background_color(Some(theme.colors.surface));
        node.set_border_color(Some(theme.colors.border));
        node.set_border_width(1.0);
        node.set_corner_radius(theme.radius.lg);
        // Rows paint edge to edge; without clipping their fills square off the
        // card's rounded corners.
        node.set_clips_to_bounds(true);
        node.set_opacity(if self.is_enabled { 1.0 } else { 0.55 });
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Table,
            label: crate::i18n::tr("Data table"),
            value: format!("{} rows, {} columns", self.model_rows, self.columns),
            state: AccessibilityState {
                is_enabled: self.is_enabled,
                ..Default::default()
            },
            ..Default::default()
        }));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let _ = self.header_height;
        vec![self.content.clone()]
    }
}
impl_primitive_view!(DataTableChrome);

/// Header plus the two body lanes, wired to one keyboard host.
struct DataTableLanes<Record: 'static, Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static> {
    controller: DataTableController<Record, Id>,
    plan: DataTablePlan<Record>,
    metrics: DataTableRowMetrics,
    window: DataTableWindows,
    offset: Point,
    moving_width: f32,
    viewport_height: f32,
    layout: TableLayout,
    session: DataTableSession<Id>,
    input: Rc<RefCell<Option<Node>>>,
}

impl<Record: Clone + 'static, Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static> View
    for DataTableLanes<Record, Id>
{
    fn body(&self, _cx: &Scope) -> AnyView {
        let header = build_header(
            &self.controller,
            &self.plan,
            &self.metrics,
            self.offset,
            &self.layout,
            &self.session,
        );
        let frozen = build_frozen_lane(
            &self.controller,
            &self.plan,
            &self.metrics,
            self.window,
            self.offset,
            &self.layout,
            &self.session,
            self.viewport_height,
            self.moving_width,
            self.input.clone(),
        );
        let moving = build_moving_lane(
            &self.controller,
            &self.plan,
            &self.metrics,
            self.window,
            self.offset,
            &self.layout,
            &self.session,
            self.moving_width,
            self.viewport_height,
            self.input.clone(),
        );

        let frozen_width = self.plan.geometry.frozen_width();
        let mut body_children: Vec<AnyView> = Vec::new();
        if self.plan.geometry.frozen_count > 0 {
            body_children.push(AnyView::new(
                AnyView::new(DataOffsetPane {
                    axis: OffsetAxis::Vertical,
                    offset: self.controller.offset.clone(),
                    content: frozen,
                })
                .frame(
                    Some(crate::modifiers::layout::FrameDimension::Points(
                        frozen_width,
                    )),
                    None,
                    None,
                    Some(0.0),
                    None,
                    None,
                )
                .flex_with(0.0, 0.0, None),
            ));
        }
        body_children.push(AnyView::new(AnyView::new(moving).flex_with(
            1.0,
            1.0,
            Some(0.0),
        )));

        let body = if self.controller.model.count() == 0 {
            AnyView::new(
                AnyView::new(EmptyState::new(
                    self.layout.empty_title.clone(),
                    "Change your filters or add a record.",
                ))
                .flex_with(1.0, 1.0, Some(0.0)),
            )
        } else {
            AnyView::new(
                BoxView::new(
                    FlexDirection::Row,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(TupleView::new(body_children)),
                )
                // The body is the lane viewport, not the lane extent: without a
                // pinned height the scroll views below grow to the full row
                // count and paint rows outside the card.
                .frame(
                    None,
                    Some(crate::modifiers::layout::FrameDimension::Points(
                        self.viewport_height,
                    )),
                    Some(0.0),
                    None,
                    None,
                    None,
                ),
            )
        };

        let column = AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(AnyView::new(header).frame(None, None, None, None, None, None)),
                    AnyView::new(Divider::horizontal()),
                    body,
                ])),
            )
            .flex_with(1.0, 1.0, Some(0.0))
            .frame(None, None, Some(0.0), Some(0.0), None, None),
        );

        AnyView::new(DataTableInputHost {
            controller: self.controller.clone(),
            plan: self.plan.clone(),
            metrics: self.metrics.clone(),
            moving_width: self.moving_width,
            viewport_height: self.viewport_height,
            input: self.input.clone(),
            content: column,
        })
    }
}

fn header_cells<
    Record: Clone + 'static,
    Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static,
>(
    controller: &DataTableController<Record, Id>,
    plan: &DataTablePlan<Record>,
    range: std::ops::Range<usize>,
    geometry: &DataTableColumnGeometry,
    offset: Point,
    layout: &TableLayout,
) -> AnyView {
    let mut cells: Vec<AnyView> = Vec::new();
    if range.start >= geometry.frozen_count && range.start < geometry.starts().len() {
        let gap = geometry.starts()[range.start] - geometry.frozen_width();
        cells.push(AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(crate::EmptyView),
            )
            .frame_width(gap.max(0.0))
            .flex_with(0.0, 0.0, None),
        ));
    }
    for index in range {
        let Some(column) = plan.columns.get(index) else {
            continue;
        };
        let width = geometry
            .widths
            .get(index)
            .copied()
            .unwrap_or(column.layout.width);
        cells.push(AnyView::new(HeaderCell {
            controller: controller.clone(),
            plan: plan.clone(),
            column: column.clone(),
            index,
            width,
            height: layout.header_height,
            is_enabled: controller.is_enabled,
            sorted: controller.model.sort(),
        }));
    }
    let _ = offset;
    AnyView::new(
        BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(TupleView::new(cells)),
        )
        .frame(None, None, None, Some(layout.header_height), None, None),
    )
}

fn build_header<
    Record: Clone + 'static,
    Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static,
>(
    controller: &DataTableController<Record, Id>,
    plan: &DataTablePlan<Record>,
    _metrics: &DataTableRowMetrics,
    offset: Point,
    layout: &TableLayout,
    _session: &DataTableSession<Id>,
) -> AnyView {
    let geometry = &plan.geometry;
    let frozen_count = geometry.frozen_count.min(plan.columns.len());
    let mut parts: Vec<AnyView> = Vec::new();
    if frozen_count > 0 {
        parts.push(AnyView::new(
            header_cells(controller, plan, 0..frozen_count, geometry, offset, layout)
                .frame_width(geometry.frozen_width())
                .flex_with(0.0, 0.0, None),
        ));
    }
    let moving = frozen_count..plan.columns.len();
    parts.push(AnyView::new(
        AnyView::new(DataOffsetPane {
            axis: OffsetAxis::Horizontal,
            offset: controller.offset.clone(),
            content: header_cells(controller, plan, moving, geometry, offset, layout),
        })
        .flex_with(1.0, 1.0, Some(0.0))
        .frame(
            None,
            None,
            Some(0.0),
            Some(layout.header_height),
            None,
            None,
        ),
    ));
    parts.push(AnyView::new(
        BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(crate::EmptyView),
        )
        .frame_width(12.0)
        .flex_with(0.0, 0.0, None),
    ));
    AnyView::new(HeaderPaint {
        content: AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::FlexStart,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(parts)),
            )
            .frame(None, None, None, Some(layout.header_height), None, None),
        ),
    })
}

fn build_frozen_lane<
    Record: Clone + 'static,
    Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static,
>(
    controller: &DataTableController<Record, Id>,
    plan: &DataTablePlan<Record>,
    metrics: &DataTableRowMetrics,
    window: DataTableWindows,
    _offset: Point,
    layout: &TableLayout,
    session: &DataTableSession<Id>,
    viewport_height: f32,
    moving_width: f32,
    input: Rc<RefCell<Option<Node>>>,
) -> AnyView {
    let frozen_count = plan.geometry.frozen_count.min(plan.columns.len());
    if frozen_count == 0 {
        return AnyView::new(crate::EmptyView);
    }
    let rows = window.rows(metrics.count);
    AnyView::new(FrozenLane {
        controller: controller.clone(),
        plan: plan.clone(),
        metrics: metrics.clone(),
        rows,
        columns: 0..frozen_count,
        layout: layout.clone(),
        session: session.clone(),
        viewport_height,
        moving_width,
        input,
    })
}

fn build_moving_lane<
    Record: Clone + 'static,
    Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static,
>(
    controller: &DataTableController<Record, Id>,
    plan: &DataTablePlan<Record>,
    metrics: &DataTableRowMetrics,
    window: DataTableWindows,
    offset: Point,
    layout: &TableLayout,
    session: &DataTableSession<Id>,
    moving_width: f32,
    viewport_height: f32,
    input: Rc<RefCell<Option<Node>>>,
) -> AnyView {
    let frozen_count = plan.geometry.frozen_count.min(plan.columns.len());
    let columns = window.columns(plan.columns.len());
    let start = columns.start.max(frozen_count);
    let rows = window.rows(metrics.count);
    let _content_width = plan.geometry.moving_width().max(moving_width);
    let content = AnyView::new(MovingRows {
        controller: controller.clone(),
        plan: plan.clone(),
        metrics: metrics.clone(),
        rows,
        columns: start..columns.end.max(start + 1),
        layout: layout.clone(),
        session: session.clone(),
        viewport_height,
        moving_width,
        input,
    });

    let scroll = ScrollView::new(ScrollAxis::Both, content)
        .scrollbar_gutter(ScrollbarGutter::Stable)
        .scroll_offset(controller.offset.binding());
    let _ = offset;
    AnyView::new(AnyView::new(scroll).flex_with(1.0, 1.0, Some(0.0)).frame(
        None,
        None,
        Some(0.0),
        Some(0.0),
        None,
        None,
    ))
}

/// A lane of absolutely stacked rows with a leading and trailing spacer, so the
/// lane's own height is the table's full scroll extent.
struct LaneRows<Record: Clone + 'static, Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static>
{
    controller: DataTableController<Record, Id>,
    plan: DataTablePlan<Record>,
    metrics: DataTableRowMetrics,
    rows: std::ops::Range<usize>,
    columns: std::ops::Range<usize>,
    layout: TableLayout,
    session: DataTableSession<Id>,
    viewport_height: f32,
    moving_width: f32,
    input: Rc<RefCell<Option<Node>>>,
}

impl<Record: Clone + 'static, Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static> View
    for LaneRows<Record, Id>
{
    fn body(&self, _cx: &Scope) -> AnyView {
        let mut children: Vec<AnyView> = Vec::new();
        let top = self.metrics.top(self.rows.start);
        if top > 0.0 {
            children.push(AnyView::new(
                BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(crate::EmptyView),
                )
                .frame_height(top)
                .flex_with(0.0, 0.0, None),
            ));
        }
        for index in self.rows.clone() {
            children.push(AnyView::new(DataTableRow {
                controller: self.controller.clone(),
                plan: self.plan.clone(),
                row: index,
                height: self.metrics.height(index),
                columns: self.columns.clone(),
                layout: self.layout.clone(),
                session: self.session.clone(),
                input: self.input.clone(),
            }));
        }
        let total = self.metrics.total();
        let bottom = (total - self.metrics.top(self.rows.end)).max(0.0);
        if bottom > 0.0 {
            children.push(AnyView::new(
                BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(crate::EmptyView),
                )
                .frame_height(bottom)
                .flex_with(0.0, 0.0, None),
            ));
        }
        let _ = (self.viewport_height, self.moving_width);
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(children)),
            )
            .frame(None, None, None, Some(total.max(1.0)), None, None),
        )
    }
}

/// The frozen lane: same rows, leading columns, no scrolling of its own.
struct FrozenLane<
    Record: Clone + 'static,
    Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static,
> {
    controller: DataTableController<Record, Id>,
    plan: DataTablePlan<Record>,
    metrics: DataTableRowMetrics,
    rows: std::ops::Range<usize>,
    columns: std::ops::Range<usize>,
    layout: TableLayout,
    session: DataTableSession<Id>,
    viewport_height: f32,
    moving_width: f32,
    input: Rc<RefCell<Option<Node>>>,
}

impl<Record: Clone + 'static, Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static> View
    for FrozenLane<Record, Id>
{
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(LaneRows {
            controller: self.controller.clone(),
            plan: self.plan.clone(),
            metrics: self.metrics.clone(),
            rows: self.rows.clone(),
            columns: self.columns.clone(),
            layout: self.layout.clone(),
            session: self.session.clone(),
            viewport_height: self.viewport_height,
            moving_width: self.moving_width,
            input: self.input.clone(),
        })
    }
}

/// The moving lane: scrolling columns inside the two-axis scroll view.
struct MovingRows<
    Record: Clone + 'static,
    Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static,
> {
    controller: DataTableController<Record, Id>,
    plan: DataTablePlan<Record>,
    metrics: DataTableRowMetrics,
    rows: std::ops::Range<usize>,
    columns: std::ops::Range<usize>,
    layout: TableLayout,
    session: DataTableSession<Id>,
    viewport_height: f32,
    moving_width: f32,
    input: Rc<RefCell<Option<Node>>>,
}

impl<Record: Clone + 'static, Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static> View
    for MovingRows<Record, Id>
{
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(
            AnyView::new(LaneRows {
                controller: self.controller.clone(),
                plan: self.plan.clone(),
                metrics: self.metrics.clone(),
                rows: self.rows.clone(),
                columns: self.columns.clone(),
                layout: self.layout.clone(),
                session: self.session.clone(),
                viewport_height: self.viewport_height,
                moving_width: self.moving_width,
                input: self.input.clone(),
            })
            .frame_width(self.plan.geometry.moving_width().max(self.moving_width)),
        )
    }
}

/// One row: the cells for one range of columns.
struct DataTableRow<
    Record: Clone + 'static,
    Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static,
> {
    controller: DataTableController<Record, Id>,
    plan: DataTablePlan<Record>,
    row: usize,
    height: f32,
    columns: std::ops::Range<usize>,
    layout: TableLayout,
    session: DataTableSession<Id>,
    input: Rc<RefCell<Option<Node>>>,
}

impl<Record: Clone + 'static, Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static> View
    for DataTableRow<Record, Id>
{
    fn body(&self, _cx: &Scope) -> AnyView {
        let Some(record) = self.controller.model.record_at(self.row) else {
            return AnyView::new(crate::EmptyView);
        };
        let Some(row_id) = self.controller.model.row_id_at(self.row) else {
            return AnyView::new(crate::EmptyView);
        };
        let selection = self.controller.selection(&self.session);
        let is_selected = selection.contains(&row_id);
        let geometry = &self.plan.geometry;
        let frozen_count = geometry.frozen_count;
        let is_frozen_lane =
            self.columns.end <= frozen_count && frozen_count < self.plan.columns.len();
        let mut cells: Vec<AnyView> = Vec::new();
        if self.columns.start >= frozen_count && self.columns.start < geometry.starts().len() {
            let gap = geometry.starts()[self.columns.start] - geometry.frozen_width();
            cells.push(AnyView::new(
                BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(crate::EmptyView),
                )
                .frame_width(gap.max(0.0))
                .flex_with(0.0, 0.0, None),
            ));
        }
        for index in self.columns.clone() {
            let Some(column) = self.plan.columns.get(index) else {
                continue;
            };
            let width = geometry
                .widths
                .get(index)
                .copied()
                .unwrap_or(column.layout.width);
            let is_active = selection.focused_id.as_ref() == Some(&row_id)
                && selection.focused_column_id.as_ref() == Some(&column.id);
            cells.push(AnyView::new(DataTableCell {
                controller: self.controller.clone(),
                plan: self.plan.clone(),
                column: column.clone(),
                column_index: index,
                row: self.row,
                row_id: row_id.clone(),
                record: record.clone(),
                is_active,
                width,
                height: self.height,
                session: self.session.clone(),
                input: self.input.clone(),
            }));
        }
        AnyView::new(DataTableRowHost {
            row: self.row,
            hover: self.controller.row_hover.clone(),
            is_selected,
            is_striped: self.layout.is_striped,
            is_enabled: self.controller.is_enabled,
            is_frozen_lane,
            height: self.height,
            separator_inset: self.layout.separator_inset.unwrap_or(0.0),
            content: AnyView::new(
                BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(TupleView::new(cells)),
                )
                .frame(None, None, None, Some(self.height), None, None),
            ),
        })
    }
}

/// One body row: paint, hairline separator and the row marker.
struct DataTableRowHost {
    row: usize,
    hover: Rc<DataTableRowHover>,
    is_selected: bool,
    is_striped: bool,
    is_enabled: bool,
    is_frozen_lane: bool,
    height: f32,
    separator_inset: f32,
    content: AnyView,
}

impl PrimitiveView for DataTableRowHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_align_items(Align::Stretch);
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        node.set_attachment(ROW_INDEX_MARKER, self.row);
        let theme = theme_for_node(node);
        let fill = if self.is_selected {
            // A selected row is a tint of the accent, not the accent itself:
            // a saturated fill drops the value contrast to nothing and looks
            // like a highlighted text run.
            theme.colors.state_layer_selected
        } else if self.is_striped && self.row % 2 == 1 {
            // macOS alternating rows: one step of fill, never a tinted wash.
            theme.colors.surface_variant
        } else {
            // The card's own fill: `background` is the page colour, which in a
            // dark theme is darker than the card and reads as a hole.
            theme.colors.surface
        };
        self.hover.register(self.row, node, fill, self.is_selected);
        let is_hovered = self.is_enabled && self.hover.current() == Some(self.row);
        node.set_background_color(Some(if is_hovered && !self.is_selected {
            theme.colors.state_layer_hover
        } else {
            fill
        }));
        if let Some(registry) = InteractionRegistryHolder::current() {
            let hover = self.hover.clone();
            let row = self.row;
            let is_enabled = self.is_enabled;
            let weak = node.downgrade();
            registry.set_hover(
                node,
                Rc::new(move |phase| {
                    let supports = Node::upgrade(&weak).is_some_and(|owner| {
                        crate::primitives::controls::adaptive::supports_control_hover(&owner)
                    });
                    if phase == HoverPhase::Enter {
                        if is_enabled && supports {
                            hover.set(Some(row));
                        }
                    } else if hover.current() == Some(row) {
                        hover.set(None);
                    }
                }),
            );
        }
        node.set_accessibility(Some(AccessibilitySemantics {
            role: if self.is_frozen_lane {
                AccessibilityRole::Group
            } else {
                AccessibilityRole::Row
            },
            label: format!("Row {}", self.row + 1),
            state: AccessibilityState {
                is_selected: self.is_selected,
                is_enabled: self.is_enabled,
                ..Default::default()
            },
            ..Default::default()
        }));
        let height = self.height;
        let separator_inset = self.separator_inset;
        let is_selected = self.is_selected;
        let draw_node = node.downgrade();
        node.update_draw(self.row, move |list, origin| {
            // A row separation is a hairline, not a box: it spans the row and
            // stops at the inset, so a flush table runs it edge to edge.
            let Some(live) = Node::upgrade(&draw_node) else {
                return;
            };
            let width = live.absolute_frame().width;
            let theme = theme_for_node(&live);
            let color = theme.colors.divider;
            let _ = list.add_rect(
                cove_core::Rect::new(
                    origin.x + separator_inset,
                    origin.y + height - 0.5,
                    (width - 2.0 * separator_inset).max(0.0),
                    0.5,
                ),
                color,
            );
            // The marker is the accent itself, so the selection still reads
            // when the tint is this light — and it survives a colour-blind
            // viewer who cannot separate the tint from the card.
            if is_selected {
                let _ = list.add_rect(
                    cove_core::Rect::new(origin.x, origin.y, 2.0, height),
                    theme.colors.accent,
                );
            }
        });
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(DataTableRowHost);

/// One body cell. Owns the draft editor when this cell is being edited.
struct DataTableCell<
    Record: Clone + 'static,
    Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static,
> {
    controller: DataTableController<Record, Id>,
    plan: DataTablePlan<Record>,
    column: TableColumn<Record>,
    column_index: usize,
    row: usize,
    row_id: Id,
    record: Record,
    is_active: bool,
    width: f32,
    height: f32,
    session: DataTableSession<Id>,
    input: Rc<RefCell<Option<Node>>>,
}

impl<Record: Clone + 'static, Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static> View
    for DataTableCell<Record, Id>
{
    fn body(&self, _cx: &Scope) -> AnyView {
        let title = self.column.title.clone();
        let draft = self
            .controller
            .open_draft(&self.session, &self.row_id, &self.column.id);
        let content = match draft.clone() {
            Some(buffer) => AnyView::new(DraftEditor {
                controller: self.controller.clone(),
                plan: self.plan.clone(),
                input: self.input.clone(),
                token: self
                    .session
                    .editing
                    .draft
                    .as_ref()
                    .map_or(0, |draft| draft.token),
                column_id: self.column.id.clone(),
                buffer,
                title: title.clone(),
                error: self
                    .session
                    .editing
                    .draft
                    .as_ref()
                    .and_then(|draft| draft.error.clone()),
                is_enabled: self.controller.is_enabled,
            }),
            None => self.column.cell(&self.record),
        };
        let alignment = self.column.layout.alignment;
        let justify = match alignment {
            cove_core::TextAlignment::Leading => Justify::FlexStart,
            cove_core::TextAlignment::Trailing => Justify::FlexEnd,
            cove_core::TextAlignment::Center => Justify::Center,
        };
        AnyView::new(DataTableCellHost {
            controller: self.controller.clone(),
            plan: self.plan.clone(),
            title,
            row: self.row,
            column_index: self.column_index,
            is_active: self.is_active,
            is_enabled: self.controller.is_enabled,
            width: self.width,
            height: self.height,
            content: AnyView::new(
                BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    justify,
                    Wrap::NoWrap,
                    0.0,
                    content,
                )
                .padding_edges(20.0, 4.0)
                .clipped(),
            ),
        })
    }
}

/// Cell shell: focus ring, hover fill, activation and the accessibility cell.
struct DataTableCellHost<
    Record: Clone + 'static,
    Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static,
> {
    controller: DataTableController<Record, Id>,
    plan: DataTablePlan<Record>,
    title: String,
    row: usize,
    column_index: usize,
    is_active: bool,
    is_enabled: bool,
    width: f32,
    height: f32,
    content: AnyView,
}

impl<Record: Clone + 'static, Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static>
    PrimitiveView for DataTableCellHost<Record, Id>
{
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(self.is_enabled);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_width(Some(self.width));
        layout.set_height(Some(self.height));
        layout.set_flex_shrink(0.0);
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_width(Some(self.width));
        layout.set_height(Some(self.height));
        layout.set_flex_shrink(0.0);
    }

    fn update_node(&self, node: &Node) {
        let theme = theme_for_node(node);
        // A disabled table removes its cells from input-enabled discovery, so
        // an accessibility read cannot reach a control the pointer cannot.
        node.set_allows_hit_testing(self.is_enabled);
        node.set_border_width(if self.is_active { 1.0 } else { 0.0 });
        node.set_border_color(Some(theme.colors.focus_ring));
        node.set_clips_to_bounds(true);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Cell,
            label: self.title.clone(),
            state: AccessibilityState {
                is_selected: self.is_active,
                is_enabled: self.is_enabled,
                ..Default::default()
            },
            ..Default::default()
        }));
        let controller = self.controller.clone();
        let _plan = self.plan.clone();
        let row = self.row;
        let column_index = self.column_index;
        let activate = self.is_enabled;
        node.set_accessibility_actions(AccessibilityActions {
            activate: activate.then(|| Rc::new(move || controller.select_row(row)) as Rc<dyn Fn()>),
            ..Default::default()
        });
        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        if !self.is_enabled {
            registry.remove(node);
            return;
        }
        node.set_cursor(Some(SystemCursor::Pointer));
        registry.set_hover(
            node,
            Rc::new(move |phase: HoverPhase| {
                if let Some(owner) = None::<Node> {
                    let _ = owner;
                }
                let _ = phase;
            }),
        );
        let controller = self.controller.clone();
        let plan = self.plan.clone();
        registry.set_pointer(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase, event_phase| {
                if event_phase == EventPhase::Capture || event.button != MouseButton::Left {
                    return EventResult::Ignored;
                }
                if phase == PointerPhase::Down {
                    controller.select(row, column_index, &plan, event.modifiers);
                    if event.clicks >= 2 {
                        controller.begin_editing(row, column_index, &plan);
                    }
                    return EventResult::Handled;
                }
                EventResult::Ignored
            }),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl<Record: Clone + 'static, Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static> View
    for DataTableCellHost<Record, Id>
{
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// The in-place editor for one cell.
struct DraftEditor<
    Record: Clone + 'static,
    Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static,
> {
    controller: DataTableController<Record, Id>,
    plan: DataTablePlan<Record>,
    /// The table's keyboard host, which regains focus once the draft closes.
    input: Rc<RefCell<Option<Node>>>,
    /// One-per-open-editor identity, so the field requests focus exactly once.
    token: u64,
    column_id: String,
    buffer: String,
    title: String,
    error: Option<String>,
    is_enabled: bool,
}

impl<Record: Clone + 'static, Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static> View
    for DraftEditor<Record, Id>
{
    fn body(&self, _cx: &Scope) -> AnyView {
        let controller = self.controller.clone();
        let plan = self.plan.clone();
        // The table's own keyboard host, so committing hands focus back to the
        // table instead of dropping it.
        let input = self.input.clone();
        let buffer = self.buffer.clone();
        let _write_controller = controller.clone();
        // `Binding` requires `Send + Sync`; the draft buffer itself lives on the
        // controller, so the source is mirrored through a `State` and written back
        // on set (same pattern as the virtual stack's offset binding).
        let text_state = cove_core::State::new(TextBuffer::new(&buffer));
        text_state.set(TextBuffer::new(&buffer));
        let text = Binding::new(
            {
                let text_state = text_state.clone();
                move || text_state.get()
            },
            {
                let text_state = text_state.clone();
                move |value: TextBuffer| text_state.set(value)
            },
        );
        let change_controller = controller.clone();
        let submit_controller = controller.clone();
        let submit_input = input.clone();
        let cancel_controller = controller.clone();
        let cancel_input = input.clone();
        let tab_controller = controller.clone();
        let tab_plan = plan.clone();
        let tab_input = input.clone();
        let blur_controller = controller.clone();
        let blur_input = input.clone();
        let mut field = TextField::new("", text)
            .behavior(TextFieldBehavior {
                disabled: !self.is_enabled,
                ..Default::default()
            })
            .events(TextFieldEvents {
                on_submit: Some(Rc::new(move || {
                    submit_controller.commit_editing(&submit_input);
                })),
                on_cancel: Some(Rc::new(move || {
                    cancel_controller.cancel_editing(&cancel_input);
                })),
                on_change: Some(Rc::new(move |value: &TextBuffer| {
                    change_controller.set_draft_buffer(value.string_value());
                })),
                // Clicking away commits the draft. Focus already moved to
                // whatever the person clicked, so this path does not restore it.
                on_blur: Some(Rc::new(move || {
                    blur_controller.commit_editing_without_focus(&blur_input);
                })),
                on_key_down: Some(Rc::new(move |event: &KeyEvent| {
                    if event.scancode != crate::Scancode::TAB {
                        return false;
                    }
                    let direction = if event.modifiers.has_shift() { -1 } else { 1 };
                    tab_controller.move_editor(direction, &tab_plan, &tab_input);
                    true
                })),
                ..Default::default()
            });
        let semantics = AccessibilitySemantics {
            role: AccessibilityRole::TextField,
            label: format!("Edit {}", self.title),
            identifier: self.column_id.clone(),
            state: AccessibilityState {
                is_invalid: self.error.is_some(),
                is_enabled: self.is_enabled,
                ..Default::default()
            },
            help: self
                .error
                .clone()
                .unwrap_or_else(|| "Return commits, Escape cancels".to_string()),
            ..Default::default()
        };
        // Opening the editor moves focus into the field, and the request
        // selects the whole value so typing replaces it.
        field.navigation = TextFieldNavigationRequest {
            focus_request_id: (self.token > 0).then_some(self.token),
            ..Default::default()
        };
        AnyView::new(TextFieldShell {
            field: AnyView::new(field.padding_edges(4.0, 2.0)),
            semantics,
        })
    }
}

/// Applies the editor's accessibility contract on the field's own node.
struct TextFieldShell {
    field: AnyView,
    semantics: AccessibilitySemantics,
}

impl PrimitiveView for TextFieldShell {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, node: &Node) {
        node.set_accessibility(Some(self.semantics.clone()));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.field.clone()]
    }
}
impl_primitive_view!(TextFieldShell);

/// One header cell: title, sort indicator, the sort action and the resize grip.
struct HeaderCell<
    Record: Clone + 'static,
    Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static,
> {
    controller: DataTableController<Record, Id>,
    plan: DataTablePlan<Record>,
    column: TableColumn<Record>,
    index: usize,
    width: f32,
    height: f32,
    is_enabled: bool,
    sorted: Option<TableSort>,
}

impl<Record: Clone + 'static, Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static> View
    for HeaderCell<Record, Id>
{
    fn body(&self, _cx: &Scope) -> AnyView {
        let column_id = self.column.id.clone();
        let sortable = self.column.compare.is_some();
        let is_sorted = self
            .sorted
            .as_ref()
            .is_some_and(|sort| sort.column_id == self.column.id);
        let direction = self
            .sorted
            .as_ref()
            .filter(|_| is_sorted)
            .map(|sort| sort.direction);
        let controller = self.controller.clone();
        let alignment = self.column.layout.alignment;
        let mut row: Vec<AnyView> = Vec::new();
        let text = Text::new(self.column.title.clone()).alignment(alignment);
        // `.truncated()` wraps the label in a tooltip host, and that wrapper
        // lays its child out leading — a trailing column's title would snap
        // back to the left edge. Leading columns keep the tooltip; the others
        // clip at the cell edge instead.
        let label = if alignment == TextAlignment::Leading {
            AnyView::new(text.truncated())
        } else {
            AnyView::new(text)
        };
        row.push(AnyView::new(
            label
                // A column label is a label, not a value: one size down from
                // the body and a step back in contrast, so the header recedes
                // and the data reads first.
                .semantic_font(SemanticFontRef::label())
                .semantic_foreground(SemanticColorRef::on_surface_variant())
                .flex_with(1.0, 1.0, Some(0.0)),
        ));
        if is_sorted {
            row.push(AnyView::new(
                Text::new(match direction {
                    Some(TableSortDirection::Descending) => "↓",
                    _ => "↑",
                })
                .semantic_font(SemanticFontRef::caption()),
            ));
        }
        // The label sits where the column's values sit: a trailing column's
        // header has to land on the same right edge as its numbers.
        let justify = match alignment {
            TextAlignment::Leading => Justify::FlexStart,
            TextAlignment::Trailing => Justify::FlexEnd,
            TextAlignment::Center => Justify::Center,
        };
        // The grip sits inside the column's own width; a 6pt sibling here
        // would widen every header column past the body column below it, and
        // the columns would drift further apart with each rendered column.
        let grip = if self.column.layout.is_resizable {
            crate::primitives::data::columns::resize_grip::RESIZE_GRIP_WIDTH
        } else {
            0.0
        };
        // A trailing label gets the grip's width back as padding, so it lands
        // on the same right edge as the values under it.
        let trailing_padding = if alignment == TextAlignment::Trailing {
            (20.0 - grip).max(0.0)
        } else {
            20.0
        };
        let label = AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                justify,
                Wrap::NoWrap,
                6.0,
                AnyView::new(TupleView::new(row)),
            )
            .padding_insets(EdgeInsets {
                leading: 20.0,
                trailing: trailing_padding,
                top: 0.0,
                bottom: 0.0,
            }),
        );
        let mut cells: Vec<AnyView> = vec![AnyView::new(HeaderCellHost {
            controller: controller.clone(),
            plan: self.plan.clone(),
            index: self.index,
            is_enabled: self.is_enabled,
            column_id,
            title: self.column.title.clone(),
            width: (self.width - grip).max(0.0),
            height: self.height,
            sortable: sortable && self.is_enabled,
            is_sorted,
            direction,
            content: label,
        })];
        if self.column.layout.is_resizable {
            cells.push(AnyView::new(shared_resize_grip(
                &self.controller,
                &self.column,
                self.is_enabled,
            )));
        }
        AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(cells)),
            )
            .frame(None, None, None, Some(self.height), None, None)
            .clipped(),
        )
    }
}

/// Header button: sort cycling plus header semantics.
struct HeaderCellHost<
    Record: Clone + 'static,
    Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static,
> {
    controller: DataTableController<Record, Id>,
    plan: DataTablePlan<Record>,
    index: usize,
    is_enabled: bool,
    column_id: String,
    title: String,
    width: f32,
    height: f32,
    sortable: bool,
    is_sorted: bool,
    direction: Option<TableSortDirection>,
    content: AnyView,
}

impl<Record: Clone + 'static, Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static>
    PrimitiveView for HeaderCellHost<Record, Id>
{
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(self.is_enabled);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_width(Some(self.width));
        layout.set_height(Some(self.height));
        layout.set_flex_shrink(0.0);
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_width(Some(self.width));
        layout.set_height(Some(self.height));
        layout.set_flex_shrink(0.0);
    }

    fn update_node(&self, node: &Node) {
        node.set_allows_hit_testing(self.is_enabled);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Button,
            label: format!("Sort by {}", self.title),
            help: match (self.is_sorted, self.direction) {
                (true, Some(TableSortDirection::Ascending)) => {
                    "Sorted ascending. Activate to sort descending.".to_string()
                }
                (true, Some(TableSortDirection::Descending)) => {
                    "Sorted descending. Activate to restore source order.".to_string()
                }
                _ => "Activate to sort ascending.".to_string(),
            },
            state: AccessibilityState {
                is_enabled: self.sortable,
                is_selected: self.is_sorted,
                ..Default::default()
            },
            ..Default::default()
        }));
        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        node.set_is_hit_testable(self.is_enabled);
        if !self.is_enabled {
            registry.remove(node);
            return;
        }
        node.set_cursor(Some(SystemCursor::Pointer));
        let controller = self.controller.clone();
        let column_id = self.column_id.clone();
        let plan = self.plan.clone();
        let index = self.index;
        let sortable = self.sortable;
        let pointer_node = node.downgrade();
        registry.set_pointer(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase, event_phase| {
                if event_phase == EventPhase::Capture || event.button != MouseButton::Left {
                    return EventResult::Ignored;
                }
                let Some(owner) = Node::upgrade(&pointer_node) else {
                    return EventResult::Ignored;
                };
                match phase {
                    PointerPhase::Down => {
                        owner.set_attachment(COLUMN_DRAG, (event.x, false));
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.acquire(&owner, false);
                        }
                        EventResult::Handled
                    }
                    PointerPhase::Up => {
                        let drag = owner.attachment::<(f32, bool)>(COLUMN_DRAG);
                        owner.remove_attachment(COLUMN_DRAG);
                        owner.set_content_offset(Point::default());
                        if PointerCaptureHolder::current()
                            .and_then(|capture| capture.target())
                            .is_some_and(|target| target == owner)
                            && let Some(capture) = PointerCaptureHolder::current()
                        {
                            capture.release();
                        }
                        match drag {
                            Some((start, true)) => {
                                let target = DataTableColumnOrder::target(
                                    index,
                                    event.x - start,
                                    &plan.geometry,
                                );
                                controller.move_column_to(&plan, index, target);
                            }
                            Some((_, false)) if sortable => controller.cycle_sort(&column_id),
                            _ => {}
                        }
                        EventResult::Handled
                    }
                }
            }),
        );
        let motion_node = node.downgrade();
        registry.set_motion(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, _phase| {
                let Some(owner) = Node::upgrade(&motion_node) else {
                    return EventResult::Ignored;
                };
                let Some((start, dragging)) = owner.attachment::<(f32, bool)>(COLUMN_DRAG) else {
                    return EventResult::Ignored;
                };
                let travel = event.x - start;
                if dragging || travel.abs() >= COLUMN_DRAG_SLOP {
                    owner.set_attachment(COLUMN_DRAG, (start, true));
                    // The label follows the pointer until the drop reorders.
                    owner.set_content_offset(Point::new(-travel, 0.0));
                }
                EventResult::Handled
            }),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl<Record: Clone + 'static, Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static> View
    for HeaderCellHost<Record, Id>
{
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Binds one column's width to the table's own storage: the caller's binding
/// when it supplied one, the mount's session otherwise.
fn shared_resize_grip<
    Record: Clone + 'static,
    Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static,
>(
    controller: &DataTableController<Record, Id>,
    column: &TableColumn<Record>,
    is_enabled: bool,
) -> crate::primitives::data::columns::ResizeGrip {
    let layout = column.layout;
    let read = controller.clone();
    let read_column = column.clone();
    let write = controller.clone();
    let write_column = column.clone();
    let width = Binding::new(
        move || {
            let stored = read.stored_widths();
            layout.constrained(
                stored
                    .get(&read_column.id)
                    .copied()
                    .unwrap_or(read_column.layout.width),
            )
        },
        move |value| write.write_width(&write_column, value),
    );
    crate::primitives::data::columns::ResizeGrip::new(
        column.title.clone(),
        width,
        layout,
        is_enabled,
    )
}

/// Header background, matching the canvas fill the reference table uses.
struct HeaderPaint {
    content: AnyView,
}

impl PrimitiveView for HeaderPaint {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, node: &Node) {
        // The header shares the card's fill: the grey region behind the card
        // is the same `surface_variant`, so a tinted band read as a notch cut
        // out of the card. Typography and the rule below carry the separation.
        node.set_background_color(Some(theme_for_node(node).colors.surface));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(HeaderPaint);

#[derive(Clone, Copy, PartialEq, Eq)]
enum OffsetAxis {
    Horizontal,
    Vertical,
}

/// Translates its content through the shared offset without rebuilding it, so
/// header and frozen-lane scrolling move existing nodes.
struct DataOffsetPane {
    axis: OffsetAxis,
    offset: State<Point>,
    content: AnyView,
}

impl PrimitiveView for DataOffsetPane {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_clips_to_bounds(true);
        node.set_is_hit_testable(true);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_align_items(Align::Stretch);
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        let offset = self.offset.get();
        node.set_content_offset(match self.axis {
            OffsetAxis::Horizontal => Point::new(offset.x, 0.0),
            OffsetAxis::Vertical => Point::new(0.0, offset.y),
        });
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(DataOffsetPane);

/// Owns the keyboard routing and the focus target for grid navigation.
struct DataTableInputHost<
    Record: Clone + 'static,
    Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static,
> {
    controller: DataTableController<Record, Id>,
    plan: DataTablePlan<Record>,
    metrics: DataTableRowMetrics,
    moving_width: f32,
    viewport_height: f32,
    input: Rc<RefCell<Option<Node>>>,
    content: AnyView,
}

impl<Record: Clone + 'static, Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static>
    PrimitiveView for DataTableInputHost<Record, Id>
{
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Group,
            label: crate::i18n::tr("Data table input"),
            ..Default::default()
        }));
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
        layout.set_flex_grow(1.0);
        layout.set_flex_shrink(1.0);
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        *self.input.borrow_mut() = Some(node.clone());
        node.set_attachment(INPUT_MARKER, true);
        node.set_is_focusable(self.controller.is_enabled);
        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        if !self.controller.is_enabled {
            registry.remove(node);
            return;
        }
        let controller = self.controller.clone();
        let plan = self.plan.clone();
        let metrics = self.metrics.clone();
        let moving_width = self.moving_width;
        let viewport_height = self.viewport_height;
        // Frozen cells are siblings of the moving ScrollView. Route their
        // wheel input through the same offset, including at either boundary.
        let wheel_controller = controller.clone();
        let wheel_plan = plan.clone();
        let wheel_metrics = metrics.clone();
        registry.set_wheel(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if phase == EventPhase::Capture {
                    return EventResult::Ignored;
                }
                let overflows_x = wheel_plan.geometry.moving_width() > moving_width;
                let overflows_y = wheel_metrics.total() > viewport_height;
                if !(event.x != 0.0 && overflows_x || event.y != 0.0 && overflows_y) {
                    return EventResult::Ignored;
                }
                let (dx, dy) = event.displacement(30.0, moving_width, viewport_height);
                let current = wheel_controller.offset.get();
                wheel_controller.offset.set(wheel_controller.clamped_offset(
                    Point::new(current.x - dx, current.y - dy),
                    &wheel_plan,
                    &wheel_metrics,
                    moving_width,
                    viewport_height,
                ));
                EventResult::Handled
            }),
        );
        let input = self.input.clone();
        registry.set_key(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if phase == EventPhase::Capture {
                    return EventResult::Ignored;
                }
                if controller.handle_key(
                    event,
                    &plan,
                    &metrics,
                    moving_width,
                    viewport_height,
                    &input,
                ) {
                    EventResult::Handled
                } else {
                    EventResult::Ignored
                }
            }),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl<Record: Clone + 'static, Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static> View
    for DataTableInputHost<Record, Id>
{
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
