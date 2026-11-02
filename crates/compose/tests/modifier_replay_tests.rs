//! Translation of `Tests/CoveUIComposeTests/ModifierReplayTests.swift`.
//!
//! Behaviour checklist:
//! - repeated modifier types through `AnyView` keep each declaration and replace
//!   parent values
//! - a width-only caller preserves an owned height, and `.auto` explicitly
//!   clears an axis
//! - a fixed frame and an outer style survive a local branch replacement through
//!   transparent wrappers
//! - modifier values update in declaration order without retaining obsolete
//!   caller values
//!
//! Model difference: Swift's `@State` members are declared inside each local
//! harness `struct` and mutated through the view value (`parent.width = 380`).
//! Rust has no property wrappers, so each harness owns a `cove_core::State` and
//! returns the same values from `dynamic_properties`, which is the equivalent
//! recomposition trigger. The asserted frames are Swift's own.

use crate::support;
use cove_composition::Scope;

use cove_composition::{AnyView, DynamicField, LayoutDebugExt, View};
use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};
use cove_core::{Color, State};
use cove_ui_compose::modifiers::layout::{FrameDimension, LayoutFrameExt};
use cove_ui_compose::modifiers::style::AppearanceExt;
use cove_ui_compose::{BoxView, Row, VerticalAlignment};

use support::GlobalTestLock;

/// The Swift `StatefulContent`: a local branch swap inside a percent-sized box.
struct StatefulContent {
    expanded: State<bool>,
}

impl StatefulContent {
    fn new() -> Self {
        Self {
            expanded: State::new(false),
        }
    }

    fn content(&self, expanded: bool) -> AnyView {
        if expanded {
            AnyView::new(
                AnyView::new(Row::new(
                    VerticalAlignment::Center,
                    0.0,
                    AnyView::new(AnyView::new(empty_box()).frame_height(600.0)),
                ))
                .frame(
                    Some(FrameDimension::Percent(100.0)),
                    Some(FrameDimension::Percent(100.0)),
                    None,
                    None,
                    None,
                    None,
                )
                .background_color(Color::from_rgba8(0xff, 0xff, 0xff, 0xff)),
            )
        } else {
            AnyView::new(
                AnyView::new(AnyView::new(empty_box()).frame_height(100.0))
                    .frame(
                        Some(FrameDimension::Percent(100.0)),
                        Some(FrameDimension::Percent(100.0)),
                        None,
                        None,
                        None,
                        None,
                    )
                    .background_color(Color::from_rgba8(0xff, 0xff, 0xff, 0xff)),
            )
        }
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

fn snapshot_frame(graph: &cove_composition::ViewGraph, name: &str) -> cove_core::Rect {
    graph
        .layout_snapshot()
        .into_iter()
        .find(|entry| entry.debug_name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("layout snapshot has {name}"))
        .absolute_frame
}

fn find_named(node: &Node, name: &str) -> Option<Node> {
    support::find_named(node, name)
}

#[test]
fn erased_declarations_keep_each_modifier_and_replace_parent_values() {
    GlobalTestLock::locked(|| {
        let content = std::rc::Rc::new(StatefulContent::new());

        struct Parent {
            width: State<f32>,
            content: std::rc::Rc<StatefulContent>,
        }
        impl View for Parent {
            fn body(&self, _cx: &Scope) -> AnyView {
                let inner = AnyView::new(self.content.content(self.content.expanded.get()));
                AnyView::new(
                    AnyView::new(AnyView::new(inner).frame_width(self.width.get()))
                        .frame_height(360.0)
                        .debug_name("erased-frame"),
                )
            }
            fn dynamic_properties(&self) -> Vec<DynamicField> {
                vec![
                    DynamicField::state("width", &self.width),
                    DynamicField::state("expanded", &self.content.expanded),
                ]
            }
            fn tag(&self) -> &'static str {
                std::any::type_name::<Self>()
            }
        }

        let parent_width = State::new(240.0f32);
        let graph = support::graph();
        let parent = Parent {
            width: parent_width.clone(),
            content: content.clone(),
        };
        let parent_width = parent.width.clone();
        let expanded = parent.content.expanded.clone();
        graph.install(parent, Default::default());

        for width in [240.0f32, 380.0, 200.0] {
            parent_width.set(width);
            for _ in 0..3 {
                expanded.set(!expanded.get());
                for _ in 0..3 {
                    graph.recomposer.commit_all();
                    graph.compute_layout(800.0, 700.0);
                }
                let frame = snapshot_frame(&graph, "erased-frame");
                assert_eq!(
                    frame.width, width,
                    "the caller's width survives a local content branch swap"
                );
                assert_eq!(frame.height, 360.0, "the caller's height is untouched");
            }
        }
    });
}

#[test]
fn a_width_only_caller_preserves_owned_height_and_auto_clears_the_axis() {
    GlobalTestLock::locked(|| {
        struct SizedContent {
            height: State<f32>,
        }
        impl View for SizedContent {
            fn body(&self, _cx: &Scope) -> AnyView {
                AnyView::new(AnyView::new(empty_box()).frame_size(100.0, self.height.get()))
            }
            fn dynamic_properties(&self) -> Vec<DynamicField> {
                vec![DynamicField::state("height", &self.height)]
            }
            fn tag(&self) -> &'static str {
                std::any::type_name::<Self>()
            }
        }

        struct Parent {
            width: State<FrameDimension>,
            height: State<f32>,
        }
        impl View for Parent {
            fn body(&self, _cx: &Scope) -> AnyView {
                AnyView::new(
                    AnyView::new(
                        AnyView::new(SizedContent {
                            height: self.height.clone(),
                        })
                        .frame(
                            Some(self.width.get()),
                            None,
                            None,
                            None,
                            None,
                            None,
                        ),
                    )
                    .debug_name("partial-frame"),
                )
            }
            fn dynamic_properties(&self) -> Vec<DynamicField> {
                vec![
                    DynamicField::state("width", &self.width),
                    DynamicField::state("height", &self.height),
                ]
            }
            fn tag(&self) -> &'static str {
                std::any::type_name::<Self>()
            }
        }

        let width = State::new(FrameDimension::Points(240.0));
        let height = State::new(120.0f32);
        let graph = support::graph();
        graph.install(
            Parent {
                width: width.clone(),
                height: height.clone(),
            },
            Default::default(),
        );

        let cases: [(FrameDimension, f32); 4] = [
            (FrameDimension::Points(240.0), 120.0),
            (FrameDimension::Points(240.0), 200.0),
            (FrameDimension::calc(50.0, -12.0), 140.0),
            (FrameDimension::Auto, 160.0),
        ];
        for (next_width, next_height) in cases {
            width.set(next_width);
            height.set(next_height);
            for _ in 0..3 {
                graph.recomposer.commit_all();
                graph.compute_layout(800.0, 700.0);
            }
            let frame = snapshot_frame(&graph, "partial-frame");
            assert_eq!(
                frame.height, next_height,
                "a width-only caller preserves the content's owned height"
            );
            match next_width {
                FrameDimension::Points(value) => assert_eq!(
                    frame.width, value,
                    "an explicit points width wins over the content's own width"
                ),
                FrameDimension::Auto => assert!(
                    frame.width != 100.0 && frame.width != 240.0,
                    ".auto explicitly clears the axis, width was {}",
                    frame.width
                ),
                FrameDimension::Calc(value) => assert_eq!(frame.width, value.resolve(800.0)),
                _ => unreachable!("the cases above cover points, calc and auto"),
            }
        }
    });
}

#[test]
fn a_fixed_frame_and_outer_style_survive_a_branch_replacement() {
    GlobalTestLock::locked(|| {
        struct Forwarder {
            expanded: State<bool>,
        }
        impl View for Forwarder {
            fn body(&self, _cx: &Scope) -> AnyView {
                AnyView::new(
                    StatefulContent {
                        expanded: self.expanded.clone(),
                    }
                    .content(self.expanded.get()),
                )
            }
            fn dynamic_properties(&self) -> Vec<DynamicField> {
                vec![DynamicField::state("expanded", &self.expanded)]
            }
            fn tag(&self) -> &'static str {
                std::any::type_name::<Self>()
            }
        }

        let expanded = State::new(false);
        let marker = Color::from_rgba8(255, 0, 0, 255);
        let graph = support::graph();
        graph.install(
            AnyView::new(
                AnyView::new(Forwarder {
                    expanded: expanded.clone(),
                })
                .frame_size(240.0, 360.0)
                .background_color(marker)
                .debug_name("bounded-component"),
            ),
            Default::default(),
        );

        for value in [false, true, false, true] {
            expanded.set(value);
            for _ in 0..3 {
                graph.recomposer.commit_all();
                graph.compute_layout(800.0, 700.0);
            }
            let frame = snapshot_frame(&graph, "bounded-component");
            assert_eq!(frame.width, 240.0, "the caller's fixed width survives");
            assert_eq!(frame.height, 360.0, "the caller's fixed height survives");

            let root = graph.tree.borrow().root().expect("root");
            let node =
                find_named(&root, "bounded-component").expect("the bounded component materialises");
            assert_eq!(
                node.background_color(),
                Some(marker),
                "the outer style survives a local branch replacement"
            );
        }
    });
}

#[test]
fn modifier_values_update_in_declaration_order() {
    GlobalTestLock::locked(|| {
        struct Parent {
            height: State<f32>,
            expanded: State<bool>,
        }
        impl View for Parent {
            fn body(&self, _cx: &Scope) -> AnyView {
                AnyView::new(
                    AnyView::new(
                        StatefulContent {
                            expanded: self.expanded.clone(),
                        }
                        .content(self.expanded.get())
                        .frame_height(200.0),
                    )
                    .frame(
                        Some(FrameDimension::Points(240.0)),
                        Some(FrameDimension::Points(self.height.get())),
                        None,
                        None,
                        None,
                        None,
                    )
                    .debug_name("child"),
                )
            }
            fn dynamic_properties(&self) -> Vec<DynamicField> {
                vec![
                    DynamicField::state("height", &self.height),
                    DynamicField::state("expanded", &self.expanded),
                ]
            }
            fn tag(&self) -> &'static str {
                std::any::type_name::<Self>()
            }
        }

        let height = State::new(360.0f32);
        let expanded = State::new(false);
        let graph = support::graph();
        graph.install(
            Parent {
                height: height.clone(),
                expanded: expanded.clone(),
            },
            Default::default(),
        );

        for next in [360.0f32, 280.0, 420.0] {
            height.set(next);
            expanded.set(!expanded.get());
            for _ in 0..3 {
                graph.recomposer.commit_all();
                graph.compute_layout(800.0, 700.0);
            }
            assert_eq!(
                snapshot_frame(&graph, "child").height,
                next,
                "the later declaration wins and does not retain the obsolete value"
            );
        }
    });
}
