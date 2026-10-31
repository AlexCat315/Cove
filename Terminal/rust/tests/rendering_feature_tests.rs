//! Translation of `Terminal/Tests/CoveUITerminalTests/RenderingFeatureTests.swift`.
//!
//! Behaviour checklist:
//! - box-drawing arms follow the Unicode names and `handles` covers its ranges
//! - lines reach every cell edge they name, so neighbours join
//! - plain-text URLs are found with their columns and without trailing punctuation
//! - Option-drag selects a rectangular block
//! - SGR 5 marks blinking text
//! - diacritic indices come from the Kitty table
//! - Unicode placeholders draw their strip of a virtual placement
//! - placeholder runs split on image changes and explicit columns

use cove_core::{Color, DrawList, Rect};
use cove_terminal::engine::{
    PLACEHOLDER_CODEPOINT, PlaceholderCell, PlaceholderRun, SelectionPhase, TerminalCellAttributes,
    TerminalEngine, TerminalPointer,
};
use cove_terminal::view::{Arms, box_drawing, detect_url};

fn engine(text: &str, columns: usize) -> TerminalEngine {
    let mut engine = TerminalEngine::new(columns, 4, 100);
    engine.resize(columns, 4, 10, 20);
    engine.feed(text.as_bytes());
    engine
}

#[test]
fn box_drawing_arms_follow_the_unicode_names() {
    assert_eq!(
        box_drawing::arms(0x2500),
        Some(Arms {
            up: 0,
            right: 1,
            down: 0,
            left: 1
        }),
        "U+2500 BOX DRAWINGS LIGHT HORIZONTAL"
    );
    assert_eq!(
        box_drawing::arms(0x253C),
        Some(Arms {
            up: 1,
            right: 1,
            down: 1,
            left: 1
        }),
        "U+253C BOX DRAWINGS LIGHT VERTICAL AND HORIZONTAL"
    );
    assert_eq!(
        box_drawing::arms(0x2543),
        Some(Arms {
            up: 2,
            right: 1,
            down: 1,
            left: 2
        }),
        "U+2543 BOX DRAWINGS LEFT UP HEAVY AND RIGHT DOWN LIGHT"
    );
    assert_eq!(
        box_drawing::arms(0x2554),
        Some(Arms {
            up: 0,
            right: 3,
            down: 3,
            left: 0
        }),
        "U+2554 BOX DRAWINGS DOUBLE DOWN AND RIGHT"
    );
    assert_eq!(
        box_drawing::arms(0x256A),
        Some(Arms {
            up: 1,
            right: 3,
            down: 1,
            left: 3
        }),
        "U+256A BOX DRAWINGS VERTICAL SINGLE AND HORIZONTAL DOUBLE"
    );
    assert_eq!(
        box_drawing::arms(0x257F),
        Some(Arms {
            up: 2,
            right: 0,
            down: 1,
            left: 0
        }),
        "U+257F BOX DRAWINGS HEAVY UP AND LIGHT DOWN"
    );
    assert_eq!(
        box_drawing::arms(0x2504),
        None,
        "a dashed line has no arm table entry; it is drawn separately"
    );
    assert!(
        box_drawing::handles(0x2588) && box_drawing::handles(0xE0B0),
        "block elements and Powerline separators are handled"
    );
    assert!(
        !box_drawing::handles(0x41),
        "ordinary ASCII text is left to the font"
    );
}

#[test]
fn lines_reach_every_cell_edge_they_name_so_neighbours_join() {
    let cell = Rect::new(0.0, 0.0, 10.0, 20.0);
    for scalar in [0x2500u32, 0x2502, 0x253C, 0x2550, 0x2551, 0x256C, 0x2588] {
        let mut list = DrawList::default();
        box_drawing::draw(scalar, cell, Color::WHITE, &mut list);

        let xs: Vec<f32> = list.vertices().iter().map(|vertex| vertex.x).collect();
        let ys: Vec<f32> = list.vertices().iter().map(|vertex| vertex.y).collect();
        assert!(
            !xs.is_empty(),
            "U+{scalar:04X} drew no geometry at all"
        );
        let arms = box_drawing::arms(scalar).unwrap_or(Arms {
            up: 1,
            right: 1,
            down: 1,
            left: 1,
        });
        let min_x = xs.iter().copied().fold(f32::INFINITY, f32::min);
        let max_x = xs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let min_y = ys.iter().copied().fold(f32::INFINITY, f32::min);
        let max_y = ys.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        if arms.left > 0 {
            assert!(min_x <= 0.0, "U+{scalar:04X} reaches the left edge");
        }
        if arms.right > 0 {
            assert!(max_x >= 10.0, "U+{scalar:04X} reaches the right edge");
        }
        if arms.up > 0 {
            assert!(min_y <= 0.0, "U+{scalar:04X} reaches the top edge");
        }
        if arms.down > 0 {
            assert!(max_y >= 20.0, "U+{scalar:04X} reaches the bottom edge");
        }
    }
}

#[test]
fn plain_text_urls_are_found_with_their_columns_and_without_trailing_punctuation() {
    let mut engine = engine("see https://example.com/a_(b), www.cove.dev. x", 60);
    let frame = engine.snapshot();

    let first = detect_url(&frame, 0, 10).expect("the bracketed URL is found");
    assert_eq!(
        first.url, "https://example.com/a_(b)",
        "the closing parenthesis that matches an opening one is kept"
    );
    assert_eq!(
        first.columns,
        (4, 28),
        "the match spans the URL's first through last column"
    );
    assert!(
        detect_url(&frame, 0, 29).is_none(),
        "the column after the URL is not part of it"
    );
    assert_eq!(
        detect_url(&frame, 0, 33).map(|matched| matched.url),
        Some("https://www.cove.dev".to_owned()),
        "a www. host is completed to https and the trailing period is trimmed"
    );
    assert!(
        detect_url(&frame, 0, 1).is_none(),
        "plain prose is not a link"
    );
}

#[test]
fn option_drag_selects_a_rectangular_block() {
    let mut engine = engine("abcdef\r\nghijkl\r\nmnopqr", 40);
    let at = |column: usize, row: usize, inset: f64| TerminalPointer {
        column,
        row,
        x: column as f64 * 10.0 + inset,
        y: row as f64 * 20.0 + 10.0,
        nanoseconds: 1,
    };

    engine.select(SelectionPhase::Press, &at(1, 0, 2.0), false);
    engine.select(SelectionPhase::Drag, &at(3, 2, 7.0), true);

    assert_eq!(
        engine.selected_text().as_deref(),
        Some("bcd\nhij\nnop"),
        "the block covers columns 1..=3 on each of the three rows"
    );
    let frame = engine.snapshot();
    assert!(frame.cell(1, 2).is_selected, "column 2 of row 1 is inside");
    assert!(!frame.cell(1, 0).is_selected, "column 0 of row 1 is outside");
    assert!(!frame.cell(1, 4).is_selected, "column 4 of row 1 is outside");
}

#[test]
fn sgr_5_marks_blinking_text() {
    let mut engine = engine("\x1b[5mon\x1b[0m off", 40);
    let frame = engine.snapshot();

    assert!(
        frame
            .cell(0, 0)
            .attributes
            .contains(TerminalCellAttributes::BLINK),
        "SGR 5 marks the cell as blinking"
    );
    assert!(
        !frame
            .cell(0, 3)
            .attributes
            .contains(TerminalCellAttributes::BLINK),
        "the reset text after it does not blink"
    );
}

#[test]
fn diacritic_indices_come_from_the_kitty_table() {
    assert_eq!(PlaceholderCell::index(0x0305), Some(0), "the first diacritic");
    assert_eq!(PlaceholderCell::index(0x030D), Some(1), "the second diacritic");
    assert_eq!(
        PlaceholderCell::index(0x1D244),
        Some(296),
        "the last entry of the 297-entry table"
    );
    assert_eq!(
        PlaceholderCell::index(0x41),
        None,
        "an ordinary letter encodes nothing"
    );
}

#[test]
fn unicode_placeholders_draw_their_strip_of_a_virtual_placement() {
    let mut engine = engine("", 40);
    engine
        .feed(b"\x1b_Ga=T,U=1,i=42,c=2,r=1,f=32,s=2,v=1,q=2;/wAA/wAA/4A=\x1b\\");
    assert!(
        engine.snapshot().images.is_empty(),
        "a virtual placement draws nothing on its own"
    );

    engine.feed(
        "\x1b[38;5;42m\u{10EEEE}\u{0305}\u{0305}\u{10EEEE}\u{0305}\x1b[0m".as_bytes(),
    );
    let frame = engine.snapshot();
    assert_eq!(frame.placeholders.len(), 2, "two placeholder cells were captured");
    assert_eq!(frame.images.len(), 1, "the run produced one image strip");

    let placement = frame.images.first().expect("the strip is placed");
    assert_eq!(placement.image_id, 42, "the foreground colour carries the id");
    assert_eq!((placement.column, placement.row), (0, 0), "at the grid origin");
    assert_eq!(
        (placement.width, placement.height),
        (20, 10),
        "a 2:1 image in a 20x20 box fills the width and is centred vertically"
    );
    assert_eq!(
        placement.offset,
        (0, 5),
        "the vertical centring shows up as a 5 pixel offset"
    );
    assert_eq!(
        (placement.source.2, placement.source.3),
        (2, 1),
        "the source strip is the whole 2x1 image"
    );

    assert_eq!(PLACEHOLDER_CODEPOINT, 0x10EEEE, "the base codepoint is U+10EEEE");
}

#[test]
fn placeholder_runs_split_on_image_changes_and_explicit_columns() {
    fn cell(column: usize, id: u32, image_column: Option<usize>) -> PlaceholderCell {
        let mut text = String::from("\u{10EEEE}\u{0305}");
        if let Some(image_column) = image_column {
            let diacritic = diacritic_at(image_column);
            text.push(char::from_u32(diacritic).expect("the diacritic is a scalar value"));
        }
        PlaceholderCell::new(&text, column, 0, id, 0).expect("the cell is a placeholder")
    }

    let cells = [
        cell(0, 1, Some(0)),
        cell(1, 1, None),
        cell(2, 1, Some(5)),
        cell(3, 2, None),
    ];
    let runs = PlaceholderRun::runs(&cells);

    let widths: Vec<usize> = runs.iter().map(|run| run.width).collect();
    let image_columns: Vec<usize> = runs.iter().map(|run| run.image_column).collect();
    let image_ids: Vec<u32> = runs.iter().map(|run| run.image_id).collect();

    assert_eq!(widths, [2, 1, 1], "the implied column joins the run, the explicit one splits it");
    assert_eq!(image_columns, [0, 5, 0], "each run carries its first image column");
    assert_eq!(image_ids, [1, 1, 2], "a new image id starts a new run");
}

/// The diacritic that encodes `index`, read back through the public index map so
/// the test does not duplicate the table.
fn diacritic_at(index: usize) -> u32 {
    const CANDIDATES: [u32; 297] = [
        0x0305, 0x030D, 0x030E, 0x0310, 0x0312, 0x033D, 0x033E, 0x033F, 0x0346, 0x034A, 0x034B,
        0x034C, 0x0350, 0x0351, 0x0352, 0x0357, 0x035B, 0x0363, 0x0364, 0x0365, 0x0366, 0x0367,
        0x0368, 0x0369, 0x036A, 0x036B, 0x036C, 0x036D, 0x036E, 0x036F, 0x0483, 0x0484, 0x0485,
        0x0486, 0x0487, 0x0592, 0x0593, 0x0594, 0x0595, 0x0597, 0x0598, 0x0599, 0x059C, 0x059D,
        0x059E, 0x059F, 0x05A0, 0x05A1, 0x05A8, 0x05A9, 0x05AB, 0x05AC, 0x05AF, 0x05C4, 0x0610,
        0x0611, 0x0612, 0x0613, 0x0614, 0x0615, 0x0616, 0x0617, 0x0657, 0x0658, 0x0659, 0x065A,
        0x065B, 0x065D, 0x065E, 0x06D6, 0x06D7, 0x06D8, 0x06D9, 0x06DA, 0x06DB, 0x06DC, 0x06DF,
        0x06E0, 0x06E1, 0x06E2, 0x06E4, 0x06E7, 0x06E8, 0x06EB, 0x06EC, 0x0730, 0x0732, 0x0733,
        0x0735, 0x0736, 0x073A, 0x073D, 0x073F, 0x0740, 0x0741, 0x0743, 0x0745, 0x0747, 0x0749,
        0x074A, 0x07EB, 0x07EC, 0x07ED, 0x07EE, 0x07EF, 0x07F0, 0x07F1, 0x07F3, 0x0816, 0x0817,
        0x0818, 0x0819, 0x081B, 0x081C, 0x081D, 0x081E, 0x081F, 0x0820, 0x0821, 0x0822, 0x0823,
        0x0825, 0x0826, 0x0827, 0x0829, 0x082A, 0x082B, 0x082C, 0x082D, 0x0951, 0x0953, 0x0954,
        0x0F82, 0x0F83, 0x0F86, 0x0F87, 0x135D, 0x135E, 0x135F, 0x17DD, 0x193A, 0x1A17, 0x1A75,
        0x1A76, 0x1A77, 0x1A78, 0x1A79, 0x1A7A, 0x1A7B, 0x1A7C, 0x1B6B, 0x1B6D, 0x1B6E, 0x1B6F,
        0x1B70, 0x1B71, 0x1B72, 0x1B73, 0x1CD0, 0x1CD1, 0x1CD2, 0x1CDA, 0x1CDB, 0x1CE0, 0x1DC0,
        0x1DC1, 0x1DC3, 0x1DC4, 0x1DC5, 0x1DC6, 0x1DC7, 0x1DC8, 0x1DC9, 0x1DCB, 0x1DCC, 0x1DD1,
        0x1DD2, 0x1DD3, 0x1DD4, 0x1DD5, 0x1DD6, 0x1DD7, 0x1DD8, 0x1DD9, 0x1DDA, 0x1DDB, 0x1DDC,
        0x1DDD, 0x1DDE, 0x1DDF, 0x1DE0, 0x1DE1, 0x1DE2, 0x1DE3, 0x1DE4, 0x1DE5, 0x1DE6, 0x1DFE,
        0x20D0, 0x20D1, 0x20D4, 0x20D5, 0x20D6, 0x20D7, 0x20DB, 0x20DC, 0x20E1, 0x20E7, 0x20E9,
        0x20F0, 0x2CEF, 0x2CF0, 0x2CF1, 0x2DE0, 0x2DE1, 0x2DE2, 0x2DE3, 0x2DE4, 0x2DE5, 0x2DE6,
        0x2DE7, 0x2DE8, 0x2DE9, 0x2DEA, 0x2DEB, 0x2DEC, 0x2DED, 0x2DEE, 0x2DEF, 0x2DF0, 0x2DF1,
        0x2DF2, 0x2DF3, 0x2DF4, 0x2DF5, 0x2DF6, 0x2DF7, 0x2DF8, 0x2DF9, 0x2DFA, 0x2DFB, 0x2DFC,
        0x2DFD, 0x2DFE, 0x2DFF, 0xA66F, 0xA67C, 0xA67D, 0xA6F0, 0xA6F1, 0xA8E0, 0xA8E1, 0xA8E2,
        0xA8E3, 0xA8E4, 0xA8E5, 0xA8E6, 0xA8E7, 0xA8E8, 0xA8E9, 0xA8EA, 0xA8EB, 0xA8EC, 0xA8ED,
        0xA8EE, 0xA8EF, 0xA8F0, 0xA8F1, 0xAAB0, 0xAAB2, 0xAAB3, 0xAAB7, 0xAAB8, 0xAABE, 0xAABF,
        0xAAC1, 0xFE20, 0xFE21, 0xFE22, 0xFE23, 0xFE24, 0xFE25, 0xFE26, 0x10A0F, 0x10A38, 0x1D185,
        0x1D186, 0x1D187, 0x1D188, 0x1D189, 0x1D1AA, 0x1D1AB, 0x1D1AC, 0x1D1AD, 0x1D242, 0x1D243,
        0x1D244,
    ];
    CANDIDATES[index]
}

