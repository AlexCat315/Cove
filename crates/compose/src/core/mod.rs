//! Compose level services shared by widgets.
pub mod log;
pub mod window_presentation;

pub use log::Logger;
pub use window_presentation::{
    FloatingWindowId, FloatingWindowRequest, WindowPresentation, WindowPresenter,
};
