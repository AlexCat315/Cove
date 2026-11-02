//! Native application hosts for desktop, iOS and Android.
pub mod android;
pub mod application;
pub mod application_services;
pub mod desktop;

pub use application::{
    ApplicationCheckpoint, ApplicationLaunch, DiagnosticsCallbacks, DiagnosticsFactory,
    DiagnosticsSession,
};
pub use application_services::{http::TransportCapabilities, install_platform_services};
pub use desktop::AppDisplayHandle;
pub use desktop::application::configuration::ApplicationConfiguration;
pub use desktop::application::runtime::{Application, ApplicationCallbacks};
