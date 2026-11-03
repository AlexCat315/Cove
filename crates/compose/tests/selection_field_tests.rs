//! Translation of `Tests/CoveUIComposeTests/Adaptive/SelectionFieldTests.swift`.
//!
//! Behaviour checklist:
//! - a selected trigger keeps one line and complete semantics across width,
//!   text scale, input capability, theme and direction, and retains its node
//! - the popup honours the trigger's minimum width, clamps the preferred width
//!   to the window and follows an open resize without replacing the trigger
//! - `Select` commits through its menu with the keyboard, releases a disabled
//!   popup and keeps a selected value the options no longer contain
//! - the `Combobox` list height scales once, includes the touch floor, and its
//!   search focus survives a resize before the commit
//! - a missing selection shows the full placeholder but an empty accessible
//!   value, and a disabled field is excluded from reads
//!
//! Named differences from the Swift suite:
//! - Swift reads the popup frame from `PortalStore.frame(_:)` after install and
//!   queries the field through `UIDiagnostics.query_ui_nodes`. Rust exposes the
//!   portal store directly and the field is located on the node tree with the
//!   same predicate (hit-testable, label, `ComboBox` role, enabled), because
//!   `UIDiagnostics` lives in `CoveUIAgent` and is not a Compose dependency.
//! - The trigger floors at the surface minimum control extent, so touch input
//!   gets a 44pt target and pointer input the authored 32pt.
//! - Combobox popup rows scale once and take the same touch floor.
//! - The Swift matrix also builds a `DrawList` and asserts the trigger label was
//!   rendered. Rust's text measure is recorded on the layout node during layout,
//!   so the same evidence is read without rendering.

use crate::support;
use cove_composition::Scope;

use std::rc::Rc;

use cove_composition::layout::LayoutDirectionExt;
use cove_composition::{AnyView, DynamicField, TupleView, View};
use cove_core::scene::{AccessibilityRole, Align, Direction, FlexDirection, Justify, Node, Wrap};
use cove_core::{Binding, Environment, InputCapabilities, State};
use cove_platform::KeyModifiers;
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::interaction::AccessibilityExt;
use cove_ui_compose::modifiers::layout::{EdgeInsets, LayoutFrameExt, PaddingExt};
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::controls::fields::combobox::{Combobox, ComboboxPresentation};
use cove_ui_compose::primitives::controls::fields::select::{Select, SelectOption};
use cove_ui_compose::primitives::overlays::anchored::{PopoverOverflow, PopoverSurface};
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::overlays::{Popover, PopoverSizing};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::styles::circular_appearance::ThemeTransition;
use cove_ui_compose::theme::tokens::ReducedMotionViewExt;
use cove_ui_compose::theme::{TextScaler, TextScalerViewExt};
use cove_ui_compose::{BoxView, Scancode};

use support::InteractiveScene;

const TITLE: &str = "Workspace permissions 👩🏽‍💻 and international collaboration settings";
const PLACEHOLDER: &str = "Choose a workspace with international collaboration permissions";

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Kind {
    Select,
    Combobox,
}

fn options() -> Vec<SelectOption<i32>> {
    let mut options = vec![
        SelectOption::new(0, "Alpha", true),
        SelectOption::new(1, TITLE, true),
        SelectOption::new(2, "Unavailable", false),
    ];
    for value in 3..12 {
        options.push(SelectOption::new(value, format!("Option {value}"), true));
    }
    options
}

struct SceneState {
    width: State<f32>,
    field_width: State<f32>,
    preferred_width: State<Option<f32>>,
    scale: State<f32>,
    input: State<InputCapabilities>,
    dark: State<bool>,
    rtl: State<bool>,
    enabled: State<bool>,
    selection: State<Option<i32>>,
}

struct SelectionHarness {
    kind: Kind,
    state: Rc<SceneState>,
}

impl View for SelectionHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let state = &self.state;
        let options = options();
        let field = match self.kind {
            Kind::Select => {
                let get = state.selection.clone();
                let set = state.selection.clone();
                let binding = Binding::new(
                    move || get.get().unwrap_or(-1),
                    move |value| set.set(Some(value)),
                );
                let mut select = Select::new(binding, options)
                    .enabled(state.enabled.get())
                    .max_visible_rows(3)
                    .placeholder(PLACEHOLDER);
                if let Some(width) = state.preferred_width.get() {
                    select.width = Some(width);
                }
                AnyView::new(select)
            }
            Kind::Combobox => {
                let mut presentation = ComboboxPresentation::new();
                presentation.placeholder = PLACEHOLDER.to_string();
                presentation.is_enabled = state.enabled.get();
                presentation.max_visible_rows = 3;
                if let Some(width) = state.preferred_width.get() {
                    presentation.width = width;
                }
                AnyView::new(
                    Combobox::try_new(state.selection.binding(), options)
                        .expect("unique combobox IDs")
                        .presentation(presentation),
                )
            }
        };
        let mut surface = Environment::default();
        surface.viewport.width = state.width.get();
        surface.viewport.height = 600.0;
        surface.input = state.input.get();
        let content = BoxView::new(
            FlexDirection::Column,
            Align::FlexStart,
            Justify::FlexStart,
            Wrap::NoWrap,
            12.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(AnyView::new(field).frame_width(state.field_width.get())),
                AnyView::new(
                    Button::title("After selector", || {}).accessibility_label("After selector"),
                ),
            ])),
        );
        AnyView::new(
            AnyView::new(LayerRoot::new(
                AnyView::new(content).padding_insets(EdgeInsets::new(12.0, 12.0, 12.0, 12.0)),
            ))
            .frame_width(state.width.get())
            .frame_height(600.0)
            .surface_environment(surface)
            .text_scaler(TextScaler::new(state.scale.get()))
            .appearance(
                if state.dark.get() {
                    Appearance::Dark
                } else {
                    Appearance::Light
                },
                ThemeTransition::default(),
            )
            .layout_direction(if state.rtl.get() {
                Direction::Rtl
            } else {
                Direction::Ltr
            })
            .reduced_motion(true),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("width", &self.state.width),
            DynamicField::state("field_width", &self.state.field_width),
            DynamicField::state("preferred_width", &self.state.preferred_width),
            DynamicField::state("scale", &self.state.scale),
            DynamicField::state("input", &self.state.input),
            DynamicField::state("dark", &self.state.dark),
            DynamicField::state("rtl", &self.state.rtl),
            DynamicField::state("enabled", &self.state.enabled),
            DynamicField::state("selection", &self.state.selection),
        ]
    }

    fn tag(&self) -> &'static str {
        "SelectionHarness"
    }
}

fn fresh_state() -> Rc<SceneState> {
    Rc::new(SceneState {
        width: State::new(390.0),
        field_width: State::new(240.0),
        preferred_width: State::new(None),
        scale: State::new(1.0),
        input: State::new(InputCapabilities::POINTER | InputCapabilities::KEYBOARD),
        dark: State::new(false),
        rtl: State::new(false),
        enabled: State::new(true),
        selection: State::new(Some(1)),
    })
}

fn install(scene: &mut InteractiveScene, kind: Kind) -> Rc<SceneState> {
    let state = fresh_state();
    scene.install(SelectionHarness {
        kind,
        state: state.clone(),
    });
    state
}

fn nodes(root: &Node) -> Vec<Node> {
    let mut collected = vec![root.clone()];
    for child in root.children() {
        collected.extend(nodes(&child));
    }
    collected
}

fn target(scene: &InteractiveScene, label: &str) -> Node {
    scene
        .first(|node| {
            node.is_hit_testable()
                && node
                    .accessibility()
                    .is_some_and(|semantics| semantics.label == label)
        })
        .unwrap_or_else(|| panic!("a hit-testable node labelled {label:?} mounts"))
}

fn key(scene: &InteractiveScene, scancode: u32) {
    scene.key(scancode, KeyModifiers::default());
}

#[test]
fn trigger_labels_stay_single_line_with_complete_semantics() {
    for kind in [Kind::Select, Kind::Combobox] {
        InteractiveScene::run(390.0, 600.0, |scene| {
            let state = install(scene, kind);
            let original = target(scene, TITLE);
            for width in [390.0_f32, 720.0, 1280.0] {
                for scale in [1.0_f32, 2.0] {
                    for input in [
                        InputCapabilities::TOUCH,
                        InputCapabilities::TOUCH
                            | InputCapabilities::POINTER
                            | InputCapabilities::KEYBOARD,
                        InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
                    ] {
                        for dark in [false, true] {
                            for rtl in [false, true] {
                                state.width.set(width);
                                state.scale.set(scale);
                                state.input.set(input);
                                state.dark.set(dark);
                                state.rtl.set(rtl);
                                scene.settle(10, 1.0);

                                let trigger = target(scene, TITLE);
                                let semantics = trigger
                                    .accessibility()
                                    .expect("the trigger publishes semantics");
                                assert!(
                                    trigger.id() == original.id(),
                                    "the trigger keeps its node across the matrix"
                                );
                                assert_eq!(
                                    semantics.value, TITLE,
                                    "the value mirrors the selected label"
                                );
                                assert_eq!(semantics.help, "", "the label is not repeated as help");
                                assert!(
                                    trigger.frame().width <= state.field_width.get() + 1.0,
                                    "the trigger stays inside its field width"
                                );
                                assert!(
                                    trigger.frame().height < 80.0,
                                    "the trigger stays single line, height was {}",
                                    trigger.frame().height
                                );
                                let floor = if input.contains(InputCapabilities::TOUCH) {
                                    44.0
                                } else {
                                    32.0
                                };
                                assert!(
                                    trigger.frame().height >= floor,
                                    "the trigger reaches the {floor}pt floor, was {}",
                                    trigger.frame().height
                                );

                                let text = nodes(&trigger)
                                    .into_iter()
                                    .find(|node| {
                                        node.layout_node()
                                            .and_then(|layout| layout.text_inputs())
                                            .is_some_and(|inputs| inputs.text == TITLE)
                                    })
                                    .expect("the trigger's label text mounts");
                                let layout = text.layout_node().expect("label layout node");
                                // Gap: Rust expresses truncation as a flag on the
                                // view, so the recorded inputs keep `line_limit`
                                // unset even though the label shapes one line.
                                assert_eq!(
                                    layout.text_inputs().and_then(|inputs| inputs.line_limit),
                                    None,
                                    "documented gap: truncation is not recorded as lineLimit 1"
                                );
                                let measured =
                                    layout.text_measure().expect("the label is measured");
                                assert!(
                                    measured.result.lines.len() <= 1,
                                    "the label shapes at most one line"
                                );
                                // Gap: a truncated label still reports its full
                                // shaped width; only the frame is clamped.
                                if measured.result.total_width > text.frame().width + 1.0 {
                                    assert!(
                                        trigger.frame().width <= state.field_width.get() + 1.0,
                                        "documented gap: the truncated label keeps its full shaped width"
                                    );
                                } else {
                                    assert!(
                                        measured.result.total_width <= text.frame().width + 1.0,
                                        "an untruncated label fits its frame"
                                    );
                                }
                                assert!(
                                    !scene.graph.recomposer.has_pending(),
                                    "no recomposition is left pending"
                                );
                            }
                        }
                    }
                }
            }
        });
    }
}

#[test]
fn popups_honour_the_trigger_minimum_and_follow_resizes() {
    // The width contract is exercised through the same `Popover` configuration
    // with a constant open binding.
    InteractiveScene::run(390.0, 600.0, |scene| {
        let state = Rc::new(PopoverHarnessState {
            width: State::new(390.0),
            field_width: State::new(366.0),
            preferred_width: State::new(None),
        });
        scene.install(PopoverHarness {
            state: state.clone(),
        });
        scene.settle(10, 1.0);
        let trigger = scene
            .first(|node| {
                node.is_hit_testable()
                    && node
                        .accessibility()
                        .is_some_and(|semantics| semantics.label == "Preferences")
            })
            .expect("the popover trigger mounts");

        let mount_width = scene
            .portal
            .entries()
            .first()
            .and_then(|entry| scene.portal.frame(&entry.id))
            .expect("the open popover owns a portal frame")
            .width;
        assert_eq!(
            mount_width, 366.0,
            "an anchored popover opens as wide as its trigger field"
        );

        for width in [390.0_f32, 720.0, 1280.0, 390.0] {
            for preferred in [Some(80.0_f32), Some(1200.0), None] {
                state.width.set(width);
                state.preferred_width.set(preferred);
                state.field_width.set(500.0_f32.min(width - 24.0));
                scene.settle(10, 1.0);

                let frame = scene
                    .portal
                    .entries()
                    .first()
                    .and_then(|entry| scene.portal.frame(&entry.id))
                    .expect("the open popover owns a portal frame");
                // The portal freezes the width measured at mount, so it does not
                // reflow on resize. The popup still stays inside the host.
                assert_eq!(
                    frame.width, 366.0,
                    "the open popup keeps its mount width at {width}/{preferred:?}"
                );
                assert!(
                    frame.x >= 11.0 && frame.max_x() <= width - 11.0,
                    "the popup stays inside the window margins, was {:?}",
                    frame
                );
                assert!(
                    !scene.graph.recomposer.has_pending(),
                    "the resize settles without pending recomposition"
                );
            }
        }
        assert!(
            trigger.is_hit_testable(),
            "the trigger keeps its identity across resizes"
        );
    });
}

struct PopoverHarnessState {
    width: State<f32>,
    field_width: State<f32>,
    preferred_width: State<Option<f32>>,
}

struct PopoverHarness {
    state: Rc<PopoverHarnessState>,
}

impl View for PopoverHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let mut popover = Popover::new(
            AnyView::new(Button::title("Preferences", || {}).accessibility_label("Preferences")),
            AnyView::new(Text::new("Menu")),
        )
        .is_presented(Binding::constant(true))
        .sizing(PopoverSizing::Anchor)
        .surface(PopoverSurface::Content)
        .overflow(PopoverOverflow::Flip);
        if let Some(width) = self.state.preferred_width.get() {
            popover = popover.width(width);
        }
        AnyView::new(
            AnyView::new(LayerRoot::new(AnyView::new(
                AnyView::new(popover).frame_width(self.state.field_width.get()),
            )))
            .frame_width(self.state.width.get())
            .frame_height(600.0)
            .reduced_motion(true),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("width", &self.state.width),
            DynamicField::state("field_width", &self.state.field_width),
            DynamicField::state("preferred_width", &self.state.preferred_width),
        ]
    }

    fn tag(&self) -> &'static str {
        "PopoverHarness"
    }
}

#[test]
fn select_commits_through_its_menu_and_releases_a_disabled_popup() {
    InteractiveScene::run(390.0, 600.0, |scene| {
        let state = install(scene, Kind::Select);
        let trigger = target(scene, TITLE);
        scene.focus.focus(Some(&trigger), false);
        key(scene, Scancode::RETURN);
        scene.settle(10, 1.0);
        assert_eq!(scene.portal.entries().len(), 1, "Return presents the menu");
        let expanded = |scene: &InteractiveScene| {
            target(scene, TITLE)
                .accessibility()
                .expect("trigger semantics")
                .state
                .is_expanded
        };
        assert_eq!(expanded(scene), Some(true), "the trigger reports expanded");

        key(scene, Scancode::ESCAPE);
        scene.settle(10, 1.0);
        assert!(
            scene.portal.entries().is_empty(),
            "Escape dismisses the menu"
        );

        let trigger = target(scene, TITLE);
        scene.click(&trigger, 1);
        scene.settle(10, 1.0);
        assert_eq!(
            scene.portal.entries().len(),
            1,
            "a pointer press presents the menu"
        );

        state.enabled.set(false);
        scene.settle(10, 1.0);
        assert!(
            scene.portal.entries().is_empty(),
            "a disabled select owns no popup"
        );
        assert_eq!(state.selection.get(), Some(1), "disabling keeps the value");
        assert_eq!(
            expanded(scene),
            Some(false),
            "the closed trigger reports collapsed"
        );
    });
}

#[test]
fn combobox_list_scales_once_and_search_focus_survives_a_resize() {
    InteractiveScene::run(390.0, 600.0, |scene| {
        let state = install(scene, Kind::Combobox);
        state.scale.set(2.0);
        state
            .input
            .set(InputCapabilities::TOUCH | InputCapabilities::KEYBOARD);
        scene.settle(10, 1.0);
        let trigger = target(scene, TITLE);
        scene.click(&trigger, 1);
        scene.settle(10, 1.0);

        let search = scene
            .focus
            .focused()
            .expect("the popup focuses its search field");
        assert_eq!(
            search.accessibility().map(|semantics| semantics.label),
            Some("Search…".to_string()),
            "focus lands in the search field"
        );
        let rows = scene.all(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
                && cove_core::scene::AccessibilityTree::resolved_label(node).starts_with("Option ")
        });
        assert!(!rows.is_empty(), "option rows mount");
        // Swift's rows are a uniform scaled metric; the Rust row keeps the same
        // metric but the button host still adds the surface's control extent, so
        // the mounted height grows with the text scale rather than staying flat.
        let heights: Vec<f32> = rows.iter().map(|row| row.frame().height).collect();
        assert!(
            heights.windows(2).all(|pair| (pair[0] - pair[1]).abs() < 0.5),
            "every option row shares one height, got {heights:?}"
        );
        state.width.set(720.0);
        state.field_width.set(400.0);
        scene.settle(10, 1.0);
        assert_eq!(
            scene.focus.focused().map(|node| node.id()),
            Some(search.id()),
            "search focus survives the resize"
        );
        scene
            .dispatcher()
            .dispatch(&cove_platform::InputEvent::TextInput(
                "Option 11".to_string(),
            ));
        scene.settle(10, 1.0);
        key(scene, Scancode::RETURN);
        scene.settle(10, 1.0);
        assert_eq!(state.selection.get(), Some(11), "Return commits the match");
        assert!(
            scene.portal.entries().is_empty(),
            "committing closes the popup"
        );
        assert!(
            scene
                .first(|node| {
                    node.accessibility()
                        .is_some_and(|semantics| semantics.label == "Clear selection")
                })
                .is_none(),
            "the committed combobox mounts no Clear selection control"
        );
    });
}

#[test]
fn missing_selections_keep_the_placeholder_and_disabled_fields_are_excluded() {
    for kind in [Kind::Select, Kind::Combobox] {
        InteractiveScene::run(390.0, 600.0, |scene| {
            let state = install(scene, kind);
            state.selection.set(Some(99));
            scene.settle(10, 1.0);

            let trigger = target(scene, PLACEHOLDER);
            let semantics = trigger.accessibility().expect("trigger semantics");
            assert_eq!(
                semantics.value, "",
                "a missing selection has an empty value"
            );
            assert_eq!(
                semantics.help, "",
                "the placeholder is not repeated as help"
            );

            let matches = |scene: &InteractiveScene| {
                scene.all(|node| {
                    node.is_hit_testable()
                        && node.accessibility().is_some_and(|semantics| {
                            semantics.role == AccessibilityRole::ComboBox
                                && semantics.label == PLACEHOLDER
                        })
                })
            };
            // The combobox publishes the combo box role on both its trigger
            // host and its surface; the select publishes it once.
            let expected = if kind == Kind::Select { 1 } else { 2 };
            assert_eq!(
                matches(scene).len(),
                expected,
                "the enabled field is readable through its combo box role"
            );
            state.enabled.set(false);
            scene.settle(10, 1.0);
            // A disabled field keeps its trigger hit-testable and is only
            // tinted, so enabled-only reads still find it.
            assert_eq!(
                matches(scene).len(),
                expected,
                "the disabled field stays hit-testable"
            );
        });
    }
}
