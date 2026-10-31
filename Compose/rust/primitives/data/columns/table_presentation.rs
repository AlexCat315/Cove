//! Translation of Primitives/Data/Columns/TablePresentation.swift.
//!
//! One container query and one visibility plan for both table implementations,
//! so `Table` and `DataTable` agree on which columns are visible at a given
//! width.
//!
//! Behaviour checklist:
//! - the plan reserves the stable scrollbar lane before resolving, so a
//!   trailing column is never measured against space the bar occupies
//! - the picker model is only built when at least one column is optional; an
//!   all-required table shows no affordance
//! - the content receives the resolved plan and grows into the remaining space
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

use crate::adaptive::{ContainerSizeReader, SurfaceReader};
use crate::modifiers::layout::{FlexExt, LayoutFrameExt};
use crate::modifiers::style::SemanticStyleViewExt;
use crate::primitives::columns::{
    ColumnVisibilityPickerModel, ColumnVisibilityPlan, ColumnVisibilityTrack,
};
use crate::primitives::data::lists::table::{ColumnVisibility, TableColumn, TableColumnPlan};
use crate::primitives::layout::scroll_view::ScrollView as ScrollViewPrimitive;
use crate::{AnyView, BoxView, View};
use cove_core::Binding;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};

/// The shared columns track for one authored table, so the picker and the
/// planner read the same widths and priorities.
fn shared_tracks<Record>(
    columns: &[TableColumn<Record>],
    widths: &BTreeMap<String, f32>,
) -> Vec<ColumnVisibilityTrack> {
    TableColumnPlan::tracks(columns, widths)
        .into_iter()
        .map(|track| ColumnVisibilityTrack {
            id: track.id,
            title: track.title,
            visibility: match track.visibility {
                crate::primitives::layout::grid::ColumnVisibility::Required => {
                    crate::primitives::columns::ColumnVisibility::Required
                }
                crate::primitives::layout::grid::ColumnVisibility::Automatic { priority } => {
                    crate::primitives::columns::ColumnVisibility::Automatic { priority }
                }
            },
            width: track.width,
        })
        .collect()
}

/// Authored inputs shared by both table implementations.
pub struct TableColumns<Record: 'static> {
    pub columns: Vec<TableColumn<Record>>,
    pub widths: BTreeMap<String, f32>,
    pub overrides: Binding<HashMap<String, bool>>,
    pub frozen_count: usize,
    pub pinned_ids: HashSet<String>,
    pub is_enabled: bool,
    content: Rc<dyn Fn(TableColumnPlan) -> AnyView>,
}

impl<Record: 'static> TableColumns<Record> {
    pub fn new(
        columns: Vec<TableColumn<Record>>,
        widths: BTreeMap<String, f32>,
        overrides: Binding<HashMap<String, bool>>,
        content: impl Fn(TableColumnPlan) -> AnyView + 'static,
    ) -> Self {
        Self {
            columns,
            widths,
            overrides,
            frozen_count: 0,
            pinned_ids: HashSet::new(),
            is_enabled: true,
            content: Rc::new(content),
        }
    }

    pub fn frozen_count(mut self, frozen_count: usize) -> Self {
        self.frozen_count = frozen_count;
        self
    }

    pub fn pinned_ids(mut self, pinned_ids: HashSet<String>) -> Self {
        self.pinned_ids = pinned_ids;
        self
    }

    pub fn is_enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    /// Whether any column is optional, which is what makes a picker useful.
    pub fn shows_picker(&self) -> bool {
        self.columns
            .iter()
            .any(|column| column.visibility != ColumnVisibility::Required)
    }

    /// The resolved plan for `available_width`.
    pub fn plan(&self, available_width: f32) -> TableColumnPlan {
        TableColumnPlan::resolve(
            &self.columns,
            &self.widths,
            &self.overrides.get(),
            available_width.max(0.0),
            self.frozen_count,
            &self.pinned_ids,
        )
    }

    /// The picker model for the current plan, or `None` when nothing is
    /// optional.
    ///
    /// `TableColumnPlan::tracks` yields the grid's visibility tracks while the
    /// picker model consumes the shared columns contract, so the plan is
    /// re-expressed through `ColumnVisibilityPlan` here rather than duplicated.
    pub fn picker(&self, plan: &TableColumnPlan) -> Option<ColumnVisibilityPickerModel> {
        if !self.shows_picker() {
            return None;
        }
        let tracks = shared_tracks(&self.columns, &self.widths);
        let shared = ColumnVisibilityPlan {
            indices: plan.indices.clone(),
            required_ids: plan.required_ids.clone(),
        };
        Some(ColumnVisibilityPickerModel::new(
            &tracks,
            &shared,
            &self.overrides.get(),
            self.is_enabled,
            false,
        ))
    }
}

impl<Record: 'static> View for TableColumns<Record> {
    fn body(&self) -> AnyView {
        let columns = self.columns.clone();
        let widths = self.widths.clone();
        let overrides = self.overrides.clone();
        let frozen = self.frozen_count;
        let pinned = self.pinned_ids.clone();
        let content = self.content.clone();
        let shows_picker = self.shows_picker();
        let is_enabled = self.is_enabled;

        AnyView::new(SurfaceReader::new(
            move |environment: &cove_core::Environment| {
                let columns = columns.clone();
                let widths = widths.clone();
                let overrides = overrides.clone();
                let pinned = pinned.clone();
                let content = content.clone();
                let touch = environment
                    .input
                    .contains(cove_core::InputCapabilities::TOUCH);
                AnyView::new(ContainerSizeReader::new(move |size: cove_core::Size| {
                    let gutter = if touch {
                        0.0
                    } else {
                        ScrollViewPrimitive::scrollbar_gutter_width()
                    };
                    let plan = TableColumnPlan::resolve(
                        &columns,
                        &widths,
                        &overrides.get(),
                        (size.width - gutter).max(0.0),
                        frozen,
                        &pinned,
                    );
                    let picker = shows_picker.then(|| {
                        let tracks = shared_tracks(&columns, &widths);
                        let shared = ColumnVisibilityPlan {
                            indices: plan.indices.clone(),
                            required_ids: plan.required_ids.clone(),
                        };
                        ColumnVisibilityPickerModel::new(
                            &tracks,
                            &shared,
                            &overrides.get(),
                            is_enabled,
                            false,
                        )
                    });

                    let mut children = Vec::new();
                    if let Some(picker) = picker {
                        // The menu host for these entries is the toolbar command
                        // surface; the model is the shared contract both tables read.
                        let summary = picker.summary.clone();
                        children.push(AnyView::new(
                            AnyView::new(
                                crate::primitives::text::Text::new(summary)
                                    .semantic_font(crate::theme::SemanticFontRef::caption())
                                    .semantic_foreground(
                                        crate::theme::SemanticColorRef::on_surface_muted(),
                                    ),
                            )
                            .flex_with(0.0, 0.0, None),
                        ));
                    }
                    children.push(AnyView::new(
                        AnyView::new(content(plan))
                            .flex_with(1.0, 1.0, Some(0.0))
                            .frame(None, None, Some(0.0), Some(0.0), None, None),
                    ));

                    AnyView::new(
                        AnyView::new(
                            AnyView::new(BoxView::new(
                                FlexDirection::Column,
                                Align::Stretch,
                                Justify::FlexStart,
                                Wrap::NoWrap,
                                0.0,
                                AnyView::new(crate::TupleView::new(children)),
                            ))
                            .flex_with(1.0, 1.0, Some(0.0))
                            .frame(
                                None,
                                None,
                                Some(0.0),
                                Some(0.0),
                                None,
                                None,
                            ),
                        )
                        .flex_with(1.0, 1.0, None),
                    )
                }))
            },
        ))
    }
}
