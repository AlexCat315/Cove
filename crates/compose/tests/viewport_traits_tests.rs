//! Translation of `Tests/CoveUIComposeTests/ViewportTraitsTests.swift`.
//!
//! Behaviour checklist:
//! - the width/height thresholds derive the size class and orientation
//! - `WidthClassReader`, `HeightClassReader` and `OrientationReader` report the
//!   surface environment, and recompose when it changes

use cove_composition::Scope;
use crate::support;

use std::rc::Rc;

use cove_composition::adaptive::{
    HeightClassReader, OrientationReader, SurfaceEnvironmentExt, WidthClassReader,
};
use cove_composition::{AnyView, Column, TupleView, View};
use cove_core::{Environment, Orientation, SizeClass, State};
use cove_ui_compose::primitives::text::Text;

use support::GlobalTestLock;

fn labels(graph: &Rc<cove_composition::ViewGraph>) -> Vec<String> {
    let mut collected = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        collect(&root, &mut collected);
    }
    collected
        .into_iter()
        .filter_map(|node| node.accessibility().map(|semantics| semantics.label))
        .collect()
}

fn collect(node: &cove_core::scene::Node, into: &mut Vec<cove_core::scene::Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

fn settle(graph: &Rc<cove_composition::ViewGraph>) {
    for _ in 0..6 {
        graph.recomposer.commit_all();
        graph.compute_layout(360.0, 300.0);
        cove_core::scene::AnimatorScheduler::current().tick(0.05);
    }
}

fn size_class_name(value: SizeClass) -> &'static str {
    match value {
        SizeClass::Compact => "compact",
        SizeClass::Medium => "medium",
        SizeClass::Expanded => "expanded",
    }
}

fn orientation_name(value: Orientation) -> &'static str {
    match value {
        Orientation::Portrait => "portrait",
        Orientation::Landscape => "landscape",
    }
}

struct Harness {
    viewport: State<(f32, f32)>,
}

impl View for Harness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let (width, height) = self.viewport.get();
        let mut surface = Environment::default();
        surface.viewport.width = width;
        surface.viewport.height = height;
        let width_reader = WidthClassReader::new(move |width_class: SizeClass| {
            HeightClassReader::new(move |height_class: SizeClass| {
                OrientationReader::new(move |orientation: Orientation| {
                    Column::new(
                        cove_composition::HorizontalAlignment::Leading,
                        4.0,
                        AnyView::new(TupleView::new(vec![
                            AnyView::new(Text::new(format!(
                                "width-{}",
                                size_class_name(width_class)
                            ))),
                            AnyView::new(Text::new(format!(
                                "height-{}",
                                size_class_name(height_class)
                            ))),
                            AnyView::new(Text::new(format!(
                                "orientation-{}",
                                orientation_name(orientation)
                            ))),
                        ])),
                    )
                })
            })
        });
        AnyView::new(AnyView::new(width_reader).surface_environment(surface))
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state(
            "viewport",
            &self.viewport,
        )]
    }
}

#[test]
fn thresholds_derive_height_class_and_orientation() {
    let mut viewport = cove_core::Viewport::default();
    viewport.width = 390.0;
    viewport.height = 599.0;
    assert_eq!(viewport.width_class(), SizeClass::Compact);
    assert_eq!(viewport.height_class(), SizeClass::Compact);
    assert_eq!(viewport.orientation(), Orientation::Portrait);

    viewport.height = 600.0;
    assert_eq!(viewport.height_class(), SizeClass::Medium);
    viewport.height = 999.0;
    assert_eq!(viewport.height_class(), SizeClass::Medium);
    viewport.height = 1000.0;
    assert_eq!(viewport.height_class(), SizeClass::Expanded);

    viewport.width = 820.0;
    viewport.height = 390.0;
    assert_eq!(
        viewport.orientation(),
        Orientation::Landscape,
        "a wider viewport is landscape"
    );
    viewport.width = 600.0;
    viewport.height = 600.0;
    assert_eq!(
        viewport.orientation(),
        Orientation::Landscape,
        "a square counts as landscape"
    );
    viewport.width = 599.0;
    assert_eq!(viewport.orientation(), Orientation::Portrait);
}

#[test]
fn readers_recompose_when_the_viewport_changes() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let viewport = State::new((1280.0_f32, 720.0_f32));
            let harness = Harness {
                viewport: viewport.clone(),
            };
            let graph = support::graph();
            graph.install_view(AnyView::new(harness), Default::default());
            settle(&graph);

            let shown = labels(&graph);
            assert!(
                shown.iter().any(|label| label == "width-expanded"),
                "1280 is expanded, got {shown:?}"
            );
            assert!(
                shown.iter().any(|label| label == "height-medium"),
                "720 is medium, got {shown:?}"
            );
            assert!(
                shown.iter().any(|label| label == "orientation-landscape"),
                "720 <= 1280 is landscape, got {shown:?}"
            );

            viewport.set((390.0, 500.0));
            settle(&graph);
            let shown = labels(&graph);
            assert!(
                shown.iter().any(|label| label == "width-compact"),
                "390 is compact, got {shown:?}"
            );
            assert!(
                shown.iter().any(|label| label == "height-compact"),
                "500 is compact, got {shown:?}"
            );
            assert!(
                shown.iter().any(|label| label == "orientation-portrait"),
                "500 > 390 is portrait, got {shown:?}"
            );
        });
    });
}
