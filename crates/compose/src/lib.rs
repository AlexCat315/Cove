//! CoveUICompose: widgets, themes, modifiers, animation, scrolling and pets.
//!
//! Widget builders keep Swift's long parameter lists, and module files intentionally
//! repeat the component name (`calendar/calendar.rs`).
#![allow(
    clippy::large_enum_variant,
    clippy::module_inception,
    clippy::needless_range_loop,
    clippy::too_many_arguments,
    clippy::type_complexity
)]
pub mod animation;
pub mod app_storage;
pub mod core;
pub mod core_exports;
pub mod foundation;
pub mod i18n;
pub mod modifiers;
pub mod observed;
pub mod pet;
pub mod prelude;
pub mod primitives;
pub mod rendering;
pub mod resource_bundle;
pub mod scrolling;
pub mod theme;

pub use cove_composition::layout::*;
pub use cove_composition::*;
pub use cove_core as scene_core;
pub use cove_core::scene::{
    AnimationCompletionCriteria, with_animation, with_animation_completion,
};
pub use cove_core::scene::{LayoutNode, Node};
/// Observable state and scene vocabulary that widgets reference through the
/// crate root, matching the Swift module's flat namespace.
pub use cove_core::{Signal, State, batch, untrack};
pub use modifiers::layout::FrameDimension;
pub use observed::Observed;
pub use primitives::layout::scroll_view::{
    ScrollAxis, ScrollGeometry, ScrollRequest, ScrollView, ScrollViewport, ScrollbarGutter,
};
pub use resource_bundle::CoveUIComposeResourceBundle;
