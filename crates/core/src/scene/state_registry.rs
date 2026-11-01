//! Translation of CoveUIScene/StateRegistry.swift.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateDescriptor {
    pub id: String,
    pub name: String,
    pub value_type: String,
    pub scope_id: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateValue {
    pub id: String,
    pub summary: String,
    pub truncated: bool,
}

struct Entry {
    descriptor: StateDescriptor,
    read: Box<dyn Fn() -> String>,
}

/// Scene-thread registry. Values are evaluated only for explicitly watched IDs.
#[derive(Default)]
pub struct StateRegistry {
    entries: BTreeMap<String, Entry>,
    next_id: u64,
}
impl StateRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    /// Returns the token that must be unregistered when the provider is removed.
    pub fn register(
        &mut self,
        stable_id: Option<String>,
        name: &str,
        value_type: &str,
        scope_id: Option<String>,
        read: impl Fn() -> String + 'static,
    ) -> Option<String> {
        if let Some(stable) = &stable_id
            && (stable.is_empty() || stable.len() > 256)
        {
            return None;
        }
        let replaces_existing = stable_id
            .as_ref()
            .is_some_and(|id| self.entries.contains_key(id));
        if self.entries.len() >= 512 && !replaces_existing {
            return None;
        }
        self.next_id = self.next_id.wrapping_add(1);
        let id = stable_id.unwrap_or_else(|| format!("provider.{}", self.next_id));
        let descriptor = StateDescriptor {
            id: id.clone(),
            name: truncate(name, 160),
            value_type: truncate(value_type, 160),
            scope_id,
        };
        self.entries.insert(
            id.clone(),
            Entry {
                descriptor,
                read: Box::new(read),
            },
        );
        Some(id)
    }
    pub fn unregister(&mut self, id: &str) {
        self.entries.remove(id);
    }
    pub fn descriptors(&self) -> Vec<StateDescriptor> {
        self.entries
            .values()
            .map(|entry| entry.descriptor.clone())
            .collect()
    }
    pub fn values(&self, ids: &[String]) -> Vec<StateValue> {
        let mut seen = HashSet::new();
        ids.iter()
            .take(128)
            .filter(|id| seen.insert((*id).clone()))
            .filter_map(|id| {
                let entry = self.entries.get(id)?;
                let text = (entry.read)();
                Some(StateValue {
                    id: id.clone(),
                    summary: truncate(&text, 1024),
                    truncated: text.chars().count() > 1024,
                })
            })
            .collect()
    }
}
fn truncate(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        text.to_owned()
    } else {
        text.chars().take(limit).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registration_is_bounded_and_deduplicated() {
        let mut registry = StateRegistry::new();
        let first = registry
            .register(None, "counter", "Int", None, || "1".into())
            .unwrap();
        assert_eq!(first, "provider.1");
        assert!(
            registry
                .register(Some(String::new()), "bad", "Int", None, String::new)
                .is_none()
        );
        assert!(
            registry
                .register(Some("x".repeat(257)), "bad", "Int", None, String::new)
                .is_none()
        );
        registry.register(Some("stable".into()), "stable", "Int", None, || "2".into());
        assert_eq!(registry.descriptors().len(), 2);
        assert_eq!(
            registry
                .values(&[first.clone(), first, "stable".into()])
                .len(),
            2
        );
        registry.unregister("stable");
        assert_eq!(registry.descriptors().len(), 1);
    }
    #[test]
    fn values_report_truncation() {
        let mut registry = StateRegistry::new();
        registry.register(Some("big".into()), "big", "String", None, || {
            "a".repeat(2000)
        });
        let value = &registry.values(&["big".into()])[0];
        assert!(value.truncated);
        assert_eq!(value.summary.chars().count(), 1024);
    }
}
