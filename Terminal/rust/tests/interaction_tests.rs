//! Translation of `Terminal/Tests/CoveUITerminalTests/InteractionTests.swift`.
//!
//! Behaviour checklist:
//! - dragging selects text; a cell joins once the pointer passes its midpoint,
//!   the copy reads through `selected_text` and the snapshot flags the cells
//! - double click selects a word, triple click a line, a fresh single click
//!   clears
//! - mouse reports follow the program's tracking mode and the SGR wire format
//! - OSC 8 hyperlinks are flagged per cell and resolve to their URI
//! - Kitty raw RGBA images land at the cursor and read back as pixels
//! - Kitty PNG images decode through the host image decoder
//!
//! Named differences from the Swift suite:
//! - Rust `select` takes an explicit `rectangle` flag (Swift encodes it in the
//!   pointer event); every case here uses the linear gesture, `false`.
//! - `image_pixels` returns a `cove_core::Bitmap` with accessor methods instead
//!   of the Swift tuple's `pixels`/`width` fields.

use cove_terminal::engine::{
    MouseAction, MouseButton, SelectionPhase, TerminalEngine, TerminalMouseEvent, TerminalPointer,
};

fn engine(text: &str) -> TerminalEngine {
    let mut engine = TerminalEngine::new(20, 4, 100);
    engine.resize(20, 4, 10, 20);
    engine.feed(text.as_bytes());
    engine
}

/// `inset` is the offset into the cell; selection includes a cell once the
/// pointer passes its midpoint.
fn pointer(column: usize, row: usize, time: u64, inset: f64) -> TerminalPointer {
    TerminalPointer {
        column,
        row,
        x: (column as f64) * 10.0 + inset,
        y: (row * 20) as f64 + 8.0,
        nanoseconds: time,
    }
}

#[test]
fn dragging_selects_text_which_copies_and_renders_as_selected() {
    let mut engine = engine("hello world\r\nsecond line");
    assert!(
        !engine.select(SelectionPhase::Press, &pointer(0, 0, 1, 2.0), false),
        "a press anchors but does not yet select"
    );
    assert!(
        engine.select(SelectionPhase::Drag, &pointer(4, 0, 0, 2.0), false),
        "the drag extends the selection"
    );
    assert_eq!(engine.selected_text().as_deref(), Some("hell"));
    engine.select(SelectionPhase::Drag, &pointer(4, 0, 0, 7.0), false);
    engine.select(SelectionPhase::Release, &pointer(4, 0, 0, 7.0), false);
    assert_eq!(
        engine.selected_text().as_deref(),
        Some("hello"),
        "a cell joins once the pointer passes its midpoint"
    );
    let frame = engine.snapshot();
    assert!(
        (0..=4).all(|column| frame.cell(0, column).is_selected),
        "cells 0..=5 render as selected"
    );
    assert!(
        !frame.cell(0, 5).is_selected,
        "the space after the word is not selected"
    );
    assert!(engine.clear_selection());
    assert_eq!(engine.selected_text(), None, "clearing removes the selection");
}

#[test]
fn double_click_selects_a_word_triple_a_line_and_single_clears() {
    let mut engine = engine("hello world\r\nsecond line");
    let second: u64 = 1_000_000_000;
    engine.select(SelectionPhase::Press, &pointer(7, 0, second, 2.0), false);
    engine.select(SelectionPhase::Release, &pointer(7, 0, 0, 2.0), false);
    assert!(
        engine.select(
            SelectionPhase::Press,
            &pointer(7, 0, second + 100_000_000, 2.0),
            false
        ),
        "the second click selects"
    );
    engine.select(SelectionPhase::Release, &pointer(7, 0, 0, 2.0), false);
    assert_eq!(engine.selected_text().as_deref(), Some("world"));
    assert!(engine.select(
        SelectionPhase::Press,
        &pointer(7, 0, second + 200_000_000, 2.0),
        false
    ));
    engine.select(SelectionPhase::Release, &pointer(7, 0, 0, 2.0), false);
    assert_eq!(
        engine.selected_text().as_deref(),
        Some("hello world"),
        "the third click takes the whole line"
    );
    engine.select(SelectionPhase::Press, &pointer(2, 1, second * 5, 2.0), false);
    assert_eq!(
        engine.selected_text(),
        None,
        "a fresh single click clears the old selection"
    );
}

#[test]
fn mouse_reports_follow_the_programs_tracking_mode_and_format() {
    let mut engine = engine("");
    assert!(!engine.is_mouse_tracking());
    engine.feed("\u{1B}[?1000h\u{1B}[?1006h".as_bytes());
    assert!(engine.is_mouse_tracking());
    let press = TerminalMouseEvent::new(MouseAction::Press, Some(MouseButton::Left), 35.0, 45.0);
    assert_eq!(
        String::from_utf8(engine.encode_mouse(&press)).expect("utf-8 report"),
        "\u{1B}[<0;4;3M"
    );
    let release = TerminalMouseEvent::new(MouseAction::Release, Some(MouseButton::Left), 35.0, 45.0);
    assert_eq!(
        String::from_utf8(engine.encode_mouse(&release)).expect("utf-8 report"),
        "\u{1B}[<0;4;3m"
    );
    let wheel = TerminalMouseEvent::new(MouseAction::Press, Some(MouseButton::WheelUp), 5.0, 5.0);
    assert_eq!(
        String::from_utf8(engine.encode_mouse(&wheel)).expect("utf-8 report"),
        "\u{1B}[<64;1;1M"
    );
    // Normal tracking (1000) does not report motion without button-event mode.
    let motion = TerminalMouseEvent::new(MouseAction::Motion, None, 50.0, 5.0);
    assert!(engine.encode_mouse(&motion).is_empty());
}

#[test]
fn osc8_hyperlinks_are_flagged_per_cell_and_resolve_to_their_uri() {
    let mut engine = engine("see \u{1B}]8;;https://example.com/a?b=1\u{1B}\\docs\u{1B}]8;;\u{1B}\\ now");
    let frame = engine.snapshot();
    assert_eq!(frame.text_of_row(0), "see docs now");
    assert!(!frame.cell(0, 3).is_hyperlink, "the space stays plain");
    assert!(
        (4..=7).all(|column| frame.cell(0, column).is_hyperlink),
        "the linked word is flagged"
    );
    assert_eq!(
        engine.hyperlink(5, 0).as_deref(),
        Some("https://example.com/a?b=1")
    );
    assert_eq!(engine.hyperlink(9, 0), None);
    assert_eq!(engine.hyperlink(99, 0), None, "a column past the grid is none");
}

#[test]
fn kitty_raw_rgba_images_land_at_the_cursor_and_read_back_as_pixels() {
    let mut engine = engine("ab");
    // 2×1 RGBA: red, then half-transparent blue.
    engine.feed("\u{1B}_Ga=T,f=32,s=2,v=1,i=7,q=2;/wAA/wAA/4A=\u{1B}\\".as_bytes());
    let frame = engine.snapshot();
    assert_eq!(frame.images.len(), 1, "one image is placed");
    let placement = frame.images.first().expect("the placement");
    assert_eq!(placement.image_id, 7);
    assert_eq!(placement.column, 2);
    assert_eq!(placement.row, 0);
    assert_eq!(placement.image_size, (2, 1));
    assert!(!placement.is_below_text);
    let pixels = engine.image_pixels(7).expect("the image decodes");
    assert_eq!(
        pixels.pixels(),
        [255, 0, 0, 255, 0, 0, 255, 128].as_slice(),
        "the raw RGBA bytes round-trip"
    );
}

#[test]
fn kitty_png_images_decode_through_the_host_image_decoder() {
    let mut engine = engine("");
    let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";
    engine.feed(format!("\u{1B}_Ga=T,f=100,i=3,q=2;{png}\u{1B}\\").as_bytes());
    let ids: Vec<u32> = engine
        .snapshot()
        .images
        .iter()
        .map(|placement| placement.image_id)
        .collect();
    assert_eq!(ids, vec![3]);
    let pixels = engine.image_pixels(3).expect("the png decodes");
    assert_eq!(pixels.width(), 1);
    assert_eq!(pixels.pixels().len(), 4, "one RGBA pixel");
}
