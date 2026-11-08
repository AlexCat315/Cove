//! Persistent, shared per-key storage for Compose views.
use cove_core::{Binding, State};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AppStorageValue {
    Bool(bool),
    Int(i64),
    Double(f64),
    String(String),
}

pub trait AppStorageStore: Send + Sync {
    fn value(&self, key: &str) -> Option<AppStorageValue>;
    fn set(&self, key: &str, value: Option<AppStorageValue>);
}

fn default_store_slot() -> &'static Mutex<Arc<dyn AppStorageStore>> {
    static STORE: OnceLock<Mutex<Arc<dyn AppStorageStore>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(Arc::new(FileAppStorageStore::new(default_path()))))
}

pub struct AppStorageDefaults;
impl AppStorageDefaults {
    pub fn store() -> Arc<dyn AppStorageStore> {
        default_store_slot()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub fn set_store(store: Arc<dyn AppStorageStore>) {
        *default_store_slot()
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = store;
    }
}

struct FileState {
    values: BTreeMap<String, AppStorageValue>,
    persistence_enabled: bool,
    load_warning: Option<String>,
    last_save_error: Option<String>,
    quarantined_path: Option<PathBuf>,
}

pub struct FileAppStorageStore {
    path: PathBuf,
    state: Mutex<FileState>,
}
impl FileAppStorageStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let mut state = FileState {
            values: BTreeMap::new(),
            persistence_enabled: true,
            load_warning: None,
            last_save_error: None,
            quarantined_path: None,
        };
        if path.exists() {
            match std::fs::read(&path).and_then(|bytes| {
                serde_json::from_slice::<BTreeMap<String, AppStorageValue>>(&bytes)
                    .map_err(std::io::Error::other)
            }) {
                Ok(values) => state.values = values,
                Err(original) => {
                    let quarantine = quarantine_path(&path);
                    match std::fs::rename(&path, &quarantine) {
                        Ok(()) => {
                            state.quarantined_path = Some(quarantine.clone());
                            state.load_warning = Some(format!(
                                "Unreadable app storage was moved to {}: {original}",
                                quarantine.file_name().unwrap_or_default().to_string_lossy()
                            ));
                        }
                        Err(error) => {
                            state.persistence_enabled = false;
                            state.load_warning = Some(format!(
                                "Unreadable app storage could not be quarantined; disk writes are disabled to preserve it: {error}; original error: {original}"
                            ));
                        }
                    }
                }
            }
        }
        if let Some(warning) = &state.load_warning {
            crate::core::Logger::new("com.cove.ui.storage").warning(warning);
        }
        Self {
            path,
            state: Mutex::new(state),
        }
    }

    pub fn default_path() -> PathBuf {
        default_path()
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn load_warning(&self) -> Option<String> {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .load_warning
            .clone()
    }
    pub fn last_save_error(&self) -> Option<String> {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .last_save_error
            .clone()
    }
    pub fn quarantined_path(&self) -> Option<PathBuf> {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .quarantined_path
            .clone()
    }
}
impl AppStorageStore for FileAppStorageStore {
    fn value(&self, key: &str) -> Option<AppStorageValue> {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .values
            .get(key)
            .cloned()
    }

    fn set(&self, key: &str, value: Option<AppStorageValue>) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        match value {
            Some(value) => {
                state.values.insert(key.to_owned(), value);
            }
            None => {
                state.values.remove(key);
            }
        }
        if !state.persistence_enabled {
            return;
        }
        let result = (|| {
            let bytes = serde_json::to_vec(&state.values)?;
            if let Some(parent) = self
                .path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                std::fs::create_dir_all(parent)?;
            }
            cove_system::files::atomic_write(
                &self.path,
                &bytes,
                cove_system::files::Access::Inherit,
            )
        })();
        match result {
            Ok(()) => state.last_save_error = None,
            Err(error) => {
                state.last_save_error = Some(error.to_string());
                crate::core::Logger::new("com.cove.ui.storage").error(format!(
                    "Failed to persist application storage at {}: {error}",
                    self.path.display()
                ));
            }
        }
    }
}

#[derive(Default)]
pub struct MemoryAppStorageStore {
    values: Mutex<BTreeMap<String, AppStorageValue>>,
}
impl MemoryAppStorageStore {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn values(&self) -> BTreeMap<String, AppStorageValue> {
        self.values
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
}
impl AppStorageStore for MemoryAppStorageStore {
    fn value(&self, key: &str) -> Option<AppStorageValue> {
        self.values
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(key)
            .cloned()
    }
    fn set(&self, key: &str, value: Option<AppStorageValue>) {
        let mut values = self
            .values
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        match value {
            Some(value) => {
                values.insert(key.into(), value);
            }
            None => {
                values.remove(key);
            }
        }
    }
}

pub trait AppStorageConvertible: Clone + PartialEq + Send + 'static {
    fn from_storage_value(value: &AppStorageValue) -> Option<Self>;
    fn storage_value(&self) -> AppStorageValue;
}

impl AppStorageConvertible for bool {
    fn from_storage_value(value: &AppStorageValue) -> Option<Self> {
        if let AppStorageValue::Bool(value) = value {
            Some(*value)
        } else {
            None
        }
    }
    fn storage_value(&self) -> AppStorageValue {
        AppStorageValue::Bool(*self)
    }
}
impl AppStorageConvertible for i64 {
    fn from_storage_value(value: &AppStorageValue) -> Option<Self> {
        if let AppStorageValue::Int(value) = value {
            Some(*value)
        } else {
            None
        }
    }
    fn storage_value(&self) -> AppStorageValue {
        AppStorageValue::Int(*self)
    }
}
macro_rules! signed_storage_integer {
    ($($integer:ty),* $(,)?) => {$(
        impl AppStorageConvertible for $integer {
            fn from_storage_value(value: &AppStorageValue) -> Option<Self> { i64::from_storage_value(value)?.try_into().ok() }
            fn storage_value(&self) -> AppStorageValue { AppStorageValue::Int(*self as i64) }
        }
    )* };
}
signed_storage_integer!(i8, i16, i32, isize);
impl AppStorageConvertible for f64 {
    fn from_storage_value(value: &AppStorageValue) -> Option<Self> {
        match value {
            AppStorageValue::Double(value) => Some(*value),
            AppStorageValue::Int(value) => Some(*value as f64),
            _ => None,
        }
    }
    fn storage_value(&self) -> AppStorageValue {
        AppStorageValue::Double(*self)
    }
}
impl AppStorageConvertible for f32 {
    fn from_storage_value(value: &AppStorageValue) -> Option<Self> {
        f64::from_storage_value(value).map(|value| value as f32)
    }
    fn storage_value(&self) -> AppStorageValue {
        AppStorageValue::Double(f64::from(*self))
    }
}
impl AppStorageConvertible for String {
    fn from_storage_value(value: &AppStorageValue) -> Option<Self> {
        if let AppStorageValue::String(value) = value {
            Some(value.clone())
        } else {
            None
        }
    }
    fn storage_value(&self) -> AppStorageValue {
        AppStorageValue::String(self.clone())
    }
}

pub trait AppStorageRawRepresentable: Clone + PartialEq + Send + 'static {
    type RawValue: AppStorageConvertible;
    fn from_raw_value(raw: Self::RawValue) -> Option<Self>;
    fn raw_value(&self) -> Self::RawValue;
}

impl<T: AppStorageRawRepresentable> AppStorageConvertible for T {
    fn from_storage_value(value: &AppStorageValue) -> Option<Self> {
        Self::from_raw_value(T::RawValue::from_storage_value(value)?)
    }
    fn storage_value(&self) -> AppStorageValue {
        self.raw_value().storage_value()
    }
}

struct AppStorageBox<T: AppStorageConvertible> {
    key: String,
    store: Arc<dyn AppStorageStore>,
    value: T,
    next_observer: u64,
    observers: BTreeMap<u64, Arc<dyn Fn(T) + Send + Sync>>,
}

pub struct AppStorage<T: AppStorageConvertible> {
    key: String,
    state: State<T>,
    shared: Arc<Mutex<AppStorageBox<T>>>,
    _subscription: Arc<Mutex<Option<BoxSubscription<T>>>>,
}
impl<T: AppStorageConvertible> Clone for AppStorage<T> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            state: self.state.clone(),
            shared: self.shared.clone(),
            _subscription: self._subscription.clone(),
        }
    }
}

impl<T: AppStorageConvertible> AppStorage<T> {
    pub fn new(
        key: impl Into<String>,
        default_value: T,
        store: Option<Arc<dyn AppStorageStore>>,
    ) -> Self {
        let key = key.into();
        let store = store.unwrap_or_else(AppStorageDefaults::store);
        let shared = shared_box(&key, store, default_value);
        let value = shared
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .value
            .clone();
        let state = State::new(value);
        let observer_state = state.clone();
        let subscription = BoxSubscription::new(
            &shared,
            Arc::new(move |value| {
                if observer_state.get() != value {
                    observer_state.set(value);
                }
            }),
        );
        Self {
            key,
            state,
            shared,
            _subscription: Arc::new(Mutex::new(Some(subscription))),
        }
    }

    pub fn key(&self) -> &str {
        &self.key
    }
    pub fn get(&self) -> T {
        self.state.get()
    }
    pub fn set(&self, value: T) {
        set_box_value(&self.shared, value);
    }
    pub fn binding(&self) -> Binding<T> {
        let get_box = self.shared.clone();
        let set_box = self.shared.clone();
        Binding::new(
            move || {
                get_box
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .value
                    .clone()
            },
            move |value| set_box_value(&set_box, value),
        )
    }
    pub fn dynamic_field(&self) -> crate::DynamicField {
        crate::DynamicField::state(self.key.clone(), &self.state)
    }
}

fn shared_box<T: AppStorageConvertible>(
    key: &str,
    store: Arc<dyn AppStorageStore>,
    default: T,
) -> Arc<Mutex<AppStorageBox<T>>> {
    static BOXES: OnceLock<Mutex<HashMap<String, Arc<dyn Any + Send + Sync>>>> = OnceLock::new();
    let identity = format!(
        "{}#{}#{key}",
        Arc::as_ptr(&store) as *const () as usize,
        std::any::type_name::<T>()
    );
    let mut boxes = BOXES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if let Some(existing) = boxes
        .get(&identity)
        .and_then(|value| value.clone().downcast::<Mutex<AppStorageBox<T>>>().ok())
    {
        return existing;
    }
    let value = store
        .value(key)
        .and_then(|value| T::from_storage_value(&value))
        .unwrap_or(default);
    let created = Arc::new(Mutex::new(AppStorageBox {
        key: key.to_owned(),
        store,
        value,
        next_observer: 1,
        observers: BTreeMap::new(),
    }));
    boxes.insert(identity, created.clone());
    created
}

struct BoxSubscription<T: AppStorageConvertible> {
    box_ref: Weak<Mutex<AppStorageBox<T>>>,
    id: u64,
}
impl<T: AppStorageConvertible> BoxSubscription<T> {
    fn new(shared: &Arc<Mutex<AppStorageBox<T>>>, callback: Arc<dyn Fn(T) + Send + Sync>) -> Self {
        let mut box_state = shared.lock().unwrap_or_else(|error| error.into_inner());
        let id = box_state.next_observer;
        box_state.next_observer = box_state
            .next_observer
            .checked_add(1)
            .expect("app storage observer id exhausted");
        box_state.observers.insert(id, callback);
        Self {
            box_ref: Arc::downgrade(shared),
            id,
        }
    }
}
impl<T: AppStorageConvertible> Drop for BoxSubscription<T> {
    fn drop(&mut self) {
        if let Some(shared) = self.box_ref.upgrade() {
            shared
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .observers
                .remove(&self.id);
        }
    }
}

fn set_box_value<T: AppStorageConvertible>(shared: &Arc<Mutex<AppStorageBox<T>>>, value: T) {
    let callbacks = {
        let mut box_state = shared.lock().unwrap_or_else(|error| error.into_inner());
        if box_state.value == value {
            return;
        }
        box_state.value = value.clone();
        box_state
            .store
            .set(&box_state.key, Some(value.storage_value()));
        box_state.observers.values().cloned().collect::<Vec<_>>()
    };
    for callback in callbacks {
        callback(value.clone());
    }
}

fn default_path() -> PathBuf {
    platform_base().join("CoveUI").join("app-storage.json")
}

/// Which platform directory policy applies. Kept as data so every host,
/// including iOS and Android, can be verified without a device.
/// Each build constructs only its own variant; the other policies are
/// exercised by `storage_path_tests` so every host stays verifiable offline.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StoragePlatform {
    Windows,
    Apple,
    Android,
    Unix,
    Other,
}

struct StoragePaths {
    home: Option<PathBuf>,
    config: Option<PathBuf>,
    bundle: Option<PathBuf>,
}

/// `config` is `APPDATA` / `XDG_CONFIG_HOME` / `COVE_APP_STORAGE_DIR`.
/// `bundle` is Android's `PACKAGE_RESOURCE_BUNDLE_PATH` (`<filesDir>/runtime`),
/// whose parent is the app-private files directory. iOS `HOME` already points
/// at the sandbox container, so `Library/Application Support` is the durable
/// location; Android has no usable `HOME` and must not write to `.local/share`.
fn base_for(platform: StoragePlatform, paths: &StoragePaths) -> PathBuf {
    let temp = std::env::temp_dir;
    match platform {
        StoragePlatform::Windows => paths.config.clone().unwrap_or_else(temp),
        StoragePlatform::Apple => paths
            .home
            .as_ref()
            .map(|home| home.join("Library/Application Support"))
            .unwrap_or_else(temp),
        StoragePlatform::Android => paths
            .config
            .clone()
            .or_else(|| {
                paths
                    .bundle
                    .as_ref()
                    .and_then(|bundle| bundle.parent())
                    .map(Path::to_path_buf)
            })
            .unwrap_or_else(temp),
        StoragePlatform::Unix => paths
            .config
            .clone()
            .or_else(|| paths.home.as_ref().map(|home| home.join(".config")))
            .unwrap_or_else(temp),
        StoragePlatform::Other => temp(),
    }
}

fn platform_base() -> PathBuf {
    #[cfg(target_os = "windows")]
    let platform = StoragePlatform::Windows;
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos",
        target_os = "visionos"
    ))]
    let platform = StoragePlatform::Apple;
    #[cfg(target_os = "android")]
    let platform = StoragePlatform::Android;
    #[cfg(all(
        unix,
        not(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos",
            target_os = "android"
        ))
    ))]
    let platform = StoragePlatform::Unix;
    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos",
        target_os = "visionos",
        target_os = "android",
        unix
    )))]
    let platform = StoragePlatform::Other;

    #[cfg(target_os = "windows")]
    let config = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(target_os = "android")]
    let config = std::env::var_os("COVE_APP_STORAGE_DIR").map(PathBuf::from);
    #[cfg(all(
        unix,
        not(any(target_os = "macos", target_os = "android", target_os = "windows"))
    ))]
    let config = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
    #[cfg(not(any(
        target_os = "windows",
        target_os = "android",
        all(
            unix,
            not(any(target_os = "macos", target_os = "android", target_os = "windows"))
        )
    )))]
    let config = None;

    #[cfg(target_os = "android")]
    let bundle = std::env::var_os("PACKAGE_RESOURCE_BUNDLE_PATH").map(PathBuf::from);
    #[cfg(not(target_os = "android"))]
    let bundle = None;

    base_for(
        platform,
        &StoragePaths {
            home: std::env::var_os("HOME").map(PathBuf::from),
            config,
            bundle,
        },
    )
}

fn quarantine_path(path: &Path) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let suffix = path
        .extension()
        .map(|extension| format!(".{}", extension.to_string_lossy()))
        .unwrap_or_default();
    path.with_file_name(format!("{stem}.corrupt-{stamp}{suffix}"))
}

#[cfg(test)]
mod storage_path_tests {
    use super::*;

    fn paths(home: Option<&str>) -> StoragePaths {
        StoragePaths {
            home: home.map(PathBuf::from),
            config: None,
            bundle: None,
        }
    }

    #[test]
    fn ios_uses_the_sandbox_application_support_directory() {
        assert_eq!(
            base_for(StoragePlatform::Apple, &paths(Some("/sandbox"))),
            PathBuf::from("/sandbox/Library/Application Support")
        );
    }

    #[test]
    fn android_prefers_the_host_supplied_private_directory() {
        let paths = StoragePaths {
            home: Some("/".into()),
            config: Some("/data/user/0/app/files".into()),
            bundle: Some("/data/user/0/app/files/runtime".into()),
        };
        assert_eq!(
            base_for(StoragePlatform::Android, &paths),
            PathBuf::from("/data/user/0/app/files")
        );
    }

    #[test]
    fn android_falls_back_to_the_resource_bundle_parent() {
        let paths = StoragePaths {
            home: None,
            config: None,
            bundle: Some("/data/user/0/app/files/runtime".into()),
        };
        assert_eq!(
            base_for(StoragePlatform::Android, &paths),
            PathBuf::from("/data/user/0/app/files")
        );
    }

    #[test]
    fn android_never_writes_to_home_local_share() {
        let paths = StoragePaths {
            home: Some("/".into()),
            config: None,
            bundle: None,
        };
        assert_eq!(
            base_for(StoragePlatform::Android, &paths),
            std::env::temp_dir()
        );
    }

    #[test]
    fn desktop_policies_are_unchanged() {
        assert_eq!(
            base_for(
                StoragePlatform::Windows,
                &StoragePaths {
                    home: None,
                    config: Some("C:/Users/me/AppData/Roaming".into()),
                    bundle: None,
                }
            ),
            PathBuf::from("C:/Users/me/AppData/Roaming")
        );
        assert_eq!(
            base_for(
                StoragePlatform::Unix,
                &StoragePaths {
                    home: Some("/home/me".into()),
                    config: None,
                    bundle: None,
                }
            ),
            PathBuf::from("/home/me/.config")
        );
    }
}
