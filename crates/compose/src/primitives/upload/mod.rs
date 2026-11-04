//! File upload: the request model, the host transport seam and the state machine.
//!
//! Compose ships no HTTP client. [`UploadTransport`] is the seam each host fills
//! (the same shape as the image download transport), so the widget layer stays
//! free of network policy. The state machine is pure: it decides what a control
//! shows, and the transport only reports what happened.
use std::sync::{Arc, RwLock};

pub mod content_type;
pub mod drag_upload;
pub mod multipart;

pub use content_type::{content_type_for, is_known};
pub use drag_upload::{DragUpload, FileReader};

use multipart::{MultipartBody, MultipartError, UploadField, UploadFilePart};

/// Everything one upload needs, independent of any widget.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadRequest {
    pub url: String,
    pub method: String,
    /// Extra request headers, in the order they are sent.
    pub headers: Vec<(String, String)>,
    /// The form field every dropped file is attached to.
    pub field: String,
    /// Non-file form fields sent alongside the files.
    pub fields: Vec<UploadField>,
    pub files: Vec<UploadFilePart>,
    /// Refuse a body larger than this before it is built.
    pub maximum_bytes: usize,
}

impl UploadRequest {
    pub fn new(url: impl Into<String>, files: Vec<UploadFilePart>) -> Self {
        Self {
            url: url.into(),
            method: "POST".into(),
            headers: Vec::new(),
            field: "file".into(),
            fields: Vec::new(),
            files,
            maximum_bytes: DEFAULT_MAXIMUM_BYTES,
        }
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.field = field.into();
        self
    }

    pub fn form_field(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.push(UploadField::new(name, value));
        self
    }

    pub fn maximum_bytes(mut self, maximum_bytes: usize) -> Self {
        self.maximum_bytes = maximum_bytes.max(1);
        self
    }

    /// Total size of the file payloads, before framing.
    pub fn payload_bytes(&self) -> u64 {
        self.files
            .iter()
            .map(|file| file.bytes.len() as u64)
            .sum::<u64>()
    }

    /// Whether the file payloads alone already exceed the limit.
    pub fn is_oversized(&self) -> bool {
        self.payload_bytes() > self.maximum_bytes as u64
    }

    /// Encodes the body, or reports why it cannot be sent.
    pub fn encode(&self, boundary: &str) -> Result<MultipartBody, MultipartError> {
        multipart::encode(boundary, &self.fields, &self.files, self.maximum_bytes)
    }
}

/// 64 MiB: large enough for a handful of images or a short video, small enough
/// that one drop cannot exhaust memory before the transport sees it.
pub const DEFAULT_MAXIMUM_BYTES: usize = 64 * 1024 * 1024;

/// What the transport did with a request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadResponse {
    pub status: u16,
    /// The body the server returned, already bounded by the transport.
    pub body: Vec<u8>,
}

impl UploadResponse {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// Why an upload failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum UploadError {
    #[error("no upload transport is installed for this platform")]
    HostUnavailable,
    #[error("the payload exceeds {limit} bytes")]
    PayloadTooLarge { limit: usize },
    #[error("the request could not be encoded: {0}")]
    Encoding(#[from] MultipartError),
    #[error("the network request failed: {0}")]
    Network(String),
    #[error("the server answered with status {status}")]
    HttpStatus { status: u16 },
    #[error("the upload was cancelled")]
    Cancelled,
}

/// Host-supplied byte transport for uploads.
///
/// Runs on a worker thread, which is why it must be `Send + Sync`. `report`
/// receives the bytes accepted so far and is called as the body is handed to the
/// network, so a control can show real progress instead of a spinner.
pub trait UploadTransport: Send + Sync {
    fn send(
        &self,
        request: &UploadRequest,
        report: &(dyn Fn(u64) + Send + Sync),
    ) -> Result<UploadResponse, UploadError>;
}

static TRANSPORT: RwLock<Option<Arc<dyn UploadTransport>>> = RwLock::new(None);

pub struct UploadTransportHolder;
impl UploadTransportHolder {
    pub fn set(transport: Option<Arc<dyn UploadTransport>>) {
        *TRANSPORT
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = transport;
    }
    pub fn current() -> Option<Arc<dyn UploadTransport>> {
        TRANSPORT
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
    pub fn is_installed() -> bool {
        Self::current().is_some()
    }
}

/// What a control shows for one upload.
#[derive(Clone, Debug, PartialEq)]
pub enum UploadState {
    Idle,
    /// The body is being read and encoded off the UI thread.
    Preparing,
    Uploading {
        sent: u64,
        total: u64,
    },
    Succeeded(UploadResponse),
    Failed(UploadError),
}

impl UploadState {
    pub fn is_busy(&self) -> bool {
        matches!(self, Self::Preparing | Self::Uploading { .. })
    }

    /// Fraction complete in `0…1`; `None` while indeterminate.
    pub fn progress(&self) -> Option<f64> {
        match self {
            Self::Uploading { sent, total } if *total > 0 => {
                Some((*sent as f64 / *total as f64).clamp(0.0, 1.0))
            }
            Self::Succeeded(_) => Some(1.0),
            _ => None,
        }
    }

    /// The message a control shows under the progress bar.
    pub fn status_text(&self) -> Option<String> {
        match self {
            Self::Idle => None,
            Self::Preparing => Some("Preparing…".into()),
            Self::Uploading { sent, total } if *total > 0 => {
                Some(format!("Uploading {sent} / {total}"))
            }
            Self::Uploading { .. } => Some("Uploading…".into()),
            Self::Succeeded(_) => Some("Uploaded".into()),
            Self::Failed(error) => Some(error.to_string()),
        }
    }

    /// Whether the transport the host installed can serve this build.
    pub fn is_supported() -> bool {
        UploadTransportHolder::is_installed()
    }
}

/// Every terminal state, so a caller can tell success from any failure without
/// matching the error itself.
pub fn outcome(state: &UploadState) -> Option<Result<u16, &UploadError>> {
    match state {
        UploadState::Succeeded(response) => Some(Ok(response.status)),
        UploadState::Failed(error) => Some(Err(error)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct FakeTransport {
        calls: AtomicU64,
        fail: bool,
    }

    impl UploadTransport for FakeTransport {
        fn send(
            &self,
            request: &UploadRequest,
            report: &(dyn Fn(u64) + Send + Sync),
        ) -> Result<UploadResponse, UploadError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            report(request.payload_bytes());
            if self.fail {
                return Err(UploadError::HttpStatus { status: 500 });
            }
            Ok(UploadResponse {
                status: 201,
                body: b"ok".to_vec(),
            })
        }
    }

    fn part() -> UploadFilePart {
        UploadFilePart::new("file", "a.png", "image/png", vec![1, 2, 3])
    }

    #[test]
    fn a_request_reports_payload_bytes_and_size() {
        let request = UploadRequest::new("https://example.test/u", vec![part()]);
        assert_eq!(request.payload_bytes(), 3);
        assert!(!request.is_oversized());
        assert!(request.maximum_bytes(2).is_oversized());
    }

    #[test]
    fn the_transport_seam_is_installed_and_removed() {
        let transport = Arc::new(FakeTransport {
            calls: AtomicU64::new(0),
            fail: false,
        });
        UploadTransportHolder::set(Some(transport.clone()));
        assert!(UploadState::is_supported());
        let seen = UploadTransportHolder::current().expect("installed");
        let response = seen
            .send(&UploadRequest::new("https://x", vec![part()]), &|_| {})
            .expect("sends");
        assert_eq!(response.status, 201);
        assert_eq!(transport.calls.load(Ordering::SeqCst), 1);
        UploadTransportHolder::set(None);
        assert!(!UploadState::is_supported());
    }

    #[test]
    fn progress_is_a_fraction_only_while_determinate() {
        assert_eq!(UploadState::Idle.progress(), None);
        assert_eq!(UploadState::Preparing.progress(), None);
        assert_eq!(
            UploadState::Uploading { sent: 5, total: 10 }.progress(),
            Some(0.5)
        );
        assert_eq!(
            UploadState::Uploading { sent: 5, total: 0 }.progress(),
            None
        );
        assert_eq!(
            UploadState::Succeeded(UploadResponse {
                status: 200,
                body: vec![]
            })
            .progress(),
            Some(1.0)
        );
    }

    #[test]
    fn a_failure_is_terminal_and_keeps_its_message() {
        let state = UploadState::Failed(UploadError::HttpStatus { status: 503 });
        assert!(!state.is_busy());
        assert_eq!(
            outcome(&state),
            Some(Err(&UploadError::HttpStatus { status: 503 }))
        );
        assert_eq!(
            state.status_text().as_deref(),
            Some("the server answered with status 503")
        );
    }

    #[test]
    fn a_success_reports_its_status() {
        let state = UploadState::Succeeded(UploadResponse {
            status: 201,
            body: b"created".to_vec(),
        });
        assert_eq!(outcome(&state), Some(Ok(201)));
        assert_eq!(state.status_text().as_deref(), Some("Uploaded"));
    }
}
