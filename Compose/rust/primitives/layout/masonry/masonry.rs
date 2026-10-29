//! Translation of Compose/Sources/CoveUICompose/Primitives/Layout/Masonry/Masonry.swift.
use cove_core::scene::{Direction, Point, Rect, Size};
use crate::{Alignment, AnyView, HorizontalAlignment, Layout, LayoutContainer, LayoutSubview, ProposedViewSize, View};

/// A balanced waterfall layout: each child is measured at the column width and
/// placed in the currently shortest column. Rows are not fixed, so cards with
/// different heights pack without leaving the vertical gaps a row grid leaves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MasonryLayout {
    pub minimum_column_width: f32,
    pub maximum_columns: usize,
    pub column_spacing: f32,
    pub item_spacing: f32,
    pub alignment: Alignment,
}
impl MasonryLayout {
    pub fn new(minimum_column_width: f32, maximum_columns: usize, column_spacing: f32,
               item_spacing: f32, alignment: Alignment) -> Self {
        Self {
            minimum_column_width: if minimum_column_width.is_finite() { minimum_column_width.max(1.0) } else { 160.0 },
            maximum_columns: maximum_columns.clamp(1, 8),
            column_spacing: if column_spacing.is_finite() { column_spacing.max(0.0) } else { 12.0 },
            item_spacing: if item_spacing.is_finite() { item_spacing.max(0.0) } else { 12.0 },
            alignment,
        }
    }
}
impl Default for MasonryLayout {
    fn default() -> Self {
        Self::new(160.0, 2, 12.0, 12.0, Alignment::TopLeading)
    }
}
impl Layout for MasonryLayout {
    fn size_that_fits(&self, proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> Size {
        self.arrangement(proposal, subviews).size
    }

    fn place_subviews(&self, bounds: Rect, _proposal: ProposedViewSize, subviews: &[LayoutSubview]) {
        if subviews.is_empty() {
            return;
        }
        let arrangement = self.arrangement(ProposedViewSize::new(Some(bounds.width), None), subviews);
        if arrangement.column_count == 0 {
            return;
        }
        let rtl = subviews.first().map(|subview| subview.layout_direction == Direction::Rtl).unwrap_or(false);
        let components = self.alignment.components();
        for index in 0..subviews.len() {
            let column = arrangement.columns[index];
            let item = arrangement.sizes[index];
            let column_width = arrangement.column_width;
            let cell_x = if rtl {
                bounds.max_x() - column as f32 * (column_width + self.column_spacing) - column_width
            } else {
                bounds.x + column as f32 * (column_width + self.column_spacing)
            };
            let available = (column_width - item.width).max(0.0);
            let x = cell_x + horizontal_offset(components.0, available, rtl);
            subviews[index].place(
                Point::new(x, bounds.y + arrangement.offsets[index]),
                ProposedViewSize::new(Some(column_width), Some(item.height)),
                true);
        }
    }
}
impl MasonryLayout {
    fn arrangement(&self, proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> MasonryArrangement {
        if subviews.is_empty() {
            return MasonryArrangement::empty();
        }
        let available_width = proposal.width.map(|width| width.max(0.0));
        let column_count = self.column_count(available_width);
        let (column_width, sizes) = if let Some(available_width) = available_width {
            let column_width = ((available_width - self.column_spacing * (column_count - 1) as f32)
                / column_count as f32).max(0.0);
            (column_width, subviews.iter().map(|subview| self.measured(subview, Some(column_width))).collect())
        } else {
            let intrinsic: Vec<f32> = subviews.iter()
                .map(|subview| self.measured(subview, None).width).collect();
            let column_width = intrinsic.iter().copied().fold(0.0f32, f32::max);
            (column_width, subviews.iter().map(|subview| self.measured(subview, Some(column_width))).collect())
        };
        let mut heights = vec![0.0f32; column_count];
        let mut offsets = vec![0.0f32; subviews.len()];
        let mut columns = vec![0usize; subviews.len()];
        for index in 0..subviews.len() {
            let column = heights.iter().enumerate()
                .min_by(|(_, left), (_, right)| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(index, _)| index).unwrap_or(0);
            columns[index] = column;
            offsets[index] = heights[column];
            heights[column] += sizes[index].height + self.item_spacing;
        }
        let content_height = (heights.iter().copied().fold(0.0f32, f32::max) - self.item_spacing).max(0.0);
        let content_width = column_width * column_count as f32
            + self.column_spacing * (column_count - 1) as f32;
        MasonryArrangement {
            column_count,
            column_width,
            columns,
            sizes,
            offsets,
            size: Size { width: content_width, height: content_height },
        }
    }

    fn column_count(&self, available_width: Option<f32>) -> usize {
        let Some(available_width) = available_width else { return self.maximum_columns };
        let fit = ((available_width + self.column_spacing)
            / (self.minimum_column_width + self.column_spacing)).floor() as i64;
        self.maximum_columns.min((fit.max(1)) as usize)
    }

    fn measured(&self, subview: &LayoutSubview, width: Option<f32>) -> Size {
        let size = subview.size_that_fits(ProposedViewSize::new(width, None));
        Size {
            width: if size.width.is_finite() { size.width.max(0.0) } else { 0.0 },
            height: if size.height.is_finite() { size.height.max(0.0) } else { 0.0 },
        }
    }
}

fn horizontal_offset(alignment: HorizontalAlignment, available: f32, rtl: bool) -> f32 {
    match alignment {
        HorizontalAlignment::Leading => if rtl { available } else { 0.0 },
        HorizontalAlignment::Center => available / 2.0,
        HorizontalAlignment::Trailing => if rtl { 0.0 } else { available },
    }
}

struct MasonryArrangement {
    column_count: usize,
    column_width: f32,
    columns: Vec<usize>,
    sizes: Vec<Size>,
    offsets: Vec<f32>,
    size: Size,
}
impl MasonryArrangement {
    fn empty() -> Self {
        Self { column_count: 0, column_width: 0.0, columns: vec![], sizes: vec![], offsets: vec![], size: Size::default() }
    }
}

/// Convenience wrapper around `MasonryLayout`.
#[derive(Clone)]
pub struct Masonry {
    pub layout: MasonryLayout,
    pub content: AnyView,
}
impl Masonry {
    pub fn new(minimum_column_width: f32, maximum_columns: usize, column_spacing: f32,
               item_spacing: f32, alignment: Alignment, content: AnyView) -> Self {
        Self {
            layout: MasonryLayout::new(minimum_column_width, maximum_columns, column_spacing, item_spacing, alignment),
            content,
        }
    }
}
impl View for Masonry {
    fn body(&self) -> AnyView {
        AnyView::new(LayoutContainer::<MasonryLayout, AnyView>::with_erased_content(
            self.layout, self.content.clone()))
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
