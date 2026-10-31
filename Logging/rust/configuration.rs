//! Translation of `Logging/Sources/CoveLogCore/Configuration/Configuration.swift`.
use super::record::Level;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Installation {
    #[default]
    Process,
    Local,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConsoleFormat {
    #[default]
    Text,
    Json,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Filtering {
    pub minimum: Level,
    pub modules: BTreeMap<String, Level>,
}
impl Default for Filtering {
    fn default() -> Self {
        Self {
            minimum: Level::Info,
            modules: BTreeMap::new(),
        }
    }
}
impl Filtering {
    /// Longest matching module prefix wins; an exact label always matches.
    pub fn level(&self, label: &str) -> Level {
        self.modules
            .iter()
            .filter(|(module, _)| label == *module || label.starts_with(&format!("{module}.")))
            .max_by_key(|(module, _)| module.len())
            .map(|(_, level)| *level)
            .unwrap_or(self.minimum)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Console {
    pub enabled: bool,
    pub format: ConsoleFormat,
}
impl Default for Console {
    fn default() -> Self {
        Self {
            enabled: true,
            format: ConsoleFormat::Text,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Storage {
    pub enabled: bool,
    /// `None` selects `LogSystem::default_directory`.
    pub directory: Option<PathBuf>,
    pub maximum_file_bytes: u64,
    pub files_per_session: usize,
    pub retained_sessions: usize,
    pub flush_on_error: bool,
}
impl Default for Storage {
    fn default() -> Self {
        Self {
            enabled: true,
            directory: None,
            maximum_file_bytes: 8 * 1024 * 1024,
            files_per_session: 3,
            retained_sessions: 3,
            flush_on_error: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Buffering {
    pub pending_records: usize,
    pub history_records: usize,
    pub message_bytes: usize,
    pub metadata_bytes: usize,
}
impl Default for Buffering {
    fn default() -> Self {
        Self {
            pending_records: 1024,
            history_records: 512,
            message_bytes: 16 * 1024,
            metadata_bytes: 8 * 1024,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Redaction {
    pub keys: BTreeSet<String>,
}
impl From<BTreeSet<String>> for Redaction {
    fn from(keys: BTreeSet<String>) -> Self {
        Self { keys }
    }
}
impl Default for Redaction {
    fn default() -> Self {
        [
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
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<String>>()
        .into()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingConfiguration {
    pub filtering: Filtering,
    pub console: Console,
    pub storage: Storage,
    pub buffering: Buffering,
    pub redaction: Redaction,
    pub installation: Installation,
}
impl Default for LoggingConfiguration {
    fn default() -> Self {
        Self {
            filtering: Filtering::default(),
            console: Console::default(),
            storage: Storage::default(),
            buffering: Buffering::default(),
            redaction: Redaction::default(),
            installation: Installation::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LoggingError {
    #[error("invalid logging limits, module filter or storage directory")]
    InvalidConfiguration,
}

impl LoggingConfiguration {
    pub fn new(configure: impl FnOnce(&mut Self)) -> Self {
        let mut value = Self::default();
        configure(&mut value);
        value
    }

    /// Preserves the Swift guard order and numeric ranges.
    pub fn validate(&self) -> Result<(), LoggingError> {
        let storage = &self.storage;
        let buffering = &self.buffering;
        if !(64 * 1024..=1024 * 1024 * 1024).contains(&storage.maximum_file_bytes)
            || !(1..=32).contains(&storage.files_per_session)
            || !(1..=64).contains(&storage.retained_sessions)
            || !(1..=65536).contains(&buffering.pending_records)
            || buffering.history_records > 65536
            || !(64..=32768).contains(&buffering.message_bytes)
            || !(64..=16384).contains(&buffering.metadata_bytes)
            || self.filtering.modules.keys().any(String::is_empty)
            || (storage.enabled
                && storage.maximum_file_bytes
                    < 8 * (buffering.message_bytes + buffering.metadata_bytes + 4096) as u64)
        {
            return Err(LoggingError::InvalidConfiguration);
        }
        if self
            .storage
            .directory
            .as_ref()
            .is_some_and(|path| path.to_string_lossy().contains("://"))
        {
            // Foundation rejects non-file URLs; a `PathBuf` has no scheme, so
            // only an explicit foreign scheme is invalid.
            return Err(LoggingError::InvalidConfiguration);
        }
        Ok(())
    }
}
