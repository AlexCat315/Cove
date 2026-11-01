//! Host safe-area insets and the surface root that publishes them.
pub mod safe_area;
pub mod surface_root;

pub use safe_area::{SafeArea, SafeAreaEdges, SafeAreaEnvironment, safe_area_insets};
pub use surface_root::SurfaceRoot;
