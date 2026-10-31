//! Translation of `Tests/CoveUIComposeTests/ScratchGeometryTests.swift`.
//!
//! The Swift suite is a scratch/debug-only `@Test("print")` that walks the
//! materialised graph and logs node geometry; it has no assertions. The Rust
//! version keeps the same graph build (a clearable Combobox inside a 390x200
//! column, given a percentage width) and turns it into a smoke test that
//! verifies the option label materialises as measured text.

mod support;

use cove_core::Binding;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui_compose::modifiers::layout::{FrameDimension, LayoutFrameExt};
use cove_ui_compose::primitives::controls::fields::{Combobox, SelectOption};
use cove_ui_compose::{AnyView, BoxView, TupleView};

use support::{GlobalTestLock, with_test_text_environment};

fn collect(root: &cove_core::scene::Node, into: &mut Vec<cove_core::scene::Node>) {
    into.push(root.clone());
    for child in root.children() {
        collect(&child, into);
    }
}

#[test]
fn combobox_graph_materialises_option_label() {
    GlobalTestLock::locked(|| {
        with_test_text_environment(|| {
            let long_label = "Workspace permissions and international collaboration 👩🏽‍💻";
            let combobox = Combobox::new(
                Binding::constant(Some(0)),
                vec![
                    SelectOption::new(0, long_label, true),
                    SelectOption::new(1, "Personal workspace", true),
                ],
            )
            .frame(
                Some(FrameDimension::Percent(100.0)),
                None,
                None,
                None,
                None,
                None,
            );

            let harness = BoxView::new(
                FlexDirection::Column,
                Align::FlexStart,
                Justify::Center,
                Wrap::NoWrap,
                12.0,
                AnyView::new(TupleView::new(vec![AnyView::new(combobox)])),
            )
            .frame(
                Some(FrameDimension::Points(390.0)),
                Some(FrameDimension::Points(200.0)),
                None,
                None,
                None,
                None,
            );

            let graph = support::graph();
            graph.install_view(AnyView::new(harness), Default::default());
            for _ in 0..10 {
                graph.recomposer.commit_all();
                graph.compute_layout(390.0, 200.0);
            }

            let root = graph.tree.borrow().root().expect("root node");
            let mut nodes = Vec::new();
            collect(&root, &mut nodes);

            let label_materialises = nodes.iter().any(|node| {
                node.layout_node()
                    .and_then(|layout| layout.text_inputs())
                    .is_some_and(|inputs| inputs.text == long_label)
            });
            assert!(
                label_materialises,
                "the clearable Combobox option label materialises"
            );
        });
    });
}
