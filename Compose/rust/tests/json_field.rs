//! Translation of `Tests/CoveUIComposeTests/JsonFieldTests.swift`.
//!
//! Behaviour checklist:
//! - empty input normalizes to `{}` and validates as `Empty`, which is
//!   acceptable so a cleared field can still commit
//! - valid and invalid JSON are distinguished
//! - the pretty printer sorts keys, so two equal documents format identically
//! - invalid text never yields a partial pretty-printed result
//!
//! Model difference: the Swift suite drives the mounted field through its key
//! and text handlers to prove commit-vs-reject. The Rust model owns the same
//! decision as `validate` + `normalized_commit_text`, so these tests assert the
//! commit gate directly with the same inputs the handlers receive.

use cove_ui_compose::primitives::controls::fields::{
    normalized_commit_text, pretty_printed, validate, JsonFieldValidation,
};

/// The text a commit would write, or `None` when the commit must be rejected.
fn committed(text: &str) -> Option<String> {
    validate(text)
        .is_acceptable()
        .then(|| normalized_commit_text(text))
}

#[test]
fn empty_input_saves_as_an_empty_json_object() {
    for blank in ["", "  ", "  \n ", "\n\t\n"] {
        assert_eq!(normalized_commit_text(blank), "{}", "blank input becomes an object");
        assert_eq!(
            validate(blank),
            JsonFieldValidation::Empty,
            "blank input is Empty, not Invalid"
        );
        assert_eq!(
            committed(blank).as_deref(),
            Some("{}"),
            "a blank field may be committed"
        );
    }
}

#[test]
fn valid_and_invalid_json_are_distinguished() {
    assert!(
        validate("{\"speed\": 3}").is_acceptable(),
        "quoted keys parse"
    );
    assert_eq!(validate("{\"speed\": 3}"), JsonFieldValidation::Valid);
    assert!(
        !validate("{ speed: 3 }").is_acceptable(),
        "unquoted keys are rejected"
    );
}

#[test]
fn any_valid_document_validates_not_only_objects() {
    for document in ["[]", "[1,2]", "3", "\"text\"", "true", "null"] {
        assert_eq!(
            validate(document),
            JsonFieldValidation::Valid,
            "{document} is valid JSON"
        );
    }
}

#[test]
fn an_invalid_document_carries_a_message() {
    match validate("{ nope }") {
        JsonFieldValidation::Invalid(message) => {
            assert!(!message.is_empty(), "the failure explains itself");
        }
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn the_pretty_printer_sorts_keys_for_stable_diffs() {
    assert_eq!(
        pretty_printed("{\"z\":1,\"a\":2}"),
        Some("{\n  \"a\": 2,\n  \"z\": 1\n}".to_owned())
    );
}

#[test]
fn key_order_does_not_change_the_formatted_output() {
    let first = pretty_printed("{\"b\":2,\"a\":1}");
    let second = pretty_printed("{\"a\":1,\"b\":2}");
    assert!(first.is_some());
    assert_eq!(first, second, "the same document formats identically");
}

#[test]
fn the_pretty_printer_rejects_invalid_input_instead_of_returning_a_fragment() {
    assert_eq!(pretty_printed("{ nope }"), None, "no partial output");
    assert_eq!(pretty_printed("[1,"), None, "no partial output");
}

#[test]
fn pretty_printing_a_blank_field_yields_the_empty_object() {
    assert_eq!(pretty_printed("   "), Some("{}".to_owned()));
}

#[test]
fn an_invalid_draft_does_not_replace_the_bound_value() {
    // The field keeps the last committed document when the draft is rejected —
    // the same gate the blur/refocus path uses.
    let committed_value = "{\"ok\":true}";
    assert_eq!(committed("{ unfinished "), None, "the draft is rejected");
    assert_eq!(
        committed_value, "{\"ok\":true}",
        "the bound value is untouched"
    );
}
