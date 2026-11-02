//! Translation of `Tests/CoveUIComposeTests/PropertyGridSizingTests.swift`.
//!
//! Behaviour checklist:
//! - a controlled collapse follows a header activation and hides the body
//! - clearing the caller's collapsed set re-shows the body
//! - an intrinsic value row grows and shrinks without overlapping the next row
//! - both the value and the following row stay inside the grid width
//! - the row's intrinsic height is exactly its content height

use crate::support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, LayoutDebugExt};
use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};
use cove_ui_compose::BoxView;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::data::{
    PropertyGrid, PropertyGridRow, PropertyGridRowLayout, PropertyGridRowSizing,
    PropertyGridScrollAxes, PropertyGridSection,
};
use cove_ui_compose::primitives::text::Text;

use support::GlobalTestLock;

fn all(graph: &Rc<cove_composition::ViewGraph>) -> Vec<Node> {
    fn collect(node: &Node, into: &mut Vec<Node>) {
        into.push(node.clone());
        for child in node.children() {
            collect(&child, into);
        }
    }
    let mut nodes = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        collect(&root, &mut nodes);
    }
    nodes
}

fn named(graph: &Rc<cove_composition::ViewGraph>, name: &str) -> Option<Node> {
    all(graph)
        .into_iter()
        .find(|node| support::debug_name(node).as_deref() == Some(name))
}

fn sized(height: f32, name: &str) -> AnyView {
    AnyView::new(
        AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(cove_composition::EmptyView),
        ))
        .frame_height(height)
        .debug_name(name),
    )
}

#[test]
fn a_controlled_collapse_hides_and_reshows_the_section_body() {
    GlobalTestLock::locked(|| {
        let collapsed: Rc<RefCell<Vec<(String, bool)>>> = Rc::new(RefCell::new(Vec::new()));
        let recorder = collapsed.clone();

        let grid = PropertyGrid::new(vec![
            PropertyGridSection::new(
                "component",
                "Component",
                vec![PropertyGridRow::new(
                    "row",
                    "Value",
                    AnyView::new(Text::new("Editable")).debug_name("controlled-grid-value"),
                )],
            )
            .collapsible(true),
        ])
        .scroll_axes(PropertyGridScrollAxes::Vertical)
        .on_section_collapse_changed(move |id, is_collapsed| {
            recorder.borrow_mut().push((id.to_string(), is_collapsed));
        })
        .controlled_collapsed(Default::default());

        let graph = support::graph();
        graph.install_view(AnyView::new(grid), Default::default());
        graph.recomposer.commit_all();
        graph.compute_layout(400.0, 480.0);

        assert!(
            named(&graph, "controlled-grid-value").is_some(),
            "an expanded section shows its rows"
        );

        // Activating the header toggles the section and notifies the caller.
        let header = named(&graph, "property-section-header-component")
            .expect("the section header materialises");
        let actions = header.accessibility_actions();
        let activate = actions.activate.expect("the header is activatable");
        activate();
        graph.recomposer.commit_all();
        graph.compute_layout(400.0, 480.0);

        assert_eq!(
            collapsed.borrow().as_slice(),
            &[("component".to_string(), true)],
            "the caller is told the new collapsed state"
        );

        // The caller's state decides the presentation: a collapsed id hides
        // the body once the grid is rebuilt with it.
        let graph = support::graph();
        let mut controlled = std::collections::HashSet::new();
        controlled.insert("component".to_string());
        graph.install_view(
            AnyView::new(
                PropertyGrid::new(vec![
                    PropertyGridSection::new(
                        "component",
                        "Component",
                        vec![PropertyGridRow::new(
                            "row",
                            "Value",
                            AnyView::new(Text::new("Editable")).debug_name("controlled-grid-value"),
                        )],
                    )
                    .collapsible(true),
                ])
                .scroll_axes(PropertyGridScrollAxes::Vertical)
                .controlled_collapsed(controlled),
            ),
            Default::default(),
        );
        graph.recomposer.commit_all();
        graph.compute_layout(400.0, 480.0);
        assert!(
            named(&graph, "controlled-grid-value").is_none(),
            "a controlled collapsed id removes the rows"
        );

        // Clearing the set brings the body back.
        let graph = support::graph();
        graph.install_view(
            AnyView::new(
                PropertyGrid::new(vec![
                    PropertyGridSection::new(
                        "component",
                        "Component",
                        vec![PropertyGridRow::new(
                            "row",
                            "Value",
                            AnyView::new(Text::new("Editable")).debug_name("controlled-grid-value"),
                        )],
                    )
                    .collapsible(true),
                ])
                .scroll_axes(PropertyGridScrollAxes::Vertical)
                .controlled_collapsed(Default::default()),
            ),
            Default::default(),
        );
        graph.recomposer.commit_all();
        graph.compute_layout(400.0, 480.0);
        assert!(
            named(&graph, "controlled-grid-value").is_some(),
            "clearing the collapsed set re-shows the rows"
        );
    });
}

#[test]
fn an_intrinsic_row_grows_and_shrinks_without_overlapping_the_next_one() {
    for width in [200.0f32, 320.0] {
        GlobalTestLock::locked(|| {
            for expanded in [false, true, false] {
                let height = if expanded { 180.0 } else { 24.0 };
                let grid = PropertyGrid::new(vec![PropertyGridSection::new(
                    "component",
                    "Component",
                    vec![
                        PropertyGridRow::new("editor", "Shapes", sized(height, "intrinsic-value"))
                            .layout(PropertyGridRowLayout::FullWidth)
                            .sizing(PropertyGridRowSizing::Intrinsic),
                        PropertyGridRow::new("next", "Enabled", sized(24.0, "next-value")),
                    ],
                )])
                .label_width(72.0)
                .min_value_width(80.0)
                .scroll_axes(PropertyGridScrollAxes::Vertical);

                let graph = support::graph();
                graph.install_view(AnyView::new(grid), Default::default());
                graph.recomposer.commit_all();
                graph.compute_layout(width, 480.0);

                let value = named(&graph, "intrinsic-value")
                    .expect("the intrinsic value row materialises")
                    .absolute_frame();
                let next = named(&graph, "next-value")
                    .expect("the following row materialises")
                    .absolute_frame();

                assert_eq!(
                    value.height, height,
                    "the intrinsic row is exactly its content height (width {width}, expanded {expanded})"
                );
                assert!(
                    next.y >= value.y + value.height,
                    "the next row starts at or below the value's bottom (width {width}, expanded {expanded})"
                );
                assert!(
                    value.x + value.width <= width + 0.5,
                    "the value stays inside the grid width (width {width})"
                );
                assert!(
                    next.x + next.width <= width + 0.5,
                    "the following row stays inside the grid width (width {width})"
                );
            }
        });
    }
}

#[test]
fn a_collapsed_section_keeps_its_header_visible() {
    GlobalTestLock::locked(|| {
        let mut controlled = std::collections::HashSet::new();
        controlled.insert("component".to_string());
        let grid = PropertyGrid::new(vec![
            PropertyGridSection::new(
                "component",
                "Component",
                vec![PropertyGridRow::new(
                    "row",
                    "Value",
                    AnyView::new(Text::new("Editable")).debug_name("hidden-value"),
                )],
            )
            .collapsible(true),
        ])
        .scroll_axes(PropertyGridScrollAxes::Vertical)
        .controlled_collapsed(controlled);

        let graph = support::graph();
        graph.install_view(AnyView::new(grid), Default::default());
        graph.recomposer.commit_all();
        graph.compute_layout(400.0, 480.0);

        assert!(
            named(&graph, "property-section-header-component").is_some(),
            "a collapsed section keeps its header"
        );
        assert!(
            named(&graph, "hidden-value").is_none(),
            "a collapsed section hides its body"
        );
    });
}

#[test]
fn the_grid_reports_its_own_sizing_defaults() {
    let grid = PropertyGrid::new(Vec::new());
    assert_eq!(grid.label_width, 96.0, "the default label column is 96pt");
    assert_eq!(
        grid.min_value_width, 220.0,
        "the default value floor is 220pt"
    );
    assert_eq!(grid.row_height, 24.0, "rows are 24pt by default");
    assert_eq!(grid.row_spacing, 1.0, "rows are separated by 1pt");
    assert_eq!(grid.section_spacing, 10.0, "sections are separated by 10pt");
    assert_eq!(grid.content_padding, 8.0, "content is inset by 8pt");
    assert_eq!(
        grid.scroll_axes,
        PropertyGridScrollAxes::Both,
        "both axes scroll by default"
    );
    assert_eq!(
        grid.empty_text, "No properties",
        "the empty state is authored"
    );
}

#[test]
fn the_authored_sizing_overrides_the_defaults() {
    let grid = PropertyGrid::new(Vec::new())
        .label_width(72.0)
        .min_value_width(80.0)
        .row_height(28.0)
        .scroll_axes(PropertyGridScrollAxes::Vertical);
    assert_eq!(grid.label_width, 72.0);
    assert_eq!(grid.min_value_width, 80.0);
    assert_eq!(grid.row_height, 28.0);
    assert_eq!(grid.scroll_axes, PropertyGridScrollAxes::Vertical);

    // Invalid values fall back rather than propagating NaN into layout.
    let guarded = PropertyGrid::new(Vec::new())
        .label_width(f32::NAN)
        .min_value_width(f32::INFINITY)
        .row_height(f32::NAN);
    assert_eq!(
        guarded.label_width, 96.0,
        "a non-finite label width reverts"
    );
    // Swift: `minValueWidth.isFinite ? max(1, minValueWidth) : 220`
    // (PropertyGrid.swift:131), so a non-finite value reverts to the default —
    // the 1pt floor only applies to a finite width.
    assert_eq!(
        guarded.min_value_width, 220.0,
        "a non-finite value width reverts to the default"
    );
    assert_eq!(guarded.row_height, 24.0, "a non-finite row height reverts");
}

#[test]
fn an_empty_grid_renders_without_rows() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install_view(
            AnyView::new(
                PropertyGrid::new(Vec::new()).scroll_axes(PropertyGridScrollAxes::Vertical),
            ),
            Default::default(),
        );
        graph.recomposer.commit_all();
        graph.compute_layout(400.0, 480.0);
        let root = graph.tree.borrow().root().expect("root node");
        assert!(
            !root.children().is_empty(),
            "an empty grid still materialises its empty state"
        );
    });
}

#[test]
fn a_section_row_count_and_secondary_text_reach_the_header() {
    let mut section = PropertyGridSection::new(
        "component",
        "Component",
        vec![
            PropertyGridRow::new("a", "A", AnyView::new(Text::new("1"))),
            PropertyGridRow::new("b", "B", AnyView::new(Text::new("2"))),
        ],
    );
    section = section.row_count(true).badge("2 properties");
    assert_eq!(
        section.secondary_text().as_deref(),
        Some("2 properties"),
        "a badge becomes the header's secondary text"
    );
    assert_eq!(section.rows.len(), 2, "both rows are kept");
}
