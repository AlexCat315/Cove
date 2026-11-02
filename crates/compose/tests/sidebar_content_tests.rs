//! Translation of `Tests/CoveUIComposeTests/SidebarContentTests.swift`.
//!
//! Behaviour checklist:
//! - mixed icon roles reserve one leading lane; dynamic text scale grows the
//!   glyphs, rows and badges without overlap; dropping icons drops the lane
//! - icon foreground and destination semantics survive focus and hover in both
//!   themes; the selection band uses the neutral sidebar role
//! - vertical keyboard selection skips disabled rows, keeps one tab stop and
//!   reveals offscreen rows; modified and horizontal keys and a disabled click
//!   change nothing
//! - header and footer slots keep separate layout boundaries under scaling
//! - initial and external selection reveal once without trapping user scroll
//!
//! Swift's parameterised `semanticsAndColor(dark:)` runs both themes inside
//! one `#[test]`.
//!
//! Named differences from the Swift suite (each pinned by a `GAP:` assertion):
//! - Lookups: Swift found rows, titles, badges and glyphs by the debug names the
//!   sidebar published (`sidebar-item-*`, `sidebar-title-*`, `sidebar-badge-*`);
//!   Rust publishes none, so rows are the labelled button host, titles its
//!   `StaticText`, badges its `Badge` node and glyphs its `Image` node.
//! - Text scale: long titles truncate on one line at 200% instead of wrapping.
//! - Row geometry: the Rust row button hugs its content, so the long row is
//!   wider than the 260pt sidebar.
//! - Hover: Rust paints the selected fill on a hovered selection instead of the
//!   hover state layered over it.
//! - Focus ring: Rust paints no 2pt `focus_ring` border on a focused row.
//! - The Rust `Sidebar` leaves its first `ScrollView` mounted as a zero-height
//!   orphan after the first recomposition; `live_nodes` skips it.

use cove_composition::Scope;
use crate::support;

use cove_composition::{AnyView, DynamicField, Scancode, TupleView, View};
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, Node, Wrap};
use cove_core::{Binding, Point, Rect, State};
use cove_platform::{InputEvent, KeyModifiers, MouseMotionEvent};
use cove_ui_compose::BoxView;
use cove_ui_compose::modifiers::layout::{FlexExt, FrameDimension, LayoutFrameExt, PaddingExt};
use cove_ui_compose::primitives::layout::sidebar::{Sidebar, SidebarItem, SidebarSection};
use cove_ui_compose::primitives::media::Icon;
use cove_ui_compose::primitives::media::ui_icon_catalog::{action, file, shape, status};
use cove_ui_compose::primitives::text::{Divider, DividerAxis, Text};
use cove_ui_compose::theme::styles::components::selection_appearance::{
    SelectionAppearance, SelectionRole,
};
use cove_ui_compose::theme::styles::{default_dark_theme, default_light_theme};
use cove_ui_compose::theme::{
    SemanticColorRef, SemanticFontRef, SemanticIconSize, SemanticStyleViewExt, TextScaler,
    TextScalerViewExt, Theme, ThemeViewExt,
};

use support::InteractiveScene;

fn sized(view: impl View + 'static, width: f32, height: f32) -> AnyView {
    AnyView::new(AnyView::new(view).frame(
        Some(FrameDimension::Points(width)),
        Some(FrameDimension::Points(height)),
        None,
        None,
        None,
        None,
    ))
}

#[derive(Clone)]
struct SidebarScene {
    viewport: f32,
    theme: Theme,
    scale: State<f32>,
    selected: State<Option<String>>,
    icons: State<bool>,
}

impl SidebarScene {
    fn new(viewport: f32, theme: Theme) -> Self {
        Self {
            viewport,
            theme,
            scale: State::new(1.0),
            selected: State::new(Some("projects".to_owned())),
            icons: State::new(true),
        }
    }
}

impl View for SidebarScene {
    fn body(&self, _cx: &Scope) -> AnyView {
        let icons = self.icons.get();
        let icon =
            |item: SidebarItem<String>, resource| if icons { item.icon(resource) } else { item };
        let items = vec![
            icon(
                SidebarItem::new("projects".to_owned(), "Projects"),
                file::folder(),
            )
            .badge("88"),
            SidebarItem::new("recent".to_owned(), "Recent activity"),
            icon(
                SidebarItem::new("disabled".to_owned(), "Team permissions"),
                status::lock(),
            )
            .enabled(false),
            icon(
                SidebarItem::new("settings".to_owned(), "Settings"),
                action::settings(),
            )
            .icon_size(SemanticIconSize::new(|_| 19.0)),
            icon(
                SidebarItem::new(
                    "long".to_owned(),
                    "A destination with a long descriptive title",
                ),
                file::file(),
            )
            .badge("999"),
        ];
        let sidebar = Sidebar::new(
            self.selected.binding(),
            vec![SidebarSection::new("workspace", items)],
        );
        AnyView::new(
            AnyView::new(
                AnyView::new(sized(sidebar, 260.0, self.viewport))
                    .text_scaler(TextScaler::new(self.scale.get())),
            )
            .theme(self.theme.clone()),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("scale", &self.scale),
            DynamicField::state("selected", &self.selected),
            DynamicField::state("icons", &self.icons),
        ]
    }
}

#[derive(Clone)]
struct SlotScene {
    scale: State<f32>,
}

impl View for SlotScene {
    fn body(&self, _cx: &Scope) -> AnyView {
        let row = |spacing: f32, children: Vec<AnyView>| {
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                spacing,
                AnyView::new(TupleView::new(children)),
            )
        };
        let header = AnyView::new(TupleView::new(vec![
            AnyView::new(cove_composition::LayoutDebugExt::debug_name(
                AnyView::new(
                    row(
                        10.0,
                        vec![
                            AnyView::new(
                                Icon::semantic(file::package(), SemanticIconSize::large())
                                    .flex_with(0.0, 0.0, None),
                            ),
                            AnyView::new(
                                AnyView::new(
                                    Text::new("Cove workspace")
                                        .semantic_font(SemanticFontRef::body_strong())
                                        .flex_with(1.0, 1.0, None),
                                )
                                .frame_min_width(0.0),
                            ),
                        ],
                    )
                    .padding(16.0),
                ),
                "sidebar-slot-header",
            )),
            AnyView::new(Divider::new(None, 1.0, DividerAxis::Horizontal)),
        ]));
        let footer = AnyView::new(TupleView::new(vec![
            AnyView::new(Divider::new(None, 1.0, DividerAxis::Horizontal)),
            AnyView::new(cove_composition::LayoutDebugExt::debug_name(
                AnyView::new(
                    row(
                        8.0,
                        vec![
                            AnyView::new(Icon::semantic(
                                shape::circle_dot(),
                                SemanticIconSize::small(),
                            )),
                            AnyView::new(
                                AnyView::new(
                                    Text::new("All changes saved")
                                        .semantic_font(SemanticFontRef::caption())
                                        .flex_with(1.0, 1.0, None),
                                )
                                .frame_min_width(0.0),
                            ),
                        ],
                    )
                    .padding(14.0),
                ),
                "sidebar-slot-footer",
            )),
        ]));
        let sidebar = Sidebar::new(
            Binding::constant(Some("project".to_owned())),
            vec![SidebarSection::new(
                "main",
                vec![SidebarItem::new("project".to_owned(), "Project")],
            )],
        )
        .header(header)
        .footer(footer);
        AnyView::new(
            AnyView::new(
                AnyView::new(sized(sidebar, 260.0, 420.0))
                    .text_scaler(TextScaler::new(self.scale.get())),
            )
            .theme(default_light_theme::value()),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("scale", &self.scale)]
    }
}

/// Every node except orphaned subtrees (see the module doc).
fn live_nodes(scene: &InteractiveScene) -> Vec<Node> {
    fn is_scroll_area(node: &Node) -> bool {
        node.accessibility()
            .is_some_and(|semantics| semantics.role == AccessibilityRole::ScrollArea)
    }
    fn walk(node: &Node, into: &mut Vec<Node>) {
        into.push(node.clone());
        let children = node.children();
        for (index, child) in children.iter().enumerate() {
            let orphaned = is_scroll_area(child)
                && child.frame().height == 0.0
                && children[index + 1..].iter().any(is_scroll_area);
            if !orphaned {
                walk(child, into);
            }
        }
    }
    let mut collected = Vec::new();
    if let Some(root) = scene.graph.tree.borrow().root() {
        walk(&root, &mut collected);
    }
    collected
}

fn subtree(root: &Node) -> Vec<Node> {
    let mut collected = vec![root.clone()];
    for child in root.children() {
        collected.extend(subtree(&child));
    }
    collected
}

fn is_image(node: &Node) -> bool {
    node.view_tag()
        .is_some_and(|tag| tag.ends_with("primitives::media::image::Image"))
}

fn row(scene: &InteractiveScene, title: &str) -> Node {
    live_nodes(scene)
        .into_iter()
        .find(|node| {
            node.attachment::<bool>("__button_host") == Some(true)
                && node
                    .accessibility()
                    .is_some_and(|semantics| semantics.label == title)
        })
        .unwrap_or_else(|| panic!("the {title:?} row is mounted"))
}

fn glyph(scene: &InteractiveScene, title: &str) -> Node {
    subtree(&row(scene, title))
        .into_iter()
        .find(is_image)
        .unwrap_or_else(|| panic!("the {title:?} row renders a glyph"))
}

fn title(scene: &InteractiveScene, title: &str) -> Rect {
    subtree(&row(scene, title))
        .into_iter()
        .find(|node| {
            node.accessibility().is_some_and(|semantics| {
                semantics.role == AccessibilityRole::StaticText && semantics.label == title
            })
        })
        .unwrap_or_else(|| panic!("the {title:?} row renders its title"))
        .absolute_frame()
}

fn badge(scene: &InteractiveScene, title: &str) -> Rect {
    subtree(&row(scene, title))
        .into_iter()
        .find(|node| {
            node.view_tag()
                .is_some_and(|tag| tag.ends_with("badge::Badge"))
        })
        .unwrap_or_else(|| panic!("the {title:?} row renders a badge"))
        .absolute_frame()
}

fn scroller(scene: &InteractiveScene) -> Node {
    live_nodes(scene)
        .into_iter()
        .find(|node| node.scroll_reveal().is_some() && node.frame().height > 0.0)
        .expect("the sidebar scrolls")
}

fn contains(outer: Rect, inner: Rect) -> bool {
    inner.x >= outer.x - 0.5
        && inner.y >= outer.y - 0.5
        && inner.max_x() <= outer.max_x() + 0.5
        && inner.max_y() <= outer.max_y() + 0.5
}

fn mid_x(rect: Rect) -> f32 {
    rect.x + rect.width / 2.0
}

fn mid_y(rect: Rect) -> f32 {
    rect.y + rect.height / 2.0
}

const LONG: &str = "A destination with a long descriptive title";

#[test]
fn scaled_alignment() {
    InteractiveScene::run(260.0, 360.0, |scene| {
        let view = SidebarScene::new(360.0, default_light_theme::value());
        scene.install(view.clone());
        let first = glyph(scene, "Projects");
        let last = glyph(scene, "Settings");
        assert_eq!(
            (first.frame().width, last.frame().width),
            (16.0, 19.0),
            "each glyph keeps its own size inside the shared lane"
        );
        assert!(
            (mid_x(first.absolute_frame()) - mid_x(last.absolute_frame())).abs() <= 0.5,
            "glyphs share one lane centre, saw {:?} vs {:?}",
            first.absolute_frame(),
            last.absolute_frame()
        );
        let before = row(scene, "Projects").frame().height;
        view.scale.set(2.0);
        scene.settle(5, 1.0);
        assert_eq!(
            (
                glyph(scene, "Projects").frame().width,
                glyph(scene, "Settings").frame().width
            ),
            (32.0, 38.0),
            "glyphs scale with the text"
        );
        let projects = title(scene, "Projects");
        let recent = title(scene, "Recent activity");
        let settings = title(scene, "Settings");
        assert!(
            projects.x == recent.x && projects.x == settings.x,
            "every title starts after the shared lane, saw {projects:?} {recent:?} {settings:?}"
        );
        let row_frame = row(scene, "Projects").absolute_frame();
        let badge_frame = badge(scene, "Projects");
        assert!(
            row_frame.height >= before
                && contains(row_frame, projects)
                && contains(row_frame, badge_frame),
            "the scaled row contains its title and badge, before {before}, row {row_frame:?}, title {projects:?}, badge {badge_frame:?}"
        );
        assert!(
            projects.max_x() + 9.0 <= badge_frame.x
                && (mid_y(projects) - mid_y(badge_frame)).abs() < 0.5,
            "the badge sits clear of the title on its centre line, title {projects:?}, badge {badge_frame:?}"
        );
        let long_row = row(scene, LONG).absolute_frame();
        let long_title = title(scene, LONG);
        let long_badge = badge(scene, LONG);
        assert!(
            long_title.height <= 36.0,
            "GAP: Swift wraps the long title over several lines at 200% (> 36pt); Rust truncates it to one line, saw {long_title:?}"
        );
        assert!(
            contains(long_row, long_title) && contains(long_row, long_badge),
            "the long row contains its title and badge, row {long_row:?}, title {long_title:?}, badge {long_badge:?}"
        );
        assert!(
            long_row.width > 260.0,
            "GAP: Swift keeps the long row inside the 260pt sidebar; the Rust row button hugs its content, saw {long_row:?}"
        );
        assert!(
            long_title.max_x() + 9.0 <= long_badge.x,
            "the long badge sits clear of its title, title {long_title:?}, badge {long_badge:?}"
        );
        view.icons.set(false);
        scene.settle(5, 1.0);
        assert!(
            !live_nodes(scene).iter().any(is_image),
            "dropping every icon mounts no glyph"
        );
        assert!(
            title(scene, "Projects").x < projects.x,
            "dropping every icon drops the shared lane"
        );
    });
}

#[test]
fn semantics_and_color() {
    for dark in [false, true] {
        InteractiveScene::run(260.0, 360.0, |scene| {
            let theme = if dark {
                default_dark_theme::value()
            } else {
                default_light_theme::value()
            };
            scene.install(SidebarScene::new(360.0, theme.clone()));
            let projects = row(scene, "Projects");
            let semantics = projects.accessibility().expect("row semantics");
            assert_eq!(
                (
                    semantics.label.as_str(),
                    semantics.value.as_str(),
                    semantics.state.is_selected
                ),
                ("Projects", "88", true),
                "the selected destination names itself and its badge (dark {dark})"
            );
            assert_eq!(
                glyph(scene, "Projects").inherited_foreground_color(),
                Some(theme.colors.on_surface),
                "the selected glyph inherits the row ink (dark {dark})"
            );
            let disabled = row(scene, "Team permissions");
            assert!(
                !disabled.is_focusable()
                    && !disabled
                        .accessibility()
                        .expect("semantics")
                        .state
                        .is_enabled,
                "a disabled destination is out of focus and reports it (dark {dark})"
            );
            let disabled_ink = glyph(scene, "Team permissions").inherited_foreground_color();
            assert_eq!(
                disabled_ink,
                Some(SemanticColorRef::on_surface_disabled().resolve(&theme)),
                "a disabled glyph uses the disabled ink (dark {dark})"
            );
            scene.focus.focus(Some(&projects), true);
            scene.settle(5, 1.0);
            let projects = row(scene, "Projects");
            let semantics = projects.accessibility().expect("row semantics");
            assert_eq!(
                (semantics.label.as_str(), semantics.value.as_str()),
                ("Projects", "88"),
                "focus keeps the destination semantics (dark {dark})"
            );
            assert!(
                !subtree(&projects).iter().any(|node| {
                    node.border_color() == Some(theme.colors.focus_ring)
                        && node.border_width() == 2.0
                }),
                "GAP: Swift paints a 2pt focus ring on the focused row; the Rust sidebar row paints none (dark {dark})"
            );
            let frame = projects.absolute_frame();
            scene
                .dispatcher()
                .dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
                    x: mid_x(frame),
                    y: mid_y(frame),
                    delta_x: 0.0,
                    delta_y: 0.0,
                    kind: Default::default(),
                }));
            scene.settle(5, 1.0);
            let projects = row(scene, "Projects");
            let semantics = projects.accessibility().expect("row semantics");
            assert!(
                semantics.state.is_selected && semantics.label == "Projects",
                "hover keeps the selection semantics (dark {dark})"
            );
            let selection = SelectionAppearance::resolve(SelectionRole::Sidebar, &theme);
            assert_eq!(
                selection.fill, theme.colors.surface_variant,
                "the sidebar selection uses the neutral grouped fill (dark {dark})"
            );
            let hovered = selection.fill.composited(selection.hover_fill);
            let fills: Vec<_> = subtree(&projects)
                .iter()
                .filter_map(Node::background_color)
                .collect();
            assert!(
                !fills.contains(&hovered) && fills.contains(&selection.fill),
                "GAP: Swift layers the hover state over the selected fill; the Rust row style picks the selected fill over hover, saw {fills:?} (dark {dark})"
            );
        });
    }
}

#[test]
fn keyboard_scroll_and_disabled_click() {
    InteractiveScene::run(260.0, 130.0, |scene| {
        let view = SidebarScene::new(130.0, default_light_theme::value());
        scene.install(view.clone());
        let recent = row(scene, "Recent activity");
        scene.focus.focus(Some(&recent), true);
        scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
        assert_eq!(
            view.selected.get().as_deref(),
            Some("settings"),
            "Down skips the disabled destination"
        );
        assert_eq!(
            scene.focus.focused().map(|node| node.id()),
            Some(row(scene, "Settings").id()),
            "focus moves to `settings` with the selection"
        );
        let tab_stops = live_nodes(scene)
            .into_iter()
            .filter(|node| node.attachment::<u64>("__roving.item").is_some() && node.is_tab_stop())
            .count();
        assert_eq!(tab_stops, 1, "exactly one destination is the tab stop");
        scene.key(Scancode::END, KeyModifiers::default());
        let scroll = scroller(scene);
        assert!(
            view.selected.get().as_deref() == Some("long") && scroll.content_offset().y > 0.0,
            "End selects the last row and scrolls to it, selection {:?}, offset {:?}",
            view.selected.get(),
            scroll.content_offset()
        );
        let target = row(scene, LONG).absolute_frame();
        assert!(
            target.max_y() <= scroll.absolute_frame().max_y() + 0.5,
            "the revealed row ends inside the viewport, target {target:?}, viewport {:?}",
            scroll.absolute_frame()
        );
        scene.key(Scancode::HOME, KeyModifiers::default());
        assert!(
            view.selected.get().as_deref() == Some("projects")
                && scroller(scene).content_offset().y <= 10.0,
            "Home returns to the top, offset {:?}",
            scroller(scene).content_offset()
        );
        scene.key(Scancode::ARROW_RIGHT, KeyModifiers::default());
        scene.key(Scancode::ARROW_DOWN, KeyModifiers::LEFT_SHIFT);
        assert_eq!(
            view.selected.get().as_deref(),
            Some("projects"),
            "horizontal and modified keys leave the selection alone"
        );
        let disabled = row(scene, "Team permissions");
        scene.click_at_point(
            Point::new(
                mid_x(disabled.absolute_frame()),
                mid_y(disabled.absolute_frame()),
            ),
            1,
        );
        assert_eq!(
            view.selected.get().as_deref(),
            Some("projects"),
            "a disabled destination ignores clicks"
        );
    });
}

#[test]
fn slot_geometry() {
    InteractiveScene::run(260.0, 420.0, |scene| {
        let view = SlotScene {
            scale: State::new(1.0),
        };
        scene.install(view.clone());
        let named = |scene: &InteractiveScene, name: &str| {
            live_nodes(scene)
                .into_iter()
                .find(|node| support::debug_name(node).as_deref() == Some(name))
                .unwrap_or_else(|| panic!("{name} mounts"))
        };
        let before = named(scene, "sidebar-slot-header").frame().height;
        view.scale.set(2.0);
        scene.settle(5, 1.0);
        let header = named(scene, "sidebar-slot-header");
        let footer = named(scene, "sidebar-slot-footer");
        let scroll = scroller(scene);
        assert!(
            header.frame().height > before,
            "the header grows with the text, before {before}, after {:?}",
            header.frame()
        );
        assert!(
            scroll.absolute_frame().y >= header.absolute_frame().max_y(),
            "the destinations start below the header, scroll {:?}, header {:?}",
            scroll.absolute_frame(),
            header.absolute_frame()
        );
        assert!(
            scroll.absolute_frame().max_y() <= footer.absolute_frame().y,
            "the destinations end above the footer, scroll {:?}, footer {:?}",
            scroll.absolute_frame(),
            footer.absolute_frame()
        );
        for slot in [&header, &footer] {
            for child in subtree(slot).into_iter().filter(|node| {
                node.layout_node()
                    .is_some_and(|layout| layout.text_inputs().is_some())
                    || is_image(node)
            }) {
                assert!(
                    contains(slot.absolute_frame(), child.absolute_frame()),
                    "slot content stays inside its slot, slot {:?}, child {:?}",
                    slot.absolute_frame(),
                    child.absolute_frame()
                );
            }
        }
        let icon = subtree(&header)
            .into_iter()
            .find(is_image)
            .expect("the header icon mounts");
        assert_eq!(
            (icon.frame().width, icon.frame().height),
            (48.0, 48.0),
            "the large header icon follows the 200% text scale"
        );
    });
}

#[test]
fn controlled_selection_reveal() {
    InteractiveScene::run(260.0, 130.0, |scene| {
        let view = SidebarScene::new(130.0, default_light_theme::value());
        scene.install(view.clone());
        view.selected.set(Some("settings".to_owned()));
        scene.settle(5, 1.0);
        let scroll = scroller(scene);
        assert!(
            scroll.content_offset().y > 0.0,
            "an external selection reveals its row"
        );
        assert!(
            row(scene, "Settings").absolute_frame().max_y()
                <= scroll.absolute_frame().max_y() + 0.5,
            "the revealed row ends inside the viewport"
        );
        scroll.set_content_offset(Point::new(0.0, 0.0));
        scene.settle(5, 1.0);
        assert_eq!(
            scroller(scene).content_offset(),
            Point::new(0.0, 0.0),
            "a later user scroll is not trapped by the reveal"
        );
        scene.install(cove_composition::EmptyView);
        let mut items: Vec<SidebarItem<String>> = (0..10)
            .map(|index| SidebarItem::new(index.to_string(), format!("Destination {index}")))
            .collect();
        items.push(SidebarItem::new("last".to_owned(), "Last"));
        scene.install(
            AnyView::new(sized(
                Sidebar::new(
                    Binding::constant(Some("last".to_owned())),
                    vec![SidebarSection::new("many", items)],
                ),
                260.0,
                130.0,
            ))
            .theme(default_light_theme::value()),
        );
        let initial = scroller(scene);
        assert!(
            initial.content_offset().y > 0.0,
            "an initial selection reveals its row"
        );
        let last = row(scene, "Last").absolute_frame();
        assert!(
            last.max_y() <= initial.absolute_frame().max_y() + 0.5,
            "the initially selected row ends inside the 130pt viewport ({last:?} in {:?})",
            initial.absolute_frame()
        );
    });
}
