//! Translation of Primitives/Interaction/DragAndDrop.swift.
//!
//! Shares a typed drag payload between descendants of one drag scope. The payload
//! type is chosen by the caller (`Draggable<T>` / `DropTarget<T>`); the session
//! stores it type-erased so targets of different payload types can coexist. The
//! scope owns the session so payloads never escape into global state. A drag
//! preview is portal-rendered above clipping; dropping calls the target's typed
//! callback. Keyboard reordering is not implied: use a reorderable list when the
//! same collection needs an Up/Down path.
use crate::Scope;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::AnyView;
use crate::modifiers::interaction::HitTestingExt;
use crate::primitives::overlays::portal::PortalPlacementMode;
use crate::primitives::overlays::portal::portal_registry::{
    PortalResource, PortalStore, PortalStoreEnvironment, PortalStoreHolder,
};
use crate::{PrimitiveView, View, ViewExt, impl_primitive_view};
use cove_core::Point;
use cove_core::scene::{
    AnyAnimationController, AnimatorScheduler, CompositionLocal, EventResult, InputHandlerRoute,
    InteractionRegistryHolder, LayoutNode, Node, NodeResource, NodeWeak, PointerCaptureHolder,
    PointerPhase, SpringSolution,
};
use cove_platform::MouseButton;

use super::drag_threshold::crossed as drag_crossed;

struct DragTarget {
    node: Option<Node>,
    accepts: Rc<dyn Fn(&dyn Any) -> bool>,
    drop: Rc<dyn Fn(&dyn Any)>,
    on_targeted: Option<Rc<dyn Fn(bool)>>,
}

/// Scene-local drag state shared by descendants of one `DragScope`.
pub struct DragSession {
    payload: RefCell<Option<Rc<dyn Any>>>,
    preview: RefCell<Option<AnyView>>,
    pointer: Cell<Point>,
    targets: RefCell<Vec<(u64, DragTarget)>>,
    active: Cell<Option<u64>>,
    /// Bumped on every `begin`; lets the preview portal tell a fresh drag from
    /// a continuation without comparing view identities.
    generation: Cell<u64>,
    on_change: RefCell<Option<Rc<dyn Fn()>>>,
}

impl PartialEq for DragSession {
    /// Session identity, not state: the ambient value is the scope itself, and
    /// a live drag mutates it without re-providing the environment.
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl DragSession {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            payload: RefCell::new(None),
            preview: RefCell::new(None),
            pointer: Cell::new(Point::default()),
            targets: RefCell::new(Vec::new()),
            active: Cell::new(None),
            generation: Cell::new(0),
            on_change: RefCell::new(None),
        })
    }

    pub fn set_on_change(&self, handler: Rc<dyn Fn()>) {
        *self.on_change.borrow_mut() = Some(handler);
    }

    pub fn is_dragging(&self) -> bool {
        self.payload.borrow().is_some()
    }

    pub fn pointer(&self) -> Point {
        self.pointer.get()
    }

    pub fn preview(&self) -> Option<AnyView> {
        self.preview.borrow().clone()
    }

    pub fn generation(&self) -> u64 {
        self.generation.get()
    }

    pub fn register(
        &self,
        node: &Node,
        accepts: Rc<dyn Fn(&dyn Any) -> bool>,
        drop: Rc<dyn Fn(&dyn Any)>,
        on_targeted: Option<Rc<dyn Fn(bool)>>,
    ) {
        let key = node.id().0;
        let mut targets = self.targets.borrow_mut();
        targets.retain(|(existing, _)| *existing != key);
        targets.push((
            key,
            DragTarget {
                node: Some(node.clone()),
                accepts,
                drop,
                on_targeted,
            },
        ));
    }

    pub fn begin<T: Any>(&self, payload: T, preview: AnyView, pointer: Point) {
        *self.payload.borrow_mut() = Some(Rc::new(payload));
        *self.preview.borrow_mut() = Some(preview);
        self.pointer.set(pointer);
        self.active.set(None);
        self.generation.set(self.generation.get().wrapping_add(1));
        self.notify();
    }

    pub fn move_to(&self, point: Point) {
        self.pointer.set(point);
        self.refresh_target();
        self.notify();
    }

    pub fn end(&self) {
        let Some(payload) = self.payload.borrow().clone() else {
            return;
        };
        self.refresh_target();
        if let Some(active) = self.active.get()
            && let Some(target) = self.targets.borrow().iter().find(|(key, _)| *key == active)
        {
            (target.1.drop)(payload.as_ref());
        }
        self.cancel();
    }

    pub fn cancel(&self) {
        if let Some(active) = self.active.get()
            && let Some(target) = self.targets.borrow().iter().find(|(key, _)| *key == active)
            && let Some(on_targeted) = target.1.on_targeted.as_ref()
        {
            on_targeted(false);
        }
        self.active.set(None);
        *self.payload.borrow_mut() = None;
        *self.preview.borrow_mut() = None;
        self.pointer.set(Point::default());
        self.notify();
    }

    fn notify(&self) {
        if let Some(on_change) = self.on_change.borrow().as_ref() {
            on_change();
        }
    }

    fn refresh_target(&self) {
        {
            let mut targets = self.targets.borrow_mut();
            targets.retain(|(_, target)| {
                target
                    .node
                    .as_ref()
                    .is_some_and(|node| node.parent().is_some())
            });
        }
        let next = self.target_at(self.pointer.get());
        let next_key = next.map(|node| node.id().0);
        if next_key == self.active.get() {
            return;
        }
        if let Some(active) = self.active.get()
            && let Some(target) = self.targets.borrow().iter().find(|(key, _)| *key == active)
            && let Some(on_targeted) = target.1.on_targeted.as_ref()
        {
            on_targeted(false);
        }
        self.active.set(next_key);
        if let Some(key) = next_key
            && let Some(target) = self
                .targets
                .borrow()
                .iter()
                .find(|(existing, _)| *existing == key)
            && let Some(on_targeted) = target.1.on_targeted.as_ref()
        {
            on_targeted(true);
        }
    }

    fn target_at(&self, point: Point) -> Option<Node> {
        let payload = self.payload.borrow().clone()?;
        let mut best: Option<(Node, usize)> = None;
        let targets = self.targets.borrow();
        for (_, target) in targets.iter() {
            let Some(node) = target.node.as_ref() else {
                continue;
            };
            if !(target.accepts)(payload.as_ref()) {
                continue;
            }
            if !node.is_layout_visible() || !node.is_interaction_enabled() {
                continue;
            }
            if !contains(node.absolute_frame(), point) {
                continue;
            }
            let depth = depth_of(node);
            if best
                .as_ref()
                .is_none_or(|(_, best_depth)| depth > *best_depth)
            {
                best = Some((node.clone(), depth));
            }
        }
        best.map(|(node, _)| node)
    }
}

fn contains(rect: cove_core::Rect, point: Point) -> bool {
    point.x >= rect.x
        && point.x <= rect.x + rect.width
        && point.y >= rect.y
        && point.y <= rect.y + rect.height
}

fn depth_of(node: &Node) -> usize {
    let mut depth = 0;
    let mut current = node.parent();
    while let Some(parent) = current {
        depth += 1;
        current = parent.parent();
    }
    depth
}

/// Environment that carries the enclosing drag session.
pub struct DragScopeEnvironment;
impl DragScopeEnvironment {
    pub fn key() -> Rc<CompositionLocal<Option<Rc<DragSession>>>> {
        thread_local! {
            static KEY: Rc<CompositionLocal<Option<Rc<DragSession>>>> =
                Rc::new(CompositionLocal::constant(None));
        }
        KEY.with(Rc::clone)
    }
}

/// Shares a typed drag payload between its descendants.
pub struct DragScope {
    /// An explicit session for callers that need to share one across scopes or
    /// drive it directly. `None` keeps the session in the view's retained slot.
    session: Option<Rc<DragSession>>,
    revision: cove_core::State<u64>,
    content: AnyView,
}

impl DragScope {
    pub fn new(content: impl View + 'static) -> Self {
        Self {
            session: None,
            revision: cove_core::State::new(0_u64),
            content: AnyView::new(content),
        }
    }

    /// Use a caller-owned session instead of the one retained by this view.
    /// Useful when the scope is rebuilt from outside a composition scope or
    /// when several scopes must share drag state.
    pub fn with_session(session: Rc<DragSession>, content: impl View + 'static) -> Self {
        Self {
            session: Some(session),
            revision: cove_core::State::new(0_u64),
            content: AnyView::new(content),
        }
    }
}

impl View for DragScope {
    fn body(&self, cx: &Scope) -> AnyView {
        // The caller may reconstruct `DragScope` on every body evaluation. The
        // session must survive that, or a reconfigure during a drag would hand
        // the pointer-up to a fresh, empty session. `Scope::remember` retains
        // one handle for this view identity.
        let session = self
            .session
            .clone()
            .unwrap_or_else(|| cx.remember(DragSession::new));
        let revision = self.revision.clone();
        let revision_for_change = revision.clone();
        session.set_on_change(Rc::new(move || {
            revision_for_change.set(revision_for_change.get().wrapping_add(1))
        }));
        let revision = revision.get();
        AnyView::new(
            crate::BoxView::new(
                cove_core::scene::FlexDirection::Column,
                cove_core::scene::Align::Stretch,
                cove_core::scene::Justify::FlexStart,
                cove_core::scene::Wrap::NoWrap,
                0.0,
                AnyView::new(crate::TupleView::new(vec![
                    self.content.clone(),
                    AnyView::new(DragPreviewPortal {
                        session: session.clone(),
                        revision,
                    }),
                ])),
            )
            .composition_local(DragScopeEnvironment::key(), Some(session)),
        )
    }

    /// `body` reads `revision` from `State`, so the
    /// scope must declare those dependencies. Without this the writes never
    /// subscribe and the view keeps its first layout forever.
    fn dynamic_properties(&self) -> Vec<crate::DynamicField> {
        vec![crate::DynamicField::state("revision", &self.revision)]
    }
}

/// A view that can start a drag with a typed payload and preview.
pub struct Draggable<T: Clone + 'static> {
    pub payload: T,
    preview: AnyView,
    content: AnyView,
}

impl<T: Clone + 'static> Draggable<T> {
    pub fn new(payload: T, preview: impl View + 'static, content: impl View + 'static) -> Self {
        Self {
            payload,
            preview: AnyView::new(preview),
            content: AnyView::new(content),
        }
    }
}

impl<T: Clone + 'static> View for Draggable<T> {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(DragSourceHost {
            payload: self.payload.clone(),
            preview: self.preview.clone(),
            content: self.content.clone(),
        })
    }
}

struct DragSourceHost<T: Clone + 'static> {
    payload: T,
    preview: AnyView,
    content: AnyView,
}

const DRAG_START: &str = "drag.start";
const DRAG_ACTIVE: &str = "drag.active";

impl<T: Clone + 'static> PrimitiveView for DragSourceHost<T> {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_align_items(cove_core::scene::Align::Stretch);
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        let Some(session) = node.composition_value(&DragScopeEnvironment::key()) else {
            return;
        };
        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        let payload = self.payload.clone();
        let preview = self.preview.clone();
        let pointer_session = session.clone();
        let pointer_node = node.downgrade();
        registry.set_pointer(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase, _event_phase| {
                if event.button != MouseButton::Left {
                    return EventResult::Ignored;
                }
                let Some(owner) = Node::upgrade(&pointer_node) else {
                    return EventResult::Ignored;
                };
                match phase {
                    PointerPhase::Down => {
                        owner.set_attachment(DRAG_START, Point::new(event.x, event.y));
                        owner.set_attachment(DRAG_ACTIVE, false);
                        // Capture immediately but keep the gesture transferable:
                        // motion reaches the source even when the first event is
                        // already outside it, while a scroll container may still
                        // claim the gesture before the drag threshold. Crossing
                        // the threshold upgrades the capture to exclusive.
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.acquire(&owner, true);
                        }
                        EventResult::Ignored
                    }
                    PointerPhase::Up => {
                        let active = owner.attachment::<bool>(DRAG_ACTIVE) == Some(true);
                        owner.remove_attachment(DRAG_START);
                        owner.set_attachment(DRAG_ACTIVE, false);
                        if !active {
                            if PointerCaptureHolder::current()
                                .and_then(|capture| capture.target())
                                .is_some_and(|target| target == owner)
                                && let Some(capture) = PointerCaptureHolder::current()
                            {
                                capture.release();
                            }
                            return EventResult::Ignored;
                        }
                        // Finish the drop before releasing capture: releasing
                        // invokes the lost-capture callback, which cancels any
                        // live session.
                        pointer_session.end();
                        if PointerCaptureHolder::current()
                            .and_then(|capture| capture.target())
                            .is_some_and(|target| target == owner)
                            && let Some(capture) = PointerCaptureHolder::current()
                        {
                            capture.release();
                        }
                        EventResult::Handled
                    }
                }
            }),
        );
        let motion_session = session.clone();
        let motion_node = node.downgrade();
        registry.set_motion(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, _phase| {
                let Some(owner) = Node::upgrade(&motion_node) else {
                    return EventResult::Ignored;
                };
                let Some(start) = owner.attachment::<Point>(DRAG_START) else {
                    return EventResult::Ignored;
                };
                let point = Point::new(event.x, event.y);
                if owner.attachment::<bool>(DRAG_ACTIVE) != Some(true) {
                    let dx = point.x - start.x;
                    let dy = point.y - start.y;
                    if !drag_crossed(dx, dy) {
                        return EventResult::Ignored;
                    }
                    owner.set_attachment(DRAG_ACTIVE, true);
                    if let Some(capture) = PointerCaptureHolder::current() {
                        // Exclusive: a scroll gesture must not take the drag
                        // away once the threshold has been crossed.
                        capture.acquire(&owner, false);
                    }
                    motion_session.begin(payload.clone(), preview.clone(), point);
                }
                motion_session.move_to(point);
                EventResult::Handled
            }),
        );

        let clear_node = node.downgrade();
        let clear_session = session.clone();
        node.set_attachment(
            cove_core::scene::PointerCaptureAttachmentKey::DID_LOSE_CAPTURE,
            Rc::new(move || {
                if let Some(owner) = Node::upgrade(&clear_node) {
                    owner.remove_attachment(DRAG_START);
                    owner.set_attachment(DRAG_ACTIVE, false);
                }
                if clear_session.is_dragging() {
                    clear_session.cancel();
                }
            }) as Rc<dyn Fn()>,
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl<T: Clone + 'static> View for DragSourceHost<T> {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// A drop area for one payload type.
pub struct DropTarget<T: Clone + 'static> {
    accepts: Rc<dyn Fn(&T) -> bool>,
    on_drop: Rc<dyn Fn(T)>,
    on_targeted: Option<Rc<dyn Fn(bool)>>,
    content: AnyView,
}

impl<T: Clone + 'static> DropTarget<T> {
    pub fn new(
        accepts: impl Fn(&T) -> bool + 'static,
        on_drop: impl Fn(T) + 'static,
        content: impl View + 'static,
    ) -> Self {
        Self {
            accepts: Rc::new(accepts),
            on_drop: Rc::new(on_drop),
            on_targeted: None,
            content: AnyView::new(content),
        }
    }

    pub fn on_targeted(mut self, on_targeted: impl Fn(bool) + 'static) -> Self {
        self.on_targeted = Some(Rc::new(on_targeted));
        self
    }
}

impl<T: Clone + 'static> View for DropTarget<T> {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(DropTargetHost {
            accepts: self.accepts.clone(),
            on_drop: self.on_drop.clone(),
            on_targeted: self.on_targeted.clone(),
            content: self.content.clone(),
        })
    }
}

struct DropTargetHost<T: Clone + 'static> {
    accepts: Rc<dyn Fn(&T) -> bool>,
    on_drop: Rc<dyn Fn(T)>,
    on_targeted: Option<Rc<dyn Fn(bool)>>,
    content: AnyView,
}

impl<T: Clone + 'static> PrimitiveView for DropTargetHost<T> {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_align_items(cove_core::scene::Align::Stretch);
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        let Some(session) = node.composition_value(&DragScopeEnvironment::key()) else {
            return;
        };
        let accepts = self.accepts.clone();
        let erased_accepts: Rc<dyn Fn(&dyn Any) -> bool> = Rc::new(move |payload: &dyn Any| {
            payload
                .downcast_ref::<T>()
                .is_some_and(|value| accepts(value))
        });
        let on_drop = self.on_drop.clone();
        let erased_drop: Rc<dyn Fn(&dyn Any)> = Rc::new(move |payload: &dyn Any| {
            if let Some(value) = payload.downcast_ref::<T>() {
                on_drop(value.clone());
            }
        });
        session.register(node, erased_accepts, erased_drop, self.on_targeted.clone());
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl<T: Clone + 'static> View for DropTargetHost<T> {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

struct DragPreviewPortal {
    session: Rc<DragSession>,
    revision: u64,
}

impl PrimitiveView for DragPreviewPortal {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        let resource: Rc<dyn NodeResource> = PortalResource::new();
        node.add_resource(resource);
        let follower: Rc<dyn NodeResource> = DragPreviewFollower::new();
        node.add_resource(follower);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        let Some(resource) = node.first_resource::<PortalResource>() else {
            return;
        };
        let Some(follower) = node.first_resource::<DragPreviewFollower>() else {
            return;
        };
        let preview = if self.session.is_dragging() {
            self.session.preview()
        } else {
            None
        };
        let Some(preview) = preview else {
            follower.exit(node, &resource);
            return;
        };
        let store = Some(
            node.composition_value(&PortalStoreEnvironment::key())
                .unwrap_or_else(PortalStoreHolder::current),
        );
        follower.drive(
            node,
            &resource,
            store,
            AnyView::new(preview.allows_hit_testing(false)),
            self.session.pointer(),
            self.session.generation(),
        );
        let _ = self.revision;
    }
}
impl_primitive_view!(DragPreviewPortal);

/// Gap between the pointer and the preview's origin.
const PREVIEW_LEAD: f32 = 12.0;

/// Follow spring: quick enough to feel attached, soft enough to read as glass.
const PREVIEW_RESPONSE: f32 = 0.16;

/// Eased pointer-follow and fade presentation for one `DragScope`'s preview.
/// The portal entry is presented once per drag with a fade-in, moved through
/// `PortalStore::update_position` on spring ticks (no recompose), and dismissed
/// with a fade-out. Reduced motion pins the preview to the pointer and skips
/// the fades.
pub struct DragPreviewFollower {
    state: Rc<FollowerState>,
    self_ref: RefCell<std::rc::Weak<Self>>,
}

struct FollowerState {
    owner: RefCell<Option<NodeWeak>>,
    scheduler: RefCell<Option<AnimatorScheduler>>,
    controller: RefCell<Option<Rc<dyn AnyAnimationController>>>,
    presented_generation: Cell<u64>,
    is_presented: Cell<bool>,
    current: Cell<Point>,
    release: Cell<Point>,
    release_velocity: Cell<Point>,
    target: Cell<Point>,
    elapsed: Cell<f64>,
    reduced: Cell<bool>,
}

struct FollowerController {
    follower: std::rc::Weak<DragPreviewFollower>,
}

impl AnyAnimationController for FollowerController {
    fn is_finished(&self) -> bool {
        self.follower.upgrade().is_none_or(|f| f.is_finished())
    }

    fn tick(&self, delta_time: f64) {
        if let Some(follower) = self.follower.upgrade() {
            follower.tick(delta_time);
        }
    }

    fn finish_immediately(&self) {
        if let Some(follower) = self.follower.upgrade() {
            follower.finish_immediately();
        }
    }

    fn cancel(&self) {
        if let Some(follower) = self.follower.upgrade() {
            follower.cancel();
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl DragPreviewFollower {
    fn new() -> Rc<Self> {
        let follower = Rc::new(Self {
            state: Rc::new(FollowerState {
                owner: RefCell::new(None),
                scheduler: RefCell::new(None),
                controller: RefCell::new(None),
                presented_generation: Cell::new(0),
                is_presented: Cell::new(false),
                current: Cell::new(Point::default()),
                release: Cell::new(Point::default()),
                release_velocity: Cell::new(Point::default()),
                target: Cell::new(Point::default()),
                elapsed: Cell::new(0.0),
                reduced: Cell::new(false),
            }),
            self_ref: RefCell::new(std::rc::Weak::new()),
        });
        *follower.self_ref.borrow_mut() = Rc::downgrade(&follower);
        follower
    }

    fn weak(&self) -> std::rc::Weak<Self> {
        self.self_ref.borrow().clone()
    }

    fn is_finished(&self) -> bool {
        self.state.controller.borrow().is_none()
    }

    /// Presents a fresh drag or re-aims the follow spring at the pointer.
    fn drive(
        self: &Rc<Self>,
        node: &Node,
        resource: &Rc<PortalResource>,
        store: Option<Rc<PortalStore>>,
        preview: AnyView,
        pointer: Point,
        generation: u64,
    ) {
        let position = Point::new(pointer.x + PREVIEW_LEAD, pointer.y + PREVIEW_LEAD);
        let reduced = node.prefers_reduced_motion();
        self.state.reduced.set(reduced);
        if self.state.presented_generation.get() != generation {
            // A dismissal still fading out is re-presented in place: the store
            // swaps the exiting entry back to live content.
            self.state.presented_generation.set(generation);
            self.state.is_presented.set(true);
            self.state.current.set(position);
            self.state.release.set(position);
            self.state.release_velocity.set(Point::default());
            self.state.target.set(position);
            self.state.elapsed.set(0.0);
            let transition = if reduced {
                None
            } else {
                Some(crate::animation::Transition::opacity())
            };
            resource.present(
                store,
                position,
                None,
                preview,
                false,
                PortalPlacementMode::Fixed,
                None,
                transition,
            );
            return;
        }
        if !self.state.is_presented.get() {
            return;
        }
        if reduced {
            self.cancel();
            self.state.current.set(position);
            self.state.target.set(position);
            resource.update_position(position);
            return;
        }
        // Rebase the analytic spring from the live position, carrying its
        // velocity so a retarget mid-flight does not visibly stutter.
        let spring = SpringSolution::new(PREVIEW_RESPONSE, 1.0);
        let elapsed = self.state.elapsed.get() as f32;
        let current = self.state.current.get();
        let release = self.state.release.get();
        let velocity = self.state.release_velocity.get();
        let target = self.state.target.get();
        self.state.release.set(current);
        self.state.release_velocity.set(Point::new(
            spring.velocity(elapsed, release.x, velocity.x, target.x),
            spring.velocity(elapsed, release.y, velocity.y, target.y),
        ));
        self.state.target.set(position);
        self.state.elapsed.set(0.0);
        self.register();
    }

    /// Fades the preview out where it stands. A new drag within the fade
    /// re-presents the same entry, so the exit is interrupted rather than
    /// queued.
    fn exit(self: &Rc<Self>, node: &Node, resource: &Rc<PortalResource>) {
        if self.state.is_presented.get() {
            self.state.is_presented.set(false);
            self.cancel();
            if self.state.reduced.get() {
                resource.unmount(node);
            } else {
                resource.dismiss(node);
            }
        } else if resource.entry_id().is_none() {
            NodeResource::unmount(&**resource, node);
        }
    }

    fn register(&self) {
        if self.state.controller.borrow().is_some() {
            return;
        }
        let controller: Rc<dyn AnyAnimationController> = Rc::new(FollowerController {
            follower: self.weak(),
        });
        *self.state.controller.borrow_mut() = Some(controller.clone());
        if let Some(scheduler) = self.state.scheduler.borrow().as_ref() {
            scheduler.register(controller);
        }
    }

    fn tick(&self, delta_time: f64) {
        let state = &self.state;
        if state.controller.borrow().is_none() || !delta_time.is_finite() || delta_time < 0.0 {
            return;
        }
        state.elapsed.set(state.elapsed.get() + delta_time);
        let spring = SpringSolution::new(PREVIEW_RESPONSE, 1.0);
        if state.elapsed.get() >= spring.settling_duration() {
            self.finish_at(state.target.get());
            return;
        }
        let t = state.elapsed.get() as f32;
        let release = state.release.get();
        let velocity = state.release_velocity.get();
        let target = state.target.get();
        let next = Point::new(
            spring.position(t, release.x, velocity.x, target.x),
            spring.position(t, release.y, velocity.y, target.y),
        );
        state.current.set(next);
        self.apply(next);
    }

    fn finish_at(&self, position: Point) {
        self.state.current.set(position);
        self.apply(position);
        self.cancel();
    }

    fn finish_immediately(&self) {
        let target = self.state.target.get();
        self.finish_at(target);
    }

    fn cancel(&self) {
        if let (Some(scheduler), Some(controller)) = (
            self.state.scheduler.borrow().as_ref(),
            self.state.controller.borrow().as_ref(),
        ) {
            scheduler.unregister(controller);
        }
        self.state.controller.borrow_mut().take();
    }

    fn apply(&self, position: Point) {
        let Some(owner) = self
            .state
            .owner
            .borrow()
            .as_ref()
            .and_then(Node::upgrade)
        else {
            return;
        };
        let Some(resource) = owner.first_resource::<PortalResource>() else {
            return;
        };
        resource.update_position(position);
    }
}

impl NodeResource for DragPreviewFollower {
    fn mount(&self, node: &Node) {
        *self.state.owner.borrow_mut() = Some(node.downgrade());
        *self.state.scheduler.borrow_mut() = Some(AnimatorScheduler::current());
    }

    fn unmount(&self, _node: &Node) {
        self.cancel();
        self.state.is_presented.set(false);
        *self.state.owner.borrow_mut() = None;
        *self.state.scheduler.borrow_mut() = None;
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}
