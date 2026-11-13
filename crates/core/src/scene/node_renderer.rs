//! Translation of CoveUIScene/NodeRenderer.swift.
//!
//! Walks a `Node` tree post-layout and emits draw commands into a `DrawList`.
//!
//! Convention: `Node::frame` is interpreted as parent-local; the renderer
//! carries a placement down the tree rather than a bare origin, so a node's
//! `transform` reaches the subtree it scales. Background fills and the
//! `Node::draw` hook (used by Text/Image) are emitted in that order so custom
//! content paints over its own background. `clips_to_bounds` pushes a scissor
//! for the duration of the subtree.
use super::rendering::{content_mask, paint_scaled};
use super::{Affine2D, Node};
use crate::{Color, DrawList, LayeredShadow, Point, Rect};

pub struct NodeRenderer;
impl NodeRenderer {
    pub fn new() -> Self {
        Self
    }

    /// Render `root` into `list`. Coordinates are produced in viewport pixels.
    pub fn render(&self, root: &Node, list: &mut DrawList) {
        let frame = root.frame();
        list.set_viewport_bounds(Some(Rect::new(frame.x, frame.y, frame.width, frame.height)));
        render_node(root, list, Affine2D::IDENTITY);
    }
}
impl Default for NodeRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// `placed` is where the parent's content sits; the node composes its own
/// placement onto it. Carrying the whole transform rather than a resolved
/// origin is what lets a scale reach this node's children — an origin alone has
/// nowhere to record how much bigger or smaller they should be drawn.
fn render_node(node: &Node, list: &mut DrawList, placed: Affine2D) {
    if !node.is_layout_visible() {
        return;
    }
    let first_vertex = list.vertices().len();
    let frame = node.frame();
    let own = node.placement_step(Point::default()).followed(placed);
    let origin = own.origin();
    let (abs_x, abs_y) = (origin.x, origin.y);
    let scale = own.a;
    let width = frame.width * scale;
    let height = frame.height * scale;

    // Every length on the node's own box is measured in the node's units, so it
    // scales with the box. A 10pt corner drawn at 0.96 is a 9.6pt corner —
    // leaving it at 10 would make the panel visibly rounder as it grew.
    let radius = node.corner_radius() * scale;
    let border_width = node.border_width() * scale;

    // 0. A reveal describes the frame, not this box.
    if let Some(reveal) = node.reveal_request() {
        list.begin_reveal(reveal);
    }

    // 1. Drop shadow (painted before background so it sits behind). The ambient
    //    halo goes down first; the tighter key shadow lands on it.
    let rect = Rect::new(abs_x, abs_y, width, height);
    // A frosted panel samples the frame behind it, so keep its own shadow out
    // of the band it samples — otherwise the frost eats the shadow. An opaque
    // background covers the shadow's interior the moment it is painted, so clip
    // to that too and keep only the visible fringe.
    let occluded = node.background_color().is_some().then_some(rect);
    list.add_shadow(
        LayeredShadow {
            ambient: node.ambient_shadow().scaled(own.shadow_scale),
            key: node.shadow().scaled(own.shadow_scale),
        },
        rect,
        radius,
        node.opacity(),
        (node.backdrop_blur() > 0.0).then_some(rect),
        occluded,
    );

    // 2. Clip stack push (covers background, content, and children; not the
    //    node's own shadow, which falls outside its box).
    let clipped = node.clips_to_bounds() && width > 0.0 && height > 0.0;
    if clipped {
        list.push_clip_rect(Rect::new(abs_x, abs_y, width, height));
    }

    // 3. Border ring, including when the background is transparent. A frosted
    //    node draws it after the fill: the backdrop composite is attached to
    //    the fill's batch and would otherwise cover the ring.
    let frosted = node.backdrop_blur() > 0.0 && node.background_color().is_some();
    if !frosted {
        emit_border(node, list, rect, radius, border_width);
    }

    // 4. Background fill (inset by border width so it does not overdraw the
    //    border ring).
    emit_background(
        node,
        list,
        abs_x,
        abs_y,
        width,
        height,
        radius,
        border_width,
    );
    if frosted {
        emit_border(node, list, rect, radius, border_width);
    }

    // 5. Custom content (Text/Image/etc). A rounded clipping container keeps it
    //    and its children inside the inner rounded edge.
    let mask = content_mask(node, abs_x, abs_y, width, height, radius, border_width);
    if let Some(mask) = mask {
        list.push_mask(mask);
    }
    if let Some(draw) = node.draw() {
        paint_scaled(&draw, list, Point::new(abs_x, abs_y), scale);
    }
    // Record a real paint of this node's own box or content. A node that only
    // places children emits no vertices of its own, so it keeps its
    // generation; the bump is diagnostic state and never dirties the node.
    if list.vertices().len() > first_vertex {
        node.bump_repaint();
    }

    // 6. Children — placed where this node's *content* sits, which is its own
    //    frame slid by -content_offset and then scaled. The offset stays
    //    fractional throughout motion and at rest.
    let content = node.placement_step(node.content_offset()).followed(placed);
    let descendant_start = list.vertices().len();
    let children = if node.children_may_need_z_sort() {
        node.render_ordered_children()
    } else {
        node.children()
    };
    for child in &children {
        render_node(child, list, content);
    }
    if mask.is_some() {
        list.pop_mask();
    }

    // 7. Overlay (scrollbars, focus rings drawn above content).
    list.multiply_opacity(descendant_start, node.opacity());
    if let Some(overlay) = node.overlay_draw() {
        paint_scaled(&overlay, list, Point::new(abs_x, abs_y), scale);
    }

    list.multiply_opacity(first_vertex, node.subtree_opacity());
    // 8. Pop clip.
    if clipped {
        list.pop_clip_rect();
    }
}

pub(crate) fn emit_border(
    node: &Node,
    list: &mut DrawList,
    rect: Rect,
    radius: f32,
    border_width: f32,
) {
    let Some(color) = node.border_color() else {
        return;
    };
    if border_width <= 0.0 || rect.width <= 0.0 || rect.height <= 0.0 {
        return;
    }
    let _ = list.add_rounded_rect_stroke(
        rect,
        radius,
        border_width,
        apply_opacity(color, node.opacity()),
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_background(
    node: &Node,
    list: &mut DrawList,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
    border_width: f32,
) {
    if node.background_color().is_none() && node.background_gradient().is_none() {
        return;
    }
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let inset = if node.border_color().is_some() && node.border_width() > 0.0 {
        border_width
    } else {
        0.0
    };
    let bg_width = (width - 2.0 * inset).max(0.0);
    let bg_height = (height - 2.0 * inset).max(0.0);
    if bg_width <= 0.0 || bg_height <= 0.0 {
        return;
    }
    let rect = Rect::new(x + inset, y + inset, bg_width, bg_height);
    let bg_radius = (radius - inset).max(0.0);
    // The backdrop covers the whole node, not just the fill: the border ring is
    // drawn on top of it and must stay crisp.
    if node.backdrop_blur() > 0.0 {
        list.begin_backdrop(crate::Backdrop {
            rect: Rect::new(x, y, width, height),
            corner_radius: radius,
            blur_radius: node.backdrop_blur(),
            opacity: node.opacity(),
        });
    }
    if let Some(gradient) = node.background_gradient() {
        let _ =
            list.add_linear_gradient(rect, bg_radius, gradient.multiplied_alpha(node.opacity()));
    } else if let Some(background) = node.background_color() {
        let color = apply_opacity(background, node.opacity());
        if bg_radius > 0.0 {
            let _ = list.add_rounded_rect(rect, bg_radius, color);
        } else {
            let _ = list.add_rect(rect, color);
        }
    }
}

pub(crate) fn apply_opacity(color: Color, opacity: f32) -> Color {
    if opacity >= 1.0 {
        return color;
    }
    Color {
        r: color.r,
        g: color.g,
        b: color.b,
        a: color.a * opacity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_is_inset_by_the_border_width() {
        let node = Node::new();
        node.set_frame(Rect::new(0.0, 0.0, 20.0, 20.0));
        node.set_background_color(Some(Color::BLACK));
        node.set_border_color(Some(Color::WHITE));
        node.set_border_width(2.0);
        let mut list = DrawList::default();
        NodeRenderer::new().render(&node, &mut list);
        // Border ring first, then the inset fill. The ring is an analytic
        // coverage stroke, so it feathers half a device pixel past the frame on
        // each side — that halo is what makes the edge smooth rather than
        // stair-stepped, and it is not part of the geometric bounds.
        let bounds = list.paint_bounds().unwrap();
        let feather = 0.5 / list.pixel_scale.max(1.0);
        assert!(
            (bounds.x + feather).abs() < 1e-4,
            "bounds start one feather before the frame, got {bounds:?}"
        );
        assert!((bounds.width - (20.0 + 2.0 * feather)).abs() < 1e-4);
        assert!(!list.vertices().is_empty());
        // The fill itself must stay inset by the full border width.
        assert!(bounds.width < 20.0 + 2.0);
    }

    #[test]
    fn hidden_subtrees_emit_nothing() {
        let root = Node::new();
        root.set_frame(Rect::new(0.0, 0.0, 10.0, 10.0));
        let child = Node::new();
        child.set_frame(Rect::new(0.0, 0.0, 10.0, 10.0));
        child.set_background_color(Some(Color::BLACK));
        child.set_layout_visibility(false);
        root.add_child(&child);
        let mut list = DrawList::default();
        NodeRenderer::new().render(&root, &mut list);
        assert!(list.vertices().is_empty());
    }

    #[test]
    fn painting_bumps_the_repaint_epoch_on_every_frame() {
        let node = Node::new();
        node.set_frame(Rect::new(0.0, 0.0, 20.0, 20.0));
        node.set_background_color(Some(Color::BLACK));
        let mut list = DrawList::default();
        let renderer = NodeRenderer::new();
        renderer.render(&node, &mut list);
        assert_eq!(node.repaint_epoch(), 1, "the first frame paints the node");
        renderer.render(&node, &mut DrawList::default());
        assert_eq!(
            node.repaint_epoch(),
            2,
            "the uncached renderer repaints the node again"
        );
    }

    #[test]
    fn nodes_that_only_place_children_keep_their_epoch() {
        let root = Node::new();
        root.set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
        let wrapper = Node::new();
        wrapper.set_frame(Rect::new(5.0, 5.0, 50.0, 50.0));
        let painted = Node::new();
        painted.set_frame(Rect::new(0.0, 0.0, 10.0, 10.0));
        painted.set_background_color(Some(Color::BLACK));
        let hidden = Node::new();
        hidden.set_frame(Rect::new(0.0, 0.0, 10.0, 10.0));
        hidden.set_background_color(Some(Color::BLACK));
        hidden.set_layout_visibility(false);
        wrapper.add_child(&painted);
        wrapper.add_child(&hidden);
        root.add_child(&wrapper);
        NodeRenderer::new().render(&root, &mut DrawList::default());
        assert_eq!(
            (root.repaint_epoch(), wrapper.repaint_epoch()),
            (0, 0),
            "a node that emits nothing of its own is not a paint"
        );
        assert_eq!(
            painted.repaint_epoch(),
            1,
            "a painted child records its own paint"
        );
        assert_eq!(hidden.repaint_epoch(), 0, "a hidden node never paints");
    }

    #[test]
    fn content_offset_moves_children() {
        let root = Node::new();
        root.set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
        root.set_content_offset(Point::new(0.0, 10.0));
        let child = Node::new();
        child.set_frame(Rect::new(0.0, 0.0, 10.0, 10.0));
        child.set_background_color(Some(Color::BLACK));
        root.add_child(&child);
        let mut list = DrawList::default();
        NodeRenderer::new().render(&root, &mut list);
        assert_eq!(
            list.paint_bounds().unwrap(),
            Rect::new(0.0, -10.0, 10.0, 10.0)
        );
    }
}
