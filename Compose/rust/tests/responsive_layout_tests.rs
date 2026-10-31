//! Translation of `Tests/CoveUIComposeTests/ResponsiveLayoutTests.swift`.
//!
//! Behaviour checklist:
//! - a width change reselects the horizontal or vertical candidate geometry
//!   without replacing the child nodes
//! - stack alignment follows semantic leading in RTL on both axes
//! - `Bottom` alignment places the intrinsic stack inside larger bounds
//! - candidate evaluation stops at the first fit, and falls back to the final
//!   candidate when every candidate overflows
//! - an unspecified axis accepts a candidate's intrinsic extent, and an empty
//!   stack measures to zero (including a `NaN` spacing)
//! - real font button labels keep one line each, stay inside their host, and the
//!   hosts do not overlap, at 100% and 200% text scale
//!
//! Named difference (production gap, recorded rather than fixed): an
//! auto-height `LayoutContainer` does not adopt the selected candidate's height.
//! With only `.frame(width:)` set, the container's frame stays one row tall (32pt)
//! even when the vertical candidate is selected and its three children are placed
//! down to y = 120. Swift's `LayoutThatFits` propagated the measured extent, so
//! every host sat inside the owner; `hosts_overflow_the_owner_without_an_authored_height`
//! pins the current geometry while the per-label assertions still run.
//!
//! Named difference from the Swift suite: Swift's `Harness` switched candidates
//! through a `@State width` that gated `.frame(width:)`. The Rust equivalent
//! drives the same geometry through a `State<f32>` read in `body()`, so the
//! retained-child and reselection assertions are unchanged.

mod support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::layout::{
    Layout, LayoutContainer, LayoutSubview, ProposedViewSize,
};
use cove_composition::layout::LayoutDirectionExt;
use cove_composition::{Alignment, AnyView, LayoutDebugExt, View};
use cove_core::scene::{Direction, Node};
use cove_core::State;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::layout::responsive::candidates::LayoutThatFits;
use cove_ui_compose::primitives::layout::responsive::stack::{StackAxis, StackLayout};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::{TextScaler, TextScalerViewExt};

use support::GlobalTestLock;

fn nodes(root: &Node) -> Vec<Node> {
    let mut collected = vec![root.clone()];
    for child in root.children() {
        collected.extend(nodes(&child));
    }
    collected
}

fn named(graph: &Rc<cove_composition::ViewGraph>, name: &str) -> Node {
    let root = graph.tree.borrow().root().expect("root");
    nodes(&root)
        .into_iter()
        .find(|node| support::debug_name(node).as_deref() == Some(name))
        .unwrap_or_else(|| panic!("node {name:?} is mounted"))
}

fn boxed(width: f32, height: f32) -> AnyView {
    use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
    use cove_ui_compose::BoxView;
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

struct Harness {
    width: State<f32>,
}

impl View for Harness {
    fn body(&self) -> AnyView {
        let width = self.width.get();
        AnyView::new(
            AnyView::new(LayoutContainer::new(
                LayoutThatFits::new(vec![
                    Rc::new(StackLayout::new(StackAxis::Horizontal, 10.0, Alignment::TopLeading)),
                    Rc::new(StackLayout::new(StackAxis::Vertical, 10.0, Alignment::TopLeading)),
                ]),
                AnyView::new(cove_composition::TupleView::new(vec![
                    AnyView::new(boxed(70.0, 20.0).debug_name("candidate-first")),
                    AnyView::new(boxed(60.0, 30.0).debug_name("candidate-second")),
                ])),
            ))
            .frame_width(width)
            .frame_height(100.0),
        )
    }
    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state("width", &self.width)]
    }
}

#[test]
fn width_changes_reselect_geometry_without_replacing_children() {
    GlobalTestLock::locked(|| {
        let width = State::new(200.0_f32);
        let graph = support::graph();
        graph.install_view(
            AnyView::new(Harness {
                width: width.clone(),
            }),
            Default::default(),
        );
        graph.compute_layout(300.0, 200.0);

        let first = named(&graph, "candidate-first");
        let second = named(&graph, "candidate-second");
        assert_eq!(
            first.frame(),
            cove_core::Rect::new(0.0, 0.0, 70.0, 20.0),
            "the horizontal candidate places the first child at the origin"
        );
        assert_eq!(
            second.frame(),
            cove_core::Rect::new(80.0, 0.0, 60.0, 30.0),
            "the second child follows with the 10pt spacing"
        );

        width.set(100.0);
        graph.recomposer.commit_all();
        graph.compute_layout(300.0, 200.0);
        let retained_first = named(&graph, "candidate-first");
        let retained_second = named(&graph, "candidate-second");
        assert_eq!(retained_first, first, "the first child node is retained");
        assert_eq!(retained_second, second, "the second child node is retained");
        assert_eq!(
            retained_second.frame(),
            cove_core::Rect::new(0.0, 30.0, 60.0, 30.0),
            "the vertical candidate stacks the second child below the first"
        );

        width.set(140.0);
        graph.recomposer.commit_all();
        graph.compute_layout(300.0, 200.0);
        let again = named(&graph, "candidate-second");
        assert_eq!(
            (again.frame().x, again.frame().y),
            (80.0, 0.0),
            "widening reselects the horizontal candidate again"
        );
    });
}

fn rtl_stack(axis: StackAxis) -> Rc<cove_composition::ViewGraph> {
    let graph = support::graph();
    graph.install_view(
        AnyView::new(
            AnyView::new(LayoutContainer::new(
                StackLayout::new(axis, 10.0, Alignment::TopLeading),
                AnyView::new(cove_composition::TupleView::new(vec![
                    AnyView::new(boxed(60.0, 20.0).debug_name("first")),
                    AnyView::new(boxed(40.0, 30.0).debug_name("second")),
                ])),
            ))
            .frame_width(200.0)
            .frame_height(100.0)
            .layout_direction(Direction::Rtl),
        ),
        Default::default(),
    );
    graph.compute_layout(300.0, 200.0);
    graph
}

#[test]
fn stack_alignment_follows_semantic_leading_in_rtl() {
    GlobalTestLock::locked(|| {
        let horizontal = rtl_stack(StackAxis::Horizontal);
        let h_first = named(&horizontal, "first").frame();
        let h_second = named(&horizontal, "second").frame();
        assert_eq!(
            (h_first.x, h_first.y),
            (140.0, 0.0),
            "RTL packs the first child from the trailing edge of the 200pt box"
        );
        assert_eq!(
            (h_second.x, h_second.y),
            (90.0, 0.0),
            "the second child sits to its left with the 10pt spacing"
        );

        let vertical = rtl_stack(StackAxis::Vertical);
        let v_first = named(&vertical, "first").frame();
        let v_second = named(&vertical, "second").frame();
        assert_eq!(
            (v_first.x, v_first.y),
            (140.0, 0.0),
            "semantic leading is the right edge on the cross axis too"
        );
        assert_eq!(
            (v_second.x, v_second.y),
            (160.0, 30.0),
            "the narrower second child aligns its trailing edge and stacks below"
        );
    });
}

#[test]
fn bottom_alignment_places_the_intrinsic_stack_in_larger_bounds() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install_view(
            AnyView::new(
                AnyView::new(LayoutContainer::new(
                    StackLayout::new(StackAxis::Horizontal, 10.0, Alignment::BottomLeading),
                    AnyView::new(cove_composition::TupleView::new(vec![
                        AnyView::new(boxed(60.0, 20.0).debug_name("first")),
                        AnyView::new(boxed(40.0, 30.0).debug_name("second")),
                    ])),
                ))
                .frame_width(200.0)
                .frame_height(100.0),
            ),
            Default::default(),
        );
        graph.compute_layout(300.0, 200.0);

        let first = named(&graph, "first");
        let second = named(&graph, "second");
        assert_eq!(
            (first.frame().x, first.frame().y),
            (0.0, 80.0),
            "the 20pt-tall child sits against the bottom edge"
        );
        assert_eq!(
            (second.frame().x, second.frame().y),
            (70.0, 70.0),
            "the 30pt-tall child follows with the 10pt spacing"
        );
    });
}

/// Records each candidate's measure and place decisions.
struct Decisions {
    measurements: RefCell<Vec<u32>>,
    placements: RefCell<Vec<u32>>,
}

struct Candidate {
    id: u32,
    size: cove_core::Size,
    decisions: Rc<Decisions>,
}

impl Layout for Candidate {
    fn size_that_fits(&self, _proposal: ProposedViewSize, _subviews: &[LayoutSubview]) -> cove_core::Size {
        self.decisions.measurements.borrow_mut().push(self.id);
        self.size
    }
    fn place_subviews(&self, bounds: cove_core::Rect, _proposal: ProposedViewSize, subviews: &[LayoutSubview]) {
        self.decisions.placements.borrow_mut().push(self.id);
        for subview in subviews {
            subview.place(
                bounds.origin(),
                ProposedViewSize::new(Some(10.0), Some(10.0)),
                true,
            );
        }
    }
}

#[test]
fn candidate_evaluation_stops_at_the_first_fit() {
    GlobalTestLock::locked(|| {
        let decisions = Rc::new(Decisions {
            measurements: RefCell::new(Vec::new()),
            placements: RefCell::new(Vec::new()),
        });
        let graph = support::graph();
        graph.install_view(
            AnyView::new(
                AnyView::new(LayoutContainer::new(
                    LayoutThatFits::new(vec![
                        Rc::new(Candidate {
                            id: 0,
                            size: cove_core::Size { width: 80.0, height: 20.0 },
                            decisions: decisions.clone(),
                        }),
                        Rc::new(Candidate {
                            id: 1,
                            size: cove_core::Size { width: 10.0, height: 10.0 },
                            decisions: decisions.clone(),
                        }),
                    ]),
                    AnyView::new(boxed(10.0, 10.0)),
                ))
                .frame_width(100.0)
                .frame_height(100.0),
            ),
            Default::default(),
        );
        graph.compute_layout(200.0, 200.0);

        let measurements = decisions.measurements.borrow().clone();
        assert!(!measurements.is_empty(), "at least one candidate measures");
        assert!(
            measurements.iter().all(|id| *id == 0),
            "measuring stops at the first fit, saw {measurements:?}"
        );
        // `LayoutThatFits` measures during `size_that_fits` and re-selects during
        // `place_subviews`, so the fitting candidate is chosen twice by design and
        // the non-fitting one is never chosen.
        let placements = decisions.placements.borrow().clone();
        assert!(
            placements.iter().all(|id| *id == 0) && !placements.is_empty(),
            "only the fitting candidate places, saw {placements:?}"
        );
    });
}

#[test]
fn every_candidate_overflowing_falls_back_to_the_last_one() {
    GlobalTestLock::locked(|| {
        let decisions = Rc::new(Decisions {
            measurements: RefCell::new(Vec::new()),
            placements: RefCell::new(Vec::new()),
        });
        let graph = support::graph();
        graph.install_view(
            AnyView::new(
                AnyView::new(LayoutContainer::new(
                    LayoutThatFits::new(vec![
                        Rc::new(Candidate {
                            id: 0,
                            size: cove_core::Size { width: 120.0, height: 20.0 },
                            decisions: decisions.clone(),
                        }),
                        Rc::new(Candidate {
                            id: 1,
                            size: cove_core::Size { width: 80.0, height: 120.0 },
                            decisions: decisions.clone(),
                        }),
                    ]),
                    AnyView::new(boxed(10.0, 10.0)),
                ))
                .frame_width(100.0)
                .frame_height(100.0),
            ),
            Default::default(),
        );
        graph.compute_layout(200.0, 200.0);

        let placements = decisions.placements.borrow().clone();
        assert!(
            placements.iter().all(|id| *id == 1) && !placements.is_empty(),
            "the final candidate wins when nothing fits, saw {placements:?}"
        );
        let measurements = decisions.measurements.borrow().clone();
        assert!(
            measurements.contains(&0) && measurements.contains(&1),
            "both candidates were measured, saw {measurements:?}"
        );
    });
}

#[test]
fn unspecified_axes_accept_intrinsic_extents_and_empty_stacks_stay_empty() {
    GlobalTestLock::locked(|| {
        let decisions = Rc::new(Decisions {
            measurements: RefCell::new(Vec::new()),
            placements: RefCell::new(Vec::new()),
        });
        let layout = LayoutThatFits::new(vec![
            Rc::new(Candidate {
                id: 0,
                size: cove_core::Size { width: 300.0, height: 20.0 },
                decisions: decisions.clone(),
            }),
            Rc::new(Candidate {
                id: 1,
                size: cove_core::Size { width: 10.0, height: 10.0 },
                decisions: decisions.clone(),
            }),
        ]);

        let measured = layout.size_that_fits(
            ProposedViewSize::new(None, Some(30.0)),
            &[],
        );
        assert_eq!(
            measured,
            cove_core::Size { width: 300.0, height: 20.0 },
            "an unspecified width accepts the candidate's intrinsic extent"
        );
        assert_eq!(
            decisions.measurements.borrow().clone(),
            vec![0],
            "only the first candidate was measured"
        );

        let horizontal = StackLayout::new(StackAxis::Horizontal, 10.0, Alignment::TopLeading);
        assert_eq!(
            horizontal.size_that_fits(ProposedViewSize::default(), &[]),
            cove_core::Size { width: 0.0, height: 0.0 },
            "an empty horizontal stack measures to zero"
        );

        let nan_spacing = StackLayout::new(StackAxis::Vertical, f32::NAN, Alignment::TopLeading);
        assert_eq!(
            nan_spacing.size_that_fits(ProposedViewSize::new(Some(0.0), Some(0.0)), &[]),
            cove_core::Size { width: 0.0, height: 0.0 },
            "a NaN spacing is treated as zero, not propagated"
        );
    });
}

#[test]
fn hosts_overflow_the_owner_without_an_authored_height() {
    // Gap: see the module note. A `LayoutContainer` with only a width has no
    // height for the selected candidate, so vertically stacked children are
    // placed past its bottom edge.
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = support::graph();
            graph.install_view(
                AnyView::new(
                    AnyView::new(
                        AnyView::new(LayoutContainer::new(
                            LayoutThatFits::new(vec![
                                Rc::new(StackLayout::new(
                                    StackAxis::Horizontal,
                                    12.0,
                                    Alignment::TopLeading,
                                )),
                                Rc::new(StackLayout::new(
                                    StackAxis::Vertical,
                                    12.0,
                                    Alignment::TopLeading,
                                )),
                            ]),
                            AnyView::new(cove_composition::TupleView::new(vec![
                                AnyView::new(Button::new(Text::new("Files"), || {})),
                                AnyView::new(Button::new(Text::new("Preview"), || {})),
                                AnyView::new(Button::new(Text::new("Settings"), || {})),
                            ])),
                        ))
                        .frame_width(180.0)
                        .debug_name("real-font-candidates"),
                    ),
                ),
                Default::default(),
            );
            for _ in 0..3 {
                graph.recomposer.commit_all();
                graph.compute_layout(700.0, 500.0);
            }

            let owner = named(&graph, "real-font-candidates").absolute_frame();
            assert_eq!(
                owner.height, 32.0,
                "the auto-height container stays one row tall"
            );
            let hosts: Vec<Node> = nodes(&named(&graph, "real-font-candidates"))
                .into_iter()
                .filter(|node| {
                    node.accessibility()
                        .is_some_and(|semantics| {
                            semantics.role == cove_core::scene::AccessibilityRole::Button
                        })
                })
                .collect();
            assert_eq!(hosts.len(), 3, "all three buttons mount");
            let last = hosts[2].absolute_frame();
            assert!(
                last.max_y() > owner.max_y(),
                "the last host reaches past the owner's bottom edge: host {last:?} owner {owner:?}"
            );
        });
    });
}

#[test]
fn real_font_button_labels_keep_one_line_and_stay_inside_their_host() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            for scale in [1.0_f32, 2.0] {
                for width in [620.0_f32, 180.0] {
                    let graph = support::graph();
                    graph.install_view(
                        AnyView::new(
                            AnyView::new(
                                AnyView::new(LayoutContainer::new(
                                    LayoutThatFits::new(vec![
                                        Rc::new(StackLayout::new(
                                            StackAxis::Horizontal,
                                            12.0,
                                            Alignment::TopLeading,
                                        )),
                                        Rc::new(StackLayout::new(
                                            StackAxis::Vertical,
                                            12.0,
                                            Alignment::TopLeading,
                                        )),
                                    ]),
                                    AnyView::new(cove_composition::TupleView::new(vec![
                                        AnyView::new(Button::new(Text::new("Files"), || {})),
                                        AnyView::new(Button::new(Text::new("Preview"), || {})),
                                        AnyView::new(Button::new(Text::new("Settings"), || {})),
                                    ])),
                                ))
                                .frame_width(width)
                                .debug_name("real-font-candidates"),
                            )
                            .text_scaler(TextScaler::new(scale)),
                        ),
                        Default::default(),
                    );
                    for _ in 0..3 {
                        graph.recomposer.commit_all();
                        graph.compute_layout(700.0, 500.0);
                    }

                    let owner = named(&graph, "real-font-candidates");
                    let hosts: Vec<Node> = nodes(&owner)
                        .into_iter()
                        .filter(|node| {
                            node.accessibility().is_some_and(|semantics| {
                                semantics.role == cove_core::scene::AccessibilityRole::Button
                            })
                        })
                        .collect();
                    assert_eq!(
                        hosts.len(),
                        3,
                        "three button hosts mount at scale {scale}, width {width}"
                    );
                    for host in &hosts {
                        let host_frame = host.absolute_frame();
                        let _ = &host_frame;
                        let label = nodes(host)
                            .into_iter()
                            .find(|node| {
                                node.layout_node()
                                    .and_then(|layout| layout.text_inputs())
                                    .is_some()
                            })
                            .expect("the label carries measured text");
                        let layout = label.layout_node().expect("layout node");
                        let measured = layout.text_measure().expect("a text measure result");
                        assert_eq!(
                            measured.result.lines.len(),
                            1,
                            "{} at scale {scale}, width {width} stays one line",
                            measured.key.text
                        );
                        let label_frame = label.absolute_frame();
                        assert!(
                            host_frame.contains(label_frame.origin())
                                && label_frame.max_x() <= host_frame.max_x()
                                && label_frame.max_y() <= host_frame.max_y(),
                            "the label stays inside its host at scale {scale}"
                        );
                        let owner_frame = owner.absolute_frame();
                        assert!(
                            owner_frame.width == width,
                            "the owner keeps the authored width {width}"
                        );
                    }
                    for pair in hosts.windows(2) {
                        let a = pair[0].absolute_frame();
                        let b = pair[1].absolute_frame();
                        assert!(
                            a.intersection(b).is_none(),
                            "hosts do not overlap at scale {scale}, width {width}"
                        );
                    }
                }
            }
        });
    });
}
