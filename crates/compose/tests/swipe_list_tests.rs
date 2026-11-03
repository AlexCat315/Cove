//! Translation of `Tests/CoveUIComposeTests/SwipeListTests.swift`.
//!
//! Behaviour checklist:
//! - a touch row is at least 44pt tall and spans the list width
//! - a left swipe reveals trailing actions without holding pointer capture, and
//!   activating an action runs its handler
//! - trailing actions reveal in RTL on a rightward drag and at larger text
//!   scales, and a drag toward the trailing edge never reveals
//! - the actions button expands the non-swipe path
//! - a vertical drag scrolls the list instead of revealing actions
//! - disabled rows expose neither swipe actions nor an actions button
//! - pointer-only surfaces never swipe; they use the actions button
//!
//! `SwipeRowHost` records its reveal in `content_offset.x`, which `reveal`
//! returns; the strip slides toward the logical leading edge in both directions.
//! Closed actions remain mounted for animation but are excluded from input and
//! accessibility. The actions button opens the same actions without a swipe.

use crate::support;
use cove_composition::Scope;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::layout::LayoutDirectionExt;
use cove_composition::{AnyView, DynamicField, View};
use cove_core::scene::{AccessibilityRole, AccessibilityTree, Direction, EventDispatcher, Node};
use cove_core::{Environment, InputCapabilities, Point, State};
use cove_platform::{InputEvent, MouseButton, MouseButtonEvent, MouseMotionEvent};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::data::lists::RowState;
use cove_ui_compose::primitives::data::lists::Selection;
use cove_ui_compose::primitives::data::lists::swipe_list::{SwipeAction, SwipeList};
use cove_ui_compose::primitives::feedback::StatusTone;
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::TextScalerViewExt;

use support::InteractiveScene;

#[derive(Clone)]
struct Harness {
    input: State<InputCapabilities>,
    rtl: State<bool>,
    scale: State<f32>,
    selection: State<Option<i32>>,
    actions: Rc<RefCell<Vec<String>>>,
    enabled: State<bool>,
    count: usize,
}

impl Harness {
    fn new(input: InputCapabilities) -> Self {
        Self {
            input: State::new(input),
            rtl: State::new(false),
            scale: State::new(1.0),
            selection: State::new(Some(0)),
            actions: Rc::new(RefCell::new(Vec::new())),
            enabled: State::new(true),
            count: 12,
        }
    }

    fn log(&self, value: &str) {
        self.actions.borrow_mut().push(value.to_string());
    }
}

impl View for Harness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let actions = self.actions.clone();
        let log_archive = actions.clone();
        let log_delete = actions.clone();
        let selection = self.selection.clone();
        let enabled = self.enabled.get();
        let count = self.count;
        let list = SwipeList::new(
            (0..count as i32).collect::<Vec<i32>>(),
            |item: &i32| *item,
            |item: &i32, _: RowState| AnyView::new(Text::new(format!("Item {}", item + 1))),
        )
        .actions(move |item: &i32| {
            let index = *item;
            let archive = log_archive.clone();
            let delete = log_delete.clone();
            vec![
                SwipeAction::new(index as u64 * 10 + 1, "Archive", move || {
                    archive.borrow_mut().push(format!("archive-{index}"));
                }),
                SwipeAction::new(index as u64 * 10 + 2, "Delete", move || {
                    delete.borrow_mut().push(format!("delete-{index}"));
                })
                .tone(StatusTone::Danger),
            ]
        })
        .selection(Selection::single(selection.binding()))
        .is_enabled(enabled)
        .row_label(|item: &i32| format!("Item {}", item + 1));
        let mut environment = Environment {
            input: self.input.get(),
            ..Environment::default()
        };
        environment.viewport.width = 360.0;
        AnyView::new(
            LayerRoot::new(list.frame_size(320.0, 400.0))
                .surface_environment(environment)
                .layout_direction(if self.rtl.get() {
                    Direction::Rtl
                } else {
                    Direction::Ltr
                })
                .text_scaler(cove_ui_compose::theme::TextScaler::new(self.scale.get())),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("input", &self.input),
            DynamicField::state("rtl", &self.rtl),
            DynamicField::state("scale", &self.scale),
            DynamicField::state("selection", &self.selection),
            DynamicField::state("enabled", &self.enabled),
        ]
    }
}

fn row(scene: &InteractiveScene, label: &str) -> Node {
    scene
        .first(|node| {
            node.accessibility().is_some_and(|semantics| {
                semantics.role == AccessibilityRole::ListItem && semantics.label == label
            })
        })
        .unwrap_or_else(|| panic!("no list row {label:?}"))
}

fn ancestor_row(node: &Node) -> Option<Node> {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if candidate
            .accessibility()
            .is_some_and(|semantics| semantics.role == AccessibilityRole::ListItem)
        {
            return Some(candidate);
        }
        current = candidate.parent();
    }
    None
}

fn accessible_action_labels(elements: &[cove_core::scene::AccessibilityElement]) -> Vec<String> {
    let mut result = Vec::new();
    for element in elements {
        if ["Archive", "Delete"].contains(&element.semantics.label.as_str()) {
            result.push(element.semantics.label.clone());
        }
        result.extend(accessible_action_labels(&element.children));
    }
    result
}

fn row_buttons(scene: &InteractiveScene, label: &str) -> Vec<Node> {
    scene
        .all(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
        })
        .into_iter()
        .filter(|button| {
            ancestor_row(button)
                .and_then(|row| row.accessibility())
                .is_some_and(|semantics| semantics.label == label)
        })
        .collect()
}

fn pointer(scene: &InteractiveScene, at: Point, phase: PointerPhase) {
    let dispatcher = scene.dispatcher();
    let event = MouseButtonEvent {
        button: MouseButton::Left,
        x: at.x,
        y: at.y,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    };
    match phase {
        PointerPhase::Down => {
            dispatcher.dispatch(&InputEvent::MouseButtonDown(event));
        }
        PointerPhase::Up => {
            dispatcher.dispatch(&InputEvent::MouseButtonUp(event));
        }
    }
}

fn motion(scene: &InteractiveScene, at: Point, dx: f32, dy: f32) {
    scene
        .dispatcher()
        .dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
            x: at.x + dx,
            y: at.y + dy,
            delta_x: dx,
            delta_y: dy,
            kind: Default::default(),
        }));
}

#[derive(Clone, Copy)]
enum PointerPhase {
    Down,
    Up,
}

fn swipe(scene: &InteractiveScene, target: &Node, dx: f32) {
    let frame = target.absolute_frame();
    let start = Point::new(frame.x + frame.width / 2.0, frame.y + frame.height / 2.0);
    pointer(scene, start, PointerPhase::Down);
    motion(scene, start, dx, 0.0);
    pointer(scene, start, PointerPhase::Up);
    scene.settle(6, 0.05);
}

fn drag_vertically(scene: &InteractiveScene, target: &Node, dy: f32) {
    let frame = target.absolute_frame();
    let start = Point::new(frame.x + frame.width / 2.0, frame.y + frame.height / 2.0);
    pointer(scene, start, PointerPhase::Down);
    motion(scene, start, 0.0, dy);
    pointer(scene, start, PointerPhase::Up);
    scene.settle(6, 0.05);
}

fn reveal(scene: &InteractiveScene, label: &str) -> f32 {
    row(scene, label).content_offset().x
}

fn toggle(scene: &InteractiveScene, label: &str) -> Node {
    row_buttons(scene, label)
        .into_iter()
        .min_by(|left, right| {
            left.frame()
                .width
                .partial_cmp(&right.frame().width)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or_else(|| panic!("no actions button in {label:?}"))
}

fn click(scene: &InteractiveScene, node: &Node) {
    let frame = node.absolute_frame();
    let point = Point::new(frame.x + frame.width / 2.0, frame.y + frame.height / 2.0);
    let dispatcher: EventDispatcher = scene.dispatcher();
    let event = MouseButtonEvent {
        button: MouseButton::Left,
        x: point.x,
        y: point.y,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    };
    dispatcher.dispatch(&InputEvent::MouseButtonDown(event.clone()));
    dispatcher.dispatch(&InputEvent::MouseButtonUp(event));
    scene.settle(6, 0.05);
}

#[test]
fn swipe_action() {
    InteractiveScene::run(360.0, 460.0, |scene| {
        let harness = Harness::new(InputCapabilities::TOUCH | InputCapabilities::KEYBOARD);
        scene.install(harness.clone());
        let first = row(scene, "Item 1");
        assert!(
            first.frame().height >= 44.0,
            "a touch row reaches the 44pt control extent; measured {}",
            first.frame().height
        );
        assert!(
            (first.frame().width - 320.0).abs() < 0.5,
            "the row spans the list width; measured {}",
            first.frame().width
        );

        swipe(scene, &first, -140.0);
        let opened = reveal(scene, "Item 1");
        assert!(
            opened >= 70.0,
            "the left swipe reveals the trailing actions; reveal {opened}"
        );
        assert!(
            scene.capture.target().is_none(),
            "the swipe releases pointer capture once it settles"
        );

        let buttons = row_buttons(scene, "Item 1");
        assert!(
            !buttons.is_empty(),
            "the revealed row exposes its action buttons"
        );
        for button in &buttons {
            assert!(
                button.frame().height >= 44.0,
                "an action is a 44pt touch target; measured {}",
                button.frame().height
            );
        }
        if let Some(activate) = buttons[0].accessibility_actions().activate {
            activate();
        }
        scene.settle(6, 0.05);
        assert!(
            harness
                .actions
                .borrow()
                .first()
                .is_some_and(|value| value.starts_with("archive") || value.starts_with("delete")),
            "activating the first action runs its handler; ran {:?}",
            harness.actions.borrow()
        );
    });
}

#[test]
fn mirrored_actions() {
    for rtl in [false, true] {
        for scale in [1.0_f32, 2.0] {
            InteractiveScene::run(360.0, 460.0, |scene| {
                let harness = Harness::new(InputCapabilities::TOUCH | InputCapabilities::KEYBOARD);
                harness.rtl.set(rtl);
                harness.scale.set(scale);
                scene.install(harness.clone());
                let first = row(scene, "Item 1");
                let width = first.frame().width;
                assert!(
                    (width - 320.0).abs() < 0.5,
                    "rtl={rtl} scale={scale}: the row spans the list width; measured {width}"
                );

                // Swift swipes by `direction == .rtl ? 140 : -140` and then by
                // the negation, so the reveal always follows the logical leading
                // edge (`SwipeListTests.swift:129`).
                let delta = if rtl { 140.0 } else { -140.0 };
                swipe(scene, &first, -delta);
                assert_eq!(
                    reveal(scene, "Item 1"),
                    0.0,
                    "rtl={rtl} scale={scale}: a drag toward the trailing edge never reveals"
                );

                swipe(scene, &first, delta);
                // `content_offset.x` runs the same physical way as the gesture,
                // so the sign follows the writing direction and the open amount
                // is its magnitude (`SwipeList.swift:298`).
                let opened = reveal(scene, "Item 1");
                let opened = if rtl { -opened } else { opened };
                assert!(
                    opened >= 70.0,
                    "rtl={rtl} scale={scale}: a drag toward the leading edge reveals; reveal {opened}"
                );
                assert!(
                    scene.capture.target().is_none(),
                    "rtl={rtl} scale={scale}: the mirrored swipe releases capture"
                );
                let buttons = row_buttons(scene, "Item 1");
                assert!(
                    !buttons.is_empty(),
                    "rtl={rtl} scale={scale}: the mirrored row exposes its action buttons"
                );

                assert!(
                    harness.actions.borrow().is_empty(),
                    "rtl={rtl}: swiping must not activate actions {:?}",
                    harness.actions.borrow()
                );

                let actions_before = harness.actions.borrow().len();
                click(scene, &toggle(scene, "Item 1"));
                assert_eq!(
                    harness.actions.borrow().len(),
                    actions_before,
                    "rtl={rtl}: the actions button only expands the row; ran {:?}",
                    harness.actions.borrow()
                );
            });
        }
    }
}

#[test]
fn actions_menu() {
    InteractiveScene::run(360.0, 460.0, |scene| {
        let harness = Harness::new(InputCapabilities::TOUCH | InputCapabilities::KEYBOARD);
        scene.install(harness.clone());
        assert!(
            !row_buttons(scene, "Item 2").is_empty(),
            "every row mounts its actions button"
        );
        click(scene, &toggle(scene, "Item 2"));
        assert!(
            reveal(scene, "Item 2") > 0.0,
            "the actions button expands the row's actions"
        );
        for title in ["Item 1", "Item 3"] {
            assert!(
                accessible_action_labels(&AccessibilityTree::snapshot(&row(scene, title)))
                    .is_empty(),
                "closed rows omit hidden actions from accessibility"
            );
        }
        let delete = row_buttons(scene, "Item 2")
            .into_iter()
            .find(|node| AccessibilityTree::resolved_label(node) == "Delete")
            .expect("the expanded row offers Delete");
        (delete
            .accessibility_actions()
            .activate
            .expect("Delete exposes activate"))();
        scene.settle(6, 0.05);
        assert_eq!(*harness.actions.borrow(), vec!["delete-1".to_string()]);
    });
}

#[test]
fn vertical_drag() {
    InteractiveScene::run(360.0, 460.0, |scene| {
        let harness = Harness::new(InputCapabilities::TOUCH | InputCapabilities::KEYBOARD);
        scene.install(harness);
        let fourth = row(scene, "Item 4");
        assert_eq!(
            reveal(scene, "Item 4"),
            0.0,
            "the row starts closed: {}",
            reveal(scene, "Item 4")
        );
        drag_vertically(scene, &fourth, -90.0);
        assert_eq!(
            reveal(scene, "Item 4"),
            0.0,
            "a vertical drag reveals no actions: {}",
            reveal(scene, "Item 4")
        );
        let scroll = scene
            .first(|node| node.scroll_reveal().is_some())
            .expect("scroll view");
        assert!(
            scroll.content_offset().y > 0.0,
            "the vertical drag scrolls the list; offset {}",
            scroll.content_offset().y
        );
    });
}

#[test]
fn disabled() {
    InteractiveScene::run(360.0, 460.0, |scene| {
        let mut harness = Harness::new(InputCapabilities::TOUCH | InputCapabilities::KEYBOARD);
        harness.enabled.set(false);
        harness.count = 4;
        scene.install(harness);
        let first = row(scene, "Item 1");
        swipe(scene, &first, -140.0);
        assert_eq!(
            reveal(scene, "Item 1"),
            0.0,
            "a disabled row never reveals actions: {}",
            reveal(scene, "Item 1")
        );
        assert!(
            row_buttons(scene, "Item 1").is_empty(),
            "a disabled row exposes no buttons; measured {}",
            row_buttons(scene, "Item 1").len()
        );
    });
}

#[test]
fn pointer_does_not_swipe() {
    InteractiveScene::run(360.0, 460.0, |scene| {
        let harness = Harness::new(InputCapabilities::POINTER | InputCapabilities::KEYBOARD);
        scene.install(harness.clone());
        let first = row(scene, "Item 1");
        swipe(scene, &first, -140.0);
        assert_eq!(
            reveal(scene, "Item 1"),
            0.0,
            "a pointer-only swipe reveals no actions: {}",
            reveal(scene, "Item 1")
        );
        assert!(
            !row_buttons(scene, "Item 1").is_empty(),
            "a pointer-only row still exposes its actions button"
        );
        let actions_before = harness.actions.borrow().len();
        click(scene, &toggle(scene, "Item 1"));
        assert_eq!(
            harness.actions.borrow().len(),
            actions_before,
            "the pointer actions button only expands the row; ran {:?}",
            harness.actions.borrow()
        );
        assert!(
            !row_buttons(scene, "Item 1").is_empty(),
            "the pointer actions button stays mounted after the press"
        );
    });
}
