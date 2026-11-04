//! Behaviour tests for the data stories.
//!
//! Translation of `GalleryContent/Stories/Data/{ChartExploration,AdaptiveTables}.swift`.

use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::data_stories::{
    AdaptiveTableSample, CHART_FOCUS_RANGE, CHART_SAMPLE_COUNT, ChartExplorationStory,
    numeric_comparison,
};
use cove_ui_compose::primitives::data::chart::ChartRange;
use cove_ui_compose::primitives::data::data_table::table_column::ComparisonResult;

#[test]
fn the_chart_starts_at_the_full_domain() {
    let story = ChartExplorationStory::new();
    assert_eq!(story.viewport(), None);
    assert_eq!(story.selection(), None);
    assert_eq!(story.revision(), 0);
}

#[test]
fn focus_zooms_to_the_interesting_range() {
    let story = ChartExplorationStory::new();
    story.focus();
    assert_eq!(
        story.viewport(),
        Some(ChartRange::new(CHART_FOCUS_RANGE.0, CHART_FOCUS_RANGE.1))
    );
}

#[test]
fn reset_clears_both_the_zoom_and_the_brush() {
    let story = ChartExplorationStory::new();
    story.focus();
    story.set_selection(Some(ChartRange::new(21.0, 30.0)));
    story.reset();
    assert_eq!(story.viewport(), None);
    assert_eq!(story.selection(), None);
}

#[test]
fn updating_the_data_bumps_the_revision_and_changes_the_curve() {
    let story = ChartExplorationStory::new();
    let before = story.samples();
    story.update_data();
    assert_eq!(story.revision(), 1);
    let after = story.samples();
    assert_ne!(
        before, after,
        "the phase shift must visibly change the signal"
    );
}

#[test]
fn the_chart_plots_a_fixed_sample_count() {
    let story = ChartExplorationStory::new();
    assert_eq!(story.samples().len(), CHART_SAMPLE_COUNT);
}

#[test]
fn the_brushed_caption_appears_only_with_a_selection() {
    let story = ChartExplorationStory::new();
    assert_eq!(story.brushed_caption(), None);
    story.set_selection(Some(ChartRange::new(20.0, 40.0)));
    assert_eq!(
        story.brushed_caption().as_deref(),
        Some("Selected 20.0–40.0")
    );
}

#[test]
fn the_adaptive_sample_defaults_to_the_data_table() {
    let sample = AdaptiveTableSample::new(GalleryOptions::default(), true);
    assert!(sample.uses_data_table());
    assert_eq!(sample.row_count(), 30);
}

#[test]
fn the_quantity_column_sorts_numerically() {
    // The displayed text is "14" vs "7"; sorting must not fall back to text order.
    assert_eq!(numeric_comparison(1, 2), ComparisonResult::Ascending);
    assert_eq!(numeric_comparison(2, 1), ComparisonResult::Descending);
    assert_eq!(numeric_comparison(3, 3), ComparisonResult::Same);
}
