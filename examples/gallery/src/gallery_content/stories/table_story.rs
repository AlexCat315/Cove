//! Translation of `GalleryContent/Stories/Data/DateTableStories.swift`'s
//! `TableStory`.
//!
//! The light table over ten thousand rows: fixed header, virtualized body,
//! synchronous sorting and resizable columns. Empty results keep the header.
use std::collections::{BTreeMap, HashMap};

use cove_composition::Scope;
use cove_core::State;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::data::data_table::table_column::{
    ColumnVisibility, ComparisonResult, TableColumn, TableColumnLayout, TableSort,
};
use cove_ui_compose::primitives::data::lists::{Table, TableOptions};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::SemanticFontRef;
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View};

use super::feedback::StorySection;
use crate::gallery_content::catalog::GalleryOptions;

/// How many inventory rows the sample builds, matching the Swift story.
pub const TABLE_RECORDS: usize = 10_000;

const CATEGORIES: [&str; 3] = ["Texture", "Material", "Mesh"];

/// One inventory row.
#[derive(Clone, Debug, PartialEq)]
pub struct InventoryRecord {
    pub id: usize,
    pub name: String,
    pub category: &'static str,
    pub stock: i32,
}

impl InventoryRecord {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            name: format!("Asset {}", id + 1),
            category: CATEGORIES[id % CATEGORIES.len()],
            stock: ((id * 17) % 250) as i32,
        }
    }

    /// The same records the Swift story builds.
    pub fn sample() -> Vec<Self> {
        (0..TABLE_RECORDS).map(Self::new).collect()
    }
}

/// `TableStory`: a large virtualized table plus its empty state.
pub struct TableStory {
    options: GalleryOptions,
    records: Vec<InventoryRecord>,
    selection: State<Option<usize>>,
    sort: State<Option<TableSort>>,
    widths: State<BTreeMap<String, f32>>,
    visibility: State<HashMap<String, bool>>,
    status: State<String>,
}

impl TableStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            records: InventoryRecord::sample(),
            selection: State::new(None),
            sort: State::new(None),
            widths: State::new(BTreeMap::new()),
            visibility: State::new(HashMap::new()),
            status: State::new("Choose a row or sort a column".to_string()),
        }
    }

    /// Sorting a stock column compares numbers, not the displayed text.
    pub fn stock_comparison(left: i32, right: i32) -> ComparisonResult {
        match left.cmp(&right) {
            std::cmp::Ordering::Less => ComparisonResult::Ascending,
            std::cmp::Ordering::Greater => ComparisonResult::Descending,
            std::cmp::Ordering::Equal => ComparisonResult::Same,
        }
    }

    /// The recorded selection, exposed so the gallery test can assert on it.
    pub fn selection(&self) -> Option<usize> {
        self.selection.get()
    }

    pub fn sort(&self) -> Option<TableSort> {
        self.sort.get()
    }

    pub fn row_count(&self) -> usize {
        self.records.len()
    }

    fn columns() -> Vec<TableColumn<InventoryRecord>> {
        let sized = |width: f32| {
            let mut layout = TableColumnLayout::default();
            layout.width = width;
            layout
        };
        vec![
            TableColumn::new("name", "Name", |record: &InventoryRecord| {
                AnyView::new(Text::new(record.name.clone()).semantic_font(SemanticFontRef::label()))
            })
            .layout(sized(220.0))
            .compare(|left: &InventoryRecord, right: &InventoryRecord| {
                match left.name.cmp(&right.name) {
                    std::cmp::Ordering::Less => ComparisonResult::Ascending,
                    std::cmp::Ordering::Greater => ComparisonResult::Descending,
                    std::cmp::Ordering::Equal => ComparisonResult::Same,
                }
            }),
            TableColumn::new("category", "Category", |record: &InventoryRecord| {
                AnyView::new(Text::new(record.category).semantic_font(SemanticFontRef::label()))
            })
            .layout(sized(180.0))
            .visibility(ColumnVisibility::Optional)
            .compare(|left: &InventoryRecord, right: &InventoryRecord| {
                match left.category.cmp(right.category) {
                    std::cmp::Ordering::Less => ComparisonResult::Ascending,
                    std::cmp::Ordering::Greater => ComparisonResult::Descending,
                    std::cmp::Ordering::Equal => ComparisonResult::Same,
                }
            }),
            TableColumn::new("stock", "Stock", |record: &InventoryRecord| {
                AnyView::new(
                    Text::new(record.stock.to_string()).semantic_font(SemanticFontRef::label()),
                )
            })
            .layout(sized(120.0))
            .visibility(ColumnVisibility::Optional)
            .compare(|left: &InventoryRecord, right: &InventoryRecord| {
                TableStory::stock_comparison(left.stock, right.stock)
            }),
        ]
    }

    fn options(&self) -> TableOptions<usize> {
        let mut options = TableOptions::default();
        options.selection = Some(cove_ui_compose::primitives::data::lists::Selection::single(
            self.selection.binding(),
        ));
        options.sort = Some(self.sort.binding());
        options.column_widths = Some(self.widths.binding());
        options.column_visibility = Some(self.visibility.binding());
        options.is_enabled = self.options.is_enabled;
        options
    }

    fn table(&self, records: Vec<InventoryRecord>) -> AnyView {
        let status = self.status.clone();
        let table = Table::new(
            records,
            |record: &InventoryRecord| record.id,
            Self::columns(),
        )
        .expect("the gallery table columns and rows are unique")
        .options(self.options())
        .on_activate(move |record: &InventoryRecord| {
            status.set(format!("Opened {}", record.name));
        });
        AnyView::new(
            AnyView::new(table)
                .frame_height(320.0)
                .debug_name("gallery-table"),
        )
    }

    fn inventory(&self) -> AnyView {
        let selected = match self.selection.get() {
            Some(id) => format!("Selected: {id}"),
            None => "Selected: None".to_string(),
        };
        AnyView::new(StorySection::new(
            "Inventory · 10,000 rows",
            "Narrow tables prioritize Stock over Category. Use Columns for \
             explicit visibility choices. Sort headers through ascending, \
             descending and source order; hidden sort columns retain their \
             ordering. Rows are virtualized.",
            AnyView::new(TupleView::new(vec![
                self.table(self.records.clone()),
                AnyView::new(BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    4.0,
                    AnyView::new(TupleView::new(vec![AnyView::new(
                        Text::new(format!("{} · {selected}", self.status.get()))
                            .semantic_font(SemanticFontRef::caption()),
                    )])),
                )),
            ])),
        ))
    }

    fn empty(&self) -> AnyView {
        AnyView::new(StorySection::new(
            "Empty data",
            "Empty results retain column headers and explain how to recover.",
            self.table(Vec::new()),
        ))
    }
}

impl View for TableStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            24.0,
            AnyView::new(TupleView::new(vec![self.inventory(), self.empty()])),
        ))
    }

    fn tag(&self) -> &'static str {
        "TableStory"
    }
}
