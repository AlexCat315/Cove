//! Optional media integration. Control and retained output do not depend on a UI or GPU.
mod frame;
mod source;
mod types;
pub use frame::*;
pub use source::*;
pub use types::*;

#[cfg(all(feature = "gst", not(target_arch = "wasm32")))]
mod native;
#[cfg(all(feature = "gst", not(target_arch = "wasm32")))]
pub use native::{MediaContext, Player};
#[cfg(all(feature = "gst", not(target_arch = "wasm32")))]
pub mod gst;
