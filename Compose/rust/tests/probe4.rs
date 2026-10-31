mod support;
use cove_composition::{AnyView, LayoutDebugExt, View};
use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};
use cove_ui_compose::BoxView;
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
use support::GlobalTestLock;

fn find(root: &Node, name: &str) -> Option<Node> {
    support::find_named(root, name)
}

#[test]
fn probe() {
    GlobalTestLock::locked(|| {
        for explicit in [false, true] {
            let graph = support::graph();
            let fill = AnyView::new(AnyView::new(BoxView::new(FlexDirection::Column, Align::Stretch, Justify::FlexStart, Wrap::NoWrap, 0.0,
                AnyView::new(cove_composition::EmptyView))).flex_with(1.0,1.0,Some(0.0))).debug_name("fill");
            let inner = AnyView::new(BoxView::new(FlexDirection::Column, Align::Stretch, Justify::FlexStart, Wrap::NoWrap, 0.0, fill));
            let root = if explicit { AnyView::new(AnyView::new(inner).frame_size(393.0, 852.0)) } else { inner };
            graph.install_view(root, Default::default());
            graph.recomposer.commit_all();
            graph.compute_layout(393.0, 852.0);
            let r = graph.tree.borrow().root().expect("root");
            let n = find(&r, "fill").expect("fill");
            println!("explicit={explicit} root={:?} fill={:?}", (r.frame().width, r.frame().height), (n.frame().width, n.frame().height));
        }
    });
}
