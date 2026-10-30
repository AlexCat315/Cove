//! Host safe-area insets and the surface root that publishes them.
pub mod safe_area;
pub mod surface_root;

pub use safe_area::{safe_area_insets, SafeArea, SafeAreaEdges, SafeAreaEnvironment};
pub use surface_root::SurfaceRoot;
