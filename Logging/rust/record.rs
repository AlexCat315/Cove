//! Translation of `Logging/Sources/CoveLogCore/Records/Record.swift`.
//!
//! Swift's `Logger.Level` comes from swift-log; the Rust workspace has no
//! logging facade dependency, so the same seven cases live here and preserve
//! ordering and wire names.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

impl Level {
    /// Stable index used by the native console router.
    pub fn native_index(self) -> i32 {
        match self {
            Self::Trace => 0,
            Self::Debug => 1,
            Self::Info => 2,
            Self::Notice => 3,
            Self::Warning => 4,
            Self::Error => 5,
            Self::Critical => 6,
        }
    }
    pub fn from_native_index(index: i32) -> Self {
        match index {
            0 => Self::Trace,
            1 => Self::Debug,
            2 => Self::Info,
            3 => Self::Notice,
            4 => Self::Warning,
            5 => Self::Error,
            6 => Self::Critical,
            _ => Self::Info,
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "trace" => Some(Self::Trace),
            "debug" => Some(Self::Debug),
            "info" => Some(Self::Info),
            "notice" => Some(Self::Notice),
            "warning" => Some(Self::Warning),
            "error" => Some(Self::Error),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Notice => "notice",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Critical => "critical",
        }
    }
}

/// Untyped metadata value. Strings, arrays and objects are the only shapes the
/// wire format accepts; decoding tries them in that order like Swift's
/// single-value container.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
impl From<String> for LogValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Location {
    pub source: String,
    pub file: String,
    pub function: String,
    pub line: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Process {
    pub pid: i64,
    pub name: String,
    #[serde(rename = "runID")]
    pub run_id: String,
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
    pub process: Process,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LogQuery {
    #[serde(default)]
    pub minimum: Level,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub text: String,
    #[serde(default, rename = "afterSequence")]
    pub after_sequence: Option<u64>,
}

impl LogQuery {
    pub fn matches(&self, record: &LogRecord) -> bool {
        if record.level < self.minimum {
            return false;
        }
        if self
            .after_sequence
            .is_some_and(|sequence| record.sequence <= sequence)
        {
            return false;
        }
        if !self.labels.is_empty()
            && !self
                .labels
                .iter()
                .any(|label| record.label == *label || record.label.starts_with(&format!("{label}.")))
        {
            return false;
        }
        if self.text.is_empty() {
            return true;
        }
        // `localizedCaseInsensitiveContains` is locale aware in Foundation; the
        // portable Rust path performs the same case-folded substring search.
        let haystack = format!(
            "{} {} {:?} {} {} {}",
            record.label,
            record.message,
            record.metadata,
            record.location.file,
            record.location.function,
            record.process.run_id
        );
        haystack.to_lowercase().contains(&self.text.to_lowercase())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogSnapshot {
    pub records: Vec<LogRecord>,
    pub dropped: u64,
    #[serde(rename = "firstAvailableSequence")]
    pub first_available_sequence: Option<u64>,
}
