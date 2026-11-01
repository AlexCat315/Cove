//! Headless state/checkpoint example. The native Counter UI is still pending migration.
use cove_core::State;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct Counter {
    count: u32,
    dark: bool,
    note: String,
}
impl Counter {
    fn restore(json: &str) -> Result<Self, String> {
        let value: Self = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if value.count > 999_999 || value.note.chars().count() > 2048 {
            return Err("invalid counter checkpoint".into());
        }
        Ok(value)
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let counter = State::new(Counter::default());
    counter.update(|state| {
        state.count = 1;
        state.note = "Cove · Rust 🦀".into();
    });
    let checkpoint = serde_json::to_string(&counter.get())?;
    let restored = Counter::restore(&checkpoint)?;
    println!("{}", serde_json::to_string_pretty(&restored)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_checkpoint_before_restoring_state() {
        assert!(Counter::restore(r#"{"count":1000000,"dark":false,"note":""}"#).is_err());
        assert!(Counter::restore(r#"{"count":1,"dark":true,"note":"你好🦀"}"#).is_ok());
        let invalid = serde_json::to_string(&Counter {
            note: "🦀".repeat(2049),
            ..Default::default()
        })
        .unwrap();
        assert!(Counter::restore(&invalid).is_err());
    }
}
