//! Transport-neutral Cove diagnostics and DevTools messages.
//!
//! Swift's `JSONValue` maps to `JsonValue` below — deliberately a re-export of
//! `serde_json::Value`, not a second type: that enum already has exactly Swift's
//! variants (`Null`/`Bool`/`Number`/`String`/`Array`/`Object`) and its accessors
//! are Swift's `stringValue`/`objectValue`. Keeping one type means a payload can
//! never hold two incompatible JSON trees.
pub mod control;
mod session;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
pub use serde_json::Value as JsonValue;
pub use session::*;

macro_rules! payload {
    ($name:ident { $($(#[$attr:meta])* $field:ident : $kind:ty),* $(,)? }) => {
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct $name { $($(#[$attr])* pub $field: $kind),* }
    }
}

payload!(Envelope { r#type: String, id: Option<i64>, payload: Option<JsonValue> });
impl Envelope {
    pub fn new(kind: impl Into<String>, id: Option<i64>, payload: Option<JsonValue>) -> Self {
        Self {
            r#type: kind.into(),
            id,
            payload,
        }
    }
    pub fn ok(&self) -> Option<Self> {
        self.id
            .map(|id| Self::new(format!("{}.ok", self.r#type), Some(id), None))
    }
    pub fn error(&self, code: &str, message: &str) -> Self {
        Self::new(
            format!("{}.err", self.r#type),
            self.id,
            Some(serde_json::json!({ "code": code, "message": message })),
        )
    }
}
payload!(NodeFrame {
    x: f64,
    y: f64,
    w: f64,
    h: f64
});
payload!(NodePoint { x: f64, y: f64 });
payload!(NodeFlags {
    hit_testable: bool,
    focusable: bool,
    clips_to_bounds: bool,
    has_background: bool,
    has_border: bool
});
payload!(NodeSummary {
    id: String, view_tag: Option<String>, debug_name: Option<String>, frame: NodeFrame,
    absolute_frame: Option<NodeFrame>, content_offset: Option<NodePoint>, z_index: Option<f64>, opacity: Option<f64>,
    layout_role: Option<String>, semantic_role: Option<String>, flags: NodeFlags, children: Vec<NodeSummary>,
    layout: Option<NodeLayoutInfo>, style: Option<NodeStyleInfo>, source: Option<SourceLocation>,
    #[serde(rename = "ownerScopeID")] owner_scope_id: Option<String>, recomposition: Option<Recomposition>,
});
payload!(TreeSnapshot {
    root: Option<NodeSummary>, invalidations: Option<Vec<InvalidationRecord>>,
    render_inventory: Option<RenderInventory>, input_inventory: Option<InputInventory>, inspection: Option<InspectionState>,
});
payload!(InvalidationRecord {
    target: String,
    source: String,
    phase: String,
    timestamp: f64
});
payload!(RenderInventory { object_count: usize, layer_roots: Vec<String> });
payload!(InputInventory { node_count: usize, focusables: Vec<String>, hit_testables: Vec<String> });
payload!(WindowResize {
    width: f64,
    height: f64
});
impl WindowResize {
    pub fn is_valid(&self) -> bool {
        (1.0..=16384.0).contains(&self.width) && (1.0..=16384.0).contains(&self.height)
    }
}
payload!(TimingFrame {
    frame: u64, recomposition_ms: Option<f64>, layout_ms: f64, draw_ms: f64,
    render_thread_ms: Option<f64>, submitted: bool, presented: Option<bool>,
    total_ms: f64, node_count: usize, batch_count: usize,
});
payload!(RegisteredState {
    id: String, name: String, value_type: String, #[serde(rename = "scopeID")] scope_id: Option<String>,
});
payload!(StateWatch { ids: Vec<String> });
impl StateWatch {
    pub fn is_valid(&self) -> bool {
        self.ids.len() <= 128
            && self
                .ids
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == self.ids.len()
            && self.ids.iter().all(|id| !id.is_empty() && id.len() <= 256)
    }
}
payload!(ObservedState {
    id: String,
    summary: String,
    truncated: bool
});
payload!(StateObservation { registered: Vec<RegisteredState>, values: Vec<ObservedState> });
payload!(InspectionInsets {
    top: f64,
    right: f64,
    bottom: f64,
    left: f64
});
payload!(NodeLayoutInfo {
    padding: InspectionInsets,
    margin: InspectionInsets,
    border: InspectionInsets,
    content_width: f64,
    content_height: f64,
    direction: String,
    flex_direction: String,
    justify_content: String,
    align_items: String,
    flex_grow: f64,
    flex_shrink: f64,
});
payload!(NodeStyleInfo { background_color: Option<String>, foreground_color: Option<String>, overrides: Vec<String> });
payload!(InspectionState {
    #[serde(rename = "selectedID")] selected_id: Option<String>,
    can_undo: bool, can_redo: bool, override_count: usize,
});
payload!(SourceLocation {
    #[serde(rename = "fileID")]
    file_id: String,
    file_path: String,
    line: u64,
    column: u64
});
payload!(RecompositionReason {
    kind: String, detail: Option<String>, #[serde(rename = "originScopeID")] origin_scope_id: Option<String>,
});
payload!(Recomposition { count: u64, initial_ms: f64, last_ms: f64, total_ms: f64, max_ms: f64, reasons: Vec<RecompositionReason> });
/// Decode an envelope payload into a concrete type. Returns `None` for an
/// absent or malformed payload, matching Swift's `decodePayload(_:from:)`.
pub fn decode_payload<T: serde::de::DeserializeOwned>(payload: &Option<JsonValue>) -> Option<T> {
    serde_json::from_value(payload.clone()?).ok()
}

/// Serialisation helpers, matching Swift's `DevToolsCodec`.
pub struct DevToolsCodec;

impl DevToolsCodec {
    /// Host-produced protocol values must be finite and JSON-encodable. A value
    /// that is not yields `Null` rather than panicking the way Swift's `try!`
    /// would.
    pub fn json<T: Serialize>(value: &T) -> JsonValue {
        serde_json::to_value(value).unwrap_or(JsonValue::Null)
    }

    /// Decode a payload slot into a concrete type.
    pub fn decode<T: DeserializeOwned>(payload: &Option<JsonValue>) -> Option<T> {
        decode_payload(payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_uses_the_swift_field_names() {
        let envelope = Envelope::new("inspect.style.undo", Some(3), None);
        let value = serde_json::to_value(&envelope).unwrap();
        assert_eq!(value["type"], "inspect.style.undo");
        assert_eq!(value["id"], 3);
        assert!(value.get("payload").is_some());
    }

    #[test]
    fn payloads_round_trip_through_the_codec() {
        let frame = TimingFrame {
            frame: 1,
            recomposition_ms: None,
            layout_ms: 0.5,
            draw_ms: 0.25,
            render_thread_ms: Some(0.125),
            submitted: true,
            presented: Some(true),
            total_ms: 1.0,
            node_count: 4,
            batch_count: 2,
        };
        let payload = Some(DevToolsCodec::json(&frame));
        assert_eq!(DevToolsCodec::decode::<TimingFrame>(&payload), Some(frame));
        let value = payload.as_ref().expect("encoded payload");
        assert!(value.get("recompositionMs").is_some());
        assert!(value.get("presented").is_some());
    }

    #[test]
    fn source_location_keeps_the_capitalised_id_keys() {
        let location = SourceLocation {
            file_id: "f".into(),
            file_path: "/tmp/a.swift".into(),
            line: 1,
            column: 2,
        };
        let value = DevToolsCodec::json(&location);
        assert_eq!(value["fileID"], "f");
        assert_eq!(value["filePath"], "/tmp/a.swift");
    }
}
