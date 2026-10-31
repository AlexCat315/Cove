mod support;
use cove_composition::{AnyView, DynamicField, LayoutDebugExt, View};
use cove_core::State;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::layout::responsive::ViewThatFits;
use cove_ui_compose::primitives::text::Text;

#[derive(Clone)]
struct S(&'static str);
impl View for S {
    fn body(&self) -> AnyView { AnyView::new(AnyView::new(Text::new("x")).debug_name(self.0)) }
    fn tag(&self) -> &'static str { "S" }
}

struct H { width: State<f32> }
impl View for H {
    fn body(&self) -> AnyView {
        AnyView::new(AnyView::new(ViewThatFits::threshold(600.0, S("wide"), S("compact"))).frame_size(self.width.get(), 100.0))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> { vec![DynamicField::state("width", &self.width)] }
    fn tag(&self) -> &'static str { "H" }
}

#[test]
fn probe() {
    support::GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let g = support::graph();
            g.install(H { width: State::new(700.0f32) }, Default::default());
            for _ in 0..3 { g.recomposer.commit_all(); g.compute_layout(700.0, 100.0); }
            let root = g.tree.borrow().root().unwrap();
            fn walk(n: &cove_core::scene::Node, d: usize) {
                println!("{:indent$}{:?} frame={:?} tag={:?} hit={} int={}", "", n.id(), n.frame(),
                    n.view_tag().map(|t| t.rsplit("::").next().unwrap_or("").to_string()),
                    n.allows_hit_testing(), n.is_interaction_enabled(), indent=d);
                for c in n.children() { walk(&c, d+1); }
            }
            walk(&root, 0);
        });
    });
}
