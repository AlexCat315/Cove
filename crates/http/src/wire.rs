//! Header list plus the HTTP/1.1 framing the socket backend speaks.
#![cfg_attr(not(cove_http_socket), allow(dead_code))]

use crate::{HttpError, Url};
use std::io::{self, BufRead, Read};

/// Ordered header list; names compare case-insensitively and may repeat.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Headers(Vec<(String, String)>);

impl Headers {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn append(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.0.push((name.into(), value.into()));
    }

    /// Replaces every existing value of `name`.
    pub fn set(&mut self, name: impl Into<String>, value: impl Into<String>) {
        let name = name.into();
        self.remove(&name);
        self.0.push((name, value.into()));
    }

    pub fn remove(&mut self, name: &str) {
        self.0
            .retain(|(existing, _)| !existing.eq_ignore_ascii_case(name));
    }

    /// The first value of `name`.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(existing, _)| existing.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub fn get_all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.0
            .iter()
            .filter(move |(existing, _)| existing.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<N: Into<String>, V: Into<String>> FromIterator<(N, V)> for Headers {
    fn from_iter<I: IntoIterator<Item = (N, V)>>(iter: I) -> Self {
        Self(
            iter.into_iter()
                .map(|(name, value)| (name.into(), value.into()))
                .collect(),
        )
    }
}

/// Headers the socket backend owns; caller values for these are ignored.
const FRAMING: [&str; 4] = ["host", "content-length", "transfer-encoding", "connection"];

pub const MAX_HEAD_BYTES: usize = 64 * 1024;

/// Serialises one HTTP/1.1 request head. The body follows unchanged.
pub fn request_head(
    method: &str,
    url: &Url,
    headers: &Headers,
    body_length: usize,
    user_agent: &str,
) -> Vec<u8> {
    let mut head = format!(
        "{method} {} HTTP/1.1\r\nHost: {}\r\n",
        url.path, url.authority
    );
    if !headers.contains("user-agent") {
        head.push_str(&format!("User-Agent: {user_agent}\r\n"));
    }
    if !headers.contains("accept") {
        head.push_str("Accept: */*\r\n");
    }
    if !headers.contains("accept-encoding") {
        head.push_str("Accept-Encoding: identity\r\n");
    }
    for (name, value) in headers.iter() {
        if !FRAMING
            .iter()
            .any(|framing| framing.eq_ignore_ascii_case(name))
        {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
    }
    if body_length > 0 || !matches!(method, "GET" | "HEAD" | "DELETE" | "OPTIONS" | "TRACE") {
        head.push_str(&format!("Content-Length: {body_length}\r\n"));
    }
    head.push_str("Connection: close\r\n\r\n");
    head.into_bytes()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResponseHead {
    pub status: u16,
    pub headers: Headers,
}

/// Reads one response head, skipping interim `1xx` responses.
pub fn read_head(reader: &mut impl BufRead) -> Result<ResponseHead, HttpError> {
    loop {
        let mut total = 0;
        let mut line = Vec::new();
        let status_line = read_line(reader, &mut line, &mut total)?
            .ok_or_else(|| invalid("connection closed before the response"))?;
        let mut parts = status_line.splitn(3, ' ');
        let version = parts.next().unwrap_or_default();
        if !version.starts_with("HTTP/1.") {
            return Err(invalid("not an HTTP/1.x response"));
        }
        let status = parts
            .next()
            .filter(|code| code.len() == 3)
            .and_then(|code| code.parse::<u16>().ok())
            .filter(|code| (100..600).contains(code))
            .ok_or_else(|| invalid("bad status line"))?;
        let mut headers: Vec<(String, String)> = Vec::new();
        loop {
            let line = read_line(reader, &mut line, &mut total)?
                .ok_or_else(|| invalid("connection closed inside the response head"))?;
            if line.is_empty() {
                break;
            }
            if line.starts_with([' ', '\t']) {
                // Obsolete line folding continues the previous value.
                let (_, value) = headers
                    .last_mut()
                    .ok_or_else(|| invalid("folded first header"))?;
                value.push(' ');
                value.push_str(line.trim());
                continue;
            }
            let (name, value) = line
                .split_once(':')
                .ok_or_else(|| invalid("bad header line"))?;
            if name.is_empty() || name.ends_with([' ', '\t']) {
                return Err(invalid("bad header name"));
            }
            headers.push((name.to_owned(), value.trim().to_owned()));
        }
        // 101 Switching Protocols is final; it is never requested here.
        if (100..200).contains(&status) && status != 101 {
            continue;
        }
        return Ok(ResponseHead {
            status,
            headers: Headers(headers),
        });
    }
}

fn read_line(
    reader: &mut impl BufRead,
    buffer: &mut Vec<u8>,
    total: &mut usize,
) -> Result<Option<String>, HttpError> {
    buffer.clear();
    let limit = (MAX_HEAD_BYTES - *total) as u64;
    let read = reader
        .by_ref()
        .take(limit + 1)
        .read_until(b'\n', buffer)
        .map_err(crate::io_error)?;
    *total += read;
    if *total > MAX_HEAD_BYTES {
        return Err(invalid("response head exceeds 64 KiB"));
    }
    if read == 0 {
        return Ok(None);
    }
    if buffer.last() != Some(&b'\n') {
        return Err(invalid("connection closed inside the response head"));
    }
    buffer.pop();
    if buffer.last() == Some(&b'\r') {
        buffer.pop();
    }
    String::from_utf8(std::mem::take(buffer))
        .map(Some)
        .map_err(|_| invalid("response head is not UTF-8"))
}

fn invalid(detail: &str) -> HttpError {
    HttpError::InvalidResponse(detail.into())
}

/// How the body after a head is delimited (RFC 9112 §6.3).
pub fn body_reader<R: BufRead + Send + 'static>(
    method: &str,
    head: &ResponseHead,
    reader: R,
) -> Result<Box<dyn Read + Send>, HttpError> {
    if method == "HEAD" || head.status == 204 || head.status == 304 {
        return Ok(Box::new(io::empty()));
    }
    if let Some(encoding) = head.headers.get("transfer-encoding") {
        if encoding
            .rsplit(',')
            .next()
            .is_some_and(|last| last.trim().eq_ignore_ascii_case("chunked"))
        {
            return Ok(Box::new(Chunked::new(reader)));
        }
        return Ok(Box::new(reader));
    }
    let mut lengths = head.headers.get_all("content-length");
    if let Some(first) = lengths.next() {
        let length = first
            .trim()
            .parse::<u64>()
            .map_err(|_| invalid("bad Content-Length"))?;
        if lengths.any(|other| other.trim().parse::<u64>().ok() != Some(length)) {
            return Err(invalid("conflicting Content-Length"));
        }
        return Ok(Box::new(Exact {
            inner: reader,
            remaining: length,
        }));
    }
    Ok(Box::new(reader))
}

/// A `Content-Length` body: early EOF is an error, not a short body.
struct Exact<R> {
    inner: R,
    remaining: u64,
}

impl<R: Read> Read for Exact<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.remaining == 0 || buffer.is_empty() {
            return Ok(0);
        }
        let wanted = buffer
            .len()
            .min(usize::try_from(self.remaining).unwrap_or(usize::MAX));
        let read = self.inner.read(&mut buffer[..wanted])?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!("body ended {} bytes early", self.remaining),
            ));
        }
        self.remaining -= read as u64;
        Ok(read)
    }
}

/// A streaming `Transfer-Encoding: chunked` decoder.
pub struct Chunked<R> {
    inner: R,
    remaining: u64,
    done: bool,
}

impl<R: BufRead> Chunked<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            remaining: 0,
            done: false,
        }
    }

    fn line(&mut self) -> io::Result<String> {
        let mut line = Vec::new();
        self.inner
            .by_ref()
            .take(4097)
            .read_until(b'\n', &mut line)?;
        if line.last() != Some(&b'\n') {
            return Err(io::Error::new(
                if line.len() > 4096 {
                    io::ErrorKind::InvalidData
                } else {
                    io::ErrorKind::UnexpectedEof
                },
                "bad chunk framing",
            ));
        }
        String::from_utf8(line)
            .map(|line| line.trim_end_matches(['\r', '\n']).to_owned())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad chunk framing"))
    }
}

impl<R: BufRead> Read for Chunked<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.done || buffer.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            let line = self.line()?;
            let size = line.split(';').next().unwrap_or_default().trim();
            self.remaining = u64::from_str_radix(size, 16)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad chunk size"))?;
            if self.remaining == 0 {
                // Trailer fields end with an empty line.
                while !self.line()?.is_empty() {}
                self.done = true;
                return Ok(0);
            }
        }
        let wanted = buffer
            .len()
            .min(usize::try_from(self.remaining).unwrap_or(usize::MAX));
        let read = self.inner.read(&mut buffer[..wanted])?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "chunk ended early",
            ));
        }
        self.remaining -= read as u64;
        if self.remaining == 0 && !self.line()?.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bad chunk terminator",
            ));
        }
        Ok(read)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_all(mut reader: impl Read) -> io::Result<Vec<u8>> {
        let mut body = Vec::new();
        reader.read_to_end(&mut body)?;
        Ok(body)
    }

    #[test]
    fn writes_request_heads_with_owned_framing() {
        let url = Url::parse("http://a.test:8080/p?q").unwrap();
        let headers: Headers = [("Host", "evil"), ("X-Key", "1"), ("Accept", "text/plain")]
            .into_iter()
            .collect();
        let head = String::from_utf8(request_head("POST", &url, &headers, 3, "T/1")).unwrap();
        assert_eq!(
            head,
            "POST /p?q HTTP/1.1\r\nHost: a.test:8080\r\nUser-Agent: T/1\r\nAccept-Encoding: identity\r\nX-Key: 1\r\nAccept: text/plain\r\nContent-Length: 3\r\nConnection: close\r\n\r\n"
        );
        let get = String::from_utf8(request_head("GET", &url, &Headers::new(), 0, "T/1")).unwrap();
        assert!(!get.contains("Content-Length"));
    }

    #[test]
    fn parses_heads_and_skips_interim_responses() {
        let mut input = &b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 201 Created\r\nX-A: 1\r\nx-a:  2 \r\nFold: a\r\n b\r\n\r\nbody"[..];
        let head = read_head(&mut input).unwrap();
        assert_eq!(head.status, 201);
        assert_eq!(head.headers.get_all("X-A").collect::<Vec<_>>(), ["1", "2"]);
        assert_eq!(head.headers.get("fold"), Some("a b"));
        assert_eq!(input, b"body");
    }

    #[test]
    fn rejects_malformed_heads() {
        for input in [
            &b"FTP/1.0 200 OK\r\n\r\n"[..],
            b"HTTP/1.1 2000 OK\r\n\r\n",
            b"HTTP/1.1 200 OK\r\nbad\r\n\r\n",
            b"HTTP/1.1 200 OK\r\nX: 1\r\n",
            b"",
        ] {
            assert!(read_head(&mut &input[..]).is_err());
        }
        let mut huge = b"HTTP/1.1 200 OK\r\n".to_vec();
        huge.extend(std::iter::repeat_n(b'a', MAX_HEAD_BYTES));
        assert!(read_head(&mut &huge[..]).is_err());
    }

    #[test]
    fn decodes_chunked_bodies_incrementally() {
        let input = &b"5;ext=1\r\nhello\r\n6\r\n world\r\n0\r\nTrailer: x\r\n\r\nNEXT"[..];
        let mut reader = Chunked::new(input);
        let mut first = [0u8; 3];
        assert_eq!(reader.read(&mut first).unwrap(), 3);
        let mut rest = Vec::new();
        reader.read_to_end(&mut rest).unwrap();
        assert_eq!([&first[..], &rest[..]].concat(), b"hello world");
        assert!(read_all(Chunked::new(&b"5\r\nhel"[..])).is_err());
        assert!(read_all(Chunked::new(&b"zz\r\n"[..])).is_err());
    }

    #[test]
    fn content_length_bodies_detect_truncation() {
        let head = |headers: &[(&str, &str)]| ResponseHead {
            status: 200,
            headers: headers.iter().copied().collect(),
        };
        let body = body_reader("GET", &head(&[("Content-Length", "4")]), &b"abcdef"[..]).unwrap();
        assert_eq!(read_all(body).unwrap(), b"abcd");
        let short = body_reader("GET", &head(&[("Content-Length", "9")]), &b"abc"[..]).unwrap();
        assert!(read_all(short).is_err());
        assert!(
            body_reader(
                "GET",
                &head(&[("Content-Length", "1"), ("Content-Length", "2")]),
                &b""[..]
            )
            .is_err()
        );
        let none = body_reader("HEAD", &head(&[("Content-Length", "4")]), &b"abcd"[..]).unwrap();
        assert!(read_all(none).unwrap().is_empty());
        let eof = body_reader("GET", &head(&[]), &b"to the end"[..]).unwrap();
        assert_eq!(read_all(eof).unwrap(), b"to the end");
    }
}
