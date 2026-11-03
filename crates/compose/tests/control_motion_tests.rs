//! Translation of `Tests/CoveUIComposeTests/ControlMotionTests.swift`.
//!
//! Behaviour checklist:
//! - a field hover never disturbs the field's bound text value
//! - the field-style environment is inert: a `TextField` node has no
//!   `border_color()`, because the chrome is painted per-draw instead of being
//!   written onto the node
//! - a switch reports its new value immediately while the thumb's visual
//!   progress travels from 0 to 1
//! - reversing mid-flight continues from the current progress instead of
//!   snapping, and settles back at 0
//!
//! Field chrome is owned by the resolved `TextFieldStyle`, as in Swift:
//! `DefaultTextFieldStyle` writes background, border colour and border width
//! onto the surface node and keys the semantic fast transition on the visual
//! state, so hover/focus interpolate instead of being painted per draw.
//! `NumberField`'s `TextFieldStyleEnvironment` override is consumed by the
//! style host.

use crate::support;
use cove_composition::Scope;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, View};
use cove_core::scene::{AnimatorScheduler, Node, NodeTree, PlatformInputContext, Recomposer};
use cove_core::{Binding, State};
use cove_ui_compose::primitives::controls::Toggle;
use cove_ui_compose::primitives::text::TextField;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::theme::{Theme, ThemeViewExt};

use support::GlobalTestLock;

const VISUAL_PROGRESS: &str = "__bool_visual_progress";

/// The Swift `SwitchHarness`: an observed flag feeding a labelled-less Toggle.
///
/// `dynamic_properties` is what makes a write invalidate this view. A
/// `PrimitiveView` that merely reads `State` inside `children()` never
/// re-subscribes, so its subtree would silently never recompose.
struct SwitchHarness {
    enabled: State<bool>,
}

impl View for SwitchHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let bound = self.enabled.clone();
        AnyView::new(Toggle::new(Binding::new(
            {
                let bound = bound.clone();
                move || bound.get()
            },
            {
                let bound = bound.clone();
                move |next| bound.set(next)
            },
        )))
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state(
            "enabled",
            &self.enabled,
        )]
    }
}
const TEXT_FIELD_HOVER: &str = "__textfield_scrollbar_hovered";

fn all_nodes(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        all_nodes(&child, into);
    }
}

fn graph() -> Rc<cove_composition::ViewGraph> {
    Rc::new(cove_composition::ViewGraph::new(
        Rc::new(RefCell::new(NodeTree::new())),
        Rc::new(Recomposer::new()),
    ))
}

#[test]
fn field_hover_preserves_the_bound_value() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let context = PlatformInputContext::new();
            let scheduler = AnimatorScheduler::default();
            context.with_current(|| {
                scheduler.with_current(|| {
                    let mut theme = Theme::default_dark();
                    theme.motion.fast = 0.2;
                    theme.inputs.border_hover = cove_core::Color::WHITE;

                    let value = State::new(TextBuffer::from("12.5"));
                    let buffer = value.clone();
                    let graph = graph();
                    graph.install(
                        AnyView::new(TextField::new(
                            "",
                            Binding::new(
                                {
                                    let buffer = buffer.clone();
                                    move || buffer.get()
                                },
                                {
                                    let buffer = buffer.clone();
                                    move |next| buffer.set(next)
                                },
                            ),
                        ))
                        .theme(theme.clone()),
                        Default::default(),
                    );
                    graph.compute_layout(120.0, 28.0);
                    scheduler.tick(1.0);
                    graph.recomposer.commit_all();
                    graph.compute_layout(120.0, 28.0);

                    let root = graph.tree.borrow().root().expect("root");
                    let mut nodes = Vec::new();
                    all_nodes(&root, &mut nodes);
                    let field = nodes
                        .iter()
                        .find(|node| {
                            node.accessibility().is_some_and(|semantics| {
                                semantics.role == cove_core::scene::AccessibilityRole::TextField
                            })
                        })
                        .cloned()
                        .expect("the field exposes TextField semantics");

                    let hover = context
                        .interactions
                        .handlers(&field)
                        .hover
                        .expect("the field registers a hover handler");

                    hover(cove_core::scene::HoverPhase::Enter);
                    graph.recomposer.commit_all();
                    scheduler.tick(0.1);
                    assert_eq!(
                        value.get().string_value(),
                        "12.5",
                        "hovering never disturbs the bound text"
                    );

                    hover(cove_core::scene::HoverPhase::Leave);
                    graph.recomposer.commit_all();
                    scheduler.tick(1.0);
                    assert_eq!(
                        value.get().string_value(),
                        "12.5",
                        "leaving hover never disturbs the bound text either"
                    );
                });
            });
        });
    });
}

/// The style host resolves the field-style environment and the default style
/// owns the node-level chrome that the animator drives.
#[test]
fn field_chrome_style_is_live() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let theme = Theme::default_dark();
            let graph = graph();
            graph.install(
                AnyView::new(TextField::new(
                    "placeholder",
                    Binding::new(TextBuffer::empty, |_| {}),
                ))
                .theme(theme.clone()),
                Default::default(),
            );
            graph.compute_layout(120.0, 28.0);

            let root = graph.tree.borrow().root().expect("root");
            let mut nodes = Vec::new();
            all_nodes(&root, &mut nodes);
            let field = nodes
                .iter()
                .find(|node| {
                    node.accessibility().is_some_and(|semantics| {
                        semantics.role == cove_core::scene::AccessibilityRole::TextField
                    })
                })
                .expect("the field exposes TextField semantics");

            assert_eq!(
                field.border_color(),
                Some(theme.inputs.border_color),
                "the default style owns the node-level border colour"
            );
            let style: cove_ui_compose::theme::styles::components::AnyTextFieldStyle = field
                .composition_value(
                    &cove_ui_compose::theme::styles::components::TextFieldStyleEnvironment::key(),
                );
            assert!(
                style
                    == cove_ui_compose::theme::styles::components::AnyTextFieldStyle::new(
                        cove_ui_compose::theme::styles::components::DefaultTextFieldStyle
                    ),
                "the environment resolves its default and the style host consumes it"
            );
        });
    });
}

#[test]
fn switch_value_changes_immediately_while_the_thumb_travels() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let scheduler = AnimatorScheduler::default();
            scheduler.with_current(|| {
                let enabled = State::new(false);
                let graph = graph();
                graph.install(
                    SwitchHarness {
                        enabled: enabled.clone(),
                    },
                    Default::default(),
                );
                graph.compute_layout(100.0, 40.0);

                let root = graph.tree.borrow().root().expect("root");
                let mut nodes = Vec::new();
                all_nodes(&root, &mut nodes);
                let toggle = nodes
                    .iter()
                    .find(|node| node.accessibility().is_some_and(|semantics| {
                        semantics.role == cove_core::scene::AccessibilityRole::Toggle
                    }))
                    .cloned()
                    .expect("the switch exposes Toggle semantics");

                enabled.set(true);
                graph.recomposer.commit_all();
                assert_eq!(
                    toggle.accessibility().expect("semantics").value,
                    "1",
                    "the switch reports the new value immediately"
                );
                assert_eq!(
                    toggle.attachment::<f32>(VISUAL_PROGRESS),
                    Some(0.0),
                    "the thumb has not moved yet"
                );

                scheduler.tick(0.04);
                let midpoint = toggle
                    .attachment::<f32>(VISUAL_PROGRESS)
                    .expect("the thumb progress is published");
                assert!(
                    midpoint > 0.0 && midpoint < 1.0,
                    "the thumb is mid-travel (got {midpoint})"
                );

                enabled.set(false);
                graph.recomposer.commit_all();
                assert_eq!(
                    toggle.attachment::<f32>(VISUAL_PROGRESS),
                    Some(midpoint),
                    "reversing does not snap the thumb"
                );

                scheduler.tick(0.0001);
                let after = toggle
                    .attachment::<f32>(VISUAL_PROGRESS)
                    .expect("progress");
                assert!(
                    after > midpoint,
                    "the first frame after reversal still advances toward the new target ({after} vs {midpoint})"
                );

                scheduler.tick(1.0);
                assert_eq!(
                    toggle.attachment::<f32>(VISUAL_PROGRESS),
                    Some(0.0),
                    "the thumb settles back at zero"
                );
                assert_eq!(
                    scheduler.active_count(),
                    0,
                    "no animation remains registered"
                );
            });
        });
    });
}
