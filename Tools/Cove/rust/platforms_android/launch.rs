//! Authored launch document written to `assets/launch.json`.
//!
//! Swift's `CoveAndroidContract` is a standalone package, but the Rust
//! translation of `SceneLaunch` lives inside `cove-hosts`, which re-exports it
//! through modules that require the Compose UI crate. The CLI must package
//! without the UI stack, so the wire shape is restated here verbatim: the same
//! field names, the same `lowercase` mode encoding and the same defaults.

use serde::{Deserialize, Serialize};

/// Which example the Android host launches.
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
    #[allow(dead_code)]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Counter => "counter",
            Self::Gallery => "gallery",
            Self::Game => "game",
            Self::Application => "application",
        }
    }
}

/// Authored launch content shared by the APK producer and native consumer.
///
/// Transient benchmark, timing, input and display state never enter this
/// document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SceneLaunch {
    pub mode: SceneLaunchMode,
    pub component: String,
    pub project: Option<String>,
}

impl Default for SceneLaunch {
    fn default() -> Self {
        Self {
            mode: SceneLaunchMode::Gallery,
            component: "introduction".into(),
            project: None,
        }
    }
}

impl SceneLaunch {
    pub fn new(
        mode: SceneLaunchMode,
        component: impl Into<String>,
        project: Option<String>,
    ) -> Self {
        Self {
            mode,
            component: component.into(),
            project,
        }
    }
}
