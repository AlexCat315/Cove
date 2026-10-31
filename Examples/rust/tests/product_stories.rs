//! Behaviour tests for the product stories.
//!
//! Translation of `GalleryContent/Stories/Layout/ProductStories.swift`, which
//! holds five independent stories. Validation and the simulated airport
//! directory are the parts with real logic.

use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::product_stories::{
    AIRPORTS, ComboboxStory, DEFAULT_MODE, FormStory, MAX_VISIBLE_TOASTS, NotificationStory,
    OFFLINE_QUERY, ProductStory, RADIO_CHOICES, RadioStory, airport_label, is_valid_email,
    product_stories, search_airports, validate_email,
};

#[test]
fn the_radio_story_starts_on_balanced() {
    let story = RadioStory::new(GalleryOptions::default());
    assert_eq!(story.selection().as_deref(), Some(DEFAULT_MODE));
    assert_eq!(story.caption(), "Selected: balanced");
}

#[test]
fn the_disabled_radio_option_refuses_selection() {
    let story = RadioStory::new(GalleryOptions::default());
    assert!(!story.select("offline"));
    assert_eq!(story.selection().as_deref(), Some(DEFAULT_MODE));
    assert!(story.select("quality"));
    assert_eq!(story.selection().as_deref(), Some("quality"));
}

#[test]
fn arrow_navigation_skips_the_disabled_option() {
    let story = RadioStory::new(GalleryOptions::default());
    let navigable = story.navigable();
    assert!(!navigable.contains(&"offline"));
    assert_eq!(navigable, vec!["fast", "balanced", "quality"]);
    assert_eq!(RADIO_CHOICES.len(), 4);
}

#[test]
fn an_email_needs_both_an_at_sign_and_a_dot() {
    assert!(is_valid_email("someone@example.com"));
    assert!(!is_valid_email("someone@example"), "no dot");
    assert!(!is_valid_email("someone.example.com"), "no at sign");
    assert!(!is_valid_email(""));
}

#[test]
fn the_email_rule_reports_its_message() {
    assert_eq!(validate_email("bad"), Some("Enter a valid email address."));
    assert_eq!(validate_email("ok@example.com"), None);
}

#[test]
fn submitting_a_blank_form_fails_and_reports_both_fields() {
    let story = FormStory::new(GalleryOptions::default());
    assert!(!story.submit());
    assert_eq!(story.status(), "Complete the required fields");
    assert!(story.error("name").is_some(), "the name is required");
    assert!(story.error("email").is_some(), "the email is required");
}

#[test]
fn a_bad_email_keeps_the_form_invalid() {
    let story = FormStory::new(GalleryOptions::default());
    story.set_name("Ada");
    story.set_email("not-an-email");
    assert!(!story.submit());
    assert!(story.error("name").is_none());
    assert_eq!(
        story.error("email").as_deref(),
        Some("Enter a valid email address.")
    );
}

#[test]
fn a_complete_form_submits() {
    let story = FormStory::new(GalleryOptions::default());
    story.set_name("Ada Lovelace");
    story.set_email("ada@example.com");
    assert!(story.submit());
    assert_eq!(story.status(), "Account created");
    assert!(story.error("name").is_none());
    assert!(story.error("email").is_none());
}

#[test]
fn a_whitespace_only_name_is_still_missing() {
    let story = FormStory::new(GalleryOptions::default());
    story.set_name("   ");
    story.set_email("ada@example.com");
    assert!(
        !story.submit(),
        "a blank-looking name does not satisfy `required`"
    );
}

#[test]
fn the_local_combobox_starts_on_shanghai() {
    let story = ComboboxStory::new(GalleryOptions::default());
    assert_eq!(story.city().as_deref(), Some("shanghai"));
    assert_eq!(story.caption(), "Selected: shanghai");
}

#[test]
fn the_unavailable_city_cannot_be_chosen() {
    let story = ComboboxStory::new(GalleryOptions::default());
    assert!(!story.select_city("unavailable"));
    assert_eq!(story.city().as_deref(), Some("shanghai"));
    assert!(story.select_city("london"));
    assert_eq!(story.city().as_deref(), Some("london"));
}

#[test]
fn clearing_the_city_reads_as_none() {
    let story = ComboboxStory::new(GalleryOptions::default());
    story.clear_city();
    assert_eq!(story.city(), None);
    assert_eq!(story.caption(), "Selected: None");
    assert_eq!(story.airport(), None);
}

#[test]
fn the_airport_directory_finds_matches_by_code_and_name() {
    assert_eq!(search_airports("PEK").unwrap().len(), 1);
    assert_eq!(
        search_airports("pek").unwrap()[0].code,
        "PEK",
        "matching is case-insensitive"
    );
    let tokyo = search_airports("tokyo").unwrap();
    assert_eq!(tokyo.len(), 2, "Narita and Haneda");
    let london = search_airports("Heathrow").unwrap();
    assert_eq!(london[0].code, "LHR");
}

#[test]
fn an_empty_airport_query_returns_everything() {
    assert_eq!(search_airports("").unwrap().len(), AIRPORTS.len());
    assert_eq!(AIRPORTS.len(), 12);
}

#[test]
fn the_offline_query_fails() {
    let error = search_airports(OFFLINE_QUERY).unwrap_err();
    assert_eq!(error, "The airport service is unreachable.");
    // Matching is case-insensitive, so the failure is reachable however it is typed.
    assert!(search_airports("OFFLINE").is_err());
}

#[test]
fn an_unmatched_airport_query_is_empty_not_an_error() {
    let found = search_airports("zzzz");
    assert!(found.is_ok());
    assert!(found.unwrap().is_empty());
}

#[test]
fn airport_labels_pair_the_code_with_the_name() {
    assert_eq!(airport_label(AIRPORTS[0]), "PEK · Beijing Capital");
}

#[test]
fn the_toast_queue_is_bounded_by_its_cap() {
    let story = NotificationStory::new();
    assert_eq!(story.visible(), 0);
    assert_eq!(story.queued(), 0);
    story.clear();
    assert_eq!(story.caption(), "Visible: 0 · Queued: 0");
    assert_eq!(MAX_VISIBLE_TOASTS, 3);
}

#[test]
fn the_notification_counter_advances() {
    let story = NotificationStory::new();
    assert_eq!(story.posted(), 0);
    assert_eq!(story.next_count(), 1);
    story.bump();
    assert_eq!(story.posted(), 1);
    assert_eq!(story.next_count(), 2);
}

#[test]
fn all_five_product_stories_are_named() {
    let stories = product_stories();
    assert_eq!(stories.len(), 5);
    for story in stories {
        assert!(!story.debug_name().is_empty());
    }
    assert_eq!(ProductStory::Radio.debug_name(), "product-radio");
}
