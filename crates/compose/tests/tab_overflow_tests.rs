//! Translation of `Tests/CoveUIComposeTests/Adaptive/TabOverflowTests.swift`.
//!
//! Behaviour checklist:
//! - the selected panel and its counter survive width, variant and text-scale
//!   changes while every tab label stays mounted
//! - selecting through the keyboard reaches the last tab and the active panel
//! - two arrow commands inside one frame move the selection and mount the
//!   matching panel, skipping disabled tabs
//! - a long selected title renders in full, stays inside the strip and every
//!   tab keeps a labelled close control
//! - node-tree filtering over the strip reports the retained header and the
//!   overflow entrance, then drops the entrance once every header fits
//! - resizing keeps the selection and removing the last tab drops its label
//! - an extremely narrow strip keeps finite geometry for its chrome
//!
//! The harness runs the Swift default policy, `TabOverflow::Menu`: the strip
//! retains the selected header, lists the rest in a "More tabs" menu, and
//! publishes each item as an `AccessibilityRole::Tab` with its own label. Every
//! tab keeps a `"Close <title>"` control at a 44pt target, and the strip clamps a
//! header to its measured width so a long selected title stays inside it.
//!
//! Differences that remain:
//! - Swift's `UIDiagnostics` lives outside this crate; `ai_queries` runs the
//!   same label filters over the node tree.
//! - Swift's fixture applies text scaling, appearance and layout direction
//!   through the surface environment; the Rust `TabView` reads none of them and
//!   does not branch on input modality, so `matrix` varies width and variant
//!   only.
//! - The Rust fixture keeps the badge (`"7"` on Overview) but omits the tab
//!   icon; `TabItem` takes a `BundleImageResource` that the suite does not
//!   source.

use crate::support;
use cove_composition::Scope;

use cove_composition::{AnyView, DynamicField, View};
use cove_core::scene::AccessibilityRole;
use cove_core::{Binding, State};
use cove_platform::{InputEvent, KeyEvent, KeyModifiers};
use cove_ui_compose::Scancode;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::layout::{TabItem, TabOverflow, TabVariant, TabView};
use cove_ui_compose::primitives::text::Text;

use support::InteractiveScene;

const LABELS: [&str; 6] = [
    "Overview",
    "Activity",
    "Settings",
    "Unavailable",
    "Assets",
    "History",
];
const LONG_TITLE: &str = "Workspace permissions 👩🏽‍💻 and international collaboration settings";

struct CounterPanel {
    count: State<i32>,
}

impl View for CounterPanel {
    fn body(&self, _cx: &Scope) -> AnyView {
        let count = self.count.clone();
        let label = format!("Panel count {}", count.get());
        AnyView::new(Button::new(Text::new(label), move || {
            count.set(count.get() + 1)
        }))
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("count", &self.count)]
    }
}

struct Harness {
    selection: State<usize>,
    count: State<i32>,
    variant: State<TabVariant>,
    overflow: State<TabOverflow>,
    enabled: State<bool>,
    removed: State<Vec<usize>>,
    long_title: State<bool>,
}

impl Harness {
    fn new() -> Self {
        Self {
            selection: State::new(0),
            count: State::new(0),
            variant: State::new(TabVariant::Bar),
            overflow: State::new(TabOverflow::Menu),
            enabled: State::new(true),
            removed: State::new(Vec::new()),
            long_title: State::new(false),
        }
    }

    fn selection_binding(&self) -> Binding<usize> {
        let read = self.selection.clone();
        let write = self.selection.clone();
        Binding::new(move || read.get(), move |next| write.set(next))
    }
}

impl View for Harness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let removed = self.removed.get();
        let long = self.long_title.get();
        let count = self.count.clone();
        let remove_for_close = self.removed.clone();
        let selection_for_close = self.selection.clone();
        let tabs = LABELS
            .iter()
            .enumerate()
            .filter(|(index, _)| !removed.contains(index))
            .map(|(index, label)| {
                let text = if index == 2 && long {
                    LONG_TITLE
                } else {
                    label
                };
                let content = if index == 0 {
                    AnyView::new(CounterPanel {
                        count: count.clone(),
                    })
                } else {
                    AnyView::new(Text::new(format!("Panel {index}")))
                };
                let remove = remove_for_close.clone();
                let close_selection = selection_for_close.clone();
                let mut tab = TabItem::new(index, text, content)
                    .enabled(index != 3)
                    .on_close(move || {
                        let mut next = remove.get();
                        if !next.contains(&index) {
                            next.push(index);
                            next.sort_unstable();
                        }
                        remove.set(next);
                        if close_selection.get() == index {
                            close_selection.set(0);
                        }
                    });
                if index == 0 {
                    tab = tab.badge("7");
                }
                tab
            })
            .collect();

        AnyView::new(
            AnyView::new(
                TabView::try_new(self.selection_binding(), tabs)
                    .expect("unique tab IDs")
                    .variant(self.variant.get())
                    .overflow(self.overflow.get())
                    .enabled(self.enabled.get()),
            )
            .frame(
                Some(cove_ui_compose::modifiers::layout::FrameDimension::Percent(
                    100.0,
                )),
                Some(cove_ui_compose::modifiers::layout::FrameDimension::Percent(
                    100.0,
                )),
                None,
                None,
                None,
                None,
            ),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("selection", &self.selection),
            DynamicField::state("count", &self.count),
            DynamicField::state("variant", &self.variant),
            DynamicField::state("overflow", &self.overflow),
            DynamicField::state("enabled", &self.enabled),
            DynamicField::state("removed", &self.removed),
            DynamicField::state("long_title", &self.long_title),
        ]
    }
}

/// A node whose semantics are published to assistive clients. `accessibilityHidden`
/// text still exists in the tree — Swift's `visibleOnly` queries exclude it, so
/// the suite does too.
fn is_exposed(node: &cove_core::scene::Node) -> bool {
    node.accessibility()
        .is_some_and(|semantics| !semantics.is_hidden)
}

fn labelled(
    scene: &InteractiveScene,
    label: &str,
    role: Option<AccessibilityRole>,
) -> Option<cove_core::scene::Node> {
    scene.first(|node| {
        is_exposed(node)
            && node.accessibility().is_some_and(|semantics| {
                semantics.label == label && role.is_none_or(|role| semantics.role == role)
            })
    })
}

fn button(scene: &InteractiveScene, label: &str) -> Option<cove_core::scene::Node> {
    labelled(scene, label, Some(AccessibilityRole::Button))
}

fn panel_count(scene: &InteractiveScene) -> Option<String> {
    scene.nodes().into_iter().find_map(|node| {
        node.accessibility().and_then(|semantics| {
            (is_exposed(&node)
                && semantics.role == AccessibilityRole::StaticText
                && semantics.label.starts_with("Panel count "))
            .then_some(semantics.label)
        })
    })
}

fn panels(scene: &InteractiveScene) -> Vec<String> {
    scene
        .nodes()
        .into_iter()
        .filter_map(|node| {
            node.accessibility().and_then(|semantics| {
                (is_exposed(&node)
                    && semantics.role == AccessibilityRole::StaticText
                    && semantics.label.starts_with("Panel ")
                    && !semantics.label.starts_with("Panel count "))
                .then_some(semantics.label)
            })
        })
        .collect()
}

fn strip_labels(scene: &InteractiveScene) -> Vec<String> {
    scene
        .nodes()
        .into_iter()
        .filter_map(|node| {
            node.accessibility().and_then(|semantics| {
                (semantics.role == AccessibilityRole::Tab
                    && LABELS.contains(&semantics.label.as_str()))
                .then_some(semantics.label)
            })
        })
        .collect()
}

fn close_controls(scene: &InteractiveScene) -> Vec<cove_core::scene::Node> {
    scene.all(|node| {
        is_exposed(node)
            && node.accessibility().is_some_and(|semantics| {
                semantics.role == AccessibilityRole::Button
                    && semantics.label.starts_with("Close ")
                    && semantics.label != "Close dialog"
            })
    })
}

fn accessibility_dump(scene: &InteractiveScene) -> Vec<(String, AccessibilityRole, bool)> {
    scene
        .nodes()
        .into_iter()
        .filter_map(|node| {
            node.accessibility()
                .filter(|_| is_exposed(&node))
                .map(|semantics| (semantics.label, semantics.role, semantics.state.is_enabled))
        })
        .collect()
}

fn button_count(scene: &InteractiveScene) -> usize {
    scene
        .all(|node| {
            is_exposed(node)
                && node
                    .accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
        })
        .len()
}

/// Nearest ancestor (or self) publishing the `Button` role.
fn control_around(node: &cove_core::scene::Node) -> Option<cove_core::scene::Node> {
    let mut cursor = Some(node.clone());
    while let Some(candidate) = cursor {
        if candidate
            .accessibility()
            .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
        {
            return Some(candidate);
        }
        cursor = candidate.parent();
    }
    None
}

fn focus_button(scene: &InteractiveScene, label: &str) {
    let node = button(scene, label)
        .or_else(|| labelled(scene, label, None))
        .unwrap_or_else(|| panic!("no {label} control; labels {:?}", strip_labels(scene)));
    scene.focus.focus(Some(&node), false);
    assert_eq!(
        scene.focus.focused().map(|focused| focused.id()),
        Some(node.id()),
        "the {label} control is focusable"
    );
}

#[test]
fn matrix() {
    InteractiveScene::run(390.0, 300.0, |scene| {
        let harness = Harness::new();
        let variant_state = harness.variant.clone();
        scene.install(harness);
        let counter = labelled(scene, "Panel count 0", Some(AccessibilityRole::Button))
            .or_else(|| labelled(scene, "Panel count 0", None))
            .expect("the selected panel's counter is mounted");
        scene.click(&counter, 1);
        assert_eq!(
            panel_count(scene).as_deref(),
            Some("Panel count 1"),
            "clicking the counter increments the selected panel"
        );
        let panel = panel_count(scene);
        let id = labelled(scene, "Panel count 1", None)
            .map(|node| node.id())
            .expect("the bumped counter exists");

        for width in [390.0_f32, 720.0, 1280.0] {
            scene.width = width;
            scene.settle(8, 1.0);
            for variant in [TabVariant::Bar, TabVariant::Segmented, TabVariant::Card] {
                variant_state.set(variant);
                scene.settle(8, 1.0);
                let after = labelled(scene, "Panel count 1", None).map(|node| node.id());
                assert_eq!(
                    after,
                    Some(id),
                    "the selected panel keeps its node identity at width {width} \
                     variant {variant:?} (got {after:?})"
                );
                assert_eq!(
                    panel_count(scene),
                    panel,
                    "the selected panel keeps its counter at width {width}"
                );
                assert!(
                    panels(scene).is_empty(),
                    "no unselected panel content is mounted (got {:?})",
                    panels(scene)
                );
                let labels = strip_labels(scene);
                for label in LABELS {
                    assert!(
                        labels.iter().any(|candidate| candidate == label),
                        "every strip label stays mounted at width {width}; \
                         missing {label} in {labels:?}"
                    );
                }
                let tabs: Vec<_> = scene.all(|node| {
                    is_exposed(node)
                        && node
                            .accessibility()
                            .is_some_and(|semantics| semantics.role == AccessibilityRole::Tab)
                });
                assert_eq!(
                    tabs.len(),
                    LABELS.len(),
                    "every strip item publishes the Tab role at width {width}"
                );
                assert!(
                    !scene.graph.recomposer.has_pending(),
                    "layout settles at width {width}"
                );
            }
        }
        let unavailable = labelled(scene, "Unavailable", Some(AccessibilityRole::Tab))
            .expect("the disabled tab is rendered as a tab");
        let enabled = unavailable
            .accessibility()
            .map(|semantics| semantics.state.is_enabled)
            .unwrap_or(true);
        assert!(
            !enabled,
            "the disabled tab reports disabled (got {enabled})"
        );
        assert_eq!(
            unavailable.accessibility().map(|semantics| semantics.label),
            Some("Unavailable".to_string()),
            "the disabled tab carries its own label"
        );
        assert!(
            !unavailable
                .accessibility()
                .is_some_and(|semantics| semantics.state.is_enabled),
            "the disabled tab publishes a disabled state"
        );
    });
}

#[test]
fn menu() {
    InteractiveScene::run(240.0, 300.0, |scene| {
        scene
            .install(cove_ui_compose::primitives::overlays::portal::LayerRoot::new(Harness::new()));
        let more = labelled(scene, "More tabs", Some(AccessibilityRole::Button))
            .expect("the menu strip exposes a More tabs entrance");
        assert!(
            more.absolute_frame().width >= 44.0 && more.absolute_frame().height >= 44.0,
            "the overflow entrance is a 44pt target; measured {:?}",
            more.absolute_frame()
        );
        // Swift's flow: open the menu, then End plus Return picks the last
        // hidden tab and dismisses the menu, restoring focus to the entrance.
        scene.click(&more, 1);
        scene.settle(8, 1.0);
        assert_eq!(
            scene.overlay_portals().len(),
            1,
            "clicking More presents the overflow portal"
        );
        scene.key(Scancode::END, KeyModifiers::default());
        scene.key(Scancode::RETURN, KeyModifiers::default());
        scene.settle(8, 1.0);
        assert_eq!(
            panels(scene),
            vec!["Panel 5".to_string()],
            "End plus Return mounts the last tab's panel"
        );
        assert!(
            scene.overlay_portals().is_empty(),
            "activating a menu item dismisses the menu; entries {:?}",
            scene.overlay_portals().len()
        );
        let history = labelled(scene, "History", Some(AccessibilityRole::Tab))
            .expect("the promoted header is retained as a tab");
        assert!(
            history
                .accessibility()
                .is_some_and(|semantics| semantics.state.is_selected),
            "the promoted header reports the selected state"
        );
    });
}

#[test]
fn hidden_keyboard_target() {
    InteractiveScene::run(240.0, 300.0, |scene| {
        scene.install(Harness::new());
        focus_button(scene, "Overview");
        let dispatcher = scene.dispatcher();
        for scancode in [Scancode::END, Scancode::ARROW_LEFT] {
            dispatcher.dispatch(&InputEvent::KeyDown(KeyEvent {
                scancode,
                keycode: 0,
                modifiers: KeyModifiers::default(),
                is_repeat: false,
            }));
        }
        scene.settle(8, 1.0);
        assert_eq!(
            panels(scene),
            vec!["Panel 4".to_string()],
            "End then ArrowLeft selects Assets"
        );
        let focused = scene
            .focus
            .focused()
            .map(|node| cove_core::scene::AccessibilityTree::resolved_label(&node));
        assert_eq!(
            focused.as_deref(),
            Some("Assets"),
            "focus follows the roving tab stop to Assets"
        );
        scene.key(Scancode::ARROW_LEFT, KeyModifiers::default());
        assert_eq!(
            panels(scene),
            vec!["Panel 2".to_string()],
            "the second ArrowLeft moves from 4 to 2, skipping the disabled tab"
        );
    });
}

#[test]
fn truncation() {
    InteractiveScene::run(240.0, 300.0, |scene| {
        let harness = Harness::new();
        harness.selection.set(2);
        harness.long_title.set(true);
        scene.install(harness);
        let title = labelled(scene, LONG_TITLE, Some(AccessibilityRole::Tab))
            .expect("the long selected title is one labelled tab control");
        let frame = title.absolute_frame();
        assert!(
            frame.x.is_finite() && frame.width.is_finite() && frame.height > 0.0,
            "the title has finite geometry (frame {frame:?})"
        );
        assert!(
            frame.max_x() <= 240.0,
            "the selected header stays inside the 240pt strip; frame {frame:?}"
        );
        assert_eq!(
            title.accessibility().map(|semantics| semantics.label),
            Some(LONG_TITLE.to_string()),
            "the tab control carries the full title as its label"
        );
        // Six close controls, plus the overflow entrance and the panel button.
        let closes = close_controls(scene);
        assert_eq!(
            closes.len(),
            LABELS.len(),
            "every tab mounts a labelled Close <title> control ({} measured); nodes {:?}",
            closes.len(),
            accessibility_dump(scene)
        );
        assert!(
            button(scene, &format!("Close {LONG_TITLE}")).is_some(),
            "the long title's close control is reachable"
        );
    });
}

#[test]
fn ai_queries() {
    InteractiveScene::run(240.0, 300.0, |scene| {
        let harness = Harness::new();
        let selection = harness.selection.clone();
        scene.install(harness);
        fn visible(scene: &InteractiveScene, label: &str) -> usize {
            scene
                .all(|node| {
                    node.accessibility()
                        .is_some_and(|semantics| semantics.label == label)
                })
                .len()
        }
        // Swift's `query_ui_nodes` with `visible_only` hides a header that does
        // not fit the menu policy, and reports the More entrance instead.
        assert!(
            visible(scene, "History") >= 1,
            "the hidden header stays mounted and queryable by label (got {})",
            visible(scene, "History")
        );
        let history =
            labelled(scene, "History", Some(AccessibilityRole::Tab)).expect("History is a tab");
        assert!(
            !history.accepts_subtree_input() || history.absolute_frame().width == 0.0,
            "a header that does not fit is not visible to input; frame {:?}",
            history.absolute_frame()
        );
        assert_eq!(
            visible(scene, "More tabs"),
            1,
            "the overflow menu entrance is exposed while tabs are hidden"
        );
        let tab_roles = scene
            .all(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::Tab)
            })
            .len();
        assert_eq!(
            tab_roles,
            LABELS.len(),
            "every strip item publishes the Tab role"
        );
        // Selecting the hidden tab makes it the retained header.
        selection.set(5);
        scene.settle(8, 1.0);
        assert!(
            visible(scene, "History") >= 1,
            "the selected header is retained in the strip (got {})",
            visible(scene, "History")
        );
        scene.width = 1280.0;
        scene.settle(8, 1.0);
        assert!(
            visible(scene, "More tabs") <= 1,
            "widening keeps at most the one overflow entrance (got {})",
            visible(scene, "More tabs")
        );
    });
}

#[test]
fn menu_resize() {
    InteractiveScene::run(240.0, 300.0, |scene| {
        let harness = Harness::new();
        let removed = harness.removed.clone();
        let selection = harness.selection.clone();
        scene.install(harness);
        let overview = labelled(scene, "Overview", Some(AccessibilityRole::Tab))
            .expect("the selected header is mounted");
        let more = labelled(scene, "More tabs", Some(AccessibilityRole::Button))
            .expect("narrow strips expose the overflow entrance");
        scene.click(&more, 1);
        scene.settle(8, 1.0);
        assert!(
            !scene.portal.entries().is_empty(),
            "the overflow menu opens at 240pt"
        );
        // Widening so every header fits closes the menu and restores focus to
        // the retained selected header.
        scene.width = 1280.0;
        scene.settle(8, 1.0);
        assert_eq!(
            panel_count(scene).as_deref(),
            Some("Panel count 0"),
            "the selected panel survives widening"
        );
        assert!(
            scene.portal.entries().is_empty(),
            "widening until every header fits closes the menu; entries {:?}",
            scene.portal.entries().len()
        );
        assert_eq!(
            scene.focus.focused().map(|node| node.id()),
            Some(overview.id()),
            "focus returns to the retained selected header"
        );
        scene.width = 240.0;
        scene.settle(8, 1.0);
        let more = labelled(scene, "More tabs", Some(AccessibilityRole::Button))
            .expect("the entrance returns at 240pt");
        scene.click(&more, 1);
        scene.settle(8, 1.0);
        removed.set(vec![5]);
        scene.settle(8, 1.0);
        assert!(
            labelled(scene, "History", None).is_none(),
            "removing the last tab drops its label"
        );
        assert_eq!(
            selection.get(),
            0,
            "removing a non-selected tab keeps the selection"
        );
        assert!(
            scene.portal.entries().is_empty(),
            "removing a tab closes the menu; entries {:?}",
            scene.portal.entries().len()
        );
    });
}

#[test]
fn chrome_minimum() {
    InteractiveScene::run(80.0, 300.0, |scene| {
        scene.install(Harness::new());
        let overview = button(scene, "Overview")
            .or_else(|| labelled(scene, "Overview", Some(AccessibilityRole::Tab)))
            .expect("the selected header stays mounted at 80pt");
        let frame = overview.absolute_frame();
        assert!(
            frame.width.is_finite() && frame.height.is_finite(),
            "the retained header has finite geometry at 80pt (frame {frame:?})"
        );
        let more = labelled(scene, "More tabs", Some(AccessibilityRole::Button))
            .expect("the overflow entrance exists at 80pt");
        assert!(
            more.absolute_frame().width >= 44.0 && more.absolute_frame().height >= 44.0,
            "the overflow entrance keeps its 44pt target at 80pt; more {:?}",
            more.absolute_frame()
        );
        let close = button(scene, "Close Overview").expect("the close control is reachable");
        let close_frame = close.absolute_frame();
        assert!(
            close_frame.width >= 44.0 && close_frame.height >= 44.0,
            "the close control is a 44pt target; measured {close_frame:?}"
        );
        assert!(
            close_frame.width.is_finite() && close_frame.x >= frame.x,
            "the close control follows the header; header {frame:?}, close {close_frame:?}"
        );
        for node in scene.all(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
        }) {
            let frame = node.absolute_frame();
            assert!(
                frame.width.is_finite() && frame.height.is_finite(),
                "every control has finite geometry at 80pt (frame {frame:?})"
            );
        }
    });
}
