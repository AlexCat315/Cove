//! Translation of Compose/Sources/CoveUICompose/Primitives/Layout/Grid/Columns.swift.

/// Width policy for one grid column. An adaptive column expands into as many
/// columns as its allocated width can hold. Children occupy one cell each.
#[derive(Clone, Debug, PartialEq)]
pub enum GridColumn {
    Fixed(f32),
    Flexible { minimum: f32, maximum: f32 },
    Adaptive { minimum: f32, maximum: f32 },
    /// Named field columns preserve their source slot when hidden. Use fixed
    /// or flexible sizing; adaptive card flows remain anonymous columns.
    Named {
        id: String,
        title: String,
        sizing: Box<GridColumn>,
        visibility: ColumnVisibility,
    },
}
impl GridColumn {
    pub fn flexible(minimum: f32, maximum: f32) -> Self {
        Self::Flexible { minimum, maximum }
    }
    pub fn adaptive(minimum: f32, maximum: f32) -> Self {
        Self::Adaptive { minimum, maximum }
    }
    pub fn named(id: impl Into<String>, title: impl Into<String>, sizing: GridColumn, visibility: ColumnVisibility) -> Self {
        Self::Named { id: id.into(), title: title.into(), sizing: Box::new(sizing), visibility }
    }
    pub fn sizing(&self) -> &GridColumn {
        match self {
            Self::Named { sizing, .. } => sizing,
            _ => self,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnVisibility {
    Required,
    Automatic { priority: i32 },
}
impl ColumnVisibility {
    pub fn automatic(priority: i32) -> Self {
        Self::Automatic { priority }
    }
    pub fn priority(self) -> i32 {
        match self {
            Self::Required => i32::MAX,
            Self::Automatic { priority } => priority,
        }
    }
}
impl Default for ColumnVisibility {
    fn default() -> Self {
        Self::Required
    }
}

/// Resolves column widths without measuring or constructing child views.
pub struct GridTrackResolver;
impl GridTrackResolver {
    pub fn minimum_width(columns: &[GridColumn], spacing: f32) -> f32 {
        columns.iter().map(|column| Self::limits(column).0).sum::<f32>()
            + Self::finite_nonnegative(spacing) * columns.len().saturating_sub(1) as f32
    }

    pub fn widths(columns: &[GridColumn], available_width: Option<f32>, spacing: f32,
                  intrinsic_widths: &[f32]) -> GridTracks {
        let columns: Vec<GridColumn> = columns.iter().map(|column| column.sizing().clone()).collect();
        if columns.is_empty() {
            return GridTracks::new(vec![], 0.0);
        }
        let gap = Self::finite_nonnegative(spacing);
        let limits: Vec<(f32, f32)> = columns.iter().map(Self::limits).collect();
        let mut widths: Vec<f32> = limits.iter().map(|limit| limit.0).collect();
        let finite_width = available_width.and_then(|width| if width.is_finite() { Some(width.max(0.0)) } else { None });

        if let Some(finite_width) = finite_width {
            let mut remaining = (finite_width - gap * (columns.len() - 1) as f32
                - widths.iter().sum::<f32>()).max(0.0);
            let mut active: Vec<usize> = (0..columns.len()).filter(|&index| limits[index].1 > widths[index]).collect();
            // Each pass either finishes distribution or saturates a track.
            while remaining > 0.0 && !active.is_empty() {
                let share = remaining / active.len() as f32;
                let mut consumed = 0.0f32;
                let mut next = Vec::new();
                for &index in &active {
                    let addition = share.min(limits[index].1 - widths[index]);
                    widths[index] += addition;
                    consumed += addition;
                    if widths[index] < limits[index].1 {
                        next.push(index);
                    }
                }
                if consumed <= 0.0 {
                    break;
                }
                remaining = (remaining - consumed).max(0.0);
                active = next;
            }
        } else {
            for (index, column) in columns.iter().enumerate() {
                if !matches!(column, GridColumn::Fixed(_)) {
                    let intrinsic = intrinsic_widths.get(index).copied().unwrap_or(0.0);
                    widths[index] = limits[index].1.min(widths[index].max(Self::finite_nonnegative(intrinsic)));
                }
            }
        }

        let mut resolved = Vec::with_capacity(columns.len());
        for (index, column) in columns.iter().enumerate() {
            let (raw_minimum, raw_maximum) = match column {
                GridColumn::Adaptive { minimum, maximum } => (*minimum, *maximum),
                _ => {
                    resolved.push(GridTracksGroup { width: widths[index], count: 1 });
                    continue;
                }
            };
            let minimum = 1.0f32.max(Self::finite_nonnegative(raw_minimum));
            let maximum = Self::upper_bound(raw_maximum, minimum);
            let capacity = ((widths[index] + gap) / (minimum + gap)).floor().max(1.0);
            let count_limit = (i32::MAX as f32 / 2.0) / columns.len() as f32;
            let count = capacity.min(count_limit) as usize;
            let cell_width = minimum.max(maximum.min(
                (widths[index] - gap * (count.saturating_sub(1)) as f32) / count as f32));
            resolved.push(GridTracksGroup { width: cell_width, count });
        }
        GridTracks::new(resolved, gap)
    }

    pub fn finite_nonnegative(value: f32) -> f32 {
        if value.is_finite() { value.max(0.0) } else { 0.0 }
    }

    fn upper_bound(value: f32, minimum: f32) -> f32 {
        if value.is_nan() { minimum } else { value.max(minimum) }
    }

    fn limits(column: &GridColumn) -> (f32, f32) {
        match column {
            GridColumn::Fixed(width) => {
                let width = Self::finite_nonnegative(*width);
                (width, width)
            }
            GridColumn::Flexible { minimum, maximum } => {
                let minimum = Self::finite_nonnegative(*minimum);
                (minimum, Self::upper_bound(*maximum, minimum))
            }
            GridColumn::Adaptive { minimum, .. } => (1.0f32.max(Self::finite_nonnegative(*minimum)), f32::INFINITY),
            GridColumn::Named { sizing, .. } => Self::limits(sizing),
        }
    }
}

/// Adaptive repetitions stay compact. Lookup visits only the authored column
/// policies; total width and count do not iterate over empty adaptive cells.
#[derive(Clone, Debug)]
pub struct GridTracks {
    groups: Vec<GridTracksGroup>,
    pub count: usize,
    pub total_width: f32,
    spacing: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridTracksGroup {
    pub width: f32,
    pub count: usize,
}
impl GridTracks {
    pub fn new(groups: Vec<GridTracksGroup>, spacing: f32) -> Self {
        let count: usize = groups.iter().map(|group| group.count).sum();
        let total_width = groups.iter().map(|group| group.width * group.count as f32).sum::<f32>()
            + spacing * count.saturating_sub(1) as f32;
        Self { groups, count, total_width, spacing }
    }
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
    pub fn spacing(&self) -> f32 {
        self.spacing
    }
    pub fn groups(&self) -> &[GridTracksGroup] {
        &self.groups
    }
    pub fn get(&self, index: usize) -> f32 {
        assert!(index < self.count, "Grid column index out of bounds");
        let mut remaining = index;
        for group in &self.groups {
            if remaining < group.count {
                return group.width;
            }
            remaining -= group.count;
        }
        panic!("Grid column index out of bounds");
    }
    pub fn iter(&self) -> GridTracksIterator<'_> {
        GridTracksIterator { groups: &self.groups, group_index: 0, repetition: 0 }
    }
}
impl<'a> IntoIterator for &'a GridTracks {
    type Item = f32;
    type IntoIter = GridTracksIterator<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
pub struct GridTracksIterator<'a> {
    groups: &'a [GridTracksGroup],
    group_index: usize,
    repetition: usize,
}
impl Iterator for GridTracksIterator<'_> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let group = *self.groups.get(self.group_index)?;
        self.repetition += 1;
        if self.repetition == group.count {
            self.repetition = 0;
            self.group_index += 1;
        }
        Some(group.width)
    }
}
