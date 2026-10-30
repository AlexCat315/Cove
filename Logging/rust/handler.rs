//! Translation of `Logging/Sources/CoveLogging/Runtime/Handler.swift`.
//!
//! swift-log's `LogHandler` protocol is replaced by an owned handler value: the
//! `log` facade is not a workspace dependency, so producers call `log` directly
//! with the same event payload.
use super::context::LogContext;
use super::record::{Level, Location, LogValue};
use super::runtime::LogRuntime;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct LogEvent {
    pub level: Level,
    pub message: String,
    pub metadata: Option<BTreeMap<String, LogValue>>,
    pub source: String,
    pub file: String,
    pub function: String,
    pub line: u64,
}

impl LogEvent {
    pub fn new(level: Level, message: impl Into<String>) -> Self {
        Self {
            level,
            message: message.into(),
            metadata: None,
            source: String::new(),
            file: String::new(),
            function: String::new(),
            line: 0,
        }
    }

    pub fn with_metadata(mut self, metadata: BTreeMap<String, LogValue>) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn with_location(mut self, location: Location) -> Self {
        self.source = location.source;
        self.file = location.file;
        self.function = location.function;
        self.line = location.line;
        self
    }
}

/// Deferred metadata merged ahead of the handler's own values, mirroring
/// swift-log's `MetadataProvider.get()`.
#[derive(Clone)]
pub struct MetadataProvider(pub Arc<dyn Fn() -> BTreeMap<String, LogValue> + Send + Sync>);

#[derive(Clone)]
pub struct UnifiedLogHandler {
    pub label: String,
    pub runtime: Arc<LogRuntime>,
    pub metadata: BTreeMap<String, LogValue>,
    pub metadata_provider: Option<MetadataProvider>,
    local_level: Option<Level>,
}

impl UnifiedLogHandler {
    pub fn new(label: impl Into<String>, runtime: Arc<LogRuntime>) -> Self {
        Self {
            label: label.into(),
            runtime,
            metadata: BTreeMap::new(),
            metadata_provider: None,
            local_level: None,
        }
    }

    pub fn log_level(&self) -> Level {
        self.local_level
            .unwrap_or_else(|| self.runtime.minimum(&self.label))
    }

    pub fn set_log_level(&mut self, level: Level) {
        self.local_level = Some(level);
    }

    pub fn metadata_value(&self, key: &str) -> Option<&LogValue> {
        self.metadata.get(key)
    }

    pub fn set_metadata_value(&mut self, key: impl Into<String>, value: Option<LogValue>) {
        let key = key.into();
        match value {
            Some(value) => {
                self.metadata.insert(key, value);
            }
            None => {
                self.metadata.remove(&key);
            }
        }
    }

    /// Ambient context is merged first, then the provider, the handler's own
    /// metadata and finally the explicit event metadata. Returns the sequence
    /// assigned to the record, or `None` when the level was filtered out or the
    /// queue rejected it.
    pub fn log_sequence(&self, event: &LogEvent) -> Option<u64> {
        // swift-log consults `logLevel` before dispatching an event.
        if event.level < self.log_level() {
            return None;
        }
        let mut values = LogContext::current();
        if let Some(provider) = &self.metadata_provider {
            for (key, value) in (provider.0)() {
                values.insert(key, value);
            }
        }
        for (key, value) in &self.metadata {
            values.insert(key.clone(), value.clone());
        }
        if let Some(metadata) = &event.metadata {
            for (key, value) in metadata {
                values.insert(key.clone(), value.clone());
            }
        }
        self.runtime.submit(&self.label, event, values)
    }

    pub fn log(&self, event: &LogEvent) {
        self.log_sequence(event);
    }
}
