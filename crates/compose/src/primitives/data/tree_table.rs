//! Translation of Primitives/Data/TreeTable/TreeTable.swift.
//!
//! A `DataTable` whose first column carries the hierarchy: indentation and a
//! disclosure button. Columns, widths, priorities, ordering, virtualization and
//! keyboard grid navigation are `DataTable`'s. With the first column focused,
//! Right expands (or moves to the first child) and Left collapses (or moves to
//! the parent), as in a tree. Sorting and cell editing are not offered:
//! siblings keep their authored order.
//!
//! Behaviour checklist:
//! - children of collapsed branches are never visited, so flattening cost
//!   follows the visible rows
//! - at least one column must be authored and ids must be unique
//! - Right on a collapsed branch expands it; on an expanded branch it focuses
//!   the first child. Left collapses an expanded branch, else focuses the parent
//! - the disclosure slot is only drawn for rows with children
//! - the table model is rebuilt only when the projection or expansion changed

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::rc::Rc;

use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt};
use crate::primitives::controls::button::{Button, ButtonIcon, ButtonIconSource};
use crate::primitives::data::data_table::{
    DataTable, DataTableModel, DataTableNavigation, DataTableOptions, DataTableSelection,
    DataTableSelectionMode,
};
use crate::primitives::data::lists::table::{
    ColumnVisibility, TableColumn, TableColumnLayout, TableLayout,
};
use crate::primitives::media::ui_icon_catalog;
use crate::theme::TextScalerReader;
use crate::theme::styles::buttons::{ButtonRole, ButtonStyleViewExt};
use crate::{AnyView, BoxView, TupleView, View};
use cove_composition::Scancode;
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, Align, FlexDirection, Justify, Wrap,
};
use cove_core::{Binding, State};
use cove_platform::KeyEvent;

/// One visible line of a `TreeTable`: the caller's record plus its place in the
/// hierarchy. Rows are rebuilt from the roots whenever expansion changes.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeTableRow<Record> {
    pub record: Record,
    pub depth: usize,
    pub has_children: bool,
    pub is_expanded: bool,
}

impl<Record> TreeTableRow<Record> {
    pub fn new(record: Record, depth: usize, has_children: bool, is_expanded: bool) -> Self {
        Self {
            record,
            depth,
            has_children,
            is_expanded,
        }
    }
}

/// Authored options for a `TreeTable`.
pub struct TreeTableOptions<Id: Ord + Clone> {
    /// Expanded branch ids; session-local when `None`.
    pub expanded: Option<Binding<BTreeSet<Id>>>,
    pub selection: Option<Binding<DataTableSelection<Id>>>,
    pub selection_mode: DataTableSelectionMode,
    pub column_widths: Option<Binding<BTreeMap<String, f32>>>,
    pub column_visibility: Option<Binding<HashMap<String, bool>>>,
    pub column_order: Option<Binding<Vec<String>>>,
    pub layout: TableLayout,
    pub is_enabled: bool,
    /// Indentation per depth level of the first column, before text scaling.
    pub indentation: f32,
}

impl<Id: Ord + Clone> Default for TreeTableOptions<Id> {
    fn default() -> Self {
        Self {
            expanded: None,
            selection: None,
            selection_mode: DataTableSelectionMode::Single,
            column_widths: None,
            column_visibility: None,
            column_order: None,
            layout: TableLayout::default(),
            is_enabled: true,
            indentation: 16.0,
        }
    }
}

impl<Id: Ord + Clone> TreeTableOptions<Id> {
    pub fn new() -> Self {
        Self::default()
    }
}

/// One authored column before it is mapped onto the row projection.
pub struct TreeTableColumn<Record> {
    pub id: String,
    pub title: String,
    pub layout: TableColumnLayout,
    pub visibility: ColumnVisibility,
    content: Rc<dyn Fn(&Record) -> AnyView>,
}

impl<Record> Clone for TreeTableColumn<Record> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            title: self.title.clone(),
            layout: self.layout,
            visibility: self.visibility,
            content: self.content.clone(),
        }
    }
}

impl<Record> TreeTableColumn<Record> {
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

    pub fn cell(&self, record: &Record) -> AnyView {
        (self.content)(record)
    }
}

/// Depth-first projection of expanded branches.
///
/// Children of collapsed branches are never visited, so cost follows the
/// visible rows rather than the whole forest.
pub fn flatten_tree_table<Record, Id>(
    roots: &[Record],
    children: &dyn Fn(&Record) -> Option<Vec<Record>>,
    id_of: &dyn Fn(&Record) -> Id,
    expanded: &BTreeSet<Id>,
) -> Vec<TreeTableRow<Record>>
where
    Record: Clone,
    Id: Ord + Clone,
{
    fn visit<Record, Id>(
        records: &[Record],
        depth: usize,
        children: &dyn Fn(&Record) -> Option<Vec<Record>>,
        id_of: &dyn Fn(&Record) -> Id,
        expanded: &BTreeSet<Id>,
        result: &mut Vec<TreeTableRow<Record>>,
    ) where
        Record: Clone,
        Id: Ord + Clone,
    {
        for record in records {
            let kids = children(record);
            let is_expanded = kids.is_some() && expanded.contains(&id_of(record));
            result.push(TreeTableRow::new(
                record.clone(),
                depth,
                kids.is_some(),
                is_expanded,
            ));
            if is_expanded && let Some(kids) = kids {
                visit(&kids, depth + 1, children, id_of, expanded, result);
            }
        }
    }
    let mut result = Vec::new();
    visit(roots, 0, children, id_of, expanded, &mut result);
    result
}

/// A `DataTable` whose first column carries the hierarchy.
pub struct TreeTable<Record, Id>
where
    Record: Clone + 'static,
    Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static,
{
    roots: Vec<Record>,
    id_of: Rc<dyn Fn(&Record) -> Id>,
    children: Rc<dyn Fn(&Record) -> Option<Vec<Record>>>,
    columns: Vec<TreeTableColumn<Record>>,
    pub options: TreeTableOptions<Id>,
    pub on_activate: Option<Rc<dyn Fn(&Record)>>,
    local_expanded: State<BTreeSet<Id>>,
    local_selection: State<DataTableSelection<Id>>,
    navigation: State<DataTableNavigation<Id>>,
    model: RefCell<
        Option<(
            Vec<Id>,
            Vec<bool>,
            Rc<DataTableModel<TreeTableRow<Record>, Id>>,
        )>,
    >,
}

impl<Record, Id> TreeTable<Record, Id>
where
    Record: Clone + 'static,
    Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static,
{
    pub fn new(
        roots: Vec<Record>,
        id_of: impl Fn(&Record) -> Id + 'static,
        children: impl Fn(&Record) -> Option<Vec<Record>> + 'static,
        columns: Vec<TreeTableColumn<Record>>,
    ) -> Self {
        assert!(
            !columns.is_empty(),
            "TreeTable requires at least one column"
        );
        let mut seen = HashSet::new();
        for column in &columns {
            assert!(
                seen.insert(column.id.clone()),
                "TreeTable column IDs must be unique"
            );
        }
        Self {
            roots,
            id_of: Rc::new(id_of),
            children: Rc::new(children),
            columns,
            options: TreeTableOptions::default(),
            on_activate: None,
            local_expanded: State::new(BTreeSet::new()),
            local_selection: State::new(DataTableSelection::new()),
            navigation: State::new(DataTableNavigation::new()),
            model: RefCell::new(None),
        }
    }

    pub fn options(mut self, options: TreeTableOptions<Id>) -> Self {
        self.options = options;
        self
    }

    pub fn on_activate(mut self, handler: impl Fn(&Record) + 'static) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }

    /// The caller's expansion when bound, else this view's session-local set.
    pub fn expanded(&self) -> BTreeSet<Id> {
        match &self.options.expanded {
            Some(binding) => binding.get(),
            None => self.local_expanded.get(),
        }
    }

    fn selection_binding(&self) -> Binding<DataTableSelection<Id>> {
        match &self.options.selection {
            Some(binding) => binding.clone(),
            None => self.local_selection.binding(),
        }
    }

    /// Writes a new expansion set to whichever store owns it.
    pub fn set_expanded(&self, next: BTreeSet<Id>) {
        match &self.options.expanded {
            Some(binding) => binding.set(next),
            None => self.local_expanded.set(next),
        }
    }

    pub fn toggle(&self, id: &Id) {
        let mut next = self.expanded();
        if !next.remove(id) {
            next.insert(id.clone());
        }
        self.set_expanded(next);
    }

    /// Rebuilds the model only when the visible projection or the expansion
    /// signature changed.
    fn model(&self) -> Rc<DataTableModel<TreeTableRow<Record>, Id>> {
        let rows = flatten_tree_table(&self.roots, &*self.children, &*self.id_of, &self.expanded());
        let ids: Vec<Id> = rows.iter().map(|row| (self.id_of)(&row.record)).collect();
        let states: Vec<bool> = rows.iter().map(|row| row.is_expanded).collect();
        if let Some((signature, expansion, model)) = &*self.model.borrow()
            && *signature == ids
            && *expansion == states
        {
            return model.clone();
        }
        let id_of = self.id_of.clone();
        let model = DataTableModel::new(rows, move |row: &TreeTableRow<Record>| id_of(&row.record));
        *self.model.borrow_mut() = Some((ids, states, model.clone()));
        model
    }

    /// The first column's cell: indentation, disclosure button, then content.
    fn mapped_columns(&self) -> Vec<TableColumn<TreeTableRow<Record>>> {
        let indentation = self.options.indentation;
        let expanded_binding = self.options.expanded.clone();
        let fallback = self.local_expanded.clone();
        let id_of = self.id_of.clone();

        self.columns
            .iter()
            .enumerate()
            .map(|(index, column)| {
                let content = column.content.clone();
                let mapped: TableColumn<TreeTableRow<Record>> = if index == 0 {
                    let toggle_expanded = expanded_binding.clone();
                    let toggle_fallback = fallback.clone();
                    let toggle_id_of = id_of.clone();
                    TableColumn::new(
                        column.id.clone(),
                        column.title.clone(),
                        move |row: &TreeTableRow<Record>| {
                            let depth = row.depth;
                            let has_children = row.has_children;
                            let is_expanded = row.is_expanded;
                            let record = row.record.clone();
                            let toggle_id = toggle_id_of(&row.record);

                            let toggle: Rc<dyn Fn()> = {
                                let expanded_binding = toggle_expanded.clone();
                                let fallback = toggle_fallback.clone();
                                let toggle_id = toggle_id.clone();
                                Rc::new(move || {
                                    let mut next = match &expanded_binding {
                                        Some(binding) => binding.get(),
                                        None => fallback.get(),
                                    };
                                    if !next.remove(&toggle_id) {
                                        next.insert(toggle_id.clone());
                                    }
                                    match &expanded_binding {
                                        Some(binding) => binding.set(next),
                                        None => fallback.set(next),
                                    }
                                })
                            };

                            let content = content.clone();
                            AnyView::new(TextScalerReader::new(
                                move |scaler: crate::theme::TextScaler| {
                                    let indent = scaler.scale(indentation) * (depth.min(12) as f32);
                                    let mut children = vec![AnyView::new(
                                        AnyView::new(crate::EmptyView)
                                            .frame_width(indent.max(0.0))
                                            .flex_with(0.0, 0.0, None),
                                    )];
                                    if has_children {
                                        let icon = if is_expanded {
                                            ButtonIconSource::resource(
                                                ui_icon_catalog::navigation::chevron_down(),
                                            )
                                        } else {
                                            ButtonIconSource::resource(
                                                ui_icon_catalog::navigation::chevron_right(),
                                            )
                                        };
                                        let label = if is_expanded { "Collapse" } else { "Expand" };
                                        let action = toggle.clone();
                                        children.push(
                                            AnyView::new(
                                                AnyView::new(
                                                    Button::new(
                                                        ButtonIcon::new(icon, Some(12.0), None),
                                                        move || action(),
                                                    )
                                                    .role(ButtonRole::Normal),
                                                )
                                                .button_style(
                                                    crate::theme::styles::buttons::secondary(),
                                                )
                                                .flex_with(0.0, 0.0, None)
                                                .accessibility({
                                                    let label = label.to_string();
                                                    move |semantics: &mut AccessibilitySemantics| {
                                                        semantics.label = label.clone();
                                                        semantics.state.is_expanded =
                                                            Some(is_expanded);
                                                    }
                                                }),
                                            ),
                                        );
                                    } else {
                                        children.push(AnyView::new(
                                            AnyView::new(crate::EmptyView)
                                                .frame_width(20.0)
                                                .flex_with(0.0, 0.0, None),
                                        ));
                                    }
                                    children.push(AnyView::new(
                                        AnyView::new(BoxView::new(
                                            FlexDirection::Row,
                                            Align::Center,
                                            Justify::FlexStart,
                                            Wrap::NoWrap,
                                            0.0,
                                            (content)(&record),
                                        ))
                                        .flex_with(1.0, 1.0, Some(0.0))
                                        .frame(
                                            None,
                                            None,
                                            Some(0.0),
                                            None,
                                            None,
                                            None,
                                        ),
                                    ));
                                    AnyView::new(BoxView::new(
                                        FlexDirection::Row,
                                        Align::Center,
                                        Justify::FlexStart,
                                        Wrap::NoWrap,
                                        2.0,
                                        AnyView::new(TupleView::new(children)),
                                    ))
                                },
                            ))
                        },
                    )
                } else {
                    TableColumn::new(
                        column.id.clone(),
                        column.title.clone(),
                        move |row: &TreeTableRow<Record>| (content)(&row.record),
                    )
                };
                let visibility = if index == 0 {
                    ColumnVisibility::Required
                } else {
                    column.visibility
                };
                mapped.layout(column.layout).visibility(visibility)
            })
            .collect()
    }
}

fn next_navigation_request_id() -> u64 {
    thread_local! { static NEXT: std::cell::Cell<u64> = const { std::cell::Cell::new(1) }; }
    NEXT.with(|next| {
        let value = next.get();
        next.set(value.wrapping_add(1));
        value
    })
}

impl<Record, Id> View for TreeTable<Record, Id>
where
    Record: Clone + 'static,
    Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static,
{
    fn body(&self) -> AnyView {
        let model = self.model();
        let mapped = self.mapped_columns();
        let options = &self.options;

        let mut table_options: DataTableOptions<Id> = DataTableOptions::new();
        table_options.selection = Some(self.selection_binding());
        table_options.selection_mode = options.selection_mode;
        table_options.column_widths = options.column_widths.clone();
        table_options.column_visibility = options.column_visibility.clone();
        table_options.column_order = options.column_order.clone();
        table_options.layout = options.layout.clone();
        table_options.is_enabled = options.is_enabled;
        table_options.frozen_column_count = 1;
        table_options.navigation = self.navigation.get();

        let mut table = DataTable::new(model.clone(), mapped).options(table_options);
        if let Some(handler) = &self.on_activate {
            let handler = handler.clone();
            table = table.on_activate(move |row: &TreeTableRow<Record>| handler(&row.record));
        }

        table.row_key_handler = Some(self.key_handler(model));
        AnyView::new(
            AnyView::new(table).accessibility(|semantics: &mut AccessibilitySemantics| {
                semantics.role = AccessibilityRole::Tree;
                semantics.label = "Tree table".to_string();
            }),
        )
    }

    fn dynamic_properties(&self) -> Vec<crate::DynamicField> {
        vec![
            crate::DynamicField::state("expanded", &self.local_expanded),
            crate::DynamicField::state("selection", &self.local_selection),
            crate::DynamicField::state("navigation", &self.navigation),
        ]
    }
}

impl<Record, Id> TreeTable<Record, Id>
where
    Record: Clone + 'static,
    Id: Ord + std::hash::Hash + Clone + Send + Sync + 'static,
{
    /// The row-navigation handler. It owns clones of every store it may write,
    /// so it cannot borrow `self` for the node's lifetime.
    fn key_handler(
        &self,
        model: Rc<DataTableModel<TreeTableRow<Record>, Id>>,
    ) -> Rc<dyn Fn(&KeyEvent, &Id) -> bool> {
        let is_enabled = self.options.is_enabled;
        let expanded_binding = self.options.expanded.clone();
        let fallback = self.local_expanded.clone();
        let selection = self.selection_binding();
        let navigation = self.navigation.clone();
        let first_column = self.columns.first().map(|column| column.id.clone());

        Rc::new(move |event: &KeyEvent, row_id: &Id| {
            if !is_enabled
                || event.modifiers.has_alt()
                || event.modifiers.has_gui()
                || event.modifiers.has_ctrl()
                || event.modifiers.has_shift()
            {
                return false;
            }
            let Some(index) = model.index_for(row_id) else {
                return false;
            };
            let Some(row) = model.record_at(index) else {
                return false;
            };

            let expanded = || match &expanded_binding {
                Some(binding) => binding.get(),
                None => fallback.get(),
            };
            let write_expanded = |next: BTreeSet<Id>| match &expanded_binding {
                Some(binding) => binding.set(next),
                None => fallback.set(next),
            };
            let toggle = |id: &Id| {
                let mut next = expanded();
                if !next.remove(id) {
                    next.insert(id.clone());
                }
                write_expanded(next);
            };
            let focus = |target: &Id| {
                let mut next_selection = selection.get();
                next_selection.selected_ids = BTreeSet::from([target.clone()]);
                next_selection.focused_id = Some(target.clone());
                next_selection.anchor_id = Some(target.clone());
                next_selection.focused_column_id = first_column.clone();
                selection.set(next_selection);

                let mut request = DataTableNavigation::new();
                request.request_id = Some(next_navigation_request_id());
                request.row_id = Some(target.clone());
                request.column_id = first_column.clone();
                navigation.set(request);
            };

            match event.scancode {
                Scancode::ARROW_RIGHT => {
                    if !row.has_children {
                        return false;
                    }
                    if !row.is_expanded {
                        toggle(row_id);
                        return true;
                    }
                    if index + 1 < model.count()
                        && let Some(next) = model.record_at(index + 1)
                        && next.depth > row.depth
                        && let Some(next_id) = model.row_id_at(index + 1)
                    {
                        focus(&next_id);
                    }
                    true
                }
                Scancode::ARROW_LEFT => {
                    if row.has_children && row.is_expanded {
                        toggle(row_id);
                        return true;
                    }
                    if row.depth == 0 {
                        return false;
                    }
                    let parent = (0..index).rev().find(|position| {
                        model
                            .record_at(*position)
                            .is_some_and(|other| other.depth < row.depth)
                    });
                    if let Some(parent) = parent
                        && let Some(parent_id) = model.row_id_at(parent)
                    {
                        focus(&parent_id);
                    }
                    true
                }
                _ => false,
            }
        })
    }
}
