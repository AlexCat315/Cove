//! Animated number text translated from Animation/Content/NumericText.swift.
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, Animation, AnimationController,
    ActiveAnimationContext, AnimatorScheduler, AnyAnimationController, FlexDirection, Align,
    LayoutNode, Node, NodeResource,
};
use cove_core::State;
use crate::primitives::text::Text;
use crate::{impl_primitive_view, AnyView, DynamicField, PrimitiveView, View};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use crate::theme::{theme_for_node, SemanticMotionRef};

/// A number whose visual label interpolates while assistive technology receives
/// the final value as soon as the target changes.
pub struct NumericText {
    value: f64,
    animation: Option<Animation>,
    format: Rc<dyn Fn(f64) -> String>,
    displayed: State<f64>,
}

impl NumericText {
    pub fn new(value: f64) -> Self {
        Self::with_format(value, None, |value| format!("{value:.0}"))
    }

    pub fn with_animation(value: f64, animation: Option<Animation>) -> Self {
        Self::with_format(value, animation, |value| format!("{value:.0}"))
    }

    pub fn with_format(
        value: f64,
        animation: Option<Animation>,
        format: impl Fn(f64) -> String + 'static,
    ) -> Self {
        let value = finite(value);
        Self { value, animation, format: Rc::new(format), displayed: State::new(value) }
    }
}

impl View for NumericText {
    fn body(&self) -> AnyView {
        let displayed = self.displayed.get();
        AnyView::new(NumberHost {
            value: self.value,
            animation: self.animation.clone(),
            label: (self.format)(self.value),
            publish: self.displayed.clone(),
            content: AnyView::new(Text::new((self.format)(displayed))),
        })
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("displayedValue", &self.displayed)]
    }
}

struct NumberHost {
    value: f64,
    animation: Option<Animation>,
    label: String,
    publish: State<f64>,
    content: AnyView,
}

impl PrimitiveView for NumberHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.add_resource(Rc::new(NumberSession::new()));
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
    }

    fn update_node(&self, node: &Node) {
        let mut semantics = AccessibilitySemantics::new(AccessibilityRole::StaticText);
        semantics.label.clone_from(&self.label);
        semantics.combines_children = true;
        node.set_accessibility(Some(semantics));

        if let Some(session) = node.first_resource::<NumberSession>() {
            let publish = self.publish.clone();
            session.update(self.value, self.animation.clone(), node.prefers_reduced_motion(), Rc::new(move |value| publish.set(value)), node);
        }
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(NumberHost);

struct NumberSession {
    target: Cell<Option<f64>>,
    current: Cell<f64>,
    controller: RefCell<Option<Rc<dyn AnyAnimationController>>>,
    scheduler: RefCell<Option<AnimatorScheduler>>,
    publish: RefCell<Rc<dyn Fn(f64)>>,
}

impl NumberSession {
    fn new() -> Self {
        Self {
            target: Cell::new(None),
            current: Cell::new(0.0),
            controller: RefCell::new(None),
            scheduler: RefCell::new(None),
            publish: RefCell::new(Rc::new(|_| {})),
        }
    }

    fn stop(&self) {
        if let Some(controller) = self.controller.borrow_mut().take() {
            controller.cancel();
            if let Some(scheduler) = self.scheduler.borrow().as_ref() {
                scheduler.unregister(&controller);
            }
        }
    }

    fn publish_value(&self, value: f64) {
        let publish = self.publish.borrow().clone();
        publish(value);
    }

    fn update(
        self: &Rc<Self>,
        value: f64,
        animation: Option<Animation>,
        reduced_motion: bool,
        publish: Rc<dyn Fn(f64)>,
        node: &Node,
    ) {
        *self.publish.borrow_mut() = publish;
        let context = ActiveAnimationContext::current();
        if context.disables_animations {
            let changed = self.target.get() != Some(value) || self.current.get() != value;
            self.stop();
            self.current.set(value);
            self.target.set(Some(value));
            if changed { self.publish_value(value); }
            return;
        }

        if reduced_motion {
            let changed = self.target.get() != Some(value)
                || self.current.get() != value
                || self.controller.borrow().as_ref().is_some_and(|controller| !controller.is_finished());
            if self.target.get() == Some(value) {
                if let Some(controller) = self.controller.borrow().as_ref() {
                    controller.finish_immediately();
                }
            }
            self.stop();
            self.current.set(value);
            self.target.set(Some(value));
            if changed { self.publish_value(value); }
            return;
        }

        if self.target.get() == Some(value) { return; }
        let mounted = self.target.get().is_some();
        self.target.set(Some(value));
        self.stop();
        if !mounted {
            self.current.set(value);
            self.publish_value(value);
            return;
        }

        let animation = animation
            .or(context.animation)
            .unwrap_or_else(|| SemanticMotionRef::medium().resolve(&theme_for_node(node)));
        let weak: Weak<Self> = Rc::downgrade(self);
        let controller = AnimationController::new(self.current.get(), value, animation, 0.0, move |value| {
            if let Some(session) = weak.upgrade() {
                session.current.set(value);
                session.publish_value(value);
            }
        });
        if let Some(completion) = context.completion {
            completion.attach(&controller);
        }
        let controller: Rc<dyn AnyAnimationController> = controller;
        if let Some(scheduler) = self.scheduler.borrow().as_ref() {
            scheduler.register(controller.clone());
        }
        *self.controller.borrow_mut() = Some(controller);
    }
}

impl NodeResource for NumberSession {
    fn mount(&self, _node: &Node) {
        *self.scheduler.borrow_mut() = Some(AnimatorScheduler::current());
    }

    fn unmount(&self, _node: &Node) {
        self.stop();
        *self.publish.borrow_mut() = Rc::new(|_| {});
        self.scheduler.borrow_mut().take();
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> { self }
}

fn finite(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}
