//! Translation of `Tests/CoveUIComposeTests/SelectEnumerationTests.swift`
//! accessibility half, plus the ComboBox role/name regression.
//!
//! Behaviour checklist:
//! - a `Select`'s trigger reads as a combo box, not a button
//! - the trigger's accessible name is the selected option's label and its value
//!   is the selected detail, so assistive technology announces both
//! - `Select` and `Combobox` publish those through the same composition locals
//!   the `Button` reads, which a wrapper node could not do

mod support;

use cove_core::Binding;
use cove_core::scene::AccessibilityRole;
use cove_ui_compose::primitives::controls::fields::combobox::Combobox;
use cove_ui_compose::primitives::controls::fields::select::{Select, SelectOption};
use cove_ui_compose::theme::styles::buttons::builtin_button_chrome::BUTTON_HOST_MARKER_KEY;

use support::InteractiveScene;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Overview,
    Details,
}

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Details => "Details",
        }
    }
}

fn options() -> Vec<SelectOption<Mode>> {
    vec![
        SelectOption::new(Mode::Overview, "Overview", true),
        SelectOption::new(Mode::Details, "Details", true),
    ]
}

fn trigger(scene: &InteractiveScene) -> cove_core::scene::Node {
    scene
        .first(|node| node.attachment::<bool>(BUTTON_HOST_MARKER_KEY).is_some())
        .expect("the popover trigger is a button host")
}

#[test]
fn select_trigger_reads_as_a_combo_box_with_the_selected_name() {
    InteractiveScene::run(400.0, 300.0, |scene| {
        scene.install(Select::new(Binding::constant(Mode::Details), options()));
        let trigger = trigger(scene);
        let semantics = trigger
            .accessibility()
            .expect("the trigger publishes accessibility semantics");

        assert_eq!(
            semantics.role,
            AccessibilityRole::ComboBox,
            "a Select trigger is a combo box, not a plain button"
        );
        assert_eq!(
            semantics.label, "Details",
            "the accessible name is the selected option's label"
        );
        assert_eq!(
            semantics.value, "Details",
            "the value mirrors the selected option so a reader announces it"
        );
        assert_eq!(
            semantics.state.is_expanded,
            Some(false),
            "a closed combo box reports an explicit collapsed state, not an absent one"
        );
    });
}

#[test]
fn combobox_trigger_reads_as_a_combo_box_with_its_title() {
    InteractiveScene::run(400.0, 300.0, |scene| {
        scene.install(Combobox::new(Binding::constant(None::<Mode>), options()));
        let trigger = trigger(scene);
        let semantics = trigger
            .accessibility()
            .expect("the trigger publishes accessibility semantics");

        assert_eq!(
            semantics.role,
            AccessibilityRole::ComboBox,
            "a Combobox trigger is a combo box"
        );
        assert_eq!(
            semantics.label, "Select an option",
            "the placeholder stands in as the accessible name before a selection"
        );
    });
}
