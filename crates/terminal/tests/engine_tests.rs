//! Translation of `Terminal/Tests/CoveUITerminalTests/EngineTests.swift`.
//!
//! Behaviour checklist:
//! - plain text, line breaks and the cursor position land on the snapshot
//! - SGR attributes and palette colours resolve per cell
//! - truecolor foreground and background survive the round trip
//! - wide characters take a cell plus a spacer, combining marks stay in one cell
//! - resize reflows soft-wrapped lines
//! - the viewport scrolls through history and back to the live screen
//! - queries are answered through the reply callback
//! - title changes and the bell are reported
//! - paste follows bracketed-paste mode
//! - key encoding follows the program's keyboard modes
//! - the alternate screen is tracked

use cove_terminal::engine::{
    CellWidth, KeyModifiers, TerminalCellAttributes, TerminalEngine, TerminalKey, TerminalRGB,
};
use std::cell::RefCell;
use std::rc::Rc;

fn engine(columns: usize, rows: usize, scrollback: usize) -> TerminalEngine {
    TerminalEngine::new(columns, rows, scrollback)
}

fn default_engine() -> TerminalEngine {
    engine(20, 4, 100)
}

#[test]
fn plain_text_line_breaks_and_cursor() {
    let mut engine = default_engine();
    engine.feed(b"Hello\r\nWorld\r\n");
    let frame = engine.snapshot();

    assert_eq!(
        frame.columns, 20,
        "the snapshot keeps the requested columns"
    );
    assert_eq!(frame.rows, 4, "the snapshot keeps the requested rows");
    assert_eq!(frame.text_of_row(0), "Hello");
    assert_eq!(frame.text_of_row(1), "World");
    assert_eq!(frame.text_of_row(2), "");
    assert!(frame.cursor.is_visible, "the cursor is visible by default");
    assert_eq!(frame.cursor.column, 0, "the newline returns to column 0");
    assert_eq!(frame.cursor.row, 2, "two lines advanced the cursor");
}

#[test]
fn sgr_attributes_and_palette_colors_resolve_per_cell() {
    let mut engine = default_engine();
    engine.feed(b"\x1b[1;32mOK\x1b[0m \x1b[4;9mx\x1b[0m \x1b[7mi\x1b[0m");
    let frame = engine.snapshot();

    assert_eq!(frame.text_of_row(0), "OK x i");
    assert_eq!(
        frame.cell(0, 0).attributes,
        TerminalCellAttributes::BOLD,
        "SGR 1 is bold"
    );
    assert!(
        frame.cell(0, 0).foreground.is_some(),
        "SGR 32 resolves a palette foreground"
    );
    assert!(
        frame.cell(0, 2).foreground.is_none() && frame.cell(0, 2).attributes.is_empty(),
        "a reset cell carries no colour or attributes"
    );
    assert_eq!(
        frame.cell(0, 3).attributes,
        TerminalCellAttributes::UNDERLINE | TerminalCellAttributes::STRIKETHROUGH,
        "SGR 4;9 is underline plus strikethrough"
    );
    assert_eq!(
        frame.cell(0, 5).attributes,
        TerminalCellAttributes::INVERSE,
        "SGR 7 is inverse"
    );
}

#[test]
fn truecolor_foreground_and_background() {
    let mut engine = default_engine();
    engine.feed(b"\x1b[38;2;10;20;30;48;2;200;100;50mZ");
    let cell = engine.snapshot().cell(0, 0).clone();

    assert_eq!(
        cell.foreground,
        Some(TerminalRGB::new(10, 20, 30)),
        "the 38;2 foreground is kept verbatim"
    );
    assert_eq!(
        cell.background,
        Some(TerminalRGB::new(200, 100, 50)),
        "the 48;2 background is kept verbatim"
    );
}

#[test]
fn wide_characters_take_a_cell_plus_a_spacer() {
    let mut engine = default_engine();
    engine.feed("你好a".as_bytes());
    let frame = engine.snapshot();

    assert_eq!(frame.cell(0, 0).text, "你", "the wide glyph owns column 0");
    assert_eq!(frame.cell(0, 0).width, CellWidth::Wide);
    assert_eq!(
        frame.cell(0, 1).width,
        CellWidth::Spacer,
        "the trailing half is a spacer so column indices equal grid positions"
    );
    assert_eq!(
        frame.cell(0, 2).text,
        "好",
        "the second wide glyph starts at 2"
    );
    assert_eq!(frame.cell(0, 2).width, CellWidth::Wide);
    assert_eq!(
        frame.cell(0, 4).text,
        "a",
        "the narrow glyph follows the pair"
    );
    assert_eq!(frame.cell(0, 4).width, CellWidth::Narrow);
    assert_eq!(frame.cursor.column, 5, "the cursor counts grid columns");
    assert_eq!(
        frame.text_of_row(0),
        "你好a",
        "the spacers are skipped when reading the row"
    );
}

#[test]
fn combining_marks_stay_in_one_grapheme_cell() {
    let mut engine = default_engine();
    engine.feed("e\u{301}x".as_bytes());
    let frame = engine.snapshot();

    assert_eq!(
        frame.cell(0, 0).text,
        "e\u{301}",
        "the combining acute stays with its base"
    );
    assert_eq!(
        frame.cell(0, 1).text,
        "x",
        "the next cell holds the next grapheme"
    );
}

#[test]
fn resize_reflows_soft_wrapped_lines() {
    let mut engine = engine(10, 3, 100);
    engine.feed(b"ABCDEFGHIJKLMNOP");

    let mut frame = engine.snapshot();
    assert_eq!(frame.text_of_row(0), "ABCDEFGHIJ");
    assert_eq!(frame.text_of_row(1), "KLMNOP");

    engine.resize(8, 4, 8, 16);
    frame = engine.snapshot();
    assert_eq!(frame.columns, 8, "the snapshot follows the resize");
    assert_eq!(frame.rows, 4);
    assert_eq!(frame.text_of_row(0), "ABCDEFGH");
    assert_eq!(
        frame.text_of_row(1),
        "IJKLMNOP",
        "the soft-wrapped line is reflowed to the new width"
    );
}

#[test]
fn viewport_scrolls_through_history_and_back() {
    let mut engine = engine(10, 3, 100);
    let lines: Vec<String> = (1..=10).map(|index| format!("line{index}")).collect();
    engine.feed(lines.join("\r\n").as_bytes());

    let mut frame = engine.snapshot();
    assert_eq!(
        frame.text_of_row(2),
        "line10",
        "the live screen ends at line10"
    );
    assert!(
        frame.scroll.is_at_bottom(),
        "output leaves the viewport at the bottom"
    );

    engine.scroll(-2);
    frame = engine.snapshot();
    assert_eq!(
        frame.text_of_row(0),
        "line6",
        "two lines back starts at line6"
    );
    assert!(
        !frame.scroll.is_at_bottom(),
        "the viewport left the live screen"
    );
    assert!(
        !frame.cursor.is_visible,
        "a scrolled-back cursor is not drawn"
    );

    engine.scroll_to_bottom();
    assert_eq!(
        engine.snapshot().text_of_row(0),
        "line8",
        "returning to the bottom restores the live screen"
    );
}

#[test]
fn queries_are_answered_through_the_reply_callback() {
    let mut engine = default_engine();
    let replies: Rc<RefCell<Vec<Vec<u8>>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = replies.clone();
    engine.set_reply_handler(Rc::new(move |bytes| sink.borrow_mut().push(bytes.to_vec())));

    engine.feed(b"ab\x1b[6n");

    let replies = replies.borrow();
    let as_text: Vec<String> = replies
        .iter()
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
        .collect();
    assert_eq!(as_text, ["\x1b[1;3R"], "DSR reports the cursor position");
}

#[test]
fn title_changes_and_bell_are_reported() {
    let mut engine = default_engine();
    let titles: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let bells: Rc<RefCell<usize>> = Rc::new(RefCell::new(0));
    let title_sink = titles.clone();
    let bell_sink = bells.clone();
    engine.set_title_handler(Rc::new(move |title| {
        title_sink.borrow_mut().push(title.to_owned());
    }));
    engine.set_bell_handler(Rc::new(move || *bell_sink.borrow_mut() += 1));

    engine.feed("\x1b]0;build — zsh\x07\x07".as_bytes());

    assert_eq!(
        *titles.borrow(),
        ["build — zsh"],
        "OSC 0 reports the new title once"
    );
    assert_eq!(
        engine.title(),
        "build — zsh",
        "the engine remembers the title"
    );
    assert_eq!(*bells.borrow(), 1, "one bell was reported");
}

#[test]
fn paste_follows_bracketed_paste_mode() {
    let mut engine = default_engine();
    assert_eq!(
        engine.encode_paste("a\nb"),
        b"a\rb".to_vec(),
        "an unbracketed paste translates the newline"
    );

    engine.feed(b"\x1b[?2004h");
    assert_eq!(
        engine.encode_paste("a\nb"),
        b"\x1b[200~a\nb\x1b[201~".to_vec(),
        "bracketed paste frames the payload and keeps its newlines"
    );
    assert_eq!(
        engine.encode_paste(""),
        Vec::<u8>::new(),
        "empty paste is empty"
    );
}

#[test]
fn key_encoding_follows_the_programs_keyboard_modes() {
    let mut engine = default_engine();
    let up = TerminalKey::new(82);

    assert_eq!(
        engine.encode_key(&up),
        b"\x1b[A".to_vec(),
        "plain cursor up"
    );

    engine.feed(b"\x1b[?1h");
    assert_eq!(
        engine.encode_key(&up),
        b"\x1bOA".to_vec(),
        "DECCKM switches the cursor keys to application mode"
    );

    assert_eq!(
        engine.encode_key(&TerminalKey::new(40)),
        vec![0x0D],
        "Return"
    );
    assert_eq!(
        engine.encode_key(&TerminalKey::new(42)),
        vec![0x7F],
        "Backspace"
    );
    assert_eq!(
        engine.encode_key(&TerminalKey {
            scancode: 6,
            modifiers: KeyModifiers::CONTROL,
            text: "c".into(),
            is_repeat: false,
        }),
        vec![0x03],
        "Control-C is the ETX byte"
    );
    assert_eq!(
        engine.encode_key(&TerminalKey {
            scancode: 43,
            modifiers: KeyModifiers::SHIFT,
            text: String::new(),
            is_repeat: false,
        }),
        b"\x1b[Z".to_vec(),
        "Shift-Tab is the back-tab sequence"
    );
    assert_eq!(
        engine.encode_key(&TerminalKey::new(0xFFFF)),
        Vec::<u8>::new(),
        "an unmapped scancode encodes to nothing"
    );
}

#[test]
fn alternate_screen_is_tracked() {
    let mut engine = default_engine();
    assert!(
        !engine.is_alternate_screen(),
        "the main screen is active at start"
    );

    engine.feed(b"\x1b[?1049h");
    assert!(
        engine.is_alternate_screen(),
        "1049h switches to the alternate screen"
    );

    engine.feed(b"\x1b[?1049l");
    assert!(
        !engine.is_alternate_screen(),
        "1049l returns to the main screen"
    );
}
