//! Linux image transport.
//!
//! The workspace ships no HTTP or TLS client, so this backend speaks HTTP/1.1
//! over `std::net::TcpStream` directly. `http://` works end to end. `https://`
//! requires a TLS handshake this backend cannot perform without a crypto
//! provider; rather than silently downgrade or return corrupt bytes, it reports
//! [`AsyncImageError::NetworkFailed`] naming the limitation, and
//! [`super::capabilities`] advertises `https = false` so a caller can route
//! around it before attempting a load.

use super::core::{self, TransportIoError, Url};
use cove_ui_compose::primitives::media::{
    AsyncImageError, DownloadProgress, ImageDownloadTransport,
};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

pub const SCHEME_HTTP: bool = true;
pub const SCHEME_HTTPS: bool = false;

/// Why `https://` cannot be served on this platform build.
pub const HTTPS_LIMITATION: &str =
    "https requires a TLS provider; the workspace dependency set ships none, so \
     Linux image loading is limited to http:// and file://";

pub struct LinuxImageTransport {
    timeout: Duration,
}

impl Default for LinuxImageTransport {
    fn default() -> Self {
        Self { timeout: Duration::from_secs(15) }
    }
}

impl LinuxImageTransport {
    pub fn new(timeout_seconds: f64) -> Self {
        let seconds = if timeout_seconds.is_finite() { timeout_seconds.clamp(0.1, 300.0) } else { 15.0 };
        Self { timeout: Duration::from_secs_f64(seconds) }
    }

    fn fetch_once(&self, url: &Url, limit: usize) -> Result<(Vec<u8>, Option<String>), AsyncImageError> {
        let address = format!("{}:{}", url.host, url.port);
        let stream = TcpStream::connect_timeout(
            &address
                .to_socket_addrs()
                .map_err(TransportIoError::network)?
                .next()
                .ok_or_else(|| TransportIoError::network(format!("{address} resolved no address")))?,
            self.timeout,
        )
        .map_err(|error| TransportIoError::network(error.to_string()))?;
        stream
            .set_read_timeout(Some(self.timeout))
            .map_err(|error| TransportIoError::network(error.to_string()))?;
        stream
            .set_write_timeout(Some(self.timeout))
            .map_err(|error| TransportIoError::network(error.to_string()))?;

        let mut writer = stream
            .try_clone()
            .map_err(|error| TransportIoError::network(error.to_string()))?;
        writer
            .write_all(&core::build_get(url))
            .map_err(|error| TransportIoError::network(error.to_string()))?;

        let mut bytes = Vec::new();
        let mut reader = std::io::BufReader::new(stream);
        // Read the head first so the policy can reject an oversized body before
        // buffering it.
        let mut head = Vec::new();
        let mut byte = [0_u8; 1];
        loop {
            if reader.read_exact(&mut byte).is_err() {
                break;
            }
            head.push(byte[0]);
            if head.ends_with(b"\r\n\r\n") {
                break;
            }
            if head.len() > 64 * 1024 {
                return Err(AsyncImageError::InvalidResponse);
            }
        }
        let parsed = core::parse_head(head.strip_suffix(b"\r\n\r\n").unwrap_or(&head))?;
        if parsed.content_length.is_some_and(|length| length > limit) {
            return Err(AsyncImageError::DownloadTooLarge);
        }
        // Redirects are surfaced to the caller so the budget is enforced in one
        // place.
        if matches!(parsed.status, 301 | 302 | 303 | 307 | 308) {
            let location = parsed.location.ok_or(AsyncImageError::InvalidResponse)?;
            return Ok((location.into_bytes(), Some("__redirect__".to_owned())));
        }
        core::validate_status(parsed.status)?;

        let mut body = Vec::new();
        reader
            .by_ref()
            .take(limit as u64 + 1)
            .read_to_end(&mut body)
            .map_err(|error| TransportIoError::network(error.to_string()))?;
        let body = if parsed.chunked {
            let (decoded, complete) = core::decode_chunked(&body);
            if !complete {
                return Err(AsyncImageError::InvalidResponse);
            }
            decoded
        } else if let Some(length) = parsed.content_length {
            if body.len() < length {
                return Err(AsyncImageError::InvalidResponse);
            }
            body.truncate(length);
            body
        } else {
            body
        };
        bytes.extend_from_slice(&body);
        if bytes.len() > limit {
            return Err(AsyncImageError::DownloadTooLarge);
        }
        Ok((bytes, parsed.content_type))
    }
}

impl ImageDownloadTransport for LinuxImageTransport {
    fn fetch(&self, url: &str, limit: usize) -> Result<(Vec<u8>, Option<String>), AsyncImageError> {
        let mut current = Url::parse(url)?;
        for _ in 0..=core::MAXIMUM_REDIRECTS {
            if current.secure {
                return Err(AsyncImageError::NetworkFailed(HTTPS_LIMITATION.to_owned()));
            }
            let (body, content_type) = self.fetch_once(&current, limit)?;
            if content_type.as_deref() == Some("__redirect__") {
                let target = String::from_utf8(body).map_err(|_| AsyncImageError::InvalidResponse)?;
                current = Url::parse(&target).or_else(|_| {
                    // A relative Location resolves against the current authority.
                    Url::parse(&format!(
                        "{}://{}{}",
                        if current.secure { "https" } else { "http" },
                        current.host_header(),
                        if target.starts_with('/') { target.clone() } else { format!("/{target}") }
                    ))
                })?;
                continue;
            }
            return Ok((body, content_type));
        }
        Err(AsyncImageError::NetworkFailed("too many image redirects".to_owned()))
    }
}

/// Progress reporting is applied by `ImageDataDownload::load`, so this backend
/// only exposes the raw fetch. Kept as a named helper so the caller can pass the
/// same signature on every platform.
pub fn fetch_with_progress(
    transport: &dyn ImageDownloadTransport,
    url: &str,
    limit: usize,
    progress: Option<&DownloadProgress>,
) -> Result<(Vec<u8>, Option<String>), AsyncImageError> {
    let result = transport.fetch(url, limit)?;
    if let Some(progress) = progress {
        progress(&result.0);
    }
    Ok(result)
}
