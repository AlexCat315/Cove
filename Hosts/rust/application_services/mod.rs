//! Host-supplied services the UI layers require but cannot own themselves.
//!
//! These are the seams `cove-ui-compose` leaves to the platform: byte transport
//! for remote images, and the single installation point a platform app calls
//! before it starts its run loop.

pub mod http;

/// Installs every platform service the UI layers expect.
///
/// Called by the desktop host before `Application::run` and by the Android
/// bridge before the first frame. Returns the transport capability report so a
/// caller can log what it will be able to load.
pub fn install_platform_services() -> http::TransportCapabilities {
    let capabilities = http::TransportCapabilities::platform();
    let timeout =
        cove_ui_compose::primitives::media::image_data_download::ImageLoadingPolicy::default()
            .timeout;
    let installed = http::install_image_transport(timeout);
    cove_logging::LogSystem::logger("com.cove.ui.host").log(&cove_logging::LogEvent::new(
        cove_logging::Level::Info,
        match capabilities.limitation {
            Some(limitation) => format!(
                "image transport installed={installed} http={} https={} file={} limitation={limitation}",
                capabilities.http, capabilities.https, capabilities.file
            ),
            None => format!(
                "image transport installed={installed} http={} https={} file={}",
                capabilities.http, capabilities.https, capabilities.file
            ),
        },
    ));
    capabilities
}
