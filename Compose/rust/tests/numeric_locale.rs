//! Translation of `Tests/CoveUIComposeTests/NumericLocaleTests.swift`.
//!
//! Behaviour checklist:
//! - localized decimals round-trip through format then parse
//! - a value that rounds to zero formats as zero
//! - `NaN` and infinities are rejected on parse
//! - a comma locale formats with a comma and parses scientific notation
//! - grouping and trailing garbage are rejected instead of silently accepted
//! - half-even rounding is used at the requested precision
//!
//! Model difference: Swift builds the format from a `Locale`. The Rust image
//! takes the decimal separator explicitly (`NumericTextFormat::new`), which is
//! the same input the locale resolves to. Each case below names the locale it
//! mirrors in a comment.

mod support;

use cove_ui_compose::primitives::controls::fields::numeric::text_format::NumericTextFormat;

/// `en_US_POSIX`, `de_DE`, `fr_FR`, `ar_EG` and `sv_SE` all resolve to one of
/// these two separators in the Rust model.
fn separated(separator: char) -> NumericTextFormat {
    NumericTextFormat::new(separator, 2)
}

#[test]
fn localized_decimals_round_trip() {
    for separator in ['.', ','] {
        let format = separated(separator);
        for value in [-1234.5f32, -0.5, 0.0, 1.25, 9999.75] {
            let text = format.format(value);
            let parsed = format.parse(&text).expect("formatted text parses back");
            assert_eq!(
                parsed, value,
                "{value} round-trips through {separator}-decimal formatting as {text}"
            );
        }
    }
}

#[test]
fn values_that_round_to_zero_lose_their_sign() {
    // Swift re-renders anything that parses back as zero as positive zero, so a
    // tiny negative formats as "0" rather than "-0".
    let format = separated('.');
    assert_eq!(format.format(0.0), "0", "an exact zero has no sign");
    assert_eq!(
        format.format(-0.00001),
        "0",
        "a tiny negative renders the same as the reference: unsigned zero"
    );
    assert_eq!(
        format.parse("-0"),
        Some(0.0),
        "either spelling parses back to zero"
    );
    assert_eq!(
        format.parse(&format.format(-0.00001)),
        Some(0.0),
        "the round trip still lands on zero"
    );
}

#[test]
fn non_finite_drafts_are_rejected() {
    let format = separated('.');
    assert_eq!(format.parse("NaN"), None, "NaN is not a number the field accepts");
    assert_eq!(format.parse("infinity"), None, "infinity is not accepted");
    assert_eq!(format.parse("inf"), None, "the short infinity spelling is rejected");
    assert_eq!(format.parse("1e999"), None, "overflowing exponents are rejected");
}

#[test]
fn a_comma_locale_formats_and_parses_locally() {
    let comma = separated(',');
    assert_eq!(comma.format(1234.5), "1234,5", "the decimal separator is a comma");
    assert_eq!(
        comma.parse("12,5e2"),
        Some(1250.0),
        "scientific notation still parses after separator normalization"
    );
    assert_eq!(
        comma.parse("2,5"),
        Some(2.5),
        "a plain decimal comma parses"
    );
}

#[test]
fn grouping_and_partial_parses_are_rejected() {
    let comma = separated(',');
    for invalid in ["1.234", "12,5suffix", "1,2,3", "", "1 234", "1e999"] {
        assert_eq!(
            comma.parse(invalid),
            None,
            "{invalid:?} must not be silently reinterpreted"
        );
    }
}

#[test]
fn grouping_separator_is_rejected_when_configured() {
    let mut format = separated(',');
    format.grouping_separator = Some('.');
    assert_eq!(
        format.parse("1.234"),
        None,
        "a dot is grouping here, so it must not parse as a decimal"
    );
    assert_eq!(format.parse("1234,5"), Some(1234.5), "plain decimals still parse");
}

#[test]
fn rounding_is_half_even_at_the_requested_precision() {
    // 0.125 at 2 decimals: half-even rounds to 0.12 (the even neighbour).
    let format = separated('.');
    assert_eq!(format.format(0.125), "0.12", "half-even rounds to the even digit");
    assert_eq!(format.format(0.135), "0.14", "the other half rounds up");
    // Trailing fraction zeros are trimmed, matching minimumFractionDigits = 0.
    assert_eq!(format.format(2.5), "2.5", "trailing zeros are dropped");
    assert_eq!(format.format(3.0), "3", "an integer has no decimal point");
}

#[test]
fn the_precision_is_capped() {
    let wide = NumericTextFormat::invariant(99);
    assert_eq!(
        wide.decimals, 6,
        "the authored precision clamps to the 6-decimal maximum"
    );
}

#[test]
fn non_ascii_digits_are_normalized() {
    // Swift's `Character.wholeNumberValue` accepts every Unicode `Nd` digit, so
    // any script's decimal digits parse the same as ASCII.
    let format = separated('.');
    assert_eq!(
        format.parse("١٢٥"),
        Some(125.0),
        "Arabic-Indic digits normalize before parsing"
    );
    assert_eq!(
        format.parse("١٢.٥"),
        Some(12.5),
        "a fully localized draft parses"
    );
    assert_eq!(
        format.parse("१२५"),
        Some(125.0),
        "Devanagari digits normalize too"
    );
    assert_eq!(
        format.parse("１２５"),
        Some(125.0),
        "fullwidth digits normalize too"
    );
    assert_eq!(
        format.parse("125"),
        Some(125.0),
        "the ASCII spelling of the same number parses"
    );
}

#[test]
fn the_invariant_locale_uses_a_dot() {
    let format = NumericTextFormat::invariant(2);
    assert_eq!(format.decimal_separator, '.');
    assert_eq!(format.format(1.5), "1.5");
    assert_eq!(format.parse("1.5"), Some(1.5));
}
