//! Per-platform HTTP byte transports for host-installed image loading.
//!
//! `cove-ui-compose` deliberately ships no HTTP client; its
//! `ImageDownloadTransport` trait is the seam each host fills. This module
//! provides one implementation per platform plus the capability report a caller
//! uses to route around a scheme a platform cannot serve.

pub mod core;

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod apple;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "windows")]
pub mod windows;

use cove_ui_compose::primitives::media::image_data_download::ImageDownloadTransport;
use std::rc::Rc;

/// What a platform transport can serve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransportCapabilities {
    pub http: bool,
    pub https: bool,
    /// `true` when `file:` URLs load without any transport at all. This is
    /// always the case: `ImageDataDownload::load` handles `file:` itself.
    pub file: bool,
    /// Non-empty when a scheme is unavailable, naming the reason.
    pub limitation: Option<&'static str>,
}

impl TransportCapabilities {
    pub const PLATFORM: Self = Self::for_current_platform();

    const fn for_current_platform() -> Self {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            Self {
                http: true,
                https: true,
                file: true,
                limitation: None,
            }
        }
        #[cfg(target_os = "windows")]
        {
            Self {
                http: true,
                https: true,
                file: true,
                limitation: None,
            }
        }
        #[cfg(target_os = "linux")]
        {
            Self {
                http: true,
                https: false,
                file: true,
                limitation: Some(linux::HTTPS_LIMITATION),
            }
        }
        #[cfg(not(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "windows",
            target_os = "linux"
        )))]
        {
            Self {
                http: false,
                https: false,
                file: true,
                limitation: Some("no platform HTTP transport is available on this target"),
            }
        }
    }
}

/// Builds this platform's transport for the given request timeout.
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub fn platform_transport(timeout_seconds: f64) -> Option<Rc<dyn ImageDownloadTransport>> {
    Some(Rc::new(apple::AppleImageTransport::new(timeout_seconds)))
}

#[cfg(target_os = "windows")]
pub fn platform_transport(timeout_seconds: f64) -> Option<Rc<dyn ImageDownloadTransport>> {
    Some(Rc::new(windows::WindowsImageTransport::new(
        timeout_seconds,
    )))
}

#[cfg(target_os = "linux")]
pub fn platform_transport(timeout_seconds: f64) -> Option<Rc<dyn ImageDownloadTransport>> {
    Some(Rc::new(linux::LinuxImageTransport::new(timeout_seconds)))
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "windows",
    target_os = "linux"
)))]
pub fn platform_transport(_timeout_seconds: f64) -> Option<Rc<dyn ImageDownloadTransport>> {
    None
}

/// Installs this platform's image transport process-wide.
///
/// Returns `false` on a target with no HTTP stack, leaving `http`/`https` loads
/// reporting `HostUnavailable` exactly as before installation.
pub fn install_image_transport(timeout_seconds: f64) -> bool {
    let transport = platform_transport(timeout_seconds);
    let installed = transport.is_some();
    cove_ui_compose::primitives::media::image_data_download::ImageDownloadTransportHolder::set(
        transport,
    );
    installed
}
