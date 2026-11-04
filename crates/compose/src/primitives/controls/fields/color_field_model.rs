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

/// Text the colour field can show and parse.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorFormat {
    #[default]
    Hex,
    Hsl,
}

impl ColorFormat {
    pub fn label(self) -> &'static str {
        match self {
            Self::Hex => "HEX",
            Self::Hsl => "HSL",
        }
    }
}

/// Formats a colour in `format`, appending alpha only for hex.
pub fn format_color(color: Color, show_alpha: bool, format: ColorFormat) -> String {
    match format {
        ColorFormat::Hex => match_alpha_hex(color, show_alpha),
        ColorFormat::Hsl => {
            let (h, s, l) = hsl_from_rgb(color);
            format!(
                "{} {:.0}% {:.0}%",
                (h * 360.0).round() as i64,
                (s * 100.0).round(),
                (l * 100.0).round()
            )
        }
    }
}

/// Parses a colour in `format`. Hex accepts `#RRGGBB(AA)`; HSL accepts
/// `h s% l%` with any separators.
pub fn parse_color(text: &str, format: ColorFormat) -> Option<Color> {
    match format {
        ColorFormat::Hex => parse_hex(text),
        ColorFormat::Hsl => parse_hsl(text),
    }
}

fn match_alpha_hex(color: Color, show_alpha: bool) -> String {
    hex_string(color, show_alpha)
}

/// Hue, saturation and lightness, each in `0…1`.
pub fn hsl_from_rgb(color: Color) -> (f32, f32, f32) {
    let max = color.r.max(color.g).max(color.b);
    let min = color.r.min(color.g).min(color.b);
    let lightness = (max + min) / 2.0;
    let delta = max - min;
    if delta <= 0.0 {
        return (0.0, 0.0, lightness);
    }
    let saturation = if lightness <= 0.5 {
        delta / (max + min)
    } else {
        delta / (2.0 - max - min)
    };
    let hue = if max == color.r {
        ((color.g - color.b) / delta).rem_euclid(6.0)
    } else if max == color.g {
        (color.b - color.r) / delta + 2.0
    } else {
        (color.r - color.g) / delta + 4.0
    } / 6.0;
    (hue.rem_euclid(1.0), saturation.clamp(0.0, 1.0), lightness)
}

/// RGB for a hue, saturation and lightness.
pub fn rgb_from_hsl(h: f32, s: f32, l: f32, alpha: f32) -> Color {
    let h = h.rem_euclid(1.0);
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    if s <= 0.0 {
        return Color {
            r: l,
            g: l,
            b: l,
            a: alpha,
        };
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let channel = |t: f32| {
        let t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 1.0 / 2.0 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    Color {
        r: channel(h + 1.0 / 3.0),
        g: channel(h),
        b: channel(h - 1.0 / 3.0),
        a: alpha,
    }
}

/// Parses `h s% l%`, the output of [`format_color`] for HSL.
pub fn parse_hsl(text: &str) -> Option<Color> {
    let trimmed = text.trim();
    let parts: Vec<&str> = trimmed
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() != 3 {
        return None;
    }
    let hue: f32 = parts[0].trim_end_matches('°').parse().ok()?;
    // The percent signs are optional so both `210 65% 62%` and `210 65 62` parse.
    let saturation = parts[1].trim_end_matches('%').parse::<f32>().ok()?;
    let lightness = parts[2].trim_end_matches('%').parse::<f32>().ok()?;
    if !hue.is_finite() || !saturation.is_finite() || !lightness.is_finite() {
        return None;
    }
    Some(rgb_from_hsl(
        hue / 360.0,
        (saturation / 100.0).clamp(0.0, 1.0),
        (lightness / 100.0).clamp(0.0, 1.0),
        1.0,
    ))
}

/// A neutral preset palette: greys plus one row of hues.
pub fn default_swatches() -> Vec<Color> {
    [
        (0, 0, 0),
        (89, 89, 89),
        (166, 166, 166),
        (255, 255, 255),
        (231, 76, 60),
        (230, 126, 34),
        (241, 196, 15),
        (46, 204, 113),
        (26, 188, 156),
        (52, 152, 219),
        (142, 68, 173),
        (233, 30, 99),
    ]
    .into_iter()
    .map(|(r, g, b)| Color::from_rgba8(r, g, b, 255))
    .collect()
}
