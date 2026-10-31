mod support;
use std::cell::RefCell;
use std::rc::Rc;
use cove_composition::{AnyView, View};
use cove_core::scene::{AccessibilityTree, Node};
use cove_core::{Environment, InputCapabilities, State};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::overlays::modal::action_sheet::{
    ActionSheet, ActionSheetAction, ActionSheetQuestion,
};
use cove_ui_compose::primitives::text::Text;
use support::InteractiveScene;

struct Harness { open: State<bool>, calls: Rc<RefCell<i32>>, width: f32, touch: bool }

impl View for Harness {
    fn body(&self) -> AnyView {
        let open = self.open.get();
        let calls = self.calls.clone();
        let question = ActionSheetQuestion::new(
            "Project actions",
            "Choose what happens to this project.",
            vec![
                ActionSheetAction::new("duplicate", "Duplicate project", move || { *calls.borrow_mut() += 1; }),
                ActionSheetAction::new("export", "Export project", || {}).enabled(false),
                ActionSheetAction::new("delete", "Delete project", || {}),
            ],
            "Dismiss actions",
        );
        let sheet = ActionSheet::new(self.open.binding(), question, Text::new("More actions"));
        let mut env = Environment::default();
        env.viewport.width = self.width;
        env.viewport.height = 820.0;
        env.input = if self.touch { InputCapabilities::TOUCH } else { InputCapabilities::POINTER };
        AnyView::new(AnyView::new(LayerRoot::new(
            AnyView::new(sheet).frame_width(self.width).frame_height(820.0))).surface_environment(env))
    }
    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state("open", &self.open)]
    }
}

fn collect(n: &Node) -> Vec<Node> {
    let mut v = vec![n.clone()];
    for c in n.children() { v.extend(collect(&c)); }
    v
}

fn walk(n: &Node, d: usize) {
    if d > 40 { return; }
    let a = n.accessibility();
    eprintln!("{}{} f={:?} role={:?} lbl={:?} en={:?} help={:?}",
        "  ".repeat(d),
        n.view_tag().unwrap_or_default().rsplit("::").next().unwrap_or(""),
        n.absolute_frame(),
        a.as_ref().map(|s| s.role),
        a.as_ref().map(|s| s.label.clone()),
        a.as_ref().map(|s| s.state.is_enabled),
        a.as_ref().map(|s| s.help.clone()));
    for c in n.children() { walk(&c, d+1); }
}

#[test]
fn probe() {
    for width in [390.0f32, 1280.0] {
        InteractiveScene::run(width, 820.0, |scene| {
            let open = State::new(false);
            scene.install(AnyView::new(Harness { open: open.clone(), calls: Rc::new(RefCell::new(0)), width, touch: true }));
            eprintln!("=== {width} closed portals={}", scene.portal.entries().len());
            open.set(true);
            scene.settle(8, 1.0);
            eprintln!("=== {width} OPEN portals={} pending={}", scene.portal.entries().len(), scene.graph.recomposer.has_pending());
            if let Some(root) = scene.graph.tree.borrow().root() {
                eprintln!("--- rows:");
                for n in collect(&root) {
                    if let Some(a) = n.accessibility()
                        && matches!(a.label.as_str(), "Duplicate project"|"Export project"|"Delete project"|"Dismiss actions")
                    { eprintln!("   {} f={:?} role={:?} lbl={:?} en={:?}", n.view_tag().unwrap_or_default().rsplit("::").next().unwrap_or(""), n.absolute_frame(), a.role, a.label, a.state.is_enabled); }
                }
                walk(&root, 0);
            }
            eprintln!("--- snapshot {}", if let Some(r) = scene.graph.tree.borrow().root() { AccessibilityTree::snapshot(&r).len() } else { 0 });
        });
    }
}
