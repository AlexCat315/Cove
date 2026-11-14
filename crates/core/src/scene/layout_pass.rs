//! Translation of CoveUIScene/LayoutPass.swift.
use super::{LayoutNode, Node};

/// Applies layout results back to the `Node` tree in a single depth-first pass.
pub struct LayoutPass;
impl LayoutPass {
    /// Calculate layout and write results to every `Node.frame`.
    ///
    /// Both trees must have the same structure (same child count and order at
    /// every level).
    pub fn run(
        root_layout_node: &LayoutNode,
        root_node: &Node,
        available_width: Option<f32>,
        available_height: Option<f32>,
    ) {
        root_layout_node.calculate_layout(available_width, available_height, super::Direction::Ltr);
        apply(root_layout_node, root_node);
    }
}

fn apply(layout_node: &LayoutNode, node: &Node) {
    node.set_frame(finite_frame(node.resolve_layout_frame(layout_node.frame())));
    // Layout observers must run for every pass, including when the resolved
    // frame is unchanged. Frame capture, anchor tracking, and other node-owned
    // resources use this hook to refresh derived geometry.
    if let Some(on_layout_update) = node.layout_did_update() {
        on_layout_update(node);
    }
    let layout_children = layout_node.children();
    let node_children = node.children();
    for (layout_child, node_child) in layout_children.iter().zip(node_children.iter()) {
        apply(layout_child, node_child);
    }
}

/// Collapses a solved frame the layout engine could not bound to a paintable one.
///
/// An unbounded axis (an intrinsic probe, an overlay measured at max content) can
/// solve a literal point extent to infinity, which then lands as `x = -inf,
/// width = inf`. Nothing downstream can use that: slicing and snapping produce
/// NaN vertices, and the RHI rejects the entire frame. A non-finite axis is
/// never meaningful, so it degrades to a zero extent instead of poisoning the
/// draw list.
fn finite_frame(frame: crate::Rect) -> crate::Rect {
    if frame.x.is_finite()
        && frame.y.is_finite()
        && frame.width.is_finite()
        && frame.height.is_finite()
    {
        return frame;
    }
    crate::Rect::new(
        if frame.x.is_finite() { frame.x } else { 0.0 },
        if frame.y.is_finite() { frame.y } else { 0.0 },
        if frame.width.is_finite() {
            frame.width.max(0.0)
        } else {
            0.0
        },
        if frame.height.is_finite() {
            frame.height.max(0.0)
        } else {
            0.0
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rect;

    #[test]
    fn an_unbounded_solve_is_collapsed_to_a_paintable_frame() {
        // An overlay measured at max content once wrote `x = -inf, width = inf`
        // here, which the draw pass turned into NaN vertices at a zero size.
        assert_eq!(
            finite_frame(Rect::new(f32::NEG_INFINITY, 0.0, f32::INFINITY, 44.0)),
            Rect::new(0.0, 0.0, 0.0, 44.0)
        );
        assert_eq!(
            finite_frame(Rect::new(f32::NAN, f32::NAN, 36.0, 4.0)),
            Rect::new(0.0, 0.0, 36.0, 4.0)
        );
        let inside = Rect::new(12.0, 34.0, 36.0, 4.0);
        assert_eq!(finite_frame(inside), inside);
    }

    #[test]
    fn frames_are_written_down_the_matching_trees() {
        let root = Node::new();
        let child = Node::new();
        root.add_child(&child);
        let layout_root = LayoutNode::new();
        layout_root.set_flex_direction(super::super::FlexDirection::Column);
        layout_root.set_width(Some(200.0));
        let layout_child = LayoutNode::new();
        layout_child.set_height(Some(40.0));
        layout_root.add_child(&layout_child);
        LayoutPass::run(&layout_root, &root, Some(200.0), Some(400.0));
        // Yoga's `calculateLayout` stretch-fits the root: this root sets a width
        // but leaves height auto, so under a 400pt owner it becomes exactly 400
        // and the 40pt child sits at the top. Hugging content here was the bug
        // that collapsed every viewport-filling root to height 0.
        assert_eq!(root.frame(), Rect::new(0.0, 0.0, 200.0, 400.0));
        assert_eq!(child.frame(), Rect::new(0.0, 0.0, 200.0, 40.0));
    }

    #[test]
    fn descendants_see_a_resolved_frame_within_the_same_pass() {
        use std::cell::Cell;
        use std::rc::Rc;
        let root = Node::new();
        let panel = Node::new();
        let row = Node::new();
        root.add_child(&panel);
        panel.add_child(&row);
        let layout_root = LayoutNode::new();
        layout_root.set_width(Some(200.0));
        let layout_panel = LayoutNode::new();
        layout_panel.set_height(Some(80.0));
        let layout_row = LayoutNode::new();
        layout_row.set_height(Some(20.0));
        layout_panel.add_child(&layout_row);
        layout_root.add_child(&layout_panel);
        panel.set_frame_resolver(Some(Rc::new(|_, solved: Rect| {
            Rect::new(
                solved.x + 30.0,
                solved.y + 50.0,
                solved.width,
                solved.height,
            )
        })));
        let seen = Rc::new(Cell::new(Rect::default()));
        let probe = seen.clone();
        row.set_layout_did_update(Some(Rc::new(move |node: &Node| {
            probe.set(node.absolute_frame());
        })));
        LayoutPass::run(&layout_root, &root, Some(200.0), Some(400.0));
        assert_eq!(panel.frame(), Rect::new(30.0, 50.0, 200.0, 80.0));
        assert_eq!(
            seen.get(),
            Rect::new(30.0, 50.0, 200.0, 20.0),
            "the row's hook reads its panel's resolved placement"
        );
    }
}
