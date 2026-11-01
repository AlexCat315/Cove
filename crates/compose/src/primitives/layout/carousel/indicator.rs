//! Translation of Primitives/Layout/Carousel/Indicator.swift.
//!
//! How a carousel shows position. `.automatic` uses dots up to eight pages and
//! a "3 / 12" count beyond, where dots stop being countable.
use std::rc::Rc;

use cove_composition::{AnyView, PrimitiveView, View};
use cove_core::scene::{LayoutNode, Node};
use cove_core::{Color, Rect};

use super::autoplay::CarouselAutoplaySession;
use crate::impl_primitive_view;

/// Position indicator style.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CarouselIndicator {
    Automatic,
    Dots,
    Fraction,
    Hidden,
}

impl CarouselIndicator {
    pub fn resolved(self, page_count: usize) -> CarouselIndicator {
        if page_count <= 1 {
            return CarouselIndicator::Hidden;
        }
        if self != CarouselIndicator::Automatic {
            return self;
        }
        if page_count <= 8 {
            CarouselIndicator::Dots
        } else {
            CarouselIndicator::Fraction
        }
    }
}

/// The active dot: an accent pill that fills with the autoplay interval, or
/// stays solid when nothing rotates.
pub struct CarouselProgressDot {
    session: Rc<CarouselAutoplaySession>,
    /// `None` when not rotating; the pill is then solid.
    interval: Option<f64>,
}

impl CarouselProgressDot {
    pub fn new(session: Rc<CarouselAutoplaySession>, interval: Option<f64>) -> Self {
        Self { session, interval }
    }
}

impl PrimitiveView for CarouselProgressDot {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_width(Some(16.0));
        layout.set_height(Some(7.0));
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        self.session.set_progress_node(Some(node.clone()));
        let accent = crate::theme::theme_for_node(node).colors.accent;
        let session = self.session.clone();
        let interval = self.interval;
        let owned = node.clone();
        node.update_draw(interval, move |list, origin| {
            let frame = owned.frame();
            let rect = Rect::new(origin.x, origin.y, frame.width, frame.height);
            let Some(interval) = interval else {
                let _ = list.add_rounded_rect(rect, rect.height / 2.0, accent);
                return;
            };
            let _ = list.add_rounded_rect(rect, rect.height / 2.0, accent.multiplied_alpha(0.3));
            let filled = rect.height.max(rect.width * session.progress(interval));
            let _ = list.add_rounded_rect(
                Rect::new(rect.x, rect.y, filled, rect.height),
                rect.height / 2.0,
                accent,
            );
        });
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        Vec::new()
    }
}
impl_primitive_view!(CarouselProgressDot);
