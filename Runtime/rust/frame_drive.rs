use crate::FrameDeadline;
use std::cell::{Cell, RefCell};
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum PlatformFrameRateMode {
    #[default]
    EventDriven,
    DisplayRefresh,
    Fixed(f64),
}
pub(super) struct HostFrameClock {
    pub last_loop_time: Cell<f64>,
    pub last_preparation_time: Cell<Option<f64>>,
    pub last_animation_tick_time: Cell<f64>,
    pub animations_previously_active: Cell<bool>,
    pub fallback_deadline: RefCell<FrameDeadline>,
    pub reported_fallback: Cell<bool>,
}
impl HostFrameClock {
    pub fn new(now: f64) -> Self {
        Self {
            last_loop_time: Cell::new(now),
            last_preparation_time: Cell::new(None),
            last_animation_tick_time: Cell::new(now),
            animations_previously_active: Cell::new(false),
            fallback_deadline: RefCell::new(FrameDeadline::default()),
            reported_fallback: Cell::new(false),
        }
    }
}
pub(super) struct HostFrameDrive {
    pub mode: Cell<PlatformFrameRateMode>,
    pub clock: RefCell<std::rc::Rc<HostFrameClock>>,
}
impl Default for HostFrameDrive {
    fn default() -> Self {
        Self {
            mode: Cell::new(PlatformFrameRateMode::EventDriven),
            clock: RefCell::new(std::rc::Rc::new(HostFrameClock::new(0.0))),
        }
    }
}
