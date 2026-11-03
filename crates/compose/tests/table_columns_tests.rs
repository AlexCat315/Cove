//! Translation of `Tests/CoveUIComposeTests/Adaptive/TableColumnsTests.swift`.
//!
//! Behaviour checklist:
//! - the column plan keeps the frozen prefix, honours overrides and pinned IDs,
//!   drops the frozen lane when it fills the viewport, and resolves equal
//!   priorities in authored order
//! - both tables mount the planned column set across width, text scale, input
//!   capabilities and theme, and settle without pending work
//! - a user-forced column set reveals a hidden column, retains resized widths
//!   and selection, and returns to the automatic plan when cleared
//! - a live edit survives automatic hiding and commits; a navigation request
//!   reveals the hidden column it targets
//! - sorting a hidden column keeps its ordering; surviving cells keep identity
//! - unbound visibility choices are per-mount; an empty dataset drops every
//!   cell; a frozen draft survives narrowing and cancels on Escape
//!
//! Named differences from the Swift suite:
//! - The shared `ColumnVisibilityPicker` supplies the "Column visibility"
//!   trigger and the "Automatic columns" menu entry. Menu rows are the shared
//!   menu's own height rather than Swift's `MenuRowMetrics` override, and the
//!   trigger carries the visible-column summary Swift keeps beside it.
//! - `F2` mounts the cell editor and moves focus into it, but the Rust
//!   `TextField` has no select-all chord, so the tests select with
//!   Shift+Home. `Return` and clicking away both commit; `Escape` cancels.
//! - `UIDiagnostics` lives in crates this one does not depend on, so
//!   `ai_queries` applies the same role/label/input-enabled filters to the node
//!   tree.

use crate::support;
use cove_composition::Scope;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use cove_composition::{AnyView, EmptyView, LayoutDebugExt, TupleView, View};
use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};
use cove_core::{Binding, Environment, InputCapabilities, State};
use cove_platform::{InputEvent, KeyModifiers};
use cove_ui_compose::BoxView;
use cove_ui_compose::Scancode;
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::interaction::AccessibilityExt;
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::data::data_table::table_column::{
    ColumnVisibility, ComparisonResult, TableColumn, TableColumnLayout, TableColumnPlan, TableSort,
    TableTextEditing,
};
use cove_ui_compose::primitives::data::data_table::{
    DataTable, DataTableModel, DataTableNavigation, DataTableOptions, DataTableSelection,
};
use cove_ui_compose::primitives::data::lists::Selection;
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::styles::circular_appearance::ThemeTransition;
use cove_ui_compose::theme::{ReducedMotionViewExt, TextScaler, TextScalerViewExt};

use support::InteractiveScene;

const HEIGHT: f32 = 360.0;

#[derive(Clone, Debug, PartialEq)]
struct Record {
    id: i32,
    name: String,
}

fn records(count: i32) -> Vec<Record> {
    (0..count)
        .map(|id| Record {
            id,
            name: format!("Record {id}"),
        })
        .collect()
}

fn columns() -> Vec<TableColumn<Record>> {
    ["Name", "Notes", "Quantity", "Category"]
        .iter()
        .enumerate()
        .map(|(index, title)| {
            let title = (*title).to_string();
            let cell_title = title.clone();
            let mut column =
                TableColumn::new(format!("c{index}"), title, move |record: &Record| {
                    AnyView::new(
                        Text::new(format!("{cell_title}: {}", record.name))
                            .debug_name(&format!("column-value-{index}-{}", record.id)),
                    )
                });
            column.layout = TableColumnLayout {
                width: if index == 0 { 140.0 } else { 100.0 },
                ..TableColumnLayout::default()
            };
            column.visibility = ColumnVisibility::Optional;
            // Swift's [-100, 0, 10, 5]: Notes leaves the plan after Category.
            column.priority = [-100, 0, 10, 5][index];
            column.compare = Some(Rc::new(|lhs: &Record, rhs: &Record| {
                match lhs.id.cmp(&rhs.id) {
                    std::cmp::Ordering::Equal => ComparisonResult::Same,
                    std::cmp::Ordering::Less => ComparisonResult::Ascending,
                    std::cmp::Ordering::Greater => ComparisonResult::Descending,
                }
            }));
            column.text_editing = Some(
                TableTextEditing::new(
                    |record: &Record| record.name.clone(),
                    |record: &mut Record, value: &str| record.name = value.to_string(),
                )
                .validate(|value| value.is_empty().then(|| "Name is required".to_string())),
            );
            column
        })
        .collect()
}

fn bind<T: Clone + Send + 'static>(state: &State<T>) -> Binding<T> {
    let read = state.clone();
    let write = state.clone();
    Binding::new(move || read.get(), move |value| write.set(value))
}

#[derive(Clone)]
struct Harness {
    controlled: bool,
    model: Rc<DataTableModel<Record, i32>>,
    width: State<f32>,
    scale: State<f32>,
    input: State<InputCapabilities>,
    dark: State<bool>,
    enabled: State<bool>,
    overrides: State<HashMap<String, bool>>,
    widths: State<BTreeMap<String, f32>>,
    selection: State<DataTableSelection<i32>>,
    selected: State<Option<i32>>,
    navigation: State<DataTableNavigation<i32>>,
}

impl Harness {
    fn new(controlled: bool) -> Self {
        Self {
            controlled,
            model: DataTableModel::try_new(records(100), |record| record.id)
                .expect("unique data table row IDs"),
            width: State::new(720.0),
            scale: State::new(1.0),
            input: State::new(InputCapabilities::POINTER | InputCapabilities::KEYBOARD),
            dark: State::new(false),
            enabled: State::new(true),
            overrides: State::new(HashMap::new()),
            widths: State::new(BTreeMap::new()),
            selection: State::new(DataTableSelection::new()),
            selected: State::new(None),
            navigation: State::new(DataTableNavigation::default()),
        }
    }
}

impl View for Harness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let widths = bind(&self.widths);
        let enabled = self.enabled.get();
        let table = AnyView::new(
            DataTable::try_new(self.model.clone(), columns())
                .expect("unique data table column IDs")
                .options(DataTableOptions {
                    selection: Some(Selection::single(bind(&self.selected))),
                    cursor: Some(bind(&self.selection)),
                    column_widths: Some(widths),
                    column_visibility: self.controlled.then(|| bind(&self.overrides)),
                    navigation: self.navigation.get(),
                    is_enabled: enabled,
                    ..DataTableOptions::default()
                })
                .flex_with(1.0, 1.0, Some(0.0)),
        );
        let after = Button::title("After table", || {}).accessibility_label("After table");

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
                    0.0,
                    AnyView::new(TupleView::new(vec![
                        table,
                        AnyView::new(after.flex_with(0.0, 0.0, None)),
                    ])),
                ))
                .flex_with(1.0, 1.0, Some(0.0)),
            )))
            .frame_width(self.width.get())
            .frame_height(HEIGHT)
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
            .reduced_motion(true),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        use cove_composition::DynamicField;
        vec![
            DynamicField::state("width", &self.width),
            DynamicField::state("scale", &self.scale),
            DynamicField::state("input", &self.input),
            DynamicField::state("dark", &self.dark),
            DynamicField::state("enabled", &self.enabled),
            DynamicField::state("overrides", &self.overrides),
            DynamicField::state("widths", &self.widths),
            DynamicField::state("selection", &self.selection),
            DynamicField::state("selected", &self.selected),
            DynamicField::state("navigation", &self.navigation),
        ]
    }
}

fn resize(scene: &mut InteractiveScene, harness: &Harness, width: f32) {
    harness.width.set(width);
    scene.width = width;
    scene.settle(10, 1.0);
}

fn cells(scene: &InteractiveScene) -> Vec<Node> {
    scene.all(|node| {
        node.accessibility()
            .is_some_and(|semantics| semantics.role == cove_core::scene::AccessibilityRole::Cell)
    })
}

fn cell_labels(scene: &InteractiveScene) -> BTreeSet<String> {
    cells(scene)
        .into_iter()
        .filter_map(|node| node.accessibility().map(|semantics| semantics.label))
        .collect()
}

fn labelled(scene: &InteractiveScene, label: &str) -> Option<Node> {
    scene.labelled(label, None)
}

fn editor_field(scene: &InteractiveScene) -> Node {
    scene
        .first(|node| {
            node.is_focusable()
                && node.accessibility().is_some_and(|semantics| {
                    semantics.role == cove_core::scene::AccessibilityRole::TextField
                        && semantics.label.is_empty()
                })
        })
        .expect("the cell editor mounts a focusable field")
}

fn type_text(scene: &InteractiveScene, text: &str) {
    scene
        .dispatcher()
        .dispatch(&InputEvent::TextInput(text.to_string()));
    scene.settle(10, 1.0);
}

#[test]
fn plans() {
    let columns = columns();
    let plan = |width: f32, overrides: HashMap<String, bool>, pinned: BTreeSet<&str>| {
        TableColumnPlan::resolve(
            &columns,
            &BTreeMap::new(),
            &overrides,
            width,
            1,
            &pinned.into_iter().map(str::to_string).collect(),
        )
    };

    assert_eq!(
        plan(720.0, HashMap::new(), BTreeSet::new()).indices,
        [0, 1, 2, 3]
    );
    assert_eq!(
        plan(378.0, HashMap::new(), BTreeSet::new()).indices,
        [0, 2, 3],
        "the lowest priority column leaves the plan first"
    );
    assert_eq!(plan(200.0, HashMap::new(), BTreeSet::new()).indices, [0]);
    assert_eq!(
        plan(
            200.0,
            HashMap::from([("c0".to_string(), false), ("c1".to_string(), true)]),
            BTreeSet::new()
        )
        .indices,
        [0, 1],
        "the frozen prefix survives an explicit hide; the forced column joins it"
    );
    assert_eq!(
        plan(200.0, HashMap::new(), BTreeSet::from(["c3"])).indices,
        [0, 3],
        "a pinned column is shown ahead of the width budget"
    );
    let narrow = plan(100.0, HashMap::new(), BTreeSet::new());
    assert_eq!(narrow.indices, [0]);
    assert_eq!(
        narrow.frozen_count, 0,
        "a frozen lane wider than the viewport unfreezes"
    );

    let all_frozen = TableColumnPlan::resolve(
        &columns,
        &BTreeMap::new(),
        &HashMap::new(),
        720.0,
        99,
        &Default::default(),
    );
    assert_eq!(all_frozen.indices, [0, 1, 2, 3]);
    assert_eq!(all_frozen.frozen_count, 4);

    let geometry = cove_ui_compose::primitives::data::data_table::DataTableColumnGeometry::new(
        vec![140.0, 100.0, 100.0, 100.0],
        4,
    );
    assert_eq!(geometry.frozen_width(), 440.0);
    assert_eq!(geometry.moving_width(), 0.0);
    assert!(geometry.visible_columns(0.0, 100.0).is_empty());

    let equal: Vec<TableColumn<i32>> = (0..3)
        .map(|index| {
            let mut column =
                TableColumn::new(index.to_string(), index.to_string(), move |value: &i32| {
                    AnyView::new(Text::new(value.to_string()))
                });
            column.visibility = ColumnVisibility::Optional;
            column.layout.width = 100.0;
            column
        })
        .collect();
    assert_eq!(
        TableColumnPlan::resolve(
            &equal,
            &BTreeMap::new(),
            &HashMap::new(),
            200.0,
            0,
            &Default::default()
        )
        .indices,
        [0, 1],
        "equal widths share the tie and the lowest authored index stays"
    );
    assert_eq!(
        TableColumnPlan::resolve(
            &equal,
            &BTreeMap::new(),
            &HashMap::from([
                ("0".to_string(), false),
                ("1".to_string(), false),
                ("2".to_string(), false)
            ]),
            0.0,
            0,
            &Default::default(),
        )
        .indices,
        [0],
        "a fully hidden table keeps one anchor column"
    );
}

#[test]
fn matrix() {
    {
        InteractiveScene::run(720.0, HEIGHT, |scene| {
            let harness = Harness::new(true);
            scene.install(harness.clone());

            for width in [390.0f32, 720.0, 1280.0] {
                for scale in [1.0f32, 2.0] {
                    for input in [
                        InputCapabilities::TOUCH,
                        InputCapabilities::TOUCH
                            | InputCapabilities::POINTER
                            | InputCapabilities::KEYBOARD,
                        InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
                    ] {
                        for dark in [false, true] {
                            harness.width.set(width);
                            harness.scale.set(scale);
                            harness.input.set(input);
                            harness.dark.set(dark);
                            scene.width = width;
                            scene.settle(10, 1.0);

                            let expected: BTreeSet<String> = if width == 390.0 {
                                ["Name", "Quantity", "Category"]
                                    .into_iter()
                                    .map(str::to_string)
                                    .collect()
                            } else {
                                ["Name", "Notes", "Quantity", "Category"]
                                    .into_iter()
                                    .map(str::to_string)
                                    .collect()
                            };
                            assert_eq!(
                                cell_labels(scene),
                                expected,
                                "width={width} keeps the planned columns"
                            );

                            let picker = labelled(scene, "Column visibility")
                                .expect("the table mounts the Column visibility picker");
                            assert!(
                                picker.frame().width >= 44.0 && picker.frame().height >= 44.0,
                                "the picker is a 44pt target at {width}pt: {:?}",
                                picker.frame()
                            );
                            assert!(
                                picker.absolute_frame().max_x() <= width,
                                "the picker stays inside {width}pt: {:?}",
                                picker.absolute_frame()
                            );

                            let expected_height = 36.0 * scale;
                            let touch_floor = if input.contains(InputCapabilities::TOUCH) {
                                44.0
                            } else {
                                0.0
                            };
                            let expected_height = expected_height.max(touch_floor);
                            for cell in cells(scene) {
                                let frame = cell.frame();
                                assert!(
                                    frame.height >= expected_height - 0.5,
                                    "cell {:?} keeps its {expected_height}pt lane ({frame:?})",
                                    cell.accessibility().map(|semantics| semantics.label)
                                );
                                assert!(
                                    frame.width > 0.0 && frame.width.is_finite(),
                                    "cell keeps a finite width ({frame:?})"
                                );
                            }
                            assert!(
                                cells(scene).len() < 80,
                                "mounts fewer than 80 cells at {width}pt"
                            );
                            assert!(
                                !scene.graph.recomposer.has_pending(),
                                "settles without pending work at {width}pt"
                            );
                        }
                    }
                }
            }
        });
    }
}

#[test]
fn picker() {
    {
        InteractiveScene::run(720.0, HEIGHT, |scene| {
            let harness = Harness::new(true);
            scene.install(harness.clone());
            resize(scene, &harness, 390.0);
            harness
                .widths
                .set(BTreeMap::from([("c2".to_string(), 110.0)]));
            harness.selected.set(Some(2));
            scene.settle(10, 1.0);

            let picker = labelled(scene, "Column visibility")
                .expect("the table mounts the Column visibility picker");
            assert!(
                scene.portal.entries().is_empty(),
                "a closed picker presents nothing"
            );
            scene.click(&picker, 1);
            assert!(
                !scene.portal.entries().is_empty(),
                "the picker opens a menu"
            );
            let name = scene
                .first(|node| {
                    node.accessibility().is_some_and(|semantics| {
                        semantics.label == "Name"
                            && semantics.role == cove_core::scene::AccessibilityRole::Button
                    })
                })
                .expect("the menu lists the required Name column");
            let semantics = name.accessibility().expect("Name carries semantics");
            assert!(
                !semantics.state.is_enabled && semantics.state.is_selected,
                "the required and shown Name is disabled and selected"
            );

            assert!(
                !cell_labels(scene).contains("Notes"),
                "the automatic 390pt plan hides Notes"
            );
            scene.click(&labelled(scene, "Notes").expect("the menu lists Notes"), 1);
            scene.settle(10, 1.0);
            assert_eq!(
                harness.overrides.get().get("c1"),
                Some(&true),
                "choosing Notes forces the column shown"
            );
            harness
                .overrides
                .set(HashMap::from([("c3".to_string(), true)]));
            scene.settle(10, 1.0);
            assert!(
                cell_labels(scene).contains("Category"),
                "forcing Category shown mounts its cells"
            );
            assert_eq!(
                harness.widths.get().get("c2"),
                Some(&110.0),
                "revealing a column keeps the resized width"
            );
            assert_eq!(
                harness.selected.get(),
                Some(2),
                "revealing a column keeps the selection"
            );

            let picker = labelled(scene, "Column visibility").expect("the picker stays mounted");
            scene.click(&picker, 1);
            scene.click(
                &labelled(scene, "Automatic columns").expect("the menu offers the automatic reset"),
                1,
            );
            scene.settle(10, 1.0);
            assert!(
                harness.overrides.get().is_empty(),
                "Automatic columns clears every override"
            );
            assert!(
                !cell_labels(scene).contains("Notes"),
                "clearing the overrides returns the DataTable to the 390pt plan"
            );

            harness.enabled.set(false);
            scene.settle(10, 1.0);
            assert!(
                scene.portal.entries().is_empty() && scene.capture.target().is_none(),
                "disabling the table closes the menu and releases capture"
            );
            let picker = labelled(scene, "Column visibility").expect("the picker stays mounted");
            assert!(
                !picker
                    .accessibility()
                    .is_some_and(|semantics| semantics.state.is_enabled),
                "a disabled table disables its picker"
            );
        });
    }
}

#[test]
fn edit_and_navigation() {
    InteractiveScene::run(720.0, HEIGHT, |scene| {
        let harness = Harness::new(true);
        scene.install(harness.clone());

        let host = scene
            .first(|node| node.attachment::<bool>("dataTable.input") == Some(true))
            .expect("the table mounts an input host");
        scene.focus.focus(Some(&host), false);
        scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
        scene.key(Scancode::ARROW_RIGHT, KeyModifiers::default());
        scene.key(Scancode::F2, KeyModifiers::default());

        let field = editor_field(scene);
        assert_eq!(
            scene.focus.focused(),
            Some(field.clone()),
            "F2 hands focus into the cell editor"
        );
        type_text(scene, "Draft");
        assert!(
            labelled(scene, "Edit Notes").is_some(),
            "the editor announces Edit Notes"
        );

        harness.width.set(200.0);
        harness
            .overrides
            .set(HashMap::from([("c1".to_string(), false)]));
        scene.width = 200.0;
        scene.settle(10, 1.0);
        assert_eq!(
            scene.focus.focused(),
            Some(field),
            "an open editor keeps focus while its column is hidden and the table narrows"
        );
        assert!(
            labelled(scene, "Edit Notes").is_some(),
            "the pinned editor stays mounted"
        );
        assert_eq!(
            harness.model.record_for(&0).map(|record| record.name),
            Some("Record 0".to_string()),
            "the draft is not committed while editing"
        );

        scene.key(Scancode::RETURN, KeyModifiers::default());
        assert_eq!(
            harness.model.record_for(&0).map(|record| record.name),
            Some("Draft".to_string()),
            "Return commits the draft"
        );
        assert_eq!(
            scene.focus.focused(),
            Some(host.clone()),
            "committing returns focus to the table host"
        );
        assert_eq!(
            harness.selection.get().focused_column_id.as_deref(),
            Some("c1"),
            "the committed cell keeps the cursor on its column"
        );

        harness.navigation.set(DataTableNavigation {
            request_id: Some(1),
            row_id: Some(99),
            column_id: Some("c1".to_string()),
        });
        scene.settle(10, 1.0);
        assert_eq!(harness.overrides.get().get("c1"), Some(&true));
        assert!(
            cell_labels(scene).contains("Notes"),
            "a navigation request reveals the hidden column it targets"
        );
    });
}

#[test]
fn sort_and_focus() {
    InteractiveScene::run(720.0, HEIGHT, |scene| {
        let harness = Harness::new(true);
        scene.install(harness.clone());
        harness.selected.set(Some(3));
        scene.settle(10, 1.0);

        let sort = labelled(scene, "Sort by Notes").expect("the header mounts");
        scene.click(&sort, 1);
        let sort = labelled(scene, "Sort by Notes").expect("the header stays mounted");
        scene.click(&sort, 1);
        scene.settle(10, 1.0);
        let first = scene
            .named("column-value-0-99")
            .expect("the descending sort mounts the last record first");

        resize(scene, &harness, 390.0);
        assert_eq!(
            scene.named("column-value-0-99"),
            Some(first.clone()),
            "the surviving cell keeps its node across the narrowing"
        );
        assert_eq!(harness.selected.get(), Some(3));

        resize(scene, &harness, 720.0);
        assert_eq!(
            scene.named("column-value-0-99"),
            Some(first),
            "widening again keeps the same node"
        );
    });
}

#[test]
fn local_session() {
    {
        InteractiveScene::run(720.0, HEIGHT, |scene| {
            let harness = Harness::new(false);
            scene.install(harness.clone());
            resize(scene, &harness, 390.0);

            assert!(
                !cell_labels(scene).contains("Notes"),
                "the unbound DataTable applies its automatic 390pt plan"
            );
            assert!(cell_labels(scene).contains("Category"));

            // An unbound choice belongs to this mounted table, so the picker
            // writes the session and nothing outside it.
            scene.click(
                &labelled(scene, "Column visibility").expect("the picker mounts"),
                1,
            );
            scene.click(&labelled(scene, "Notes").expect("the menu lists Notes"), 1);
            scene.settle(10, 1.0);
            assert!(
                cell_labels(scene).contains("Notes"),
                "the local choice reveals Notes"
            );

            resize(scene, &harness, 720.0);
            resize(scene, &harness, 390.0);
            assert!(
                cell_labels(scene).contains("Notes"),
                "the local choice survives a width round trip"
            );

            scene.graph.install(EmptyView, Default::default());
            scene.install(harness.clone());
            scene.settle(10, 1.0);
            assert!(
                !cell_labels(scene).contains("Notes"),
                "remounting resets the local choices"
            );
        });
    }
}

#[test]
fn ai_queries() {
    InteractiveScene::run(720.0, HEIGHT, |scene| {
        let harness = Harness::new(true);
        scene.install(harness.clone());
        resize(scene, &harness, 390.0);

        let count = |scene: &InteractiveScene, label: &str| {
            scene
                .nodes()
                .into_iter()
                .filter(|node| {
                    node.accepts_subtree_input()
                        && node.accessibility().is_some_and(|semantics| {
                            semantics.role == cove_core::scene::AccessibilityRole::Cell
                                && semantics.label == label
                        })
                })
                .count()
        };

        assert_eq!(
            count(scene, "Notes"),
            0,
            "the automatically hidden column is not discovered"
        );
        harness
            .overrides
            .set(HashMap::from([("c1".to_string(), true)]));
        scene.settle(10, 1.0);
        assert!(
            count(scene, "Notes") > 0,
            "the explicitly revealed column is discovered"
        );

        harness.enabled.set(false);
        scene.settle(10, 1.0);
        assert_eq!(
            count(scene, "Notes"),
            0,
            "a disabled DataTable drops its cells from input-enabled discovery"
        );
    });
}

#[test]
fn empty_and_frozen() {
    InteractiveScene::run(720.0, HEIGHT, |scene| {
        let harness = Harness::new(true);
        scene.install(harness.clone());
        resize(scene, &harness, 200.0);

        let rows = scene.all(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == cove_core::scene::AccessibilityRole::Row)
        });
        assert!(!rows.is_empty(), "a narrow table still mounts rows");

        let groups = scene.all(|node| {
            node.accessibility().is_some_and(|semantics| {
                semantics.role == cove_core::scene::AccessibilityRole::Group
                    && semantics.label.starts_with("Row ")
            })
        });
        assert!(
            groups.is_empty(),
            "the sole frozen lane mounts no Group row labelled {:?}",
            groups
                .first()
                .and_then(|node| node.accessibility().map(|semantics| semantics.label))
        );
        assert_eq!(
            cell_labels(scene),
            ["Name"].into_iter().map(str::to_string).collect(),
            "a 200pt viewport keeps only the frozen column"
        );

        harness.model.replace_rows(vec![]).unwrap();
        resize(scene, &harness, 720.0);
        for title in ["Name", "Notes", "Quantity", "Category"] {
            assert!(
                labelled(scene, &format!("Sort by {title}")).is_some(),
                "an empty table keeps the Sort by {title} header button"
            );
        }
        assert!(cells(scene).is_empty(), "an empty dataset mounts no cells");
    });
}

#[test]
fn frozen_edit() {
    InteractiveScene::run(720.0, HEIGHT, |scene| {
        let harness = Harness::new(true);
        scene.install(harness.clone());

        let host = scene
            .first(|node| node.attachment::<bool>("dataTable.input") == Some(true))
            .expect("the table mounts an input host");
        scene.focus.focus(Some(&host), false);
        scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
        scene.key(Scancode::F2, KeyModifiers::default());
        let field = editor_field(scene);
        assert_eq!(
            scene.focus.focused(),
            Some(field.clone()),
            "F2 hands focus into the frozen editor"
        );
        type_text(scene, "Uncommitted");

        resize(scene, &harness, 150.0);
        assert_eq!(
            harness.model.record_for(&0).map(|record| record.name),
            Some("Record 0".to_string()),
            "the frozen draft does not reach the model before commit"
        );
        assert_eq!(
            scene.focus.focused(),
            Some(field.clone()),
            "the frozen editor keeps focus at 150pt"
        );
        assert!(
            labelled(scene, "Edit Name").is_some(),
            "the frozen editor stays mounted at 150pt"
        );

        scene.key(Scancode::ESCAPE, KeyModifiers::default());
        assert_eq!(
            harness.model.record_for(&0).map(|record| record.name),
            Some("Record 0".to_string()),
            "Escape drops the draft"
        );

        scene.key(Scancode::F2, KeyModifiers::default());
        let field = editor_field(scene);
        type_text(scene, "Committed by blur");
        let after = labelled(scene, "After table").expect("the trailing button is mounted");
        scene.click(&after, 1);
        scene.settle(10, 1.0);
        assert_eq!(
            harness.model.record_for(&0).map(|record| record.name),
            Some("Committed by blur".to_string()),
            "clicking away commits the draft"
        );
        assert_eq!(
            scene.focus.focused(),
            Some(after),
            "focus moves to the clicked button"
        );
    });
}
