//! Process-local preview overrides for the showcase application.
//!
//! Swift sources: `Examples/Sources/CoveUIDemo/Launch.swift` and
//! `Examples/Sources/CoveUIDemo/main.swift`. `main.swift` is an entry point, so
//! it becomes [`demo::main`] rather than a `bin` target: a host calls it with
//! its own argument list.
//!
//! The parsing rules mirror `ShowcaseLaunch` exactly: an extent is accepted only
//! when it parses as `i32` inside `1...8192`, flags are presence-tested, and a
//! `--component` value that names no page falls back to the introduction.

use crate::gallery_content::{GalleryCatalog, GalleryPage};

/// The showcase window title.
pub const TITLE: &str = "CoveUI — Component Showcase";

/// Arguments accepted by the showcase application.
#[derive(Clone, Debug, PartialEq)]
pub struct ShowcaseLaunch {
    /// `--shared-counter` renders the counter instead of the gallery.
    pub shared_counter: bool,
    /// The gallery page selected by `--component NAME`.
    pub page: GalleryPage,
    pub dark: bool,
    pub width: Option<i32>,
    pub height: Option<i32>,
    /// `--no-vsync` disables vertical sync.
    pub vsync: bool,
    /// `--performance-hud` shows the diagnostics overlay.
    pub performance_hud: bool,
    /// `--benchmark` switches frames to a continuous policy.
    pub benchmark: bool,
    pub frame_rate: Option<i32>,
    pub reduce_motion: bool,
    pub rtl: bool,
    /// `--input` selects the synthetic input capabilities.
    pub input: ShowcaseInput,
    /// `--text-scale` multiplies the text scaler.
    pub text_scale: f64,
}

/// `--input` capability presets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShowcaseInput {
    /// No override: the host's own capabilities.
    #[default]
    Inherited,
    Touch,
    Mixed,
    Pointer,
}

impl ShowcaseLaunch {
    /// Parses the argument list, ignoring unknown arguments as the original does.
    pub fn new(arguments: &[String]) -> Self {
        let page = Self::argument("--component", arguments)
            .and_then(|name| GalleryCatalog::page(&name))
            .unwrap_or(GalleryPage::Introduction);
        Self {
            shared_counter: arguments.iter().any(|value| value == "--shared-counter"),
            page,
            dark: arguments.iter().any(|value| value == "--dark"),
            width: Self::extent("--width", arguments),
            height: Self::extent("--height", arguments),
            vsync: !arguments.iter().any(|value| value == "--no-vsync"),
            performance_hud: arguments.iter().any(|value| value == "--performance-hud"),
            benchmark: arguments.iter().any(|value| value == "--benchmark"),
            frame_rate: Self::extent("--frame-rate", arguments),
            reduce_motion: arguments.iter().any(|value| value == "--reduce-motion"),
            rtl: arguments.iter().any(|value| value == "--rtl"),
            input: match Self::argument("--input", arguments).as_deref() {
                Some("touch") => ShowcaseInput::Touch,
                Some("mixed") => ShowcaseInput::Mixed,
                Some("pointer") => ShowcaseInput::Pointer,
                _ => ShowcaseInput::Inherited,
            },
            // `min(2, max(1, value))`, with a non-finite value falling back to 1.
            text_scale: Self::argument("--text-scale", arguments)
                .and_then(|value| value.parse::<f64>().ok())
                .map(|value| if value.is_finite() { value.clamp(1.0, 2.0) } else { 1.0 })
                .unwrap_or(1.0),
        }
    }

    /// The value following `name`, or `None` when absent or last.
    pub fn argument(name: &str, arguments: &[String]) -> Option<String> {
        let index = arguments.iter().position(|value| value == name)?;
        arguments.get(index + 1).cloned()
    }

    /// An extent between 1 and 8192 inclusive.
    fn extent(name: &str, arguments: &[String]) -> Option<i32> {
        let value = Self::argument(name, arguments)?.parse::<i32>().ok()?;
        (1..=8192).contains(&value).then_some(value)
    }

    /// `--benchmark` switches the frame policy from event-driven to continuous.
    pub fn continuous_frames(&self) -> bool {
        self.benchmark
    }
}

impl Default for ShowcaseLaunch {
    fn default() -> Self {
        Self::new(&[])
    }
}

/// `Examples/Sources/CoveUIDemo/main.swift`.
///
/// The Swift entry point builds the launch description and hands it to
/// `Application.run`; the Rust host supplies its own runner, so this returns the
/// launch description for that host to render.
pub fn main(arguments: &[String]) -> ShowcaseLaunch {
    ShowcaseLaunch::new(arguments)
}

#[cfg(test)]
mod checks {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn defaults_match_an_empty_command_line() {
        let launch = ShowcaseLaunch::new(&[]);
        assert!(!launch.shared_counter);
        assert_eq!(launch.page, GalleryPage::Introduction);
        assert!(!launch.dark);
        assert_eq!(launch.width, None);
        assert_eq!(launch.height, None);
        assert!(launch.vsync);
        assert!(!launch.performance_hud);
        assert!(!launch.benchmark);
        assert_eq!(launch.frame_rate, None);
        assert!(!launch.reduce_motion);
        assert!(!launch.rtl);
        assert_eq!(launch.input, ShowcaseInput::Inherited);
        assert_eq!(launch.text_scale, 1.0);
        assert!(!launch.continuous_frames());
    }

    #[test]
    fn extents_require_one_to_8192() {
        let launch = ShowcaseLaunch::new(&args(&["--width", "390", "--height", "8192"]));
        assert_eq!(launch.width, Some(390));
        assert_eq!(launch.height, Some(8192));
        // 0, 8193, and non-numbers are rejected rather than clamped.
        for value in ["0", "8193", "-5", "abc", "", "1.5"] {
            let launch = ShowcaseLaunch::new(&args(&["--width", value]));
            assert_eq!(launch.width, None, "--width {value} must be rejected");
        }
    }

    #[test]
    fn extents_require_a_following_value() {
        assert_eq!(ShowcaseLaunch::new(&args(&["--width"])).width, None);
    }

    #[test]
    fn flags_are_presence_tested() {
        let launch = ShowcaseLaunch::new(&args(&[
            "--dark",
            "--no-vsync",
            "--performance-hud",
            "--benchmark",
            "--reduce-motion",
            "--rtl",
            "--shared-counter",
        ]));
        assert!(launch.dark);
        assert!(!launch.vsync);
        assert!(launch.performance_hud);
        assert!(launch.benchmark);
        assert!(launch.continuous_frames());
        assert!(launch.reduce_motion);
        assert!(launch.rtl);
        assert!(launch.shared_counter);
    }

    #[test]
    fn component_names_resolve_through_the_catalog() {
        assert_eq!(
            ShowcaseLaunch::new(&args(&["--component", "button"])).page,
            GalleryPage::Button
        );
        // Title matching works too, case-insensitively.
        assert_eq!(
            ShowcaseLaunch::new(&args(&["--component", "Dashboard"])).page,
            GalleryPage::Introduction
        );
        // An unknown name falls back to the introduction.
        assert_eq!(
            ShowcaseLaunch::new(&args(&["--component", "nope"])).page,
            GalleryPage::Introduction
        );
        assert_eq!(
            ShowcaseLaunch::new(&args(&["--component"])).page,
            GalleryPage::Introduction
        );
    }

    #[test]
    fn input_presets_select_capabilities() {
        for (value, expected) in [
            ("touch", ShowcaseInput::Touch),
            ("mixed", ShowcaseInput::Mixed),
            ("pointer", ShowcaseInput::Pointer),
            ("keyboard", ShowcaseInput::Inherited),
        ] {
            assert_eq!(
                ShowcaseLaunch::new(&args(&["--input", value])).input,
                expected,
                "--input {value}"
            );
        }
    }

    #[test]
    fn text_scale_is_clamped_to_one_through_two() {
        assert_eq!(ShowcaseLaunch::new(&args(&["--text-scale", "1.5"])).text_scale, 1.5);
        assert_eq!(ShowcaseLaunch::new(&args(&["--text-scale", "0.5"])).text_scale, 1.0);
        assert_eq!(ShowcaseLaunch::new(&args(&["--text-scale", "9"])).text_scale, 2.0);
        assert_eq!(ShowcaseLaunch::new(&args(&["--text-scale", "nope"])).text_scale, 1.0);
    }

    #[test]
    fn frame_rate_uses_the_extent_rule() {
        assert_eq!(
            ShowcaseLaunch::new(&args(&["--frame-rate", "120"])).frame_rate,
            Some(120)
        );
        assert_eq!(
            ShowcaseLaunch::new(&args(&["--frame-rate", "0"])).frame_rate,
            None
        );
    }

    #[test]
    fn main_returns_the_parsed_launch_description() {
        let launch = main(&args(&["--component", "stepper", "--dark"]));
        assert_eq!(launch.page, GalleryPage::Stepper);
        assert!(launch.dark);
        assert_eq!(TITLE, "CoveUI — Component Showcase");
    }
}
