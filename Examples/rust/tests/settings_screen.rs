//! Behaviour tests for the mobile settings screen.
//!
//! Translation of `Examples/Mobile/Sources/AppContent/Settings.swift`.

use cove_examples::mobile::model::MobileModel;
use cove_examples::mobile::settings::{APPEARANCE_CHOICES, SettingsRow, SettingsScreen, sections};

fn screen() -> SettingsScreen {
    SettingsScreen::new(MobileModel::new())
}

#[test]
fn the_screen_lists_three_sections_in_authored_order() {
    let sections = sections();
    assert_eq!(sections.len(), 3);
    assert_eq!(sections[0].title.as_deref(), Some("General"));
    assert_eq!(sections[1].title.as_deref(), Some("Accessibility"));
    assert_eq!(sections[2].title, None, "the sign-out group is untitled");
}

#[test]
fn general_holds_two_toggles_and_the_appearance_row() {
    let rows = &sections()[0].rows;
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].label(), "Notifications");
    assert_eq!(rows[1].label(), "Sync over cellular");
    assert_eq!(rows[2].label(), "Appearance");
    assert!(rows[2].shows_disclosure(), "appearance opens a sheet");
    assert!(!rows[0].shows_disclosure());
}

#[test]
fn sign_out_is_the_only_destructive_row() {
    let rows = &sections()[2].rows;
    assert_eq!(rows.len(), 1);
    assert!(rows[0].is_destructive());
    let total_destructive: usize = sections()
        .iter()
        .flat_map(|section| section.rows.iter())
        .filter(|row| row.is_destructive())
        .count();
    assert_eq!(total_destructive, 1);
}

#[test]
fn toggles_write_through_to_the_model() {
    let screen = screen();
    screen.set_notifications(true);
    screen.set_sync_over_cellular(true);
    screen.set_reduce_motion(true);
    let model = screen.model();
    assert!(model.notifications);
    assert!(model.sync_over_cellular);
    assert!(model.reduce_motion);
}

#[test]
fn the_appearance_sheet_starts_closed() {
    let screen = screen();
    assert!(!screen.is_choosing_appearance());
    screen.open_appearance_sheet();
    assert!(screen.is_choosing_appearance());
}

#[test]
fn choosing_an_appearance_records_it_and_closes_the_sheet() {
    let screen = screen();
    screen.open_appearance_sheet();
    assert!(screen.choose_appearance("Dark"));
    assert_eq!(screen.model().appearance, "Dark");
    assert!(
        !screen.is_choosing_appearance(),
        "choosing closes the sheet"
    );
}

#[test]
fn every_offered_choice_is_accepted() {
    let screen = screen();
    for choice in APPEARANCE_CHOICES {
        assert!(
            screen.choose_appearance(choice),
            "{choice} should be accepted"
        );
        assert_eq!(screen.model().appearance, choice);
    }
}

#[test]
fn an_unoffered_appearance_is_rejected() {
    let screen = screen();
    let before = screen.model().appearance;
    assert!(
        !screen.choose_appearance("Sepia"),
        "only the three choices exist"
    );
    assert_eq!(screen.model().appearance, before, "the model is untouched");
}

#[test]
fn the_sheet_offers_system_light_and_dark() {
    assert_eq!(APPEARANCE_CHOICES, ["System", "Light", "Dark"]);
}
