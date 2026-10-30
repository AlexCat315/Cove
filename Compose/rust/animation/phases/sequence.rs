//! Discrete animation phases translated from Animation/Phases/Sequence.swift.
use cove_core::scene::{
    ActiveAnimationContext, Align, Animation, AnimationController, AnimationCurve,
    AnimatorScheduler, AnyAnimationController, FlexDirection, LayoutNode, Node, NodeResource,
};
use cove_core::State;
use crate::{AnyView, DynamicField, PrimitiveView, View, ViewKind};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

/// Advances through discrete values when `trigger` changes, optionally looping
/// back to the first phase after the last phase.
pub struct PhaseAnimator<Phase, Trigger>
where
    Phase: Clone + Eq + 'static,
    Trigger: Clone + Eq + 'static,
{
    phases: Vec<Phase>,
    trigger: Trigger,
    repeating: bool,
    animation: Rc<dyn Fn(&Phase) -> Animation>,
    content: Rc<dyn Fn(Phase) -> AnyView>,
    index: State<usize>,
}

impl<Phase, Trigger> PhaseAnimator<Phase, Trigger>
where
    Phase: Clone + Eq + 'static,
    Trigger: Clone + Eq + 'static,
{
    pub fn new<Content>(
        phases: Vec<Phase>,
        trigger: Trigger,
        repeating: bool,
        content: impl Fn(Phase) -> Content + 'static,
    ) -> Self
    where
        Content: View + 'static,
    {
        Self::with_animation(phases, trigger, repeating, |_| Animation::default(), content)
    }

    pub fn with_animation<Content>(
        phases: Vec<Phase>,
        trigger: Trigger,
        repeating: bool,
        animation: impl Fn(&Phase) -> Animation + 'static,
        content: impl Fn(Phase) -> Content + 'static,
    ) -> Self
    where
        Content: View + 'static,
    {
        assert!(!phases.is_empty(), "a phase animator requires at least one phase");
        Self {
            phases,
            trigger,
            repeating,
            animation: Rc::new(animation),
            content: Rc::new(move |phase| AnyView::new(content(phase))),
            index: State::new(0),
        }
    }
}

impl<Phase, Trigger> View for PhaseAnimator<Phase, Trigger>
where
    Phase: Clone + Eq + 'static,
    Trigger: Clone + Eq + 'static,
{
    fn body(&self) -> AnyView {
        let index = self.index.get().min(self.phases.len() - 1);
        let animations = self.phases.iter().map(|phase| (self.animation)(phase)).collect();
        let state = self.index.clone();
        AnyView::new(PhaseHost {
            phases: self.phases.clone(),
            trigger: self.trigger.clone(),
            repeating: self.repeating,
            animations,
            publish: Rc::new(move |next, animation| {
                if state.get() == next { return; }
                ActiveAnimationContext::with(animation, None, None, || state.set(next));
            }),
            content: (self.content)(self.phases[index].clone()),
        })
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("phaseIndex", &self.index)]
    }
}

struct PhaseHost<Phase, Trigger>
where
    Phase: Clone + Eq + 'static,
    Trigger: Clone + Eq + 'static,
{
    phases: Vec<Phase>,
    trigger: Trigger,
    repeating: bool,
    animations: Vec<Animation>,
    publish: Rc<dyn Fn(usize, Option<Animation>)>,
    content: AnyView,
}

impl<Phase, Trigger> PrimitiveView for PhaseHost<Phase, Trigger>
where
    Phase: Clone + Eq + 'static,
    Trigger: Clone + Eq + 'static,
{
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.add_resource(Rc::new(PhaseSession::<Phase, Trigger>::new()));
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> { Some(LayoutNode::new()) }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
    }

    fn update_node(&self, node: &Node) {
        let Some(session) = node.first_resource::<PhaseSession<Phase, Trigger>>() else { return };
        session.update(
            self.phases.clone(),
            self.trigger.clone(),
            self.repeating,
            self.animations.clone(),
            node.prefers_reduced_motion(),
            self.publish.clone(),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> { vec![self.content.clone()] }
}

impl<Phase, Trigger> View for PhaseHost<Phase, Trigger>
where
    Phase: Clone + Eq + 'static,
    Trigger: Clone + Eq + 'static,
{
    fn kind(&self) -> ViewKind<'_> { ViewKind::Primitive(self) }
}

struct PhaseSession<Phase, Trigger> {
    phases: RefCell<Vec<Phase>>,
    animations: RefCell<Vec<Animation>>,
    trigger: RefCell<Option<Trigger>>,
    repeating: Cell<bool>,
    reduced_motion: Cell<bool>,
    controller: RefCell<Option<Rc<dyn AnyAnimationController>>>,
    scheduler: RefCell<Option<AnimatorScheduler>>,
    publish: RefCell<Rc<dyn Fn(usize, Option<Animation>)>>,
}

impl<Phase: Clone + Eq + 'static, Trigger: Clone + Eq + 'static> PhaseSession<Phase, Trigger> {
    fn new() -> Self {
        Self {
            phases: RefCell::new(Vec::new()),
            animations: RefCell::new(Vec::new()),
            trigger: RefCell::new(None),
            repeating: Cell::new(false),
            reduced_motion: Cell::new(false),
            controller: RefCell::new(None),
            scheduler: RefCell::new(None),
            publish: RefCell::new(Rc::new(|_, _| {})),
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

    fn send(&self, index: usize, animation: Option<Animation>) {
        let publish = self.publish.borrow().clone();
        publish(index, animation);
    }

    fn update(
        self: &Rc<Self>,
        phases: Vec<Phase>,
        trigger: Trigger,
        repeating: bool,
        animations: Vec<Animation>,
        reduced_motion: bool,
        publish: Rc<dyn Fn(usize, Option<Animation>)>,
    ) {
        *self.publish.borrow_mut() = publish;
        let mounted = self.trigger.borrow().is_some();
        let triggered = mounted && self.trigger.borrow().as_ref() != Some(&trigger);
        let was_playing = self.controller.borrow().as_ref().is_some_and(|controller| !controller.is_finished());
        let plan_changed = *self.phases.borrow() != phases || *self.animations.borrow() != animations;
        let was_repeating = self.repeating.get();
        let changed = triggered || !mounted || plan_changed || was_repeating != repeating
            || self.reduced_motion.get() != reduced_motion;
        if !changed { return; }

        *self.phases.borrow_mut() = phases.clone();
        *self.animations.borrow_mut() = animations.clone();
        *self.trigger.borrow_mut() = Some(trigger);
        self.repeating.set(repeating);
        self.reduced_motion.set(reduced_motion);
        if reduced_motion && was_playing {
            if let Some(controller) = self.controller.borrow().as_ref() {
                controller.finish_immediately();
            }
        }
        self.stop();

        if reduced_motion {
            if repeating { self.send(0, None); }
            else if triggered || was_playing { self.send(phases.len() - 1, None); }
            return;
        }
        if !triggered && !repeating && !(plan_changed && was_playing) {
            if was_repeating { self.send(phases.len() - 1, None); }
            else if mounted && plan_changed { self.send(0, None); }
            return;
        }

        let targets: Vec<usize> = (1..phases.len()).chain(repeating.then_some(0)).collect();
        if targets.is_empty() { return; }
        let mut total = 0.0_f64;
        let ends: Vec<f64> = targets.iter().map(|&target| {
            total += (animations[target].duration + animations[target].delay).max(0.000001);
            total
        }).collect();
        let last_index = Rc::new(Cell::new(None::<usize>));
        let targets = Rc::new(targets);
        let animations = Rc::new(animations);
        let weak: Weak<Self> = Rc::downgrade(self);
        let apply = move |time: f64| {
            let mut segment = 0;
            while segment + 1 < ends.len() && time >= ends[segment] { segment += 1; }
            let index = targets[segment];
            if last_index.replace(Some(index)) == Some(index) { return; }
            if let Some(session) = weak.upgrade() {
                session.send(index, Some(animations[index].clone()));
            }
        };
        apply(0.0);
        let clock = Animation::new(total, AnimationCurve::Linear);
        let clock = if repeating { clock.repeat_forever(false) } else { clock };
        let controller = AnimationController::new(0.0, total, clock, 0.0, apply);
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

impl<Phase: Clone + Eq + 'static, Trigger: Clone + Eq + 'static> NodeResource for PhaseSession<Phase, Trigger> {
    fn mount(&self, _node: &Node) {
        *self.scheduler.borrow_mut() = Some(AnimatorScheduler::current());
    }

    fn unmount(&self, _node: &Node) {
        self.stop();
        *self.publish.borrow_mut() = Rc::new(|_, _| {});
        self.scheduler.borrow_mut().take();
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> { self }
}
