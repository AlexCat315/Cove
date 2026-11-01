//! Translation of Theme/Styles/CircularAppearance.swift.
//!
//! A whole-window reveal from the control that changes appearance. Keyboard
//! and accessibility activation use the same control center as pointer input;
//! the centre is recorded by `WindowRevealContext::record_activation` in the
//! button host, so this file only consumes it.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use cove_core::scene::{
    Animation, AnimationController, AnimationCurve, AnimatorScheduler, AnyAnimationController,
    LayoutNode, Node, NodeResource, NodeWeak, WindowRevealContext,
};
use cove_core::{Point, Reveal};

use crate::theme::{Theme, ThemeViewExt};
use crate::{AnyView, PrimitiveView, View, ViewKind};

/// Appearance-swap transition. `Instant` swaps in the new palette on the next
/// frame; `Circular` reveals the window through an expanding disc centred on
/// the control that requested the change.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ThemeTransition {
    #[default]
    Instant,
    /// `duration` is in seconds and must be finite and positive to animate.
    Circular { duration: f64 },
}
impl ThemeTransition {
    /// Swift's `ThemeTransition.circular(duration: 1.0)` default argument.
    pub fn circular() -> Self {
        Self::Circular { duration: 1.0 }
    }
}

/// Circular reveal host. The host owns no layout node; the wrapped content is
/// the only child, so the reveal boundary never inserts an extra container
/// into the tree.
pub struct CircularThemeHost<Value: Clone + PartialEq + 'static> {
    pub theme: Theme,
    pub value: Value,
    pub transition: ThemeTransition,
    pub content: AnyView,
}
impl<Value: Clone + PartialEq + 'static> CircularThemeHost<Value> {
    pub fn new(theme: Theme, value: Value, transition: ThemeTransition, content: AnyView) -> Self {
        Self {
            theme,
            value,
            transition,
            content,
        }
    }
}

impl<Value: Clone + PartialEq + 'static> PrimitiveView for CircularThemeHost<Value> {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.add_resource(Rc::new(ThemeRevealSession::<Value>::new()));
        node
    }

    fn update_node(&self, node: &Node) {
        if let Some(session) = node.first_resource::<ThemeRevealSession<Value>>() {
            session.configure(self.value.clone(), self.transition, node);
        }
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![AnyView::new(self.content.clone().theme(self.theme.clone()))]
    }
}

impl<Value: Clone + PartialEq + 'static> View for CircularThemeHost<Value> {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Per-host reveal driver. Keeps the last value it was configured with so a
/// recomposition that leaves the value untouched does not restart the disc.
struct ThemeRevealSession<Value: Clone + PartialEq + 'static> {
    value: RefCell<Option<Value>>,
    controller: RefCell<Option<Rc<dyn AnyAnimationController>>>,
    scheduler: RefCell<Option<AnimatorScheduler>>,
    sequence: Cell<u64>,
}
impl<Value: Clone + PartialEq + 'static> ThemeRevealSession<Value> {
    fn new() -> Self {
        Self {
            value: RefCell::new(None),
            controller: RefCell::new(None),
            scheduler: RefCell::new(None),
            sequence: Cell::new(0),
        }
    }

    fn configure(&self, value: Value, transition: ThemeTransition, node: &Node) {
        let Some(previous) = self.value.borrow().clone() else {
            *self.value.borrow_mut() = Some(value);
            return;
        };
        if previous == value {
            // A reduced-motion preference or an instant swap arriving while a
            // reveal is still running must tear the disc down, not leave it
            // mid-presentation.
            if node.prefers_reduced_motion() || transition == ThemeTransition::Instant {
                self.stop();
                node.request_reveal(None);
            }
            return;
        }
        *self.value.borrow_mut() = Some(value);
        self.stop();

        let ThemeTransition::Circular { duration } = transition else {
            node.request_reveal(None);
            return;
        };
        if !duration.is_finite() || duration <= 0.0 || node.prefers_reduced_motion() {
            node.request_reveal(None);
            return;
        }

        let bounds = WindowRevealContext::bounds(node);
        let origin = WindowRevealContext::activation_origin(node).unwrap_or(Point::new(
            bounds.x + bounds.width / 2.0,
            bounds.y + bounds.height / 2.0,
        ));
        let sequence = self.sequence.get().wrapping_add(1);
        self.sequence.set(sequence);

        apply_progress(node, origin, sequence, 0.0);

        // Cubic-bezier(0.4, 0, 0.2, 1) — the standard motion curve, authored
        // directly because a theme swap may run before a theme is available on
        // the reveal host itself.
        let animation = Animation::new(duration, AnimationCurve::CubicBezier(0.4, 0.0, 0.2, 1.0));
        let weak: NodeWeak = node.downgrade();
        let clock = AnimationController::new(0.0f32, 1.0f32, animation, 0.0, move |progress| {
            if let Some(node) = Node::upgrade(&weak) {
                apply_progress(&node, origin, sequence, progress);
            }
        });
        let controller: Rc<dyn AnyAnimationController> = clock;
        if let Some(scheduler) = self.scheduler.borrow().as_ref() {
            scheduler.register(controller.clone());
        }
        *self.controller.borrow_mut() = Some(controller);
    }

    fn stop(&self) {
        let controller = self.controller.borrow_mut().take();
        if let Some(controller) = controller {
            controller.cancel();
            if let Some(scheduler) = self.scheduler.borrow().as_ref() {
                scheduler.unregister(&controller);
            }
        }
    }
}

/// Radius that reaches the farthest window corner, scaled by progress.
fn apply_progress(node: &Node, origin: Point, sequence: u64, progress: f32) {
    if progress >= 1.0 {
        node.request_reveal(None);
        return;
    }
    let bounds = WindowRevealContext::bounds(node);
    let dx = (origin.x - bounds.x)
        .abs()
        .max((origin.x - bounds.max_x()).abs());
    let dy = (origin.y - bounds.y)
        .abs()
        .max((origin.y - bounds.max_y()).abs());
    let radius = (dx * dx + dy * dy).sqrt() * progress;
    node.request_reveal(Some(Reveal::new(sequence, origin.x, origin.y, radius, 1.0)));
}

impl<Value: Clone + PartialEq + 'static> NodeResource for ThemeRevealSession<Value> {
    fn mount(&self, _node: &Node) {
        *self.scheduler.borrow_mut() = Some(AnimatorScheduler::current());
    }
    fn unmount(&self, node: &Node) {
        self.stop();
        node.request_reveal(None);
    }
    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}
