//! Translation of `Tests/CoveUIComposeTests/ButtonStyleTests.swift`.
//!
//! Behaviour checklist:
//! - the default style is the primary style: an accent background at rest
//! - `.secondary` swaps in a surface face with a hairline border
//! - `.destructive` uses the error colour slot, and `role: .destructive` alone
//!   does not change the style
//! - a disabled button renders the disabled appearance
//! - a custom theme flows through the style
//! - hover and press repaint the chrome without recomposing
//! - Return activates the button, and a repeated key does not
//! - a disabled button registers no key handler
//! - a `Hashable` style whose value changes updates the composition local
//! - the toggle style pairs an accent fill with an on-accent foreground when
//!   selected, and paints no fill when unselected

use cove_composition::Scope;
use crate::support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, PrimitiveView, View, ViewGraph};
use cove_core::Color;
use cove_core::scene::{
    AccessibilityRole, AnimatorScheduler, EventResult, InteractionRegistry,
    InteractionRegistryHolder, Node, NodeTree, Recomposer,
};
use cove_platform::{KeyEvent, MouseButton, MouseButtonEvent};
use cove_ui_compose::modifiers::style::AppearanceExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::media::Icon;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::styles::buttons::{
    ButtonRole, ButtonStyleViewExt, destructive, secondary, toggle,
};
use cove_ui_compose::theme::styles::circular_appearance::ThemeTransition;
use cove_ui_compose::theme::{Theme, ThemeViewExt};

use support::GlobalTestLock;

fn all_nodes(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        all_nodes(&child, into);
    }
}

/// The deepest node with a painted background, i.e. the styled body's fill.
fn find_filled(node: &Node) -> Option<Node> {
    if node.background_color().is_some_and(|color| color.a > 0.0) {
        return Some(node.clone());
    }
    for child in node.children() {
        if let Some(found) = find_filled(&child) {
            return Some(found);
        }
    }
    None
}

/// The node that carries the button's interaction handlers.
fn find_button_host(graph: &ViewGraph) -> Option<Node> {
    let root = graph.tree.borrow().root()?;
    let mut nodes = Vec::new();
    all_nodes(&root, &mut nodes);
    nodes.into_iter().find(|node| {
        node.accessibility()
            .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
            && node.attachment::<bool>("__button_host") == Some(true)
    })
}

fn graph() -> Rc<ViewGraph> {
    Rc::new(ViewGraph::new(
        Rc::new(RefCell::new(NodeTree::new())),
        Rc::new(Recomposer::new()),
    ))
}

fn install<V: View + 'static>(view: V) -> Rc<ViewGraph> {
    let graph = graph();
    graph.install(view, Default::default());
    graph
}

fn mounted_fill(graph: &ViewGraph) -> Option<Color> {
    let root = graph.tree.borrow().root()?;
    find_filled(&root).and_then(|node| node.background_color())
}

#[test]
fn the_default_style_is_primary() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry));
        let graph = install(Button::new(Text::new("Save"), || {}));
        assert_eq!(
            mounted_fill(&graph),
            Some(Theme::default_dark().colors.accent),
            "the default style paints the accent background"
        );
    });
}

#[test]
fn the_secondary_style_overrides_the_default() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry));
        let graph = install(Button::new(Text::new("Cancel"), || {}).button_style(secondary()));
        let root = graph.tree.borrow().root().expect("root");
        let filled = find_filled(&root).expect("the styled body paints a fill");
        assert_eq!(
            filled.background_color(),
            Some(Theme::default_dark().colors.surface),
            "the secondary style uses a solid control face"
        );
        assert_eq!(
            filled.border_width(),
            1.0,
            "and a 1px hairline border at rest"
        );
    });
}

#[test]
fn the_destructive_style_uses_the_error_slot() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry));
        let graph = install(
            Button::with_options(
                Text::new("Delete"),
                || {},
                ButtonRole::Destructive,
                true,
                false,
                false,
                None,
            )
            .button_style(destructive()),
        );
        assert_eq!(
            mounted_fill(&graph),
            Some(Theme::default_dark().colors.error),
            "the destructive style paints the error slot"
        );
    });
}

#[test]
fn a_destructive_role_does_not_imply_a_style() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry));
        let graph = install(Button::with_options(
            Text::new("Delete"),
            || {},
            ButtonRole::Destructive,
            true,
            false,
            false,
            None,
        ));
        assert_eq!(
            mounted_fill(&graph),
            Some(Theme::default_dark().colors.accent),
            "the role alone leaves the primary style in place"
        );
    });
}

#[test]
fn a_disabled_button_renders_the_disabled_appearance() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry));
        let graph = install(Button::with_options(
            Text::new("Save"),
            || {},
            ButtonRole::Normal,
            false,
            false,
            false,
            None,
        ));
        assert_eq!(
            mounted_fill(&graph),
            Some(Theme::default_dark().colors.surface),
            "a disabled button falls back to the surface face"
        );
    });
}

#[test]
fn a_custom_theme_flows_into_the_button_style() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry));
        let mut custom = Theme::default_dark();
        custom.colors.accent = Color {
            r: 0.0,
            g: 1.0,
            b: 0.5,
            a: 1.0,
        };
        let graph =
            install(AnyView::new(Button::new(Text::new("Go"), || {})).theme(custom.clone()));
        assert_eq!(
            mounted_fill(&graph),
            Some(custom.colors.accent),
            "the theme override reaches the style"
        );
    });
}

#[test]
fn return_activates_the_button_and_a_repeat_does_not() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        let fired = Rc::new(std::cell::Cell::new(0u32));
        let counter = fired.clone();
        let graph = install(Button::new(Text::new("Go"), move || {
            counter.set(counter.get() + 1)
        }));
        let host = find_button_host(&graph).expect("the button host is mounted");
        let key = registry
            .handlers(&host)
            .key
            .expect("the button registers a key handler");

        let press = |is_repeat: bool| {
            key(
                &KeyEvent {
                    scancode: 40,
                    keycode: 0,
                    modifiers: Default::default(),
                    is_repeat,
                },
                cove_core::scene::EventPhase::Target,
            )
        };
        assert_eq!(
            press(false),
            EventResult::Handled,
            "Return key-down activates the button"
        );
        assert_eq!(fired.get(), 1, "one press fires the action once");

        assert_eq!(press(true), EventResult::Ignored, "a key repeat is ignored");
        assert_eq!(fired.get(), 1, "a key repeat does not fire again");
    });
}

#[test]
fn hover_and_press_repaint_without_recomposing() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let theme = Theme::default_dark();
            let fired = Rc::new(std::cell::Cell::new(0u32));
            let counter = fired.clone();
            let graph = install(Button::new(Text::new("Tap"), move || {
                counter.set(counter.get() + 1)
            }));
            graph.compute_layout(120.0, 40.0);

            let host = find_button_host(&graph).expect("the button host is mounted");
            let hover = registry
                .handlers(&host)
                .hover
                .expect("the button registers a hover handler");
            let pointer = registry
                .handlers(&host)
                .pointer
                .expect("the button registers a pointer handler");
            assert!(
                !graph.recomposer.has_pending(),
                "a settled button has nothing queued"
            );

            hover(cove_core::scene::HoverPhase::Enter);
            assert!(
                !graph.recomposer.has_pending(),
                "hovering does not recompose"
            );
            scheduler.tick(1.0);
            assert_eq!(
                mounted_fill(&graph),
                Some(theme.colors.accent_hover),
                "hovering repaints the accent hover colour"
            );

            let event = MouseButtonEvent {
                button: MouseButton::Left,
                x: 8.0,
                y: 8.0,
                clicks: 1,
                modifiers: Default::default(),
                kind: Default::default(),
            };
            assert_eq!(
                pointer(
                    &event,
                    cove_core::scene::PointerPhase::Down,
                    cove_core::scene::EventPhase::Target
                ),
                EventResult::Handled
            );
            assert!(
                !graph.recomposer.has_pending(),
                "pressing does not recompose"
            );
            scheduler.tick(1.0);
            assert_eq!(
                mounted_fill(&graph),
                Some(theme.colors.accent_pressed),
                "pressing repaints the accent pressed colour"
            );

            assert_eq!(
                pointer(
                    &event,
                    cove_core::scene::PointerPhase::Up,
                    cove_core::scene::EventPhase::Target
                ),
                EventResult::Handled
            );
            assert!(
                !graph.recomposer.has_pending(),
                "releasing does not recompose"
            );
            scheduler.tick(1.0);
            assert_eq!(
                mounted_fill(&graph),
                Some(theme.colors.accent_hover),
                "releasing returns to the hover colour"
            );
            assert_eq!(fired.get(), 1, "the release fires the action once");
        });
    });
}

#[test]
fn a_disabled_button_registers_no_key_handler() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        let graph = install(Button::with_options(
            Text::new("Go"),
            || {},
            ButtonRole::Normal,
            false,
            false,
            false,
            None,
        ));
        let host = find_button_host(&graph).expect("the button host is mounted");
        assert!(
            registry.handlers(&host).key.is_none(),
            "a disabled button registers no key handler"
        );
    });
}

/// A style whose fill depends on a caller-supplied flag, mirroring the Swift
/// `StatefulFillButtonStyle`.
#[derive(Clone, PartialEq, Eq, Hash)]
struct StatefulFillStyle {
    active: bool,
}

impl cove_ui_compose::theme::styles::buttons::ButtonStyle for StatefulFillStyle {
    fn style_identity(&self) -> u64 {
        u64::from(self.active)
    }

    fn make_body(
        &self,
        configuration: cove_ui_compose::theme::styles::buttons::ButtonStyleConfiguration,
    ) -> AnyView {
        let fill = if self.active {
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }
        } else {
            Color {
                r: 0.0,
                g: 1.0,
                b: 0.0,
                a: 1.0,
            }
        };
        AnyView::new(
            AnyView::new(cove_ui_compose::BoxView::new(
                cove_core::scene::FlexDirection::Row,
                cove_core::scene::Align::Center,
                cove_core::scene::Justify::Center,
                cove_core::scene::Wrap::NoWrap,
                0.0,
                configuration.label.clone(),
            ))
            .background_color(fill),
        )
    }
}

#[test]
fn a_hashable_style_value_change_updates_the_composition_local() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        let active = cove_core::State::new(true);
        let graph = install(StatefulStyleHarness {
            active: active.clone(),
        });
        assert_eq!(
            mounted_fill(&graph),
            Some(Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0
            }),
            "the initial style value paints red"
        );

        active.set(false);
        graph.recomposer.commit_all();
        assert_eq!(
            mounted_fill(&graph),
            Some(Color {
                r: 0.0,
                g: 1.0,
                b: 0.0,
                a: 1.0
            }),
            "the new style value paints green"
        );
    });
}

struct StatefulStyleHarness {
    active: cove_core::State<bool>,
}

impl View for StatefulStyleHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let active = self.active.get();
        let toggle = self.active.clone();
        AnyView::new(
            Button::new(Text::new("State"), move || toggle.set(!toggle.get()))
                .button_style(StatefulFillStyle { active }),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state(
            "active",
            &self.active,
        )]
    }
}

fn contains_foreground(color: Color, node: &Node) -> bool {
    if node.foreground_color() == Some(color) {
        return true;
    }
    node.children()
        .iter()
        .any(|child| contains_foreground(color, child))
}

#[test]
fn the_toggle_style_pairs_an_accent_fill_with_an_on_accent_foreground() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        let theme = Theme::default_dark();

        for selected in [true, false] {
            let graph = graph();
            graph.install(
                Button::with_options(
                    Text::new("Chip"),
                    || {},
                    ButtonRole::Normal,
                    true,
                    false,
                    selected,
                    None,
                )
                .button_style(toggle()),
                Default::default(),
            );
            graph.compute_layout(200.0, 60.0);
            let fill = mounted_fill(&graph);
            if selected {
                assert_eq!(
                    fill,
                    Some(theme.colors.accent),
                    "a selected toggle paints the accent fill"
                );
            } else {
                assert_eq!(fill, None, "an unselected toggle paints no fill");
            }
        }
    });
}

#[test]
fn toggle_icon_labels_keep_readable_contrast_in_both_appearances() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));

        for light in [true, false] {
            let theme = if light {
                Theme::default_light()
            } else {
                Theme::default_dark()
            };
            for selected in [false, true] {
                let graph = graph();
                let icon = Icon::new(
                    cove_ui_compose::primitives::media::ui_icon_catalog::file::folder(),
                    16.0,
                );
                graph.install(
                    AnyView::new(
                        Button::with_options(
                            icon,
                            || {},
                            ButtonRole::Normal,
                            true,
                            false,
                            selected,
                            None,
                        )
                        .button_style(toggle()),
                    )
                    .appearance(
                        if light {
                            Appearance::Light
                        } else {
                            Appearance::Dark
                        },
                        ThemeTransition::default(),
                    ),
                    Default::default(),
                );
                graph.compute_layout(60.0, 40.0);

                let expected = if selected {
                    theme.colors.on_accent
                } else {
                    theme.colors.on_surface
                };
                let root = graph.tree.borrow().root().expect("root");
                assert!(
                    contains_foreground(expected, &root),
                    "a {} {} icon paints its expected foreground",
                    if light { "light" } else { "dark" },
                    if selected { "selected" } else { "unselected" }
                );
            }
        }
    });
}
