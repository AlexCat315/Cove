//! Behaviour tests for JSON field validation and formatting.
//!
//! Translation of `Primitives/Controls/Fields/JsonField.swift`. The contract that
//! matters: a cleared field saves as `{}` rather than reporting an error, and
//! formatting is stable regardless of the input's key order.

use cove_ui_compose::primitives::controls::fields::{
    normalized_commit_text, pretty_printed, validate, JsonFieldLabels, JsonFieldValidation,
};

#[test]
fn a_blank_field_normalizes_to_an_empty_object() {
    assert_eq!(normalized_commit_text(""), "{}");
    assert_eq!(normalized_commit_text("   \n\t "), "{}");
    assert_eq!(normalized_commit_text(" {\"a\":1} "), "{\"a\":1}");
}

#[test]
fn a_blank_field_validates_as_empty_and_is_acceptable() {
    let verdict = validate("");
    assert_eq!(verdict, JsonFieldValidation::Empty);
    assert!(verdict.is_acceptable(), "an empty field may be committed");
}

#[test]
fn a_typed_empty_object_is_valid_not_empty() {
    // The distinction is whether the person typed anything at all.
    assert_eq!(validate("{}"), JsonFieldValidation::Valid);
    assert_eq!(validate("  {}  "), JsonFieldValidation::Valid);
}

#[test]
fn any_valid_document_validates() {
    for text in [
        r#"{"a": 1}"#,
        r#"[1, 2, 3]"#,
        r#""text""#,
        "42",
        "true",
        "null",
        r#"{"nested": {"deep": [1, {"x": null}]}}"#,
    ] {
        assert_eq!(validate(text), JsonFieldValidation::Valid, "{text} should be valid");
    }
}

#[test]
fn malformed_json_is_invalid_and_not_acceptable() {
    for text in [r#"{"a":}"#, "{", "[1, 2", "not json", r#"{"a": 1,}"#] {
        let verdict = validate(text);
        assert!(
            matches!(verdict, JsonFieldValidation::Invalid(_)),
            "{text} should be invalid, got {verdict:?}"
        );
        assert!(!verdict.is_acceptable());
    }
}

#[test]
fn pretty_printing_sorts_keys() {
    let pretty = pretty_printed(r#"{"b": 1, "a": 2}"#).expect("valid input formats");
    let a = pretty.find("\"a\"").expect("key a present");
    let b = pretty.find("\"b\"").expect("key b present");
    assert!(a < b, "keys are sorted, got:\n{pretty}");
}

#[test]
fn pretty_printing_is_stable_across_key_order() {
    let one = pretty_printed(r#"{"b": 1, "a": 2}"#).unwrap();
    let other = pretty_printed(r#"{"a": 2, "b": 1}"#).unwrap();
    assert_eq!(one, other, "semantically equal documents format identically");
}

#[test]
fn pretty_printing_an_empty_field_formats_the_empty_object() {
    assert_eq!(pretty_printed("").as_deref(), Some("{}"));
}

#[test]
fn pretty_printing_invalid_input_returns_nothing() {
    assert_eq!(pretty_printed("{"), None, "no partial result for invalid input");
}

#[test]
fn pretty_output_indents_nested_structures() {
    let pretty = pretty_printed(r#"{"a":{"b":1}}"#).unwrap();
    assert!(pretty.contains('\n'), "pretty output is multi-line:\n{pretty}");
    assert!(pretty.contains("  "), "pretty output is indented:\n{pretty}");
}

#[test]
fn default_labels_match_the_authored_copy() {
    let labels = JsonFieldLabels::default();
    assert_eq!(labels.format, "Format");
    assert_eq!(labels.empty, "Empty saves as {}");
    assert_eq!(labels.apply, "Apply");
}
