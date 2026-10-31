//! Translation of NativeRHI/Vulkan: the concrete Vulkan backend.
//!
//! The module layout follows `Graphics/Sources/NativeRHI/Vulkan`: `sys` is the
//! handwritten FFI layer (no binding crate, driver loaded at runtime), `setup`
//! owns instance/device creation, and the remaining modules mirror the Swift
//! `Backend/`, `Resources/`, `Presentation/` and `Submission/` directories.
pub(crate) mod acceleration;
pub(crate) mod backend;
pub(crate) mod formats;
pub(crate) mod memory;
pub(crate) mod pipelines;
pub(crate) mod presentation;
pub(crate) mod registries;
pub(crate) mod rendering;
pub(crate) mod resources;
pub(crate) mod setup;
pub(crate) mod status;
pub(crate) mod submission;
pub(crate) mod sys;

pub use backend::{VulkanBackend, make_backend};
// Pure VSync policy: no handles, so it is unit-testable on any host. Editing this
// line? Keep it a re-export; the module itself stays crate-private.
pub use presentation::choose_presentation_mode;
