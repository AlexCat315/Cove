//! Translation of `Tests/CoveUIComposeTests/Adaptive/NavigationRailTests.swift`.
//!
//! Behaviour checklist:
//! - rail widths, row targets, names, badge values and icon clamps hold across
//!   theme, input and 200 percent text
//! - header action state, selection, focus, reveal and scrolling survive
//!   presentation swaps
//! - vertical roving skips disabled rows, wraps, and separates browsing from
//!   activation
//! - the icon-only rail paints a selection band, no accent lane, and reacts to
//!   hover
//! - touch activates at the row edge; disabling or unmounting a pressed rail
//!   releases capture and focus
//! - a disabled destination is invisible to input-enabled discovery
//!
//! Named differences from the Swift suite:
//! - Swift reads `UIDiagnostics` and the agent toolbox (CoveUIAgent, not a
//!   Compose dependency). The same contract is asserted directly on the node
//!   tree: role/label/value lookups and enabled-state counts.
//! - Swift rows carry the debug name `sidebar-item-<id>`. The Rust `Sidebar`
//!   publishes none, so rows are found by accessible label — the
//!   `sidebar_tests.rs` precedent.
//! - Swift's rail label wraps a long title over more than one line; the Rust
//!   rail caption wraps too, because it is laid out at the rail's width instead
//!   of being truncated.
//! - Swift's row help text `"Destination 5 (123456)"` is the rail row tooltip;
//!   the Rust rail row publishes the same breadcrumb-plus-badge help string.
//! - Swift's arrow browsing moves focus with the selection. The Rust roving
//!   host moves the selection but leaves focus where it started: `RovingItem`
//!   sits on the non-focusable destination wrapper, so the host never focuses
//!   the new row.

use crate::support;
use cove_composition::Scope;

use std::rc::Rc;

use cove_composition::{AnyView, EmptyView, LayoutDebugExt, TupleView, View, ViewGraph};
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, Node, Wrap};
use cove_core::{Binding, Color, Environment, InputCapabilities, Point, State};
use cove_platform::{
    InputEvent, KeyEvent, KeyModifiers, MouseButton, MouseButtonEvent, MouseMotionEvent,
};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::interaction::AccessibilityExt;
use cove_ui_compose::modifiers::layout::{FlexExt, FrameDimension, LayoutFrameExt};
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::layout::NavigationRail;
use cove_ui_compose::primitives::layout::sidebar::{
    Sidebar, SidebarItem, SidebarPresentation, SidebarSection,
};
use cove_ui_compose::primitives::media::ui_icon_catalog;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::ReducedMotionViewExt;
use cove_ui_compose::theme::{
    SemanticIconSize, TextScaler, TextScalerViewExt, Theme, ThemeViewExt,
};
use cove_ui_compose::{BoxView, Scancode};

use support::InteractiveScene;

const LONG_TITLE: &str = "NotificationsAndWorkspacePermissions";

fn nodes_of(root: &Node) -> Vec<Node> {
    let mut nodes = vec![root.clone()];
    for child in root.children() {
        nodes.extend(nodes_of(&child));
    }
    nodes
}

fn text_of(node: &Node) -> Option<String> {
    node.layout_node()
        .and_then(|layout| layout.text_inputs())
        .map(|inputs| inputs.text)
}

fn mid_x(rect: cove_core::Rect) -> f32 {
    rect.x + rect.width / 2.0
}

fn mid_y(rect: cove_core::Rect) -> f32 {
    rect.y + rect.height / 2.0
}

fn glyph(node: &Node) -> Node {
    // The sized glyph is the `Image` node; the `IconMetricsReader` wrapper
    // around it carries no frame of its own.
    nodes_of(node)
        .into_iter()
        .filter(|node| node.view_tag().is_some_and(|tag| tag.contains("Image")))
        .find(|node| node.frame().width > 0.0)
        .expect("the row mounts its icon glyph")
}

fn items(enabled: bool) -> Vec<SidebarItem<i32>> {
    (0..30)
        .map(|index| {
            let title = if index == 4 {
                LONG_TITLE.to_string()
            } else {
                format!("Destination {index}")
            };
            let mut item = SidebarItem::new(index, title)
                .icon_size(if index == 5 {
                    SemanticIconSize::new(|_| 400.0)
                } else {
                    SemanticIconSize::medium()
                })
                .enabled(enabled && index != 2);
            if index != 3 {
                item = item.icon(ui_icon_catalog::file::folder());
            }
            if index == 5 {
                item = item.badge("123456");
            }
            item
        })
        .collect()
}

/// The Swift `Header`: local state that keeps its identity across presentation
/// swaps and relabels itself on activation.
struct RailHeader {
    count: State<u32>,
}

impl View for RailHeader {
    fn body(&self, _cx: &Scope) -> AnyView {
        let count = self.count.get();
        let state = self.count.clone();
        AnyView::new(
            AnyView::new(Button::new(
                Text::new(format!("H {}", count + 1)),
                move || state.set(state.get() + 1),
            ))
            .accessibility(move |semantics| {
                semantics.label = format!("Header action {count}");
            })
            .frame(None, None, Some(44.0), Some(44.0), None, None)
            .debug_name("rail-header"),
        )
    }
}

struct HarnessView {
    facade: bool,
    presentation: State<SidebarPresentation>,
    selected: State<Option<i32>>,
    width: State<f32>,
    scale: State<f32>,
    dark: State<bool>,
    input: State<InputCapabilities>,
    enabled: State<bool>,
    activations: State<i32>,
    header_count: State<u32>,
}

impl View for HarnessView {
    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![
            cove_composition::DynamicField::state("presentation", &self.presentation),
            cove_composition::DynamicField::state("selected", &self.selected),
            cove_composition::DynamicField::state("width", &self.width),
            cove_composition::DynamicField::state("scale", &self.scale),
            cove_composition::DynamicField::state("dark", &self.dark),
            cove_composition::DynamicField::state("input", &self.input),
            cove_composition::DynamicField::state("enabled", &self.enabled),
            cove_composition::DynamicField::state("activations", &self.activations),
            cove_composition::DynamicField::state("header_count", &self.header_count),
        ]
    }

    fn body(&self, _cx: &Scope) -> AnyView {
        let width = self.width.get();
        let presentation = self.presentation.get();
        let mut surface = Environment::default();
        surface.viewport.width = width;
        surface.input = self.input.get();

        let selection = Binding::new(
            {
                let selected = self.selected.clone();
                move || selected.get()
            },
            {
                let selected = self.selected.clone();
                move |value: Option<i32>| selected.set(value)
            },
        );
        let on_activate = {
            let activations = self.activations.clone();
            move |_: i32| activations.set(activations.get() + 1)
        };
        let header = RailHeader {
            count: self.header_count.clone(),
        };
        let navigation = if self.facade {
            AnyView::new(
                NavigationRail::try_new(selection, items(self.enabled.get()))
                    .expect("unique rail IDs")
                    .shows_labels(presentation.shows_labels())
                    .on_activate(on_activate)
                    .header(header)
                    .footer(Text::new("End")),
            )
        } else {
            AnyView::new(
                Sidebar::try_new(
                    selection,
                    vec![SidebarSection::new("main", items(self.enabled.get())).title("Workspace")],
                )
                .expect("unique sidebar IDs")
                .presentation(presentation)
                .on_activate(on_activate)
                .header(header)
                .footer(Text::new("End")),
            )
        };

        let lane = AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(AnyView::new(navigation).flex_with(1.0, 1.0, Some(0.0))),
            ))
            .flex_with(0.0, 0.0, None),
        );
        // One frame modifier in both branches: swapping modifier *types* here
        // would remount the lane on every presentation change. The width only
        // applies in expanded mode (the Swift scaffold frames the pane at 240);
        // rail mode leaves the lane `Auto`.
        let lane = AnyView::new(
            AnyView::new(lane)
                .frame(
                    if !self.facade && !presentation.is_rail() {
                        Some(FrameDimension::Points(240.0))
                    } else {
                        Some(FrameDimension::Auto)
                    },
                    None,
                    None,
                    None,
                    None,
                    None,
                )
                .debug_name("rail-navigation"),
        );
        let content = AnyView::new(
            AnyView::new(EmptyView)
                .flex_with(1.0, 1.0, Some(0.0))
                .debug_name("rail-content"),
        );
        AnyView::new(
            AnyView::new(
                AnyView::new(BoxView::new(
                    FlexDirection::Row,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(TupleView::new(vec![lane, content])),
                ))
                .flex_with(1.0, 1.0, Some(0.0)),
            )
            .frame_size(width, 420.0)
            .surface_environment(surface)
            .text_scaler(TextScaler::new(self.scale.get()))
            .theme(if self.dark.get() {
                Theme::default_dark()
            } else {
                Theme::default_light()
            })
            .reduced_motion(true),
        )
    }
}

#[derive(Clone)]
struct States {
    presentation: State<SidebarPresentation>,
    selected: State<Option<i32>>,
    width: State<f32>,
    scale: State<f32>,
    dark: State<bool>,
    input: State<InputCapabilities>,
    enabled: State<bool>,
    activations: State<i32>,
    header_count: State<u32>,
}

impl States {
    fn new() -> Self {
        Self {
            presentation: State::new(SidebarPresentation::Rail { shows_labels: true }),
            selected: State::new(Some(29)),
            width: State::new(720.0_f32),
            scale: State::new(1.0_f32),
            dark: State::new(false),
            input: State::new(InputCapabilities::POINTER | InputCapabilities::KEYBOARD),
            enabled: State::new(true),
            activations: State::new(0_i32),
            header_count: State::new(0_u32),
        }
    }

    fn harness(&self, facade: bool) -> HarnessView {
        HarnessView {
            facade,
            presentation: self.presentation.clone(),
            selected: self.selected.clone(),
            width: self.width.clone(),
            scale: self.scale.clone(),
            dark: self.dark.clone(),
            input: self.input.clone(),
            enabled: self.enabled.clone(),
            activations: self.activations.clone(),
            header_count: self.header_count.clone(),
        }
    }
}

fn row(scene: &InteractiveScene, index: i32) -> Node {
    // The authored row-4 title is the long single-word string; every other row
    // is "Destination <index>", matching the Swift harness.
    let title = if index == 4 {
        LONG_TITLE.to_string()
    } else {
        format!("Destination {index}")
    };
    scene
        .labelled(&title, Some(AccessibilityRole::Button))
        .unwrap_or_else(|| panic!("row {index} is mounted"))
}

#[test]
fn rail_geometry_holds_across_theme_density_and_text() {
    let states = States::new();
    InteractiveScene::run(720.0, 420.0, |scene| {
        scene.install(states.harness(true));
        let retained = row(scene, 0);
        for scale in [1.0_f32, 2.0] {
            for input in [
                InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
                InputCapabilities::TOUCH,
                InputCapabilities::TOUCH | InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
            ] {
                for dark in [false, true] {
                    for labels in [true, false] {
                        for width in [390.0_f32, 720.0, 1280.0] {
                            states.width.set(width);
                            states.scale.set(scale);
                            states.input.set(input);
                            states.dark.set(dark);
                            states.presentation.set(SidebarPresentation::Rail {
                                shows_labels: labels,
                            });
                            scene.width = width;
                            scene.settle(4, 1.0);

                            let nav = scene
                                .named("rail-navigation")
                                .expect("the navigation lane is mounted");
                            let nav_width = nav.frame().width;
                            assert!(
                                nav_width.is_finite() && nav_width > 0.0,
                                "the rail measures {nav_width} at scale {scale}, labels {labels}"
                            );
                            let rail_width = if labels { 72.0 } else { 56.0 };
                            assert_eq!(nav_width, rail_width, "the rail uses its authored width");
                            assert_eq!(
                                row(scene, 0).id(),
                                retained.id(),
                                "destination identity survives the swap"
                            );
                            let current = row(scene, 5);
                            let row_width = current.frame().width;
                            let row_glyph_width = glyph(&current).frame().width;
                            assert_eq!(
                                row_width, rail_width,
                                "the rail row fills the {rail_width} rail, glyph {row_glyph_width}"
                            );
                            assert!(
                                current.frame().height >= 44.0,
                                "rail rows floor at the 44pt touch target, saw {}",
                                current.frame().height
                            );
                            let semantics = current
                                .accessibility()
                                .expect("the row publishes semantics");
                            assert_eq!(semantics.label, "Destination 5");
                            assert_eq!(
                                semantics.value, "123456",
                                "the badge lands on the accessibility value"
                            );
                            assert_eq!(
                                semantics.help, "Destination 5 (123456)",
                                "the rail row composes the breadcrumb plus badge help string"
                            );

                            let glyph = glyph(&current);
                            let clamp = rail_width - 16.0;
                            assert!(
                                glyph.frame().width <= clamp + 0.5,
                                "the glyph clamps to railWidth - 16 ({clamp}), saw {}",
                                glyph.frame().width
                            );
                            let bounds = glyph.absolute_frame();
                            let lane_bounds = nav.absolute_frame();
                            assert!(
                                bounds.x >= lane_bounds.x - 0.5
                                    && bounds.max_x() <= lane_bounds.max_x() + 0.5,
                                "the glyph stays inside the rail: glyph [{}, {}], rail [{}, {}]",
                                bounds.x,
                                bounds.max_x(),
                                lane_bounds.x,
                                lane_bounds.max_x(),
                            );

                            let title = nodes_of(&row(scene, 4))
                                .into_iter()
                                .find(|node| text_of(node).as_deref() == Some(LONG_TITLE));
                            if labels {
                                let text = title.expect("the rail renders the title label");
                                let layout =
                                    text.layout_node().expect("the text node has a layout");
                                let measured =
                                    layout.text_measure().expect("the title records a measure");
                                assert!(text.frame().width > 0.0);
                                assert!(
                                    text.absolute_frame().max_x()
                                        <= nav.absolute_frame().max_x() + 0.5,
                                    "the caption stays inside the rail at scale {scale}"
                                );
                                assert!(
                                    measured.result.lines.len() > 1,
                                    "the long title wraps onto more than one line, saw {}",
                                    measured.result.lines.len()
                                );
                                for line in &measured.result.lines {
                                    assert!(line.width <= text.frame().width + 1.0);
                                }
                            } else {
                                assert!(title.is_none(), "the icon-only rail drops the title text");
                            }

                            let reported: Vec<Node> = scene.all(|node| {
                                node.accessibility().is_some_and(|semantics| {
                                    semantics.role == AccessibilityRole::Button
                                        && semantics.label == LONG_TITLE
                                })
                            });
                            assert_eq!(
                                reported.len(),
                                1,
                                "the long title is discoverable exactly once, no \
                                 decorative duplicates"
                            );
                            assert!(
                                !scene.graph.recomposer.has_pending(),
                                "the settle converged at scale {scale}, labels {labels}"
                            );
                        }
                    }
                }
            }
        }
    });
}

#[test]
fn presentations_retain_identity_header_focus_and_visibility() {
    let states = States::new();
    states.selected.set(Some(29));
    InteractiveScene::run(720.0, 420.0, |scene| {
        scene.install(states.harness(false));
        let header = scene
            .named("rail-header")
            .expect("the rail header is mounted");
        header
            .accessibility_actions()
            .activate
            .expect("the header is activatable")();
        scene.settle(4, 1.0);
        let selected = row(scene, 29);
        scene.focus.focus(Some(&selected), true);
        let mut tracked = selected.clone();

        for mode in [
            SidebarPresentation::Expanded,
            SidebarPresentation::Rail {
                shows_labels: false,
            },
            SidebarPresentation::Rail { shows_labels: true },
            SidebarPresentation::Expanded,
            SidebarPresentation::Rail { shows_labels: true },
            SidebarPresentation::Rail {
                shows_labels: false,
            },
        ] {
            states.presentation.set(mode);
            scene.settle(8, 1.0);
            let current = row(scene, 29);
            let current_header = scene
                .named("rail-header")
                .expect("the header survives the presentation swap");
            assert_eq!(current.id(), tracked.id(), "mode {mode:?} retains the row");
            assert!(
                scene
                    .focus
                    .focused()
                    .is_some_and(|focused| focused.id() == tracked.id()),
                "mode {mode:?} keeps row focus"
            );
            tracked = current.clone();
            assert_eq!(
                current_header.id(),
                header.id(),
                "the header node is retained"
            );
            assert_eq!(
                header.accessibility().map(|semantics| semantics.label),
                Some("Header action 1".to_string()),
                "the header keeps its local activation state across modes"
            );
            let nav = scene
                .named("rail-navigation")
                .expect("the navigation lane is mounted");
            match mode.rail_width() {
                Some(rail) => {
                    assert_eq!(
                        nav.frame().width,
                        rail,
                        "the navigation lane follows the presentation width"
                    );
                }
                None => {
                    assert_eq!(
                        nav.frame().width,
                        240.0,
                        "the expanded lane keeps the authored 240pt width"
                    );
                }
            }
            let scroller = scene
                .first(|node| node.scroll_reveal().is_some())
                .expect("the rail body is scrollable");
            let viewport = scroller.absolute_frame();
            let frame = current.absolute_frame();            let visible = frame.y.max(viewport.y)..frame.max_y().min(viewport.max_y());
            let visible_height = (visible.end - visible.start).max(0.0);
            assert!(
                visible_height >= frame.height - 1.0,
                "the selected row is fully revealed inside the viewport \
                 (visible {visible_height} of {})",
                frame.height,
            );
            assert_eq!(states.selected.get(), Some(29));
            assert_eq!(
                states.activations.get(),
                0,
                "presentation churn must not fire activation"
            );
        }

        let scroller = scene
            .first(|node| node.scroll_reveal().is_some())
            .expect("the rail body is scrollable");
        scroller.set_content_offset(Point { x: 0.0, y: 0.0 });
        scene.settle(4, 1.0);
        assert_eq!(
            scroller.content_offset().y,
            0.0,
            "ordinary scrolling must not be pulled back to selection"
        );
    });
}

#[test]
fn roving_navigation_skips_wraps_and_separates_activation() {
    let states = States::new();
    InteractiveScene::run(720.0, 420.0, |scene| {
        scene.install(states.harness(false));
        let start = row(scene, 29);
        scene.focus.focus(Some(&start), true);
        scene.key(Scancode::HOME, KeyModifiers::default());
        scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
        scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
        let third = row(scene, 3);
        assert_eq!(
            states.selected.get(),
            Some(3),
            "down arrows skip the disabled row 2"
        );
        let focused = scene
            .focus
            .focused()
            .expect("a row keeps focus while browsing");
        let mut cursor = Some(focused.clone());
        while let Some(node) = cursor.clone() {
            if node.id() == third.id() {
                break;
            }
            cursor = node.parent();
        }
        assert!(
            cursor.is_some(),
            "focus follows the selection to row 3, saw {:?}",
            focused.accessibility().map(|semantics| semantics.label)
        );
        assert_eq!(states.activations.get(), 0, "browsing does not activate");

        scene.key(Scancode::ARROW_RIGHT, KeyModifiers::default());
        scene.key(Scancode::ARROW_DOWN, KeyModifiers::LEFT_SHIFT);
        assert_eq!(
            states.selected.get(),
            Some(3),
            "the rail is vertical-only: horizontal and modified arrows are no-ops"
        );

        // The button activates on key *release* (Swift's `Button.press`), and
        // `InteractiveScene::key` only sends the press half.
        scene.key(Scancode::RETURN, KeyModifiers::default());
        scene.dispatcher().dispatch(&InputEvent::KeyUp(KeyEvent {
            scancode: Scancode::RETURN,
            keycode: 0,
            modifiers: KeyModifiers::default(),
            is_repeat: false,
        }));
        scene.settle(5, 1.0);
        assert_eq!(
            states.activations.get(),
            1,
            "Return activates the focused row exactly once"
        );

        scene.key(Scancode::END, KeyModifiers::default());
        scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
        assert_eq!(states.selected.get(), Some(0), "the roving group wraps");

        let tab_stops = scene
            .nodes()
            .into_iter()
            .filter(|node| node.attachment::<u64>("__roving.item").is_some() && node.is_tab_stop())
            .count();
        assert_eq!(tab_stops, 1, "the rail exposes exactly one roving tab stop");

        let selected = row(scene, 0);
        assert!(
            nodes_of(&selected)
                .iter()
                .all(|node| support::debug_name(node).as_deref()
                    != Some("rail-selection-indicator")),
            "the selected row carries no accent-lane chrome node"
        );
        assert!(
            nodes_of(&selected)
                .iter()
                .all(|node| support::debug_name(node).as_deref()
                    != Some("rail-selection-tile")),
            "a labelled rail uses the full-width pill, not the icon-only tile"
        );
    });
}

#[test]
fn icon_only_rail_marks_selection_and_hovers_without_an_accent_lane() {
    let states = States::new();
    InteractiveScene::run(720.0, 420.0, |scene| {
        states.dark.set(true);
        states.presentation.set(SidebarPresentation::Rail {
            shows_labels: false,
        });
        states.selected.set(Some(0));
        scene.install(states.harness(false));

        let selected = row(scene, 0);
        let theme = Theme::default_dark();
        let tile = nodes_of(&selected)
            .into_iter()
            .find(|node| support::debug_name(node).as_deref() == Some("rail-selection-tile"))
            .expect("the icon-only rail floats a rail-selection-tile");
        assert_eq!(
            (tile.frame().width, tile.frame().height),
            (40.0, 40.0),
            "the tile measures 40x40"
        );
        let row_frame = selected.absolute_frame();
        let tile_frame = tile.absolute_frame();
        assert!(
            (mid_x(tile_frame) - mid_x(row_frame)).abs() < 0.5
                && (mid_y(tile_frame) - mid_y(row_frame)).abs() < 0.5,
            "the tile centres on the row: tile {tile_frame:?}, row {row_frame:?}"
        );
        let selected_glyph = glyph(&selected);
        let glyph_frame = selected_glyph.absolute_frame();
        assert!(
            (mid_x(glyph_frame) - mid_x(tile_frame)).abs() < 0.5
                && (mid_y(glyph_frame) - mid_y(tile_frame)).abs() < 0.5,
            "the glyph centres in the tile: glyph {glyph_frame:?}, tile {tile_frame:?}"
        );
        assert_eq!(
            tile.background_color(),
            Some(theme.colors.surface_variant),
            "the resting tile paints the neutral selection fill"
        );
        assert_eq!(
            selected_glyph.inherited_foreground_color(),
            Some(theme.colors.on_surface),
            "a selected glyph brightens to onSurface"
        );
        assert!(
            !nodes_of(&selected).iter().any(|node| {
                support::debug_name(node).as_deref() == Some("rail-selection-indicator")
            }),
            "the accent lane was dropped"
        );

        let hovered = row(scene, 1);
        assert_eq!(
            glyph(&hovered).inherited_foreground_color(),
            Some(theme.colors.on_surface_variant),
            "enabled glyphs carry the variant tint"
        );
        let bounds = hovered.absolute_frame();
        scene
            .dispatcher()
            .dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
                x: mid_x(bounds),
                y: mid_y(bounds),
                delta_x: 0.0,
                delta_y: 0.0,
                kind: Default::default(),
            }));
        scene.settle(4, 1.0);
        let hovered = row(scene, 1);
        let hover_tile = nodes_of(&hovered)
            .into_iter()
            .find(|node| support::debug_name(node).as_deref() == Some("rail-selection-tile"))
            .expect("the hovered row keeps its tile");
        assert_eq!(
            hover_tile.background_color(),
            Some(theme.colors.state_layer_hover),
            "hovering swaps the tile to the hover state layer"
        );
        assert_eq!(
            glyph(&hovered).inherited_foreground_color(),
            Some(theme.colors.on_surface),
            "hovering brightens the glyph to onSurface"
        );
    });
}

#[test]
fn touch_activation_releases_capture_on_disable_and_unmount() {
    let states = States::new();
    states.selected.set(Some(0));
    states.presentation.set(SidebarPresentation::Rail {
        shows_labels: false,
    });
    InteractiveScene::run(720.0, 420.0, |scene| {
        states.input.set(InputCapabilities::TOUCH);
        scene.install(states.harness(false));
        let row = row(scene, 0);
        let bounds = row.absolute_frame();
        // The clamps keep every rail row inside the viewport, so the probe can
        // aim at the row's own bottom-left corner.
        let point = Point {
            x: bounds.x + 1.0,
            y: bounds.max_y() - 1.0,
        };
        scene.click_at_point(point, 1);
        assert_eq!(
            states.activations.get(),
            1,
            "a touch at the row edge activates"
        );
        assert_eq!(states.selected.get(), Some(0));

        let down = MouseButtonEvent {
            button: MouseButton::Left,
            x: point.x,
            y: point.y,
            clicks: 1,
            modifiers: Default::default(),
            kind: Default::default(),
        };
        scene
            .dispatcher()
            .dispatch(&InputEvent::MouseButtonDown(down.clone()));
        assert!(
            scene
                .capture
                .target()
                .is_some_and(|target| target.id() == row.id()),
            "the press captures the row"
        );

        states.enabled.set(false);
        scene.settle(8, 1.0);
        assert!(
            scene.capture.target().is_none(),
            "disabling the pressed rail releases capture"
        );
        assert!(
            !row.is_focusable(),
            "the disabled row leaves the focus chain"
        );
        assert_eq!(
            row.accessibility()
                .map(|semantics| semantics.state.is_enabled),
            Some(false),
            "the row reports itself disabled to accessibility"
        );
        scene
            .dispatcher()
            .dispatch(&InputEvent::MouseButtonUp(down.clone()));
        assert_eq!(
            states.activations.get(),
            1,
            "a release after disable must not activate"
        );

        states.enabled.set(true);
        scene.settle(8, 1.0);
        scene
            .dispatcher()
            .dispatch(&InputEvent::MouseButtonDown(down));
        scene.graph.install(EmptyView, Default::default());
        scene.graph.recomposer.commit_all();
        assert!(
            scene.capture.target().is_none(),
            "unmounting a pressed rail releases capture"
        );
        assert!(
            scene.focus.focused().is_none(),
            "unmounting a pressed rail clears focus"
        );
    });
}

#[test]
fn discovery_finds_full_names_without_disabled_targets() {
    support::GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph: Rc<ViewGraph> = support::graph();
            graph.install_view(
                AnyView::new(
                    AnyView::new(
                        NavigationRail::try_new(
                            Binding::constant(Some(0)),
                            vec![
                                SidebarItem::new(0, "Workspace notifications").badge("999"),
                                SidebarItem::new(1, "Unavailable").enabled(false),
                            ],
                        )
                        .expect("unique rail IDs")
                        .shows_labels(false),
                    )
                    .frame_height(180.0),
                ),
                Default::default(),
            );
            for _ in 0..8 {
                graph.recomposer.commit_all();
                graph.compute_layout(390.0, 180.0);
            }
            let root = graph.tree.borrow().root().expect("root");
            let buttons = nodes_of(&root)
                .into_iter()
                .filter_map(|node| {
                    let semantics = node.accessibility()?;
                    (semantics.role == AccessibilityRole::Button).then_some((node, semantics))
                })
                .collect::<Vec<(Node, cove_core::scene::AccessibilitySemantics)>>();

            let enabled: Vec<&Node> = buttons
                .iter()
                .filter(|(_, semantics)| {
                    semantics.label == "Workspace notifications" && semantics.state.is_enabled
                })
                .map(|(node, _)| node)
                .collect();
            assert_eq!(
                enabled.len(),
                1,
                "exactly one input-enabled control answers to the full name"
            );
            let found = buttons
                .iter()
                .find(|(_, semantics)| semantics.label == "Workspace notifications")
                .expect("the destination is discoverable");
            assert_eq!(found.1.value, "999");
            assert!(
                found.1.state.is_selected,
                "the constant selection lands selected"
            );
            let unavailable_enabled = buttons
                .iter()
                .filter(|(_, semantics)| {
                    semantics.label == "Unavailable" && semantics.state.is_enabled
                })
                .count();
            assert_eq!(
                unavailable_enabled, 0,
                "the disabled destination is invisible to input-enabled discovery"
            );
        });
    });
}
