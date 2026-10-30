//! Translation of Primitives/Text/Code/GutterLayout.swift.
//!
//! Turns a laid-out document into the gutter's painted controls. Rows that are
//! only partly visible still paint as text but never expose a clipped target,
//! so a control can only be hit where it is fully drawn.
//!
//! Behaviour checklist:
//! - one control set per physical source line, not per visual row
//! - the execution-line band paints across the row with a leading rule, plus an
//!   arrow in the breakpoint lane when no breakpoint list is bound
//! - a touch surface with any affordance gets one combined control per line
//! - a collapsed fold contributes a summary badge past the visible text, but
//!   only when the badge fits inside the field
use std::collections::BTreeSet;

use cove_core::{DrawList, Font, Point, Rect};
use crate::primitives::text::code::gutter_geometry::{
    TextGutterControl, TextGutterControlKind, TextGutterGeometry,
};
use crate::primitives::text::code::gutter_options::{TextBreakpointState, TextEditorGutter};
use crate::primitives::text::code::gutter_session::TextGutterSession;
use crate::theme::Theme;

/// One laid-out visual row, as the gutter addresses it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GutterRow {
    /// The physical source line this visual row belongs to.
    pub line: usize,
    /// Row top in field coordinates.
    pub top_y: f32,
    /// Painted width of the row's text.
    pub width: f32,
}

/// Everything the layout needs from the field, so it does not reach into the
/// view for ambient state.
pub struct TextGutterInputs<'a> {
    pub gutter: &'a TextEditorGutter,
    pub has_folds: bool,
    pub shows_line_numbers: bool,
    pub vertical: bool,
    pub secure: bool,
    pub has_active_composition: bool,
    pub theme: &'a Theme,
    pub font: &'a Font,
    pub line_height: f32,
    pub touch: bool,
    /// The field's own height, used to reject clipped controls.
    pub field_height: f32,
    pub field_width: f32,
}

/// The gutter's horizontal geometry for one field.
pub fn gutter_geometry(inputs: &TextGutterInputs<'_>, number_width: f32) -> TextGutterGeometry {
    TextGutterGeometry::new(
        inputs.gutter.is_enabled(),
        inputs.gutter.breakpoints.is_some(),
        inputs.gutter.execution_line.is_some(),
        inputs.has_folds,
        number_width,
        inputs.font,
        inputs.theme,
        inputs.touch,
    )
}

/// Repaints the gutter and publishes the controls a session should host.
///
/// `lines` are the document's visual rows; `line_index_for` maps a row's start
/// cluster to its physical source line. Returns the controls it published.
pub fn update_gutter_controls(
    session: &TextGutterSession,
    inputs: &TextGutterInputs<'_>,
    geometry: &TextGutterGeometry,
    rows: &[GutterRow],
    text_origin_x: f32,
    text_origin_y: f32,
    origin: Point,
    list: &mut DrawList,
) -> Vec<TextGutterControl> {
    if !inputs.shows_line_numbers
        || !inputs.vertical
        || inputs.secure
        || inputs.has_active_composition
        || !(inputs.gutter.is_enabled() || inputs.has_folds)
    {
        session.update(Vec::new());
        return Vec::new();
    }

    let x = text_origin_x - origin.x - geometry.width;
    let mut controls: Vec<TextGutterControl> = Vec::new();
    let mut last_line: Option<usize> = None;

    for row in rows {
        let line = row.line;
        if last_line == Some(line) {
            continue;
        }
        last_line = Some(line);
        let y = text_origin_y - origin.y + row.top_y;
        if y < 0.0 || y + inputs.line_height > inputs.field_height {
            continue;
        }

        if inputs.gutter.execution_line == Some(line) {
            let _ = list.add_rect(
                Rect::new(origin.x, origin.y + y, inputs.field_width, inputs.line_height),
                inputs.theme.colors.warning.multiplied_alpha(0.10),
            );
            let _ = list.add_rect(
                Rect::new(origin.x, origin.y + y, 2.0, inputs.line_height),
                inputs.theme.colors.warning.multiplied_alpha(0.8),
            );
            if inputs.gutter.breakpoints.is_none()
                && let Some(lane) = geometry.breakpoint_x
            {
                let arrow_x = origin.x + x + lane + 7.0;
                let arrow_y = origin.y + y + inputs.line_height / 2.0;
                let _ = list.add_triangle(
                    Point::new(arrow_x, arrow_y - 5.0),
                    Point::new(arrow_x + 9.0, arrow_y),
                    Point::new(arrow_x, arrow_y + 5.0),
                    inputs.theme.colors.warning,
                );
            }
        }

        let control_height = if geometry.combined {
            inputs.line_height
        } else {
            inputs.line_height.min(24.0)
        };
        let top = y + (inputs.line_height - control_height) / 2.0;
        let markers = session.markers_by_line();
        let has_markers = markers.get(&line).map(|m| !m.is_empty()).unwrap_or(false);
        let has_fold = session.folds().region(line).is_some();

        if geometry.combined
            && (inputs.gutter.breakpoints.is_some() || has_fold || has_markers)
        {
            controls.push(TextGutterControl::new(
                line,
                TextGutterControlKind::Combined,
                x,
                top,
                44.0,
                control_height,
                0,
            ));
        } else {
            if let Some(lane) = geometry.breakpoint_x
                && inputs.gutter.breakpoints.is_some()
            {
                controls.push(TextGutterControl::new(
                    line,
                    TextGutterControlKind::Breakpoint,
                    x + lane,
                    top,
                    24.0,
                    control_height,
                    0,
                ));
            }
            if let Some(lane) = geometry.marker_x
                && has_markers
            {
                controls.push(TextGutterControl::new(
                    line,
                    TextGutterControlKind::Markers,
                    x + lane,
                    top,
                    24.0,
                    control_height,
                    0,
                ));
            }
            if let Some(lane) = geometry.fold_x
                && has_fold
            {
                controls.push(TextGutterControl::new(
                    line,
                    TextGutterControlKind::Fold,
                    x + lane,
                    top,
                    20.0,
                    control_height,
                    0,
                ));
            }
        }

        if let Some(region) = session.folds().region(line)
            && session.folds().is_collapsed(line)
            && let Some(last_row) = rows.iter().rev().find(|row| row.line == line)
        {
            let badge_x = text_origin_x - origin.x + last_row.width + 8.0;
            let width = 80.0_f32.max(64.0).min(80.0).min(inputs.field_width - badge_x - 8.0);
            if badge_x >= text_origin_x - origin.x && badge_x + width <= inputs.field_width {
                controls.push(TextGutterControl::new(
                    line,
                    TextGutterControlKind::Summary,
                    badge_x,
                    text_origin_y - origin.y + last_row.top_y,
                    width,
                    inputs.line_height,
                    region.last_line.saturating_sub(region.first_line),
                ));
            }
        }
    }

    session.update(controls.clone());
    controls
}

/// Whether any breakpoint inside a collapsed region is still enabled, so the
/// summary badge can carry an error tone.
pub fn folded_breakpoints(gutter: &TextEditorGutter, header: usize, region_last: usize) -> usize {
    let Some(binding) = &gutter.breakpoints else { return 0 };
    binding
        .get()
        .iter()
        .filter(|point| point.line > header && point.line <= region_last)
        .count()
}

/// The tone the folded-breakpoint dot takes: error while any hidden breakpoint
/// is enabled, muted otherwise.
pub fn folded_breakpoint_is_active(
    gutter: &TextEditorGutter,
    header: usize,
    region_last: usize,
) -> bool {
    let Some(binding) = &gutter.breakpoints else { return false };
    binding.get().iter().any(|point| {
        point.line > header && point.line <= region_last && point.state != TextBreakpointState::Disabled
    })
}

/// The lines present in a document, in order, deduplicated.
pub fn distinct_lines(rows: &[GutterRow]) -> BTreeSet<usize> {
    rows.iter().map(|row| row.line).collect()
}
