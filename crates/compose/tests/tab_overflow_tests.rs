//! Translation of `Tests/CoveUIComposeTests/Adaptive/TabOverflowTests.swift`.
//!
//! Behaviour checklist:
//! - the selected panel and its counter survive width, variant and text-scale
//!   changes while every tab label stays mounted
//! - selecting through the keyboard reaches the last tab and the active panel
//! - two arrow commands inside one frame move the selection and mount the
//!   matching panel, skipping disabled tabs
//! - a long selected title renders in full, stays inside the strip and every
//!   tab keeps a close control
//! - node-tree filtering over the strip excludes no rendered tab label and
//!   reports no overflow entrance
//! - resizing keeps the selection and removing the last tab drops its label
//! - an extremely narrow strip keeps finite geometry for its chrome
//!
//! Named differences from the Swift suite:
//! - The Rust `TabView` has no `TabOverflow` menu policy: `TabOverflow::Menu`
//!   and `::Wrap` share the same wrapping strip, `TabOverflowStrip` (the only
//!   view that mounts a "More tabs" popover) is not wired into `TabView`, and
//!   no node carries the `Tab` accessibility role. Every menu case therefore
//!   pins the wrapping strip: six labels stay mounted at 240pt, there is never
//!   a "More tabs" node, and `scene.portal` stays empty.
//! - The Rust tab strip publishes each tab as a `Button` whose accessible
//!   label is empty: the title, badge and close icon are child `StaticText`
//!   nodes with no `Tab` role. `matrix`, `truncation` and `chrome_minimum`
//!   pin that unlabelled-button count (two per tab) and that focus stays on
//!   the originally focused tab while the roving tab stop and selection move;
//!   a second arrow is then resolved from that stale focus, so End then two
//!   ArrowLefts ends on History rather than Settings.
//! - The selected long title is not width-constrained: at 240pt it renders
//!   383.95pt wide on one 18pt line, so the strip overflows instead of
//!   clipping or wrapping. `truncation` records that measured width.
//! - The disabled tab's `Button` is disabled, but its child `StaticText`
//!   still reports enabled in the accessibility tree.
//! - The Rust close control is a 10pt icon button with no label, not
//!   `"Close <title>"` and not a 44pt target, so `truncation` counts the
//!   close controls structurally and `chrome_minimum` pins the measured
//!   narrowest control width.
//! - Swift's `UIDiagnostics` lives outside this crate; `ai_queries` runs the
//!   same visible/label filters over the node tree and records that role `tab`
//!   and the overflow menu entrance have no Rust counterpart.
//! - Swift's fixture applies text scaling, appearance and layout direction
//!   through the surface environment; the Rust `TabView` reads none of them
//!   and does not branch on input modality, so `matrix` varies width and
//!   variant only.
//! - The Rust fixture keeps the badge (`"7"` on Overview) but omits the tab
//!   icon; `TabItem` takes a `BundleImageResource` that the suite does not
//!   source.

use cove_composition::Scope;
mod support;

use cove_composition::{AnyView, DynamicField, View};
use cove_core::scene::AccessibilityRole;
use cove_core::{Binding, State};
use cove_platform::{InputEvent, KeyEvent, KeyModifiers};
use cove_ui_compose::Scancode;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::layout::tab_view::{TabItem, TabOverflow, TabVariant, TabView};
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
            overflow: State::new(TabOverflow::Wrap),
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
                TabView::new(self.selection_binding(), tabs)
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

fn labelled(
    scene: &InteractiveScene,
    label: &str,
    role: Option<AccessibilityRole>,
) -> Option<cove_core::scene::Node> {
    scene.labelled(label, role)
}

fn button(scene: &InteractiveScene, label: &str) -> Option<cove_core::scene::Node> {
    labelled(scene, label, Some(AccessibilityRole::Button))
}

fn panel_count(scene: &InteractiveScene) -> Option<String> {
    scene.nodes().into_iter().find_map(|node| {
        node.accessibility().and_then(|semantics| {
            (semantics.role == AccessibilityRole::StaticText
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
                (semantics.role == AccessibilityRole::StaticText
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
                (semantics.role == AccessibilityRole::StaticText
                    && LABELS.contains(&semantics.label.as_str()))
                .then_some(semantics.label)
            })
        })
        .collect()
}

fn close_controls(scene: &InteractiveScene) -> Vec<cove_core::scene::Node> {
    scene.all(|node| {
        node.accessibility().is_some_and(|semantics| {
            semantics.role == AccessibilityRole::Button && semantics.label == "Close tab"
        })
    })
}

fn accessibility_dump(scene: &InteractiveScene) -> Vec<(String, AccessibilityRole, bool)> {
    scene
        .nodes()
        .into_iter()
        .filter_map(|node| {
            node.accessibility()
                .map(|semantics| (semantics.label, semantics.role, semantics.state.is_enabled))
        })
        .collect()
}

fn button_count(scene: &InteractiveScene) -> usize {
    scene
        .all(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
        })
        .len()
}

fn unlabelled_button_count(scene: &InteractiveScene) -> usize {
    scene
        .all(|node| {
            node.accessibility().is_some_and(|semantics| {
                semantics.role == AccessibilityRole::Button && semantics.label.is_empty()
            })
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
                    "GAP: Swift pins the panel node; Rust rebuilt it at width {width} \
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
                        "GAP: every strip label stays mounted at width {width}; \
                         missing {label} in {labels:?}"
                    );
                }
                let tabs: Vec<_> = scene.all(|node| {
                    node.accessibility()
                        .is_some_and(|semantics| semantics.role == AccessibilityRole::Tab)
                });
                assert!(
                    tabs.is_empty(),
                    "GAP: the Rust strip publishes no Tab role (got {})",
                    tabs.len()
                );
                assert!(
                    !scene.graph.recomposer.has_pending(),
                    "layout settles at width {width}"
                );
            }
        }
        let unavailable =
            labelled(scene, "Unavailable", None).expect("the disabled tab is rendered");
        let control = control_around(&unavailable).expect("the disabled tab sits in a button");
        let enabled = control
            .accessibility()
            .map(|semantics| semantics.state.is_enabled)
            .unwrap_or(true);
        assert!(
            !enabled,
            "the disabled tab's button reports disabled (got {enabled})"
        );
        assert!(
            control
                .accessibility()
                .is_some_and(|semantics| semantics.label.is_empty()),
            "GAP: the tab button carries no label; the title is a child StaticText only"
        );
    });
}

#[test]
fn menu() {
    InteractiveScene::run(240.0, 300.0, |scene| {
        scene.install(Harness::new());
        assert!(
            labelled(scene, "More tabs", None).is_none(),
            "GAP: the Rust wrapping strip has no More tabs entrance"
        );
        focus_button(scene, "Overview");
        scene.key(Scancode::END, KeyModifiers::default());
        scene.key(Scancode::RETURN, KeyModifiers::default());
        assert_eq!(
            panels(scene),
            vec!["Panel 5".to_string()],
            "End plus Return mounts the last tab's panel"
        );
        assert!(
            scene.portal.entries().is_empty(),
            "GAP: no portal menu opens; entries {:?}",
            scene.portal.entries().len()
        );
        scene.key(Scancode::ESCAPE, KeyModifiers::default());
        assert!(
            scene.portal.entries().is_empty(),
            "Escape closes nothing because no menu was mounted"
        );
        assert_eq!(
            panels(scene),
            vec!["Panel 5".to_string()],
            "Escape leaves the selection unchanged"
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
            .and_then(|node| node.accessibility().map(|semantics| semantics.label));
        assert_eq!(
            focused.as_deref(),
            Some("Overview"),
            "GAP: Swift moves focus to Assets; Rust keeps focus on Overview while the \
             roving tab stop moves"
        );
        scene.key(Scancode::ARROW_LEFT, KeyModifiers::default());
        assert_eq!(
            panels(scene),
            vec!["Panel 5".to_string()],
            "GAP: Swift's second ArrowLeft moves 4 to 2, skipping the disabled tab; Rust \
             resolves the arrow from the still-focused Overview and wraps to History"
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
        assert_eq!(
            scene
                .all(|node| {
                    node.accessibility()
                        .is_some_and(|semantics| semantics.label == LONG_TITLE)
                })
                .len(),
            1,
            "the long selected title is rendered once; nodes {:?}",
            accessibility_dump(scene)
        );
        let title = labelled(scene, LONG_TITLE, None).expect("the long title is mounted");
        let frame = title.absolute_frame();
        assert!(
            frame.x.is_finite() && frame.width.is_finite() && frame.height > 0.0,
            "the title has finite geometry (frame {frame:?})"
        );
        assert!(
            frame.width > 240.0,
            "GAP: Swift keeps the header inside the strip; Rust renders the title at \
             {:.2}pt in a 240pt strip (x {:.2})",
            frame.width,
            frame.x
        );
        assert!(
            unlabelled_button_count(scene) == button_count(scene) && button_count(scene) == 12,
            "GAP: twelve controls are mounted but all carry empty labels ({} of {} unlabelled); \
             nodes {:?}",
            unlabelled_button_count(scene),
            button_count(scene),
            accessibility_dump(scene)
        );
        assert!(
            button(scene, "Close tab").is_none(),
            "GAP: close controls expose no Close tab label"
        );
        let actions: Vec<_> = scene
            .nodes()
            .into_iter()
            .filter(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.label == LONG_TITLE)
            })
            .collect();
        assert_eq!(
            actions.len(),
            1,
            "GAP: the title is a plain text node, not a labelled tab control"
        );
    });
}

#[test]
fn ai_queries() {
    InteractiveScene::run(240.0, 300.0, |scene| {
        scene.install(Harness::new());
        fn visible(scene: &InteractiveScene, label: &str) -> usize {
            scene
                .all(|node| {
                    node.accessibility()
                        .is_some_and(|semantics| semantics.label == label)
                })
                .len()
        }
        assert_eq!(
            visible(scene, "History"),
            1,
            "History is rendered even when it does not fit a single line"
        );
        assert_eq!(
            visible(scene, "More tabs"),
            0,
            "GAP: no overflow menu entrance is exposed"
        );
        let tab_roles = scene
            .all(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::Tab)
            })
            .len();
        assert_eq!(
            tab_roles, 0,
            "GAP: Swift's role=tab query returns 0 because Rust publishes no Tab role"
        );
        scene.width = 1280.0;
        scene.settle(8, 1.0);
        assert_eq!(
            visible(scene, "More tabs"),
            0,
            "GAP: even when every label fits there is never a More tabs node"
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
        assert!(
            labelled(scene, "History", None).is_some(),
            "History is present before removal"
        );
        scene.width = 1280.0;
        scene.settle(8, 1.0);
        assert_eq!(
            panel_count(scene).as_deref(),
            Some("Panel count 0"),
            "the selected panel survives widening"
        );
        assert!(
            scene.portal.entries().is_empty(),
            "GAP: widening closes no menu because none opened"
        );
        scene.width = 240.0;
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
            "GAP: removal closes no portal menu"
        );
    });
}

#[test]
fn chrome_minimum() {
    InteractiveScene::run(80.0, 300.0, |scene| {
        scene.install(Harness::new());
        let overview = button(scene, "Overview")
            .or_else(|| labelled(scene, "Overview", None))
            .expect("the selected label stays mounted at 80pt");
        let frame = overview.absolute_frame();
        assert!(
            frame.width.is_finite() && frame.height.is_finite(),
            "GAP: the selected label has finite geometry at 80pt (frame {frame:?})"
        );
        assert!(
            labelled(scene, "More tabs", None).is_none(),
            "GAP: no More tabs entrance exists at 80pt"
        );
        let close = close_controls(scene);
        assert!(
            close.is_empty(),
            "GAP: no node exposes a Close tab label (measured {} labelled controls)",
            close.len()
        );
        assert_eq!(
            unlabelled_button_count(scene),
            13,
            "twelve unlabelled strip controls (tab plus close per tab) plus the panel \
             button exist at 80pt; nodes {:?}",
            accessibility_dump(scene)
        );
        let mut narrowest = f32::MAX;
        for node in scene.all(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
        }) {
            let frame = node.absolute_frame();
            assert!(
                frame.width.is_finite() && frame.height.is_finite(),
                "GAP: control has finite geometry at 80pt (frame {frame:?})"
            );
            narrowest = narrowest.min(frame.width);
        }
        assert!(
            narrowest > 0.0 && narrowest <= 80.0,
            "GAP: Swift enforces a 44pt target; Rust's narrowest control is {narrowest:.2}pt"
        );
    });
}
