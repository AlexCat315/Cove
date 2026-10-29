//! Translation of CoveUIScene/Scrolling/ScrollAnimator.swift.
use super::physics::{constrain, RubberBand, ScrollAxisMotion, ScrollPhysics, ScrollSpring, ScrollVelocityTracker};
use crate::scene::{Animation, AnimationController, AnimationCurve, AnyAnimationController, AnimatorScheduler, Interpolatable};
use crate::{Point, Size};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

impl Interpolatable for Point {
    fn interpolate(a: &Self, b: &Self, t: f32) -> Self {
        Point { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t }
    }
}

struct State {
    position: Point,
    offset: Point,
    maximum_offset: Point,
    viewport_size: Size,
    x_motion: ScrollAxisMotion,
    y_motion: ScrollAxisMotion,
    tracker_x: ScrollVelocityTracker,
    tracker_y: ScrollVelocityTracker,
    release_deadline: Option<f64>,
    is_finished: bool,
    target_animation: Option<Rc<AnimationController<Point>>>,
}

/// Owns every kind of motion one scrollable offset can have: the inertia that
/// carries content along after input stops, the rubber band it stretches
/// against past its bounds, and the spring that brings it back.
///
/// Two coordinate systems are kept apart on purpose:
/// - **raw position** — where input has pushed the content, unbounded.
/// - **visible offset** — what gets written out, compressed past the range.
pub struct ScrollAnimator {
    state: RefCell<State>,
    physics: Cell<ScrollPhysics>,
    on_change: RefCell<Option<Rc<dyn Fn(Point)>>>,
    clock: RefCell<Rc<dyn Fn() -> f64>>,
    scheduler: Option<AnimatorScheduler>,
    self_ref: RefCell<Weak<ScrollAnimator>>,
}
impl ScrollAnimator {
    pub fn new(physics: ScrollPhysics, maximum_offset: Point, viewport_size: Size, initial_offset: Point, on_change: Option<Rc<dyn Fn(Point)>>) -> Rc<Self> {
        let position = constrain(initial_offset, maximum_offset);
        let animator = Rc::new(Self {
            state: RefCell::new(State {
                position,
                offset: position,
                maximum_offset,
                viewport_size,
                x_motion: ScrollAxisMotion::default(),
                y_motion: ScrollAxisMotion::default(),
                tracker_x: ScrollVelocityTracker::default(),
                tracker_y: ScrollVelocityTracker::default(),
                release_deadline: None,
                is_finished: true,
                target_animation: None,
            }),
            physics: Cell::new(physics),
            on_change: RefCell::new(on_change),
            clock: RefCell::new(Rc::new(monotonic_clock)),
            scheduler: Some(AnimatorScheduler::current()),
            self_ref: RefCell::new(Weak::new()),
        });
        *animator.self_ref.borrow_mut() = Rc::downgrade(&animator);
        animator
    }
    pub fn physics(&self) -> ScrollPhysics {
        self.physics.get()
    }
    pub fn set_physics(&self, physics: ScrollPhysics) {
        self.physics.set(physics);
    }
    pub fn set_on_change(&self, on_change: Option<Rc<dyn Fn(Point)>>) {
        *self.on_change.borrow_mut() = on_change;
    }
    pub fn set_clock(&self, clock: Rc<dyn Fn() -> f64>) {
        *self.clock.borrow_mut() = clock;
    }
    fn now(&self) -> f64 {
        (self.clock.borrow())()
    }
    pub fn offset(&self) -> Point {
        self.state.borrow().offset
    }
    pub fn position(&self) -> Point {
        self.state.borrow().position
    }
    pub fn maximum_offset(&self) -> Point {
        self.state.borrow().maximum_offset
    }
    pub fn viewport_size(&self) -> Size {
        self.state.borrow().viewport_size
    }
    /// Velocity of the current glide, in points per second.
    pub fn velocity(&self) -> Point {
        let state = self.state.borrow();
        Point { x: state.x_motion.glide.unwrap_or(0.0), y: state.y_motion.glide.unwrap_or(0.0) }
    }
    /// `true` while the return spring owns the offset.
    pub fn is_settling(&self) -> bool {
        let state = self.state.borrow();
        state.x_motion.spring.is_some() || state.y_motion.spring.is_some()
    }
    /// `true` while inertia owns the offset.
    pub fn is_gliding(&self) -> bool {
        let state = self.state.borrow();
        state.x_motion.glide.is_some() || state.y_motion.glide.is_some()
    }
    /// `true` while any axis is past its bound.
    pub fn is_overscrolled(&self) -> bool {
        let state = self.state.borrow();
        RubberBand::excess(state.position.x, state.maximum_offset.x) != 0.0
            || RubberBand::excess(state.position.y, state.maximum_offset.y) != 0.0
    }

    // MARK: - Range

    /// Publish freshly laid-out geometry. A range that *shrank* pulls the offset
    /// back in immediately and drops the recorded velocity.
    pub fn update(&self, maximum_offset: Point, viewport_size: Size) {
        let mut state = self.state.borrow_mut();
        let previous = state.maximum_offset;
        state.maximum_offset = maximum_offset;
        state.viewport_size = viewport_size;
        if !(maximum_offset.x < previous.x || maximum_offset.y < previous.y) {
            return;
        }
        let constrained = constrain(state.position, maximum_offset);
        if constrained == state.position {
            return;
        }
        state.target_animation = None;
        state.x_motion.stop();
        state.y_motion.stop();
        state.tracker_x.reset();
        state.tracker_y.reset();
        state.release_deadline = None;
        state.position = constrained;
        let offset = constrained;
        drop(state);
        self.set_offset(offset);
        self.stop_driving_frames();
    }
    /// Adopt an offset written by someone else — a binding, a `scrollTo` call,
    /// a layout correction.
    pub fn adopt(&self, new_offset: Point) {
        if new_offset == self.offset() {
            return;
        }
        let mut state = self.state.borrow_mut();
        state.release_deadline = None;
        state.target_animation = None;
        state.x_motion.stop();
        state.y_motion.stop();
        state.tracker_x.reset();
        state.tracker_y.reset();
        state.position = constrain(new_offset, state.maximum_offset);
        let position = state.position;
        drop(state);
        self.set_offset(position);
        self.stop_driving_frames();
    }

    // MARK: - Input

    /// Follow a platform-owned wheel/trackpad stream exactly.
    pub fn scroll_directly(&self, delta: Point) -> Point {
        if !delta.x.is_finite() || !delta.y.is_finite() {
            return self.offset();
        }
        self.cancel();
        {
            let mut state = self.state.borrow_mut();
            let maximum = state.maximum_offset;
            let offset = state.offset;
            state.position = constrain(Point { x: offset.x + delta.x, y: offset.y + delta.y }, maximum);
            let position = state.position;
            drop(state);
            self.set_offset(position);
        }
        self.offset()
    }
    /// Apply one increment from an application-owned gesture and return its
    /// visible offset.
    pub fn scroll_by(&self, delta: Point) -> Point {
        let time = self.now();
        self.synchronize_raw_position();
        let was_coasting = self.is_gliding();
        self.stop_animation();
        let maximum = self.state.borrow().maximum_offset;
        let position = self.position();
        let next = self.capped(Point { x: position.x + delta.x, y: position.y + delta.y }, maximum);
        {
            let mut state = self.state.borrow_mut();
            state.release_deadline = None;
            if was_coasting {
                state.tracker_x.reset();
                state.tracker_y.reset();
            }
            state.position = next;
        }
        self.record_input(time);
        let visible = self.visible_offset(next);
        self.set_offset(visible);
        if self.is_overscrolled() {
            let launch = self.current_velocity();
            self.reset_tracking();
            self.begin_settling(launch);
        } else if self.physics.get().momentum.is_enabled {
            self.state.borrow_mut().release_deadline = Some(time + self.physics.get().momentum.idle_window.max(0.0));
            self.start_driving_frames();
        }
        self.offset()
    }
    /// Place an application-owned gesture's raw position while recording
    /// velocity for `settle`.
    pub fn move_to(&self, new_position: Point) -> Point {
        {
            let mut state = self.state.borrow_mut();
            state.release_deadline = None;
        }
        self.synchronize_raw_position();
        let was_coasting = self.is_gliding();
        self.stop_animation();
        let maximum = self.state.borrow().maximum_offset;
        let position = self.capped(new_position, maximum);
        {
            let mut state = self.state.borrow_mut();
            if was_coasting {
                state.tracker_x.reset();
                state.tracker_y.reset();
            }
            state.position = position;
        }
        self.record_input(self.now());
        let visible = self.visible_offset(self.position());
        self.set_offset(visible);
        self.offset()
    }
    /// Release input that has stopped, whether or not anything announced it.
    pub fn settle(&self) {
        {
            let mut state = self.state.borrow_mut();
            state.release_deadline = None;
        }
        let launch = self.current_velocity();
        let floor = self.physics.get().momentum.minimum_sample_count;
        let mut thrown = launch;
        {
            let state = self.state.borrow();
            if state.tracker_x.sample_count() < floor {
                thrown.x = 0.0;
            }
            if state.tracker_y.sample_count() < floor {
                thrown.y = 0.0;
            }
        }
        self.reset_tracking();
        let speed = thrown.x.hypot(thrown.y);
        if !self.physics.get().momentum.is_enabled || self.is_overscrolled() || speed <= self.physics.get().momentum.minimum_velocity {
            self.settle_now(thrown);
            return;
        }
        self.begin_glide(thrown);
    }
    /// Stop a glide dead where it is, leaving the current offset alone.
    pub fn stop_coasting(&self) {
        {
            let mut state = self.state.borrow_mut();
            state.release_deadline = None;
        }
        self.reset_tracking();
        {
            let mut state = self.state.borrow_mut();
            state.x_motion.glide = None;
            state.y_motion.glide = None;
        }
        if !self.is_settling() {
            self.stop_driving_frames();
        }
    }

    // MARK: - Frame driver

    pub fn tick(&self, delta_time: f64) {
        if let Some(deadline) = self.state.borrow().release_deadline {
            if self.now() < deadline {
                return;
            }
            self.settle();
            return;
        }
        if !delta_time.is_finite() || delta_time <= 0.0 {
            return;
        }
        let step = delta_time as f32;
        let physics = self.physics.get();
        let (x, y) = {
            let mut state = self.state.borrow_mut();
            let maximum = state.maximum_offset;
            let viewport = state.viewport_size;
            let x = state.x_motion.advance(state.position.x, maximum.x, viewport.width, physics, step);
            let y = state.y_motion.advance(state.position.y, maximum.y, viewport.height, physics, step);
            state.position = Point { x: x.0, y: y.0 };
            (x.1, y.1)
        };
        self.set_offset(Point { x, y });
        if !self.is_gliding() && !self.is_settling() {
            self.stop_driving_frames();
        }
    }
    /// Land immediately on the in-bounds position.
    pub fn finish_immediately(&self) {
        if !(self.is_settling() || self.is_gliding() || self.is_overscrolled() || self.state.borrow().release_deadline.is_some()) {
            return;
        }
        {
            let mut state = self.state.borrow_mut();
            state.release_deadline = None;
            state.position = constrain(state.position, state.maximum_offset);
        }
        self.stop_animation();
        let position = self.position();
        self.set_offset(position);
        self.state.borrow_mut().is_finished = true;
    }
    /// Move to an application-owned offset over a fixed duration.
    pub fn scroll_to(&self, target: Point, duration: f64) {
        let maximum = self.state.borrow().maximum_offset;
        let target = constrain(target, maximum);
        if !duration.is_finite() || duration <= 0.0 || target == self.position() {
            self.adopt(target);
            return;
        }
        self.stop_animation();
        {
            let mut state = self.state.borrow_mut();
            state.release_deadline = None;
            state.is_finished = false;
            state.tracker_x.reset();
            state.tracker_y.reset();
        }
        let from = self.position();
        let weak = self.self_ref.borrow().clone();
        let controller = AnimationController::new(from, target, Animation::ease_in_out(duration), 0.0, move |point: Point| {
            if let Some(animator) = weak.upgrade() {
                animator.state.borrow_mut().position = point;
                let visible = animator.visible_offset(point);
                animator.set_offset(visible);
            }
        });
        let weak = self.self_ref.borrow().clone();
        let weak_controller = Rc::downgrade(&controller);
        let on_finish: crate::scene::AnimationCallback = Rc::new(move || {
            if let Some(animator) = weak.upgrade() {
                let is_current = {
                    let state = animator.state.borrow();
                    state.target_animation.as_ref().is_some_and(|current| weak_controller.upgrade().is_some_and(|finished| Rc::ptr_eq(current, &finished)))
                };
                {
                    let mut state = animator.state.borrow_mut();
                    state.position = target;
                    state.is_finished = true;
                    if is_current {
                        state.target_animation = None;
                    }
                }
                animator.set_offset(target);
            }
        });
        controller.set_on_finish(Some(on_finish));
        self.state.borrow_mut().target_animation = Some(controller.clone());
        if let Some(scheduler) = &self.scheduler {
            scheduler.register(controller);
        }
    }
    /// Drop whatever motion is running without applying anything further.
    pub fn cancel(&self) {
        {
            let mut state = self.state.borrow_mut();
            state.release_deadline = None;
        }
        self.reset_tracking();
        self.stop_animation();
    }

    // MARK: - Velocity

    /// How fast the gesture is moving right now, in points per second.
    pub fn current_velocity(&self) -> Point {
        let now = self.now();
        let physics = self.physics.get();
        let stale_after = physics.momentum.stale_window.max(0.0);
        let ceiling = physics.momentum.maximum_velocity.max(0.0);
        let mut state = self.state.borrow_mut();
        state.tracker_x.window = physics.momentum.tracking_window;
        state.tracker_y.window = physics.momentum.tracking_window;
        let x = state.tracker_x.velocity(Some(now), Some(stale_after));
        let y = state.tracker_y.velocity(Some(now), Some(stale_after));
        Point { x: x.clamp(-ceiling, ceiling), y: y.clamp(-ceiling, ceiling) }
    }

    // MARK: - Private

    fn synchronize_raw_position(&self) {
        if !self.is_settling() {
            return;
        }
        let offset = self.offset();
        let raw = self.raw_position(offset);
        let maximum = self.state.borrow().maximum_offset;
        self.state.borrow_mut().position = self.capped(raw, maximum);
    }
    fn record_input(&self, time: f64) {
        let physics = self.physics.get();
        let position = self.position();
        let mut state = self.state.borrow_mut();
        state.tracker_x.window = physics.momentum.tracking_window;
        state.tracker_y.window = physics.momentum.tracking_window;
        state.tracker_x.add(time, position.x);
        state.tracker_y.add(time, position.y);
    }
    fn reset_tracking(&self) {
        let mut state = self.state.borrow_mut();
        state.tracker_x.reset();
        state.tracker_y.reset();
    }
    fn start_driving_frames(&self) {
        self.state.borrow_mut().is_finished = false;
        if let (Some(scheduler), Some(animator)) = (&self.scheduler, self.self_ref.borrow().upgrade()) {
            let erased: Rc<dyn AnyAnimationController> = animator;
            scheduler.register(erased);
        }
    }
    fn stop_driving_frames(&self) {
        self.state.borrow_mut().is_finished = true;
        if let (Some(scheduler), Some(animator)) = (&self.scheduler, self.self_ref.borrow().upgrade()) {
            let erased: Rc<dyn AnyAnimationController> = animator;
            scheduler.unregister(&erased);
        }
    }
    fn begin_glide(&self, launch: Point) {
        if launch.x == 0.0 && launch.y == 0.0 {
            self.settle_now(Point::default());
            return;
        }
        {
            let mut state = self.state.borrow_mut();
            state.x_motion.glide = Some(launch.x);
            state.y_motion.glide = Some(launch.y);
        }
        self.start_driving_frames();
    }
    fn settle_now(&self, launch: Point) {
        if !self.is_overscrolled() {
            self.stop_animation();
            let mut state = self.state.borrow_mut();
            state.position = constrain(state.position, state.maximum_offset);
            return;
        }
        self.begin_settling(launch);
    }
    fn begin_settling(&self, velocity: Point) {
        let physics = self.physics.get();
        let settings = physics.settle;
        let (position, offset, maximum) = {
            let mut state = self.state.borrow_mut();
            state.release_deadline = None;
            state.x_motion.glide = None;
            state.y_motion.glide = None;
            (state.position, state.offset, state.maximum_offset)
        };
        let target = constrain(position, maximum);
        {
            let mut state = self.state.borrow_mut();
            state.x_motion.spring = (position.x < 0.0 || position.x > maximum.x).then_some(ScrollSpring::new(offset.x, velocity.x, target.x, settings));
            state.y_motion.spring = (position.y < 0.0 || position.y > maximum.y).then_some(ScrollSpring::new(offset.y, velocity.y, target.y, settings));
        }
        if !physics.rubber_band.is_enabled {
            self.finish_immediately();
            return;
        }
        self.start_driving_frames();
    }
    fn stop_animation(&self) {
        let target = self.state.borrow_mut().target_animation.take();
        if let Some(target) = target {
            target.cancel();
        }
        let mut state = self.state.borrow_mut();
        state.x_motion.stop();
        state.y_motion.stop();
        drop(state);
        self.stop_driving_frames();
    }
    fn set_offset(&self, next: Point) {
        if next == self.state.borrow().offset {
            return;
        }
        self.state.borrow_mut().offset = next;
        let callback = self.on_change.borrow().clone();
        if let Some(callback) = callback {
            callback(next);
        }
    }
    fn visible_offset(&self, raw: Point) -> Point {
        let state = self.state.borrow();
        let physics = self.physics.get();
        Point {
            x: axis_offset(raw.x, state.maximum_offset.x, state.viewport_size.width, physics),
            y: axis_offset(raw.y, state.maximum_offset.y, state.viewport_size.height, physics),
        }
    }
    fn raw_position(&self, visible: Point) -> Point {
        let state = self.state.borrow();
        let physics = self.physics.get();
        Point {
            x: axis_position(visible.x, state.maximum_offset.x, state.viewport_size.width, physics),
            y: axis_position(visible.y, state.maximum_offset.y, state.viewport_size.height, physics),
        }
    }
    fn capped(&self, raw: Point, maximum: Point) -> Point {
        let physics = self.physics.get();
        let viewport = self.state.borrow().viewport_size;
        Point {
            x: capped_axis(raw.x, maximum.x, physics.rubber_band.maximum_raw_overscroll * viewport.width),
            y: capped_axis(raw.y, maximum.y, physics.rubber_band.maximum_raw_overscroll * viewport.height),
        }
    }
}

fn axis_offset(raw: f32, maximum: f32, extent: f32, physics: ScrollPhysics) -> f32 {
    if !physics.rubber_band.is_enabled || !extent.is_finite() || extent <= 0.0 {
        return raw.clamp(0.0, maximum.max(0.0));
    }
    let excess = RubberBand::excess(raw, maximum);
    if excess == 0.0 {
        return raw;
    }
    let projected = RubberBand::project(excess, extent, physics.rubber_band.compliance);
    (if excess < 0.0 { 0.0 } else { maximum }) + projected
}
fn axis_position(visible: f32, maximum: f32, extent: f32, physics: ScrollPhysics) -> f32 {
    if !physics.rubber_band.is_enabled || !extent.is_finite() || extent <= 0.0 {
        return visible.clamp(0.0, maximum.max(0.0));
    }
    let excess = RubberBand::excess(visible, maximum);
    if excess == 0.0 {
        return visible;
    }
    let raw = RubberBand::unproject(excess, extent, physics.rubber_band.compliance);
    (if excess < 0.0 { 0.0 } else { maximum }) + raw
}
fn capped_axis(value: f32, maximum: f32, limit: f32) -> f32 {
    if !limit.is_finite() {
        return value.clamp(0.0, maximum.max(0.0));
    }
    let excess = RubberBand::excess(value, maximum);
    if excess.abs() <= limit.max(0.0) {
        return value;
    }
    let bounded = limit.max(0.0) * if excess < 0.0 { -1.0 } else { 1.0 };
    (if excess < 0.0 { 0.0 } else { maximum }) + bounded
}

fn monotonic_clock() -> f64 {
    crate::scene::monotonic_seconds()
}

impl AnyAnimationController for ScrollAnimator {
    fn is_finished(&self) -> bool {
        self.state.borrow().is_finished
    }
    fn tick(&self, delta_time: f64) {
        ScrollAnimator::tick(self, delta_time);
    }
    fn finish_immediately(&self) {
        ScrollAnimator::finish_immediately(self);
    }
    fn cancel(&self) {
        ScrollAnimator::cancel(self);
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Convenience: a default animator with no callbacks.
pub fn default_animator() -> Rc<ScrollAnimator> {
    ScrollAnimator::new(ScrollPhysics::DEFAULT, Point::default(), Size::default(), Point::default(), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_scrolling_clamps_to_the_range() {
        let animator = ScrollAnimator::new(ScrollPhysics::DEFAULT, Point { x: 100.0, y: 100.0 }, Size { width: 50.0, height: 50.0 }, Point::default(), None);
        assert_eq!(animator.scroll_directly(Point { x: 30.0, y: 40.0 }), Point { x: 30.0, y: 40.0 });
        assert_eq!(animator.scroll_directly(Point { x: 500.0, y: 500.0 }), Point { x: 100.0, y: 100.0 });
    }

    #[test]
    fn adopting_an_offset_resets_motion() {
        let animator = ScrollAnimator::new(ScrollPhysics::DEFAULT, Point { x: 100.0, y: 100.0 }, Size { width: 50.0, height: 50.0 }, Point::default(), None);
        animator.adopt(Point { x: 250.0, y: -50.0 });
        assert_eq!(animator.offset(), Point { x: 100.0, y: 0.0 });
        assert!(animator.is_finished());
    }

    #[test]
    fn shrinking_range_pulls_the_offset_back() {
        let animator = ScrollAnimator::new(ScrollPhysics::DEFAULT, Point { x: 100.0, y: 100.0 }, Size { width: 50.0, height: 50.0 }, Point { x: 90.0, y: 0.0 }, None);
        animator.update(Point { x: 40.0, y: 100.0 }, Size { width: 50.0, height: 50.0 });
        assert_eq!(animator.offset(), Point { x: 40.0, y: 0.0 });
    }

    #[test]
    fn overscroll_starts_the_return_spring() {
        let animator = ScrollAnimator::new(ScrollPhysics::DEFAULT, Point { x: 100.0, y: 0.0 }, Size { width: 100.0, height: 100.0 }, Point::default(), None);
        animator.scroll_by(Point { x: 140.0, y: 0.0 });
        assert!(animator.is_overscrolled());
        assert!(animator.is_settling());
        assert!(animator.offset().x > 100.0 && animator.offset().x < 140.0);
        for _ in 0..600 {
            animator.tick(1.0 / 60.0);
        }
        assert!(!animator.is_settling());
        assert_eq!(animator.offset().x, 100.0);
    }
}
