//! `Examples/Sources/CoveUIDemo/main.swift` and `Launch.swift`: the desktop
//! component showcase.
use cove_composition::AnyView;
use cove_composition::layout::LayoutDirectionExt;
use cove_core::scene::Direction;
use cove_core::{Environment, InputCapabilities};
use cove_examples::counter::CounterView;
use cove_examples::demo::{ShowcaseInput, ShowcaseLaunch, TITLE};
use cove_examples::gallery_content::{CounterContent, GalleryView};
use cove_hosts::desktop::application::configuration::AppFrameDrivePolicy;
use cove_hosts::desktop::diagnostics::ApplicationCheckpoint;
use cove_hosts::{Application, ApplicationCallbacks, ApplicationConfiguration};
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

fn main() -> Result<(), String> {
    let arguments: Vec<String> = std::env::args().collect();
    let launch = ShowcaseLaunch::new(&arguments);
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
        return Application::run(
            configuration,
            None,
            AnyView::new(content),
            ApplicationCallbacks::default(),
            Some(checkpoint),
        );
    }
    let root = AnyView::new(SurfaceReader::new(move |inherited: &Environment| {
        let gallery = GalleryView::new(
            launch.page,
            if launch.dark {
                Appearance::Dark
            } else {
                Appearance::Light
            },
            launch.text_scale,
        );
        AnyView::new(gallery)
            .surface_environment(surface(&launch, inherited))
            .reduced_motion(launch.reduce_motion || inherited.reduces_motion)
            .layout_direction(if launch.rtl {
                Direction::Rtl
            } else {
                Direction::Ltr
            })
    }));
    Application::run(
        configuration,
        None,
        root,
        ApplicationCallbacks::default(),
        None,
    )
}
