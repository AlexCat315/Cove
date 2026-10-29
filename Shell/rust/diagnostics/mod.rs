use std::{collections::BTreeMap, ffi::{CStr, c_char}, sync::{Arc, OnceLock, RwLock}};
use cove_logging::{Level, Location, LogRuntime, LogValue};
static RUNTIME: OnceLock<RwLock<Option<Arc<LogRuntime>>>> = OnceLock::new();
pub fn set_sdl_log_runtime(runtime: Option<Arc<LogRuntime>>) { *RUNTIME.get_or_init(||RwLock::new(None)).write().unwrap_or_else(|e|e.into_inner())=runtime; }
pub fn install_sdl_logging(){
    unsafe extern "C" fn output(category:i32,priority:i32,message:*const c_char){
        let _=std::panic::catch_unwind(||{
            let runtime=RUNTIME.get_or_init(||RwLock::new(None)).read().unwrap_or_else(|e|e.into_inner()).clone();let Some(runtime)=runtime else{return;};
            let message=if message.is_null(){String::new()}else{ // SAFETY: SDL passes a NUL terminated message valid for this callback.
                unsafe{CStr::from_ptr(message)}.to_string_lossy().into_owned()};
            let level=match priority{1|2=>Level::Trace,3=>Level::Debug,5=>Level::Warning,6=>Level::Error,7=>Level::Critical,_=>Level::Info};
            let _=runtime.log(level,"com.cove.engine.sdl",&message,BTreeMap::from([("category".into(),LogValue::from(category.to_string()))]),Location::default());
        });
    }
    // SAFETY: static function pointer outlives SDL, contains panics and accesses synchronized state only.
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    INSTALLED.call_once(|| { // SAFETY: this static callback is installed only once before SDL begins producing logs.
        unsafe{crate::native::cove_shell_log_install(output)}; });
}
