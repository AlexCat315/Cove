//! Translation of `Primitives/Controls/Fields/JsonField.swift` (the model half:
//! normalization, validation and pretty-printing).
//!
//! An empty field commits as `{}` rather than failing validation, and formatting
//! sorts keys so two semantically equal documents format identically.
//!
//! Behaviour checklist:
//! - empty or whitespace-only input normalizes to `{}` and validates as `Empty`
//! - `Empty` is acceptable, so a blank field can be committed
//! - malformed JSON is `Invalid` with a message, and is not acceptable
//! - any valid document (object, array, scalar) validates as `Valid`
//! - pretty-printing sorts keys and is stable across key order
//! - pretty-printing returns `None` for invalid input rather than a partial result

/// Result of validating the field's text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JsonFieldValidation {
    Valid,
    Empty,
    Invalid(String),
}

impl JsonFieldValidation {
    /// Whether the text may be committed.
    ///
    /// `Empty` is acceptable: a blank field saves as `{}`.
    pub fn is_acceptable(&self) -> bool {
        matches!(self, Self::Valid | Self::Empty)
    }
}

/// The text that should be committed for `text`.
///
/// Whitespace-only input becomes the empty object, so a cleared field is valid
/// rather than a parse error.
pub fn normalized_commit_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        "{}".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// Validates JSON text.
pub fn validate(text: &str) -> JsonFieldValidation {
    let normalized = normalized_commit_text(text);
    match serde_json::from_str::<serde_json::Value>(&normalized) {
        Ok(_) => {
            // Only a genuinely blank field reports `Empty`; a typed `{}` is Valid.
            if normalized == "{}" && text.trim().is_empty() {
                JsonFieldValidation::Empty
            } else {
                JsonFieldValidation::Valid
            }
        }
        Err(error) => JsonFieldValidation::Invalid(error.to_string()),
    }
}

/// Pretty-printed JSON with sorted keys, or `None` when the text is invalid.
pub fn pretty_printed(text: &str) -> Option<String> {
    let normalized = normalized_commit_text(text);
    let value: serde_json::Value = serde_json::from_str(&normalized).ok()?;
    // `serde_json::Value` objects are `BTreeMap`-backed, so keys come out sorted
    // without an explicit option — matching `sortedKeys`.
    serde_json::to_string_pretty(&value).ok()
}

/// The labels a JSON field shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonFieldLabels {
    pub format: String,
    pub revert: String,
    pub valid: String,
    pub empty: String,
    pub expand: String,
    pub apply: String,
    pub cancel: String,
    pub resize: String,
}

impl Default for JsonFieldLabels {
    fn default() -> Self {
        Self {
            format: "Format".into(),
            revert: "Revert".into(),
            valid: "Valid JSON".into(),
            empty: "Empty saves as {}".into(),
            expand: "Expand JSON Editor".into(),
            apply: "Apply".into(),
            cancel: "Cancel".into(),
            resize: "Editor height".into(),
        }
    }
}
