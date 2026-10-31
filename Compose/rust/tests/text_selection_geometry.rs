//! Translation of `Tests/CoveUIComposeTests/TextSelectionGeometryTests.swift`.
//!
//! Behaviour checklist:
//! - word boundaries select the word under an offset
//! - `PointerKind` round-trips through JSON and events carry a source kind
//! - an event without an explicit kind defaults to mouse
//!
//! Model difference: the Swift suite also drives `TextLayoutResult.offset(at:)`,
//! `selectionRects(_:)` and `caretRect(at:)`. Those methods live on the rich
//! paragraph flow in the Rust image (`RichTextFlow::offset/caret_rect/
//! selection_rects`), which needs a real text environment and shaped glyphs, so
//! that half is covered by the flow's own geometry rather than a synthesized
//! glyph table.

mod support;

use cove_platform::{MouseButton, MouseButtonEvent, MouseMotionEvent, PointerKind};
use cove_ui_compose::primitives::text::rich::text_selection::TextBoundaries;

#[test]
fn word_boundary_selects_the_word_under_an_offset() {
    assert_eq!(
        TextBoundaries::word("hello world", 3),
        0..5,
        "an offset inside the first word selects it whole"
    );
    assert_eq!(
        TextBoundaries::word("hello world", 8),
        6..11,
        "an offset inside the second word selects that one"
    );
    // Swift falls back to "the single character under the offset" when no word
    // range contains it, so a space offset yields the space itself. The Rust
    // fallback agrees — it is the *word* lookup that differs from the
    // `enumerateSubstrings(.byWords)` behaviour Swift relies on.
    assert_eq!(
        TextBoundaries::word("hello world", 5),
        5..6,
        "a whitespace offset has no enclosing word"
    );
}

#[test]
fn word_boundary_treats_the_sentence_end_correctly() {
    assert_eq!(
        TextBoundaries::word("one two", 6),
        4..7,
        "the final character belongs to the trailing word"
    );
    assert_eq!(
        TextBoundaries::word("a", 0),
        0..1,
        "a single-character word selects itself"
    );
}

#[test]
fn pointer_kind_round_trips_through_json() {
    for kind in [PointerKind::Mouse, PointerKind::Touch, PointerKind::Pen] {
        let encoded = serde_json::to_string(&kind).expect("a pointer kind serializes");
        let decoded: PointerKind =
            serde_json::from_str(&encoded).expect("a pointer kind deserializes");
        assert_eq!(decoded, kind, "{kind:?} survives a round trip");
    }
}

#[test]
fn events_carry_their_source_kind() {
    let down = MouseButtonEvent {
        button: MouseButton::Left,
        x: 1.0,
        y: 2.0,
        clicks: 1,
        modifiers: Default::default(),
        kind: PointerKind::Touch,
    };
    assert_eq!(down.kind, PointerKind::Touch, "a touch press reports touch");

    let motion = MouseMotionEvent {
        x: 1.0,
        y: 2.0,
        delta_x: 0.0,
        delta_y: 0.0,
        kind: PointerKind::Pen,
    };
    assert_eq!(motion.kind, PointerKind::Pen, "a stylus move reports pen");
}

#[test]
fn an_event_without_a_kind_defaults_to_mouse() {
    let plain = MouseButtonEvent {
        button: MouseButton::Left,
        x: 0.0,
        y: 0.0,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    };
    assert_eq!(
        plain.kind,
        PointerKind::Mouse,
        "legacy callers that omit the source keep mouse behaviour"
    );

    let plain_motion = MouseMotionEvent {
        x: 0.0,
        y: 0.0,
        delta_x: 0.0,
        delta_y: 0.0,
        kind: Default::default(),
    };
    assert_eq!(plain_motion.kind, PointerKind::Mouse);
}

#[test]
fn pointer_kind_flags_are_distinct() {
    // The numeric discriminants are part of the wire format.
    assert_eq!(PointerKind::Mouse as u8, 1);
    assert_eq!(PointerKind::Touch as u8, 2);
    assert_eq!(PointerKind::Pen as u8, 3);
}
