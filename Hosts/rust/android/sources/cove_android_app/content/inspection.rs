//! Translation of Android/CoveAndroidApp/Content/Inspection.swift and
//! Content/InputSnapshot.swift.

use super::scene::Bounds;
use cove_core::scene::AccessibilityElement;
use cove_core::scene::TextInputSnapshot;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TypographyStatistics {
    pub fonts: usize,
    pub glyphs: usize,
    pub missing_glyphs: usize,
    pub atlas_pixels: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SemanticNode {
    pub id: String,
    pub role: String,
    pub label: String,
    pub value: String,
    pub bounds: Bounds,
    pub enabled: bool,
    pub selected: bool,
}

/// The IME and touch routing need only the focused editor, not a traversal and
/// serialization of every semantic node and diagnostic control bound.
#[derive(Clone, Debug, Serialize)]
pub struct InputSnapshot {
    pub focused: Option<String>,
    pub editor: Option<TextInputSnapshot>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Snapshot<'a> {
    pub session: &'a str,
    pub state: BTreeMap<String, String>,
    pub focused: Option<String>,
    pub preedit: String,
    pub controls: BTreeMap<String, Bounds>,
    pub width: f32,
    pub height: f32,
    pub scale: f32,
    pub typography: TypographyStatistics,
    pub vertices: usize,
    pub indices: usize,
    pub frames: u64,
    pub mode: &'static str,
    pub page: Option<String>,
    pub editor: Option<TextInputSnapshot>,
    pub nodes: Vec<SemanticNode>,
}

/// Flattens the accessibility snapshot into the diagnostic node list, one entry
/// per element in tree order.
pub fn semantic_nodes(elements: &[AccessibilityElement]) -> Vec<SemanticNode> {
    fn flatten(element: &AccessibilityElement, nodes: &mut Vec<SemanticNode>) {
        let semantics = &element.semantics;
        nodes.push(SemanticNode {
            id: element.id().0.to_string(),
            role: serde_json::to_value(semantics.role)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_default(),
            label: semantics.label.clone(),
            value: semantics.value.clone(),
            bounds: Bounds::new(element.frame),
            enabled: semantics.state.is_enabled,
            selected: semantics.state.is_selected,
        });
        for child in &element.children {
            flatten(child, nodes);
        }
    }
    let mut nodes = Vec::new();
    for element in elements {
        flatten(element, &mut nodes);
    }
    nodes
}
