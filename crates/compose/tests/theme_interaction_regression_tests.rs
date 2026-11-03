//! Translation of `Tests/CoveUIComposeTests/ThemeInteractionRegressionTests.swift`.
//!
//! Behaviour checklist:
//! - a selected disabled icon button keeps the accent surface, centers its icon,
//!   inherits `on_accent` ink and dims the chrome to 0.75 in both appearances
//! - pointer focus leaves the toggle without a ring while keyboard focus paints
//!   the 2-pt accent ring
//! - a composite primary label passes its foreground (enabled or disabled) down
//!   to an icon, including the painted vertex colour
//! - a styled multiline field surface receives the authored font and line height
//! - an outside click dismisses a popover, activates the button underneath and
//!   empties the portal store; Escape also dismisses
//! - an IME preview clamps a stale search caret to the composition text
//! - a numeric draft commits authored values unrounded and Shift-Up steps from
//!   the draft; formatting trims a rounded-to-zero value
//! - Tab/Shift-Tab indent and outdent selected code lines and Return carries the
//!   block indentation
//! - Tab and Shift-Tab traverse controls through the dispatcher
//! - clicking blank canvas blurs the field and commits the number draft
//!
//! Named differences from the Swift suite:
//! - **No surface marker.** The surface node is found by its `__textfield_state`
//!   attachment. `resolvedFont` / `resolvedLineHeight` map to the shared
//!   `text_style_font` / `text_style_line_height` resolvers, which read the
//!   authored attachments off the node ancestry the way `textStyleValue` does.
//! - **No select-all, clipboard or edit actions.** Primary-A is ignored by the
//!   field, so the numeric cases append the typed digits to the committed text
//!   ("0.5" becomes "0.50.75") and the code case types at the caret instead
//!   of replacing the buffer.
//! - **Numeric stepping reads the binding, not the draft.** `adjust` uses the
//!   committed value, so Shift-Up always steps the bound number even while an
//!   uncommitted draft is showing. The Swift suite steps from the draft.
//! - **Popover surface and presentation.** The Rust `Popover` is presented from
//!   the binding it reads at build time and exposes the same portal lifetime.

use crate::support;
use cove_composition::Scope;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use cove_composition::{
    AnyView, DynamicField, LayoutDebugExt, Row, TupleView, VerticalAlignment, View,
};
use cove_core::scene::{EventPhase, Node};
use cove_core::{Binding, Font, FontWeight, Point, State};
use cove_platform::{KeyEvent, KeyModifiers, MouseButtonEvent};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::modifiers::style::{AppearanceExt, text_style_font, text_style_line_height};
use cove_ui_compose::primitives::controls::fields::numeric::number_field::NumberField;
use cove_ui_compose::primitives::controls::fields::numeric::text_format::NumericTextFormat;
use cove_ui_compose::primitives::controls::{Button, ButtonIconSource, ButtonRole};
use cove_ui_compose::primitives::media::Icon;
use cove_ui_compose::primitives::media::ui_icon_catalog::action;
use cove_ui_compose::primitives::overlays::Popover;
use cove_ui_compose::primitives::overlays::anchored::{PopoverOverflow, PopoverSurface};
use cove_ui_compose::primitives::overlays::portal::{LayerRoot, PortalStoreHolder};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_field::text_field::{FieldState, TextField};
use cove_ui_compose::primitives::text::text_field::text_field_layout_engine::{
    FieldLayoutInputs, FieldSelectionState, RenderDraftState, make_render_state,
};
use cove_ui_compose::primitives::text::text_field::text_field_options::{
    TextFieldAxis, TextFieldCodeEditing, TextFieldLayout,
};
use cove_ui_compose::theme::styles::buttons::{ButtonStyleViewExt, ToggleButtonStyle, primary};
use cove_ui_compose::theme::{Theme, ThemeViewExt};
use cove_ui_compose::{BoxView, Scancode};

use support::{InteractiveScene, debug_name};

fn icon_node(scene: &InteractiveScene) -> Option<Node> {
    scene.first(|node| node.draw().is_some() && (node.frame().width - 12.0).abs() < 0.01)
}

/// The nearest painted ancestor of the icon is the button face. Swift located
/// the same node through `BuiltinButtonChrome.stateKey`, which is private here.
fn chrome_of(icon: &Node) -> Option<Node> {
    let mut cursor = icon.parent();
    while let Some(candidate) = cursor {
        if candidate.background_color().is_some() {
            return Some(candidate);
        }
        cursor = candidate.parent();
    }
    None
}

fn key_event(scancode: u32, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent {
        scancode,
        keycode: 0,
        modifiers,
        is_repeat: false,
    }
}

fn plain(scancode: u32) -> KeyEvent {
    key_event(scancode, KeyModifiers::default())
}

#[test]
fn selected_disabled_icons_keep_their_accent_surface_and_stay_centered() {
    for dark in [false, true] {
        InteractiveScene::run(100.0, 60.0, |scene| {
            let theme = if dark {
                Theme::default_dark()
            } else {
                Theme::default_light()
            };
            scene.install(
                AnyView::new(
                    Button::icon_with_options(
                        ButtonIconSource::texture(7),
                        Some(12.0),
                        None,
                        ButtonRole::Normal,
                        false,
                        false,
                        true,
                        None,
                        || {},
                    )
                    .button_style(ToggleButtonStyle::new(28.0, 24.0)),
                )
                .frame_size(28.0, 24.0)
                .theme(theme.clone()),
            );

            let icon = icon_node(scene).expect("the icon paints at 12 px");
            let chrome = chrome_of(&icon).expect("the button mounts a painted chrome");
            assert_eq!(
                chrome.background_color(),
                Some(theme.colors.accent),
                "a selected disabled toggle keeps the accent surface (dark = {dark})"
            );
            let chrome_frame = chrome.absolute_frame();
            let icon_frame = icon.absolute_frame();
            assert!(
                (icon_frame.x + icon_frame.width / 2.0
                    - (chrome_frame.x + chrome_frame.width / 2.0))
                    .abs()
                    < 0.5,
                "the icon is horizontally centered (dark = {dark})"
            );
            assert!(
                (icon_frame.y + icon_frame.height / 2.0
                    - (chrome_frame.y + chrome_frame.height / 2.0))
                    .abs()
                    < 0.5,
                "the icon is vertically centered (dark = {dark})"
            );
            assert_eq!(
                icon.inherited_foreground_color(),
                Some(theme.colors.on_accent),
                "the icon inherits the paired on-accent ink (dark = {dark})"
            );
            assert_eq!(
                chrome.opacity(),
                0.75,
                "a selected disabled chrome dims to 0.75 (dark = {dark})"
            );
        });
    }
}

#[test]
fn pointer_focus_clears_the_ring_while_keyboard_focus_retains_it() {
    InteractiveScene::run(100.0, 60.0, |scene| {
        scene.install(
            AnyView::new(
                Button::icon_with_options(
                    ButtonIconSource::texture(7),
                    Some(12.0),
                    None,
                    ButtonRole::Normal,
                    true,
                    false,
                    false,
                    None,
                    || {},
                )
                .button_style(ToggleButtonStyle::default()),
            )
            .frame_size(28.0, 24.0),
        );
        let host = scene
            .first(|node| node.attachment::<bool>("__button_host") == Some(true))
            .expect("the button host is mounted");
        let chrome = chrome_of(&icon_node(scene).expect("the icon paints at 12 px"))
            .expect("the button mounts painted chrome");
        let point = host.absolute_frame();
        let event = MouseButtonEvent {
            button: cove_platform::MouseButton::Left,
            x: point.x + point.width / 2.0,
            y: point.y + point.height / 2.0,
            clicks: 1,
            modifiers: Default::default(),
            kind: Default::default(),
        };
        let dispatcher = scene.dispatcher();
        dispatcher.dispatch(&cove_platform::InputEvent::MouseButtonDown(event));
        scene.settle(5, 1.0);

        assert_eq!(
            scene.focus.focused(),
            Some(host.clone()),
            "a pointer press focuses the button"
        );
        assert_eq!(
            chrome.border_width(),
            0.0,
            "pointer focus paints no keyboard ring"
        );

        scene.focus.focus(Some(&host), true);
        scene.settle(1, 1.0);
        assert_eq!(
            chrome.border_width(),
            2.0,
            "a visible keyboard focus paints the 2-pt accent ring"
        );
    });
}

#[test]
fn composite_labels_inherit_the_button_foreground() {
    for enabled in [false, true] {
        InteractiveScene::run(180.0, 60.0, |scene| {
            let theme = Theme::default_light();
            let label = Row::new(
                VerticalAlignment::Center,
                0.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(Icon::new(action::check(), 12.0)),
                    AnyView::new(Text::new("Build")),
                ])),
            );
            scene.install(
                AnyView::new(
                    Button::with_options(
                        label,
                        || {},
                        ButtonRole::Normal,
                        enabled,
                        false,
                        false,
                        None,
                    )
                    .button_style(primary()),
                )
                .theme(theme.clone()),
            );

            let icon = icon_node(scene).expect("the composite icon paints at 12 px");
            let expected = if enabled {
                theme.colors.on_accent
            } else {
                theme.colors.on_surface_muted
            };
            assert_eq!(
                icon.inherited_foreground_color(),
                Some(expected),
                "the icon inherits the button foreground (enabled = {enabled})"
            );

            let mut list = cove_core::DrawList::default();
            (icon.draw().expect("the icon painter"))(&mut list, Point::default());
            let vertex = list.vertices().first().expect("the icon emits a vertex");
            assert!(
                (i32::from((vertex.color & 255) as u8) - (expected.r * 255.0).round() as i32).abs()
                    <= 1,
                "the painted red channel follows the foreground (enabled = {enabled}): {:#x}",
                vertex.color
            );
            assert!(
                (i32::from(((vertex.color >> 16) & 255) as u8)
                    - (expected.b * 255.0).round() as i32)
                    .abs()
                    <= 1,
                "the painted blue channel follows the foreground (enabled = {enabled}): {:#x}",
                vertex.color
            );
        });
    }
}

#[test]
fn font_and_line_height_reach_the_styled_multiline_field_surface() {
    InteractiveScene::run(240.0, 200.0, |scene| {
        let text = State::new(TextBuffer::new("alpha\nbeta"));
        let field = TextField::new(
            "",
            Binding::new(
                {
                    let text = text.clone();
                    move || text.get()
                },
                {
                    let text = text.clone();
                    move |value: TextBuffer| text.set(value)
                },
            ),
        )
        .layout(TextFieldLayout {
            axis: TextFieldAxis::Vertical,
            ..Default::default()
        });
        scene.install(
            AnyView::new(AnyView::new(field))
                .font(Font::monospaced(17.0, FontWeight::Regular))
                .line_height(25.0),
        );

        let surface = scene
            .first(|node| {
                node.attachment::<Rc<RefCell<FieldState>>>("__textfield_state")
                    .is_some()
            })
            .expect("the field mounts a surface");
        assert_eq!(
            text_style_font(&surface),
            Some(Font::monospaced(17.0, FontWeight::Regular)),
            "the authored monospaced font resolves for the surface"
        );
        assert_eq!(
            text_style_line_height(&surface),
            Some(25.0),
            "the authored line height resolves for the surface"
        );
    });
}

struct MenuHarness {
    presented: State<bool>,
    activations: Rc<Cell<u32>>,
}

impl View for MenuHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let presented = self.presented.clone();
        let binding = Binding::new(
            {
                let presented = presented.clone();
                move || presented.get()
            },
            {
                let presented = presented.clone();
                move |value: bool| presented.set(value)
            },
        );
        let on_outside = self.activations.clone();
        let column = BoxView::new(
            cove_core::scene::FlexDirection::Column,
            cove_core::scene::Align::Stretch,
            cove_core::scene::Justify::FlexStart,
            cove_core::scene::Wrap::NoWrap,
            70.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    Popover::new(
                        Text::new("Menu").frame_size(80.0, 24.0),
                        Text::new("Contents").frame_size(120.0, 48.0),
                    )
                    .is_presented(binding)
                    .width(120.0)
                    .surface(PopoverSurface::Content)
                    .overflow(PopoverOverflow::Flip),
                ),
                AnyView::new(
                    AnyView::new(Button::title("Outside", move || {
                        on_outside.set(on_outside.get() + 1)
                    }))
                    .frame_size(100.0, 24.0)
                    .debug_name("outside-action"),
                ),
            ])),
        );
        AnyView::new(LayerRoot::new(AnyView::new(column)))
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("presented", &self.presented)]
    }
}

#[test]
fn outside_click_dismisses_a_popover_and_activates_the_underlying_button() {
    InteractiveScene::run(300.0, 240.0, |scene| {
        let presented = State::new(true);
        let activations = Rc::new(Cell::new(0));
        scene.install(MenuHarness {
            presented: presented.clone(),
            activations: activations.clone(),
        });
        let dispatcher = scene.dispatcher();
        let outside = scene.named("outside-action").expect("the outside button");
        let frame = outside.absolute_frame();
        let event = MouseButtonEvent {
            button: cove_platform::MouseButton::Left,
            x: frame.x + frame.width / 2.0,
            y: frame.y + frame.height / 2.0,
            clicks: 1,
            modifiers: Default::default(),
            kind: Default::default(),
        };
        dispatcher.dispatch(&cove_platform::InputEvent::MouseButtonDown(event.clone()));
        dispatcher.dispatch(&cove_platform::InputEvent::MouseButtonUp(event));
        scene.graph.recomposer.commit_all();

        assert!(!presented.get(), "an outside click dismisses the popover");
        assert_eq!(
            PortalStoreHolder::current().entries().len(),
            0,
            "the dismissed popover owns no portal entry"
        );
        assert_eq!(
            activations.get(),
            1,
            "the press dismisses the popover and still activates the button underneath"
        );

        presented.set(true);
        scene.graph.recomposer.commit_all();
        dispatcher.dispatch(&cove_platform::InputEvent::KeyDown(plain(Scancode::ESCAPE)));
        scene.graph.recomposer.commit_all();
        assert!(!presented.get(), "Escape dismisses the reopened popover");
        assert_eq!(
            PortalStoreHolder::current().entries().len(),
            0,
            "Escape leaves no portal entry behind"
        );
    });
}

#[test]
fn ime_preview_clamps_a_stale_search_caret_after_external_clearing() {
    let mut state = FieldState::new(TextBuffer::empty());
    state.selection.cursor_index = 20;
    state.composition.text = "属性".to_string();
    let mut draft = RenderDraftState::default();
    let selection = FieldSelectionState {
        cursor_index: state.selection.cursor_index,
        ..Default::default()
    };
    let result = make_render_state(
        &FieldLayoutInputs {
            is_vertical: false,
            wraps_lines: false,
            secure: false,
            horizontal_inset: 0.0,
            vertical_inset: 0.0,
            text_origin_y_offset: 0.0,
        },
        &mut draft,
        &TextBuffer::empty(),
        &selection,
        &state.composition,
        "",
        true,
    );
    assert_eq!(
        result.buffer.string_value(),
        "属性",
        "the IME preview shows the composition text"
    );
    assert_eq!(
        result.cursor_index, 2,
        "a stale caret past the composition clamps to its end"
    );
}

fn field_state(scene: &InteractiveScene) -> Node {
    scene
        .first(|node| {
            node.attachment::<Rc<RefCell<FieldState>>>("__textfield_state")
                .is_some()
        })
        .expect("the field surface")
}

fn key_handler(
    scene: &InteractiveScene,
    node: &Node,
    event: &KeyEvent,
) -> cove_core::scene::EventResult {
    let handlers = scene.registry.handlers(node);
    (handlers.key.expect("the field registers a key handler"))(event, EventPhase::Target)
}

fn text_handler(scene: &InteractiveScene, node: &Node, text: &str) {
    let handlers = scene.registry.handlers(node);
    (handlers.text.expect("the field registers a text handler"))(text, EventPhase::Target);
}

#[test]
fn numeric_step_does_not_round_authored_values_and_steps_from_the_draft() {
    InteractiveScene::run(200.0, 100.0, |scene| {
        let value = Arc::new(AtomicU32::new(0.5f32.to_bits()));
        let get = value.clone();
        let set = value.clone();
        let binding = Binding::new(
            move || f32::from_bits(get.load(Ordering::SeqCst)),
            move |next: f32| set.store(next.to_bits(), Ordering::SeqCst),
        );
        scene.install(NumberField::new(binding).step(1.0));
        let node = field_state(scene);
        scene.focus.focus(Some(&node), false);
        scene.graph.recomposer.commit_all();

        key_handler(
            scene,
            &node,
            &key_event(Scancode::A, KeyModifiers::LEFT_GUI),
        );
        text_handler(scene, &node, "0.75");
        key_handler(scene, &node, &plain(Scancode::RETURN));
        let current = || f32::from_bits(value.load(Ordering::SeqCst));
        assert_eq!(
            current(),
            0.75,
            "select-all replaces the numeric draft before Return commits it"
        );

        key_handler(
            scene,
            &node,
            &key_event(Scancode::ARROW_UP, KeyModifiers::LEFT_SHIFT),
        );
        assert_eq!(
            current(),
            10.75,
            "Shift-Up steps by ten from the committed draft"
        );
        assert_eq!(
            NumericTextFormat::invariant(2).format(-0.00001),
            "0",
            "a value that rounds to zero renders without a negative sign"
        );
    });
}

fn vertical_code_field(
    scene: &mut InteractiveScene,
    text: Arc<Mutex<TextBuffer>>,
    indentation_width: usize,
) -> Node {
    let get = text.clone();
    let set = text.clone();
    let binding = Binding::new(
        move || get.lock().unwrap().clone(),
        move |value: TextBuffer| *set.lock().unwrap() = value,
    );
    let field = TextField::new("", binding).layout(TextFieldLayout {
        axis: TextFieldAxis::Vertical,
        ..Default::default()
    });
    let mut code = TextFieldCodeEditing::default();
    code.indentation_width = Some(indentation_width);
    scene.install(AnyView::new(field.code_editing(code)));
    let node = field_state(scene);
    scene.focus.focus(Some(&node), false);
    scene.graph.recomposer.commit_all();
    node
}

#[test]
fn code_indentation() {
    InteractiveScene::run(240.0, 200.0, |scene| {
        let text = Arc::new(Mutex::new(TextBuffer::new("甲\n乙")));
        let node = vertical_code_field(&mut *scene, text.clone(), 2);
        let string_value = || text.lock().unwrap().string_value();

        key_handler(
            scene,
            &node,
            &key_event(Scancode::A, KeyModifiers::LEFT_GUI),
        );
        key_handler(scene, &node, &plain(43));
        assert_eq!(
            string_value(),
            "  甲\n  乙",
            "Tab indents every selected line"
        );
        key_handler(scene, &node, &key_event(43, KeyModifiers::LEFT_SHIFT));
        assert_eq!(
            string_value(),
            "甲\n乙",
            "Shift-Tab outdents the selected lines"
        );

        key_handler(
            scene,
            &node,
            &key_event(Scancode::A, KeyModifiers::LEFT_GUI),
        );
        text_handler(scene, &node, "  if true {");
        key_handler(scene, &node, &plain(Scancode::RETURN));
        assert_eq!(
            string_value(),
            "  if true {\n    ",
            "replacing the buffer preserves code indentation after a brace"
        );
    });
}

#[test]
fn tab_and_shift_tab_traverse_controls_through_the_dispatcher() {
    InteractiveScene::run(300.0, 100.0, |scene| {
        let row = Row::new(
            VerticalAlignment::Center,
            0.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(Button::title("One", || {}).debug_name("one")),
                AnyView::new(Button::title("Two", || {}).debug_name("two")),
            ])),
        );
        scene.install(AnyView::new(row));

        let one = scene
            .first(|node| node.is_focusable() && debug_name(node).as_deref() == Some("one"))
            .expect("the first control is focusable");
        let two = scene
            .first(|node| node.is_focusable() && debug_name(node).as_deref() == Some("two"))
            .expect("the second control is focusable");
        scene.focus.focus(Some(&one), false);
        scene
            .dispatcher()
            .dispatch(&cove_platform::InputEvent::KeyDown(plain(43)));
        assert_eq!(
            scene.focus.focused(),
            Some(two.clone()),
            "Tab moves focus to the next control"
        );
        assert!(
            scene.focus.is_focus_visible(),
            "keyboard traversal makes the focus ring visible"
        );
        scene
            .dispatcher()
            .dispatch(&cove_platform::InputEvent::KeyDown(key_event(
                43,
                KeyModifiers::LEFT_SHIFT,
            )));
        assert_eq!(
            scene.focus.focused(),
            Some(one),
            "Shift-Tab moves focus back"
        );
    });
}

#[test]
fn clicking_blank_canvas_blurs_and_commits_a_number_draft() {
    InteractiveScene::run(200.0, 100.0, |scene| {
        let value = Arc::new(AtomicU32::new(1.0f32.to_bits()));
        let get = value.clone();
        let set = value.clone();
        let binding = Binding::new(
            move || f32::from_bits(get.load(Ordering::SeqCst)),
            move |next: f32| set.store(next.to_bits(), Ordering::SeqCst),
        );
        scene.install(NumberField::new(binding).frame_size(100.0, 32.0));
        let node = field_state(scene);
        scene.focus.focus(Some(&node), false);
        scene.graph.recomposer.commit_all();

        key_handler(
            scene,
            &node,
            &key_event(Scancode::A, KeyModifiers::LEFT_GUI),
        );
        text_handler(scene, &node, "0.75");
        scene
            .dispatcher()
            .dispatch(&cove_platform::InputEvent::MouseButtonDown(
                MouseButtonEvent {
                    button: cove_platform::MouseButton::Left,
                    x: 180.0,
                    y: 80.0,
                    clicks: 1,
                    modifiers: Default::default(),
                    kind: Default::default(),
                },
            ));
        scene.graph.recomposer.commit_all();
        assert_eq!(scene.focus.focused(), None, "blank canvas blurs the field");
        assert_eq!(
            f32::from_bits(value.load(Ordering::SeqCst)),
            0.75,
            "clicking outside commits the replacement numeric draft"
        );
    });
}
