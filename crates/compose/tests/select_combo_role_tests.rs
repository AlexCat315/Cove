//! Translation of `Tests/CoveUIComposeTests/SelectEnumerationTests.swift`
//! accessibility half, plus the ComboBox role/name regression.
//!
//! Behaviour checklist:
//! - a `Select`'s trigger reads as a combo box, not a button
//! - the trigger's accessible name is the selected option's label and its value
//!   is the selected detail, so assistive technology announces both
//! - `Select` and `Combobox` publish those through the same composition locals
//!   the `Button` reads, which a wrapper node could not do

use crate::support;

use cove_composition::{AnyView, TupleView};
use cove_core::scene::AccessibilityRole;
use cove_core::{Binding, Color};
use cove_platform::{InputEvent, MouseMotionEvent};
use cove_ui_compose::primitives::controls::fields::color_field::ColorField;
use cove_ui_compose::primitives::controls::fields::combobox::Combobox;
use cove_ui_compose::primitives::controls::fields::select::{Select, SelectOption};
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::theme::styles::buttons::builtin_button_chrome::BUTTON_HOST_MARKER_KEY;
use cove_ui_compose::{Row, VerticalAlignment};

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

fn combo_boxes(scene: &InteractiveScene) -> Vec<cove_core::scene::Node> {
    scene.all(|node| {
        node.attachment::<bool>(BUTTON_HOST_MARKER_KEY).is_some()
            && node
                .accessibility()
                .is_some_and(|semantics| semantics.role == AccessibilityRole::ComboBox)
    })
}

fn expanded(node: &cove_core::scene::Node) -> Option<bool> {
    node.accessibility()?.state.is_expanded
}

#[test]
fn a_click_opens_only_the_select_it_landed_on() {
    InteractiveScene::run(600.0, 400.0, |scene| {
        scene.install(LayerRoot::new(AnyView::new(Row::new(
            VerticalAlignment::Center,
            8.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(Select::new(Binding::constant(Mode::Overview), options())),
                AnyView::new(Select::new(Binding::constant(Mode::Details), options())),
            ])),
        ))));
        let triggers = combo_boxes(scene);
        assert_eq!(triggers.len(), 2);

        // One settle: the click alone must recompose the control, with no
        // unrelated state change to carry it along.
        scene.click(&triggers[0], 1);
        scene.settle(1, 1.0 / 60.0);
        let triggers = combo_boxes(scene);
        assert_eq!(expanded(&triggers[0]), Some(true), "the clicked select opens");
        assert_eq!(expanded(&triggers[1]), Some(false), "its neighbour stays shut");
        assert_eq!(scene.portal.entries().len(), 1, "the popup is mounted");

        scene.click(&triggers[0], 1);
        scene.settle(1, 1.0 / 60.0);
        assert_eq!(expanded(&combo_boxes(scene)[0]), Some(false), "a second click closes it");
    });
}

#[test]
fn a_narrow_select_sizes_its_trigger_and_keeps_every_option_on_one_line() {
    let options = ["100%", "125%", "150%", "200%", "A much longer option"]
        .map(|label| SelectOption::new(label, label, true))
        .to_vec();
    InteractiveScene::run(600.0, 400.0, |scene| {
        scene.install(LayerRoot::new(AnyView::new(
            Select::new(Binding::constant("100%"), options).width(86.0),
        )));
        let trigger = combo_boxes(scene).remove(0);
        assert_eq!(trigger.absolute_frame().width, 86.0, "width sizes the trigger");

        scene.click(&trigger, 1);
        scene.settle(4, 1.0 / 60.0);
        let rows: Vec<_> = scene
            .all(|node| node.attachment::<u64>("__menu_item_id").is_some())
            .into_iter()
            .map(|node| node.absolute_frame())
            .collect();
        assert_eq!(rows.len(), 5, "every option is mounted");
        assert!(
            rows.iter().all(|row| row.height == rows[0].height),
            "no option wraps: {rows:?}"
        );
        assert!(rows[0].width >= 86.0 - 8.0, "the popup is at least the trigger");
    });
}

#[test]
fn a_click_opens_the_color_field_popover() {
    InteractiveScene::run(600.0, 400.0, |scene| {
        scene.install(LayerRoot::new(AnyView::new(ColorField::new(
            Binding::constant(Color::WHITE),
        ))));
        let trigger = trigger(scene);
        assert_eq!(expanded(&trigger), Some(false));
        scene.click(&trigger, 1);
        scene.settle(1, 1.0 / 60.0);
        assert_eq!(expanded(&self::trigger(scene)), Some(true));
    });
}

#[test]
fn the_highlight_moves_with_the_pointer_and_the_arrows() {
    InteractiveScene::run(600.0, 400.0, |scene| {
        scene.install(LayerRoot::new(AnyView::new(
            Select::new(Binding::constant(Mode::Overview), options()).width(160.0),
        )));
        let trigger = combo_boxes(scene).remove(0);
        scene.click(&trigger, 1);
        scene.settle(4, 1.0 / 60.0);
        let row = |scene: &InteractiveScene, id: u64| {
            scene
                .first(|node| node.attachment::<u64>("__menu_item_id") == Some(id))
                .unwrap_or_else(|| panic!("row {id} is mounted"))
        };
        let lit = |scene: &InteractiveScene, id: u64| {
            row(scene, id)
                .attachment::<Color>("__menu_visual_fill")
                .is_some_and(|fill| fill != Color::CLEAR)
        };
        assert!(lit(scene, 0), "the popup opens on the selected row");
        assert!(!lit(scene, 1));

        let target = row(scene, 1).absolute_frame();
        scene.dispatcher().dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
            x: target.x + target.width / 2.0,
            y: target.y + target.height / 2.0,
            ..Default::default()
        }));
        scene.settle(2, 1.0 / 60.0);
        assert!(lit(scene, 1), "the highlight follows the pointer");
        assert!(!lit(scene, 0), "one row is highlighted at a time");

        scene.key(cove_ui_compose::Scancode::ARROW_UP, Default::default());
        scene.settle(2, 1.0 / 60.0);
        assert!(lit(scene, 0), "the highlight follows the arrows");
    });
}
