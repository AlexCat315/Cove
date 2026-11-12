//! Bounded in-process HTTP activity log for DevTools.
//!
//! The recorder sits on [`Client::send`](crate::Client::send), so the records
//! describe the requests this process actually issued rather than a proxy or a
//! screenshot of the system. It is enabled in debug builds and can be toggled
//! at runtime. Bodies are truncated; credentials in headers are redacted.

use crate::{Headers, Request};
use std::collections::VecDeque;
use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Oldest records are evicted after this many entries.
pub const MAX_RECORDS: usize = 256;
/// Per-body capture limit. Larger bodies are marked truncated.
pub const MAX_BODY_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Pending,
    Complete,
    Failed,
    Cancelled,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Complete => "complete",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Clone, Debug)]
pub struct NetworkRecord {
    pub id: u64,
    pub method: String,
    pub url: String,
    pub status: Option<u16>,
    pub error: Option<String>,
    /// Milliseconds since the Unix epoch when the request started.
    pub started_at_ms: f64,
    /// Time until the response head arrived.
    pub ttfb_ms: Option<f64>,
    /// Time until the body finished, was cancelled, or failed.
    pub duration_ms: f64,
    pub request_bytes: usize,
    pub response_bytes: u64,
    pub request_headers: Vec<(String, String)>,
    pub response_headers: Vec<(String, String)>,
    pub request_body: Vec<u8>,
    pub request_body_truncated: bool,
    pub response_body: Vec<u8>,
    pub response_body_truncated: bool,
    pub state: State,
}

struct Entry {
    record: NetworkRecord,
    started: Instant,
}

#[derive(Default)]
struct Log {
    entries: VecDeque<Entry>,
}

static LOG: OnceLock<Mutex<Log>> = OnceLock::new();
static ENABLED: AtomicBool = AtomicBool::new(cfg!(debug_assertions));
static CAPTURE_BODIES: AtomicBool = AtomicBool::new(cfg!(debug_assertions));
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn log() -> &'static Mutex<Log> {
    LOG.get_or_init(|| Mutex::new(Log::default()))
}

fn lock() -> std::sync::MutexGuard<'static, Log> {
    log().lock().unwrap_or_else(|error| error.into_inner())
}

/// Enable or disable recording. Disabled builds incur one atomic read per
/// request and keep the existing history until it is cleared.
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Capture request and response bodies up to [`MAX_BODY_BYTES`]. Metadata is
/// recorded regardless.
pub fn set_capture_bodies(capture: bool) {
    CAPTURE_BODIES.store(capture, Ordering::Relaxed);
}

pub fn clear() {
    lock().entries.clear();
}

pub fn snapshot() -> Vec<NetworkRecord> {
    lock()
        .entries
        .iter()
        .map(|entry| entry.record.clone())
        .collect()
}

fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64() * 1000.0)
        .unwrap_or_default()
}

fn redact(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "authorization" | "proxy-authorization" | "cookie" | "set-cookie" | "x-api-key" | "api-key"
    )
}

fn header_pairs(headers: &Headers) -> Vec<(String, String)> {
    headers
        .iter()
        .map(|(name, value)| {
            (
                name.to_owned(),
                if redact(name) {
                    "<redacted>".to_owned()
                } else {
                    value.to_owned()
                },
            )
        })
        .collect()
}

fn body_prefix(body: &[u8]) -> (Vec<u8>, bool) {
    if !CAPTURE_BODIES.load(Ordering::Relaxed) {
        return (Vec::new(), false);
    }
    let limit = body.len().min(MAX_BODY_BYTES);
    (body[..limit].to_vec(), body.len() > limit)
}

/// Record the start of a request. Returns `None` when recording is disabled.
pub fn begin(request: &Request) -> Option<u64> {
    if !is_enabled() {
        return None;
    }
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let (body, truncated) = body_prefix(&request.body);
    let record = NetworkRecord {
        id,
        method: request.method.clone(),
        url: request.url.clone(),
        status: None,
        error: None,
        started_at_ms: now_ms(),
        ttfb_ms: None,
        duration_ms: 0.0,
        request_bytes: request.body.len(),
        response_bytes: 0,
        request_headers: header_pairs(&request.headers),
        response_headers: Vec::new(),
        request_body: body,
        request_body_truncated: truncated,
        response_body: Vec::new(),
        response_body_truncated: false,
        state: State::Pending,
    };
    let mut log = lock();
    log.entries.push_front(Entry {
        record,
        started: Instant::now(),
    });
    while log.entries.len() > MAX_RECORDS {
        log.entries.pop_back();
    }
    Some(id)
}

fn update(id: Option<u64>, update: impl FnOnce(&mut Entry)) {
    let Some(id) = id else {
        return;
    };
    let mut log = lock();
    if let Some(entry) = log.entries.iter_mut().find(|entry| entry.record.id == id) {
        update(entry);
    }
}

/// Record the response head. The record stays pending until its body ends.
pub fn response(id: Option<u64>, status: u16, headers: Headers) {
    update(id, |entry| {
        entry.record.status = Some(status);
        entry.record.response_headers = header_pairs(&headers);
        entry.record.ttfb_ms = Some(entry.started.elapsed().as_secs_f64() * 1000.0);
    });
}

pub fn failure(id: Option<u64>, error: &str) {
    update(id, |entry| {
        entry.record.state = State::Failed;
        entry.record.error = Some(error.to_owned());
        entry.record.duration_ms = entry.started.elapsed().as_secs_f64() * 1000.0;
    });
}

fn body_chunk(id: u64, bytes: &[u8]) {
    update(Some(id), |entry| {
        entry.record.response_bytes += bytes.len() as u64;
        if !CAPTURE_BODIES.load(Ordering::Relaxed) {
            return;
        }
        if entry.record.response_body.len() >= MAX_BODY_BYTES {
            entry.record.response_body_truncated = true;
            return;
        }
        let room = MAX_BODY_BYTES - entry.record.response_body.len();
        if bytes.len() > room {
            entry.record.response_body.extend_from_slice(&bytes[..room]);
            entry.record.response_body_truncated = true;
        } else {
            entry.record.response_body.extend_from_slice(bytes);
        }
    });
}

fn body_end(id: u64, complete: bool) {
    update(Some(id), |entry| {
        entry.record.duration_ms = entry.started.elapsed().as_secs_f64() * 1000.0;
        if entry.record.state == State::Pending {
            entry.record.state = if complete {
                State::Complete
            } else {
                State::Cancelled
            };
        }
    });
}

fn body_error(id: u64, error: &str) {
    update(Some(id), |entry| {
        entry.record.duration_ms = entry.started.elapsed().as_secs_f64() * 1000.0;
        entry.record.state = State::Failed;
        entry.record.error = Some(error.to_owned());
    });
}

/// Wrap a streamed response body so its size and bytes reach the record.
pub(crate) fn wrap_body(body: Box<dyn Read + Send>, id: Option<u64>) -> Box<dyn Read + Send> {
    match id {
        Some(id) => Box::new(RecordedBody {
            inner: body,
            id,
            done: false,
        }),
        None => body,
    }
}

struct RecordedBody {
    inner: Box<dyn Read + Send>,
    id: u64,
    done: bool,
}

impl Read for RecordedBody {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match self.inner.read(buffer) {
            Ok(0) => {
                if !self.done {
                    self.done = true;
                    body_end(self.id, true);
                }
                Ok(0)
            }
            Ok(read) => {
                body_chunk(self.id, &buffer[..read]);
                Ok(read)
            }
            Err(error) => {
                if !self.done {
                    self.done = true;
                    body_error(self.id, &error.to_string());
                }
                Err(error)
            }
        }
    }
}

impl Drop for RecordedBody {
    fn drop(&mut self) {
        if !self.done {
            self.done = true;
            body_end(self.id, false);
        }
    }
}
