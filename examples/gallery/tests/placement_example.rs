//! Behaviour tests for the placement playground.
//!
//! Translation of
//! `GalleryContent/Stories/Overlays/Popconfirm/PlacementExample.swift`. The compact
//! mirror has three branches that are easy to transpose: a leading placement must
//! park the trigger on the right so the bubble has room on the left, and vice
//! versa.

use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::placement_example::{
    PlacementExample, VARIANTS, compact_anchor, compact_trigger_width, shows_full_grid,
};
use cove_ui_compose::primitives::overlays::PopoverPlacement;

#[test]
fn the_twelve_variants_cover_every_placement_once() {
    assert_eq!(VARIANTS.len(), 12);
    let mut seen: Vec<PopoverPlacement> =
        VARIANTS.iter().map(|variant| variant.placement).collect();
    seen.sort_by_key(|placement| PopoverPlacement::ALL.iter().position(|p| p == placement));
    assert_eq!(
        seen.len(),
        PopoverPlacement::ALL.len(),
        "one variant per placement"
    );
    for placement in PopoverPlacement::ALL {
        assert!(seen.contains(&placement), "{placement:?} is missing");
    }
}

#[test]
fn the_grid_shows_only_when_wide_enough() {
    assert!(!shows_full_grid(339.0));
    assert!(shows_full_grid(340.0));
    assert!(shows_full_grid(600.0));
}

#[test]
fn a_leading_placement_parks_the_trigger_on_the_right() {
    // Leading means the bubble is on the left, so the trigger must sit right.
    let anchor = compact_anchor(PopoverPlacement::Leading, 200.0, 80.0);
    assert_eq!(anchor.x, 120.0);
    assert_eq!(anchor.justify, cove_core::scene::Justify::FlexEnd);
}

#[test]
fn a_trailing_placement_parks_the_trigger_on_the_left() {
    let anchor = compact_anchor(PopoverPlacement::Trailing, 200.0, 80.0);
    assert_eq!(anchor.x, 0.0);
    assert_eq!(anchor.justify, cove_core::scene::Justify::FlexStart);
}

#[test]
fn a_centred_placement_splits_the_difference() {
    let anchor = compact_anchor(PopoverPlacement::Top, 200.0, 80.0);
    assert_eq!(anchor.x, 60.0);
    assert_eq!(anchor.justify, cove_core::scene::Justify::Center);
}

#[test]
fn the_anchor_never_goes_negative_when_the_trigger_is_wider_than_the_space() {
    let anchor = compact_anchor(PopoverPlacement::Leading, 50.0, 80.0);
    assert_eq!(anchor.x, 0.0, "a too-wide trigger clamps to the edge");
    let centred = compact_anchor(PopoverPlacement::Top, 50.0, 80.0);
    assert_eq!(centred.x, 0.0);
}

#[test]
fn top_placements_sit_low_and_side_placements_centre() {
    assert_eq!(compact_anchor(PopoverPlacement::Top, 200.0, 80.0).y, 130.0);
    assert_eq!(
        compact_anchor(PopoverPlacement::TopLeading, 200.0, 80.0).y,
        130.0
    );
    assert_eq!(
        compact_anchor(PopoverPlacement::Leading, 200.0, 80.0).y,
        55.0
    );
    assert_eq!(
        compact_anchor(PopoverPlacement::TrailingBottom, 200.0, 80.0).y,
        55.0
    );
    assert_eq!(compact_anchor(PopoverPlacement::Bottom, 200.0, 80.0).y, 0.0);
    assert_eq!(
        compact_anchor(PopoverPlacement::BottomTrailing, 200.0, 80.0).y,
        0.0
    );
}

#[test]
fn the_compact_trigger_is_capped_by_the_viewport() {
    // A wide viewport uses the scaled basis.
    assert_eq!(compact_trigger_width(400.0, 1.0), 80.0);
    // A narrow viewport cannot exceed itself.
    assert_eq!(compact_trigger_width(60.0, 1.0), 60.0);
}

#[test]
fn the_story_starts_on_the_top_placement() {
    let story = PlacementExample::new(GalleryOptions::default(), |_| {});
    assert_eq!(story.selected(), 1);
    assert_eq!(story.selected_variant().label, "Top");
}

#[test]
fn selecting_clamps_to_the_last_variant() {
    let story = PlacementExample::new(GalleryOptions::default(), |_| {});
    story.set_selected(99);
    assert_eq!(story.selected(), VARIANTS.len() - 1);
}
