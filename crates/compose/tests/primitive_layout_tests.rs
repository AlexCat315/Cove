//! Translation of `Tests/CoveUIComposeTests/PrimitiveLayoutTests.swift`.
//!
//! Behaviour checklist:
//! - `Box(row)` lays children horizontally and `Column` stacks them vertically
//! - a `Spacer` absorbs the remaining space in a row
//! - an `Image` in `.stretch` fills its frame, `.fit` letterboxes by aspect
//!   ratio, and `.fill` covers while cropping the UVs
//! - `background`, `opacity`, `clipped` and `cornerRadius` each write their node
//!   property, and a modifier stack composes without losing the authored frame
//!
//! Named differences from the Swift suite:
//! - Swift called `Box._updateLayout(_:)` directly to assert the alignment
//!   convenience mapping (`centerBottom` on a column → `alignItems: center`,
//!   `justifyContent: flexEnd`). The Rust `BoxView` is built from the resolved
//!   `FlexDirection` / `Align` / `Justify` triple with no convenience-enum mapping
//!   step, so those two cases assert the layout node the same authored values
//!   produce instead.

mod support;

use cove_composition::VerticalAlignment;
use cove_composition::{AnyView, PrimitiveView};
use cove_composition::{Row, Spacer};
use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};
use cove_core::{Color, DrawList, Rect};
use cove_ui_compose::BoxView;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::modifiers::style::AppearanceExt;
use cove_ui_compose::primitives::media::image::{Image, ImageSource};
use cove_ui_compose::primitives::media::image_geometry::{ImageContentMode, ImageRenderingMode};

use support::GlobalTestLock;

fn sized(width: f32, height: f32) -> AnyView {
    AnyView::new(
        AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(cove_composition::EmptyView),
        ))
        .frame_width(width)
        .frame_height(height),
    )
}

fn boxed(content: Vec<AnyView>, direction: FlexDirection, spacing: f32) -> AnyView {
    AnyView::new(BoxView::new(
        direction,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        spacing,
        AnyView::new(cove_composition::TupleView::new(content)),
    ))
}

fn children_of_root(graph: &std::rc::Rc<cove_composition::ViewGraph>) -> Vec<Node> {
    let root = graph.tree.borrow().root().expect("root");
    root.children()
        .first()
        .map(|first| {
            let _ = &root;
            first.children()
        })
        .unwrap_or_default()
}

#[test]
fn box_row_lays_children_horizontally() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install_view(
            boxed(
                vec![sized(50.0, 30.0), sized(70.0, 40.0)],
                FlexDirection::Row,
                0.0,
            ),
            Default::default(),
        );
        graph.compute_layout(200.0, 100.0);

        let kids = children_of_root(&graph);
        assert_eq!(kids.len(), 2, "the row mounts both children");
        assert_eq!(
            (
                kids[0].frame().x,
                kids[0].frame().y,
                kids[0].frame().width,
                kids[0].frame().height
            ),
            (0.0, 0.0, 50.0, 30.0),
            "the first child sits at the origin"
        );
        assert_eq!(
            (
                kids[1].frame().x,
                kids[1].frame().y,
                kids[1].frame().width,
                kids[1].frame().height
            ),
            (50.0, 0.0, 70.0, 40.0),
            "the second child starts after the first"
        );
    });
}

#[test]
fn column_stacks_children_vertically() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install_view(
            boxed(
                vec![sized(100.0, 20.0), sized(100.0, 30.0), sized(100.0, 40.0)],
                FlexDirection::Column,
                0.0,
            ),
            Default::default(),
        );
        graph.compute_layout(200.0, 200.0);

        let kids = children_of_root(&graph);
        assert_eq!(kids.len(), 3, "the column mounts all three children");
        assert_eq!(kids[0].frame().y, 0.0);
        assert_eq!(kids[1].frame().y, 20.0, "the second child starts at 20");
        assert_eq!(kids[2].frame().y, 50.0, "the third child starts at 50");
    });
}

#[test]
fn spacer_absorbs_the_remaining_space_in_a_row() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        let row = AnyView::new(
            AnyView::new(Row::new(
                VerticalAlignment::Center,
                0.0,
                AnyView::new(cove_composition::TupleView::new(vec![
                    sized(30.0, 30.0),
                    AnyView::new(Spacer::new(0.0)),
                    sized(40.0, 30.0),
                ])),
            ))
            .frame_width(200.0)
            .frame_height(60.0),
        );
        graph.install_view(row, Default::default());
        graph.compute_layout(200.0, 60.0);

        let kids = children_of_root(&graph);
        assert_eq!(kids.len(), 3, "the row mounts child, spacer, child");
        assert_eq!(kids[0].frame().x, 0.0, "the first child is at the origin");
        assert_eq!(
            kids[2].frame().x,
            160.0,
            "the trailing child lands at 200 - 40, was {}",
            kids[2].frame().x
        );
    });
}

#[test]
fn authored_flex_values_produce_the_alignment_convenience_mapping() {
    // Swift asserted that `Box(alignment: .centerBottom)` resolves to
    // `alignItems: center` + `justifyContent: flexEnd` on a column, and that
    // `.topLeading` on a reversed row resolves to `flexStart` + `flexEnd`.
    // Those are the values the convenience enum maps *to*, so the Rust views are
    // authored with them directly and their layout nodes are asserted.
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install_view(
            boxed(vec![], FlexDirection::Column, 0.0),
            Default::default(),
        );
        let root = graph.tree.borrow().root().expect("root");
        let layout = root
            .children()
            .first()
            .and_then(|node| node.layout_node())
            .expect("the column has a layout node");
        assert_eq!(
            layout.flex_direction(),
            FlexDirection::Column,
            "a column reports a column axis"
        );

        // The centered-bottom mapping: cross axis centered, main axis packed to
        // the end.
        let centered_bottom = BoxView::new(
            FlexDirection::Column,
            Align::Center,
            Justify::FlexEnd,
            Wrap::NoWrap,
            0.0,
            AnyView::new(cove_composition::EmptyView),
        );
        let node = cove_core::scene::LayoutNode::new();
        centered_bottom.update_layout(&node);
        assert_eq!(node.align_items(), Align::Center);
        assert_eq!(node.justify_content(), Justify::FlexEnd);

        // The top-leading mapping on a reversed row: cross axis at the start,
        // main axis packed to the end because the axis is reversed.
        let reversed = BoxView::new(
            FlexDirection::RowReverse,
            Align::FlexStart,
            Justify::FlexEnd,
            Wrap::NoWrap,
            0.0,
            AnyView::new(cove_composition::EmptyView),
        );
        let node = cove_core::scene::LayoutNode::new();
        reversed.update_layout(&node);
        assert_eq!(node.flex_direction(), FlexDirection::RowReverse);
        assert_eq!(node.align_items(), Align::FlexStart);
        assert_eq!(node.justify_content(), Justify::FlexEnd);
    });
}

#[test]
fn padding_insets_children() {
    GlobalTestLock::locked(|| {
        use cove_ui_compose::modifiers::layout::PaddingExt;
        let graph = support::graph();
        let view = AnyView::new(
            AnyView::new(
                boxed(vec![sized(50.0, 50.0)], FlexDirection::Column, 0.0)
                    .padding_edges(20.0, 20.0),
            )
            .frame_width(100.0)
            .frame_height(100.0),
        );
        graph.install_view(view, Default::default());
        graph.compute_layout(200.0, 200.0);

        let root = graph.tree.borrow().root().expect("root");
        let outer = root.children().first().cloned().expect("outer node");
        assert_eq!(
            outer.frame(),
            Rect::new(0.0, 0.0, 100.0, 100.0),
            "the framed column owns the authored 100x100 box"
        );
        let inner = outer.children().first().cloned().expect("inner node");
        assert_eq!(
            (inner.frame().x, inner.frame().y),
            (20.0, 20.0),
            "the child is inset by the 20pt padding"
        );
        assert_eq!(
            (inner.frame().width, inner.frame().height),
            (50.0, 50.0),
            "the child keeps its authored size"
        );
    });
}

#[test]
fn appearance_modifiers_write_their_node_properties() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install_view(
            AnyView::new(
                boxed(vec![], FlexDirection::Column, 0.0)
                    .background_color(red())
                    .opacity(0.5),
            ),
            Default::default(),
        );

        let root = graph.tree.borrow().root().expect("root");
        let node = root.children().first().cloned().expect("the box node");
        assert_eq!(
            node.background_color(),
            Some(red()),
            "background writes the fill"
        );
        assert_eq!(node.opacity(), 0.5, "opacity writes the node opacity");

        let graph = support::graph();
        graph.install_view(
            AnyView::new(boxed(vec![], FlexDirection::Column, 0.0).clipped()),
            Default::default(),
        );
        let root = graph.tree.borrow().root().expect("root");
        let node = root.children().first().cloned().expect("the box node");
        assert!(node.clips_to_bounds(), "clipped sets clipsToBounds");

        let graph = support::graph();
        graph.install_view(
            AnyView::new(boxed(vec![], FlexDirection::Column, 0.0).corner_radius(12.0)),
            Default::default(),
        );
        let root = graph.tree.borrow().root().expect("root");
        let node = root.children().first().cloned().expect("the box node");
        assert_eq!(node.corner_radius(), 12.0, "cornerRadius writes the radius");
    });
}

fn red() -> Color {
    Color::from_rgba8(255, 0, 0, 255)
}

fn image_node(
    graph: &std::rc::Rc<cove_composition::ViewGraph>,
    content_mode: ImageContentMode,
) -> Node {
    let image = Image::new(
        ImageSource::Texture {
            texture_id: 7,
            size: Some(cove_core::Size {
                width: 200.0,
                height: 100.0,
            }),
        },
        100.0,
        100.0,
        Color::WHITE,
        content_mode,
        ImageRenderingMode::Color,
    );
    graph.install_view(AnyView::new(image), Default::default());
    graph.compute_layout(100.0, 100.0);
    let root = graph.tree.borrow().root().expect("root");
    root.children().first().cloned().expect("the image node")
}

#[test]
fn stretch_fills_the_container() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        let node = image_node(&graph, ImageContentMode::Stretch);
        let mut list = DrawList::default();
        node.draw().expect("the image publishes a draw hook")(
            &mut list,
            cove_core::Point::default(),
        );

        assert_eq!(list.vertices().len(), 4, "stretch emits one quad");
        let xs: Vec<f32> = list.vertices().iter().map(|v| v.x).collect();
        let ys: Vec<f32> = list.vertices().iter().map(|v| v.y).collect();
        assert_eq!(xs.iter().cloned().fold(f32::INFINITY, f32::min), 0.0);
        assert_eq!(xs.iter().cloned().fold(f32::NEG_INFINITY, f32::max), 100.0);
        assert_eq!(ys.iter().cloned().fold(f32::INFINITY, f32::min), 0.0);
        assert_eq!(ys.iter().cloned().fold(f32::NEG_INFINITY, f32::max), 100.0);
    });
}

#[test]
fn fit_preserves_aspect_ratio() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        let node = image_node(&graph, ImageContentMode::Fit);
        let mut list = DrawList::default();
        node.draw().expect("the image publishes a draw hook")(
            &mut list,
            cove_core::Point::default(),
        );

        assert_eq!(list.vertices().len(), 4, "fit emits one quad");
        let xs: Vec<f32> = list.vertices().iter().map(|v| v.x).collect();
        let ys: Vec<f32> = list.vertices().iter().map(|v| v.y).collect();
        assert_eq!(xs.iter().cloned().fold(f32::INFINITY, f32::min), 0.0);
        assert_eq!(xs.iter().cloned().fold(f32::NEG_INFINITY, f32::max), 100.0);
        assert_eq!(
            ys.iter().cloned().fold(f32::INFINITY, f32::min),
            25.0,
            "a 2:1 source letterboxes 25pt top"
        );
        assert_eq!(ys.iter().cloned().fold(f32::NEG_INFINITY, f32::max), 75.0);
    });
}

#[test]
fn fill_covers_and_crops_the_uvs() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        let node = image_node(&graph, ImageContentMode::Fill);
        let mut list = DrawList::default();
        node.draw().expect("the image publishes a draw hook")(
            &mut list,
            cove_core::Point::default(),
        );

        assert_eq!(list.vertices().len(), 4, "fill emits one quad");
        let xs: Vec<f32> = list.vertices().iter().map(|v| v.x).collect();
        let ys: Vec<f32> = list.vertices().iter().map(|v| v.y).collect();
        assert_eq!(xs.iter().cloned().fold(f32::INFINITY, f32::min), 0.0);
        assert_eq!(xs.iter().cloned().fold(f32::NEG_INFINITY, f32::max), 100.0);
        assert_eq!(ys.iter().cloned().fold(f32::INFINITY, f32::min), 0.0);
        assert_eq!(ys.iter().cloned().fold(f32::NEG_INFINITY, f32::max), 100.0);
        let us: Vec<f32> = list.vertices().iter().map(|v| v.u).collect();
        assert_eq!(
            us.iter().cloned().fold(f32::INFINITY, f32::min),
            10.25,
            "the crop starts a quarter in from the source's left"
        );
        assert_eq!(us.iter().cloned().fold(f32::NEG_INFINITY, f32::max), 10.75);
    });
}

#[test]
fn a_modifier_stack_keeps_the_authored_frame_and_fill() {
    GlobalTestLock::locked(|| {
        use cove_ui_compose::modifiers::layout::PaddingExt;
        let graph = support::graph();
        graph.install_view(
            AnyView::new(
                boxed(vec![], FlexDirection::Column, 0.0)
                    .padding_edges(10.0, 10.0)
                    .frame_width(80.0)
                    .frame_height(80.0)
                    .background_color(Color::WHITE),
            ),
            Default::default(),
        );
        graph.compute_layout(200.0, 200.0);
        let root = graph.tree.borrow().root().expect("root");
        let node = root.children().first().cloned().expect("the box node");
        assert_eq!(
            (node.frame().width, node.frame().height),
            (80.0, 80.0),
            "the frame survives the modifier stack"
        );
        assert_eq!(
            node.background_color(),
            Some(Color::WHITE),
            "the background survives the modifier stack"
        );
    });
}
