//! Translation of `Tests/CoveUIComposeTests/VirtualizationEstimateTests.swift`.
//!
//! Behaviour checklist:
//! - unknown heights start from the estimate and converge to measured rows
//! - metrics replace estimates by id and forget them when the width changes
//! - a long reorderable list mounts only the viewport window

use crate::support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, LayoutDebugExt};
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui_compose::BoxView;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::data::lists::RowState;
use cove_ui_compose::primitives::data::lists::reorderable::ReorderableList;
use cove_ui_compose::primitives::layout::virtualization::{
    EstimatedRowMetrics, EstimatedVirtualStack, VariableRowIndex,
};
use cove_ui_compose::primitives::text::Text;

use support::InteractiveScene;

fn estimated_row(index: &u64) -> AnyView {
    let index = *index;
    let height = if index.is_multiple_of(2) { 50.0 } else { 20.0 };
    AnyView::new(
        AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(cove_composition::EmptyView),
            ))
            .frame_height(height),
        )
        .debug_name(&format!("estimated-{index}")),
    )
}

#[test]
fn estimated_rows() {
    InteractiveScene::run(300.0, 400.0, |scene| {
        let stack =
            EstimatedVirtualStack::<u64>::new((0..10_000u64).collect(), 30.0, estimated_row);
        scene.install(AnyView::new(stack).frame_width(300.0).frame_height(400.0));
        for _ in 0..3 {
            scene.settle(1, 1.0);
        }

        let mounted = scene.all(|node| {
            support::debug_name(node).is_some_and(|name| name.starts_with("estimated-"))
        });
        assert!(
            !mounted.is_empty() && mounted.len() < 60,
            "only a window of estimated rows mounts (got {})",
            mounted.len()
        );

        let first = scene.named("estimated-0").expect("the first row mounts");
        let second = scene.named("estimated-1").expect("the second row mounts");

        assert!(
            (first.frame().height - 50.0).abs() < 0.5,
            "the first row lays out at its natural 50pt height (got {})",
            first.frame().height
        );

        // Each mounted row reports its laid-out height, so the index replaces the
        // estimate and the next window spaces rows by the measurement.
        assert!(
            (second.absolute_frame().y - 50.0).abs() < 1.0,
            "the second row starts where the measured first row ends (got {})",
            second.absolute_frame().y
        );
    });
}

#[test]
fn metrics_cache() {
    let mut metrics = EstimatedRowMetrics::<u64>::new();
    let ids: Vec<u64> = (0..4).collect();
    let estimate = 10.0;

    let mut rows = VariableRowIndex::<u64>::new();
    let heights: Vec<f32> = ids.iter().map(|_| estimate).collect();
    rows.replace(&ids, &heights, 0.0);
    assert_eq!(
        rows.total_height(),
        40.0,
        "four unmeasured rows total four estimates"
    );

    assert!(
        metrics.record(2, 30.0),
        "recording a new height reports a change"
    );
    assert!(
        !metrics.record(2, 30.2),
        "a sub-0.5pt change is not worth a recompute"
    );

    let heights: Vec<f32> = ids
        .iter()
        .map(|id| metrics.height_of(id, estimate))
        .collect();
    rows.replace(&ids, &heights, 0.0);
    assert_eq!(
        rows.total_height(),
        60.0,
        "the measurement replaces the estimate for that id"
    );

    // A width change drops the measurements and the estimates take over again.
    metrics = EstimatedRowMetrics::new();
    let heights: Vec<f32> = ids
        .iter()
        .map(|id| metrics.height_of(id, estimate))
        .collect();
    rows.replace(&ids, &heights, 0.0);
    assert_eq!(
        rows.total_height(),
        40.0,
        "a width change forgets the measurements"
    );
    assert_eq!(
        metrics.height_of(&2, estimate),
        estimate,
        "the forgotten row falls back to the estimate"
    );
}

#[test]
fn reorder_window() {
    InteractiveScene::run(300.0, 400.0, |scene| {
        let items: Rc<RefCell<Vec<u64>>> = Rc::new(RefCell::new((0..1_000u64).collect()));
        let reordered = items.clone();
        let current = items.borrow().clone();
        let list = ReorderableList::new(
            current,
            |item: &u64| *item,
            |item: &u64, _: RowState| {
                AnyView::new(
                    AnyView::new(Text::new(format!("Item {item}")))
                        .debug_name(&format!("reorder-row-{item}")),
                )
            },
        )
        .on_move(move |from, to| {
            let mut items = reordered.borrow_mut();
            let moved = items.remove(from);
            items.insert(to, moved);
        })
        .row_height(36.0)
        .row_label(|item: &u64| format!("Item {item}"));

        scene.install(AnyView::new(list).frame_width(300.0).frame_height(360.0));

        let rows = scene.all(|node| {
            support::debug_name(node).is_some_and(|name| name.starts_with("reorder-row-"))
        });
        assert!(
            rows.len() > 5 && rows.len() < 40,
            "only the viewport window of rows mounts (got {})",
            rows.len()
        );
        assert!(
            scene.named("reorder-row-999").is_none(),
            "the far end of the list stays unmounted"
        );

        let handle = scene
            .labelled("Reorder Item 0, position 1 of 1000", None)
            .expect("the first row exposes a reorder handle");
        scene.focus.focus(Some(&handle), false);
        scene.key(cove_ui_compose::Scancode::ARROW_DOWN, Default::default());
        assert_eq!(
            items.borrow()[..2],
            [1, 0],
            "the keyboard moves the first row down one position"
        );
    });
}
