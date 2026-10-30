//! Retained insertion/removal lifecycle from Animation/AnimatedVisibility.swift.
use crate::animation::transition::TransitionEffect;
use super::super::Transition;
use cove_core::scene::{
    Animation, AnimationController, AnimatorScheduler, AnyAnimationController, Display, FocusChainHolder,
    LayoutNode, LayoutUpdateHook, Node, NodeResource, PointerCaptureHolder,
};
use cove_core::{Point, State};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;
use crate::theme::{theme_for_node, SemanticMotionRef};
use crate::modifiers::layout::FlexExt;
use crate::{impl_primitive_view, AnyView, DynamicField, PrimitiveView, View};

enum AnimationChoice {
    Fixed(Option<Animation>),
    Motion(Option<SemanticMotionRef>),
}

/// Holds content during an exit transition and removes it when the animation settles.
pub struct AnimatedVisibility {
    pub is_visible: bool,
    pub transition: Transition,
    pub animate_on_mount: bool,
    pub stagger_index: usize,
    pub stagger_interval: Duration,
    retains_content: bool,
    animation: AnimationChoice,
    content: AnyView,
    on_visibility_settled: Option<Rc<dyn Fn(bool)>>,
    completion_revision: State<u64>,
}

impl AnimatedVisibility {
    pub fn collapsing(
        is_visible: bool,
        collapses: bool,
        animation: Option<Animation>,
        content: impl View + 'static,
    ) -> Self {
        let transition = if collapses {
            Transition::opacity().combined(Transition::collapse())
        } else {
            Transition::opacity()
        };
        Self::new(is_visible, transition, animation, false, None, content)
    }

    pub fn new(
        is_visible: bool,
        transition: Transition,
        animation: Option<Animation>,
        animate_on_mount: bool,
        on_visibility_settled: Option<Rc<dyn Fn(bool)>>,
        content: impl View + 'static,
    ) -> Self {
        Self::build(is_visible, transition, AnimationChoice::Fixed(animation), animate_on_mount, false, 0, Duration::ZERO, on_visibility_settled, AnyView::new(content))
    }

    pub fn with_motion(
        is_visible: bool,
        transition: Transition,
        motion: Option<SemanticMotionRef>,
        animate_on_mount: bool,
        on_visibility_settled: Option<Rc<dyn Fn(bool)>>,
        content: impl View + 'static,
    ) -> Self {
        Self::build(is_visible, transition, AnimationChoice::Motion(motion), animate_on_mount, false, 0, Duration::ZERO, on_visibility_settled, AnyView::new(content))
    }

    pub fn retaining_content(mut self, retain: bool) -> Self {
        self.retains_content = retain;
        self
    }

    pub fn staggered(mut self, index: usize, interval: Duration) -> Self {
        self.stagger_index = index;
        self.stagger_interval = interval;
        self
    }

    fn build(
        is_visible: bool,
        transition: Transition,
        animation: AnimationChoice,
        animate_on_mount: bool,
        retains_content: bool,
        stagger_index: usize,
        stagger_interval: Duration,
        on_visibility_settled: Option<Rc<dyn Fn(bool)>>,
        content: AnyView,
    ) -> Self {
        Self {
            is_visible,
            transition,
            animate_on_mount,
            stagger_index,
            stagger_interval,
            retains_content,
            animation,
            content,
            on_visibility_settled,
            completion_revision: State::new(0),
        }
    }
}

impl View for AnimatedVisibility {
    fn body(&self) -> AnyView {
        let revision = self.completion_revision.clone();
        let settled = self.on_visibility_settled.clone();
        let visible = self.is_visible;
        let on_finished: Rc<dyn Fn()> = Rc::new(move || {
            revision.set(revision.get().wrapping_add(1));
            if let Some(settled) = &settled { settled(visible); }
        });
        AnyView::new(VisibilityHost {
            is_visible: self.is_visible,
            transition: self.transition,
            animate_on_mount: self.animate_on_mount,
            retains_content: self.retains_content,
            animation: match &self.animation {
                AnimationChoice::Fixed(animation) => AnimationChoice::Fixed(animation.clone()),
                AnimationChoice::Motion(motion) => AnimationChoice::Motion(motion.clone()),
            },
            stagger_index: self.stagger_index,
            stagger_interval: self.stagger_interval,
            content: self.content.clone(),
            on_finished,
        })
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("completionRevision", &self.completion_revision)]
    }
}

struct VisibilityState {
    progress: Cell<f32>,
    visual: Cell<TransitionEffect>,
    target: Cell<Option<bool>>,
    expanded_height: Cell<f32>,
    controller: RefCell<Option<Rc<dyn AnyAnimationController>>>,
    scheduler: RefCell<Option<AnimatorScheduler>>,
    collapses: Cell<bool>,
    retains_content: Cell<bool>,
    on_finished: RefCell<Rc<dyn Fn()>>,
    layout_hook_installed: Cell<bool>,
    previous_layout: RefCell<Option<LayoutUpdateHook>>,
    installed_layout: RefCell<Option<LayoutUpdateHook>>,
}

impl VisibilityState {
    fn new() -> Self {
        Self {
            progress: Cell::new(0.0),
            visual: Cell::new(TransitionEffect::default()),
            target: Cell::new(None),
            expanded_height: Cell::new(0.0),
            controller: RefCell::new(None),
            scheduler: RefCell::new(None),
            collapses: Cell::new(false),
            retains_content: Cell::new(false),
            on_finished: RefCell::new(Rc::new(|| {})),
            layout_hook_installed: Cell::new(false),
            previous_layout: RefCell::new(None),
            installed_layout: RefCell::new(None),
        }
    }

    fn cancel(&self) {
        let controller = self.controller.borrow_mut().take();
        if let Some(controller) = controller {
            controller.cancel();
            if let Some(scheduler) = self.scheduler.borrow().as_ref() { scheduler.unregister(&controller); }
        }
    }

    fn animate(&self, controller: Rc<dyn AnyAnimationController>) {
        let running = !controller.is_finished();
        *self.controller.borrow_mut() = Some(controller.clone());
        if running && let Some(scheduler) = self.scheduler.borrow().as_ref() {
            scheduler.register(controller);
        }
    }

    fn settle_immediately(&self) {
        let controller = self.controller.borrow().clone();
        if let Some(controller) = controller.filter(|controller| !controller.is_finished()) {
            controller.finish_immediately();
            if let Some(scheduler) = self.scheduler.borrow().as_ref() { scheduler.unregister(&controller); }
        }
        self.controller.borrow_mut().take();
    }
}

impl NodeResource for VisibilityState {
    fn mount(&self, _node: &Node) {
        *self.scheduler.borrow_mut() = Some(AnimatorScheduler::current());
    }

    fn unmount(&self, node: &Node) {
        self.cancel();
        *self.on_finished.borrow_mut() = Rc::new(|| {});
        self.scheduler.borrow_mut().take();
        if let Some(installed) = self.installed_layout.borrow_mut().take()
            && node.layout_did_update().is_some_and(|current| Rc::ptr_eq(&current, &installed))
        {
            node.set_layout_did_update(self.previous_layout.borrow_mut().take());
        } else {
            self.previous_layout.borrow_mut().take();
        }
        self.layout_hook_installed.set(false);
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any> { self }
}

struct VisibilityHost {
    is_visible: bool,
    transition: Transition,
    animate_on_mount: bool,
    retains_content: bool,
    animation: AnimationChoice,
    stagger_index: usize,
    stagger_interval: Duration,
    content: AnyView,
    on_finished: Rc<dyn Fn()>,
}

impl VisibilityHost {
    fn collapses(&self) -> bool {
        self.transition.insertion.collapse || self.transition.removal.collapse
    }

    fn install_layout_hook(node: &Node, state: &Rc<VisibilityState>) {
        if state.layout_hook_installed.get() { return; }
        let previous = node.layout_did_update();
        *state.previous_layout.borrow_mut() = previous.clone();
        let weak_state = Rc::downgrade(state);
        let hook: LayoutUpdateHook = Rc::new(move |node| {
            if let Some(previous) = &previous { previous(node); }
            let Some(state) = weak_state.upgrade() else { return };
            if let Some(layout) = node.layout_node()
                && let Some(child) = layout.children().first()
            {
                state.expanded_height.set(child.frame().height);
            }
            Self::apply_state(&state, node);
        });
        node.set_layout_did_update(Some(hook.clone()));
        *state.installed_layout.borrow_mut() = Some(hook);
        state.layout_hook_installed.set(true);
    }

    fn resolve_animation(&self, node: &Node) -> Animation {
        match &self.animation {
            AnimationChoice::Fixed(Some(animation)) => animation.clone(),
            AnimationChoice::Fixed(None) => SemanticMotionRef::medium().resolve(&theme_for_node(node)),
            AnimationChoice::Motion(Some(motion)) => motion.resolve(&theme_for_node(node)),
            AnimationChoice::Motion(None) => SemanticMotionRef::medium().resolve(&theme_for_node(node)),
        }
    }

    fn finish(state: &VisibilityState) {
        let callback = state.on_finished.borrow().clone();
        callback();
    }

    fn apply_state(state: &VisibilityState, node: &Node) {
        let target = state.target.get();
        let progress = state.progress.get();
        let visual = state.visual.get();
        if state.retains_content.get()
            && let Some(layout) = node.layout_node()
        {
            layout.set_display(if target == Some(false) && progress == 0.0 { Display::None } else { Display::Flex });
        }
        let opacity = if state.retains_content.get() && target == Some(false) && progress == 0.0 {
            0.0
        } else {
            visual.opacity.clamp(0.0, 1.0)
        };
        node.set_opacity(opacity);
        node.set_content_offset(Point::new(-visual.x, -visual.y));
        node.set_transform(visual.scale);

        if state.collapses.get()
            && let Some(layout) = node.layout_node()
        {
            let maximum_height = if target == Some(false) && progress == 0.0 {
                Some(0.0)
            } else if progress >= 1.0 {
                None
            } else if state.expanded_height.get() > 0.0 {
                Some(state.expanded_height.get() * progress.max(0.0))
            } else {
                None
            };
            layout.set_max_height(maximum_height);
        }
    }

    fn update_visibility(&self, node: &Node, state: &Rc<VisibilityState>) {
        let collapses = self.collapses();
        state.collapses.set(collapses);
        state.retains_content.set(self.retains_content);
        *state.on_finished.borrow_mut() = self.on_finished.clone();
        node.set_clips_to_bounds(collapses);
        node.set_allows_hit_testing(self.is_visible);
        node.set_is_interaction_enabled(self.is_visible);
        Self::install_layout_hook(node, state);

        if !self.is_visible {
            if let Some(focus) = FocusChainHolder::current()
                && focus.focused().is_some_and(|focused| !focused.accepts_subtree_input())
            {
                focus.clear();
            }
            if let Some(capture) = PointerCaptureHolder::current()
                && capture.target().is_some_and(|target| !target.accepts_subtree_input())
            {
                capture.release();
            }
        }

        if state.target.get() == Some(self.is_visible) {
            if node.prefers_reduced_motion() || (self.retains_content && self.transition == Transition::IDENTITY) {
                state.settle_immediately();
            }
            return;
        }

        let was_mounted = state.target.get().is_some();
        state.target.set(Some(self.is_visible));
        state.cancel();
        let target = if self.is_visible { 1.0 } else { 0.0 };
        let goal = if self.is_visible { TransitionEffect::default() } else { self.transition.removal };

        if node.prefers_reduced_motion() || (self.retains_content && self.transition == Transition::IDENTITY) {
            state.progress.set(target);
            state.visual.set(goal);
            Self::apply_state(state, node);
            if was_mounted || (self.animate_on_mount && self.is_visible) { Self::finish(state); }
            return;
        }

        if !was_mounted {
            state.progress.set(if self.is_visible && self.animate_on_mount { 0.0 } else { target });
            state.visual.set(if self.is_visible && self.animate_on_mount { self.transition.insertion } else { goal });
            if !self.animate_on_mount || !self.is_visible {
                Self::apply_state(state, node);
                return;
            }
        }
        if self.is_visible && state.progress.get() == 0.0 {
            state.visual.set(self.transition.insertion);
        }

        let from = state.visual.get();
        let from_progress = state.progress.get();
        let mut animation = self.resolve_animation(node);
        if self.is_visible && self.stagger_index > 0 {
            animation.delay = animation.delay.max(0.0) + self.stagger_index as f64 * self.stagger_interval.as_secs_f64();
        }
        let weak_state: Weak<VisibilityState> = Rc::downgrade(state);
        let weak_node = node.downgrade();
        let controller = AnimationController::new(from_progress, target, animation, 0.0, move |amount| {
            let (Some(state), Some(node)) = (weak_state.upgrade(), Node::upgrade(&weak_node)) else { return };
            state.progress.set(from_progress + (target - from_progress) * amount);
            state.visual.set(from.interpolating(goal, amount));
            Self::apply_state(&state, &node);
            if amount == 1.0 { Self::finish(&state); }
        });
        let controller: Rc<dyn AnyAnimationController> = controller;
        state.animate(controller);
        Self::apply_state(state, node);
    }
}

impl PrimitiveView for VisibilityHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.add_resource(Rc::new(VisibilityState::new()));
        node
    }

    fn update_node(&self, node: &Node) {
        if let Some(state) = node.first_resource::<VisibilityState>() {
            self.update_visibility(node, &state);
        }
    }

    fn make_layout_node(&self) -> Option<LayoutNode> { Some(LayoutNode::new()) }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(cove_core::scene::FlexDirection::Column);
        layout.set_align_items(cove_core::scene::Align::Stretch);
        layout.set_min_height(Some(0.0));
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let progress = node.first_resource::<VisibilityState>().map_or(0.0, |state| state.progress.get());
        if !self.retains_content && !self.is_visible && progress == 0.0 { return Vec::new(); }
        if self.collapses() {
            let content = self.content.clone();
            vec![AnyView::new(crate::BoxView::new(
                cove_core::scene::FlexDirection::Column,
                cove_core::scene::Align::Stretch,
                cove_core::scene::Justify::FlexStart,
                cove_core::scene::Wrap::NoWrap,
                0.0,
                content,
            ).flex(0.0).shrink(0.0))]
        } else {
            vec![self.content.clone()]
        }
    }
}
impl_primitive_view!(VisibilityHost);
