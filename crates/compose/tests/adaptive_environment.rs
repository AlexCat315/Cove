//! Translation of `Tests/CoveUIComposeTests/AdaptiveEnvironmentTests.swift`.
//!
//! Behaviour checklist:
//! - `WidthClassReader` follows the surface breakpoints independently of input
//!   density (599 → compact, 600/999 → medium, 1000 → expanded)
//! - the layout density floor preserves authored minima and their changes while
//!   active, and restores the authored minima when cleared

use crate::support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::AnyView;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_core::{Environment, InputCapabilities, Size};
use cove_ui_compose::BoxView;
use cove_ui_compose::adaptive::{SurfaceEnvironmentExt, WidthClassReader};
use cove_ui_compose::modifiers::layout::{FrameDimension, LayoutFrameExt};

use support::GlobalTestLock;

#[test]
fn width_classes_follow_surface_breakpoints() {
    GlobalTestLock::locked(|| {
        let reads: Rc<RefCell<Vec<cove_core::SizeClass>>> = Rc::new(RefCell::new(Vec::new()));
        let graph = support::graph();
        for (width, expected) in [
            (599.0_f32, cove_core::SizeClass::Compact),
            (600.0, cove_core::SizeClass::Medium),
            (999.0, cove_core::SizeClass::Medium),
            (1000.0, cove_core::SizeClass::Expanded),
        ] {
            let mut surface = Environment::default();
            surface.viewport.width = width;
            surface.input = InputCapabilities::TOUCH;

            let recorded = reads.clone();
            let reader = WidthClassReader::new(move |value| {
                recorded.borrow_mut().push(value);
                AnyView::new(cove_composition::EmptyView)
            });
            graph.install_view(
                AnyView::new(reader.surface_environment(surface)),
                Default::default(),
            );
            assert_eq!(
                reads.borrow().last().copied(),
                Some(expected),
                "width {width} resolves to {expected:?}"
            );
        }
    });
}

#[test]
fn minimum_floor_preserves_authored_minima() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        let boxed = AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(cove_composition::EmptyView),
            ))
            .frame(
                Some(FrameDimension::Points(20.0)),
                Some(FrameDimension::Points(20.0)),
                Some(30.0),
                Some(32.0),
                None,
                None,
            ),
        );
        graph.install_view(boxed, Default::default());
        graph.compute_layout(300.0, 200.0);

        let node = graph
            .tree
            .borrow()
            .root()
            .and_then(|root| root.children().first().cloned())
            .expect("boxed node");
        let layout = node.layout_node().expect("layout node");

        layout.set_minimum_size_floor(Some(Size {
            width: 44.0,
            height: 44.0,
        }));
        graph.compute_layout(300.0, 200.0);
        assert_eq!(
            (node.frame().width, node.frame().height),
            (44.0, 44.0),
            "the floor raises the authored minimum"
        );
        assert_eq!(
            (layout.min_width(), layout.min_height()),
            (Some(30.0), Some(32.0)),
            "the authored minima are preserved underneath the floor"
        );

        layout.set_min_width(Some(60.0));
        layout.set_min_height(Some(36.0));
        graph.compute_layout(300.0, 200.0);
        assert_eq!(
            (node.frame().width, node.frame().height),
            (60.0, 44.0),
            "width follows the new authored minimum past the floor, height stays floored"
        );

        layout.set_minimum_size_floor(None);
        graph.compute_layout(300.0, 200.0);
        assert_eq!(
            (node.frame().width, node.frame().height),
            (60.0, 36.0),
            "clearing the floor restores the authored minima"
        );

        layout.set_min_width(None);
        layout.set_min_height(None);
        graph.compute_layout(300.0, 200.0);
        assert_eq!(
            (node.frame().width, node.frame().height),
            (20.0, 20.0),
            "with no minima the frame returns to the authored size"
        );
    });
}
