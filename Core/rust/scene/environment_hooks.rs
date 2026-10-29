//! Translation of CoveUIScene/{ReducedMotion,UIWorkScheduler,WindowReveal}.swift.
use super::{CompositionLocal, Node};
use crate::{Rect, Reveal};
use std::cell::RefCell;
use std::rc::Rc;

/// Application-owned accessibility preference. A nearer provider may override
/// an ancestor; loading indicators keep a visible static form when enabled.
pub struct ReducedMotionEnvironment;
impl ReducedMotionEnvironment {
    pub fn key() -> &'static CompositionLocal<bool> {
        static KEY: std::sync::OnceLock<CompositionLocal<bool>> = std::sync::OnceLock::new();
        KEY.get_or_init(|| CompositionLocal::constant(false))
    }
}
impl Node {
    /// `true` when this node's nearest provider disables motion.
    pub fn prefers_reduced_motion(&self) -> bool {
        self.composition_value(ReducedMotionEnvironment::key())
    }
}

/// Process-wide bridge for work that must run on the UI run-loop thread.
pub struct UIWorkSchedulerHolder;
impl UIWorkSchedulerHolder {
    thread_local! {
        static ENQUEUE: RefCell<Option<Rc<dyn Fn(Rc<dyn Fn()>)>>> = const { RefCell::new(None) };
    }
    pub fn set_enqueue(enqueue: Option<Rc<dyn Fn(Rc<dyn Fn()>)>>) {
        Self::ENQUEUE.with(|slot| *slot.borrow_mut() = enqueue);
    }
    pub fn schedule(work: Rc<dyn Fn()>) {
        Self::ENQUEUE.with(|slot| {
            if let Some(enqueue) = slot.borrow().as_ref() {
                enqueue(work);
            }
        });
    }
}

/// A window's reveal state: the frame-level compositing request plus the
/// sequence number that lets a layer tell a new reveal from a replayed one.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WindowReveal {
    pub sequence: u64,
    pub center_x: f32,
    pub center_y: f32,
    pub radius: f32,
    pub feather: f32,
}
impl WindowReveal {
    pub fn new(center_x: f32, center_y: f32, radius: f32) -> Self {
        Self { sequence: 0, center_x, center_y, radius, feather: 0.0 }
    }
    /// Build the frame-level request the renderer reads while recording.
    pub fn request(&mut self) -> Reveal {
        self.sequence = self.sequence.wrapping_add(1);
        Reveal::new(self.sequence, self.center_x, self.center_y, self.radius, self.feather)
    }
    pub fn covers(&self, rect: Rect) -> bool {
        let dx = (rect.x + rect.width / 2.0 - self.center_x).abs();
        let dy = (rect.y + rect.height / 2.0 - self.center_y).abs();
        dx * dx + dy * dy <= self.radius * self.radius
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn reveal_requests_carry_increasing_sequences() {
        let mut reveal = WindowReveal::new(10.0, 10.0, 40.0);
        assert_eq!(reveal.request().sequence, 1);
        assert_eq!(reveal.request().sequence, 2);
        assert!(reveal.covers(Rect::new(0.0, 0.0, 20.0, 20.0)));
        assert!(!reveal.covers(Rect::new(200.0, 200.0, 10.0, 10.0)));
    }

    #[test]
    fn reduced_motion_defaults_to_false_and_honours_providers() {
        let node = Node::new();
        assert!(!node.prefers_reduced_motion());
        node.set_composition_value(ReducedMotionEnvironment::key(), true);
        assert!(node.prefers_reduced_motion());
    }

    #[test]
    fn work_scheduler_forwards_to_the_host() {
        let seen = Rc::new(Cell::new(0));
        let counter = seen.clone();
        UIWorkSchedulerHolder::set_enqueue(Some(Rc::new(move |work: Rc<dyn Fn()>| work())));
        UIWorkSchedulerHolder::schedule(Rc::new(move || counter.set(counter.get() + 1)));
        assert_eq!(seen.get(), 1);
        UIWorkSchedulerHolder::set_enqueue(None);
    }
}
