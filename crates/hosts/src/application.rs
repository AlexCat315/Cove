//! Shared native application declaration and entry point.

use crate::{Application, ApplicationCallbacks, ApplicationConfiguration};
use cove_composition::{AnyView, View};

pub use crate::desktop::diagnostics::ApplicationCheckpoint;
pub use crate::desktop::diagnostics::integration::{
    DiagnosticsCallbacks, DiagnosticsFactory, DiagnosticsSession,
};

/// Root content and launch settings shared by desktop, iOS and Android hosts.
/// Window size and chrome are consumed by desktop/iOS; Android owns its window
/// through the Activity and consumes rendering, typography and runtime settings.
pub struct ApplicationLaunch {
    pub root: AnyView,
    pub configuration: ApplicationConfiguration,
    pub checkpoint: Option<ApplicationCheckpoint>,
    /// Diagnostics backend installed for this run. A build that links the
    /// DevTools crate supplies it; otherwise DevTools stays off.
    pub diagnostics: Option<DiagnosticsFactory>,
}

impl ApplicationLaunch {
    pub fn new(root: impl View + 'static) -> Self {
        Self {
            root: AnyView::new(root),
            configuration: ApplicationConfiguration::default(),
            checkpoint: None,
            diagnostics: None,
        }
    }

    /// Installs the diagnostics backend the host builds its DevTools session
    /// from. Ignored unless the configuration enables DevTools.
    pub fn diagnostics(mut self, factory: DiagnosticsFactory) -> Self {
        self.diagnostics = Some(factory);
        self
    }

    pub fn configuration(mut self, configuration: ApplicationConfiguration) -> Self {
        self.configuration = configuration;
        self
    }

    pub fn configure(mut self, configure: impl FnOnce(&mut ApplicationConfiguration)) -> Self {
        configure(&mut self.configuration);
        self
    }

    pub fn checkpoint(mut self, checkpoint: ApplicationCheckpoint) -> Self {
        self.checkpoint = Some(checkpoint);
        self
    }

    /// Runs the native window host. Android invokes the registered factory from
    /// its Activity instead of entering this blocking loop.
    pub fn run(self) -> Result<(), String> {
        crate::desktop::diagnostics::integration::set_diagnostics_factory(self.diagnostics);
        Application::run(
            self.configuration,
            None,
            self.root,
            ApplicationCallbacks::default(),
            self.checkpoint,
        )
    }
}

/// Declares an application's native entry points.
///
/// Pass a root factory for defaults, or `launch = factory` for configuration and
/// checkpoint hooks. Both forms expose `application()` for constructing the
/// declaration, `run()` for desktop/iOS, and `JNI_OnLoad` for Android.
///
/// ```ignore
/// cove_ui::application!(app::make_content);
/// // Or: cove_ui::application!(launch = app::application);
/// ```
#[macro_export]
macro_rules! application {
    ($root:path) => {
        $crate::application!(@declare $crate::ApplicationLaunch::new($root()));
    };
    (launch = $factory:path) => {
        $crate::application!(@declare $factory());
    };
    (@declare $launch:expr) => {
        pub fn application() -> $crate::ApplicationLaunch {
            $launch
        }

        pub fn run() -> ::std::result::Result<(), ::std::string::String> {
            application().run()
        }

        /// # Safety
        /// Called once by the VM when it loads this library.
        #[cfg(all(target_os = "android", not(test)))]
        #[unsafe(no_mangle)]
        #[allow(non_snake_case)]
        pub unsafe extern "C" fn JNI_OnLoad(
            vm: *mut ::std::ffi::c_void,
            reserved: *mut ::std::ffi::c_void,
        ) -> i32 {
            $crate::android::install_application(application);
            // SAFETY: forwarded from the VM's library load.
            unsafe { $crate::android::jni::on_load(vm, reserved) }
        }
    };
}
