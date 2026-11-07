//! Translation of CoveUIScene/Input/EventDispatcher.swift.
//!
//! Routes window-local input events to nodes in the CoveUI tree.
//!
//! Per-window, per-tree. Construct one `EventDispatcher` for each `NodeTree`
//! and feed it events from the platform host's polling loop.
//!
//! Delivery model (DOM-style):
//! 1. `capture` phase — root → target, ancestors get a chance first
//! 2. `target` phase — the deepest hit-tested node
//! 3. `bubble`  phase — target → root, ancestors get a chance last
//!
//! A handler returning `Handled` stops propagation in the current phase.
//! `PointerCapture::acquire` overrides hit-testing for non-down pointer events.
use super::interaction_registry::{InputHandlerSlot, RoutedInputNode};
use crate::scene::{
    EventPhase, EventResult, FocusChain, Handlers, HitResult, HitTester, HoverPhase,
    InputDispatchKind, InputDispatchTrace, InputHandlerRole, InputHandlerRoute,
    InputRoutingPriority, InteractionRegistry, KeyPhase, Node, NodeTree, PointerCapture,
    PointerPhase, WheelGesture, WheelRoutingAttachmentKey, WheelRoutingPriority,
};
use crate::{ElementId, Point};
use cove_platform::{
    InputEvent, KeyEvent, MouseButton, MouseButtonEvent, MouseMotionEvent, MouseWheelEvent,
    SystemCursor, TextEditingEvent, WindowId,
};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

type CursorSink = Rc<dyn Fn(SystemCursor)>;
type EventSink = Rc<dyn Fn(&InputEvent)>;
type EventInterceptor = Rc<dyn Fn(&InputEvent) -> bool>;
type TraceSink = Rc<dyn Fn(&InputDispatchTrace)>;

pub struct EventDispatcher {
    pub tree: Rc<RefCell<NodeTree>>,
    pub interactions: Rc<InteractionRegistry>,
    pub capture: Rc<PointerCapture>,
    pub focus_chain: Rc<FocusChain>,
    pub window_id: WindowId,
    /// Invoked whenever the resolved hover cursor changes. The dispatcher
    /// computes the cursor by walking the hover path leaf → root and using
    /// the deepest non-`None` `Node::cursor`. `None` resolves to `Arrow`.
    pub cursor_sink: Option<CursorSink>,
    /// Optional debug hook for tooling. Invoked once per dispatched event.
    pub event_sink: Option<EventSink>,
    /// Consumed diagnostic input never reaches application routes or recording.
    pub event_interceptor: Option<EventInterceptor>,
    pub trace_sink: Option<TraceSink>,
    active_cursor: Cell<SystemCursor>,
    /// Last known pointer position from `MouseMotion`. Used to hit-test wheel
    /// events (which carry no position on SDL3) so they reach the node under
    /// the cursor instead of only the root or focused node.
    last_cursor: Cell<Option<Point>>,
    /// Root → leaf path currently considered hovered. Derived from motion
    /// hit-testing and diffed against the next motion path to synthesize
    /// enter / leave transitions.
    hovered_path: RefCell<Vec<Node>>,
    wheel_gesture: RefCell<WheelGesture>,
}

impl EventDispatcher {
    pub fn new(
        tree: Rc<RefCell<NodeTree>>,
        interactions: Rc<InteractionRegistry>,
        capture: Rc<PointerCapture>,
        focus_chain: Rc<FocusChain>,
        window_id: WindowId,
    ) -> Self {
        Self {
            tree,
            interactions,
            capture,
            focus_chain,
            window_id,
            cursor_sink: None,
            event_sink: None,
            event_interceptor: None,
            trace_sink: None,
            active_cursor: Cell::new(SystemCursor::Arrow),
            last_cursor: Cell::new(None),
            hovered_path: RefCell::new(Vec::new()),
            wheel_gesture: RefCell::new(WheelGesture::default()),
        }
    }

    pub fn cursor(&self) -> SystemCursor {
        self.active_cursor.get()
    }

    pub fn hovered_path(&self) -> Vec<Node> {
        self.hovered_path.borrow().clone()
    }

    // MARK: - Public dispatch

    pub fn dispatch(&self, event: &InputEvent) -> bool {
        if let Some(interceptor) = &self.event_interceptor
            && interceptor(event)
        {
            return false;
        }
        if let Some(sink) = &self.event_sink {
            sink(event);
        }
        if let Some(target) = self.capture.target()
            && !target.accepts_subtree_input()
        {
            self.capture.release();
        }
        if let Some(focused) = self.focus_chain.focused()
            && !focused.accepts_subtree_input()
        {
            self.focus_chain.clear();
        }
        match event {
            InputEvent::MouseButtonDown(e) => {
                self.wheel_gesture.borrow_mut().interrupt();
                self.dispatch_pointer_down(e);
            }
            InputEvent::MouseButtonUp(e) => self.dispatch_pointer_up(e),
            InputEvent::MouseMotion(e) => self.dispatch_motion(e),
            InputEvent::MouseWheel(e) => self.dispatch_wheel(e),
            InputEvent::KeyDown(e) => self.dispatch_key(e, KeyPhase::Down),
            InputEvent::KeyUp(e) => self.dispatch_key(e, KeyPhase::Up),
            InputEvent::TextEditing(e) => self.dispatch_text_editing(e),
            InputEvent::TextInput(text) => self.dispatch_text(text),
            InputEvent::WindowFocusLost | InputEvent::WindowMinimized => {
                self.wheel_gesture.borrow_mut().interrupt();
            }
            // Window lifecycle events are not handled here.
            _ => {}
        }
        true
    }

    // MARK: - Pointer

    fn dispatch_pointer_down(&self, event: &MouseButtonEvent) {
        let point = Point::new(event.x, event.y);
        self.last_cursor.set(Some(point));
        let kind = EventKind::Pointer(event.clone(), PointerPhase::Down);
        if self.deliver_global_routes(
            &kind,
            &InputHandlerRole::Overlay,
            InputRoutingPriority::MODAL,
            &HashSet::new(),
        ) == EventResult::Handled
        {
            return;
        }
        let Some(hit) = self.hit_test(point) else {
            if event.button == MouseButton::Left {
                self.focus_chain.focus(None, false);
            }
            return;
        };
        // Scroll chrome belongs to the actual hit path. A portal above a
        // viewport must not arm a background touch gesture whose release
        // will only visit the portal's ancestors.
        if self.deliver_priority(
            &hit.path,
            &kind,
            InputRoutingPriority::CHROME,
            EventPhase::Capture,
        ) == EventResult::Handled
        {
            return;
        }
        if !self.focus_chain.permits_input(&hit.node) {
            return;
        }
        // A press on chrome (a scroll bar) belongs to the chrome alone: it
        // never falls through to a text selection or control behind it.
        if hit.node.chrome_contains(hit.local_point) {
            let _ = self.invoke(&hit.node, &kind, EventPhase::Target);
            return;
        }
        let previous_focus = self.focus_chain.focused();
        let _ = self.deliver(&hit.path, &kind);
        // Give explicit handler focus changes priority; otherwise focus the
        // clicked control or blur an editor when clicking the canvas.
        if event.button == MouseButton::Left && self.focus_chain.focused() == previous_focus {
            let target = hit
                .path
                .iter()
                .rev()
                .find(|node| node.is_focusable() && node.accepts_subtree_input());
            let should_focus = target.is_none_or(|node| node.automatically_focus_on_pointer_down());
            if should_focus {
                self.focus_chain.focus(target, false);
            }
        }
    }

    fn dispatch_pointer_up(&self, event: &MouseButtonEvent) {
        let point = Point::new(event.x, event.y);
        self.last_cursor.set(Some(point));
        let kind = EventKind::Pointer(event.clone(), PointerPhase::Up);
        if let Some(captured) = self.capture.target() {
            let path = path_from_root(&captured);
            let _ = self.deliver(&path, &kind);
            if self.capture.target().is_none() {
                self.refresh_hover(point);
            }
            return;
        }
        let Some(hit) = self.hit_test(point) else {
            return;
        };
        let _ = self.deliver(&hit.path, &kind);
        self.refresh_hover(point);
    }

    fn dispatch_motion(&self, event: &MouseMotionEvent) {
        let point = Point::new(event.x, event.y);
        self.last_cursor.set(Some(point));
        let kind = EventKind::Motion(event.clone());
        if let Some(captured) = self.capture.target() {
            let path = path_from_root(&captured);
            self.update_hover_path(&path);
            if self.capture.allows_gesture_transfer()
                && self.deliver_scroll_gesture(&path, event) == EventResult::Handled
            {
                return;
            }
            let _ = self.deliver(&path, &kind);
            return;
        }
        let Some(hit) = self.hit_test(point) else {
            self.update_hover_path(&[]);
            return;
        };
        self.update_hover_path(&hit.path);
        if self.deliver_scroll_gesture(&hit.path, event) == EventResult::Handled {
            return;
        }
        let _ = self.deliver(&hit.path, &kind);
        // A node may vary its cursor within its own bounds (a link inside a
        // paragraph), so motion re-resolves it even when the path is unchanged.
        let hovered = self.hovered_path();
        self.update_cursor(&hovered);
    }

    /// The deepest scroll container gets the first gesture attempt, before a
    /// pending child tap consumes motion. Exclusive control drags skip this.
    fn deliver_scroll_gesture(&self, path: &[Node], event: &MouseMotionEvent) -> EventResult {
        let kind = EventKind::Motion(event.clone());
        for candidate in self.route_candidates(path, &kind, InputRoutingPriority::CHROME) {
            if candidate.route.role != InputHandlerRole::ScrollChrome {
                continue;
            }
            if self.invoke(&candidate.node, &kind, EventPhase::Target) == EventResult::Handled {
                return EventResult::Handled;
            }
        }
        EventResult::Ignored
    }

    fn refresh_hover(&self, point: Point) {
        match self.hit_test(point) {
            Some(hit) => self.update_hover_path(&hit.path),
            None => self.update_hover_path(&[]),
        }
    }

    fn dispatch_wheel(&self, event: &MouseWheelEvent) {
        if !(event.x.is_finite()
            && event.y.is_finite()
            && event.input.timestamp.is_finite()
            && event.input.timestamp >= 0.0
            && event.mouse_x.is_none_or(f32::is_finite)
            && event.mouse_y.is_none_or(f32::is_finite))
        {
            return;
        }
        if !self.wheel_gesture.borrow_mut().prepare(event.input) {
            return;
        }
        let pinned_path = {
            let tree = self.tree.borrow();
            self.wheel_gesture.borrow().path(&tree)
        };
        if let Some(path) = pinned_path {
            match path.last() {
                Some(target) if self.focus_chain.permits_input(target) => {
                    let _ = self.deliver_wheel(&path, event);
                }
                _ => self.wheel_gesture.borrow_mut().interrupt(),
            }
            self.wheel_gesture.borrow_mut().finish(event.input);
            return;
        }
        let mut event = event.clone();
        if let (Some(mouse_x), Some(mouse_y)) = (event.mouse_x, event.mouse_y) {
            if mouse_x == 0.0
                && mouse_y == 0.0
                && let Some(last) = self.last_cursor.get()
            {
                event.mouse_x = Some(last.x);
                event.mouse_y = Some(last.y);
            } else {
                self.last_cursor.set(Some(Point::new(mouse_x, mouse_y)));
            }
        } else if let Some(last) = self.last_cursor.get() {
            event.mouse_x = Some(last.x);
            event.mouse_y = Some(last.y);
        }
        let hit_path = self
            .last_cursor
            .get()
            .and_then(|cursor| self.hit_test(cursor))
            .map(|hit| hit.path);
        let focused_path = self
            .focus_chain
            .focused()
            .map(|focused| path_from_root(&focused));
        let preferred_focused_path = self.preferred_focused_wheel_path(focused_path.as_deref());
        let preferred_hit_path = hit_path.clone().filter(|path| {
            path.iter().any(|node| {
                node.attachment::<WheelRoutingPriority>(WheelRoutingAttachmentKey::PRIORITY)
                    == Some(WheelRoutingPriority::PreferHit)
            })
        });

        // Wheel delivery is target-first rather than full capture/target/bubble.
        // Nested scrollables need the deepest target to consume the gesture
        // before an ancestor ScrollView moves, otherwise inner editors can
        // never keep their own scroll context.
        if let Some(path) = preferred_hit_path.as_deref()
            && self.deliver_wheel(path, &event) == EventResult::Handled
        {
            return;
        }
        if let Some(path) = preferred_focused_path.as_deref()
            && !same_wheel_target(Some(path), preferred_hit_path.as_deref())
            && self.deliver_wheel(path, &event) == EventResult::Handled
        {
            return;
        }
        if let Some(path) = hit_path.as_deref()
            && !same_wheel_target(Some(path), preferred_hit_path.as_deref())
            && !same_wheel_target(Some(path), preferred_focused_path.as_deref())
            && self.deliver_wheel(path, &event) == EventResult::Handled
        {
            return;
        }
        if let Some(path) = focused_path.as_deref()
            && !same_wheel_target(Some(path), preferred_focused_path.as_deref())
            && !same_wheel_target(Some(path), hit_path.as_deref())
            && self.deliver_wheel(path, &event) == EventResult::Handled
        {
            return;
        }
        if self.focus_chain.has_modal_scope() {
            self.wheel_gesture.borrow_mut().finish(event.input);
            return;
        }
        let kind = EventKind::Wheel(event.clone());
        if self.deliver_global_routes(
            &kind,
            &InputHandlerRole::Scroll,
            InputRoutingPriority::NORMAL,
            &HashSet::new(),
        ) == EventResult::Handled
        {
            self.wheel_gesture.borrow_mut().finish(event.input);
            return;
        }
        let root = self.tree.borrow().root();
        if let Some(root) = root {
            let _ = self.deliver_wheel(&[root], &event);
        }
        self.wheel_gesture.borrow_mut().finish(event.input);
    }

    /// Hit-test entry point. Walks the live `Node` tree — classification is
    /// read fresh off each `Node`.
    fn hit_test(&self, point: Point) -> Option<HitResult> {
        let root = self.tree.borrow().root()?;
        HitTester::hit_test(&root, point)
    }

    // MARK: - Key

    fn dispatch_key(&self, event: &KeyEvent, phase: KeyPhase) {
        self.focus_chain.ensure_modal_focus();
        let kind = EventKind::Key(event.clone(), phase);
        // A mounted transient overlay is visually and semantically above the
        // focused control. Give it first refusal so Escape/menu navigation is
        // reliable even if a viewport or drag capture retained keyboard focus.
        if self.deliver_global_routes(
            &kind,
            &InputHandlerRole::Overlay,
            InputRoutingPriority::MODAL,
            &HashSet::new(),
        ) == EventResult::Handled
        {
            return;
        }
        // Pointer-capture intercept: while a node owns capture (typically a
        // drag in progress), give its key handler the first opportunity to
        // consume the event. Lets drags implement Esc-to-cancel without
        // also needing keyboard focus.
        if let Some(captured) = self.capture.target()
            && self.invoke(&captured, &kind, EventPhase::Target) == EventResult::Handled
        {
            return;
        }
        if phase == KeyPhase::Down
            && (event.modifiers.has_ctrl() || event.modifiers.has_gui())
            && [4, 6, 25, 27, 28, 29].contains(&event.scancode)
            && let Some(focused) = self.focus_chain.focused()
            && self.interactions.handlers(&focused).text.is_some()
            && self.invoke(&focused, &kind, EventPhase::Target) == EventResult::Handled
        {
            return;
        }
        let focused_path = self
            .focus_chain
            .focused()
            .map(|focused| path_from_root(&focused));
        if let Some(path) = focused_path.as_deref()
            && self.deliver_key_path(path, event, phase) == EventResult::Handled
        {
            return;
        }
        // Editable code surfaces can consume Tab above. Other controls use
        // the framework's keyboard traversal rather than requiring a host hook.
        if phase == KeyPhase::Down
            && event.scancode == 43
            && !(event.modifiers.has_ctrl()
                || event.modifiers.has_gui()
                || event.modifiers.has_alt())
            && let Some(root) = self.tree.borrow().root()
        {
            if event.modifiers.has_shift() {
                self.focus_chain.focus_previous(&root);
            } else {
                self.focus_chain.focus_next(&root);
            }
            return;
        }
        if self.focus_chain.has_modal_scope() {
            return;
        }
        let excluded: HashSet<ElementId> = focused_path
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(Node::id)
            .collect();
        let _ = self.deliver_global_routes(
            &kind,
            &InputHandlerRole::Shortcut,
            InputRoutingPriority::SYSTEM,
            &excluded,
        );
    }

    fn dispatch_text(&self, text: &str) {
        let Some(focused) = self.focus_chain.focused() else {
            return;
        };
        let path = path_from_root(&focused);
        let _ = self.deliver(&path, &EventKind::Text(text.to_string()));
    }

    fn dispatch_text_editing(&self, event: &TextEditingEvent) {
        let Some(focused) = self.focus_chain.focused() else {
            return;
        };
        let path = path_from_root(&focused);
        let _ = self.deliver(&path, &EventKind::Editing(event.clone()));
    }

    // MARK: - Delivery

    fn deliver(&self, path: &[Node], kind: &EventKind) -> EventResult {
        let Some((target, ancestors)) = path.split_last() else {
            return EventResult::Ignored;
        };

        // Capture phase: root → target (exclusive of target).
        for node in ancestors {
            if self.invoke(node, kind, EventPhase::Capture) == EventResult::Handled {
                return EventResult::Handled;
            }
        }

        // Target phase.
        if self.invoke(target, kind, EventPhase::Target) == EventResult::Handled {
            return EventResult::Handled;
        }

        // Bubble phase: target's parent → root.
        for node in ancestors.iter().rev() {
            if self.invoke(node, kind, EventPhase::Bubble) == EventResult::Handled {
                return EventResult::Handled;
            }
        }

        EventResult::Ignored
    }

    fn deliver_wheel(&self, path: &[Node], event: &MouseWheelEvent) -> EventResult {
        let Some((target, ancestors)) = path.split_last() else {
            return EventResult::Ignored;
        };
        let kind = EventKind::Wheel(event.clone());

        if self.invoke(target, &kind, EventPhase::Target) == EventResult::Handled {
            self.wheel_gesture
                .borrow_mut()
                .consumed(target, event.input);
            return EventResult::Handled;
        }

        for node in ancestors.iter().rev() {
            if self.invoke(node, &kind, EventPhase::Bubble) == EventResult::Handled {
                self.wheel_gesture.borrow_mut().consumed(node, event.input);
                return EventResult::Handled;
            }
        }

        EventResult::Ignored
    }

    fn deliver_priority(
        &self,
        path: &[Node],
        kind: &EventKind,
        min_priority: InputRoutingPriority,
        phase: EventPhase,
    ) -> EventResult {
        for candidate in self.route_candidates(path, kind, min_priority) {
            if self.invoke(&candidate.node, kind, phase) == EventResult::Handled {
                return EventResult::Handled;
            }
        }
        EventResult::Ignored
    }

    fn deliver_global_routes(
        &self,
        kind: &EventKind,
        role: &InputHandlerRole,
        min_priority: InputRoutingPriority,
        excluded: &HashSet<ElementId>,
    ) -> EventResult {
        let Some(root) = self.tree.borrow().root() else {
            return EventResult::Ignored;
        };
        let routed = self
            .interactions
            .routed_nodes(handler_slot(kind), role, min_priority);
        let mut candidates: Vec<RouteCandidate> = Vec::with_capacity(routed.len());
        for item in routed {
            let RoutedInputNode { node, route } = item;
            if excluded.contains(&node.id()) {
                continue;
            }
            if !node.accepts_subtree_input() {
                continue;
            }
            if matches!(
                kind,
                EventKind::Key(..) | EventKind::Text(_) | EventKind::Editing(_)
            ) && !self.focus_chain.permits_keyboard_input(&node)
            {
                continue;
            }
            let Some(depth) = depth_under(&node, &root) else {
                continue;
            };
            candidates.push(RouteCandidate { node, route, depth });
        }
        candidates.sort_by(route_candidate_cmp);
        for candidate in &candidates {
            let phase =
                if *role == InputHandlerRole::Shortcut || *role == InputHandlerRole::ScrollChrome {
                    EventPhase::Capture
                } else {
                    EventPhase::Target
                };
            if self.invoke(&candidate.node, kind, phase) == EventResult::Handled {
                if let EventKind::Wheel(event) = kind {
                    self.wheel_gesture
                        .borrow_mut()
                        .consumed(&candidate.node, event.input);
                }
                return EventResult::Handled;
            }
        }
        EventResult::Ignored
    }

    fn route_candidates(
        &self,
        path: &[Node],
        kind: &EventKind,
        min_priority: InputRoutingPriority,
    ) -> Vec<RouteCandidate> {
        let mut out: Vec<RouteCandidate> = Vec::new();
        for (index, node) in path.iter().enumerate() {
            let handlers = self.interactions.handlers(node);
            let Some(route) = route_for(&handlers, kind) else {
                continue;
            };
            if route.priority < min_priority {
                continue;
            }
            out.push(RouteCandidate {
                node: node.clone(),
                route,
                depth: index,
            });
        }
        out.sort_by(route_candidate_cmp);
        out
    }

    fn deliver_key_path(&self, path: &[Node], event: &KeyEvent, phase: KeyPhase) -> EventResult {
        let kind = EventKind::Key(event.clone(), phase);
        for candidate in self.route_candidates(path, &kind, InputRoutingPriority::SYSTEM) {
            if candidate.route.role != InputHandlerRole::Shortcut {
                continue;
            }
            if self.invoke(&candidate.node, &kind, EventPhase::Capture) == EventResult::Handled {
                return EventResult::Handled;
            }
        }
        self.deliver(path, &kind)
    }

    fn preferred_focused_wheel_path(&self, focused_path: Option<&[Node]>) -> Option<Vec<Node>> {
        let focused_path = focused_path?;
        let focused = focused_path.last()?;
        let priority =
            focused.attachment::<WheelRoutingPriority>(WheelRoutingAttachmentKey::PRIORITY)?;
        if priority != WheelRoutingPriority::PreferFocused {
            return None;
        }
        if let Some(pointer) = self.last_cursor.get()
            && !focused.absolute_frame().contains(pointer)
        {
            return None;
        }
        Some(focused_path.to_vec())
    }

    fn invoke(&self, node: &Node, kind: &EventKind, phase: EventPhase) -> EventResult {
        if !node.accepts_subtree_input() {
            return EventResult::Ignored;
        }
        let handlers = self.interactions.handlers(node);
        let result = match kind {
            EventKind::Pointer(event, pointer_phase) => handlers
                .pointer
                .as_ref()
                .map_or(EventResult::Ignored, |handler| {
                    handler(event, *pointer_phase, phase)
                }),
            EventKind::Motion(event) => handlers
                .motion
                .as_ref()
                .map_or(EventResult::Ignored, |handler| handler(event, phase)),
            EventKind::Wheel(event) => handlers
                .wheel
                .as_ref()
                .map_or(EventResult::Ignored, |handler| handler(event, phase)),
            EventKind::Key(event, key_phase) => {
                let handler = match key_phase {
                    KeyPhase::Down => handlers.key.as_ref(),
                    KeyPhase::Up => handlers.key_up.as_ref(),
                };
                handler.map_or(EventResult::Ignored, |handler| handler(event, phase))
            }
            EventKind::Editing(event) => handlers
                .editing
                .as_ref()
                .map_or(EventResult::Ignored, |handler| handler(event, phase)),
            EventKind::Text(text) => handlers
                .text
                .as_ref()
                .map_or(EventResult::Ignored, |handler| handler(text, phase)),
        };
        if let Some(sink) = &self.trace_sink {
            sink(&InputDispatchTrace {
                kind: dispatch_kind(kind),
                node_id: node.id(),
                phase,
                route: route_for(&handlers, kind),
                result,
            });
        }
        result
    }

    // MARK: - Helpers

    fn update_hover_path(&self, new_path: &[Node]) {
        let old_path = self.hovered_path.borrow().clone();
        let common = common_prefix_length(&old_path, new_path);

        if common == old_path.len() && common == new_path.len() {
            return;
        }

        if common < old_path.len() {
            for node in old_path[common..].iter().rev() {
                if let Some(handler) = self.interactions.handlers(node).hover {
                    handler(HoverPhase::Leave);
                }
            }
        }
        if common < new_path.len() {
            for node in &new_path[common..] {
                if let Some(handler) = self.interactions.handlers(node).hover {
                    handler(HoverPhase::Enter);
                }
            }
        }

        *self.hovered_path.borrow_mut() = new_path.to_vec();
        self.update_cursor(new_path);
    }

    fn update_cursor(&self, path: &[Node]) {
        let mut resolved = SystemCursor::Arrow;
        // Over chrome the pointer is the plain arrow, whatever the content
        // underneath (selectable text) would ask for.
        let over_chrome = path
            .last()
            .zip(self.last_cursor.get())
            .is_some_and(|(node, point)| node.chrome_contains(node.convert_from_window(point)));
        for node in path.iter().rev().filter(|_| !over_chrome) {
            if let Some(cursor) = node.cursor() {
                resolved = cursor;
                break;
            }
        }
        if resolved != self.active_cursor.get() {
            self.active_cursor.set(resolved);
            if let Some(sink) = &self.cursor_sink {
                sink(resolved);
            }
        }
    }
}

enum EventKind {
    Pointer(MouseButtonEvent, PointerPhase),
    Motion(MouseMotionEvent),
    Wheel(MouseWheelEvent),
    Key(KeyEvent, KeyPhase),
    Editing(TextEditingEvent),
    Text(String),
}

struct RouteCandidate {
    node: Node,
    route: InputHandlerRoute,
    depth: usize,
}

/// Priority first (higher wins), then depth (deeper wins).
fn route_candidate_cmp(lhs: &RouteCandidate, rhs: &RouteCandidate) -> std::cmp::Ordering {
    rhs.route
        .priority
        .cmp(&lhs.route.priority)
        .then_with(|| rhs.depth.cmp(&lhs.depth))
}

fn same_wheel_target(lhs: Option<&[Node]>, rhs: Option<&[Node]>) -> bool {
    match (
        lhs.and_then(|path| path.last()),
        rhs.and_then(|path| path.last()),
    ) {
        (Some(lhs), Some(rhs)) => lhs == rhs,
        _ => false,
    }
}

fn handler_slot(kind: &EventKind) -> InputHandlerSlot {
    match kind {
        EventKind::Pointer(..) => InputHandlerSlot::Pointer,
        EventKind::Motion(_) => InputHandlerSlot::Motion,
        EventKind::Wheel(_) => InputHandlerSlot::Wheel,
        EventKind::Key(_, KeyPhase::Down) => InputHandlerSlot::Key,
        EventKind::Key(_, KeyPhase::Up) => InputHandlerSlot::KeyUp,
        EventKind::Editing(_) => InputHandlerSlot::Editing,
        EventKind::Text(_) => InputHandlerSlot::Text,
    }
}

fn dispatch_kind(kind: &EventKind) -> InputDispatchKind {
    match kind {
        EventKind::Pointer(_, PointerPhase::Down) => InputDispatchKind::PointerDown,
        EventKind::Pointer(_, PointerPhase::Up) => InputDispatchKind::PointerUp,
        EventKind::Motion(_) => InputDispatchKind::Motion,
        EventKind::Wheel(_) => InputDispatchKind::Wheel,
        EventKind::Key(_, KeyPhase::Down) => InputDispatchKind::KeyDown,
        EventKind::Key(_, KeyPhase::Up) => InputDispatchKind::KeyUp,
        EventKind::Editing(_) => InputDispatchKind::Editing,
        EventKind::Text(_) => InputDispatchKind::Text,
    }
}

fn route_for(handlers: &Handlers, kind: &EventKind) -> Option<InputHandlerRoute> {
    match kind {
        EventKind::Pointer(..) => handlers.pointer_route.clone(),
        EventKind::Motion(_) => handlers.motion_route.clone(),
        EventKind::Wheel(_) => handlers.wheel_route.clone(),
        EventKind::Key(_, KeyPhase::Down) => handlers.key_route.clone(),
        EventKind::Key(_, KeyPhase::Up) => handlers.key_up_route.clone(),
        EventKind::Editing(_) => handlers.editing_route.clone(),
        EventKind::Text(_) => handlers.text_route.clone(),
    }
}

fn depth_under(node: &Node, root: &Node) -> Option<usize> {
    let mut depth = 0;
    let mut cursor = Some(node.clone());
    while let Some(current) = cursor {
        if &current == root {
            return Some(depth);
        }
        depth += 1;
        cursor = current.parent();
    }
    None
}

/// Build a root → leaf path for a given node by walking the parent chain.
fn path_from_root(node: &Node) -> Vec<Node> {
    let mut out: Vec<Node> = Vec::new();
    let mut cursor = Some(node.clone());
    while let Some(current) = cursor {
        cursor = current.parent();
        out.push(current);
    }
    out.reverse();
    out
}

fn common_prefix_length(lhs: &[Node], rhs: &[Node]) -> usize {
    let count = lhs.len().min(rhs.len());
    for index in 0..count {
        if lhs[index] != rhs[index] {
            return index;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rect;
    use crate::scene::PointerHandler;
    use cove_platform::MouseButton;
    use std::cell::Cell;

    fn dispatcher() -> (EventDispatcher, Rc<InteractionRegistry>, Node, Node) {
        let mut tree = NodeTree::new();
        let root = Node::new();
        root.set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
        root.set_is_hit_testable(true);
        let child = Node::new();
        child.set_frame(Rect::new(10.0, 10.0, 50.0, 50.0));
        child.set_is_hit_testable(true);
        root.add_child(&child);
        tree.set_root(Some(root.clone()));
        let interactions = Rc::new(InteractionRegistry::new());
        let dispatcher = EventDispatcher::new(
            Rc::new(RefCell::new(tree)),
            interactions.clone(),
            Rc::new(PointerCapture::new()),
            Rc::new(FocusChain::new()),
            WindowId::MAIN,
        );
        (dispatcher, interactions, root, child)
    }

    fn down(x: f32, y: f32) -> InputEvent {
        InputEvent::MouseButtonDown(MouseButtonEvent {
            button: MouseButton::Left,
            x,
            y,
            clicks: 1,
            ..MouseButtonEvent::default()
        })
    }

    #[test]
    fn delivery_runs_capture_then_target_then_bubble() {
        let (dispatcher, interactions, root, child) = dispatcher();
        let order = Rc::new(RefCell::new(Vec::new()));
        let record = {
            let order = order.clone();
            move |label: &'static str| {
                let order = order.clone();
                Rc::new(
                    move |_: &MouseButtonEvent, _: PointerPhase, _: EventPhase| {
                        order.borrow_mut().push(label);
                        EventResult::Ignored
                    },
                ) as PointerHandler
            }
        };
        interactions.set_pointer(&root, InputHandlerRoute::control(), record("root"));
        interactions.set_pointer(&child, InputHandlerRoute::control(), record("child"));
        dispatcher.dispatch(&down(20.0, 20.0));
        assert_eq!(*order.borrow(), vec!["root", "child", "root"]);
    }

    #[test]
    fn handled_target_stops_bubble() {
        let (dispatcher, interactions, root, child) = dispatcher();
        let root_calls = Rc::new(Cell::new(0));
        let counter = root_calls.clone();
        interactions.set_pointer(
            &root,
            InputHandlerRoute::control(),
            Rc::new(move |_, _, _| {
                counter.set(counter.get() + 1);
                EventResult::Ignored
            }),
        );
        interactions.set_pointer(
            &child,
            InputHandlerRoute::control(),
            Rc::new(|_, _, phase| {
                if phase == EventPhase::Target {
                    EventResult::Handled
                } else {
                    EventResult::Ignored
                }
            }),
        );
        dispatcher.dispatch(&down(20.0, 20.0));
        // Capture once, no bubble after the target consumed the event.
        assert_eq!(root_calls.get(), 1);
    }

    #[test]
    fn hover_path_emits_enter_and_leave() {
        let (dispatcher, interactions, _root, child) = dispatcher();
        let log = Rc::new(RefCell::new(Vec::new()));
        let logger = log.clone();
        interactions.set_hover(
            &child,
            Rc::new(move |phase| logger.borrow_mut().push(phase)),
        );
        let motion = |x: f32, y: f32| {
            InputEvent::MouseMotion(MouseMotionEvent {
                x,
                y,
                ..MouseMotionEvent::default()
            })
        };
        dispatcher.dispatch(&motion(20.0, 20.0));
        assert_eq!(*log.borrow(), vec![HoverPhase::Enter]);
        dispatcher.dispatch(&motion(150.0, 150.0));
        assert_eq!(*log.borrow(), vec![HoverPhase::Enter, HoverPhase::Leave]);
    }

    #[test]
    fn pointer_down_moves_focus_to_the_clicked_control() {
        let (dispatcher, _interactions, _root, child) = dispatcher();
        child.set_is_focusable(true);
        dispatcher.dispatch(&down(20.0, 20.0));
        assert_eq!(dispatcher.focus_chain.focused(), Some(child));
    }

    #[test]
    fn cursor_resolves_from_the_deepest_node() {
        let (dispatcher, _interactions, root, child) = dispatcher();
        root.set_cursor(Some(SystemCursor::Pointer));
        child.set_cursor(Some(SystemCursor::Ibeam));
        let seen = Rc::new(RefCell::new(Vec::new()));
        let log = seen.clone();
        let dispatcher = EventDispatcher {
            cursor_sink: Some(Rc::new(move |cursor| log.borrow_mut().push(cursor))),
            ..dispatcher
        };
        dispatcher.dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
            x: 20.0,
            y: 20.0,
            ..MouseMotionEvent::default()
        }));
        assert_eq!(*seen.borrow(), vec![SystemCursor::Ibeam]);
    }
}
