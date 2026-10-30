//! Temporary diagnostic styles layered over the authored scene.
//!
//! Diagnostic styles are independent of authored styles, so recomposition
//! cannot erase them or freeze the restoration value. Mutable state lives in
//! `RefCell`s because a snapshot must be readable through `&self` while the
//! server still holds exclusive access for command handling.
use crate::inspection::inspector::SceneInspector;
use cove_core::scene::{Display, InvalidationSource, LayoutInsets, Node, NodeTree};
use cove_core::{Color, DrawList, Point, Rect};
use cove_devtools_protocol::{Envelope, InspectionState};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Style {
    padding: Option<LayoutInsets>,
    background: Option<Color>,
    foreground: Option<Color>,
}

impl Style {
    fn is_empty(self) -> bool {
        self.padding.is_none() && self.background.is_none() && self.foreground.is_none()
    }
}

struct NodeRef {
    node: Option<Node>,
}

const MAX_OVERRIDE_NODES: usize = 256;
const MAX_UNDO_DEPTH: usize = 128;

/// Scene-thread-only inspection editor.
pub struct SceneEditor {
    tree: Rc<RefCell<NodeTree>>,
    indexed_root: RefCell<Option<Node>>,
    indexed_version: Cell<Option<u64>>,
    node_index: RefCell<BTreeMap<String, Rc<RefCell<NodeRef>>>>,
    overrides: RefCell<BTreeMap<String, Style>>,
    override_nodes: RefCell<BTreeMap<String, Rc<RefCell<NodeRef>>>>,
    undo: RefCell<Vec<BTreeMap<String, Style>>>,
    redo: RefCell<Vec<BTreeMap<String, Style>>>,
    selected: RefCell<Option<Node>>,
    hovered: RefCell<Option<Node>>,
    picking: Cell<bool>,
    swallowed_buttons: RefCell<Vec<cove_platform::MouseButton>>,
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
            override_nodes: RefCell::new(BTreeMap::new()),
            undo: RefCell::new(Vec::new()),
            redo: RefCell::new(Vec::new()),
            selected: RefCell::new(None),
            hovered: RefCell::new(None),
            picking: Cell::new(false),
            swallowed_buttons: RefCell::new(Vec::new()),
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
                let reference = Rc::new(RefCell::new(NodeRef { node: Some(node.clone()) }));
                index.insert(node.id().0.to_string(), reference.clone());
                index.insert(SceneInspector::identifier(&node), reference);
                stack.extend(node.children());
            }
            self.indexed_version.set(Some(root.subtree_structure_version()));
            *self.indexed_root.borrow_mut() = Some(root);
        }
        self.node_index
            .borrow()
            .get(id)
            .and_then(|reference| reference.borrow().node.clone())
            .filter(|node| node.parent().is_some() || self.tree.borrow().root().as_ref() == Some(node))
    }

    pub fn select(&self, id: Option<&str>) {
        let node = id.and_then(|id| self.find(id));
        *self.selected.borrow_mut() = node;
        *self.hovered.borrow_mut() = None;
        self.notify();
    }

    pub fn selected_node(&self) -> Option<Node> {
        self.selected.borrow().clone()
    }

    pub fn hovered_node(&self) -> Option<Node> {
        self.hovered.borrow().clone()
    }

    pub fn picking(&self) -> bool {
        self.picking.get()
    }

    pub fn state(&self) -> InspectionState {
        self.prune();
        InspectionState {
            selected_id: self.selected.borrow().as_ref().map(|node| node.id().0.to_string()),
            hovered_id: self.hovered.borrow().as_ref().map(|node| node.id().0.to_string()),
            picking: self.picking.get(),
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
        if self.selected.borrow().as_ref().is_some_and(|node| self.find(&node.id().0.to_string()).is_none()) {
            *self.selected.borrow_mut() = None;
        }
        if self.hovered.borrow().as_ref().is_some_and(|node| self.find(&node.id().0.to_string()).is_none()) {
            *self.hovered.borrow_mut() = None;
        }
    }

    /// Disconnect/stop discards temporary styles and history. An outstanding
    /// picker mouse-up is still consumed to avoid leaking half a gesture.
    pub fn reset(&self) {
        self.apply(BTreeMap::new());
        self.undo.borrow_mut().clear();
        self.redo.borrow_mut().clear();
        *self.selected.borrow_mut() = None;
        *self.hovered.borrow_mut() = None;
        self.picking.set(false);
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
                continue;
            };
            node.set_debug_background_color(style.and_then(|style| style.background));
            node.set_debug_foreground_color(style.and_then(|style| style.foreground));
            if let Some(layout) = node.layout_node() {
                layout.set_debug_padding(style.and_then(|style| style.padding));
            }
            node.mark_render_dirty(InvalidationSource::style_set("debugStyle"));
            if style.is_some() {
                self.override_nodes
                    .borrow_mut()
                    .insert(id.clone(), Rc::new(RefCell::new(NodeRef { node: Some(node) })));
            } else {
                self.override_nodes.borrow_mut().remove(&id);
            }
        }
        *self.overrides.borrow_mut() = live;
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

    /// Handle one `inspect.*` command. Validation already ran in
    /// `Session::validate`, so this only enforces scene-dependent limits.
    pub fn handle(&self, request: &Envelope) -> Envelope {
        self.prune();
        let payload = request.payload.as_ref().and_then(|value| value.as_object());
        match request.r#type.as_str() {
            "inspect.pick.start" => {
                self.picking.set(true);
                *self.hovered.borrow_mut() = None;
            }
            "inspect.pick.stop" => {
                self.picking.set(false);
                *self.hovered.borrow_mut() = None;
            }
            "inspect.hover" | "inspect.pick" => {
                let Some(point) = payload.and_then(|payload| {
                    Some(Point {
                        x: payload.get("x")?.as_f64()? as f32,
                        y: payload.get("y")?.as_f64()? as f32,
                    })
                }) else {
                    return error(request, "bad_request", "Expected finite window coordinates x/y");
                };
                let node = self.pick(point);
                if request.r#type == "inspect.hover" {
                    *self.hovered.borrow_mut() = node;
                } else {
                    *self.selected.borrow_mut() = node;
                    *self.hovered.borrow_mut() = None;
                    self.picking.set(false);
                }
            }
            "inspect.style.set" | "inspect.style.clear" => {
                let Some(id) = payload
                    .and_then(|payload| payload.get("id"))
                    .and_then(|value| value.as_str())
                else {
                    return error(request, "bad_request", "Expected node id");
                };
                let Some(node) = self.find(id) else {
                    return error(request, "stale_node", "Selected node is no longer in the scene");
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
                        return error(request, "bad_request", "Expected node id and supported properties");
                    };
                    if properties.get("padding").is_some_and(|value| value.is_object())
                        && node.layout_node().is_none()
                    {
                        return error(
                            request,
                            "unsupported",
                            "This composition anchor has no layout box; select a child",
                        );
                    }
                    let mut style = next.get(&node_id).copied().unwrap_or(Style {
                        padding: None,
                        background: None,
                        foreground: None,
                    });
                    for (key, value) in properties {
                        match key.as_str() {
                            "padding" => {
                                style.padding = value.as_object().map(|edges| LayoutInsets {
                                    top: edges.get("top").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                                    right: edges.get("right").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                                    bottom: edges.get("bottom").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                                    left: edges.get("left").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                                });
                            }
                            "backgroundColor" => {
                                style.background = value.as_str().and_then(Self::color);
                            }
                            "foregroundColor" => {
                                style.foreground = value.as_str().and_then(Self::color);
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
                    return error(request, "limit", "At most 256 nodes may have temporary styles");
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
        let mut response = request.ok().unwrap_or_else(|| Envelope::new(
            format!("{}.ok", request.r#type),
            request.id,
            None,
        ));
        response.payload = serde_json::to_value(state).ok();
        response
    }

    /// Visual hit test, in reverse paint order. Text and disabled controls are
    /// inspectable; clipping, scrolling and invisible subtrees match rendering.
    ///
    /// Children are visited before their parent (last child first, highest z
    /// first) so the topmost painted node wins, exactly as Swift's recursive
    /// `visit` does.
    pub fn pick(&self, point: Point) -> Option<Node> {
        let root = self.tree.borrow().root()?;
        fn visit(node: &Node, point: Point) -> Option<Node> {
            if !node.is_layout_visible()
                || node.opacity() <= 0.0
                || node.subtree_opacity() <= 0.0
                || node.layout_node().is_some_and(|layout| layout.display() == Display::None)
            {
                return None;
            }
            let frame = node.absolute_frame();
            let inside = frame.contains(point);
            if !inside && node.clips_to_bounds() {
                return None;
            }
            // Reverse paint order: greater z first, then later siblings first.
            let mut children: Vec<(usize, Node)> = node.children().into_iter().enumerate().collect();
            children.sort_by(|(left_index, left), (right_index, right)| {
                right
                    .z_index()
                    .partial_cmp(&left.z_index())
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(right_index.cmp(left_index))
            });
            for (_, child) in children {
                if let Some(hit) = visit(&child, point) {
                    return Some(hit);
                }
            }
            (inside && (node.layout_node().is_some() || node.background_color().is_some())).then(|| node.clone())
        }
        visit(&root, point)
    }

    /// Consume pointer input while picking. Returns `true` when the event was
    /// handled and the host must not forward it to the scene.
    pub fn intercept(&self, event: &cove_platform::InputEvent) -> bool {
        use cove_platform::InputEvent;
        if let InputEvent::MouseButtonUp(button) = event
            && let Some(position) = self
                .swallowed_buttons
                .borrow()
                .iter()
                .position(|candidate| *candidate == button.button)
        {
            self.swallowed_buttons.borrow_mut().remove(position);
            return true;
        }
        if !self.picking.get() {
            return false;
        }
        match event {
            InputEvent::MouseMotion(motion) => {
                *self.hovered.borrow_mut() = self.pick(Point { x: motion.x, y: motion.y });
                self.notify();
                true
            }
            InputEvent::MouseButtonDown(button) => {
                self.swallowed_buttons.borrow_mut().push(button.button);
                if button.button == cove_platform::MouseButton::Left {
                    *self.selected.borrow_mut() = self.pick(Point { x: button.x, y: button.y });
                    self.picking.set(false);
                    *self.hovered.borrow_mut() = None;
                    self.notify();
                }
                true
            }
            InputEvent::KeyDown(key) => {
                // 41 is Escape in the SDL/USB-HID scancode table.
                if key.scancode == 41 {
                    self.picking.set(false);
                    *self.hovered.borrow_mut() = None;
                    self.notify();
                }
                true
            }
            InputEvent::MouseButtonUp(_) | InputEvent::MouseWheel(_) | InputEvent::KeyUp(_) => true,
            _ => false,
        }
    }

    /// Draw the hover/selection outline plus padding tint, clipped by ancestors.
    pub fn draw_overlay(&self, list: &mut DrawList) {
        self.prune();
        let node = self.hovered.borrow().clone().or_else(|| self.selected.borrow().clone());
        let Some(node) = node else { return };
        let frame = node.absolute_frame();
        if frame.width <= 0.0 || frame.height <= 0.0 {
            return;
        }
        let mut clips = Vec::new();
        let mut parent = node.parent();
        while let Some(current) = parent {
            if current.clips_to_bounds() {
                clips.push(current.absolute_frame());
            }
            parent = current.parent();
        }
        for clip in &clips {
            list.push_clip_rect(*clip);
        }
        let hovered = self.hovered.borrow().is_some();
        if hovered
            && let Some(padding) = node.layout_node().map(|layout| layout.resolved_padding())
        {
            let tint = Color::from_rgba8(100, 210, 130, 75);
            let _ = list.add_rect(Rect::new(frame.x, frame.y, frame.width, padding.top), tint);
            let _ = list.add_rect(
                Rect::new(frame.x, frame.y + (frame.height - padding.bottom).max(0.0), frame.width, padding.bottom),
                tint,
            );
            let _ = list.add_rect(Rect::new(frame.x, frame.y, padding.left, frame.height), tint);
            let _ = list.add_rect(
                Rect::new(frame.x + (frame.width - padding.right).max(0.0), frame.y, padding.right, frame.height),
                tint,
            );
        }
        let color = if hovered {
            Color::from_rgba8(250, 180, 40, 255)
        } else {
            Color::from_rgba8(80, 160, 255, 255)
        };
        let _ = list.add_rect(Rect::new(frame.x, frame.y, frame.width, 2.0_f32.min(frame.height)), color);
        let _ = list.add_rect(
            Rect::new(frame.x, frame.y + (frame.height - 2.0).max(0.0), frame.width, 2.0_f32.min(frame.height)),
            color,
        );
        let _ = list.add_rect(Rect::new(frame.x, frame.y, 2.0_f32.min(frame.width), frame.height), color);
        let _ = list.add_rect(
            Rect::new(frame.x + (frame.width - 2.0).max(0.0), frame.y, 2.0_f32.min(frame.width), frame.height),
            color,
        );
        for _ in &clips {
            list.pop_clip_rect();
        }
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
        let rgba = if digits.len() == 6 { (value << 8) | 255 } else { value };
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

impl std::fmt::Debug for SceneEditor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SceneEditor")
            .field("overrides", &self.overrides.borrow().len())
            .field("picking", &self.picking.get())
            .finish_non_exhaustive()
    }
}
