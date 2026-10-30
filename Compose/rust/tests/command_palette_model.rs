//! Behaviour tests for command-palette ranking and projection.
//!
//! Translation of `Primitives/Overlays/Command/Search.swift` and the search half
//! of `CommandPalette.swift`. Ranking precedence and arrow wrapping are exactly
//! the kind of arithmetic that drifts silently.

use cove_ui_compose::primitives::overlays::{
    destination, display_entries, entries, normalize, ranked, record_recent, CommandEntry,
    CommandItem,
};

fn items() -> Vec<CommandItem> {
    vec![
        CommandItem::new("open", "Open File", "File"),
        CommandItem::new("save", "Save File", "File"),
        CommandItem::new("close", "Close Window", "Window"),
        CommandItem::new("quit", "Quit Cove", "Application"),
    ]
}

fn ids(list: &[CommandItem]) -> Vec<&str> {
    list.iter().map(|item| item.id.as_str()).collect()
}

#[test]
fn folding_ignores_case_accents_and_width() {
    assert_eq!(normalize("Café"), "cafe");
    assert_eq!(normalize("CAFE"), "cafe");
    // Fullwidth Latin folds to halfwidth.
    assert_eq!(normalize("ＯＰＥＮ"), "open");
}

#[test]
fn an_empty_query_returns_everything() {
    assert_eq!(ranked(&items(), "").len(), 4);
    assert_eq!(ranked(&items(), "   ").len(), 4, "whitespace is not a term");
}

#[test]
fn a_title_prefix_outranks_containment() {
    let list = vec![
        CommandItem::new("a", "Reopen", "G"),
        CommandItem::new("b", "Open File", "G"),
    ];
    // "open" prefixes title b and is contained in title a.
    assert_eq!(ids(&ranked(&list, "open")), vec!["b", "a"]);
}

#[test]
fn a_word_prefix_outranks_containment() {
    let list = vec![
        CommandItem::new("a", "Close Open", "G"),   // containment
        CommandItem::new("b", "Open Window", "G"),  // word prefix
    ];
    assert_eq!(ids(&ranked(&list, "open")), vec!["b", "a"]);
}

#[test]
fn keywords_match_but_score_lower_than_the_title() {
    let list = vec![
        CommandItem::new("kw", "Toggle Sidebar", "G").with_keywords(["panel"]),
        CommandItem::new("title", "Panel Layout", "G"),
    ];
    let ordered = ranked(&list, "panel");
    assert_eq!(ids(&ordered), vec!["title", "kw"], "title hit outranks a keyword hit");
}

#[test]
fn a_subsequence_matches() {
    // "sf" is a subsequence of "Save File".
    let list = vec![CommandItem::new("s", "Save File", "G")];
    assert_eq!(ids(&ranked(&list, "sf")), vec!["s"]);
}

#[test]
fn a_group_or_subtitle_hit_still_matches() {
    let list = vec![CommandItem::new("q", "Quit", "Application")];
    assert_eq!(ids(&ranked(&list, "application")), vec!["q"]);
}

#[test]
fn a_term_that_matches_nothing_rejects_the_item() {
    let list = vec![CommandItem::new("a", "Open File", "G")];
    assert!(ranked(&list, "zzzz").is_empty());
}

#[test]
fn every_term_must_match() {
    let list = vec![CommandItem::new("a", "Open File", "G")];
    assert_eq!(ids(&ranked(&list, "open file")), vec!["a"]);
    assert!(ranked(&list, "open nothing").is_empty(), "all terms are required");
}

#[test]
fn enabled_items_outrank_disabled_ones() {
    let list = vec![
        CommandItem::new("off", "Open Disabled", "G").disabled(),
        CommandItem::new("on", "Open Enabled", "G"),
    ];
    assert_eq!(ids(&ranked(&list, "open")), vec!["on", "off"]);
}

#[test]
fn ties_keep_source_order() {
    let list = vec![
        CommandItem::new("first", "Open A", "G"),
        CommandItem::new("second", "Open B", "G"),
    ];
    assert_eq!(ids(&ranked(&list, "open")), vec!["first", "second"]);
}

#[test]
fn results_group_only_when_asked_and_only_with_multiple_groups() {
    let list = items();
    // Grouping off: no headers.
    let flat = entries(&list, false);
    assert!(flat.iter().all(|row| matches!(row, CommandEntry::Item(_))));

    // Grouping on with several groups: headers appear in first-appearance order.
    let grouped = entries(&list, true);
    let headers: Vec<&str> = grouped
        .iter()
        .filter_map(|row| match row {
            CommandEntry::Header(title) => Some(title.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(headers, vec!["File", "Window", "Application"]);

    // Grouping on but a single group: no header at all.
    let single = vec![CommandItem::new("a", "A", "Only"), CommandItem::new("b", "B", "Only")];
    assert!(entries(&single, true).iter().all(|row| matches!(row, CommandEntry::Item(_))));
}

#[test]
fn entry_ids_are_namespaced() {
    let header = CommandEntry::Header("Recent".into());
    let item = CommandEntry::Item(CommandItem::new("open", "Open", "G"));
    assert_eq!(header.id(), "header:Recent");
    assert_eq!(item.id(), "item:open");
}

#[test]
fn recents_lead_the_list_once_and_are_capped() {
    let list = items();
    let rows = display_entries(&list, &["save".into(), "open".into()], true, 5, true);
    match &rows[0] {
        CommandEntry::Header(title) => assert_eq!(title, "Recent"),
        _ => panic!("recents are headed"),
    }
    assert_eq!(rows[1].id(), "item:save");
    assert_eq!(rows[2].id(), "item:open");
    // The recents do not appear a second time in the body.
    let body: Vec<String> = rows[3..].iter().map(|row| row.id()).collect();
    assert!(!body.contains(&"item:save".to_owned()));
    assert!(!body.contains(&"item:open".to_owned()));
}

#[test]
fn recents_skip_disabled_items_and_respect_the_cap() {
    let list = vec![
        CommandItem::new("a", "A", "G"),
        CommandItem::new("b", "B", "G").disabled(),
    ];
    let rows = display_entries(&list, &["b".into(), "a".into()], true, 5, false);
    assert_eq!(rows[0].id(), "header:Recent");
    assert_eq!(rows[1].id(), "item:a", "the disabled recent is skipped");

    // A limit of one keeps only the first usable recent.
    let rows = display_entries(&list, &["a".into()], true, 1, false);
    assert_eq!(rows[1].id(), "item:a");
    assert_eq!(rows.len(), 3, "header + one recent + the remaining item");
}

#[test]
fn without_recents_the_list_is_just_the_items() {
    let rows = display_entries(&items(), &["save".into()], false, 5, false);
    assert!(rows.iter().all(|row| matches!(row, CommandEntry::Item(_))));
}

#[test]
fn arrow_navigation_wraps_at_both_ends() {
    // Forward past the end wraps to zero.
    assert_eq!(destination(4, Some(3), 1, true), Some(0));
    // Backward past the start wraps to the end.
    assert_eq!(destination(4, Some(0), -1, true), Some(3));
    // Within range moves by one.
    assert_eq!(destination(4, Some(1), 1, true), Some(2));
}

#[test]
fn page_jumps_clamp_instead_of_wrapping() {
    assert_eq!(destination(10, Some(8), 5, false), Some(9), "clamps at the end");
    assert_eq!(destination(10, Some(1), -5, false), Some(0), "clamps at the start");
    assert_eq!(destination(10, Some(2), 3, false), Some(5));
}

#[test]
fn a_missing_highlight_starts_from_the_edge_the_move_came_from() {
    assert_eq!(destination(4, None, 1, true), Some(0), "forward from nothing starts at the top");
    assert_eq!(destination(4, None, -1, true), Some(3), "backward starts at the bottom");
    // An out-of-range current is treated the same way.
    assert_eq!(destination(4, Some(9), 1, true), Some(0));
}

#[test]
fn an_empty_list_has_no_destination() {
    assert_eq!(destination(0, None, 1, true), None);
}

#[test]
fn a_huge_delta_does_not_overflow() {
    // Reducing the modulus before adding keeps this well-defined.
    assert_eq!(destination(4, Some(0), i64::MAX, true), Some(3));
    // `i64::MIN % 4 == 0`, so a full multiple of the count lands back on 0 —
    // the point is that it is defined at all rather than overflowing.
    assert_eq!(destination(4, Some(0), i64::MIN, true), Some(0));
}

#[test]
fn recording_a_recent_moves_it_to_the_front_and_caps_the_list() {
    let mut recents = vec!["a".to_owned(), "b".to_owned(), "c".to_owned()];
    record_recent(&mut recents, "c", 3);
    assert_eq!(recents, vec!["c", "a", "b"], "c moves to the front");

    record_recent(&mut recents, "d", 3);
    assert_eq!(recents, vec!["d", "c", "a"], "the oldest falls off");

    // A limit of zero records nothing.
    let mut none = Vec::new();
    record_recent(&mut none, "x", 0);
    assert!(none.is_empty());
}
