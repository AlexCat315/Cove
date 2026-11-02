//! Mounted session state survives parent reconstruction and drives real input.
use crate::support;
use cove_composition::{AnyView, Scope, View};
use cove_core::scene::{AccessibilityRole, AccessibilityTree, AnimatorScheduler, Node};
use cove_core::{Binding, Owner, Signal};
use cove_platform::{InputEvent, KeyModifiers};
use cove_ui_compose::primitives::controls::fields::combobox::{Combobox, ComboboxSearch};
use cove_ui_compose::primitives::controls::fields::select::SelectOption;
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use support::InteractiveScene;

#[derive(Clone)]
struct SearchHarness {
    revision: Signal<u32>,
    selection: Binding<Option<i32>>,
    calls: Arc<Mutex<Vec<String>>>,
}
impl View for SearchHarness {
    fn body(&self, _: &Scope) -> AnyView {
        let _ = self.revision.get();
        let calls = self.calls.clone();
        let mut search = ComboboxSearch::new(move |query| {
            calls.lock().unwrap().push(query.to_string());
            Ok(vec![SelectOption::new(1, format!("Result {query}"), true)])
        });
        search.delay_ms = 100;
        AnyView::new(LayerRoot::new(Combobox::search(
            self.selection.clone(),
            None,
            search,
        )))
    }
}
fn trigger(scene: &InteractiveScene) -> Node {
    scene
        .first(|node| {
            node.accessibility()
                .is_some_and(|a| a.role == AccessibilityRole::ComboBox)
        })
        .expect("combobox trigger")
}
fn field(scene: &InteractiveScene) -> Node {
    scene
        .first(|node| {
            node.accessibility()
                .is_some_and(|a| a.role == AccessibilityRole::TextField)
        })
        .expect("search input")
}
fn pump_until(scene: &mut InteractiveScene, predicate: impl Fn(&InteractiveScene) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while {
        cove_core::reactive::drain_posted_signal_updates();
        !predicate(scene)
    } {
        assert!(Instant::now() < deadline, "query completion timed out");
        std::thread::sleep(Duration::from_millis(1));
        scene.settle(2, 0.01);
    }
    cove_core::reactive::drain_posted_signal_updates();
    scene.settle(3, 0.01);
}
#[test]
fn remote_search_debounces_and_survives_parent_rebuilds() {
    InteractiveScene::run(640.0, 600.0, |scene| {
        let owner = Owner::new();
        let selection = owner.signal(None::<i32>);
        let revision = owner.signal(0_u32);
        let calls = Arc::new(Mutex::new(Vec::new()));
        scene.install(SearchHarness {
            revision,
            selection: selection.binding(),
            calls: calls.clone(),
        });
        trigger(scene).accessibility_actions().activate.unwrap()();
        scene.settle(2, 0.01);
        pump_until(scene, |_| calls.lock().unwrap().contains(&String::new()));
        let input = field(scene);
        scene.focus.focus(Some(&input), true);
        scene
            .dispatcher()
            .dispatch(&InputEvent::TextInput("a".into()));
        scene.graph.recomposer.commit_all();
        scene
            .dispatcher()
            .dispatch(&InputEvent::TextInput("b".into()));
        scene.graph.recomposer.commit_all();
        assert_eq!(
            *calls.lock().unwrap(),
            [String::new()],
            "typing waits for the quiet period"
        );
        revision.set(1);
        scene.graph.recomposer.commit_all();
        scene.graph.compute_layout(scene.width, scene.height);
        assert_eq!(
            field(scene).id(),
            input.id(),
            "parent reconstruction retains the search input"
        );
        assert_eq!(field(scene).resolved_accessibility_value(), "ab");
        AnimatorScheduler::current().tick(0.11);
        pump_until(scene, |scene| {
            scene
                .first(|node| AccessibilityTree::resolved_label(node) == "Result ab")
                .is_some()
        });
        assert_eq!(
            *calls.lock().unwrap(),
            [String::new(), "ab".to_string()],
            "the superseded query never executes"
        );
        scene.focus.focus(Some(&field(scene)), true);
        scene.key(cove_ui_compose::Scancode::RETURN, KeyModifiers::default());
        assert_eq!(selection.get(), Some(1));
        assert!(
            scene.portal.entries().is_empty(),
            "committing closes the results"
        );
        assert_eq!(
            AccessibilityTree::resolved_label(&trigger(scene)),
            "Result ab"
        );
    });
}
#[test]
fn closing_a_combobox_cancels_the_debounced_request() {
    InteractiveScene::run(640.0, 600.0, |scene| {
        let owner = Owner::new();
        let selection = owner.signal(None::<i32>);
        let revision = owner.signal(0_u32);
        let calls = Arc::new(Mutex::new(Vec::new()));
        scene.install(SearchHarness {
            revision,
            selection: selection.binding(),
            calls: calls.clone(),
        });
        trigger(scene).accessibility_actions().activate.unwrap()();
        scene.settle(2, 0.01);
        pump_until(scene, |_| !calls.lock().unwrap().is_empty());
        scene.focus.focus(Some(&field(scene)), true);
        scene
            .dispatcher()
            .dispatch(&InputEvent::TextInput("cancelled".into()));
        scene.graph.recomposer.commit_all();
        scene
            .dispatcher()
            .dispatch(&InputEvent::KeyDown(cove_platform::KeyEvent {
                scancode: cove_ui_compose::Scancode::ESCAPE,
                keycode: 0,
                modifiers: KeyModifiers::default(),
                is_repeat: false,
            }));
        scene.settle(3, 0.01);
        AnimatorScheduler::current().tick(0.2);
        scene.settle(3, 0.01);
        assert_eq!(
            *calls.lock().unwrap(),
            [String::new()],
            "dismissed requests never call the data source"
        );
        assert!(scene.portal.entries().is_empty());
    });
}
