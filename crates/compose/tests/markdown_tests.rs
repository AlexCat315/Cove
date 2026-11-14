//! Translation of `Tests/CoveUIComposeTests/MarkdownTests.swift`.
//!
//! Behaviour inventory:
//! - inline emphasis, code, strikethrough and links become styled runs; nested
//!   `**bold *both***` unions the traits; adjacent plain text is merged
//! - soft breaks join into a space, hard breaks keep a newline
//! - block structure: headings, quotes, ordered lists with a nested bullet list,
//!   GFM task lists, fenced code with its language, rules and tables with
//!   alignment, header cells and rows
//! - equal sources reuse one parse
//! - headings are exposed as headings and are taller than body text
//! - a long code line keeps its width and scrolls inside a scroll area
//! - paragraphs wrap to the container and task items expose a read-only,
//!   selected checkbox
//! - table columns fit their widest cell and rows share their height
//! - word boundaries cover Latin words, lone punctuation and CJK text
//!
use crate::support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, EmptyView, ViewGraph};
use cove_core::scene::ClipboardHolder;
use cove_core::scene::{
    AccessibilityRole, EventPhase, EventResult, FocusChain, FocusChainHolder, InteractionRegistry,
    InteractionRegistryHolder, Node, PointerCapture, PointerCaptureHolder, PointerPhase,
};
use cove_core::{Environment, InputCapabilities};
use cove_platform::{KeyEvent, KeyModifiers, MouseButton, MouseButtonEvent, MouseMotionEvent};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::primitives::text::markdown::{
    MarkdownAlignment, MarkdownBlock, MarkdownView, blocks, runs,
};
use cove_ui_compose::primitives::text::rich::run::{TextRun, TextTraits};
use cove_ui_compose::primitives::text::rich::text_selection::{
    SharedTextSelectionScope, TextBoundaries, TextSelectionRoot,
};

use support::GlobalTestLock;

fn run_with(text: &str, traits: TextTraits) -> TextRun {
    TextRun::with_traits(text, traits)
}

#[test]
fn inline_runs() {
    let parsed = blocks("A **bold *both*** `code` ~~gone~~ [site](https://a.b) <br>end");
    let Some(MarkdownBlock::Paragraph(paragraph)) = parsed.first() else {
        panic!("expected a paragraph, got {:?}", parsed.first());
    };
    assert!(
        paragraph.contains(&run_with("code", TextTraits::CODE)),
        "inline code keeps the code trait, got {paragraph:?}"
    );
    assert!(
        paragraph.iter().any(|run| run.text == "<br>"),
        "inline HTML survives as a run, got {paragraph:?}"
    );
    let plain = &paragraph[0];
    assert_eq!(
        plain.text, "A ",
        "the plain lead-in ends where the strong span opens, got {paragraph:?}"
    );
    assert_eq!(
        plain.traits,
        TextTraits::from_raw(0),
        "the lead-in carries no traits"
    );
    let bold = paragraph
        .iter()
        .find(|run| run.text.contains("bold"))
        .expect("the bold word survives as text");
    assert_eq!(
        bold.traits,
        TextTraits::BOLD,
        "the strong trait survives the block parser"
    );
    let both = paragraph
        .iter()
        .find(|run| run.text == "both")
        .expect("the nested word survives");
    assert_eq!(
        both.traits,
        TextTraits::BOLD.union(TextTraits::ITALIC),
        "nested emphasis inside strong keeps both traits"
    );
    let struck = paragraph
        .iter()
        .find(|run| run.text.contains("gone"))
        .expect("the struck word survives as text");
    assert_eq!(
        struck.traits,
        TextTraits::STRIKETHROUGH,
        "strikethrough survives the block parser"
    );
    let link = paragraph
        .iter()
        .find(|run| run.text.contains("site"))
        .expect("the link label survives as text");
    assert_eq!(
        link.link.as_deref(),
        Some("https://a.b"),
        "the link destination survives the block parser"
    );
    assert!(
        !paragraph
            .windows(2)
            .any(|pair| pair[0].traits == pair[1].traits && pair[0].link == pair[1].link),
        "adjacent plain text is merged rather than shaped piece by piece, got {paragraph:?}"
    );
}

#[test]
fn breaks() {
    let soft = blocks("one\ntwo");
    let hard = blocks("one  \ntwo");
    let Some(MarkdownBlock::Paragraph(soft)) = soft.first() else {
        panic!("expected a soft-break paragraph");
    };
    let Some(MarkdownBlock::Paragraph(hard)) = hard.first() else {
        panic!("expected a hard-break paragraph");
    };
    let soft_text: String = soft.iter().map(|run| run.text.as_str()).collect();
    let hard_text: String = hard.iter().map(|run| run.text.as_str()).collect();
    assert_eq!(soft_text, "one two", "a soft break joins with a space");
    assert_eq!(hard_text, "one\ntwo", "a hard break keeps the newline");
}

#[test]
fn block_structure() {
    let source = "# Title\n## Sub\n\n> quoted\n\n3. three\n4. four\n   - nested\n\n- [x] done\n- [ ] todo\n\n```swift\nlet x = 1\n```\n\n---\n\n| A | B |\n| :-: | --: |\n| 1 | 2 |\n";
    let parsed = blocks(source);
    assert_eq!(parsed.len(), 8, "eight blocks, got {parsed:?}");
    assert_eq!(
        parsed[0],
        MarkdownBlock::Heading {
            level: 1,
            runs: vec![TextRun::new("Title")],
        },
        "the first heading"
    );
    assert_eq!(
        parsed[1],
        MarkdownBlock::Heading {
            level: 2,
            runs: vec![TextRun::new("Sub")],
        },
        "the second heading"
    );
    assert_eq!(
        parsed[2],
        MarkdownBlock::Quote(vec![MarkdownBlock::paragraph("quoted")]),
        "a quote holds its paragraph"
    );

    let MarkdownBlock::List(ordered) = &parsed[3] else {
        panic!("expected the ordered list, got {:?}", parsed[3]);
    };
    let MarkdownBlock::List(tasks) = &parsed[4] else {
        panic!("expected the task list, got {:?}", parsed[4]);
    };
    let MarkdownBlock::Table(table) = &parsed[7] else {
        panic!("expected the table, got {:?}", parsed[7]);
    };
    assert_eq!(ordered.start, Some(3), "the ordered list starts at 3");
    assert_eq!(ordered.items.len(), 2, "two ordered items");
    assert_eq!(
        ordered.items[0].blocks.len(),
        1,
        "a tight list item keeps its own text, got items {:?}",
        ordered.items
    );
    assert_eq!(
        ordered.items[1].blocks[0],
        MarkdownBlock::paragraph("four"),
        "a tight item's text stays in the item, ahead of its nested list"
    );
    let MarkdownBlock::List(nested) = &ordered.items[1].blocks[1] else {
        panic!("expected a nested list, got {:?}", ordered.items[1].blocks);
    };
    assert_eq!(
        nested.items[0].blocks,
        vec![MarkdownBlock::paragraph("nested")],
        "the nested item holds only its own text"
    );
    assert_eq!(nested.start, None, "the nested list is bulleted");
    assert_eq!(
        tasks
            .items
            .iter()
            .map(|item| item.is_checked)
            .collect::<Vec<_>>(),
        vec![Some(true), Some(false)],
        "task items report their checkbox state"
    );
    assert_eq!(
        parsed[5],
        MarkdownBlock::Code {
            language: Some("swift".to_string()),
            text: "let x = 1".to_string(),
        },
        "the fenced block keeps its language and drops the trailing newline"
    );
    assert_eq!(parsed[6], MarkdownBlock::Rule, "the thematic break");
    assert_eq!(
        table.alignments,
        vec![MarkdownAlignment::Center, MarkdownAlignment::Trailing],
        "the delimiter row sets the alignments"
    );
    assert_eq!(table.header.len(), 2, "two header cells");
    assert_eq!(
        table.rows,
        vec![vec![vec![TextRun::new("1")], vec![TextRun::new("2")]]],
        "the body row holds one run list per cell"
    );
}

#[test]
fn equal_sources_reuse_one_parse() {
    let source = format!("cache *me* {}", std::process::id());
    assert_eq!(
        blocks(&source),
        blocks(&source),
        "the same source parses to the same blocks"
    );
    assert!(!blocks(&source).is_empty(), "the source parses");
}

fn nodes(graph: &ViewGraph) -> Vec<Node> {
    fn visit(node: &Node, into: &mut Vec<Node>) {
        into.push(node.clone());
        for child in node.children() {
            visit(&child, into);
        }
    }
    let mut collected = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        visit(&root, &mut collected);
    }
    collected
}

fn render(source: &str, width: f32, check: impl FnOnce(&[Node])) {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = support::graph();
            graph.install_view(AnyView::new(MarkdownView::new(source)), Default::default());
            graph.compute_layout(width, 2000.0);
            let collected = nodes(&graph);
            check(&collected);
            graph.install_view(AnyView::new(EmptyView), Default::default());
        });
    });
}

fn label(node: &Node) -> String {
    node.accessibility()
        .map(|semantics| semantics.label)
        .unwrap_or_default()
}

fn role(node: &Node) -> Option<AccessibilityRole> {
    node.accessibility().map(|semantics| semantics.role)
}

#[test]
fn headings() {
    render("# Big\n\nbody", 360.0, |mounted| {
        let headings: Vec<&Node> = mounted
            .iter()
            .filter(|node| role(node) == Some(AccessibilityRole::Heading))
            .collect();
        let heading = headings.first().expect("the heading is announced");
        let body = mounted
            .iter()
            .find(|node| label(node) == "body")
            .expect("the body paragraph is mounted");
        assert_eq!(label(heading), "Big", "the heading carries its text");
        let frames: Vec<_> = headings.iter().map(|node| node.absolute_frame()).collect();
        assert!(
            headings
                .iter()
                .any(|node| node.absolute_frame().height > body.absolute_frame().height),
            "a heading is taller than body text; headings {frames:?} body {:?}",
            body.absolute_frame()
        );
    });
}

#[test]
fn thematic_break_keeps_its_air_outside_the_hairline() {
    render("---\n\ntext", 320.0, |mounted| {
        let theme = cove_ui_compose::theme::theme_for_node(&mounted[0]);
        let rule = mounted
            .iter()
            .find(|node| node.background_color() == Some(theme.colors.divider))
            .expect("the thematic break draws a divider");
        assert_eq!(
            rule.frame().height,
            1.0,
            "the rule stays a one-point hairline"
        );
        let band = rule
            .parent()
            .expect("the rule sits inside the section band it breaks");
        assert_eq!(
            band.frame().height,
            rule.frame().height + 8.0,
            "the block's air belongs to the wrapper, saw {:?}",
            band.frame()
        );
    });
}

#[test]
fn code_scrolls() {
    let line = "x".repeat(200);
    let source = format!("```\n{line}\n```");
    render(&source, 320.0, |mounted| {
        let code = mounted
            .iter()
            .find(|node| label(node) == line)
            .expect("the code line is mounted");
        let frame = code.absolute_frame();
        assert!(
            frame.width > 320.0,
            "code keeps its unwrapped intrinsic width: {frame:?}"
        );
        let lines = code.layout_node().expect("code layout").frame();
        assert!(lines.height > 0.0, "code has a measured line height");
        assert!(
            mounted
                .iter()
                .any(|node| role(node) == Some(AccessibilityRole::ScrollArea)),
            "the code block scrolls sideways"
        );
    });
}

#[test]
fn wraps_and_tasks() {
    render(
        "word word word word word word word word word word word word\n\n- [x] done",
        200.0,
        |mounted| {
            let paragraph = mounted
                .iter()
                .find(|node| label(node).starts_with("word"))
                .expect("the paragraph is mounted");
            let frame = paragraph.absolute_frame();
            assert!(
                frame.width <= 200.0,
                "the paragraph wraps to the container, got {}",
                frame.width
            );
            assert!(
                frame.height > 20.0,
                "a wrapped paragraph is taller than one line, got {}",
                frame.height
            );
            let checkbox = mounted
                .iter()
                .find(|node| role(node) == Some(AccessibilityRole::Checkbox))
                .expect("the task item exposes a checkbox");
            let state = checkbox.accessibility().expect("checkbox semantics").state;
            assert!(state.is_selected, "the checked task reports a selected box");
            assert!(
                state.is_read_only,
                "the task box is read-only in rendered Markdown"
            );
        },
    );
}

#[test]
fn table_columns() {
    render(
        "| A | Longer header |\n| --- | --- |\n| 1 | 2 |",
        600.0,
        |mounted| {
            let cells: Vec<Node> = mounted
                .iter()
                .filter(|node| matches!(label(node).as_str(), "A" | "Longer header" | "1" | "2"))
                .cloned()
                .collect();
            assert_eq!(cells.len(), 4, "four cells are mounted, got {cells:?}");
            let cell = |text: &str| {
                cells
                    .iter()
                    .find(|node| label(node) == text)
                    .expect("the cell is present")
                    .absolute_frame()
            };
            let a = cell("A");
            let one = cell("1");
            let header = cell("Longer header");
            assert!(
                (a.x - one.x).abs() < 0.5,
                "both cells share the first column, A {} one {}",
                a.x,
                one.x
            );
            assert!(
                header.x > a.max_x(),
                "the second column starts after the first, header {} first {}",
                header.x,
                a.max_x()
            );
            assert!(
                one.y > a.max_y() - 0.5,
                "the body row sits below the header, one {} A {}",
                one.y,
                a.max_y()
            );
        },
    );
}

#[test]
fn heading_paragraph_and_table_links_report_opens() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = support::graph();
            let opened: Rc<RefCell<Vec<String>>> = Rc::default();
            let sink = opened.clone();
            let source = concat!(
                "# [Title](https://example.com/title)\n\n",
                "See [docs](https://example.com/docs).\n\n",
                "| [cell](https://example.com/cell) |\n| ---- |\n| plain |\n",
            );
            graph.install_view(
                AnyView::new(
                    MarkdownView::new(source)
                        .on_open_link(move |link| sink.borrow_mut().push(link.to_string())),
                ),
                Default::default(),
            );
            graph.compute_layout(320.0, 2000.0);
            let collected = nodes(&graph);
            let links: Vec<Node> = collected
                .iter()
                .filter(|node| role(node) == Some(AccessibilityRole::Link))
                .cloned()
                .collect();
            assert_eq!(
                links.len(),
                3,
                "heading, paragraph and table cells each expose a link anchor"
            );
            for node in &links {
                node.accessibility_actions()
                    .activate
                    .as_ref()
                    .expect("a link anchor with an opener is actionable")();
            }
            let urls = opened.borrow();
            assert!(
                urls.contains(&"https://example.com/title".to_string()),
                "the heading link reports: {urls:?}"
            );
            assert!(
                urls.contains(&"https://example.com/docs".to_string()),
                "the paragraph link reports: {urls:?}"
            );
            assert!(
                urls.contains(&"https://example.com/cell".to_string()),
                "the table cell link reports: {urls:?}"
            );
            graph.install_view(AnyView::new(EmptyView), Default::default());
        });
    });
}

struct Document {
    graph: Rc<ViewGraph>,
    registry: Rc<InteractionRegistry>,
    focus: Rc<FocusChain>,
    capture: Rc<PointerCapture>,
    scope: SharedTextSelectionScope,
    clipboard: Rc<RefCell<Vec<String>>>,
}

impl Document {
    fn nodes(&self) -> Vec<Node> {
        nodes(&self.graph)
    }

    /// The hit-testable paragraph surface carrying `text`.
    fn paragraph(&self, text: &str) -> Option<Node> {
        self.nodes().into_iter().find(|node| {
            node.is_hit_testable()
                && node
                    .accessibility()
                    .is_some_and(|semantics| semantics.label == text)
        })
    }

    fn copied(&self) -> String {
        self.clipboard.borrow().join("")
    }
}

fn with_document(source: &str, input: InputCapabilities, body: impl FnOnce(&Document)) {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let previous_registry = InteractionRegistryHolder::current();
            let previous_focus = FocusChainHolder::current();
            let previous_capture = PointerCaptureHolder::current();
            let registry = Rc::new(InteractionRegistry::new());
            let focus = Rc::new(FocusChain::new());
            let capture = Rc::new(PointerCapture::new());
            InteractionRegistryHolder::set_current(Some(registry.clone()));
            FocusChainHolder::set_current(Some(focus.clone()));
            PointerCaptureHolder::set_current(Some(capture.clone()));
            let clipboard = Rc::new(RefCell::new(Vec::<String>::new()));
            let sink = clipboard.clone();
            ClipboardHolder::set_write(Some(Rc::new(move |text: &str| {
                sink.borrow_mut().push(text.to_string());
            })));

            let mut environment = Environment::default();
            environment.input = input;
            environment.viewport.width = 400.0;
            environment.viewport.height = 800.0;
            let selection = TextSelectionRoot::new(AnyView::new(
                MarkdownView::new(source).surface_environment(environment),
            ));
            let scope = selection.scope();
            let graph = support::graph();
            graph.install_view(AnyView::new(selection), Default::default());
            graph.compute_layout(400.0, 800.0);

            body(&Document {
                graph: graph.clone(),
                registry,
                focus,
                capture,
                scope,
                clipboard,
            });

            graph.install_view(AnyView::new(EmptyView), Default::default());
            ClipboardHolder::set_write(None);
            InteractionRegistryHolder::set_current(previous_registry);
            FocusChainHolder::set_current(previous_focus);
            PointerCaptureHolder::set_current(previous_capture);
        });
    });
}

fn press(x: f32, y: f32, clicks: u8, modifiers: KeyModifiers) -> MouseButtonEvent {
    MouseButtonEvent {
        button: MouseButton::Left,
        x,
        y,
        clicks,
        modifiers,
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

#[test]
fn drag_across_blocks_copies_in_reading_order() {
    with_document(
        "# Title\n\nFirst paragraph.\n\nSecond one.",
        InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
        |document| {
            let title = document.paragraph("Title").unwrap();
            let second = document.paragraph("Second one.").unwrap();
            let handlers = document.registry.handlers(&title);
            let pointer = handlers.pointer.unwrap();
            let start = title.absolute_frame();
            let end = second.absolute_frame();
            assert_eq!(
                pointer(
                    &press(
                        start.x + 1.0,
                        start.y + start.height / 2.0,
                        1,
                        KeyModifiers::default()
                    ),
                    PointerPhase::Down,
                    EventPhase::Target
                ),
                EventResult::Handled
            );
            assert_eq!(document.focus.focused(), Some(title.clone()));
            assert_eq!(
                handlers.motion.unwrap()(
                    &motion(end.x + 40.0, end.y + end.height / 2.0),
                    EventPhase::Target
                ),
                EventResult::Handled
            );
            pointer(
                &press(
                    end.x + 40.0,
                    end.y + end.height / 2.0,
                    1,
                    KeyModifiers::default(),
                ),
                PointerPhase::Up,
                EventPhase::Target,
            );
            assert!(document.capture.target().is_none());
            let selected = document.scope.borrow_mut().selected_text();
            assert!(
                selected.starts_with("Title\nFirst paragraph.\n"),
                "selection follows document reading order: {selected:?}"
            );
            document.registry.handlers(&title).key.unwrap()(
                &KeyEvent {
                    scancode: cove_ui_compose::Scancode::C,
                    keycode: 0,
                    modifiers: KeyModifiers::LEFT_CTRL,
                    is_repeat: false,
                },
                EventPhase::Target,
            );
            assert_eq!(document.copied(), selected);
        },
    );
}

#[test]
fn click_granularity() {
    with_document(
        "alpha beta gamma\n\nnext",
        InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
        |document| {
            let paragraph = document.paragraph("alpha beta gamma").unwrap();
            let handlers = document.registry.handlers(&paragraph);
            let pointer = handlers.pointer.unwrap();
            let frame = paragraph.absolute_frame();
            for clicks in [2, 3] {
                for phase in [PointerPhase::Down, PointerPhase::Up] {
                    pointer(
                        &press(
                            frame.x + 5.0,
                            frame.y + frame.height / 2.0,
                            clicks,
                            KeyModifiers::default(),
                        ),
                        phase,
                        EventPhase::Target,
                    );
                }
                assert_eq!(
                    document.scope.borrow_mut().selected_text(),
                    if clicks == 2 {
                        "alpha"
                    } else {
                        "alpha beta gamma"
                    }
                );
            }
            let key = document.registry.handlers(&paragraph).key.unwrap();
            key(
                &KeyEvent {
                    scancode: cove_ui_compose::Scancode::C,
                    keycode: 0,
                    modifiers: KeyModifiers::LEFT_CTRL,
                    is_repeat: false,
                },
                EventPhase::Target,
            );
            assert_eq!(document.copied(), "alpha beta gamma");
            key(
                &KeyEvent {
                    scancode: cove_ui_compose::Scancode::A,
                    keycode: 0,
                    modifiers: KeyModifiers::LEFT_CTRL,
                    is_repeat: false,
                },
                EventPhase::Target,
            );
            assert_eq!(
                document.scope.borrow_mut().selected_text(),
                "alpha beta gamma\nnext"
            );
            key(
                &KeyEvent {
                    scancode: cove_ui_compose::Scancode::ESCAPE,
                    keycode: 0,
                    modifiers: KeyModifiers::default(),
                    is_repeat: false,
                },
                EventPhase::Target,
            );
            assert!(!document.scope.borrow().has_selection());
        },
    );
}

#[test]
fn touch_long_press_selects_the_word() {
    with_document("alpha beta", InputCapabilities::TOUCH, |document| {
        let paragraph = document.paragraph("alpha beta").unwrap();
        let pointer = document.registry.handlers(&paragraph).pointer.unwrap();
        let frame = paragraph.absolute_frame();
        let mut point = press(
            frame.x + 4.0,
            frame.y + frame.height / 2.0,
            1,
            KeyModifiers::default(),
        );
        point.kind = cove_platform::PointerKind::Touch;
        assert_eq!(
            pointer(&point, PointerPhase::Down, EventPhase::Target),
            EventResult::Ignored
        );
        assert_eq!(
            pointer(&point, PointerPhase::Up, EventPhase::Target),
            EventResult::Ignored
        );
        assert!(
            !document.scope.borrow().has_selection(),
            "quick taps leave scrolling and links available"
        );
        pointer(&point, PointerPhase::Down, EventPhase::Target);
        paragraph.set_attachment(
            "__text_selection_touch",
            (
                std::time::Instant::now() - std::time::Duration::from_millis(500),
                cove_core::Point::new(point.x, point.y),
            ),
        );
        assert_eq!(
            pointer(&point, PointerPhase::Up, EventPhase::Target),
            EventResult::Handled
        );
        assert_eq!(document.scope.borrow_mut().selected_text(), "alpha");
        assert_eq!(document.focus.focused(), Some(paragraph));
    });
}

#[test]
fn word_boundaries() {
    assert_eq!(
        TextBoundaries::word("hello world", 7),
        6..11,
        "an offset inside the second word selects it whole"
    );
    assert_eq!(
        TextBoundaries::word("hello world", 11),
        6..11,
        "an offset at the very end still selects the last word"
    );
    assert_eq!(
        TextBoundaries::word("a, b", 1),
        1..2,
        "a lone punctuation mark selects itself"
    );
    assert!(
        !TextBoundaries::word("中文分词", 0).is_empty(),
        "a CJK offset yields a non-empty range"
    );
}

#[test]
fn runs_parser_matches_blocks_paragraph() {
    let inline = runs("A **bold** word");
    let MarkdownBlock::Paragraph(paragraph) = &blocks("A **bold** word")[0] else {
        panic!("expected a paragraph");
    };
    assert!(
        inline.contains(&run_with("bold", TextTraits::BOLD)),
        "the inline entry point keeps the strong trait, got {inline:?}"
    );
    assert_eq!(
        paragraph, &inline,
        "the block parser shapes the same runs as the inline entry point, got {paragraph:?}"
    );
}
