//! Translation of CoveUIScene/LayerAwareNodeRenderer.swift.
//!
//! Executes a flat paint plan and records each layer-root subtree into its own
//! cached `DrawList`, then composites cached layer slices into the frame
//! `DrawList` by appending their vertices/indices.
//!
//! Cache invariants per layer-root `RenderObject`:
//! - `cache_invalid == false` AND
//! - the layer's placement at composite time matches the one captured when the
//!   cache was recorded (`last_placement`: position, paint and shadow scales)
//!   AND
//! - the layer's parent clip stack at composite time matches the one captured
//!   when the cache was recorded (`last_clip_stack`).
//!
//! Stable unclipped leaf layers may move under the same parent clip without
//! re-recording. Their cached vertices are translated while being appended;
//! this is the common path for text and controls moving during scrolling. That
//! shortcut needs the scale to be unchanged as well — translating vertices
//! cannot resize them, so a scaling layer has to re-record.
//!
//! Visible geometry matches `NodeRenderer` for the same tree. Completely
//! clipped subtrees and cached offscreen leaves can be omitted. Cached layers
//! append as independent batch runs rather than coalescing across boundaries.
use super::node_renderer::{emit_background, emit_border};
use super::render_culling::RenderCulling;
use super::rendering::{Painter, content_mask, paint_scaled};
use super::{Affine2D, Node, RenderObject, RenderTree};
use crate::{Clip, DrawList, LayeredShadow, Point, Rect, RoundedRect};
use std::rc::Rc;

pub struct LayerAwareNodeRenderer {
    content_placements: Vec<Affine2D>,
}

struct OpenNode {
    object: Option<RenderObject>,
    node: Option<Node>,
    end: usize,
    /// `Some` when this record records into its own layer list; `None` when it
    /// paints into the nearest enclosing list. A record that owns a list is a
    /// layer whose slice `finish` caches.
    list: Option<DrawList>,
    own: Affine2D,
    /// Shared, so descending into a child that adds no clip costs no copy.
    parent_clips: Rc<[Clip]>,
    child_clips: Rc<[Clip]>,
    allows_container_cache: bool,
    first_vertex: usize,
    descendant_start: usize,
    mask: Option<RoundedRect>,
    clipped: bool,
    inherited_clip_revision: u64,
}

/// The list primitives are currently recorded into: the nearest enclosing
/// record that owns one. The bottom (frame) record always owns its list.
fn target_list(open: &mut [OpenNode]) -> &mut DrawList {
    for record in open.iter_mut().rev() {
        if let Some(list) = &mut record.list {
            return list;
        }
    }
    unreachable!("the frame record always owns a list")
}

impl LayerAwareNodeRenderer {
    pub fn new() -> Self {
        Self {
            content_placements: Vec::new(),
        }
    }

    /// Execute the cached preorder plan. Exit records preserve painter, mask,
    /// overlay and opacity ordering without using the call stack.
    pub fn render(&mut self, tree: &RenderTree, frame: &mut DrawList) {
        let Some(root_object) = tree.root() else {
            return;
        };
        let Some(root_node) = root_object.node() else {
            return;
        };
        let root_frame = root_node.frame();
        frame.set_viewport_bounds(Some(Rect::new(
            root_frame.x,
            root_frame.y,
            root_frame.width,
            root_frame.height,
        )));
        let plan = tree.paint_plan();
        // Keep the hot buffer locally owned, avoiding a field access for every
        // node. A reentrant render borrows its own independent buffer.
        let mut placements = std::mem::take(&mut self.content_placements);
        if placements.len() != plan.objects.len() {
            placements = vec![Affine2D::IDENTITY; plan.objects.len()];
        }
        let mut open: Vec<OpenNode> = vec![OpenNode {
            object: None,
            node: None,
            end: usize::MAX,
            list: Some(std::mem::take(frame)),
            own: Affine2D::IDENTITY,
            parent_clips: Rc::from([]),
            child_clips: Rc::from([]),
            allows_container_cache: true,
            first_vertex: 0,
            descendant_start: 0,
            mask: None,
            clipped: false,
            inherited_clip_revision: 0,
        }];
        let mut index = 0;
        while index < plan.objects.len() || open.len() > 1 {
            if open.len() > 1 && index >= open.last().map_or(0, |record| record.end) {
                finish(&mut open);
                continue;
            }
            if index >= plan.objects.len() {
                break;
            }
            let object = plan.objects[index].clone();
            let end = plan.subtree_ends[index];
            let Some(node) = object.node() else {
                index = end;
                continue;
            };
            if !node.is_layout_visible() {
                index = end;
                continue;
            }
            let parent_index = plan.parents[index];
            let placed = if parent_index >= 0 {
                placements[parent_index as usize]
            } else {
                Affine2D::IDENTITY
            };
            let parent_clips = open
                .last()
                .map_or_else(|| Rc::from([]), |record| record.child_clips.clone());
            let allows_cache = open
                .last()
                .is_none_or(|record| record.allows_container_cache);
            let own = node.placement_step(Point::default()).followed(placed);
            let bounds = Rect::new(
                own.e,
                own.f,
                node.frame().width * own.a,
                node.frame().height * own.a,
            );
            if node.clips_to_bounds() && bounds.width > 0.0 && bounds.height > 0.0 {
                let destination = target_list(&mut open);
                if RenderCulling::is_outside(bounds, destination) {
                    index = end;
                    continue;
                }
            }

            let has_children = object.has_children();
            if has_children {
                placements[index] = if node.content_offset() == Point::default() {
                    own
                } else {
                    node.placement_step(node.content_offset()).followed(placed)
                };
            }
            // Pure structural wrappers contribute only placement. They need
            // neither a paint record nor an exit operation.
            if !object.is_layer_root()
                && node.opacity() == 1.0
                && node.subtree_opacity() == 1.0
                && node.background_color().is_none()
                && node.background_gradient().is_none()
                && (node.border_color().is_none() || node.border_width() <= 0.0)
                && node.reveal_request().is_none()
            {
                index += 1;
                continue;
            }

            let mut layer: Option<DrawList> = None;
            let mut child_allows_cache = allows_cache;
            let mut inherited_clip_revision = 0;
            if object.is_layer_root() {
                let placement_unchanged = object.last_placement() == Some(own);
                let clip_unchanged = object.last_clip_stack_eq(&parent_clips);
                let reused = {
                    let destination = target_list(&mut open);
                    reuse(
                        &object,
                        &node,
                        destination,
                        own,
                        &parent_clips,
                        placement_unchanged,
                        clip_unchanged,
                    )
                };
                if reused {
                    index = end;
                    continue;
                }
                let defer_container = has_children
                    && (!allows_cache
                        || object.cache_invalid()
                        || !placement_unchanged
                        || !clip_unchanged);
                if !defer_container {
                    let mut list = DrawList::default();
                    {
                        let destination = target_list(&mut open);
                        list.pixel_scale = destination.pixel_scale;
                        list.geometry_antialiasing = destination.geometry_antialiasing;
                        list.set_viewport_bounds(destination.viewport_bounds);
                    }
                    for clip in parent_clips.iter() {
                        list.push_clip(*clip);
                    }
                    inherited_clip_revision = list.clip_revision;
                    layer = Some(list);
                    child_allows_cache = false;
                }
            }
            begin(
                &mut open,
                &object,
                &node,
                end,
                layer,
                own,
                &parent_clips,
                child_allows_cache,
                inherited_clip_revision,
            );
            index += 1;
        }
        if let Some(frame_record) = open.pop()
            && let Some(list) = frame_record.list
        {
            *frame = list;
        }
        self.content_placements = placements;
    }
}
impl Default for LayerAwareNodeRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::too_many_arguments)]
fn begin(
    open: &mut Vec<OpenNode>,
    object: &RenderObject,
    node: &Node,
    end: usize,
    layer: Option<DrawList>,
    own: Affine2D,
    parent_clips: &Rc<[Clip]>,
    allows_container_cache: bool,
    inherited_clip_revision: u64,
) {
    let scale = own.a;
    let (x, y) = (own.e, own.f);
    let width = node.frame().width * scale;
    let height = node.frame().height * scale;
    let radius = node.corner_radius() * scale;
    let border_width = node.border_width() * scale;
    let clipped = node.clips_to_bounds() && width > 0.0 && height > 0.0;
    let clip = Rect::new(x, y, width, height);
    let mask = content_mask(node, x, y, width, height, radius, border_width);
    let child_clips: Rc<[Clip]> = if clipped {
        parent_clips
            .iter()
            .copied()
            .chain(std::iter::once(Clip { rect: clip, mask }))
            .collect()
    } else {
        parent_clips.clone()
    };
    open.push(OpenNode {
        object: Some(object.clone()),
        node: Some(node.clone()),
        end,
        list: layer,
        own,
        parent_clips: parent_clips.clone(),
        child_clips,
        allows_container_cache,
        first_vertex: 0,
        descendant_start: 0,
        mask,
        clipped,
        inherited_clip_revision,
    });
    let first_vertex = target_list(open).vertices().len();
    if let Some(reveal) = node.reveal_request() {
        target_list(open).begin_reveal(reveal);
    }
    if clipped {
        target_list(open).push_clip_rect(clip);
    }
    let rect = Rect::new(x, y, width, height);
    let occluded = node.background_color().is_some().then_some(rect);
    let layers = LayeredShadow {
        ambient: node.ambient_shadow().scaled(own.shadow_scale),
        key: node.shadow().scaled(own.shadow_scale),
    };
    target_list(open).add_shadow(
        layers,
        rect,
        radius,
        node.opacity(),
        (node.backdrop_blur() > 0.0).then_some(rect),
        occluded,
    );
    let frosted = node.backdrop_blur() > 0.0 && node.background_color().is_some();
    if !frosted {
        emit_border(node, target_list(open), rect, radius, border_width);
    }
    emit_background(
        node,
        target_list(open),
        x,
        y,
        width,
        height,
        radius,
        border_width,
    );
    if frosted {
        emit_border(node, target_list(open), rect, radius, border_width);
    }
    if let Some(mask) = mask {
        target_list(open).push_mask(mask);
    }
    if let Some(draw) = node.draw() {
        paint_scaled(&draw, target_list(open), Point::new(x, y), scale);
    }
    let descendant_start = target_list(open).vertices().len();
    let record = open.last_mut().expect("the record was just pushed");
    record.first_vertex = first_vertex;
    record.descendant_start = descendant_start;
}

fn finish(open: &mut Vec<OpenNode>) {
    let Some(record) = open.last() else { return };
    let mask = record.mask;
    let clipped = record.clipped;
    let opacity = record.node.as_ref().map_or(1.0, Node::opacity);
    let subtree_opacity = record.node.as_ref().map_or(1.0, Node::subtree_opacity);
    let overlay: Option<Painter> = record.node.as_ref().and_then(Node::overlay_draw);
    let (own, first_vertex, descendant_start) =
        (record.own, record.first_vertex, record.descendant_start);
    {
        let list = target_list(open);
        if mask.is_some() {
            list.pop_mask();
        }
        list.multiply_opacity(descendant_start, opacity);
        if let Some(overlay) = &overlay {
            paint_scaled(overlay, list, Point::new(own.e, own.f), own.a);
        }
        list.multiply_opacity(first_vertex, subtree_opacity);
        if clipped {
            list.pop_clip_rect();
        }
    }
    let Some(record) = open.pop() else { return };
    let Some(object) = record.object else { return };
    if !object.is_layer_root() {
        return;
    }
    object.set_last_placement(Some(record.own));
    object.set_last_clip_stack(record.parent_clips.to_vec());
    object.set_cache_invalid(false);
    let Some(mut list) = record.list else {
        object.set_cached_layer_list(None);
        return;
    };
    for clip in record.parent_clips.iter().rev() {
        list.pop_clip(*clip);
    }
    object.set_cached_layer_uses_local_clips(list.clip_revision != record.inherited_clip_revision);
    object.set_cached_paint_bounds(if !object.has_children() {
        RenderCulling::bounds(&list)
    } else {
        None
    });
    object.set_cached_layer_list(Some(list.clone()));
    if let Some(bounds) = object.cached_paint_bounds() {
        let destination = target_list(open);
        if RenderCulling::is_outside(bounds, destination) {
            return;
        }
    }
    let destination = target_list(open);
    let _ = destination.append_preclipped(&list, &record.parent_clips, Point::default());
}

/// A cache hit also handles offscreen leaves: its whole subtree is skipped.
fn reuse(
    object: &RenderObject,
    node: &Node,
    list: &mut DrawList,
    own: Affine2D,
    clips: &[Clip],
    placement_unchanged: bool,
    clip_unchanged: bool,
) -> bool {
    if object.cache_invalid() || !clip_unchanged {
        return false;
    }
    let Some(cached) = object.cached_layer_list() else {
        return false;
    };
    if cached.pixel_scale != list.pixel_scale
        || cached.geometry_antialiasing != list.geometry_antialiasing
    {
        return false;
    }
    if !object.has_children()
        && let Some(bounds) = object.cached_paint_bounds()
        && let Some(old) = object.last_placement()
        && own.a == old.a
        && own.shadow_scale == old.shadow_scale
    {
        let moved = Rect::new(
            bounds.x + own.e - old.e,
            bounds.y + own.f - old.f,
            bounds.width,
            bounds.height,
        );
        if RenderCulling::is_outside(moved, list) {
            return true;
        }
    }
    if placement_unchanged {
        let _ = list.append_preclipped(&cached, clips, Point::default());
        return true;
    }
    if !object.has_children()
        && !node.clips_to_bounds()
        && !object.cached_layer_uses_local_clips()
        && let Some(old) = object.last_placement()
        && own.a == old.a
        && own.shadow_scale == old.shadow_scale
        && !crosses_rounded_corner(object.cached_paint_bounds(), old, own, clips)
    {
        let _ = list.append_preclipped(&cached, clips, Point::new(own.e - old.e, own.f - old.f));
        return true;
    }
    false
}

/// A cached leaf was clipped where it was recorded. If the mask cut any of it
/// there, or it would touch a rounded corner after moving, re-record it instead
/// of translating geometry that was cut for the old position. A leaf recorded
/// entirely outside the mask kept its vertices but lost every triangle, so
/// moving it into view would paint nothing.
fn crosses_rounded_corner(
    bounds: Option<Rect>,
    old: Affine2D,
    new: Affine2D,
    clips: &[Clip],
) -> bool {
    let masks: Vec<RoundedRect> = clips
        .iter()
        .filter_map(|clip| clip.mask)
        .filter(|mask| mask.radius > 0.0)
        .collect();
    if masks.is_empty() {
        return false;
    }
    let Some(bounds) = bounds else { return true };
    let moved = Rect::new(
        bounds.x + new.e - old.e,
        bounds.y + new.f - old.f,
        bounds.width,
        bounds.height,
    );
    masks
        .iter()
        .any(|mask| !mask.contains_bounds(bounds) || mask.touches_corner(moved))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Color;

    fn scene() -> (Node, Node, RenderTree) {
        let root = Node::new();
        root.set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
        let card = Node::new();
        card.set_frame(Rect::new(10.0, 10.0, 50.0, 50.0));
        card.set_background_color(Some(Color::BLACK));
        // Only a layer root owns a cached `DrawList`; a plain painted node is
        // drawn straight into its parent's list. Clipping makes this card a
        // layer, which is the thing under test here.
        card.set_clips_to_bounds(true);
        root.add_child(&card);
        let tree = RenderTree::new();
        tree.install(&root);
        (root, card, tree)
    }

    #[test]
    fn first_frame_records_then_reuses_the_cache() {
        let (_root, card, tree) = scene();
        let mut renderer = LayerAwareNodeRenderer::new();
        let mut frame = DrawList::default();
        renderer.render(&tree, &mut frame);
        let first = frame.vertices().len();
        assert!(first > 0);

        let object = tree.render_object(&card).unwrap();
        assert!(!object.cache_invalid());

        let mut second = DrawList::default();
        renderer.render(&tree, &mut second);
        assert_eq!(second.vertices().len(), first);
    }

    #[test]
    fn invalidated_layer_rerecords() {
        let (_root, card, tree) = scene();
        let mut renderer = LayerAwareNodeRenderer::new();
        let mut frame = DrawList::default();
        renderer.render(&tree, &mut frame);
        let object = tree.render_object(&card).unwrap();
        object.set_cache_invalid(true);
        let mut second = DrawList::default();
        renderer.render(&tree, &mut second);
        assert!(!second.vertices().is_empty());
        assert!(!object.cache_invalid());
    }

    #[test]
    fn structural_wrappers_emit_nothing() {
        let root = Node::new();
        root.set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
        let wrapper = Node::new();
        wrapper.set_frame(Rect::new(5.0, 5.0, 20.0, 20.0));
        root.add_child(&wrapper);
        let tree = RenderTree::new();
        tree.install(&root);
        let mut renderer = LayerAwareNodeRenderer::new();
        let mut frame = DrawList::default();
        renderer.render(&tree, &mut frame);
        assert!(frame.vertices().is_empty());
    }
}
