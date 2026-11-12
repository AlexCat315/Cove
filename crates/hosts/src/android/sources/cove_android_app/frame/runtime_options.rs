//! Translation of Android/CoveAndroidApp/Frame/RuntimeOptions.swift.

use cove_logging::{LogLaunch, LoggingConfiguration};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};

/// Process-local diagnostics; these options never enter a scene checkpoint.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DevToolsLaunch {
    pub endpoint: String,
    pub token: String,
    pub app_title: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct RuntimeOptions {
    pub benchmark: bool,
    pub timings: bool,
    pub vsync: bool,
    pub logging: LogLaunch,
    pub devtools: Option<DevToolsLaunch>,
}

impl Default for RuntimeOptions {
    fn default() -> Self {
        Self {
            benchmark: false,
            timings: false,
            vsync: true,
            logging: LogLaunch::new(
                LoggingConfiguration::default(),
                generated_run_id(),
                "com.cove.android.application",
            ),
            devtools: None,
        }
    }
}

impl RuntimeOptions {
    pub fn from_json(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}

fn generated_run_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{nanos:032x}")
}
