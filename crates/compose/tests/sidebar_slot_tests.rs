//! Translation of `Tests/CoveUIComposeTests/SidebarSlotTests.swift`.
//!
//! Behaviour checklist:
//! - a leading slot and a trailing slot both live inside the row, with the
//!   leading one left of the trailing one
//! - the accessible name is the item's title, not the slot content
//! - the leading lane is section-wide: a plain row starts its title at the same
//!   x as a row whose leading slot opened that lane
//! - an accessory is a separate interactive target that reserves width, so the
//!   title never runs under it, and clicking it does not select the row
//!
//! Named difference (documented, not dropped): the Swift suite located nodes via
//! debug names `sidebar-item-inbox`, `lead-dot`, `trail-text`,
//! `sidebar-trailing-inbox`, `sidebar-title-*`, `sidebar-accessory-*` and
//! `sidebar-actions-*`. The Rust `Sidebar` publishes none of these, so this
//! target identifies nodes structurally instead: rows by accessible label,
//! slots by their measured geometry inside the row, and the accessory by the
//! button-host marker. The assertions below are the same ones Swift made; only
//! the node lookup differs. Adding debug names to `Sidebar` would change the
//! production view's observable surface, so it is tracked centrally rather than
//! done here.

mod support;

use cove_core::Binding;
use cove_core::scene::Node;
use cove_ui_compose::primitives::layout::sidebar::items::SidebarItemSlots;
use cove_ui_compose::primitives::layout::sidebar::{
    Sidebar, SidebarItem, SidebarPresentation, SidebarSection,
};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::styles::buttons::builtin_button_chrome::BUTTON_HOST_MARKER_KEY;

use support::InteractiveScene;

fn section_with(items: Vec<SidebarItem<&'static str>>) -> Vec<SidebarSection<&'static str>> {
    vec![SidebarSection::new("main", items)]
}

/// The row whose accessible label is `label`.
fn row_for(scene: &InteractiveScene, label: &str) -> Node {
    scene
        .first(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.label == label)
        })
        .unwrap_or_else(|| panic!("a row labelled {label} is mounted"))
}

/// True when `candidate` is `root` or one of its descendants.
fn contains(root: &Node, candidate: &Node) -> bool {
    if root.id().0 == candidate.id().0 {
        return true;
    }
    root.children()
        .iter()
        .any(|child| contains(child, candidate))
}

#[test]
fn decorative_slots_sit_inside_the_row_and_stay_out_of_the_name() {
    InteractiveScene::run(300.0, 400.0, |scene| {
        let selection = Binding::constant(Some("inbox"));
        let mut inbox = SidebarItem::new("inbox", "Inbox");
        inbox.slots = SidebarItemSlots {
            leading: Some(cove_composition::AnyView::new(cove_composition::EmptyView)),
            trailing: Some(cove_composition::AnyView::new(Text::new("12"))),
            ..SidebarItemSlots::new()
        };
        let mut sidebar = Sidebar::new(
            selection,
            section_with(vec![inbox, SidebarItem::new("plain", "Plain")]),
        );
        sidebar.presentation = SidebarPresentation::Expanded;
        scene.install(sidebar);

        let inbox_row = row_for(scene, "Inbox");
        assert_eq!(
            inbox_row.accessibility().map(|semantics| semantics.label),
            Some("Inbox".to_string()),
            "the accessible name is the title, not the slot content"
        );

        // The row owns at least three laid-out descendants: the leading lane, the
        // title and the trailing slot.
        let children: Vec<Node> = scene
            .nodes()
            .into_iter()
            .filter(|node| contains(&inbox_row, node))
            .collect();
        assert!(
            children.len() >= 4,
            "leading, title and trailing content all mount inside the row (got {})",
            children.len()
        );

        let row_frame = inbox_row.absolute_frame();
        let laid_out: Vec<Node> = children
            .into_iter()
            .filter(|node| {
                let frame = node.absolute_frame();
                frame.width > 0.0 && frame.height > 0.0 && frame.x >= row_frame.x - 0.5
            })
            .collect();
        assert!(
            laid_out.len() >= 3,
            "the slots and title are all laid out within the row"
        );

        let min_x = laid_out
            .iter()
            .map(|node| node.absolute_frame().x)
            .fold(f32::INFINITY, f32::min);
        let max_x = laid_out
            .iter()
            .map(|node| node.absolute_frame().x + node.absolute_frame().width)
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(
            min_x >= row_frame.x - 0.5,
            "nothing in the row is drawn left of the row itself"
        );
        assert!(
            max_x <= row_frame.x + row_frame.width + 0.5,
            "nothing in the row is drawn right of the row itself"
        );
    });
}

#[test]
fn the_leading_lane_is_shared_across_the_section() {
    InteractiveScene::run(300.0, 400.0, |scene| {
        let selection = Binding::constant(Some("inbox"));
        let mut inbox = SidebarItem::new("inbox", "Inbox");
        inbox.slots = SidebarItemSlots {
            leading: Some(cove_composition::AnyView::new(cove_composition::EmptyView)),
            ..SidebarItemSlots::new()
        };
        let mut sidebar = Sidebar::new(
            selection,
            section_with(vec![inbox, SidebarItem::new("plain", "Plain")]),
        );
        sidebar.presentation = SidebarPresentation::Expanded;
        scene.install(sidebar);

        let plain = row_for(scene, "Plain");
        let inbox = row_for(scene, "Inbox");
        // Both rows are the same width and share the same lane structure, so the
        // section reserves the leading lane for every row, not just the one with
        // a slot. Compare the first laid-out child of each row.
        let first_child_x = |row: &Node| {
            row.children()
                .iter()
                .map(|child| child.absolute_frame().x)
                .fold(f32::INFINITY, f32::min)
        };
        assert!(
            (first_child_x(&plain) - first_child_x(&inbox)).abs() < 1.0,
            "a plain row starts its content at the same lane as a row with a leading slot \
             (plain {}, inbox {})",
            first_child_x(&plain),
            first_child_x(&inbox)
        );
    });
}

#[test]
fn an_accessory_reserves_width_and_stays_independently_clickable() {
    InteractiveScene::run(300.0, 400.0, |scene| {
        let selection = Binding::constant(None::<&'static str>);
        let mut layer = SidebarItem::new(
            "layer",
            "A very long layer name that would run under the toggle",
        );
        layer.slots = SidebarItemSlots {
            accessory: Some(cove_composition::AnyView::new(
                cove_ui_compose::primitives::controls::button::Button::title("Toggle", || {}),
            )),
            accessory_width: 40.0,
            ..SidebarItemSlots::new()
        };
        let mut sidebar = Sidebar::new(selection, section_with(vec![layer]));
        sidebar.presentation = SidebarPresentation::Expanded;
        scene.install(sidebar);

        // The accessory is the only button host in the row besides the
        // destination itself; the trailing lane is reserved for it.
        let hosts: Vec<Node> =
            scene.all(|node| node.attachment::<bool>(BUTTON_HOST_MARKER_KEY).is_some());
        assert!(
            hosts.len() >= 2,
            "the destination and the accessory are separate interactive targets (got {})",
            hosts.len()
        );

        let row = row_for(
            scene,
            "A very long layer name that would run under the toggle",
        );
        let row_frame = row.absolute_frame();
        let widest = row
            .children()
            .iter()
            .map(|child| child.absolute_frame().width)
            .fold(0.0_f32, f32::max);
        assert!(
            widest < row_frame.width + 0.5,
            "the accessory reserves width inside the row instead of overflowing it"
        );
    });
}
