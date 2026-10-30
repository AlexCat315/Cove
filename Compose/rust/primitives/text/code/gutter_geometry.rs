//! Translation of Primitives/Text/Code/GutterGeometry.swift.
//!
//! One resolved gutter geometry: how wide the number lane is, where each
//! interactive lane starts, and how much room the whole gutter takes. Virtualization
//! and hit testing consume the same geometry so the painted controls and their
//! targets cannot drift.
use cove_core::{Font, Rect};
use crate::theme::Theme;

/// One resolved gutter geometry for a code field.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextGutterGeometry {
    pub width: f32,
    pub number_right: f32,
    pub breakpoint_x: Option<f32>,
    pub marker_x: Option<f32>,
    pub fold_x: Option<f32>,
    /// Touch surfaces fold the lanes into one 44pt control.
    pub combined: bool,
    pub control_width: f32,
}

impl TextGutterGeometry {
    /// Lanes are ordered breakpoints, markers, numbers, folds from the leading
    /// edge. A touch surface with any gutter affordance uses the single
    /// combined control instead of separate lanes.
    pub fn new(
        gutter_enabled: bool,
        has_breakpoints: bool,
        has_execution_line: bool,
        has_folds: bool,
        number_width: f32,
        font: &Font,
        theme: &Theme,
        touch: bool,
    ) -> Self {
        let folds = has_folds;
        let combined = touch && (gutter_enabled || folds);
        let control_width = if combined { 44.0 } else { 24.0 };
        let mut leading = 0.0_f32;

        let (breakpoint_x, marker_x, fold_x);
        if combined {
            breakpoint_x = Some(0.0);
            marker_x = None;
            fold_x = None;
            leading = 44.0;
        } else {
            breakpoint_x = (has_breakpoints || has_execution_line).then_some(leading);
            if breakpoint_x.is_some() {
                leading += 24.0;
            }
            marker_x = gutter_enabled.then_some(leading);
            if marker_x.is_some() {
                leading += 24.0;
            }
            fold_x = folds.then_some(leading + theme.spacing.lg + number_width + 4.0);
        }

        let number_right = leading + theme.spacing.lg + number_width;
        let width = number_right
            + theme
                .spacing
                .xxl
                .max(font.size())
                .max(if folds && !combined { 36.0 } else { 0.0 });

        Self {
            width,
            number_right,
            breakpoint_x,
            marker_x,
            fold_x,
            combined,
            control_width,
        }
    }
}

/// Which gutter affordance a control represents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextGutterControlKind {
    Breakpoint,
    Markers,
    Fold,
    /// One control for every affordance on a touch surface.
    Combined,
    /// The badge counting the lines hidden by a collapsed fold.
    Summary,
}

impl TextGutterControlKind {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Breakpoint => "breakpoint",
            Self::Markers => "markers",
            Self::Fold => "fold",
            Self::Combined => "combined",
            Self::Summary => "summary",
        }
    }
}

/// One interactive target in the gutter, positioned in field coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct TextGutterControl {
    pub id: String,
    pub line: usize,
    pub kind: TextGutterControlKind,
    pub frame: Rect,
    /// Lines hidden by a collapsed fold; zero for every other kind.
    pub count: usize,
}

impl TextGutterControl {
    pub fn new(
        line: usize,
        kind: TextGutterControlKind,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        count: usize,
    ) -> Self {
        Self {
            id: format!("{}-{line}", kind.slug()),
            line,
            kind,
            frame: Rect::new(x, y, width, height),
            count,
        }
    }

    /// The stable identity used by roving focus and debug names.
    pub fn identity(&self) -> &str {
        &self.id
    }
}
