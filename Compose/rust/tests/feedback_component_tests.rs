//! Translation of `Tests/CoveUIComposeTests/FeedbackComponentTests.swift`.
//!
//! Behaviour checklist:
//! - status tones are a five-case catalogue with five distinct glyphs
//! - solid ink stays legible against its own tone fill in both themes
//! - the command palette ranks tighter matches first and disabled entries last
//! - recent commands lead the browse list once, are enabled-only, and never
//!   duplicate an item
//! - progress clamps out-of-range values and a ring keeps a usable thickness
//! - timeline orders events and hides the trailing connector by default

use std::collections::HashSet;

use cove_ui_compose::primitives::data::{Timeline, TimelineEvent};
use cove_ui_compose::primitives::feedback::{
    ProgressCircle, ProgressView, StatusTone,
};
use cove_ui_compose::primitives::overlays::{
    CommandEntry, CommandItem, display_entries, ranked,
};
use cove_ui_compose::theme::Theme;

fn luminance(color: cove_core::Color) -> f32 {
    0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b
}

fn command(id: &str, group: &str, is_enabled: bool) -> CommandItem {
    let item = CommandItem::new(id, id, group);
    if is_enabled { item } else { item.disabled() }
}

fn entry_ids(entries: Vec<CommandEntry>) -> Vec<String> {
    entries.into_iter().map(|entry| entry.id()).collect()
}

#[test]
fn status_tones_are_five_distinct_glyphs() {
    assert_eq!(StatusTone::ALL.len(), 5, "the catalogue has five tones");
    let glyphs: HashSet<String> = StatusTone::ALL
        .into_iter()
        .map(|tone| tone.icon().name)
        .collect();
    assert_eq!(
        glyphs.len(),
        StatusTone::ALL.len(),
        "every tone maps to its own glyph, got {glyphs:?}"
    );
}

#[test]
fn solid_ink_keeps_contrast_against_its_own_fill() {
    for theme in [Theme::default_dark(), Theme::default_light()] {
        for tone in StatusTone::ALL {
            let ink = tone.ink_on_solid(&theme);
            let fill = tone.color().resolve(&theme);
            let gap = (luminance(ink) - luminance(fill)).abs();
            assert!(
                gap > 0.3,
                "{tone:?} ink washes out on its own fill: ink {:?} fill {:?} gap {gap}",
                ink,
                fill
            );
        }
    }
}

#[test]
fn palette_ranks_tighter_matches_first_and_disabled_last() {
    let items = [
        command("Generate Textures", "Tools", true),
        command("Git Toggle", "Tools", true),
        command("Toggle Theme", "View", true),
        command("Deploy to Device", "Build", false),
    ];

    assert_eq!(
        ranked(&items, "gt").first().map(|item| item.title.as_str()),
        Some("Git Toggle"),
        "`gt` skips fewer characters in the git command"
    );
    assert_eq!(
        ranked(&items, "toggle")
            .first()
            .map(|item| item.title.as_str()),
        Some("Toggle Theme"),
        "a title prefix beats a mid-string hit"
    );

    let ranked_to = ranked(&items, "to");
    let for_to: Vec<&str> = ranked_to
        .iter()
        .map(|item| item.title.as_str())
        .collect();
    assert_eq!(
        for_to.last().copied(),
        Some("Deploy to Device"),
        "a disabled command is listed but never sorts first: {for_to:?}"
    );
    assert_ne!(
        for_to.first().copied(),
        Some("Deploy to Device"),
        "a disabled command is never first: {for_to:?}"
    );

    let authored: Vec<&str> = items.iter().map(|item| item.title.as_str()).collect();
    let browsed_items = ranked(&items, "   ");
    let browsed: Vec<&str> = browsed_items
        .iter()
        .map(|item| item.title.as_str())
        .collect();
    assert_eq!(
        browsed, authored,
        "an empty query keeps authored order and drops nothing"
    );
}

#[test]
fn recent_commands_lead_the_browse_list_once() {
    let items = [
        command("Open File", "File", true),
        command("Toggle Theme", "View", true),
        command("Deploy", "Build", false),
    ];

    let ids = entry_ids(display_entries(
        &items,
        &["Toggle Theme".to_string(), "missing".to_string()],
        true,
        3,
        false,
    ));
    assert_eq!(
        ids,
        ["header:Recent", "item:Toggle Theme", "item:Open File", "item:Deploy"],
        "the known recent leads and the unknown id is dropped"
    );

    let deduped = entry_ids(display_entries(
        &items,
        &["Toggle Theme".to_string()],
        true,
        1,
        false,
    ));
    assert_eq!(
        deduped,
        ["header:Recent", "item:Toggle Theme", "item:Open File", "item:Deploy"],
        "the recent item is not repeated below its own section"
    );

    let disabled = entry_ids(display_entries(
        &items,
        &["Deploy".to_string()],
        true,
        3,
        false,
    ));
    assert!(
        !disabled.iter().any(|id| id == "header:Recent"),
        "a disabled recent produces no section: {disabled:?}"
    );
    assert!(
        disabled.iter().any(|id| id == "item:Deploy"),
        "the disabled command still appears: {disabled:?}"
    );

    let hidden = entry_ids(display_entries(
        &items,
        &["Toggle Theme".to_string()],
        false,
        3,
        false,
    ));
    assert_eq!(
        hidden,
        ["item:Open File", "item:Toggle Theme", "item:Deploy"],
        "hidden recents keep authored order"
    );
}

#[test]
fn progress_clamps_out_of_range_values() {
    assert_eq!(ProgressView::new(Some(2.5)).clamped(), 1.0);
    assert_eq!(ProgressView::new(Some(-1.0)).clamped(), 0.0);
    assert_eq!(ProgressView::new(None).clamped(), 0.0);
    assert_eq!(ProgressView::new(Some(f64::NAN)).clamped(), 0.0);
    assert_eq!(ProgressView::new(Some(0.42)).clamped(), 0.42);
}

#[test]
fn progress_circle_keeps_a_usable_ring_at_every_diameter() {
    let small = ProgressCircle::new(Some(0.5)).diameter(16.0);
    let large = ProgressCircle::new(Some(0.5)).diameter(64.0);

    let small_thickness = small.resolved_thickness(16.0);
    let large_thickness = large.resolved_thickness(64.0);
    assert!(
        small_thickness >= 2.0,
        "a 16pt ring stays visible, got {small_thickness}"
    );
    assert!(
        large_thickness < 32.0,
        "a 64pt ring never swallows its own centre, got {large_thickness}"
    );
    assert!(
        small_thickness < large_thickness,
        "thickness scales with diameter: {small_thickness} vs {large_thickness}"
    );

    let forced = ProgressCircle::new(Some(0.5))
        .diameter(20.0)
        .thickness(40.0);
    assert_eq!(
        forced.resolved_thickness(20.0),
        10.0,
        "an explicit thickness is honoured but clamped to half the diameter"
    );
}

#[test]
fn timeline_orders_events_and_hides_the_trailing_connector() {
    let timeline = Timeline::new(vec![
        TimelineEvent::new("Started").tone(StatusTone::Info),
        TimelineEvent::new("Failed").tone(StatusTone::Danger),
    ]);
    let titles: Vec<&str> = timeline.events().iter().map(|event| event.title.as_str()).collect();
    assert_eq!(titles, ["Started", "Failed"]);
    assert_eq!(timeline.events()[0].tone, StatusTone::Info);
    assert!(
        !timeline.shows_trailing_connector,
        "the sequence ends where the list ends by default"
    );

    let revealed = Timeline::new(Vec::new()).configure(|timeline| {
        timeline.shows_trailing_connector = true;
    });
    assert!(revealed.shows_trailing_connector);
}
