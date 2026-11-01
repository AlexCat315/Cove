//! Translation of Primitives/Data/Lists/Table.swift.
//! Also translates Primitives/Data/Columns/Visibility.swift via the
//! `TableColumnPlan` type below.
//!
//! A fixed header and virtualized rows sharing one horizontal scroll coordinate
//! system. Cell content may include editors; embedded controls receive input
//! before row selection.
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::AppearanceExt;
use crate::modifiers::style::SemanticStyleViewExt;
use crate::primitives::feedback::EmptyState;
use crate::primitives::layout::grid::{ColumnVisibilityPlan, ColumnVisibilityTrack};
use crate::primitives::layout::{ScrollAxis, ScrollView, ScrollbarGutter};
use crate::primitives::text::Text;
use crate::theme::{SemanticFontRef, theme_for_node};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View};
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, Align, FlexDirection, Justify, Node, Wrap,
};
use cove_core::{Binding, TextAlignment};

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

/// Authored table options.
#[derive(Clone)]
pub struct TableOptions<Id: PartialEq + Clone> {
    pub selection: Option<Binding<Option<Id>>>,
    pub sort: Option<Binding<Option<TableSort>>>,
    /// When absent, resized widths belong to this mounted table's session.
    pub column_widths: Option<Binding<BTreeMap<String, f32>>>,
    /// Per-column user choices: `Some(true)` forces shown, `Some(false)` hides.
    pub column_visibility: Option<Binding<BTreeMap<String, bool>>>,
    pub layout: TableLayout,
    pub is_enabled: bool,
}

impl<Id: PartialEq + Clone> Default for TableOptions<Id> {
    fn default() -> Self {
        Self {
            selection: None,
            sort: None,
            column_widths: None,
            column_visibility: None,
            layout: TableLayout::default(),
            is_enabled: true,
        }
    }
}

impl<Id: PartialEq + Clone> TableOptions<Id> {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Session-only state for a mounted table.
struct TableSession<Id> {
    selection: Option<Id>,
    sort: Option<TableSort>,
    widths: BTreeMap<String, f32>,
}

impl<Id> Default for TableSession<Id> {
    fn default() -> Self {
        Self {
            selection: None,
            sort: None,
            widths: BTreeMap::new(),
        }
    }
}

/// A fixed header and virtualized rows sharing one horizontal scroll.
pub struct Table<Record: 'static, Id: PartialEq + Clone + 'static> {
    pub records: Rc<Vec<Record>>,
    pub row_id: Rc<dyn Fn(&Record) -> Id>,
    pub columns: Vec<TableColumn<Record>>,
    pub options: TableOptions<Id>,
    pub on_activate: Option<Rc<dyn Fn(&Record)>>,
    session: Rc<RefCell<TableSession<Id>>>,
}

impl<Record: 'static, Id: PartialEq + Clone + 'static> Table<Record, Id> {
    ///
    /// Column and row IDs must be unique.
    pub fn new(
        records: Vec<Record>,
        row_id: impl Fn(&Record) -> Id + 'static,
        columns: Vec<TableColumn<Record>>,
    ) -> Self {
        assert!(
            columns.iter().enumerate().all(|(index, column)| columns
                .iter()
                .enumerate()
                .all(|(other, other_column)| other == index || other_column.id != column.id)),
            "Table column IDs must be unique"
        );
        let row_id = Rc::new(row_id);
        let ids: Vec<Id> = records.iter().map(|record| row_id(record)).collect();
        assert!(
            ids.iter().enumerate().all(|(index, id)| ids
                .iter()
                .enumerate()
                .all(|(other, candidate)| other == index || candidate != id)),
            "Table row IDs must be unique"
        );
        Self {
            records: Rc::new(records),
            row_id,
            columns,
            options: TableOptions::default(),
            on_activate: None,
            session: Rc::new(RefCell::new(TableSession::default())),
        }
    }

    pub fn options(mut self, options: TableOptions<Id>) -> Self {
        self.options = options;
        self
    }

    pub fn on_activate(mut self, handler: impl Fn(&Record) + 'static) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }

    /// The current selection: the caller's binding, or this session's.
    fn selected_id(&self) -> Option<Id> {
        match self.options.selection.as_ref() {
            Some(binding) => binding.get(),
            None => self.session.borrow().selection.clone(),
        }
    }

    fn sort(&self) -> Option<TableSort> {
        match self.options.sort.as_ref() {
            Some(binding) => binding.get(),
            None => self.session.borrow().sort.clone(),
        }
    }

    fn width_of(&self, column: &TableColumn<Record>) -> f32 {
        let stored = match self.options.column_widths.as_ref() {
            Some(binding) => binding.get().get(&column.id).copied(),
            None => self.session.borrow().widths.get(&column.id).copied(),
        };
        column
            .layout
            .constrained(stored.unwrap_or(column.layout.width))
    }

    /// Cycle a column's sort: ascending, then descending, then source order.
    pub fn cycle_sort(&self, column_id: &str) {
        let next = match self.sort() {
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
            _ => Some(TableSort::new(column_id)),
        };
        match self.options.sort.as_ref() {
            Some(binding) => binding.set(next),
            None => self.session.borrow_mut().sort = next,
        }
    }

    /// Sort records, keeping source order for equal comparisons in both
    /// directions.
    pub fn sorted(
        records: &[Record],
        columns: &[TableColumn<Record>],
        sort: Option<&TableSort>,
    ) -> Vec<usize> {
        let mut order: Vec<usize> = (0..records.len()).collect();
        let Some(sort) = sort else { return order };
        let Some(column) = columns.iter().find(|column| column.id == sort.column_id) else {
            return order;
        };
        let Some(compare) = column.compare.as_ref() else {
            return order;
        };
        order.sort_by(|lhs, rhs| {
            let comparison = compare(&records[*lhs], &records[*rhs]);
            match comparison {
                ComparisonResult::Same => lhs.cmp(rhs),
                ComparisonResult::Ascending => {
                    if sort.direction == TableSortDirection::Ascending {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Greater
                    }
                }
                ComparisonResult::Descending => {
                    if sort.direction == TableSortDirection::Ascending {
                        std::cmp::Ordering::Greater
                    } else {
                        std::cmp::Ordering::Less
                    }
                }
            }
        });
        order
    }
}

impl<Record: 'static, Id: PartialEq + Clone + 'static> View for Table<Record, Id> {
    fn body(&self) -> AnyView {
        let layout = self.options.layout.clone().validated();
        let sort = self.sort();
        let order = Self::sorted(&self.records, &self.columns, sort.as_ref());
        // Explicit per-column overrides win: `false` hides, `true` forces shown.
        // Automatic fitting needs the Columns/Visibility resolver, which is not
        // ported; absent overrides therefore leave a column as authored.
        let overrides = self
            .options
            .column_visibility
            .as_ref()
            .map(|binding| binding.get());
        let visible: Vec<bool> = self
            .columns
            .iter()
            .map(
                |column| match overrides.as_ref().and_then(|map| map.get(&column.id)) {
                    Some(false) => false,
                    Some(true) => true,
                    None => column.visibility != ColumnVisibility::Hidden,
                },
            )
            .collect();
        let widths: Vec<f32> = self
            .columns
            .iter()
            .map(|column| self.width_of(column))
            .collect();
        let total_width: f32 = widths.iter().sum();

        let mut header_cells: Vec<AnyView> = Vec::new();
        for (index, column) in self.columns.iter().enumerate() {
            if !visible[index] {
                continue;
            }
            let is_sorted = sort
                .as_ref()
                .is_some_and(|sort| sort.column_id == column.id);
            let direction = sort
                .as_ref()
                .filter(|_| is_sorted)
                .map(|sort| sort.direction);
            let table_session = self.session.clone();
            let sort_binding = self.options.sort.clone();
            let enabled = self.options.is_enabled;
            let column_id = column.id.clone();
            let title = column.title.clone();
            let sortable = column.compare.is_some();
            header_cells.push(AnyView::new(TableHeaderCell {
                title: title.clone(),
                column_id: column_id.clone(),
                width: widths[index],
                height: layout.header_height,
                sortable,
                is_enabled: enabled,
                is_sorted,
                direction,
                on_sort: {
                    let table_session = table_session.clone();
                    let sort_binding = sort_binding.clone();
                    Rc::new(move || {
                        let next = match (
                            sort_binding.as_ref().map(|binding| binding.get()).flatten(),
                            is_sorted,
                        ) {
                            (Some(current), true)
                                if current.direction == TableSortDirection::Ascending =>
                            {
                                Some(TableSort {
                                    column_id: column_id.clone(),
                                    direction: TableSortDirection::Descending,
                                })
                            }
                            (_, true) => None,
                            _ => Some(TableSort::new(column_id.clone())),
                        };
                        match sort_binding.as_ref() {
                            Some(binding) => binding.set(next),
                            None => table_session.borrow_mut().sort = next,
                        }
                    })
                },
            }));
        }

        let header = AnyView::new(TableHeaderPaint {
            content: AnyView::new(
                BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(TupleView::new(header_cells)),
                )
                .frame(None, None, None, Some(layout.header_height), None, None),
            ),
        });

        if order.is_empty() {
            return AnyView::new(EmptyState::new(
                layout.empty_title.clone(),
                "Change your filters or add a record.",
            ));
        }

        let mut rows: Vec<AnyView> = Vec::new();
        for (position, record_index) in order.iter().enumerate() {
            // Copy the index so the row's press handler owns it.
            let row_index = *record_index;
            let record = &self.records[row_index];
            let mut cells: Vec<AnyView> = Vec::new();
            for (index, column) in self.columns.iter().enumerate() {
                if !visible[index] {
                    continue;
                }
                cells.push(AnyView::new(TableCell {
                    content: column.cell(record),
                    width: widths[index],
                    height: layout.row_height,
                    alignment: column.layout.alignment,
                    label: column.title.clone(),
                }));
            }
            let row_key = (self.row_id)(record);
            let selected = self.selected_id();
            let is_selected = selected.as_ref() == Some(&row_key);
            let selection_binding = self.options.selection.clone();
            let session = self.session.clone();
            let on_activate = self.on_activate.clone();
            let records_for_rows = self.records.clone();
            rows.push(AnyView::new(TableRowPaint {
                is_striped: layout.is_striped && position % 2 == 1,
                is_selected,
                is_enabled: self.options.is_enabled,
                on_press: Rc::new(move || {
                    match selection_binding.as_ref() {
                        Some(binding) => binding.set(Some(row_key.clone())),
                        None => session.borrow_mut().selection = Some(row_key.clone()),
                    }
                    if let Some(handler) = on_activate.as_ref() {
                        if let Some(record) = records_for_rows.get(row_index) {
                            handler(record);
                        }
                    }
                }),
                content: AnyView::new(
                    BoxView::new(
                        FlexDirection::Row,
                        Align::Center,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        0.0,
                        AnyView::new(TupleView::new(cells)),
                    )
                    .frame(
                        None,
                        None,
                        None,
                        Some(layout.row_height),
                        None,
                        None,
                    ),
                ),
            }));
        }

        let scroll = ScrollView::new(
            ScrollAxis::Horizontal,
            AnyView::new(TupleView::new(vec![
                header,
                AnyView::new(crate::primitives::text::Divider::new(
                    None,
                    1.0,
                    crate::primitives::text::DividerAxis::Horizontal,
                )),
                AnyView::new(
                    ScrollView::vertical(AnyView::new(BoxView::new(
                        FlexDirection::Column,
                        Align::Stretch,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        0.0,
                        AnyView::new(TupleView::new(rows)),
                    )))
                    .flex_with(1.0, 1.0, Some(0.0)),
                ),
            ])),
        )
        .scrollbar_gutter(ScrollbarGutter::Stable);
        AnyView::new(TableChromeHost {
            content: AnyView::new(AnyView::new(scroll).frame(
                None,
                None,
                None,
                None,
                Some(total_width + 14.0),
                None,
            )),
            is_enabled: self.options.is_enabled,
        })
    }
}

/// The table's outer chrome and table-level semantics.
struct TableChromeHost {
    content: AnyView,
    is_enabled: bool,
}

impl PrimitiveView for TableChromeHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, node: &Node) {
        let theme = theme_for_node(node);
        node.set_border_color(Some(theme.colors.border));
        node.set_border_width(1.0);
        node.set_corner_radius(theme.radius.lg);
        node.set_opacity(if self.is_enabled { 1.0 } else { 0.55 });
        node.set_allows_hit_testing(self.is_enabled);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Table,
            state: cove_core::scene::AccessibilityState {
                is_enabled: self.is_enabled,
                ..Default::default()
            },
            ..Default::default()
        }));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
crate::impl_primitive_view!(TableChromeHost);

/// The header sits on the canvas, matching the Swift `.background(.background)`.
struct TableHeaderPaint {
    content: AnyView,
}

impl PrimitiveView for TableHeaderPaint {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, node: &Node) {
        node.set_background_color(Some(theme_for_node(node).colors.background));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
crate::impl_primitive_view!(TableHeaderPaint);

/// A body row: the strike tint alternates here, where the node's theme is known.
struct TableRowPaint {
    is_striped: bool,
    is_selected: bool,
    is_enabled: bool,
    on_press: Rc<dyn Fn()>,
    content: AnyView,
}

impl PrimitiveView for TableRowPaint {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(self.is_enabled);
        node
    }

    fn update_node(&self, node: &Node) {
        let theme = theme_for_node(node);
        node.set_background_color(if self.is_selected {
            Some(theme.colors.selection)
        } else if self.is_striped {
            Some(theme.colors.on_surface.multiplied_alpha(0.04))
        } else {
            None
        });
        let Some(registry) = cove_core::scene::InteractionRegistryHolder::current() else {
            return;
        };
        if !self.is_enabled {
            registry.remove(node);
            return;
        }
        let on_press = self.on_press.clone();
        registry.set_pointer(
            node,
            cove_core::scene::InputHandlerRoute::control(),
            Rc::new(move |event, phase, event_phase| {
                if event.button != cove_platform::MouseButton::Left
                    || phase != cove_core::scene::PointerPhase::Up
                    || event_phase != cove_core::scene::EventPhase::Target
                {
                    return cove_core::scene::EventResult::Ignored;
                }
                on_press();
                cove_core::scene::EventResult::Handled
            }),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
crate::impl_primitive_view!(TableRowPaint);

/// One header cell: title, sort indicator and the sort action.
struct TableHeaderCell {
    title: String,
    column_id: String,
    width: f32,
    height: f32,
    sortable: bool,
    is_enabled: bool,
    is_sorted: bool,
    direction: Option<TableSortDirection>,
    on_sort: Rc<dyn Fn()>,
}

impl PrimitiveView for TableHeaderCell {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(self.is_enabled && self.sortable);
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
            state: cove_core::scene::AccessibilityState {
                is_enabled: self.is_enabled && self.sortable,
                is_selected: self.is_sorted,
                ..Default::default()
            },
            ..Default::default()
        }));
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_width(Some(self.width));
        layout.set_height(Some(self.height));
        layout.set_flex_shrink(0.0);
        Some(layout)
    }

    fn update_layout(&self, layout: &cove_core::scene::LayoutNode) {
        layout.set_width(Some(self.width));
        layout.set_height(Some(self.height));
        layout.set_flex_shrink(0.0);
    }

    fn update_node(&self, node: &Node) {
        node.set_attachment("table.columnId", self.column_id.clone());
        let Some(registry) = cove_core::scene::InteractionRegistryHolder::current() else {
            return;
        };
        if !self.is_enabled || !self.sortable {
            registry.remove(node);
            return;
        }
        let on_sort = self.on_sort.clone();
        registry.set_pointer(
            node,
            cove_core::scene::InputHandlerRoute::control(),
            Rc::new(move |event, phase, event_phase| {
                if event.button != cove_platform::MouseButton::Left
                    || phase != cove_core::scene::PointerPhase::Up
                    || event_phase != cove_core::scene::EventPhase::Target
                {
                    return cove_core::scene::EventResult::Ignored;
                }
                on_sort();
                cove_core::scene::EventResult::Handled
            }),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let mut row: Vec<AnyView> = vec![AnyView::new(
            Text::new(self.title.clone()).semantic_font(SemanticFontRef::label()),
        )];
        if self.is_sorted {
            row.push(AnyView::new(
                Text::new(match self.direction {
                    Some(TableSortDirection::Descending) => "↓",
                    _ => "↑",
                })
                .semantic_font(SemanticFontRef::caption()),
            ));
        }
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                6.0,
                AnyView::new(TupleView::new(row)),
            )
            .padding_edges(10.0, 0.0)
            .flex_with(1.0, 1.0, None),
        )]
    }
}
crate::impl_primitive_view!(TableHeaderCell);

/// One body cell.
struct TableCell {
    content: AnyView,
    width: f32,
    height: f32,
    alignment: TextAlignment,
    label: String,
}

impl PrimitiveView for TableCell {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Cell,
            label: self.label.clone(),
            ..Default::default()
        }));
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_width(Some(self.width));
        layout.set_height(Some(self.height));
        layout.set_flex_shrink(0.0);
        Some(layout)
    }

    fn update_layout(&self, layout: &cove_core::scene::LayoutNode) {
        layout.set_width(Some(self.width));
        layout.set_height(Some(self.height));
        layout.set_flex_shrink(0.0);
    }

    fn update_node(&self, node: &Node) {
        node.set_clips_to_bounds(true);
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let justify = match self.alignment {
            TextAlignment::Leading => Justify::FlexStart,
            TextAlignment::Trailing => Justify::FlexEnd,
            TextAlignment::Center => Justify::Center,
        };
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                justify,
                Wrap::NoWrap,
                0.0,
                self.content.clone(),
            )
            .padding_edges(10.0, 4.0)
            .clipped(),
        )]
    }
}
crate::impl_primitive_view!(TableCell);

// MARK: - Column plan

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
