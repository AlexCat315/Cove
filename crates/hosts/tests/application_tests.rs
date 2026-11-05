//! Translation of `Tests/CoveUITests/ApplicationTests.swift`.
//!
//! Behaviour checklist:
//! - application defaults are idle and configuration groups are independent
//! - invalid frame rates fall back to the display cadence
//! - window rendering overrides inherit the application defaults
//! - the window render settings resolve against the application defaults

use cove_hosts::ApplicationConfiguration;
use cove_hosts::desktop::application::configuration::{
    AppFrameDrivePolicy, AppWindowRendering, FramesConfiguration, RenderingConfiguration,
};

#[test]
fn defaults_are_idle_and_groups_are_independent() {
    let original = ApplicationConfiguration::default();
    assert_eq!(
        original.frames.policy,
        AppFrameDrivePolicy::EventDriven,
        "the default policy is idle"
    );
    assert!(original.rendering.vsync, "vsync defaults on");
    assert!(original.window.shows_dock_icon, "the dock icon defaults on");
    assert_eq!(
        original.frames.validated_target_rate(),
        None,
        "no authored rate by default"
    );

    let mut customized = ApplicationConfiguration::default();
    customized.window.title = "Consumer".into();
    customized.window.shows_dock_icon = false;
    customized.typography.font_size = 18.0;
    customized.frames = FramesConfiguration {
        policy: AppFrameDrivePolicy::EventDriven,
        target_rate: Some(120.0),
    };

    assert_eq!(customized.window.title, "Consumer");
    assert_eq!(customized.frames.validated_target_rate(), Some(120.0));
    assert_eq!(
        customized.rendering.sample_count, original.rendering.sample_count,
        "touching one group leaves the others at their defaults",
    );
    assert!(!customized.window.shows_dock_icon);
}

#[test]
fn invalid_frame_rates_use_the_display_cadence() {
    for value in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
        let config = ApplicationConfiguration {
            frames: FramesConfiguration {
                policy: AppFrameDrivePolicy::EventDriven,
                target_rate: Some(value),
            },
            ..ApplicationConfiguration::default()
        };
        assert_eq!(
            config.frames.validated_target_rate(),
            None,
            "{value} is not a usable frame rate",
        );
    }

    let config = ApplicationConfiguration {
        frames: FramesConfiguration {
            policy: AppFrameDrivePolicy::EventDriven,
            target_rate: Some(1000.0),
        },
        ..ApplicationConfiguration::default()
    };
    assert_eq!(
        config.frames.validated_target_rate(),
        Some(240.0),
        "an over-range rate clamps to the fastest supported cadence",
    );
}

#[test]
fn window_rendering_overrides_inherit_defaults() {
    let mut config = ApplicationConfiguration::default();
    config.rendering = RenderingConfiguration {
        sample_count: 4,
        ..config.rendering.clone()
    };

    let inherited = cove_hosts::desktop::window::AppWindowRenderSettings::resolve(
        &config,
        AppWindowRendering::default(),
    );
    let overridden = cove_hosts::desktop::window::AppWindowRenderSettings::resolve(
        &config,
        AppWindowRendering::new(Some(2), None, None),
    );

    assert_eq!(
        inherited.samples, 4,
        "an unset window inherits the application sample count"
    );
    assert_eq!(overridden.samples, 2, "the window override wins");
    assert_eq!(
        overridden.clear_color, inherited.clear_color,
        "the untouched field still inherits",
    );

    let clamped = cove_hosts::desktop::window::AppWindowRenderSettings::resolve(
        &config,
        AppWindowRendering::new(Some(0), None, None),
    );
    assert_eq!(clamped.samples, 1, "a zero sample count clamps to one");
}
