//! Translation of `Tests/CoveUIComposeTests/Adaptive/CompactLabelTests.swift`.
//!
//! Behaviour checklist, across every width / text-scale / direction / theme
//! combination:
//! - the long title survives in three separate labels without losing text, and
//!   each measures more than one line and stays inside the 220pt column
//! - the tag's remove control keeps its 44pt touch target below the pointer
//!   width, and stays inside the column
//! - every key cap glyph sits inside the key-cap row
//! - the decorative modifier glyphs are announced once as part of the spoken
//!   shortcut, never as standalone elements
//! - the field label's text aligns to the label's leading edge (mirrored in RTL)
//! - activating the field label focuses the field named by `target_id`

use crate::support;

use cove_composition::layout::LayoutDirectionExt;
use cove_composition::{AnyView, LayoutDebugExt, PrimitiveView, View, ViewKind};
use cove_core::scene::{
    AccessibilityElement, AccessibilityRole, AccessibilityTree, Align, Direction, FlexDirection,
    Justify, Node, PlatformInputContext, Wrap,
};
use cove_core::{Environment, InputCapabilities};
use cove_ui_compose::BoxView;
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::interaction::AccessibilityExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::FieldLabel;
use cove_ui_compose::primitives::controls::KeyCap;
use cove_ui_compose::primitives::controls::Tag;
use cove_ui_compose::primitives::controls::fields::select::KeyboardShortcutPlatform;
use cove_ui_compose::primitives::feedback::{Badge, StatusTone};
use cove_ui_compose::primitives::text::TextField;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::theme::TextScalerViewExt;
use cove_ui_compose::theme::{Theme, ThemeViewExt};

use support::GlobalTestLock;

const TITLE: &str = "Workspace collaboration reviewers awaiting approval";
const KEY: &str = "Open workspace command palette";

/// A focusable stand-in for the text field, which cannot be constructed while
/// `TextField` takes an unsatisfiable `Binding<TextBuffer>`.
struct FocusableProbe;

impl PrimitiveView for FocusableProbe {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_focusable(true);
        node
    }
    fn update_node(&self, node: &Node) {
        node.set_is_focusable(true);
    }
    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        Some(cove_core::scene::LayoutNode::new())
    }
}

impl View for FocusableProbe {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

fn all_nodes(root: &Node, into: &mut Vec<Node>) {
    into.push(root.clone());
    for child in root.children() {
        all_nodes(&child, into);
    }
}

fn flatten(elements: &[AccessibilityElement], into: &mut Vec<AccessibilityElement>) {
    for element in elements {
        into.push(element.clone());
        flatten(&element.children, into);
    }
}

#[test]
fn long_labels_wrap_without_losing_text_or_controls() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            for width in [390.0_f32, 720.0, 1280.0] {
                for scale in [1.0_f32, 2.0] {
                    for rtl in [false, true] {
                        for (label, theme) in [
                            ("light", Theme::default_light()),
                            ("dark", Theme::default_dark()),
                        ] {
                            let context = PlatformInputContext::new();
                            context.with_current(|| {
                                let mut surface = Environment::default();
                                surface.viewport.width = width;
                                surface.input = if width == 1280.0 {
                                    InputCapabilities::POINTER | InputCapabilities::KEYBOARD
                                } else {
                                    InputCapabilities::TOUCH | InputCapabilities::KEYBOARD
                                };

                                let mut column: Vec<AnyView> = Vec::new();
                                column.push(AnyView::new(
                                    AnyView::new(
                                        Badge::new(TITLE).tone(StatusTone::Warning).shows_dot(true),
                                    )
                                    .debug_name("long-badge"),
                                ));
                                column.push(AnyView::new(
                                    AnyView::new(
                                        AnyView::new(Tag {
                                            title: TITLE.to_string(),
                                            icon: Some(cove_ui_compose::primitives::media::ui_icon_catalog::file::folder()),
                                            on_remove: Some(std::rc::Rc::new(|| {})),
                                            ..Tag::new(TITLE)
                                        })
                                        .debug_name("long-tag"),
                                    ),
                                ));
                                column.push(AnyView::new(
                                    AnyView::new(
                                        // The test asserts the macOS spoken
                                        // form ("Command Shift …"), so pin the
                                        // platform instead of following the
                                        // host — the glyph-to-speech mapping
                                        // under test only exists on macOS.
                                        KeyCap::new(["⌘", "⇧", KEY]).configure(|cap| {
                                            cap.platform = KeyboardShortcutPlatform::MacOS;
                                        }),
                                    )
                                    .debug_name("long-caps"),
                                ));
                                column.push(AnyView::new(
                                    FieldLabel::new(TITLE).configure(|options| {
                                        options.target_id = Some("name".to_string());
                                    }),
                                ));
                                column.push(AnyView::new(
                                    AnyView::new(TextField::new(
                                        "Name",
                                        cove_core::Binding::new(
                                            TextBuffer::empty,
                                            |_| {},
                                        ),
                                    ))
                                    .accessibility(|semantics| {
                                        semantics.identifier = "name".to_string();
                                    }),
                                ));

                                let content = AnyView::new(
                                    AnyView::new(BoxView::new(
                                        FlexDirection::Column,
                                        Align::Stretch,
                                        Justify::FlexStart,
                                        Wrap::NoWrap,
                                        16.0,
                                        AnyView::new(cove_composition::TupleView::new(column)),
                                    ))
                                    .frame_width(220.0)
                                    .surface_environment(surface.clone())
                                    .text_scaler(cove_ui_compose::theme::TextScaler::new(scale))
                                    .layout_direction(if rtl { Direction::Rtl } else { Direction::Ltr })
                                    .theme(theme.clone()),
                                );

                                let graph = support::graph();
                                graph.install_view(content, Default::default());
                                for _ in 0..6 {
                                    graph.recomposer.commit_all();
                                    graph.compute_layout(width, 2000.0);
                                }

                                let root = graph.tree.borrow().root().expect("root node");
                                let mut nodes = Vec::new();
                                all_nodes(&root, &mut nodes);

                                // The long title survives in three separate labels.
                                let title_nodes: Vec<Node> = nodes
                                    .iter()
                                    .filter(|node| {
                                        node.layout_node()
                                            .and_then(|layout| layout.text_inputs())
                                            .is_some_and(|inputs| inputs.text == TITLE)
                                    })
                                    .cloned()
                                    .collect();
                                assert_eq!(
                                    title_nodes.len(),
                                    3,
                                    "badge, tag and field label each keep the full title ({label}/{width}/{scale}/rtl={rtl})"
                                );
                                for text in &title_nodes {
                                    assert!(
                                        !text.clips_to_bounds(),
                                        "the title is not clipped"
                                    );
                                    assert!(text.frame().width > 0.0, "the title occupies width");
                                    let layout = text.layout_node().expect("layout node");
                                    let measure = layout.text_measure().expect("a text measure entry");
                                    assert_eq!(measure.key.text, TITLE);
                                    assert!(
                                        measure.result.lines.len() > 1,
                                        "the long title wraps to more than one line ({label}/{width}/{scale}/rtl={rtl}, {} line(s))",
                                        measure.result.lines.len()
                                    );
                                    let frame = text.absolute_frame();
                                    assert!(
                                        frame.x >= 0.0 && frame.max_x() <= 220.0,
                                        "the title stays inside the 220pt column ({label}/{width}/{scale}/rtl={rtl})"
                                    );
                                }

                                // The tag's remove control keeps its touch target
                                // below the pointer width.
                                let remove = nodes
                                    .iter()
                                    .find(|node| {
                                        node.accessibility().is_some_and(|semantics| {
                                            semantics.role == AccessibilityRole::Button
                                                && semantics.label == format!("Remove {TITLE}")
                                        })
                                    })
                                    .cloned()
                                    .expect("the tag exposes its remove control");
                                if width != 1280.0 {
                                    assert!(
                                        remove.frame().width >= 44.0 && remove.frame().height >= 44.0,
                                        "a touch surface keeps a 44pt remove target ({}x{})",
                                        remove.frame().width,
                                        remove.frame().height
                                    );
                                }
                                let remove_frame = remove.absolute_frame();
                                assert!(
                                    remove_frame.x >= 0.0 && remove_frame.max_x() <= 220.0,
                                    "the remove control stays inside the column"
                                );

                                // Key cap glyphs stay inside the cap row.
                                let caps = support::find_named(&root, "long-caps")
                                    .expect("the key cap row node")
                                    .absolute_frame();
                                for text in nodes.iter().filter(|node| {
                                    node.layout_node()
                                        .and_then(|layout| layout.text_inputs())
                                        .is_some_and(|inputs| {
                                            inputs.text == "⌘" || inputs.text == "⇧" || inputs.text == KEY
                                        })
                                }) {
                                    let frame = text.absolute_frame();
                                    assert!(
                                        frame.x >= caps.x && frame.max_x() <= caps.max_x(),
                                        "each key cap glyph sits inside the cap row"
                                    );
                                }

                                // Decorative modifier glyphs announce once.
                                let mut spoken = Vec::new();
                                flatten(&AccessibilityTree::snapshot(&root), &mut spoken);
                                let expected = format!("Command Shift {KEY}");
                                assert_eq!(
                                    spoken
                                        .iter()
                                        .filter(|element| element.semantics.label == expected)
                                        .count(),
                                    1,
                                    "the shortcut is announced exactly once"
                                );
                                assert!(
                                    !spoken.iter().any(|element| {
                                        element.semantics.label == "⌘" || element.semantics.label == "⇧"
                                    }),
                                    "the decorative glyphs are never standalone elements"
                                );

                                // The field label's text aligns to the label's
                                // leading edge.
                                let label_button = nodes
                                    .iter()
                                    .find(|node| {
                                        node.accessibility().is_some_and(|semantics| {
                                            semantics.role == AccessibilityRole::Button
                                                && semantics.label == TITLE
                                        })
                                    })
                                    .cloned()
                                    .expect("the field label is an accessible button");
                                let mut label_nodes = Vec::new();
                                all_nodes(&label_button, &mut label_nodes);
                                let label_text = label_nodes
                                    .into_iter()
                                    .find(|node| {
                                        node.layout_node()
                                            .and_then(|layout| layout.text_inputs())
                                            .is_some_and(|inputs| inputs.text == TITLE)
                                    })
                                    .expect("the label text node");
                                let label_frame = label_button.absolute_frame();
                                let text_frame = label_text.absolute_frame();
                                if rtl {
                                    assert!(
                                        (text_frame.max_x() - label_frame.max_x()).abs() < 0.5,
                                        "RTL aligns the label text to the trailing edge"
                                    );
                                } else {
                                    assert!(
                                        (text_frame.x - label_frame.x).abs() < 0.5,
                                        "LTR aligns the label text to the leading edge"
                                    );
                                }

                                // Activating the label focuses the named field.
                                let target = nodes
                                    .iter()
                                    .find(|node| {
                                        node.accessibility().is_some_and(|semantics| {
                                            semantics.identifier == "name"
                                        })
                                    })
                                    .cloned()
                                    .expect("the field named `name`");
                                if let Some(activate) = label_button.accessibility_actions().activate {
                                    activate();
                                }
                                let focused = context
                                    .focus_chain
                                    .focused()
                                    .expect("focus moved to the field");
                                assert_eq!(
                                    focused.id(),
                                    target.id(),
                                    "the label activation focuses the target field"
                                );
                            });
                        }
                    }
                }
            }
        });
    });
}
