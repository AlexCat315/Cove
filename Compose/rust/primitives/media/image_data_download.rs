//! Translation of Primitives/Media/ImageDataDownload.swift.
//!
//! Transport metadata stays with the bytes until the decoder is selected. The
//! Swift version owns a `URLSession` delegate; Rust has no bundled HTTP client,
//! so the byte transport is supplied by the host through
//! [`ImageDownloadTransport`] while this module keeps the bounded-streaming,
//! cancellation and MIME-to-format policy identical.
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

/// Bytes plus the transport hint that selects a decoder.
#[derive(Clone, Debug, PartialEq)]
pub struct DownloadedImage {
    pub data: Vec<u8>,
    pub format_hint: String,
}

impl DownloadedImage {
    pub fn new(data: Vec<u8>, url: &str, mime_type: Option<&str>) -> Self {
        let mime = mime_type
            .and_then(|mime| mime.split(';').next())
            .map(|mime| mime.trim().to_ascii_lowercase());
        let format_hint = match mime.as_deref() {
            Some("image/svg+xml") => "svg".to_string(),
            Some("image/png") => "png".to_string(),
            Some("image/jpeg") => "jpg".to_string(),
            Some("image/gif") => "gif".to_string(),
            Some("image/webp") => "webp".to_string(),
            Some("image/bmp") => "bmp".to_string(),
            Some("image/tiff") => "tiff".to_string(),
            Some("image/heic") => "heic".to_string(),
            Some("image/heif") => "heif".to_string(),
            Some("application/pdf") => "pdf".to_string(),
            _ => path_extension(url),
        };
        Self { data, format_hint }
    }
}

fn path_extension(url: &str) -> String {
    let without_query = url.split(['?', '#']).next().unwrap_or(url);
    PathBuf::from(without_query)
        .extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

/// Bounds applied to one streaming request.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageLoadingPolicy {
    pub timeout: f64,
    pub maximum_download_bytes: usize,
    pub maximum_source_pixels: u64,
}

impl Default for ImageLoadingPolicy {
    fn default() -> Self {
        Self {
            timeout: 15.0,
            maximum_download_bytes: 20 * 1024 * 1024,
            maximum_source_pixels: 32_000_000,
        }
    }
}

impl ImageLoadingPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate(&mut self) {
        self.timeout = if self.timeout.is_finite() {
            self.timeout.max(0.1)
        } else {
            15.0
        };
        self.maximum_download_bytes = self.maximum_download_bytes.max(1);
        self.maximum_source_pixels = self.maximum_source_pixels.max(1);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AsyncImageError {
    #[error("Image host unavailable")]
    HostUnavailable,
    #[error("Unsupported image URL")]
    UnsupportedUrl,
    #[error("Invalid image response")]
    InvalidResponse,
    #[error("Image file not found")]
    FileNotFound,
    #[error("Could not read image file")]
    FileReadFailed,
    #[error("Image request returned HTTP {0}")]
    HttpStatus(u16),
    #[error("Image exceeds the download limit")]
    DownloadTooLarge,
    #[error("Could not decode image: {0}")]
    DecodeFailed(String),
    #[error("Could not load image: {0}")]
    NetworkFailed(String),
}

impl AsyncImageError {
    pub fn message(&self) -> String {
        self.to_string()
    }
}

/// Receives the bytes received so far, at most once per 64 KiB.
pub type DownloadProgress = Rc<dyn Fn(&[u8])>;

/// Host-supplied byte transport. Returns the full response body and declares
/// the MIME type so [`DownloadedImage`] can pick the decoder hint.
///
/// This trait is the host's responsibility: the workspace deliberately ships no
/// HTTP client, so no layer below the platform shell can open a socket. Each
/// host installs one implementation with
/// [`ImageDownloadTransportHolder::set`] (macOS/iOS `URLSession`, Windows
/// WinHTTP, Linux std + the platform TLS stack). Without an installed
/// transport, `http`/`https` loads fail with
/// [`AsyncImageError::HostUnavailable`]; `file:` URLs never need one.
pub trait ImageDownloadTransport {
    fn fetch(&self, url: &str, limit: usize) -> Result<(Vec<u8>, Option<String>), AsyncImageError>;
}

thread_local! {
    static TRANSPORT: RefCell<Option<Rc<dyn ImageDownloadTransport>>> = const { RefCell::new(None) };
}

pub struct ImageDownloadTransportHolder;
impl ImageDownloadTransportHolder {
    pub fn set(transport: Option<Rc<dyn ImageDownloadTransport>>) {
        TRANSPORT.with(|slot| *slot.borrow_mut() = transport);
    }
    pub fn current() -> Option<Rc<dyn ImageDownloadTransport>> {
        TRANSPORT.with(|slot| slot.borrow().clone())
    }
}

const PROGRESS_CHUNK_BYTES: usize = 64 * 1024;

/// A single bounded streaming request.
pub struct ImageDataDownload;

impl ImageDataDownload {
    pub fn load(
        url: &str,
        policy: &ImageLoadingPolicy,
        progress: Option<&DownloadProgress>,
    ) -> Result<DownloadedImage, AsyncImageError> {
        let mut policy = policy.clone();
        policy.validate();
        if url.starts_with("file:") {
            let path = url
                .trim_start_matches("file://")
                .trim_start_matches("file:");
            return Self::load_file(path, &policy);
        }
        let scheme = url.split(':').next().unwrap_or("").to_ascii_lowercase();
        if scheme != "http" && scheme != "https" {
            return Err(AsyncImageError::UnsupportedUrl);
        }
        let transport =
            ImageDownloadTransportHolder::current().ok_or(AsyncImageError::HostUnavailable)?;
        let (data, mime) = transport.fetch(url, policy.maximum_download_bytes)?;
        if data.len() > policy.maximum_download_bytes {
            return Err(AsyncImageError::DownloadTooLarge);
        }
        if let Some(progress) = progress {
            for end in (PROGRESS_CHUNK_BYTES..=data.len()).step_by(PROGRESS_CHUNK_BYTES) {
                progress(&data[..end]);
            }
        }
        Ok(DownloadedImage::new(data, url, mime.as_deref()))
    }

    fn load_file(
        path: &str,
        policy: &ImageLoadingPolicy,
    ) -> Result<DownloadedImage, AsyncImageError> {
        let metadata = std::fs::metadata(path).map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => AsyncImageError::FileNotFound,
            _ => AsyncImageError::FileReadFailed,
        })?;
        if metadata.len() > policy.maximum_download_bytes as u64 {
            return Err(AsyncImageError::DownloadTooLarge);
        }
        let data = std::fs::read(path).map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => AsyncImageError::FileNotFound,
            _ => AsyncImageError::FileReadFailed,
        })?;
        if data.len() > policy.maximum_download_bytes {
            return Err(AsyncImageError::DownloadTooLarge);
        }
        Ok(DownloadedImage {
            data,
            format_hint: path_extension(path),
        })
    }

    /// HTTP response metadata checks performed before a body is accepted.
    pub fn accepts_response(
        status: u16,
        expected_content_length: Option<u64>,
        limit: usize,
    ) -> Result<(), AsyncImageError> {
        if !(200..300).contains(&status) {
            return Err(AsyncImageError::HttpStatus(status));
        }
        if expected_content_length.is_some_and(|length| length > limit as u64) {
            return Err(AsyncImageError::DownloadTooLarge);
        }
        Ok(())
    }
}

/// Bounded body accumulator mirroring the delegate's 64 KiB progress gate.
pub struct DownloadProgressReporter {
    data: RefCell<Vec<u8>>,
    reported: RefCell<usize>,
    limit: usize,
    progress: Option<DownloadProgress>,
}

impl DownloadProgressReporter {
    pub fn new(limit: usize, progress: Option<DownloadProgress>) -> Arc<Self> {
        Arc::new(Self {
            data: RefCell::new(Vec::new()),
            reported: RefCell::new(0),
            limit,
            progress,
        })
    }

    /// Appends `chunk`; returns the received-so-far snapshot when the 64 KiB
    /// gate opened, so progressive formats can show a preview mid-download.
    pub fn receive(&self, chunk: &[u8]) -> Result<Option<Vec<u8>>, AsyncImageError> {
        let mut data = self.data.borrow_mut();
        if chunk.len() > self.limit.saturating_sub(data.len()) {
            return Err(AsyncImageError::DownloadTooLarge);
        }
        data.extend_from_slice(chunk);
        if self.progress.is_some() {
            let mut reported = self.reported.borrow_mut();
            if data.len() - *reported >= PROGRESS_CHUNK_BYTES {
                *reported = data.len();
                return Ok(Some(data.clone()));
            }
        }
        Ok(None)
    }

    pub fn received(&self) -> usize {
        self.data.borrow().len()
    }

    pub fn into_data(&self) -> Vec<u8> {
        self.data.borrow().clone()
    }
}
