//! Translation of `Tests/CoveUIRuntimeTests/FallbackStyleTests.swift`.
//!
//! When Latin text is styled bold and a CJK cluster falls back to PingFang, the
//! fallback has to be the *bold* PingFang face, not the regular one, or a bold
//! heading mixes weights mid-line. Swift builds its styled fallback stack from
//! the primary font's real CoreText weight trait rather than from the requested
//! `FontWeight`.
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

    assert_ne!(
        bold, regular.0,
        "bold CJK resolves to a distinct styled face ({bold})",
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

    assert_ne!(
        regular_name, bold_name,
        "regular and bold CJK resolve distinct faces in one family ({regular_name})",
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

#[cfg(any(target_os = "macos", target_os = "ios"))]
#[test]
fn named_bold_primary_passes_its_resolved_weight_to_fallback() {
    let (regular, _) = fallback_face(FontWeight::Regular);
    let mut provider = FontProvider::new(14.0, 1.0);
    let primary = provider
        .load_primary_font("Helvetica-Bold", FontWeight::Regular, false)
        .expect("named primary");
    assert!(primary.post_script_name.contains("Bold"));
    let fallback = provider
        .resolve_runs("ab粗体")
        .last()
        .expect("CJK fallback")
        .font
        .post_script_name
        .clone();
    assert_ne!(
        fallback, regular,
        "fallback inherits the actual bold face despite a regular request"
    );
}
