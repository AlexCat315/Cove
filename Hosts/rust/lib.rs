//! Application hosts for desktop and Android. The iOS app entry lives with the
//! mobile example content, mirroring the Swift package layout.
pub mod android;
pub mod application_services;
pub mod desktop;

pub use application_services::{http::TransportCapabilities, install_platform_services};
pub use desktop::AppDisplayHandle;
pub use desktop::application::configuration::ApplicationConfiguration;
pub use desktop::application::runtime::{Application, ApplicationCallbacks};
