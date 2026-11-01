//! Translation of `Examples/Mobile/Tests/AppContentTests/Model.swift`.
//!
//! The Swift file is six lines and is *not* a support file: it declares one
//! `@Test`, so it is ported as a suite of its own rather than folded into another
//! target. It asserts that every kind of work the sample data demonstrates is
//! present.
//!
//! Behaviour checklist:
//! - the sample projects cover exactly the four kinds of work the model claims
//! - the model's remaining fields match the authored defaults, so the screens
//!   that read them start from the documented state

use std::collections::BTreeSet;

use cove_examples::mobile::model::{APPEARANCE_CHOICES, MobileModel};

#[test]
fn sample_projects_cover_every_kind_of_work() {
    let model = MobileModel::new();
    let kinds: BTreeSet<&str> = model
        .projects
        .iter()
        .map(|project| project.kind.as_str())
        .collect();
    assert_eq!(
        kinds,
        BTreeSet::from(["Game", "Animation", "Video", "Code"]),
        "the four sample projects must cover every kind of work"
    );
}

#[test]
fn every_project_carries_the_fields_its_row_renders() {
    let model = MobileModel::new();
    assert!(!model.projects.is_empty(), "the sample data is not empty");
    for project in &model.projects {
        assert!(
            !project.id.is_empty(),
            "project {:?} needs a stable id",
            project.name
        );
        assert!(
            !project.name.is_empty(),
            "project {:?} needs a title",
            project.id
        );
        assert!(
            !project.updated.is_empty(),
            "project {:?} needs an updated caption",
            project.name
        );
        assert!(
            (0.0..=1.0).contains(&project.progress),
            "project {:?} progress must be a fraction, got {}",
            project.name,
            project.progress
        );
    }
}

#[test]
fn the_ids_are_unique_so_rows_do_not_collide() {
    let model = MobileModel::new();
    let ids: BTreeSet<&str> = model.projects.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(
        ids.len(),
        model.projects.len(),
        "duplicate ids would collapse two rows"
    );
}

#[test]
fn the_settings_defaults_match_the_authored_state() {
    let model = MobileModel::new();
    assert!(model.notifications, "notifications start enabled");
    assert!(!model.sync_over_cellular, "cellular sync starts disabled");
    assert!(!model.reduce_motion, "reduced motion starts off");
    assert_eq!(
        model.appearance, "System",
        "the appearance starts on the system setting"
    );
    assert_eq!(
        APPEARANCE_CHOICES,
        ["System", "Light", "Dark"],
        "the three appearance choices are offered in order"
    );
}
