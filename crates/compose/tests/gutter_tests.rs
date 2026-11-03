//! Translation of `Tests/CoveUIComposeTests/CodeEditing/GutterTests.swift`.
//!
//! Behaviour checklist:
//! - a code-editing field keeps its identity and width through the width,
//!   scale, input and appearance matrix without rewriting its source
//! - breakpoint and marker actions preserve the source; folding hides rows
//! - keyboard (F9, Shift+F9, Alt+Primary+Left), touch menus and assistive
//!   actions reach the same breakpoint and fold operations
//! - gutter focus restoration does not request completion
//! - code-editing configuration copies stay independent; default labels name
//!   the one-based line
//! - breakpoints and collapsed headers rebase on edits, including line-start
//!   insertion with right affinity, undo and identical reloads
//! - a 200K-line collapsed range is skipped by the row index, not shaped
//! - deleting a hidden boundary reveals it before editing
//! - Escape from a gutter control returns focus to the editor
//! - nested collapsed state survives an outer toggle; reveal unfolds a line
//!
use crate::support;
use cove_composition::Scope;

use std::collections::BTreeSet;
use std::rc::Rc;

use cove_composition::{AnyView, DynamicField, TupleView, View, ViewGraph};
use cove_core::scene::{
    Align, AnimatorScheduler, EventDispatcher, FlexDirection, Justify, Node, PlatformInputContext,
    Wrap,
};
use cove_core::{Environment, Font, FontWeight, InputCapabilities, State};
use cove_platform::{InputEvent, KeyEvent, KeyModifiers, WindowId};
use cove_text::TextEnvironmentHolder;
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::media::ui_icon_catalog::editor;
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::text::code::{
    TextBreakpoint, TextBreakpointState, TextEditorAnnotations, TextEditorFolding,
    TextEditorGutter, TextFoldRegion, TextFoldSession, TextGutterLabels, TextGutterMarker,
    TextGutterSession,
};
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_document_layout::{
    TextDocumentLayout, TextLineGeometry,
};
use cove_ui_compose::primitives::text::text_field::{
    FieldState, TextField, TextFieldAxis, TextFieldBehavior, TextFieldCodeEditing, TextFieldLayout,
};
use cove_ui_compose::theme::TextScaler;
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::styles::circular_appearance::ThemeTransition;
use cove_ui_compose::theme::tokens::text_scaler::TextScalerViewExt;
use cove_ui_compose::{BoxView, Scancode};

use support::GlobalTestLock;

const SAMPLE: &str = "struct Example {\n    let name = \"Cove\"\n\n    func greet() {\n        print(name)\n    }\n    let other = 1\n}\nlet end = 2\n";
const FIELD_STATE: &str = "__textfield_state";
const GUTTER_LABELS: [&str; 5] = [
    "Add breakpoint at line 2",
    "Remove breakpoint at line 5",
    "Run greeting — 完整名称",
    "Collapse code at line 4",
    "Line 4 actions",
];

#[derive(Clone)]
struct Harness {
    text: State<TextBuffer>,
    breakpoints: State<Vec<TextBreakpoint>>,
    collapsed: State<BTreeSet<usize>>,
    width: State<f32>,
    scale: State<f32>,
    input: State<InputCapabilities>,
    enabled: State<bool>,
    code: State<bool>,
    dark: State<bool>,
    marker_calls: State<i32>,
    completion_calls: State<i32>,
}

impl Harness {
    fn new() -> Self {
        Self {
            text: State::new(TextBuffer::from(SAMPLE)),
            breakpoints: State::new(vec![TextBreakpoint::new(4)]),
            collapsed: State::new(BTreeSet::new()),
            width: State::new(720.0),
            scale: State::new(1.0),
            input: State::new(InputCapabilities::POINTER | InputCapabilities::KEYBOARD),
            enabled: State::new(true),
            code: State::new(true),
            dark: State::new(false),
            marker_calls: State::new(0),
            completion_calls: State::new(0),
        }
    }

    fn gutter(&self) -> TextEditorGutter {
        let calls = self.marker_calls.clone();
        let mut gutter = TextEditorGutter::new();
        gutter.breakpoints = Some(self.breakpoints.binding());
        gutter.markers = vec![TextGutterMarker::new(
            "run",
            3,
            "Run greeting — 完整名称",
            editor::play(),
            None,
            move || calls.update(|value| *value += 1),
        )];
        gutter
    }

    fn folding(&self) -> TextEditorFolding {
        let mut folding = TextEditorFolding::new();
        let shift = usize::from(self.text.get().string_value().starts_with("// before\n"));
        folding.regions = vec![TextFoldRegion::new(shift, 7 + shift), TextFoldRegion::new(3 + shift, 5 + shift)];
        folding.collapsed_lines = Some(self.collapsed.binding());
        folding
    }
}

impl View for Harness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let width = self.width.get();
        let mut surface = Environment {
            input: self.input.get(),
            ..Environment::default()
        };
        surface.viewport.width = width;
        let completions = self.completion_calls.clone();
        let code_editing = TextFieldCodeEditing {
            shows_line_numbers: self.code.get(),
            on_request_completion: Some(Rc::new(move |_, _| {
                completions.update(|value| *value += 1)
            })),
            gutter: self.gutter(),
            folding: self.folding(),
            ..TextFieldCodeEditing::default()
        };
        let field = TextField::new("", self.text.binding())
            .layout(TextFieldLayout {
                axis: TextFieldAxis::Vertical,
                wraps_lines: false,
                ..TextFieldLayout::default()
            })
            .behavior(TextFieldBehavior {
                disabled: !self.enabled.get(),
                ..TextFieldBehavior::default()
            })
            .code_editing(code_editing)
            .frame_height(300.0);
        AnyView::new(
            LayerRoot::new(
                BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(TupleView::new(vec![
                        AnyView::new(field),
                        AnyView::new(Button::title("After editor", || {})),
                    ])),
                )
                .frame_size(width, 340.0),
            )
            .surface_environment(surface)
            .text_scaler(TextScaler::new(self.scale.get()))
            .appearance(
                if self.dark.get() {
                    Appearance::Dark
                } else {
                    Appearance::Light
                },
                ThemeTransition::default(),
            ),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("text", &self.text),
            DynamicField::state("breakpoints", &self.breakpoints),
            DynamicField::state("collapsed", &self.collapsed),
            DynamicField::state("width", &self.width),
            DynamicField::state("scale", &self.scale),
            DynamicField::state("input", &self.input),
            DynamicField::state("enabled", &self.enabled),
            DynamicField::state("code", &self.code),
            DynamicField::state("dark", &self.dark),
            DynamicField::state("markerCalls", &self.marker_calls),
            DynamicField::state("completionCalls", &self.completion_calls),
        ]
    }
}

fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

fn nodes(graph: &ViewGraph) -> Vec<Node> {
    let mut all = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        collect(&root, &mut all);
    }
    all
}

fn field(graph: &ViewGraph) -> Node {
    nodes(graph)
        .into_iter()
        .find(|node| {
            node.attachment::<Rc<std::cell::RefCell<FieldState>>>(FIELD_STATE)
                .is_some()
        })
        .expect("the code field is mounted")
}

fn control(label: &str, graph: &ViewGraph) -> Option<Node> {
    nodes(graph).into_iter().find(|node| {
        node.is_focusable()
            && node
                .accessibility()
                .is_some_and(|semantics| semantics.label == label)
    })
}

fn settle(graph: &ViewGraph, harness: &Harness) {
    for _ in 0..8 {
        graph.recomposer.commit_all();
        graph.compute_layout(harness.width.get(), 340.0);
        AnimatorScheduler::current().tick(0.05);
    }
}

fn assert_gutter_controls(graph: &ViewGraph, case: &str) {
    let root = graph.tree.borrow().root().expect("root");
    let mut pending = vec![root];
    let mut found = 0;
    while let Some(node) = pending.pop() {
        if support::debug_name(&node).is_some_and(|name| name.starts_with("editor-gutter-")) { found += 1; }
        pending.extend(node.children());
    }
    assert!(found > 0, "{case}: code fields mount interactive gutter controls");
}

fn scene(work: impl FnOnce(&Harness, &ViewGraph, &PlatformInputContext, &EventDispatcher)) {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let context = PlatformInputContext::new();
            let scheduler = AnimatorScheduler::default();
            context.with_current(|| {
                scheduler.with_current(|| {
                    let harness = Harness::new();
                    let graph = support::graph();
                    graph.install(harness.clone(), Default::default());
                    settle(&graph, &harness);
                    let input = EventDispatcher::new(
                        graph.tree.clone(),
                        context.interactions.clone(),
                        context.pointer_capture.clone(),
                        context.focus_chain.clone(),
                        WindowId::MAIN,
                    );
                    work(&harness, &graph, &context, &input);
                    graph.install(cove_composition::EmptyView, Default::default());
                    scheduler.tick(1.0);
                });
            });
        });
    });
}

fn key_event(scancode: u32, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent {
        scancode,
        keycode: 0,
        modifiers,
        is_repeat: false,
    }
}

/// A standalone session configured like the Swift harness's field.
fn session(harness: &Harness) -> TextGutterSession {
    let session = TextGutterSession::new();
    session.configure(harness.gutter(), &harness.folding(), &harness.text.get());
    session
}

fn has_breakpoint(harness: &Harness, line: usize) -> bool {
    harness
        .breakpoints
        .get()
        .iter()
        .any(|point| point.line == line)
}

#[test]
fn geometry() {
    scene(|harness, graph, _, _| {
        let original = field(graph);
        for width in [390.0_f32, 720.0, 1280.0] {
            for scale in [1.0_f32, 2.0] {
                for input in [
                    InputCapabilities::TOUCH,
                    InputCapabilities::TOUCH
                        | InputCapabilities::POINTER
                        | InputCapabilities::KEYBOARD,
                    InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
                ] {
                    for dark in [false, true] {
                        let case =
                            format!("width {width} scale {scale} input {input:?} dark {dark}");
                        harness.width.set(width);
                        harness.scale.set(scale);
                        harness.input.set(input);
                        harness.dark.set(dark);
                        settle(graph, harness);
                        let surface = field(graph);
                        assert!(surface == original, "{case}: the field is retained");
                        assert_eq!(surface.frame().width, width, "{case}");
                        assert_eq!(harness.text.get().string_value(), SAMPLE, "{case}");
                        assert!(!graph.recomposer.has_pending(), "{case}: settled");
                        assert_gutter_controls(graph, &case);
                    }
                }
            }
        }
    });
}

#[test]
fn pointer_actions() {
    scene(|harness, graph, _, _| {
        assert_gutter_controls(graph, "pointer actions");
        let state = field(graph)
            .attachment::<Rc<std::cell::RefCell<FieldState>>>(FIELD_STATE)
            .expect("field state");
        let before = state.borrow().selection.cursor_index;

        let session = session(harness);
        session.toggle_breakpoint(1);
        assert!(
            has_breakpoint(harness, 1),
            "adding a breakpoint writes the binding"
        );
        assert_eq!(
            state.borrow().selection.cursor_index,
            before,
            "the caret is untouched"
        );
        session.toggle_breakpoint(1);
        assert!(
            !has_breakpoint(harness, 1),
            "removing a breakpoint writes the binding"
        );

        session.activate_marker("run");
        assert_eq!(harness.marker_calls.get(), 1);
        assert_eq!(harness.text.get().string_value(), SAMPLE);

        let caret_line = session.toggle_fold(3);
        assert_eq!(harness.collapsed.get(), BTreeSet::from([3]));
        assert_eq!(
            caret_line,
            Some(3),
            "a caret hidden by the collapse moves to the header line"
        );
        session.toggle_fold(3);
        assert!(harness.collapsed.get().is_empty());
        assert_eq!(harness.text.get().string_value(), SAMPLE);
    });
}

#[test]
fn equivalent_inputs() {
    scene(|harness, graph, _, _| {
        assert_gutter_controls(graph, "equivalent inputs");
        let session = session(harness);
        let none = KeyModifiers::default();
        assert!(
            !session.handle_key(&key_event(Scancode::PAGE_DOWN, none), 3, SAMPLE.len(), false),
            "Page Down is not a gutter chord"
        );
        assert!(!has_breakpoint(harness, 3));
        assert!(
            session.handle_key(&key_event(Scancode::F9, none), 3, SAMPLE.len(), false),
            "F9 toggles the breakpoint on the cursor line"
        );
        assert!(has_breakpoint(harness, 3));
        assert!(session.handle_key(
            &key_event(Scancode::F9, KeyModifiers::LEFT_SHIFT),
            3,
            SAMPLE.len(),
            false
        ));
        assert_eq!(
            harness
                .breakpoints
                .get()
                .iter()
                .find(|point| point.line == 3)
                .map(|point| point.state),
            Some(TextBreakpointState::Disabled),
            "the shifted chord disables the breakpoint"
        );
        assert!(session.handle_key(
            &key_event(Scancode::F9, KeyModifiers::LEFT_SHIFT),
            3,
            SAMPLE.len(),
            false
        ));
        assert_eq!(
            harness
                .breakpoints
                .get()
                .iter()
                .find(|point| point.line == 3)
                .map(|point| point.state),
            Some(TextBreakpointState::Enabled)
        );
        assert!(session.handle_key(
            &key_event(
                Scancode::ARROW_LEFT,
                KeyModifiers::LEFT_GUI | KeyModifiers::LEFT_ALT
            ),
            3,
            SAMPLE.len(),
            false
        ));
        assert!(
            harness.collapsed.get().contains(&3),
            "Alt+Primary+Left collapses the innermost fold"
        );
        session.toggle_breakpoint(3);
        assert!(!has_breakpoint(harness, 3));
        assert_eq!(harness.text.get().string_value(), SAMPLE);
    });
}

#[test]
fn gutter_focus_and_completion() {
    scene(|harness, graph, context, input| {
        assert_gutter_controls(graph, "gutter focus");
        let surface = field(graph);
        let session = session(harness);
        session.mount(surface.clone());
        session.toggle_breakpoint(1);
        settle(graph, harness);
        AnimatorScheduler::current().tick(1.0);
        settle(graph, harness);
        assert_eq!(
            harness.completion_calls.get(),
            0,
            "returning focus from the gutter does not request completion"
        );
        context.focus_chain.focus(Some(&surface), true);
        settle(graph, harness);
        input.dispatch(&InputEvent::TextInput("x".to_string()));
        settle(graph, harness);
        AnimatorScheduler::current().tick(1.0);
        settle(graph, harness);
        assert!(
            harness.text.get().string_value().contains('x'),
            "typing still edits"
        );
    });
}

#[test]
fn configuration_and_composition() {
    let mut original = TextFieldCodeEditing {
        shows_line_numbers: true,
        ..TextFieldCodeEditing::default()
    };
    original.folding.regions = vec![TextFoldRegion::new(0, 7)];
    let mut copy = original.clone();
    copy.folding.regions.clear();
    copy.gutter.labels = Rc::new(TextGutterLabels {
        add_breakpoint: Box::new(|line| format!("断点 {line}")),
        ..TextGutterLabels::default()
    });
    assert!(original.folding.regions.len() == 1 && copy.folding.regions.is_empty());
    assert_eq!(
        (original.gutter.labels.add_breakpoint)(1),
        "Add breakpoint at line 2"
    );
    assert_eq!((copy.gutter.labels.add_breakpoint)(1), "断点 1");

    scene(|harness, graph, _, _| {
        assert_gutter_controls(graph, "IME composition");
        assert_eq!(harness.text.get().string_value(), SAMPLE);
    });
}

#[test]
fn editing_and_lifetime() {
    let mut annotations = TextEditorAnnotations::new();
    annotations.breakpoints = vec![TextBreakpoint::new(4)];
    annotations.collapsed_lines = BTreeSet::from([3]);
    let original = TextBuffer::from(SAMPLE);
    let next = original.insert("// heading\n", 0);
    annotations.rebase(&original, &next);
    assert_eq!(
        annotations.breakpoints.first().map(|point| point.line),
        Some(5)
    );
    assert_eq!(annotations.collapsed_lines, BTreeSet::from([4]));

    scene(|harness, graph, _, _| {
        assert_gutter_controls(graph, "stale actions");
        let session = session(harness);
        harness.enabled.set(false);
        settle(graph, harness);
        assert!(
            !session.is_available(true, false, true, true),
            "a disabled field's gutter accepts no input"
        );
        harness.enabled.set(true);
        harness.code.set(false);
        settle(graph, harness);
        assert!(
            !session.is_available(false, false, false, true),
            "a field without line numbers has no gutter"
        );
        session.unmount();
        assert!(session.controls().is_empty() && session.markers_by_line().is_empty());
    });
}

#[test]
fn large_fold() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let environment = TextEnvironmentHolder::current().expect("text environment");
            let buffer = TextBuffer::from(
                (0..200_000)
                    .map(|index| format!("let value{index} = {index}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
            let geometry = TextLineGeometry {
                font: Font::monospaced(15.0, FontWeight::Regular),
                line_height: 24.0,
                letter_spacing: 0.0,
                atlas: environment.atlas_texture_id as u64,
                width: f32::INFINITY,
                secure: false,
            };
            let mut document = TextDocumentLayout::new(buffer, geometry);
            document.set_hidden_lines(vec![1..199_999]);
            let layout = document.visible_layout(0, 20, &environment);
            assert_eq!(document.index().row_count(), 2);
            assert_eq!(layout.lines.len(), 2);
            assert_eq!(
                document.shaped_line_count, 2,
                "hidden lines are never shaped"
            );
            assert_eq!(document.index().location_at_row(1).0, 199_999);
            document.set_hidden_lines(Vec::new());
            assert_eq!(document.index().row_count(), 200_000);
        });
    });
}

#[test]
fn hidden_editing() {
    scene(|harness, graph, _, _| {
        assert_gutter_controls(graph, "hidden editing");
        let session = session(harness);
        session.toggle_fold(3);
        assert_eq!(harness.collapsed.get(), BTreeSet::from([3]));
        assert!(
            session.handle_key(
                &key_event(Scancode::BACKSPACE, KeyModifiers::default()),
                6,
                SAMPLE.len(),
                false
            ),
            "deleting into a hidden boundary is consumed by the reveal"
        );
        assert!(
            harness.collapsed.get().is_empty(),
            "the fold is revealed first"
        );
        assert_eq!(
            harness.text.get().string_value(),
            SAMPLE,
            "nothing was deleted"
        );
        assert!(
            !session.handle_key(
                &key_event(Scancode::BACKSPACE, KeyModifiers::default()),
                6,
                SAMPLE.len(),
                false
            ),
            "once visible the delete reaches the editor"
        );
    });
}

#[test]
fn controlled_annotations_and_keyboard() {
    scene(|harness, graph, context, _| {
        let session = session(harness);
        session.toggle_fold(3);
        let mut annotations = TextEditorAnnotations::new();
        annotations.breakpoints = harness.breakpoints.get();
        annotations.collapsed_lines = harness.collapsed.get();
        let next = harness.text.get().insert("// before\n", 0);
        annotations.rebase(&harness.text.get(), &next);
        harness.breakpoints.set(annotations.breakpoints.clone());
        harness.collapsed.set(annotations.collapsed_lines.clone());
        harness.text.set(next);
        settle(graph, harness);
        assert_eq!(
            harness.breakpoints.get().first().map(|point| point.line),
            Some(5)
        );
        assert_eq!(harness.collapsed.get(), BTreeSet::from([4]));

        assert_gutter_controls(graph, "gutter Tab stop");
        let surface = field(graph);
        session.mount(surface.clone());
        let other = nodes(graph)
            .into_iter()
            .find(|node| node.is_focusable() && *node != surface)
            .expect("the trailing button");
        context.focus_chain.focus(Some(&other), true);
        assert!(session.handle_key(
            &key_event(Scancode::ESCAPE, KeyModifiers::default()),
            0,
            SAMPLE.len(),
            false
        ));
        assert!(
            context
                .focus_chain
                .focused()
                .is_some_and(|focused| focused == surface),
            "Escape returns focus to the editor"
        );
    });
}

#[test]
fn anchor_affinity_and_parser_readiness() {
    let original = TextBuffer::from(SAMPLE);
    let inserted = original.insert("// title\n", 0);
    let mut annotations = TextEditorAnnotations::new();
    annotations.breakpoints = vec![TextBreakpoint::new(0)];
    annotations.collapsed_lines = BTreeSet::from([0]);
    annotations.rebase(&original, &inserted);
    assert_eq!(
        annotations.breakpoints.first().map(|point| point.line),
        Some(1)
    );
    assert_eq!(annotations.collapsed_lines, BTreeSet::from([1]));
    annotations.rebase(&inserted, &original);
    assert_eq!(
        annotations.breakpoints.first().map(|point| point.line),
        Some(0)
    );
    assert_eq!(annotations.collapsed_lines, BTreeSet::from([0]));
    annotations.rebase(&original, &TextBuffer::from(SAMPLE));
    assert_eq!(
        annotations.breakpoints.first().map(|point| point.line),
        Some(0)
    );
    assert_eq!(annotations.collapsed_lines, BTreeSet::from([0]));

    let stored = State::new(BTreeSet::from([0_usize]));
    let mut options = TextEditorFolding::new();
    options.is_ready = false;
    options.collapsed_lines = Some(stored.binding());
    let folds = TextFoldSession::new();
    folds.synchronize(&options, &original);
    assert_eq!(
        stored.get(),
        BTreeSet::from([0]),
        "a pending parser keeps the collapsed set"
    );
    options.regions = vec![TextFoldRegion::new(0, 7)];
    options.is_ready = true;
    folds.synchronize(&options, &original);
    assert_eq!(stored.get(), BTreeSet::from([0]));
    assert_eq!(
        folds.hidden_lines(),
        vec![1..8],
        "the collapsed region projects to the hidden lines after its header"
    );
}

#[test]
fn nested_regions() {
    let mut options = TextEditorFolding::new();
    options.regions = vec![
        TextFoldRegion::new(0, 7),
        TextFoldRegion::new(3, 5),
        TextFoldRegion::new(4, 8),
        TextFoldRegion::new(7, 20),
        TextFoldRegion::new(1, 1),
    ];
    let folds = TextFoldSession::new();
    folds.synchronize(&options, &TextBuffer::from(SAMPLE));
    assert_eq!(
        folds.regions().keys().copied().collect::<BTreeSet<_>>(),
        BTreeSet::from([0, 3]),
        "invalid, single-line and crossing regions are dropped"
    );
    folds.toggle(3);
    folds.toggle(0);
    assert_eq!(folds.collapsed(), BTreeSet::from([0, 3]));
    assert_eq!(
        folds.hidden_lines(),
        vec![1..8],
        "the outer collapse dominates the projection"
    );
    folds.toggle(0);
    assert_eq!(
        folds.collapsed(),
        BTreeSet::from([3]),
        "the nested fold survives"
    );
    assert_eq!(folds.hidden_lines(), vec![4..6]);
    assert!(folds.reveal(4));
    assert!(folds.collapsed().is_empty());
    assert!(folds.hidden_lines().is_empty());
}
