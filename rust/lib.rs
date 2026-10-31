//! Rust foundation for Cove. See `Docs/rust-migration.md` for the migration boundary.
//! The native UI runtime is not yet available through this crate.
pub use cove_core as core;
pub use cove_devtools_protocol as devtools_protocol;
pub use cove_logging as logging;
pub use cove_platform as platform;
pub use cove_system as system;

pub use cove_graphics as graphics;
pub use cove_imaging as imaging;
pub use cove_text as text;

pub use cove_browser as browser;
pub use cove_composition as composition;
pub use cove_devtools as devtools;
pub use cove_hosts as hosts;
pub use cove_terminal as terminal;
pub use cove_ui_addons as addons;
pub use cove_ui_compose as compose;

/// Declares an application's root view for every platform host.
///
/// Expands to `pub fn run()`, which the desktop and iOS `main` calls, and on
/// Android to the library's `JNI_OnLoad`, which installs the same root for the
/// APK host.
///
/// ```ignore
/// cove_ui::application!(app::make_application_content);
/// ```
#[macro_export]
macro_rules! application {
    ($root:path) => {
        /// Runs the application until its last window closes.
        pub fn run() -> ::std::result::Result<(), ::std::string::String> {
            $crate::hosts::Application::run(
                ::std::default::Default::default(),
                ::std::option::Option::None,
                $crate::composition::AnyView::new($root()),
                ::std::default::Default::default(),
                ::std::option::Option::None,
            )
        }

        /// # Safety
        /// Called once by the VM when it loads this library.
        #[cfg(target_os = "android")]
        #[unsafe(no_mangle)]
        #[allow(non_snake_case)]
        pub unsafe extern "C" fn JNI_OnLoad(
            vm: *mut ::std::ffi::c_void,
            reserved: *mut ::std::ffi::c_void,
        ) -> i32 {
            $crate::hosts::android::install_application_content(|| {
                $crate::composition::AnyView::new($root())
            });
            // SAFETY: forwarded from the VM's library load.
            unsafe { $crate::hosts::android::jni::on_load(vm, reserved) }
        }
    };
}
