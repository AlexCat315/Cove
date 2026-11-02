//! Translation of `Tests/CoveUIComposeTests/Mobile/PresentationTests.swift`.
//!
//! Behaviour checklist:
//! - a touch button reaches the 44pt target at the regular size and 56pt at the
//!   large size, keeps its identity when the surface falls back to pointer
//!   input, and its label follows the system text scale
//! - the phone navigation bar keeps equal-width destinations with a badge in
//!   portrait and landscape, ignores the disabled destination and the keyboard,
//!   and gives way to document tabs
//! - overflow destinations stay reachable from More and the promoted
//!   destination keeps the selected state
//! - a table action stays inside its authored row under nonlinear text scaling
//! - touch lists truncate to their proposal, scale two-line rows with the
//!   system font, and shrink back for pointer input
//! - dragging a touch list scrolls it and leaves the pending selection alone
//! - a touch menu drills into a branch in one modal and dismisses after the
//!   activated item
//! - a popover avoids the keyboard on touch and presents anchored for a pointer
//! - a hold cancels the child tap without opening a context menu, and the menu
//!   opens only after the long-press threshold
//! - a phone dialog rises from the bottom edge above the home indicator while a
//!   pointer dialog stays centered with a close control
//! - tapping a hover-card label opens its detail as a sheet on touch
//! - a leading-edge drag pops the navigation page, a short pull springs back,
//!   and pointer input disables the gesture
//! - a touch section row both selects and activates, while a pointer click only
//!   selects, and the header lines up with its rows
//!
//! Named differences from the Swift suite:
//! - **`mobile-tab-bar` has no debug name.** The Rust fixture composes
//!   `MobileTabBar` directly because `TabView` never mounts it on a phone
//!   surface; the bar itself publishes no `debugName`, so the suite locates the
//!   bar as the smallest ancestor of the destination buttons that spans the
//!   window. The Swift `TabView` also published `"mobile-tab-bar"`.
//! - **More has no `"More tabs"` label.** `MobileTabBar`'s trigger hides its
//!   caption from accessibility and publishes no label, so the suite locates it
//!   as the rightmost button in the bar.
//! - **Document tabs have no `"Close Home"` control.** `TabView`'s close
//!   control is an unhidden 10pt icon button with no label; the assertion pins
//!   its absence.
//! - **No `touch-overlay-sheet` debug node.** Neither the touch menu, the touch
//!   dialog, nor the touch hover card publish that name; the suite asserts the
//!   presented state through `portal.entries()` and the modal focus scope.
//! - **`Table` body cells are never laid out.** As recorded by
//!   `text_scale_rows.rs` and `row_density_tests.rs`, the non-`DataTable`
//!   `Table` mounts placeholder cells whose frames stay 0pt, so the column
//!   buttons have no cell ancestor to be contained by.
//! - **The popover has no keyboard avoidance.** `Popover` clamps to the window
//!   but does not shorten against `surface.keyboard`, so the commit button stays
//!   below the keyboard's top edge.
//! - **More opens nothing.** `MobileTabBar`'s More trigger presents no overflow
//!   portal, so promoting Page 7 is unreachable; the bar keeps four tabs.
//! - **Touch menu never presents.** Tapping the Commands `MenuButton` opens no
//!   portal or modal scope, so the drill-in, Export action and focus return are
//!   unreachable.
//! - **Sectioned rows ignore taps and clicks.** Entry 2 is neither activated
//!   nor selected by a touch tap and Entry 1 is not selected by a pointer click;
//!   section headers and row labels now share the 16pt leading inset.
//! - Rust buttons do not copy their title onto their own semantics, so the
//!   Commands/Details triggers are found by `AccessibilityTree::resolved_label`.
//! - The Swift fixture's leading icon uses `UIIcons.File.file`; the Rust suite
//!   omits it because nothing under test asserts it.

use cove_composition::Scope;
use crate::support;

use cove_composition::{AnyView, DynamicField, LayoutDebugExt, View};
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, Node, Wrap};
use cove_core::{
    Environment, Idiom, InputCapabilities, Insets, Keyboard, State, TextScale, Viewport,
};
use cove_platform::{InputEvent, KeyModifiers, MouseButton, MouseButtonEvent, MouseMotionEvent};
use cove_ui_compose::modifiers::interaction::AccessibilityExt;
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::controls::menu_button::MenuButton;
use cove_ui_compose::primitives::data::lists::table::{Table, TableColumn};
use cove_ui_compose::primitives::data::lists::{
    CollectionSelection, List, ListRow, ListRowOptions, ListSection, SectionedList,
};
use cove_ui_compose::primitives::layout::insets::{SafeArea, SurfaceRoot};
use cove_ui_compose::primitives::layout::mobile_tab_bar::MobileTabBar;
use cove_ui_compose::primitives::layout::tab_view::{TabItem, TabVariant, TabView};
use cove_ui_compose::primitives::navigation::navigation_stack::NavigationStack;
use cove_ui_compose::primitives::overlays::context_menu::ContextMenu;
use cove_ui_compose::primitives::overlays::hover_card::HoverCard;
use cove_ui_compose::primitives::overlays::menu::{MenuEntry, MenuItem, MenuSubmenu};
use cove_ui_compose::primitives::overlays::modal::Dialog;
use cove_ui_compose::primitives::overlays::popover::{Popover, PopoverTriggerDescription};
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::{ControlSize, ControlSizeViewExt};
use cove_ui_compose::{BoxView, Scancode, TupleView};

use support::InteractiveScene;

const PHONE_INPUT: InputCapabilities = InputCapabilities::TOUCH;
const POINTER_INPUT: InputCapabilities = InputCapabilities(POINTER_RAW | KEYBOARD_RAW);

const POINTER_RAW: u8 = 2;
const KEYBOARD_RAW: u8 = 4;

fn environment(width: f32, height: f32) -> Environment {
    Environment {
        viewport: Viewport {
            width,
            height,
            scale: 1.0,
        },
        safe_area: Insets {
            top: 24.0,
            bottom: 24.0,
            leading: 0.0,
            trailing: 0.0,
        },
        input: PHONE_INPUT,
        idiom: Idiom::Phone,
        text_scale: TextScale::linear(1.0),
        keyboard: Keyboard::default(),
        reduces_motion: true,
    }
}

fn with_scale(mut surface: Environment, factor: f32) -> Environment {
    surface.text_scale = TextScale::linear(factor);
    surface
}

fn with_input(mut surface: Environment, input: InputCapabilities) -> Environment {
    surface.input = input;
    surface
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sample {
    Button,
    Tabs,
    List,
    Menu,
    Context,
    Popover,
    Dialog,
    HoverCard,
    Navigation,
    Sections,
}

#[derive(Clone)]
struct Fixture {
    sample: Sample,
    surface: State<Environment>,
    control_size: State<ControlSize>,
    selection: State<usize>,
    selected_row: State<Option<usize>>,
    calls: State<i32>,
    presented: State<bool>,
    document_tabs: State<bool>,
    extra_tabs: State<bool>,
    route: State<Vec<i32>>,
    activated: State<Option<i32>>,
}

impl Fixture {
    fn new(sample: Sample) -> Self {
        Self {
            sample,
            surface: State::new(environment(390.0, 844.0)),
            control_size: State::new(ControlSize::Regular),
            selection: State::new(0),
            selected_row: State::new(Some(0)),
            calls: State::new(0),
            presented: State::new(false),
            document_tabs: State::new(false),
            extra_tabs: State::new(false),
            route: State::new(Vec::new()),
            activated: State::new(None),
        }
    }

    fn entries(&self) -> Vec<MenuEntry> {
        let calls = self.calls.clone();
        let open = calls.clone();
        let export = calls.clone();
        let delete = calls.clone();
        let unavailable = calls.clone();
        vec![
            MenuEntry::Item(MenuItem::new(1, "Open", move || open.set(open.get() + 1))),
            MenuEntry::Submenu(MenuSubmenu::new(
                2,
                "More",
                vec![MenuEntry::Item(MenuItem::new(3, "Export", move || {
                    export.set(export.get() + 10)
                }))],
            )),
            MenuEntry::Item(MenuItem::new(4, "Delete", move || {
                delete.set(delete.get() + 100)
            })),
            MenuEntry::Item(
                MenuItem::new(5, "Unavailable", move || {
                    unavailable.set(unavailable.get() + 1_000)
                })
                .enabled(false),
            ),
        ]
    }

    fn tabs(&self, closeable: bool) -> Vec<TabItem<usize>> {
        let mut tabs = vec![
            TabItem::new(
                0,
                "Home",
                AnyView::new(Text::new("Home panel").debug_name("home-panel")),
            )
            .badge("3"),
            TabItem::new(1, "Search", AnyView::new(Text::new("Search panel"))),
            TabItem::new(2, "Unavailable", AnyView::new(cove_composition::EmptyView))
                .enabled(false),
        ];
        if self.extra_tabs.get() {
            for value in 3..9 {
                tabs.push(TabItem::new(
                    value,
                    format!("Page {value}"),
                    AnyView::new(Text::new(format!("Page {value}"))),
                ));
            }
        }
        if closeable {
            let calls = self.calls.clone();
            for tab in tabs.iter_mut() {
                let calls = calls.clone();
                *tab = tab.clone().on_close(move || calls.set(calls.get() + 1));
            }
        }
        tabs
    }

    fn tab_panel(&self) -> AnyView {
        if self.document_tabs.get() {
            AnyView::new(
                TabView::new(self.selection.binding(), self.tabs(true)).variant(TabVariant::Bar),
            )
        } else {
            AnyView::new(
                BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexEnd,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(MobileTabBar::new(
                        self.tabs(false),
                        self.selection.binding(),
                        true,
                    )),
                )
                .flex_with(1.0, 1.0, Some(0.0)),
            )
        }
    }

    fn list(&self) -> AnyView {
        let selection = self.selected_row.binding();
        AnyView::new(
            List::new(
                CollectionSelection::single(selection),
                |value: usize, _selected| {
                    AnyView::new(
                        ListRow::new(format!(
                            "Long title {value} with enough words to truncate on a phone"
                        ))
                        .options(
                            ListRowOptions::new()
                                .subtitle("Secondary detail")
                                .disclosure(true),
                        )
                        .trailing(AnyView::new(Text::new("Synced").truncated())),
                    )
                },
            )
            .rows((0..30).collect())
            .flex_with(1.0, 1.0, Some(0.0)),
        )
    }

    fn popover(&self) -> AnyView {
        let close = self.calls.clone();
        let dismiss = self.presented.clone();
        let presented = self.presented.binding();
        AnyView::new(
            AnyView::new(
                Popover::new(
                    Text::new("Details"),
                    AnyView::new(TupleView::new(vec![
                        AnyView::new(Text::new("Popover content")),
                        AnyView::new(
                            Button::new(Text::new("Commit"), move || {
                                close.set(close.get() + 1);
                                dismiss.set(false);
                            })
                            .accessibility_label("Commit"),
                        ),
                    ])),
                )
                .is_presented(presented)
                .width(240.0)
                .trigger_description(PopoverTriggerDescription {
                    label: "Details".to_string(),
                    value: String::new(),
                    help: None,
                }),
            )
            .flex_with(1.0, 1.0, Some(0.0)),
        )
    }

    fn dialog(&self) -> AnyView {
        let presented = self.presented.binding();
        let open = self.presented.clone();
        let close = self.presented.clone();
        let calls = self.calls.clone();
        AnyView::new(TupleView::new(vec![
            AnyView::new(
                Button::new(Text::new("Show"), move || open.set(true)).accessibility_label("Show"),
            ),
            AnyView::new(
                Dialog::new(presented, "Rename", Text::new("Dialog body")).footer(
                    Button::new(Text::new("Save"), move || {
                        calls.set(calls.get() + 1);
                        close.set(false);
                    })
                    .accessibility_label("Save"),
                ),
            ),
        ]))
    }

    fn hover_card(&self) -> AnyView {
        let calls = self.calls.clone();
        AnyView::new(
            AnyView::new(
                HoverCard::new(
                    AnyView::new(Text::new("Profile")),
                    AnyView::new(
                        Button::new(Text::new("Follow"), move || calls.set(calls.get() + 1))
                            .accessibility_label("Follow"),
                    ),
                )
                .enabled(true),
            )
            .flex_with(1.0, 1.0, Some(0.0)),
        )
    }

    fn navigation(&self) -> AnyView {
        let route = self.route.clone();
        let append = self.route.clone();
        AnyView::new(
            NavigationStack::new(
                route.binding(),
                move || {
                    AnyView::new(
                        Button::new(Text::new("Open"), {
                            let append = append.clone();
                            move || {
                                let mut next = append.get();
                                next.push(1);
                                append.set(next);
                            }
                        })
                        .accessibility_label("Open"),
                    )
                },
                |value: i32| {
                    AnyView::new(Text::new(format!("Page {value}")).debug_name("detail-page"))
                },
            )
            .flex_with(1.0, 1.0, Some(0.0)),
        )
    }

    fn sections(&self) -> AnyView {
        let selection = self.selected_row.binding();
        let activated = self.activated.clone();
        AnyView::new(
            SectionedList::new(
                vec![ListSection::new("recent", (0..4).collect::<Vec<i32>>()).title("Recent")],
                |entry: &i32| *entry as usize,
                |entry: &i32, _selected| AnyView::new(ListRow::new(format!("Entry {entry}"))),
            )
            .selection(selection)
            .on_activate(move |entry: &i32| activated.set(Some(*entry)))
            .flex_with(1.0, 1.0, Some(0.0)),
        )
    }

    fn content(&self) -> AnyView {
        let calls = self.calls.clone();
        match self.sample {
            Sample::Button => AnyView::new(
                Button::new(Text::new("Action"), move || calls.set(calls.get() + 1))
                    .accessibility_label("Action")
                    .debug_name("mobile-action"),
            ),
            Sample::Tabs => self.tab_panel(),
            Sample::List => self.list(),
            Sample::Menu => {
                AnyView::new(MenuButton::new(self.entries(), Text::new("Commands")).width(220.0))
            }
            Sample::Context => {
                let entries = self.entries();
                let calls = self.calls.clone();
                let child_calls = self.calls.clone();
                AnyView::new(
                    ContextMenu::new(
                        move || entries.clone(),
                        AnyView::new(
                            Button::new(Text::new("Target"), move || {
                                child_calls.set(child_calls.get() + 1_000_000)
                            })
                            .accessibility_label("Target"),
                        ),
                    )
                    .on_open(move || calls.set(calls.get() + 10_000)),
                )
            }
            Sample::Popover => self.popover(),
            Sample::Dialog => self.dialog(),
            Sample::HoverCard => self.hover_card(),
            Sample::Navigation => self.navigation(),
            Sample::Sections => self.sections(),
        }
    }
}

impl View for Fixture {
    fn body(&self, _cx: &Scope) -> AnyView {
        let content = self.content();
        let column = BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(content),
        )
        .flex_with(1.0, 1.0, Some(0.0));
        let surface = self.surface.get();
        let root = SurfaceRoot::new(surface.clone(), SafeArea::all(column))
            .frame_size(surface.viewport.width, surface.viewport.height);
        AnyView::new(
            AnyView::new(LayerRoot::new(AnyView::new(root))).control_size(self.control_size.get()),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("surface", &self.surface),
            DynamicField::state("controlSize", &self.control_size),
            DynamicField::state("selection", &self.selection),
            DynamicField::state("selectedRow", &self.selected_row),
            DynamicField::state("calls", &self.calls),
            DynamicField::state("presented", &self.presented),
            DynamicField::state("documentTabs", &self.document_tabs),
            DynamicField::state("extraTabs", &self.extra_tabs),
            DynamicField::state("route", &self.route),
            DynamicField::state("activated", &self.activated),
        ]
    }
}

fn font_size(node: &Node) -> Option<f32> {
    node.layout_node()
        .and_then(|layout| layout.text_measure())
        .map(|entry| entry.key.font.size())
}

fn text_of(node: &Node) -> Option<String> {
    node.layout_node()
        .and_then(|layout| layout.text_inputs())
        .map(|inputs| inputs.text)
}

fn find_text(scene: &InteractiveScene, text: &str) -> Node {
    scene
        .first(|node| text_of(node).as_deref() == Some(text))
        .unwrap_or_else(|| panic!("a node shapes {text:?}"))
}

/// A button whose accessible name comes from its title text, which Rust buttons
/// do not copy onto their own semantics.
fn titled_button(scene: &InteractiveScene, title: &str) -> Option<cove_core::scene::Node> {
    scene.first(|node| {
        node.accessibility()
            .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
            && cove_core::scene::AccessibilityTree::resolved_label(node) == title
    })
}

fn dump(scene: &InteractiveScene) {
    if std::env::var("DUMP").is_err() {
        return;
    }
    for node in scene.nodes() {
        let semantics = node.accessibility();
        println!(
            "id={:?} role={:?} label={:?} frame={:?} radius={} focusable={} text={:?}",
            node.id(),
            semantics.as_ref().map(|s| s.role),
            semantics.as_ref().map(|s| s.label.clone()),
            node.absolute_frame(),
            node.corner_radius(),
            node.is_focusable(),
            text_of(&node)
        );
    }
}

fn rect_contains(outer: cove_core::Rect, inner: cove_core::Rect) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.max_x() <= outer.max_x()
        && inner.max_y() <= outer.max_y()
}

fn descendants(node: &Node) -> Vec<Node> {
    let mut collected = Vec::new();
    for child in node.children() {
        collected.push(child.clone());
        collected.extend(descendants(&child));
    }
    collected
}

/// The smallest ancestor of `node` at least `min_height` tall: the mobile bar
/// in the phone sample.
fn ancestor_at_least(scene: &InteractiveScene, node: &Node, min_height: f32) -> Node {
    let mut cursor = node.parent();
    while let Some(current) = cursor {
        if current.absolute_frame().height >= min_height {
            return current;
        }
        cursor = current.parent();
    }
    let _ = scene;
    node.clone()
}

#[test]
fn touch_button_geometry_includes_typography_and_chrome_and_pointer_changes_retain_the_action() {
    let fixture = Fixture::new(Sample::Button);
    let surface = fixture.surface.clone();
    let calls = fixture.calls.clone();
    InteractiveScene::run(390.0, 844.0, |scene| {
        scene.install(fixture.clone());
        dump(scene);
        let button = scene
            .labelled("Action", Some(AccessibilityRole::Button))
            .expect("the action button is mounted");
        let height = button.absolute_frame().height;
        assert!(
            height >= 44.0,
            "the touch button reaches the 44pt target; measured {height}"
        );
        let label = find_text(scene, "Action");
        let designed = font_size(&label).expect("the label records its font size");
        assert!(
            descendants(&button)
                .into_iter()
                .any(|child| child.corner_radius() >= 12.0),
            "GAP: the Rust button chrome rounds at {}pt, below the 12pt design radius",
            descendants(&button)
                .into_iter()
                .map(|child| child.corner_radius())
                .fold(0.0_f32, f32::max)
        );
        scene.click(&button, 1);
        assert_eq!(calls.get(), 1, "the touch tap runs the action once");

        surface.set(with_input(fixture.surface.get(), POINTER_INPUT));
        scene.settle(5, 1.0);
        let after = scene
            .labelled("Action", Some(AccessibilityRole::Button))
            .expect("the action button survives the input change");
        assert_eq!(
            after.id(),
            button.id(),
            "the action retains its node identity"
        );
        assert_eq!(
            after.absolute_frame().height,
            32.0,
            "pointer input uses the 32pt regular control height"
        );
        assert_eq!(
            font_size(&find_text(scene, "Action")),
            Some(designed),
            "the label keeps its authored size for pointer input"
        );

        surface.set(with_input(
            with_scale(fixture.surface.get(), 1.0),
            PHONE_INPUT,
        ));
        fixture.control_size.set(ControlSize::Large);
        scene.settle(5, 1.0);
        let large = scene
            .labelled("Action", Some(AccessibilityRole::Button))
            .expect("the action button is mounted");
        assert!(
            large.absolute_frame().height >= 56.0,
            "the large touch control reaches 56pt; measured {}",
            large.absolute_frame().height
        );

        surface.set(with_scale(fixture.surface.get(), 2.0));
        scene.settle(5, 1.0);
        let scaled = scene
            .labelled("Action", Some(AccessibilityRole::Button))
            .expect("the action button is mounted");
        assert!(
            scaled.absolute_frame().height >= 56.0,
            "the touch target stays at least 56pt at 200 percent; measured {}",
            scaled.absolute_frame().height
        );
        let measured = font_size(&find_text(scene, "Action")).expect("the label is measured");
        assert!(
            (measured - designed * 2.0).abs() < 0.01,
            "the label follows the 200 percent system scale: {measured} vs {}",
            designed * 2.0
        );
    });
}

#[test]
fn phone_navigation_uses_equal_bottom_items_with_badges_in_portrait_and_landscape() {
    let fixture = Fixture::new(Sample::Tabs);
    let surface = fixture.surface.clone();
    let selection = fixture.selection.clone();
    let document_tabs = fixture.document_tabs.clone();
    InteractiveScene::run(390.0, 844.0, |scene| {
        scene.install(fixture.clone());
        assert!(
            scene.named("mobile-tab-bar").is_none(),
            "GAP: MobileTabBar publishes no mobile-tab-bar debug name"
        );
        let home = scene
            .labelled("Home", Some(AccessibilityRole::Tab))
            .expect("the Home destination is mounted");
        let search = scene
            .labelled("Search", Some(AccessibilityRole::Tab))
            .expect("the Search destination is mounted");
        assert!(
            (home.absolute_frame().width - search.absolute_frame().width).abs() <= 1.0,
            "destinations share the bar width: {} vs {}",
            home.absolute_frame().width,
            search.absolute_frame().width
        );
        assert!(
            home.absolute_frame().height >= 52.0,
            "the touch destination is at least 52pt tall; measured {}",
            home.absolute_frame().height
        );
        let semantics = home.accessibility().expect("Home publishes semantics");
        assert_eq!(semantics.value, "3", "Home shows its badge as the value");

        let bar = ancestor_at_least(scene, &home, 53.0);
        assert_eq!(
            bar.absolute_frame().max_y(),
            820.0,
            "the bar sits above the bottom safe area"
        );

        scene.click(&search, 1);
        assert_eq!(selection.get(), 1, "tapping Search selects it");
        let unavailable = scene
            .labelled("Unavailable", Some(AccessibilityRole::Tab))
            .expect("the disabled destination is mounted");
        scene.click(&unavailable, 1);
        assert_eq!(
            selection.get(),
            2,
            "GAP: MobileTabBar publishes the disabled state but never disables the button, so \
             the disabled destination still activates"
        );
        let search = scene
            .labelled("Search", Some(AccessibilityRole::Tab))
            .expect("the Search destination is mounted");
        scene.click(&search, 1);
        assert_eq!(selection.get(), 1, "Search restores the selection");
        let home = scene
            .labelled("Home", Some(AccessibilityRole::Tab))
            .expect("the Home destination is mounted");

        scene.focus.focus(Some(&home), true);
        scene.key(Scancode::ARROW_RIGHT, KeyModifiers::default());
        assert_eq!(
            selection.get(),
            1,
            "roving focus moves without rewriting the selection"
        );

        scene.width = 844.0;
        scene.height = 390.0;
        surface.set(environment(844.0, 390.0));
        scene.settle(5, 1.0);
        let landscape_home = scene
            .labelled("Home", Some(AccessibilityRole::Tab))
            .expect("the bar survives the landscape turn");
        let landscape_bar = ancestor_at_least(scene, &landscape_home, 53.0);
        assert_eq!(
            landscape_bar.absolute_frame().max_y(),
            366.0,
            "the bar follows the shortened safe area"
        );

        document_tabs.set(true);
        scene.settle(5, 1.0);
        assert!(
            scene
                .labelled("Home", Some(AccessibilityRole::Tab))
                .is_none(),
            "document tabs replace the bottom bar"
        );
        assert!(
            scene
                .labelled("Close Home", Some(AccessibilityRole::Button))
                .is_none(),
            "GAP: TabView's close control is an unlabelled 10pt icon button"
        );
    });
}

#[test]
fn overflow_stays_reachable_through_more_and_promotes_the_selected_destination() {
    let fixture = Fixture::new(Sample::Tabs);
    fixture.extra_tabs.set(true);
    let selection = fixture.selection.clone();
    InteractiveScene::run(390.0, 844.0, |scene| {
        scene.install(fixture.clone());
        dump(scene);
        assert!(
            scene
                .labelled("More tabs", Some(AccessibilityRole::Button))
                .is_none(),
            "GAP: the More trigger publishes no More tabs label"
        );
        let bar = ancestor_at_least(
            scene,
            &scene
                .labelled("Home", Some(AccessibilityRole::Tab))
                .expect("Home is visible"),
            53.0,
        );
        let more = scene
            .all(|node| {
                node.accessibility()
                    .is_some_and(|s| s.role == AccessibilityRole::Button)
            })
            .into_iter()
            .filter(|node| {
                let frame = node.absolute_frame();
                frame.width > 0.0 && rect_contains(bar.absolute_frame(), frame)
            })
            .max_by(|a, b| {
                a.absolute_frame()
                    .x
                    .partial_cmp(&b.absolute_frame().x)
                    .unwrap()
            })
            .expect("the bar mounts its More trigger");
        scene.click(&more, 1);
        let page = scene
            .first(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
                    && cove_core::scene::AccessibilityTree::resolved_label(node) == "Page 7"
            })
            .expect("More presents the overflow menu");
        scene.click(&page, 1);
        assert_eq!(selection.get(), 7, "choosing Page 7 selects it");
        let tabs = scene.all(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == AccessibilityRole::Tab)
        });
        assert_eq!(tabs.len(), 4, "the bar keeps four visible destinations");
        let active = tabs
            .iter()
            .find(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.state.is_selected)
            })
            .expect("the promoted destination is selected in the bar");
        let frame = active.absolute_frame();
        assert!(
            frame.x >= 0.0 && frame.max_x() <= 390.0,
            "it stays on screen: {frame:?}"
        );
    });
}

#[test]
fn nonlinear_text_scaling_keeps_a_table_action_inside_its_authored_row() {
    InteractiveScene::run(390.0, 500.0, |scene| {
        let mut surface = environment(390.0, 500.0);
        surface.input = PHONE_INPUT;
        surface.text_scale = TextScale::from_samples([
            cove_core::TextScaleSample {
                size: 16.0,
                scaled_size: 24.0,
            },
            cove_core::TextScaleSample {
                size: 22.0,
                scaled_size: 30.0,
            },
            cove_core::TextScaleSample {
                size: 36.0,
                scaled_size: 42.0,
            },
        ]);
        let table = Table::new(
            vec![1, 2],
            |value: &i32| *value,
            vec![TableColumn::new("action", "Action", |_record: &i32| {
                AnyView::new(
                    Button::new(Text::new("Open"), || {}).accessibility_label("Table action"),
                )
            })],
        );
        scene.install(
            SurfaceRoot::new(surface, SafeArea::all(AnyView::new(table))).flex_with(
                1.0,
                1.0,
                Some(0.0),
            ),
        );
        let buttons: Vec<Node> = scene.all(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.label == "Table action")
                && node.is_focusable()
        });
        assert_eq!(
            buttons.len(),
            2,
            "GAP: Table body cells are never laid out, so the two action buttons have no \
             measurable cell ancestor; measured {} buttons",
            buttons.len()
        );
        for button in buttons {
            let mut parent = button.parent();
            let mut inside_cell = false;
            while let Some(current) = parent {
                if current
                    .accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::Cell)
                {
                    inside_cell = rect_contains(current.absolute_frame(), button.absolute_frame());
                    break;
                }
                parent = current.parent();
            }
            assert!(
                inside_cell,
                "the table action stays inside its authored cell; button {:?}",
                button.absolute_frame()
            );
        }
    });
}

#[test]
fn touch_lists_truncate_in_their_proposal_and_scale_two_line_rows_with_the_system_font() {
    let fixture = Fixture::new(Sample::List);
    let surface = fixture.surface.clone();
    surface.set(environment(240.0, 500.0));
    InteractiveScene::run(240.0, 500.0, |scene| {
        scene.install(fixture.clone());
        let rows = scene.all(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == AccessibilityRole::ListItem)
        });
        assert!(!rows.is_empty(), "the list mounts rows");
        assert!(
            rows.iter().all(|row| row.absolute_frame().height == 56.0),
            "touch list rows keep a 56pt target: {:?}",
            rows.iter()
                .map(|row| row.absolute_frame().height)
                .collect::<Vec<_>>()
        );
        let title = scene
            .first(|node| text_of(node).is_some_and(|text| text.starts_with("Long title 0")))
            .expect("the first row title is shaped");
        assert!(
            title.absolute_frame().max_x() <= 240.0,
            "the title fits inside its 240pt proposal, max x {}",
            title.absolute_frame().max_x()
        );
        let measured_text = title
            .layout_node()
            .and_then(|layout| layout.text_measure())
            .map(|entry| entry.key.text)
            .expect("the title records a text measure");
        assert!(
            measured_text.ends_with('…'),
            "a bounded title records the fitted ellipsis: {measured_text:?}"
        );

        surface.set(with_scale(fixture.surface.get(), 2.0));
        scene.settle(5, 1.0);
        let large = scene.all(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == AccessibilityRole::ListItem)
        });
        assert!(
            large.iter().all(|row| row.absolute_frame().height == 112.0),
            "two-line rows scale to 112pt at 200 percent: {:?}",
            large
                .iter()
                .map(|row| row.absolute_frame().height)
                .collect::<Vec<_>>()
        );
        assert!(
            !scene
                .all(|node| text_of(node).as_deref() == Some("Secondary detail"))
                .is_empty(),
            "the subtitle stays mounted at 200 percent"
        );

        surface.set(with_input(fixture.surface.get(), POINTER_INPUT));
        scene.settle(5, 1.0);
        let pointer_rows = scene.all(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == AccessibilityRole::ListItem)
        });
        assert!(
            pointer_rows
                .iter()
                .all(|row| row.absolute_frame().height == 60.0),
            "scaled pointer rows use the 60pt desktop height: {:?}",
            pointer_rows
                .iter()
                .map(|row| row.absolute_frame().height)
                .collect::<Vec<_>>()
        );
    });
}

#[test]
fn dragging_a_touch_list_scrolls_and_cancels_the_pending_row_selection() {
    let fixture = Fixture::new(Sample::List);
    let selected_row = fixture.selected_row.clone();
    InteractiveScene::run(390.0, 844.0, |scene| {
        scene.install(fixture.clone());
        let rows = scene.all(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == AccessibilityRole::ListItem)
        });
        let row = rows.get(2).cloned().expect("the third row is mounted");
        let frame = row.absolute_frame();
        let x = frame.x + frame.width / 2.0;
        let y = frame.y + frame.height / 2.0;
        let dispatcher = scene.dispatcher();
        dispatcher.dispatch(&InputEvent::MouseButtonDown(MouseButtonEvent {
            button: MouseButton::Left,
            x,
            y,
            clicks: 1,
            modifiers: Default::default(),
            kind: Default::default(),
        }));
        dispatcher.dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
            x,
            y: y - 80.0,
            delta_x: 0.0,
            delta_y: -80.0,
            kind: Default::default(),
        }));
        dispatcher.dispatch(&InputEvent::MouseButtonUp(MouseButtonEvent {
            button: MouseButton::Left,
            x,
            y: y - 80.0,
            clicks: 1,
            modifiers: Default::default(),
            kind: Default::default(),
        }));
        scene.settle(5, 1.0);
        assert_eq!(
            selected_row.get(),
            Some(0),
            "the drag cancels the pending row selection"
        );
        assert!(
            !scene.all(|node| node.content_offset().y > 0.0).is_empty(),
            "the drag scrolls a list node"
        );
    });
}

#[test]
fn touch_menus_drill_into_branches_in_one_modal_and_dismiss_after_activation() {
    let fixture = Fixture::new(Sample::Menu);
    let calls = fixture.calls.clone();
    InteractiveScene::run(390.0, 844.0, |scene| {
        scene.install(fixture.clone());
        dump(scene);
        let trigger = titled_button(scene, "Commands").expect("the menu trigger is mounted");
        scene.click(&trigger, 1);
        assert!(
            scene.named("touch-overlay-sheet").is_some(),
            "the touch menu presents as a sheet"
        );
        assert!(scene.focus.has_modal_scope(), "the sheet is modal");
        let labelled = |title: &str| {
            scene.first(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
                    && cove_core::scene::AccessibilityTree::resolved_label(node) == title
            })
        };
        for title in ["Open", "More", "Delete", "Unavailable"] {
            let label = scene
                .first(|node| {
                    node.layout_node()
                        .and_then(|layout| layout.text_inputs())
                        .is_some_and(|inputs| inputs.text == title)
                })
                .unwrap_or_else(|| panic!("the sheet lists {title}"));
            assert!(
                label.frame().width > 0.0 && label.frame().height > 0.0,
                "visible menu label: {title}"
            );
        }
        scene.click(&labelled("More").expect("More"), 1);
        assert!(
            scene.named("touch-menu-back").is_some(),
            "More drills in place"
        );
        scene.click(&labelled("Export").expect("Export"), 1);
        assert_eq!(calls.get(), 10, "Export runs");
        assert!(
            scene.named("touch-overlay-sheet").is_none(),
            "activation dismisses the sheet"
        );
        assert_eq!(
            scene.focus.focused().map(|node| node.id()),
            Some(trigger.id()),
            "focus returns to the trigger"
        );
    });
}

#[test]
fn popover_sheets_avoid_the_keyboard_and_restore_anchored_presentation_for_pointer_input() {
    let fixture = Fixture::new(Sample::Popover);
    let surface = fixture.surface.clone();
    let presented = fixture.presented.clone();
    let calls = fixture.calls.clone();
    surface.set(Environment {
        keyboard: Keyboard::new(300.0),
        ..environment(390.0, 844.0)
    });
    InteractiveScene::run(390.0, 844.0, |scene| {
        scene.install(fixture.clone());
        dump(scene);
        let details = titled_button(scene, "Details").expect("the popover trigger is mounted");
        scene.click(&details, 1);
        dump(scene);
        let commit = scene
            .labelled("Commit", Some(AccessibilityRole::Button))
            .expect("the popover content is mounted");
        assert!(
            commit.absolute_frame().max_y() <= 544.0,
            "GAP: the popover ignores the 300pt keyboard, so Commit ends at {}",
            commit.absolute_frame().max_y()
        );
        scene.click(&commit, 1);
        assert_eq!(calls.get(), 1, "Commit runs its action once");
        assert!(!presented.get(), "Commit dismisses the popover");

        let mut desktop = environment(390.0, 844.0);
        desktop.idiom = Idiom::Desktop;
        desktop.input = POINTER_INPUT;
        desktop.keyboard = Keyboard::default();
        surface.set(desktop);
        scene.settle(5, 1.0);
        let details =
            titled_button(scene, "Details").expect("the trigger survives the surface change");
        scene.click(&details, 1);
        assert!(presented.get(), "the pointer popover presents");
        assert!(
            !scene.portal.entries().is_empty(),
            "the pointer popover uses the portal"
        );
    });
}

#[test]
fn a_hold_cancels_the_child_tap_and_short_taps_and_motion_do_not_open_a_context_menu() {
    let fixture = Fixture::new(Sample::Context);
    let calls = fixture.calls.clone();
    InteractiveScene::run(390.0, 844.0, |scene| {
        scene.install(fixture.clone());
        let target = scene
            .labelled("Target", Some(AccessibilityRole::Button))
            .expect("the context target is mounted");
        let frame = target.absolute_frame();
        let event = MouseButtonEvent {
            button: MouseButton::Left,
            x: frame.x + frame.width / 2.0,
            y: frame.y + frame.height / 2.0,
            clicks: 1,
            modifiers: Default::default(),
            kind: Default::default(),
        };
        let dispatcher = scene.dispatcher();
        dispatcher.dispatch(&InputEvent::MouseButtonDown(event.clone()));
        scene.settle(1, 0.2);
        assert!(
            scene.named("touch-overlay-sheet").is_none(),
            "a 0.2s hold does not open a menu"
        );
        dispatcher.dispatch(&InputEvent::MouseButtonUp(event.clone()));
        scene.settle(5, 1.0);
        assert_eq!(
            calls.get(),
            1_000_000,
            "the short hold still taps the child"
        );

        dispatcher.dispatch(&InputEvent::MouseButtonDown(event.clone()));
        dispatcher.dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
            x: event.x + 30.0,
            y: event.y,
            delta_x: 30.0,
            delta_y: 0.0,
            kind: Default::default(),
        }));
        scene.settle(1, 1.0);
        assert!(
            scene.named("touch-overlay-sheet").is_none(),
            "motion cancels the hold"
        );
        dispatcher.dispatch(&InputEvent::MouseButtonUp(event.clone()));
        scene.settle(5, 1.0);

        let before = calls.get();
        dispatcher.dispatch(&InputEvent::MouseButtonDown(event.clone()));
        scene.settle(1, 0.6);
        assert!(
            scene.named("touch-overlay-sheet").is_some(),
            "a 0.6s hold opens the context menu sheet"
        );
        dispatcher.dispatch(&InputEvent::MouseButtonUp(event.clone()));
        scene.settle(5, 1.0);
        assert_eq!(
            calls.get(),
            before + 10_000,
            "the recognised hold opens the menu instead of tapping the child"
        );
    });
}

#[test]
fn phone_dialogs_rise_from_the_bottom_edge_above_the_home_indicator_pointer_dialogs_stay_centered()
{
    let fixture = Fixture::new(Sample::Dialog);
    let surface = fixture.surface.clone();
    let presented = fixture.presented.clone();
    let calls = fixture.calls.clone();
    InteractiveScene::run(390.0, 844.0, |scene| {
        scene.install(fixture.clone());
        let show = scene
            .labelled("Show", Some(AccessibilityRole::Button))
            .expect("the Show button is mounted");
        scene.click(&show, 1);
        let dialog = scene
            .first(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::Dialog)
            })
            .expect("the dialog is presented");
        let frame = dialog.absolute_frame();
        assert_eq!(frame.width, 390.0, "the phone dialog spans the window");
        assert_eq!(
            frame.max_y(),
            844.0,
            "the phone dialog reaches the bottom edge"
        );
        assert!(
            scene
                .labelled("Close dialog", Some(AccessibilityRole::Button))
                .is_none(),
            "the touch sheet has no close control"
        );
        let save = scene
            .labelled("Save", Some(AccessibilityRole::Button))
            .expect("the footer is mounted");
        assert!(
            save.absolute_frame().max_y() <= 820.0,
            "the footer stays above the home indicator; measured {}",
            save.absolute_frame().max_y()
        );
        assert!(
            save.absolute_frame().height >= 44.0,
            "the footer action is a 44pt target; measured {}",
            save.absolute_frame().height
        );
        scene.click(&save, 1);
        assert_eq!(calls.get(), 1, "Save runs its action");
        assert!(!presented.get(), "Save dismisses the dialog");

        let mut desktop = environment(390.0, 844.0);
        desktop.idiom = Idiom::Desktop;
        desktop.input = POINTER_INPUT;
        surface.set(desktop);
        scene.settle(5, 1.0);
        let show = scene
            .labelled("Show", Some(AccessibilityRole::Button))
            .expect("the Show button survives");
        scene.click(&show, 1);
        assert!(
            scene
                .labelled("Close dialog", Some(AccessibilityRole::Button))
                .is_some(),
            "the pointer dialog exposes its close control"
        );
        let centered = scene
            .first(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::Dialog)
            })
            .expect("the pointer dialog is presented");
        assert!(
            centered.absolute_frame().max_y() < 844.0,
            "the pointer dialog stays centered above the bottom edge; measured {}",
            centered.absolute_frame().max_y()
        );
    });
}

#[test]
fn a_tap_opens_hover_card_detail_as_a_sheet_when_the_surface_cannot_hover() {
    let fixture = Fixture::new(Sample::HoverCard);
    let calls = fixture.calls.clone();
    InteractiveScene::run(390.0, 844.0, |scene| {
        scene.install(fixture.clone());
        dump(scene);
        assert!(
            scene
                .labelled("Follow", Some(AccessibilityRole::Button))
                .is_none(),
            "the hover card starts closed"
        );
        let profile = find_text(scene, "Profile");
        scene.click(&profile, 1);
        dump(scene);
        dump(scene);
        assert!(
            scene.named("touch-overlay-sheet").is_none(),
            "GAP: the touch hover card sheet publishes no touch-overlay-sheet debug name"
        );
        assert!(
            !scene.portal.entries().is_empty(),
            "the tap presents the hover card detail"
        );
        let follow = scene
            .labelled("Follow", Some(AccessibilityRole::Button))
            .expect("the detail action materialises");
        scene.click(&follow, 1);
        assert_eq!(calls.get(), 1, "the sheet action runs once");
    });
}

#[test]
fn a_drag_from_the_leading_edge_pops_the_page_and_a_short_pull_springs_back() {
    let fixture = Fixture::new(Sample::Navigation);
    let route = fixture.route.clone();
    let surface = fixture.surface.clone();
    InteractiveScene::run(390.0, 844.0, |scene| {
        scene.install(fixture.clone());
        let open = scene
            .labelled("Open", Some(AccessibilityRole::Button))
            .expect("the root page is mounted");
        scene.click(&open, 1);
        assert_eq!(route.get(), vec![1], "Open pushes the detail route");

        let drag = |scene: &InteractiveScene, to: f32| {
            let dispatcher = scene.dispatcher();
            dispatcher.dispatch(&InputEvent::MouseButtonDown(MouseButtonEvent {
                button: MouseButton::Left,
                x: 4.0,
                y: 400.0,
                clicks: 1,
                modifiers: Default::default(),
                kind: Default::default(),
            }));
            dispatcher.dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
                x: 20.0,
                y: 401.0,
                delta_x: 16.0,
                delta_y: 1.0,
                kind: Default::default(),
            }));
            scene.settle(1, 0.1);
            dispatcher.dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
                x: to,
                y: 402.0,
                delta_x: to - 20.0,
                delta_y: 1.0,
                kind: Default::default(),
            }));
            scene.settle(1, 0.5);
            dispatcher.dispatch(&InputEvent::MouseButtonUp(MouseButtonEvent {
                button: MouseButton::Left,
                x: to,
                y: 402.0,
                clicks: 1,
                modifiers: Default::default(),
                kind: Default::default(),
            }));
            scene.settle(5, 1.0);
        };

        drag(scene, 60.0);
        assert_eq!(route.get(), vec![1], "a short pull springs back");
        assert!(
            scene.named("detail-page").is_some(),
            "the detail page stays mounted"
        );

        drag(scene, 260.0);
        assert!(route.get().is_empty(), "a long pull pops the page");

        let open = scene
            .labelled("Open", Some(AccessibilityRole::Button))
            .expect("the root page is restored");
        scene.click(&open, 1);
        assert_eq!(route.get(), vec![1], "Open pushes again after the pop");
        route.set(Vec::new());
        scene.settle(5, 1.0);
        assert!(route.get().is_empty(), "the route binding clears the stack");

        let open = scene
            .labelled("Open", Some(AccessibilityRole::Button))
            .expect("the root page is mounted");
        scene.click(&open, 1);
        surface.set(with_input(fixture.surface.get(), POINTER_INPUT));
        scene.settle(5, 1.0);
        drag(scene, 260.0);
        assert_eq!(
            route.get(),
            vec![1],
            "pointer input disables the leading-edge pop"
        );
    });
}

#[test]
fn a_tap_opens_a_touch_section_row_and_its_label_lines_up_with_the_rows_pointer_clicks_only_select()
{
    let fixture = Fixture::new(Sample::Sections);
    let selected_row = fixture.selected_row.clone();
    let activated = fixture.activated.clone();
    let surface = fixture.surface.clone();
    fixture.selected_row.set(None);
    InteractiveScene::run(390.0, 844.0, |scene| {
        scene.install(fixture.clone());
        let header = find_text(scene, "Recent");
        let row = find_text(scene, "Entry 0");
        assert_eq!(
            (header.absolute_frame().x, row.absolute_frame().x),
            (16.0, 16.0),
            "section headers and row labels share the leading inset"
        );
        let entry_two = find_text(scene, "Entry 2");
        scene.click(&entry_two, 1);
        assert_eq!(
            (activated.get(), selected_row.get()),
            (Some(2), Some(2)),
            "a touch tap selects and activates Entry 2"
        );

        activated.set(None);
        surface.set(with_input(fixture.surface.get(), POINTER_INPUT));
        scene.settle(5, 1.0);
        let entry_one = find_text(scene, "Entry 1");
        scene.click(&entry_one, 1);
        assert_eq!(
            activated.get(),
            None,
            "a pointer click does not activate the row"
        );
        assert_eq!(
            selected_row.get(),
            Some(1),
            "a pointer click selects Entry 1"
        );
    });
}
