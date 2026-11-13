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
    /// A fixed track, authored in points.
    pub const fn points(value: f32) -> Self {
        Self::length(LayoutStyleValue::Points(value))
    }
    /// A fixed track, authored as a percentage of the container.
    pub const fn percent(value: f32) -> Self {
        Self::length(LayoutStyleValue::Percent(value))
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
    /// `repeat(count, tracks)`, including `auto-fill` and `auto-fit` counts.
    pub fn repeat(count: GridRepeat, tracks: impl IntoIterator<Item = GridTrack>) -> Self {
        Self::Repeat {
            count,
            tracks: tracks.into_iter().collect(),
        }
    }

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

/// Failure modes of a named-area template, kept apart from `GridStyle` so the
/// authoring layer can report a bad template instead of silently dropping it.
#[derive(Clone, Debug, PartialEq)]
pub enum GridAreaError {
    /// The template has no rows.
    Empty,
    /// A row has a different token count than the first row.
    Ragged {
        row: usize,
        expected: usize,
        found: usize,
    },
    /// The cells of one name do not form the rectangle CSS requires.
    NonRectangular { name: String },
}

impl std::fmt::Display for GridAreaError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(formatter, "a grid area template needs at least one row"),
            Self::Ragged {
                row,
                expected,
                found,
            } => write!(
                formatter,
                "grid area row {row} has {found} cells, but the first row has {expected}"
            ),
            Self::NonRectangular { name } => write!(
                formatter,
                "grid area `{name}` is not a rectangle; every name must cover a filled rectangle"
            ),
        }
    }
}

impl std::error::Error for GridAreaError {}

/// One named area resolved to explicit grid lines, ready for a child placement.
///
/// Lines are 1-based, as in CSS: an area covering only the first column reports
/// `column_start: 1` and `column_end: 2`, which spans exactly one track.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GridAreaSpan {
    pub column_start: i16,
    pub column_end: i16,
    pub row_start: i16,
    pub row_end: i16,
}

/// Named grid areas authored as CSS-like row strings.
///
/// `GridAreaTemplate::new(&["header header", "main side"])` names `header`
/// across the first row and `main`/`side` in the second. A `.` token leaves a
/// cell unnamed. Every name must fill a rectangle, exactly as CSS requires;
/// [`Self::area`] resolves a name to explicit lines for a child placement.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct GridAreaTemplate {
    cells: Vec<Vec<Option<String>>>,
}

impl GridAreaTemplate {
    /// Parses whitespace-separated area rows. Tokens are `.` or a CSS ident.
    pub fn new<T: AsRef<str>>(rows: &[T]) -> Result<Self, GridAreaError> {
        let parsed: Vec<Vec<Option<String>>> = rows
            .iter()
            .map(|row| {
                row.as_ref()
                    .split_whitespace()
                    .map(|token| {
                        let token = token.trim();
                        if token == "." {
                            None
                        } else {
                            Some(token.to_string())
                        }
                    })
                    .collect()
            })
            .collect();
        let Some(first) = parsed.first() else {
            return Err(GridAreaError::Empty);
        };
        if first.is_empty() {
            return Err(GridAreaError::Empty);
        }
        let expected = first.len();
        if let Some(row) = parsed.iter().position(|cells| cells.len() != expected) {
            return Err(GridAreaError::Ragged {
                row,
                expected,
                found: parsed[row].len(),
            });
        }
        let template = Self { cells: parsed };
        template.validate()?;
        Ok(template)
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn row_count(&self) -> usize {
        self.cells.len()
    }

    pub fn column_count(&self) -> usize {
        self.cells.first().map_or(0, Vec::len)
    }

    /// The rectangle of `name`, or `None` when the template has no such area.
    pub fn area(&self, name: &str) -> Option<GridAreaSpan> {
        self.bounds(name)
            .map(|(top, left, bottom, right)| GridAreaSpan {
                column_start: left as i16 + 1,
                column_end: right as i16 + 2,
                row_start: top as i16 + 1,
                row_end: bottom as i16 + 2,
            })
    }

    /// Checks every name covers a filled rectangle of exactly its own cells.
    fn validate(&self) -> Result<(), GridAreaError> {
        for name in self.names() {
            let Some((top, left, bottom, right)) = self.bounds(&name) else {
                continue;
            };
            for row in top..=bottom {
                for column in left..=right {
                    if self.cells[row][column].as_deref() != Some(name.as_str()) {
                        return Err(GridAreaError::NonRectangular { name });
                    }
                }
            }
        }
        Ok(())
    }

    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .cells
            .iter()
            .flatten()
            .filter_map(|cell| cell.clone())
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }

    /// Inclusive cell bounds of `name`: `(top, left, bottom, right)`.
    fn bounds(&self, name: &str) -> Option<(usize, usize, usize, usize)> {
        let mut bounds: Option<(usize, usize, usize, usize)> = None;
        for (row, cells) in self.cells.iter().enumerate() {
            for (column, cell) in cells.iter().enumerate() {
                if cell.as_deref() != Some(name) {
                    continue;
                }
                bounds = Some(match bounds {
                    Some((top, left, bottom, right)) => (
                        top.min(row),
                        left.min(column),
                        bottom.max(row),
                        right.max(column),
                    ),
                    None => (row, column, row, column),
                });
            }
        }
        bounds
    }

    /// Taffy consumes one rectangle per name plus the template extents, which
    /// unnamed (`.`) cells still contribute to.
    fn to_taffy(&self) -> Option<taffy::GridTemplateAreas<String>> {
        if self.is_empty() {
            return None;
        }
        let mut areas = Vec::new();
        for name in self.names() {
            let span = self.area(&name)?;
            areas.push(taffy::GridTemplateArea {
                name,
                row_start: span.row_start as u16,
                row_end: span.row_end as u16,
                column_start: span.column_start as u16,
                column_end: span.column_end as u16,
            });
        }
        Some(taffy::GridTemplateAreas {
            areas,
            row_count: self.row_count() as u16,
            column_count: self.column_count() as u16,
        })
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
    /// Named areas; `Some` also sizes the explicit grid to the template.
    pub areas: Option<GridAreaTemplate>,
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
            areas: None,
            justify_items: None,
            justify_self: None,
        }
    }
}
impl GridStyle {
    /// CSS lets `grid-template-areas` imply the explicit tracks it covers; the
    /// authored layer relies on that so a template alone lays out a grid.
    fn effective_columns(&self) -> Vec<GridTemplateTrack> {
        if self.columns.is_empty()
            && let Some(areas) = &self.areas
        {
            return vec![GridTrack::AUTO.into(); areas.column_count()];
        }
        self.columns.clone()
    }

    fn effective_rows(&self) -> Vec<GridTemplateTrack> {
        if self.rows.is_empty()
            && let Some(areas) = &self.areas
        {
            return vec![GridTrack::AUTO.into(); areas.row_count()];
        }
        self.rows.clone()
    }

    pub(super) fn apply(&self, style: &mut taffy::Style, calc: &CalcStorage) {
        style.grid_template_columns = self
            .effective_columns()
            .iter()
            .map(|track| track.to_taffy(calc))
            .collect();
        style.grid_template_rows = self
            .effective_rows()
            .iter()
            .map(|track| track.to_taffy(calc))
            .collect();
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
        style.grid_template_areas = self.areas.as_ref().and_then(|areas| areas.to_taffy());
        if let Some(align) = self.justify_items {
            style.justify_items = super::style::align_items(align);
        }
        style.justify_self = self.justify_self.map(super::style::align_items);
    }
}
