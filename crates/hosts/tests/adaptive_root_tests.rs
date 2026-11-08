//! The desktop/iOS root publishes every later host environment change, not
//! only the first frame's.

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::adaptive::SurfaceReader;
use cove_composition::{AnyView, EmptyView, ViewGraph};
use cove_core::scene::{NodeTree, Recomposer};
use cove_core::{DeviceClass, Environment, FoldableKind, Posture, State};
use cove_hosts::desktop::application::AdaptiveApplicationContent;

#[test]
fn environment_changes_after_install_reach_content() {
    changes_reach_content(AnyView::new);
}

#[test]
fn environment_changes_reach_content_under_the_host_text_scaler() {
    use cove_ui_compose::theme::tokens::TextScaler;
    changes_reach_content(|root| AnyView::new(root.text_scaler(TextScaler::identity())));
}

#[test]
fn environment_changes_reach_content_when_the_root_is_wrapped() {
    use cove_ui_compose::theme::tokens::{TextScaler, TextScalerViewExt};
    changes_reach_content(|root| {
        AnyView::new(AnyView::new(root).text_scaler(TextScaler::identity()))
    });
}

fn changes_reach_content(install: impl FnOnce(AdaptiveApplicationContent) -> AnyView) {
    let environment = State::new(Environment::default());
    let seen: Rc<RefCell<Vec<(f32, DeviceClass, Posture)>>> = Rc::default();
    let recorded = seen.clone();
    let content = AnyView::new(SurfaceReader::new(move |surface: &Environment| {
        recorded.borrow_mut().push((
            surface.viewport.width,
            surface.device_class,
            surface.posture,
        ));
        AnyView::new(EmptyView)
    }));
    let graph = Rc::new(ViewGraph::new(
        Rc::new(RefCell::new(NodeTree::new())),
        Rc::new(Recomposer::new()),
    ));
    graph.install_view(
        install(AdaptiveApplicationContent::new(
            content,
            environment.clone(),
        )),
        Default::default(),
    );
    assert_eq!(seen.borrow().last().map(|entry| entry.0), Some(1280.0));

    let mut next = environment.get();
    next.viewport.width = 841.0;
    next.device_class = DeviceClass::Foldable(FoldableKind::Book);
    next.posture = Posture::Book;
    environment.set(next);
    graph.recomposer.commit_all();

    assert_eq!(
        seen.borrow().last().copied(),
        Some((
            841.0,
            DeviceClass::Foldable(FoldableKind::Book),
            Posture::Book
        )),
        "a resize and a device change after install reach the content"
    );
}
