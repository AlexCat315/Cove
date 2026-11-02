//! Translation of `Tests/CoveUIComposeTests/Adaptive/SurfaceRootTests.swift`.
//!
//! Behaviour checklist:
//! - nested surfaces replace the host text ramp instead of compounding it, while
//!   retaining the authored application scale
//! - the safe area is consumed once, and an open IME replaces the bottom inset
//!   only when it is taller than the safe-area floor

use crate::support;

use cove_composition::{AnyView, LayoutDebugExt, PrimitiveView, View, ViewKind};
use cove_core::TextScaleSample;
use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};
use cove_core::{Environment, Insets, Keyboard, TextScale};
use cove_ui_compose::BoxView;
use cove_ui_compose::modifiers::layout::FlexExt;
use cove_ui_compose::primitives::layout::insets::safe_area::{SafeArea, SafeAreaEdges};
use cove_ui_compose::primitives::layout::insets::surface_root::SurfaceRoot;
use cove_ui_compose::theme::TextScaler;
use cove_ui_compose::theme::tokens::text_scaler::TextScalerViewExt;

use support::GlobalTestLock;

/// Records the resolved body/heading text extents, mirroring the Swift `Probe`.
struct Probe;

impl PrimitiveView for Probe {
    fn make_node(&self) -> Node {
        Node::new()
    }
    fn update_node(&self, node: &Node) {
        let scaler = cove_ui_compose::theme::node_text_scaler(node);
        node.set_attachment("body-size", scaler.scale(16.0));
        node.set_attachment("heading-size", scaler.scale(32.0));
    }
    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        Some(cove_core::scene::LayoutNode::new())
    }
}

impl View for Probe {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

fn find_probe(root: &Node) -> Option<Node> {
    if root.attachment::<f32>("body-size").is_some() {
        return Some(root.clone());
    }
    for child in root.children() {
        if let Some(found) = find_probe(&child) {
            return Some(found);
        }
    }
    None
}

#[test]
fn nested_surfaces_replace_the_host_ramp() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        let mut outer = Environment::default();
        outer.text_scale = TextScale::linear(1.5);
        let mut inner = Environment::default();
        inner.text_scale = TextScale::from_samples([
            TextScaleSample {
                size: 16.0,
                scaled_size: 32.0,
            },
            TextScaleSample {
                size: 32.0,
                scaled_size: 48.0,
            },
        ]);

        let content = AnyView::new(SurfaceRoot::new(
            outer,
            AnyView::new(SurfaceRoot::new(inner.clone(), AnyView::new(Probe)))
                .text_scaler(TextScaler::new(1.25)),
        ));
        graph.install_view(content, Default::default());
        graph.compute_layout(393.0, 852.0);

        let root = graph.tree.borrow().root().expect("root node");
        let probe = find_probe(&root).expect("the probe recorded its text extents");

        assert_eq!(
            probe.attachment::<f32>("body-size"),
            Some(40.0),
            "body text resolves through the inner ramp at the authored 1.25 scale"
        );
        assert_eq!(
            probe.attachment::<f32>("heading-size"),
            Some(60.0),
            "heading text resolves through the inner ramp at the authored 1.25 scale"
        );

        // The same numbers fall out of the documented composition rule.
        let ramp = TextScaler::default()
            .applying_system_scale(inner.text_scale)
            .multiplied(1.25);
        assert_eq!(ramp.scale(16.0), 40.0);
        assert_eq!(ramp.scale(32.0), 60.0);
    });
}

#[test]
fn safe_area_is_consumed_once_and_ime_replaces_the_bottom_floor() {
    GlobalTestLock::locked(|| {
        let mut surface = Environment::default();
        surface.safe_area = Insets {
            top: 59.0,
            leading: 12.0,
            bottom: 34.0,
            trailing: 20.0,
        };

        for keyboard in [0.0_f32, 336.0, 12.0, 0.0, f32::NAN] {
            surface.keyboard = Keyboard::new(keyboard);
            let graph = support::graph();
            let fill = AnyView::new(
                AnyView::new(
                    AnyView::new(BoxView::new(
                        FlexDirection::Column,
                        Align::Stretch,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        0.0,
                        AnyView::new(cove_composition::EmptyView),
                    ))
                    .flex_with(1.0, 1.0, Some(0.0)),
                )
                .debug_name("surface-fill"),
            );

            let content = AnyView::new(SurfaceRoot::new(
                surface.clone(),
                AnyView::new(SafeArea::new(
                    SafeAreaEdges::TOP,
                    AnyView::new(SafeArea::new(
                        SafeAreaEdges::ALL,
                        AnyView::new(fill).flex_with(1.0, 1.0, Some(0.0)),
                    ))
                    .flex_with(1.0, 1.0, Some(0.0)),
                )),
            ));
            graph.install_view(content, Default::default());
            for _ in 0..3 {
                graph.recomposer.commit_all();
                graph.compute_layout(393.0, 852.0);
            }

            let root = graph.tree.borrow().root().expect("root node");
            let node = support::find_named(&root, "surface-fill").expect("the fill node exists");
            let frame = node.absolute_frame();
            let bottom = if keyboard.is_finite() {
                34.0_f32.max(keyboard)
            } else {
                34.0
            };

            assert_eq!(
                (frame.x, frame.y),
                (12.0, 59.0),
                "leading and top insets are consumed once (keyboard {keyboard})"
            );
            assert_eq!(
                frame.width, 361.0,
                "width is the viewport minus both horizontal insets (keyboard {keyboard})"
            );
            assert_eq!(
                frame.height,
                852.0 - 59.0 - bottom,
                "height is the viewport minus top and the resolved bottom (keyboard {keyboard})"
            );
        }
    });
}
