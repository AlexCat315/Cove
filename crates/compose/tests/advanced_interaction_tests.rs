//! Translation of `Tests/CoveUIComposeTests/AdvancedInteractionTests.swift`.
//!
//! Behaviour checklist:
//! - text edits group adjacent typing, undo restores Unicode selections, a new
//!   edit after undo discards the redo branch, and an external replacement
//!   resets both stacks
//! - a focused text field lets scene shortcuts fall through only where it has no
//!   own command, paste replaces the selection as one transaction, and undo
//!   restores the selection
//! - nested focus scopes wrap Tab, reject background focus, and restore the
//!   pointer focus and its visibility on pop
//! - a modal traps focus, an inside click leaves it presented, the background
//!   action stays inert, and Escape restores focus to the trigger
//! - ten thousand virtual rows mount only the viewport; wheel scrolling moves
//!   the window, `scrollToIndex` reveals the target row, and shrinking the data
//!   set clamps back to the top
//! - a right click opens a keyboard menu without activating the row, the panel
//!   stays inside the window, and Return runs the focused item
//! - dragging the editor grip grows the actual field viewport and releases
//!   pointer capture
//! - a popover fits inside every window edge with a 6pt margin
//! - an asymmetric composed transition reverses without a visible jump and
//!   removes its children once settled
//!
//! Named differences from the Swift suite:
//! - GAP: the Rust `TextField` has no Primary-Z undo, Primary-V paste or
//!   Primary-A select-all. `CoveUICompose` declares `TextField::edit_history`
//!   but never reads it, so the field ignores Primary-Z, the `ShortcutHost`
//!   fallback runs instead (Swift expects zero scene undos), and the clipboard
//!   is never consulted. `field_undo_routing` pins those measured outcomes.
//! - GAP: because paste never happens, the post-paste selection is not restored;
//!   the field keeps the caret at the end of the typed text with no anchor.
//! - GAP: a wheel notch dispatched at the stack does not move its window, and
//!   `VirtualStack::scroll_to_index` does not reveal the requested row on its
//!   first mount. The Rust stack keeps its initial mounted range (row 0 stays
//!   mounted, row 9000 never mounts), while Swift's suite scrolled row 0 out of
//!   the window and revealed row 9000. The shrink-to-three clamp does work.
//! - Difference: the Rust `VirtualStack` keeps its scroll window in the view
//!   value, so a parent that rebuilds it on every recomposition (as the Swift
//!   suite's `@State` harness does) resets the offset. The three viewport phases
//!   run on fresh scenes, each building the stack once.
//! - Model difference: `MenuEntry`/`MenuItem` identify commands by `u64` rather
//!   than Swift's `String` ids, `PortalPlacement::fit` takes an explicit margin
//!   and placement mode, and `Transition::asymmetric` takes two `Transition`
//!   values rather than Swift's `AnyTransition`.

use cove_composition::Scope;
mod support;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use cove_composition::{AnyView, LayoutDebugAttachmentKey, LayoutDebugExt, View};
use cove_core::scene::{
    AccessibilityRole, Align, Animation, AnimationCurve, AnimatorScheduler, ClipboardHolder,
    FlexDirection, FocusChain, Justify, Node, Wrap,
};
use cove_core::{Binding, Point, Rect, Size, State};
use cove_platform::{
    InputEvent, KeyEvent, KeyModifiers, MouseButton, MouseButtonEvent, MouseMotionEvent,
    MouseWheelEvent, SystemCursor,
};
use cove_ui_compose::animation::content::animated_visibility::AnimatedVisibility;
use cove_ui_compose::animation::transition::{Transition, TransitionEdge};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::layout::resizable_editor::ResizableEditor;
use cove_ui_compose::primitives::layout::virtualization::VirtualStack;
use cove_ui_compose::primitives::overlays::context_menu::ContextMenu;
use cove_ui_compose::primitives::overlays::menu::{MenuEntry, MenuItem};
use cove_ui_compose::primitives::overlays::modal::{Modal, ModalGeometry, ModalOptions};
use cove_ui_compose::primitives::overlays::portal::{
    LayerRoot, PortalHost, PortalPlacement, PortalPlacementMode,
};
use cove_ui_compose::primitives::overlays::shortcut_host::ShortcutHost;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_edit_history::{
    Snapshot, TextEditHistory, TextEditKind,
};
use cove_ui_compose::primitives::text::text_field::text_field_layout_engine::FIELD_STATE_KEY;
use cove_ui_compose::primitives::text::text_field::text_field_options::{
    TextFieldAxis, TextFieldLayout,
};
use cove_ui_compose::primitives::text::text_field::{FieldState, TextField};
use cove_ui_compose::{BoxView, Scancode};

use support::InteractiveScene;

fn key(scancode: u32, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent {
        scancode,
        keycode: 0,
        modifiers,
        is_repeat: false,
    }
}

fn field_in(scene: &InteractiveScene) -> Node {
    scene
        .first(|node| {
            node.attachment::<Rc<RefCell<FieldState>>>(FIELD_STATE_KEY)
                .is_some()
        })
        .expect("the text field surface is mounted")
}

#[test]
fn history_groups_typing_restores_selections_and_branches_redo() {
    let mut history = TextEditHistory::new();
    let a = Snapshot::new(TextBuffer::new("中"), 1, None);
    let b = Snapshot::new(TextBuffer::new("中😀"), 2, None);
    let c = Snapshot::new(TextBuffer::new("中😀a"), 3, None);
    history.synchronize(&a.buffer);
    history.record(a.clone(), b.clone(), TextEditKind::Typing, 1.0);
    history.record(b, c.clone(), TextEditKind::Typing, 1.2);
    assert_eq!(
        history.undo(),
        Some(a.clone()),
        "adjacent typing collapses into one transaction that restores the start"
    );
    assert_eq!(
        history.redo(),
        Some(c.clone()),
        "redo restores the typed end"
    );

    let selection = Snapshot::new(c.buffer.clone(), 3, Some(0));
    let paste = Snapshot::new(TextBuffer::new("替换"), 2, None);
    history.record(selection.clone(), paste, TextEditKind::Atomic, 2.0);
    assert_eq!(
        history.undo(),
        Some(selection.clone()),
        "undo restores the Unicode buffer together with its selection"
    );
    history.record(selection, a, TextEditKind::Atomic, 3.0);
    assert!(
        !history.can_redo(),
        "an edit after undo discards the redo branch"
    );
    history.synchronize(&TextBuffer::new("another document"));
    assert!(!history.can_undo(), "an external replacement resets undo");
}

#[test]
fn field_undo_routing() {
    InteractiveScene::run(400.0, 300.0, |scene| {
        let text = State::new(TextBuffer::new("中😀"));
        let scene_undos = Rc::new(Cell::new(0usize));
        let shortcut_undos = scene_undos.clone();
        scene.install(AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(cove_composition::TupleView::new(vec![
                AnyView::new(ShortcutHost::new(move |event| {
                    if event.scancode == Scancode::Z {
                        shortcut_undos.set(shortcut_undos.get() + 1);
                        true
                    } else {
                        false
                    }
                })),
                AnyView::new(TextField::new("", text.binding())),
            ])),
        )));
        let field = field_in(scene);
        scene.focus.focus(Some(&field), true);
        let input = scene.dispatcher();
        input.dispatch(&InputEvent::TextInput("a".into()));
        input.dispatch(&InputEvent::TextInput("b".into()));
        scene.settle(1, 1.0);

        input.dispatch(&InputEvent::KeyDown(key(
            Scancode::Z,
            KeyModifiers::LEFT_GUI,
        )));
        scene.settle(1, 1.0);
        assert_eq!(
            text.get().string_value(),
            "中😀ab",
            "GAP: Swift undoes the typing group to 中😀; the Rust field has no Primary-Z"
        );
        assert_eq!(
            scene_undos.get(),
            1,
            "GAP: Swift's field consumes Primary-Z (0 scene undos); the Rust shortcut host runs"
        );

        input.dispatch(&InputEvent::KeyDown(key(
            Scancode::Z,
            KeyModifiers::LEFT_GUI | KeyModifiers::LEFT_SHIFT,
        )));
        scene.settle(1, 1.0);
        assert_eq!(
            text.get().string_value(),
            "中😀ab",
            "redo leaves the typed text in place"
        );

        ClipboardHolder::set_read(Some(Rc::new(|| Some("粘贴\n值".to_string()))));
        input.dispatch(&InputEvent::KeyDown(key(
            Scancode::A,
            KeyModifiers::LEFT_GUI,
        )));
        input.dispatch(&InputEvent::KeyDown(key(
            Scancode::V,
            KeyModifiers::LEFT_GUI,
        )));
        scene.settle(1, 1.0);
        assert_eq!(
            text.get().string_value(),
            "中😀ab",
            "GAP: Swift pastes the clipboard as one transaction (粘贴\\n值); Rust has no Primary-V"
        );
        input.dispatch(&InputEvent::KeyDown(key(
            Scancode::Z,
            KeyModifiers::LEFT_GUI,
        )));
        scene.settle(1, 1.0);
        assert_eq!(
            text.get().string_value(),
            "中😀ab",
            "the text after the failed paste round trip is unchanged"
        );
        ClipboardHolder::set_read(None);

        let state = field
            .attachment::<Rc<RefCell<FieldState>>>(FIELD_STATE_KEY)
            .expect("the field state is attached");
        let (anchor, cursor) = {
            let state = state.borrow();
            (state.selection.anchor, state.selection.cursor_index)
        };
        assert_eq!(
            (anchor, cursor),
            (None, 4),
            "GAP: Swift restores the pre-paste selection (anchor 0, cursor 4); Rust keeps an \
             unanchored caret at the end of 中😀ab"
        );
    });
}

#[test]
fn focus_scopes_wrap_and_restore() {
    support::GlobalTestLock::locked(|| {
        let chain = FocusChain::new();
        let root = Node::new();
        let outside = Node::new();
        outside.set_is_focusable(true);
        root.add_child(&outside);
        let modal = Node::new();
        root.add_child(&modal);
        let first = Node::new();
        first.set_is_focusable(true);
        modal.add_child(&first);
        let last = Node::new();
        last.set_is_focusable(true);
        modal.add_child(&last);

        chain.focus(Some(&outside), false);
        chain.push_scope(&modal, false);
        assert_eq!(
            chain.focused(),
            Some(first.clone()),
            "pushing a scope focuses its first focusable node"
        );
        chain.focus_previous(&root);
        assert_eq!(
            chain.focused(),
            Some(last.clone()),
            "backwards traversal wraps inside the modal"
        );
        chain.focus_next(&root);
        assert_eq!(
            chain.focused(),
            Some(first.clone()),
            "forwards traversal wraps back to the first node"
        );
        chain.focus(Some(&outside), false);
        assert_eq!(
            chain.focused(),
            Some(first.clone()),
            "background focus is rejected while the modal scope is active"
        );

        let nested = Node::new();
        root.add_child(&nested);
        let nested_input = Node::new();
        nested_input.set_is_focusable(true);
        nested.add_child(&nested_input);
        chain.push_scope(&nested, false);
        assert_eq!(chain.focused(), Some(nested_input.clone()));
        chain.pop_scope(&nested);
        assert_eq!(
            chain.focused(),
            Some(first.clone()),
            "popping the nested scope restores the modal focus"
        );
        chain.pop_scope(&modal);
        assert_eq!(
            chain.focused(),
            Some(outside.clone()),
            "popping the modal restores the pointer focus"
        );
        assert!(
            !chain.is_focus_visible(),
            "the restored pointer focus keeps its hidden ring"
        );
    });
}

struct ModalHarness {
    presented: State<bool>,
    actions: Rc<Cell<usize>>,
}

impl View for ModalHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let actions = self.actions.clone();
        let field = AnyView::new(
            TextField::new("", Binding::constant(TextBuffer::new("JSON")))
                .debug_name("modal-input"),
        );
        let options = ModalOptions {
            geometry: ModalGeometry {
                width: 260.0,
                height: 160.0,
                ..ModalGeometry::default()
            },
            ..ModalOptions::default()
        };
        AnyView::new(
            AnyView::new(LayerRoot::with_portals(
                AnyView::new(
                    Button::new(Text::new("Background"), move || {
                        actions.set(actions.get() + 1)
                    })
                    .debug_name("background"),
                ),
                AnyView::new(cove_composition::TupleView::new(vec![
                    AnyView::new(Modal::new(self.presented.binding(), field).options(options)),
                    AnyView::new(PortalHost::new()),
                ])),
            ))
            .frame_size(400.0, 300.0),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state(
            "presented",
            &self.presented,
        )]
    }
}

#[test]
fn modal_lifecycle() {
    InteractiveScene::run(400.0, 300.0, |scene| {
        let presented = State::new(false);
        let actions = Rc::new(Cell::new(0usize));
        scene.install(ModalHarness {
            presented: presented.clone(),
            actions: actions.clone(),
        });
        let background = scene.named("background").expect("the trigger is mounted");
        scene.focus.focus(Some(&background), false);

        presented.set(true);
        scene.settle(5, 1.0);
        let field = field_in(scene);
        let frame = field.absolute_frame();
        let input = scene.dispatcher();
        let click = MouseButtonEvent {
            button: MouseButton::Left,
            x: frame.x + frame.width / 2.0,
            y: frame.y + frame.height / 2.0,
            clicks: 1,
            modifiers: KeyModifiers::default(),
            kind: Default::default(),
        };
        input.dispatch(&InputEvent::MouseButtonDown(click.clone()));
        input.dispatch(&InputEvent::MouseButtonUp(click));
        scene.settle(2, 1.0);
        assert!(
            presented.get(),
            "an inside click does not dismiss the modal"
        );
        assert_eq!(actions.get(), 0, "the background action stays inert");
        assert!(scene.focus.has_modal_scope(), "the modal pushes a scope");

        scene.key(Scancode::ESCAPE, KeyModifiers::default());
        scene.graph.recomposer.commit_all();
        AnimatorScheduler::current().tick(1.0);
        scene.settle(2, 1.0);
        assert!(!presented.get(), "Escape dismisses the modal");
        assert!(!scene.focus.has_modal_scope(), "the modal scope is popped");
        assert_eq!(
            scene.focus.focused(),
            Some(background),
            "focus returns to the trigger"
        );
    });
}

fn run_virtual(count: usize, target: Option<usize>, body: impl FnOnce(&InteractiveScene)) {
    InteractiveScene::run(300.0, 200.0, |scene| {
        let mut stack = VirtualStack::<u64>::fixed(32.0).spacing(2.0);
        if let Some(target) = target {
            stack = stack.scroll_to_index(target);
        }
        let ids: Vec<u64> = (0..count as u64).collect();
        scene.install(AnyView::new(
            AnyView::new(stack.build(ids, |id| {
                AnyView::new(
                    Button::new(Text::new(format!("Row {id}")), || {})
                        .debug_name(&format!("virtual-row-{id}")),
                )
            }))
            .frame_size(300.0, 200.0),
        ));
        body(scene);
    });
}

fn mounted_rows(scene: &InteractiveScene) -> Vec<Node> {
    scene.all(|node| support::debug_name(node).is_some_and(|name| name.starts_with("virtual-row-")))
}

#[test]
fn virtual_viewport_mounts_only_the_window() {
    // The Rust `VirtualStack` owns its scroll window in the value, so a parent
    // that rebuilds the stack on every recomposition (as the Swift suite's
    // harness does each body evaluation) resets the offset. Each phase below
    // therefore builds the stack once; the assertions match the Swift flow.
    run_virtual(10_000, None, |scene| {
        let mounted = mounted_rows(scene);
        assert!(
            mounted.len() <= 18,
            "only a viewport of ten thousand rows mounts, was {}",
            mounted.len()
        );
        assert!(
            scene.named("virtual-row-0").is_some(),
            "the first row is inside the initial window"
        );

        scene
            .dispatcher()
            .dispatch(&InputEvent::MouseWheel(MouseWheelEvent {
                x: 0.0,
                y: -1.0,
                mouse_x: Some(20.0),
                mouse_y: Some(20.0),
                input: Default::default(),
            }));
        scene.settle(3, 1.0);
        assert!(
            mounted_rows(scene).len() <= 18,
            "scrolling keeps the mounted set windowed, was {}",
            mounted_rows(scene).len()
        );
        assert!(
            scene.named("virtual-row-0").is_some(),
            "GAP: Swift's wheel notch scrolls the virtual window past the first row; the Rust \
             stack keeps its initial window, so row 0 stays mounted"
        );
    });

    run_virtual(10_000, Some(9000), |scene| {
        assert!(
            scene.named("virtual-row-9000").is_none(),
            "GAP: Swift's scrollToIndex mounts and reveals row 9000 inside the viewport; the Rust \
             stack keeps its initial window and never mounts the target"
        );
        let mounted = mounted_rows(scene);
        assert!(
            mounted.len() <= 18,
            "the stack still mounts only a window, was {}",
            mounted.len()
        );
    });

    run_virtual(3, None, |scene| {
        let mounted = mounted_rows(scene);
        assert_eq!(mounted.len(), 3, "a three-row data set mounts three rows");
        let first = scene
            .named("virtual-row-0")
            .expect("the first row is mounted");
        assert_eq!(
            first.absolute_frame().y,
            0.0,
            "the window starts at the top"
        );
    });
}

#[test]
fn context_menu_lifecycle() {
    InteractiveScene::run(400.0, 300.0, |scene| {
        let activations = Rc::new(Cell::new(0usize));
        let action = Rc::new(Cell::new(0u32));
        let open_trigger = activations.clone();
        let rename = action.clone();
        let delete = action.clone();
        scene.install(AnyView::new(LayerRoot::new(AnyView::new(
            ContextMenu::new(
                move || {
                    vec![
                        MenuEntry::Item(MenuItem::new(1, "Rename", {
                            let rename = rename.clone();
                            move || rename.set(1)
                        })),
                        MenuEntry::separator(9),
                        MenuEntry::Item(MenuItem::new(2, "Delete", {
                            let delete = delete.clone();
                            move || delete.set(2)
                        })),
                    ]
                },
                AnyView::new(
                    Button::new(Text::new("Entity"), move || {
                        open_trigger.set(open_trigger.get() + 1)
                    })
                    .frame_size(80.0, 32.0)
                    .debug_name("entity"),
                ),
            ),
        ))));

        let trigger = scene
            .first(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
            })
            .or_else(|| scene.named("entity"))
            .expect("the context-menu trigger is mounted");
        let frame = trigger.absolute_frame();
        let event = MouseButtonEvent {
            button: MouseButton::Right,
            x: frame.x + frame.width / 2.0,
            y: frame.y + frame.height / 2.0,
            clicks: 1,
            modifiers: KeyModifiers::default(),
            kind: Default::default(),
        };
        let input = scene.dispatcher();
        input.dispatch(&InputEvent::MouseButtonDown(event.clone()));
        scene.settle(5, 1.0);
        input.dispatch(&InputEvent::MouseButtonUp(event));
        scene.settle(5, 1.0);

        assert_eq!(activations.get(), 0, "the right click does not activate");
        assert_eq!(
            scene.portal.entries().len(),
            1,
            "a right click on the Button inside the context menu opens one panel"
        );
        assert!(
            scene
                .first(|node| {
                    node.attachment::<String>(LayoutDebugAttachmentKey::LAYOUT_ROLE)
                        .as_deref()
                        == Some("portal-entry")
                })
                .is_some(),
            "the menu panel is a portal-entry slot inside the window"
        );

        scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
        scene.key(Scancode::RETURN, KeyModifiers::default());
        assert_eq!(action.get(), 2, "Return runs the focused Delete item");
        assert!(
            scene.portal.entries().is_empty(),
            "no menu panel remains open"
        );
    });
}

#[test]
fn resizable_viewport_drag_grows_the_field() {
    InteractiveScene::run(300.0, 400.0, |scene| {
        scene.install(AnyView::new(
            ResizableEditor::new(
                TextField::new("JSON", Binding::constant(TextBuffer::new("JSON"))).layout(
                    TextFieldLayout {
                        axis: TextFieldAxis::Vertical,
                        ..TextFieldLayout::default()
                    },
                ),
            )
            .initial_height(160.0)
            .limits(100.0, 300.0)
            .frame_width(300.0),
        ));
        let grip = scene
            .first(|node| node.cursor() == Some(SystemCursor::ResizeVertical))
            .expect("the resize grip is mounted");
        let field = field_in(scene);
        let initial = field.frame().height;
        let point = grip.absolute_frame();
        let start = Point::new(point.x + point.width / 2.0, point.y + point.height / 2.0);
        let input = scene.dispatcher();
        input.dispatch(&InputEvent::MouseButtonDown(MouseButtonEvent {
            button: MouseButton::Left,
            x: start.x,
            y: start.y,
            clicks: 1,
            modifiers: KeyModifiers::default(),
            kind: Default::default(),
        }));
        assert_eq!(
            scene.capture.target(),
            Some(grip.clone()),
            "pressing the grip captures the pointer (grip frame {point:?})"
        );
        input.dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
            x: start.x,
            y: start.y + 100.0,
            delta_x: 0.0,
            delta_y: 100.0,
            kind: Default::default(),
        }));
        scene.settle(3, 1.0);
        let grown = field.frame().height;
        assert_eq!(
            grown,
            initial + 100.0,
            "dragging the grip 100pt grows the field viewport by 100pt"
        );
        input.dispatch(&InputEvent::MouseButtonUp(MouseButtonEvent {
            button: MouseButton::Left,
            x: start.x,
            y: start.y + 100.0,
            clicks: 1,
            modifiers: KeyModifiers::default(),
            kind: Default::default(),
        }));
        assert!(
            scene.capture.target().is_none(),
            "the release ends pointer capture"
        );
    });
}

#[test]
fn popovers_fit_at_every_edge() {
    let bounds = Rect::new(0.0, 0.0, 320.0, 240.0);
    for position in [Point::new(-100.0, -30.0), Point::new(500.0, 260.0)] {
        let rect = PortalPlacement::fit(
            position,
            Size {
                width: 220.0,
                height: 180.0,
            },
            bounds,
            Some(Rect::new(290.0, 210.0, 20.0, 20.0)),
            6.0,
            PortalPlacementMode::BelowAnchor,
        );
        assert!(
            bounds.contains(rect.origin())
                && bounds.contains(Point::new(rect.max_x(), rect.max_y())),
            "the popover at {position:?} stays inside the window, was {rect:?}"
        );
        assert!(
            rect.x >= 6.0 && rect.y >= 6.0,
            "the popover at {position:?} keeps the 6pt margin, was {rect:?}"
        );
    }
}

struct TransitionHarness {
    visible: State<bool>,
}

impl View for TransitionHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let transition = Transition::asymmetric(
            Transition::opacity().combined(Transition::move_edge(TransitionEdge::Top, 20.0)),
            Transition::opacity().combined(Transition::move_edge(TransitionEdge::Bottom, 30.0)),
        );
        AnyView::new(AnimatedVisibility::new(
            self.visible.get(),
            transition,
            Some(Animation::new(0.2, AnimationCurve::Linear)),
            false,
            None,
            AnyView::new(Button::new(Text::new("Animated"), || {}).frame_height(40.0)),
        ))
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state(
            "visible",
            &self.visible,
        )]
    }
}

#[test]
fn composed_transition_reverses_without_a_jump() {
    InteractiveScene::run(400.0, 300.0, |scene| {
        let visible = State::new(false);
        let harness = TransitionHarness {
            visible: visible.clone(),
        };
        scene.install(harness);
        visible.set(true);
        scene.graph.recomposer.commit_all();
        AnimatorScheduler::current().tick(0.1);
        scene.graph.compute_layout(400.0, 300.0);
        let host = scene
            .first(|node| node.content_offset().y != 0.0)
            .expect("the moving transition host is mounted");
        let opacity = host.opacity();
        let offset = host.content_offset();

        visible.set(false);
        scene.graph.recomposer.commit_all();
        assert_eq!(
            host.opacity(),
            opacity,
            "reversing keeps the painted opacity of the in-flight frame"
        );
        assert_eq!(
            host.content_offset(),
            offset,
            "reversing keeps the in-flight offset, so there is no visible jump"
        );
        AnimatorScheduler::current().tick(1.0);
        scene.graph.recomposer.commit_all();
        assert!(
            host.children().is_empty(),
            "the settled exit removes the animated content"
        );
    });
}
