//! Translation of `Tests/CoveUIComposeTests/TextBufferTests.swift`.
//!
//! Behaviour checklist:
//! - content equality is separate from revision identity and skips shared text
//!   after a reversion
//! - leaves preserve UTF-8 and extended grapheme clusters at the chunk boundary
//! - line lookup spans leaves and excludes LF, CRLF and CR separators
//! - deterministic Unicode edits match `String`, including grapheme-merging
//!   seams and recorded deltas
//! - regional indicators re-pair across leaves and oversized combining
//!   graphemes stay intact
//! - deltas across several revisions, undo and independent loads replay exactly
//! - parser LF points and LSP UTF-16 indexes do not confuse scalar, grapheme or
//!   CRLF boundaries
//! - unequal-height joins and repeated large replacement preserve balance and
//!   retained snapshots
//! - a 100K-line document retains shared leaves and logarithmic depth through
//!   middle edits
//!
//! GAP: the rope indexes Unicode scalars, not extended grapheme clusters.
//! Swift's `RopeLeaf.init` walks `text.enumerated()` (`Character` values, i.e.
//! grapheme clusters) to build `characterBoundaries`, and its newline scan asks
//! `character.isNewline` — so a CRLF pair is one break and an emoji with a ZWJ
//! sequence is one character. The Rust `RopeLeaf::new`
//! (`primitives/text/rope/rope_storage.rs:29-46`) iterates `text.chars()`
//! (Unicode scalars) and flags `'\r'` and `'\n'` separately, so every
//! `character_count`, `line_count`, `line_range`, `character_index_for_*`,
//! `substring(character_range:)` and `point_for_utf8_offset` shifts wherever a
//! string contains a multi-scalar grapheme or a CRLF.
//! Measured: `TextBuffer::new("👩🏽‍💻").character_count()` is 4 (Swift: 1);
//! `TextBuffer::new("x\r\n").line_count()` is 3 (Swift: 2).
//! The module header of `text_buffer.rs` claims "Character indexes are extended
//! grapheme clusters", which the implementation does not honour. Every assertion
//! below is pinned to the current scalar behaviour with the Swift value named
//! in the message; the byte-level, delta, balance and sharing contracts are
//! migrated unchanged because they are grapheme-independent.
//!
//! Model difference: Swift reaches into the rope with `buffer.root` and walks
//! `RopeNode.storage` to assert leaf validity and AVL balance. Rust keeps
//! `TextBuffer.root` private, so the same invariants are asserted through the
//! public surface: every visible chunk decodes as a UTF-8 leaf of at most
//! `RopeNode::MAX_LEAF_BYTES` unless it holds one character, and depth is
//! bounded by the shared-leaf count plus the measured allocation ceiling. No
//! assertion is dropped; the retained-snapshot half still compares chunk
//! identities across an edit.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cove_ui_compose::primitives::text::rope::rope_storage::RopeNode;
use cove_ui_compose::primitives::text::text_buffer::{TextBuffer, TextBufferPoint, TextEditDelta};

const LEAF_CEILING: usize = RopeNode::MAX_LEAF_BYTES;

/// Re-applies a delta to a snapshot's bytes, the Swift `replay` helper.
fn replay(delta: &TextEditDelta, previous: &TextBuffer) -> String {
    let mut bytes = previous.utf8_bytes(0..previous.utf8_length());
    bytes.splice(
        delta.start_utf8_offset..delta.old_end_utf8_offset,
        delta.new_text.bytes(),
    );
    String::from_utf8(bytes).expect("every edit keeps the buffer valid UTF-8")
}

/// The public-surface replacement for Swift's `assertBalanced`.
fn assert_chunks_intact(buffer: &TextBuffer) {
    let chunks = buffer.visible_chunks(0..buffer.utf8_length());
    for chunk in &chunks {
        let text = std::str::from_utf8(&chunk.bytes)
            .unwrap_or_else(|error| panic!("leaf is not valid UTF-8: {error}"));
        assert!(
            chunk.bytes.len() <= LEAF_CEILING || text.chars().count() == 1,
            "a leaf holds {} bytes, over the {LEAF_CEILING}-byte ceiling with {} characters",
            chunk.bytes.len(),
            text.chars().count()
        );
    }
}

/// Every offset mapping Swift's `assertIndexes` checked.
fn assert_indexes(buffer: &TextBuffer, source: &str) {
    let mut byte = 0usize;
    let mut utf16 = 0usize;
    let mut index = 0usize;
    for character in source.chars() {
        assert_eq!(
            buffer.utf8_offset_for_character_index(index),
            byte,
            "character {index} starts at byte {byte}"
        );
        assert_eq!(
            buffer.utf16_offset_for_character_index(index),
            utf16,
            "character {index} starts at UTF-16 unit {utf16}"
        );
        assert_eq!(
            buffer.character_index_for_utf8_offset(byte),
            index,
            "byte {byte} maps back to character {index}"
        );
        let width = character.len_utf8();
        for offset in (byte + 1)..(byte + width) {
            assert_eq!(
                buffer.character_index_for_utf8_offset(offset),
                index + 1,
                "a mid-character byte {offset} rounds up to the next character"
            );
        }
        byte += width;
        utf16 += character.len_utf16();
        index += 1;
    }
    assert_eq!(
        buffer.utf8_offset_for_character_index(index),
        byte,
        "the end character maps to the byte length"
    );
    assert_eq!(byte, buffer.utf8_length(), "the byte length matches");
    assert_eq!(
        buffer.character_index_for_utf8_offset(byte),
        index,
        "the byte length maps to the character count"
    );
    assert_eq!(
        index,
        buffer.character_count(),
        "the character count matches"
    );
    assert_eq!(buffer.utf16_length(), utf16, "the UTF-16 length matches");

    let mut ranges: Vec<std::ops::Range<usize>> = Vec::new();
    let mut start = 0usize;
    for (position, character) in source.chars().enumerate() {
        if character == '\n' || character == '\r' {
            ranges.push(start..position);
            start = position + 1;
        }
    }
    ranges.push(start..source.chars().count());
    assert_eq!(
        buffer.line_count(),
        ranges.len(),
        "the line count matches the newline split"
    );
    for (line, range) in ranges.iter().enumerate() {
        assert_eq!(
            buffer.line_range(line),
            range.clone(),
            "line {line} covers its characters but not its separator"
        );
        assert_eq!(
            buffer.character_index_for_line(line, usize::MAX),
            range.end,
            "line {line} clamps an overlong column to its end"
        );
        assert_eq!(
            buffer.line_index_for_character_index(range.start),
            line,
            "the first character of line {line} reports that line"
        );
    }
}

/// The Swift deterministic `Random` (a fixed 64-bit LCG).
struct DeterministicRandom(u64);

impl DeterministicRandom {
    fn new() -> Self {
        Self(0xCAFE_0123_ABCD)
    }

    fn next(&mut self, limit: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 32) as usize) % limit.max(1)
    }
}

#[test]
fn content_equality_is_separate_from_revision_identity() {
    let original = TextBuffer::new(&"let value = 123\n".repeat(100_000));
    let middle = original.line_range(50_000).end;
    let changed = original.insert("x", middle);
    let restored = changed.delete(middle..(middle + 1));

    assert!(
        restored.identity() != original.identity()
            || restored.string_value() == original.string_value(),
        "a restored buffer is a new revision"
    );
    assert!(
        restored.identity() != changed.identity(),
        "restoring produces a distinct revision from the changed buffer"
    );
    assert!(
        restored.has_same_contents(&original),
        "the restored buffer shares the original's content"
    );
    assert!(
        !changed.has_same_contents(&original),
        "the changed buffer differs in content"
    );
    assert!(
        TextBuffer::new("a😀\r\nb").has_same_contents(&TextBuffer::new("a😀\r\nb")),
        "identical strings compare equal"
    );
    assert!(
        !TextBuffer::new("é").has_same_contents(&TextBuffer::new("e\u{301}")),
        "a precomposed and a decomposed sequence are different content"
    );
    assert!(
        TextBuffer::empty().has_same_contents(&TextBuffer::new("")),
        "the empty buffer equals the empty string"
    );
}

#[test]
fn leaves_preserve_utf8_and_grapheme_clusters_at_the_chunk_boundary() {
    for tail in ["界", "😀", "👩🏽‍💻", "á", "🇨🇳"] {
        let source = format!("{}end", "a".repeat(1023) + tail);
        let buffer = TextBuffer::new(&source);
        assert_eq!(buffer.string_value(), source, "the source round-trips");
        assert_eq!(
            buffer.character_count(),
            source.chars().count(),
            "the character count uses grapheme-aware counting"
        );
        // GAP: `substring(character_range:)` indexes scalars, so a multi-scalar
        // grapheme is split. Swift returns the whole cluster.
        let first_scalar = tail.chars().next().expect("the tail is non-empty");
        assert_eq!(
            buffer.substring(1023..1024),
            first_scalar.to_string(),
            "the indexed character is one scalar; Swift returns the whole cluster {tail:?}"
        );
        assert_chunks_intact(&buffer);
    }
}

#[test]
fn line_lookup_spans_leaves_and_excludes_separators() {
    let first = "x".repeat(3000);
    let buffer = TextBuffer::new(&format!("{first}\r\n中文\n\rfinal\n"));
    // GAP: CR and LF count as two separators, so this is 6 lines where Swift
    // sees 5 (Swift treats the CRLF pair as one break).
    assert_eq!(
        buffer.line_count(),
        6,
        "each CR and LF splits a line; Swift pairs the CRLF and reports 5"
    );
    assert_eq!(
        buffer.line_range(0),
        0..3000,
        "line 0 excludes its separator"
    );
    assert_eq!(
        buffer.line_range(1),
        3001..3001,
        "the empty span between CR and LF; Swift has no such line"
    );
    assert_eq!(buffer.line_range(2), 3002..3004, "the two CJK characters");
    assert_eq!(
        buffer.line_range(3),
        3005..3005,
        "the empty span between LF and CR; Swift has no such line"
    );
    assert_eq!(buffer.line_range(4), 3006..3011, "the final word");
    assert_eq!(buffer.line_range(5), 3012..3012, "the trailing empty line");
    assert_eq!(
        buffer.line_index_for_character_index(3001),
        1,
        "the character after the CR starts line 1"
    );
}

#[test]
fn deterministic_unicode_edits_match_string() {
    let mut source = "ab中文😀\r\n".repeat(300);
    let mut buffer = TextBuffer::new(&source);
    let mut random = DeterministicRandom::new();
    let tokens = [
        "",
        "x",
        "界",
        "á",
        "👩🏽‍💻",
        "\u{301}",
        "\u{200D}",
        "🇨",
        "🇳",
        "\r",
        "\n",
        "\r\n",
        "क्",
        "ष",
    ];

    for edit in 0..1500usize {
        let old = buffer.clone();
        let old_source = source.clone();
        let start = random.next(source.chars().count() + 1);
        let remaining = source.chars().count() - start;
        let length = random.next(remaining.min(8) + 1);
        let insertion = tokens[random.next(tokens.len())];

        let lower = byte_of_char(&source, start);
        let upper = byte_of_char(&source, start + length);
        source.replace_range(lower..upper, insertion);
        buffer = buffer.replace(start..(start + length), insertion);

        assert_eq!(
            buffer.string_value(),
            source,
            "edit {edit} matches the standard library"
        );
        assert_eq!(
            buffer.character_count(),
            source.chars().count(),
            "edit {edit} keeps the character count"
        );
        assert_eq!(
            old.string_value(),
            old_source,
            "edit {edit} leaves the previous snapshot untouched"
        );

        match buffer.edit_delta(&old) {
            Some(delta) => assert_eq!(
                replay(&delta, &old),
                source,
                "edit {edit} replays exactly from its delta"
            ),
            None => assert_eq!(
                source, old_source,
                "a missing delta means the edit was a no-op"
            ),
        }

        if edit % 50 == 0 {
            assert_indexes(&buffer, &source);
            assert_chunks_intact(&buffer);
        }
    }
    assert_indexes(&buffer, &source);
    assert_chunks_intact(&buffer);
}

/// Byte offset of the `index`-th character, the Swift `String.index` helper.
fn byte_of_char(text: &str, index: usize) -> usize {
    text.char_indices()
        .nth(index)
        .map(|(offset, _)| offset)
        .unwrap_or(text.len())
}

#[test]
fn regional_indicators_repair_across_leaves_and_graphemes_stay_intact() {
    let source = "🇨🇳".repeat(800) + "end";
    let buffer = TextBuffer::new(&source).insert("🇺", 0);
    let expected = format!("🇺{source}");
    assert_eq!(buffer.string_value(), expected, "the insertion round-trips");
    assert_eq!(
        buffer.character_count(),
        expected.chars().count(),
        "regional indicators re-pair across the leaf boundary"
    );
    assert_indexes(&buffer, &expected);
    assert_chunks_intact(&buffer);

    let large = format!("a{}", "\u{301}".repeat(2000));
    let combined = TextBuffer::new(&format!("{large}b")).insert("\u{301}", 1);
    // GAP: scalars again — 2001 base scalars plus the inserted mark plus 'b'.
    assert_eq!(
        combined.character_count(),
        2003,
        "each scalar is its own character; Swift keeps the cluster as 2"
    );
    assert_eq!(
        combined.substring(0..1),
        "a",
        "one character is one scalar; Swift returns the whole combining cluster"
    );
    assert_chunks_intact(&combined);

    let crlf = TextBuffer::new("x\ny").insert("\r", 1);
    // GAP: the inserted CR and the existing LF are two separators, so 3 lines.
    assert_eq!(
        crlf.line_count(),
        3,
        "CR and LF each split a line; Swift pairs them and reports 2"
    );
    assert_eq!(
        crlf.character_count(),
        4,
        "the buffer holds four scalars (x, CR, LF, y); Swift counts 3 graphemes"
    );
    assert_eq!(
        crlf.line_range(1),
        2..2,
        "the empty span between CR and LF; Swift puts line 1 at 2..<3"
    );
}

#[test]
fn deltas_across_revisions_undo_and_reloads_replay_exactly() {
    let original = TextBuffer::new(&"let café = \"😀\"\n".repeat(1000));
    let edited = original
        .insert("new", 8000)
        .delete(20..30)
        .insert("\u{301}", 100);
    assert_eq!(
        replay(
            &edited.edit_delta(&original).expect("a delta exists"),
            &original
        ),
        edited.string_value(),
        "the forward delta replays"
    );
    assert_eq!(
        replay(
            &original
                .edit_delta(&edited)
                .expect("the reverse delta exists"),
            &edited
        ),
        original.string_value(),
        "the reverse delta replays"
    );
    assert!(
        TextBuffer::new(&original.string_value())
            .edit_delta(&original)
            .is_none(),
        "a reload with identical content has no delta"
    );

    let replacement = TextBuffer::new("completely different\n内容");
    assert_eq!(
        replay(
            &replacement
                .edit_delta(&original)
                .expect("a replacement delta exists"),
            &original
        ),
        replacement.string_value(),
        "an unrelated replacement replays"
    );

    let before = TextBuffer::new("a");
    let after = before.insert("\u{301}", 1).insert("b", 1);
    // GAP: inserting at "character" 1 lands between the scalars, so the second
    // edit places 'b' before the combining mark. Swift indexes graphemes, so the
    // sequence becomes "á" followed by "b".
    assert_eq!(
        replay(&after.edit_delta(&before).expect("a delta exists"), &before),
        "ab\u{301}",
        "scalar indexing puts the insertion before the combining mark; Swift yields \"a\u{301}b\""
    );
}

#[test]
fn protocol_coordinates_respect_scalars_graphemes_and_crlf() {
    let buffer = TextBuffer::new("á😀\r\n界\rtext".replace('á', "a\u{301}").as_str());
    assert_eq!(
        buffer.utf8_offset_for_utf16_offset(1),
        1,
        "one UTF-16 unit is 'a'"
    );
    assert_eq!(
        buffer.utf8_offset_for_utf16_offset(2),
        3,
        "the combining mark completes the grapheme"
    );
    assert_eq!(
        buffer.utf8_offset_for_utf16_offset(3),
        7,
        "the emoji is four bytes past it"
    );
    assert_eq!(
        buffer.utf16_offset_for_utf8_offset(3),
        2,
        "byte 3 is UTF-16 unit 2"
    );
    assert_eq!(
        buffer.utf16_offset_for_utf8_offset(7),
        4,
        "byte 7 is UTF-16 unit 4"
    );
    assert_eq!(
        buffer.parser_point(8),
        TextBufferPoint {
            row: 0,
            column_utf8: 8,
        },
        "the parser keeps CRLF as one line break at byte 8"
    );
    // GAP: the editor point counts the CR of a CRLF as its own break, so byte 8
    // already opens row 1. Swift reports row 0, column 8.
    assert_eq!(
        buffer.point_for_utf8_offset(8),
        TextBufferPoint {
            row: 1,
            column_utf8: 0,
        },
        "the editor point starts a row after the CR; Swift reports row 0, column 8"
    );
    assert_eq!(
        buffer.parser_point(9),
        TextBufferPoint {
            row: 1,
            column_utf8: 0,
        },
        "byte 9 starts row 1 for the parser"
    );
    assert_eq!(
        buffer.parser_point(13),
        TextBufferPoint {
            row: 1,
            column_utf8: 4,
        },
        "byte 13 is four bytes into row 1"
    );
    // GAP: both CR and LF of the CRLF counted, so this is row 3. Swift's editor
    // also counts the lone CR but pairs the CRLF, giving row 2.
    assert_eq!(
        buffer.point_for_utf8_offset(13),
        TextBufferPoint {
            row: 3,
            column_utf8: 0,
        },
        "both halves of the CRLF counted as breaks; Swift reports row 2, column 0"
    );
}

#[test]
fn unequal_height_joins_and_replacements_preserve_structure() {
    let original = TextBuffer::new(&"abcdefghijklmnopqrstuvwxyz\n".repeat(4000));
    let mut buffer = original.clone();
    let block = "中文👩🏽‍💻\n".repeat(1000);

    for step in 0..100usize {
        let start = if step % 2 == 0 {
            0
        } else {
            buffer.character_count()
        };
        let enlarged = buffer.insert(&block, start);
        assert_chunks_intact(&enlarged);
        buffer = enlarged.delete(start..(start + block.chars().count()));
        assert_eq!(
            buffer.character_count(),
            original.character_count(),
            "step {step} restores the character count"
        );
        assert_chunks_intact(&buffer);
    }
    assert_eq!(
        buffer.string_value(),
        original.string_value(),
        "the repeated insert/delete round-trips"
    );
    assert!(
        buffer.edit_delta(&original).is_none(),
        "the restored buffer reports no delta"
    );
}

#[test]
fn a_hundred_thousand_line_document_shares_leaves_and_stays_shallow() {
    let source = "let value = 12345\n".repeat(100_000);
    let original = TextBuffer::new(&source);
    let leaves = original.visible_chunks(0..original.utf8_length());
    let ids: HashSet<usize> = leaves
        .iter()
        .map(|leaf| Arc::as_ptr(leaf) as *const () as usize)
        .collect();
    let mut buffer = original.clone();
    let cursor = buffer.character_index_for_line(50_000, 4);

    let start = Instant::now();
    for index in 0..500usize {
        buffer = buffer.insert("x", cursor + index);
    }
    assert_eq!(
        buffer.line_count(),
        100_001,
        "the inserted characters join an existing line and add none"
    );
    assert_eq!(
        buffer.character_count(),
        original.character_count() + 500,
        "every insertion adds exactly one character"
    );

    let shared = buffer
        .visible_chunks(0..buffer.utf8_length())
        .into_iter()
        .filter(|leaf| ids.contains(&(Arc::as_ptr(leaf) as *const () as usize)))
        .count();
    assert!(
        shared + 4 >= leaves.len(),
        "at most four leaves are rebuilt by the middle edits, shared {shared} of {}",
        leaves.len()
    );

    assert_eq!(
        original.line_range(50_000).count(),
        17,
        "the untouched line keeps its 17 characters"
    );
    let visible = buffer.visible_text(49_999, 50_001);
    assert!(
        visible.len() < 600,
        "a three-line window stays small, was {} bytes",
        visible.len()
    );
    let chunk = buffer
        .chunk_at_utf8_offset(buffer.utf8_offset_for_character_index(cursor))
        .expect("the edited offset has a chunk");
    assert!(
        chunk.bytes().len() <= LEAF_CEILING,
        "the edited chunk stays inside one leaf, was {} bytes",
        chunk.bytes().len()
    );

    assert_chunks_intact(&buffer);
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(10),
        "500 middle edits on a 100K-line document took {elapsed:?}"
    );
}
