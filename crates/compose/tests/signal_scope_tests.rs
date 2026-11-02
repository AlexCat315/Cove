//! Scope-owned state: `cx.state` signals survive parent rebuilds, reads
//! through a `Binding` subscribe the reader, and primitives materialised by a
//! component see writes the component itself never read.

use crate::support;

use std::cell::Cell;
use std::rc::Rc;

use cove_composition::{AnyView, PrimitiveView, Scope, View, impl_primitive_view};
use cove_core::scene::{AccessibilityRole, AccessibilityTree, LayoutNode, Node};
use cove_core::{Binding, Signal};
use cove_ui_compose::primitives::controls::MenuButtonTitle;
use cove_ui_compose::primitives::overlays::menu::{MenuEntry, MenuItem};
use cove_ui_compose::primitives::overlays::portal::LayerRoot;

use support::InteractiveScene;

/// Publishes the bound value as its view tag from `update_node`, the way a
/// presenter primitive reads `is_presented`.
struct Probe {
    value: Binding<i32>,
}

impl PrimitiveView for Probe {
    fn make_node(&self) -> Node {
        Node::new()
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        node.set_view_tag(Some(format!("probe:{}", self.value.get())));
    }
}
impl_primitive_view!(Probe);

/// Owns `count` but never reads it; only the probe below it does.
struct Owner {
    count: Rc<Cell<Option<Signal<i32>>>>,
    bodies: Rc<Cell<usize>>,
}

impl View for Owner {
    fn body(&self, cx: &Scope) -> AnyView {
        self.bodies.set(self.bodies.get() + 1);
        let count = cx.state(|| 0);
        self.count.set(Some(count));
        AnyView::new(Probe {
            value: count.into(),
        })
    }
}

/// Rebuilds its child value whenever `generation` changes.
struct Parent {
    generation: Signal<u32>,
    count: Rc<Cell<Option<Signal<i32>>>>,
    bodies: Rc<Cell<usize>>,
}

impl View for Parent {
    fn body(&self, _cx: &Scope) -> AnyView {
        self.generation.get();
        AnyView::new(Owner {
            count: self.count.clone(),
            bodies: self.bodies.clone(),
        })
    }
}

fn probe_value(scene: &InteractiveScene) -> Option<String> {
    scene
        .first(|node| node.view_tag().is_some_and(|tag| tag.starts_with("probe:")))
        .and_then(|node| node.view_tag())
}

#[test]
fn state_survives_parent_rebuilds_and_primitives_observe_writes() {
    InteractiveScene::run(400.0, 300.0, |scene| {
        let generation = Signal::detached(0_u32);
        let count = Rc::new(Cell::new(None));
        let bodies = Rc::new(Cell::new(0));
        scene.install(Parent {
            generation,
            count: count.clone(),
            bodies: bodies.clone(),
        });
        let signal = count.get().expect("the owner ran its body");
        assert_eq!(probe_value(scene).as_deref(), Some("probe:0"));

        signal.set(3);
        scene.settle(1, 1.0);
        assert_eq!(
            probe_value(scene).as_deref(),
            Some("probe:3"),
            "a primitive under the owner sees a write the owner never read"
        );

        let before = bodies.get();
        generation.set(1);
        scene.settle(1, 1.0);
        assert!(bodies.get() > before, "the parent rebuilt the owner");
        assert_eq!(
            count.get(),
            Some(signal),
            "the rebuilt owner kept its signal"
        );
        assert_eq!(probe_value(scene).as_deref(), Some("probe:3"));
        generation.dispose();
    });
}

#[test]
fn menu_button_opens_and_an_item_closes_it() {
    InteractiveScene::run(800.0, 600.0, |scene| {
        let ran = Rc::new(Cell::new(0));
        let tally = ran.clone();
        scene.install(AnyView::new(LayerRoot::new(AnyView::new(
            MenuButtonTitle::new(
                "Options",
                vec![MenuEntry::Item(MenuItem::new(1, "Save", move || {
                    tally.set(tally.get() + 1)
                }))],
            ),
        ))));
        assert!(scene.portal.entries().is_empty());

        let trigger = scene
            .first(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
                    && AccessibilityTree::resolved_label(node) == "Options"
            })
            .expect("the trigger is mounted");
        scene.click(&trigger, 1);
        assert_eq!(scene.portal.entries().len(), 1, "the click opens the menu");

        let save = scene
            .labelled("Save", None)
            .expect("the menu item is mounted");
        scene.click(&save, 1);
        assert_eq!(ran.get(), 1, "the item ran its action");
        assert!(
            scene.portal.entries().is_empty(),
            "activating an item closes the menu"
        );
    });
}

#[test]
fn a_trigger_description_names_the_popover_trigger() {
    use cove_ui_compose::primitives::overlays::{Popover, PopoverTriggerDescription};
    use cove_ui_compose::primitives::text::Text;
    InteractiveScene::run(400.0, 300.0, |scene| {
        scene.install(AnyView::new(LayerRoot::new(AnyView::new(
            Popover::new(Text::new("Open"), Text::new("Body")).trigger_description(
                PopoverTriggerDescription {
                    label: "Described".to_string(),
                    value: "now".to_string(),
                    help: None,
                },
            ),
        ))));
        let trigger = scene
            .first(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
            })
            .expect("the trigger is mounted");
        let semantics = trigger.accessibility().expect("semantics");
        assert_eq!(semantics.label, "Described");
        assert_eq!(semantics.value, "now");
    });
}
