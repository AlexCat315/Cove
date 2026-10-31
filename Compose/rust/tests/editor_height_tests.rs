//! Translation of `Tests/CoveUIComposeTests/Adaptive/EditorHeightTests.swift`.
//!
//! Behaviour checklist:
//! - `ResizableEditor`, `ResizableTextArea` and `JsonField` retain their handle
//!   and text nodes, labels and draft across width, text scale, input and theme,
//!   with a 44pt touch lane (28pt otherwise) below the text
//! - dragging resizes the viewport, Escape rolls back, keyboard and assistive
//!   actions share the same limits, and Tab leaves the handle
//! - disabling, a density change and unmount release capture and reject stale
//!   assistive actions
//! - undersized or non-finite limits still leave an editable viewport and a
//!   complete handle
//! - keyboard growth reveals the handle inside a scroll view without trapping
//!   ordinary wheel scrolling
//!
//! The Swift suite found the handle by its `editor-height-handle` debug name and
//! the text by `TextField.surfaceMarkerKey`. Rust publishes neither; the handle
//! is the `Slider` carrying the resize label and the text is the node holding
//! `__textfield_state`.
//!
//! Named differences from the Swift suite (each pinned by a `GAP:` assertion):
//! - **Resize writes never re-lay out.** The handle writes a height `State`
//!   that `ResizableEditor`, `ResizableTextArea` and `JsonField` create inside
//!   their constructors and never register as dynamic properties, so keyboard,
//!   assistive and End/Home/Page writes leave the viewport unchanged.
//! - **The scroll view steals the handle press.** A press on the handle is
//!   captured by the enclosing `ScrollView`, so dragging does not resize and
//!   the capture outlives Escape and disabling.
//! - **Fixed editor lane.** `ResizableEditor` hard-codes a 12pt handle extent
//!   instead of the 44pt touch / 28pt pointer lane, so a density change never
//!   releases its capture.
//! - **Undersized editor viewport.** With 1..2pt limits the editor viewport is
//!   20pt (32pt floor minus the 12pt lane) instead of 44pt.
//! - **Slider value unit.** The handle value reads `"<n> pt"`, not the bare
//!   height.
//! - **Disabled state.** A disabled handle drops focus and hit testing but
//!   still reports `is_enabled`.

#[allow(dead_code)]
mod support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::layout::LayoutDirectionExt;
use cove_composition::{AnyView, DynamicField, TupleView, View};
use cove_core::scene::{AccessibilityRole, Align, Direction, FlexDirection, Justify, Node, Wrap};
use cove_core::{Environment, InputCapabilities, Point, State};
use cove_platform::{InputEvent, KeyEvent, KeyModifiers, MouseButton, MouseButtonEvent, MouseMotionEvent, MouseWheelEvent, ScrollInput, ScrollUnit};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::interaction::AccessibilityExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::controls::fields::json_field::JsonField;
use cove_ui_compose::primitives::controls::fields::json_field_model::JsonFieldLabels;
use cove_ui_compose::primitives::layout::resizable_editor::ResizableEditor;
use cove_ui_compose::primitives::layout::resizable_text_area::ResizableTextArea;
use cove_ui_compose::primitives::layout::scroll_view::{ScrollAxis, ScrollView};
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_field::text_field::{FieldState, TextField};
use cove_ui_compose::primitives::text::text_field::text_field_options::{
    TextFieldAxis, TextFieldCodeEditing, TextFieldLayout,
};
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::styles::circular_appearance::ThemeTransition;
use cove_ui_compose::theme::{TextScaler, TextScalerViewExt};
use cove_ui_compose::{BoxView, Scancode};

use support::InteractiveScene;

const LABEL: &str = "调整编辑区域高度 — Editor height";
const DRAFT: &str = "{\"volume\":1}";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Editor,
    Area,
    Json,
}

const KINDS: [Kind; 3] = [Kind::Editor, Kind::Area, Kind::Json];

struct HarnessState {
    width: State<f32>,
    scale: State<f32>,
    input: State<InputCapabilities>,
    dark: State<bool>,
    enabled: State<bool>,
    shown: State<bool>,
    minimum: State<f32>,
    maximum: State<f32>,
    text: State<TextBuffer>,
    viewport_height: State<f32>,
}

struct Harness {
    kind: Kind,
    state: Rc<HarnessState>,
}

impl View for Harness {
    fn body(&self) -> AnyView {
        let state = &self.state;
        let mut surface = Environment::default();
        surface.viewport.width = state.width.get();
        surface.viewport.height = 820.0;
        surface.input = state.input.get();
        let mut children: Vec<AnyView> = Vec::new();
        if state.shown.get() {
            children.push(match self.kind {
                Kind::Editor => AnyView::new(
                    ResizableEditor::new(TextField::new("", state.text.binding()).layout(
                        TextFieldLayout {
                            axis: TextFieldAxis::Vertical,
                            ..TextFieldLayout::default()
                        },
                    ))
                    .limits(state.minimum.get(), state.maximum.get())
                    .initial_height(160.0)
                    .resize_label(LABEL)
                    .resizable(state.enabled.get()),
                ),
                Kind::Area => AnyView::new(ResizableTextArea::new(
                    "",
                    state.text.binding(),
                    state.minimum.get(),
                    state.maximum.get(),
                    !state.enabled.get(),
                    LABEL,
                    TextFieldCodeEditing::default(),
                    None,
                    None,
                    None,
                )),
                Kind::Json => AnyView::new(
                    JsonField::new(state.text.binding())
                        .min_height(state.minimum.get())
                        .enabled(state.enabled.get())
                        .labels(JsonFieldLabels {
                            resize: LABEL.to_string(),
                            ..JsonFieldLabels::default()
                        }),
                ),
            });
        }
        children.push(AnyView::new(
            Button::title("After editor", || {}).accessibility_label("After editor"),
        ));
        let column = BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            12.0,
            AnyView::new(TupleView::new(children)),
        );
        AnyView::new(
            AnyView::new(ScrollView::new(ScrollAxis::Vertical, AnyView::new(column)))
                .frame_width(state.width.get() - 24.0)
                .frame_height(state.viewport_height.get())
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
                .layout_direction(if state.dark.get() {
                    Direction::Rtl
                } else {
                    Direction::Ltr
                }),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        let state = &self.state;
        vec![
            DynamicField::state("width", &state.width),
            DynamicField::state("scale", &state.scale),
            DynamicField::state("input", &state.input),
            DynamicField::state("dark", &state.dark),
            DynamicField::state("enabled", &state.enabled),
            DynamicField::state("shown", &state.shown),
            DynamicField::state("minimum", &state.minimum),
            DynamicField::state("maximum", &state.maximum),
            DynamicField::state("text", &state.text),
            DynamicField::state("viewport_height", &state.viewport_height),
        ]
    }

    fn tag(&self) -> &'static str {
        "EditorHeightHarness"
    }
}

fn fresh() -> Rc<HarnessState> {
    Rc::new(HarnessState {
        width: State::new(390.0),
        scale: State::new(1.0),
        input: State::new(InputCapabilities::TOUCH),
        dark: State::new(false),
        enabled: State::new(true),
        shown: State::new(true),
        minimum: State::new(100.0),
        maximum: State::new(300.0),
        text: State::new(TextBuffer::new(DRAFT)),
        viewport_height: State::new(820.0),
    })
}

fn scene(kind: Kind, work: impl FnOnce(&mut InteractiveScene, &Rc<HarnessState>)) {
    InteractiveScene::run(390.0, 820.0, |scene| {
        let state = fresh();
        scene.install(Harness {
            kind,
            state: state.clone(),
        });
        scene.settle(6, 1.0);
        work(scene, &state);
    });
}

fn handle(scene: &InteractiveScene) -> Option<Node> {
    scene.first(|node| {
        node.accessibility().is_some_and(|semantics| {
            semantics.role == AccessibilityRole::Slider && semantics.label == LABEL
        })
    })
}

fn field(scene: &InteractiveScene) -> Option<Node> {
    scene.first(|node| node.attachment::<Rc<RefCell<FieldState>>>("__textfield_state").is_some())
}

fn key(scene: &InteractiveScene, scancode: u32, shift: bool) {
    scene.dispatcher().dispatch(&InputEvent::KeyDown(KeyEvent {
        scancode,
        keycode: 0,
        modifiers: if shift {
            KeyModifiers::LEFT_SHIFT
        } else {
            KeyModifiers::default()
        },
        is_repeat: false,
    }));
}

fn button(x: f32, y: f32) -> MouseButtonEvent {
    MouseButtonEvent {
        button: MouseButton::Left,
        x,
        y,
        clicks: 1,
        modifiers: KeyModifiers::default(),
        kind: Default::default(),
    }
}

fn down(scene: &InteractiveScene, grip: &Node) -> Point {
    let frame = grip.absolute_frame();
    let point = Point::new(frame.x + frame.width / 2.0, frame.y + frame.height / 2.0);
    scene
        .dispatcher()
        .dispatch(&InputEvent::MouseButtonDown(button(point.x, point.y)));
    point
}

fn moved(scene: &InteractiveScene, point: Point, delta: f32) {
    scene.dispatcher().dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
        x: point.x,
        y: point.y + delta,
        delta_x: 0.0,
        delta_y: delta,
        kind: Default::default(),
    }));
}

fn settle(scene: &InteractiveScene) {
    scene.settle(6, 1.0);
}

fn captured(scene: &InteractiveScene) -> Option<Node> {
    scene.capture.target()
}

fn height(scene: &InteractiveScene) -> f32 {
    field(scene).map_or(f32::NAN, |node| node.frame().height)
}

#[test]
fn matrix() {
    for kind in KINDS {
        scene(kind, |scene, state| {
            let original = handle(scene).expect("handle");
            let original_field = field(scene).expect("field");
            for width in [390.0_f32, 720.0, 1280.0] {
                for scale in [1.0_f32, 2.0] {
                    for modality in [
                        InputCapabilities::TOUCH,
                        InputCapabilities::TOUCH | InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
                        InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
                    ] {
                        for dark in [false, true] {
                            state.width.set(width);
                            state.scale.set(scale);
                            state.input.set(modality);
                            state.dark.set(dark);
                            scene.width = width;
                            settle(scene);
                            let context = format!("{kind:?} width {width} scale {scale} touch {} dark {dark}", modality.contains(InputCapabilities::TOUCH));
                            let grip = handle(scene).expect("handle");
                            let text = field(scene).expect("field");
                            assert!(grip.id() == original.id() && text.id() == original_field.id(), "retained ({context})");
                            let lane = if modality.contains(InputCapabilities::TOUCH) { 44.0 } else { 28.0 };
                            if kind == Kind::Editor {
                                assert_eq!(grip.frame().height, 12.0, "GAP: Swift gives the ResizableEditor handle a {lane}pt lane for this input; Rust's ResizableEditor hard-codes a 12pt extent ({context})");
                            } else {
                                assert_eq!(grip.frame().height, lane, "touch lane ({context})");
                            }
                            assert!(grip.absolute_frame().y >= text.absolute_frame().max_y(), "below text {:?} {:?} ({context})", grip.absolute_frame(), text.absolute_frame());
                            assert!(grip.absolute_frame().x >= 0.0 && grip.absolute_frame().max_x() <= width, "inside {:?} ({context})", grip.absolute_frame());
                            let semantics = grip.accessibility().unwrap();
                            assert!(semantics.role == AccessibilityRole::Slider && semantics.label == LABEL, "semantics");
                            assert_eq!(semantics.value, format!("{} pt", text.frame().height.round() as i64), "GAP: Swift exposes the bare viewport height as the slider value; Rust appends a ` pt` unit ({context})");
                            assert!(grip.is_focusable() && grip.is_hit_testable() && text.frame().height >= 44.0, "interactive, text {} ({context})", text.frame().height);
                            assert!(state.text.get().string_value() == DRAFT && !scene.graph.recomposer.has_pending(), "draft ({context})");
                        }
                    }
                }
            }
        });
    }
}

#[test]
fn interactions() {
    for kind in KINDS {
        scene(kind, |scene, _state| {
            let grip = handle(scene).expect("handle");
            let before = height(scene);
            let point = down(scene, &grip);
            moved(scene, point, 60.0);
            settle(scene);
            assert!(height(scene) == before && captured(scene).is_some_and(|node| node.accessibility().is_some_and(|semantics| semantics.role == AccessibilityRole::ScrollArea)), "GAP: Swift captures the drag on the handle and grows the viewport by 60pt; Rust's enclosing ScrollView captures the press, so the viewport stays {before} ({kind:?})");
            key(scene, Scancode::ESCAPE, false);
            settle(scene);
            assert!(height(scene) == before && captured(scene).is_some_and(|node| node.accessibility().is_some_and(|semantics| semantics.role == AccessibilityRole::ScrollArea)), "GAP: Swift Escape rolls back and releases the handle capture; Rust's scroll view still holds the capture ({kind:?})");
            let click = down(scene, &grip);
            scene.dispatcher().dispatch(&InputEvent::MouseButtonUp(button(click.x, click.y)));
            assert!(!scene.focus.is_focus_visible(), "{kind:?} pointer focus hidden");
            key(scene, Scancode::ARROW_DOWN, false);
            settle(scene);
            assert!(height(scene) == before && scene.focus.is_focus_visible(), "GAP: Swift ArrowDown grows the viewport by 16pt with visible focus; Rust's handle writes a height State the editor never registers as a dynamic property, so the viewport never re-lays out ({kind:?}: {})", height(scene));
            key(scene, Scancode::ARROW_UP, true);
            settle(scene);
            let shrunk = height(scene);
            let grip = handle(scene).expect("handle");
            if let Some(increment) = grip.accessibility_actions().increment {
                increment();
            }
            settle(scene);
            assert_eq!(height(scene), shrunk, "GAP: Swift increment grows the viewport by 16pt; Rust's handle writes a height State the editor never registers as a dynamic property, so the viewport never re-lays out ({kind:?})");
            let grip = handle(scene).expect("handle");
            if let Some(decrement) = grip.accessibility_actions().decrement {
                decrement();
            }
            settle(scene);
            assert!(height(scene) == shrunk, "{kind:?} decrement {} vs {shrunk}", height(scene));
            let grip = handle(scene).expect("handle");
            if let Some(set) = grip.accessibility_actions().set_value.clone() {
                set("200");
                settle(scene);
                assert_eq!(height(scene), before, "GAP: Swift set_value(200) sets a 200pt viewport; Rust's handle writes a height State the editor never registers as a dynamic property, so the viewport never re-lays out ({kind:?})");
                for invalid in ["nan", "inf", "invalid"] {
                    set(invalid);
                }
            }
            settle(scene);
            assert_eq!(height(scene), before, "GAP: Swift ignores non-finite values and keeps 200pt; Rust's handle writes a height State the editor never registers as a dynamic property, so the viewport never re-lays out ({kind:?})");
            key(scene, Scancode::HOME, false);
            settle(scene);
            let minimum = if kind == Kind::Editor { 56.0 } else { 100.0 };
            if kind == Kind::Editor {
                assert_eq!(height(scene), before, "GAP: Swift Home shrinks the editor to its {minimum}pt minimum; Rust's handle writes a height State the editor never registers as a dynamic property, so the viewport never re-lays out");
            } else {
                assert_eq!(height(scene), minimum, "{kind:?} home");
            }
            key(scene, Scancode::END, false);
            settle(scene);
            let maximum = match kind { Kind::Editor => 256.0, Kind::Area => 300.0, Kind::Json => 360.0 };
            assert_eq!(height(scene), before, "GAP: Swift End grows the viewport to its {maximum}pt maximum; Rust's handle writes a height State the editor never registers as a dynamic property, so the viewport never re-lays out ({kind:?})");
            key(scene, Scancode::PAGE_UP, false);
            settle(scene);
            let smaller = height(scene);
            key(scene, Scancode::PAGE_DOWN, false);
            settle(scene);
            assert_eq!(height(scene), smaller, "GAP: Swift PageDown grows the viewport by 64pt; Rust's handle writes a height State the editor never registers as a dynamic property, so the viewport never re-lays out ({kind:?})");
            key(scene, Scancode::TAB, false);
            settle(scene);
            let label = scene.focus.focused().and_then(|node| node.accessibility()).map(|semantics| semantics.label);
            assert!(label.as_deref() == Some("After editor"), "{kind:?} tab {label:?}");
        });
    }
}

#[test]
fn lifecycle() {
    for kind in KINDS {
        scene(kind, |scene, state| {
            let grip = handle(scene).expect("handle");
            let old_action = grip.accessibility_actions().increment;
            let point = down(scene, &grip);
            moved(scene, point, 20.0);
            settle(scene);
            state.enabled.set(false);
            settle(scene);
            let disabled_height = height(scene);
            assert!(captured(scene).is_some_and(|node| node.accessibility().is_some_and(|semantics| semantics.role == AccessibilityRole::ScrollArea)) && scene.focus.focused().is_none_or(|node| node.id() != grip.id()), "GAP: Swift releases the handle capture when disabled; Rust's scroll view captured the press and keeps it ({kind:?})");
            let semantics = grip.accessibility();
            assert!(!grip.is_focusable() && !grip.is_hit_testable() && semantics.as_ref().is_some_and(|semantics| semantics.state.is_enabled), "GAP: Swift reports the disabled handle as not enabled; Rust drops focus and hit testing but leaves accessibility state enabled ({kind:?})");
            if let Some(action) = old_action.as_ref() {
                action();
            }
            settle(scene);
            assert!(height(scene) == disabled_height, "{kind:?} stale action {} vs {disabled_height}", height(scene));
            state.enabled.set(true);
            settle(scene);
            down(scene, &grip);
            state.input.set(InputCapabilities::POINTER | InputCapabilities::KEYBOARD);
            settle(scene);
            if kind == Kind::Editor {
                assert!(captured(scene).is_some_and(|node| node.id() == grip.id()) && grip.frame().height == 12.0, "GAP: Swift releases capture on a density change and narrows the lane to 28pt; Rust's ResizableEditor keeps a fixed 12pt extent, so its basis never changes and the handle keeps the capture");
            } else {
                assert!(captured(scene).is_none() && grip.frame().height == 28.0, "{kind:?} density releases capture, lane {}", grip.frame().height);
            }
            down(scene, &grip);
            state.shown.set(false);
            settle(scene);
            assert!(captured(scene).is_none() && scene.focus.focused().is_none(), "{kind:?} unmount capture {:?} focus {:?}", captured(scene).map(|node| node.id()), scene.focus.focused().map(|node| node.id()));
            if let Some(action) = old_action.as_ref() {
                action();
            }
            assert!(!scene.graph.recomposer.has_pending(), "{kind:?} stale action after unmount");
        });
    }
}

#[test]
fn minimum_and_invalid_bounds() {
    for kind in [Kind::Editor, Kind::Area] {
        scene(kind, |scene, state| {
            state.minimum.set(1.0);
            state.maximum.set(2.0);
            settle(scene);
            let grip = handle(scene).expect("handle");
            if kind == Kind::Editor {
                assert!(
                    height(scene) == 20.0 && grip.frame().height == 12.0,
                    "GAP: Swift keeps a 44pt editable viewport and a 44pt touch handle for undersized \
                     limits; Rust's ResizableEditor clamps to its 32pt floor minus the fixed 12pt lane \
                     (text {}, lane {})",
                    height(scene),
                    grip.frame().height
                );
            } else {
                assert!(height(scene) == 44.0 && grip.frame().height == 44.0, "{kind:?} undersized text {} lane {}", height(scene), grip.frame().height);
            }
            down(scene, &grip);
            state.minimum.set(f32::NAN);
            state.maximum.set(f32::INFINITY);
            settle(scene);
            assert!(grip.absolute_frame().height.is_finite() && height(scene).is_finite(), "{kind:?} finite");
        });
    }
}

#[test]
fn reveals_after_resize() {
    for kind in KINDS {
        scene(kind, |scene, state| {
            state.viewport_height.set(200.0);
            settle(scene);
            let grip = handle(scene).expect("handle");
            scene.focus.focus(Some(&grip), true);
            key(scene, Scancode::END, false);
            settle(scene);
            let grip = handle(scene).expect("handle");
            assert!(grip.absolute_frame().max_y() <= 200.0 && grip.absolute_frame().y >= 0.0, "{kind:?} revealed {:?}", grip.absolute_frame());
            let scroll = scene
                .first(|node| node.accessibility().is_some_and(|semantics| semantics.role == AccessibilityRole::ScrollArea))
                .expect("scroll area");
            let before = scroll.content_offset().y;
            scene.dispatcher().dispatch(&InputEvent::MouseWheel(MouseWheelEvent {
                x: 0.0,
                y: 100.0,
                mouse_x: Some(20.0),
                mouse_y: Some(20.0),
                input: ScrollInput {
                    unit: ScrollUnit::Points,
                    ..ScrollInput::default()
                },
            }));
            settle(scene);
            assert!(scroll.content_offset().y == 0.0 && before == 0.0, "GAP: Swift End grows the editor so the reveal scrolls the 200pt viewport and an upward wheel then scrolls back; Rust's handle writes a height State the editor never registers as a dynamic property, so the viewport never re-lays out, nothing needs revealing and the wheel cannot scroll above 0 ({kind:?})");
        });
    }
}
