//! Translation of `Primitives/Data/Lists/Table.swift`.
//!
//! A fixed header and virtualized rows sharing one horizontal scroll
//! coordinate system. This is the light table: records arrive as a plain slice,
//! sorting is synchronous, and the mount owns its own session.
//!
//! Reach for `DataTable` instead when the data has to outlive a re-render, run
//! background sorting, freeze columns, reorder columns or edit cells.
//!
//! Behaviour checklist:
//! - a `compare` column cycles ascending, descending, then source order
//! - a sort on a hidden column keeps ordering the rows
//! - resized widths are constrained per column and survive a re-render
//! - a narrow table drops optional columns instead of clipping them
//! - empty results keep their headers and explain how to recover
//! - a selected row stays edge to edge so column boundaries read straight
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

use crate::Scope;
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::data::columns::resize_grip::RESIZE_GRIP_WIDTH;
use crate::primitives::data::columns::ResizeGrip;
use crate::primitives::data::data_table::model::sorted_order;
use crate::primitives::data::data_table::table_column::{
    ColumnVisibility, TableColumn, TableColumnPlan, TableLayout, TableSort, TableSortDirection,
};
use crate::primitives::data::lists::selection::{CollectionId, Selection};
use crate::primitives::feedback::EmptyState;
use crate::primitives::text::{Divider, Text, TextAlignment};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View, impl_primitive_view};
use cove_core::Binding;
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, AccessibilityState, Align, EventPhase, EventResult,
    FlexDirection, InputHandlerRoute, InteractionRegistryHolder, Justify, Node, PointerPhase, Wrap,
};

/// What the mount remembers while it is on screen.
#[derive(Clone)]
struct TableSession<Id> {
    widths: BTreeMap<String, f32>,
    visibility: HashMap<String, bool>,
    sort: Option<TableSort>,
    /// Only used when the caller supplied no `Selection`.
    selection: Option<Id>,
}

impl<Id> Default for TableSession<Id> {
    fn default() -> Self {
        Self {
            widths: BTreeMap::new(),
            visibility: HashMap::new(),
            sort: None,
            selection: None,
        }
    }
}

/// Groups digits the way a count is written in a toolbar: `1,000`, `200,000`.
pub(crate) fn grouped_count(count: usize) -> String {
    let digits = count.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// The leading toolbar slot: how much data the table is showing.
pub(crate) fn toolbar_count(count: usize) -> AnyView {
    AnyView::new(
        Text::new(format!("{} rows", grouped_count(count)))
            .semantic_font(crate::theme::SemanticFontRef::label())
            .semantic_foreground(crate::theme::SemanticColorRef::on_surface())
            .flex_with(0.0, 0.0, None),
    )
}

/// Authored inputs for a `Table`.
pub struct TableOptions<Id: CollectionId> {
    /// Rows the user can select. Session-local single selection when absent.
    pub selection: Option<Selection<Id>>,
    /// The current sort. Session-local when absent.
    pub sort: Option<Binding<Option<TableSort>>>,
    /// Resized widths. The mount keeps them when absent.
    pub column_widths: Option<Binding<BTreeMap<String, f32>>>,
    /// Per-column user choices: `true` forces shown, `false` hides.
    pub column_visibility: Option<Binding<HashMap<String, bool>>>,
    pub layout: TableLayout,
    pub is_enabled: bool,
}

impl<Id: CollectionId> Default for TableOptions<Id> {
    fn default() -> Self {
        let mut layout = TableLayout::default();
        layout.validate();
        Self {
            selection: None,
            sort: None,
            column_widths: None,
            column_visibility: None,
            layout,
            is_enabled: true,
        }
    }
}

impl<Id: CollectionId> TableOptions<Id> {
    pub fn new() -> Self {
        Self::default()
    }
}

/// A fixed header and virtualized rows over a plain record slice.
pub struct Table<Record: Clone + 'static, Id: CollectionId> {
    records: Rc<Vec<Record>>,
    row_id: Rc<dyn Fn(&Record) -> Id>,
    columns: Vec<TableColumn<Record>>,
    options: TableOptions<Id>,
    on_activate: Option<Rc<dyn Fn(&Record)>>,
    session: cove_core::State<TableSession<Id>>,
}

impl<Record: Clone + 'static, Id: CollectionId> Table<Record, Id> {
    /// A table over plain records.
    ///
    /// Column and row IDs must be unique: a duplicated id would let two rows
    /// claim the same selection.
    pub fn new(
        records: Vec<Record>,
        row_id: impl Fn(&Record) -> Id + 'static,
        columns: Vec<TableColumn<Record>>,
    ) -> Result<Self, crate::primitives::DuplicateId> {
        crate::primitives::check_ids(columns.iter().map(|column| &column.id), "Table column")?;
        let row_id = Rc::new(row_id);
        let mut seen = HashSet::new();
        for record in &records {
            if !seen.insert(row_id(record)) {
                return Err(crate::primitives::DuplicateId("Table row"));
            }
        }
        Ok(Self {
            records: Rc::new(records),
            row_id,
            columns,
            options: TableOptions::default(),
            on_activate: None,
            session: cove_core::State::new(TableSession::default()),
        })
    }

    pub fn options(mut self, options: TableOptions<Id>) -> Self {
        self.options = options;
        self.options.layout.validate();
        self
    }

    pub fn on_activate(mut self, handler: impl Fn(&Record) + 'static) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }

    /// The columns the caller authored, in declaration order.
    pub fn columns(&self) -> &[TableColumn<Record>] {
        &self.columns
    }

    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// The direction this column is sorted by, if it is the sort column.
    pub fn sort_direction(&self, column_id: &str) -> Option<TableSortDirection> {
        self.current_sort()
            .filter(|sort| sort.column_id == column_id)
            .map(|sort| sort.direction)
    }

    fn current_sort(&self) -> Option<TableSort> {
        match &self.options.sort {
            Some(binding) => binding.get(),
            None => self.session.get().sort,
        }
    }

    fn sort_binding(&self) -> Binding<Option<TableSort>> {
        match &self.options.sort {
            Some(binding) => binding.clone(),
            None => {
                let read = self.session.clone();
                let write = self.session.clone();
                Binding::new(
                    move || read.get().sort,
                    move |value| write.update(|state| state.sort = value),
                )
            }
        }
    }

    fn selection(&self) -> Selection<Id> {
        match &self.options.selection {
            Some(selection) => selection.clone(),
            None => {
                let read = self.session.clone();
                let write = self.session.clone();
                Selection::single(Binding::new(
                    move || read.get().selection,
                    move |value| write.update(|state| state.selection = value),
                ))
            }
        }
    }

    fn widths(&self) -> Binding<BTreeMap<String, f32>> {
        match &self.options.column_widths {
            Some(binding) => binding.clone(),
            None => {
                let read = self.session.clone();
                let write = self.session.clone();
                Binding::new(
                    move || read.get().widths,
                    move |value| write.update(|state| state.widths = value),
                )
            }
        }
    }

    fn overrides(&self) -> Binding<HashMap<String, bool>> {
        match &self.options.column_visibility {
            Some(binding) => binding.clone(),
            None => {
                let read = self.session.clone();
                let write = self.session.clone();
                Binding::new(
                    move || read.get().visibility,
                    move |value| write.update(|state| state.visibility = value),
                )
            }
        }
    }

    /// The width one column occupies, constrained to its authored bounds.
    fn column_width(&self, column: &TableColumn<Record>, stored: &BTreeMap<String, f32>) -> f32 {
        column.layout.constrained(
            stored
                .get(&column.id)
                .copied()
                .unwrap_or(column.layout.width),
        )
    }

    /// Ascending, descending, then back to source order.
    fn cycle_sort(&self, column_id: &str) {
        let binding = self.sort_binding();
        let next = match binding.get() {
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
            _ => Some(TableSort {
                column_id: column_id.to_string(),
                direction: TableSortDirection::Ascending,
            }),
        };
        binding.set(next);
    }

    /// The rows in the order they are drawn.
    ///
    /// The sort column is looked up among every authored column, so hiding the
    /// sorted column does not silently drop its ordering.
    fn ordered_records(&self) -> Vec<Record> {
        let Some(sort) = self.current_sort() else {
            return self.records.as_ref().clone();
        };
        let Some(compare) = self
            .columns
            .iter()
            .find(|column| column.id == sort.column_id)
            .and_then(|column| column.compare.clone())
        else {
            return self.records.as_ref().clone();
        };
        sorted_order(self.records.as_ref(), &sort, compare.as_ref())
            .into_iter()
            .filter_map(|index| self.records.get(index).cloned())
            .collect()
    }

    /// The columns a given width can show, and their constrained widths.
    fn plan(&self, width: f32) -> (Vec<(TableColumn<Record>, f32)>, TableColumnPlan) {
        let stored = self.widths().get();
        let widths: BTreeMap<String, f32> = self
            .columns
            .iter()
            .map(|column| (column.id.clone(), self.column_width(column, &stored)))
            .collect();
        let overrides = self.overrides().get();
        let plan = TableColumnPlan::resolve(
            &self.columns,
            &widths,
            &overrides,
            width,
            0,
            &HashSet::new(),
        );
        let presented: Vec<(TableColumn<Record>, f32)> = plan
            .indices
            .iter()
            .filter_map(|index| self.columns.get(*index))
            .map(|column| {
                let constrained = widths
                    .get(&column.id)
                    .copied()
                    .unwrap_or(column.layout.width);
                (column.clone(), constrained)
            })
            .collect();
        let presented = if self.options.layout.fills_width && !presented.is_empty() {
            let used: f32 = presented.iter().map(|(_, width)| *width).sum();
            let slack = width - used;
            let mut filled = presented;
            if slack > 0.5 {
                // A flexible column absorbs the unused width so the trailing
                // columns keep their authored size against the right edge.
                let target = filled
                    .iter()
                    .rposition(|(column, _)| column.layout.is_flex)
                    .unwrap_or(filled.len() - 1);
                if let Some((column, width)) = filled.get_mut(target) {
                    *width = column.layout.constrained(*width + slack);
                }
            }
            filled
        } else {
            presented
        };
        (presented, plan)
    }

    fn build(&self, width: f32) -> AnyView {
        let layout = self.options.layout.clone();
        let is_enabled = self.options.is_enabled;
        let (presented, plan) = self.plan(width);
        let records = Rc::new(self.ordered_records());
        let row_id = self.row_id.clone();

        let by_id: Rc<HashMap<Id, Record>> = Rc::new(
            records
                .iter()
                .map(|record| (row_id(record), record.clone()))
                .collect(),
        );
        let ids: Vec<Id> = records.iter().map(|record| row_id(record)).collect();
        // One pass up front: the row closure asks for its own index on every
        // rebuild, so scanning the record list per row would be quadratic.
        let index_by_id: Rc<HashMap<Id, usize>> = Rc::new(
            records
                .iter()
                .enumerate()
                .map(|(index, record)| (row_id(record), index))
                .collect(),
        );

        let list = if ids.is_empty() {
            AnyView::new(
                AnyView::new(EmptyState::new(
                    layout.empty_title.clone(),
                    "Change your filters or add a record.",
                ))
                .flex_with(1.0, 1.0, None),
            )
        } else {
            let columns = Rc::new(presented.clone());
            let lookup = by_id.clone();
            let indices = index_by_id.clone();
            let row_height = layout.row_height;
            let is_striped = layout.is_striped;
            let row_content = move |id: &Id, state: crate::primitives::data::lists::RowState| {
                let Some(record) = lookup.get(id) else {
                    return AnyView::new(crate::EmptyView);
                };
                let index = indices.get(id).copied().unwrap_or(0);
                let cells: Vec<AnyView> = columns
                    .iter()
                    .map(|(column, width)| {
                        AnyView::new(cell((column.content())(record), column, *width, row_height))
                    })
                    .collect();
                AnyView::new(row(cells, state.is_selected, is_striped && index % 2 == 1))
            };
            let mut list = crate::primitives::data::lists::List::new(ids.clone(), row_content)
                .selection(self.selection())
                .row_height(row_height)
                .row_spacing(0.0)
                // The table paints the card; the rows must not paint a second
                // one, and their columns must start where the header's do.
                .flush_rows(true)
                .separator_inset(layout.separator_inset.unwrap_or(0.0));
            if let Some(handler) = &self.on_activate {
                let handler = handler.clone();
                let lookup = by_id.clone();
                list = list.on_activate(move |id: &Id| {
                    if let Some(record) = lookup.get(id) {
                        handler(record);
                    }
                });
            }
            AnyView::new(list.flex_with(1.0, 1.0, Some(0.0)))
        };

        let header = self.header(&presented, &layout, is_enabled);
        let grid = AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(vec![
                    header,
                    AnyView::new(Divider::new()),
                    list,
                ])),
            )
            .flex_with(1.0, 1.0, None),
        );

        let mut children: Vec<AnyView> = Vec::new();
        if let Some(picker) = self.picker(&plan, is_enabled) {
            // The card's first row is a toolbar, not a caption strip: the
            // data set's size anchors the leading edge and the column action
            // the trailing one, on the same 20pt gutter the cells use. No rule
            // below it — the header already has its own band, and a rule there
            // would cut the card in half.
            children.push(AnyView::new(
                AnyView::new(
                    BoxView::new(
                        FlexDirection::Row,
                        Align::Center,
                        Justify::SpaceBetween,
                        Wrap::NoWrap,
                        8.0,
                        AnyView::new(TupleView::new(vec![toolbar_count(ids.len()), picker])),
                    )
                    .padding_edges(20.0, 0.0)
                    .frame_height(44.0),
                )
                .flex_with(0.0, 0.0, None),
            ));
        }
        children.push(grid);

        AnyView::new(
            AnyView::new(
                BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(TupleView::new(children)),
                )
                // The rows paint no fill of their own (flush rows are
                // transparent), so the card has to own the surface — otherwise
                // the table shows the region behind it.
                .semantic_background(crate::theme::SemanticColorRef::surface())
                .semantic_border(crate::theme::SemanticColorRef::border(), 1.0)
                .corner_radius_metric(crate::theme::ThemeMetric::group_radius()),
            )
            .opacity(if is_enabled { 1.0 } else { 0.55 })
            .accessibility(move |semantics: &mut AccessibilitySemantics| {
                semantics.role = AccessibilityRole::Table;
                semantics.state = AccessibilityState {
                    is_enabled,
                    ..Default::default()
                };
            })
            .flex_with(1.0, 1.0, None),
        )
    }

    fn header(
        &self,
        presented: &[(TableColumn<Record>, f32)],
        layout: &TableLayout,
        is_enabled: bool,
    ) -> AnyView {
        let cells: Vec<AnyView> = presented
            .iter()
            .map(|(column, width)| {
                let direction = self.sort_direction(&column.id);
                let sortable = column.compare.is_some() && is_enabled;
                // The grip lives *inside* the column's width, not beside it: a
                // 6pt sibling in the header row would make every header column
                // wider than the body column it labels, and the gap would grow
                // with each column.
                let grip = if column.layout.is_resizable {
                    RESIZE_GRIP_WIDTH
                } else {
                    0.0
                };
                let mut parts: Vec<AnyView> = vec![AnyView::new(header_cell(
                    column.title.clone(),
                    (*width - grip).max(0.0),
                    layout.header_height,
                    column.layout.alignment,
                    direction,
                    sortable,
                    {
                        let table = self.clone_handle();
                        let column_id = column.id.clone();
                        move || table.cycle_sort(&column_id)
                    },
                ))];
                if column.layout.is_resizable {
                    let stored = self.widths();
                    let column_id = column.id.clone();
                    let authored = column.layout;
                    let read = stored.clone();
                    let read_id = column_id.clone();
                    let write = stored.clone();
                    let write_id = column_id.clone();
                    let width_binding = Binding::new(
                        move || {
                            authored.constrained(
                                read.get().get(&read_id).copied().unwrap_or(authored.width),
                            )
                        },
                        move |value| {
                            let mut next = write.get();
                            next.insert(write_id.clone(), authored.constrained(value));
                            write.set(next);
                        },
                    );
                    parts.push(AnyView::new(ResizeGrip::new(
                        column.title.clone(),
                        width_binding,
                        authored,
                        is_enabled,
                    )));
                }
                AnyView::new(
                    BoxView::new(
                        FlexDirection::Row,
                        Align::Center,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        0.0,
                        AnyView::new(TupleView::new(parts)),
                    )
                    .frame_height(layout.header_height),
                )
            })
            .collect();
        AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(cells)),
            )
            // The header shares the card's fill and is separated by its rule
            // instead: a grey band here became a notch against the grey region
            // the card sits on.
            .semantic_background(crate::theme::SemanticColorRef::surface()),
        )
    }

    fn picker(&self, plan: &TableColumnPlan, is_enabled: bool) -> Option<AnyView> {
        if !self
            .columns
            .iter()
            .any(|column| column.visibility != ColumnVisibility::Required)
        {
            return None;
        }
        let stored = self.widths().get();
        let widths: BTreeMap<String, f32> = self
            .columns
            .iter()
            .map(|column| (column.id.clone(), self.column_width(column, &stored)))
            .collect();
        let tracks = crate::primitives::data::columns::picker_tracks(&TableColumnPlan::tracks(
            &self.columns,
            &widths,
        ));
        Some(AnyView::new(
            AnyView::new(crate::primitives::columns::ColumnVisibilityPicker::new(
                tracks,
                crate::primitives::columns::ColumnVisibilityPlan {
                    indices: plan.indices.clone(),
                    required_ids: plan.required_ids.clone(),
                },
                self.overrides(),
                is_enabled,
            ))
            .flex_with(0.0, 0.0, None),
        ))
    }

    /// A handle the header closures own; the session state is shared, so a
    /// click and the grid always agree.
    fn clone_handle(&self) -> Table<Record, Id> {
        Table {
            records: self.records.clone(),
            row_id: self.row_id.clone(),
            columns: self.columns.clone(),
            options: TableOptions {
                selection: self.options.selection.clone(),
                sort: self.options.sort.clone(),
                column_widths: self.options.column_widths.clone(),
                column_visibility: self.options.column_visibility.clone(),
                layout: self.options.layout.clone(),
                is_enabled: self.options.is_enabled,
            },
            on_activate: self.on_activate.clone(),
            session: self.session.clone(),
        }
    }
}

impl<Record: Clone + 'static, Id: CollectionId> Clone for Table<Record, Id> {
    fn clone(&self) -> Self {
        Self {
            records: self.records.clone(),
            row_id: self.row_id.clone(),
            columns: self.columns.clone(),
            options: TableOptions {
                selection: self.options.selection.clone(),
                sort: self.options.sort.clone(),
                column_widths: self.options.column_widths.clone(),
                column_visibility: self.options.column_visibility.clone(),
                layout: self.options.layout.clone(),
                is_enabled: self.options.is_enabled,
            },
            on_activate: self.on_activate.clone(),
            session: self.session.clone(),
        }
    }
}

impl<Record: Clone + 'static, Id: CollectionId> View for Table<Record, Id> {
    fn body(&self, _cx: &Scope) -> AnyView {
        // A resize or a sort rewrites the session, so the whole grid rebuilds
        // rather than each cell holding a stale width.
        let _ = self.session.get();
        let table = self.clone_handle();
        AnyView::new(crate::adaptive::ContainerSizeReader::new(move |size| {
            TableSurface {
                table: table.clone(),
                size,
            }
        }))
    }

    fn dynamic_properties(&self) -> Vec<crate::DynamicField> {
        vec![crate::DynamicField::state("table.session", &self.session)]
    }

    fn tag(&self) -> &'static str {
        "Table"
    }
}

/// The measured surface: one container query decides which columns fit.
struct TableSurface<Record: Clone + 'static, Id: CollectionId> {
    table: Table<Record, Id>,
    size: cove_core::Size,
}

impl<Record: Clone + 'static, Id: CollectionId> View for TableSurface<Record, Id> {
    fn body(&self, _cx: &Scope) -> AnyView {
        self.table.build(self.size.width)
    }

    fn tag(&self) -> &'static str {
        "TableSurface"
    }
}

/// One grid row: cells edge to edge so column boundaries read straight down.
fn row(cells: Vec<AnyView>, is_selected: bool, is_striped: bool) -> AnyView {
    let fill = if is_selected {
        Some(crate::theme::SemanticColorRef::accent())
    } else if is_striped {
        Some(crate::theme::SemanticColorRef::surface_variant())
    } else {
        None
    };
    let base = BoxView::new(
        FlexDirection::Row,
        Align::Center,
        Justify::FlexStart,
        Wrap::NoWrap,
        0.0,
        AnyView::new(TupleView::new(cells)),
    );
    match fill {
        Some(color) => AnyView::new(base.semantic_background(color)),
        None => AnyView::new(base),
    }
}

/// One cell: the authored content in the column's aligned slot.
fn cell<Record>(
    content: AnyView,
    column: &TableColumn<Record>,
    width: f32,
    row_height: f32,
) -> AnyView {
    let justify = match column.layout.alignment {
        TextAlignment::Leading => Justify::FlexStart,
        TextAlignment::Center => Justify::Center,
        TextAlignment::Trailing => Justify::FlexEnd,
    };
    let label = column.title.clone();
    AnyView::new(
        AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                justify,
                Wrap::NoWrap,
                0.0,
                content,
            )
            .padding_edges(20.0, 0.0)
            .frame_size(width, row_height),
        )
        .clipped()
        .accessibility(move |semantics: &mut AccessibilitySemantics| {
            semantics.role = AccessibilityRole::Cell;
            semantics.label = label.clone();
        }),
    )
}

/// A header cell: the title, its sort indicator and the sort action.
fn header_cell(
    title: String,
    width: f32,
    height: f32,
    alignment: cove_core::TextAlignment,
    direction: Option<TableSortDirection>,
    sortable: bool,
    sort: impl Fn() + 'static,
) -> AnyView {
    let indicator = match direction {
        Some(TableSortDirection::Ascending) => " \u{2191}",
        Some(TableSortDirection::Descending) => " \u{2193}",
        None => "",
    };
    // The label follows the column's alignment, so a numeric column's header
    // sits over its numbers instead of over the values' left edge.
    let justify = match alignment {
        cove_core::TextAlignment::Leading => Justify::FlexStart,
        cove_core::TextAlignment::Trailing => Justify::FlexEnd,
        cove_core::TextAlignment::Center => Justify::Center,
    };
    let content = AnyView::new(
        BoxView::new(
            FlexDirection::Row,
            Align::Center,
            justify,
            Wrap::NoWrap,
            6.0,
            AnyView::new(TupleView::new(vec![AnyView::new(
                AnyView::new(Text::new(format!("{title}{indicator}")))
                    // A column label is a label, not a value: semibold and one
                    // step back in contrast, so the rows read as the data.
                    .semantic_font(crate::theme::SemanticFontRef::body_strong())
                    .semantic_foreground(crate::theme::SemanticColorRef::on_surface_muted()),
            )])),
        )
        .padding_edges(20.0, 0.0)
        .frame_size(width, height),
    );
    AnyView::new(AnyView::new(HeaderButton {
        content,
        label: title,
        direction,
        sortable,
        sort: Rc::new(sort),
    }))
}

/// The header acts as a button: pointer and keyboard both cycle the sort.
struct HeaderButton {
    content: AnyView,
    label: String,
    direction: Option<TableSortDirection>,
    sortable: bool,
    sort: Rc<dyn Fn()>,
}

impl PrimitiveView for HeaderButton {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(self.sortable);
        node.set_is_focusable(self.sortable);
        node
    }

    fn update_node(&self, node: &Node) {
        // A sortable header is tapped; on touch the header row grows to the
        // target instead of the cell shrinking it.
        crate::primitives::controls::adaptive::apply_touch_control_extent(node, self.sortable);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Button,
            label: format!("Sort by {}", self.label),
            state: AccessibilityState {
                is_enabled: self.sortable,
                is_selected: self.direction.is_some(),
                ..Default::default()
            },
            ..Default::default()
        }));
        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        if !self.sortable {
            registry.remove(node);
            return;
        }
        let click = self.sort.clone();
        registry.set_pointer(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |_event, phase, event_phase| {
                if phase == PointerPhase::Up && event_phase == EventPhase::Target {
                    click();
                    return EventResult::Handled;
                }
                EventResult::Ignored
            }),
        );
        let key = self.sort.clone();
        registry.set_key(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if phase != EventPhase::Target
                    || event.is_repeat
                    || !matches!(
                        event.scancode,
                        crate::Scancode::RETURN
                            | crate::Scancode::SPACE
                            | crate::Scancode::KEYPAD_ENTER
                    )
                {
                    return EventResult::Ignored;
                }
                key();
                EventResult::Handled
            }),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(HeaderButton);
