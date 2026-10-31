//! Client-side protocol state.
//!
//! `DevToolsSession` is the per-connection subscription and cursor record. The
//! protocol crate owns it (`cove_devtools_protocol::Session`) so the Browser,
//! Wasm and native hosts all share one implementation; this module re-exports it
//! under the name the Swift source used and adds the pieces a client needs that
//! are not part of the wire schema.
pub use cove_devtools_protocol::{
    Envelope, HelloHostInfo, HelloPayload, PROTOCOL_VERSION, Session, Subscription, decode_payload,
    state as decode_state_map,
};

/// Per-connection state. The Swift type was `DevToolsSession`; it is the same
/// value as the protocol crate's `Session`, so there is exactly one definition.
pub type DevToolsSession = Session;

/// Encode a serialisable value into the untyped payload slot of an envelope.
pub fn encode_payload<T: serde::Serialize>(value: &T) -> Option<serde_json::Value> {
    serde_json::to_value(value).ok()
}

/// Build an error envelope for a request that could not be answered.
pub fn error_envelope(request: &Envelope, code: &str, message: &str) -> Envelope {
    request.error(code, message)
}

/// Build the `.ok` envelope for a request, or `None` for a notification.
pub fn ok_envelope(request: &Envelope) -> Option<Envelope> {
    request.ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_alias_exposes_validation() {
        let mut session = DevToolsSession::default();
        let request = Envelope::new("tree.subscribe", Some(1), None);
        assert!(session.validate(&request).is_none());
        assert!(session.set(Subscription::Tree, true));
        // Idempotent: the set did not change.
        assert!(!session.set(Subscription::Tree, true));
        assert!(session.subscriptions.contains(&Subscription::Tree));
        session.reset();
        assert!(session.subscriptions.is_empty());
    }

    #[test]
    fn decode_state_map_rejects_non_string_values() {
        let payload = Some(serde_json::json!({"a": "1", "b": "2"}));
        assert_eq!(decode_state_map(&payload).map(|map| map.len()), Some(2));
        let payload = Some(serde_json::json!({"a": 1}));
        assert!(decode_state_map(&payload).is_none());
    }

    #[test]
    fn payload_round_trips_through_the_envelope() {
        let watch = cove_devtools_protocol::StateWatch {
            ids: vec!["a".into()],
        };
        let request = Envelope::new("state.subscribe", Some(7), encode_payload(&watch));
        let decoded: Option<cove_devtools_protocol::StateWatch> = decode_payload(&request.payload);
        assert_eq!(decoded, Some(watch));
        assert_eq!(ok_envelope(&request).unwrap().r#type, "state.subscribe.ok");
    }
}
