//! Translation of `Tests/CoveUIComposeTests/ProductComponentTests.swift`.
//!
//! This suite is large and mixes model contracts with mounted-view interaction.
//! The cases ported here are the ones whose behaviour the Rust port expresses
//! through public model APIs; the rest are recorded below as blocked on gaps that
//! also affect other suites.
//!
//! Behaviour checklist:
//! - `TimeOfDay::parse` rejects invalid components and serialisation defaults
//!   fill missing fields
//! - pagination bounds huge values and exposes deterministic gap positions
//!
//! Recorded differences (reported, not fixed):
//! - `Accordion::toggle` needs a `Binding<BTreeSet<Id>>`; the Swift case also
//!   toggles a disabled entry and asserts `isEnabled` suppresses the change. The
//!   Rust `AccordionItem` has no `is_enabled` field, so that half is unavailable.
//! - The toast, notification-host, combobox-filter, table, calendar, radio and
//!   alert cases each drive runtime seams (an injectable clock, portal lifetime
//!   bookkeeping, `Binding`-driven presentation) that the `Binding` gap already
//!   blocks; they are recorded rather than restated.

mod support;

use cove_ui_compose::primitives::controls::date_values::TimeOfDay;
use cove_ui_compose::primitives::layout::navigation_components::{PaginationItem, PaginationModel};

#[test]
fn time_values() {
    assert_eq!(
        TimeOfDay::parse("23:59", false),
        Some(TimeOfDay::new(23, 59, 0)),
        "a well-formed time parses"
    );
    assert_eq!(
        TimeOfDay::parse("24:00", false),
        None,
        "hour 24 is rejected"
    );
    assert_eq!(
        TimeOfDay::parse("09:99", false),
        None,
        "minute 99 is rejected"
    );
    assert_eq!(
        TimeOfDay::parse("09:30:45", true).map(|value| value.second),
        Some(45),
        "seconds are read when requested"
    );
    assert_eq!(
        TimeOfDay::parse("09:30:", true),
        None,
        "an empty seconds field is rejected"
    );

    let value = TimeOfDay::new(17, 2, 11);
    let encoded = serde_json::to_string(&value).expect("a time round-trips through JSON");
    let decoded: TimeOfDay = serde_json::from_str(&encoded).expect("the encoding decodes");
    assert_eq!(decoded, value, "JSON round-trips the authored time");

    let defaulted: TimeOfDay = serde_json::from_str("{}").expect("missing fields default");
    assert_eq!(
        defaulted,
        TimeOfDay::default(),
        "an empty object decodes to the default time"
    );
}

#[test]
fn pagination_bounds() {
    assert!(
        PaginationModel::items(1, 0, 1).is_empty(),
        "a pager with no pages shows nothing"
    );
    assert_eq!(
        PaginationModel::normalized(0, 3),
        1,
        "a current page below one clamps to the first page"
    );
    assert_eq!(
        PaginationModel::items(8, 25, 1),
        vec![
            PaginationItem::Page(1),
            PaginationItem::LeadingGap,
            PaginationItem::Page(7),
            PaginationItem::Page(8),
            PaginationItem::Page(9),
            PaginationItem::TrailingGap,
            PaginationItem::Page(25),
        ],
        "a wide pager collapses the skipped ranges into gaps"
    );
    assert!(
        PaginationModel::items(usize::MAX, usize::MAX, 1).len() < 10,
        "an enormous total still renders a bounded pager"
    );
    assert_eq!(
        PaginationModel::items(2, 5, 1),
        (1..=5).map(PaginationItem::Page).collect::<Vec<_>>(),
        "a short pager lists every page"
    );
}
