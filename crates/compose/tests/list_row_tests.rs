//! Translation of `Tests/CoveUIComposeTests/ListRowTests.swift`.
//!
//! Behaviour checklist:
//! - across width, text scale, input mode, appearance and direction, a row's
//!   title yields to its fixed slots, stays one clipped line, keeps its node
//!   identity, and the combined list-item label carries every slot
//! - desktop rows whose fixed slots overflow scroll horizontally inside a
//!   viewport that stays within the surface
//! - embedded actions leave row selection alone; pointer and keyboard
//!   activation share the collection model
//! - oversized fixed content scrolls horizontally (wheel and increment) and
//!   drops the scroll semantics once it fits again
//! - a row mounted by navigation has its full title width in its first pass
//! - a 200K-row collection mounts only a viewport after End
//! - title-only, leading-only and conditional-trailing rows keep nonzero text
//!   and a stable title node

use cove_composition::Scope;
mod support;

use std::collections::BTreeSet;

use cove_composition::layout::LayoutDirectionExt;
use cove_composition::{
    AnyView, DynamicField, LayoutDebugExt, TupleView, VerticalAlignment, View, ViewGraph,
};
use cove_core::scene::{
    AccessibilityElement, AccessibilityRole, AccessibilityTree, Align, AnimatorScheduler,
    Direction, EventDispatcher, FlexDirection, Justify, Node, PlatformInputContext, Wrap,
};
use cove_core::{Environment, InputCapabilities, Point, State};
use cove_platform::{
    InputEvent, KeyEvent, KeyModifiers, MouseButton, MouseButtonEvent, MouseWheelEvent,
    ScrollInput, ScrollUnit, WindowId,
};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::interaction::AccessibilityExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::data::lists::{
    CollectionSelection, List, ListRow, ListSection, SectionedList,
};
use cove_ui_compose::primitives::feedback::{Badge, StatusTone};
use cove_ui_compose::primitives::media::icon::Icon;
use cove_ui_compose::primitives::media::ui_icon_catalog::{file, status};
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::styles::buttons::button_style::{ButtonStyleViewExt, ghost};
use cove_ui_compose::theme::styles::circular_appearance::ThemeTransition;
use cove_ui_compose::theme::tokens::SemanticIconSize;
use cove_ui_compose::theme::tokens::semantic_modifiers::SemanticStyleViewExt;
use cove_ui_compose::theme::tokens::text_scaler::TextScalerViewExt;
use cove_ui_compose::theme::{SemanticColorRef, SemanticFontRef, TextScaler};
use cove_ui_compose::{BoxView, Row, Scancode};

use support::GlobalTestLock;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Single,
    Multiple,
    Sections,
}

const KINDS: [Kind; 3] = [Kind::Single, Kind::Multiple, Kind::Sections];

fn title(index: i32) -> String {
    format!(
        "Document {} — 工作区设置与权限 👨‍👩‍👧‍👦 — a long localized title that must remain readable",
        index + 1
    )
}

#[derive(Clone)]
struct Harness {
    kind: Kind,
    width: State<f32>,
    scale: State<f32>,
    input: State<InputCapabilities>,
    rtl: State<bool>,
    dark: State<bool>,
    count: State<i32>,
    selected: State<Option<i32>>,
    multiple: State<BTreeSet<i32>>,
    actions: State<i32>,
    activated: State<Option<i32>>,
    has_action: State<bool>,
}

impl Harness {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            width: State::new(390.0),
            scale: State::new(1.0),
            input: State::new(InputCapabilities::POINTER | InputCapabilities::KEYBOARD),
            rtl: State::new(false),
            dark: State::new(false),
            count: State::new(18),
            selected: State::new(Some(1)),
            multiple: State::new(BTreeSet::from([1])),
            actions: State::new(0),
            activated: State::new(None),
            has_action: State::new(false),
        }
    }

    fn row(&self, index: i32) -> AnyView {
        let mut trailing = vec![
            AnyView::new(Badge::new("99").tone(StatusTone::Info)),
            AnyView::new(
                Text::new("Synced")
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted())
                    .debug_name(&format!("row-state-{index}")),
            ),
        ];
        if self.has_action.get() {
            let actions = self.actions.clone();
            trailing.push(AnyView::new(
                Button::title("Inspect", move || actions.update(|value| *value += 1))
                    .button_style(ghost())
                    .accessibility_label(format!("Inspect {index}")),
            ));
        }
        AnyView::new(
            ListRow::new(title(index))
                .leading(
                    Icon::semantic_with_ref(
                        status::bookmark(),
                        SemanticIconSize::medium(),
                        SemanticColorRef::accent(),
                    )
                    .accessibility_hidden(true)
                    .debug_name(&format!("row-icon-{index}")),
                )
                .trailing(
                    Row::new(
                        VerticalAlignment::Center,
                        8.0,
                        AnyView::new(TupleView::new(trailing)),
                    )
                    .debug_name(&format!("row-trailing-{index}")),
                )
                .semantic_font(SemanticFontRef::body()),
        )
    }

    fn collection(&self) -> AnyView {
        let count = self.count.get();
        let activated = self.activated.clone();
        match self.kind {
            Kind::Single => {
                let this = self.clone();
                AnyView::new(
                    List::new(
                        CollectionSelection::single(self.selected.binding()),
                        move |index, _| this.row(index),
                    )
                    .rows((0..count).collect())
                    .on_activate(move |id| activated.set(Some(id))),
                )
            }
            Kind::Multiple => {
                let this = self.clone();
                AnyView::new(
                    List::new(
                        CollectionSelection::multiple(self.multiple.binding()),
                        move |index, _| this.row(index),
                    )
                    .rows((0..count).collect())
                    .on_activate(move |id| activated.set(Some(id))),
                )
            }
            Kind::Sections => {
                let this = self.clone();
                AnyView::new(
                    SectionedList::new(
                        vec![
                            ListSection::new("a", (0..count / 2).collect::<Vec<i32>>())
                                .title("Favorites"),
                            ListSection::new("b", (count / 2..count).collect::<Vec<i32>>())
                                .title("Scene"),
                        ],
                        |item: &i32| *item,
                        move |item, _| this.row(*item),
                    )
                    .selection(self.selected.binding())
                    .sticky_headers(true)
                    .on_activate(move |item| activated.set(Some(*item))),
                )
            }
        }
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
        AnyView::new(
            LayerRoot::new(self.collection().frame_size(width, 240.0))
                .surface_environment(surface)
                .text_scaler(TextScaler::new(self.scale.get()))
                .appearance(
                    if self.dark.get() {
                        Appearance::Dark
                    } else {
                        Appearance::Light
                    },
                    ThemeTransition::default(),
                )
                .layout_direction(if self.rtl.get() {
                    Direction::Rtl
                } else {
                    Direction::Ltr
                }),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("width", &self.width),
            DynamicField::state("scale", &self.scale),
            DynamicField::state("input", &self.input),
            DynamicField::state("rtl", &self.rtl),
            DynamicField::state("dark", &self.dark),
            DynamicField::state("count", &self.count),
            DynamicField::state("selected", &self.selected),
            DynamicField::state("multiple", &self.multiple),
            DynamicField::state("actions", &self.actions),
            DynamicField::state("activated", &self.activated),
            DynamicField::state("hasAction", &self.has_action),
        ]
    }
}

fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

fn nodes_below(node: &Node) -> Vec<Node> {
    let mut all = Vec::new();
    collect(node, &mut all);
    all
}

fn nodes(graph: &ViewGraph) -> Vec<Node> {
    graph
        .tree
        .borrow()
        .root()
        .map(|root| nodes_below(&root))
        .unwrap_or_default()
}

fn flatten(items: &[AccessibilityElement]) -> Vec<AccessibilityElement> {
    items
        .iter()
        .flat_map(|item| {
            let mut all = vec![item.clone()];
            all.extend(flatten(&item.children));
            all
        })
        .collect()
}

fn list_items(graph: &ViewGraph) -> Vec<AccessibilityElement> {
    let root = graph.tree.borrow().root().expect("root");
    flatten(&AccessibilityTree::snapshot(&root))
        .into_iter()
        .filter(|element| element.semantics.role == AccessibilityRole::ListItem)
        .collect()
}

fn settle_at(graph: &ViewGraph, width: f32) {
    for _ in 0..8 {
        graph.recomposer.commit_all();
        graph.compute_layout(width, 240.0);
    }
}

fn settle(graph: &ViewGraph, harness: &Harness) {
    settle_at(graph, harness.width.get());
}

fn named(name: &str, graph: &ViewGraph) -> Node {
    nodes(graph)
        .into_iter()
        .find(|node| support::debug_name(node).as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no node named {name}"))
}

fn text_node(value: &str, graph: &ViewGraph) -> Node {
    nodes(graph)
        .into_iter()
        .find(|node| {
            node.layout_node()
                .and_then(|layout| layout.text_inputs())
                .is_some_and(|inputs| inputs.text == value)
        })
        .unwrap_or_else(|| panic!("no text node {value:?}"))
}

fn keyboard_host(graph: &ViewGraph, context: &PlatformInputContext) -> Option<Node> {
    nodes(graph).into_iter().find(|node| {
        node.is_focusable()
            && node
                .accessibility()
                .is_none_or(|semantics| semantics.role != AccessibilityRole::Button)
            && context.interactions.handlers(node).key.is_some()
    })
}

fn combined_scroll_area(node: &Node) -> bool {
    node.accessibility().is_some_and(|semantics| {
        semantics.role == AccessibilityRole::ScrollArea && semantics.combines_children
    })
}

fn scene(
    kind: Kind,
    work: impl FnOnce(&Harness, &ViewGraph, &PlatformInputContext, &EventDispatcher),
) {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let context = PlatformInputContext::new();
            let scheduler = AnimatorScheduler::default();
            context.with_current(|| {
                scheduler.with_current(|| {
                    let harness = Harness::new(kind);
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

fn click(node: &Node, input: &EventDispatcher) {
    let frame = node.absolute_frame();
    let event = MouseButtonEvent {
        button: MouseButton::Left,
        x: frame.x + frame.width / 2.0,
        y: frame.y + frame.height / 2.0,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    };
    input.dispatch(&InputEvent::MouseButtonDown(event.clone()));
    input.dispatch(&InputEvent::MouseButtonUp(event));
}

fn key(code: u32, input: &EventDispatcher) {
    input.dispatch(&InputEvent::KeyDown(KeyEvent {
        scancode: code,
        keycode: 0,
        modifiers: KeyModifiers::default(),
        is_repeat: false,
    }));
}

#[test]
fn matrix() {
    for kind in KINDS {
        scene(kind, |harness, graph, _, _| {
            let original = text_node(&title(0), graph);
            for width in [240.0_f32, 390.0, 720.0, 1280.0] {
                for scale in [1.0_f32, 2.0] {
                    for input in [
                        InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
                        InputCapabilities::TOUCH,
                        InputCapabilities::TOUCH
                            | InputCapabilities::POINTER
                            | InputCapabilities::KEYBOARD,
                    ] {
                        for dark in [false, true] {
                            for rtl in [false, true] {
                                let case = format!(
                                    "{kind:?} width {width} scale {scale} input {input:?} dark {dark} rtl {rtl}"
                                );
                                harness.width.set(width);
                                harness.scale.set(scale);
                                harness.input.set(input);
                                harness.dark.set(dark);
                                harness.rtl.set(rtl);
                                settle(graph, harness);
                                let title_node = text_node(&title(0), graph);
                                let trailing = named("row-trailing-0", graph);
                                let icon = named("row-icon-0", graph);
                                assert!(
                                    title_node == original
                                        && title_node.clips_to_bounds()
                                        && title_node.frame().width > 0.0,
                                    "{case}: the title is retained, clipped and nonzero"
                                );
                                let touch = input.contains(InputCapabilities::TOUCH);
                                let trailing_frame = trailing.absolute_frame();
                                if !touch
                                    && (trailing_frame.x < -0.5
                                        || trailing_frame.x + trailing_frame.width > width + 0.5)
                                {
                                    let viewport = nodes(graph)
                                        .into_iter()
                                        .find(|node| {
                                            combined_scroll_area(node)
                                                && nodes_below(node).contains(&title_node)
                                        })
                                        .unwrap_or_else(|| {
                                            panic!("{case}: overflowing slots scroll horizontally")
                                        });
                                    let frame = viewport.absolute_frame();
                                    assert!(
                                        frame.x >= -0.5 && frame.x + frame.width <= width + 0.5,
                                        "{case}: the viewport stays inside the surface"
                                    );
                                    viewport
                                        .accessibility_actions()
                                        .increment
                                        .as_ref()
                                        .expect("increment")();
                                    settle(graph, harness);
                                    assert!(viewport.content_offset().x > 0.0, "{case}");
                                    viewport.set_content_offset(Point::default());
                                    settle(graph, harness);
                                }
                                let title_frame = title_node.absolute_frame();
                                let icon_frame = icon.absolute_frame();
                                if touch {
                                    assert!(
                                        title_frame.x >= -0.5
                                            && title_frame.x + title_frame.width <= width + 0.5,
                                        "{case}: a touch title stays within the surface"
                                    );
                                } else if rtl {
                                    assert!(
                                        trailing_frame.x + trailing_frame.width
                                            <= title_frame.x + 0.5,
                                        "{case}: trailing precedes the title in RTL"
                                    );
                                    assert!(
                                        title_frame.x + title_frame.width <= icon_frame.x + 0.5,
                                        "{case}: the title precedes the icon in RTL"
                                    );
                                } else {
                                    assert!(
                                        icon_frame.x + icon_frame.width <= title_frame.x + 0.5,
                                        "{case}: the icon precedes the title"
                                    );
                                    assert!(
                                        title_frame.x + title_frame.width <= trailing_frame.x + 0.5,
                                        "{case}: the title precedes the trailing slot"
                                    );
                                }
                                let cache = title_node
                                    .layout_node()
                                    .and_then(|layout| layout.text_measure())
                                    .unwrap_or_else(|| panic!("{case}: measured title"));
                                assert_eq!(cache.result.lines.len(), 1, "{case}: one line");
                                assert!(
                                    list_items(graph).iter().any(|row| {
                                        let label = &row.semantics.label;
                                        label.contains(&title(0))
                                            && label.contains("Synced")
                                            && label.contains("99")
                                    }),
                                    "{case}: the combined row label carries every slot"
                                );
                                assert!(
                                    harness.selected.get() == Some(1)
                                        && harness.multiple.get() == BTreeSet::from([1])
                                        && !graph.recomposer.has_pending(),
                                    "{case}: selection untouched and settled"
                                );
                            }
                        }
                    }
                }
            }
        });
    }
}

#[test]
fn actions_and_selection() {
    for kind in KINDS {
        scene(kind, |harness, graph, context, input| {
            harness.has_action.set(true);
            settle(graph, harness);
            let action = nodes(graph)
                .into_iter()
                .find(|node| {
                    node.is_focusable()
                        && node
                            .accessibility()
                            .is_some_and(|semantics| semantics.label == "Inspect 0")
                })
                .expect("the embedded action");
            click(&action, input);
            settle(graph, harness);
            assert!(
                harness.actions.get() == 1
                    && harness.selected.get() == Some(1)
                    && harness.multiple.get() == BTreeSet::from([1]),
                "{kind:?}: the action fires without touching selection"
            );
            click(&text_node(&title(0), graph), input);
            settle(graph, harness);
            if kind == Kind::Multiple {
                assert_eq!(harness.multiple.get(), BTreeSet::from([0]));
            } else {
                assert_eq!(harness.selected.get(), Some(0), "{kind:?}");
            }
            let host = keyboard_host(graph, context).expect("keyboard host");
            context.focus_chain.focus(Some(&host), true);
            key(Scancode::END, input);
            settle(graph, harness);
            key(Scancode::RETURN, input);
            assert_eq!(
                harness.activated.get(),
                Some(harness.count.get() - 1),
                "{kind:?}"
            );
        });
    }
}

#[test]
fn overflow() {
    scene(Kind::Single, |harness, graph, _, input| {
        for rtl in [false, true] {
            harness.width.set(120.0);
            harness.scale.set(2.0);
            harness.rtl.set(rtl);
            settle(graph, harness);
            let scroll = nodes(graph)
                .into_iter()
                .find(combined_scroll_area)
                .expect("an overflowing row scrolls");
            let row = list_items(graph).into_iter().next().expect("a list item");
            assert!(
                row.semantics.label.contains(&title(0)) && row.semantics.label.contains("Synced"),
                "rtl {rtl}: label {:?}",
                row.semantics.label
            );
            let frame = scroll.absolute_frame();
            input.dispatch(&InputEvent::MouseWheel(MouseWheelEvent {
                x: -60.0,
                y: 0.0,
                mouse_x: Some(frame.x + frame.width / 2.0),
                mouse_y: Some(frame.y + frame.height / 2.0),
                input: ScrollInput {
                    unit: ScrollUnit::Points,
                    ..Default::default()
                },
            }));
            settle(graph, harness);
            assert!(
                scroll.content_offset().x > 0.0,
                "rtl {rtl}: the wheel scrolls"
            );
            scroll
                .accessibility_actions()
                .increment
                .as_ref()
                .expect("increment")();
            settle(graph, harness);
            assert!(
                scroll.content_offset().x > 0.0 && harness.selected.get() == Some(1),
                "rtl {rtl}"
            );
            harness.width.set(720.0);
            settle(graph, harness);
            assert!(
                scroll.accessibility().is_none() && scroll.content_offset().x == 0.0,
                "rtl {rtl}: fitting content drops the scroll semantics"
            );
        }
    });
}

#[test]
fn first_frame_width() {
    scene(Kind::Single, |harness, graph, _, _| {
        let last = harness.count.get() - 1;
        harness.selected.set(Some(last));
        graph.recomposer.commit_all();
        graph.compute_layout(harness.width.get(), 240.0);
        let first = text_node(&title(last), graph).frame().width;
        settle(graph, harness);
        let settled = text_node(&title(last), graph).frame().width;
        assert!(settled > 100.0, "settled width {settled}");
        assert!(
            (first - settled).abs() < 1.0,
            "first {first} vs settled {settled}"
        );
    });
}

#[test]
fn windowing() {
    for kind in [Kind::Single, Kind::Multiple] {
        scene(kind, |harness, graph, context, input| {
            harness.count.set(200_000);
            settle(graph, harness);
            let host = keyboard_host(graph, context).expect("keyboard host");
            context.focus_chain.focus(Some(&host), true);
            key(Scancode::END, input);
            settle(graph, harness);
            let rows = nodes(graph)
                .into_iter()
                .filter(|node| {
                    node.accessibility()
                        .is_some_and(|semantics| semantics.role == AccessibilityRole::ListItem)
                })
                .count();
            assert!(rows < 30, "{kind:?}: {rows} rows mounted");
            assert!(text_node(&title(199_999), graph).frame().width > 0.0);
        });
    }
}

#[derive(Clone)]
struct EmptySlots {
    selected: State<bool>,
}

impl View for EmptySlots {
    fn body(&self, _cx: &Scope) -> AnyView {
        let conditional = if self.selected.get() {
            AnyView::new(Badge::new("Selected").tone(StatusTone::Info))
        } else {
            AnyView::new(cove_composition::EmptyView)
        };
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                4.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(ListRow::new("Title only").frame_height(44.0)),
                    AnyView::new(
                        ListRow::new("Leading only")
                            .leading(Icon::semantic(file::file(), SemanticIconSize::medium()))
                            .trailing(cove_composition::EmptyView)
                            .frame_height(44.0),
                    ),
                    AnyView::new(
                        ListRow::new("Conditional trailing")
                            .trailing(conditional)
                            .frame_height(44.0),
                    ),
                ])),
            )
            .frame_width(240.0),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("selected", &self.selected)]
    }
}

#[test]
fn empty_slots() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let context = PlatformInputContext::new();
            context.with_current(|| {
                let harness = EmptySlots {
                    selected: State::new(false),
                };
                let graph = support::graph();
                graph.install(harness.clone(), Default::default());
                let layout = || {
                    for _ in 0..8 {
                        graph.recomposer.commit_all();
                        graph.compute_layout(240.0, 180.0);
                    }
                };
                layout();
                let original = text_node("Conditional trailing", &graph);
                for selected in [true, false, true] {
                    harness.selected.set(selected);
                    layout();
                    for value in ["Title only", "Leading only", "Conditional trailing"] {
                        let node = text_node(value, &graph);
                        assert!(
                            node.frame().width > 0.0 && node.frame().height > 0.0,
                            "selected {selected}: {value} has a nonzero frame"
                        );
                    }
                    assert!(
                        text_node("Conditional trailing", &graph) == original,
                        "selected {selected}: the title node is retained"
                    );
                }
                graph.install(cove_composition::EmptyView, Default::default());
            });
        });
    });
}
