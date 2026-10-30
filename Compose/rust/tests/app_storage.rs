//! Translation of `Tests/CoveUIComposeTests/AppStorageTests.swift`.
//!
//! Behaviour checklist:
//! - `FileAppStorageStore` round-trips values across instances
//! - corrupt JSON is quarantined before fresh preferences are written
//! - a disk failure is reported while the live value is retained
//! - a raw-representable enum persists through its raw value
//! - two wrappers over one key share the live value
//!
//! Model difference: Swift uses the `@AppStorage` property wrapper, which
//! recomposes the reading view. The Rust image splits that into an explicit
//! `AppStorage` handle plus the view's own recompose wiring, so the
//! recomposition half is asserted through the shared value and the store
//! contents rather than an annotated property.

mod support;

use std::sync::Arc;

use cove_ui_compose::app_storage::{
    AppStorage, AppStorageConvertible, AppStorageRawRepresentable, AppStorageStore,
    AppStorageValue, FileAppStorageStore, MemoryAppStorageStore,
};

use support::GlobalTestLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TestTheme {
    Dark,
    Light,
}

impl AppStorageRawRepresentable for TestTheme {
    type RawValue = String;
    fn from_raw_value(raw: String) -> Option<Self> {
        match raw.as_str() {
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            _ => None,
        }
    }
    fn raw_value(&self) -> String {
        match self {
            Self::Dark => "dark".to_string(),
            Self::Light => "light".to_string(),
        }
    }
}

fn temporary_directory(tag: &str) -> std::path::PathBuf {
    let unique = format!(
        "cove-appstorage-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    );
    std::env::temp_dir().join(unique)
}

#[test]
fn the_file_store_round_trips_values_across_instances() {
    GlobalTestLock::locked(|| {
        let directory = temporary_directory("roundtrip");
        let path = directory.join("app-storage.json");

        let store = FileAppStorageStore::new(&path);
        store.set("theme", Some(AppStorageValue::String("light".into())));
        store.set("grid", Some(AppStorageValue::Bool(true)));
        store.set("count", Some(AppStorageValue::Int(42)));
        store.set("scale", Some(AppStorageValue::Double(0.5)));

        let reloaded = FileAppStorageStore::new(&path);
        assert_eq!(
            reloaded.value("theme"),
            Some(AppStorageValue::String("light".into())),
            "a string survives a reopen"
        );
        assert_eq!(
            reloaded.value("grid"),
            Some(AppStorageValue::Bool(true)),
            "a bool survives a reopen"
        );
        assert_eq!(
            reloaded.value("count"),
            Some(AppStorageValue::Int(42)),
            "an int survives a reopen"
        );
        assert_eq!(
            reloaded.value("scale"),
            Some(AppStorageValue::Double(0.5)),
            "a double survives a reopen"
        );
        assert_eq!(reloaded.value("missing"), None, "a missing key reads as none");

        let _ = std::fs::remove_dir_all(&directory);
    });
}

#[test]
fn corrupt_json_is_quarantined_before_fresh_preferences() {
    GlobalTestLock::locked(|| {
        let directory = temporary_directory("corrupt");
        let path = directory.join("app-storage.json");
        std::fs::create_dir_all(&directory).expect("the temp directory is creatable");
        std::fs::write(&path, b"{ definitely-not-json").expect("the corrupt file is writable");

        let store = FileAppStorageStore::new(&path);
        assert_eq!(
            store.value("theme"),
            None,
            "a corrupt file yields no values"
        );
        assert!(
            store.load_warning().is_some(),
            "the caller is told the file could not be read"
        );
        let quarantined = store.quarantined_path().expect("the bad file was moved aside");
        assert!(
            quarantined.is_file(),
            "the quarantine keeps the unreadable bytes"
        );
        assert!(!path.exists(), "the live path is cleared for a fresh file");

        store.set("theme", Some(AppStorageValue::String("light".into())));
        assert_eq!(store.last_save_error(), None, "the fresh write succeeds");
        assert!(quarantined.is_file(), "quarantining is not undone by a write");
        assert_eq!(
            FileAppStorageStore::new(&path).value("theme"),
            Some(AppStorageValue::String("light".into())),
            "the new value is readable by a fresh instance"
        );

        let _ = std::fs::remove_dir_all(&directory);
    });
}

#[test]
fn a_disk_failure_is_reported_while_the_live_value_is_retained() {
    GlobalTestLock::locked(|| {
        let directory = temporary_directory("readonly");
        let blocked_parent = directory.join("not-a-directory");
        let path = blocked_parent.join("app-storage.json");
        std::fs::create_dir_all(&directory).expect("the temp directory is creatable");
        // A regular file where a directory would have to be blocks the save.
        std::fs::write(&blocked_parent, b"block directory creation")
            .expect("the blocking file is writable");

        let store = FileAppStorageStore::new(&path);
        store.set("grid", Some(AppStorageValue::Bool(true)));

        assert_eq!(
            store.value("grid"),
            Some(AppStorageValue::Bool(true)),
            "the in-memory value survives a failed save"
        );
        assert!(
            store.last_save_error().is_some(),
            "the caller is told the save failed"
        );
        assert!(!path.exists(), "no file was written");

        let _ = std::fs::remove_dir_all(&directory);
    });
}

#[test]
fn a_raw_representable_enum_persists_through_its_raw_value() {
    GlobalTestLock::locked(|| {
        let store = Arc::new(MemoryAppStorageStore::new());
        let theme: AppStorage<TestTheme> =
            AppStorage::new("test.enum.theme", TestTheme::Dark, Some(store.clone()));

        assert_eq!(theme.get(), TestTheme::Dark, "the default applies first");
        theme.set(TestTheme::Light);
        assert_eq!(
            store.values().get("test.enum.theme"),
            Some(&AppStorageValue::String("light".into())),
            "the raw value is what is stored"
        );

        // A second wrapper over the same key sees the live value.
        let again: AppStorage<TestTheme> =
            AppStorage::new("test.enum.theme", TestTheme::Dark, Some(store.clone()));
        assert_eq!(
            again.get(),
            TestTheme::Light,
            "a later wrapper reads the persisted value"
        );
    });
}

#[test]
fn two_wrappers_over_one_key_stay_in_sync() {
    GlobalTestLock::locked(|| {
        let store = Arc::new(MemoryAppStorageStore::new());
        let first: AppStorage<String> =
            AppStorage::new("test.view.theme", "dark".to_string(), Some(store.clone()));
        let second: AppStorage<String> =
            AppStorage::new("test.view.theme", "dark".to_string(), Some(store.clone()));

        let mut observed = None;
        second.binding().get();
        let binding = first.binding();
        binding.set("light".to_string());

        // Both handles read through the shared box, so a write is visible to
        // the other immediately.
        assert_eq!(first.get(), "light", "the writing handle sees its value");
        assert_eq!(second.get(), "light", "the sibling handle sees the same value");
        assert_eq!(
            store.values().get("test.view.theme"),
            Some(&AppStorageValue::String("light".into())),
            "the write reached the store"
        );

        observed = Some(second.get());
        assert_eq!(observed.as_deref(), Some("light"));
    });
}

#[test]
fn writing_the_same_value_again_does_not_churn() {
    GlobalTestLock::locked(|| {
        let store = Arc::new(MemoryAppStorageStore::new());
        let first: AppStorage<String> =
            AppStorage::new("test.churn", "dark".to_string(), Some(store.clone()));
        first.set("light".to_string());

        let value = first.get();
        first.set(value.clone());
        assert_eq!(
            first.get(),
            value,
            "a redundant write leaves the value unchanged"
        );
        assert_eq!(
            store.values().get("test.churn"),
            Some(&AppStorageValue::String("light".into())),
            "and does not disturb the stored value"
        );
    });
}

#[test]
fn the_memory_store_keeps_values_per_key() {
    let store = MemoryAppStorageStore::new();
    store.set("a", Some(AppStorageValue::Int(1)));
    store.set("b", Some(AppStorageValue::String("two".into())));
    assert_eq!(store.value("a"), Some(AppStorageValue::Int(1)));
    assert_eq!(store.value("b"), Some(AppStorageValue::String("two".into())));

    store.set("a", None);
    assert_eq!(store.value("a"), None, "setting none removes the key");
    assert_eq!(
        store.values().len(),
        1,
        "only the remaining key is present"
    );
}

#[test]
fn the_defaults_holder_can_be_swapped_and_restored() {
    GlobalTestLock::locked(|| {
        let previous = cove_ui_compose::app_storage::AppStorageDefaults::store();
        let store = Arc::new(MemoryAppStorageStore::new());
        cove_ui_compose::app_storage::AppStorageDefaults::set_store(store.clone());

        let value: AppStorage<i64> = AppStorage::new("test.swap", 7i64, None);
        value.set(9);
        assert_eq!(
            store.values().get("test.swap"),
            Some(&AppStorageValue::Int(9)),
            "the wrapper uses the currently installed default store"
        );

        cove_ui_compose::app_storage::AppStorageDefaults::set_store(previous);
    });
}

#[test]
fn scalar_conversions_keep_their_storage_kind() {
    assert_eq!(
        <bool as AppStorageConvertible>::storage_value(&true),
        AppStorageValue::Bool(true)
    );
    assert_eq!(
        <i32 as AppStorageConvertible>::storage_value(&-3),
        AppStorageValue::Int(-3)
    );
    assert_eq!(
        <f32 as AppStorageConvertible>::storage_value(&1.5),
        AppStorageValue::Double(1.5)
    );
    assert_eq!(
        <String as AppStorageConvertible>::storage_value(&"x".to_string()),
        AppStorageValue::String("x".into())
    );

    assert_eq!(
        <bool as AppStorageConvertible>::from_storage_value(&AppStorageValue::Int(1)),
        None,
        "a bool does not read from an int"
    );
    assert_eq!(
        <f64 as AppStorageConvertible>::from_storage_value(&AppStorageValue::Int(4)),
        Some(4.0),
        "a double widens an int, matching Swift's NSNumber bridging"
    );
}
