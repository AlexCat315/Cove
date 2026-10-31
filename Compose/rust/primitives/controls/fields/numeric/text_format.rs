//! Translation of Primitives/Controls/Fields/Numeric/TextFormat.swift.
//!
//! Locale affects presentation and input only; the binding stays a plain `f32`.
//! Strict parsing consumes the full draft instead of accepting numeric prefixes.

/// Rounding used for display, matching `NumberFormatter`'s `.halfEven`.
pub const MAX_DECIMALS: usize = 6;

/// Locale-aware number presentation for numeric fields.
#[derive(Clone, Debug, PartialEq)]
pub struct NumericTextFormat {
    /// Decimal separator for this locale. `'.'` for the invariant locale.
    pub decimal_separator: char,
    /// Grouping separator, if the locale defines one.
    pub grouping_separator: Option<char>,
    pub minus_sign: char,
    pub plus_sign: char,
    pub decimals: usize,
}

impl NumericTextFormat {
    pub fn new(decimal_separator: char, decimals: usize) -> Self {
        Self {
            decimal_separator,
            grouping_separator: None,
            minus_sign: '-',
            plus_sign: '+',
            decimals: decimals.min(MAX_DECIMALS),
        }
    }

    pub fn invariant(decimals: usize) -> Self {
        Self::new('.', decimals)
    }

    /// Format with at most `decimals` fraction digits, no grouping.
    pub fn format(&self, value: f32) -> String {
        let value = if value == 0.0 { 0.0 } else { value };
        let decimals = self.decimals.min(MAX_DECIMALS);
        // Half-even rounding at the requested precision, without grouping.
        let factor = 10f64.powi(decimals as i32);
        let scaled = (value as f64) * factor;
        let rounded = round_half_even(scaled) / factor;
        let mut text = format!("{rounded:.decimals$}");
        // Trim trailing fraction zeros, matching `minimumFractionDigits = 0`.
        if decimals > 0 && text.contains('.') {
            while text.ends_with('0') {
                text.pop();
            }
            if text.ends_with('.') {
                text.pop();
            }
        }
        // `Format` keeps the sign of a rounded-to-zero value (`-0`); the source
        // re-renders anything that parses back as zero as positive zero.
        if rounded == 0.0 {
            text = format!("{:.decimals$}", 0.0f64);
            if decimals > 0 && text.contains('.') {
                while text.ends_with('0') {
                    text.pop();
                }
                if text.ends_with('.') {
                    text.pop();
                }
            }
        }
        if self.decimal_separator != '.' {
            text = text.replace('.', &self.decimal_separator.to_string());
        }
        text
    }

    /// Strict parse: the whole draft must be a number.
    ///
    /// Grouping is deliberately unsupported: a decimal editor must not silently
    /// reinterpret `1.234` as `1234` in a comma-decimal locale.
    pub fn parse(&self, text: &str) -> Option<f32> {
        let mut draft = text.trim().to_string();
        if self.decimal_separator != '.' && draft.contains('.') {
            return None;
        }
        if let Some(grouping) = self.grouping_separator
            && grouping != self.decimal_separator
            && draft.contains(grouping)
        {
            return None;
        }
        draft = draft.replace(self.decimal_separator, ".");
        draft = draft.replace(self.minus_sign, "-");
        draft = draft.replace(self.plus_sign, "+");
        // Strip bidi control characters the platform may inject.
        draft = draft
            .chars()
            .filter(|character| !matches!(*character, '\u{061C}' | '\u{200E}' | '\u{200F}'))
            .collect();
        // Normalize non-ASCII decimal digits (Arabic-Indic, Devanagari, ...) to
        // ASCII. `char::to_digit` only understands ASCII, so map every Nd block
        // through its zero codepoint instead.
        draft = draft.chars().map(decimal_digit_ascii).collect();
        if draft.is_empty() {
            return None;
        }
        let value: f32 = draft.parse().ok()?;
        if value.is_finite() { Some(value) } else { None }
    }
}

/// `f64` half-even rounding, matching `NumberFormatter.roundingMode`.
fn round_half_even(value: f64) -> f64 {
    let floor = value.floor();
    let fraction = value - floor;
    if (fraction - 0.5).abs() < f64::EPSILON {
        if (floor as i64) % 2 == 0 {
            floor
        } else {
            floor + 1.0
        }
    } else {
        value.round()
    }
}

/// The zero codepoint of every Unicode `Nd` block.
///
/// Each block is ten contiguous codepoints, so a character is a decimal digit
/// exactly when it falls in `zero..zero + 10`. Generated from the Unicode
/// general-category data; all 770 `Nd` characters are covered.
const DECIMAL_ZERO_BLOCKS: [u32; 77] = [
    0x0030, 0x0660, 0x06F0, 0x07C0, 0x0966, 0x09E6, 0x0A66, 0x0AE6, 0x0B66, 0x0BE6, 0x0C66, 0x0CE6,
    0x0D66, 0x0DE6, 0x0E50, 0x0ED0, 0x0F20, 0x1040, 0x1090, 0x17E0, 0x1810, 0x1946, 0x19D0, 0x1A80,
    0x1A90, 0x1B50, 0x1BB0, 0x1C40, 0x1C50, 0xA620, 0xA8D0, 0xA900, 0xA9D0, 0xA9F0, 0xAA50, 0xABF0,
    0xFF10, 0x104A0, 0x10D30, 0x10D40, 0x11066, 0x110F0, 0x11136, 0x111D0, 0x112F0, 0x11450,
    0x114D0, 0x11650, 0x116C0, 0x116D0, 0x116DA, 0x11730, 0x118E0, 0x11950, 0x11BF0, 0x11C50,
    0x11D50, 0x11DA0, 0x11DE0, 0x11F50, 0x16130, 0x16A60, 0x16AC0, 0x16B50, 0x16D70, 0x1CCF0,
    0x1D7CE, 0x1D7D8, 0x1D7E2, 0x1D7EC, 0x1D7F6, 0x1E140, 0x1E2F0, 0x1E4F0, 0x1E5F1, 0x1E950,
    0x1FBF0,
];

/// Maps a decimal digit in any script to its ASCII digit; other characters are
/// returned unchanged.
///
/// Swift uses `Character.wholeNumberValue`, which accepts every `Nd` character.
/// The parse path is per keystroke over a short draft, so a linear scan of the
/// block table is cheaper than a binary search's bookkeeping.
pub fn decimal_digit_ascii(character: char) -> char {
    let scalar = character as u32;
    for zero in DECIMAL_ZERO_BLOCKS {
        if scalar >= zero && scalar < zero + 10 {
            return char::from_digit(scalar - zero, 10).unwrap_or(character);
        }
    }
    character
}
