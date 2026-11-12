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
                if !["padding", "backgroundColor", "foregroundColor"].contains(&key.as_str()) {
                    return Some("Unsupported style property");
                }
                if value.is_null() {
                    continue;
                }
                if key == "padding" {
                    if !value.as_object().is_some_and(|p| {
                        exact_keys(p, &["top", "right", "bottom", "left"])
                            && p.values()
                                .all(|v| v.as_f64().is_some_and(|n| (0.0..=4096.0).contains(&n)))
                    }) {
                        return Some("Padding requires top/right/bottom/left in 0..4096 points");
                    }
                } else if !valid_color(value) {
                    return Some("Colors require #RRGGBB or #RRGGBBAA");
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
