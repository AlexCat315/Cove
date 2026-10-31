//! Translation of `Primitives/Controls/Fields/ColorField.swift` (the model half:
//! hex formatting, parsing and panel sizing).
//!
//! Behaviour checklist:
//! - a colour formats as uppercase `#RRGGBB`, or `#RRGGBBAA` when alpha is shown
//! - channel values clamp to 0…255 after rounding
//! - parsing accepts an optional leading `#`, in 6 or 8 hex digits, any case
//! - parsing rejects any other length or a non-hex character
//! - an 8-digit value's last pair is alpha
//! - the popover width is clamped between 176 and 280, inset 24 from the viewport

use cove_core::Color;

/// Formats a colour as an uppercase hex string.
pub fn hex_string(color: Color, show_alpha: bool) -> String {
    let (r, g, b) = (clamp8(color.r), clamp8(color.g), clamp8(color.b));
    if show_alpha {
        format!("#{r:02X}{g:02X}{b:02X}{:02X}", clamp8(color.a))
    } else {
        format!("#{r:02X}{g:02X}{b:02X}")
    }
}

/// Rounds one channel from 0…1 to 0…255.
fn clamp8(value: f32) -> u8 {
    // `(value * 255).rounded()` then clamped, so NaN maps to 0 rather than
    // trapping.
    if !value.is_finite() {
        return 0;
    }
    (value * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Parses `#RRGGBB` or `#RRGGBBAA`, in either case, with an optional `#`.
///
/// Six digits produce an opaque colour; eight take the last pair as alpha.
pub fn parse_hex(text: &str) -> Option<Color> {
    let trimmed = text.trim();
    let digits = trimmed.strip_prefix('#').unwrap_or(trimmed);
    if !digits
        .chars()
        .all(|character| character.is_ascii_hexdigit())
    {
        return None;
    }
    let channel = |value: u32| (value & 0xFF) as f32 / 255.0;
    match digits.len() {
        6 => {
            let value = u32::from_str_radix(digits, 16).ok()?;
            Some(Color {
                r: channel(value >> 16),
                g: channel(value >> 8),
                b: channel(value),
                a: 1.0,
            })
        }
        8 => {
            let value = u32::from_str_radix(digits, 16).ok()?;
            Some(Color {
                r: channel(value >> 24),
                g: channel(value >> 16),
                b: channel(value >> 8),
                a: channel(value),
            })
        }
        _ => None,
    }
}

/// The popover width for a given viewport width.
pub fn panel_width(viewport: f32) -> f32 {
    (viewport - 24.0).clamp(176.0, 280.0)
}
