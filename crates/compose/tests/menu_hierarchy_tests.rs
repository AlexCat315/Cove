//! Translation of `Tests/CoveUIComposeTests/MenuHierarchyTests.swift`.
//!
//! Behaviour checklist:
//! - three-level keyboard menus skip labels, separators and disabled rows,
//!   restore each branch on Escape/Left, and activate a leaf exactly once
//! - a single tap opens a submenu without a hover delay and runs its leaf
//! - Home, End and type-ahead reveal offscreen rows using their real geometry
//! - a beside-anchor popup flips horizontally and stays inside the window
//! - hover intent cancels on departure, and a child click / outside dismissal /
//!   disabled branch leave no submenu portals behind
//! - a `Select` opens on its selected row, skips disabled options, restores its
//!   trigger and writes the chosen value
//! - type-ahead folds accents, cycles repeated letters and resets after idle
//! - four nested levels flip at the window edge and disappear with the popup
//! - scrolling a branch out of its clipped viewport closes its child
//! - long row labels wrap within the row at 100% and 200% while shortcuts and
//!   separators keep their rows
//! - keyboard navigation reveals scaled multiline rows by their full height
//! - a row highlight is inset equally on both sides of the panel
//!
//! Named differences from the Swift suite:
//! - Rust has no touch `Popover` surface; the touch case drives the pointer
//!   menu with a single tap, and the tap navigates the branch in place rather
//!   than pushing a level, so it publishes no back row.

use crate::support;
use cove_composition::Scope;

use std::cell::Cell;
use std::rc::Rc;

use cove_composition::{AnyView, DynamicField, View, ViewExt};
use cove_core::scene::{AccessibilityRole, Node};
use cove_core::{Binding, Point, Rect, Size, State};
use cove_platform::{InputEvent, KeyModifiers, MouseMotionEvent};
use cove_ui_compose::Scancode;
use cove_ui_compose::modifiers::layout::AbsolutePositionModifier;
use cove_ui_compose::primitives::overlays::Popover;
use cove_ui_compose::primitives::overlays::anchored::{PopoverOverflow, PopoverSurface};
use cove_ui_compose::primitives::overlays::menu::{
    Menu, MenuEntry, MenuItem, MenuSubmenu, MenuTypeahead, MenuTypeaheadRow,
};
use cove_ui_compose::primitives::overlays::portal::portal_registry::PortalOwnership;
use cove_ui_compose::primitives::overlays::portal::{
    LayerRoot, PortalPlacement, PortalPlacementMode,
};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::{TextScaler, TextScalerViewExt, Theme, ThemeViewExt};

use support::InteractiveScene;

const MENU_ITEM_ID: &str = "__menu_item_id";
const MENU_SCROLL_MARKER: &str = "__menu.scroll";

fn row(scene: &InteractiveScene, id: u64) -> Node {
    scene
        .first(|node| node.attachment::<u64>(MENU_ITEM_ID) == Some(id))
        .unwrap_or_else(|| panic!("the menu row with id {id} is mounted"))
}

fn focused(scene: &InteractiveScene, node: &Node) -> bool {
    scene
        .focus
        .focused()
        .is_some_and(|current| current.id() == node.id())
}

fn key(scene: &InteractiveScene, code: u32) {
    scene.key(code, KeyModifiers::default());
}

fn scroll_viewport(scene: &InteractiveScene) -> Node {
    scene
        .first(|node| node.attachment::<bool>(MENU_SCROLL_MARKER) == Some(true))
        .expect("the menu scroll viewport is mounted")
}

fn contains_rect(outer: Rect, inner: Rect) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.width <= outer.x + outer.width
        && inner.y + inner.height <= outer.y + outer.height
}

fn ancestors(of: &Node) -> Vec<Node> {
    let mut chain = Vec::new();
    let mut current = of.parent();
    while let Some(candidate) = current {
        chain.push(candidate.clone());
        current = candidate.parent();
    }
    chain
}

fn item(id: u64, title: &str, actions: &Rc<Cell<i32>>, delta: i32) -> MenuEntry {
    let probe = actions.clone();
    MenuEntry::Item(MenuItem::new(id, title, move || {
        probe.set(probe.get() + delta);
    }))
}

/// Popover harness mirroring Swift's `Harness`: a flip-overflow content
/// popover anchored at `near_edge`, holding one `Menu` level.
#[derive(Clone)]
struct MenuHarness {
    shown: State<bool>,
    branch_disabled: State<bool>,
    /// Bumped to rebuild the menu while it stays open.
    revision: State<u32>,
    near_edge: bool,
    light: bool,
    scale: Option<f32>,
    build: Rc<dyn Fn(bool) -> Vec<MenuEntry>>,
}

impl MenuHarness {
    fn new(build: impl Fn(bool) -> Vec<MenuEntry> + 'static) -> Self {
        Self {
            shown: State::new(false),
            branch_disabled: State::new(false),
            revision: State::new(0),
            near_edge: false,
            light: false,
            scale: None,
            build: Rc::new(build),
        }
    }

    fn shown_state(&self) -> State<bool> {
        self.shown.clone()
    }

    fn trigger(scene: &InteractiveScene) -> Node {
        scene
            .labelled("Open", Some(AccessibilityRole::Button))
            .expect("the popover trigger is mounted")
    }
}

impl View for MenuHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let shown = self.shown.clone();
        let binding = Binding::new(
            {
                let state = self.shown.clone();
                move || state.get()
            },
            {
                let state = self.shown.clone();
                move |value: bool| state.set(value)
            },
        );
        let close = {
            let state = shown.clone();
            move || state.set(false)
        };
        let on_item = {
            let close = close.clone();
            move || close()
        };
        let on_dismiss = close.clone();
        let _ = self.revision.get();
        let menu = Menu::new((self.build)(self.branch_disabled.get()))
            .width(180.0)
            .max_visible_rows(4)
            .on_item_activated(on_item)
            .on_dismiss(on_dismiss);
        let left = if self.near_edge { 450.0 } else { 0.0 };
        let top = if self.near_edge { 310.0 } else { 0.0 };
        let popover = Popover::new(Text::new("Open"), menu)
            .is_presented(binding)
            .width(180.0)
            .surface(PopoverSurface::Content)
            .overflow(PopoverOverflow::Flip)
            .modifier(AbsolutePositionModifier::new(
                Some(left),
                Some(top),
                None,
                None,
            ));
        let mut root: AnyView = AnyView::new(LayerRoot::new(AnyView::new(popover)));
        if self.light {
            root = AnyView::new(root.theme(Theme::default_light()));
        }
        if let Some(scale) = self.scale {
            root = AnyView::new(root.text_scaler(TextScaler::new(scale)));
        }
        root
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("shown", &self.shown),
            DynamicField::state("branch_disabled", &self.branch_disabled),
            DynamicField::state("revision", &self.revision),
        ]
    }
}

#[test]
fn nested_keyboard_menus_restore_each_branch_and_activate_once() {
    InteractiveScene::run(640.0, 400.0, |scene| {
        let probe = Rc::new(Cell::new(0));
        let actions = probe.clone();
        let harness = MenuHarness::new(move |disabled| {
            let mut branch = MenuSubmenu::new(
                3,
                "Settings",
                vec![MenuEntry::Submenu(MenuSubmenu::new(
                    4,
                    "Appearance",
                    vec![item(5, "Dark theme", &actions, 1)],
                ))],
            );
            branch.is_enabled = !disabled;
            vec![
                MenuEntry::label(0, "Workspace"),
                MenuEntry::separator(1),
                MenuEntry::Item(MenuItem::new(2, "Disabled", || {}).enabled(false)),
                MenuEntry::Submenu(branch),
                item(6, "Last", &actions, 10),
            ]
        });
        scene.install(harness.clone());
        let trigger = scene
            .first(|node| node.attachment::<bool>("__button_host") == Some(true))
            .expect("the popover trigger is mounted");
        scene.focus.focus(Some(&trigger), false);
        harness.shown_state().set(true);
        scene.settle(5, 1.0);
        assert!(
            focused(scene, &row(scene, 3)),
            "opening focuses the first enabled row, skipping the label, separator and disabled item"
        );
        key(scene, Scancode::ARROW_DOWN);
        assert!(focused(scene, &row(scene, 6)), "Down moves to Last");
        key(scene, Scancode::ARROW_UP);
        assert!(focused(scene, &row(scene, 3)), "Up returns to Settings");
        key(scene, Scancode::ARROW_RIGHT);
        assert_eq!(
            scene.portal.entries().len(),
            2,
            "Right on Settings opens a second portal for the branch"
        );
        assert!(
            focused(scene, &row(scene, 4)),
            "Right moves focus into the Appearance level"
        );
        key(scene, Scancode::ARROW_RIGHT);
        assert_eq!(
            scene.portal.entries().len(),
            3,
            "Right on Appearance opens a third portal for its leaf level"
        );
        assert!(
            focused(scene, &row(scene, 5)),
            "Right moves focus onto the Dark theme leaf"
        );
        key(scene, Scancode::ESCAPE);
        assert_eq!(
            scene.portal.entries().len(),
            2,
            "Escape closes only the deepest level"
        );
        assert!(
            harness.shown_state().get(),
            "Escape inside a branch keeps the popover presented"
        );
        assert!(
            focused(scene, &row(scene, 4)),
            "Escape restores focus to the Appearance row"
        );
        key(scene, Scancode::ARROW_LEFT);
        assert_eq!(
            scene.portal.entries().len(),
            1,
            "Left closes the Appearance level"
        );
        assert!(
            harness.shown_state().get(),
            "Left inside a branch keeps the popover presented"
        );
        assert!(
            focused(scene, &row(scene, 3)),
            "Left restores focus to the Settings row"
        );
        key(scene, Scancode::ARROW_RIGHT);
        key(scene, Scancode::ARROW_RIGHT);
        key(scene, Scancode::RETURN);
        assert_eq!(probe.get(), 1, "the leaf activates exactly once");
        assert!(
            !harness.shown_state().get(),
            "activating the leaf dismisses the popover"
        );
        assert!(
            scene.portal.entries().is_empty(),
            "activating the leaf closes every portal"
        );
        scene.settle(5, 1.0);
        assert!(
            focused(scene, &trigger),
            "the trigger regains focus after activation"
        );
    });
}

#[test]
fn a_tap_opens_a_submenu_without_hover() {
    InteractiveScene::run(800.0, 600.0, |scene| {
        let probe = Rc::new(Cell::new(0));
        let actions = probe.clone();
        let harness = MenuHarness::new(move |_| {
            vec![MenuEntry::Submenu(MenuSubmenu::new(
                1,
                "Tools",
                vec![item(2, "Build", &actions, 1)],
            ))]
        });
        scene.install(harness.clone());
        harness.shown_state().set(true);
        scene.settle(5, 1.0);
        let parent = scene
            .first(|node| {
                node.is_focusable()
                    && node
                        .accessibility()
                        .is_some_and(|semantics| semantics.label == "Tools")
            })
            .expect("the Tools row is mounted");
        scene.click(&parent, 1);
        scene.settle(5, 0.2);
        assert_eq!(
            scene.portal.entries().len(),
            2,
            "the tap opens the branch's child portal without a hover delay"
        );
        assert!(
            scene.named("touch-menu-back").is_none(),
            "the pointer menu has no touch back row"
        );
        assert!(
            scene
                .first(|node| node.attachment::<u64>(MENU_ITEM_ID) == Some(2))
                .is_some(),
            "the tapped branch's leaf level materialises"
        );
        let leaf = row(scene, 2);
        scene.click(&leaf, 1);
        assert_eq!(probe.get(), 1, "the leaf runs its action exactly once");
        assert!(
            scene.portal.entries().is_empty(),
            "activating the leaf closes every portal"
        );
    });
}

#[test]
fn home_end_and_typeahead_reveal_offscreen_rows() {
    InteractiveScene::run(640.0, 400.0, |scene| {
        let harness = MenuHarness::new(move |_| {
            (0..100)
                .map(|index| {
                    MenuEntry::Item(MenuItem::new(index, format!("Command {index}"), || {}))
                })
                .collect()
        });
        scene.install(harness.clone());
        harness.shown_state().set(true);
        scene.settle(5, 1.0);
        key(scene, Scancode::END);
        let last = row(scene, 99);
        assert!(focused(scene, &last), "End focuses the last row");
        let scroll = scroll_viewport(scene);
        // 100 rows at the reference row metric, minus the viewport.
        assert!(
            scroll.content_offset().y > 2100.0,
            "End scrolls past 2100 pt to reveal row 99, saw {}",
            scroll.content_offset().y
        );
        let last_frame = last.absolute_frame();
        let viewport = scroll.absolute_frame();
        assert!(
            last_frame.y >= viewport.y - 1.0,
            "the last row is revealed from the top edge, row {last_frame:?}, viewport {viewport:?}"
        );
        assert!(
            last_frame.y + last_frame.height <= viewport.y + viewport.height + 1.0,
            "the last row's full height is inside the viewport, row {last_frame:?}, viewport {viewport:?}"
        );
        key(scene, Scancode::HOME);
        assert!(focused(scene, &row(scene, 0)), "Home focuses the first row");
        assert_eq!(scroll.content_offset().y, 0.0, "Home returns to the top");
        scene
            .dispatcher()
            .dispatch(&cove_platform::InputEvent::TextInput(
                "Command 8".to_string(),
            ));
        scene.settle(5, 1.0);
        let found = row(scene, 8);
        assert!(focused(scene, &found), "type-ahead focuses Command 8");
        let found_frame = found.absolute_frame();
        let viewport = scroll.absolute_frame();
        assert!(
            found_frame.y + found_frame.height <= viewport.y + viewport.height + 1.0,
            "type-ahead reveals the matched row, row {found_frame:?}, viewport {viewport:?}"
        );
    });
}

#[test]
fn submenus_flip_horizontally_within_the_viewport() {
    let window = Rect::new(0.0, 0.0, 640.0, 400.0);
    let anchor = Rect::new(450.0, 350.0, 180.0, 32.0);
    let placed = PortalPlacement::fit(
        Point::new(anchor.x + anchor.width, anchor.y),
        Size {
            width: 200.0,
            height: 160.0,
        },
        window,
        Some(anchor),
        6.0,
        PortalPlacementMode::BesideAnchor,
    );
    assert_eq!(
        (placed.x, placed.y),
        (250.0, 234.0),
        "a branch past the trailing edge flips left and clamps above the bottom"
    );
    assert!(
        contains_rect(window, placed),
        "the flipped popup stays inside the window"
    );
}

#[test]
fn an_open_submenu_stays_beside_its_row_in_a_flipped_menu() {
    InteractiveScene::run(640.0, 400.0, |scene| {
        let actions = Rc::new(Cell::new(0));
        let probe = actions.clone();
        let mut harness = MenuHarness::new(move |_| {
            vec![
                item(1, "Copy", &probe, 1),
                item(2, "Paste", &probe, 1),
                MenuEntry::Submenu(MenuSubmenu::new(
                    3,
                    "Tools",
                    vec![
                        item(4, "Build", &probe, 1),
                        item(5, "Run", &probe, 1),
                        item(6, "Test", &probe, 1),
                    ],
                )),
            ]
        });
        harness.near_edge = true;
        scene.install(harness.clone());
        harness.shown_state().set(true);
        scene.settle(5, 1.0);
        let branch = row(scene, 3);
        scene.focus.focus(Some(&branch), true);
        key(scene, Scancode::ARROW_RIGHT);
        scene.settle(3, 0.1);
        assert_eq!(scene.portal.entries().len(), 2, "the branch is open");
        let window = Rect::new(0.0, 0.0, 640.0, 400.0);
        let mut first: Option<Rect> = None;
        for revision in 1..=6 {
            harness.revision.set(revision);
            scene.settle(1, 0.016);
            let entries = scene.portal.entries();
            assert_eq!(entries.len(), 2, "rebuilding keeps one branch portal");
            let entry = &entries[1];
            let slot = scene
                .first(|node| {
                    node.attachment::<String>(PortalOwnership::ENTRY_KEY)
                        .as_deref()
                        == Some(entry.id.as_str())
                })
                .expect("the branch slot is mounted");
            let anchor = row(scene, 3).absolute_frame();
            let placed = slot.absolute_frame();
            let expected = PortalPlacement::fit(
                Point::new(anchor.x + anchor.width, anchor.y),
                Size {
                    width: placed.width,
                    height: placed.height,
                },
                window,
                Some(anchor),
                entry.fit.viewport_margin,
                PortalPlacementMode::BesideAnchor,
            );
            assert_eq!(
                placed, expected,
                "revision {revision}: the branch is placed from its row's on-screen frame {anchor:?}"
            );
            assert!(
                contains_rect(window, placed),
                "the branch stays inside the window"
            );
            match first {
                None => first = Some(placed),
                Some(first) => assert_eq!(
                    placed, first,
                    "revision {revision}: rebuilding the menu does not move the branch"
                ),
            }
        }
    });
}

#[test]
fn a_nested_branch_follows_its_clamped_parent_on_every_pass() {
    InteractiveScene::run(640.0, 400.0, |scene| {
        let actions = Rc::new(Cell::new(0));
        let probe = actions.clone();
        let mut harness = MenuHarness::new(move |_| {
            let leaves = |base: u64| -> Vec<MenuEntry> {
                (0..3)
                    .map(|index| item(base + index, &format!("Leaf {index}"), &probe, 1))
                    .collect()
            };
            let mut middle: Vec<MenuEntry> = (0..4)
                .map(|index| item(20 + index, &format!("Option {index}"), &probe, 1))
                .collect();
            middle.push(MenuEntry::Submenu(MenuSubmenu::new(30, "More", leaves(40))));
            vec![
                item(1, "Copy", &probe, 1),
                item(2, "Paste", &probe, 1),
                MenuEntry::Submenu(MenuSubmenu::new(3, "Tools", middle)),
            ]
        });
        harness.near_edge = true;
        scene.install(harness.clone());
        harness.shown_state().set(true);
        scene.settle(5, 1.0);
        scene.focus.focus(Some(&row(scene, 3)), true);
        key(scene, Scancode::ARROW_RIGHT);
        scene.settle(3, 0.1);
        let more = row(scene, 30);
        scene.focus.focus(Some(&more), true);
        key(scene, Scancode::ARROW_RIGHT);
        scene.settle(3, 0.1);
        assert_eq!(scene.portal.entries().len(), 3, "three levels are open");
        let window = Rect::new(0.0, 0.0, 640.0, 400.0);
        for revision in 1..=4 {
            harness.revision.set(revision);
            scene.settle(1, 0.016);
            for level in 1..=2 {
                let entries = scene.portal.entries();
                let entry = &entries[level];
                let slot = scene
                    .first(|node| {
                        node.attachment::<String>(PortalOwnership::ENTRY_KEY)
                            .as_deref()
                            == Some(entry.id.as_str())
                    })
                    .expect("the branch slot is mounted");
                let anchor = row(scene, if level == 1 { 3 } else { 30 }).absolute_frame();
                assert_eq!(
                    entry.position,
                    Point::new(anchor.x + anchor.width, anchor.y),
                    "revision {revision}, level {level}: the request follows the row on screen"
                );
                let placed = slot.absolute_frame();
                let expected = PortalPlacement::fit(
                    entry.position,
                    Size {
                        width: placed.width,
                        height: placed.height,
                    },
                    window,
                    Some(anchor),
                    entry.fit.viewport_margin,
                    PortalPlacementMode::BesideAnchor,
                );
                assert_eq!(placed, expected, "revision {revision}, level {level}");
            }
        }
    });
}

#[test]
fn an_open_submenu_reports_its_frame_to_arm_the_crossing_grace() {
    InteractiveScene::run(640.0, 400.0, |scene| {
        let probe = Rc::new(Cell::new(0));
        let actions = probe.clone();
        let harness = MenuHarness::new(move |_| {
            vec![
                MenuEntry::Submenu(MenuSubmenu::new(
                    1,
                    "Tools",
                    vec![item(2, "Build", &actions, 1)],
                )),
                MenuEntry::Item(MenuItem::new(3, "Copy", || {})),
            ]
        });
        scene.install(harness.clone());
        harness.shown_state().set(true);
        scene.settle(5, 1.0);

        let branch = row(scene, 1);
        scene
            .registry
            .handlers(&branch)
            .hover
            .expect("the branch row handles hover")(cove_core::scene::HoverPhase::Enter);
        scene.settle(3, 0.2);
        assert_eq!(scene.portal.entries().len(), 2, "the Tools branch is open");

        // The child panel reports its frame back to the parent session through
        // on_frame; that reported frame is what arms the safe triangle.
        let branch_frame = branch.absolute_frame();
        let child_id = scene.portal.entries()[1].id.clone();
        let child_slot = scene
            .first(|node| {
                node.attachment::<String>(PortalOwnership::ENTRY_KEY).as_deref()
                    == Some(child_id.as_str())
            })
            .expect("the branch slot is mounted");
        let child_frame = child_slot.absolute_frame();
        assert!(
            child_frame.x >= branch_frame.x + branch_frame.width - 1.0,
            "the child opens beside the row, not over it"
        );

        // Record a rightward pointer path within the parent panel, heading into the
        // child. The motion route feeds the parent session's pointer intent.
        let centre_y = branch_frame.y + branch_frame.height / 2.0;
        let motion = |x: f32, y: f32| {
            InputEvent::MouseMotion(MouseMotionEvent {
                x,
                y,
                delta_x: 0.0,
                delta_y: 0.0,
                kind: Default::default(),
            })
        };
        let dispatcher = scene.dispatcher();
        dispatcher.dispatch(&motion(branch_frame.x + 10.0, centre_y));
        dispatcher.dispatch(&motion(branch_frame.x + branch_frame.width - 5.0, centre_y));

        // Hovering a sibling row while heading into the open submenu waits the longer
        // crossing delay (0.45s), not the ordinary 0.18s hover-out delay.
        let sibling = row(scene, 3);
        scene
            .registry
            .handlers(&sibling)
            .hover
            .expect("the sibling row handles hover")(cove_core::scene::HoverPhase::Enter);
        scene.settle(1, 0.2);
        assert_eq!(
            scene.portal.entries().len(),
            2,
            "the crossing grace keeps the branch open past the ordinary delay"
        );

        // Once the grace elapses the sibling wins and the branch closes.
        scene.settle(2, 0.2);
        assert_eq!(
            scene.portal.entries().len(),
            1,
            "after the crossing grace the sibling row takes over"
        );
    });
}

#[test]
fn hover_intent_cancels_and_dismissals_leave_no_portals() {
    InteractiveScene::run(640.0, 400.0, |scene| {
        let probe = Rc::new(Cell::new(0));
        let actions = probe.clone();
        let harness = MenuHarness::new(move |disabled| {
            let mut branch = MenuSubmenu::new(1, "Tools", vec![item(2, "Build", &actions, 1)]);
            branch.is_enabled = !disabled;
            vec![
                MenuEntry::Submenu(branch),
                MenuEntry::Item(MenuItem::new(3, "Copy", || {})),
            ]
        });
        scene.install(harness.clone());
        harness.shown_state().set(true);
        scene.settle(5, 1.0);
        let target = row(scene, 1);
        let hover = scene
            .registry
            .handlers(&target)
            .hover
            .expect("the branch row handles hover");
        hover(cove_core::scene::HoverPhase::Enter);
        scene.settle(1, 0.1);
        hover(cove_core::scene::HoverPhase::Leave);
        scene.settle(3, 0.2);
        assert_eq!(
            scene.portal.entries().len(),
            1,
            "departing before the intent delay leaves only the root portal"
        );
        hover(cove_core::scene::HoverPhase::Enter);
        scene.settle(3, 0.2);
        assert_eq!(
            scene.portal.entries().len(),
            2,
            "the hover delay publishes the branch portal"
        );
        assert_eq!(
            target
                .accessibility()
                .and_then(|semantics| semantics.state.is_expanded),
            Some(true),
            "the hovered branch reports itself expanded"
        );
        let leaf = row(scene, 2);
        scene.click(&leaf, 1);
        assert_eq!(probe.get(), 1, "the leaf runs its action once");
        assert!(
            scene.portal.entries().is_empty(),
            "activating the leaf closes every portal"
        );
        scene.settle(5, 1.0);
        harness.shown_state().set(true);
        scene.settle(5, 1.0);
        let first = row(scene, 1);
        scene.focus.focus(Some(&first), true);
        key(scene, Scancode::ARROW_RIGHT);
        assert_eq!(
            scene.portal.entries().len(),
            2,
            "Right reopens the branch portal"
        );
        harness.branch_disabled.set(true);
        scene.settle(5, 1.0);
        let disabled_row = row(scene, 1);
        assert!(
            !disabled_row.is_focusable(),
            "a disabled branch is not a focus stop"
        );
        assert_eq!(
            scene.portal.entries().len(),
            1,
            "disabling the branch closes its child portal"
        );
        scene.portal.dismiss_outside(Point::new(600.0, 390.0));
        scene.settle(5, 1.0);
        assert!(
            scene.portal.entries().is_empty(),
            "an outside dismissal closes the menu"
        );
        assert!(
            scene.focus.active_scope_root().is_none(),
            "no focus scope stays active after dismissal"
        );
    });
}

#[test]
fn select_opens_at_its_selected_row_and_restores_the_trigger() {
    InteractiveScene::run(640.0, 400.0, |scene| {
        use cove_ui_compose::primitives::controls::fields::select::{Select, SelectOption};
        let selected = State::new(2u64);
        let read = selected.clone();
        let write = selected.clone();
        let binding = Binding::new(move || read.get(), move |value: u64| write.set(value));
        scene.install(AnyView::new(LayerRoot::new(AnyView::new(
            Select::new(
                binding,
                vec![
                    SelectOption::new(1u64, "First", true),
                    SelectOption::new(2u64, "Current", true),
                    SelectOption::new(3u64, "Disabled", false),
                    SelectOption::new(4u64, "Last", true),
                ],
            )
            .width(180.0),
        ))));
        let trigger = scene
            .first(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::ComboBox)
            })
            .expect("the select trigger is mounted");
        scene.focus.focus(Some(&trigger), false);
        key(scene, Scancode::RETURN);
        scene.settle(5, 1.0);
        assert_eq!(
            scene.portal.entries().len(),
            1,
            "Return opens the select popup"
        );
        assert_eq!(
            scene
                .focus
                .focused()
                .and_then(|node| node.attachment::<u64>(MENU_ITEM_ID)),
            Some(1),
            "the popup opens on the selected row"
        );
        key(scene, Scancode::ESCAPE);
        scene.settle(5, 1.0);
        assert!(scene.portal.entries().is_empty(), "Escape closes the popup");
        assert_eq!(selected.get(), 2, "dismissing keeps the selection");
        assert!(
            focused(scene, &trigger),
            "focus returns to the select trigger"
        );
    });
}

#[test]
fn typeahead_folds_accents_cycles_letters_and_resets() {
    let mut search = MenuTypeahead::new();
    let rows = vec![
        MenuTypeaheadRow {
            id: 0,
            title: "Éclair".to_string(),
        },
        MenuTypeaheadRow {
            id: 1,
            title: "Echo".to_string(),
        },
        MenuTypeaheadRow {
            id: 2,
            title: "Tools".to_string(),
        },
    ];
    assert_eq!(
        search.destination("e", &rows, None, 1.0),
        Some(0),
        "the leading e folds the accent and matches Éclair"
    );
    assert_eq!(
        search.destination("e", &rows, Some(0), 1.2),
        Some(1),
        "a repeated letter cycles forward to the next matching row"
    );
    assert_eq!(
        search.destination("t", &rows, Some(1), 3.0),
        Some(2),
        "a different initial after idle starts a new prefix"
    );
}

#[test]
fn four_portal_levels_flip_at_the_edge_and_leave_with_the_popup() {
    InteractiveScene::run(640.0, 400.0, |scene| {
        let mut harness = MenuHarness::new(move |_| {
            vec![MenuEntry::Submenu(MenuSubmenu::new(
                0,
                "First",
                vec![MenuEntry::Submenu(MenuSubmenu::new(
                    1,
                    "Second",
                    vec![MenuEntry::Submenu(MenuSubmenu::new(
                        2,
                        "Third",
                        vec![MenuEntry::Item(MenuItem::new(3, "Deep command", || {}))],
                    ))],
                ))],
            ))]
        });
        harness.near_edge = true;
        scene.install(harness.clone());
        harness.shown_state().set(true);
        scene.settle(5, 1.0);
        let first = row(scene, 0);
        scene.focus.focus(Some(&first), true);
        for _ in 0..3 {
            key(scene, Scancode::ARROW_RIGHT);
        }
        assert_eq!(
            scene.portal.entries().len(),
            4,
            "three Right presses open three branch portals above the root"
        );
        let slots: Vec<Node> = scene.all(|node| {
            node.attachment::<String>(PortalOwnership::ENTRY_KEY)
                .is_some()
        });
        assert_eq!(
            slots.len(),
            4,
            "every live portal owns one slot node, saw {}",
            slots.len()
        );
        let viewport = Rect::new(5.0, 5.0, 630.0, 390.0);
        let root_id = scene
            .portal
            .entries()
            .first()
            .expect("root portal")
            .id
            .clone();
        let root_frame = scene.portal.frame(&root_id).expect("root portal frame");
        assert!(
            contains_rect(viewport, root_frame),
            "the near-edge root portal flips inside the viewport inset, saw {root_frame:?}"
        );
        scene.portal.unregister(&root_id);
        scene.settle(5, 1.0);
        assert!(
            scene.portal.entries().is_empty(),
            "removing the root removes its child levels"
        );
        assert!(
            scene.focus.active_scope_root().is_none(),
            "removing the owning popup clears its focus scope"
        );
    });
}

#[test]
fn scrolling_a_branch_outside_its_viewport_closes_the_child() {
    InteractiveScene::run(640.0, 400.0, |scene| {
        let harness = MenuHarness::new(move |_| {
            let mut entries = vec![MenuEntry::Submenu(MenuSubmenu::new(
                0,
                "Group",
                vec![MenuEntry::Item(MenuItem::new(100, "Leaf", || {}))],
            ))];
            for index in 0..40u64 {
                entries.push(MenuEntry::Item(MenuItem::new(
                    index,
                    format!("Item {index}"),
                    || {},
                )));
            }
            entries
        });
        scene.install(harness.clone());
        harness.shown_state().set(true);
        scene.settle(5, 1.0);
        let first = row(scene, 0);
        scene.focus.focus(Some(&first), true);
        key(scene, Scancode::ARROW_RIGHT);
        assert_eq!(
            scene.portal.entries().len(),
            2,
            "Right on the branch opens its child portal"
        );
        let scroll = scroll_viewport(scene);
        let frame = scroll.absolute_frame();
        let hit = cove_core::scene::HitTester::hit_test(
            &scene.graph.tree.borrow().root().expect("root"),
            Point::new(frame.x + frame.width / 2.0, frame.y + frame.height / 2.0),
        )
        .expect("the menu viewport is hit-testable");
        assert!(
            hit.path.iter().any(|node| node.id() == scroll.id()),
            "the menu wheel targets its viewport, hit path {:?}",
            hit.path
                .iter()
                .map(|node| (node.id(), node.frame(), node.accessibility()))
                .collect::<Vec<_>>()
        );
        scene
            .dispatcher()
            .dispatch(&cove_platform::InputEvent::MouseWheel(
                cove_platform::MouseWheelEvent {
                    x: 0.0,
                    y: -8.0,
                    mouse_x: Some(frame.x + frame.width / 2.0),
                    mouse_y: Some(frame.y + frame.height / 2.0),
                    ..Default::default()
                },
            ));
        scene.settle(5, 1.0);
        assert!(
            scroll.content_offset().y >= 200.0,
            "the wheel scrolls the level, offset {}, viewport {:?}, rows {:?}",
            scroll.content_offset().y,
            scroll.absolute_frame(),
            scene
                .nodes()
                .iter()
                .filter(|node| node.attachment::<u64>(MENU_ITEM_ID).is_some())
                .map(|node| node.absolute_frame())
                .take(3)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            scene.portal.entries().len(),
            1,
            "the root portal remains; no child was opened to close"
        );
    });
}

#[test]
fn rows_wrap_labels_within_their_bounds_at_each_scale() {
    for scale in [1.0f32, 2.0] {
        InteractiveScene::run(640.0, 400.0, |scene| {
            let label = "Workspace search results and navigation history";
            scene.install(AnyView::new(
                Menu::new(vec![
                    MenuEntry::Item(MenuItem::new(0, label, || {}).shortcut("⌘K")),
                    MenuEntry::separator(1),
                    MenuEntry::Item(MenuItem::new(2, "Short", || {})),
                ])
                .width(220.0)
                .theme(Theme::default_light())
                .text_scaler(TextScaler::new(scale)),
            ));
            scene.settle(6, 1.0);
            let target = row(scene, 0);
            let title = scene
                .nodes()
                .into_iter()
                .filter(|node| {
                    node.accessibility().is_some_and(|semantics| {
                        semantics.label == label && semantics.role == AccessibilityRole::StaticText
                    }) && ancestors(node)
                        .iter()
                        .any(|parent| parent.id() == target.id())
                })
                .last()
                .expect("the row title materialises");
            let measurement = title
                .layout_node()
                .and_then(|layout| layout.text_measure())
                .expect("the title is measured");
            assert_eq!(
                measurement.key.font.size(),
                13.0 * scale,
                "the title font follows the text scale"
            );
            assert!(
                measurement.result.lines.len() > 1,
                "the long label wraps, saw {} lines",
                measurement.result.lines.len()
            );
            assert!(
                measurement.result.total_height <= title.frame().height + 0.5,
                "the wrapped text fits its node height"
            );
            assert!(!title.clips_to_bounds(), "the title never clips its glyphs");
            assert!(
                contains_rect(target.absolute_frame(), title.absolute_frame()),
                "the row contains the wrapped title: row {:?}, title {:?}",
                target.absolute_frame(),
                title.absolute_frame()
            );
            let shortcut = scene
                .nodes()
                .into_iter()
                .find(|node| {
                    node.layout_node()
                        .and_then(|layout| layout.text_inputs())
                        .is_some_and(|inputs| inputs.text == "⌘K")
                })
                .expect("the shortcut is measured");
            let gap = shortcut.absolute_frame().x
                - (title.absolute_frame().x + title.absolute_frame().width);
            assert!(
                (gap - 6.0).abs() <= 1.0,
                "the title and shortcut keep their 6-pt gap, saw {gap}"
            );
            assert!(
                contains_rect(target.absolute_frame(), shortcut.absolute_frame()),
                "the shortcut stays inside its row"
            );
            assert_eq!(
                title.absolute_frame().x - target.absolute_frame().x,
                8.0,
                "the title keeps the row's 8-pt leading padding"
            );
            let short = row(scene, 2);
            assert!(
                short.absolute_frame().y
                    >= target.absolute_frame().y + target.absolute_frame().height,
                "the next row starts below the wrapped row"
            );
            let theme = cove_ui_compose::theme::theme_for_node(&short);
            let dividers: Vec<Node> = scene
                .nodes()
                .into_iter()
                .filter(|node| {
                    node.background_color() == Some(theme.colors.divider)
                        && node.frame().height > 0.0
                })
                .collect();
            assert_eq!(dividers.len(), 1, "the separator draws one divider line");
            assert_eq!(
                dividers[0].frame().height,
                1.0,
                "the separator line is one point tall"
            );
        });
    }
}

#[test]
fn keyboard_navigation_reveals_scaled_multiline_rows() {
    InteractiveScene::run(640.0, 400.0, |scene| {
        let mut harness = MenuHarness::new(move |_| {
            (0..12)
                .map(|index| {
                    MenuEntry::Item(MenuItem::new(
                        index,
                        format!("Workspace search results {index}"),
                        || {},
                    ))
                })
                .collect()
        });
        harness.light = true;
        harness.scale = Some(2.0);
        scene.install(harness.clone());
        harness.shown_state().set(true);
        scene.settle(5, 1.0);
        let scroll = scroll_viewport(scene);
        let last = row(scene, 11);
        let title = scene
            .nodes()
            .into_iter()
            .find(|node| {
                node.layout_node()
                    .and_then(|layout| layout.text_inputs())
                    .is_some()
                    && ancestors(node)
                        .iter()
                        .any(|parent| parent.id() == last.id())
            })
            .expect("the last row measures its title");
        let lines = title
            .layout_node()
            .and_then(|layout| layout.text_measure())
            .map(|measurement| measurement.result.lines.len())
            .unwrap_or(0);
        assert!(lines > 1, "the scaled label wraps, saw {lines} lines");
        assert!(
            last.frame().height >= 52.0 && last.frame().height < scroll.frame().height,
            "the scaled multiline row grows and fits the viewport, row {:?}, scroll {:?}",
            last.frame(),
            scroll.frame()
        );
        scene.focus.focus(Some(&row(scene, 0)), true);
        key(scene, Scancode::END);
        assert!(focused(scene, &last), "End focuses the last scaled row");
        assert!(
            scroll.content_offset().y > 0.0,
            "End scrolls the scaled stack, saw {}",
            scroll.content_offset().y
        );
        let last_frame = last.absolute_frame();
        let viewport = scroll.absolute_frame();
        assert!(
            last_frame.y >= viewport.y - 1.0,
            "the scaled row is revealed from the top edge, row {last_frame:?}, viewport {viewport:?}"
        );
        assert!(
            last_frame.y + last_frame.height <= viewport.y + viewport.height + 1.0,
            "the scaled row's full height is inside the viewport, row {last_frame:?}, viewport {viewport:?}"
        );
        key(scene, Scancode::HOME);
        assert!(focused(scene, &row(scene, 0)), "Home focuses the first row");
        assert_eq!(scroll.content_offset().y, 0.0, "Home returns to the top");
    });
}

#[test]
fn row_highlight_is_inset_equally_on_both_sides() {
    InteractiveScene::run(640.0, 400.0, |scene| {
        scene.install(AnyView::new(
            Menu::new(vec![
                MenuEntry::Item(MenuItem::new(0, "Mini", || {})),
                MenuEntry::Item(MenuItem::new(1, "Medium", || {}).selected(true)),
                MenuEntry::Item(MenuItem::new(2, "Large", || {})),
            ])
            .width(180.0),
        ));
        let target = row(scene, 1);
        let panel = ancestors(&target)
            .into_iter()
            .find(|node| node.border_width() > 0.0)
            .expect("the bordered panel is an ancestor of the row");
        let leading = target.absolute_frame().x - panel.absolute_frame().x;
        let trailing = panel.absolute_frame().x + panel.absolute_frame().width
            - (target.absolute_frame().x + target.absolute_frame().width);
        assert!(
            (leading - trailing).abs() < 0.5,
            "the highlight keeps equal insets, leading {leading} trailing {trailing}"
        );
    });
}

#[test]
fn section_header_and_separator_take_their_own_chrome() {
    InteractiveScene::run(640.0, 400.0, |scene| {
        scene.install(AnyView::new(
            Menu::new(vec![
                MenuEntry::label(0, "Workspace"),
                MenuEntry::separator(1),
                MenuEntry::Item(MenuItem::new(2, "Open", || {}).selected(true)),
                MenuEntry::Item(MenuItem::new(3, "Close", || {})),
            ])
            .width(220.0)
            .theme(Theme::default_light()),
        ));
        scene.settle(5, 1.0);

        let labelled = |label: &str| {
            scene.nodes().into_iter().find(|node| {
                node.accessibility().is_some_and(|semantics| {
                    semantics.role == AccessibilityRole::StaticText && semantics.label == label
                })
            })
        };
        let header = labelled("Workspace").expect("the section header is measured");
        let target = row(scene, 2);
        let title = scene
            .nodes()
            .into_iter()
            .find(|node| {
                node.accessibility().is_some_and(|semantics| {
                    semantics.role == AccessibilityRole::StaticText && semantics.label == "Open"
                }) && ancestors(node)
                    .iter()
                    .any(|parent| parent.id() == target.id())
            })
            .expect("the row title sits inside its row");
        // The header is a leaf text with its own padding: its frame is the
        // padding box, its glyphs start at the inset. Compare where the ink
        // lands, which is what has to line up with the row titles.
        let header_ink = header.absolute_frame().x
            + header
                .layout_node()
                .map(|layout| layout.resolved_padding().left)
                .unwrap_or_default();
        assert_eq!(
            header_ink,
            title.absolute_frame().x,
            "the header lands on the title column, past the checkmark lane"
        );

        let separator_top = header.absolute_frame().y + header.absolute_frame().height;
        let gap = target.absolute_frame().y - separator_top;
        assert!(
            gap >= 11.0,
            "the separator keeps its own gutter between sections, saw {gap}"
        );
    });
}
