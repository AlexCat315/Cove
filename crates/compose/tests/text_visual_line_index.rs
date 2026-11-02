//! Translation of `Tests/CoveUIComposeTests/TextVisualLineIndexTests.swift`.
//!
//! Behaviour checklist:
//! - wrapping and splicing keep every line/row mapping exact, and a snapshot
//!   taken before a change still reports its own totals
//! - the run tree stays balanced under thousands of edits (`height < 32`)
//! - a very large unwrapped document collapses to a single run

use cove_ui_compose::primitives::text::text_field::text_visual_line_index::TextVisualLineIndex;

#[test]
fn wrapping_and_splicing_preserve_positions_and_snapshots() {
    let mut index = TextVisualLineIndex::new(1_000);
    let mut expected: Vec<usize> = vec![1; 1_000];
    let mut seed: u64 = 321;

    let mut random = |maximum: usize| -> usize {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        ((seed >> 32) as usize) % maximum
    };

    for iteration in 0..2_000 {
        let old = index.clone();
        let old_total: usize = expected.iter().sum();
        let line = random(expected.len());

        if iteration % 3 == 0 {
            let end = expected.len().min(line + random(8) + 1);
            let count = random(8) + 1;
            index.replace(line..end, count);
            expected.splice(line..end, std::iter::repeat_n(1, count));
        } else {
            let count = random(20) + 1;
            index.measure(line, count);
            expected[line] = count;
        }

        assert_eq!(
            old.row_count(),
            old_total,
            "a snapshot keeps its own row total"
        );
        assert_eq!(index.line_count(), expected.len());
        assert_eq!(index.row_count(), expected.iter().sum::<usize>());
        assert!(index.height() < 32, "the run tree stays balanced");

        if iteration % 50 == 0 {
            let mut row = 0;
            for (line, count) in expected.iter().enumerate() {
                assert_eq!(
                    index.first_row_of_line(line),
                    row,
                    "line {line} starts at row {row}"
                );
                for offset in [0, count - 1] {
                    let (mapped_line, subrow) = index.location_at_row(row + offset);
                    assert_eq!(
                        (mapped_line, subrow),
                        (line, offset),
                        "row {} maps back to ({line}, {offset})",
                        row + offset
                    );
                }
                row += count;
            }
        }
    }
}

#[test]
fn large_unwrapped_document_remains_one_run() {
    let mut index = TextVisualLineIndex::new(200_000);
    for _ in 0..1_000 {
        index.replace(100_000..100_001, 1);
    }
    assert_eq!(index.height(), 1, "unwrapped lines share one run");
    assert_eq!(index.first_row_of_line(150_000), 150_000);
    assert_eq!(index.location_at_row(150_000).0, 150_000);
}
