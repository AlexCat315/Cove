//! Translation of `Logging/Sources/CoveLogging/Runtime/Runtime.swift`.
use super::configuration::{LoggingConfiguration, Storage};
use super::executor::LogExecutor;
use super::file::LogFile;
use super::formatting::LogFormatter;
use super::handler::{LogEvent, UnifiedLogHandler};
use super::history::LogHistory;
use super::observation::LogObservation;
use super::record::{Level, Location, LogQuery, LogRecord, LogSnapshot, LogValue, Process};
use super::sanitizer::RecordSanitizer;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct LoggingStatus {
    pub written: u64,
    pub dropped: u64,
    pub pending: usize,
    pub file_failures: u64,
    pub file: Option<PathBuf>,
}

struct Capture {
    configuration: LoggingConfiguration,
    identity: Process,
    pending: Vec<LogRecord>,
    scheduled: bool,
    sequence: u64,
    dropped: u64,
    reported_drops: u64,
    configured: bool,
}

struct Subscriber {
    query: LogQuery,
    receive: Arc<dyn Fn(&LogRecord) + Send + Sync>,
}

pub struct LogRuntime {
    worker: LogExecutor,
    capture: Mutex<Capture>,
    formatter: LogFormatter,
    history: Mutex<LogHistory>,
    subscribers: Mutex<BTreeMap<u64, Subscriber>>,
    next_subscriber: AtomicU64,
    written: AtomicU64,
    write_failures: AtomicU64,
    file: Mutex<Option<LogFile>>,
}

impl LogRuntime {
    pub fn new(run_id: impl Into<String>) -> Arc<Self> {
        let configuration = LoggingConfiguration::new(|value| value.storage.enabled = false);
        let sanitizer = RecordSanitizer::new(&configuration);
        let identity = Process {
            pid: i64::from(std::process::id()),
            name: sanitizer.text(&process_name(), 256),
            run_id: sanitizer.text(&run_id.into(), 256),
        };
        Arc::new(Self {
            worker: LogExecutor::new(),
            capture: Mutex::new(Capture {
                configuration,
                identity,
                pending: Vec::new(),
                scheduled: false,
                sequence: 0,
                dropped: 0,
                reported_drops: 0,
                configured: false,
            }),
            formatter: LogFormatter::new(),
            history: Mutex::new(LogHistory::new(512)),
            subscribers: Mutex::new(BTreeMap::new()),
            next_subscriber: AtomicU64::new(0),
            written: AtomicU64::new(0),
            write_failures: AtomicU64::new(0),
            file: Mutex::new(None),
        })
    }

    pub fn configuration(&self) -> LoggingConfiguration {
        self.capture
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .configuration
            .clone()
    }

    pub fn identity(&self) -> Process {
        self.capture
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .identity
            .clone()
    }

    pub fn minimum(&self, label: &str) -> Level {
        self.capture
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .configuration
            .filtering
            .level(label)
    }

    pub fn logger(self: &Arc<Self>, label: &str) -> UnifiedLogHandler {
        UnifiedLogHandler::new(label, self.clone())
    }

    /// Aggregated entry point kept for producers that already hold a location
    /// and metadata. Delegates to the handler path so filtering, sanitizing and
    /// subscriber delivery are identical.
    pub fn log(
        &self,
        level: Level,
        label: &str,
        message: &str,
        metadata: BTreeMap<String, LogValue>,
        location: Location,
    ) -> io::Result<Option<u64>> {
        let handler = UnifiedLogHandler::new(
            label,
            self.clone_handle(),
        );
        let mut event = LogEvent::new(level, message).with_location(location);
        if !metadata.is_empty() {
            event = event.with_metadata(metadata);
        }
        Ok(handler.log_sequence(&event))
    }

    /// Installs configuration, opens the session file and replays the history
    /// that predates it. Repeated identical configuration is a no-op.
    pub fn configure(
        self: &Arc<Self>,
        configuration: LoggingConfiguration,
        application: &str,
        run_id: Option<String>,
    ) -> io::Result<()> {
        configuration
            .validate()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error.to_string()))?;
        let sanitizer = RecordSanitizer::new(&configuration);
        let application = sanitizer.text(application, 256);
        let run_id = run_id.map(|value| sanitizer.text(&value, 256));
        let unchanged = {
            let capture = self.capture.lock().unwrap_or_else(|e| e.into_inner());
            capture.configured
                && capture.configuration == configuration
                && capture.identity.name == application
                && run_id.as_ref().is_none_or(|run_id| capture.identity.run_id == *run_id)
        };
        if unchanged {
            return Ok(());
        }
        let this = self.clone();
        self.worker.sync(move || {
            this.drain();
            this.flush_file();
            let startup = {
                let mut capture = this.capture.lock().unwrap_or_else(|e| e.into_inner());
                let startup = !capture.configured;
                capture.configuration = configuration.clone();
                capture.identity = Process {
                    pid: capture.identity.pid,
                    name: application.clone(),
                    run_id: run_id.unwrap_or_else(|| capture.identity.run_id.clone()),
                };
                capture.configured = true;
                if startup {
                    let identity = capture.identity.clone();
                    for record in &mut capture.pending {
                        record.process = identity.clone();
                    }
                }
                startup
            };
            if startup {
                this.history
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .identify_startup(&this.identity());
            }
            this.history
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .resize(configuration.buffering.history_records);
            if !configuration.storage.enabled {
                this.file.lock().unwrap_or_else(|e| e.into_inner()).take();
                return;
            }
            let root = configuration
                .storage
                .directory
                .clone()
                .unwrap_or_else(crate::system::LogSystem::default_directory);
            let had_file = this
                .file
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_some();
            match LogFile::new(&root, &application, configuration.storage.clone()) {
                Ok(file) => {
                    let mut slot = this.file.lock().unwrap_or_else(|e| e.into_inner());
                    *slot = Some(file);
                    if !had_file {
                        let records = this
                            .history
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .records();
                        for record in records {
                            match this.formatter.json(&record) {
                                Ok(data) => {
                                    if let Some(file) = slot.as_mut() {
                                        if let Err(error) = file.append(&data) {
                                            drop(slot);
                                            this.storage_failed(&error);
                                            break;
                                        }
                                    }
                                }
                                Err(error) => {
                                    drop(slot);
                                    this.storage_failed(&error);
                                    break;
                                }
                            }
                        }
                    }
                }
                Err(error) => this.storage_failed(&error),
            }
        });
        Ok(())
    }

    pub fn submit(
        &self,
        label: &str,
        event: &LogEvent,
        metadata: BTreeMap<String, LogValue>,
    ) -> Option<u64> {
        let configuration = self.configuration();
        let mut sanitizer = RecordSanitizer::new(&configuration);
        let message = sanitizer.text(&event.message, configuration.buffering.message_bytes);
        let values = sanitizer.metadata(&metadata);
        let location = Location {
            source: sanitizer.text(&event.source, 512),
            file: sanitizer.text(&event.file, 1024),
            function: sanitizer.text(&event.function, 1024),
            line: event.line,
        };
        let label = sanitizer.text(label, 256);
        let mut fallback = false;
        let mut assigned = None;
        let schedule = {
            let mut capture = self.capture.lock().unwrap_or_else(|e| e.into_inner());
            capture.sequence = capture.sequence.wrapping_add(1);
            let record = LogRecord {
                sequence: capture.sequence,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|duration| duration.as_secs_f64())
                    .unwrap_or_default(),
                level: event.level,
                label,
                message: message.clone(),
                metadata: values,
                location,
                process: capture.identity.clone(),
            };
            if capture.pending.len() >= configuration.buffering.pending_records {
                capture.dropped = capture.dropped.wrapping_add(1);
                if event.level >= Level::Error {
                    // Preserve the newest high-severity event; its displaced
                    // predecessor is counted.
                    capture.pending.remove(0);
                    assigned = Some(record.sequence);
                    capture.pending.push(record);
                    fallback = true;
                }
                false
            } else {
                assigned = Some(record.sequence);
                capture.pending.push(record);
                if capture.scheduled {
                    false
                } else {
                    capture.scheduled = true;
                    true
                }
            }
        };
        if fallback {
            crate::system::console_write(
                5,
                &format!("[Cove logging queue saturated] {message}"),
            );
        }
        if schedule {
            let this = self.clone_handle();
            self.worker.r#async(move || this.drain());
        }
        if event.level >= Level::Error && configuration.storage.flush_on_error {
            self.flush();
        }
        assigned
    }

    fn drain(&self) {
        let batch = {
            let mut capture = self.capture.lock().unwrap_or_else(|e| e.into_inner());
            std::mem::take(&mut capture.pending)
        };
        for record in batch {
            self.output(&record);
        }
        let drops = {
            let mut capture = self.capture.lock().unwrap_or_else(|e| e.into_inner());
            let count = capture.dropped.saturating_sub(capture.reported_drops);
            capture.reported_drops = capture.dropped;
            count
        };
        if drops > 0 {
            let total = self
                .capture
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .dropped;
            let metadata = [
                ("dropped".to_owned(), LogValue::from(drops.to_string())),
                ("total_dropped".to_owned(), LogValue::from(total.to_string())),
            ]
            .into();
            let mut event = LogEvent::new(Level::Warning, "Log queue capacity exceeded");
            event.metadata = Some(metadata);
            self.clone_handle().logger("com.cove.logging").log(&event);
        }
        let pending = {
            let mut capture = self.capture.lock().unwrap_or_else(|e| e.into_inner());
            let pending = !capture.pending.is_empty();
            if !pending {
                capture.scheduled = false;
            }
            pending
        };
        // Yield between bounded batches so snapshots/configuration cannot
        // starve under a log flood.
        if pending {
            let this = self.clone_handle();
            self.worker.r#async(move || this.drain());
        }
    }

    fn output(&self, record: &LogRecord) {
        let configuration = self.configuration();
        if configuration.console.enabled {
            self.formatter.console(record, configuration.console.format);
        }
        if configuration.storage.enabled {
            let mut slot = self.file.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(file) = slot.as_mut() {
                match self.formatter.json(record) {
                    Ok(data) => {
                        if let Err(error) = file.append(&data) {
                            drop(slot);
                            self.storage_failed(&error);
                        }
                    }
                    Err(error) => {
                        drop(slot);
                        self.storage_failed(&error);
                    }
                }
            }
        }
        if configuration.buffering.history_records > 0 {
            self.history
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .append(record.clone());
        }
        self.written.fetch_add(1, Ordering::Relaxed);
        let subscribers: Vec<Arc<dyn Fn(&LogRecord) + Send + Sync>> = {
            let subscribers = self.subscribers.lock().unwrap_or_else(|e| e.into_inner());
            subscribers
                .values()
                .filter(|subscriber| subscriber.query.matches(record))
                .map(|subscriber| subscriber.receive.clone())
                .collect()
        };
        // Never hold the subscriber lock while running user callbacks.
        for receive in subscribers {
            receive(record);
        }
    }

    fn storage_failed(&self, error: &(dyn std::error::Error + 'static)) {
        self.file.lock().unwrap_or_else(|e| e.into_inner()).take();
        self.write_failures.fetch_add(1, Ordering::Relaxed);
        let mut event = LogEvent::new(Level::Error, "Log file output failed");
        event.metadata = Some(
            [("error".to_owned(), LogValue::from(error.to_string()))]
                .into(),
        );
        self.clone_handle().logger("com.cove.logging").log(&event);
        crate::system::console_write(
            5,
            &format!(
                "Cove log file unavailable: {error}; console and history remain active"
            ),
        );
    }

    pub fn flush(&self) {
        if self.worker.is_current() {
            return;
        }
        let this = self.clone_handle();
        self.worker.sync(move || {
            this.drain();
            this.flush_file();
        });
    }

    fn flush_file(&self) {
        let mut slot = self.file.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(file) = slot.as_mut() {
            if let Err(error) = file.flush() {
                drop(slot);
                self.storage_failed(&error);
            }
        }
    }

    pub fn snapshot(&self, query: &LogQuery) -> LogSnapshot {
        let this = self.clone_handle();
        let query = query.clone();
        self.worker.sync(move || {
            let records = this
                .history
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .records();
            let first = records.first().map(|record| record.sequence);
            LogSnapshot {
                records: records
                    .into_iter()
                    .filter(|record| query.matches(record))
                    .collect(),
                dropped: this
                    .capture
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .dropped,
                first_available_sequence: first,
            }
        })
    }

    pub fn observe(
        self: &Arc<Self>,
        query: LogQuery,
        replay: bool,
        receive: Arc<dyn Fn(&LogRecord) + Send + Sync>,
    ) -> LogObservation {
        let id = self.next_subscriber.fetch_add(1, Ordering::Relaxed) + 1;
        let this = self.clone();
        let subscribe = receive.clone();
        self.worker.sync(move || {
            if replay {
                for record in this
                    .history
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .records()
                {
                    if query.matches(&record) {
                        subscribe(&record);
                    }
                }
            }
            this.subscribers
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(
                    id,
                    Subscriber {
                        query: query.clone(),
                        receive,
                    },
                );
        });
        let weak: Weak<Self> = Arc::downgrade(self);
        LogObservation::new(move || {
            if let Some(runtime) = weak.upgrade() {
                runtime
                    .subscribers
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&id);
            }
        })
    }

    pub fn status(&self) -> LoggingStatus {
        let this = self.clone_handle();
        self.worker.sync(move || {
            let file = this
                .file
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
                .map(LogFile::path);
            let capture = this.capture.lock().unwrap_or_else(|e| e.into_inner());
            LoggingStatus {
                written: this.written.load(Ordering::Relaxed),
                dropped: capture.dropped,
                pending: capture.pending.len(),
                file_failures: this.write_failures.load(Ordering::Relaxed),
                file,
            }
        })
    }

    fn clone_handle(&self) -> Arc<Self> {
        // `submit`, `drain` and `snapshot` are also reachable through the
        // runtime stored by a handler; an Arc is required to schedule work.
        RUNTIME_HANDLES.with(|handles| {
            handles
                .borrow()
                .iter()
                .find(|handle| std::ptr::eq(handle.as_ref(), self))
                .cloned()
        })
        .expect("log runtime invoked through an Arc handle")
    }
}

thread_local! {
    /// Registration of the current Arc so `&self` methods can schedule work.
    static RUNTIME_HANDLES: std::cell::RefCell<Vec<Arc<LogRuntime>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Registers `runtime` for the current thread. The system runtime calls this
/// once during bootstrap; callers that construct a runtime directly must call
/// it before submitting records.
pub fn register_runtime(runtime: &Arc<LogRuntime>) {
    RUNTIME_HANDLES.with(|handles| {
        let mut handles = handles.borrow_mut();
        if !handles.iter().any(|handle| Arc::ptr_eq(handle, runtime)) {
            handles.push(runtime.clone());
        }
    });
}

fn process_name() -> String {
    std::env::args()
        .next()
        .and_then(|path| {
            Path::new(&path)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "Cove".into())
}

/// Opens the session file for a runtime configured by `LogSystem`.
pub fn open_storage(root: &Path, application: &str, storage: Storage) -> io::Result<LogFile> {
    if !storage.directory.as_ref().is_none_or(|path| path.starts_with(root)) {
        fs::create_dir_all(root)?;
    }
    LogFile::new(root, application, storage)
}
