use crate::*;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Subscription {
    Tree,
    Log,
    Timing,
    Mirror,
    State,
    Timeline,
}

#[derive(Default)]
pub struct Session {
    pub subscriptions: BTreeSet<Subscription>,
    pub watched_state_ids: Vec<String>,
    pub log_query: cove_logging::LogQuery,
    pub log_sequence: u64,
    pub log_dropped: u64,
    pub timeline_sequence: u64,
}
fn decode<T: DeserializeOwned>(request: &Envelope) -> Option<T> {
    serde_json::from_value(request.payload.clone()?).ok()
}
fn no_payload(request: &Envelope) -> bool {
    request
        .payload
        .as_ref()
        .is_none_or(|p| p.is_null() || p.as_object().is_some_and(|p| p.is_empty()))
}
fn state(payload: &Option<JsonValue>) -> Option<BTreeMap<String, String>> {
    let object = payload.as_ref()?.as_object()?;
    object
        .iter()
        .map(|(k, v)| Some((k.clone(), v.as_str()?.into())))
        .collect()
}
impl Session {
    pub fn set(&mut self, stream: Subscription, enabled: bool) -> bool {
        if enabled {
            self.subscriptions.insert(stream)
        } else {
            self.subscriptions.remove(&stream)
        }
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn validate(&self, request: &Envelope) -> Option<Envelope> {
        if request.r#type.starts_with("inspect.") {
            return inspection_error(request).map(|message| request.error("bad_request", message));
        }
        let valid = match request.r#type.as_str() {
            "log.subscribe" => {
                request.payload.is_none()
                    || request.payload.as_ref().is_some_and(|p| {
                        let Some(p) = p.as_object() else {
                            return false;
                        };
                        let labels = p.get("labels").and_then(JsonValue::as_array);
                        let text = p.get("text").and_then(JsonValue::as_str);
                        p.get("minimum").is_none_or(|v| {
                            serde_json::from_value::<cove_logging::Level>(v.clone()).is_ok()
                        }) && (!p.contains_key("labels")
                            || labels.is_some_and(|a| {
                                a.len() <= 64
                                    && a.iter().all(|v| {
                                        v.as_str().is_some_and(|s| !s.is_empty() && s.len() <= 256)
                                    })
                            }))
                            && (!p.contains_key("text") || text.is_some_and(|s| s.len() <= 4096))
                            && p.get("afterSequence")
                                .is_none_or(|v| v.as_u64().is_some() || v.is_null())
                    })
            }
            "state.subscribe" => decode::<StateWatch>(request).is_some_and(|p| p.is_valid()),
            "state.list" | "state.unsubscribe" | "timeline.subscribe" | "timeline.unsubscribe" => {
                no_payload(request)
            }
            "select.node" => request
                .payload
                .as_ref()
                .and_then(|p| p.get("id"))
                .and_then(JsonValue::as_str)
                .is_some(),
            "state.restore" | "state.diff" => state(&request.payload).is_some(),
            "mirror.input" | "mirror.activate"
                if !self.subscriptions.contains(&Subscription::Mirror) =>
            {
                return Some(request.error(
                    "invalid_state",
                    "mirror commands require an active mirror subscription",
                ));
            }
            "mirror.input" => decode::<MirrorInput>(request).is_some(),
            "mirror.activate" => decode::<MirrorActivate>(request).is_some(),
            "window.resize" => decode::<WindowResize>(request).is_some_and(|p| p.is_valid()),
            "mirror.start" => request.payload.is_none() || decode::<MirrorStart>(request).is_some(),
            "input.replay" => decode::<InputRecording>(request).is_some_and(|p| p.is_valid()),
            _ => true,
        };
        (!valid).then(|| request.error("bad_request", "invalid or unbounded command payload"))
    }
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
pub fn inspection_error(request: &Envelope) -> Option<&'static str> {
    let object = request.payload.as_ref().and_then(JsonValue::as_object);
    match request.r#type.as_str() {
        "inspect.hover" | "inspect.pick" => {
            if !object.is_some_and(|p| {
                exact_keys(p, &["x", "y"])
                    && ["x", "y"].iter().all(|key| {
                        p[*key]
                            .as_f64()
                            .is_some_and(|n| n.is_finite() && n.abs() <= 1_000_000.0)
                    })
            }) {
                return Some("Expected finite window coordinates x/y");
            }
        }
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
        "inspect.pick.start"
        | "inspect.pick.stop"
        | "inspect.style.undo"
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
    #[test]
    fn mirror_requires_subscription_and_inspection_is_strict() {
        let mut session = Session::default();
        let request = Envelope::new(
            "mirror.activate",
            Some(3),
            Some(serde_json::json!({"node_id":"7"})),
        );
        assert_eq!(
            session.validate(&request).unwrap().payload.unwrap()["code"],
            "invalid_state"
        );
        session.set(Subscription::Mirror, true);
        assert!(session.validate(&request).is_none());
        let request = Envelope::new(
            "inspect.style.set",
            None,
            Some(serde_json::json!({
                "id":"7", "properties":{"backgroundColor":"#12345678", "padding":{"top":1,"right":2,"bottom":3,"left":4}}
            })),
        );
        assert!(session.validate(&request).is_none());
        let request = Envelope::new(
            "inspect.hover",
            None,
            Some(serde_json::json!({"x":0,"y":0,"extra":true})),
        );
        assert!(session.validate(&request).is_some());
    }
    #[test]
    fn watched_ids_are_bounded_and_unique() {
        assert!(
            !StateWatch {
                ids: vec!["a".into(), "a".into()]
            }
            .is_valid()
        );
        assert!(
            !StateWatch {
                ids: vec!["a".repeat(257)]
            }
            .is_valid()
        );
        let request = Envelope::new("state.list", Some(5), None);
        assert_eq!(request.ok().unwrap().r#type, "state.list.ok");
    }
}
