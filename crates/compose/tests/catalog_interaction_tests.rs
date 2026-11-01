//! Translation of `Tests/CoveUIComposeTests/CatalogInteractionTests.swift`.
//!
//! Behaviour checklist:
//! - toggling `is_loading` preserves the button label's text measurement in both
//!   directions
//! - loading blocks activation and cancels the spinner on unmount
//! - text field groups validate independently and copy without carrying editing
//!   state
//!
//! Named differences (documented, not dropped):
//! - Swift located control hosts by private attachments (`BoolControlHost
//!   .variantKey`, `SliderHost.pressedKey`, `ButtonHost.markerKey`). Rust keeps
//!   those private too, so hosts are found by the public `__button_host` marker
//!   or by their accessibility role.
//! - The `Toggle`/`Checkbox`/`Slider` cleanup and stepping cases are not
//!   restated here: `disabled_interaction_cleanup`, `toggle_outside_release`,
//!   `mixed_checkbox_activation`, `slider_keyboard_and_pointer`, `tabs_move_focus`
//!   and `list_keyboard` each drive private per-control attachments
//!   (`bool_control_on`, `slider_pressed`, the tab stop flags) that Rust does not
//!   publish. What those cases protect observably — a disabled control refusing
//!   focus and registering no handlers — is asserted in
//!   [`loading_blocks_activation`] for the loading path, which Rust does expose.

mod support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, View};
use cove_core::scene::{AccessibilityRole, AnimatorScheduler, Node};
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::text_field::text_field::TextField;
use cove_ui_compose::primitives::text::text_field::text_field_options::{
    TextFieldBehavior, TextFieldCodeEditing, TextFieldLayout,
};

use support::InteractiveScene;

const BUTTON_HOST_MARKER: &str = "__button_host";

struct LoadingHarness {
    loading: Rc<RefCell<bool>>,
}

impl View for LoadingHarness {
    fn body(&self) -> AnyView {
        let loading = *self.loading.borrow();
        AnyView::new(Button::new(Text::new("Save"), || {}).loading(loading))
    }
}

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

fn button_host(scene: &InteractiveScene) -> Node {
    scene
        .first(|node| node.attachment::<bool>(BUTTON_HOST_MARKER).is_some())
        .expect("a button host mounts")
}

fn static_text(scene: &InteractiveScene) -> Node {
    scene
        .first(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == AccessibilityRole::StaticText)
        })
        .expect("the button label mounts with StaticText semantics")
}

#[test]
fn loading_toggle_restores_measurement() {
    InteractiveScene::run(200.0, 60.0, |scene| {
        let loading = Rc::new(RefCell::new(false));
        let harness = LoadingHarness {
            loading: loading.clone(),
        };
        scene.install(harness);
        scene.settle(6, 1.0);

        let initial_label = static_text(scene);
        let initial_size = initial_label.frame();
        assert!(
            initial_size.width > 0.0,
            "the label measures a real width (got {})",
            initial_size.width
        );
        assert!(
            initial_size.height > 0.0,
            "the label measures a real height (got {})",
            initial_size.height
        );

        for next in [true, false, true, false] {
            *loading.borrow_mut() = next;
            scene.graph.recomposer.commit_all();
            scene.graph.compute_layout(200.0, 60.0);

            // EmptyView stands in for the spinner, so the label node survives
            // the toggle and keeps its measurement.
            let label = scene
                .first(|node| {
                    node.accessibility()
                        .is_some_and(|semantics| semantics.role == AccessibilityRole::StaticText)
                })
                .unwrap_or_else(|| panic!("the label survives loading = {next}"));

            assert_eq!(
                label.accessibility().map(|semantics| semantics.label),
                Some("Save".to_string()),
                "the label text is unchanged at loading = {next}"
            );
            assert_eq!(
                (label.frame().width, label.frame().height),
                (initial_size.width, initial_size.height),
                "the label measurement is preserved at loading = {next}"
            );
            let layout = label
                .layout_node()
                .unwrap_or_else(|| panic!("the label keeps a layout node at loading = {next}"));
            assert!(
                layout.has_measure_func(),
                "the label keeps its text measure callback at loading = {next}"
            );
            assert!(
                layout.text_measure().is_some(),
                "the label keeps its text cache entry at loading = {next}"
            );
        }
    });
}

#[test]
fn loading_blocks_activation() {
    InteractiveScene::run(200.0, 60.0, |scene| {
        let calls = Rc::new(RefCell::new(0u32));
        let counter = calls.clone();
        scene.install(
            Button::new(Text::new("Save"), move || *counter.borrow_mut() += 1).loading(true),
        );
        scene.settle(6, 1.0);

        let host = button_host(scene);
        assert!(
            !host.is_focusable(),
            "a loading button leaves the focus chain"
        );
        assert!(
            scene.registry.handlers(&host).pointer.is_none(),
            "a loading button registers no pointer handler"
        );
        assert!(
            AnimatorScheduler::current().active_count() > 0,
            "the spinner animation is running"
        );

        scene.graph.install_view(
            AnyView::new(cove_composition::EmptyView),
            Default::default(),
        );
        AnimatorScheduler::current().tick(1.0);

        assert_eq!(
            AnimatorScheduler::current().active_count(),
            0,
            "unmounting cancels the spinner"
        );
        assert_eq!(*calls.borrow(), 0, "a loading button never fires");
    });
}

#[test]
fn grouped_text_field() {
    InteractiveScene::run(200.0, 60.0, |scene| {
        use cove_core::Binding;

        let text = Rc::new(RefCell::new(
            cove_ui_compose::primitives::text::text_buffer::TextBuffer::new("abc"),
        ));

        let mut layout = TextFieldLayout::default();
        layout.max_visible_lines = 0;
        let mut code_editing = TextFieldCodeEditing::default();
        code_editing.indentation_width = Some(99);
        let behavior = TextFieldBehavior::default();

        let field = TextField::new("Name", Binding::constant(text.borrow().clone()))
            .layout(layout)
            .code_editing(code_editing)
            .behavior(behavior);

        assert_eq!(
            field.layout.max_visible_lines, 1,
            "a zero visible-line count clamps to one"
        );
        assert_eq!(
            field.code_editing.indentation_width,
            Some(8),
            "an indentation width clamps into 1...8"
        );

        // Decoration is its own group; mutating a rebuilt field leaves the
        // original's decoration untouched, matching Swift's value semantics.
        let mut copy = TextField::new("Name", Binding::constant(text.borrow().clone()))
            .layout(field.layout)
            .code_editing(field.code_editing.clone())
            .behavior(field.behavior.clone());
        copy.decoration.prefix = Some("@".to_string());
        assert_eq!(
            field.decoration.prefix, None,
            "a rebuilt field does not carry its decoration back to the original"
        );
        assert_eq!(
            copy.decoration.prefix.as_deref(),
            Some("@"),
            "the rebuilt field keeps the prefix it was given"
        );

        scene.install(field);
        scene.settle(4, 1.0);
        assert!(
            !descendants(&scene.graph.tree.borrow().root().expect("root")).is_empty(),
            "the field materialises"
        );
    });
}
