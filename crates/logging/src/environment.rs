//! Translation of `Logging/Sources/CoveLogCore/Configuration/Environment.swift`.
use super::configuration::{ConsoleFormat, LoggingConfiguration, LoggingError};
use super::record::Level;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogLaunch {
    pub configuration: LoggingConfiguration,
    #[serde(rename = "runID")]
    pub run_id: String,
    pub application: String,
}
impl LogLaunch {
    pub fn new(
        configuration: LoggingConfiguration,
        run_id: impl Into<String>,
        application: impl Into<String>,
    ) -> Self {
        Self {
            configuration,
            run_id: run_id.into(),
            application: application.into(),
        }
    }
}

impl LoggingConfiguration {
    /// Environment overrides are process-local and never mutate authored
    /// configuration. `environment` mirrors `ProcessInfo.processInfo.environment`.
    pub fn apply_environment(
        &mut self,
        environment: &BTreeMap<String, String>,
    ) -> Result<(), LoggingError> {
        if let Some(value) = environment.get("COVE_LOG_LEVEL") {
            self.filtering.minimum =
                Level::parse(value).ok_or(LoggingError::InvalidConfiguration)?;
        }
        if let Some(value) = environment.get("COVE_LOG_MODULES") {
            self.filtering.modules =
                serde_json::from_str(value).map_err(|_| LoggingError::InvalidConfiguration)?;
        }
        if let Some(value) = environment.get("COVE_LOG_CONSOLE") {
            self.console.enabled = parse_flag(value)?;
        }
        if let Some(value) = environment.get("COVE_LOG_FILES") {
            self.storage.enabled = parse_flag(value)?;
        }
        if let Some(value) = environment.get("COVE_LOG_DIRECTORY") {
            self.storage.directory = Some(PathBuf::from(value));
        }
        if let Some(value) = environment.get("COVE_LOG_FORMAT") {
            self.console.format = match value.as_str() {
                "text" => ConsoleFormat::Text,
                "json" => ConsoleFormat::Json,
                _ => return Err(LoggingError::InvalidConfiguration),
            };
        }
        #[cfg(target_os = "wasi")]
        {
            // The crates/browser/WASI host has no persistent file sink.
            self.storage.enabled = false;
        }
        self.validate()
    }

    pub fn apply_process_environment(&mut self) -> Result<(), LoggingError> {
        let environment = std::env::vars().collect();
        self.apply_environment(&environment)
    }
}

fn parse_flag(value: &str) -> Result<bool, LoggingError> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" => Ok(true),
        "0" | "false" | "no" => Ok(false),
        _ => Err(LoggingError::InvalidConfiguration),
    }
}
