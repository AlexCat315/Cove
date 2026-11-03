//! Translation of Primitives/Data/Chart/ChartDataTable.swift.
//!
//! A chart's data as a table: one row per x position, one column per series.
//! The plot is a picture of this table, and a picture is the one thing a screen
//! reader cannot describe, so the container carries the table role, each row a
//! row role and each value a cell role.
//!
//! Behaviour checklist:
//! - `maximum_rows` bounds the nodes it creates; past that the table says how
//!   many rows it left out rather than growing without limit
//! - a numeric x axis with several series pairs by each series' own x values,
//!   so those charts list one datum per row
//! - a categorical x axis gives one row per position with one cell per series
//! - a missing or non-finite value reads as a gap, not a zero

use crate::Scope;
use std::rc::Rc;
use std::sync::Arc;

use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, PaddingExt};
use crate::modifiers::style::SemanticStyleViewExt;
use crate::primitives::data::chart::{ChartSeries, ChartSlice, ChartXDomain, Selection};
use crate::primitives::text::TextAlignment;
use crate::primitives::text::{Divider, Text};
use crate::theme::{SemanticColorRef, SemanticFontRef};
use crate::{AnyView, BoxView, TupleView, View};
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, Align, FlexDirection, Justify, Wrap,
};

/// The gap marker for a value that was never measured, or a slot the series
/// does not reach.
pub const CHART_DATA_GAP: &str = "—";

/// One row: what it is about, and one cell per column.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartDataTableEntry {
    pub label: String,
    pub cells: Vec<String>,
}

/// A chart's data as a table.
pub struct ChartDataTable {
    label: String,
    axis_label: String,
    columns: Vec<String>,
    rows: Vec<ChartDataTableEntry>,
    /// Rows the table did not have room for.
    omitted: usize,
}

impl ChartDataTable {
    /// Series over an evenly spaced or numeric x axis.
    ///
    /// A numeric x domain with several series pairs rows by the first series'
    /// own x values, so values that were never measured together are not
    /// placed in one row.
    pub fn new(
        series: Vec<ChartSeries>,
        x: ChartXDomain,
        label: impl Into<String>,
        axis_label: impl Into<String>,
        maximum_rows: usize,
        format: Option<Arc<dyn Fn(f32) -> String + Send + Sync>>,
    ) -> Self {
        let text: Rc<dyn Fn(f32) -> String> = match format {
            Some(format) => Rc::new(move |value| format(value)),
            None => Rc::new(Selection::default_text),
        };
        let limit = maximum_rows.max(1);
        let shown: Vec<usize> = series
            .iter()
            .enumerate()
            .filter(|(_, series)| !series.data.is_empty())
            .map(|(index, _)| index)
            .collect();
        let label = label.into();

        if x.is_numeric() && shown.len() > 1 {
            let entries: Vec<ChartDataTableEntry> = shown
                .iter()
                .flat_map(|index| {
                    let text = text.clone();
                    let series = &series[*index];
                    series.data.iter().map(move |datum| ChartDataTableEntry {
                        label: Self::name(&series.title, *index),
                        cells: vec![
                            text(datum.x),
                            if datum.y.is_finite() {
                                text(datum.y)
                            } else {
                                CHART_DATA_GAP.to_string()
                            },
                        ],
                    })
                })
                .collect();
            let axis_label = axis_label.into();
            return Self {
                label,
                axis_label: if axis_label.is_empty() {
                    "Series".to_string()
                } else {
                    axis_label
                },
                columns: vec!["X".to_string(), "Value".to_string()],
                omitted: entries.len().saturating_sub(limit),
                rows: entries.into_iter().take(limit).collect(),
            };
        }

        let first = shown.first().and_then(|index| series.get(*index));
        let count = shown
            .iter()
            .filter_map(|index| series.get(*index).map(|series| series.data.len()))
            .max()
            .unwrap_or(0);
        let columns: Vec<String> = shown
            .iter()
            .map(|index| Self::name(&series[*index].title, *index))
            .collect();
        let rows: Vec<ChartDataTableEntry> = (0..count.min(limit))
            .map(|position| ChartDataTableEntry {
                label: Self::position(&x, position, first, &text),
                cells: shown
                    .iter()
                    .map(|index| match series[*index].data.get(position) {
                        Some(datum) if datum.y.is_finite() => text(datum.y),
                        _ => CHART_DATA_GAP.to_string(),
                    })
                    .collect(),
            })
            .collect();

        Self {
            label,
            axis_label: axis_label.into(),
            columns,
            rows,
            omitted: count.saturating_sub(limit),
        }
    }

    /// Pie and donut slices, with each slice's share of the total.
    pub fn slices(
        slices: Vec<ChartSlice>,
        label: impl Into<String>,
        maximum_rows: usize,
        format: Option<Arc<dyn Fn(f32) -> String + Send + Sync>>,
    ) -> Self {
        let text: Rc<dyn Fn(f32) -> String> = match format {
            Some(format) => Rc::new(move |value| format(value)),
            None => Rc::new(Selection::default_text),
        };
        let limit = maximum_rows.max(1);
        let total: f32 = slices
            .iter()
            .map(|slice| {
                if slice.value.is_finite() {
                    slice.value.max(0.0)
                } else {
                    0.0
                }
            })
            .sum();
        let rows: Vec<ChartDataTableEntry> = slices
            .iter()
            .take(limit)
            .enumerate()
            .map(|(position, slice)| {
                let share = if total > 0.0 {
                    (slice.value.max(0.0) / total * 100.0).round() as i64
                } else {
                    0
                };
                ChartDataTableEntry {
                    label: if slice.title.is_empty() {
                        format!("Slice {}", position + 1)
                    } else {
                        slice.title.clone()
                    },
                    cells: vec![
                        text(slice.value),
                        if total > 0.0 {
                            format!("{share}%")
                        } else {
                            CHART_DATA_GAP.to_string()
                        },
                    ],
                }
            })
            .collect();

        Self {
            label: label.into(),
            axis_label: String::new(),
            columns: vec!["Value".to_string(), "Share".to_string()],
            rows,
            omitted: slices.len().saturating_sub(limit),
        }
    }

    pub fn rows(&self) -> &[ChartDataTableEntry] {
        &self.rows
    }

    pub fn columns(&self) -> &[String] {
        &self.columns
    }

    pub fn omitted(&self) -> usize {
        self.omitted
    }

    fn name(title: &str, index: usize) -> String {
        if title.is_empty() {
            format!("Series {}", index + 1)
        } else {
            title.to_string()
        }
    }

    /// What the x axis calls this position: the category name, or the value on
    /// a numeric axis.
    fn position(
        x: &ChartXDomain,
        index: usize,
        series: Option<&ChartSeries>,
        text: &Rc<dyn Fn(f32) -> String>,
    ) -> String {
        match x {
            ChartXDomain::Categorical(labels) => labels
                .get(index)
                .cloned()
                .unwrap_or_else(|| format!("{}", index + 1)),
            ChartXDomain::Numeric => match series.and_then(|series| series.data.get(index)) {
                Some(datum) => text(datum.x),
                None => format!("{}", index + 1),
            },
        }
    }
}

impl View for ChartDataTable {
    fn body(&self, _cx: &Scope) -> AnyView {
        let header = AnyView::new(
            AnyView::new(ChartDataTableRow {
                label: self.axis_label.clone(),
                columns: self.columns.clone(),
                cells: self.columns.clone(),
                is_header: true,
            })
            .semantic_background(SemanticColorRef::surface_variant()),
        );

        let mut children = vec![header, AnyView::new(Divider::horizontal())];
        for row in &self.rows {
            children.push(AnyView::new(ChartDataTableRow {
                label: row.label.clone(),
                columns: self.columns.clone(),
                cells: row.cells.clone(),
                is_header: false,
            }));
        }
        if self.omitted > 0 {
            let noun = if self.omitted == 1 { "row" } else { "rows" };
            children.push(AnyView::new(
                AnyView::new(AnyView::new(
                    AnyView::new(
                        Text::new(format!("{} more {noun} not shown", self.omitted))
                            .semantic_font(SemanticFontRef::caption())
                            .semantic_foreground(SemanticColorRef::on_surface_muted()),
                    )
                    .padding_edges(8.0, 6.0),
                ))
                .accessibility(|semantics: &mut AccessibilitySemantics| {
                    semantics.role = AccessibilityRole::Status;
                }),
            ));
        }

        let label = self.label.clone();
        AnyView::new(
            AnyView::new(
                AnyView::new(BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(TupleView::new(children)),
                ))
                .accessibility(move |semantics: &mut AccessibilitySemantics| {
                    semantics.role = AccessibilityRole::Table;
                    semantics.label = label.clone();
                }),
            )
            .with_modifier(cove_composition::LayoutDebugModifier::debug_name(
                "chart-data-table",
            )),
        )
    }
}

/// One table row: the label column, then a cell per series. The header reuses
/// the same row so the columns line up without a second layout.
struct ChartDataTableRow {
    label: String,
    /// One name per value column, so a cell can say which series it came from.
    columns: Vec<String>,
    cells: Vec<String>,
    is_header: bool,
}

impl View for ChartDataTableRow {
    fn body(&self, _cx: &Scope) -> AnyView {
        let mut items: Vec<(String, Option<String>)> = vec![(self.label.clone(), None)];
        for (index, cell) in self.cells.iter().enumerate() {
            items.push((cell.clone(), self.columns.get(index).cloned()));
        }
        let cells: Vec<AnyView> = items
            .into_iter()
            .map(|(text, column)| {
                AnyView::new(ChartDataTableCell {
                    text,
                    column,
                    is_header: self.is_header,
                })
            })
            .collect();

        let label = self.label.clone();
        let is_header = self.is_header;
        AnyView::new(
            AnyView::new(
                AnyView::new(BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    8.0,
                    AnyView::new(TupleView::new(cells)),
                ))
                .padding_edges(8.0, 5.0),
            )
            .accessibility(move |semantics: &mut AccessibilitySemantics| {
                semantics.role = AccessibilityRole::Row;
                semantics.label = if is_header {
                    "Columns".to_string()
                } else {
                    label.clone()
                };
            }),
        )
    }
}

/// A header cell is just a heading; a body cell is a value that names the
/// column it came from, which is what a screen reader reads before it.
struct ChartDataTableCell {
    text: String,
    /// The series this cell belongs to. `None` for the label column.
    column: Option<String>,
    is_header: bool,
}

impl View for ChartDataTableCell {
    fn body(&self, _cx: &Scope) -> AnyView {
        let alignment = if self.column.is_none() {
            TextAlignment::Leading
        } else {
            TextAlignment::Trailing
        };
        let grow = if self.column.is_none() { 0.0 } else { 1.0 };
        let _ = grow;
        let text = AnyView::new(
            AnyView::new(
                Text::new(self.text.clone())
                    .alignment(alignment)
                    .line_limit(Some(1))
                    .semantic_font(if self.is_header {
                        SemanticFontRef::label()
                    } else {
                        SemanticFontRef::caption()
                    })
                    .semantic_foreground(if self.is_header {
                        SemanticColorRef::on_surface()
                    } else {
                        SemanticColorRef::on_surface_variant()
                    }),
            )
            .flex_with(
                if self.column.is_none() { 1.2 } else { 1.0 },
                1.0,
                Some(0.0),
            ),
        );

        if self.is_header {
            return text;
        }
        let Some(column) = self.column.clone() else {
            return text;
        };
        let value = self.text.clone();
        AnyView::new(AnyView::new(text).accessibility(
            move |semantics: &mut AccessibilitySemantics| {
                semantics.role = AccessibilityRole::Cell;
                semantics.label = column.clone();
                semantics.value = value.clone();
            },
        ))
    }
}
