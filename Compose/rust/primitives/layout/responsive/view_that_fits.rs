//! Translation of `Primitives/Layout/Responsive/ViewThatFits.swift`.
//!
//! Selects wide or compact content using the allocated container width. Each
//! branch keeps its own identity and local state for as long as this container
//! exists; inactive branches stay mounted but take no layout space, paint no
//! pixels, and receive no input or accessibility. Shared bindings keep working
//! across the two presentations.
//!
//! Two selection modes:
//! - `Threshold(width)` picks wide when the container is at least `width` wide.
//! - `Intrinsic` measures the wide branch's natural size and picks it only when
//!   that fits; measurement uses an unspecified proposal so a parent width
//!   cannot make a wrapping wide branch look like it fits.

use cove_composition::adaptive::ContainerSizeReader;
use cove_composition::{AnyView, PrimitiveView, View, ViewExt, impl_primitive_view};
// Layout vocabulary is re-exported at the Compose crate root.
use crate::{Layout, LayoutContainer, LayoutSubview, ProposedViewSize};
use cove_core::scene::{Align, Display, FlexDirection, LayoutNode, Node, Wrap};
use cove_core::{Point, Rect, Size};

use crate::BoxView;

/// How `ViewThatFits` decides between its two branches.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ViewThatFitsMode {
    /// Wide while the container is at least this wide.
    Threshold(f32),
    /// Wide while its own natural size fits the proposal.
    Intrinsic,
}

impl ViewThatFitsMode {
    /// Non-finite and negative thresholds collapse to zero, so a threshold mode
    /// always selects wide rather than misbehaving.
    pub fn threshold(value: f32) -> Self {
        Self::Threshold(if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        })
    }
}

/// Picks between a wide and a compact presentation of the same content.
pub struct ViewThatFits {
    mode: ViewThatFitsMode,
    wide: AnyView,
    compact: AnyView,
}

impl ViewThatFits {
    /// Threshold mode at the compact-width boundary.
    pub fn new(wide: impl View + 'static, compact: impl View + 'static) -> Self {
        Self::fitting(ViewThatFitsMode::threshold(700.0), wide, compact)
    }

    pub fn threshold(value: f32, wide: impl View + 'static, compact: impl View + 'static) -> Self {
        Self::fitting(ViewThatFitsMode::threshold(value), wide, compact)
    }

    pub fn intrinsic(wide: impl View + 'static, compact: impl View + 'static) -> Self {
        Self::fitting(ViewThatFitsMode::Intrinsic, wide, compact)
    }

    pub fn fitting(
        mode: ViewThatFitsMode,
        wide: impl View + 'static,
        compact: impl View + 'static,
    ) -> Self {
        Self {
            mode,
            wide: AnyView::new(wide),
            compact: AnyView::new(compact),
        }
    }
}

impl View for ViewThatFits {
    fn body(&self) -> AnyView {
        match self.mode {
            ViewThatFitsMode::Threshold(threshold) => {
                let wide = self.wide.clone();
                let compact = self.compact.clone();
                AnyView::new(ContainerSizeReader::new(move |size: Size| {
                    let is_wide = size.width >= threshold;
                    AnyView::new(BoxView::new(
                        FlexDirection::Column,
                        Align::Stretch,
                        cove_core::scene::Justify::FlexStart,
                        Wrap::NoWrap,
                        0.0,
                        AnyView::new(cove_composition::TupleView::new(vec![
                            AnyView::new(
                                AnyView::new(RetainedBranch::new(is_wide, wide.clone())).id("wide"),
                            ),
                            AnyView::new(
                                AnyView::new(RetainedBranch::new(!is_wide, compact.clone()))
                                    .id("compact"),
                            ),
                        ])),
                    ))
                }))
            }
            ViewThatFitsMode::Intrinsic => {
                let content = AnyView::new(cove_composition::TupleView::new(vec![
                    self.wide.clone(),
                    self.compact.clone(),
                ]));
                AnyView::new(LayoutContainer::new(IntrinsicFittingLayout, content))
            }
        }
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Measures the wide branch's natural size and selects wide only when it fits.
struct IntrinsicFittingLayout;

impl IntrinsicFittingLayout {
    /// The chosen index and the size to report.
    fn selection(&self, proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> (usize, Size) {
        let Some(wide) = subviews.first() else {
            return (0, Size::default());
        };
        // An unspecified proposal measures the natural size; a parent width must
        // not make a wrapping wide branch look like it fits.
        let natural = wide.size_that_fits(ProposedViewSize::UNSPECIFIED);
        let fits_width = proposal
            .width
            .is_none_or(|width| natural.width <= width + 0.5);
        let fits_height = proposal
            .height
            .is_none_or(|height| natural.height <= height + 0.5);
        if fits_width && fits_height {
            (0, wide.size_that_fits(proposal))
        } else {
            let index = 1.min(subviews.len().saturating_sub(1));
            (
                index,
                subviews
                    .get(index)
                    .map_or(Size::default(), |sub| sub.size_that_fits(proposal)),
            )
        }
    }
}

impl Layout for IntrinsicFittingLayout {
    fn size_that_fits(&self, proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> Size {
        self.selection(proposal, subviews).1
    }

    fn place_subviews(
        &self,
        bounds: Rect,
        _proposal: ProposedViewSize,
        subviews: &[LayoutSubview],
    ) {
        let Some(wide) = subviews.first() else { return };
        let natural = wide.size_that_fits(ProposedViewSize::UNSPECIFIED);
        let fits = natural.width <= bounds.width + 0.5 && natural.height <= bounds.height + 0.5;
        let chosen = if fits {
            0
        } else {
            1.min(subviews.len().saturating_sub(1))
        };
        let proposal = ProposedViewSize::new(Some(bounds.width), Some(bounds.height));
        for (index, subview) in subviews.iter().enumerate() {
            subview.place(
                Point {
                    x: bounds.x,
                    y: bounds.y,
                },
                proposal,
                index == chosen,
            );
        }
    }
}

/// A branch that stays mounted but is inert while inactive.
///
/// It is created the first time it becomes active and then keeps its identity and
/// local state. While inactive it takes no layout space, paints nothing, and
/// receives no input — and if it holds focus or pointer capture, both are
/// released so a hidden branch cannot keep the session.
struct RetainedBranch {
    is_active: bool,
    content: AnyView,
}

impl RetainedBranch {
    fn new(is_active: bool, content: AnyView) -> Self {
        Self { is_active, content }
    }
}

const BRANCH_ACTIVATED: &str = "__responsiveBranchActivated";

/// Whether `candidate` is `ancestor` or one of its descendants.
fn is_within(candidate: &Node, ancestor: &Node) -> bool {
    let mut current = Some(candidate.clone());
    while let Some(node) = current {
        if node.id() == ancestor.id() {
            return true;
        }
        current = node.parent();
    }
    false
}

impl PrimitiveView for RetainedBranch {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_align_items(Align::Stretch);
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_display(if self.is_active {
            Display::Flex
        } else {
            Display::None
        });
    }

    fn update_node(&self, node: &Node) {
        if self.is_active {
            node.set_attachment(BRANCH_ACTIVATED, true);
        }
        node.set_is_interaction_enabled(self.is_active);
        node.set_opacity(if self.is_active { 1.0 } else { 0.0 });
        if !self.is_active {
            // A hidden branch must not keep focus or pointer capture, or it
            // would swallow input while invisible.
            if let Some(chain) = cove_core::scene::FocusChainHolder::current()
                && chain
                    .focused()
                    .is_some_and(|focused| is_within(&focused, node))
            {
                chain.clear();
            }
            if let Some(capture) = cove_core::scene::PointerCaptureHolder::current()
                && capture
                    .target()
                    .is_some_and(|target| is_within(&target, node))
            {
                capture.release();
            }
        }
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        // Swift creates the branch body only once it has been activated, so an
        // inactive branch that has never been shown materialises nothing.
        match node.attachment::<bool>(BRANCH_ACTIVATED) {
            Some(true) => vec![self.content.clone()],
            _ => Vec::new(),
        }
    }
}

impl_primitive_view!(RetainedBranch);
