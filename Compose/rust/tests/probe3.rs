mod support;
use cove_composition::{AnyView, View};
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui_compose::BoxView;
use cove_ui_compose::modifiers::layout::FlexExt;
use support::GlobalTestLock;

#[test]
fn probe() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        let col = AnyView::new(BoxView::new(FlexDirection::Column, Align::Stretch, Justify::FlexStart, Wrap::NoWrap, 0.0,
            AnyView::new(cove_composition::EmptyView)));
        graph.install_view(AnyView::new(AnyView::new(col).flex_with(1.0,1.0,Some(0.0))), Default::default());
        graph.recomposer.commit_all();
        graph.layout_root().set_height(Some(852.0));
        graph.compute_layout(393.0, 852.0);
        let r = graph.tree.borrow().root().expect("root");
        for c in r.children() { println!("child frame={:?} layout={:?}", (c.frame().width, c.frame().height), c.layout_node().map(|l| (l.frame().width, l.frame().height))); }
        println!("root layout={:?}", graph.layout_root().frame());
    });
}
