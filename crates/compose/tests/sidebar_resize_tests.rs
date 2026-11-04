//! Translation of `Tests/CoveUIComposeTests/Adaptive/SidebarResizeTests.swift`.
//!
//! Behaviour checklist:
//! - `SidebarResizeOptions::validate` repairs each bound independently and a
//!   fixed range exposes no useless handle
//! - a sidebar that fits can still fold to a rail; one the window forces into a
//!   rail stays forced
//! - an explicitly bounded sidebar keeps its height and never collapses the
//!   adjacent story content
//! - width fitting follows the container, the input modality and the layout
//!   direction while the document, the focus, the selection and the authored
//!   width are retained
//! - dragging captures outside the lane, a right button captures nothing,
//!   Escape restores the starting width and the keys share the resolved bounds
//! - RTL dragging grows leftward; shape, mode, input disabling and teardown all
//!   release pointer capture
//! - temporary constraints and invalid requests never overwrite the authored
//!   width, and a bounded slider stays discoverable
//!
//!
//! Harness notes:
//! - Rows are found by accessible label; the Rust `Sidebar` publishes no
//!   `sidebar-item-*` debug names.
//! - The test lane keeps the navigation slot inside one stable container: when
//!   a sibling's modifier chain changed shape (framed `Sidebar` to bare
//!   `SidebarToggle`) the positional reconciler reused nodes across the switch
//!   and the following document subtree went missing.

use crate::support;
use cove_composition::Scope;

use cove_composition::layout::LayoutDirectionExt;
use cove_composition::{AnyView, EmptyView, LayoutDebugExt, Scancode, TupleView, View};
use cove_core::scene::{
    AccessibilityRole, Align, Direction, ElementId, FlexDirection, Justify, Node, Wrap,
};
use cove_core::{Binding, Environment, InputCapabilities, Point, State};
use cove_platform::{InputEvent, KeyModifiers, MouseButton, MouseButtonEvent, MouseMotionEvent};
use cove_ui_compose::BoxView;
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::interaction::HitTestingExt;
use cove_ui_compose::modifiers::layout::{FlexExt, FrameDimension, LayoutFrameExt};
use cove_ui_compose::primitives::layout::ScrollView;
use cove_ui_compose::primitives::layout::sidebar::Sidebar;
use cove_ui_compose::primitives::layout::sidebar::{
    AdaptiveSidebar, AdaptiveSidebarOptions, SidebarGeometry, SidebarItem, SidebarLayoutPolicy,
    SidebarResizeHandle, SidebarResizeOptions, SidebarSection, SidebarToggle,
};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::TextField;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::styles::circular_appearance::ThemeTransition;
use cove_ui_compose::theme::{ReducedMotionViewExt, TextScaler, TextScalerViewExt};
use support::InteractiveScene;

fn items() -> Vec<SidebarItem<i32>> {
    vec![
        SidebarItem::new(0, "Overview"),
        SidebarItem::new(1, "Documents"),
    ]
}

fn selected_binding(selected: &State<Option<i32>>) -> Binding<Option<i32>> {
    Binding::new(
        {
            let selected = selected.clone();
            move || selected.get()
        },
        {
            let selected = selected.clone();
            move |value| selected.set(value)
        },
    )
}

fn width_binding(width: &State<f32>) -> Binding<f32> {
    Binding::new(
        {
            let width = width.clone();
            move || width.get()
        },
        {
            let width = width.clone();
            move |value| width.set(value)
        },
    )
}

fn shell_policy(shows_rail_labels: bool, always_expanded: bool) -> SidebarLayoutPolicy {
    let mut policy = SidebarLayoutPolicy::default();
    policy.shows_rail_labels = shows_rail_labels;
    if always_expanded {
        policy.compact_maximum_width = 0.0;
        policy.expanded_minimum_width = 0.0;
    }
    policy
}

fn subtree(root: &Node) -> Vec<Node> {
    let mut collected = vec![root.clone()];
    fn walk(node: &Node, into: &mut Vec<Node>) {
        for child in node.children() {
            into.push(child.clone());
            walk(&child, into);
        }
    }
    walk(root, &mut collected);
    collected
}

fn label(node: &Node) -> String {
    node.accessibility()
        .map(|semantics| semantics.label)
        .unwrap_or_default()
}

fn role_of(node: &Node) -> Option<AccessibilityRole> {
    node.accessibility().map(|semantics| semantics.role)
}

fn id(node: &Node) -> ElementId {
    node.id()
}

fn resize_lane(scene: &InteractiveScene) -> Option<Node> {
    scene.first(|node| {
        node.is_focusable() && label(node) == "Resize navigation" && node.accepts_subtree_input()
    })
}

fn press_down(scene: &InteractiveScene, x: f32, y: f32, button: MouseButton) {
    scene
        .dispatcher()
        .dispatch(&InputEvent::MouseButtonDown(MouseButtonEvent {
            button,
            x,
            y,
            clicks: 1,
            modifiers: KeyModifiers::default(),
            kind: Default::default(),
        }));
}

fn press_up(scene: &InteractiveScene, x: f32, y: f32, button: MouseButton) {
    scene
        .dispatcher()
        .dispatch(&InputEvent::MouseButtonUp(MouseButtonEvent {
            button,
            x,
            y,
            clicks: 1,
            modifiers: KeyModifiers::default(),
            kind: Default::default(),
        }));
}

fn motion(scene: &InteractiveScene, x: f32) {
    scene
        .dispatcher()
        .dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
            x,
            y: 300.0,
            delta_x: 0.0,
            delta_y: 0.0,
            kind: Default::default(),
        }));
}

fn grab(scene: &InteractiveScene, node: &Node) {
    let frame = node.absolute_frame();
    press_down(
        scene,
        frame.x + frame.width / 2.0,
        frame.y + frame.height / 2.0,
        MouseButton::Left,
    );
}

/// The Swift `Document`: a retained draft and a tall scroll view.
#[derive(Clone)]
struct Document {
    text: State<TextBuffer>,
}

impl Document {
    fn new() -> Self {
        Self {
            text: State::new(TextBuffer::new("Retained draft")),
        }
    }
}

impl View for Document {
    fn body(&self, _cx: &Scope) -> AnyView {
        let text = self.text.clone();
        let field = AnyView::new(TextField::new(
            "Draft",
            Binding::new(
                {
                    let text = text.clone();
                    move || text.get()
                },
                {
                    let text = text.clone();
                    move |value| text.set(value)
                },
            ),
        ))
        .debug_name("resize-draft");
        let scroll = AnyView::new(ScrollView::vertical(AnyView::new(
            AnyView::new(Text::new("Content")).frame_height(1200.0),
        )))
        .flex_with(1.0, 1.0, Some(0.0))
        .debug_name("resize-scroll");
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(field),
                    AnyView::new(scroll),
                ])),
            )
            .debug_name("resize-content"),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state("text", &self.text)]
    }
}

/// The Swift `Harness`, driving the production `AdaptiveSidebar` shell.
#[derive(Clone)]
struct Shell {
    requested: State<f32>,
    selected: State<Option<i32>>,
    adjustable: State<bool>,
    blocked: State<bool>,
    scale: State<f32>,
    dark: State<bool>,
    rtl: State<bool>,
    input: State<InputCapabilities>,
    always_expanded: State<bool>,
    document: Document,
}

impl Shell {
    fn new() -> Self {
        Self {
            requested: State::new(240.0),
            selected: State::new(Some(1)),
            adjustable: State::new(true),
            blocked: State::new(false),
            scale: State::new(1.0),
            dark: State::new(false),
            rtl: State::new(false),
            input: State::new(InputCapabilities::POINTER | InputCapabilities::KEYBOARD),
            always_expanded: State::new(false),
            document: Document::new(),
        }
    }
}

impl View for Shell {
    fn body(&self, _cx: &Scope) -> AnyView {
        let mut options = AdaptiveSidebarOptions::default();
        options.layout = shell_policy(false, self.always_expanded.get());
        let sidebar = AdaptiveSidebar::try_new(
            "Workspace",
            selected_binding(&self.selected),
            vec![SidebarSection::new("items", items())],
        )
        .expect("unique sidebar IDs")
        .options(options)
        .content(self.document.clone());
        let sidebar = if self.adjustable.get() {
            sidebar.width(width_binding(&self.requested))
        } else {
            sidebar
        };
        let sidebar = AnyView::new(sidebar).allows_hit_testing(!self.blocked.get());
        let mut surface = Environment::default();
        surface.viewport.width = 1280.0;
        surface.viewport.height = 600.0;
        surface.input = self.input.get();
        AnyView::new(
            AnyView::new(
                cove_ui_compose::primitives::overlays::portal::LayerRoot::new(
                    AnyView::new(sidebar).flex_with(1.0, 1.0, Some(0.0)),
                ),
            )
            .surface_environment(surface)
            .text_scaler(TextScaler::new(self.scale.get()))
            .appearance(
                if self.dark.get() {
                    Appearance::Dark
                } else {
                    Appearance::Light
                },
                ThemeTransition::default(),
            )
            .layout_direction(if self.rtl.get() {
                Direction::Rtl
            } else {
                Direction::Ltr
            })
            .reduced_motion(true),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![
            cove_composition::DynamicField::state("requested", &self.requested),
            cove_composition::DynamicField::state("selected", &self.selected),
            cove_composition::DynamicField::state("adjustable", &self.adjustable),
            cove_composition::DynamicField::state("blocked", &self.blocked),
            cove_composition::DynamicField::state("scale", &self.scale),
            cove_composition::DynamicField::state("dark", &self.dark),
            cove_composition::DynamicField::state("rtl", &self.rtl),
            cove_composition::DynamicField::state("input", &self.input),
            cove_composition::DynamicField::state("alwaysExpanded", &self.always_expanded),
        ]
    }
}

/// The Swift `Harness` with the resize wiring the shell lacks: the lane is the
/// production `SidebarResizeHandle` sized by the production `SidebarGeometry`.
#[derive(Clone)]
struct Lane {
    requested: State<f32>,
    selected: State<Option<i32>>,
    adjustable: State<bool>,
    blocked: State<bool>,
    rtl: State<bool>,
    touch: State<bool>,
    always_expanded: State<bool>,
    viewport: State<f32>,
    document: Document,
}

impl Lane {
    fn new(width: f32) -> Self {
        Self {
            requested: State::new(width),
            selected: State::new(Some(1)),
            adjustable: State::new(true),
            blocked: State::new(false),
            rtl: State::new(false),
            touch: State::new(false),
            always_expanded: State::new(false),
            viewport: State::new(1280.0),
            document: Document::new(),
        }
    }

    fn geometry(&self, viewport: f32) -> SidebarGeometry {
        let policy = shell_policy(false, self.always_expanded.get());
        let requested = self.adjustable.get().then(|| self.requested.get());
        SidebarGeometry::new(
            viewport,
            &policy,
            false,
            false,
            requested,
            &SidebarResizeOptions::default(),
            self.handle_extent(),
        )
    }

    fn handle_extent(&self) -> f32 {
        if self.touch.get() { 44.0 } else { 7.0 }
    }
}

impl View for Lane {
    fn body(&self, _cx: &Scope) -> AnyView {
        let policy = shell_policy(false, self.always_expanded.get());
        let geometry = self.geometry(self.document_frame_width());
        let sidebar = Sidebar::try_new(
            selected_binding(&self.selected),
            vec![SidebarSection::new("items", items())],
        )
        .expect("unique sidebar IDs")
        .presentation(geometry.presentation(policy.shows_rail_labels))
        .minimum_row_height(
            if geometry.mode == cove_ui_compose::primitives::layout::sidebar::SidebarMode::Expanded
            {
                38.0
            } else {
                44.0
            },
        );
        let mut lanes: Vec<AnyView> = Vec::new();
        let navigation = if geometry.mode
            != cove_ui_compose::primitives::layout::sidebar::SidebarMode::Compact
        {
            AnyView::new(AnyView::new(sidebar).frame(
                Some(FrameDimension::Points(geometry.docked_width)),
                Some(FrameDimension::Points(600.0)),
                None,
                None,
                None,
                None,
            ))
        } else {
            AnyView::new(SidebarToggle::new("Open navigation", false, || {}))
        };
        lanes.push(AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                navigation,
            ))
            .debug_name("lane-navigation"),
        ));
        let mut handle = SidebarResizeHandle::new(
            geometry.resize_range,
            geometry.viewport_width,
            geometry.docked_width,
        )
        .extent(geometry.resize_handle_width);
        if self.adjustable.get() {
            handle = handle.value(width_binding(&self.requested));
        }
        lanes.push(AnyView::new(
            AnyView::new(AnyView::new(handle).allows_hit_testing(!self.blocked.get()))
                .debug_name("lane-handle"),
        ));
        lanes.push(AnyView::new(AnyView::new(self.document.clone()).flex_with(
            1.0,
            1.0,
            Some(0.0),
        )));
        let mut surface = Environment::default();
        surface.viewport.width = geometry.viewport_width;
        surface.viewport.height = 600.0;
        surface.input = if self.touch.get() {
            InputCapabilities::TOUCH | InputCapabilities::KEYBOARD
        } else {
            InputCapabilities::POINTER | InputCapabilities::KEYBOARD
        };
        AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(lanes)),
            ))
            .surface_environment(surface)
            .layout_direction(if self.rtl.get() {
                Direction::Rtl
            } else {
                Direction::Ltr
            })
            .reduced_motion(true),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![
            cove_composition::DynamicField::state("requested", &self.requested),
            cove_composition::DynamicField::state("selected", &self.selected),
            cove_composition::DynamicField::state("adjustable", &self.adjustable),
            cove_composition::DynamicField::state("blocked", &self.blocked),
            cove_composition::DynamicField::state("rtl", &self.rtl),
            cove_composition::DynamicField::state("touch", &self.touch),
            cove_composition::DynamicField::state("alwaysExpanded", &self.always_expanded),
            cove_composition::DynamicField::state("viewport", &self.viewport),
        ]
    }
}

impl Lane {
    /// The lane always fills the scene width; the body reads it from the state
    /// the test mutates through `InteractiveScene::settle`.
    fn document_frame_width(&self) -> f32 {
        self.viewport.get()
    }

    fn set_viewport(&self, scene: &mut InteractiveScene, width: f32) {
        self.viewport.set(width);
        scene.width = width;
        scene.settle(8, 1.0);
    }
}

#[test]
fn limits() {
    let mut limits = SidebarResizeOptions::default();
    limits.minimum_width = f32::NAN;
    limits.maximum_width = f32::INFINITY;
    limits.minimum_content_width = f32::NAN;
    limits.validate();
    assert_eq!(
        (
            limits.minimum_width,
            limits.maximum_width,
            limits.minimum_content_width
        ),
        (180.0, 480.0, 320.0),
        "degenerate bounds fall back to the authored defaults"
    );
    limits.minimum_width = 500.0;
    limits.maximum_width = 100.0;
    limits.minimum_content_width = -20.0;
    limits.validate();
    assert_eq!(
        (limits.maximum_width, limits.minimum_content_width),
        (500.0, 44.0),
        "an inverted maximum follows the minimum and a negative content floor lands on 44"
    );
    let policy = SidebarLayoutPolicy {
        compact_maximum_width: 0.0,
        expanded_minimum_width: 0.0,
        ..SidebarLayoutPolicy::default()
    };
    let fixed = SidebarGeometry::new(1000.0, &policy, false, false, Some(900.0), &limits, 7.0);
    assert_eq!(
        (fixed.resize_handle_width, fixed.docked_width),
        (0.0, 500.0),
        "a range with no room to move exposes no handle and docks at the single legal width"
    );
}

#[test]
fn collapse() {
    let policy = SidebarLayoutPolicy::default();
    let resizing = SidebarResizeOptions::default();
    let wide = SidebarGeometry::new(1280.0, &policy, false, true, Some(260.0), &resizing, 7.0);
    assert_eq!(
        (
            wide.mode,
            wide.is_collapsed,
            wide.resize_handle_width,
            wide.docked_width
        ),
        (
            cove_ui_compose::primitives::layout::sidebar::SidebarMode::Rail,
            true,
            0.0,
            72.0
        ),
        "a sidebar that fits can still fold to a labelled rail"
    );
    let narrow = SidebarGeometry::new(800.0, &policy, false, true, None, &resizing, 7.0);
    assert_eq!(
        (
            narrow.mode,
            narrow.is_collapsed,
            narrow.resize_handle_width,
            narrow.docked_width
        ),
        (
            cove_ui_compose::primitives::layout::sidebar::SidebarMode::Rail,
            false,
            0.0,
            72.0
        ),
        "a rail the window forces stays forced rather than chosen"
    );
    let open = SidebarGeometry::new(1280.0, &policy, false, false, Some(260.0), &resizing, 7.0);
    assert_eq!(
        (open.mode, open.docked_width, open.resize_handle_width),
        (
            cove_ui_compose::primitives::layout::sidebar::SidebarMode::Expanded,
            260.0,
            7.0
        ),
        "a docked sidebar takes the requested width and owns a lane"
    );
}

#[test]
fn embedded_height() {
    InteractiveScene::run(1280.0, 600.0, |scene| {
        let selected = State::new(Some(1_i32));
        let requested = State::new(240.0_f32);
        #[derive(Clone)]
        struct Embedded {
            selected: State<Option<i32>>,
            requested: State<f32>,
        }
        impl View for Embedded {
            fn body(&self, _cx: &Scope) -> AnyView {
                let mut options = AdaptiveSidebarOptions::default();
                options.layout.shows_rail_labels = false;
                options.layout.expanded_minimum_width = 600.0;
                let sidebar = AdaptiveSidebar::try_new(
                    "Embedded",
                    selected_binding(&self.selected),
                    vec![SidebarSection::new("items", items())],
                )
                .expect("unique sidebar IDs")
                .options(options)
                .width(width_binding(&self.requested))
                .content(Document::new());
                let after =
                    AnyView::new(Text::new("After navigation")).debug_name("after-navigation");
                AnyView::new(
                    AnyView::new(
                        cove_ui_compose::primitives::overlays::portal::LayerRoot::new(
                            AnyView::new(BoxView::new(
                                FlexDirection::Column,
                                Align::FlexStart,
                                Justify::FlexStart,
                                Wrap::NoWrap,
                                12.0,
                                AnyView::new(TupleView::new(vec![
                                    AnyView::new(
                                        AnyView::new(sidebar)
                                            .frame(
                                                Some(FrameDimension::Points(720.0)),
                                                Some(FrameDimension::Points(220.0)),
                                                None,
                                                None,
                                                None,
                                                None,
                                            )
                                            .debug_name("bounded-sidebar"),
                                    ),
                                    AnyView::new(after),
                                ])),
                            )),
                        ),
                    )
                    .frame(
                        Some(FrameDimension::Points(1280.0)),
                        Some(FrameDimension::Points(600.0)),
                        None,
                        None,
                        None,
                        None,
                    )
                    .surface_environment(Environment {
                        viewport: cove_core::Viewport {
                            width: 1280.0,
                            height: 600.0,
                            ..Default::default()
                        },
                        input: InputCapabilities::TOUCH | InputCapabilities::KEYBOARD,
                        ..Default::default()
                    })
                    .text_scaler(TextScaler::new(2.0)),
                )
            }
            fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
                vec![
                    cove_composition::DynamicField::state("selected", &self.selected),
                    cove_composition::DynamicField::state("requested", &self.requested),
                ]
            }
        }
        scene.install(Embedded {
            selected: selected.clone(),
            requested: requested.clone(),
        });
        let shell = scene
            .named("bounded-sidebar")
            .expect("the bounded shell mounts");
        assert!(
            (shell.frame().height - 220.0).abs() < 1.0,
            "an explicit height survives the adaptive shell, saw {}",
            shell.frame().height
        );
        let lane = resize_lane(scene).expect("the bounded shell mounts a resize lane");
        assert!(
            (lane.frame().height - 220.0).abs() < 1.0,
            "the resize lane shares the bounded height, saw {}",
            lane.frame().height
        );
        let after = scene
            .named("after-navigation")
            .expect("the sibling story content mounts");
        let shell_frame = shell.absolute_frame();
        assert!(
            after.absolute_frame().y >= shell_frame.y + shell_frame.height + 11.0,
            "the navigation cannot collapse the adjacent content, shell {shell_frame:?}, after {:?}",
            after.absolute_frame()
        );
        let overview = scene
            .first(|node| node.is_focusable() && label(node) == "Overview")
            .expect("the first destination mounts");
        let documents = scene
            .first(|node| node.is_focusable() && label(node) == "Documents")
            .expect("the second destination mounts");
        assert!(
            documents.absolute_frame().y
                >= overview.absolute_frame().y + overview.absolute_frame().height,
            "rows stack instead of overlapping, overview {:?}, documents {:?}",
            overview.absolute_frame(),
            documents.absolute_frame()
        );
        assert_eq!(
            selected.get(),
            Some(1),
            "the bounded shell never rewrites the caller's selection"
        );
        assert_eq!(requested.get(), 240.0, "and never the authored width");
    });
}

#[test]
fn shell_matrix() {
    InteractiveScene::run(1280.0, 600.0, |scene| {
        let shell = Shell::new();
        scene.install(shell.clone());
        shell.requested.set(320.0);
        scene.settle(8, 1.0);
        let document = scene.named("resize-content").expect("the page mounts");
        let field = subtree(&scene.named("resize-draft").expect("draft"))
            .into_iter()
            .find(|node| role_of(node) == Some(AccessibilityRole::TextField))
            .expect("the draft field is a text field");
        let scroll = scene
            .named("resize-scroll")
            .expect("the scroll view mounts");
        let resizer = resize_lane(scene).expect("the resize lane mounts at 1280");
        scene.focus.focus(Some(&field), false);
        scene
            .dispatcher()
            .dispatch(&InputEvent::TextInput(" edited".to_string()));
        scene.settle(8, 1.0);
        scroll.set_content_offset(Point::new(0.0, 80.0));
        for width in [1280.0_f32, 720.0, 390.0, 1000.0, 1280.0] {
            for scale in [1.0_f32, 2.0] {
                for input in [
                    InputCapabilities::TOUCH,
                    InputCapabilities::TOUCH
                        | InputCapabilities::POINTER
                        | InputCapabilities::KEYBOARD,
                    InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
                ] {
                    for dark in [false, true] {
                        for rtl in [false, true] {
                            scene.width = width;
                            shell.scale.set(scale);
                            shell.input.set(input);
                            shell.dark.set(dark);
                            shell.rtl.set(rtl);
                            scene.settle(8, 1.0);
                            let case = format!(
                                "width {width} scale {scale} input {input:?} dark {dark} rtl {rtl}"
                            );
                            assert_eq!(
                                id(&scene.named("resize-content").expect("page")),
                                id(&document),
                                "the page subtree is retained ({case})"
                            );
                            assert_eq!(
                                scene.focus.focused().map(|node| id(&node)),
                                Some(id(&field)),
                                "the draft keeps focus ({case})"
                            );
                            assert!(
                                field.resolved_accessibility_value().contains("edited"),
                                "the typed draft survives ({case})"
                            );
                            assert_eq!(
                                shell.requested.get(),
                                320.0,
                                "a fitted container never rewrites the authored width ({case})"
                            );
                            assert_eq!(shell.selected.get(), Some(1), "nor the selection ({case})");
                            assert_eq!(
                                scene
                                    .named("resize-scroll")
                                    .expect("scroll")
                                    .content_offset()
                                    .y,
                                80.0,
                                "and never the page scroll position ({case})"
                            );
                            let navigation: f32 = if width >= 1000.0 {
                                320.0
                            } else if width >= 600.0 {
                                56.0
                            } else {
                                0.0
                            };
                            let lane: f32 = if width >= 1000.0 {
                                if input.contains(InputCapabilities::TOUCH) {
                                    44.0
                                } else {
                                    7.0
                                }
                            } else {
                                0.0
                            };
                            let page = scene.named("resize-content").expect("page");
                            let expected_width = width - navigation - lane;
                            let edge = if rtl {
                                page.absolute_frame().max_x() - expected_width
                            } else {
                                page.absolute_frame().x - (navigation + lane)
                            };
                            assert!(
                                (page.frame().width - expected_width).abs() < 1.0
                                    && edge.abs() < 1.0,
                                "the navigation and the resize lane dock beside the page, saw {:?} ({case})",
                                page.absolute_frame()
                            );
                            if width >= 1000.0 {
                                assert_eq!(
                                    resize_lane(scene).map(|node| id(&node)),
                                    Some(id(&resizer)),
                                    "the lane keeps its identity ({case})"
                                );
                                assert!(
                                    (resizer.frame().width - lane).abs() < 1.0
                                        && resizer.frame().height >= 44.0,
                                    "the lane is {lane}pt wide and touch-tall, saw {:?} ({case})",
                                    resizer.frame()
                                );
                                assert_eq!(
                                    resizer.accessibility().map(|semantics| semantics.value),
                                    Some("320 pt".to_string()),
                                    "the lane reports its width ({case})"
                                );
                            } else {
                                assert!(
                                    !resizer.is_focusable()
                                        && !resizer.is_hit_testable()
                                        && resizer.accessibility().is_none(),
                                    "a vanished lane is inert ({case})"
                                );
                            }
                            assert!(
                                !scene.graph.recomposer.has_pending(),
                                "the shell settles without pending work ({case})"
                            );
                        }
                    }
                }
            }
        }
    });
}

#[test]
fn drag_and_keyboard() {
    InteractiveScene::run(1280.0, 600.0, |scene| {
        let lane = Lane::new(240.0);
        scene.install(lane.clone());
        let resize = resize_lane(scene).expect("the lane owns a resize handle");
        let start = {
            let frame = resize.absolute_frame();
            frame.x + frame.width / 2.0
        };
        let middle = {
            let frame = resize.absolute_frame();
            frame.y + frame.height / 2.0
        };
        press_down(scene, start, middle, MouseButton::Right);
        assert!(
            scene.capture.target().is_none(),
            "a secondary button never captures the lane"
        );
        press_down(scene, start, middle, MouseButton::Left);
        assert_eq!(
            scene.capture.target().map(|node| id(&node)),
            Some(id(&resize)),
            "the left button captures the lane"
        );
        assert!(
            !scene.focus.is_focus_visible(),
            "a pointer grab stays quiet for focus"
        );
        motion(scene, start + 100.0);
        scene.settle(8, 1.0);
        assert_eq!(lane.requested.get(), 340.0, "dragging tracks the pointer");
        assert_eq!(
            scene.capture.target().map(|node| id(&node)),
            Some(id(&resize)),
            "capture survives recomposition"
        );
        motion(scene, start - 500.0);
        scene.settle(8, 1.0);
        assert_eq!(lane.requested.get(), 180.0, "and clamps to the low bound");
        scene.key(Scancode::ESCAPE, KeyModifiers::default());
        assert_eq!(
            lane.requested.get(),
            240.0,
            "Escape restores the width the drag started from"
        );
        assert!(scene.capture.target().is_none(), "and releases the capture");
        scene.focus.focus(Some(&resize), true);
        scene.key(Scancode::ARROW_RIGHT, KeyModifiers::default());
        assert_eq!(lane.requested.get(), 256.0, "one arrow is one step");
        scene.key(Scancode::ARROW_RIGHT, KeyModifiers::LEFT_SHIFT);
        assert_eq!(lane.requested.get(), 288.0, "shift doubles the step");
        scene.key(Scancode::ARROW_LEFT, KeyModifiers::default());
        assert_eq!(
            lane.requested.get(),
            272.0,
            "and the reverse direction shrinks"
        );
        scene.key(Scancode::END, KeyModifiers::default());
        assert_eq!(lane.requested.get(), 480.0, "End goes to the high bound");
        scene.key(Scancode::HOME, KeyModifiers::default());
        assert_eq!(lane.requested.get(), 180.0, "Home to the low bound");
        scene.key(Scancode::ARROW_RIGHT, KeyModifiers::LEFT_CTRL);
        assert_eq!(
            lane.requested.get(),
            180.0,
            "a modified arrow belongs to the host, not the lane"
        );
        let actions = resize.accessibility_actions();
        let increment = actions.increment.expect("the slider offers increment");
        increment();
        increment();
        scene.settle(8, 1.0);
        assert_eq!(lane.requested.get(), 212.0, "two semantic steps walk up");
        let decrement = actions.decrement.expect("the slider offers decrement");
        decrement();
        scene.settle(8, 1.0);
        assert_eq!(lane.requested.get(), 196.0, "and down");
        let set_value = actions.set_value.expect("the slider accepts a value");
        set_value("260 pt");
        scene.settle(8, 1.0);
        assert_eq!(lane.requested.get(), 260.0, "a parsed value lands verbatim");
        set_value("nan");
        scene.settle(8, 1.0);
        assert_eq!(
            lane.requested.get(),
            260.0,
            "and a non-finite one is ignored"
        );
        assert_eq!(
            label(&resize),
            "Resize navigation",
            "the lane keeps one addressable node throughout"
        );
    });
}

#[test]
fn lane_lifecycle() {
    InteractiveScene::run(1280.0, 600.0, |scene| {
        let lane = Shell::new();
        lane.requested.set(320.0);
        scene.install(lane.clone());
        lane.rtl.set(true);
        scene.settle(8, 1.0);
        let resize = resize_lane(scene).expect("the lane mounts at 1280");
        let x = {
            let frame = resize.absolute_frame();
            frame.x + frame.width / 2.0
        };
        press_down(scene, x, 300.0, MouseButton::Left);
        assert_eq!(
            scene.capture.target().map(|node| id(&node)),
            Some(id(&resize)),
            "RTL captures the lane like LTR does, handle {:?}",
            resize.absolute_frame()
        );
        motion(scene, x - 48.0);
        scene.settle(8, 1.0);
        assert_eq!(
            lane.requested.get(),
            368.0,
            "RTL dragging leftward grows the navigation"
        );
        scene.width = 720.0;
        scene.settle(8, 1.0);
        assert!(
            scene.capture.target().is_none(),
            "leaving the docked layout releases the capture"
        );
        assert_eq!(
            lane.requested.get(),
            368.0,
            "and never rewrites the authored width"
        );
        assert_eq!(
            scene.focus.focused().map(|node| label(&node)),
            Some("Documents".to_string()),
            "leaving the resize lane hands focus to the selected destination"
        );
        scene.width = 1280.0;
        scene.settle(8, 1.0);
        assert_eq!(
            resize_lane(scene).map(|node| id(&node)),
            Some(id(&resize)),
            "returning to the docked layout restores the same lane node"
        );
        grab(scene, &resize);
        scene.width = 390.0;
        scene.settle(8, 1.0);
        assert!(
            scene.capture.target().is_none(),
            "a compact container releases the lane"
        );
        assert!(
            resize_lane(scene).is_none(),
            "the compact breakpoint removes the lane"
        );
        assert_eq!(
            scene.focus.focused().map(|node| label(&node)),
            Some("Open navigation".to_string()),
            "compact navigation hands focus to its opener"
        );
        scene.width = 1280.0;
        scene.settle(8, 1.0);
        grab(scene, &resize);
        lane.input
            .set(InputCapabilities::TOUCH | InputCapabilities::KEYBOARD);
        scene.settle(8, 1.0);
        assert!(
            scene.capture.target().is_none(),
            "switching the surface to touch drops the capture"
        );
        assert_eq!(
            resize.frame().width,
            44.0,
            "a touch lane reserves the 44pt target"
        );
        grab(scene, &resize);
        lane.blocked.set(true);
        scene.settle(8, 1.0);
        assert!(
            scene.capture.target().is_none(),
            "a disabled subtree never captures"
        );
        assert!(
            !resize.accepts_subtree_input(),
            "and the lane leaves the interaction graph"
        );
        assert!(
            scene.focus.focused().is_none(),
            "a disabled subtree holds no focus, saw {:?}",
            scene.focus.focused().map(|node| label(&node))
        );
        lane.blocked.set(false);
        scene.settle(8, 1.0);
        grab(scene, &resize);
        assert_eq!(
            scene.capture.target().map(|node| id(&node)),
            Some(id(&resize)),
            "re-enabling makes the lane grabbable again"
        );
        lane.adjustable.set(false);
        scene.settle(8, 1.0);
        assert!(
            scene.capture.target().is_none(),
            "a caller-owned width without a binding drops the lane"
        );
        assert!(
            !resize.is_focusable(),
            "and the handle leaves the tab order"
        );
        lane.adjustable.set(true);
        scene.settle(8, 1.0);
        grab(scene, &resize);
        assert_eq!(
            scene.capture.target().map(|node| id(&node)),
            Some(id(&resize)),
            "the lane comes back with the binding"
        );
        scene.graph.install(EmptyView, Default::default());
        scene.graph.recomposer.commit_all();
        assert!(
            scene.capture.target().is_none(),
            "unmounting releases the capture"
        );
        assert!(
            scene.focus.focused().is_none()
                || scene
                    .focus
                    .focused()
                    .is_none_or(|node| id(&node) != id(&resize)),
            "and drops the handle from focus, saw {:?}",
            scene.focus.focused().map(|node| label(&node))
        );
    });
}

#[test]
fn fitting_and_ai() {
    InteractiveScene::run(1280.0, 600.0, |scene| {
        let lane = Lane::new(400.0);
        scene.install(lane.clone());
        lane.always_expanded.set(true);
        lane.set_viewport(scene, 500.0);
        assert_eq!(
            lane.requested.get(),
            400.0,
            "a temporary narrow container never overwrites the authored width"
        );
        let page = scene
            .named("resize-content")
            .expect("the page mounts")
            .frame();
        assert!(
            (page.width - 320.0).abs() < 1.0,
            "the sidebar docks at its floor and leaves 320 of content, saw {}",
            page.width
        );
        assert!(
            scene
                .nodes()
                .into_iter()
                .find(|node| label(node) == "Resize navigation")
                .is_none(),
            "fixed bounds expose no handle"
        );
        lane.set_viewport(scene, 640.0);
        let resize = resize_lane(scene).expect("a movable range exposes the handle");
        assert_eq!(
            lane.requested.get(),
            400.0,
            "fitting clamps the presentation, not the request"
        );
        assert_eq!(
            resize.accessibility().expect("semantics").value,
            "313 pt",
            "the lane reports the width it can actually take"
        );
        lane.set_viewport(scene, 1280.0);
        assert_eq!(
            resize.accessibility().expect("semantics").value,
            "400 pt",
            "and the authored width once everything fits"
        );
        let geometry = lane.geometry(1280.0);
        assert_eq!(
            (geometry.docked_width, geometry.resize_handle_width),
            (400.0, 7.0),
            "the resolved geometry agrees with the lane"
        );
        lane.requested.set(f32::NAN);
        scene.settle(8, 1.0);
        assert!(
            lane.requested.get().is_nan(),
            "an invalid request is left untouched rather than normalised into the caller's width"
        );
        assert_eq!(
            lane.geometry(1280.0).docked_width,
            240.0,
            "the geometry falls back to the policy width"
        );
        assert_eq!(
            resize.accessibility().expect("semantics").value,
            "240 pt",
            "the handle reports the resolved fallback width, not the raw NaN binding"
        );
        let increment = resize
            .accessibility_actions()
            .increment
            .expect("the slider offers increment");
        increment();
        scene.settle(8, 1.0);
        assert_eq!(
            lane.requested.get(),
            256.0,
            "the increment steps from the resolved fallback width"
        );
        let query = |scene: &InteractiveScene, value: &str| {
            scene
                .all(|node| {
                    node.accessibility().is_some_and(|semantics| {
                        semantics.role == AccessibilityRole::Slider && semantics.value == value
                    }) && node.is_layout_visible()
                        && node.accepts_subtree_input()
                })
                .len()
        };
        assert_eq!(
            query(scene, "256 pt"),
            1,
            "discovery finds the slider at its stepped width"
        );
        lane.requested.set(256.0);
        scene.settle(8, 1.0);
        assert_eq!(
            query(scene, "256 pt"),
            1,
            "discovery finds exactly one visible, enabled slider at a valid width"
        );
        let x = {
            let frame = resize.absolute_frame();
            frame.x + frame.width / 2.0
        };
        press_down(scene, x, 300.0, MouseButton::Left);
        press_up(scene, x, 300.0, MouseButton::Left);
        scene.settle(5, 1.0);
        assert!(
            scene.capture.target().is_none(),
            "the press releases on pointer up"
        );
    });
}
