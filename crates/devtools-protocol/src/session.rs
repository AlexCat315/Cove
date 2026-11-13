use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn no_payload(request: &Envelope) -> bool {
    request
        .payload
        .as_ref()
        .is_none_or(|p| p.is_null() || p.as_object().is_some_and(|p| p.is_empty()))
}
fn exact_keys(object: &serde_json::Map<String, JsonValue>, keys: &[&str]) -> bool {
    object.len() == keys.len() && keys.iter().all(|key| object.contains_key(*key))
}
fn valid_id(value: Option<&JsonValue>) -> bool {
    value
        .and_then(JsonValue::as_str)
        .is_some_and(|s| !s.is_empty() && s.len() <= 128)
}
fn valid_color(value: &JsonValue) -> bool {
    value.as_str().is_some_and(|s| {
        (s.len() == 7 || s.len() == 9)
            && s.starts_with('#')
            && s.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
    })
}

/// Every property `inspect.style.set` accepts. A `null` value clears that one
/// property and restores its authored value.
pub const STYLE_PROPERTIES: [&str; 7] = [
    "padding",
    "backgroundColor",
    "foregroundColor",
    "flexGrow",
    "flexShrink",
    "justifyContent",
    "alignItems",
];

/// Properties that only exist on a node with a layout box.
pub const LAYOUT_PROPERTIES: [&str; 5] = [
    "padding",
    "flexGrow",
    "flexShrink",
    "justifyContent",
    "alignItems",
];

/// Whether this property needs a layout box on the node.
pub fn is_layout_property(name: &str) -> bool {
    LAYOUT_PROPERTIES.contains(&name)
}

/// Upper bound for a temporary flex grow or shrink factor.
pub const MAX_FLEX_FACTOR: f64 = 1000.0;

/// `cove_core::scene::Justify` names, spelled as `NodeLayoutInfo` reports them.
pub const JUSTIFY_CONTENT: [&str; 6] = [
    "FlexStart",
    "Center",
    "FlexEnd",
    "SpaceBetween",
    "SpaceAround",
    "SpaceEvenly",
];

/// `cove_core::scene::Align` names, spelled as `NodeLayoutInfo` reports them.
/// `alignItems` accepts every one; `justifyContent` does not take `Auto`,
/// `Stretch` or `Baseline`.
pub const ALIGN_ITEMS: [&str; 9] = [
    "Auto",
    "FlexStart",
    "Center",
    "FlexEnd",
    "Stretch",
    "Baseline",
    "SpaceBetween",
    "SpaceAround",
    "SpaceEvenly",
];

/// Swift's `InspectionValidation`: the command allowlist plus the payload
/// predicates the session uses to reject a malformed inspection request.
pub struct InspectionValidation;

impl InspectionValidation {
    /// Every inspection command. A name outside this set is unknown, not
    /// merely invalid.
    pub const COMMANDS: [&'static str; 6] = [
        "inspect.style.set",
        "inspect.style.undo",
        "inspect.style.redo",
        "inspect.style.clear",
        "inspect.style.clearAll",
        "inspect.recomposition.reset",
    ];

    pub fn is_command(kind: &str) -> bool {
        Self::COMMANDS.contains(&kind)
    }

    /// Node identifiers are non-empty and at most 128 bytes.
    pub fn valid_id(value: Option<&JsonValue>) -> bool {
        valid_id(value)
    }

    /// `#RRGGBB` or `#RRGGBBAA`.
    pub fn valid_color(value: &JsonValue) -> bool {
        valid_color(value)
    }

    /// A finite JSON number.
    pub fn number(value: Option<&JsonValue>) -> Option<f64> {
        value
            .and_then(JsonValue::as_f64)
            .filter(|number| number.is_finite())
    }

    /// The reason a request cannot run, or `None` when it is valid.
    pub fn error(request: &Envelope) -> Option<&'static str> {
        inspection_error(request)
    }
}

pub fn inspection_error(request: &Envelope) -> Option<&'static str> {
    let object = request.payload.as_ref().and_then(JsonValue::as_object);
    match request.r#type.as_str() {
        "inspect.style.clear" => {
            if !object.is_some_and(|p| exact_keys(p, &["id"]) && valid_id(p.get("id"))) {
                return Some("Expected node id");
            }
        }
        "inspect.style.set" => {
            let Some(p) =
                object.filter(|p| exact_keys(p, &["id", "properties"]) && valid_id(p.get("id")))
            else {
                return Some("Expected node id and supported properties");
            };
            let Some(properties) = p["properties"].as_object().filter(|p| !p.is_empty()) else {
                return Some("Expected node id and supported properties");
            };
            for (key, value) in properties {
                if !STYLE_PROPERTIES.contains(&key.as_str()) {
                    return Some("Unsupported style property");
                }
                if value.is_null() {
                    continue;
                }
                match key.as_str() {
                    "padding" => {
                        if !value.as_object().is_some_and(|p| {
                            exact_keys(p, &["top", "right", "bottom", "left"])
                                && p.values().all(|v| {
                                    v.as_f64().is_some_and(|n| (0.0..=4096.0).contains(&n))
                                })
                        }) {
                            return Some(
                                "Padding requires top/right/bottom/left in 0..4096 points",
                            );
                        }
                    }
                    "backgroundColor" | "foregroundColor" => {
                        if !valid_color(value) {
                            return Some("Colors require #RRGGBB or #RRGGBBAA");
                        }
                    }
                    "flexGrow" | "flexShrink" => {
                        if !value.as_f64().is_some_and(|number| {
                            number.is_finite() && (0.0..=MAX_FLEX_FACTOR).contains(&number)
                        }) {
                            return Some("Flex grow and shrink require numbers in 0..1000");
                        }
                    }
                    "justifyContent" => {
                        if !value
                            .as_str()
                            .is_some_and(|name| JUSTIFY_CONTENT.contains(&name))
                        {
                            return Some(
                                "Justify content requires FlexStart, Center, FlexEnd, SpaceBetween, SpaceAround or SpaceEvenly",
                            );
                        }
                    }
                    "alignItems" => {
                        if !value
                            .as_str()
                            .is_some_and(|name| ALIGN_ITEMS.contains(&name))
                        {
                            return Some(
                                "Align items requires Auto, FlexStart, Center, FlexEnd, Stretch, Baseline, SpaceBetween, SpaceAround or SpaceEvenly",
                            );
                        }
                    }
                    _ => return Some("Unsupported style property"),
                }
            }
        }
        "inspect.style.undo"
        | "inspect.style.redo"
        | "inspect.style.clearAll"
        | "inspect.recomposition.reset" => {
            if !no_payload(request) {
                return Some("This command takes no properties");
            }
        }
        _ => return Some("Unknown inspection command"),
    }
    None
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StateChange {
    pub before: String,
    pub after: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StateDifference {
    pub added: BTreeMap<String, String>,
    pub removed: BTreeMap<String, String>,
    pub changed: BTreeMap<String, StateChange>,
}
impl StateDifference {
    pub fn new(before: &BTreeMap<String, String>, after: &BTreeMap<String, String>) -> Self {
        Self {
            added: after
                .iter()
                .filter(|(k, _)| !before.contains_key(*k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            removed: before
                .iter()
                .filter(|(k, _)| !after.contains_key(*k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            changed: before
                .iter()
                .filter_map(|(k, old)| {
                    after.get(k).filter(|new| old != *new).map(|new| {
                        (
                            k.clone(),
                            StateChange {
                                before: old.clone(),
                                after: new.clone(),
                            },
                        )
                    })
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(properties: serde_json::Value) -> Option<&'static str> {
        inspection_error(&Envelope::new(
            "inspect.style.set",
            Some(1),
            Some(serde_json::json!({"id": "node", "properties": properties})),
        ))
    }

    #[test]
    fn flex_properties_accept_their_real_ranges_and_names() {
        assert_eq!(set(serde_json::json!({"flexGrow": 0})), None);
        assert_eq!(set(serde_json::json!({"flexShrink": 1000})), None);
        assert_eq!(set(serde_json::json!({"flexGrow": 1.5})), None);
        for name in JUSTIFY_CONTENT {
            assert_eq!(
                set(serde_json::json!({"justifyContent": name})),
                None,
                "{name}"
            );
        }
        for name in ALIGN_ITEMS {
            assert_eq!(set(serde_json::json!({"alignItems": name})), None, "{name}");
        }
        assert_eq!(
            set(serde_json::json!({
                "flexGrow": 1,
                "flexShrink": 2,
                "justifyContent": "SpaceBetween",
                "alignItems": "Baseline",
            })),
            None
        );
    }

    #[test]
    fn flex_properties_reject_values_the_layout_node_cannot_take() {
        assert!(set(serde_json::json!({"flexGrow": -1})).is_some());
        assert!(set(serde_json::json!({"flexGrow": 1000.5})).is_some());
        assert!(set(serde_json::json!({"flexShrink": "1"})).is_some());
        assert!(set(serde_json::json!({"flexGrow": true})).is_some());
        assert!(set(serde_json::json!({"justifyContent": "flex-start"})).is_some());
        assert!(set(serde_json::json!({"justifyContent": "Auto"})).is_some());
        assert!(set(serde_json::json!({"justifyContent": "Baseline"})).is_some());
        assert!(set(serde_json::json!({"alignItems": "Start"})).is_some());
        assert!(set(serde_json::json!({"alignItems": 1})).is_some());
        assert!(set(serde_json::json!({"flexBasis": 1})).is_some());
    }

    #[test]
    fn null_clears_one_property_and_leaves_the_rest_validated() {
        assert_eq!(set(serde_json::json!({"flexGrow": null})), None);
        assert_eq!(set(serde_json::json!({"justifyContent": null})), None);
        assert!(set(serde_json::json!({"flexGrow": null, "justifyContent": "Start"})).is_some());
    }

    #[test]
    fn style_property_names_are_the_ones_the_editor_understands() {
        assert_eq!(
            STYLE_PROPERTIES,
            [
                "padding",
                "backgroundColor",
                "foregroundColor",
                "flexGrow",
                "flexShrink",
                "justifyContent",
                "alignItems",
            ]
        );
        assert!(LAYOUT_PROPERTIES.contains(&"padding"));
        assert!(!LAYOUT_PROPERTIES.contains(&"backgroundColor"));
        assert!(is_layout_property("flexGrow"));
        assert!(!is_layout_property("foregroundColor"));
    }
}
