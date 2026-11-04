//! File upload transport over `cove-http`.
//!
//! Mirrors [`crate::application_services::http`] for downloads: the compose
//! layer owns the `UploadTransport` trait and this adapter supplies the bytes.
//! The body is a single `Vec<u8>`, which is why `UploadRequest::maximum_bytes`
//! bounds what a drop can carry.

use cove_http::{Client, ClientConfig, HttpError, Request};
use cove_ui_compose::primitives::upload::{
    UploadError, UploadRequest, UploadResponse, UploadTransport, UploadTransportHolder,
};
use std::sync::Arc;
use std::time::Duration;

/// Ceiling on the response body an upload reads back. Servers answer a upload
/// with a small JSON receipt; anything larger is not a receipt.
const MAXIMUM_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

pub struct HttpUploadTransport {
    client: Client,
}

impl HttpUploadTransport {
    pub fn new(timeout_seconds: f64) -> Self {
        let seconds = if timeout_seconds.is_finite() {
            timeout_seconds.clamp(0.1, 300.0)
        } else {
            60.0
        };
        Self {
            client: Client::new(ClientConfig {
                timeout: Duration::from_secs_f64(seconds),
                max_redirects: 0,
                user_agent: "CoveHost/1".into(),
            }),
        }
    }

    /// The request this upload would send, without touching the network.
    ///
    /// Shared with the tests and with a host that wants to log or intercept the
    /// attempt before it leaves the process.
    pub fn build_request(request: &UploadRequest) -> Result<Request, UploadError> {
        if request.is_oversized() {
            return Err(UploadError::PayloadTooLarge {
                limit: request.maximum_bytes,
            });
        }
        let encoded = request.encode(DEFAULT_BOUNDARY)?;
        let mut built = Request::new(request.method.clone(), request.url.clone())
            .header("Content-Type", encoded.content_type.clone())
            .header("Content-Length", encoded.body.len().to_string())
            .header("Accept", "application/json, */*;q=0.5")
            .body(encoded.body);
        for (name, value) in &request.headers {
            built = built.header(name.clone(), value.clone());
        }
        Ok(built)
    }
}

/// Fixed boundary: the request is built once per attempt and a caller retrying
/// the same drop should produce the same bytes.
const DEFAULT_BOUNDARY: &str = "cove-upload-boundary";

fn upload_error(error: HttpError) -> UploadError {
    match error {
        HttpError::Timeout => UploadError::Network("the request timed out".into()),
        HttpError::BodyTooLarge(_) => UploadError::PayloadTooLarge {
            limit: MAXIMUM_RESPONSE_BYTES,
        },
        other => UploadError::Network(other.to_string()),
    }
}

impl UploadTransport for HttpUploadTransport {
    fn send(
        &self,
        request: &UploadRequest,
        report: &(dyn Fn(u64) + Send + Sync),
    ) -> Result<UploadResponse, UploadError> {
        let built = Self::build_request(request)?;
        // The client sends the body in one write, so progress reaches its
        // ceiling before the response arrives.
        report(request.payload_bytes());
        let response = self.client.send(built).map_err(upload_error)?;
        let status = response.status;
        let body = response
            .bytes(MAXIMUM_RESPONSE_BYTES)
            .map_err(upload_error)?;
        Ok(UploadResponse { status, body })
    }
}

/// Installs the upload transport process-wide. Returns `false` on a target with
/// no HTTP stack, leaving `DragUpload` disabled instead of silently failing.
pub fn install_upload_transport(timeout_seconds: f64) -> bool {
    let installed = Client::capabilities().http;
    let transport: Option<Arc<dyn UploadTransport>> =
        installed.then(|| Arc::new(HttpUploadTransport::new(timeout_seconds)) as _);
    UploadTransportHolder::set(transport);
    installed
}

#[cfg(test)]
mod tests {
    use super::*;
    use cove_ui_compose::primitives::upload::multipart::UploadFilePart;

    fn request() -> UploadRequest {
        UploadRequest::new(
            "https://example.test/upload",
            vec![UploadFilePart::new(
                "file",
                "a.png",
                "image/png",
                vec![1, 2, 3],
            )],
        )
        .header("Authorization", "Bearer t")
    }

    #[test]
    fn a_request_carries_the_multipart_headers() {
        let built = HttpUploadTransport::build_request(&request()).expect("builds");
        assert_eq!(built.method, "POST");
        assert_eq!(built.url, "https://example.test/upload");
        assert_eq!(
            built.headers.get("content-type"),
            Some("multipart/form-data; boundary=cove-upload-boundary")
        );
        // The length must describe the body that is actually sent, not a
        // constant that drifts when the encoder changes.
        assert_eq!(
            built.headers.get("content-length"),
            Some(built.body.len().to_string().as_str())
        );
        assert_eq!(built.headers.get("authorization"), Some("Bearer t"));
    }

    #[test]
    fn an_oversized_payload_is_rejected_before_encoding() {
        let request = request().maximum_bytes(2);
        assert_eq!(
            HttpUploadTransport::build_request(&request),
            Err(UploadError::PayloadTooLarge { limit: 2 })
        );
    }

    #[test]
    fn a_http_failure_maps_to_a_network_error() {
        assert!(matches!(
            upload_error(HttpError::Timeout),
            UploadError::Network(message) if message.contains("timed out")
        ));
        assert_eq!(
            upload_error(HttpError::BodyTooLarge(9)),
            UploadError::PayloadTooLarge {
                limit: MAXIMUM_RESPONSE_BYTES
            }
        );
    }
}
