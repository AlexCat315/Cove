//! Adaptive popup placement translated from Portal/PortalPlacement.swift.
use cove_core::scene::Node;
use cove_core::{Point, Rect, Size};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PortalPlacementMode {
    #[default]
    BelowAnchor,
    BesideAnchor,
    /// The owner already chose a side and available region; clamp to it.
    Fixed,
}

pub struct PortalPlacement;

impl PortalPlacement {
    pub fn fit(
        position: Point,
        size: Size,
        window: Rect,
        anchor: Option<Rect>,
        margin: f32,
        placement: PortalPlacementMode,
    ) -> Rect {
        let inset_x = margin.min(window.width / 2.0);
        let inset_y = margin.min(window.height / 2.0);
        let bounds = Rect::new(
            window.x + inset_x,
            window.y + inset_y,
            window.width - inset_x * 2.0,
            window.height - inset_y * 2.0,
        );
        let width = size.width.max(0.0).min(bounds.width.max(0.0));
        let height = size.height.max(0.0).min(bounds.height.max(0.0));
        let mut x = position.x;
        let mut y = position.y;
        if placement == PortalPlacementMode::BesideAnchor
            && x + width > bounds.max_x()
            && let Some(anchor) = anchor
        {
            let left_space = anchor.x - bounds.x;
            let right_space = bounds.max_x() - anchor.max_x();
            if left_space >= width || left_space > right_space {
                x = anchor.x - width;
            }
        }
        x = x.max(bounds.x).min(bounds.max_x() - width);
        if placement == PortalPlacementMode::BelowAnchor
            && y + height > bounds.max_y()
            && let Some(anchor) = anchor
            && anchor.y - height >= bounds.y
        {
            y = anchor.y - height;
        }
        y = y.max(bounds.y).min(bounds.max_y() - height);
        Rect::new(x, y, width, height)
    }
}

/// Places a portal slot at a window-space rect. The slot's frame is relative
/// to its host, which can sit below a safe-area inset or a title bar.
pub fn set_window_frame(node: &Node, window: Rect) {
    node.set_frame(window_frame_in_parent(node, window));
}

/// A window-space rect expressed in `node`'s parent space.
pub fn window_frame_in_parent(node: &Node, window: Rect) -> Rect {
    let origin = node
        .parent()
        .map(|parent| parent.absolute_frame())
        .unwrap_or_default();
    Rect::new(
        window.x - origin.x,
        window.y - origin.y,
        window.width,
        window.height,
    )
}

pub fn portal_window_bounds(node: &Node) -> Rect {
    let mut root = node.clone();
    while let Some(parent) = root.parent() {
        root = parent;
    }
    let frame = root.frame();
    let layout = root.layout_node();
    Rect::new(
        0.0,
        0.0,
        if frame.width > 0.0 {
            frame.width
        } else {
            layout
                .as_ref()
                .and_then(|layout| layout.width())
                .unwrap_or(1024.0)
        },
        if frame.height > 0.0 {
            frame.height
        } else {
            layout
                .as_ref()
                .and_then(|layout| layout.height())
                .unwrap_or(768.0)
        },
    )
}
