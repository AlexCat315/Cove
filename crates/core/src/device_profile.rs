//! Platform conventions and hardware class. Both stay constant for a process;
//! layout follows the window, never these values.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    #[serde(rename = "macos")]
    MacOS,
    Windows,
    Linux,
    #[serde(rename = "ios")]
    IOS,
    #[serde(rename = "ipados")]
    IPadOS,
    Android,
    Web,
}

impl Platform {
    pub const ALL: [Self; 7] = [
        Self::MacOS,
        Self::Windows,
        Self::Linux,
        Self::IOS,
        Self::IPadOS,
        Self::Android,
        Self::Web,
    ];

    /// The compile target's platform. iOS hosts promote this to `IPadOS` once
    /// the device reports a tablet.
    pub const fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOS
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(any(target_os = "ios", target_os = "visionos")) {
            Self::IOS
        } else if cfg!(target_os = "android") {
            Self::Android
        } else if cfg!(target_family = "wasm") {
            Self::Web
        } else {
            Self::Linux
        }
    }

    pub const fn design_language(self) -> DesignLanguage {
        match self {
            Self::MacOS | Self::IOS | Self::IPadOS => DesignLanguage::Cupertino,
            Self::Android | Self::Web => DesignLanguage::Material,
            Self::Windows => DesignLanguage::Fluent,
            Self::Linux => DesignLanguage::Adwaita,
        }
    }

    pub const fn is_mobile(self) -> bool {
        matches!(self, Self::IOS | Self::IPadOS | Self::Android)
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::MacOS => "macos",
            Self::Windows => "windows",
            Self::Linux => "linux",
            Self::IOS => "ios",
            Self::IPadOS => "ipados",
            Self::Android => "android",
            Self::Web => "web",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|platform| platform.name().eq_ignore_ascii_case(value))
    }
}

impl Default for Platform {
    fn default() -> Self {
        Self::current()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DesignLanguage {
    Cupertino,
    Material,
    Fluent,
    Adwaita,
}

impl DesignLanguage {
    pub const ALL: [Self; 4] = [Self::Cupertino, Self::Material, Self::Fluent, Self::Adwaita];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Cupertino => "cupertino",
            Self::Material => "material",
            Self::Fluent => "fluent",
            Self::Adwaita => "adwaita",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|language| language.name().eq_ignore_ascii_case(value))
    }

    /// Minimum touch target in logical points.
    pub const fn touch_target(self) -> f32 {
        match self {
            Self::Material => 48.0,
            Self::Cupertino | Self::Fluent | Self::Adwaita => 44.0,
        }
    }
}

impl Default for DesignLanguage {
    fn default() -> Self {
        Platform::current().design_language()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FoldableKind {
    /// Fold across the long axis; unfolded it approaches a tablet.
    Book,
    /// Fold across the short axis; folded it shows a cover display.
    Flip,
    /// Two panels separated by an occluding physical hinge.
    DualScreen,
}

impl FoldableKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Book => "book",
            Self::Flip => "flip",
            Self::DualScreen => "dual-screen",
        }
    }
}

/// Hardware class. A foldable keeps its class through every posture, so
/// conventions never change when it opens; its window class does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "&'static str", try_from = "String")]
pub enum DeviceClass {
    Phone,
    Foldable(FoldableKind),
    Tablet,
    #[default]
    Desktop,
}

impl DeviceClass {
    /// Phones and foldables: held in one or two hands, touch first.
    pub const fn is_handheld(self) -> bool {
        matches!(self, Self::Phone | Self::Foldable(_))
    }

    pub const fn is_foldable(self) -> bool {
        matches!(self, Self::Foldable(_))
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Phone => "phone",
            Self::Foldable(FoldableKind::Book) => "foldable-book",
            Self::Foldable(FoldableKind::Flip) => "foldable-flip",
            Self::Foldable(FoldableKind::DualScreen) => "foldable-dual-screen",
            Self::Tablet => "tablet",
            Self::Desktop => "desktop",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [
            Self::Phone,
            Self::Foldable(FoldableKind::Book),
            Self::Foldable(FoldableKind::Flip),
            Self::Foldable(FoldableKind::DualScreen),
            Self::Tablet,
            Self::Desktop,
        ]
        .into_iter()
        .find(|class| class.name().eq_ignore_ascii_case(value))
    }
}

impl From<DeviceClass> for &'static str {
    fn from(value: DeviceClass) -> Self {
        value.name()
    }
}

impl TryFrom<String> for DeviceClass {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).ok_or_else(|| format!("unknown device class {value}"))
    }
}
