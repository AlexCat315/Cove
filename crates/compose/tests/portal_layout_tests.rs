//! Translation of `Tests/CoveUIComposeTests/PortalLayoutTests.swift`.
//!
//! Behaviour checklist:
//! - `PortalHost` keeps rendering its owning window's store after the ambient
//!   store changes, so an entry registered later on the owning store still lays
//!   out at its authored position
//! - the portal layer takes no part in the content's flex distribution: the
//!   header keeps its 40pt, the fill takes the remaining 160pt, the portal layer
//!   spans the full 200pt viewport, and its entry sits at its authored origin

mod support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, LayoutDebugExt, ViewExt, ViewGraph};
use cove_core::Point;
use cove_core::scene::{Align, FlexDirection, Justify, NodeTree, Recomposer, Wrap};
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
use cove_ui_compose::primitives::overlays::portal::{
    LayerRoot, PortalHost, PortalStore, PortalStoreEnvironment, PortalStoreHolder,
};
use cove_ui_compose::{BoxView, LayoutDebugExt as _};

use support::GlobalTestLock;

struct StoreScope(Rc<PortalStore>);
impl cove_composition::ViewModifier for StoreScope {
    fn scope_apply(&self, node: &cove_core::scene::Node) -> Option<bool> {
        let key = PortalStoreEnvironment::key();
        let changed = node
            .composition_value(&key)
            .is_none_or(|store| !Rc::ptr_eq(&store, &self.0));
        node.set_composition_value(&key, Some(self.0.clone()));
        Some(changed)
    }
    fn is_scope_applying(&self) -> bool {
        true
    }
}

fn empty_box() -> BoxView {
    BoxView::new(
        FlexDirection::Column,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        0.0,
        AnyView::new(cove_composition::EmptyView),
    )
}

fn store() -> Rc<PortalStore> {
    Rc::new(PortalStore::new())
}

#[test]
fn portal_host_keeps_rendering_its_owning_store() {
    GlobalTestLock::locked(|| {
        let previous_store = PortalStoreHolder::current();
        let owning_store = store();
        let unrelated_store = store();
        PortalStoreHolder::set_current(Some(owning_store.clone()));

        let graph = Rc::new(ViewGraph::new(
            Rc::new(RefCell::new(NodeTree::new())),
            Rc::new(Recomposer::new()),
        ));
        graph.install(
            LayerRoot::with_portals(
                AnyView::new(empty_box()).frame_size(300.0, 200.0),
                PortalHost::new(),
            )
            .modifier(StoreScope(owning_store.clone())),
            Default::default(),
        );

        // Application scopes the ambient per window. Recomposition can be
        // requested after that scope unwinds, so the host must not resolve its
        // entries from whichever store happens to be ambient later.
        PortalStoreHolder::set_current(Some(unrelated_store.clone()));
        owning_store.register(
            Some("owning-window-popover".to_string()),
            Point::new(30.0, 40.0),
            Some(120.0),
            AnyView::new(AnyView::new(empty_box()).frame_height(60.0)),
        );
        graph.recomposer.commit_all();
        graph.compute_layout(300.0, 200.0);

        let snapshot = graph.layout_snapshot();
        let entry = snapshot
            .iter()
            .find(|entry| entry.debug_name.as_deref() == Some("owning-window-popover"))
            .expect("the owning store's entry is laid out");
        assert_eq!(
            (entry.absolute_frame.x, entry.absolute_frame.y),
            (30.0, 40.0),
            "the entry sits at its authored position from the owning store"
        );

        owning_store.clear();
        unrelated_store.clear();
        PortalStoreHolder::set_current(Some(previous_store));
    });
}

#[test]
fn portal_host_does_not_participate_in_content_flex() {
    GlobalTestLock::locked(|| {
        PortalStoreHolder::current().clear();
        PortalStoreHolder::current().register(
            Some("test-popover".to_string()),
            Point::new(24.0, 32.0),
            Some(120.0),
            AnyView::new(
                AnyView::new(empty_box())
                    .frame_height(96.0)
                    .debug_name("portal-content"),
            ),
        );

        let graph = Rc::new(ViewGraph::new(
            Rc::new(RefCell::new(NodeTree::new())),
            Rc::new(Recomposer::new()),
        ));
        graph.install(
            LayerRoot::with_portals(
                AnyView::new(
                    AnyView::new(BoxView::new(
                        FlexDirection::Column,
                        Align::Stretch,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        0.0,
                        AnyView::new(cove_composition::TupleView::new(vec![
                            AnyView::new(
                                AnyView::new(empty_box())
                                    .frame_height(40.0)
                                    .debug_name("main-header"),
                            ),
                            AnyView::new(empty_box().flex(1.0).debug_name("main-fill")),
                        ])),
                    ))
                    .flex(1.0)
                    .debug_name("main-content"),
                ),
                PortalHost::new(),
            ),
            Default::default(),
        );

        graph.compute_layout(300.0, 200.0);
        let snapshot = graph.layout_snapshot();

        let header = snapshot
            .iter()
            .find(|entry| entry.debug_name.as_deref() == Some("main-header"))
            .expect("header");
        assert_eq!(
            header.absolute_frame.height, 40.0,
            "the header keeps its 40pt"
        );

        let fill = snapshot
            .iter()
            .find(|entry| entry.debug_name.as_deref() == Some("main-fill"))
            .expect("fill");
        assert_eq!(
            fill.absolute_frame.y, 40.0,
            "the fill starts below the header"
        );
        assert_eq!(
            fill.absolute_frame.height, 160.0,
            "the fill takes the remaining 160pt"
        );

        let layer = snapshot
            .iter()
            .find(|entry| entry.layout_role.as_deref() == Some("portal-layer"))
            .expect("portal layer");
        assert_eq!(
            layer.absolute_frame.height, 200.0,
            "the portal layer spans the viewport without consuming flex"
        );

        let popover = snapshot
            .iter()
            .find(|entry| entry.debug_name.as_deref() == Some("test-popover"))
            .expect("popover entry");
        assert_eq!(popover.absolute_frame.x, 24.0, "the entry keeps its x");
        assert_eq!(popover.absolute_frame.y, 32.0, "the entry keeps its y");

        PortalStoreHolder::current().clear();
    });
}
