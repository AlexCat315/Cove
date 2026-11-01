//! Triggered and repeating keyframe tracks translated from Animation/Keyframes/Track.swift.
use crate::{AnyView, DynamicField, PrimitiveView, View, ViewKind};
use cove_core::State;
use cove_core::scene::{
    ActiveAnimationContext, Align, Animation, AnimationController, AnimationCurve,
    AnimatorScheduler, AnyAnimationController, FlexDirection, Interpolatable, KeyframeTimeline,
    LayoutNode, Node, NodeResource,
};
use std::cell::RefCell;
use std::rc::{Rc, Weak};

/// Plays an authored timeline when its trigger changes.
pub struct KeyframeAnimator<Value, Trigger>
where
    Value: Interpolatable + Send,
    Trigger: Clone + Eq + 'static,
{
    timeline: KeyframeTimeline<Value>,
    trigger: Trigger,
    repeating: bool,
    content: Rc<dyn Fn(Value) -> AnyView>,
    value: State<Value>,
}

impl<Value, Trigger> KeyframeAnimator<Value, Trigger>
where
    Value: Interpolatable + Send,
    Trigger: Clone + Eq + 'static,
{
    pub fn new<Content>(
        timeline: KeyframeTimeline<Value>,
        trigger: Trigger,
        repeating: bool,
        content: impl Fn(Value) -> Content + 'static,
    ) -> Self
    where
        Content: View + 'static,
    {
        let value = State::new(timeline.initial_value.clone());
        Self {
            timeline,
            trigger,
            repeating,
            content: Rc::new(move |value| AnyView::new(content(value))),
            value,
        }
    }
}

impl<Value, Trigger> View for KeyframeAnimator<Value, Trigger>
where
    Value: Interpolatable + Send,
    Trigger: Clone + Eq + 'static,
{
    fn body(&self) -> AnyView {
        AnyView::new(TrackHost {
            timeline: self.timeline.clone(),
            trigger: self.trigger.clone(),
            repeating: self.repeating,
            publish: self.value.clone(),
            content: (self.content)(self.value.get()),
        })
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("keyframeValue", &self.value)]
    }
}

struct TrackHost<Value, Trigger>
where
    Value: Interpolatable + Send,
    Trigger: Clone + Eq + 'static,
{
    timeline: KeyframeTimeline<Value>,
    trigger: Trigger,
    repeating: bool,
    publish: State<Value>,
    content: AnyView,
}

impl<Value, Trigger> PrimitiveView for TrackHost<Value, Trigger>
where
    Value: Interpolatable + Send,
    Trigger: Clone + Eq + 'static,
{
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.add_resource(Rc::new(TrackSession::<Trigger>::new()));
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
        let Some(session) = node.first_resource::<TrackSession<Trigger>>() else {
            return;
        };
        let timeline = self.timeline.clone();
        let publish = self.publish.clone();
        session.update(
            self.trigger.clone(),
            self.repeating,
            node.prefers_reduced_motion(),
            timeline.duration(),
            Rc::new(move |time| publish.set(timeline.value(time))),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}

impl<Value, Trigger> View for TrackHost<Value, Trigger>
where
    Value: Interpolatable + Send,
    Trigger: Clone + Eq + 'static,
{
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
}

struct TrackSession<Trigger> {
    trigger: RefCell<Option<Trigger>>,
    repeating: RefCell<bool>,
    reduced_motion: RefCell<bool>,
    duration: RefCell<f64>,
    controller: RefCell<Option<Rc<dyn AnyAnimationController>>>,
    scheduler: RefCell<Option<AnimatorScheduler>>,
    publish: RefCell<Rc<dyn Fn(f64)>>,
}

impl<Trigger: Clone + Eq + 'static> TrackSession<Trigger> {
    fn new() -> Self {
        Self {
            trigger: RefCell::new(None),
            repeating: RefCell::new(false),
            reduced_motion: RefCell::new(false),
            duration: RefCell::new(0.0),
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

    fn publish_time(&self, time: f64) {
        let publish = self.publish.borrow().clone();
        publish(time);
    }

    fn update(
        self: &Rc<Self>,
        trigger: Trigger,
        repeating: bool,
        reduced_motion: bool,
        duration: f64,
        publish: Rc<dyn Fn(f64)>,
    ) {
        *self.publish.borrow_mut() = publish;
        let mounted = self.trigger.borrow().is_some();
        let triggered = mounted && self.trigger.borrow().as_ref() != Some(&trigger);
        let was_playing = self
            .controller
            .borrow()
            .as_ref()
            .is_some_and(|controller| !controller.is_finished());
        let was_repeating = *self.repeating.borrow();
        let duration_changed = *self.duration.borrow() != duration;
        let changed = triggered
            || !mounted
            || was_repeating != repeating
            || *self.reduced_motion.borrow() != reduced_motion
            || duration_changed;
        if !changed {
            return;
        }

        *self.trigger.borrow_mut() = Some(trigger);
        *self.repeating.borrow_mut() = repeating;
        *self.reduced_motion.borrow_mut() = reduced_motion;
        *self.duration.borrow_mut() = duration;
        if reduced_motion
            && was_playing
            && let Some(controller) = self.controller.borrow().as_ref()
        {
            controller.finish_immediately();
        }
        self.stop();

        if reduced_motion {
            if repeating {
                self.publish_time(0.0);
            } else if triggered || was_playing {
                self.publish_time(duration);
            }
            return;
        }

        if !triggered && !repeating && !(duration_changed && was_playing) {
            if was_repeating {
                self.publish_time(duration);
            }
            return;
        }

        self.publish_time(0.0);
        let animation = Animation::new(duration, AnimationCurve::Linear);
        let animation = if repeating {
            animation.repeat_forever(false)
        } else {
            animation
        };
        let weak: Weak<Self> = Rc::downgrade(self);
        let controller = AnimationController::new(0.0, duration, animation, 0.0, move |time| {
            if let Some(session) = weak.upgrade() {
                session.publish_time(time);
            }
        });
        let context = ActiveAnimationContext::current();
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

impl<Trigger: Clone + Eq + 'static> NodeResource for TrackSession<Trigger> {
    fn mount(&self, _node: &Node) {
        *self.scheduler.borrow_mut() = Some(AnimatorScheduler::current());
    }

    fn unmount(&self, _node: &Node) {
        self.stop();
        *self.publish.borrow_mut() = Rc::new(|_| {});
        self.scheduler.borrow_mut().take();
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}
