//! Translation of CoveAndroidContract/Launch.swift.

use serde::{Deserialize, Serialize};

/// Authored launch content shared by the APK producer and native consumer.
/// Transient benchmark, timing, input and display state never enter this
/// document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SceneLaunchMode {
    Counter,
    #[default]
    Gallery,
    Game,
    Application,
}

impl SceneLaunchMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Counter => "counter",
            Self::Gallery => "gallery",
            Self::Game => "game",
            Self::Application => "application",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SceneLaunch {
    pub mode: SceneLaunchMode,
    pub component: String,
    pub project: Option<String>,
}

impl Default for SceneLaunch {
    fn default() -> Self {
        Self { mode: SceneLaunchMode::Gallery, component: "introduction".to_owned(), project: None }
    }
}

impl SceneLaunch {
    pub fn new(mode: SceneLaunchMode, component: impl Into<String>, project: Option<String>) -> Self {
        Self { mode, component: component.into(), project }
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}
