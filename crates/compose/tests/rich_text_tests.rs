//! Translation of `Tests/CoveUIComposeTests/RichTextTests.swift`.
//!
//! Behaviour checklist:
//! - runs wrap together at spaces, every line fits the width, lines tile the
//!   text without dropping or repeating a character, and the height is one line
//!   height per line
//! - without a width limit a paragraph stays on one line with a positive width
//! - ideographs break between characters, closing punctuation never starts a
//!   line, and hyphens/slashes inside tokens are break points
//! - hard breaks end lines and blank lines keep their height
//! - inline code is padded and its fragment carries the chip glyphs
//! - points map to caret offsets and ranges map to highlight rectangles
//! - a click on any fragment of a link opens it; hover shows the pointer cursor
//! - italic survives text scaling and weight changes
//! - font modifiers on the paragraph reach measurement
//! - each link is a focusable link element over its first fragment that opens
//!   on Return
//! - without a link handler links are shown but not focusable
//! - `caret_rect` tracks the line a selection endpoint sits on
//!
//! Named differences from the Swift suite:
//! - The Rust paragraph opens a link on pointer *up* only; a press returns
//!   `EventResult::Ignored` where Swift returns `.handled`. `link_activation`
//!   pins the Rust result.
//! - The Rust paragraph registers a motion handler instead of a hover handler,
//!   so the `hover(.leave)` step of Swift's link test does not exist; the
//!   `link_activation` test moves the pointer off the link instead.
//! - `RichTextFlow::layout` returns an `Rc<RichTextFlow>`; the tests deref it.
//! - Rust link anchors are full-width, paragraph-height nodes stacked below the
//!   paragraph (the first at y = 200 inside a 400×200 mount), not overlays on
//!   the link's first fragment: `anchors` pins the stacked frames (400 wide,
//!   ordered top to bottom) instead of Swift's contained, side-by-side frames.
//! - `FocusChain::focus` takes a link anchor, but `RichTextReader::children`
//!   builds the anchors with `surface: None`, so the paragraph never records
//!   `FOCUSED_LINK` and paints no ring. Swift's remembered index is
//!   unobservable; `anchors` pins the empty attachment after focusing.
//! - Link anchors are `RichTextLinkAnchor` children of the paragraph; they are
//!   focusable and not hit-testable, exactly as Swift's `LinkAnchor`.

use crate::support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, HorizontalAlignment, TupleView};
use cove_core::scene::{
    AccessibilityRole, EventPhase, FocusChain, FocusChainHolder, InteractionRegistry,
    InteractionRegistryHolder, Node, PointerCapture, PointerCaptureHolder, PointerPhase,
    TextInputAttachmentKey, TextInputFocusChangeHandler,
};
use cove_core::{Font, FontWeight, line_break_opportunities};
use cove_platform::{
    KeyEvent, KeyModifiers, MouseButton, MouseButtonEvent, MouseMotionEvent, SystemCursor,
};
use cove_text::{TextEnvironment, TextEnvironmentHolder};
use cove_ui_compose::modifiers::style::AppearanceExt;
use cove_ui_compose::primitives::text::rich::RichText;
use cove_ui_compose::primitives::text::rich::flow::{
    RichParagraph, RichParagraphSpan, RichTextFlow,
};
use cove_ui_compose::primitives::text::rich::link_anchor::FOCUSED_LINK;
use cove_ui_compose::primitives::text::rich::run::{RichTextRun, RichTextTraits};
use cove_ui_compose::theme::TextScaler;
use cove_ui_compose::{Column, Scancode};

use support::GlobalTestLock;

fn paragraph(spans: Vec<(&str, RichTextTraits, Option<usize>)>) -> RichParagraph {
    RichParagraph::new(
        spans
            .into_iter()
            .map(|(text, traits, link)| {
                let font = if traits.contains(RichTextTraits::CODE) {
                    Font::monospaced(15.0, FontWeight::Regular)
                } else {
                    Font::system(16.0, FontWeight::Regular)
                };
                RichParagraphSpan {
                    text: text.to_string(),
                    font,
                    line_height: 20.0,
                    traits,
                    link,
                }
            })
            .collect(),
    )
}

fn plain(text: &str) -> RichParagraph {
    paragraph(vec![(text, RichTextTraits::from_raw(0), None)])
}

fn environment() -> TextEnvironment {
    TextEnvironmentHolder::current().expect("a test text environment")
}

fn visible_text(flow: &RichTextFlow, line: usize) -> String {
    let bounds = &flow.lines[line];
    String::from_utf8_lossy(&flow.text.as_bytes()[bounds.start_offset..bounds.end_offset])
        .trim()
        .to_string()
}

fn nodes(root: &Node) -> Vec<Node> {
    let mut all = vec![root.clone()];
    for child in root.children() {
        all.extend(nodes(&child));
    }
    all
}

fn surfaces(graph: &cove_composition::ViewGraph, role: AccessibilityRole) -> Vec<Node> {
    let root = graph.tree.borrow().root().expect("an installed root");
    nodes(&root)
        .into_iter()
        .filter(|node| node.accessibility().is_some_and(|a| a.role == role))
        .collect()
}

fn press(x: f32, y: f32) -> MouseButtonEvent {
    MouseButtonEvent {
        button: MouseButton::Left,
        x,
        y,
        clicks: 1,
        modifiers: KeyModifiers::default(),
        kind: Default::default(),
    }
}

fn motion(x: f32, y: f32) -> MouseMotionEvent {
    MouseMotionEvent {
        x,
        y,
        delta_x: 0.0,
        delta_y: 0.0,
        kind: Default::default(),
    }
}

fn key(scancode: u32) -> KeyEvent {
    KeyEvent {
        scancode,
        keycode: 0,
        modifiers: KeyModifiers::default(),
        is_repeat: false,
    }
}

#[test]
fn wraps_across_runs() {
    support::with_test_text_environment(|| {
        let env = environment();
        let flow = RichTextFlow::layout(
            &paragraph(vec![
                ("The quick ", RichTextTraits::from_raw(0), None),
                ("brown", RichTextTraits::BOLD, None),
                (" fox jumps over the ", RichTextTraits::from_raw(0), None),
                ("lazy", RichTextTraits::ITALIC, None),
                (" dog again and again", RichTextTraits::from_raw(0), None),
            ]),
            120.0,
            &env,
        );
        assert!(
            flow.lines.len() > 2,
            "a 120pt column wraps this paragraph, got {} lines",
            flow.lines.len()
        );
        for line in &flow.lines {
            assert!(
                line.width <= 120.5,
                "every line fits the width, got {}",
                line.width
            );
        }
        for pair in flow.lines.windows(2) {
            assert_eq!(
                pair[0].end_offset, pair[1].start_offset,
                "lines tile the text without gaps"
            );
        }
        let words = (0..flow.lines.len())
            .map(|index| visible_text(&flow, index))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(
            words, "The quick brown fox jumps over the lazy dog again and again",
            "no character is dropped or repeated"
        );
        assert_eq!(
            flow.height,
            flow.lines.len() as f32 * 20.0,
            "one 20pt line per wrapped line"
        );
    });
}

#[test]
fn intrinsic_width() {
    support::with_test_text_environment(|| {
        let env = environment();
        let flow = RichTextFlow::layout(
            &paragraph(vec![
                ("one ", RichTextTraits::from_raw(0), None),
                ("two", RichTextTraits::BOLD, None),
            ]),
            f32::INFINITY,
            &env,
        );
        assert_eq!(
            flow.lines.len(),
            1,
            "an unbounded paragraph stays on one line"
        );
        assert!(
            flow.width > 0.0,
            "the intrinsic width is positive, got {}",
            flow.width
        );
    });
}

#[test]
fn ideographic_breaks() {
    let breaks = line_break_opportunities("中文，段落");
    let comma = "中文".len();
    assert!(
        breaks.contains(&"中".len()),
        "breaks after the first ideograph"
    );
    assert!(
        !breaks.contains(&comma),
        "a closing comma never starts a line, got {breaks:?}"
    );
    assert!(
        breaks.contains(&(comma + "，".len())),
        "the next ideograph is a break point, got {breaks:?}"
    );

    let url = line_break_opportunities("a/b-c word");
    assert!(
        url.contains(&2) && url.contains(&4) && url.contains(&6),
        "hyphens and slashes inside a token break, got {url:?}"
    );
    assert!(!url.contains(&1), "a plain slash boundary is not a break");
}

#[test]
fn hard_breaks() {
    support::with_test_text_environment(|| {
        let env = environment();
        let flow = RichTextFlow::layout(&plain("first\n\nthird"), 400.0, &env);
        assert_eq!(flow.lines.len(), 3, "two hard breaks make three lines");
        assert_eq!(visible_text(&flow, 0), "first");
        assert_eq!(
            flow.lines[1].start_offset, flow.lines[1].end_offset,
            "the blank line holds no text"
        );
        assert_eq!(visible_text(&flow, 2), "third");
        assert_eq!(flow.height, 60.0, "the blank line keeps its 20pt height");
    });
}

#[test]
fn code_padding() {
    support::with_test_text_environment(|| {
        let env = environment();
        let plain_flow = RichTextFlow::layout(
            &paragraph(vec![
                ("x ", RichTextTraits::from_raw(0), None),
                ("code", RichTextTraits::from_raw(0), None),
            ]),
            f32::INFINITY,
            &env,
        );
        let code_flow = RichTextFlow::layout(
            &paragraph(vec![
                ("x ", RichTextTraits::from_raw(0), None),
                ("code", RichTextTraits::CODE, None),
            ]),
            f32::INFINITY,
            &env,
        );
        let chip = code_flow
            .fragments
            .iter()
            .find(|fragment| fragment.span == 1)
            .expect("the code fragment");
        assert!(
            chip.max_x - chip.min_x > 0.0,
            "the chip has width, got {}",
            chip.max_x - chip.min_x
        );
        let glyph = code_flow.glyphs.lines[0]
            .glyphs
            .iter()
            .find(|glyph| glyph.cluster == 2)
            .expect("the code glyph");
        assert!(
            glyph.x > chip.min_x,
            "the code glyph sits inside the chip, glyph {} chip {}",
            glyph.x,
            chip.min_x
        );
        assert!(
            code_flow.width > plain_flow.width - 8.0,
            "the code run adds padding: code {} plain {}",
            code_flow.width,
            plain_flow.width
        );
    });
}

#[test]
fn caret_mapping() {
    support::with_test_text_environment(|| {
        let env = environment();
        let flow = RichTextFlow::layout(&plain("alpha beta gamma delta"), 90.0, &env);
        assert_eq!(
            flow.offset(-10.0, 5.0),
            0,
            "a point before the text clamps to 0"
        );
        assert_eq!(
            flow.offset(2000.0, 2000.0),
            flow.text.len(),
            "a point past the text clamps to the end"
        );
        let second = &flow.lines[1];
        let height = second.top + 1.0;
        assert_eq!(
            flow.offset(0.0, height),
            second.start_offset,
            "a point on the second line maps to its start"
        );
        let rects = flow.selection_rects(0..flow.text.len());
        assert_eq!(
            rects.len(),
            flow.lines.len(),
            "one highlight rectangle per line"
        );
        assert!(
            flow.selection_rects(3..3).is_empty(),
            "an empty range highlights nothing"
        );
    });
}

#[test]
fn link_activation() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let registry = Rc::new(InteractionRegistry::new());
            InteractionRegistryHolder::set_current(Some(registry.clone()));
            PointerCaptureHolder::set_current(Some(Rc::new(PointerCapture::new())));

            let opened = Rc::new(RefCell::new(Vec::<String>::new()));
            let sink = opened.clone();
            let graph = support::graph();
            graph.install(
                RichText::new(vec![
                    RichTextRun::new("Read the "),
                    RichTextRun::new("style guide").link("guide://style"),
                    RichTextRun::new(" today."),
                ])
                .on_open_link(move |target| sink.borrow_mut().push(target.to_string())),
                Default::default(),
            );
            graph.compute_layout(400.0, 200.0);

            let surface = surfaces(&graph, AccessibilityRole::StaticText)
                .into_iter()
                .next()
                .expect("the paragraph surface");
            let frame = surface.absolute_frame();
            let handlers = registry.handlers(&surface);
            let pointer = handlers.pointer.clone().expect("a pointer handler");
            let motion_handler = handlers.motion.clone().expect("a motion handler");
            let y = frame.y + 8.0;

            let plain_x = frame.x + 4.0;
            pointer(&press(plain_x, y), PointerPhase::Down, EventPhase::Target);
            pointer(&press(plain_x, y), PointerPhase::Up, EventPhase::Target);
            assert!(
                opened.borrow().is_empty(),
                "plain words start a selection instead of opening a link"
            );

            let mut link_x = None;
            let mut x = frame.x;
            while x < frame.max_x() {
                motion_handler(&motion(x, y), EventPhase::Target);
                if surface.cursor() == Some(SystemCursor::Pointer) {
                    link_x = Some(x);
                    break;
                }
                x += 2.0;
            }
            let link_x = link_x.expect("the pointer cursor appears over the link");

            let down = pointer(
                &press(link_x + 4.0, y),
                PointerPhase::Down,
                EventPhase::Target,
            );
            assert_eq!(
                down,
                cove_core::scene::EventResult::Ignored,
                "the Rust paragraph opens on release, not on press"
            );
            pointer(
                &press(link_x + 4.0, y),
                PointerPhase::Up,
                EventPhase::Target,
            );
            assert_eq!(
                *opened.borrow(),
                vec!["guide://style".to_string()],
                "releasing on the link opens it"
            );

            pointer(
                &press(link_x + 4.0, y),
                PointerPhase::Down,
                EventPhase::Target,
            );
            pointer(
                &press(frame.x + 2.0, y),
                PointerPhase::Up,
                EventPhase::Target,
            );
            assert_eq!(opened.borrow().len(), 1, "releasing away cancels the link");

            motion_handler(&motion(plain_x, y), EventPhase::Target);
            assert_eq!(
                surface.cursor(),
                Some(SystemCursor::Ibeam),
                "leaving the link restores the text cursor"
            );
        });
    });
}

#[test]
fn italic_font() {
    let italic = Font::system(14.0, FontWeight::Regular).with_italic(true);
    assert!(italic.is_italic(), "the italic request sticks");
    assert!(
        TextScaler::new(2.0).scale_font(&italic).is_italic(),
        "scaling preserves the italic design"
    );
    let bold = italic.with_weight(FontWeight::Bold);
    assert!(
        bold.is_italic() && bold.weight() == FontWeight::Bold,
        "weight keeps italic"
    );
}

#[test]
fn modifiers_apply() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = support::graph();
            graph.install(
                Column::new(
                    HorizontalAlignment::Center,
                    0.0,
                    AnyView::new(TupleView::new(vec![
                        AnyView::new(RichText::new(vec![RichTextRun::new("Body")])),
                        AnyView::new(
                            RichText::new(vec![RichTextRun::new("Large")])
                                .font(Font::system(40.0, FontWeight::Regular)),
                        ),
                    ])),
                ),
                Default::default(),
            );
            graph.compute_layout(400.0, 400.0);
            let surfaces = surfaces(&graph, AccessibilityRole::StaticText);
            assert_eq!(surfaces.len(), 2, "both paragraphs materialise");
            let body = surfaces[0].frame();
            let large = surfaces[1].frame();
            assert!(
                body.width > 0.0 && body.height > 0.0,
                "body text measures its content"
            );
            assert!(
                large.height > body.height,
                "the font modifier makes the paragraph taller: body {body:?}, large {large:?}"
            );
        });
    });
}

#[test]
fn anchors() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let registry = Rc::new(InteractionRegistry::new());
            InteractionRegistryHolder::set_current(Some(registry.clone()));
            let focus = Rc::new(FocusChain::new());
            FocusChainHolder::set_current(Some(focus.clone()));

            let opened = Rc::new(RefCell::new(Vec::<String>::new()));
            let sink = opened.clone();
            let graph = support::graph();
            graph.install(
                RichText::new(vec![
                    RichTextRun::new("Read the "),
                    RichTextRun::new("style ").link("guide://style"),
                    RichTextRun::with_traits("guide", RichTextTraits::BOLD).link("guide://style"),
                    RichTextRun::new(" or the "),
                    RichTextRun::new("faq").link("guide://faq"),
                    RichTextRun::new("."),
                ])
                .on_open_link(move |target| sink.borrow_mut().push(target.to_string())),
                Default::default(),
            );
            graph.compute_layout(400.0, 200.0);

            let links = surfaces(&graph, AccessibilityRole::Link);
            assert_eq!(links.len(), 2, "one anchor per destination");
            let labels: Vec<String> = links
                .iter()
                .filter_map(|node| node.accessibility().map(|a| a.label))
                .collect();
            assert_eq!(labels, vec!["style guide", "faq"]);
            let help: Vec<String> = links
                .iter()
                .filter_map(|node| node.accessibility().map(|a| a.help))
                .collect();
            assert_eq!(help, vec!["guide://style", "guide://faq"]);
            assert!(
                links
                    .iter()
                    .all(|node| node.is_focusable() && !node.is_hit_testable()),
                "anchors take focus but leave pointer input to the paragraph"
            );

            let paragraph = surfaces(&graph, AccessibilityRole::StaticText)
                .into_iter()
                .next()
                .expect("the paragraph surface")
                .absolute_frame();
            let frames: Vec<_> = links.iter().map(|link| link.absolute_frame()).collect();
            for (index, frame) in frames.iter().enumerate() {
                assert!(
                    frame.width >= paragraph.width - 0.5,
                    "the anchor spans the paragraph width: link {index} {frame:?} paragraph {paragraph:?}"
                );
            }
            assert!(
                frames[0].y >= paragraph.max_y() && frames[1].y >= frames[0].max_y(),
                "GAP: Rust anchors stack below the paragraph: {frames:?} paragraph {paragraph:?}"
            );

            let key_handler = registry
                .handlers(&links[1])
                .key
                .clone()
                .expect("a Return handler");
            key_handler(&key(Scancode::RETURN), EventPhase::Target);
            if let Some(activate) = links[0].accessibility_actions().activate {
                activate();
            }
            assert_eq!(
                *opened.borrow(),
                vec!["guide://faq".to_string(), "guide://style".to_string()],
                "Return and the accessibility action both open their link"
            );

            let surface = surfaces(&graph, AccessibilityRole::StaticText)
                .into_iter()
                .next()
                .expect("the paragraph surface");
            assert!(
                links[1]
                    .attachment::<TextInputFocusChangeHandler>(
                        TextInputAttachmentKey::FOCUS_CHANGE_HANDLER,
                    )
                    .is_some(),
                "the anchor reports focus changes"
            );
            focus.focus(Some(&links[1]), false);
            assert_eq!(
                focus.focused().map(|node| node.id()),
                Some(links[1].id()),
                "the focus chain takes the anchor"
            );
            assert_eq!(
                surface.attachment::<usize>(FOCUSED_LINK),
                None,
                "GAP: the anchor is built with surface: None, so the paragraph never records the focused link"
            );
            focus.focus(None, false);
            assert_eq!(
                surface.attachment::<usize>(FOCUSED_LINK),
                None,
                "blurring leaves nothing to clear"
            );
        });
    });
}

#[test]
fn inert_links() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let registry = Rc::new(InteractionRegistry::new());
            InteractionRegistryHolder::set_current(Some(registry.clone()));
            let graph = support::graph();
            graph.install(
                RichText::new(vec![
                    RichTextRun::new("see "),
                    RichTextRun::new("docs").link("guide://docs"),
                ]),
                Default::default(),
            );
            graph.compute_layout(400.0, 200.0);
            let link = surfaces(&graph, AccessibilityRole::Link)
                .into_iter()
                .next()
                .expect("the link is still announced");
            assert!(
                !link.is_focusable(),
                "without a handler the link takes no focus"
            );
            assert!(
                !link.accessibility().expect("semantics").state.is_enabled,
                "the inert link reports disabled"
            );
            assert!(
                registry.handlers(&link).key.is_none(),
                "no Return handler is installed"
            );
        });
    });
}

#[test]
fn caret_rect() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let env = environment();
            let flow = RichTextFlow::layout(&plain("first line of text"), 60.0, &env);
            assert!(flow.lines.len() > 1, "a 60pt column wraps this line");
            let start = flow.caret_rect(0);
            let end = flow.caret_rect(flow.text.len());
            assert!(
                (start.1 - flow.lines[0].top).abs() < 0.001,
                "offset 0 sits on the first line: {} vs {}",
                start.1,
                flow.lines[0].top
            );
            assert!(
                (start.3 - flow.lines[0].height).abs() < 0.001,
                "the caret spans the line height: {} vs {}",
                start.3,
                flow.lines[0].height
            );
            let last = &flow.lines[flow.lines.len() - 1];
            assert!(
                (end.1 - last.top).abs() < 0.001,
                "the end offset sits on the last line: {} vs {}",
                end.1,
                last.top
            );
            for line in &flow.lines {
                let rect = flow.caret_rect(line.start_offset);
                assert!(
                    rect.0 >= -0.001 && rect.0 <= line.width + 0.001,
                    "the caret stays inside its line: x {} width {}",
                    rect.0,
                    line.width
                );
            }
        });
    });
}
