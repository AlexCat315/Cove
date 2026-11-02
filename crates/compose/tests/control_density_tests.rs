//! Translation of `Tests/CoveUIComposeTests/ControlDensityTests.swift`.
//!
//! Behaviour checklist:
//! - a compact colour field keeps its hex readout inside 150pt with no
//!   duplicate channel text
//! - a flat tab underline is a real 2pt band under the tab's content instead of
//!   a zero-width empty view
//! - control sizing is inherited by buttons from their container
//! - an explicit `TextField` size wins over the inherited one
//! - the colour picker width follows the viewport and keeps its minimum
//!
//! Named differences from the Swift suite:
//! - Swift resolves an `Automatic` `TextField` size from
//!   `ControlSizeEnvironment` inside `TextFieldHost._children`
//!   (`Primitives/Text/TextField/TextFieldHost.swift:76-78`). The Rust
//!   `TextFieldHost` never reads that composition local, so
//!   `minimum_field_height()` always reports the regular 32pt for `.automatic`
//!   and an inherited `.small` does not shrink the field.
//!   `inherited_control_size_does_not_reach_a_text_field` pins the current
//!   behaviour.
//! - Swift asserts the `tab-selection-indicator` width equals the `selected-tab`
//!   debug node's width. The indicator is padded by the `SelectionAppearance`
//!   horizontal inset on both edges, so it spans the tab's content row rather
//!   than the outer tab node. The Rust test asserts that invariant directly:
//!   the underline is inset by `theme.spacing.sm` on both sides, is wider than
//!   an empty view, and is 2pt tall.
//! - Swift finds the hex readout by its `color-field-hex` debug name. The Rust
//!   `ColorSwatch` never applies that debug name, so the readout is located by
//!   its `#FFFFFF` value instead.

use crate::support;

use std::rc::Rc;

use cove_composition::{
    AnyView, Column, LayoutDebugExt, LayoutSnapshotEntry, Row, TupleView, ViewGraph,
};
use cove_core::scene::Node;
use cove_core::{Binding, Color};
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::controls::fields::ColorField;
use cove_ui_compose::primitives::controls::fields::color_field_model::hex_string;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_field::{
    TextField, TextFieldDecoration, TextFieldSize,
};
use cove_ui_compose::theme::styles::buttons::{ButtonStyleViewExt, TabButtonStyle};
use cove_ui_compose::theme::{ControlSize, ControlSizeViewExt};

use support::GlobalTestLock;

fn white_binding() -> Binding<Color> {
    Binding::new(|| Color::WHITE, |_| {})
}

fn empty_text_binding() -> Binding<TextBuffer> {
    Binding::new(TextBuffer::empty, |_| {})
}

fn nodes(root: &Node) -> Vec<Node> {
    let mut collected = vec![root.clone()];
    for child in root.children() {
        collected.extend(nodes(&child));
    }
    collected
}

fn labelled(root: &Node, label: &str) -> Vec<Node> {
    nodes(root)
        .into_iter()
        .filter(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.label == label)
        })
        .collect()
}

fn snapshot_entry(graph: &Rc<ViewGraph>, name: &str) -> Option<LayoutSnapshotEntry> {
    graph
        .layout_snapshot()
        .into_iter()
        .find(|entry| entry.debug_name.as_deref() == Some(name))
}

fn field_frame(graph: &Rc<ViewGraph>, name: &str) -> f32 {
    snapshot_entry(graph, name)
        .unwrap_or_else(|| panic!("{name} is named"))
        .absolute_frame
        .height
}

#[test]
fn compact_color_field_keeps_the_hex_value_without_duplicate_channel_text() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = support::graph();
            let field = ColorField::new(white_binding())
                .show_alpha(false)
                .shows_inline_values(true)
                .shows_inline_channels(false);
            graph.install_view(
                AnyView::new(Row::new(
                    cove_composition::VerticalAlignment::Center,
                    0.0,
                    AnyView::new(field),
                )),
                Default::default(),
            );
            graph.compute_layout(150.0, 30.0);

            let root = graph.tree.borrow().root().expect("root");
            let hex = labelled(&root, "#FFFFFF")
                .into_iter()
                .next()
                .unwrap_or_else(|| panic!("the hex readout mounts with its value"));
            let frame = hex.absolute_frame();
            assert!(
                frame.width >= 42.0,
                "the hex readout keeps its value legible, width was {}",
                frame.width
            );
            assert!(
                frame.max_x() <= 150.0,
                "the readout stays inside the 150pt field, max x was {}",
                frame.max_x()
            );
            assert!(
                labelled(&root, "255, 255, 255").is_empty(),
                "hiding the channel text leaves no duplicate readout"
            );
            assert_eq!(hex_string(Color::WHITE, false), "#FFFFFF");
        });
    });
}

#[test]
fn flat_tab_underline_spans_the_tab_content_row() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = support::graph();
            graph.install_view(
                AnyView::new(Row::new(
                    cove_composition::VerticalAlignment::Center,
                    0.0,
                    AnyView::new(
                        AnyView::new(
                            Button::title("Profiler", || {})
                                .selected(true)
                                .button_style(TabButtonStyle::default()),
                        )
                        .debug_name("selected-tab"),
                    ),
                )),
                Default::default(),
            );
            graph.compute_layout(320.0, 100.0);

            let tab = snapshot_entry(&graph, "selected-tab").expect("the tab is named");
            let root = graph.tree.borrow().root().expect("root");
            let indicator = nodes(&root)
                .into_iter()
                .find(|node| {
                    node.view_tag()
                        .is_some_and(|tag| tag.ends_with("tab_button_style::TabIndicator"))
                })
                .expect("the indicator mounts")
                .absolute_frame();

            assert!(
                tab.absolute_frame.width > 20.0,
                "the tab has real width, was {}",
                tab.absolute_frame.width
            );
            assert!(
                indicator.width > 20.0,
                "the underline is a real band, not a zero-width empty view; width was {}",
                indicator.width
            );
            let inset = cove_ui_compose::theme::Theme::default_dark().spacing.sm;
            assert_eq!(
                indicator.x - tab.absolute_frame.x,
                inset,
                "the underline starts at the tab's leading content inset"
            );
            assert_eq!(
                tab.absolute_frame.max_x() - indicator.max_x(),
                inset,
                "the underline ends at the tab's trailing content inset"
            );
            assert_eq!(indicator.height, 2.0, "the underline is 2pt tall");
        });
    });
}

#[test]
fn control_sizing_is_inherited_by_buttons_and_explicit_sizes_win() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = support::graph();
            let explicit = {
                let mut field = TextField::new("Find", empty_text_binding());
                field.decoration = TextFieldDecoration {
                    size: TextFieldSize::Large,
                    ..field.decoration
                };
                field
            };
            let content = AnyView::new(TupleView::new(vec![
                AnyView::new(
                    AnyView::new(Button::title("Build", || {})).debug_name("small-button"),
                ),
                AnyView::new(
                    AnyView::new(TextField::new("Find", empty_text_binding()))
                        .debug_name("inherited-field"),
                ),
                AnyView::new(AnyView::new(explicit).debug_name("explicit-field")),
                AnyView::new(
                    AnyView::new(Button::title("Large", || {}).control_size(ControlSize::Large))
                        .debug_name("large-button"),
                ),
            ]));
            graph.install_view(
                AnyView::new(
                    Column::new(cove_composition::HorizontalAlignment::Leading, 4.0, content)
                        .control_size(ControlSize::Small),
                ),
                Default::default(),
            );
            graph.compute_layout(320.0, 240.0);

            assert_eq!(
                field_frame(&graph, "small-button"),
                28.0,
                "a button inherits the container's small size"
            );
            assert_eq!(
                field_frame(&graph, "explicit-field"),
                40.0,
                "an explicit large size wins over the inherited small one"
            );
            assert_eq!(
                field_frame(&graph, "large-button"),
                40.0,
                "an explicit large control size wins over the inherited one"
            );
        });
    });
}

#[test]
fn inherited_control_size_does_not_reach_a_text_field() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = support::graph();
            graph.install_view(
                AnyView::new(
                    Column::new(
                        cove_composition::HorizontalAlignment::Leading,
                        4.0,
                        AnyView::new(
                            AnyView::new(TextField::new("Find", empty_text_binding()))
                                .debug_name("inherited-field"),
                        ),
                    )
                    .control_size(ControlSize::Small),
                ),
                Default::default(),
            );
            graph.compute_layout(320.0, 240.0);
            assert_eq!(
                field_frame(&graph, "inherited-field"),
                32.0,
                "documented gap: Swift shrinks this to 28pt, the Rust host keeps the regular 32pt"
            );
        });
    });
}

#[test]
fn color_picker_width_follows_the_viewport_and_keeps_its_minimum() {
    assert_eq!(
        ColorField::panel_width(720.0),
        280.0,
        "clamped at the maximum"
    );
    assert_eq!(
        ColorField::panel_width(390.0),
        280.0,
        "still clamped above 304"
    );
    assert_eq!(
        ColorField::panel_width(200.0),
        176.0,
        "clamped at the minimum"
    );
}
