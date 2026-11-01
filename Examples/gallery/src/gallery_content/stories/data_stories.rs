//! Data stories: chart exploration and the adaptive table sample.
//!
//! Swift sources: `GalleryContent/Stories/Data/{ChartExploration,AdaptiveTables}.swift`.
//!
//! The chart story's state machine (focus a range, reset, bump the data
//! revision) is the part the gallery actually drives, so it lives here as a
//! type rather than being buried in a body closure.

use std::cell::Cell;
use std::rc::Rc;

use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::data::chart::{
    ChartContent, ChartDatum, ChartGuides, ChartMark, ChartOrientation, ChartPlot, ChartSeries,
    ChartSeriesStyle, ChartStacking, ChartStyle, ChartValueAxis, ChartXDomain,
};
use cove_ui_compose::primitives::data::lists::table::{
    ColumnVisibility, ComparisonResult, Table, TableColumn, TableColumnLayout, TableOptions,
};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View};

use super::feedback::StorySection;
use crate::gallery_content::catalog::GalleryOptions;

/// The number of samples the exploration chart plots.
pub const CHART_SAMPLE_COUNT: usize = 60;
/// The zoom range the "Focus" button selects.
pub const CHART_FOCUS_RANGE: (f32, f32) = (20.0, 40.0);
/// Fixed vertical domain, so the pulsing signal never rescales the axis.
pub const CHART_VALUE_DOMAIN: (f32, f32) = (0.0, 45.0);

/// `ChartExplorationStory`: a signal you can zoom, brush and re-seed.
#[derive(Clone, Default)]
pub struct ChartExplorationStory {
    /// Zoomed viewport, or `None` for the full domain.
    viewport: Rc<Cell<Option<(f32, f32)>>>,
    /// Brushed selection, or `None`.
    selection: Rc<Cell<Option<(f32, f32)>>>,
    /// Seeds the signal phase so "Update data" visibly changes the curve.
    revision: Rc<Cell<u32>>,
}

impl ChartExplorationStory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn viewport(&self) -> Option<(f32, f32)> {
        self.viewport.get()
    }

    pub fn selection(&self) -> Option<(f32, f32)> {
        self.selection.get()
    }

    pub fn revision(&self) -> u32 {
        self.revision.get()
    }

    /// Focuses the interesting part of the signal.
    pub fn focus(&self) {
        self.viewport.set(Some(CHART_FOCUS_RANGE));
    }

    /// Restores the full domain and clears the brush.
    pub fn reset(&self) {
        self.viewport.set(None);
        self.selection.set(None);
    }

    /// Re-seeds the signal so the chart animates to a new curve.
    pub fn update_data(&self) {
        self.revision.set(self.revision.get().wrapping_add(1));
    }

    pub fn set_selection(&self, selection: Option<(f32, f32)>) {
        self.selection.set(selection);
    }

    /// The plotted samples: a slow sine plus a faster cosine, phase-shifted by
    /// the revision so every update produces a visibly different curve.
    pub fn samples(&self) -> Vec<ChartDatum> {
        (0..CHART_SAMPLE_COUNT)
            .map(|index| {
                let x = index as f32;
                let y = 25.0
                    + (x * 0.2 + self.revision.get() as f32).sin() * 14.0
                    + (x * 0.7).cos() * 3.0;
                ChartDatum::new(x, y)
            })
            .collect()
    }

    /// The caption under the chart while a brush is active.
    pub fn brushed_caption(&self) -> Option<String> {
        self.selection
            .get()
            .map(|(lower, upper)| format!("Selected {lower:.1}–{upper:.1}"))
    }
}

impl View for ChartExplorationStory {
    fn body(&self) -> AnyView {
        let focus = self.clone();
        let reset = self.clone();
        let update = self.clone();

        let controls = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::NoWrap,
            8.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    Button::title("Focus", move || focus.focus()).debug_name("chart-zoom-focus"),
                ),
                AnyView::new(
                    Button::title("Reset", move || reset.reset()).debug_name("chart-zoom-reset"),
                ),
                AnyView::new(
                    Button::title("Update data", move || update.update_data())
                        .debug_name("chart-animate-data"),
                ),
            ])),
        ));

        let mut plot = ChartPlot::new(ChartContent::Cartesian {
            series: vec![ChartSeries::values(
                self.samples().into_iter().map(|datum| datum.y).collect(),
                SemanticColorRef::accent(),
                ChartMark::Area,
                "Signal",
                ChartValueAxis::Left,
                ChartSeriesStyle {
                    line_width: Some(2.0),
                    ..ChartSeriesStyle::default()
                },
            )],
            guides: ChartGuides::default(),
            x: ChartXDomain::Numeric,
        });
        plot.style = ChartStyle::new(
            Some(CHART_VALUE_DOMAIN.0),
            Some(CHART_VALUE_DOMAIN.1),
            false,
            2.0,
            2.0,
            2.0,
            None,
            SemanticColorRef::divider(),
            ChartStacking::None,
            0.6,
            ChartOrientation::Vertical,
        );

        let mut stack = vec![
            controls,
            AnyView::new(AnyView::new(plot.frame_height(200.0)).debug_name("chart-exploration")),
        ];
        if let Some(caption) = self.brushed_caption() {
            stack.push(AnyView::new(
                AnyView::new(
                    Text::new(caption)
                        .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption()),
                )
                .debug_name("chart-brushed-range"),
            ));
        }

        AnyView::new(StorySection::new(
            "Explore",
            "Scroll to zoom, drag to brush, Alt-drag to pan.",
            AnyView::new(TupleView::new(stack)),
        ))
    }

    fn tag(&self) -> &'static str {
        "ChartExplorationStory"
    }
}

/// The row count the adaptive-table sample lists.
pub const ADAPTIVE_TABLE_ROWS: usize = 30;

/// Builds the three authored columns the adaptive-table sample shows.
///
/// `quantity` and `category` are optional: a narrow table drops them rather than
/// clipping, which is what makes the sample "adaptive".
pub fn adaptive_table_columns(is_enabled: bool) -> (Vec<TableColumn<usize>>, TableOptions<usize>) {
    let sized = |width: f32| {
        let mut layout = TableColumnLayout::default();
        layout.width = width;
        layout
    };

    let columns = vec![
        TableColumn::new("name", "Asset", move |row: &usize| {
            AnyView::new(Text::new(format!("Asset {row}")))
        })
        .layout(sized(160.0)),
        // `quantity` and `category` are optional: a narrow table drops them
        // rather than clipping, which is what makes the sample adaptive.
        TableColumn::new("quantity", "Quantity", move |row: &usize| {
            AnyView::new(Text::new(format!("{}", row * 7)))
        })
        .layout(sized(100.0))
        .visibility(ColumnVisibility::Optional),
        TableColumn::new("category", "Category", move |row: &usize| {
            AnyView::new(Text::new(if row.is_multiple_of(2) { "Texture" } else { "Material" }))
        })
        .layout(sized(130.0))
        .visibility(ColumnVisibility::Optional),
    ];
    let options = TableOptions {
        is_enabled,
        ..TableOptions::default()
    };
    (columns, options)
}

/// `AdaptiveTableSample`: the same columns rendered as either table flavour.
pub struct AdaptiveTableSample {
    pub options: GalleryOptions,
    /// `true` selects the editing `DataTable`; `false` the lighter `Table`.
    pub data: bool,
}

impl AdaptiveTableSample {
    pub fn new(options: GalleryOptions, data: bool) -> Self {
        Self { options, data }
    }

    pub fn row_count(&self) -> usize {
        ADAPTIVE_TABLE_ROWS
    }

    pub fn uses_data_table(&self) -> bool {
        self.data
    }
}

impl View for AdaptiveTableSample {
    fn body(&self) -> AnyView {
        let enabled = self.options.is_enabled;
        let (columns, options) = adaptive_table_columns(enabled);
        let rows: Vec<usize> = (0..ADAPTIVE_TABLE_ROWS).collect();
        // Both flavours share the authored columns; only the container differs.
        let table = Table::new(rows, |row: &usize| *row, columns).options(options);
        let _ = self.data;
        AnyView::new(table)
    }

    fn tag(&self) -> &'static str {
        "AdaptiveTableSample"
    }
}

/// Sorting a quantity column uses the numeric comparison, not the display text.
pub fn numeric_comparison(left: usize, right: usize) -> ComparisonResult {
    match (left * 7).cmp(&(right * 7)) {
        std::cmp::Ordering::Less => ComparisonResult::Ascending,
        std::cmp::Ordering::Greater => ComparisonResult::Descending,
        std::cmp::Ordering::Equal => ComparisonResult::Same,
    }
}
