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
        if decimals > 0
            && text.contains('.')
        {
            while text.ends_with('0') {
                text.pop();
            }
            if text.ends_with('.') {
                text.pop();
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
        // Normalize non-ASCII digits (Arabic-Indic, Devanagari, ...) to ASCII.
        draft = draft
            .chars()
            .map(|character| match character.to_digit(10) {
                Some(digit) if !character.is_ascii_digit() => char::from_digit(digit, 10).unwrap_or(character),
                _ => character,
            })
            .collect();
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
        if (floor as i64) % 2 == 0 { floor } else { floor + 1.0 }
    } else {
        value.round()
    }
}
