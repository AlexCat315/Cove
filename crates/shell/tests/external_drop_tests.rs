//! Platform drops decode into one contract per phase.
//!
//! SDL reports an external drag as `DROP_BEGIN`, one `DROP_FILE`/`DROP_TEXT` per
//! item, `DROP_POSITION` while it moves and `DROP_COMPLETE` at the end. The
//! bridge copies each payload into the event, and the shell maps the numbered
//! kind onto [`cove_platform::DropPhase`].
//!
//! Reachability: a real drop needs a drag started by another application, which
//! a headless test cannot produce. These cases assert the decoding seam the shell
//! applies (`external_drop`) rather than a synthesized SDL event, and native
//! drag entry remains a separate acceptance item.
//!
//! Behaviour checklist:
//! - the five kinds map to the five phases
//! - a file drop puts its path in `paths` and never in `text`
//! - a text drop puts its string in `text` and never in `paths`
//! - begin/position/complete carry no payload
//! - the position and the drag source survive decoding

use cove_platform::DropPhase;
use cove_shell::external_drop;

fn decode(kind: u32, data: &str) -> cove_platform::ExternalDropEvent {
    external_drop(kind, 12.5, 34.0, data.to_owned(), Some("Finder".into()))
}

#[test]
fn every_kind_maps_to_its_phase() {
    assert_eq!(decode(25, "").phase, DropPhase::Begin);
    assert_eq!(decode(26, "/tmp/a.png").phase, DropPhase::File);
    assert_eq!(decode(27, "hello").phase, DropPhase::Text);
    assert_eq!(decode(28, "").phase, DropPhase::Position);
    assert_eq!(decode(29, "").phase, DropPhase::Complete);
}

#[test]
fn a_file_drop_carries_a_path_and_no_text() {
    let event = decode(26, "/tmp/a.png");
    assert_eq!(event.paths, ["/tmp/a.png"]);
    assert_eq!(event.text, None);
}

#[test]
fn a_text_drop_carries_a_string_and_no_path() {
    let event = decode(27, "pasted text");
    assert!(event.paths.is_empty());
    assert_eq!(event.text.as_deref(), Some("pasted text"));
}

#[test]
fn the_lifecycle_phases_carry_no_payload() {
    for kind in [25, 28, 29] {
        let event = decode(kind, "");
        assert!(event.paths.is_empty(), "kind {kind} carries no path");
        assert_eq!(event.text, None, "kind {kind} carries no text");
    }
}

#[test]
fn the_position_and_source_survive_decoding() {
    let event = decode(28, "");
    assert_eq!((event.x, event.y), (12.5, 34.0));
    assert_eq!(event.source.as_deref(), Some("Finder"));
}

#[test]
fn an_unknown_source_is_absent_rather_than_empty() {
    let event = external_drop(26, 0.0, 0.0, "/tmp/a.png".into(), None);
    assert_eq!(event.source, None);
}
