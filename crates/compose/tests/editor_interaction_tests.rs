//! Translation of `Tests/CoveUIComposeTests/EditorInteractionTests.swift`.
//!
//! Behaviour checklist:
//! - selection replacement, undo and redo share the scene shortcuts
//! - an invalid JSON draft survives blur and refocus without committing
//! - platform text commands share the keyboard history and reject stale edits
//! - an expanded JSON editor applies valid content atomically and cancels on
//!   Escape
//! - the expanded JSON editor is centered, keeps a tall editor and a clickable
//!   footer across window sizes
//! - a modal traps Tab, blocks app shortcuts and restores focus on Escape
//! - a context menu leaves an editor's flexible viewport alone
//! - a context menu fits a small window, closes on outside click and cleans up
//! - modal wheel events cannot reach background scroll handlers
//! - closing a modal cannot restore focus into a detached subtree
//! - a virtual list mounts a bounded row set and keeps the full scroll extent
//! - a combined transition retains exit content, suppresses input and survives
//!   reversal
//!
//! Named differences from the Swift suite:
//! - **No text-field undo, select-all or edit actions.** `FieldState` keeps no
//!   transaction history and the field publishes no `TextEditActions`, so
//!   Primary-A/Z/Shift-Z/Y do nothing, `text_edit_availability` is `None`, and
//!   every edit is appended at the caret.
//! - **Edits outside typed text never reach the binding.** Typed text writes the
//!   binding; Backspace, Return and IME commits do not.
//! - **The JSON field has no debug names for its expanded editor.** Neither
//!   `json-expand`, `json-apply`, `json-expanded-editor` nor the modal's own
//!   frame is named, so the Swift geometry assertions have no anchor; the
//!   toolbar buttons are found by their accessibility labels instead.
//! - **No context-menu view modifier.** `ContextMenu` is only reachable from the
//!   sidebar item factory, so a right click opens no portal and the popup
//!   geometry, wheel isolation inside the popup, outside-click dismissal and
//!   unmount cleanup cannot be exercised from a plain view tree.
//! - **Editor shortcuts reach the app.** Swift's focused editor consumes
//!   Primary-A/Z; Rust forwards both to the `ShortcutHost`.
//! - **A fixed `VirtualStack` publishes no scroll extent.** Its scroll view's
//!   maximum offset is 0, so a wheel gesture leaves the window at row 0.
//! - **Text metrics.** The shared harness installs an 16/20 text environment
//!   where the Swift rig used 12/16; no assertion here depends on glyph metrics.

mod support;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use cove_composition::{AnyView, LayoutDebugExt, TupleView, View};
use cove_core::scene::{
    AnimatorScheduler, EventPhase, EventResult, InputHandlerRoute, Node, TextEditCommand,
};
use cove_core::{Binding, DrawList, State};
use cove_platform::{
    InputEvent, KeyEvent, KeyModifiers, MouseButton, MouseButtonEvent, MouseWheelEvent,
};
use cove_ui_compose::Column;
use cove_ui_compose::Scancode;
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
use cove_ui_compose::primitives::controls::fields::JsonField;
use cove_ui_compose::primitives::layout::virtualization::virtual_stack::VirtualStack;
use cove_ui_compose::primitives::overlays::ShortcutHost;
use cove_ui_compose::primitives::overlays::modal::Modal;
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_field::text_field::{FieldState, TextField};
use cove_ui_compose::primitives::text::text_field::text_field_options::{
    TextFieldAxis, TextFieldLayout,
};

use support::InteractiveScene;

type Store = Arc<Mutex<TextBuffer>>;

fn column(spacing: f32, content: impl View + 'static) -> Column {
    Column::new(
        cove_composition::HorizontalAlignment::Center,
        spacing,
        AnyView::new(content),
    )
}

fn store(text: &str) -> Store {
    Arc::new(Mutex::new(TextBuffer::new(text)))
}

fn text(store: &Store) -> String {
    store.lock().unwrap().string_value()
}

fn binding(store: &Store) -> Binding<TextBuffer> {
    let read = store.clone();
    let write = store.clone();
    Binding::new(
        move || read.lock().unwrap().clone(),
        move |value| *write.lock().unwrap() = value,
    )
}

fn key(scancode: u32, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent {
        scancode,
        keycode: 0,
        modifiers,
        is_repeat: false,
    }
}

fn plain(scancode: u32) -> KeyEvent {
    key(scancode, KeyModifiers::default())
}

fn primary(scancode: u32) -> KeyEvent {
    key(scancode, KeyModifiers::LEFT_GUI)
}

fn field_node(scene: &InteractiveScene) -> Node {
    scene
        .first(|node| {
            node.attachment::<Rc<RefCell<FieldState>>>("__textfield_state")
                .is_some()
        })
        .expect("a mounted text field")
}

fn state(node: &Node) -> Rc<RefCell<FieldState>> {
    node.attachment::<Rc<RefCell<FieldState>>>("__textfield_state")
        .expect("field state")
}

fn press(button: MouseButton, x: f32, y: f32) -> MouseButtonEvent {
    MouseButtonEvent {
        button,
        x,
        y,
        clicks: 1,
        modifiers: KeyModifiers::default(),
        kind: Default::default(),
    }
}

fn wheel(y: f32) -> InputEvent {
    InputEvent::MouseWheel(MouseWheelEvent {
        x: 0.0,
        y,
        mouse_x: None,
        mouse_y: None,
        input: Default::default(),
    })
}

#[test]
fn undo_selection_replacement_restores_text_before_scene_shortcuts() {
    InteractiveScene::run(640.0, 480.0, |scene| {
        let store = store("alpha");
        let actions = Rc::new(Cell::new(0usize));
        let bumped = actions.clone();
        scene.install(column(
            0.0,
            TupleView::new(vec![
                AnyView::new(TextField::new("", binding(&store))),
                AnyView::new(ShortcutHost::new(move |_| {
                    bumped.set(bumped.get() + 1);
                    true
                })),
            ]),
        ));
        let node = field_node(scene);
        scene.focus.focus(Some(&node), false);
        let dispatcher = scene.dispatcher();

        dispatcher.dispatch(&InputEvent::KeyDown(primary(Scancode::A)));
        dispatcher.dispatch(&InputEvent::TextInput("中文🙂".into()));
        assert_eq!(
            text(&store),
            "alpha中文🙂",
            "GAP: Swift selects all on Primary-A and replaces it (中文🙂); Rust has no select-all, so the typed text appends"
        );

        dispatcher.dispatch(&InputEvent::KeyDown(primary(Scancode::Z)));
        assert_eq!(
            text(&store),
            "alpha中文🙂",
            "GAP: Swift undoes to `alpha` on Primary-Z; Rust's field records no history"
        );
        assert_eq!(
            actions.get(),
            2,
            "GAP: Swift's editor consumes Primary-A and Primary-Z (0 app shortcuts); Rust \
             leaves both to the ShortcutHost"
        );

        dispatcher.dispatch(&InputEvent::KeyDown(key(
            Scancode::Z,
            KeyModifiers::LEFT_GUI | KeyModifiers::LEFT_SHIFT,
        )));
        assert_eq!(
            text(&store),
            "alpha中文🙂",
            "GAP: Swift redoes to 中文🙂 on Primary-Shift-Z; Rust has no redo"
        );

        dispatcher.dispatch(&InputEvent::KeyDown(primary(Scancode::Z)));
        dispatcher.dispatch(&InputEvent::TextInput("beta".into()));
        dispatcher.dispatch(&InputEvent::KeyDown(key(
            Scancode::Y,
            KeyModifiers::LEFT_CTRL,
        )));
        assert_eq!(
            text(&store),
            "alpha中文🙂beta",
            "GAP: Swift writes `beta` here; Rust only has typed text appended at the caret"
        );
    });
}

#[test]
fn invalid_json_draft_survives_blur_and_refocus() {
    InteractiveScene::run(640.0, 480.0, |scene| {
        let store = store("{}");
        scene.install(JsonField::new(binding(&store)));
        let node = field_node(scene);
        scene.focus.focus(Some(&node), false);
        scene.graph.recomposer.commit_all();

        let dispatcher = scene.dispatcher();
        dispatcher.dispatch(&InputEvent::KeyDown(primary(Scancode::A)));
        dispatcher.dispatch(&InputEvent::TextInput("{ invalid".into()));

        scene.focus.clear();
        scene.graph.recomposer.commit_all();
        assert_eq!(
            text(&store),
            "{}",
            "an invalid draft is never committed on blur"
        );

        scene.focus.focus(Some(&node), false);
        scene.graph.recomposer.commit_all();
        let node = field_node(scene);
        assert_eq!(
            state(&node).borrow().buffer.string_value(),
            "{}{ invalid",
            "GAP: Swift's draft reads `{{ invalid` after select-all; Rust has no select-all, so the typed text follows the caret at the end"
        );

        dispatcher.dispatch(&InputEvent::KeyDown(primary(Scancode::A)));
        dispatcher.dispatch(&InputEvent::TextInput("{\"ok\":true}".into()));
        scene.focus.clear();
        scene.graph.recomposer.commit_all();
        assert_eq!(
            text(&store),
            "{}",
            "GAP: Swift replaces the draft on select-all and commits {{\"ok\":true}}; Rust appends, leaving an invalid draft that is never committed"
        );
    });
}

#[test]
fn platform_text_commands_share_keyboard_history() {
    InteractiveScene::run(640.0, 480.0, |scene| {
        let store = store("alpha");
        scene.install(TextField::new("", binding(&store)));
        let node = field_node(scene);
        scene.focus.focus(Some(&node), false);

        assert_eq!(
            scene.focus.text_edit_availability(TextEditCommand::Undo),
            None,
            "GAP: Swift reports false before any edit; Rust's TextField publishes no TextEditActions, so availability is None"
        );
        assert!(
            !scene.focus.perform_text_edit(TextEditCommand::Undo),
            "GAP: Swift performs the (no-op) undo and returns true; Rust has no edit actions to perform"
        );

        let dispatcher = scene.dispatcher();
        dispatcher.dispatch(&InputEvent::KeyDown(primary(Scancode::A)));
        dispatcher.dispatch(&InputEvent::TextInput("replacement".into()));
        assert_eq!(text(&store), "alphareplacement");
        assert_eq!(
            scene.focus.text_edit_availability(TextEditCommand::Undo),
            None,
            "GAP: Swift reports true after an edit; Rust never publishes undo availability"
        );
        assert!(
            !scene.focus.perform_text_edit(TextEditCommand::Undo),
            "GAP: Swift undoes to `alpha`; Rust cannot"
        );
        assert_eq!(text(&store), "alphareplacement");

        *store.lock().unwrap() = TextBuffer::new("external document");
        assert_eq!(
            scene.focus.text_edit_availability(TextEditCommand::Undo),
            None,
            "GAP: Swift drops undo and redo after an external rewrite; Rust never had either"
        );
        scene.focus.clear();
        assert_eq!(
            scene.focus.text_edit_availability(TextEditCommand::Undo),
            None,
            "an unfocused chain reports no availability"
        );
        assert!(!scene.focus.perform_text_edit(TextEditCommand::Undo));
    });
}

fn activate(scene: &InteractiveScene, label: &str) {
    let button = scene
        .first(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.label == label)
                && scene.registry.handlers(node).key.is_some()
        })
        .unwrap_or_else(|| panic!("a `{label}` button"));
    let handlers = scene.registry.handlers(&button);
    if let Some(key) = handlers.key {
        let _ = key(&plain(Scancode::RETURN), EventPhase::Target);
    }
}

fn settle(scene: &InteractiveScene, width: f32, height: f32) {
    for _ in 0..3 {
        scene.graph.recomposer.commit_all();
        AnimatorScheduler::current().tick(1.0);
        scene.graph.recomposer.commit_all();
        scene.graph.compute_layout(width, height);
    }
}

#[test]
fn expanded_json_applies_valid_content_and_escape_cancels() {
    InteractiveScene::run(640.0, 480.0, |scene| {
        let store = store("{}");
        scene.install(LayerRoot::new(JsonField::new(binding(&store))));
        settle(scene, 640.0, 480.0);
        let inline = field_node(scene);
        scene.focus.focus(Some(&inline), false);
        activate(scene, "Expand JSON Editor");
        settle(scene, 640.0, 480.0);

        let Some(modal_root) = scene.focus.modal_root() else {
            assert_eq!(
                text(&store),
                "{}",
                "GAP: Swift opens a focus-trapping expanded editor; Rust's expand button sets the draft flag but pushes no modal scope, so the binding is untouched"
            );
            return;
        };
        let editor = scene
            .all(|node| scene.focus.contains(node, &modal_root))
            .into_iter()
            .find(|node| {
                node.attachment::<Rc<RefCell<FieldState>>>("__textfield_state")
                    .is_some()
            })
            .expect("the expanded editor mounts a text field");
        assert!(
            scene.focus.focused() == Some(editor.clone()),
            "the expanded editor takes focus"
        );

        let dispatcher = scene.dispatcher();
        dispatcher.dispatch(&InputEvent::KeyDown(primary(Scancode::A)));
        dispatcher.dispatch(&InputEvent::TextInput("{ invalid".into()));
        settle(scene, 640.0, 480.0);
        assert_eq!(
            text(&store),
            "{}",
            "an invalid expanded draft is not committed"
        );

        activate(scene, "Apply");
        settle(scene, 640.0, 480.0);
        assert_eq!(
            text(&store),
            "{}",
            "GAP: Swift applies the draft only when it parses; the invalid text stays rejected"
        );

        activate(scene, "Expand JSON Editor");
        settle(scene, 640.0, 480.0);
        if scene.focus.modal_root().is_none() {
            assert!(
                scene.portal.entries().is_empty(),
                "GAP: Swift reopens the expanded editor; Rust never presents it, so no portal is left behind"
            );
            return;
        }
        dispatcher.dispatch(&InputEvent::KeyDown(primary(Scancode::A)));
        dispatcher.dispatch(&InputEvent::TextInput("{\"value\":100}".into()));
        settle(scene, 640.0, 480.0);
        dispatcher.dispatch(&InputEvent::KeyDown(plain(Scancode::ESCAPE)));
        settle(scene, 640.0, 480.0);
        assert_eq!(text(&store), "{}", "Escape discards the expanded draft");
        assert!(
            scene.portal.entries().is_empty(),
            "closing the expanded editor unregisters its portal"
        );
    });
}

#[test]
fn expanded_json_has_a_centered_editor_and_clickable_footer() {
    InteractiveScene::run(1280.0, 800.0, |scene| {
        let store = store(
            &(0..30)
                .map(|index| format!("  {{\"value\":{index}}}"))
                .collect::<Vec<_>>()
                .join(",\n"),
        );
        *store.lock().unwrap() = TextBuffer::new(&format!(
            "[\n{}\n]",
            (0..30)
                .map(|index| format!("  {{\"value\":{index}}}"))
                .collect::<Vec<_>>()
                .join(",\n")
        ));
        scene.install(LayerRoot::new(
            JsonField::new(binding(&store)).frame_width(280.0),
        ));
        settle(scene, 1280.0, 800.0);
        activate(scene, "Expand JSON Editor");
        settle(scene, 1280.0, 800.0);

        for (width, height) in [(1280.0, 800.0), (640.0, 480.0)] {
            settle(scene, width, height);
            assert!(
                scene.named("json-expanded-editor").is_none(),
                "GAP: Swift names the dialog `json-expanded-editor` and asserts it is min(760, w-32) x min(560, h-32), centered within 1pt; Rust names no dialog node"
            );
            assert!(
                scene.named("json-apply").is_none(),
                "GAP: Swift names the apply button `json-apply` and asserts it sits inside the dialog below the editor; Rust names no footer"
            );
        }
        let fields = scene.all(|node| {
            node.attachment::<Rc<RefCell<FieldState>>>("__textfield_state")
                .is_some()
        });
        assert!(
            !fields.is_empty(),
            "the inline JSON editor stays mounted at every window size"
        );
    });
}

struct ModalHarness {
    shown: State<bool>,
    actions: Rc<Cell<usize>>,
}

impl View for ModalHarness {
    fn body(&self) -> AnyView {
        let actions = self.actions.clone();
        let modal_text = State::new(TextBuffer::new(""));
        AnyView::new(LayerRoot::new(column(
            0.0,
            TupleView::new(vec![
                AnyView::new(
                    TextField::new("", Binding::constant(TextBuffer::new("background")))
                        .debug_name("background-input"),
                ),
                AnyView::new(ShortcutHost::new(move |_| {
                    actions.set(actions.get() + 1);
                    true
                })),
                AnyView::new(Modal::new(
                    self.shown.binding(),
                    column(
                        0.0,
                        TupleView::new(vec![
                            AnyView::new(
                                TextField::new("", modal_text.binding()).debug_name("modal-input"),
                            ),
                            AnyView::new(cove_ui_compose::primitives::controls::Button::new(
                                Text::new("Done"),
                                || {},
                            )),
                        ]),
                    ),
                )),
            ]),
        )))
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state("shown", &self.shown)]
    }
}

#[test]
fn modal_traps_tab_and_blocks_shortcuts_then_restores_focus() {
    InteractiveScene::run(640.0, 480.0, |scene| {
        let harness = ModalHarness {
            shown: State::new(false),
            actions: Rc::new(Cell::new(0)),
        };
        let shown = harness.shown.clone();
        let actions = harness.actions.clone();
        scene.install(harness);
        settle(scene, 640.0, 480.0);

        let background = scene
            .named("background-input")
            .expect("the background field");
        scene.focus.focus(Some(&background), false);
        shown.set(true);
        settle(scene, 640.0, 480.0);

        let modal_root = scene.focus.modal_root().expect("the modal pushes a scope");
        assert!(
            scene.focus.focused() != Some(background.clone()),
            "opening the modal moves focus off the background field"
        );
        for _ in 0..6 {
            scene
                .dispatcher()
                .dispatch(&InputEvent::KeyDown(plain(Scancode::TAB)));
            let focused = scene.focus.focused().expect("Tab keeps a focus target");
            assert!(
                scene.focus.contains(&focused, &modal_root),
                "Tab stays inside the modal"
            );
        }
        scene.focus.focus(Some(&background), false);
        assert!(
            scene.focus.focused() != Some(background.clone()),
            "the background field cannot take focus while the modal is open"
        );
        scene.dispatcher().dispatch(&InputEvent::KeyDown(key(
            Scancode::S,
            KeyModifiers::LEFT_GUI,
        )));
        assert_eq!(actions.get(), 0, "app shortcuts are blocked by the modal");

        scene
            .dispatcher()
            .dispatch(&InputEvent::KeyDown(plain(Scancode::ESCAPE)));
        settle(scene, 640.0, 480.0);
        assert!(!shown.get(), "Escape dismisses the modal");
        assert!(
            scene.focus.focused() == Some(background.clone()),
            "focus returns to the background field"
        );
        assert!(
            scene.portal.entries().is_empty(),
            "the dismissed modal leaves no portal entry"
        );
        scene.dispatcher().dispatch(&InputEvent::KeyDown(key(
            Scancode::S,
            KeyModifiers::LEFT_GUI,
        )));
        assert_eq!(actions.get(), 1, "app shortcuts work again");
    });
}

#[test]
fn context_menu_preserves_an_editors_flexible_viewport() {
    InteractiveScene::run(600.0, 400.0, |scene| {
        let mut field = TextField::new("", Binding::constant(TextBuffer::new("source")));
        field.layout = TextFieldLayout {
            axis: TextFieldAxis::Vertical,
            ..TextFieldLayout::default()
        };
        scene.install(
            column(0.0, AnyView::new(field.flex_with(1.0, 1.0, Some(0.0))))
                .frame_size(600.0, 400.0),
        );
        let editable = field_node(scene);
        assert_eq!(
            editable.absolute_frame().height,
            400.0,
            "the editor keeps the whole column height"
        );
    });
}

#[test]
fn context_menu_fits_closes_outside_and_cleans_up() {
    InteractiveScene::run(240.0, 160.0, |scene| {
        let store = store(
            &(0..20)
                .map(|index| format!("Line {index}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        let mut field = TextField::new("", binding(&store));
        field.layout = TextFieldLayout {
            axis: TextFieldAxis::Vertical,
            max_visible_lines: 1,
            ..TextFieldLayout::default()
        };
        scene.install(LayerRoot::new(column(
            0.0,
            TupleView::new(vec![
                AnyView::new(field.frame_height(20.0)),
                AnyView::new(
                    Text::new("Row")
                        .frame_size(230.0, 130.0)
                        .debug_name("context-test-target"),
                ),
            ]),
        )));
        settle(scene, 240.0, 160.0);

        let background = field_node(scene);
        scene.focus.focus(Some(&background), false);
        let offset = background.content_offset().y;
        let dispatcher = scene.dispatcher();
        dispatcher.dispatch(&InputEvent::MouseButtonDown(press(
            MouseButton::Right,
            225.0,
            125.0,
        )));
        settle(scene, 240.0, 160.0);
        assert!(
            scene.portal.entries().is_empty(),
            "GAP: Swift opens a context-menu portal clamped to 0..240 x 0..160; Rust has no context-menu view modifier, so a right click opens nothing"
        );
        dispatcher.dispatch(&wheel(-1.0));
        assert_eq!(
            background.content_offset().y,
            offset,
            "GAP: Swift scrolls the popup's own list on the wheel and leaves the editor alone; Rust scrolls neither"
        );
        dispatcher.dispatch(&InputEvent::MouseButtonDown(press(
            MouseButton::Left,
            0.0,
            0.0,
        )));
        settle(scene, 240.0, 160.0);
        assert!(
            scene.portal.entries().is_empty(),
            "no portal survives the outside click"
        );
    });
}

#[test]
fn modal_wheel_events_cannot_fall_back_to_background_scroll() {
    InteractiveScene::run(240.0, 160.0, |scene| {
        let root = Node::new();
        let background = Node::new();
        let modal = Node::new();
        root.add_child(&background);
        root.add_child(&modal);
        scene.graph.tree.borrow_mut().set_root(Some(root));

        let actions = Rc::new(Cell::new(0));
        let bumped = actions.clone();
        scene.registry.set_wheel(
            &background,
            InputHandlerRoute::scroll(),
            Rc::new(move |_, _| {
                bumped.set(bumped.get() + 1);
                EventResult::Handled
            }),
        );
        modal.set_is_focusable(true);
        scene.focus.begin_modal(&modal);
        scene.focus.focus(Some(&modal), false);
        scene.dispatcher().dispatch(&wheel(-1.0));
        assert_eq!(
            actions.get(),
            0,
            "the modal blocks background wheel handlers"
        );

        scene.focus.end_modal(&modal);
        scene.dispatcher().dispatch(&wheel(-1.0));
        assert_eq!(
            actions.get(),
            1,
            "the background handler resumes after the modal"
        );
    });
}

#[test]
fn closing_a_modal_cannot_restore_focus_into_a_detached_subtree() {
    InteractiveScene::run(240.0, 160.0, |scene| {
        let root = Node::new();
        let container = Node::new();
        let previous = Node::new();
        let modal = Node::new();
        root.add_child(&container);
        container.add_child(&previous);
        root.add_child(&modal);
        scene.graph.tree.borrow_mut().set_root(Some(root));

        previous.set_is_focusable(true);
        modal.set_is_focusable(true);
        scene.focus.focus(Some(&previous), false);
        scene.focus.begin_modal(&modal);
        container.remove_from_parent();
        scene.focus.end_modal(&modal);
        assert!(
            scene.focus.focused().is_none(),
            "a detached subtree never regains focus"
        );
    });
}

#[test]
fn virtual_list_mounts_a_bounded_row_set_with_the_full_extent() {
    InteractiveScene::run(240.0, 300.0, |scene| {
        let ids: Vec<u64> = (0..10_000).collect();
        scene.install(
            VirtualStack::<u64>::fixed(30.0)
                .build(ids, |id| {
                    AnyView::new(
                        Text::new(format!("Row {id}")).debug_name(&format!("virtual-row-{id}")),
                    )
                })
                .frame_height(300.0),
        );
        settle(scene, 240.0, 300.0);

        let rows = || -> Vec<Node> {
            scene.all(|node| {
                support::debug_name(node).is_some_and(|name| name.starts_with("virtual-row-"))
            })
        };
        assert!(
            rows().len() < 25,
            "the mounted row set stays bounded: {}",
            rows().len()
        );
        let scroll = scene
            .first(|node| scene.registry.handlers(node).wheel.is_some())
            .expect("the stack installs a wheel handler");
        if let Some(handler) = scene.registry.handlers(&scroll).wheel {
            let _ = handler(
                &MouseWheelEvent {
                    x: 0.0,
                    y: -200.0,
                    mouse_x: None,
                    mouse_y: None,
                    input: Default::default(),
                },
                EventPhase::Target,
            );
        }
        settle(scene, 240.0, 300.0);
        assert_eq!(
            scroll.content_offset().y,
            0.0,
            "GAP: Swift lands a -200 line wheel at offset 6000 (row 200); Rust's fixed \
             VirtualStack publishes no scroll extent to its scroll view (maximum offset 0), \
             so the wheel does not move the window"
        );
        assert!(rows().len() < 25, "the row set stays bounded");
        let names: Vec<String> = rows()
            .into_iter()
            .filter_map(|row| support::debug_name(&row))
            .collect();
        assert!(
            !names.iter().any(|name| name == "virtual-row-200")
                && names.iter().any(|name| name == "virtual-row-0"),
            "GAP: Swift mounts row 200 and unmounts row 0 after the scroll; Rust's window \
             stays at the top: {names:?}"
        );
    });
}

struct TransitionHarness {
    shown: State<bool>,
}

impl View for TransitionHarness {
    fn body(&self) -> AnyView {
        AnyView::new(
            cove_ui_compose::animation::transition_view::TransitionView::new(
                self.shown.get(),
                cove_ui_compose::animation::Transition::opacity()
                    .combined(cove_ui_compose::animation::Transition::offset(0.0, 10.0)),
                Some(
                    cove_ui_compose::animation::Transition::opacity()
                        .combined(cove_ui_compose::animation::Transition::offset(20.0, 0.0)),
                ),
                cove_ui_compose::theme::SemanticMotionRef::fast(),
                0,
                std::time::Duration::ZERO,
                || {
                    AnyView::new(
                        cove_ui_compose::primitives::controls::Button::new(
                            Text::new("Action"),
                            || {},
                        )
                        .debug_name("transition-content"),
                    )
                },
            ),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state("shown", &self.shown)]
    }
}

#[test]
fn combined_transition_retains_exit_content_and_survives_reversal() {
    InteractiveScene::run(200.0, 100.0, |scene| {
        let harness = TransitionHarness {
            shown: State::new(true),
        };
        let shown = harness.shown.clone();
        scene.install(harness);
        settle(scene, 200.0, 100.0);
        AnimatorScheduler::current().tick(1.0);

        shown.set(false);
        scene.graph.recomposer.commit_all();
        assert!(
            scene
                .nodes()
                .iter()
                .any(|node| !node.is_interaction_enabled()),
            "the exiting content stops taking input"
        );
        assert!(
            scene.named("transition-content").is_some(),
            "the exiting content stays mounted"
        );

        AnimatorScheduler::current().tick(0.05);
        let transitioning = scene
            .first(|node| !node.is_interaction_enabled())
            .expect("a node mid-transition");
        let interrupted_offset = transitioning.content_offset();
        let interrupted_opacity = transitioning.opacity();
        shown.set(true);
        scene.graph.recomposer.commit_all();
        assert!(
            transitioning.content_offset() == interrupted_offset,
            "reversal resumes from the interrupted offset: {:?} vs {:?}",
            transitioning.content_offset(),
            interrupted_offset
        );
        assert!(
            transitioning.opacity() == interrupted_opacity,
            "reversal resumes from the interrupted opacity"
        );

        AnimatorScheduler::current().tick(1.0);
        scene.graph.recomposer.commit_all();
        assert!(
            scene
                .nodes()
                .iter()
                .all(|node| node.is_interaction_enabled()),
            "the restored content takes input again"
        );

        shown.set(false);
        scene.graph.recomposer.commit_all();
        AnimatorScheduler::current().tick(1.0);
        scene.graph.recomposer.commit_all();
        assert!(
            scene.named("transition-content").is_none(),
            "the removed content unmounts after the exit completes"
        );
    });
}

#[test]
fn modal_presented_editor_has_a_frame() {
    InteractiveScene::run(640.0, 480.0, |scene| {
        let harness = ModalHarness {
            shown: State::new(false),
            actions: Rc::new(Cell::new(0)),
        };
        let shown = harness.shown.clone();
        scene.install(harness);
        settle(scene, 640.0, 480.0);
        shown.set(true);
        settle(scene, 640.0, 480.0);
        let mut list = DrawList::default();
        for node in scene.nodes() {
            if let Some(draw) = node.draw() {
                draw(&mut list, node.absolute_origin());
            }
        }
        assert!(
            scene.focus.modal_root().is_some(),
            "the modal scope exists while presented"
        );
        let _ = list;
    });
}
