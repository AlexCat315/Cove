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
    root: AnyView,
    pub configuration: ApplicationConfiguration,
    pub checkpoint: Option<ApplicationCheckpoint>,
    /// Diagnostics backend installed for this run. A build that links the
    /// DevTools crate supplies it; otherwise DevTools stays off.
    pub diagnostics: Option<DiagnosticsFactory>,
    menu_bar: Option<LaunchMenuBar>,
    content_wrapper: Option<Box<dyn Fn(AnyView) -> AnyView>>,
}

enum LaunchMenuBar {
    Native(crate::NativeMenuBar),
    #[cfg(target_os = "windows")]
    Immersive(crate::NativeMenuBar),
}

impl ApplicationLaunch {
    pub fn new(root: impl View + 'static) -> Self {
        Self {
            root: AnyView::new(root),
            configuration: ApplicationConfiguration::default(),
            checkpoint: None,
            diagnostics: None,
            menu_bar: None,
            content_wrapper: None,
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

    /// Presents the application menu with AppKit on macOS.
    /// When the platform has no native application menu, keeps the system frame.
    pub fn native_menu_bar(mut self, menus: impl Into<crate::NativeMenuBar>) -> Self {
        self.menu_bar = Some(LaunchMenuBar::Native(menus.into()));
        self
    }

    /// Presents the same menus in the Cove title bar on Windows or in the
    /// system application menu bar on hosts that provide one.
    pub fn menu_bar(self, menus: impl Into<crate::NativeMenuBar>) -> Self {
        #[cfg(target_os = "windows")]
        {
            let mut launch = self;
            launch.menu_bar = Some(LaunchMenuBar::Immersive(menus.into()));
            launch
        }
        #[cfg(not(target_os = "windows"))]
        {
            if crate::NativeMenuBar::is_supported() {
                self.native_menu_bar(menus)
            } else {
                self
            }
        }
    }

    /// Applies an app-wide view wrapper, such as a theme, to the complete
    /// content. On Windows this includes the title bar installed by `menu_bar`.
    pub fn wrap_content<V: View + 'static>(
        mut self,
        wrap: impl Fn(AnyView) -> V + 'static,
    ) -> Self {
        self.content_wrapper = Some(Box::new(move |content| AnyView::new(wrap(content))));
        self
    }

    /// Resolves the content for an embedded host such as Android or the web.
    pub fn content(&self) -> AnyView {
        self.apply_content_wrapper(self.root.clone())
    }

    fn apply_content_wrapper(&self, content: AnyView) -> AnyView {
        match &self.content_wrapper {
            Some(wrap) => wrap(content),
            None => content,
        }
    }

    fn window_content(&self) -> AnyView {
        let content = match &self.menu_bar {
            #[cfg(target_os = "windows")]
            Some(LaunchMenuBar::Immersive(bar)) => {
                let mut menus = bar.menus.clone();
                if !bar.application_items.is_empty() {
                    let name = if bar.app_name.is_empty() {
                        &self.configuration.window.title
                    } else {
                        &bar.app_name
                    };
                    menus.insert(
                        0,
                        crate::NativeMenu::new(name, bar.application_items.clone()),
                    );
                }
                crate::desktop::window::ImmersiveMenuBar::new(menus)
                    .title(&self.configuration.window.title)
                    .with_content(self.root.clone())
            }
            _ => self.root.clone(),
        };
        self.apply_content_wrapper(content)
    }

    /// Chooses what the platform composites behind clear pixels of the window.
    pub fn window_backdrop(mut self, backdrop: crate::WindowBackdrop) -> Self {
        self.configuration.window.backdrop = backdrop;
        self
    }

    /// Runs the native window host on the calling thread, which then owns the
    /// platform directly. Android invokes the registered factory from its Activity
    /// instead of entering this blocking loop.
    pub fn run(mut self) -> Result<(), String> {
        let root = self.window_content();
        crate::desktop::diagnostics::integration::set_diagnostics_factory(self.diagnostics);
        let mut callbacks = ApplicationCallbacks::default();
        match self.menu_bar.take() {
            Some(LaunchMenuBar::Native(mut bar)) => {
                self.configuration.window.title_bar_style =
                    crate::desktop::application::configuration::AppWindowTitleBarStyle::Standard;
                if crate::NativeMenuBar::is_supported() {
                    if bar.app_name.is_empty() {
                        bar.app_name = self.configuration.window.title.clone();
                        bar.application_titles =
                            crate::desktop::window::NativeApplicationMenuTitles::for_app_name(
                                &bar.app_name,
                            );
                    }
                    callbacks.ready = Some(std::rc::Rc::new(
                        move |display: &crate::AppDisplayHandle| {
                            display.install_native_menu_bar(&bar);
                        },
                    ));
                } else {
                    cove_logging::LogSystem::logger("com.cove.ui.application").log(
                        &cove_logging::LogEvent::new(
                            cove_logging::Level::Warning,
                            "Native application menus are unavailable on this system",
                        ),
                    );
                }
            }
            #[cfg(target_os = "windows")]
            Some(LaunchMenuBar::Immersive(_)) => {
                self.configuration.window.title_bar_style =
                    crate::desktop::application::configuration::AppWindowTitleBarStyle::HiddenInset;
            }
            None => {}
        }
        Application::run(self.configuration, None, root, callbacks, self.checkpoint)
    }

    /// The process entry: called on the main thread, it keeps that thread as the
    /// platform pump and builds and runs the application on a UI thread. The
    /// declaration is made there because views are bound to the thread that
    /// created them.
    ///
    /// `COVE_SINGLE_THREAD=1` runs UI and platform on the main thread instead. iOS
    /// enters UIKit through SDL and stays on its main thread, as does Android
    /// through its Activity.
    pub fn launch(factory: impl FnOnce() -> Self + Send + 'static) -> Result<(), String> {
        #[cfg(target_os = "ios")]
        {
            cove_shell::PlatformApplication::run(move || {
                factory()
                    .run()
                    .map_err(cove_shell::ShellError::InitializationFailed)
            })
            .map_err(|error| error.to_string())
        }
        #[cfg(target_os = "android")]
        {
            factory().run()
        }
        #[cfg(target_arch = "wasm32")]
        {
            // The browser host drives frames through its C ABI and reads the
            // declaration published by the module initializer, so there is no
            // blocking loop to enter here.
            let _ = factory;
            Ok(())
        }
        #[cfg(not(any(target_os = "ios", target_os = "android", target_arch = "wasm32")))]
        {
            if std::env::var_os("COVE_SINGLE_THREAD").is_some_and(|value| value == "1") {
                return factory().run();
            }
            cove_runtime::platform::run_with_ui_thread(
                || {
                    cove_shell::make_default_shell()
                        .map(|shell| Box::new(shell) as Box<dyn cove_shell::Shell>)
                },
                move || factory().run(),
            )
            .map_err(|error| error.to_string())?
        }
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use cove_composition::{DynamicField, Scope, ViewGraph};
    use cove_core::State;
    use cove_core::scene::{Node, NodeTree, Recomposer};
    use cove_ui_compose::primitives::text::Text;
    use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
    use cove_ui_compose::theme::styles::circular_appearance::ThemeTransition;
    use cove_ui_compose::theme::{Theme, theme_for_node};
    use std::{cell::RefCell, rc::Rc};

    struct ThemedContent {
        appearance: State<Appearance>,
        content: AnyView,
    }

    impl View for ThemedContent {
        fn body(&self, _cx: &Scope) -> AnyView {
            AnyView::new(
                self.content
                    .clone()
                    .appearance(self.appearance.get(), ThemeTransition::Instant),
            )
        }

        fn dynamic_properties(&self) -> Vec<DynamicField> {
            vec![DynamicField::state("appearance", &self.appearance)]
        }
    }

    #[test]
    fn wrapper_applies_to_the_window_menu_and_body() {
        let light = Theme::default_light();
        let appearance = State::new(Appearance::Light);
        let observed = appearance.clone();
        let launch = ApplicationLaunch::new(Text::new("Body"))
            .menu_bar(vec![
                crate::NativeMenu::named("Navigate").action("Home", || {}),
            ])
            .configure(|config| config.window.title = "Configured caption".into())
            .wrap_content(move |content| ThemedContent {
                appearance: observed.clone(),
                content,
            });
        let tree = Rc::new(RefCell::new(NodeTree::new()));
        let graph = Rc::new(ViewGraph::new(tree.clone(), Rc::new(Recomposer::new())));
        graph.install_view(launch.window_content(), Default::default());
        graph.recomposer.commit_all();
        graph.compute_layout(450.0, 300.0);

        fn find(node: &Node, predicate: &impl Fn(&Node) -> bool) -> Option<Node> {
            if predicate(node) {
                Some(node.clone())
            } else {
                node.children()
                    .iter()
                    .find_map(|child| find(child, predicate))
            }
        }

        let root = tree.borrow().root().unwrap();
        let labelled = |label| {
            find(&root, &|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.label == label)
            })
            .expect("label is mounted")
        };
        let menu = labelled("Navigate");
        let caption = labelled("Configured caption");
        let body = labelled("Body");
        for node in [&menu, &caption, &body] {
            assert_eq!(theme_for_node(node).colors, light.colors);
        }
        assert!(!caption.is_hit_testable(), "caption preserves window drag");
        let menu = menu.absolute_frame();
        let caption = caption.absolute_frame();
        assert!(menu.x + menu.width <= caption.x);
        assert!(caption.x + caption.width <= 450.0);
        assert!(body.absolute_frame().y >= menu.y + menu.height);
        assert_eq!(body.absolute_frame().max_y(), 300.0);

        let title_bar = || {
            find(&root, &|node| {
                node.attachment::<String>(cove_composition::LayoutDebugAttachmentKey::DEBUG_NAME)
                    .as_deref()
                    == Some("app-window-title-bar")
            })
            .expect("mounted title bar")
        };
        assert_eq!(title_bar().background_color(), Some(light.colors.surface));
        let dark = Theme::default_dark();
        appearance.set(Appearance::Dark);
        cove_core::scene::drain_scene_invalidations();
        graph.recomposer.commit_all();
        graph.compute_layout(450.0, 300.0);
        assert_eq!(
            theme_for_node(&labelled("Body")).colors.background,
            dark.colors.background
        );
        assert_eq!(title_bar().background_color(), Some(dark.colors.surface));
    }
}

/// Declares an application's native entry points.
///
/// Pass a root factory for defaults, or `launch = factory` for configuration and
/// checkpoint hooks. Both forms expose `application()` for constructing the
/// declaration, `run()` for desktop/iOS, `JNI_OnLoad` for Android, and a Wasm
/// module initializer that publishes the declaration to the browser host.
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
            $crate::ApplicationLaunch::launch(application)
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

        /// Publishes the declaration when the Wasm module initializes, before the
        /// browser host makes its first `cove_render` call.
        #[cfg(all(target_arch = "wasm32", not(test)))]
        #[used]
        #[link_section = ".init_array"]
        static __COVE_APPLICATION_INIT: extern "C" fn() = {
            extern "C" fn register() {
                $crate::wasm::install_application(application);
            }
            register
        };
    };
}
