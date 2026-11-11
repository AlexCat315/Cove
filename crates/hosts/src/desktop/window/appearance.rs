//! Native chrome follows the composed theme.
//!
//! The title bar, window controls and platform materials take the light or
//! dark appearance of the theme the root provides, not the system setting, so
//! a light UI never sits on a dark sidebar material.
use cove_core::scene::Node;
use cove_shell::WindowAppearance;
use cove_ui_compose::theme::theme_environment::provided_theme_within;

const LAST_KEY: &str = "__window_appearance.last";
const TINT_KEY: &str = "__window_appearance.tint";

/// The appearance the root's theme asks for, when it changed since the last
/// call for this root.
pub fn changed_appearance(root: &Node) -> Option<WindowAppearance> {
    let theme = provided_theme_within(root, 48, 2048)?;
    let background = theme.colors.background;
    let luminance = 0.2126 * background.r + 0.7152 * background.g + 0.0722 * background.b;
    let appearance = if luminance > 0.5 {
        WindowAppearance::Light
    } else {
        WindowAppearance::Dark
    };
    if root.attachment::<WindowAppearance>(LAST_KEY) == Some(appearance) {
        return None;
    }
    root.set_attachment(LAST_KEY, appearance);
    Some(appearance)
}

/// The root paints a continuous surface under the window's chrome and content.
pub fn sync_window_background(root: &Node, translucent: bool, native_title_row: bool) {
    let tint = if translucent || native_title_row {
        provided_theme_within(root, 48, 2048).and_then(|theme| {
            if !translucent {
                return Some(theme.colors.surface_variant);
            }
            let tint =
                cove_ui_compose::theme::tokens::material::window_backdrop_tint(&theme.colors);
            if cfg!(target_os = "windows") {
                // DWM's backdrop can be darker than AppKit's light material.
                tint.or_else(|| Some(theme.colors.surface_variant.multiplied_alpha(0.60)))
            } else {
                tint
            }
        })
    } else {
        None
    };
    if root.attachment::<Option<cove_core::Color>>(TINT_KEY) == Some(tint) {
        return;
    }
    root.set_attachment(TINT_KEY, tint);
    root.set_background_color(tint);
}
