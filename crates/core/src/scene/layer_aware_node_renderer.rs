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

/// One layer the frame composited, with the index window it contributed.
///
/// A layer owns a contiguous index range in the frame draw list, so a recorded
/// pass draw belongs to the layer whose range contains it. `recorded` is false
/// when the frame reused the layer's cached list; `translated` says that reuse
/// only moved cached vertices instead of re-recording them.
#[derive(Clone, Debug, PartialEq)]
pub struct CompositedLayer {
    /// The layer root's element id, in the same form DevTools reports node ids.
    pub node: Option<String>,
    pub first_index: u64,
    pub index_count: u64,
    pub recorded: bool,
    pub translated: bool,
    /// Bounds of the layer at composite time, in frame coordinates.
    pub bounds: Rect,
}

/// One layer root the culling step dropped before it could composite.
#[derive(Clone, Debug, PartialEq)]
pub struct SkippedLayer {
    pub node: Option<String>,
    pub bounds: Rect,
}

/// Layer census for one frame: what composited, in paint order, plus what was
/// skipped. Skipped entries are the layers an author expects to see and cannot
/// find on screen, so they are recorded rather than silently dropped.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayerCensus {
    pub layers: Vec<CompositedLayer>,
    pub skipped: Vec<SkippedLayer>,
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
    census_start: Option<usize>,
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
        self.render_with_census(tree, frame, None);
    }

    /// Renders the frame and, when `census` is given, records every composited
    /// and skipped layer. The census allocates only when asked for, so the plain
    /// `render` path stays allocation-free.
    pub fn render_with_census(
        &mut self,
        tree: &RenderTree,
        frame: &mut DrawList,
        mut census: Option<&mut LayerCensus>,
    ) {
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
            census_start: None,
        }];
        let mut index = 0;
        while index < plan.objects.len() || open.len() > 1 {
            if open.len() > 1 && index >= open.last().map_or(0, |record| record.end) {
                let event = finish(&mut open, census.as_deref_mut());
                if let (Some(census), Some(event)) = (census.as_deref_mut(), event) {
                    match event {
                        LayerEvent::Composited(layer) => census.layers.push(layer),
                        LayerEvent::Skipped(layer) => census.skipped.push(layer),
                    }
                }
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
                    // A layer root culled here never reaches `finish`, so this is
                    // the only place that can report it.
                    if object.is_layer_root()
                        && let Some(census) = census.as_deref_mut()
                    {
                        census.skipped.push(SkippedLayer {
                            node: Some(node.id().0.to_string()),
                            bounds,
                        });
                    }
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
                let (outcome, first_index) = {
                    let destination = target_list(&mut open);
                    let first_index = destination.indices().len() as u64;
                    let outcome = reuse(
                        &object,
                        &node,
                        destination,
                        own,
                        &parent_clips,
                        placement_unchanged,
                        clip_unchanged,
                    );
                    (outcome, first_index)
                };
                let layer_bounds = Rect::new(
                    own.e,
                    own.f,
                    node.frame().width * own.a,
                    node.frame().height * own.a,
                );
                match outcome {
                    ReuseOutcome::Missed => {}
                    ReuseOutcome::Reused { translated } => {
                        if let Some(census) = census.as_deref_mut() {
                            let index_count =
                                target_list(&mut open).indices().len() as u64 - first_index;
                            census.layers.push(CompositedLayer {
                                node: Some(node.id().0.to_string()),
                                first_index,
                                index_count,
                                recorded: false,
                                translated,
                                bounds: layer_bounds,
                            });
                        }
                        index = end;
                        continue;
                    }
                    ReuseOutcome::Skipped => {
                        if let Some(census) = census.as_deref_mut() {
                            census.skipped.push(SkippedLayer {
                                node: Some(node.id().0.to_string()),
                                bounds: layer_bounds,
                            });
                        }
                        index = end;
                        continue;
                    }
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
            let census_start = layer
                .as_ref()
                .and(census.as_ref())
                .map(|census| census.layers.len());
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
                census_start,
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
    census_start: Option<usize>,
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
        census_start,
    });
    let first_vertex = target_list(open).vertices().len();
    if let Some(reveal) = node.reveal_request() {
        target_list(open).begin_reveal(reveal);
    }
    // A node's own shadow falls outside its box, so it is painted before the
    // node's clip goes on: clipping to bounds clips content, not the shadow.
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
    if clipped {
        target_list(open).push_clip_rect(clip);
    }
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

/// What a collapsing layer root did to the frame draw list.
enum LayerEvent {
    Composited(CompositedLayer),
    Skipped(SkippedLayer),
}

fn layer_bounds(record: &OpenNode) -> Rect {
    let scale = record.own.a;
    match record.node.as_ref() {
        Some(node) => Rect::new(
            record.own.e,
            record.own.f,
            node.frame().width * scale,
            node.frame().height * scale,
        ),
        None => Rect::new(record.own.e, record.own.f, 0.0, 0.0),
    }
}

fn finish(open: &mut Vec<OpenNode>, mut census: Option<&mut LayerCensus>) -> Option<LayerEvent> {
    let Some(record) = open.last() else {
        return None;
    };
    let mask = record.mask;
    let clipped = record.clipped;
    let opacity = record.node.as_ref().map_or(1.0, Node::opacity);
    let subtree_opacity = record.node.as_ref().map_or(1.0, Node::subtree_opacity);
    let overlay: Option<Painter> = record.node.as_ref().and_then(Node::overlay_draw);
    let node = record.node.clone();
    let (own, first_vertex, descendant_start) =
        (record.own, record.first_vertex, record.descendant_start);
    {
        let list = target_list(open);
        if mask.is_some() {
            list.pop_mask();
        }
        list.multiply_opacity(descendant_start, opacity);
        let before_overlay = list.vertices().len();
        if let Some(overlay) = &overlay {
            paint_scaled(overlay, list, Point::new(own.e, own.f), own.a);
        }
        // A cache hit bypasses the node's record. Descendant vertices are
        // excluded from this node's paint count.
        if let Some(node) = node
            && (descendant_start > first_vertex || list.vertices().len() > before_overlay)
        {
            node.bump_repaint();
        }
        list.multiply_opacity(first_vertex, subtree_opacity);
        if clipped {
            list.pop_clip_rect();
        }
    }
    let mut record = open.pop()?;
    let object = record.object.as_ref()?;
    if !object.is_layer_root() {
        return None;
    }
    object.set_last_placement(Some(record.own));
    object.set_last_clip_stack(record.parent_clips.to_vec());
    object.set_cache_invalid(false);
    let Some(mut list) = record.list.take() else {
        // No own list means the container drew straight into its parent's list:
        // it never composited independently, so it is not a layer a reader would
        // look for. A layer that recorded but painted nothing keeps its list and
        // is reported with an empty window instead.
        object.set_cached_layer_list(None);
        return None;
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
    if let Some(paint_bounds) = object.cached_paint_bounds() {
        let destination = target_list(open);
        if RenderCulling::is_outside(paint_bounds, destination) {
            if let Some(start) = record.census_start
                && let Some(census) = census
            {
                census.layers.truncate(start);
                return Some(LayerEvent::Skipped(SkippedLayer {
                    node: record.node.as_ref().map(|node| node.id().0.to_string()),
                    bounds: layer_bounds(&record),
                }));
            }
            return None;
        }
    }
    let destination = target_list(open);
    let first_index = destination.indices().len() as u64;
    let can_rebase = census.is_some() && destination.can_append_preclipped(&record.parent_clips);
    let appended = destination.append_preclipped(&list, &record.parent_clips, Point::default());
    let census = census.take()?;
    if let Some(start) = record.census_start {
        if can_rebase && appended.is_ok() {
            for layer in &mut census.layers[start..] {
                layer.first_index += first_index;
            }
        } else {
            // Reapplying a rounded mask can replace indices. The parent still
            // owns the resulting range, but descendant ranges are unmappable.
            census.layers.truncate(start);
        }
    }
    appended.ok()?;
    let index_count = destination.indices().len() as u64 - first_index;
    Some(LayerEvent::Composited(CompositedLayer {
        node: record.node.as_ref().map(|node| node.id().0.to_string()),
        first_index,
        index_count,
        recorded: true,
        translated: false,
        bounds: layer_bounds(&record),
    }))
}

/// What a layer root's cache lookup did.
enum ReuseOutcome {
    /// Nothing cached, or the cache is unusable: the caller records the subtree.
    Missed,
    /// Reused the cached list. `translated` means only the vertices moved.
    Reused { translated: bool },
    /// The cached layer sits entirely outside the frame, so its subtree was
    /// skipped without appending anything.
    Skipped,
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
) -> ReuseOutcome {
    if object.cache_invalid() || !clip_unchanged {
        return ReuseOutcome::Missed;
    }
    let Some(cached) = object.cached_layer_list() else {
        return ReuseOutcome::Missed;
    };
    if cached.pixel_scale != list.pixel_scale
        || cached.geometry_antialiasing != list.geometry_antialiasing
    {
        return ReuseOutcome::Missed;
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
            return ReuseOutcome::Skipped;
        }
    }
    if placement_unchanged {
        let _ = list.append_preclipped(&cached, clips, Point::default());
        return ReuseOutcome::Reused { translated: false };
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
        return ReuseOutcome::Reused { translated: true };
    }
    ReuseOutcome::Missed
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
    fn the_census_names_the_layer_and_its_index_window() {
        let (_root, card, tree) = scene();
        let mut renderer = LayerAwareNodeRenderer::new();
        let mut frame = DrawList::default();
        let mut census = LayerCensus::default();
        renderer.render_with_census(&tree, &mut frame, Some(&mut census));
        assert_eq!(census.layers.len(), 1, "the clipped card is one layer");
        let layer = &census.layers[0];
        assert_eq!(
            layer.node.as_deref(),
            Some(card.id().0.to_string().as_str())
        );
        assert!(layer.recorded, "a first frame records, it cannot reuse");
        assert_eq!(
            layer.index_count as usize,
            frame.indices().len(),
            "the layer owns every index this frame drew"
        );
        assert_eq!(layer.first_index, 0);
        assert_eq!(
            (layer.bounds.x, layer.bounds.y, layer.bounds.width),
            (10.0, 10.0, 50.0),
            "the census reports the layer's own frame, not the root's"
        );
        assert!(census.skipped.is_empty());
    }

    #[test]
    fn a_cached_layer_reports_reuse_instead_of_recording() {
        let (_root, card, tree) = scene();
        let mut renderer = LayerAwareNodeRenderer::new();
        renderer.render(&tree, &mut DrawList::default());
        let mut frame = DrawList::default();
        let mut census = LayerCensus::default();
        renderer.render_with_census(&tree, &mut frame, Some(&mut census));
        // The root had no own list on the first frame (a container defers), so
        // on the second it composites as a layer of its own and the card's
        // window nests inside it. Windows therefore nest, and a pass that draws
        // the outer window touches both layers.
        let card_layer = census
            .layers
            .iter()
            .find(|layer| layer.node.as_deref() == Some(card.id().0.to_string().as_str()))
            .expect("the card composites on every frame");
        assert!(
            !card_layer.recorded,
            "an unchanged layer comes back from its cache"
        );
        assert!(!card_layer.translated);
        assert_eq!(card_layer.index_count as usize, frame.indices().len());
    }

    #[test]
    fn nested_layers_report_indices_relative_to_the_frame() {
        let (root, card, tree) = scene();
        let inner = Node::new();
        inner.set_frame(Rect::new(4.0, 4.0, 8.0, 8.0));
        inner.set_background_color(Some(Color::WHITE));
        inner.set_clips_to_bounds(true);
        card.add_child(&inner);
        let first = Node::new();
        first.set_frame(Rect::new(0.0, 0.0, 3.0, 3.0));
        first.set_background_color(Some(Color::BLACK));
        root.add_child(&first);
        root.reorder_children(&[first, card.clone()]);
        tree.install(&root);

        let mut renderer = LayerAwareNodeRenderer::new();
        renderer.render(&tree, &mut DrawList::default());
        // Keep the root in the frame list so the card can cache its subtree.
        tree.render_object(&root).unwrap().set_cache_invalid(true);
        let mut frame = DrawList::default();
        let mut census = LayerCensus::default();
        renderer.render_with_census(&tree, &mut frame, Some(&mut census));
        let layer = |node: &Node| {
            let id = node.id().0.to_string();
            census
                .layers
                .iter()
                .find(|layer| layer.node.as_deref() == Some(id.as_str()))
                .unwrap()
        };
        let parent = layer(&card);
        let child = layer(&inner);
        assert!(parent.first_index > 0);
        assert_eq!(child.first_index, parent.first_index + 6);
        assert_eq!(
            child.first_index + child.index_count,
            parent.first_index + parent.index_count
        );
        assert_eq!(
            child.first_index + child.index_count,
            frame.indices().len() as u64
        );
    }

    #[test]
    fn a_layer_outside_the_frame_is_reported_as_skipped() {
        let (root, card, tree) = scene();
        let mut renderer = LayerAwareNodeRenderer::new();
        renderer.render(&tree, &mut DrawList::default());
        // Shrinking the root puts the card's window fully outside it, and a
        // clipping layer root is culled before it can composite.
        root.set_frame(Rect::new(0.0, 0.0, 1.0, 1.0));
        let mut frame = DrawList::default();
        let mut census = LayerCensus::default();
        renderer.render_with_census(&tree, &mut frame, Some(&mut census));
        assert!(frame.indices().is_empty());
        assert_eq!(census.layers.len(), 0);
        assert_eq!(census.skipped.len(), 1, "the frame culled the layer");
        assert_eq!(
            census.skipped[0].node.as_deref(),
            Some(card.id().0.to_string().as_str())
        );
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

    #[test]
    fn a_layer_served_from_cache_does_not_bump_the_repaint_epoch() {
        let (_root, card, tree) = scene();
        let mut renderer = LayerAwareNodeRenderer::new();
        renderer.render(&tree, &mut DrawList::default());
        assert_eq!(card.repaint_epoch(), 1, "the first frame paints the card");

        renderer.render(&tree, &mut DrawList::default());
        assert_eq!(
            card.repaint_epoch(),
            1,
            "the reused cached layer appends vertices without repainting"
        );
    }

    #[test]
    fn a_layer_that_has_to_rerecord_bumps_the_repaint_epoch() {
        let (_root, card, tree) = scene();
        let mut renderer = LayerAwareNodeRenderer::new();
        renderer.render(&tree, &mut DrawList::default());
        let object = tree.render_object(&card).unwrap();
        object.set_cache_invalid(true);
        renderer.render(&tree, &mut DrawList::default());
        assert_eq!(
            card.repaint_epoch(),
            2,
            "a re-recorded layer really paints again"
        );
    }
}
