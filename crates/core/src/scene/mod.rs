//! CoveUIScene: the retained scene tree, layout, input and animation layer.
mod accessibility;
mod ambient_slot;
mod animatable_property;
mod animatable_write;
pub mod animation;
mod component_diagnostics;
mod composition_local;
mod element_id;
mod environment_hooks;
mod flex_props;
mod image_metrics;
pub mod input;
mod invalidation_log;
mod layer_aware_node_renderer;
mod layout_node;
mod layout_pass;
mod node;
mod node_geometry;
mod node_renderer;
mod node_resource;
mod node_transform;
mod node_tree;
mod observable_object;
mod observable_state_tracking;
mod performance_timeline;
mod recomposer;
mod render_culling;
mod render_tree;
mod rendering;
pub mod scrolling;
mod state_registry;
mod surface_painting;
mod timing_trace;

pub use accessibility::*;
pub use ambient_slot::*;
pub use animatable_property::*;
pub use animation::*;
pub use component_diagnostics::*;
pub use composition_local::*;
pub use element_id::*;
pub use environment_hooks::*;
pub use flex_props::*;
pub use image_metrics::*;
pub use input::*;
pub use invalidation_log::*;
pub use layer_aware_node_renderer::*;
pub use layout_node::*;
pub use layout_pass::*;
pub use node::*;
pub use node_renderer::*;
pub use node_resource::*;
pub use node_transform::*;
pub use node_tree::*;
pub use observable_object::*;
pub use observable_state_tracking::*;
pub use performance_timeline::*;
pub use recomposer::*;
pub use render_tree::*;
pub use rendering::*;
pub use scrolling::*;
pub use state_registry::*;
pub use timing_trace::*;

mod local_invalidation;
pub use local_invalidation::*;

/// Draw-layer vocabulary that scene nodes, custom layouts and theme styles
/// consume without reaching back into the crate root.
pub use crate::{Color, DrawList, LayeredShadow, SurfaceShadow, TextureId, color_glyph_atlas_id};
/// Geometry is defined next to the draw layer but is the vocabulary scene
/// nodes, layout and composition use, so expose it from this module too.
pub use crate::{Point, Rect, Size};
