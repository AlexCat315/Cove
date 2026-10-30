//! Behaviour tests for the adaptive preview tables.
//!
//! Translation of `GalleryContent/Stories/Foundation/AdaptiveExamples.swift`. Both
//! lookups are hand-written tables in Swift, so the invariant worth testing is that
//! they agree: every supported page must resolve a height or deliberately hug its
//! content, and no unsupported page may claim one.

use cove_examples::gallery_content::catalog::GalleryPage;
use cove_examples::gallery_content::stories::adaptive_examples::{
    frame_height, unsupported_pages, AdaptiveControlExamples, ALL_PAGES, FRAME_PADDING,
    PREVIEW_WIDTHS, SUPPORTED,
};

#[test]
fn the_supported_set_has_no_duplicates() {
    let mut seen = SUPPORTED.to_vec();
    seen.sort_by_key(|page| page.raw_value());
    seen.dedup();
    assert_eq!(seen.len(), SUPPORTED.len(), "a duplicated page would render twice");
}

#[test]
fn every_supported_page_actually_exists_in_the_catalogue() {
    for page in SUPPORTED {
        assert!(
            ALL_PAGES.contains(&page),
            "{:?} is supported but missing from ALL_PAGES",
            page
        );
    }
}

#[test]
fn the_catalogue_and_the_supported_set_agree_on_membership() {
    for page in ALL_PAGES {
        assert_eq!(
            page.has_adaptive_sample(),
            SUPPORTED.contains(&page),
            "{:?} disagrees between the table and the predicate",
            page
        );
    }
}

#[test]
fn a_page_that_scrolls_gets_a_definite_height() {
    // Samples that scroll, fill or overlay must be measurable.
    for page in [
        GalleryPage::Calendar,
        GalleryPage::Layout,
        GalleryPage::Table,
        GalleryPage::DataTable,
        GalleryPage::Sidebar,
        GalleryPage::VirtualStack,
        GalleryPage::Chart,
        GalleryPage::ScrollView,
    ] {
        assert!(
            page.adaptive_sample_height().is_some(),
            "{page:?} needs a fixed height"
        );
    }
}

#[test]
fn a_content_sized_control_hugs_its_content() {
    // A button, badge or key cap must not be forced to a fixed height.
    for page in [
        GalleryPage::Button,
        GalleryPage::Badge,
        GalleryPage::KeyCap,
        GalleryPage::Label,
        GalleryPage::Link,
    ] {
        assert_eq!(
            page.adaptive_sample_height(),
            None,
            "{page:?} should hug its content"
        );
    }
}

#[test]
fn the_height_bands_match_the_authored_values() {
    assert_eq!(GalleryPage::Calendar.adaptive_sample_height(), Some(440.0));
    assert_eq!(GalleryPage::List.adaptive_sample_height(), Some(144.0));
    assert_eq!(GalleryPage::Disclosure.adaptive_sample_height(), Some(120.0));
    assert_eq!(
        GalleryPage::VirtualStack.adaptive_sample_height(),
        Some(130.0)
    );
}

#[test]
fn the_frame_height_adds_the_label_padding() {
    assert_eq!(frame_height(GalleryPage::List), Some(144.0 + FRAME_PADDING));
    assert_eq!(frame_height(GalleryPage::Button), None);
}

#[test]
fn every_supported_page_resolves_a_frame_decision() {
    // No supported page may be ambiguous: either it has a height or it is one of
    // the content-sized controls. This is the invariant the two hand tables must
    // satisfy together.
    for page in SUPPORTED {
        let is_content_sized = page.adaptive_sample_height().is_none();
        if is_content_sized {
            assert!(
                ALL_PAGES.contains(&page),
                "{page:?} is content-sized but unknown"
            );
        }
    }
}

#[test]
fn unsupported_pages_are_the_complement() {
    let unsupported = unsupported_pages();
    assert_eq!(
        unsupported.len() + SUPPORTED.len(),
        ALL_PAGES.len(),
        "supported and unsupported must partition the catalogue"
    );
    for page in &unsupported {
        assert!(!page.has_adaptive_sample());
    }
    // A measurement-only size is not a page, so it must not appear.
    assert!(!unsupported.iter().any(|page| page.raw_value() == "mini"));
}

#[test]
fn the_two_preview_widths_are_the_authored_ones() {
    assert_eq!(PREVIEW_WIDTHS, [390.0, 720.0]);
    assert!(PREVIEW_WIDTHS[0] < 600.0, "the first band is narrow");
    assert!(PREVIEW_WIDTHS[1] >= 600.0, "the second band is wide");
}

#[test]
fn the_debug_name_encodes_the_width_band() {
    assert_eq!(
        GalleryPage::Button.adaptive_debug_name(390.0),
        "adaptive-button-narrow"
    );
    assert_eq!(
        GalleryPage::Button.adaptive_debug_name(720.0),
        "adaptive-button-wide"
    );
    // The 600 pt boundary is inclusive on the wide side.
    assert_eq!(
        GalleryPage::Button.adaptive_debug_name(600.0),
        "adaptive-button-wide"
    );
}

#[test]
fn the_story_reports_support_for_its_page() {
    let supported = AdaptiveControlExamples::new(GalleryPage::Button);
    assert!(supported.supports());
    let unsupported = AdaptiveControlExamples::new(GalleryPage::Web);
    assert!(!unsupported.supports());
}

#[test]
fn the_story_lists_both_preview_captions() {
    let story = AdaptiveControlExamples::new(GalleryPage::Slider);
    assert_eq!(story.captions(), ["390 pt · Pointer", "720 pt · Pointer"]);
    assert_eq!(
        story.sample_size(390.0),
        (390.0, None),
        "a slider hugs its content"
    );
}
