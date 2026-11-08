//! DevTools server: configuration, host adapters and protocol dispatch.
use cove_devtools_protocol::{
    Envelope, HelloHostInfo, HelloPayload, InputRecording, MirrorActivate, MirrorInput,
    MirrorStart, StateObservation, TimelineSnapshot, TreeSnapshot, WindowResize,
};
use cove_logging::{Level, LogQuery, LogRecord, LogSystem};

/// Configuration for the in-process DevTools WebSocket server.
#[derive(Clone, Debug, PartialEq)]
pub struct DevToolsConfig {
    pub host: String,
    pub port: u16,
    pub app_title: String,
    pub enabled: bool,
    pub launch: Option<cove_devtools_protocol::DevToolsLaunch>,
}

impl Default for DevToolsConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 9229,
            app_title: "CoveUI".into(),
            enabled: true,
            launch: None,
        }
    }
}

impl DevToolsConfig {
    pub fn new(
        host: impl Into<String>,
        port: u16,
        app_title: impl Into<String>,
        enabled: bool,
    ) -> Self {
        Self {
            host: host.into(),
            port,
            app_title: app_title.into(),
            enabled,
            launch: None,
        }
    }

    /// Enable the server when `COVE_DEVTOOLS=1`. Debug builds only, matching the
    /// Swift `#if !DEBUG` gate.
    pub fn from_environment(app_title: &str) -> Option<Self> {
        if !cfg!(debug_assertions) {
            return None;
        }
        let environment = |key: &str| std::env::var(key).ok();
        if environment("COVE_DEVTOOLS").as_deref() != Some("1") {
            return None;
        }
        let host = environment("COVE_DEVTOOLS_HOST").unwrap_or_else(|| "127.0.0.1".into());
        let port = environment("COVE_DEVTOOLS_PORT")
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(9229);
        let mut config = Self::new(host, port, app_title, true);
        if let (Some(id), Some(token), Some(path)) = (
            environment("COVE_DEVTOOLS_SESSION_ID"),
            environment("COVE_DEVTOOLS_TOKEN"),
            environment("COVE_DEVTOOLS_SESSION_FILE"),
        ) {
            config.launch = Some(cove_devtools_protocol::DevToolsLaunch {
                id,
                token,
                descriptor_path: path,
                app_title: app_title.into(),
            });
        }
        Some(config)
    }
}

/// Host adapters configure only the diagnostics they implement before starting
/// the server.
#[derive(Default)]
pub struct DevToolsHost {
    pub scene: DevToolsHostScene,
    pub checkpoint: DevToolsHostCheckpoint,
    pub recording: DevToolsHostRecording,
    pub observation: DevToolsHostObservation,
    pub mirror: DevToolsHostMirror,
    pub window: DevToolsHostWindow,
    /// Schedules work onto the host's UI thread. A synchronous host loop cannot
    /// yield to an async executor, so it supplies this explicitly.
    pub execute: Option<HostMainExecutor>,
}

/// Produces a tree snapshot. The host runtime invokes it on its scene thread
/// before the JSON encode happens.
pub type SceneSnapshotProvider = std::rc::Rc<dyn Fn() -> TreeSnapshot>;

/// Invoked when a client requests `select.node`. The host decides what
/// selecting means, such as drawing an overlay.
pub type NodeSelectionHandler = std::rc::Rc<dyn Fn(&str)>;

pub type NodeSelectionClearHandler = std::rc::Rc<dyn Fn()>;

/// Schedules a callback onto the host's UI thread.
pub type HostMainExecutor = std::rc::Rc<dyn Fn(Box<dyn FnOnce()>)>;

#[derive(Default)]
pub struct DevToolsHostScene {
    pub inspect: Option<std::rc::Rc<dyn Fn(&Envelope) -> Envelope>>,
    pub reset: Option<std::rc::Rc<dyn Fn()>>,
    pub snapshot: Option<SceneSnapshotProvider>,
    pub select: Option<NodeSelectionHandler>,
    pub clear_selection: Option<NodeSelectionClearHandler>,
    pub diagnostics: Option<std::rc::Rc<dyn Fn(&Envelope) -> Envelope>>,
}

#[derive(Default)]
pub struct DevToolsHostCheckpoint {
    pub restore: Option<std::rc::Rc<dyn Fn(&std::collections::BTreeMap<String, String>) -> bool>>,
    pub capture: Option<std::rc::Rc<dyn Fn() -> std::collections::BTreeMap<String, String>>>,
}

#[derive(Default)]
pub struct DevToolsHostRecording {
    pub start: Option<std::rc::Rc<dyn Fn() -> bool>>,
    pub stop: Option<std::rc::Rc<dyn Fn() -> Option<InputRecording>>>,
    pub replay: Option<std::rc::Rc<dyn Fn(&InputRecording) -> bool>>,
}

#[derive(Default)]
pub struct DevToolsHostObservation {
    pub state: Option<std::rc::Rc<dyn Fn(&[String]) -> StateObservation>>,
    pub timeline: Option<std::rc::Rc<dyn Fn(u64) -> TimelineSnapshot>>,
    pub capture: Option<std::rc::Rc<dyn Fn(bool)>>,
}

#[derive(Default)]
pub struct DevToolsHostMirror {
    pub start: Option<std::rc::Rc<dyn Fn(&MirrorStart)>>,
    pub stop: Option<std::rc::Rc<dyn Fn()>>,
    pub input: Option<std::rc::Rc<dyn Fn(&MirrorInput)>>,
    /// Returns `None` on success, or the reason the action could not run.
    pub activate: Option<std::rc::Rc<dyn Fn(&MirrorActivate) -> Option<String>>>,
}

#[derive(Default)]
pub struct DevToolsHostWindow {
    /// Returns `None` on success, or the reason the window could not resize.
    pub resize: Option<std::rc::Rc<dyn Fn(&WindowResize) -> Option<String>>>,
    /// Replaces the device facts the host reports (platform, device class,
    /// posture, folds). Takes the request object and returns the resulting
    /// environment, or why it was rejected.
    pub environment:
        Option<std::rc::Rc<dyn Fn(&serde_json::Value) -> Result<serde_json::Value, String>>>,
}

/// One bounded handoff from the log writer to the socket side.
///
/// Neither side waits on the other: records accumulate in a bounded queue and a
/// drop counter is reported as a `log.gap`, so a slow client cannot stall the
/// host's logger.
pub struct DevToolsLogFeed {
    inner: std::sync::Mutex<LogFeedState>,
    observation: std::sync::Mutex<Option<cove_logging::LogObservation>>,
}

#[derive(Default)]
struct LogFeedState {
    pending: Vec<LogRecord>,
    dropped: u64,
}

/// Records a client could keep up with; beyond this only the loss count grows.
const LOG_QUEUE_LIMIT: usize = 1024;
const LOG_SNAPSHOT_RECORDS: usize = 200;
const LOG_SNAPSHOT_BYTES: usize = 512 * 1024;

impl Default for DevToolsLogFeed {
    fn default() -> Self {
        Self {
            inner: std::sync::Mutex::new(LogFeedState::default()),
            observation: std::sync::Mutex::new(None),
        }
    }
}

impl DevToolsLogFeed {
    /// Subscribe to the process logger. The callback only enqueues, so the
    /// logger's own thread is never blocked by a client.
    pub fn start(&self, sink: std::sync::Arc<dyn Fn(LogRecord) + Send + Sync>) {
        let observation = LogSystem::observe(
            LogQuery::default(),
            false,
            std::sync::Arc::new(move |record| sink(record.clone())),
        );
        *self
            .observation
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(observation);
    }

    pub fn stop(&self) {
        let observation = self
            .observation
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        if let Some(observation) = observation {
            observation.cancel();
        }
        let mut state = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        state.pending.clear();
        state.dropped = 0;
    }

    pub fn enqueue(&self, record: LogRecord) {
        let mut state = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        if state.pending.len() >= LOG_QUEUE_LIMIT {
            state.dropped = state.dropped.wrapping_add(1);
            return;
        }
        state.pending.push(record);
    }

    /// Take everything queued plus the accumulated drop count.
    pub fn drain(&self) -> (Vec<LogRecord>, u64) {
        let mut state = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        let records = std::mem::take(&mut state.pending);
        let dropped = std::mem::take(&mut state.dropped);
        (records, dropped)
    }

    pub fn has_pending(&self) -> bool {
        !self
            .inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .pending
            .is_empty()
    }

    /// Bounded newest-first snapshot, dropped when it would exceed the byte cap.
    pub fn snapshot(query: &LogQuery) -> cove_logging::LogSnapshot {
        let snapshot = LogSystem::snapshot(query.clone());
        let mut records = Vec::new();
        let mut bytes = 0usize;
        for record in snapshot.records.iter().rev().take(LOG_SNAPSHOT_RECORDS) {
            let Ok(size) = serde_json::to_vec(record) else {
                continue;
            };
            if bytes + size.len() > LOG_SNAPSHOT_BYTES {
                break;
            }
            bytes += size.len();
            records.push(record.clone());
        }
        records.reverse();
        cove_logging::LogSnapshot {
            records,
            dropped: snapshot.dropped,
            first_available_sequence: snapshot.first_available_sequence,
        }
    }
}

/// Rejection reasons for a server that cannot bind as configured.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DevServerConfigurationError {
    UnsupportedHost(String),
}

impl DevServerConfigurationError {
    pub fn message(&self) -> String {
        match self {
            Self::UnsupportedHost(host) => format!(
                "Unsupported DevTools host '{host}'. Use localhost/127.0.0.1/::1 for loopback or 0.0.0.0/:: for all interfaces."
            ),
        }
    }
}

impl std::fmt::Display for DevServerConfigurationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message())
    }
}

impl std::error::Error for DevServerConfigurationError {}

/// Highest level a client asked for, if it narrowed the window.
pub fn query_minimum(query: &LogQuery) -> Level {
    query.minimum
}

/// The platform name reported in `hello`.
pub fn current_platform_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "linux") {
        "Linux"
    } else if cfg!(target_os = "windows") {
        "Windows"
    } else if cfg!(target_os = "ios") {
        "iOS"
    } else if cfg!(target_os = "android") {
        "Android"
    } else {
        "unknown"
    }
}

/// Build the `hello` envelope sent to every new connection.
pub fn hello_envelope(config: &DevToolsConfig, capabilities: &[String]) -> Envelope {
    let payload = HelloPayload::new(
        HelloHostInfo {
            pid: std::process::id(),
            app_title: config.app_title.clone(),
            platform: current_platform_name().into(),
        },
        capabilities.to_vec(),
    );
    Envelope::new("hello", None, serde_json::to_value(payload).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_advertises_the_protocol_and_capabilities() {
        let config = DevToolsConfig::new("127.0.0.1", 0, "Test", true);
        let envelope = hello_envelope(&config, &["tree".into(), "select".into()]);
        assert_eq!(envelope.r#type, "hello");
        let payload: HelloPayload = serde_json::from_value(envelope.payload.unwrap()).unwrap();
        assert_eq!(payload.protocol, cove_devtools_protocol::PROTOCOL_VERSION);
        assert_eq!(payload.host.app_title, "Test");
        assert_eq!(payload.capabilities, vec!["tree", "select"]);
    }

    #[test]
    fn log_feed_bounds_its_queue_and_reports_losses() {
        let feed = DevToolsLogFeed::default();
        let record = LogRecord {
            sequence: 1,
            timestamp: 0.0,
            level: Level::Info,
            label: "test".into(),
            message: "m".into(),
            metadata: Default::default(),
            location: Default::default(),
            process: Default::default(),
        };
        for _ in 0..LOG_QUEUE_LIMIT + 5 {
            feed.enqueue(record.clone());
        }
        let (records, dropped) = feed.drain();
        assert_eq!(records.len(), LOG_QUEUE_LIMIT);
        assert_eq!(dropped, 5);
        assert!(!feed.has_pending());
        // Draining resets the drop counter.
        assert_eq!(feed.drain(), (Vec::new(), 0));
    }

    #[test]
    fn environment_opt_in_requires_the_debug_flag() {
        // Nothing is set in the test environment, so this must be `None`.
        if std::env::var("COVE_DEVTOOLS").ok().as_deref() != Some("1") {
            assert!(DevToolsConfig::from_environment("Test").is_none());
        }
    }
}
