//! Shared HTTP machinery for the host image transports.
//!
//! `ImageDataDownload` in `cove-ui-compose` deliberately ships no client: each
//! platform host installs an [`ImageDownloadTransport`]. Windows and Apple hand
//! the whole request to the system HTTP stack; Linux has no such stack in the
//! workspace dependency set, so this module provides a real HTTP/1.1 client over
//! `std::net` and the shared response parser every backend reports through.

use cove_ui_compose::primitives::media::image_data_download::AsyncImageError;

/// A parsed request target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Url {
    pub secure: bool,
    pub host: String,
    pub port: u16,
    pub path: String,
    pub authority: String,
}

impl Url {
    /// Parses an `http`/`https` URL. Rejects anything else, a missing host, and
    /// non-numeric or out-of-range ports.
    pub fn parse(url: &str) -> Result<Self, AsyncImageError> {
        let (scheme, rest) = url.split_once("://").ok_or(AsyncImageError::UnsupportedUrl)?;
        let secure = match scheme.to_ascii_lowercase().as_str() {
            "https" => true,
            "http" => false,
            _ => return Err(AsyncImageError::UnsupportedUrl),
        };
        // Strip any fragment; it is never sent on the wire.
        let rest = rest.split('#').next().unwrap_or(rest);
        let (authority, query) = match rest.find('/') {
            Some(index) => (&rest[..index], &rest[index..]),
            None => (rest, "/"),
        };
        // Userinfo is not supported: it would need an Authorization header and
        // the Swift transport never sent one.
        if authority.contains('@') {
            return Err(AsyncImageError::UnsupportedUrl);
        }
        let (host, port) = match authority.rfind(':') {
            // A bare IPv6 literal has multiple colons and no port.
            Some(index) if !authority[index..].contains(']') && authority.matches(':').count() == 1 => {
                let port = authority[index + 1..]
                    .parse::<u16>()
                    .map_err(|_| AsyncImageError::UnsupportedUrl)?;
                (&authority[..index], port)
            }
            _ => (authority, if secure { 443 } else { 80 }),
        };
        let host = host.trim_start_matches('[').trim_end_matches(']');
        if host.is_empty() {
            return Err(AsyncImageError::UnsupportedUrl);
        }
        let path = format!("/{}", query.trim_start_matches('/'));
        Ok(Self {
            secure,
            host: host.to_owned(),
            port,
            path,
            authority: authority.to_owned(),
        })
    }

    /// `host:port` for the `Host` header.
    pub fn host_header(&self) -> String {
        self.authority.clone()
    }
}

/// A parsed HTTP response head.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResponseHead {
    pub status: u16,
    pub content_length: Option<usize>,
    pub chunked: bool,
    pub content_type: Option<String>,
    pub location: Option<String>,
}

/// Splits a header block from its body at the first blank line.
fn split_head(bytes: &[u8]) -> Option<(&[u8], &[u8])> {
    let position = bytes.windows(4).position(|window| window == b"\r\n\r\n")?;
    Some((&bytes[..position], &bytes[position + 4..]))
}

/// Parses a response head into status, framing and metadata.
pub fn parse_head(head: &[u8]) -> Result<ResponseHead, AsyncImageError> {
    let text = String::from_utf8_lossy(head);
    let mut lines = text.lines();
    let status_line = lines.next().ok_or(AsyncImageError::InvalidResponse)?;
    let mut parts = status_line.split_whitespace();
    let version = parts.next().ok_or(AsyncImageError::InvalidResponse)?;
    if !version.starts_with("HTTP/") {
        return Err(AsyncImageError::InvalidResponse);
    }
    let status = parts
        .next()
        .ok_or(AsyncImageError::InvalidResponse)?
        .parse::<u16>()
        .map_err(|_| AsyncImageError::InvalidResponse)?;
    let mut content_length = None;
    let mut chunked = false;
    let mut content_type = None;
    let mut location = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else { continue };
        let value = value.trim();
        match name.trim().to_ascii_lowercase().as_str() {
            "content-length" => content_length = value.parse::<usize>().ok(),
            "transfer-encoding" => {
                chunked = value.to_ascii_lowercase().contains("chunked");
            }
            "content-type" => content_type = Some(value.to_owned()),
            "location" => location = Some(value.to_owned()),
            _ => {}
        }
    }
    Ok(ResponseHead { status, content_length, chunked, content_type, location })
}

/// Decodes a chunked body, returning the bytes decoded so far and whether the
/// terminating zero-length chunk was seen.
pub fn decode_chunked(body: &[u8]) -> (Vec<u8>, bool) {
    let mut result = Vec::new();
    let mut cursor = 0;
    loop {
        let Some(relative) = body[cursor..].windows(2).position(|window| window == b"\r\n") else {
            return (result, false);
        };
        let size_line = &body[cursor..cursor + relative];
        // Chunk extensions follow `;` and carry no data.
        let size_text = size_line
            .split(|byte| *byte == b';')
            .next()
            .unwrap_or(size_line);
        let Ok(size_text) = std::str::from_utf8(size_text) else {
            return (result, false);
        };
        let Ok(size) = usize::from_str_radix(size_text.trim(), 16) else {
            return (result, false);
        };
        cursor += relative + 2;
        if size == 0 {
            return (result, true);
        }
        if cursor + size > body.len() {
            result.extend_from_slice(&body[cursor..]);
            return (result, false);
        }
        result.extend_from_slice(&body[cursor..cursor + size]);
        cursor += size;
        // Trailing CRLF after each chunk's data.
        cursor += 2.min(body.len().saturating_sub(cursor));
    }
}

/// Asks the transport policy whether a response body is acceptable before the
/// caller keeps it.
pub fn validate_status(status: u16) -> Result<(), AsyncImageError> {
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(AsyncImageError::HttpStatus(status))
    }
}

/// Redirect budget, matching the bounded behaviour of the Swift URLSession
/// transport. A redirect loop must not hang a UI thread.
pub const MAXIMUM_REDIRECTS: usize = 5;

/// Builds the HTTP/1.1 request bytes for one GET.
pub fn build_get(url: &Url) -> Vec<u8> {
    format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: CoveHost/1\r\nAccept: */*\r\nAccept-Encoding: identity\r\nConnection: close\r\n\r\n",
        url.path,
        url.host_header()
    )
    .into_bytes()
}

impl From<std::io::Error> for TransportIoError {
    fn from(error: std::io::Error) -> Self {
        Self(error.to_string())
    }
}

/// I/O error wrapper so platform backends share one message shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportIoError(pub String);

impl TransportIoError {
    /// Maps to the `NetworkFailed` case the image loader reports.
    pub fn into_image_error(self) -> AsyncImageError {
        AsyncImageError::NetworkFailed(self.0)
    }

    pub fn network(detail: impl Into<String>) -> AsyncImageError {
        AsyncImageError::NetworkFailed(detail.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_secure_url_with_port() {
        let url = Url::parse("https://example.com:8443/a/b?c=d").unwrap();
        assert!(url.secure);
        assert_eq!(url.host, "example.com");
        assert_eq!(url.port, 8443);
        assert_eq!(url.path, "/a/b?c=d");
    }

    #[test]
    fn parses_plain_url_without_path() {
        let url = Url::parse("http://example.com").unwrap();
        assert!(!url.secure);
        assert_eq!(url.port, 80);
        assert_eq!(url.path, "/");
    }

    #[test]
    fn rejects_unknown_scheme() {
        assert_eq!(Url::parse("ftp://example.com"), Err(AsyncImageError::UnsupportedUrl));
    }

    #[test]
    fn parses_status_and_framing() {
        let head = b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: 42\r\n";
        let parsed = parse_head(head).unwrap();
        assert_eq!(parsed.status, 200);
        assert_eq!(parsed.content_length, Some(42));
        assert_eq!(parsed.content_type.as_deref(), Some("image/png"));
        assert!(!parsed.chunked);
    }

    #[test]
    fn decodes_chunked_body() {
        let body = b"5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n";
        let (decoded, complete) = decode_chunked(body);
        assert!(complete);
        assert_eq!(decoded, b"hello world");
    }

    #[test]
    fn reports_incomplete_chunked_body() {
        let (decoded, complete) = decode_chunked(b"5\r\nhello\r\n");
        assert!(!complete);
        assert_eq!(decoded, b"hello");
    }

    #[test]
    fn splits_head_from_body() {
        let (head, body) = split_head(b"HTTP/1.1 200 OK\r\nA: b\r\n\r\npayload").unwrap();
        assert_eq!(body, b"payload");
        assert!(head.starts_with(b"HTTP/1.1 200"));
    }
}
