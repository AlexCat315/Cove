//! Showcase configuration and content for the shared application entry point.
use super::{ShowcaseInput, ShowcaseLaunch, ShowcaseMenuBar, TITLE};
use crate::counter::CounterView;
use crate::gallery_content::catalog::GalleryPage;
use crate::gallery_content::{CounterContent, GalleryView};
use cove_composition::layout::LayoutDirectionExt;
use cove_composition::{AnyView, DynamicField, LayoutDebugExt, Scope, View};
use cove_core::scene::Direction;
use cove_core::{Environment, InputCapabilities, State};
use cove_hosts::desktop::application::composition::AppDisplayHandleHolder;
use cove_hosts::desktop::application::configuration::AppFrameDrivePolicy;
use cove_hosts::desktop::diagnostics::ApplicationCheckpoint;
use cove_hosts::{
    ApplicationConfiguration, ApplicationLaunch, NativeMenu, NativeMenuAction, NativeMenuBar,
    NativeMenuItem, NativeMenuKeyModifiers, TitleBarDivider, WindowBackdrop,
};
use cove_ui_compose::adaptive::{SurfaceEnvironment, SurfaceEnvironmentExt, SurfaceReader};
use cove_ui_compose::primitives::controls::{Button, ButtonIconSource};
use cove_ui_compose::primitives::media::ui_icon_catalog as icons;
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::styles::circular_appearance::ThemeTransition;
use cove_ui_compose::theme::tokens::ReducedMotionViewExt;

fn configuration(launch: &ShowcaseLaunch) -> ApplicationConfiguration {
    let mut configuration = ApplicationConfiguration::new(Some(TITLE));
    if let Some(width) = launch.width {
        configuration.window.width = width;
    }
    if let Some(height) = launch.height {
        configuration.window.height = height;
    }
    // The translucent material is the window default, independent of which
    // single-row look is selected.
    configuration.window.backdrop = WindowBackdrop::Sidebar;
    if !launch.shared_counter
        && launch.menu_bar.uses_immersive_title_bar()
        && launch.menu_bar == ShowcaseMenuBar::Separated
    {
        configuration.window.title_bar_divider = TitleBarDivider::Hairline;
    }
    configuration.rendering.vsync = launch.vsync;
    configuration.diagnostics.shows_performance_hud = launch.performance_hud;
    configuration.frames.policy = if launch.continuous_frames() {
        AppFrameDrivePolicy::Continuous
    } else {
        AppFrameDrivePolicy::EventDriven
    };
    if let Some(rate) = launch.frame_rate {
        configuration.frames.target_rate = Some(f64::from(rate));
    }
    configuration
}

/// The host's live viewport with the process-local preview overrides applied.
fn surface(launch: &ShowcaseLaunch, inherited: &Environment) -> Environment {
    let mut surface = inherited.clone();
    match launch.input {
        ShowcaseInput::Touch => surface.input = InputCapabilities::TOUCH,
        ShowcaseInput::Mixed => {
            surface.input =
                InputCapabilities::TOUCH | InputCapabilities::POINTER | InputCapabilities::KEYBOARD;
        }
        ShowcaseInput::Pointer => {
            surface.input = InputCapabilities::POINTER | InputCapabilities::KEYBOARD;
        }
        ShowcaseInput::Inherited => {}
    }
    if launch.reduce_motion {
        surface.reduces_motion = true;
    }
    surface
}

pub fn application() -> ApplicationLaunch {
    ShowcaseLaunch::new(&std::env::args().collect::<Vec<_>>()).application()
}

fn navigation_menu(gallery: &GalleryView) -> NativeMenu {
    NativeMenu::new(
        "Navigate",
        [
            ("Dashboard", "1", GalleryPage::Introduction),
            ("Calendar", "2", GalleryPage::Calendar),
        ]
        .into_iter()
        .map(|(title, key, destination)| {
            let page = gallery.page.clone();
            let showing = gallery.showing_page.clone();
            NativeMenuItem::Action(
                NativeMenuAction::new(title, move || {
                    page.set(destination);
                    showing.set(true);
                })
                .with_shortcut(key, NativeMenuKeyModifiers::primary()),
            )
        })
        .collect(),
    )
}

fn title_bar_divider_button() -> AnyView {
    AnyView::new(SurfaceReader::new(|environment: &Environment| {
        let separated = environment.title_bar_divider;
        let (icon, tip) = if separated {
            (icons::layout::rows_2(), "Show a seamless title bar")
        } else {
            (icons::layout::panel_top(), "Show a title bar divider")
        };
        AnyView::new(
            Button::icon_default(ButtonIconSource::resource(icon), move || {
                if let Some(handle) = AppDisplayHandleHolder::current() {
                    handle.set_title_bar_divider(!separated);
                }
            })
            .selected(separated)
            .tooltip(tip)
            .debug_name("gallery-title-bar-divider"),
        )
    }))
}

struct ShowcaseContent {
    launch: ShowcaseLaunch,
    appearance: State<Appearance>,
    content: AnyView,
}

impl View for ShowcaseContent {
    fn body(&self, cx: &Scope) -> AnyView {
        let inherited = cx.composition_value(&SurfaceEnvironment::key());
        AnyView::new(
            AnyView::new(
                self.content
                    .clone()
                    .appearance(self.appearance.get(), ThemeTransition::circular()),
            )
            .surface_environment(surface(&self.launch, &inherited))
            .reduced_motion(self.launch.reduce_motion || inherited.reduces_motion)
            .layout_direction(if self.launch.rtl {
                Direction::Rtl
            } else {
                Direction::Ltr
            }),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("appearance", &self.appearance)]
    }
}

impl ShowcaseLaunch {
    pub fn application(self) -> ApplicationLaunch {
        let launch = self;
        let configuration = configuration(&launch);
        if launch.shared_counter {
            // The counter view shares its state, so the checkpoint and the
            // rendered content observe the same values.
            let counter = CounterView::new();
            let capture = counter.clone();
            let restore = counter.clone();
            let checkpoint = ApplicationCheckpoint::new(
                move || capture.checkpoint(),
                move |values| restore.restore(values),
            );
            let content = CounterContent::new(counter);
            return ApplicationLaunch::new(content)
                .configuration(configuration)
                .checkpoint(checkpoint)
                .diagnostics(cove_hosts::desktop::diagnostics::devtools::factory());
        }
        let gallery = GalleryView::new(
            launch.page,
            if launch.dark {
                Appearance::Dark
            } else {
                Appearance::Light
            },
            launch.text_scale,
        );
        let menu = (launch.menu_bar != ShowcaseMenuBar::None)
            .then(|| NativeMenuBar::new("CoveUI", vec![], vec![navigation_menu(&gallery)]));
        let appearance = gallery.appearance.clone();
        let root = if launch.menu_bar.uses_immersive_title_bar() {
            // The window menu and the stories share the host's portal layer.
            AnyView::new(
                gallery
                    .with_footer_action(title_bar_divider_button())
                    .into_content(),
            )
        } else {
            AnyView::new(LayerRoot::new(gallery.into_content()))
        };
        let app = ApplicationLaunch::new(root)
            .configuration(configuration)
            .wrap_content(move |content| ShowcaseContent {
                launch: launch.clone(),
                appearance: appearance.clone(),
                content,
            })
            .diagnostics(cove_hosts::desktop::diagnostics::devtools::factory());
        match menu {
            Some(menu) => app.menu_bar(menu),
            _ => app,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launch(flag: &str) -> ShowcaseLaunch {
        ShowcaseLaunch::new(&[
            "gallery".to_owned(),
            "--menu-bar".to_owned(),
            flag.to_owned(),
        ])
    }

    #[test]
    fn immersive_flags_only_set_the_windows_divider_preference() {
        let seamless = configuration(&launch("immersive"));
        let separated = configuration(&launch("separated"));
        assert_eq!(seamless.window.backdrop, separated.window.backdrop);
        assert_eq!(seamless.window.backdrop, WindowBackdrop::Sidebar);
        assert_eq!(seamless.window.title_bar_divider, TitleBarDivider::Seamless);
        assert_eq!(
            separated.window.title_bar_divider,
            if cfg!(target_os = "windows") {
                TitleBarDivider::Hairline
            } else {
                TitleBarDivider::Seamless
            }
        );
    }

    #[test]
    fn shared_navigation_menu_supplies_the_native_items_and_actions() {
        let gallery = GalleryView::new(GalleryPage::Introduction, Appearance::Light, 1.0);
        gallery.show_navigation();
        let (snapshot, actions) = NativeMenuBar::from(vec![navigation_menu(&gallery)]).snapshot();
        assert_eq!(snapshot.menus.len(), 1);
        assert_eq!(snapshot.menus[0].title, "Navigate");
        let items = &snapshot.menus[0].items.items;
        assert_eq!(
            items
                .iter()
                .map(|item| item.title.as_str())
                .collect::<Vec<_>>(),
            ["Dashboard", "Calendar"]
        );
        assert_eq!(
            items[1].shortcut,
            Some(NativeMenuKeyModifiers::primary().shortcut("2"))
        );
        for (item, destination) in [(1, GalleryPage::Calendar), (0, GalleryPage::Introduction)] {
            let (_, action) = actions
                .iter()
                .find(|(id, _)| *id == items[item].id)
                .unwrap();
            (action.action)();
            assert_eq!(gallery.current_page(), destination);
            assert!(gallery.showing_page.get());
        }
    }
}
