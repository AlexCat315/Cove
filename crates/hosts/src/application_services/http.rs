//! Image byte transport over `cove-http`.
//!
//! `cove-ui-compose` ships no HTTP client; its `ImageDownloadTransport` trait
//! is the seam each host fills. Every platform uses the same adapter: the
//! client picks NSURLSession, WinHTTP or crates/http/1.1 with OpenSSL.

use cove_http::{Client, ClientConfig, HttpError, Request};
use cove_ui_compose::primitives::media::image_data_download::{
    AsyncImageError, ImageDownloadTransport, ImageDownloadTransportHolder,
};
use std::rc::Rc;
use std::time::Duration;

/// What the platform transport can serve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransportCapabilities {
    pub http: bool,
    pub https: bool,
    /// `file:` URLs load without a transport (`ImageDataDownload::load`).
    pub file: bool,
    /// Why a scheme is unavailable.
    pub limitation: Option<&'static str>,
}

impl TransportCapabilities {
    pub fn platform() -> Self {
        let capabilities = Client::capabilities();
        Self {
            http: capabilities.http,
            https: capabilities.https,
            file: true,
            limitation: capabilities.limitation,
        }
    }
}

pub struct HttpImageTransport {
    client: Client,
}

impl HttpImageTransport {
    pub fn new(timeout_seconds: f64) -> Self {
        let seconds = if timeout_seconds.is_finite() {
            timeout_seconds.clamp(0.1, 300.0)
        } else {
            15.0
        };
        Self {
            client: Client::new(ClientConfig {
                timeout: Duration::from_secs_f64(seconds),
                max_redirects: 5,
                user_agent: "CoveHost/1".into(),
            }),
        }
    }
}

fn image_error(error: HttpError) -> AsyncImageError {
    match error {
        HttpError::UnsupportedUrl(_) | HttpError::InvalidRequest(_) => {
            AsyncImageError::UnsupportedUrl
        }
        HttpError::Unavailable(_) => AsyncImageError::HostUnavailable,
        HttpError::InvalidResponse(_) => AsyncImageError::InvalidResponse,
        HttpError::BodyTooLarge(_) => AsyncImageError::DownloadTooLarge,
        other => AsyncImageError::NetworkFailed(other.to_string()),
    }
}

impl ImageDownloadTransport for HttpImageTransport {
    fn fetch(&self, url: &str, limit: usize) -> Result<(Vec<u8>, Option<String>), AsyncImageError> {
        let response = self
            .client
            .send(Request::get(url).header("Accept", "image/*,*/*;q=0.8"))
            .map_err(image_error)?;
        if !response.is_success() {
            return Err(AsyncImageError::HttpStatus(response.status));
        }
        let content_type = response.header("content-type").map(str::to_owned);
        let bytes = response.bytes(limit).map_err(image_error)?;
        Ok((bytes, content_type))
    }
}

/// Installs the image transport process-wide (for this UI thread). Returns
/// `false` on a target with no HTTP stack, leaving `http`/`https` loads
/// reporting `HostUnavailable`.
pub fn install_image_transport(timeout_seconds: f64) -> bool {
    let installed = Client::capabilities().http;
    let transport: Option<Rc<dyn ImageDownloadTransport>> =
        installed.then(|| Rc::new(HttpImageTransport::new(timeout_seconds)) as _);
    ImageDownloadTransportHolder::set(transport);
    installed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_client_errors_to_image_errors() {
        assert_eq!(
            image_error(HttpError::UnsupportedUrl("x".into())),
            AsyncImageError::UnsupportedUrl
        );
        assert_eq!(
            image_error(HttpError::BodyTooLarge(1)),
            AsyncImageError::DownloadTooLarge
        );
        assert!(matches!(
            image_error(HttpError::Timeout),
            AsyncImageError::NetworkFailed(_)
        ));
    }

    #[test]
    fn rejects_non_http_urls_before_any_io() {
        let transport = HttpImageTransport::new(1.0);
        assert_eq!(
            transport.fetch("ftp://example.com/a.png", 1024),
            Err(AsyncImageError::UnsupportedUrl)
        );
    }
}
