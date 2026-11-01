//! Geometry, owned draw buffers, observable state and animation primitives.
//!
//! The `UI`-prefixed names in `Core/Sources/CoveUICore` (Swift) are spelled without the
//! prefix here; the rename is deliberate and each pair is a straight port, not a
//! gap. `UILinearGradient` → [`LinearGradient`], `UIVertex` → [`Vertex`],
//! `UIRect` → [`Rect`], `UIClip` → [`Clip`], `UIRoundedRect` →
//! [`RoundedRect`], `UIShader` → [`UI_SHADER_WGSL`], `UIReveal` → [`Reveal`],
//! `LineBreaks` → [`line_break_opportunities`], `StateReadTracking` →
//! [`ReadScope`], `UIEnvironment` → [`Environment`], `UIBackdrop` →
//! [`Backdrop`], `UITextureAsset` → [`TextureAsset`], `DecodedImage` →
//! [`Bitmap`], `InputContext` → [`scene::PlatformInputContext`], `TextLayout` →
//! [`TextLayoutResult`] and `LayoutCalculationState` →
//! [`scene::LayoutNodeData`].
mod adaptive_environment;
mod analytic_clipper;
mod analytic_stroke;
mod bidi;
mod binding;
mod bitmap;
mod content_scale;
mod draw;
mod draw_list_resources;
mod draw_primitives;
mod dynamic_property;
mod font;
mod geometry;
mod geometry_kernels;
mod glyph_metrics;
mod line_breaks;
mod linear_gradient;
pub mod reactive;
mod rounded_clip;
pub mod scene;
mod shaped_glyph;
mod spring;
mod state;
mod state_read_tracking;
mod surface_shadow;
mod text_layout;
mod text_measure_slot;
mod text_scale;
mod ui_reveal;
mod ui_shader;

pub use adaptive_environment::*;
pub use analytic_clipper::GeometryAntialiasing;
pub use bidi::*;
pub use binding::*;
pub use bitmap::*;
pub use content_scale::*;
pub use draw::*;
pub use draw_list_resources::*;
pub use dynamic_property::*;
pub use font::*;
pub use geometry::*;
pub use glyph_metrics::*;
pub use line_breaks::*;
pub use linear_gradient::*;
pub use reactive::{AnySignal, Observer, Owner, Signal, SignalSender, batch, untrack};
pub use rounded_clip::*;
pub use scene::*;
pub use shaped_glyph::*;
pub use spring::*;
pub use state::*;
pub use state_read_tracking::*;
pub use surface_shadow::*;
pub use text_layout::*;
pub use text_measure_slot::*;
pub use text_scale::*;
pub use ui_reveal::*;
pub use ui_shader::*;
