//! Compiles the actual template UI against the public facade and drives it.
#[path = "../Templates/app/src/app.rs"]
mod app;
#[allow(dead_code)]
#[path = "../crates/compose/tests/support/mod.rs"]
mod support;

mod resources {
    pub fn welcome() -> String {
        include_str!("../Templates/app/Assets/welcome.txt")
            .trim()
            .into()
    }
}

use cove_composition::{AnyView, Scope, View};
use cove_core::scene::{AccessibilityRole, AccessibilityTree};
use cove_core::{Owner, Signal};
use cove_platform::InputEvent;
use support::InteractiveScene;

cove_ui::application!(launch = app::application);

struct Parent(Signal<u32>);
impl View for Parent {
    fn body(&self, _: &Scope) -> AnyView {
        self.0.track();
        application().content()
    }
}

#[test]
fn template_controls_recompose_and_survive_parent_reconstruction() {
    InteractiveScene::run(600.0, 400.0, |scene| {
        let owner = Owner::new();
        let revision = owner.signal(0_u32);
        scene.install(Parent(revision));
        let has_text = |text: &str| {
            scene
                .first(|node| {
                    node.layout_node()
                        .and_then(|layout| layout.text_inputs())
                        .is_some_and(|inputs| inputs.text == text)
                })
                .is_some()
        };
        assert!(has_text("Count: 0"));
        let add = scene
            .first(|node| {
                node.is_focusable() && AccessibilityTree::resolved_label(node) == "Add one"
            })
            .unwrap();
        scene.click(&add, 1);
        assert!(has_text("Count: 1"));
        let field = scene
            .first(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::TextField)
            })
            .unwrap();
        scene.focus.focus(Some(&field), true);
        scene
            .dispatcher()
            .dispatch(&InputEvent::TextInput("Ada".into()));
        scene.settle(3, 0.01);
        let entered = field.resolved_accessibility_value();
        assert!(entered.contains("Ada"));
        revision.set(1);
        scene.settle(3, 0.01);
        assert!(has_text("Count: 1"));
        assert_eq!(field.resolved_accessibility_value(), entered);
        let reset = scene
            .first(|node| node.is_focusable() && AccessibilityTree::resolved_label(node) == "Reset")
            .unwrap();
        scene.click(&reset, 1);
        assert!(has_text("Count: 0"));
        assert_eq!(field.resolved_accessibility_value(), entered);
    });
}
