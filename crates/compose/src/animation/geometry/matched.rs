//! Translation of Animation/Geometry/Matched.swift.
//!
//! Move a surface between layout destinations with the same scene-local ID. Its
//! frame animates while layout reserves the destination immediately.
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use crate::theme::{SemanticMotionRef, theme_for_node};
use crate::{AnyView, PrimitiveView, View};
use cove_core::Rect;
use cove_core::scene::{
    ActiveAnimationContext, Animation, AnimationCompletionGroup, AnimationController,
    AnimatorScheduler, AnyAnimationController, FlexDirection, LayoutNode, LayoutUpdateHook, Node,
    NodeResource,
};

const MATCHED_FRAME: &str = "motion.matchedFrame";

#[derive(Clone, Copy, PartialEq)]
struct MatchedEntry {
    node_id: u64,
    /// Draw frame, animated from the previous destination.
    frame: Rect,
    /// Layout destination reserved this pass.
    target: Rect,
    generation: u64,
}

/// Owned by the scene root, with no global registration or public namespace.
pub struct GeometrySession {
    entries: RefCell<HashMap<u64, MatchedEntry>>,
    generation: Cell<u64>,
    self_ref: RefCell<Weak<Self>>,
}

impl GeometrySession {
    fn new() -> Rc<Self> {
        let session = Rc::new(Self {
            entries: RefCell::new(HashMap::new()),
            generation: Cell::new(0),
            self_ref: RefCell::new(Weak::new()),
        });
        *session.self_ref.borrow_mut() = Rc::downgrade(&session);
        session
    }

    pub fn generation(&self) -> u64 {
        self.generation.get()
    }

    fn entry(&self, id: u64) -> Option<MatchedEntry> {
        self.entries.borrow().get(&id).copied()
    }

    fn insert(&self, id: u64, entry: MatchedEntry) {
        self.entries.borrow_mut().insert(id, entry);
    }

    fn remove_node(&self, id: u64, node_id: u64) {
        let mut entries = self.entries.borrow_mut();
        if let Some(entry) = entries.get_mut(&id)
            && entry.node_id == node_id
        {
            entry.node_id = 0;
            entry.generation = self.generation.get();
        }
        let _ = entries;
    }

    fn complete_layout(&self) {
        self.generation.set(self.generation.get().wrapping_add(1));
        let generation = self.generation.get();
        let mut entries = self.entries.borrow_mut();
        entries.retain(|_, entry| {
            entry.node_id != 0 || generation.wrapping_sub(entry.generation) <= 1
        });
    }
}

impl NodeResource for GeometrySession {
    fn mount(&self, node: &Node) {
        let previous = node.layout_did_update();
        let weak_session = self.session_weak();
        node.set_layout_did_update(Some(Rc::new(move |owner: &Node| {
            if let Some(previous) = previous.clone() {
                previous(owner);
            }
            if let Some(session) = weak_session.upgrade() {
                session.complete_layout();
            }
        }) as LayoutUpdateHook));
    }

    fn unmount(&self, _node: &Node) {
        self.entries.borrow_mut().clear();
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

impl GeometrySession {
    fn session_weak(&self) -> Weak<Self> {
        self.self_ref.borrow().clone()
    }
}

/// Animated surface frame for one matched ID.
pub struct MatchedSurface {
    node: RefCell<Option<Node>>,
    session: RefCell<Option<Rc<GeometrySession>>>,
    id: Cell<Option<u64>>,
    animation: RefCell<Option<Animation>>,
    layout_frame: RefCell<Option<Rect>>,
    pending: RefCell<Option<Rc<AnimationController<f32>>>>,
    completion: RefCell<Option<Rc<AnimationCompletionGroup>>>,
    scheduler: RefCell<Option<AnimatorScheduler>>,
    suppressed: Cell<bool>,
    surface: RefCell<Option<Node>>,
    self_ref: RefCell<Weak<Self>>,
}

impl MatchedSurface {
    fn new() -> Rc<Self> {
        let surface = Rc::new(Self {
            node: RefCell::new(None),
            session: RefCell::new(None),
            id: Cell::new(None),
            animation: RefCell::new(None),
            layout_frame: RefCell::new(None),
            pending: RefCell::new(None),
            completion: RefCell::new(None),
            scheduler: RefCell::new(None),
            suppressed: Cell::new(false),
            surface: RefCell::new(None),
            self_ref: RefCell::new(Weak::new()),
        });
        *surface.self_ref.borrow_mut() = Rc::downgrade(&surface);
        surface
    }

    fn configure(&self, id: u64, animation: Animation) {
        if self.id.get() != Some(id) {
            *self.layout_frame.borrow_mut() = None;
        }
        self.id.set(Some(id));
        *self.animation.borrow_mut() = Some(animation.clone());
        let context = ActiveAnimationContext::current();
        self.suppressed.set(context.disables_animations);
        if let (Some(node), Some(surface)) =
            (self.node.borrow().clone(), self.surface.borrow().clone())
            && (node.prefers_reduced_motion() || self.suppressed.get())
            && let Some(session) = self.session.borrow().clone()
            && let Some(entry) = session.entry(id)
        {
            let entry_cell = Rc::new(Cell::new(entry));
            let weak_surface = surface.downgrade();
            let drawn = entry_cell.clone();
            surface.animatable_set_with(MATCHED_FRAME, entry.frame, entry.target, move |rect| {
                drawn.set(MatchedEntry {
                    frame: rect,
                    ..drawn.get()
                });
                if let Some(owner) = Node::upgrade(&weak_surface) {
                    place(&owner, rect);
                }
            });
        }
        if let Some(group) = context.completion
            && !self
                .completion
                .borrow()
                .as_ref()
                .is_some_and(|current| Rc::ptr_eq(current, &group))
        {
            if let Some(pending) = self.pending.borrow_mut().take()
                && let Some(scheduler) = self.scheduler.borrow().as_ref()
            {
                let any: Rc<dyn AnyAnimationController> = pending.clone();
                scheduler.unregister(&any);
            }
            *self.completion.borrow_mut() = Some(group.clone());
            // Native hosts tick before layout. Keep this completion open until
            // layout can enroll the real frame controller, even on that order.
            let reservation = AnimationController::new(
                0.0_f32,
                1.0_f32,
                Animation::new(f64::MAX, cove_core::scene::AnimationCurve::Linear),
                0.0,
                |_| {},
            );
            group.attach(&reservation);
            if let Some(scheduler) = self.scheduler.borrow().as_ref() {
                scheduler.register(reservation.clone());
            }
            *self.pending.borrow_mut() = Some(reservation);
        }
    }

    fn complete_registration(&self) {
        if let Some(pending) = self.pending.borrow_mut().take() {
            let any: Rc<dyn AnyAnimationController> = pending.clone();
            if let Some(scheduler) = self.scheduler.borrow().as_ref() {
                scheduler.unregister(&any);
            }
            pending.finish_immediately();
        }
        *self.completion.borrow_mut() = None;
    }

    fn layout(&self, owner: &Node) {
        let _guard = RegistrationGuard { surface: self };
        let Some(id) = self.id.get() else { return };
        let Some(node) = find_surface(owner) else {
            return;
        };
        *self.surface.borrow_mut() = Some(node.clone());
        let root = {
            let mut root = node.clone();
            while let Some(parent) = root.parent() {
                root = parent;
            }
            root
        };
        let session = match root.first_resource::<GeometrySession>() {
            Some(session) => session,
            None => {
                let session = GeometrySession::new();
                let resource: Rc<dyn NodeResource> = session.clone();
                root.add_resource(resource);
                session
            }
        };
        *self.session.borrow_mut() = Some(session.clone());
        let target = node.absolute_frame();
        let local = node.frame();
        let Some(mut entry) = session.entry(id) else {
            session.insert(
                id,
                MatchedEntry {
                    node_id: owner.id().0,
                    frame: target,
                    target,
                    generation: session.generation(),
                },
            );
            *self.layout_frame.borrow_mut() = Some(local);
            return;
        };
        assert!(
            entry.node_id == 0 || entry.node_id == owner.id().0,
            "Matched geometry IDs must be unique within a scene"
        );
        let changed = entry.node_id != owner.id().0
            || entry.target != target
            || *self.layout_frame.borrow() != Some(local);
        entry.node_id = owner.id().0;
        entry.generation = session.generation();
        *self.layout_frame.borrow_mut() = Some(local);
        let animation = self
            .animation
            .borrow()
            .clone()
            .unwrap_or_else(|| SemanticMotionRef::smooth().resolve(&theme_for_node(&node)));
        if node.prefers_reduced_motion() || self.suppressed.get() || animation.duration == 0.0 {
            node.cancel_animation(MATCHED_FRAME);
            entry.target = target;
            entry.frame = target;
            session.insert(id, entry);
            return;
        }
        if changed {
            entry.target = target;
            session.insert(id, entry);
            let completion = self.completion.borrow().clone();
            let drawn = Rc::new(Cell::new(entry));
            let weak_node = node.downgrade();
            let drawn_inner = drawn.clone();
            ActiveAnimationContext::with(Some(animation), completion, None, || {
                node.animatable_set_with(MATCHED_FRAME, entry.frame, target, move |rect| {
                    drawn_inner.set(MatchedEntry {
                        frame: rect,
                        ..drawn_inner.get()
                    });
                    if let Some(owner) = Node::upgrade(&weak_node) {
                        place(&owner, rect);
                    }
                });
            });
            let mut stored = entry;
            stored.frame = drawn.get().frame;
            session.insert(id, stored);
        } else {
            place(&node, entry.frame);
        }
    }
}

struct RegistrationGuard<'a> {
    surface: &'a MatchedSurface,
}

impl Drop for RegistrationGuard<'_> {
    fn drop(&mut self) {
        self.surface.complete_registration();
    }
}

fn find_surface(node: &Node) -> Option<Node> {
    if node.layout_node().is_some() {
        return Some(node.clone());
    }
    for child in node.children() {
        if let Some(found) = find_surface(&child) {
            return Some(found);
        }
    }
    None
}

fn place(node: &Node, rect: Rect) {
    let Some(parent) = node.parent() else { return };
    let point = parent.convert_from_window(cove_core::Point::new(rect.x, rect.y));
    let parent_frame = parent.frame();
    let scale = if parent_frame.width > 0.0 {
        parent.absolute_frame().width / parent_frame.width
    } else {
        1.0
    };
    if scale <= 0.0 {
        return;
    }
    let offset = parent.content_offset();
    node.set_frame(Rect::new(
        point.x + offset.x,
        point.y + offset.y,
        rect.width / scale,
        rect.height / scale,
    ));
}

impl NodeResource for MatchedSurface {
    fn mount(&self, node: &Node) {
        *self.node.borrow_mut() = Some(node.clone());
        *self.scheduler.borrow_mut() = Some(AnimatorScheduler::current());
        let weak_surface = self.session_weak();
        node.set_layout_did_update(Some(Rc::new(move |owner: &Node| {
            if let Some(surface) = weak_surface.upgrade() {
                surface.layout(owner);
            }
        }) as LayoutUpdateHook));
    }

    fn unmount(&self, node: &Node) {
        if let Some(pending) = self.pending.borrow_mut().take() {
            let any: Rc<dyn AnyAnimationController> = pending.clone();
            if let Some(scheduler) = self.scheduler.borrow().as_ref() {
                scheduler.unregister(&any);
            }
            pending.cancel();
        }
        *self.completion.borrow_mut() = None;
        if let (Some(id), Some(session)) = (self.id.get(), self.session.borrow().clone()) {
            session.remove_node(id, node.id().0);
        }
        *self.node.borrow_mut() = None;
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

impl MatchedSurface {
    fn session_weak(&self) -> Weak<Self> {
        self.self_ref.borrow().clone()
    }
}

/// Animates a surface between layout destinations that share an ID.
pub struct Matched<Content: View> {
    pub id: u64,
    pub animation: Option<Animation>,
    content: Content,
}

impl<Content: View + Clone> Matched<Content> {
    pub fn new(id: impl std::hash::Hash, content: Content) -> Self {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        id.hash(&mut hasher);
        Self {
            id: hasher.finish(),
            animation: None,
            content,
        }
    }

    pub fn animation(mut self, animation: Animation) -> Self {
        self.animation = Some(animation);
        self
    }
}

impl<Content: View + Clone + 'static> PrimitiveView for Matched<Content> {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        let surface: Rc<dyn NodeResource> = MatchedSurface::new();
        node.add_resource(surface);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(cove_core::scene::Align::Stretch);
    }

    fn update_node(&self, node: &Node) {
        let Some(surface) = node.first_resource::<MatchedSurface>() else {
            return;
        };
        let animation = self.animation.clone().unwrap_or_else(|| {
            ActiveAnimationContext::current()
                .animation
                .unwrap_or_else(|| SemanticMotionRef::smooth().resolve(&theme_for_node(node)))
        });
        surface.configure(self.id, animation);
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![AnyView::new(self.content.clone())]
    }
}
impl<Content: View + Clone + 'static> View for Matched<Content> {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
}
