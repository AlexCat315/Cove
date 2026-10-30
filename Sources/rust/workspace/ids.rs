//! String newtypes shared by the workspace document and commands.
//!
//! Swift declares one `RawRepresentable` struct per identity kind. Rust keeps
//! them distinct so a panel id can never be passed where a group id is
//! expected, while serialising transparently as the raw string.
use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! identifier {
    ($($name:ident),* $(,)?) => {$(
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);
        impl $name {
            pub fn new(value: impl Into<String>) -> Self { Self(value.into()) }
            pub fn raw_value(&self) -> &str { &self.0 }
            pub fn as_str(&self) -> &str { &self.0 }
        }
        impl From<&str> for $name {
            fn from(value: &str) -> Self { Self(value.into()) }
        }
        impl From<String> for $name {
            fn from(value: String) -> Self { Self(value) }
        }
        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    )*};
}

identifier!(
    WorkspacePanelID,
    WorkspaceTabGroupID,
    WorkspaceSplitID,
    WorkspaceFloatingWindowID,
    WorkspaceSlotID,
);

impl WorkspaceSlotID {
    pub const CONTENT: Self = Self(String::new());
    pub fn content() -> Self { Self("content".into()) }
    pub fn leading() -> Self { Self("leading".into()) }
    pub fn center() -> Self { Self("center".into()) }
    pub fn trailing() -> Self { Self("trailing".into()) }
    pub fn top() -> Self { Self("top".into()) }
    pub fn bottom() -> Self { Self("bottom".into()) }
    pub fn overlay() -> Self { Self("overlay".into()) }
    pub fn floating() -> Self { Self("floating".into()) }

    pub fn standard_editor_slots() -> Vec<Self> {
        vec![Self::leading(), Self::center(), Self::trailing(), Self::bottom()]
    }

    pub fn chrome_leading_rail() -> Self { Self("chrome.leading.rail".into()) }
    pub fn chrome_trailing_rail() -> Self { Self("chrome.trailing.rail".into()) }
    pub fn chrome_top_rail() -> Self { Self("chrome.top.rail".into()) }
    pub fn chrome_bottom_rail() -> Self { Self("chrome.bottom.rail".into()) }

    pub fn chrome_rail(edge: WorkspaceEdge) -> Self {
        match edge {
            WorkspaceEdge::Leading => Self::chrome_leading_rail(),
            WorkspaceEdge::Trailing => Self::chrome_trailing_rail(),
            WorkspaceEdge::Top => Self::chrome_top_rail(),
            WorkspaceEdge::Bottom => Self::chrome_bottom_rail(),
        }
    }
}

impl Default for WorkspaceSlotID {
    fn default() -> Self { Self::content() }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceSplitAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceEdge {
    Leading,
    Trailing,
    Top,
    Bottom,
}

impl WorkspaceEdge {
    pub fn split_axis(self) -> WorkspaceSplitAxis {
        match self {
            Self::Leading | Self::Trailing => WorkspaceSplitAxis::Horizontal,
            Self::Top | Self::Bottom => WorkspaceSplitAxis::Vertical,
        }
    }
    pub fn inserts_before_anchor(self) -> bool {
        matches!(self, Self::Leading | Self::Top)
    }
    pub fn raw_value(self) -> &'static str {
        match self {
            Self::Leading => "leading",
            Self::Trailing => "trailing",
            Self::Top => "top",
            Self::Bottom => "bottom",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkspacePlacementKind {
    TabGroup,
    Slot,
    Split,
}
