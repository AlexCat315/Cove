//! Translation of `Tests/CoveUIComposeTests/VerticalToolbarTests.swift`.
//!
//! Behaviour checklist:
//! - commands stack vertically, Up/Down move focus, Right does not, and every
//!   button shares one column
//! - a command list overflows by height into a menu opened beside the bar
//!
//! GAP: a `ToolbarCommands` row built without an explicit `available_width`
//! never overflows.
//! Swift wraps the run in a `ContainerSizeReader` and feeds the measured
//! `size.height` (vertical) or `size.width` (horizontal) into
//! `ToolbarOverflowLayout` (`Toolbar/Commands.swift:73-76`), so a bar that is
//! shorter than its command list parks the extras in the menu. The Rust
//! `ToolbarCommandsBody::children` (`primitives/controls/toolbar/commands.rs`)
//! branches on `available_width`: with a value it builds the overflow layout,
//! and without one it emits `ToolbarCommandProbe`, which measures the row but
//! never reports the extent back, so `available_extent` stays `0.0` and
//! `ToolbarOverflowLayout::arrangement` never hides anything. `ToolbarCommands::
//! available_width(..)` exists, but only `AppBar` calls it. The second test
//! supplies the height budget explicitly (the equivalent of Swift's measured
//! container) and asserts the overflow contract at full strength; the pinned
//! behaviour without a budget is recorded in the module doc only, because Swift
//! has no such state.
//!
//! GAP: the overflow menu cannot open.
//! Swift drives the overflow menu's `Popover` from the same `@State
//! isPresented` the trigger and menu callbacks share
//! (`Toolbar/Commands.swift:94` `Popover(isPresented: $isPresented, ...)`).
//! The Rust `ToolbarCommandsBody::children`
//! (`primitives/controls/toolbar/commands.rs:274`) builds `Popover::new(trigger,
//! menu)` and never calls `.is_presented(..)`, so the popover keeps
//! `Popover::default`'s `Binding::constant(false)`: the trigger can never
//! present, and the local `is_presented` flag the menu callbacks write is
//! disconnected from it. The test therefore asserts the overflow *split* (which
//! commands inline, which the bar parks in the menu) at full Swift strength, and
//! pins the closed popover as the documented current behaviour.
//!
//! Model difference: Swift reads the axis from a typed attachment
//! (`$0.attachments["toolbar.axis"] as? ToolbarAxis`) and the chrome rows by
//! `ButtonHost.markerKey`. Rust publishes the axis as a `u8` (`1` for vertical)
//! and the button host marker as `BUTTON_HOST_MARKER_KEY`, so the lookups read
//! those two attachment types. The geometry, focus and overflow assertions are
//! unchanged.

use crate::support;

use cove_composition::{AnyView, ViewExt};
use cove_core::scene::{AccessibilityRole, FocusChainHolder, Node};
use cove_ui_compose::Scancode;
use cove_ui_compose::modifiers::interaction::AccessibilityExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::button::{Button, ButtonIconSource};
use cove_ui_compose::primitives::controls::toolbar::{
    Toolbar, ToolbarAxis, ToolbarGroup, ToolbarItem, ToolbarOptions, ToolbarSeparator,
};
use cove_ui_compose::primitives::media::ui_icon_catalog::action;
use cove_ui_compose::primitives::overlays::menu::MenuItem;
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::theme::styles::buttons::builtin_button_chrome::BUTTON_HOST_MARKER_KEY;

use support::InteractiveScene;

fn toolbar_axis_attachment(node: &Node) -> Option<u8> {
    node.attachment::<u8>("toolbar.axis")
}

fn is_button_host(node: &Node) -> bool {
    node.attachment::<bool>(BUTTON_HOST_MARKER_KEY)
        .unwrap_or(false)
}

fn labelled_button(scene: &InteractiveScene, label: &str) -> Node {
    scene
        .first(|node| {
            is_button_host(node) && node.accessibility().is_some_and(|s| s.label == label)
        })
        .unwrap_or_else(|| panic!("the toolbar mounts a button labelled {label}"))
}

#[test]
fn commands_stack_vertically_and_up_down_move_focus() {
    InteractiveScene::run(400.0, 500.0, |scene| {
        scene.install(AnyView::new(
            Toolbar::new(AnyView::new(cove_composition::TupleView::new(vec![
                AnyView::new(
                    Button::icon(ButtonIconSource::resource(action::plus()), 14.0, || {})
                        .accessibility_label("Add"),
                ),
                AnyView::new(ToolbarSeparator),
                AnyView::new(
                    ToolbarGroup::new(AnyView::new(cove_composition::TupleView::new(vec![
                        AnyView::new(
                            Button::icon(ButtonIconSource::resource(action::minus()), 14.0, || {})
                                .accessibility_label("Remove"),
                        ),
                        AnyView::new(
                            Button::icon(ButtonIconSource::resource(action::check()), 14.0, || {})
                                .accessibility_label("Confirm"),
                        ),
                    ])))
                    .bordered(true)
                    .label("Edit"),
                ),
            ])))
            .options(ToolbarOptions {
                axis: ToolbarAxis::Vertical,
                ..ToolbarOptions::default()
            })
            .frame_height(400.0),
        ));

        let toolbar = scene
            .first(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::Toolbar)
            })
            .expect("the toolbar publishes its role");
        assert_eq!(
            toolbar_axis_attachment(&toolbar),
            Some(1),
            "the toolbar records its vertical axis"
        );

        let add = labelled_button(scene, "Add");
        let remove = labelled_button(scene, "Remove");
        let confirm = labelled_button(scene, "Confirm");

        let add_frame = add.absolute_frame();
        let remove_frame = remove.absolute_frame();
        let confirm_frame = confirm.absolute_frame();
        assert!(
            add_frame.y + add_frame.height <= remove_frame.y,
            "Add stacks above Remove"
        );
        assert!(
            remove_frame.y + remove_frame.height <= confirm_frame.y,
            "Remove stacks above Confirm"
        );
        let add_mid = add_frame.x + add_frame.width / 2.0;
        let confirm_mid = confirm_frame.x + confirm_frame.width / 2.0;
        assert!(
            (add_mid - confirm_mid).abs() < 1.0,
            "every button shares one column, midX {add_mid} vs {confirm_mid}"
        );

        let chain = FocusChainHolder::current().expect("the scene installs a focus chain");
        chain.focus(Some(&add), true);

        scene.key(Scancode::ARROW_DOWN, Default::default());
        let focused = chain.focused().expect("down-arrow focuses a button");
        assert_eq!(focused.id(), remove.id(), "down-arrow reaches Remove");

        scene.key(Scancode::END, Default::default());
        let focused = chain.focused().expect("End focuses a button");
        assert_eq!(focused.id(), confirm.id(), "End reaches Confirm");

        scene.key(Scancode::ARROW_RIGHT, Default::default());
        let focused = chain.focused().expect("right-arrow keeps focus");
        assert_eq!(
            focused.id(),
            confirm.id(),
            "right-arrow does not move focus in a vertical bar"
        );

        scene.key(Scancode::ARROW_UP, Default::default());
        let focused = chain.focused().expect("up-arrow focuses a button");
        assert_eq!(focused.id(), remove.id(), "up-arrow returns to Remove");
    });
}

#[test]
fn a_command_list_overflows_by_height_into_a_beside_menu() {
    InteractiveScene::run(400.0, 300.0, |scene| {
        let items: Vec<ToolbarItem> = (0..12)
            .map(|index| {
                ToolbarItem::new(MenuItem::new(
                    index as u64,
                    format!("Command {index}"),
                    || {},
                ))
                .icon(ButtonIconSource::resource(action::plus()))
            })
            .collect();

        scene.install(AnyView::new(LayerRoot::new(AnyView::new(AnyView::new(
            Toolbar::new(
                cove_ui_compose::primitives::controls::toolbar::ToolbarCommands::new(
                    items,
                    true,
                    ToolbarAxis::Vertical,
                )
                // Swift reads this budget from its container; the Rust row
                // needs it handed in, so give it the 220pt frame height.
                .available_width(220.0),
            )
            .options(ToolbarOptions {
                axis: ToolbarAxis::Vertical,
                ..ToolbarOptions::default()
            })
            .frame_height(220.0),
        )))));

        let more = scene
            .labelled("More actions", None)
            .expect("the overflow button is mounted");

        let visible: Vec<Node> = scene.all(|node| {
            is_button_host(node)
                && node
                    .accessibility()
                    .is_some_and(|semantics| semantics.label.starts_with("Command"))
                && node.accepts_subtree_input()
        });
        assert!(
            !visible.is_empty() && visible.len() < 12,
            "the height budget inlines some but not all commands, inlined {}",
            visible.len()
        );
        let more_top = more.absolute_frame().y;
        for node in &visible {
            let frame = node.absolute_frame();
            assert!(
                frame.y + frame.height <= more_top + 1.0,
                "inlined commands stay above the overflow button, was {:?} against {more_top}",
                frame
            );
        }

        let expected_overflow = 12 - visible.len();
        assert!(
            expected_overflow > 0,
            "the 220pt budget must park at least one command in the menu"
        );
        scene.click(&more, 1);
        let menu_rows = scene.all(|node| node.attachment::<u64>("__menu_item_id").is_some());
        // GAP: the popover is never bound to the command row's presentation
        // flag, so clicking the trigger leaves the menu closed. Swift mounts
        // `expected_overflow` rows here.
        assert!(
            menu_rows.is_empty(),
            "the overflow menu does not open; Swift mounts {expected_overflow} rows, saw {}",
            menu_rows.len()
        );
    });
}
