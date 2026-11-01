//! Pet state machine and one-clock entrance/jump timelines.
use super::{JumpArc, PetPose, PetRenderState, PetSpriteSheet, PetTrip, clamp01};
use cove_core::scene::{
    AnimationCurve, AnimatorScheduler, AnyAnimationController, ObservableObject,
    ObservablePublisher,
};
use cove_core::{Point, Rect};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Copy)]
struct Entrance {
    duration: f64,
}

/// Owns the pet's current anchors, intent and deterministic animation state.
pub struct PetWorld {
    sprite_sheet: PetSpriteSheet,
    travel_speed: Cell<f64>,
    minimum_jump_duration: Cell<f64>,
    anchors: RefCell<HashMap<String, Rect>>,
    current_anchor_id: RefCell<String>,
    perch_pose: Cell<PetPose>,
    entrance: Cell<Option<Entrance>>,
    entrance_anchor: RefCell<Option<String>>,
    entrance_elapsed: Cell<f64>,
    has_entered: Cell<bool>,
    trip: RefCell<Option<PetTrip>>,
    trip_elapsed: Cell<f64>,
    render: Cell<PetRenderState>,
    finished: Cell<bool>,
    random_state: Cell<u64>,
    publisher: ObservablePublisher,
}

impl PetWorld {
    pub fn new(sprite_sheet: PetSpriteSheet, initial_anchor_id: impl Into<String>) -> Rc<Self> {
        Self::with_pose(sprite_sheet, initial_anchor_id, PetPose::Idle)
    }

    pub fn with_pose(
        sprite_sheet: PetSpriteSheet,
        initial_anchor_id: impl Into<String>,
        perch_pose: PetPose,
    ) -> Rc<Self> {
        Rc::new(Self {
            sprite_sheet,
            travel_speed: Cell::new(430.0),
            minimum_jump_duration: Cell::new(0.58),
            anchors: RefCell::new(HashMap::new()),
            current_anchor_id: RefCell::new(initial_anchor_id.into()),
            perch_pose: Cell::new(perch_pose),
            entrance: Cell::new(None),
            entrance_anchor: RefCell::new(None),
            entrance_elapsed: Cell::new(0.0),
            has_entered: Cell::new(false),
            trip: RefCell::new(None),
            trip_elapsed: Cell::new(0.0),
            render: Cell::new(PetRenderState::new(
                Point::default(),
                perch_pose,
                1.0,
                1.0,
                1.0,
                0.0,
            )),
            finished: Cell::new(true),
            random_state: Cell::new(0x9e37_79b9_7f4a_7c15),
            publisher: ObservablePublisher::default(),
        })
    }

    pub fn sprite_sheet(&self) -> &PetSpriteSheet {
        &self.sprite_sheet
    }
    pub fn set_travel_speed(&self, points_per_second: f64) {
        if points_per_second.is_finite() && points_per_second > 0.0 {
            self.travel_speed.set(points_per_second);
        }
    }
    pub fn set_minimum_jump_duration(&self, seconds: f64) {
        if seconds.is_finite() && seconds > 0.0 {
            self.minimum_jump_duration.set(seconds);
        }
    }
    pub fn anchors(&self) -> HashMap<String, Rect> {
        self.anchors.borrow().clone()
    }
    pub fn current_anchor_id(&self) -> String {
        self.current_anchor_id.borrow().clone()
    }
    pub fn perch_pose(&self) -> PetPose {
        self.perch_pose.get()
    }
    pub fn render(&self) -> PetRenderState {
        self.render.get()
    }
    pub fn is_busy(&self) -> bool {
        self.trip.borrow().is_some() || self.entrance.get().is_some()
    }

    pub fn go(self: &Rc<Self>, target_id: &str) -> bool {
        let current_id = self.current_anchor_id.borrow().clone();
        if self.is_busy() || target_id == current_id {
            return false;
        }
        let (Some(start), Some(end)) = (self.perch_point(&current_id), self.perch_point(target_id))
        else {
            return false;
        };
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let distance = dx.hypot(dy).max(1.0);
        let peak = 64.0_f32.max(distance * 0.34 + dy.abs() * 0.15);
        let facing = if dx >= 0.0 { 1.0 } else { -1.0 };
        let duration = self
            .minimum_jump_duration
            .get()
            .max(f64::from(distance) / self.travel_speed.get());
        *self.trip.borrow_mut() = Some(PetTrip::new(
            current_id,
            target_id,
            JumpArc::new(start, end, peak),
            duration,
            facing,
        ));
        self.trip_elapsed.set(0.0);
        self.start_clock();
        true
    }

    pub fn toggle_rest(&self) {
        if self.is_busy() {
            return;
        }
        let pose = if self.perch_pose.get() == PetPose::Sit {
            PetPose::Idle
        } else {
            PetPose::Sit
        };
        self.perch_pose.set(pose);
        let mut render = self.render.get();
        render.pose = pose;
        self.render.set(render);
        self.publisher.send();
    }

    pub fn wander(self: &Rc<Self>) -> Option<String> {
        let current = self.current_anchor_id.borrow().clone();
        let candidates: Vec<String> = self
            .anchors
            .borrow()
            .keys()
            .filter(|key| **key != current)
            .cloned()
            .collect();
        if candidates.is_empty() {
            return None;
        }
        let mut state = self.random_state.get();
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        self.random_state.set(state);
        let index = (state as usize) % candidates.len();
        let target = candidates[index].clone();
        self.go(&target).then_some(target)
    }

    pub fn update_anchor(self: &Rc<Self>, id: impl Into<String>, frame: Rect) {
        let id = id.into();
        let changed = self.anchors.borrow().get(&id).copied() != Some(frame);
        self.anchors.borrow_mut().insert(id.clone(), frame);
        if !self.has_entered.get()
            && id == *self.current_anchor_id.borrow()
            && frame != Rect::default()
        {
            self.begin_entrance(id);
            return;
        }
        if !self.is_busy()
            && id == *self.current_anchor_id.borrow()
            && changed
            && let Some(point) = self.perch_point(&id)
        {
            let mut render = self.render.get();
            render.position = point;
            self.render.set(render);
            self.publisher.send();
        }
    }

    pub fn remove_anchor(&self, id: &str) {
        self.anchors.borrow_mut().remove(id);
    }

    pub fn tick_world(&self, delta_time: f64) {
        let mut changed = false;
        if let Some(entrance) = self.entrance.get() {
            self.entrance_elapsed
                .set(self.entrance_elapsed.get() + delta_time);
            let normalized = (self.entrance_elapsed.get() / entrance.duration) as f32;
            if normalized >= 1.0 {
                self.finish_entrance();
            } else {
                let anchor_id = self.entrance_anchor.borrow().clone();
                let point = anchor_id
                    .as_deref()
                    .and_then(|id| self.perch_point(id))
                    .unwrap_or(self.render.get().position);
                self.render.set(Self::evaluate_entrance(
                    normalized,
                    point,
                    self.perch_pose.get(),
                ));
            }
            changed = true;
        } else if let Some(trip) = self.trip.borrow().clone() {
            self.trip_elapsed.set(self.trip_elapsed.get() + delta_time);
            let normalized = (self.trip_elapsed.get() / trip.total_duration) as f32;
            if normalized >= 1.0 {
                self.finish_trip(&trip);
            } else {
                self.render.set(Self::evaluate_trip(&trip, normalized));
            }
            changed = true;
        }
        if changed {
            self.publisher.send();
        }
        self.stop_clock_if_idle();
    }

    pub fn finish_immediately(&self) {
        if self.entrance.get().is_some() {
            self.finish_entrance();
        }
        if let Some(trip) = self.trip.borrow().clone() {
            self.finish_trip(&trip);
        }
        self.publisher.send();
        self.stop_clock_if_idle();
    }

    pub fn cancel(&self) {
        self.entrance.set(None);
        *self.entrance_anchor.borrow_mut() = None;
        self.trip.borrow_mut().take();
        self.stop_clock_if_idle();
    }

    fn start_clock(self: &Rc<Self>) {
        if !self.finished.replace(false) {
            return;
        }
        let controller: Rc<dyn AnyAnimationController> = self.clone();
        AnimatorScheduler::current().register(controller);
    }
    fn stop_clock_if_idle(&self) {
        if !self.is_busy() {
            self.finished.set(true);
        }
    }
    fn begin_entrance(self: &Rc<Self>, anchor_id: String) {
        self.has_entered.set(true);
        self.entrance.set(Some(Entrance { duration: 0.6 }));
        *self.entrance_anchor.borrow_mut() = Some(anchor_id);
        self.entrance_elapsed.set(0.0);
        let mut render = self.render.get();
        render.pose = self.perch_pose.get();
        self.render.set(render);
        self.start_clock();
    }
    fn finish_entrance(&self) {
        self.entrance.set(None);
        let anchor_id = self.entrance_anchor.borrow_mut().take();
        let mut render = self.render.get();
        if let Some(point) = anchor_id.as_deref().and_then(|id| self.perch_point(id)) {
            render.position = point;
        }
        render.scale_x = 1.0;
        render.scale_y = 1.0;
        render.presence = 1.0;
        render.pose = self.perch_pose.get();
        self.render.set(render);
    }
    fn finish_trip(&self, trip: &PetTrip) {
        self.trip.borrow_mut().take();
        *self.current_anchor_id.borrow_mut() = trip.to_anchor.clone();
        self.perch_pose.set(PetPose::Sit);
        let mut render = self.render.get();
        render.pose = PetPose::Sit;
        render.scale_x = 1.0;
        render.scale_y = 1.0;
        render.facing = trip.facing;
        render.presence = 1.0;
        if let Some(point) = self.perch_point(&trip.to_anchor) {
            render.position = point;
        }
        self.render.set(render);
    }
    fn perch_point(&self, anchor_id: &str) -> Option<Point> {
        let frame = *self.anchors.borrow().get(anchor_id)?;
        Some(Point::new(frame.x + frame.width / 2.0, frame.y))
    }
    fn evaluate_entrance(normalized: f32, point: Point, pose: PetPose) -> PetRenderState {
        let progress = clamp01(normalized);
        let envelope = (-6.0 * progress).exp() * (9.0 * progress).cos();
        let scale = 1.0 - 0.7 * envelope;
        let presence = clamp01((progress - 0.05) / 0.25);
        PetRenderState::new(point, pose, scale, scale, 1.0, presence)
    }
    fn evaluate_trip(trip: &PetTrip, normalized: f32) -> PetRenderState {
        let time = clamp01(normalized);
        let arc = trip.arc;
        if time < PetTrip::WINDUP_END {
            let windup = clamp01(time / PetTrip::WINDUP_END);
            let eased = AnimationCurve::EaseIn.evaluate(windup);
            let x_scale = 1.0 + 0.26 * eased;
            let y_scale = 1.0 - 0.26 * eased;
            return PetRenderState::new(
                arc.start,
                PetPose::Windup,
                x_scale,
                y_scale,
                trip.facing,
                1.0,
            );
        }
        if time < PetTrip::AIRBORNE_END {
            let progress = trip.air_progress(time).unwrap_or(1.0);
            let point = arc.position(progress);
            let velocity = arc.vertical_velocity(progress);
            let maximum_velocity = arc
                .vertical_velocity(0.0)
                .abs()
                .max(arc.vertical_velocity(1.0).abs())
                .max(1.0);
            let stretch = clamp01(velocity.abs() / maximum_velocity);
            let pose = if velocity < -0.18 * maximum_velocity {
                PetPose::Rise
            } else if velocity > 0.18 * maximum_velocity {
                PetPose::Fall
            } else {
                PetPose::Apex
            };
            return PetRenderState::new(
                point,
                pose,
                1.0 - 0.10 * stretch,
                1.0 + 0.13 * stretch,
                trip.facing,
                1.0,
            );
        }
        if time < PetTrip::LAND_END {
            let progress = clamp01(
                (time - PetTrip::AIRBORNE_END) / (PetTrip::LAND_END - PetTrip::AIRBORNE_END),
            );
            let amount = 1.0 - 0.25 * AnimationCurve::EaseOut.evaluate(progress);
            return PetRenderState::new(
                arc.end,
                PetPose::Land,
                1.0 + 0.30 * amount,
                1.0 - 0.30 * amount,
                trip.facing,
                1.0,
            );
        }
        let settle = clamp01((time - PetTrip::LAND_END) / (1.0 - PetTrip::LAND_END));
        let envelope = (-5.5 * settle).exp() * (7.5 * settle).cos();
        PetRenderState::new(
            arc.end,
            PetPose::Sit,
            1.0 + 0.30 * envelope,
            1.0 - 0.30 * envelope,
            trip.facing,
            1.0,
        )
    }
}

impl ObservableObject for PetWorld {
    fn register_observer(&self, handler: Rc<dyn Fn()>) -> u64 {
        self.publisher.register_observer(handler)
    }
    fn unregister_observer(&self, token: u64) {
        self.publisher.unregister_observer(token);
    }
}

impl AnyAnimationController for PetWorld {
    fn is_finished(&self) -> bool {
        self.finished.get()
    }
    fn tick(&self, delta_time: f64) {
        self.tick_world(delta_time);
    }
    fn finish_immediately(&self) {
        PetWorld::finish_immediately(self);
    }
    fn cancel(&self) {
        PetWorld::cancel(self);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
