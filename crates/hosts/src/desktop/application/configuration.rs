//! Translation of Desktop/Sources/CoveUI/Application/Configuration.swift.

use cove_core::GeometryAntialiasing;
use cove_graphics::rhi::DeviceConfig;
use cove_imaging::ImageCacheLimits;
use cove_logging::LoggingConfiguration;
use cove_platform::WindowId;
use cove_text::SystemFontDefaults;
use cove_text::TextCacheLimits;
use cove_ui_compose::theme::tokens::TextScaler;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AppWindowTitleBarStyle {
    #[default]
    Standard,
    HiddenInset,
    /// No title bar and no traffic lights. Intended for shaped floating
    /// widgets whose whole window is a 56–300pt silhouette.
    Borderless,
}

impl AppWindowTitleBarStyle {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn platform_style(self) -> cove_shell::WindowTitleBarStyle {
        match self {
            Self::Standard => cove_shell::WindowTitleBarStyle::Standard,
            Self::HiddenInset => cove_shell::WindowTitleBarStyle::HiddenInset,
            Self::Borderless => cove_shell::WindowTitleBarStyle::Borderless,
        }
    }
}

/// How the single-row title bar meets the content beneath it.
///
/// Both variants share the same row, the same window controls and the same
/// backdrop; only the row's fill and its bottom hairline differ. This keeps the
/// choice independent of [`AppWindowTitleBarStyle`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TitleBarDivider {
    /// The window material runs straight through the bar: no fill, no line.
    #[default]
    Seamless,
    /// A tinted surface plus a one-pixel hairline, like a classic menu bar.
    Hairline,
}

impl TitleBarDivider {
    pub fn separated(self) -> bool {
        self == Self::Hairline
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AppFrameDrivePolicy {
    /// Redraw when input, state, animations or an explicit request changes the UI.
    #[default]
    EventDriven,
    /// Redraw at display cadence, for games and explicit benchmarks.
    Continuous,
}

/// `None` values inherit the application rendering defaults.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AppWindowRendering {
    pub geometry_antialiasing: Option<GeometryAntialiasing>,
    pub sample_count: Option<u32>,
    pub clear_color: Option<[f32; 4]>,
}

impl AppWindowRendering {
    pub fn new(
        sample_count: Option<u32>,
        clear_color: Option<[f32; 4]>,
        geometry_antialiasing: Option<GeometryAntialiasing>,
    ) -> Self {
        Self {
            sample_count,
            clear_color,
            geometry_antialiasing,
        }
    }
}

/// Public alias for the click-through silhouette type used by window config.
pub type AppWindowShape = cove_shell::WindowShape;
/// Public alias for the initial placement descriptor.
pub type AppWindowPlacement = WindowPlacement;

/// Where the window first appears. Coordinates are in window units (logical
/// points); with `display` set they are relative to that monitor's origin.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WindowPlacement {
    /// Top-left position. `None` centers on the target display, or lets the OS
    /// choose when no display is given.
    pub position: Option<(f32, f32)>,
    /// Monitor index in platform display order. `None` uses the default.
    pub display: Option<usize>,
}

impl WindowPlacement {
    pub fn at(x: f32, y: f32) -> Self {
        Self {
            position: Some((x, y)),
            display: None,
        }
    }

    pub fn on_display(index: usize) -> Self {
        Self {
            position: None,
            display: Some(index),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindowConfiguration {
    pub title: String,
    pub width: i32,
    pub height: i32,
    /// Initial monitor/placement. `None` keeps the platform default.
    pub placement: Option<WindowPlacement>,
    /// Optional click-through silhouette (circle / rounded-rectangle). Useful
    /// for floating widgets; the renderer must draw a matching alpha channel.
    pub shape: Option<cove_shell::WindowShape>,
    /// Smallest size the user may resize the window to, in logical points.
    /// Zero leaves that axis unbounded.
    pub min_width: i32,
    pub min_height: i32,
    pub title_bar_style: AppWindowTitleBarStyle,
    /// Visual separation of the single-row menu bar. Independent of `backdrop`
    /// and `title_bar_style`, so both looks use the same layout and material.
    pub title_bar_divider: TitleBarDivider,
    pub shows_dock_icon: bool,
    pub rendering: AppWindowRendering,
}

impl Default for WindowConfiguration {
    fn default() -> Self {
        Self {
            title: std::env::current_exe()
                .ok()
                .and_then(|path| {
                    path.file_stem()
                        .map(|stem| stem.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| "CoveUI".to_owned()),
            width: 1280,
            height: 720,
            min_width: 0,
            min_height: 0,
            title_bar_style: AppWindowTitleBarStyle::Standard,
            title_bar_divider: TitleBarDivider::Seamless,
            shows_dock_icon: true,
            placement: None,
            shape: None,
            rendering: AppWindowRendering::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypographyConfiguration {
    pub primary_font_name: String,
    pub font_size: f32,
    pub line_height: f32,
    pub scaler: TextScaler,
}

impl Default for TypographyConfiguration {
    fn default() -> Self {
        Self {
            primary_font_name: SystemFontDefaults::primary_font_name(),
            font_size: 14.0,
            line_height: 20.0,
            scaler: TextScaler::identity(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderingConfiguration {
    pub clear_color: [f32; 4],
    pub device: DeviceConfig,
    pub sample_count: u32,
    pub geometry_antialiasing: GeometryAntialiasing,
    pub vsync: bool,
}

impl Default for RenderingConfiguration {
    fn default() -> Self {
        Self {
            clear_color: [0.05, 0.06, 0.08, 1.0],
            device: DeviceConfig::default(),
            sample_count: 4,
            geometry_antialiasing: GeometryAntialiasing::Multisample,
            vsync: true,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FramesConfiguration {
    pub policy: AppFrameDrivePolicy,
    pub target_rate: Option<f64>,
}

impl FramesConfiguration {
    pub fn validated_target_rate(&self) -> Option<f64> {
        let target = self
            .target_rate
            .filter(|rate| rate.is_finite() && *rate > 0.0)?;
        Some(target.clamp(1.0, 240.0))
    }
}

/// DevTools launch configuration, resolved from the environment when omitted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiagnosticsConfiguration {
    pub dev_tools: Option<super::super::diagnostics::integration::DevToolsConfiguration>,
    pub shows_performance_hud: bool,
}

impl DiagnosticsConfiguration {
    /// A debug build attaches DevTools on its own, so `cargo run` needs neither
    /// an environment variable nor a second command. `COVE_DEVTOOLS` overrides
    /// that default: the documented off values disable it, anything else opts
    /// in. A release build stays off unless it opts in.
    ///
    /// Android is excluded from the debug default: a self-hosted controller
    /// publishes its endpoint through a discovery file, and an app process has
    /// no writable location a desktop can read, so attaching on its own aborted
    /// startup with `Set COVE_DEVTOOLS_SESSION_FILE for local discovery`. A
    /// paired launch (`cargo cove run --devtools`) supplies an explicit
    /// connection and is unaffected.
    pub fn from_environment(
        app_title: &str,
    ) -> Option<super::super::diagnostics::integration::DevToolsConfiguration> {
        let enabled = match std::env::var("COVE_DEVTOOLS") {
            Ok(value) => !matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "" | "0" | "false" | "no" | "off"
            ),
            Err(_) => cfg!(debug_assertions) && !cfg!(target_os = "android"),
        };
        enabled.then(
            || super::super::diagnostics::integration::DevToolsConfiguration {
                app_title: app_title.to_owned(),
                window: None,
                connection: None,
            },
        )
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct MemoryConfiguration {
    pub text: TextCacheLimits,
    pub images: ImageCacheLimits,
}

/// Application defaults are owned by the subsystem that consumes them.
/// Configure only the groups the application needs; native GPU setup is
/// optional.
#[derive(Clone, Debug, PartialEq)]
pub struct ApplicationConfiguration {
    pub logging: LoggingConfiguration,
    pub window: WindowConfiguration,
    pub typography: TypographyConfiguration,
    pub rendering: RenderingConfiguration,
    pub frames: FramesConfiguration,
    pub diagnostics: DiagnosticsConfiguration,
    pub memory: MemoryConfiguration,
}

impl Default for ApplicationConfiguration {
    fn default() -> Self {
        let window = WindowConfiguration::default();
        let diagnostics = DiagnosticsConfiguration {
            dev_tools: DiagnosticsConfiguration::from_environment(&window.title),
            shows_performance_hud: false,
        };
        Self {
            logging: LoggingConfiguration::default(),
            window,
            typography: TypographyConfiguration::default(),
            rendering: RenderingConfiguration::default(),
            frames: FramesConfiguration::default(),
            diagnostics,
            memory: MemoryConfiguration::default(),
        }
    }
}

impl ApplicationConfiguration {
    pub fn new(title: Option<&str>) -> Self {
        let mut configuration = Self::default();
        if let Some(title) = title {
            configuration.window.title = title.to_owned();
            configuration.diagnostics.dev_tools = DiagnosticsConfiguration::from_environment(title);
        }
        configuration
    }

    /// The main window identifier the host opens first.
    pub fn main_window_id(&self) -> WindowId {
        WindowId::MAIN
    }
}

#[cfg(test)]
mod placement_tests {
    use super::*;

    #[test]
    fn placement_at_sets_position_without_display() {
        let p = WindowPlacement::at(100.0, 200.0);
        assert_eq!(p.position, Some((100.0, 200.0)));
        assert_eq!(p.display, None);
    }

    #[test]
    fn placement_on_display_sets_display_without_position() {
        let p = WindowPlacement::on_display(2);
        assert_eq!(p.display, Some(2));
        assert_eq!(p.position, None);
    }

    #[test]
    fn default_configuration_has_no_placement() {
        assert_eq!(WindowConfiguration::default().placement, None);
    }
}

#[cfg(test)]
mod shape_tests {
    use super::*;

    #[test]
    fn window_shape_variants_are_distinct() {
        assert_ne!(
            AppWindowShape::Ellipse,
            AppWindowShape::RoundedRect { radius: 8.0 }
        );
    }

    #[test]
    fn default_configuration_has_no_shape() {
        assert_eq!(WindowConfiguration::default().shape, None);
    }
}
