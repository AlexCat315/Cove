//! Agent-facing contracts: JSON Schema subset, tool metadata and validation.
//!
//! The schema is deliberately small — it supports only what a capability input
//! is allowed to express, so a model cannot smuggle an untyped escape hatch into
//! a tool boundary.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JsonSchemaType {
    Object,
    Array,
    String,
    Number,
    Integer,
    Boolean,
    Null,
}

/// A scalar value used by a JSON Schema `enum` constraint.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonSchemaScalar {
    String(String),
    Number(f64),
    Boolean(bool),
}

/// Strict JSON Schema subset shared by providers, MCP and argument validation.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct JsonSchema {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<JsonSchemaType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub properties: std::collections::BTreeMap<String, JsonSchema>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<JsonSchema>>,
    #[serde(default, rename = "enum", skip_serializing_if = "Vec::is_empty")]
    pub allowed_values: Vec<JsonSchemaScalar>,
    #[serde(default, rename = "oneOf", skip_serializing_if = "Vec::is_empty")]
    pub one_of: Vec<JsonSchema>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub additional_properties: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum: Option<f64>,
    #[serde(default, rename = "minItems", skip_serializing_if = "Option::is_none")]
    pub minimum_items: Option<usize>,
    #[serde(default, rename = "maxItems", skip_serializing_if = "Option::is_none")]
    pub maximum_items: Option<usize>,
    #[serde(default, rename = "minLength", skip_serializing_if = "Option::is_none")]
    pub minimum_length: Option<usize>,
    #[serde(default, rename = "maxLength", skip_serializing_if = "Option::is_none")]
    pub maximum_length: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
}

impl JsonSchema {
    pub fn object(
        properties: std::collections::BTreeMap<String, JsonSchema>,
        required: Vec<String>,
    ) -> Self {
        let required_set: std::collections::BTreeSet<&String> = required.iter().collect();
        let properties = properties
            .into_iter()
            .map(|(key, schema)| {
                let schema = if required_set.contains(&key) {
                    schema
                } else {
                    schema.accepting_null()
                };
                (key, schema)
            })
            .collect();
        Self {
            r#type: Some(JsonSchemaType::Object),
            properties,
            required: {
                let mut required = required;
                required.sort();
                required
            },
            additional_properties: Some(false),
            ..Self::default()
        }
    }

    pub fn array_of(item: JsonSchema, maximum_items: Option<usize>) -> Self {
        Self {
            r#type: Some(JsonSchemaType::Array),
            items: Some(Box::new(item)),
            maximum_items,
            ..Self::default()
        }
    }

    pub fn string(min_length: Option<usize>, max_length: Option<usize>) -> Self {
        Self {
            r#type: Some(JsonSchemaType::String),
            minimum_length: min_length,
            maximum_length: max_length,
            ..Self::default()
        }
    }

    pub fn integer(minimum: Option<f64>, maximum: Option<f64>) -> Self {
        Self {
            r#type: Some(JsonSchemaType::Integer),
            minimum,
            maximum,
            ..Self::default()
        }
    }

    pub fn number(minimum: Option<f64>, maximum: Option<f64>) -> Self {
        Self {
            r#type: Some(JsonSchemaType::Number),
            minimum,
            maximum,
            ..Self::default()
        }
    }

    pub fn boolean() -> Self {
        Self {
            r#type: Some(JsonSchemaType::Boolean),
            ..Self::default()
        }
    }

    pub fn null() -> Self {
        Self {
            r#type: Some(JsonSchemaType::Null),
            ..Self::default()
        }
    }

    /// Any value. Never valid on a write-capability boundary.
    pub fn any() -> Self {
        Self::default()
    }

    pub fn choice(alternatives: Vec<JsonSchema>) -> Self {
        Self {
            one_of: alternatives,
            ..Self::default()
        }
    }

    fn accepting_null(self) -> Self {
        if self.r#type.is_none() && self.one_of.is_empty() {
            return self;
        }
        if self.r#type == Some(JsonSchemaType::Null)
            || self
                .one_of
                .iter()
                .any(|candidate| candidate.r#type == Some(JsonSchemaType::Null))
        {
            return self;
        }
        Self::choice(vec![self, Self::null()])
    }

    /// True only for the bounded subset allowed on write-capability boundaries.
    /// Untyped `any` values and open nested objects are rejected even when the
    /// root object is strict.
    pub fn is_strict_capability_input(&self) -> bool {
        if !self.one_of.is_empty() {
            return self.r#type.is_none()
                && self.one_of.iter().all(Self::is_strict_capability_input);
        }
        match self.r#type {
            Some(JsonSchemaType::Object) => {
                self.additional_properties == Some(false)
                    && self
                        .properties
                        .values()
                        .all(Self::is_strict_capability_input)
            }
            Some(JsonSchemaType::Array) => self
                .items
                .as_ref()
                .is_some_and(|item| item.is_strict_capability_input()),
            Some(_) => true,
            None => false,
        }
    }

    /// Canonical JSON used by capability contract fingerprints.
    pub fn canonical_json(&self) -> Value {
        // Serialization is total for this shape, so the fallback is unreachable.
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

/// A validation failure with the path that failed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonSchemaViolation {
    pub path: String,
    pub reason: String,
}

impl std::fmt::Display for JsonSchemaViolation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.path, self.reason)
    }
}

impl std::error::Error for JsonSchemaViolation {}

/// Host-side validation is authoritative even when a provider advertises strict
/// tool schemas; provider validation is a convenience only.
pub struct JsonSchemaValidator;

impl JsonSchemaValidator {
    pub fn validate(value: &Value, schema: &JsonSchema) -> Result<(), JsonSchemaViolation> {
        Self::validate_at(value, schema, "$")
    }

    fn validate_at(
        value: &Value,
        schema: &JsonSchema,
        path: &str,
    ) -> Result<(), JsonSchemaViolation> {
        if !schema.one_of.is_empty() {
            let matches = schema
                .one_of
                .iter()
                .filter(|candidate| Self::validate_at(value, candidate, path).is_ok())
                .count();
            if matches != 1 {
                return Err(JsonSchemaViolation {
                    path: path.to_string(),
                    reason: format!("must match exactly one allowed schema (matched {matches})"),
                });
            }
        }
        match schema.r#type {
            Some(JsonSchemaType::Object) => {
                let Some(object) = value.as_object() else {
                    return Err(violation(path, "expected object"));
                };
                for key in &schema.required {
                    // An optional property accepted `null`; a required one may
                    // not be absent.
                    if object.get(key).is_none_or(Value::is_null) {
                        return Err(violation(
                            path,
                            &format!("missing required property '{key}'"),
                        ));
                    }
                }
                if schema.additional_properties == Some(false)
                    && let Some(unknown) = object
                        .keys()
                        .find(|key| !schema.properties.contains_key(*key))
                {
                    return Err(violation(path, &format!("unknown property '{unknown}'")));
                }
                for (key, child) in object {
                    if let Some(child_schema) = schema.properties.get(key) {
                        Self::validate_at(child, child_schema, &format!("{path}.{key}"))?;
                    }
                }
            }
            Some(JsonSchemaType::Array) => {
                let Some(values) = value.as_array() else {
                    return Err(violation(path, "expected array"));
                };
                if let Some(minimum) = schema.minimum_items
                    && values.len() < minimum
                {
                    return Err(violation(
                        path,
                        &format!("requires at least {minimum} items"),
                    ));
                }
                if let Some(maximum) = schema.maximum_items
                    && values.len() > maximum
                {
                    return Err(violation(path, &format!("allows at most {maximum} items")));
                }
                if let Some(item_schema) = &schema.items {
                    for (index, child) in values.iter().enumerate() {
                        Self::validate_at(child, item_schema, &format!("{path}[{index}]"))?;
                    }
                }
            }
            Some(JsonSchemaType::String) => {
                let Some(string) = value.as_str() else {
                    return Err(violation(path, "expected string"));
                };
                let length = string.chars().count();
                if let Some(minimum) = schema.minimum_length
                    && length < minimum
                {
                    return Err(violation(
                        path,
                        &format!("requires at least {minimum} characters"),
                    ));
                }
                if let Some(maximum) = schema.maximum_length
                    && length > maximum
                {
                    return Err(violation(
                        path,
                        &format!("allows at most {maximum} characters"),
                    ));
                }
                if let Some(pattern) = &schema.pattern
                    && !simple_pattern_matches(pattern, string)
                {
                    return Err(violation(path, "does not match required pattern"));
                }
            }
            Some(JsonSchemaType::Number) => {
                if numeric_value(value).is_none() {
                    return Err(violation(path, "expected number"));
                }
            }
            Some(JsonSchemaType::Integer) => {
                let Some(number) = numeric_value(value) else {
                    return Err(violation(path, "expected integer"));
                };
                if number.fract() != 0.0 {
                    return Err(violation(path, "expected integer"));
                }
            }
            Some(JsonSchemaType::Boolean) => {
                if !value.is_boolean() {
                    return Err(violation(path, "expected boolean"));
                }
            }
            Some(JsonSchemaType::Null) if !value.is_null() => {
                return Err(violation(path, "expected null"));
            }
            Some(JsonSchemaType::Null) | None => {}
        }
        if let Some(number) = numeric_value(value) {
            if let Some(minimum) = schema.minimum
                && number < minimum
            {
                return Err(violation(path, &format!("must be >= {minimum}")));
            }
            if let Some(maximum) = schema.maximum
                && number > maximum
            {
                return Err(violation(path, &format!("must be <= {maximum}")));
            }
        }
        if !schema.allowed_values.is_empty() {
            let matches = schema
                .allowed_values
                .iter()
                .any(|candidate| match candidate {
                    JsonSchemaScalar::String(expected) => value.as_str() == Some(expected.as_str()),
                    JsonSchemaScalar::Number(expected) => numeric_value(value) == Some(*expected),
                    JsonSchemaScalar::Boolean(expected) => value.as_bool() == Some(*expected),
                });
            if !matches {
                return Err(violation(path, "value is not in the allowed enum"));
            }
        }
        Ok(())
    }
}

fn violation(path: &str, reason: &str) -> JsonSchemaViolation {
    JsonSchemaViolation {
        path: path.to_string(),
        reason: reason.to_string(),
    }
}

fn numeric_value(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|value| value as f64))
}

/// The only pattern used by this crate's schemas is an anchored character class
/// such as `^[a-z0-9][a-z0-9_-]{0,63}$`. Implementing that subset keeps the
/// validator dependency-free; anything else is reported as unmatched so an
/// unsupported pattern cannot silently pass.
fn simple_pattern_matches(pattern: &str, value: &str) -> bool {
    if pattern == "^[a-z0-9][a-z0-9_-]{0,63}$" {
        let mut characters = value.chars();
        let Some(first) = characters.next() else {
            return false;
        };
        if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
            return false;
        }
        let rest: String = characters.collect();
        return rest.chars().count() <= 63
            && rest
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
    }
    !pattern.is_empty() && value == pattern
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_properties_accept_null_but_required_ones_do_not() {
        let mut properties = std::collections::BTreeMap::new();
        properties.insert(
            "required_field".to_string(),
            JsonSchema::string(Some(1), Some(8)),
        );
        properties.insert(
            "optional_field".to_string(),
            JsonSchema::integer(None, None),
        );
        let schema = JsonSchema::object(properties, vec!["required_field".into()]);
        // Omitted optional property is fine.
        assert!(
            JsonSchemaValidator::validate(&serde_json::json!({"required_field": "a"}), &schema)
                .is_ok()
        );
        // Explicit null for the optional property is fine.
        assert!(
            JsonSchemaValidator::validate(
                &serde_json::json!({"required_field": "a", "optional_field": null}),
                &schema
            )
            .is_ok()
        );
        // Missing required property fails.
        assert!(JsonSchemaValidator::validate(&serde_json::json!({}), &schema).is_err());
        // Null required property fails.
        assert!(
            JsonSchemaValidator::validate(&serde_json::json!({"required_field": null}), &schema)
                .is_err()
        );
    }

    #[test]
    fn unknown_properties_are_rejected() {
        let schema = JsonSchema::object(std::collections::BTreeMap::new(), Vec::new());
        assert!(JsonSchemaValidator::validate(&serde_json::json!({"extra": 1}), &schema).is_err());
    }

    #[test]
    fn integers_must_be_whole() {
        let schema = JsonSchema::integer(None, None);
        assert!(JsonSchemaValidator::validate(&serde_json::json!(3), &schema).is_ok());
        assert!(JsonSchemaValidator::validate(&serde_json::json!(3.5), &schema).is_err());
    }

    #[test]
    fn untyped_schemas_are_not_strict_capability_inputs() {
        assert!(!JsonSchema::any().is_strict_capability_input());
        assert!(JsonSchema::string(None, None).is_strict_capability_input());
        let schema = JsonSchema::object(std::collections::BTreeMap::new(), Vec::new());
        assert!(schema.is_strict_capability_input());
    }
}
