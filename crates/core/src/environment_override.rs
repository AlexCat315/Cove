//! Replaces host-reported facts, for brand-wide design languages, embedded
//! device previews and acceptance runs of other platforms on a desktop.
use serde::{Deserialize, Serialize};

use crate::{
    DesignLanguage, DeviceClass, DisplayFeature, Environment, FeatureOrientation, FoldableKind,
    InputCapabilities, Insets, Platform, Posture, Rect, Size,
};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EnvironmentOverride {
    pub platform: Option<Platform>,
    pub design_language: Option<DesignLanguage>,
    pub device_class: Option<DeviceClass>,
    pub posture: Option<Posture>,
    pub display_features: Option<Vec<DisplayFeature>>,
    pub input: Option<InputCapabilities>,
    /// Lower bound for the host's safe area, so a desktop window previewing a
    /// phone keeps space for its status bar and home indicator.
    pub minimum_safe_area: Option<Insets>,
}

impl EnvironmentOverride {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// A platform without an explicit design language implies that platform's.
    pub fn apply(&self, environment: &mut Environment) {
        if let Some(platform) = self.platform {
            environment.platform = platform;
            environment.design_language = platform.design_language();
        }
        if let Some(language) = self.design_language {
            environment.design_language = language;
        }
        if let Some(class) = self.device_class {
            environment.device_class = class;
        }
        if let Some(posture) = self.posture {
            environment.posture = posture;
        }
        if let Some(features) = &self.display_features {
            environment.display_features = features
                .iter()
                .filter_map(|feature| feature.normalized())
                .collect();
        }
        if let Some(input) = self.input {
            environment.input = input;
        }
        if let Some(minimum) = self.minimum_safe_area {
            let area = &mut environment.safe_area;
            area.top = area.top.max(minimum.top);
            area.bottom = area.bottom.max(minimum.bottom);
            area.leading = area.leading.max(minimum.leading);
            area.trailing = area.trailing.max(minimum.trailing);
        }
    }

    /// Fields set in `other` replace this override's.
    pub fn merged(mut self, other: &Self) -> Self {
        macro_rules! take {
            ($($field:ident),*) => { $(if other.$field.is_some() { self.$field = other.$field.clone(); })* };
        }
        take!(
            platform,
            design_language,
            device_class,
            posture,
            display_features,
            input,
            minimum_safe_area
        );
        self
    }
}

/// A complete device configuration: window size plus the facts its host
/// would report. Sizes are typical values, not specific models.
#[derive(Clone, Debug, PartialEq)]
pub struct DevicePreset {
    pub name: &'static str,
    pub size: Size,
    pub environment: EnvironmentOverride,
}

impl DevicePreset {
    pub const NAMES: [&'static str; 13] = [
        "iphone",
        "ipad-split",
        "android-phone",
        "android-tablet",
        "fold-cover",
        "fold-open",
        "fold-book",
        "fold-tabletop",
        "flip-open",
        "dual-screen",
        "macos",
        "windows",
        "linux",
    ];

    pub fn all() -> impl Iterator<Item = Self> {
        Self::NAMES.into_iter().filter_map(Self::named)
    }

    pub fn named(name: &str) -> Option<Self> {
        let touch = InputCapabilities::TOUCH | InputCapabilities::KEYBOARD;
        let desktop = InputCapabilities::POINTER | InputCapabilities::KEYBOARD;
        let ios_bars = Insets {
            top: 59.0,
            bottom: 34.0,
            ..Insets::default()
        };
        let android_bars = Insets {
            top: 24.0,
            bottom: 24.0,
            ..Insets::default()
        };
        let mobile =
            |platform, class, bars, posture, features: Vec<DisplayFeature>| EnvironmentOverride {
                platform: Some(platform),
                device_class: Some(class),
                posture: Some(posture),
                display_features: Some(features),
                input: Some(touch),
                minimum_safe_area: Some(bars),
                ..EnvironmentOverride::default()
            };
        let book = DeviceClass::Foldable(FoldableKind::Book);
        let vertical_fold = |width: f32, height: f32, separates| {
            DisplayFeature::fold(
                Rect::new(width * 0.5, 0.0, 0.0, height),
                FeatureOrientation::Vertical,
                separates,
            )
        };
        let horizontal_fold = |width: f32, height: f32, separates| {
            DisplayFeature::fold(
                Rect::new(0.0, height * 0.5, width, 0.0),
                FeatureOrientation::Horizontal,
                separates,
            )
        };
        let (size, environment) = match name.to_ascii_lowercase().as_str() {
            "iphone" => (
                Size::new(393.0, 852.0),
                mobile(
                    Platform::IOS,
                    DeviceClass::Phone,
                    ios_bars,
                    Posture::Flat,
                    vec![],
                ),
            ),
            "ipad-split" => (
                Size::new(694.0, 1024.0),
                mobile(
                    Platform::IPadOS,
                    DeviceClass::Tablet,
                    Insets {
                        top: 24.0,
                        bottom: 20.0,
                        ..Insets::default()
                    },
                    Posture::Flat,
                    vec![],
                ),
            ),
            "android-phone" => (
                Size::new(412.0, 915.0),
                mobile(
                    Platform::Android,
                    DeviceClass::Phone,
                    android_bars,
                    Posture::Flat,
                    vec![],
                ),
            ),
            "android-tablet" => (
                Size::new(1280.0, 800.0),
                mobile(
                    Platform::Android,
                    DeviceClass::Tablet,
                    android_bars,
                    Posture::Flat,
                    vec![],
                ),
            ),
            "fold-cover" => (
                Size::new(360.0, 800.0),
                mobile(Platform::Android, book, android_bars, Posture::Flat, vec![]),
            ),
            "fold-open" => (
                Size::new(841.0, 701.0),
                mobile(
                    Platform::Android,
                    book,
                    android_bars,
                    Posture::Flat,
                    vec![vertical_fold(841.0, 701.0, false)],
                ),
            ),
            "fold-book" => (
                Size::new(841.0, 701.0),
                mobile(
                    Platform::Android,
                    book,
                    android_bars,
                    Posture::Book,
                    vec![vertical_fold(841.0, 701.0, true)],
                ),
            ),
            "fold-tabletop" => (
                Size::new(701.0, 841.0),
                mobile(
                    Platform::Android,
                    book,
                    android_bars,
                    Posture::Tabletop,
                    vec![horizontal_fold(701.0, 841.0, true)],
                ),
            ),
            "flip-open" => (
                Size::new(412.0, 915.0),
                mobile(
                    Platform::Android,
                    DeviceClass::Foldable(FoldableKind::Flip),
                    android_bars,
                    Posture::Flat,
                    vec![horizontal_fold(412.0, 915.0, false)],
                ),
            ),
            "dual-screen" => (
                Size::new(1114.0, 705.0),
                mobile(
                    Platform::Android,
                    DeviceClass::Foldable(FoldableKind::DualScreen),
                    android_bars,
                    Posture::Flat,
                    vec![DisplayFeature::hinge(
                        Rect::new(540.0, 0.0, 34.0, 705.0),
                        FeatureOrientation::Vertical,
                    )],
                ),
            ),
            platform @ ("macos" | "windows" | "linux") => (
                Size::new(1280.0, 800.0),
                EnvironmentOverride {
                    platform: Platform::parse(platform),
                    device_class: Some(DeviceClass::Desktop),
                    posture: Some(Posture::Flat),
                    display_features: Some(vec![]),
                    input: Some(desktop),
                    ..EnvironmentOverride::default()
                },
            ),
            _ => return None,
        };
        let name = Self::NAMES
            .into_iter()
            .find(|candidate| candidate.eq_ignore_ascii_case(name))?;
        Some(Self {
            name,
            size,
            environment,
        })
    }
}

/// A DevTools environment change: optionally reset, then optionally start from
/// a preset, then apply the explicit fields on top.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EnvironmentRequest {
    pub clear: bool,
    pub preset: Option<String>,
    #[serde(flatten)]
    pub environment: EnvironmentOverride,
}

impl EnvironmentRequest {
    /// The override that results from applying this request to `current`, and
    /// the window size the preset asks for.
    pub fn resolve(
        &self,
        current: &EnvironmentOverride,
    ) -> Result<(EnvironmentOverride, Option<Size>), String> {
        let mut base = if self.clear {
            EnvironmentOverride::default()
        } else {
            current.clone()
        };
        let mut size = None;
        if let Some(name) = &self.preset {
            let preset = DevicePreset::named(name).ok_or_else(|| {
                format!(
                    "unknown preset {name}; expected one of {}",
                    DevicePreset::NAMES.join(", ")
                )
            })?;
            base = preset.environment;
            size = Some(preset.size);
        }
        Ok((base.merged(&self.environment), size))
    }
}
