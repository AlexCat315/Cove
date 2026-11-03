//! Translation of `Tests/CoveUIRuntimeTests/FallbackStyleTests.swift`.
//!
//! When Latin text is styled bold and a CJK cluster falls back to PingFang, the
//! fallback has to be the *bold* PingFang face, not the regular one, or a bold
//! heading mixes weights mid-line. Swift builds its styled fallback stack from
//! the primary font's real CoreText weight trait rather than from the requested
//! `FontWeight`.
//!
//! Named difference (production gap, not fixed here):
//! `cove_text::FontProvider::covering_apple_font` constructs each stack family
//! with `AppleFont::new(Some(family), size, self.weight, self.italic)` instead of
//! copying the primary font's resolved `kCTFontWeightTrait` into the descriptor
//! the way `FontProvider.swift` does. On this machine that makes the bold CJK
//! fallback resolve to `PingFangSC-Regular`, identical to the regular request, so
//! Swift's `bold != regular` / `weight(bold) > weight(regular)` assertions cannot
//! hold. The reload-stability and run-coverage behaviours do hold and are
//!
//! Behaviour checklist:
//! - reloading the primary font refreshes the cached fallback style
//! - a reload back to regular reuses the same fallback font identity
//! - resolved runs concatenate back to the original text

use cove_core::FontWeight;
use cove_text::{FontProvider, SystemFontDefaults};

/// The Swift suite pinned `.AppleSystemUIFont`; Rust resolves the same request
/// through `SystemFontDefaults::primary_font_name()`, which is what every host
/// passes in production.
fn provider() -> FontProvider {
    let mut provider = FontProvider::new(14.0, 1.0);
    assert!(
        provider
            .load_primary_font(
                &SystemFontDefaults::primary_font_name(),
                FontWeight::Regular,
                false
            )
            .is_some(),
        "the system primary font loads"
    );
    provider
}

fn fallback_face(weight: FontWeight) -> (String, i64) {
    let mut provider = FontProvider::new(14.0, 1.0);
    assert!(
        provider
            .load_primary_font(&SystemFontDefaults::primary_font_name(), weight, false)
            .is_some(),
        "the system primary font loads at {weight:?}"
    );
    let last = provider
        .resolve_runs("ab粗体")
        .last()
        .map(|run| (run.font.post_script_name.clone(), run.font.id))
        .expect("the CJK cluster resolves to a fallback run");
    (last.0, last.1)
}

#[test]
fn primary_style_change() {
    let mut provider = provider();
    let regular = provider
        .resolve_runs("ab粗体")
        .last()
        .map(|run| (run.font.post_script_name.clone(), run.font.id))
        .expect("the CJK cluster resolves");

    assert!(
        provider
            .load_primary_font(
                &SystemFontDefaults::primary_font_name(),
                FontWeight::Bold,
                false
            )
            .is_some(),
        "the primary font reloads bold"
    );
    let bold_primary = provider
        .primary_font()
        .map(|font| font.post_script_name.clone())
        .expect("the reloaded primary font is reported");
    assert!(
        bold_primary.contains("Bold"),
        "the reloaded primary face is the bold one, got {bold_primary}",
    );
    let bold = provider
        .resolve_runs("ab粗体")
        .last()
        .map(|run| run.font.post_script_name.clone())
        .expect("the CJK cluster resolves while bold");

    // Swift asserts the CJK fallback becomes heavier here. It does not: see the
    // module-level named difference.
    assert_ne!(
        bold, regular.0,
        "bold CJK falls back to the same face as regular ({bold}); Swift \
         FontProvider.swift:435-448 copies the primary's kCTFontWeightTrait into \
         the styled fallback descriptor and expects a heavier PingFang face",
    );

    assert!(
        provider
            .load_primary_font(
                &SystemFontDefaults::primary_font_name(),
                FontWeight::Regular,
                false
            )
            .is_some(),
        "the primary font reloads regular"
    );
    let reloaded = provider
        .resolve_runs("ab粗体")
        .last()
        .map(|run| run.font.id)
        .expect("the CJK cluster resolves after the reload");
    assert_eq!(
        reloaded, regular.1,
        "reloading the primary font restores the cached regular fallback identity",
    );
    assert_eq!(
        provider
            .resolve_runs("ab粗体")
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>(),
        "ab粗体",
        "resolved runs concatenate back to the sourced text",
    );
}

#[test]
fn bold_fallback() {
    let (regular_name, _) = fallback_face(FontWeight::Regular);
    let (bold_name, _) = fallback_face(FontWeight::Bold);

    // Swift asserts `regular != bold`, `weight(regular) <= 0.05` and
    // `weight(bold) > weight(regular)`, plus a shared family prefix. Only the
    // family invariant survives in the current port.
    assert_ne!(
        regular_name, bold_name,
        "regular and bold CJK both fall back to {regular_name}; Swift expects \
         distinct faces of the same family (bold heavier than regular)",
    );
    let family = |name: &str| {
        name.split('-')
            .next()
            .map(str::to_owned)
            .unwrap_or_default()
    };
    assert_eq!(
        family(&regular_name),
        family(&bold_name),
        "both weights stay in one family ({regular_name} vs {bold_name})",
    );
}
