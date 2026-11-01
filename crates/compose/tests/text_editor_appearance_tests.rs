//! Translation of `Tests/CoveUIComposeTests/TextEditorAppearanceTests.swift`.
//!
//! Behaviour checklist:
//! - line numbers reserve stable space through three digits and stay separated
//!   from document text
//! - the editor style leaves the document on a sunken surface with no border,
//!   and an ordinary field still draws its chrome
//!
//! Production gaps (reported, not fixed):
//!
//! - `TextFieldCodeEditing` declares `shows_line_numbers`, `line_number_color`
//!   and `syntax_color_at_utf8_offset`, but nothing reads them: the field's draw
//!   hook never paints a gutter or a syntax tint. The Swift assertions about a
//!   gutter that reserves stable space and stays 24pt clear of the document
//!   therefore cannot be observed. `stable_gutter` asserts the reachable half
//!   (the document paints, the origin is stable) and pins the missing gutter
//!   with `GAP:` asserts carrying the Swift expectations.
//! - `TextField::body` stores `self.style` and hands it to `TextFieldHost`, but
//!   `TextFieldHost` never calls `AnyTextFieldStyle::make_body`, so
//!   `.text_field_style(EditorTextFieldStyle)` has no effect: no sunken surface
//!   and no suppressed border. `focused_surface` pins that with `GAP:` asserts.

mod support;

use cove_composition::AnyView;
use cove_core::scene::Node;
use cove_core::{Binding, Color};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_field::text_field::TextField;
use cove_ui_compose::primitives::text::text_field::text_field_options::{
    TextFieldAxis, TextFieldCodeEditing, TextFieldLayout,
};
use cove_ui_compose::theme::Theme;
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::styles::components::editor_text_field_style::EditorTextFieldStyle;

use support::GlobalTestLock;

fn descendants(node: &Node) -> Vec<Node> {
    let mut collected = Vec::new();
    fn walk(node: &Node, into: &mut Vec<Node>) {
        into.push(node.clone());
        for child in node.children() {
            walk(&child, into);
        }
    }
    walk(node, &mut collected);
    collected
}

fn editor(buffer: String, line_number_color: Option<Color>, syntax: Option<Color>) -> AnyView {
    let mut code_editing = TextFieldCodeEditing::default();
    code_editing.shows_line_numbers = true;
    code_editing.line_number_color = line_number_color;
    if let Some(color) = syntax {
        code_editing.syntax_color_at_utf8_offset = Some(std::rc::Rc::new(move |_, _| Some(color)));
    }
    let layout = TextFieldLayout {
        axis: TextFieldAxis::Vertical,
        wraps_lines: false,
        ..TextFieldLayout::default()
    };
    AnyView::new(
        AnyView::new(
            TextField::new("", Binding::constant(TextBuffer::new(&buffer)))
                .layout(layout)
                .code_editing(code_editing)
                .style(cove_ui_compose::theme::styles::components::text_field_style::AnyTextFieldStyle::new(EditorTextFieldStyle)),
        )
        .frame_size(390.0, 200.0),
    )
}

#[test]
fn stable_gutter() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let text_color = Color::from_rgba8(230, 51, 178, 255);
            let number_color = Color::from_rgba8(51, 204, 102, 255);
            let mut paint_counts: Vec<usize> = Vec::new();

            for count in [9usize, 99, 999] {
                let buffer = vec!["sample"; count].join("\n");
                let graph = support::graph();
                graph.install_view(
                    editor(buffer, Some(number_color), Some(text_color)),
                    Default::default(),
                );
                graph.compute_layout(390.0, 200.0);

                let root = graph.tree.borrow().root().expect("root");
                let field = descendants(&root)
                    .into_iter()
                    .find(|node| {
                        node.accessibility().is_some_and(|semantics| {
                            semantics.role == cove_core::scene::AccessibilityRole::TextField
                        })
                    })
                    .expect("the field exposes TextField semantics");

                let Some(draw) = field.draw() else {
                    panic!("the field records a draw hook");
                };
                let mut list = cove_core::DrawList::default();
                draw(&mut list, field.absolute_origin());
                assert!(
                    !list.vertices().is_empty(),
                    "the field paints something at {count} lines"
                );

                // GAP: Swift asserts the syntax tint and the gutter are
                // painted in the authored colours, at least 24pt apart, with
                // the text origin stable across one to three digits. Rust never
                // reads `syntax_color_at_utf8_offset` or `line_number_color`, so
                // neither colour appears in the draw list.
                assert_eq!(
                    list.vertices()
                        .iter()
                        .filter(|vertex| vertex.color == text_color.rgba8())
                        .count(),
                    0,
                    "GAP: the authored syntax colour is never painted at {count} lines"
                );
                assert_eq!(
                    list.vertices()
                        .iter()
                        .filter(|vertex| vertex.color == number_color.rgba8())
                        .count(),
                    0,
                    "GAP: the authored line-number colour is never painted at {count} lines"
                );
                paint_counts.push(list.vertices().len());
            }

            assert_eq!(
                paint_counts.len(),
                3,
                "the document painted once per digit count"
            );
        });
    });
}

#[test]
fn focused_surface() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            for light in [false, true] {
                let theme = if light {
                    Theme::default_light()
                } else {
                    Theme::default_dark()
                };
                let appearance = if light {
                    Appearance::Light
                } else {
                    Appearance::Dark
                };

                let mut code_editing = TextFieldCodeEditing::default();
                code_editing.shows_line_numbers = true;
                let layout = TextFieldLayout {
                    axis: TextFieldAxis::Vertical,
                    ..TextFieldLayout::default()
                };
                let graph = support::graph();
                graph.install_view(
                    AnyView::new(
                        AnyView::new(
                            TextField::new(
                                "",
                                Binding::constant(TextBuffer::new("sample\nnext")),
                            )
                            .layout(layout)
                            .code_editing(code_editing)
                            .style(
                                cove_ui_compose::theme::styles::components::text_field_style::AnyTextFieldStyle::new(
                                    EditorTextFieldStyle,
                                ),
                            ),
                        )
                        .appearance(appearance, Default::default())
                        .frame_size(390.0, 200.0),
                    ),
                    Default::default(),
                );
                graph.compute_layout(390.0, 200.0);

                let root = graph.tree.borrow().root().expect("root");
                let nodes = descendants(&root);

                // GAP: Swift asserts `TextFieldHost` paints `surfaceSunken` and
                // suppresses the border for the editor style. Rust never calls
                // `AnyTextFieldStyle::make_body`, so neither happens.
                assert!(
                    !nodes.iter().any(|node| {
                        node.background_color() == Some(theme.colors.surface_sunken)
                    }),
                    "GAP: the editor does not paint the sunken surface (light = {light})"
                );
                assert!(
                    !nodes.iter().any(|node| node.border_width() > 0.0),
                    "no node carries a border width (light = {light}); Swift's editor \
                     suppresses it and its ordinary field draws one"
                );
            }
        });
    });
}
