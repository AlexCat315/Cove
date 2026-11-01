//! Translation of Primitives/Text/Markdown/TableBlock.swift.
//!
//! A GFM table: columns sized to their content within a readable range, a tinted
//! header, row rules, and horizontal scrolling when the table is wider than its
//! container.
use std::rc::Rc;

use crate::LayoutContainer;
use crate::theme::theme_for_node;
use crate::{AnyView, Layout, LayoutSubview, ProposedViewSize, View};
use cove_core::scene::{InvalidationSource, Node};
use cove_core::{Point, Rect, Size, TextAlignment};

use super::document::{MarkdownAlignment, MarkdownTable};

/// Narrowest a column may be, so short headers still read as columns.
pub const MINIMUM_COLUMN: f32 = 56.0;
/// Widest a content-measured column may grow before the table scrolls.
pub const MAXIMUM_COLUMN: f32 = 320.0;

/// Column and row extents for one table.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TableGeometry {
    pub widths: Vec<f32>,
    pub heights: Vec<f32>,
}

impl TableGeometry {
    pub fn size(&self) -> Size {
        Size {
            width: self.widths.iter().sum(),
            height: self.heights.iter().sum(),
        }
    }
}

/// Subviews: the header tint, then `rows x columns` cells in row order, then one
/// rule under every row but the last.
#[derive(Clone, Copy, Debug)]
pub struct MarkdownTableLayout {
    pub columns: usize,
    pub rows: usize,
}

impl MarkdownTableLayout {
    pub fn new(columns: usize, rows: usize) -> Self {
        Self { columns, rows }
    }

    fn geometry(&self, subviews: &[LayoutSubview]) -> TableGeometry {
        if self.columns == 0 || subviews.len() < 1 + self.rows * self.columns {
            return TableGeometry::default();
        }
        let cell = |row: usize, column: usize| &subviews[1 + row * self.columns + column];
        let widths: Vec<f32> = (0..self.columns)
            .map(|column| {
                let natural = (0..self.rows)
                    .map(|row| {
                        cell(row, column)
                            .size_that_fits(ProposedViewSize::UNSPECIFIED)
                            .width
                    })
                    .fold(0.0_f32, f32::max);
                MAXIMUM_COLUMN.min(MINIMUM_COLUMN.max(natural.ceil()))
            })
            .collect();
        let heights: Vec<f32> = (0..self.rows)
            .map(|row| {
                (0..self.columns)
                    .map(|column| {
                        cell(row, column)
                            .size_that_fits(ProposedViewSize::new(Some(widths[column]), None))
                            .height
                    })
                    .fold(0.0_f32, f32::max)
                    .ceil()
            })
            .collect();
        TableGeometry { widths, heights }
    }
}

impl Layout for MarkdownTableLayout {
    fn size_that_fits(&self, proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> Size {
        let _ = proposal;
        self.geometry(subviews).size()
    }

    fn place_subviews(
        &self,
        bounds: Rect,
        _proposal: ProposedViewSize,
        subviews: &[LayoutSubview],
    ) {
        let geometry = self.geometry(subviews);
        if geometry.widths.is_empty() {
            return;
        }
        let total = geometry.size().width;
        subviews[0].place(
            Point::new(bounds.x, bounds.y),
            ProposedViewSize::new(Some(total), geometry.heights.first().copied()),
            true,
        );
        let rtl = subviews
            .first()
            .is_some_and(|subview| subview.layout_direction == cove_core::scene::Direction::Rtl);
        let mut y = bounds.y;
        for row in 0..self.rows {
            let mut x = bounds.x;
            for column in 0..self.columns {
                // The leading/trailing columns follow the reading direction.
                let index = if rtl {
                    self.columns - 1 - column
                } else {
                    column
                };
                subviews[1 + row * self.columns + index].place(
                    Point::new(x, y),
                    ProposedViewSize::new(
                        Some(geometry.widths[index]),
                        geometry.heights.get(row).copied(),
                    ),
                    true,
                );
                x += geometry.widths[index];
            }
            y += geometry.heights[row];
            let rule = 1 + self.rows * self.columns + row;
            if row + 1 < self.rows
                && let Some(rule) = subviews.get(rule)
            {
                rule.place(
                    Point::new(bounds.x, y - 1.0),
                    ProposedViewSize::new(Some(total), Some(1.0)),
                    true,
                );
            }
        }
    }
}

/// A rendered GFM table with its own horizontal scroll.
pub struct MarkdownTableView {
    pub table: MarkdownTable,
    pub on_open_link: Option<Rc<dyn Fn(&str)>>,
}

impl MarkdownTableView {
    pub fn new(table: MarkdownTable) -> Self {
        Self {
            table,
            on_open_link: None,
        }
    }

    pub fn on_open_link(mut self, handler: impl Fn(&str) + 'static) -> Self {
        self.on_open_link = Some(Rc::new(handler));
        self
    }

    fn cell_alignment(&self, column: usize) -> TextAlignment {
        match self
            .table
            .alignments
            .get(column)
            .copied()
            .unwrap_or(MarkdownAlignment::Leading)
        {
            MarkdownAlignment::Leading => TextAlignment::Leading,
            MarkdownAlignment::Center => TextAlignment::Center,
            MarkdownAlignment::Trailing => TextAlignment::Trailing,
        }
    }
}

impl View for MarkdownTableView {
    fn body(&self) -> AnyView {
        let columns = self
            .table
            .header
            .len()
            .max(self.table.rows.iter().map(Vec::len).max().unwrap_or(0));
        let mut cell_views: Vec<AnyView> = Vec::new();
        // Tinted header backdrop.
        cell_views.push(AnyView::new(TableHeaderTint));
        let mut rows: Vec<Vec<Vec<crate::primitives::text::rich::run::RichTextRun>>> =
            vec![self.table.header.clone()];
        rows.extend(self.table.rows.clone());
        for (row_index, row) in rows.iter().enumerate() {
            for column in 0..columns {
                let runs = row.get(column).cloned().unwrap_or_default();
                let content = if row_index == 0 {
                    runs.into_iter()
                        .map(|run| crate::primitives::text::rich::run::RichTextRun {
                            text: run.text,
                            traits: run
                                .traits
                                .union(crate::primitives::text::rich::run::RichTextTraits::BOLD),
                            link: run.link,
                        })
                        .collect()
                } else {
                    runs
                };
                cell_views.push(AnyView::new(TableTextCell {
                    runs: content,
                    alignment: self.cell_alignment(column),
                }));
            }
        }
        let row_count = rows.len();
        for _ in 0..row_count.saturating_sub(1) {
            cell_views.push(AnyView::new(crate::primitives::text::Divider::new(
                None,
                1.0,
                crate::primitives::text::DividerAxis::Horizontal,
            )));
        }
        let layout = MarkdownTableLayout::new(columns, row_count);
        AnyView::new(
            crate::primitives::layout::ScrollView::new(
                crate::primitives::layout::ScrollAxis::Horizontal,
                AnyView::new(TableCard {
                    content: AnyView::new(
                        LayoutContainer::<MarkdownTableLayout, AnyView>::with_erased_content(
                            layout,
                            AnyView::new(crate::TupleView::new(cell_views)),
                        ),
                    ),
                }),
            )
            .accessibility_label("Table"),
        )
    }
}

/// Tinted header band behind the first row.
struct TableHeaderTint;

impl crate::PrimitiveView for TableHeaderTint {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, node: &Node) {
        node.set_background_color(Some(theme_for_node(node).colors.surface_variant));
    }
}
crate::impl_primitive_view!(TableHeaderTint);

/// One table cell: text that fills its column so centring and trailing
/// alignment apply.
struct TableTextCell {
    runs: Vec<crate::primitives::text::rich::run::RichTextRun>,
    alignment: TextAlignment,
}

impl crate::PrimitiveView for TableTextCell {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_padding(8.0, cove_core::scene::Edge::Top);
        layout.set_padding(12.0, cove_core::scene::Edge::Start);
        layout.set_padding(8.0, cove_core::scene::Edge::Bottom);
        layout.set_padding(12.0, cove_core::scene::Edge::End);
        Some(layout)
    }

    fn update_layout(&self, layout: &cove_core::scene::LayoutNode) {
        layout.set_padding(8.0, cove_core::scene::Edge::Top);
        layout.set_padding(12.0, cove_core::scene::Edge::Start);
        layout.set_padding(8.0, cove_core::scene::Edge::Bottom);
        layout.set_padding(12.0, cove_core::scene::Edge::End);
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        // Real runs, so a cell keeps per-run bold, code and links.
        vec![AnyView::new(
            crate::primitives::text::rich::RichText::new(self.runs.clone())
                .alignment(self.alignment)
                .display_only(true),
        )]
    }
}
crate::impl_primitive_view!(TableTextCell);

/// Rounded, bordered card around the table grid.
struct TableCard {
    content: AnyView,
}

impl crate::PrimitiveView for TableCard {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_clips_to_bounds(true);
        node
    }

    fn update_node(&self, node: &Node) {
        let theme = theme_for_node(node);
        node.set_corner_radius(8.0);
        node.set_border_color(Some(theme.colors.border));
        node.set_border_width(1.0);
        node.mark_render_dirty(InvalidationSource::style_set("markdown.table"));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
crate::impl_primitive_view!(TableCard);
