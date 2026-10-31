//! Translation of `Tests/CoveUIComposeTests/BidiTests.swift`.
//!
//! Behaviour checklist:
//! - character classes cover strong, number, separator and mark types
//! - levels follow the paragraph direction, numbers and neutrals
//! - each hard line break starts a paragraph with its own base direction
//! - L2 reverses runs from the highest level down
//! - text layout orders runs visually and right-aligns right-to-left paragraphs
//! - rich text aligns each paragraph line by its own direction
//!
//! The Rust `BidiParagraphs` exposes `paragraph_at_byte` rather than the Swift
//! `paragraph(atByte:)`, and its `BidiClass` variants are spelled `L`, `R`,
//! `Al`, `En`, `An`, `Es`, `Et`, `Cs`, `Ws`, `B`, `Nsm`, `On`; both are
//! straight renames, not gaps.

mod support;

use cove_core::{BidiClass, BidiParagraphs, PositionedGlyph};
use cove_text::TextEnvironment;
use cove_ui_compose::primitives::text::rich::flow::{
    RichParagraph, RichParagraphSpan, RichTextFlow,
};

use support::GlobalTestLock;

fn first_byte_of(text: &str, needle: &str) -> usize {
    text.find(needle).unwrap_or_else(|| panic!("{needle:?} occurs in {text:?}"))
}

#[test]
fn classes() {
    assert_eq!(BidiClass::of('a'), BidiClass::L, "a Latin letter is L");
    assert_eq!(BidiClass::of('ש'), BidiClass::R, "a Hebrew letter is R");
    assert_eq!(BidiClass::of('م'), BidiClass::Al, "an Arabic letter is AL");
    assert_eq!(BidiClass::of('1'), BidiClass::En, "a digit is EN");
    assert_eq!(BidiClass::of('\u{0661}'), BidiClass::An, "an Arabic digit is AN");
    assert_eq!(BidiClass::of('+'), BidiClass::Es, "a plus sign is ES");
    assert_eq!(BidiClass::of('%'), BidiClass::Et, "a percent sign is ET");
    assert_eq!(BidiClass::of(','), BidiClass::Cs, "a comma is CS");
    assert_eq!(BidiClass::of(' '), BidiClass::Ws, "a space is WS");
    assert_eq!(BidiClass::of('\n'), BidiClass::B, "a newline is B");
    assert_eq!(BidiClass::of('\u{05B4}'), BidiClass::Nsm, "a Hebrew point is NSM");
    assert_eq!(BidiClass::of('!'), BidiClass::On, "an exclamation mark is ON");
    assert_eq!(
        BidiClass::of('\u{200F}'),
        BidiClass::R,
        "a right-to-left mark is R"
    );
    assert_eq!(BidiClass::of('中'), BidiClass::L, "a Han ideograph is L");
}

#[test]
fn levels() {
    let ltr = BidiParagraphs::new("abc שלום def", false);
    assert_eq!(
        ltr.paragraphs[0].base_level, 0,
        "a Latin-first paragraph has embedding level 0"
    );
    let hebrew_start = first_byte_of("abc שלום def", "ש");
    assert_eq!(ltr.level_at_byte(0), 0, "the Latin run is level 0");
    assert_eq!(
        ltr.level_at_byte(hebrew_start),
        1,
        "the Hebrew run is level 1"
    );
    assert_eq!(
        ltr.level_at_byte(hebrew_start - 1),
        0,
        "the space between L and R takes the embedding level"
    );

    let rtl = BidiParagraphs::new("שלום 123 world!", false);
    assert!(
        rtl.paragraphs[0].is_right_to_left(),
        "a Hebrew-first paragraph is right to left"
    );
    let digits = first_byte_of("שלום 123 world!", "1");
    assert_eq!(rtl.level_at_byte(digits), 2, "the digits are level 2");
    assert_eq!(
        rtl.level_at_byte(first_byte_of("שלום 123 world!", "w")),
        2,
        "the trailing neutral keeps the number's level"
    );
    assert_eq!(
        rtl.level_at_byte(rtl.levels.len() - 1),
        1,
        "a trailing neutral at the paragraph end resolves to the embedding direction"
    );

    let arabic = BidiParagraphs::new("عدد 12", false);
    assert_eq!(
        arabic.level_at_byte(first_byte_of("عدد 12", "1")),
        2,
        "W2 turns EN after AL into AN, still level 2"
    );
    assert!(
        !BidiParagraphs::contains_right_to_left("plain text 123"),
        "plain Latin text carries no right-to-left character"
    );
    assert!(
        BidiParagraphs::contains_right_to_left("x\u{0627}"),
        "a single Arabic letter makes the text right-to-left"
    );
}

#[test]
fn paragraphs() {
    let text = "Hello\nשלום";
    let bidi = BidiParagraphs::new(text, false);
    assert_eq!(bidi.paragraphs.len(), 2, "the newline splits two paragraphs");
    assert_eq!(
        bidi.paragraphs[0].base_level, 0,
        "the first paragraph is left to right"
    );
    assert_eq!(
        bidi.paragraphs[1].base_level, 1,
        "the second paragraph is right to left"
    );
    assert!(
        bidi.paragraph_at_byte(text.len() - 1).is_right_to_left(),
        "the last byte falls in the right-to-left paragraph"
    );
    assert_eq!(
        BidiParagraphs::new("123", true).paragraphs[0].base_level,
        1,
        "a digits-only paragraph inherits the default direction"
    );
}

#[test]
fn visual_order() {
    assert_eq!(
        BidiParagraphs::visual_order(&[0, 0, 1, 1, 0]),
        vec![0, 1, 3, 2, 4],
        "only the level-1 run reverses"
    );
    assert_eq!(
        BidiParagraphs::visual_order(&[1, 1, 2, 2, 1]),
        vec![4, 2, 3, 1, 0],
        "the highest level reverses first, then its enclosing run"
    );
    assert_eq!(
        BidiParagraphs::visual_order(&[0, 0]),
        vec![0, 1],
        "an all-LTR line keeps its order"
    );
}

fn layout_glyphs(
    environment: &TextEnvironment,
    text: &str,
    max_width: f32,
) -> Vec<PositionedGlyph> {
    environment
        .cached_layout(text, None, None, max_width, Default::default(), 0.0)
        .lines
        .first()
        .map(|line| line.glyphs.clone())
        .unwrap_or_default()
}

#[test]
fn layout_order() {
    GlobalTestLock::locked(|| {
        let environment = support::test_text_environment(16.0, 20.0);
        {
            let text = "abc שלום";
            let glyphs = layout_glyphs(&environment, text, 400.0);
            let latin: Vec<f32> = glyphs
                .iter()
                .filter(|glyph| glyph.cluster < 3)
                .map(|glyph| glyph.x)
                .collect();
            let mut sorted_latin = latin.clone();
            sorted_latin.sort_by(f32::total_cmp);
            assert_eq!(
                latin, sorted_latin,
                "Latin glyphs advance left to right ({latin:?})"
            );

            // Hebrew letters are drawn right to left: later characters sit
            // further left.
            let mut hebrew: Vec<&PositionedGlyph> =
                glyphs.iter().filter(|glyph| glyph.cluster >= 4).collect();
            hebrew.sort_by_key(|glyph| glyph.cluster);
            let by_cluster: Vec<f32> = hebrew.iter().map(|glyph| glyph.x).collect();
            if by_cluster.len() > 1 {
                let mut descending = by_cluster.clone();
                descending.sort_by(|left, right| right.total_cmp(left));
                assert_eq!(
                    by_cluster, descending,
                    "later Hebrew characters sit further left ({by_cluster:?})"
                );
            }

            let rtl_layout = environment.cached_layout("שלום", None, None, 400.0, Default::default(), 0.0);
            let rtl_line = &rtl_layout.lines[0];
            if let Some(first) = rtl_line
                .glyphs
                .iter()
                .map(|glyph| glyph.x)
                .fold(None::<f32>, |current, x| Some(current.map_or(x, |value| value.min(x))))
            {
                assert!(
                    first >= 400.0 - rtl_line.width - 0.5,
                    "a right-to-left line hugs the right margin (first glyph {first}, width {})",
                    rtl_line.width
                );
            }

            let ltr_layout = environment.cached_layout("abc", None, None, 400.0, Default::default(), 0.0);
            let leftmost = ltr_layout.lines[0]
                .glyphs
                .iter()
                .map(|glyph| glyph.x)
                .fold(f32::INFINITY, f32::min);
            assert!(
                leftmost < 1.0,
                "a left-to-right line starts at the left margin (leftmost {leftmost})"
            );
        }
    });
}

#[test]
fn rich_text_alignment() {
    GlobalTestLock::locked(|| {
        let environment = support::test_text_environment(16.0, 20.0);
        {
            let paragraph = RichParagraph::new(vec![RichParagraphSpan {
                text: "English\nעברית".to_string(),
                font: environment.default_font.clone(),
                line_height: 20.0,
                traits: Default::default(),
                link: None,
            }]);
            let flow = RichTextFlow::layout(&paragraph, 300.0, &environment);
            assert_eq!(
                flow.lines.len(),
                2,
                "the line break makes two measured lines"
            );
            assert!(
                !flow.lines[0].is_right_to_left,
                "the Latin line is left to right"
            );
            assert!(
                flow.lines[1].is_right_to_left,
                "the Hebrew line is right to left"
            );
            let second: Vec<f32> = flow.glyphs.lines[1]
                .glyphs
                .iter()
                .map(|glyph| glyph.x)
                .collect();
            let leftmost = second.iter().copied().fold(f32::INFINITY, f32::min);
            assert!(
                leftmost > 300.0 - flow.lines[1].width - 1.0,
                "the right-to-left line is right aligned (leftmost {leftmost}, width {})",
                flow.lines[1].width
            );
        }
    });
}
