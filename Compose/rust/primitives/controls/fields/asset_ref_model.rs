//! Translation of `Primitives/Controls/Fields/AssetRefField.swift` (the model
//! half: kind matching, picker filtering and the field description).
//!
//! Kind strings are free-form (`"image"`, `"Texture"`, `"Static Mesh"`), so they
//! match by keyword rather than equality — the full kind stays in the tooltip and
//! the field description.
//!
//! Behaviour checklist:
//! - an unknown or absent kind falls back to the generic file icon
//! - matching is lowercase substring matching over the kind
//! - the first matching group wins, so group order is the precedence
//! - an empty `accepted_kinds` accepts every picker option
//! - a non-empty set filters by exact kind
//! - the field description joins kind and subtitle, skipping absent parts
//! - an empty field that accepts exactly one kind hints that kind

use std::collections::BTreeSet;

/// A reference to an asset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetRef {
    pub id: String,
    pub name: String,
    pub subtitle: Option<String>,
    pub kind: String,
    pub preview_path: Option<String>,
}

impl AssetRef {
    pub fn new(id: impl Into<String>, name: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            subtitle: None,
            kind: kind.into(),
            preview_path: None,
        }
    }

    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn with_preview_path(mut self, path: impl Into<String>) -> Self {
        self.preview_path = Some(path.into());
        self
    }
}

/// The icon category an asset kind maps to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetIconKind {
    Image,
    Mesh,
    Material,
    Audio,
    Video,
    Scene,
    Script,
    Text,
    Package,
    /// Anything unrecognised.
    File,
}

impl AssetIconKind {
    /// The keyword groups, in precedence order.
    ///
    /// Order is the contract: the first group with any matching keyword wins, so
    /// `"image shader"` reads as an image because that group is earlier.
    const GROUPS: [(&'static [&'static str], AssetIconKind); 9] = [
        (&["image", "texture", "sprite", "png", "jpg", "jpeg", "webp", "svg"], AssetIconKind::Image),
        (&["mesh", "model", "gltf", "glb", "obj"], AssetIconKind::Mesh),
        (&["material", "shader"], AssetIconKind::Material),
        (&["audio", "sound", "music", "wav", "mp3"], AssetIconKind::Audio),
        (&["video", "movie", "mp4"], AssetIconKind::Video),
        (&["scene", "prefab", "level"], AssetIconKind::Scene),
        (&["script", "code", "swift"], AssetIconKind::Script),
        (&["text", "json", "document"], AssetIconKind::Text),
        (&["package", "bundle", "archive"], AssetIconKind::Package),
    ];
}

/// The icon category for an asset kind, or `File` when nothing matches.
pub fn icon_for(kind: Option<&str>) -> AssetIconKind {
    let Some(kind) = kind.map(str::to_lowercase) else { return AssetIconKind::File };
    AssetIconKind::GROUPS
        .iter()
        .find(|(keywords, _)| keywords.iter().any(|keyword| kind.contains(keyword)))
        .map(|(_, icon)| *icon)
        .unwrap_or(AssetIconKind::File)
}

/// Which picker options the field offers.
///
/// An empty accepted set means "anything", which is how a general-purpose asset
/// slot is authored.
pub fn picker_options<'a>(
    options: &'a [AssetRef],
    accepted_kinds: &BTreeSet<String>,
) -> Vec<&'a AssetRef> {
    options
        .iter()
        .filter(|option| accepted_kinds.is_empty() || accepted_kinds.contains(&option.kind))
        .collect()
}

/// The description shown under the field: kind and subtitle joined by ` · `.
pub fn field_description(reference: Option<&AssetRef>) -> String {
    let Some(reference) = reference else { return String::new() };
    [Some(reference.kind.as_str()), reference.subtitle.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ")
}

/// The kind an empty field should hint at.
///
/// Only a slot that accepts exactly one kind hints; a multi-kind slot shows the
/// generic icon instead of picking one arbitrarily.
pub fn placeholder_kind<'a>(accepted_kinds: &'a BTreeSet<String>) -> Option<&'a str> {
    if accepted_kinds.len() == 1 {
        accepted_kinds.iter().next().map(String::as_str)
    } else {
        None
    }
}

/// The tooltip for an asset tile: the full kind, or a stand-in when absent.
pub fn field_tooltip(kind: Option<&str>) -> &str {
    kind.unwrap_or("No asset")
}

/// Whether the reference's preview path is usable.
pub fn has_preview(reference: Option<&AssetRef>) -> bool {
    reference
        .and_then(|reference| reference.preview_path.as_deref())
        .is_some_and(|path| !path.is_empty())
}
