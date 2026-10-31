//! The Compose-side terminal surface: metrics, painting, routing and state.
pub mod blink_timer;
pub mod box_drawing;
pub mod cell_metrics;
pub mod image_textures;
pub mod key_routing;
pub mod link_opener;
pub mod painter;
pub mod pointer_router;
pub mod surface_state;
pub mod terminal_view;
pub mod url_detector;

pub use blink_timer::BlinkTimer;
pub use box_drawing::{Arms, arms};
pub use cell_metrics::CellMetrics;
pub use image_textures::ImageTextures;
pub use key_routing::KeyRouting;
pub use painter::TerminalPainter;
pub use pointer_router::{PointerGeometry, PointerRouter};
pub use surface_state::SurfaceState;
pub use terminal_view::TerminalView;
pub use url_detector::{UrlMatch, find as detect_url};
