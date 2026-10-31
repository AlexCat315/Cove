//! Translation of `Tests/CoveUIComposeTests/DisclosureContainerTests.swift`.
//!
//! Behaviour checklist:
//! - the accordion keeps scaled titles single-line, publishes expansion
//!   semantics for every header, and skips the disabled header in the roving
//!   focus order
//! - arrow-down moves focus to the last enabled header and Return expands it
//!   while collapsing the previously expanded one
//! - closing a section clears focused editing immediately, while the exit paint
//!   is still mounted
//! - disclosure and panel headers fit one scaled title line and preserve
//!   accessories
//! - tree virtualization, row hit regions and disclosure slots share the same
//!   scaled geometry
//!
//! Named differences:
//!
//! - **No `accordion-draft` / `disclosure-label` / `disclosure-header` /
//!   `panel-action` / `tree-text-N` debug names.** The Rust `Accordion`,
//!   `DisclosureGroup`, `PanelStyle` and `Tree` do not publish those
//!   `LayoutDebugAttachmentKey.debugName` attachments. Each test locates the
//!   same node by the contract that *is* published: accessibility roles
//!   (`Button` with `is_expanded`, `TextField`, `TreeItem`), the measured text
//!   input for a title, and the font/glyph width for the tree's disclosure icon.
//!   The Swift assertions themselves are unchanged.
//! - **`Tree` takes an explicit `TreeOptions` rather than a `configure` block**,
//!   and `TreeSelection::primary` is a `Binding<Option<Id>>`. The harness builds
//!   the same configuration through those fields.

mod support;

use std::cell::Cell;
use std::collections::{BTreeSet, HashSet};
use std::rc::Rc;

use cove_composition::{AnyView, LayoutDebugExt, View, ViewExt};
use cove_core::scene::{
    AccessibilityRole, Align, EventDispatcher, FlexDirection, FocusChain, InteractionRegistry,
    InteractionRegistryHolder, Justify, Node, PointerCapture, Wrap,
};
use cove_core::{Binding, State};
use cove_platform::{
    InputEvent, KeyEvent, KeyModifiers, MouseButton, MouseButtonEvent, WindowId,
};
use cove_ui_compose::Scancode;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::data::lists::tree::{Tree, TreeOptions, TreeSelection};
use cove_ui_compose::primitives::layout::{Accordion, AccordionItem, DisclosureGroup, Panel};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::text_field::TextField;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::theme::tokens::text_scaler::TextScalerViewExt;
use cove_ui_compose::theme::{TextScaler, Theme, ThemeViewExt};
use cove_ui_compose::{BoxView, EmptyView};

use support::GlobalTestLock;

const WIDTH: f32 = 340.0;
const HEIGHT: f32 = 700.0;

fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

fn all(graph: &cove_composition::ViewGraph) -> Vec<Node> {
    let mut collected = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        collect(&root, &mut collected);
    }
    collected
}

fn settle(graph: &cove_composition::ViewGraph, width: f32, height: f32) {
    for _ in 0..4 {
        graph.recomposer.commit_all();
        graph.compute_layout(width, height);
    }
}

/// The `AccordionScene` from the Swift suite: a three-item accordion whose
/// first section holds a debug-named text field and whose second is disabled.
struct AccordionScene {
    expanded: State<BTreeSet<&'static str>>,
}

impl AccordionScene {
    fn new() -> Self {
        let mut initial = BTreeSet::new();
        initial.insert("first");
        Self {
            expanded: State::new(initial),
        }
    }

    fn view(&self) -> AnyView {
        AnyView::new(Accordion::new(
            vec![
                AccordionItem::new(
                    "first",
                    "Project preferences with a wrapping title",
                    AnyView::new(TextField::new(
                        "Draft",
                        Binding::constant(TextBuffer::from("Draft")),
                    ))
                    .debug_name("accordion-draft"),
                ),
                AccordionItem::new("disabled", "Unavailable", AnyView::new(EmptyView))
                    .enabled(false),
                AccordionItem::new(
                    "second",
                    "Activity",
                    AnyView::new(Text::new("Recent changes")),
                ),
            ],
            self.expanded.binding(),
        ))
    }
}

fn accordion_headers(graph: &cove_composition::ViewGraph) -> Vec<Node> {
    all(graph)
        .into_iter()
        .filter(|node| {
            node.accessibility().is_some_and(|semantics| {
                semantics.role == AccessibilityRole::Button
                    && semantics.state.is_expanded.is_some()
            })
        })
        .collect()
}

fn dispatcher(
    graph: &cove_composition::ViewGraph,
    interactions: &Rc<InteractionRegistry>,
    focus: &Rc<FocusChain>,
    capture: &Rc<PointerCapture>,
) -> EventDispatcher {
    EventDispatcher::new(
        graph.tree.clone(),
        interactions.clone(),
        capture.clone(),
        focus.clone(),
        WindowId::MAIN,
    )
}

fn key(dispatcher: &EventDispatcher, scancode: u32) {
    dispatcher.dispatch(&InputEvent::KeyDown(KeyEvent {
        scancode,
        keycode: 0,
        modifiers: KeyModifiers::default(),
        is_repeat: false,
    }));
}

#[test]
fn the_accordion_keeps_scaled_titles_and_roving_focus_semantics() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let interactions = Rc::new(InteractionRegistry::new());
            let focus = Rc::new(FocusChain::new());
            let capture = Rc::new(PointerCapture::new());
            InteractionRegistryHolder::set_current(Some(interactions.clone()));

            let graph = support::graph();
            let scene = AccordionScene::new();
            graph.install_view(
                AnyView::new(
                    AnyView::new(scene.view())
                        .frame_width(WIDTH)
                        .theme(Theme::default_light())
                        .text_scaler(TextScaler::new(2.0)),
                ),
                Default::default(),
            );
            settle(&graph, WIDTH, HEIGHT);

            let headers = accordion_headers(&graph);
            assert_eq!(
                headers.len(),
                3,
                "every section publishes an expandable header"
            );
            let first = headers.first().cloned().expect("the first header");
            let last = headers.last().cloned().expect("the last header");
            assert_eq!(
                first.frame().height,
                64.0,
                "the scaled title keeps one 64pt line"
            );
            assert!(
                !headers[1].is_focusable(),
                "the disabled header is skipped by the roving focus order"
            );

            // Focus the first header and check its published expansion state.
            focus.focus(Some(&first), false);
            settle(&graph, WIDTH, HEIGHT);
            assert_eq!(
                first
                    .accessibility()
                    .and_then(|semantics| semantics.state.is_expanded),
                Some(true),
                "the first section reports itself expanded"
            );

            let dispatcher = dispatcher(&graph, &interactions, &focus, &capture);
            key(&dispatcher, Scancode::ARROW_DOWN);
            settle(&graph, WIDTH, HEIGHT);
            let focused = focus.focused().expect("arrow-down focuses a header");
            assert_eq!(
                focused.id(),
                last.id(),
                "arrow-down skips the disabled header and reaches the last one"
            );
            assert_eq!(
                scene.expanded.get(),
                BTreeSet::from(["first"]),
                "moving focus does not change the expansion"
            );

            key(&dispatcher, Scancode::RETURN);
            settle(&graph, WIDTH, HEIGHT);
            cove_core::scene::AnimatorScheduler::current().tick(1.0);
            settle(&graph, WIDTH, HEIGHT);
            assert_eq!(
                scene.expanded.get(),
                BTreeSet::from(["second"]),
                "Return expands the focused section and collapses the previous one"
            );
            assert_eq!(
                first
                    .accessibility()
                    .and_then(|semantics| semantics.state.is_expanded),
                Some(false),
                "the first section reports itself collapsed"
            );
            assert_eq!(
                last.accessibility()
                    .and_then(|semantics| semantics.state.is_expanded),
                Some(true),
                "the second section reports itself expanded"
            );
            assert!(
                !graph
                    .layout_snapshot()
                    .iter()
                    .any(|entry| entry.debug_name.as_deref() == Some("accordion-draft")),
                "the collapsed section's text field leaves layout"
            );

            InteractionRegistryHolder::set_current(None);
        });
    });
}

#[test]
fn closing_a_section_clears_focused_editing_immediately() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let interactions = Rc::new(InteractionRegistry::new());
            let focus = Rc::new(FocusChain::new());
            InteractionRegistryHolder::set_current(Some(interactions.clone()));

            let graph = support::graph();
            let scene = AccordionScene::new();
            graph.install_view(
                AnyView::new(
                    AnyView::new(scene.view())
                        .frame_width(WIDTH)
                        .theme(Theme::default_light())
                        .text_scaler(TextScaler::new(2.0)),
                ),
                Default::default(),
            );
            settle(&graph, WIDTH, HEIGHT);

            let input = all(&graph)
                .into_iter()
                .find(|node| {
                    node.accessibility()
                        .is_some_and(|semantics| semantics.role == AccessibilityRole::TextField)
                })
                .expect("the first section holds a text field");
            focus.focus(Some(&input), false);

            scene.expanded.set(BTreeSet::new());
            settle(&graph, WIDTH, HEIGHT);

            assert!(
                focus.focused().is_none(),
                "closing the section clears focused editing"
            );
            assert!(
                !input.accepts_subtree_input(),
                "the closing field refuses input immediately"
            );
            assert!(
                all(&graph).iter().any(|node| node.id() == input.id()),
                "the exit paint is still mounted before the animation finishes"
            );

            cove_core::scene::AnimatorScheduler::current().tick(1.0);
            settle(&graph, WIDTH, HEIGHT);
            assert!(
                !all(&graph).iter().any(|node| node.id() == input.id()),
                "the field is torn down once the exit animation settles"
            );

            InteractionRegistryHolder::set_current(None);
        });
    });
}

#[test]
fn disclosure_and_panel_headers_fit_one_scaled_title_line() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = support::graph();
            let header = AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(cove_composition::TupleView::new(vec![
                    AnyView::new(DisclosureGroup::title(
                        Binding::constant(false),
                        "A long disclosure heading that wraps",
                        AnyView::new(EmptyView),
                    )),
                    AnyView::new(
                        AnyView::new(Panel::new("Inspector", AnyView::new(Text::new("Content"))))
                            .frame_height(220.0),
                    ),
                ])),
            ))
            .theme(Theme::default_light())
            .text_scaler(TextScaler::new(2.0));
            graph.install_view(AnyView::new(header), Default::default());
            settle(&graph, WIDTH, HEIGHT);

            let title = all(&graph)
                .into_iter()
                .find(|node| {
                    node.layout_node()
                        .and_then(|layout| layout.text_inputs())
                        .is_some_and(|inputs| inputs.text == "Inspector")
                })
                .expect("the panel title materialises as measured text");
            assert_eq!(
                title.frame().height,
                36.0,
                "the panel title fits one scaled line"
            );

            let disclosure = all(&graph)
                .into_iter()
                .find(|node| {
                    node.layout_node()
                        .and_then(|layout| layout.text_inputs())
                        .is_some_and(|inputs| {
                            inputs.text == "A long disclosure heading that wraps"
                        })
                })
                .expect("the disclosure label materialises as measured text");
            assert_eq!(
                disclosure.frame().height,
                36.0,
                "the disclosure label fits one scaled line instead of wrapping"
            );
        });
    });
}

#[test]
fn tree_rows_share_the_scaled_geometry() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let interactions = Rc::new(InteractionRegistry::new());
            let focus = Rc::new(FocusChain::new());
            let capture = Rc::new(PointerCapture::new());
            InteractionRegistryHolder::set_current(Some(interactions.clone()));

            // `Binding` requires thread-safe closures, so the observed state is a
            // pair of atomics rather than `Rc<Cell>`. The test only reads back what
            // it wrote, which is all the Swift `Binding` round trip asserted.
            let primary = std::sync::Arc::new(std::sync::atomic::AtomicI32::new(i32::MIN));
            let primary_read = primary.clone();
            let primary_write = primary.clone();
            let expanded = std::sync::Arc::new(std::sync::Mutex::new(HashSet::from([0i32])));
            let expanded_read = expanded.clone();
            let expanded_write = expanded.clone();

            let mut options: TreeOptions<i32, i32> = TreeOptions::default();
            options.selection = TreeSelection {
                primary: Binding::new(
                    move || {
                        let value = primary_read.load(std::sync::atomic::Ordering::Relaxed);
                        (value != i32::MIN).then_some(value)
                    },
                    move |value: Option<i32>| {
                        primary_write.store(
                            value.unwrap_or(i32::MIN),
                            std::sync::atomic::Ordering::Relaxed,
                        );
                    },
                ),
                expanded: Some(Binding::new(
                    move || {
                        expanded_read
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .clone()
                    },
                    move |value| {
                        *expanded_write
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner()) = value;
                    },
                )),
                ..TreeSelection::default()
            };

            let tree = Tree::new(
                vec![0i32],
                |value| *value,
                |value| {
                    if *value == 0 {
                        vec![1, 2]
                    } else {
                        Vec::new()
                    }
                },
                None,
                options,
                |value: &i32, _selected, _focused_for_row, _depth| {
                    AnyView::new(
                        AnyView::new(Text::new(format!("Node {value}")))
                            .debug_name(&format!("tree-text-{value}")),
                    )
                },
            );

            let graph = support::graph();
            graph.install_view(
                AnyView::new(
                    AnyView::new(tree)
                        .theme(Theme::default_light())
                        .text_scaler(TextScaler::new(2.0)),
                ),
                Default::default(),
            );
            settle(&graph, WIDTH, HEIGHT);

            let rows: Vec<Node> = all(&graph)
                .into_iter()
                .filter(|node| {
                    node.accessibility()
                        .is_some_and(|semantics| semantics.role == AccessibilityRole::TreeItem)
                })
                .collect();
            assert_eq!(rows.len(), 3, "the expanded root and its two children");
            for row in &rows {
                assert_eq!(
                    row.frame().height,
                    60.0,
                    "every row uses the scaled 60pt height, was {}",
                    row.frame().height
                );
            }

            let root_text = graph
                .layout_snapshot()
                .into_iter()
                .find(|entry| entry.debug_name.as_deref() == Some("tree-text-0"))
                .expect("the root text materialises")
                .absolute_frame;
            let child_text = graph
                .layout_snapshot()
                .into_iter()
                .find(|entry| entry.debug_name.as_deref() == Some("tree-text-1"))
                .expect("the child text materialises")
                .absolute_frame;
            assert_eq!(root_text.height, 40.0, "the text keeps a 40pt scaled line");
            assert_eq!(
                child_text.y - root_text.y,
                60.0,
                "the child sits one row below the root"
            );
            assert_eq!(
                child_text.x - root_text.x,
                32.0,
                "the child is indented by the disclosure slot"
            );
            assert!(
                all(&graph).iter().any(|node| {
                    node.view_tag()
                        .is_some_and(|tag| tag.contains("Image"))
                        && (node.frame().width - 24.0).abs() < 0.5
                }),
                "the disclosure glyph renders at 24pt"
            );

            let dispatcher = dispatcher(&graph, &interactions, &focus, &capture);
            let event = MouseButtonEvent {
                button: MouseButton::Left,
                x: child_text.x + child_text.width / 2.0,
                y: child_text.y + child_text.height / 2.0,
                clicks: 1,
                modifiers: KeyModifiers::default(),
                kind: Default::default(),
            };
            dispatcher.dispatch(&InputEvent::MouseButtonDown(event.clone()));
            dispatcher.dispatch(&InputEvent::MouseButtonUp(event));
            let chosen = primary.load(std::sync::atomic::Ordering::Relaxed);
            assert_eq!(
                (chosen != i32::MIN).then_some(chosen),
                Some(1),
                "clicking the child row selects it"
            );

            InteractionRegistryHolder::set_current(None);
        });
    });
}
