//! Bounded structured logging, redaction, filtering, history and rotating JSONL output.
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Trace,
    Debug,
    #[default]
    Info,
    Notice,
    Warning,
    Error,
    Critical,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LogValue {
    String(String),
    Array(Vec<LogValue>),
    Object(BTreeMap<String, LogValue>),
}
impl From<&str> for LogValue {
    fn from(value: &str) -> Self {
        Self::String(value.into())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    pub source: String,
    pub file: String,
    pub function: String,
    pub line: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogRecord {
    pub sequence: u64,
    pub timestamp: f64,
    pub level: Level,
    pub label: String,
    pub message: String,
    pub metadata: BTreeMap<String, LogValue>,
    pub location: Location,
    pub pid: u32,
}
#[derive(Clone, Debug)]
pub struct LogQuery {
    pub minimum: Level,
    pub labels: Vec<String>,
    pub text: String,
    pub after_sequence: Option<u64>,
}
impl Default for LogQuery {
    fn default() -> Self {
        Self {
            minimum: Level::Trace,
            labels: vec![],
            text: String::new(),
            after_sequence: None,
        }
    }
}
impl LogQuery {
    pub fn matches(&self, record: &LogRecord) -> bool {
        record.level >= self.minimum
            && self.after_sequence.is_none_or(|s| record.sequence > s)
            && (self.labels.is_empty() || self.labels.iter().any(|l| in_module(&record.label, l)))
            && (self.text.is_empty()
                || format!(
                    "{} {} {:?} {} {}",
                    record.label,
                    record.message,
                    record.metadata,
                    record.location.file,
                    record.location.function
                )
                .to_lowercase()
                .contains(&self.text.to_lowercase()))
    }
}
#[derive(Clone, Debug)]
pub struct LogSnapshot {
    pub records: Vec<LogRecord>,
    pub dropped: u64,
    pub first_available_sequence: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct Configuration {
    pub minimum: Level,
    pub modules: BTreeMap<String, Level>,
    pub console: bool,
    pub console_json: bool,
    pub history_records: usize,
    pub message_bytes: usize,
    pub metadata_bytes: usize,
    pub redacted_keys: BTreeSet<String>,
    pub storage: Option<StorageConfiguration>,
}
#[derive(Clone, Debug)]
pub struct StorageConfiguration {
    pub directory: PathBuf,
    pub maximum_file_bytes: u64,
    pub files: usize,
}
impl Default for Configuration {
    fn default() -> Self {
        Self {
            minimum: Level::Info,
            modules: BTreeMap::new(),
            console: true,
            console_json: false,
            history_records: 512,
            message_bytes: 16 * 1024,
            metadata_bytes: 8 * 1024,
            redacted_keys: [
                "password",
                "passwd",
                "token",
                "secret",
                "authorization",
                "cookie",
                "api_key",
                "apikey",
                "access_token",
                "refresh_token",
            ]
            .map(str::to_owned)
            .into(),
            storage: None,
        }
    }
}
fn in_module(label: &str, module: &str) -> bool {
    label == module
        || label
            .strip_prefix(module)
            .is_some_and(|rest| rest.starts_with('.'))
}
impl Configuration {
    fn validate(&self) -> io::Result<()> {
        if self.history_records > 65536
            || !(64..=32768).contains(&self.message_bytes)
            || !(64..=16384).contains(&self.metadata_bytes)
            || self.modules.keys().any(String::is_empty)
            || self
                .storage
                .as_ref()
                .is_some_and(|s| !(1..=32).contains(&s.files) || s.maximum_file_bytes < 65536)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid logging configuration",
            ));
        }
        Ok(())
    }
    fn threshold(&self, label: &str) -> Level {
        self.modules
            .iter()
            .filter(|(module, _)| in_module(label, module))
            .max_by_key(|(module, _)| module.len())
            .map(|(_, level)| *level)
            .unwrap_or(self.minimum)
    }
}
struct Storage {
    config: StorageConfiguration,
    file: File,
    bytes: u64,
}
impl Storage {
    fn open(config: StorageConfiguration) -> io::Result<Self> {
        fs::create_dir_all(&config.directory)?;
        cove_system::files::restrict_path(&config.directory, true)?;
        let path = config.directory.join("current.jsonl");
        if !path.exists() {
            cove_system::files::atomic_write(&path, b"", cove_system::files::Access::OwnerOnly)?;
        }
        cove_system::files::restrict_path(&path, false)?;
        let file = OpenOptions::new().append(true).open(path)?;
        let bytes = file.metadata()?.len();
        Ok(Self {
            config,
            file,
            bytes,
        })
    }
    fn write(&mut self, record: &LogRecord) -> io::Result<()> {
        let mut data = serde_json::to_vec(record)?;
        data.push(b'\n');
        if data.len() as u64 > self.config.maximum_file_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "encoded log record exceeds the file limit",
            ));
        }
        if self.bytes > 0
            && self.bytes.saturating_add(data.len() as u64) > self.config.maximum_file_bytes
        {
            self.file.flush()?;
            // Close the current file before rename on Windows.
            let replacement = tempfile_placeholder(&self.config.directory)?;
            let old = std::mem::replace(&mut self.file, replacement);
            drop(old);
            for i in (1..self.config.files).rev() {
                let old = self.config.directory.join(if i == 1 {
                    "current.jsonl".into()
                } else {
                    format!("{}.jsonl", i - 1)
                });
                let next = self.config.directory.join(format!("{i}.jsonl"));
                if old.exists() {
                    if next.exists() {
                        fs::remove_file(&next)?;
                    }
                    fs::rename(old, next)?;
                }
            }
            let path = self.config.directory.join("current.jsonl");
            cove_system::files::atomic_write(&path, b"", cove_system::files::Access::OwnerOnly)?;
            self.file = OpenOptions::new().append(true).open(path)?;
            self.bytes = 0;
        }
        self.file.write_all(&data)?;
        self.bytes += data.len() as u64;
        if record.level >= Level::Error {
            self.file.flush()?;
        }
        Ok(())
    }
}
fn tempfile_placeholder(directory: &std::path::Path) -> io::Result<File> {
    // A separate lock file also provides a stable handle during rotation.
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join(".rotation"))
}

struct Inner {
    next: u64,
    dropped: u64,
    history: VecDeque<LogRecord>,
    storage: Option<Storage>,
}
#[derive(Clone)]
pub struct LogRuntime {
    config: Arc<Configuration>,
    inner: Arc<Mutex<Inner>>,
}
impl LogRuntime {
    pub fn new(mut config: Configuration) -> io::Result<Self> {
        config.validate()?;
        config.redacted_keys = config
            .redacted_keys
            .into_iter()
            .map(|key| key.to_lowercase())
            .collect();
        let storage = config.storage.clone().map(Storage::open).transpose()?;
        Ok(Self {
            config: Arc::new(config),
            inner: Arc::new(Mutex::new(Inner {
                next: 1,
                dropped: 0,
                history: VecDeque::new(),
                storage,
            })),
        })
    }
    pub fn log(
        &self,
        level: Level,
        label: &str,
        message: &str,
        metadata: BTreeMap<String, LogValue>,
        location: Location,
    ) -> io::Result<Option<u64>> {
        if level < self.config.threshold(label) {
            return Ok(None);
        }
        let mut metadata = metadata;
        sanitize(
            &mut metadata,
            &self.config.redacted_keys,
            self.config.metadata_bytes,
            0,
        );
        if serde_json::to_vec(&metadata)?.len() > self.config.metadata_bytes {
            metadata.clear();
            metadata.insert(
                "truncated".into(),
                LogValue::from("metadata limit exceeded"),
            );
        }
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let sequence = inner.next;
        inner.next = inner
            .next
            .checked_add(1)
            .ok_or_else(|| io::Error::other("log sequence exhausted"))?;
        let record = LogRecord {
            sequence,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
            level,
            label: truncate(label, 1024),
            message: truncate(message, self.config.message_bytes),
            metadata,
            location: Location {
                source: truncate(&location.source, 1024),
                file: truncate(&location.file, 4096),
                function: truncate(&location.function, 1024),
                line: location.line,
            },
            pid: std::process::id(),
        };
        // History is always available even when an output sink fails.
        if self.config.history_records > 0 {
            if inner.history.len() == self.config.history_records {
                inner.history.pop_front();
                inner.dropped += 1;
            }
            inner.history.push_back(record.clone());
        }
        if let Some(storage) = &mut inner.storage {
            storage.write(&record)?;
        }
        if self.config.console {
            let mut stderr = io::stderr().lock();
            if self.config.console_json {
                serde_json::to_writer(&mut stderr, &record)?;
                writeln!(stderr)?;
            } else {
                writeln!(
                    stderr,
                    "{:?} {}: {}",
                    record.level, record.label, record.message
                )?;
            }
        }
        Ok(Some(sequence))
    }
    pub fn snapshot(&self, query: &LogQuery) -> LogSnapshot {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        LogSnapshot {
            records: inner
                .history
                .iter()
                .filter(|r| query.matches(r))
                .cloned()
                .collect(),
            dropped: inner.dropped,
            first_available_sequence: inner.history.front().map(|r| r.sequence),
        }
    }
}
fn truncate(value: &str, maximum: usize) -> String {
    let mut end = value.len().min(maximum);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].into()
}
fn sanitize(
    metadata: &mut BTreeMap<String, LogValue>,
    keys: &BTreeSet<String>,
    limit: usize,
    depth: usize,
) {
    for (key, value) in metadata {
        if keys.contains(&key.to_lowercase()) {
            *value = LogValue::from("<redacted>");
            continue;
        }
        sanitize_value(value, keys, limit, depth + 1);
    }
}
fn sanitize_value(value: &mut LogValue, keys: &BTreeSet<String>, limit: usize, depth: usize) {
    if depth > 16 {
        *value = LogValue::from("<depth limit>");
        return;
    }
    match value {
        LogValue::String(text) => *text = truncate(text, limit),
        LogValue::Object(map) => sanitize(map, keys, limit, depth),
        LogValue::Array(items) => {
            items.truncate(256);
            for value in items {
                sanitize_value(value, keys, limit, depth + 1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_redaction_filtering_and_bounded_history() {
        let logger = LogRuntime::new(Configuration {
            console: false,
            history_records: 2,
            message_bytes: 64,
            modules: [
                ("cove".into(), Level::Warning),
                ("cove.render".into(), Level::Debug),
            ]
            .into(),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(
            logger
                .log(
                    Level::Info,
                    "cove.shell",
                    "ignored",
                    BTreeMap::new(),
                    Location::default()
                )
                .unwrap(),
            None
        );
        for _ in 0..3 {
            logger
                .log(
                    Level::Debug,
                    "cove.render.gpu",
                    &"🦀".repeat(100),
                    [(
                        "nested".into(),
                        LogValue::Object(
                            [("Authorization".into(), LogValue::from("secret"))].into(),
                        ),
                    )]
                    .into(),
                    Location::default(),
                )
                .unwrap();
        }
        let history = logger.snapshot(&LogQuery {
            minimum: Level::Trace,
            labels: vec!["cove.render".into()],
            ..Default::default()
        });
        assert_eq!(
            (
                history.records.len(),
                history.dropped,
                history.first_available_sequence
            ),
            (2, 1, Some(2))
        );
        assert_eq!(history.records[0].message.len(), 64);
        let json = serde_json::to_string(&history.records).unwrap();
        assert!(!json.contains("secret"));
        assert!(json.contains("<redacted>"));
    }
    #[test]
    fn rotating_storage_stays_bounded_and_valid_jsonl() {
        let root = tempfile::tempdir().unwrap();
        let logger = LogRuntime::new(Configuration {
            console: false,
            message_bytes: 32768,
            storage: Some(StorageConfiguration {
                directory: root.path().join("logs"),
                maximum_file_bytes: 65536,
                files: 3,
            }),
            ..Default::default()
        })
        .unwrap();
        for _ in 0..20 {
            logger
                .log(
                    Level::Error,
                    "test",
                    &"x".repeat(30000),
                    BTreeMap::new(),
                    Location::default(),
                )
                .unwrap();
        }
        let files: Vec<_> = fs::read_dir(root.path().join("logs"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "jsonl"))
            .collect();
        assert_eq!(files.len(), 3);
        for path in files {
            let contents = fs::read_to_string(&path).unwrap();
            assert!(contents.len() <= 65536);
            for line in contents.lines() {
                serde_json::from_str::<LogRecord>(line).unwrap();
            }
        }
    }
}
