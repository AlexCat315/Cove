use cove_logging::{Level, Location, LogRuntime};
use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock, RwLock},
};
static RUNTIME: OnceLock<RwLock<Option<Arc<LogRuntime>>>> = OnceLock::new();
pub fn set_runtime_logger(runtime: Option<Arc<LogRuntime>>) {
    *RUNTIME
        .get_or_init(|| RwLock::new(None))
        .write()
        .unwrap_or_else(|e| e.into_inner()) = runtime;
}
pub(super) fn log(level: Level, message: &str) {
    let runtime = RUNTIME
        .get_or_init(|| RwLock::new(None))
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    if let Some(runtime) = runtime {
        let _ = runtime.log(
            level,
            "com.cove.ui.runtime",
            message,
            BTreeMap::new(),
            Location::default(),
        );
    }
}
