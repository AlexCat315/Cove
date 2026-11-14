//! Regression: the overlap slider must actually move the members.
//!
//! Each member sits in an absolutely positioned `AvatarGroupSlot`. A recompose
//! reuses those nodes and only re-runs `update_node` / `update_layout`, so a
//! placement written exclusively in `make_layout_node` freezes at the value it
//! had when the slot first materialised: dragging the slider changed the
//! group's own width and nothing else -- the avatars stayed stacked exactly
//! where they started.

use crate::support;

use std::rc::Rc;

use cove_composition::layout::LayoutDirectionExt;
use cove_composition::{AnyView, DynamicField, Scope, View};
use cove_core::State;
use cove_core::scene::Node;
use cove_ui_compose::primitives::data::avatar::avatar::Avatar;
use cove_ui_compose::primitives::data::avatar::group::{AvatarGroup, AvatarGroupOptions};

use support::GlobalTestLock;

struct OverlapHarness {
    overlap: State<f64>,
    diameter: f32,
}

impl View for OverlapHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let mut options = AvatarGroupOptions::default();
        options.limit = 3;
        options.overlap = self.overlap.get() as f32;
        options.diameter = Some(self.diameter);
        let mut group = AvatarGroup::new(vec![
            Avatar::new("Ada Lovelace"),
            Avatar::new("Jason Lee"),
            Avatar::new("Mei Chen"),
        ]);
        group.options = options;
        AnyView::new(group)
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("overlap", &self.overlap)]
    }
}

fn slot_frames(graph: &Rc<cove_composition::ViewGraph>) -> Vec<(f32, f32)> {
    fn walk(node: &Node, out: &mut Vec<(f32, f32)>) {
        if node
            .view_tag()
            .is_some_and(|tag| tag.contains("AvatarGroupSlot"))
        {
            let layout = node.layout_node().expect("slot has a layout node");
            let frame = layout.frame();
            out.push((frame.x, frame.width));
        }
        for child in node.children() {
            walk(&child, out);
        }
    }
    let mut out = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        walk(&root, &mut out);
    }
    out
}

#[test]
fn the_overlap_state_moves_each_slot() {
    GlobalTestLock::locked(|| {
        let diameter = 48.0f32;
        let overlap = State::new(0.3f64);
        let harness = OverlapHarness {
            overlap: overlap.clone(),
            diameter,
        };
        let graph = support::graph();
        graph.install_view(AnyView::new(harness), Default::default());
        graph.recomposer.commit_all();
        graph.compute_layout(600.0, 200.0);

        let stacked = slot_frames(&graph);
        assert_eq!(stacked.len(), 3, "three members are laid out");
        let first_step = stacked[1].0 - stacked[0].0;
        assert!(
            (first_step - diameter * 0.7).abs() < 0.6,
            "default overlap 0.3 leaves a 0.7 step, got {first_step}"
        );

        // Drag the slider to zero overlap: every member must move apart.
        overlap.set(0.0);
        graph.recomposer.commit_all();
        graph.compute_layout(600.0, 200.0);

        let spread = slot_frames(&graph);
        assert_eq!(spread.len(), 3);
        let step = spread[1].0 - spread[0].0;
        assert!(
            (step - diameter).abs() < 0.6,
            "zero overlap places members a full diameter apart, got {step}"
        );
        assert!(
            step > first_step,
            "removing overlap must widen the spacing ({first_step} -> {step})"
        );
        assert!(
            ((spread[2].0 - spread[0].0) - diameter * 2.0).abs() < 1.2,
            "the last slot lands two diameters after the first"
        );

        // And back, so the placement is not a one-way latch.
        overlap.set(0.3);
        graph.recomposer.commit_all();
        graph.compute_layout(600.0, 200.0);
        let again = slot_frames(&graph);
        assert!(
            ((again[1].0 - again[0].0) - first_step).abs() < 0.6,
            "returning to 0.3 restores the stacked step"
        );
    });
}
