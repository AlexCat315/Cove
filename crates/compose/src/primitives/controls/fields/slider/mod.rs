//! Slider control: axis geometry, stateful host and tick paint policy.
pub mod geometry;
pub mod host;
pub mod paint;
pub mod slider;

pub use geometry::{SliderAxis, SliderGeometry};
pub use host::SliderHost;
pub use paint::{SliderTick, ticks};
pub use slider::Slider;
