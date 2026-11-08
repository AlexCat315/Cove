//! HTTP(S) byte-range input over `cove-http`, which brings native TLS on every
//! platform (Foundation, WinHTTP, OpenSSL elsewhere). The bundled FFmpeg is built
//! without TLS, so `http(s)://` sources are routed through this input.
//!
//! The first request is made lazily on the backend thread. Servers that answer
//! a range request with `206` are seekable; a plain `200` is streamed forward.
use crate::{CancellationToken, MediaError, MediaErrorKind, MediaInput, MediaResult};
use cove_http::{Client, ClientConfig, Request, Response};
use std::io::Read;
use std::sync::Mutex;
use std::time::Duration;

struct Connection {
    length: Option<u64>,
    seekable: bool,
    position: u64,
    body: Option<Box<dyn Read + Send>>,
}

/// A [`MediaInput`] reading an `http://` or `https://` resource.
pub struct HttpInput {
    client: Client,
    url: String,
    headers: Vec<(String, String)>,
    /// `None` until the first request is answered.
    connection: Mutex<Option<Connection>>,
}

impl HttpInput {
    pub fn new(url: impl Into<String>) -> MediaResult<Self> {
        let url = url.into();
        let lower = url.to_ascii_lowercase();
        if !(lower.starts_with("http://") || lower.starts_with("https://")) {
            return Err(MediaError::new(
                MediaErrorKind::InvalidInput,
                "HTTP input needs an http:// or https:// URL",
            ));
        }
        Ok(Self {
            client: Client::new(ClientConfig {
                timeout: Duration::from_secs(20),
                max_redirects: 8,
                user_agent: "Cove-Media/1".into(),
            }),
            url,
            headers: Vec::new(),
            connection: Mutex::new(None),
        })
    }

    /// Adds a request header, for example an authorization token or a referer.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    fn request(&self, from: u64) -> MediaResult<Response> {
        let mut request = Request::get(&self.url).header("Range", format!("bytes={from}-"));
        for (name, value) in &self.headers {
            request = request.header(name.clone(), value.clone());
        }
        let response = self
            .client
            .send(request)
            .map_err(|error| MediaError::new(MediaErrorKind::Source, error.to_string()))?;
        if response.status == 416 {
            return Err(MediaError::new(
                MediaErrorKind::Source,
                "HTTP range is beyond the end of the resource",
            ));
        }
        if !response.is_success() {
            return Err(MediaError::new(
                MediaErrorKind::Source,
                format!("HTTP {} for {}", response.status, self.url),
            ));
        }
        Ok(response)
    }

    /// Answers the first request and records the resource's length and range support.
    fn connect(&self, from: u64) -> MediaResult<Connection> {
        let response = self.request(from)?;
        let partial = response.status == 206;
        let total = response
            .header("content-range")
            .and_then(|range| range.rsplit('/').next())
            .and_then(|total| total.trim().parse::<u64>().ok());
        let length = if partial {
            total
        } else {
            response.content_length()
        };
        let seekable = partial
            || response
                .header("accept-ranges")
                .is_some_and(|ranges| ranges.eq_ignore_ascii_case("bytes"));
        Ok(Connection {
            length,
            seekable,
            position: if partial { from } else { 0 },
            body: Some(response.into_body()),
        })
    }

    fn with_connection<T>(
        &self,
        body: impl FnOnce(&Self, &mut Connection) -> MediaResult<T>,
    ) -> MediaResult<T> {
        let mut slot = self
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if slot.is_none() {
            *slot = Some(self.connect(0)?);
        }
        let connection = slot.as_mut().expect("connected above");
        body(self, connection)
    }
}

impl MediaInput for HttpInput {
    fn length(&self) -> Option<u64> {
        self.with_connection(|_, connection| Ok(connection.length))
            .ok()
            .flatten()
    }

    fn is_seekable(&self) -> bool {
        self.with_connection(|_, connection| Ok(connection.seekable))
            .unwrap_or(false)
    }

    fn read(&mut self, bytes: &mut [u8], cancellation: &CancellationToken) -> MediaResult<usize> {
        cancellation.check()?;
        self.with_connection(|input, connection| {
            if connection
                .length
                .is_some_and(|length| connection.position >= length)
            {
                return Ok(0);
            }
            if connection.body.is_none() {
                let response = input.request(connection.position)?;
                connection.body = Some(response.into_body());
            }
            let body = connection.body.as_mut().expect("opened above");
            let count = body
                .read(bytes)
                .map_err(|error| MediaError::new(MediaErrorKind::Source, error.to_string()))?;
            connection.position += count as u64;
            Ok(count)
        })
    }

    fn seek(&mut self, offset: u64, cancellation: &CancellationToken) -> MediaResult<()> {
        cancellation.check()?;
        self.with_connection(|_, connection| {
            if offset == connection.position {
                return Ok(());
            }
            if !connection.seekable {
                return Err(MediaError::new(
                    MediaErrorKind::Unsupported,
                    "the server does not accept byte ranges",
                ));
            }
            if connection.length.is_some_and(|length| offset > length) {
                return Err(MediaError::new(
                    MediaErrorKind::InvalidInput,
                    "source seek exceeds length",
                ));
            }
            // The next read opens a new range request at the target.
            connection.body = None;
            connection.position = offset;
            Ok(())
        })
    }
}

/// `http(s)://` URIs that are not playlists are read through [`HttpInput`].
pub(crate) fn route(source: crate::MediaSource) -> MediaResult<crate::MediaSource> {
    match source {
        crate::MediaSource::Uri(uri) if is_progressive_http(&uri) => {
            Ok(crate::MediaSource::Input(Box::new(HttpInput::new(uri)?)))
        }
        other => Ok(other),
    }
}

fn is_progressive_http(uri: &str) -> bool {
    let lower = uri.to_ascii_lowercase();
    let path = lower.split(['?', '#']).next().unwrap_or("");
    (lower.starts_with("http://") || lower.starts_with("https://")) && !path.ends_with(".m3u8")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    /// A one-file HTTP/1.1 server that honours `Range: bytes=N-`.
    fn serve(body: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let address = listener.local_addr().expect("address");
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream.try_clone().expect("clone"));
                let mut from = 0usize;
                let mut line = String::new();
                while reader.read_line(&mut line).is_ok_and(|n| n > 0) {
                    let lower = line.to_ascii_lowercase();
                    if let Some(range) = lower.strip_prefix("range: bytes=") {
                        from = range.trim().trim_end_matches('-').parse().unwrap_or(0);
                    }
                    if line == "\r\n" {
                        break;
                    }
                    line.clear();
                }
                let slice = &body[from.min(body.len())..];
                let head = format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {from}-{}/{}\r\nConnection: close\r\n\r\n",
                    slice.len(),
                    body.len().saturating_sub(1),
                    body.len()
                );
                let mut stream = stream;
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(slice);
            }
        });
        format!("http://{address}/clip.bin")
    }

    #[test]
    fn range_requests_seek_and_read() {
        let body: Vec<u8> = (0..=255u8).cycle().take(10_000).collect();
        let mut input = HttpInput::new(serve(body.clone())).expect("input");
        let token = CancellationToken::default();
        assert!(input.is_seekable());
        assert_eq!(input.length(), Some(10_000));
        let mut buffer = [0u8; 16];
        let count = input.read(&mut buffer, &token).expect("read");
        assert_eq!(&buffer[..count], &body[..count]);
        input.seek(9_000, &token).expect("seek");
        let count = input.read(&mut buffer, &token).expect("read");
        assert_eq!(&buffer[..count], &body[9_000..9_000 + count]);
        token.cancel();
        assert_eq!(
            input.read(&mut buffer, &token).expect_err("cancelled").kind,
            MediaErrorKind::Cancelled
        );
    }

    #[test]
    fn playlists_stay_with_the_backend() {
        assert!(is_progressive_http("https://example.com/a.mp4?x=1"));
        assert!(!is_progressive_http("https://example.com/live.m3u8"));
        assert!(!is_progressive_http("rtmp://example.com/live"));
    }
}
