//! CSS Grid styles independent of engine-owned calc handles.
use super::value::{CalcStorage, LayoutStyleValue};
pub use taffy::{GridAutoFlow, GridPlacement, RepetitionCount as GridRepeat};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GridBreadth {
    Auto,
    MinContent,
    MaxContent,
    Length(LayoutStyleValue),
    Fraction(f32),
    FitContent(LayoutStyleValue),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridTrack {
    pub min: GridBreadth,
    pub max: GridBreadth,
}
impl GridTrack {
    pub const AUTO: Self = Self::minmax(GridBreadth::Auto, GridBreadth::Auto);
    pub const fn minmax(min: GridBreadth, max: GridBreadth) -> Self {
        Self { min, max }
    }
    pub const fn length(value: LayoutStyleValue) -> Self {
        Self::minmax(GridBreadth::Length(value), GridBreadth::Length(value))
    }
    pub const fn fr(value: f32) -> Self {
        Self::minmax(GridBreadth::Auto, GridBreadth::Fraction(value))
    }
    fn to_taffy(self, calc: &CalcStorage) -> taffy::TrackSizingFunction {
        use taffy::{MaxTrackSizingFunction as Max, MinTrackSizingFunction as Min};
        let min = match self.min {
            GridBreadth::Auto => Min::auto(),
            GridBreadth::MinContent => Min::min_content(),
            GridBreadth::MaxContent => Min::max_content(),
            GridBreadth::Length(LayoutStyleValue::Points(v)) => Min::length(v),
            GridBreadth::Length(LayoutStyleValue::Percent(v)) => Min::percent(v / 100.0),
            GridBreadth::Length(LayoutStyleValue::Calc(v)) => Min::calc(calc.insert(v)),
            _ => panic!("grid track minimum requires auto, intrinsic size or a length"),
        };
        let max = match self.max {
            GridBreadth::Auto => Max::auto(),
            GridBreadth::MinContent => Max::min_content(),
            GridBreadth::MaxContent => Max::max_content(),
            GridBreadth::Length(LayoutStyleValue::Points(v)) => Max::length(v),
            GridBreadth::Length(LayoutStyleValue::Percent(v)) => Max::percent(v / 100.0),
            GridBreadth::Length(LayoutStyleValue::Calc(v)) => Max::calc(calc.insert(v)),
            GridBreadth::Fraction(v) => {
                assert!(v.is_finite() && v >= 0.0);
                Max::fr(v)
            }
            GridBreadth::FitContent(LayoutStyleValue::Points(v)) => Max::fit_content_px(v),
            GridBreadth::FitContent(LayoutStyleValue::Percent(v)) => {
                Max::fit_content_percent(v / 100.0)
            }
            GridBreadth::FitContent(LayoutStyleValue::Calc(v)) => {
                Max::fit_content_calc(calc.insert(v))
            }
            _ => panic!("invalid grid track maximum"),
        };
        taffy::MinMax { min, max }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum GridTemplateTrack {
    Track(GridTrack),
    Repeat {
        count: GridRepeat,
        tracks: Vec<GridTrack>,
    },
}
impl From<GridTrack> for GridTemplateTrack {
    fn from(value: GridTrack) -> Self {
        Self::Track(value)
    }
}
impl GridTemplateTrack {
    fn to_taffy(&self, calc: &CalcStorage) -> taffy::GridTemplateComponent<String> {
        match self {
            Self::Track(track) => taffy::GridTemplateComponent::Single(track.to_taffy(calc)),
            Self::Repeat { count, tracks } => {
                assert!(!tracks.is_empty() && !matches!(count, GridRepeat::Count(0)));
                taffy::GridTemplateComponent::Repeat(taffy::GridTemplateRepetition {
                    count: *count,
                    tracks: tracks.iter().map(|track| track.to_taffy(calc)).collect(),
                    line_names: vec![],
                })
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GridStyle {
    pub columns: Vec<GridTemplateTrack>,
    pub rows: Vec<GridTemplateTrack>,
    pub auto_columns: Vec<GridTrack>,
    pub auto_rows: Vec<GridTrack>,
    pub auto_flow: GridAutoFlow,
    pub column: [GridPlacement<String>; 2],
    pub row: [GridPlacement<String>; 2],
    pub column_names: Vec<Vec<String>>,
    pub row_names: Vec<Vec<String>>,
    /// Horizontal alignment of children; None uses CSS Grid's default stretch.
    pub justify_items: Option<super::Align>,
    pub justify_self: Option<super::Align>,
}
impl Default for GridStyle {
    fn default() -> Self {
        Self {
            columns: vec![],
            rows: vec![],
            auto_columns: vec![],
            auto_rows: vec![],
            auto_flow: GridAutoFlow::Row,
            column: [GridPlacement::Auto, GridPlacement::Auto],
            row: [GridPlacement::Auto, GridPlacement::Auto],
            column_names: vec![],
            row_names: vec![],
            justify_items: None,
            justify_self: None,
        }
    }
}
impl GridStyle {
    pub(super) fn apply(&self, style: &mut taffy::Style, calc: &CalcStorage) {
        style.grid_template_columns = self
            .columns
            .iter()
            .map(|track| track.to_taffy(calc))
            .collect();
        style.grid_template_rows = self.rows.iter().map(|track| track.to_taffy(calc)).collect();
        style.grid_auto_columns = self
            .auto_columns
            .iter()
            .map(|track| track.to_taffy(calc))
            .collect();
        style.grid_auto_rows = self
            .auto_rows
            .iter()
            .map(|track| track.to_taffy(calc))
            .collect();
        style.grid_auto_flow = self.auto_flow;
        style.grid_column = taffy::Line {
            start: self.column[0].clone(),
            end: self.column[1].clone(),
        };
        style.grid_row = taffy::Line {
            start: self.row[0].clone(),
            end: self.row[1].clone(),
        };
        style
            .grid_template_column_names
            .clone_from(&self.column_names);
        style.grid_template_row_names.clone_from(&self.row_names);
        if let Some(align) = self.justify_items {
            style.justify_items = super::style::align_items(align);
        }
        style.justify_self = self.justify_self.map(super::style::align_items);
    }
}
