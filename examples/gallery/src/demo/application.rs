//! Showcase configuration and content for the shared application entry point.
use super::{ShowcaseInput, ShowcaseLaunch, ShowcaseMenuBar, TITLE};
use crate::counter::CounterView;
use crate::gallery_content::catalog::GalleryPage;
use crate::gallery_content::{CounterContent, GalleryView};
use cove_composition::AnyView;
use cove_composition::layout::LayoutDirectionExt;
use cove_core::scene::Direction;
use cove_core::{Environment, InputCapabilities};
use cove_hosts::desktop::application::configuration::{
    AppFrameDrivePolicy, AppWindowTitleBarStyle,
};
use cove_hosts::desktop::diagnostics::ApplicationCheckpoint;
use cove_hosts::{
    ApplicationConfiguration, ApplicationLaunch, ImmersiveMenuBar, NativeMenu, NativeMenuAction,
    NativeMenuItem, NativeMenuKeyModifiers, TitleBarDivider, WindowBackdrop,
};
use cove_ui_compose::adaptive::{SurfaceEnvironmentExt, SurfaceReader};
use cove_ui_compose::theme::styles::appearance_modifier::Appearance;
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
        && matches!(
            launch.menu_bar,
            ShowcaseMenuBar::Immersive | ShowcaseMenuBar::Separated
        )
    {
        configuration.window.title_bar_style = AppWindowTitleBarStyle::HiddenInset;
    }
    // The divider is the only difference between the two single-row looks.
    configuration.window.title_bar_divider = match launch.menu_bar {
        ShowcaseMenuBar::Separated => TitleBarDivider::Hairline,
        _ => TitleBarDivider::Seamless,
    };
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
        let mut gallery = GalleryView::new(
            launch.page,
            if launch.dark {
                Appearance::Dark
            } else {
                Appearance::Light
            },
            launch.text_scale,
        );
        let page = gallery.page.clone();
        let showing = gallery.showing_page.clone();
        let menu = NativeMenu::new(
            "Navigate",
            [
                ("Dashboard", "1", GalleryPage::Introduction),
                ("Calendar", "2", GalleryPage::Calendar),
            ]
            .into_iter()
            .map(|(title, key, destination)| {
                let page = page.clone();
                let showing = showing.clone();
                NativeMenuItem::Action(
                    NativeMenuAction::new(title, move || {
                        page.set(destination);
                        showing.set(true);
                    })
                    .with_shortcut(key, NativeMenuKeyModifiers::primary()),
                )
            })
            .collect(),
        );
        if launch.menu_bar != ShowcaseMenuBar::None {
            gallery = gallery.with_menu_bar(ImmersiveMenuBar::new(vec![menu]));
        }
        let gallery = AnyView::new(gallery);
        let root = AnyView::new(SurfaceReader::new(move |inherited: &Environment| {
            gallery
                .clone()
                .surface_environment(surface(&launch, inherited))
                .reduced_motion(launch.reduce_motion || inherited.reduces_motion)
                .layout_direction(if launch.rtl {
                    Direction::Rtl
                } else {
                    Direction::Ltr
                })
        }));
        ApplicationLaunch::new(root)
            .configuration(configuration)
            .diagnostics(cove_hosts::desktop::diagnostics::devtools::factory())
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

    /// The two looks must differ only in the divider: same row, same frame,
    /// same translucent material.
    #[test]
    fn single_row_modes_share_the_frame_and_backdrop() {
        let seamless = configuration(&launch("immersive"));
        let separated = configuration(&launch("separated"));
        assert_eq!(seamless.window.backdrop, separated.window.backdrop);
        assert_eq!(seamless.window.backdrop, WindowBackdrop::Sidebar);
        assert_eq!(
            seamless.window.title_bar_style,
            separated.window.title_bar_style
        );
        assert_eq!(
            seamless.window.title_bar_style,
            AppWindowTitleBarStyle::HiddenInset
        );
        assert_eq!(seamless.window.title_bar_divider, TitleBarDivider::Seamless);
        assert_eq!(
            separated.window.title_bar_divider,
            TitleBarDivider::Hairline
        );
    }
}
