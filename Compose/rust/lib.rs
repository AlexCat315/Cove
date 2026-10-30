//! CoveUICompose: widgets, themes, modifiers, animation, scrolling and pets.
pub mod animation;
pub mod app_storage;
pub mod core;
pub mod core_exports;
pub mod foundation;
pub mod i18n;
pub mod modifiers;
pub mod observed;
pub mod pet;
pub mod primitives;
pub mod rendering;
pub mod resource_bundle;
pub mod scrolling;
pub mod theme;

pub use cove_composition::*;
pub use cove_core as scene_core;
pub use cove_core::scene::{with_animation, with_animation_completion, AnimationCompletionCriteria};
pub use resource_bundle::CoveUIComposeResourceBundle;
