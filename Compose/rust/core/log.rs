//! Compose's structured logging channel.
use cove_logging::{Configuration, Level, Location, LogRuntime};
use std::sync::{Arc, OnceLock};

#[derive(Clone, Debug)]
pub struct Logger {
    label: Arc<str>,
}

impl Logger {
    pub fn new(label: impl Into<Arc<str>>) -> Self {
        Self { label: label.into() }
    }

    pub fn compose() -> Self {
        Self::new("com.cove.ui.compose")
    }

    pub fn log(&self, level: Level, message: impl AsRef<str>) {
        let _ = runtime().log(level, &self.label, message.as_ref(), Default::default(), Location::default());
    }

    pub fn warning(&self, message: impl AsRef<str>) {
        self.log(Level::Warning, message);
    }

    pub fn error(&self, message: impl AsRef<str>) {
        self.log(Level::Error, message);
    }
}

fn runtime() -> &'static LogRuntime {
    static RUNTIME: OnceLock<LogRuntime> = OnceLock::new();
    RUNTIME.get_or_init(|| LogRuntime::new(Configuration::default()).expect("default logging configuration is valid"))
}
