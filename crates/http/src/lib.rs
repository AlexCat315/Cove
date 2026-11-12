//! Blocking HTTP client with a streamed response body.
//!
//! Backends:
//! - macOS/iOS: `NSURLSession` (system proxy, trust, HTTP/2)
//! - Windows: WinHTTP
//! - Linux, Android and other Unix: HTTP/1.1 over `std::net`, TLS through the
//!   system OpenSSL (`libssl`), loaded at runtime by a small C binding
//!
//! No async runtime: `send` blocks the calling thread until the response head
//! arrives, and the body is read through [`std::io::Read`].
mod redirect;
mod url;
mod wire;

pub mod recording;

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod apple;
#[cfg(cove_http_socket)]
mod socket;
#[cfg(cove_http_socket)]
mod tls;
#[cfg(target_os = "windows")]
mod windows;

pub use url::Url;
pub use wire::Headers;

use std::io::Read;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum HttpError {
    #[error("unsupported URL: {0}")]
    UnsupportedUrl(String),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("HTTP is unavailable: {0}")]
    Unavailable(String),
    #[error("network failure: {0}")]
    Network(String),
    #[error("TLS failure: {0}")]
    Tls(String),
    #[error("request timed out")]
    Timeout,
    #[error("invalid response: {0}")]
    InvalidResponse(String),
    #[error("too many redirects")]
    TooManyRedirects,
    #[error("response body exceeds {0} bytes")]
    BodyTooLarge(usize),
}

impl From<HttpError> for std::io::Error {
    fn from(error: HttpError) -> Self {
        let kind = match error {
            HttpError::Timeout => std::io::ErrorKind::TimedOut,
            HttpError::InvalidResponse(_) => std::io::ErrorKind::InvalidData,
            _ => std::io::ErrorKind::Other,
        };
        std::io::Error::new(kind, error)
    }
}

fn io_error(error: std::io::Error) -> HttpError {
    if let Some(inner) = error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<HttpError>())
    {
        return inner.clone();
    }
    match error.kind() {
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => HttpError::Timeout,
        std::io::ErrorKind::UnexpectedEof | std::io::ErrorKind::InvalidData => {
            HttpError::InvalidResponse(error.to_string())
        }
        _ => HttpError::Network(error.to_string()),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientConfig {
    /// Connect timeout and the longest wait for any single read or write.
    pub timeout: Duration,
    /// Redirects followed before [`HttpError::TooManyRedirects`]; 0 returns
    /// the 3xx response itself.
    pub max_redirects: usize,
    pub user_agent: String,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_redirects: 5,
            user_agent: "Cove/1".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    pub url: String,
    pub headers: Headers,
    pub body: Vec<u8>,
}

impl Request {
    pub fn new(method: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            url: url.into(),
            headers: Headers::default(),
            body: Vec::new(),
        }
    }

    pub fn get(url: impl Into<String>) -> Self {
        Self::new("GET", url)
    }

    pub fn post(url: impl Into<String>, body: impl Into<Vec<u8>>) -> Self {
        Self::new("POST", url).body(body)
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.append(name, value);
        self
    }

    pub fn body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self
    }

    /// Rejects methods and headers that could split the request on the wire.
    fn validate(&self) -> Result<Url, HttpError> {
        let token = |text: &str| {
            !text.is_empty()
                && text
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
        };
        if !token(&self.method) {
            return Err(HttpError::InvalidRequest(format!(
                "method {:?}",
                self.method
            )));
        }
        for (name, value) in self.headers.iter() {
            if !token(name)
                || value
                    .bytes()
                    .any(|byte| byte == b'\r' || byte == b'\n' || byte == 0)
            {
                return Err(HttpError::InvalidRequest(format!("header {name:?}")));
            }
        }
        Url::parse(&self.url)
    }
}

pub struct Response {
    pub status: u16,
    pub headers: Headers,
    /// The URL that produced this response, after redirects.
    pub url: String,
    body: Box<dyn Read + Send>,
}

impl Response {
    pub(crate) fn new(
        status: u16,
        headers: Headers,
        url: String,
        body: Box<dyn Read + Send>,
    ) -> Self {
        Self {
            status,
            headers,
            url,
            body,
        }
    }

    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name)
    }

    pub fn content_length(&self) -> Option<u64> {
        self.header("content-length")?.trim().parse().ok()
    }

    /// The streamed body.
    pub fn body(&mut self) -> &mut (dyn Read + Send) {
        &mut *self.body
    }

    pub fn into_body(self) -> Box<dyn Read + Send> {
        self.body
    }

    /// Reads the whole body, failing once it exceeds `limit` bytes.
    pub fn bytes(mut self, limit: usize) -> Result<Vec<u8>, HttpError> {
        if self
            .content_length()
            .is_some_and(|length| length > limit as u64)
        {
            return Err(HttpError::BodyTooLarge(limit));
        }
        let mut body = Vec::new();
        (&mut self.body)
            .take(limit as u64 + 1)
            .read_to_end(&mut body)
            .map_err(io_error)?;
        if body.len() > limit {
            return Err(HttpError::BodyTooLarge(limit));
        }
        Ok(body)
    }
}

impl std::fmt::Debug for Response {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Response")
            .field("status", &self.status)
            .field("headers", &self.headers)
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}

/// What this build can reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities {
    pub http: bool,
    pub https: bool,
    /// Why a scheme is unavailable.
    pub limitation: Option<&'static str>,
}

#[derive(Clone, Debug, Default)]
pub struct Client {
    config: ClientConfig,
}

impl Client {
    pub fn new(config: ClientConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    pub fn send(&self, request: Request) -> Result<Response, HttpError> {
        let url = request.validate()?;
        let record = recording::begin(&request);
        match backend::send(&self.config, request, url) {
            Ok(mut response) => {
                recording::response(record, response.status, response.headers.clone());
                let body = std::mem::replace(&mut response.body, Box::new(std::io::empty()));
                response.body = recording::wrap_body(body, record);
                Ok(response)
            }
            Err(error) => {
                recording::failure(record, &error.to_string());
                Err(error)
            }
        }
    }

    pub fn capabilities() -> Capabilities {
        backend::capabilities()
    }
}

#[cfg(all(any(target_os = "macos", target_os = "ios"), not(cove_http_socket)))]
use apple as backend;
#[cfg(cove_http_socket)]
use socket as backend;
#[cfg(target_os = "windows")]
use windows as backend;

#[cfg(not(any(unix, windows)))]
mod backend {
    use super::*;

    pub fn send(_: &ClientConfig, _: Request, _: Url) -> Result<Response, HttpError> {
        Err(HttpError::Unavailable(
            "no HTTP stack on this target".into(),
        ))
    }

    pub fn capabilities() -> Capabilities {
        Capabilities {
            http: false,
            https: false,
            limitation: Some("no HTTP stack on this target"),
        }
    }
}
