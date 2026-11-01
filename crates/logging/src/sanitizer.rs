//! Translation of `Logging/Sources/CoveLogging/Runtime/Sanitizer.swift`.
use super::configuration::LoggingConfiguration;
use super::record::LogValue;
use std::collections::BTreeMap;

/// Bounded recursive redaction. The byte budget and node count are shared by
/// keys, scalars and nested containers, so one wide object cannot starve the
/// rest of a record.
pub struct RecordSanitizer<'a> {
    configuration: &'a LoggingConfiguration,
    budget: i64,
    nodes: i32,
}
impl<'a> RecordSanitizer<'a> {
    pub fn new(configuration: &'a LoggingConfiguration) -> Self {
        Self {
            configuration,
            budget: configuration.buffering.metadata_bytes as i64,
            nodes: 64,
        }
    }

    /// Keeps at most `bytes`, reserving 16 bytes for the marker, and truncates
    /// on a UTF-8 scalar boundary like Swift's `String.Index` walk.
    pub fn text(&self, text: &str, bytes: usize) -> String {
        if text.len() <= bytes {
            return text.into();
        }
        let maximum = bytes.saturating_sub(16);
        let mut end = 0;
        for (index, character) in text.char_indices() {
            let next = index + character.len_utf8();
            if next > maximum {
                break;
            }
            end = next;
        }
        format!("{}…[truncated]", &text[..end])
    }

    pub fn metadata(&mut self, values: &BTreeMap<String, LogValue>) -> BTreeMap<String, LogValue> {
        self.metadata_at(values, 0)
    }

    fn metadata_at(
        &mut self,
        values: &BTreeMap<String, LogValue>,
        depth: usize,
    ) -> BTreeMap<String, LogValue> {
        let mut result = BTreeMap::new();
        // `sorted keys` is the natural BTreeMap order.
        for (key, value) in values {
            if self.budget <= 0 || self.nodes <= 0 || depth >= 8 {
                result.insert("_truncated".into(), LogValue::from("true"));
                break;
            }
            self.nodes -= 1;
            let name = self.text(key, (256).min(self.budget.max(0) as usize));
            self.budget -= name.len() as i64;
            if self
                .configuration
                .redaction
                .keys
                .iter()
                .any(|candidate| candidate.to_lowercase() == key.to_lowercase())
            {
                result.insert(name, LogValue::from("[redacted]"));
            } else {
                result.insert(name, self.convert(value, depth));
            }
        }
        result
    }

    fn convert(&mut self, value: &LogValue, depth: usize) -> LogValue {
        if self.budget <= 0 || self.nodes <= 0 || depth >= 8 {
            return LogValue::from("[truncated]");
        }
        match value {
            LogValue::Object(values) => LogValue::Object(self.metadata_at(values, depth + 1)),
            LogValue::Array(values) => {
                let mut result = Vec::new();
                for value in values {
                    if self.budget <= 0 || self.nodes <= 0 {
                        result.push(LogValue::from("[truncated]"));
                        break;
                    }
                    self.nodes -= 1;
                    result.push(self.convert(value, depth + 1));
                }
                LogValue::Array(result)
            }
            LogValue::String(value) => self.scalar(value),
        }
    }

    fn scalar(&mut self, value: &str) -> LogValue {
        let maximum = (2048).min(self.budget.max(0) as usize);
        let string = self.text(value, maximum);
        self.budget -= string.len() as i64;
        LogValue::String(string)
    }
}
