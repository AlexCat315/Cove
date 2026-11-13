//! Temporary diagnostic styles layered over the authored scene.
//!
//! Colors and padding have independent debug slots, so recomposition cannot
//! erase them or freeze the restoration value. Flex factors and alignment have
//! no debug slot: they are written to the node's `LayoutNode`, so the authored
//! values are captured first and written back when the override ends. Mutable
//! state lives in `RefCell`s because a snapshot must be readable through
//! `&self` while the server still holds exclusive access for command handling.
use crate::inspection::inspector::SceneInspector;
use cove_core::scene::{
    Align, InvalidationSource, Justify, LayoutInsets, Node, NodeTree, image_pixel_metrics,
};
use cove_core::{Color, DrawList, Rect, TextLine};
use cove_devtools_protocol::{Envelope, InspectionState};
use serde::Serialize;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;

/// The temporary style a node carries, one optional value per property. A
/// missing value either clears that property or leaves it alone, depending on
/// whether the property was overridden before.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Style {
    padding: Option<LayoutInsets>,
    background: Option<Color>,
    foreground: Option<Color>,
    flex_grow: Option<f32>,
    flex_shrink: Option<f32>,
    justify_content: Option<Justify>,
    align_items: Option<Align>,
}

impl Style {
    fn is_empty(self) -> bool {
        self.padding.is_none()
            && self.background.is_none()
            && self.foreground.is_none()
            && self.flex_grow.is_none()
            && self.flex_shrink.is_none()
            && self.justify_content.is_none()
            && self.align_items.is_none()
    }
}

/// The authored flex values captured the first time each field is overridden.
///
/// Flex style lives on the node's `LayoutNode` and has no debug overlay
/// equivalent, so restoring it needs a snapshot taken before the first edit.
/// Each field is captured independently, which keeps a later partial update
/// from losing an earlier field's original.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct FlexOriginal {
    grow: Option<f32>,
    shrink: Option<f32>,
    justify: Option<Justify>,
    align: Option<Align>,
}

impl FlexOriginal {
    fn is_empty(self) -> bool {
        self.grow.is_none()
            && self.shrink.is_none()
            && self.justify.is_none()
            && self.align.is_none()
    }
}

/// Apply `desired` to one flex field while holding its original.
///
/// The first override captures the live value. Clearing the override writes the
/// original back. Returns whether the layout value changed, so the caller can
/// mark real layout invalidation only when something moved.
fn override_flex_value<T: Copy + PartialEq>(
    original: &mut Option<T>,
    current: T,
    desired: Option<T>,
    mut apply: impl FnMut(T),
) -> bool {
    match desired {
        Some(value) => {
            if original.is_none() {
                *original = Some(current);
            }
            if current == value {
                return false;
            }
            apply(value);
            true
        }
        None => match original.take() {
            Some(restored) if current != restored => {
                apply(restored);
                true
            }
            _ => false,
        },
    }
}

/// Debug paint guidelines the runtime can actually draw.
///
/// Guidelines outline live node frames. Text baselines are their own layer
/// (`SceneEditor::baselines`) because they come from measured text layout, not
/// from frames; oversized images are a third layer backed by the image
/// primitive's real source and display pixel sizes; repaint highlighting is a
/// fourth, colored by each node's real paint generation. Slow animations are a
/// separate runtime setting, not part of this state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugPaint {
    pub guidelines: bool,
}

struct NodeRef {
    node: Option<Node>,
}

const MAX_OVERRIDE_NODES: usize = 256;
const MAX_UNDO_DEPTH: usize = 128;
/// Guidelines stop after this many visited nodes, so an overlay frame stays
/// bounded even for a gallery-sized tree.
const MAX_GUIDELINE_VISITS: usize = 4096;
/// Outlines emitted per frame. Each outline costs four one-pixel quads.
pub const MAX_GUIDELINE_NODES: usize = 512;
/// Baseline lines stop after this many nodes, so an overlay frame with deeply
/// nested text stays bounded.
const MAX_BASELINE_VISITS: usize = 4096;
/// Baseline lines emitted per frame, across every node. Each line is one quad.
pub const MAX_BASELINES: usize = 512;
/// Baseline color: green, distinct from the amber guidelines and blue border.
const BASELINE_COLOR: Color = Color {
    r: 0.0,
    g: 200.0 / 255.0,
    b: 120.0 / 255.0,
    a: 220.0 / 255.0,
};
/// Oversized-image walks stop after this many visited nodes, so an overlay
/// frame stays bounded even for a gallery-sized tree.
const MAX_OVERSIZED_IMAGE_VISITS: usize = 4096;
/// Oversized-image outlines emitted per frame. Each outline is four 2px quads.
pub const MAX_OVERSIZED_IMAGES: usize = 512;
/// A decoded source must cover at least this multiple of its painted display
/// area to be highlighted. Exactly four times is not enough; above it is.
pub const OVERSIZED_IMAGE_AREA_RATIO: f32 = 4.0;
/// Oversized-image color: magenta, distinct from guidelines, baselines and the
/// blue selection border.
const OVERSIZED_IMAGE_COLOR: Color = Color {
    r: 1.0,
    g: 45.0 / 255.0,
    b: 120.0 / 255.0,
    a: 230.0 / 255.0,
};
/// Repaint-highlight walks stop after this many visited nodes, so an overlay
/// frame stays bounded even for a gallery-sized tree.
const MAX_REPAINT_VISITS: usize = 4096;
/// Repaint outlines emitted per frame. Each outline is four 2px quads.
pub const MAX_REPAINT_NODES: usize = 512;
/// Colors a painted node cycles through, indexed by its paint generation.
///
/// A node's generation advances only when a renderer really paints its own box
/// or content, so a layer served from a cache keeps its color while a node that
/// repaints moves to the next entry. These are debug-only colors, deliberately
/// distinct from the amber guidelines, green baselines, magenta oversized-image
/// outlines and the blue selection border.
pub const REPAINT_PALETTE: [Color; 6] = [
    Color {
        r: 230.0 / 255.0,
        g: 70.0 / 255.0,
        b: 70.0 / 255.0,
        a: 220.0 / 255.0,
    },
    Color {
        r: 240.0 / 255.0,
        g: 140.0 / 255.0,
        b: 50.0 / 255.0,
        a: 220.0 / 255.0,
    },
    Color {
        r: 235.0 / 255.0,
        g: 205.0 / 255.0,
        b: 60.0 / 255.0,
        a: 220.0 / 255.0,
    },
    Color {
        r: 80.0 / 255.0,
        g: 200.0 / 255.0,
        b: 120.0 / 255.0,
        a: 220.0 / 255.0,
    },
    Color {
        r: 60.0 / 255.0,
        g: 190.0 / 255.0,
        b: 220.0 / 255.0,
        a: 220.0 / 255.0,
    },
    Color {
        r: 160.0 / 255.0,
        g: 120.0 / 255.0,
        b: 240.0 / 255.0,
        a: 220.0 / 255.0,
    },
];

/// The repaint color for a node's paint generation.
pub fn repaint_color(epoch: u64) -> Color {
    REPAINT_PALETTE[(epoch % REPAINT_PALETTE.len() as u64) as usize]
}

/// Scene-thread-only inspection editor.
pub struct SceneEditor {
    tree: Rc<RefCell<NodeTree>>,
    indexed_root: RefCell<Option<Node>>,
    indexed_version: Cell<Option<u64>>,
    node_index: RefCell<BTreeMap<String, Rc<RefCell<NodeRef>>>>,
    overrides: RefCell<BTreeMap<String, Style>>,
    flex_originals: RefCell<BTreeMap<String, FlexOriginal>>,
    override_nodes: RefCell<BTreeMap<String, Rc<RefCell<NodeRef>>>>,
    undo: RefCell<Vec<BTreeMap<String, Style>>>,
    redo: RefCell<Vec<BTreeMap<String, Style>>>,
    selected: RefCell<Option<Node>>,
    debug_paint: Cell<DebugPaint>,
    baselines: Cell<bool>,
    oversized_images: Cell<bool>,
    repaint_highlight: Cell<bool>,
    /// Invoked after any change that the host may want to redraw for.
    pub on_change: RefCell<Option<Rc<dyn Fn()>>>,
}

impl SceneEditor {
    pub fn new(tree: Rc<RefCell<NodeTree>>) -> Self {
        Self {
            tree,
            indexed_root: RefCell::new(None),
            indexed_version: Cell::new(None),
            node_index: RefCell::new(BTreeMap::new()),
            overrides: RefCell::new(BTreeMap::new()),
            flex_originals: RefCell::new(BTreeMap::new()),
            override_nodes: RefCell::new(BTreeMap::new()),
            undo: RefCell::new(Vec::new()),
            redo: RefCell::new(Vec::new()),
            selected: RefCell::new(None),
            debug_paint: Cell::new(DebugPaint::default()),
            baselines: Cell::new(false),
            oversized_images: Cell::new(false),
            repaint_highlight: Cell::new(false),
            on_change: RefCell::new(None),
        }
    }

    fn notify(&self) {
        let callback = self.on_change.borrow().clone();
        if let Some(callback) = callback {
            callback();
        }
    }

    /// Look a node up by either identity form, rebuilding the index when the
    /// tree structure changed. A stale index would resolve ids to dead nodes,
    /// which is worse than paying for one walk.
    pub fn find(&self, id: &str) -> Option<Node> {
        let root = self.tree.borrow().root();
        let Some(root) = root else {
            self.node_index.borrow_mut().clear();
            *self.indexed_root.borrow_mut() = None;
            self.indexed_version.set(None);
            return None;
        };
        let same_root = self
            .indexed_root
            .borrow()
            .as_ref()
            .is_some_and(|indexed| indexed == &root);
        let same_version = self.indexed_version.get() == Some(root.subtree_structure_version());
        if !same_root || !same_version {
            let mut index = self.node_index.borrow_mut();
            index.clear();
            let mut stack = vec![root.clone()];
            while let Some(node) = stack.pop() {
                let reference = Rc::new(RefCell::new(NodeRef {
                    node: Some(node.clone()),
                }));
                index.insert(SceneInspector::identifier(&node), reference);
                stack.extend(node.children());
            }
            self.indexed_version
                .set(Some(root.subtree_structure_version()));
            *self.indexed_root.borrow_mut() = Some(root);
        }
        self.node_index
            .borrow()
            .get(id)
            .and_then(|reference| reference.borrow().node.clone())
            .filter(|node| {
                node.parent().is_some() || self.tree.borrow().root().as_ref() == Some(node)
            })
    }

    /// Highlight a live component in the next rendered frame.
    pub fn select(&self, id: Option<&str>) -> bool {
        let node = id.and_then(|id| self.find(id));
        if id.is_some() && node.is_none() {
            return false;
        }
        *self.selected.borrow_mut() = node;
        self.notify();
        true
    }

    pub fn selected_node(&self) -> Option<Node> {
        self.selected.borrow().clone()
    }

    /// The debug paint state the next overlay frame will draw.
    pub fn debug_paint(&self) -> DebugPaint {
        self.debug_paint.get()
    }

    /// Apply debug paint and return the state that took effect.
    ///
    /// The returned value is what `draw_overlay` will use.
    pub fn set_debug_paint(&self, paint: DebugPaint) -> DebugPaint {
        if self.debug_paint.get() != paint {
            self.debug_paint.set(paint);
            self.notify();
        }
        paint
    }

    /// Whether the next overlay draws text baselines.
    pub fn baselines(&self) -> bool {
        self.baselines.get()
    }

    /// Enable or disable text baselines and return the state that took effect.
    pub fn set_baselines(&self, enabled: bool) -> bool {
        if self.baselines.get() != enabled {
            self.baselines.set(enabled);
            self.notify();
        }
        enabled
    }

    /// Whether the next overlay highlights oversized images.
    pub fn oversized_images(&self) -> bool {
        self.oversized_images.get()
    }

    /// Enable or disable oversized-image highlighting and return the state
    /// that took effect.
    pub fn set_oversized_images(&self, enabled: bool) -> bool {
        if self.oversized_images.get() != enabled {
            self.oversized_images.set(enabled);
            self.notify();
        }
        enabled
    }

    /// Whether the next overlay highlights repainted nodes.
    pub fn repaint_highlight(&self) -> bool {
        self.repaint_highlight.get()
    }

    /// Enable or disable repaint highlighting and return the state that took
    /// effect.
    pub fn set_repaint_highlight(&self, enabled: bool) -> bool {
        if self.repaint_highlight.get() != enabled {
            self.repaint_highlight.set(enabled);
            self.notify();
        }
        enabled
    }

    pub fn state(&self) -> InspectionState {
        self.prune();
        InspectionState {
            selected_id: self
                .selected
                .borrow()
                .as_ref()
                .map(|node| node.id().0.to_string()),
            can_undo: !self.undo.borrow().is_empty(),
            can_redo: !self.redo.borrow().is_empty(),
            override_count: self.overrides.borrow().len(),
        }
    }

    /// Drop overrides and selections that point at unmounted nodes.
    pub fn prune(&self) {
        let dead: Vec<String> = self
            .overrides
            .borrow()
            .keys()
            .filter(|id| self.find(id).is_none())
            .cloned()
            .collect();
        if !dead.is_empty() {
            let live: BTreeMap<String, Style> = self
                .overrides
                .borrow()
                .iter()
                .filter(|(id, _)| !dead.contains(id))
                .map(|(id, style)| (id.clone(), *style))
                .collect();
            self.apply(live);
        }
        if self
            .selected
            .borrow()
            .as_ref()
            .is_some_and(|node| self.find(&node.id().0.to_string()).is_none())
        {
            *self.selected.borrow_mut() = None;
        }
    }

    /// Disconnect/stop discards temporary styles and history.
    pub fn reset(&self) {
        self.apply(BTreeMap::new());
        // `apply` restored every live and retained node; anything left points at
        // a node this editor can no longer reach.
        self.override_nodes.borrow_mut().clear();
        self.flex_originals.borrow_mut().clear();
        self.undo.borrow_mut().clear();
        self.redo.borrow_mut().clear();
        *self.selected.borrow_mut() = None;
        self.debug_paint.set(DebugPaint::default());
        self.baselines.set(false);
        self.oversized_images.set(false);
        self.repaint_highlight.set(false);
        self.notify();
    }

    fn apply(&self, next: BTreeMap<String, Style>) {
        let live: BTreeMap<String, Style> = next
            .into_iter()
            .filter(|(id, _)| self.find(id).is_some())
            .collect();
        let mut ids: Vec<String> = self.overrides.borrow().keys().cloned().collect();
        for id in live.keys() {
            if !ids.contains(id) {
                ids.push(id.clone());
            }
        }
        for id in ids {
            let style = live.get(&id).copied();
            if self.overrides.borrow().get(&id).copied() == style {
                continue;
            }
            let node = self.find(&id).or_else(|| {
                self.override_nodes
                    .borrow()
                    .get(&id)
                    .and_then(|reference| reference.borrow().node.clone())
            });
            let Some(node) = node else {
                self.override_nodes.borrow_mut().remove(&id);
                self.flex_originals.borrow_mut().remove(&id);
                continue;
            };
            node.set_debug_background_color(style.and_then(|style| style.background));
            node.set_debug_foreground_color(style.and_then(|style| style.foreground));
            if let Some(layout) = node.layout_node() {
                layout.set_debug_padding(style.and_then(|style| style.padding));
            }
            // The layout node invalidates its own cached style and layout, and
            // the node flag records the layout phase for hosts and the
            // invalidation log. Render invalidation then covers a repaint.
            if self.apply_flex(&id, &node, style.unwrap_or_default()) {
                node.mark_dirty(InvalidationSource::style_set("flex"));
            } else {
                node.mark_render_dirty(InvalidationSource::style_set("debugStyle"));
            }
            if style.is_some() {
                self.override_nodes.borrow_mut().insert(
                    id.clone(),
                    Rc::new(RefCell::new(NodeRef { node: Some(node) })),
                );
            } else {
                self.override_nodes.borrow_mut().remove(&id);
            }
        }
        *self.overrides.borrow_mut() = live;
    }

    /// Apply or restore this node's temporary flex values.
    ///
    /// Only fields with a desired value are written; a field that is absent but
    /// has a captured original is restored. That is what makes a later partial
    /// update keep every earlier field's restoration intact.
    fn apply_flex(&self, id: &str, node: &Node, style: Style) -> bool {
        let Some(layout) = node.layout_node() else {
            self.flex_originals.borrow_mut().remove(id);
            return false;
        };
        let mut original = self
            .flex_originals
            .borrow()
            .get(id)
            .copied()
            .unwrap_or_default();
        let mut changed = false;
        changed |= override_flex_value(
            &mut original.grow,
            layout.flex_grow(),
            style.flex_grow,
            |value| layout.set_flex_grow(value),
        );
        changed |= override_flex_value(
            &mut original.shrink,
            layout.flex_shrink(),
            style.flex_shrink,
            |value| layout.set_flex_shrink(value),
        );
        changed |= override_flex_value(
            &mut original.justify,
            layout.justify_content(),
            style.justify_content,
            |value| layout.set_justify_content(value),
        );
        changed |= override_flex_value(
            &mut original.align,
            layout.align_items(),
            style.align_items,
            |value| layout.set_align_items(value),
        );
        if original.is_empty() {
            self.flex_originals.borrow_mut().remove(id);
        } else {
            self.flex_originals
                .borrow_mut()
                .insert(id.to_owned(), original);
        }
        changed
    }

    /// The properties this node currently overrides, in a stable order.
    ///
    /// A snapshot publishes these names so a client can tell an override from
    /// the authored value without diffing geometry.
    pub fn override_properties(&self, id: &str) -> Vec<&'static str> {
        let overrides = self.overrides.borrow();
        let Some(style) = overrides.get(id) else {
            return Vec::new();
        };
        let mut names = Vec::with_capacity(7);
        if style.padding.is_some() {
            names.push("padding");
        }
        if style.background.is_some() {
            names.push("backgroundColor");
        }
        if style.foreground.is_some() {
            names.push("foregroundColor");
        }
        if style.flex_grow.is_some() {
            names.push("flexGrow");
        }
        if style.flex_shrink.is_some() {
            names.push("flexShrink");
        }
        if style.justify_content.is_some() {
            names.push("justifyContent");
        }
        if style.align_items.is_some() {
            names.push("alignItems");
        }
        names
    }

    fn commit(&self, next: BTreeMap<String, Style>) {
        if *self.overrides.borrow() == next {
            return;
        }
        {
            let mut undo = self.undo.borrow_mut();
            undo.push(self.overrides.borrow().clone());
            if undo.len() > MAX_UNDO_DEPTH {
                undo.remove(0);
            }
        }
        self.redo.borrow_mut().clear();
        self.apply(next);
    }

    /// Validate and apply one inspection command.
    pub fn handle(&self, request: &Envelope) -> Envelope {
        if let Some(message) = cove_devtools_protocol::InspectionValidation::error(request) {
            return request.error("bad_request", message);
        }
        self.prune();
        let payload = request.payload.as_ref().and_then(|value| value.as_object());
        match request.r#type.as_str() {
            "inspect.style.set" | "inspect.style.clear" => {
                let Some(id) = payload
                    .and_then(|payload| payload.get("id"))
                    .and_then(|value| value.as_str())
                else {
                    return error(request, "bad_request", "Expected node id");
                };
                let Some(node) = self.find(id) else {
                    return error(
                        request,
                        "stale_node",
                        "Selected node is no longer in the scene",
                    );
                };
                let node_id = node.id().0.to_string();
                let mut next = self.overrides.borrow().clone();
                if request.r#type == "inspect.style.clear" {
                    next.remove(&node_id);
                } else {
                    let Some(properties) = payload
                        .and_then(|payload| payload.get("properties"))
                        .and_then(|value| value.as_object())
                    else {
                        return error(
                            request,
                            "bad_request",
                            "Expected node id and supported properties",
                        );
                    };
                    let layoutless = node.layout_node().is_none();
                    if layoutless
                        && properties.iter().any(|(key, value)| {
                            !value.is_null() && cove_devtools_protocol::is_layout_property(key)
                        })
                    {
                        return error(
                            request,
                            "unsupported",
                            "This composition anchor has no layout box; select a child",
                        );
                    }
                    let mut style = next.get(&node_id).copied().unwrap_or_default();
                    for (key, value) in properties {
                        match key.as_str() {
                            "padding" => {
                                style.padding = value.as_object().map(|edges| LayoutInsets {
                                    top: edges.get("top").and_then(|v| v.as_f64()).unwrap_or(0.0)
                                        as f32,
                                    right: edges
                                        .get("right")
                                        .and_then(|v| v.as_f64())
                                        .unwrap_or(0.0)
                                        as f32,
                                    bottom: edges
                                        .get("bottom")
                                        .and_then(|v| v.as_f64())
                                        .unwrap_or(0.0)
                                        as f32,
                                    left: edges.get("left").and_then(|v| v.as_f64()).unwrap_or(0.0)
                                        as f32,
                                });
                            }
                            "backgroundColor" => {
                                style.background = value.as_str().and_then(Self::color);
                            }
                            "foregroundColor" => {
                                style.foreground = value.as_str().and_then(Self::color);
                            }
                            "flexGrow" => style.flex_grow = Self::flex_factor(value),
                            "flexShrink" => style.flex_shrink = Self::flex_factor(value),
                            "justifyContent" => {
                                style.justify_content = value.as_str().and_then(Self::justify);
                            }
                            "alignItems" => {
                                style.align_items = value.as_str().and_then(Self::align);
                            }
                            _ => {}
                        }
                    }
                    if style.is_empty() {
                        next.remove(&node_id);
                    } else {
                        next.insert(node_id, style);
                    }
                }
                if next.len() > MAX_OVERRIDE_NODES {
                    return error(
                        request,
                        "limit",
                        "At most 256 nodes may have temporary styles",
                    );
                }
                self.commit(next);
            }
            "inspect.style.clearAll" => self.commit(BTreeMap::new()),
            "inspect.recomposition.reset" => {
                let root = self.tree.borrow().root();
                if let Some(root) = root {
                    let mut stack = vec![root];
                    while let Some(node) = stack.pop() {
                        if let Some(mut metrics) = node.recomposition_metrics() {
                            metrics.reset();
                            node.set_recomposition_metrics(Some(metrics));
                        }
                        stack.extend(node.children());
                    }
                }
            }
            "inspect.style.undo" => {
                let previous = self.undo.borrow_mut().pop();
                if let Some(previous) = previous {
                    self.redo.borrow_mut().push(self.overrides.borrow().clone());
                    self.apply(previous);
                }
            }
            "inspect.style.redo" => {
                let next = self.redo.borrow_mut().pop();
                if let Some(next) = next {
                    self.undo.borrow_mut().push(self.overrides.borrow().clone());
                    self.apply(next);
                }
            }
            _ => {}
        }
        self.notify();
        let state = self.state();
        // The response echoes the request type with `.ok`, carrying the new
        // inspection state so a client never has to poll.
        let mut response = request
            .ok()
            .unwrap_or_else(|| Envelope::new(format!("{}.ok", request.r#type), request.id, None));
        response.payload = serde_json::to_value(state).ok();
        response
    }

    pub fn draw_overlay(&self, list: &mut DrawList) {
        self.prune();
        if self.debug_paint.get().guidelines {
            self.draw_guidelines(list);
        }
        if self.baselines.get() {
            self.draw_baselines(list);
        }
        if self.oversized_images.get() {
            self.draw_oversized_images(list);
        }
        if self.repaint_highlight.get() {
            self.draw_repaint_highlight(list);
        }
        let Some(node) = self.selected_node() else {
            return;
        };
        let frame = node.absolute_frame();
        if frame.width <= 0.0 || frame.height <= 0.0 {
            return;
        }
        let clips = clip_chain(&node);
        for clip in &clips {
            list.push_clip_rect(*clip);
        }
        let color = Color::from_rgba8(80, 160, 255, 255);
        outline(list, frame, 2.0, color);
        for _ in &clips {
            list.pop_clip_rect();
        }
    }

    /// One-pixel outlines around every visible live node frame.
    ///
    /// Frames come from the tree's own `absolute_frame`, so a guideline follows
    /// scrolling and transforms instead of being a snapshot. The walk is
    /// breadth-first and bounded; each outline is clipped to the root viewport
    /// and to the ancestors that clip their own bounds, the same clip chain the
    /// selected node's border uses. Selection is drawn afterwards, on top.
    fn draw_guidelines(&self, list: &mut DrawList) {
        let Some(root) = self.tree.borrow().root() else {
            return;
        };
        let viewport = root.absolute_frame();
        if !valid_frame(viewport) {
            return;
        }
        let color = Color::from_rgba8(255, 170, 0, 200);
        list.push_clip_rect(viewport);
        let mut queue = VecDeque::from([root]);
        let mut visited = 0usize;
        let mut drawn = 0usize;
        while let Some(node) = queue.pop_front() {
            if visited >= MAX_GUIDELINE_VISITS || drawn >= MAX_GUIDELINE_NODES {
                break;
            }
            visited += 1;
            // The renderer drops an invisible node with its subtree; guidelines
            // follow the same rule so they never outline something not painted.
            if !node.is_layout_visible()
                || node.is_detached()
                || node.opacity() <= 0.0
                || node.subtree_opacity() <= 0.0
            {
                continue;
            }
            let frame = node.absolute_frame();
            if valid_frame(frame) {
                let clips = clip_chain(&node);
                for clip in &clips {
                    list.push_clip_rect(*clip);
                }
                outline(list, frame, 1.0, color);
                for _ in &clips {
                    list.pop_clip_rect();
                }
                drawn += 1;
            }
            if queue.len() < MAX_GUIDELINE_VISITS {
                let room = MAX_GUIDELINE_VISITS - queue.len();
                queue.extend(node.children().into_iter().take(room));
            }
        }
        list.pop_clip_rect();
    }

    /// One-pixel baselines at the real baseline of every measured text line.
    ///
    /// Only nodes whose `LayoutNode` cached a `TextLayoutResult` contribute, so
    /// a non-text node never gets an invented baseline. The walk mirrors
    /// guidelines: breadth-first, bounded, skipping detached, invisible and
    /// transparent nodes, clipped to the root viewport and to each node's own
    /// clipping ancestors.
    fn draw_baselines(&self, list: &mut DrawList) {
        let Some(root) = self.tree.borrow().root() else {
            return;
        };
        let viewport = root.absolute_frame();
        if !valid_frame(viewport) {
            return;
        }
        list.push_clip_rect(viewport);
        let mut queue = VecDeque::from([root]);
        let mut visited = 0usize;
        let mut drawn = 0usize;
        while let Some(node) = queue.pop_front() {
            if visited >= MAX_BASELINE_VISITS || drawn >= MAX_BASELINES {
                break;
            }
            visited += 1;
            if !node.is_layout_visible()
                || node.is_detached()
                || node.opacity() <= 0.0
                || node.subtree_opacity() <= 0.0
            {
                continue;
            }
            let frame = node.absolute_frame();
            if valid_frame(frame)
                && let Some(layout) = node.layout_node()
                && let Some(measured) = layout.text_measure()
            {
                let padding = layout.resolved_padding();
                let clips = clip_chain(&node);
                for clip in &clips {
                    list.push_clip_rect(*clip);
                }
                for line in measured.result.lines.iter().take(MAX_BASELINES - drawn) {
                    if draw_baseline(list, frame, padding.top, padding.left, line) {
                        drawn += 1;
                    }
                }
                for _ in &clips {
                    list.pop_clip_rect();
                }
            }
            if queue.len() < MAX_BASELINE_VISITS {
                let room = MAX_BASELINE_VISITS - queue.len();
                queue.extend(node.children().into_iter().take(room));
            }
        }
        list.pop_clip_rect();
    }

    /// Two-pixel outlines around every visible image that paints far fewer
    /// pixels than its decoded source.
    ///
    /// Metrics come from the image primitive's last paint: the decoded source
    /// extent and the destination rect scaled by that draw list's pixel scale.
    /// A node without metrics is not an image or has no known source size, and
    /// vector sources rasterize at the draw size, so neither can be oversized.
    /// The walk mirrors guidelines: breadth-first, bounded, skipping detached,
    /// invisible and transparent nodes, clipped to the root viewport and to
    /// each node's own clipping ancestors.
    fn draw_oversized_images(&self, list: &mut DrawList) {
        let Some(root) = self.tree.borrow().root() else {
            return;
        };
        let viewport = root.absolute_frame();
        if !valid_frame(viewport) {
            return;
        }
        list.push_clip_rect(viewport);
        let mut queue = VecDeque::from([root]);
        let mut visited = 0usize;
        let mut drawn = 0usize;
        while let Some(node) = queue.pop_front() {
            if visited >= MAX_OVERSIZED_IMAGE_VISITS || drawn >= MAX_OVERSIZED_IMAGES {
                break;
            }
            visited += 1;
            if !node.is_layout_visible()
                || node.is_detached()
                || node.opacity() <= 0.0
                || node.subtree_opacity() <= 0.0
            {
                continue;
            }
            let frame = node.absolute_frame();
            if valid_frame(frame) && is_oversized_image(&node) {
                let clips = clip_chain(&node);
                for clip in &clips {
                    list.push_clip_rect(*clip);
                }
                outline(list, frame, 2.0, OVERSIZED_IMAGE_COLOR);
                for _ in &clips {
                    list.pop_clip_rect();
                }
                drawn += 1;
            }
            if queue.len() < MAX_OVERSIZED_IMAGE_VISITS {
                let room = MAX_OVERSIZED_IMAGE_VISITS - queue.len();
                queue.extend(node.children().into_iter().take(room));
            }
        }
        list.pop_clip_rect();
    }

    /// Two-pixel outlines around every visible node that has really painted.
    ///
    /// The color comes from the node's `repaint_epoch`, which the renderers
    /// bump only when they emit that node's own box or content. A node retained
    /// in a layer cache is appended from its cached draw list, never painted, so
    /// its generation and color stay put while a repainting node cycles. Nodes
    /// that have never painted have no repaint color and are skipped rather
    /// than given a made-up one. The walk mirrors guidelines: breadth-first,
    /// bounded, skipping detached, invisible and transparent nodes, clipped to
    /// the root viewport and to each node's own clipping ancestors.
    fn draw_repaint_highlight(&self, list: &mut DrawList) {
        let Some(root) = self.tree.borrow().root() else {
            return;
        };
        let viewport = root.absolute_frame();
        if !valid_frame(viewport) {
            return;
        }
        list.push_clip_rect(viewport);
        let mut queue = VecDeque::from([root]);
        let mut visited = 0usize;
        let mut drawn = 0usize;
        while let Some(node) = queue.pop_front() {
            if visited >= MAX_REPAINT_VISITS || drawn >= MAX_REPAINT_NODES {
                break;
            }
            visited += 1;
            if !node.is_layout_visible()
                || node.is_detached()
                || node.opacity() <= 0.0
                || node.subtree_opacity() <= 0.0
            {
                continue;
            }
            let frame = node.absolute_frame();
            let epoch = node.repaint_epoch();
            if valid_frame(frame) && epoch > 0 {
                let clips = clip_chain(&node);
                for clip in &clips {
                    list.push_clip_rect(*clip);
                }
                outline(list, frame, 2.0, repaint_color(epoch));
                for _ in &clips {
                    list.pop_clip_rect();
                }
                drawn += 1;
            }
            if queue.len() < MAX_REPAINT_VISITS {
                let room = MAX_REPAINT_VISITS - queue.len();
                queue.extend(node.children().into_iter().take(room));
            }
        }
        list.pop_clip_rect();
    }

    /// A temporary flex factor, or `None` for a null property.
    ///
    /// Validation already bounds the number; this check keeps a direct
    /// `handle` call from writing a value the layout engine cannot take.
    fn flex_factor(value: &serde_json::Value) -> Option<f32> {
        value.as_f64().map(|number| number as f32).filter(|number| {
            number.is_finite()
                && (0.0..=cove_devtools_protocol::MAX_FLEX_FACTOR as f32).contains(number)
        })
    }

    /// `Justify` by its inspector name.
    fn justify(name: &str) -> Option<Justify> {
        Some(match name {
            "FlexStart" => Justify::FlexStart,
            "Center" => Justify::Center,
            "FlexEnd" => Justify::FlexEnd,
            "SpaceBetween" => Justify::SpaceBetween,
            "SpaceAround" => Justify::SpaceAround,
            "SpaceEvenly" => Justify::SpaceEvenly,
            _ => return None,
        })
    }

    /// `Align` by its inspector name.
    fn align(name: &str) -> Option<Align> {
        Some(match name {
            "Auto" => Align::Auto,
            "FlexStart" => Align::FlexStart,
            "Center" => Align::Center,
            "FlexEnd" => Align::FlexEnd,
            "Stretch" => Align::Stretch,
            "Baseline" => Align::Baseline,
            "SpaceBetween" => Align::SpaceBetween,
            "SpaceAround" => Align::SpaceAround,
            "SpaceEvenly" => Align::SpaceEvenly,
            _ => return None,
        })
    }

    /// `#rrggbbaa` for a color, or `None`.
    pub fn hex(color: Option<Color>) -> Option<String> {
        color.map(|color| {
            let value = color.rgba8();
            format!(
                "#{:02x}{:02x}{:02x}{:02x}",
                value & 255,
                (value >> 8) & 255,
                (value >> 16) & 255,
                (value >> 24) & 255
            )
        })
    }

    pub fn color(hex: &str) -> Option<Color> {
        let digits = hex.strip_prefix('#')?;
        let value = u32::from_str_radix(digits, 16).ok()?;
        let rgba = if digits.len() == 6 {
            (value << 8) | 255
        } else {
            value
        };
        Some(Color::from_rgba8(
            ((rgba >> 24) & 255) as u8,
            ((rgba >> 16) & 255) as u8,
            ((rgba >> 8) & 255) as u8,
            (rgba & 255) as u8,
        ))
    }
}

fn error(request: &Envelope, code: &str, message: &str) -> Envelope {
    let mut response = request.error(code, message);
    response.payload = Some(serde_json::json!({ "code": code, "message": message }));
    response
}

fn valid_frame(frame: Rect) -> bool {
    frame.width > 0.0
        && frame.height > 0.0
        && frame.x.is_finite()
        && frame.y.is_finite()
        && frame.width.is_finite()
        && frame.height.is_finite()
}

/// Whether the node's real decoded source covers more than
/// [`OVERSIZED_IMAGE_AREA_RATIO`] times its painted display area.
fn is_oversized_image(node: &Node) -> bool {
    let Some(metrics) = image_pixel_metrics(node) else {
        return false;
    };
    if metrics.vector {
        return false;
    }
    metrics
        .area_ratio()
        .is_some_and(|ratio| ratio > OVERSIZED_IMAGE_AREA_RATIO)
}

/// The frames that clip this node, nearest ancestor first.
fn clip_chain(node: &Node) -> Vec<Rect> {
    let mut clips = Vec::new();
    let mut parent = node.parent();
    while let Some(current) = parent {
        if current.clips_to_bounds() {
            clips.push(current.absolute_frame());
        }
        parent = current.parent();
    }
    clips
}

/// One-pixel line at `line`'s baseline inside `frame`.
///
/// `line.baseline_y` is relative to the node's text origin, so the frame origin
/// places it in window space. The line starts at the frame's left edge and its
/// width is the measured line width, clamped to the frame. The baseline must
/// fall inside the frame vertically; anything else is stale or malformed layout
/// data and is skipped rather than painted outside the node.
fn draw_baseline(
    list: &mut DrawList,
    frame: Rect,
    padding_top: f32,
    padding_left: f32,
    line: &TextLine,
) -> bool {
    if !line.baseline_y.is_finite() || !line.width.is_finite() {
        return false;
    }
    let baseline = padding_top + line.baseline_y;
    if baseline < 0.0 || baseline > frame.height {
        return false;
    }
    let width = line.width.min((frame.width - padding_left).max(0.0));
    if width <= 0.0 || !padding_left.is_finite() {
        return false;
    }
    list.add_rect(
        Rect::new(frame.x + padding_left, frame.y + baseline, width, 1.0),
        BASELINE_COLOR,
    )
    .is_ok()
}

/// Four quads forming a `thickness`-wide border inside `frame`.
fn outline(list: &mut DrawList, frame: Rect, thickness: f32, color: Color) {
    let horizontal = thickness.min(frame.height);
    let vertical = thickness.min(frame.width);
    let _ = list.add_rect(Rect::new(frame.x, frame.y, frame.width, horizontal), color);
    let _ = list.add_rect(
        Rect::new(
            frame.x,
            frame.y + (frame.height - horizontal).max(0.0),
            frame.width,
            horizontal,
        ),
        color,
    );
    let _ = list.add_rect(Rect::new(frame.x, frame.y, vertical, frame.height), color);
    let _ = list.add_rect(
        Rect::new(
            frame.x + (frame.width - vertical).max(0.0),
            frame.y,
            vertical,
            frame.height,
        ),
        color,
    );
}

impl std::fmt::Debug for SceneEditor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SceneEditor")
            .field("overrides", &self.overrides.borrow().len())
            .finish_non_exhaustive()
    }
}
