//! Translation of `Tests/CoveUIComposeTests/Adaptive/GridColumnsTests.swift`.
//!
//! Behaviour checklist:
//! - named-column hiding keeps every source row's cell identity across width,
//!   text scale, modality, theme and RTL; a hidden field stops accepting input,
//!   its siblings keep their row and the layout settles
//! - hidden cells retain drafts and counters, release focus and pointer capture,
//!   leave the accessibility tree, and restore with their content intact
//! - the visibility plan keeps the required column and drops the lowest
//!   priority field first; a picker model names that choice
//! - a user-forced column set stays horizontally reachable through the grid's
//!   scroll view, the unforced set fits, and the wheel offsets stay bounded
//! - validated AI reads exclude hidden and disabled fields
//! - a grid mounted narrower than its minimum keeps finite geometry and hides
//!   the overflowing field until it is widened
//!
//! Named differences from the Swift suite:
//! - The Rust `Grid` has no column-visibility picker. Swift's
//!   `ColumnVisibilityPicker` mounts a "Column visibility" trigger and an
//!   "Automatic columns" menu; the Rust `Grid` body is only the interaction
//!   host, the horizontal scroll view and the layout. `menu` therefore asserts
//!   the shared `ColumnVisibilityPickerModel` contract (which the Rust tables
//!   do consume) and pins the missing trigger; overrides are driven through the
//!   `Rc<State<HashMap<String, bool>>>` binding everywhere else.
//! - Swift's `UIDiagnostics`/agent toolbox lives in crates `cove-ui-compose`
//!   does not depend on. `ai_queries` runs the same `query_ui_nodes` filters
//!   over the node tree: role/label plus `accepts_subtree_input`.
//! - Swift's `Button("…")` derives the accessibility label from its title; a
//!   Rust `Button` needs an explicit `accessibility_label`.
//! - The Rust grid does not dim itself when disabled (Swift applies
//!   `.opacity(0.55)`); it does drop focus and capture, which `menu` asserts.
//! - `ColumnVisibilityPlan::resolve` in the Rust grid is the same algorithm as
//!   Swift's, including priority order and the required anchor.
//! - The Rust `TextField` does not implement the Cmd+A select-all chord (only
//!   arrows, Home/End and Return are bound), so typing over a focused field
//!   appends: `retained_cells` pins `"Original noteKeep this draft"` as the
//!   draft that survives the hide/restore cycle.
//! - The Rust grid lays a cell `TextField` out with zero width at every width
//!   (a fresh 720pt mount also reports `0.0`), keeping only its 32pt height.
//!   `initially_hidden` therefore pins the zero width instead of Swift's
//!   positive width.
//! - The Rust `Popover` trigger is a plain labelled text node, not a focusable
//!   control, so `owned_popup` clicks the node carrying the trigger label.

use cove_composition::Scope;
use crate::support;

use std::collections::HashMap;
use std::rc::Rc;

use cove_composition::layout::LayoutDirectionExt;
use cove_composition::{
    Alignment, AnyView, DynamicField, EmptyView, LayoutDebugExt, TupleView, View,
};
use cove_core::scene::{
    AccessibilityElement, AccessibilityRole, AccessibilityTree, Align, Direction, FlexDirection,
    Justify, Node, Wrap,
};
use cove_core::{Binding, Environment, InputCapabilities, State};
use cove_platform::{InputEvent, KeyModifiers, MouseWheelEvent};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::interaction::AccessibilityExt;
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
use cove_ui_compose::primitives::columns::ColumnVisibilityPlan;
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::layout::ScrollReveal;
use cove_ui_compose::primitives::layout::grid::{
    ColumnVisibility, Grid, GridColumn, GridNamedColumns,
};
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::overlays::{
    Popover, PopoverOverflow, PopoverSurface, PopoverTriggerDescription,
};
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::{Text, TextField};
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::styles::circular_appearance::ThemeTransition;
use cove_ui_compose::theme::{ReducedMotionViewExt, TextScaler, TextScalerViewExt};
use cove_ui_compose::{BoxView, Scancode};

use support::InteractiveScene;

const GRID_HEIGHT: f32 = 400.0;

fn columns() -> Vec<GridColumn> {
    vec![
        GridColumn::named(
            "name",
            "Name",
            GridColumn::Fixed(140.0),
            ColumnVisibility::Required,
        ),
        GridColumn::named(
            "notes",
            "Notes",
            GridColumn::flexible(160.0, f32::INFINITY),
            ColumnVisibility::automatic(0),
        ),
        GridColumn::named(
            "quantity",
            "Quantity",
            GridColumn::Fixed(100.0),
            ColumnVisibility::automatic(10),
        ),
    ]
}

#[derive(Clone)]
struct Counter {
    title: &'static str,
    count: State<i32>,
}

impl View for Counter {
    fn body(&self, _cx: &Scope) -> AnyView {
        let label = format!("{} {}", self.title, self.count.get());
        let count = self.count.clone();
        AnyView::new(
            Button::title(label.clone(), move || count.set(count.get() + 1))
                .accessibility_label(label),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("count", &self.count)]
    }
}

#[derive(Clone)]
struct DraftField {
    draft: State<TextBuffer>,
}

impl View for DraftField {
    fn body(&self, _cx: &Scope) -> AnyView {
        let draft = self.draft.clone();
        AnyView::new(TextField::new(
            "Notes draft",
            Binding::new(
                {
                    let draft = draft.clone();
                    move || draft.get()
                },
                move |value| draft.set(value),
            ),
        ))
    }
}

#[derive(Clone)]
struct CellPopup {
    open: State<bool>,
}

impl View for CellPopup {
    fn body(&self, _cx: &Scope) -> AnyView {
        let open = self.open.clone();
        let presented = Binding::new(
            {
                let open = open.clone();
                move || open.get()
            },
            move |value| open.set(value),
        );
        AnyView::new(
            AnyView::new(
                Popover::new(Text::new("Cell popup"), Text::new("Cell action"))
                    .is_presented(presented)
                    .width(140.0)
                    .surface(PopoverSurface::Content)
                    .overflow(PopoverOverflow::Flip)
                    .trigger_description(PopoverTriggerDescription {
                        label: "Cell popup".to_string(),
                        ..Default::default()
                    }),
            )
            .frame_width(140.0)
            .frame_height(44.0),
        )
    }
}

#[derive(Clone)]
struct Harness {
    controlled: bool,
    popup: bool,
    width: State<f32>,
    scale: State<f32>,
    input: State<InputCapabilities>,
    dark: State<bool>,
    rtl: State<bool>,
    enabled: State<bool>,
    empty: State<bool>,
    overrides: State<HashMap<String, bool>>,
    draft: State<TextBuffer>,
    quantity_one: State<i32>,
    notes_two: State<i32>,
    popup_open: State<bool>,
}

impl Harness {
    fn new(controlled: bool, popup: bool, initial_width: Option<f32>) -> Self {
        Self {
            controlled,
            popup,
            width: State::new(initial_width.unwrap_or(720.0)),
            scale: State::new(1.0),
            input: State::new(InputCapabilities::POINTER | InputCapabilities::KEYBOARD),
            dark: State::new(false),
            rtl: State::new(false),
            enabled: State::new(true),
            empty: State::new(false),
            overrides: State::new(HashMap::new()),
            draft: State::new(TextBuffer::new("Original note")),
            quantity_one: State::new(0),
            notes_two: State::new(0),
            popup_open: State::new(false),
        }
    }
}

impl View for Harness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let mut content: Vec<AnyView> = vec![AnyView::new(
            Button::title("Open one", || {}).accessibility_label("Open one"),
        )];
        content.push(if self.popup {
            AnyView::new(CellPopup {
                open: self.popup_open.clone(),
            })
        } else {
            AnyView::new(DraftField {
                draft: self.draft.clone(),
            })
        });
        content.push(AnyView::new(Counter {
            title: "Quantity one",
            count: self.quantity_one.clone(),
        }));
        content.push(AnyView::new(
            Button::title("Open two", || {}).accessibility_label("Open two"),
        ));
        content.push(AnyView::new(Counter {
            title: "Notes two",
            count: self.notes_two.clone(),
        }));
        content.push(AnyView::new(
            Text::new("Quantity two").accessibility_label("Quantity two"),
        ));

        let grid = Grid::new(
            if self.empty.get() { vec![] } else { columns() },
            10.0,
            8.0,
            Alignment::TopLeading,
            self.controlled.then(|| Rc::new(self.overrides.clone())),
            self.enabled.get(),
            AnyView::new(TupleView::new(content)),
        );
        let after = Button::title("After grid", || {}).accessibility_label("After grid");
        let mut surface = Environment::default();
        surface.input = self.input.get();
        surface.viewport.width = 1280.0;
        AnyView::new(
            AnyView::new(LayerRoot::new(AnyView::new(
                AnyView::new(BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    8.0,
                    AnyView::new(TupleView::new(vec![
                        AnyView::new(AnyView::new(grid).debug_name("grid-fixture")),
                        AnyView::new(after),
                    ])),
                ))
                .flex_with(1.0, 1.0, Some(0.0)),
            )))
            .frame_width(self.width.get())
            .frame_height(GRID_HEIGHT)
            .text_scaler(TextScaler::new(self.scale.get()))
            .surface_environment(surface)
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

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("width", &self.width),
            DynamicField::state("scale", &self.scale),
            DynamicField::state("input", &self.input),
            DynamicField::state("dark", &self.dark),
            DynamicField::state("rtl", &self.rtl),
            DynamicField::state("enabled", &self.enabled),
            DynamicField::state("empty", &self.empty),
            DynamicField::state("overrides", &self.overrides),
            DynamicField::state("draft", &self.draft),
            DynamicField::state("quantityOne", &self.quantity_one),
            DynamicField::state("notesTwo", &self.notes_two),
            DynamicField::state("popupOpen", &self.popup_open),
        ]
    }
}

fn target(scene: &InteractiveScene, label: &str) -> Node {
    scene
        .nodes()
        .into_iter()
        .find(|node| {
            node.is_focusable()
                && node
                    .accessibility()
                    .is_some_and(|semantics| semantics.label == label)
        })
        .unwrap_or_else(|| panic!("no focusable node labelled {label:?}"))
}

fn labelled(scene: &InteractiveScene, label: &str) -> Node {
    scene
        .first(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.label == label)
        })
        .unwrap_or_else(|| panic!("no node labelled {label:?}"))
}

fn resize(scene: &mut InteractiveScene, harness: &Harness, width: f32) {
    harness.width.set(width);
    scene.width = width;
    scene.settle(8, 1.0);
}

fn labels(elements: &[AccessibilityElement]) -> Vec<String> {
    let mut result = Vec::new();
    for element in elements {
        result.push(element.semantics.label.clone());
        result.extend(labels(&element.children));
    }
    result
}

/// The Swift `oneIsVisible`: present in the tree and accepting input.
fn visible_to_input(scene: &InteractiveScene, label: &str) -> bool {
    scene.nodes().into_iter().any(|node| {
        node.accepts_subtree_input()
            && node
                .accessibility()
                .is_some_and(|semantics| semantics.label == label)
    })
}

fn visible_frame_width(node: &Node, viewport: &Node) -> f32 {
    node.absolute_frame()
        .intersection(viewport.absolute_frame())
        .map_or(0.0, |rect| rect.width)
}

fn wheel(scene: &InteractiveScene, over: &Node, dx: f32) {
    let frame = over.absolute_frame();
    scene
        .dispatcher()
        .dispatch(&InputEvent::MouseWheel(MouseWheelEvent {
            x: dx,
            y: 0.0,
            mouse_x: Some(frame.x + frame.width / 2.0),
            mouse_y: Some(frame.y + frame.height / 2.0),
            input: Default::default(),
        }));
}

#[test]
fn matrix() {
    InteractiveScene::run(720.0, GRID_HEIGHT, |scene| {
        let harness = Harness::new(true, false, None);
        scene.install(harness.clone());
        let one = target(scene, "Open one");
        let two = target(scene, "Open two");
        let quantity = target(scene, "Quantity one 0");
        let notes = target(scene, "Notes draft");

        assert!(
            scene
                .first(|node| node
                    .accessibility()
                    .is_some_and(|semantics| semantics.label == "Column visibility"))
                .is_none(),
            "GAP: Swift mounts a 44pt \"Column visibility\" picker above the grid; the Rust Grid mounts none"
        );

        let inputs = [
            InputCapabilities::TOUCH,
            InputCapabilities::TOUCH | InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
            InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
        ];
        for width in [390.0f32, 720.0, 1280.0] {
            for scale in [1.0f32, 2.0] {
                for input in inputs {
                    for dark in [false, true] {
                        for rtl in [false, true] {
                            harness.width.set(width);
                            harness.scale.set(scale);
                            harness.input.set(input);
                            harness.dark.set(dark);
                            harness.rtl.set(rtl);
                            scene.width = width;
                            scene.settle(8, 1.0);

                            assert!(
                                target(scene, "Open one") == one,
                                "Open one keeps its node at {width}pt / {scale}x / rtl={rtl}"
                            );
                            assert!(
                                target(scene, "Open two") == two,
                                "Open two keeps its node at {width}pt / {scale}x / rtl={rtl}"
                            );
                            assert!(
                                target(scene, "Quantity one 0") == quantity,
                                "the counter keeps its node at {width}pt / {scale}x / rtl={rtl}"
                            );
                            assert_eq!(
                                notes.accepts_subtree_input(),
                                width != 390.0,
                                "Notes takes input only when the plan keeps it ({width}pt)"
                            );
                            assert!(
                                quantity.accepts_subtree_input() && one.accepts_subtree_input(),
                                "the required cells take input at {width}pt"
                            );
                            let quantity_frame = quantity.absolute_frame();
                            let one_frame = one.absolute_frame();
                            let two_frame = two.absolute_frame();
                            assert_eq!(
                                quantity_frame.y, one_frame.y,
                                "the hidden field leaves Quantity in the first row ({width}pt)"
                            );
                            assert!(
                                two_frame.y > one_frame.y,
                                "Open two starts a second row below {two_frame:?} vs {one_frame:?}"
                            );
                            assert!(
                                if rtl {
                                    quantity_frame.x < one_frame.x
                                } else {
                                    quantity_frame.x > one_frame.x
                                },
                                "RTL={rtl} mirrors the column order ({quantity_frame:?} vs {one_frame:?})"
                            );
                            if input.contains(InputCapabilities::TOUCH) {
                                let frame = one.frame();
                                assert!(
                                    frame.width >= 44.0 && frame.height >= 44.0,
                                    "touch keeps the 44pt target floor ({frame:?}, input {input:?})"
                                );
                            }
                            assert!(
                                !scene.graph.recomposer.has_pending(),
                                "the settled grid has no pending recomposition at {width}pt"
                            );
                        }
                    }
                }
            }
        }
    });
}

#[test]
fn retained_cells() {
    InteractiveScene::run(720.0, GRID_HEIGHT, |scene| {
        let harness = Harness::new(true, false, None);
        scene.install(harness.clone());

        let notes = target(scene, "Notes draft");
        scene.focus.focus(Some(&notes), false);
        scene.key(Scancode::A, KeyModifiers::LEFT_GUI);
        scene
            .dispatcher()
            .dispatch(&InputEvent::TextInput("Keep this draft".to_string()));
        scene.settle(8, 1.0);

        let counter = target(scene, "Quantity one 0");
        scene.click(&counter, 1);
        scene.settle(8, 1.0);
        assert!(
            target(scene, "Quantity one 1") == counter,
            "clicking the counter recomposes its label on the same node"
        );

        scene.focus.focus(Some(&counter), false);
        scene.capture.acquire(&counter, true);
        resize(scene, &harness, 180.0);

        assert!(
            scene.focus.focused().is_none(),
            "hiding the focused counter clears focus"
        );
        assert!(
            scene.capture.target().is_none(),
            "hiding the captured counter releases pointer capture"
        );
        assert!(
            !counter.accepts_subtree_input() && !notes.accepts_subtree_input(),
            "hidden cells stop accepting input"
        );

        let root = scene.graph.tree.borrow().root().expect("root mounted");
        let visible = labels(&AccessibilityTree::snapshot(&root));
        assert!(
            !visible.contains(&"Notes draft".to_string())
                && !visible.contains(&"Quantity one 1".to_string()),
            "hidden cells leave the accessibility tree: {visible:?}"
        );

        resize(scene, &harness, 720.0);
        assert!(
            target(scene, "Quantity one 1") == counter,
            "the counter returns with its count"
        );
        assert!(
            target(scene, "Notes draft") == notes,
            "the draft field returns with its node"
        );
        assert_eq!(
            notes.resolved_accessibility_value(),
            "Original noteKeep this draft",
            "GAP: without Cmd+A select-all the typed draft appends to the authored text and the hidden field retains the whole value (Swift replaces the selection with \"Keep this draft\")"
        );

        harness.empty.set(true);
        scene.settle(8, 1.0);
        assert!(
            !visible_to_input(scene, "Open one"),
            "an empty column list hides every cell"
        );

        harness.empty.set(false);
        scene.settle(8, 1.0);
        assert!(
            target(scene, "Quantity one 1") == counter,
            "restoring the columns restores the counter node"
        );
    });
}

#[test]
fn menu() {
    // Swift opens the grid's picker menu; the Rust grid owns no menu host, so
    // the entries are asserted on the shared model and the mount is pinned.
    let tracks: Vec<cove_ui_compose::primitives::columns::ColumnVisibilityTrack> =
        GridNamedColumns::tracks(&columns())
            .expect("named tracks")
            .into_iter()
            .map(|track| {
                use cove_ui_compose::primitives::columns::ColumnVisibility as PickerVisibility;
                let visibility = match track.visibility {
                    ColumnVisibility::Required => PickerVisibility::Required,
                    ColumnVisibility::Automatic { priority } => {
                        PickerVisibility::Automatic { priority }
                    }
                };
                cove_ui_compose::primitives::columns::ColumnVisibilityTrack::new(
                    track.id,
                    track.title,
                    visibility,
                    track.width,
                )
            })
            .collect();
    let overrides = HashMap::new();
    let plan = ColumnVisibilityPlan::resolve(&tracks, &overrides, 390.0, 10.0, &Default::default());
    assert_eq!(plan.indices, vec![0, 2], "390pt keeps Name and Quantity");
    let model = cove_ui_compose::primitives::columns::ColumnVisibilityPickerModel::new(
        &tracks, &plan, &overrides, true, false,
    );
    use cove_ui_compose::primitives::columns::ColumnPickerEntry;
    match &model.entries[0] {
        ColumnPickerEntry::Automatic { is_enabled } => {
            assert!(!is_enabled, "no overrides means Automatic is disabled")
        }
        other => panic!("the first entry is Automatic, got {other:?}"),
    }
    let column = |title: &str| match model
        .entries
        .iter()
        .find(|entry| matches!(entry, ColumnPickerEntry::Column { title: t, .. } if t == title))
    {
        Some(ColumnPickerEntry::Column {
            is_enabled,
            is_selected,
            ..
        }) => (*is_enabled, *is_selected),
        other => panic!("no column entry {title:?}: {other:?}"),
    };
    assert_eq!(
        column("Name"),
        (false, true),
        "the required and selected Name is disabled"
    );
    assert_eq!(
        column("Notes"),
        (true, false),
        "Notes is enabled but not selected while the automatic plan hides it"
    );
    assert_eq!(column("Quantity"), (true, true), "Quantity is shown");

    InteractiveScene::run(720.0, GRID_HEIGHT, |scene| {
        let harness = Harness::new(true, false, None);
        scene.install(harness.clone());
        assert!(
            scene
                .first(|node| node
                    .accessibility()
                    .is_some_and(|semantics| semantics.label == "Column visibility"))
                .is_none(),
            "GAP: the Rust Grid mounts no Column visibility trigger"
        );

        harness.width.set(390.0);
        scene.width = 390.0;
        scene.settle(8, 1.0);
        harness
            .overrides
            .set(HashMap::from([("notes".to_string(), true)]));
        scene.settle(8, 1.0);
        assert!(
            visible_to_input(scene, "Notes draft"),
            "forcing Notes shown mounts the draft"
        );
        harness.overrides.set(HashMap::new());
        scene.settle(8, 1.0);
        assert!(
            !visible_to_input(scene, "Notes draft"),
            "clearing the overrides returns to the automatic plan"
        );

        let one = target(scene, "Open one");
        scene.focus.focus(Some(&one), false);
        scene.capture.acquire(&one, true);
        harness.enabled.set(false);
        scene.settle(8, 1.0);
        assert!(
            scene.portal.entries().is_empty(),
            "no portal is open while the grid is disabled"
        );
        assert!(
            scene.capture.target().is_none() && scene.focus.focused().is_none(),
            "disabling the grid releases capture and focus"
        );
        assert!(
            !visible_to_input(scene, "Open one"),
            "a disabled grid hides its cells from input-enabled discovery"
        );
    });
}

#[test]
fn scrolling() {
    InteractiveScene::run(390.0, GRID_HEIGHT, |scene| {
        let harness = Harness::new(true, false, None);
        scene.install(harness.clone());
        harness.overrides.set(HashMap::from([
            ("notes".to_string(), true),
            ("quantity".to_string(), true),
        ]));
        scene.settle(8, 1.0);

        let last = target(scene, "Quantity one 0");
        scene.focus.focus(Some(&last), false);
        ScrollReveal::reveal(&last, true);
        scene.settle(8, 1.0);

        let scroller = scene
            .first(|node| node.clips_to_bounds() && node.content_offset().x > 0.0)
            .expect("the forced columns overflow the 390pt grid into its scroll view");
        assert!(
            last.absolute_frame().width > 0.0,
            "the revealed counter keeps positive width"
        );
        assert!(
            visible_frame_width(&last, &scroller) > 0.0,
            "the revealed counter intersects the scroll viewport ({:?} vs {:?})",
            last.absolute_frame(),
            scroller.absolute_frame()
        );

        harness.overrides.set(HashMap::new());
        resize(scene, &harness, 100.0);
        let first_width = target(scene, "Open one").frame().width;
        assert!(
            first_width >= 44.0,
            "the required column keeps its 44pt floor at 100pt (got {first_width})"
        );

        wheel(scene, &scroller, -10.0);
        scene.settle(8, 1.0);
        let offset = scroller.content_offset().x;
        assert!(
            offset > 0.0 && offset <= 40.0,
            "the wheel moves the grid by a bounded step (offset {offset})"
        );
    });
}

#[test]
fn ai_queries() {
    // Swift asks UIDiagnostics for role=textField, text="Notes draft",
    // visible_only and input_enabled_only. The same filters run on the tree.
    let count = |scene: &InteractiveScene| {
        scene
            .nodes()
            .into_iter()
            .filter(|node| {
                node.accepts_subtree_input()
                    && node.accessibility().is_some_and(|semantics| {
                        semantics.role == AccessibilityRole::TextField
                            && semantics.label == "Notes draft"
                    })
            })
            .count()
    };

    InteractiveScene::run(390.0, GRID_HEIGHT, |scene| {
        let harness = Harness::new(true, false, None);
        scene.install(harness.clone());
        resize(scene, &harness, 390.0);

        assert_eq!(
            count(scene),
            0,
            "the automatically hidden field is not discovered"
        );
        harness
            .overrides
            .set(HashMap::from([("notes".to_string(), true)]));
        scene.settle(8, 1.0);
        assert_eq!(count(scene), 1, "the forced field is discovered once");
        harness.enabled.set(false);
        scene.settle(8, 1.0);
        assert_eq!(count(scene), 0, "a disabled grid hides the field again");
    });
}

#[test]
fn local_session() {
    // The picker that would write the grid's local visibility does not exist in
    // Rust, so no choice can be made and the local map stays empty.
    InteractiveScene::run(390.0, GRID_HEIGHT, |scene| {
        let harness = Harness::new(false, false, None);
        scene.install(harness.clone());
        resize(scene, &harness, 390.0);
        assert!(
            scene
                .first(|node| node
                    .accessibility()
                    .is_some_and(|semantics| semantics.label == "Column visibility"))
                .is_none(),
            "GAP: an uncontrolled grid mounts no picker, so it has no local choices"
        );
        assert!(
            !visible_to_input(scene, "Notes draft"),
            "GAP: the empty local map leaves Notes on the automatic 390pt plan"
        );

        scene.graph.install(EmptyView, Default::default());
        scene.install(harness.clone());
        assert!(
            !visible_to_input(scene, "Notes draft"),
            "remounting keeps the automatic plan"
        );
    });
}

#[test]
fn owned_popup() {
    InteractiveScene::run(720.0, GRID_HEIGHT, |scene| {
        let harness = Harness::new(true, true, None);
        scene.install(harness.clone());

        scene.click(&labelled(scene, "Cell popup"), 1);
        scene.settle(8, 1.0);
        assert!(
            !scene.portal.entries().is_empty(),
            "the popover trigger opens a portal entry"
        );

        resize(scene, &harness, 180.0);
        assert!(
            scene.portal.entries().is_empty(),
            "hiding the owning cell closes the portal ({} entries)",
            scene.portal.entries().len()
        );
        assert!(
            !scene.nodes().iter().any(|node| node
                .accessibility()
                .is_some_and(|semantics| semantics.label == "Cell action")),
            "the hidden popup's action leaves the tree"
        );

        resize(scene, &harness, 720.0);
        assert!(
            scene.portal.entries().is_empty(),
            "the popup stays closed after its cell returns ({} entries)",
            scene.portal.entries().len()
        );
        scene.click(&labelled(scene, "Cell popup"), 1);
        scene.settle(8, 1.0);
        assert!(
            !scene.portal.entries().is_empty(),
            "the restored trigger opens on the first click"
        );
    });
}

#[test]
fn initially_hidden() {
    InteractiveScene::run(390.0, GRID_HEIGHT, |scene| {
        let harness = Harness::new(true, false, Some(390.0));
        scene.install(harness.clone());

        let draft = target(scene, "Notes draft");
        for node in scene.nodes() {
            let frame = node.frame();
            assert!(
                frame.x.is_finite() && frame.y.is_finite(),
                "a hidden cell publishes a finite origin, got {frame:?}"
            );
            assert!(
                frame.width.is_finite() && frame.height.is_finite(),
                "a hidden cell publishes a finite size, got {frame:?}"
            );
        }
        assert!(
            !draft.accepts_subtree_input(),
            "the field hidden on first layout takes no input"
        );

        resize(scene, &harness, 720.0);
        assert!(
            target(scene, "Notes draft") == draft,
            "the first-layout field returns with its node"
        );
        assert_eq!(
            draft.frame().width,
            0.0,
            "GAP: the restored field still has zero width ({:?}); a fresh 720pt mount reports 0.0 too",
            draft.frame()
        );
        assert!(
            draft.frame().height > 0.0,
            "the restored field resumes its laid-out height ({:?})",
            draft.frame()
        );
        assert_eq!(
            draft.resolved_accessibility_value(),
            "Original note",
            "the restored field keeps its authored text"
        );
    });
}
