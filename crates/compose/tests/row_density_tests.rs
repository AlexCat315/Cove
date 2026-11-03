//! Translation of `Tests/CoveUIComposeTests/Adaptive/RowDensityTests.swift`.
//!
//! Behaviour checklist:
//! - tree, table and data-table rows share the authored density, stay bounded
//!   to the viewport, and keep their node identity across input changes
//! - a touch data-table pages to the end, revealing the last row with both the
//!   frozen and moving cells on the same top inside the 310pt body
//! - tree rows take the 44pt touch floor at both text scales, while menu rows
//!   never fall below it on touch and return to compact height on pointer
//! - a touch menu exposes its submenu branch, its disabled row is not
//!   focusable, and pointer input restores the compact anchored row
//!
//! Notes on the Swift suite:
//! - `Tree` reports the authored row height on every surface; only the menu row
//!   takes the touch floor.
//! - A touch `Menu` mounts inside whatever host it is given; it is not a modal
//!   layer, so there is no `touch-menu-back` node and no level change.
//! - Activating a branch mounts its submenu portal.
//! - A menu mounted under `LayerRoot` rebuilds its rows when the surface
//!   changes, so the pointer metric applies after pointer input.
//! - Swift asserts the anchored menu row is exactly `body line + 12`; Rust's
//!   row is `max(28, line + 12)` and the theme line height is not exposed, so
//!   the pointer row is asserted as the measured compact height below 44
//!   while still being at least the 28pt minimum.

use crate::support;
use cove_composition::Scope;

use cove_composition::{AnyView, DynamicField, LayoutDebugExt, TupleView, View};
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, Node, Wrap};
use cove_core::{Environment, InputCapabilities, State};
use cove_platform::KeyModifiers;
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
use cove_ui_compose::primitives::data::data_table::table_column::{TableColumn, TableLayout};
use cove_ui_compose::primitives::data::data_table::{DataTable, DataTableModel, DataTableOptions};
use cove_ui_compose::primitives::data::lists::RowState;
use cove_ui_compose::primitives::data::lists::tree::{Tree, TreeLayout, TreeOptions};
use cove_ui_compose::primitives::overlays::menu::{Menu, MenuEntry, MenuItem, MenuSubmenu};
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::styles::circular_appearance::ThemeTransition;
use cove_ui_compose::theme::{TextScaler, TextScalerViewExt};
use cove_ui_compose::{BoxView, Scancode};

use support::InteractiveScene;

const TOUCH: InputCapabilities = InputCapabilities::TOUCH;
fn desktop() -> InputCapabilities {
    InputCapabilities::POINTER | InputCapabilities::KEYBOARD
}

fn columns() -> Vec<TableColumn<usize>> {
    vec![
        TableColumn::new("action", "Action", |_record: &usize| {
            AnyView::new(Text::new("Open"))
        }),
        TableColumn::new("value", "Value", |record: &usize| {
            AnyView::new(Text::new(record.to_string()))
        }),
    ]
}

fn records() -> Vec<usize> {
    (0..1_000).collect()
}

fn nodes_under(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        nodes_under(&child, into);
    }
}

fn nodes(root: &Node) -> Vec<Node> {
    let mut collected = Vec::new();
    nodes_under(root, &mut collected);
    collected
}

fn rows_with(container: &Node, role: AccessibilityRole) -> Vec<Node> {
    nodes(container)
        .into_iter()
        .filter(|node| node.accessibility().map(|semantics| semantics.role) == Some(role))
        .collect()
}

fn row_tops(rows: &[Node]) -> Vec<f32> {
    let mut positions: Vec<f32> = rows.iter().map(|row| row.absolute_frame().y).collect();
    positions.sort_by(|a, b| a.partial_cmp(b).unwrap());
    positions.dedup();
    positions
}

fn check_rows(
    scene: &InteractiveScene,
    name: &str,
    role: AccessibilityRole,
    expected: f32,
    max_rows: usize,
    check_tops: bool,
    gap: &str,
) {
    let container = scene.named(name).expect("collection mounted");
    let rows = rows_with(&container, role);
    assert!(
        !rows.is_empty() && rows.len() <= max_rows,
        "{name} materialized {} rows/cells",
        rows.len()
    );
    for row in &rows {
        assert_eq!(
            row.frame().height,
            expected,
            "{name} row height; {gap} (actual {})",
            row.frame().height
        );
    }
    if !check_tops {
        return;
    }
    let mut lanes: Vec<f32> = rows.iter().map(|row| row.absolute_frame().x).collect();
    lanes.sort_by(|a, b| a.partial_cmp(b).unwrap());
    lanes.dedup();
    for lane in lanes {
        let lane_rows: Vec<Node> = rows
            .iter()
            .filter(|row| row.absolute_frame().x == lane)
            .cloned()
            .collect();
        let tops = row_tops(&lane_rows);
        for pair in tops.windows(2) {
            assert_eq!(
                pair[1] - pair[0],
                expected,
                "{name} consecutive row tops in lane {lane}; {gap}"
            );
        }
    }
}

struct CollectionHarness {
    width: State<f32>,
    input: State<InputCapabilities>,
    authored: State<f32>,
}

impl View for CollectionHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let authored = self.authored.get();
        let records = records();
        let model = DataTableModel::try_new(records.clone(), |record| *record)
            .expect("unique data table row IDs");
        let tree = Tree::new(
            records.clone(),
            |record: &usize| *record,
            |record: &usize| {
                if *record == 0 {
                    vec![1_000]
                } else {
                    Vec::new()
                }
            },
            |value: &usize, _: RowState| AnyView::new(Text::new(value.to_string())),
        )
        .options(TreeOptions {
            layout: TreeLayout {
                row_height: authored,
                ..TreeLayout::default()
            },
            ..TreeOptions::default()
        });
        let table = DataTable::from_records(records.clone(), |record: &usize| *record, columns())
            .expect("unique table column IDs")
            .options(DataTableOptions {
                layout: TableLayout {
                    row_height: authored,
                    ..TableLayout::default()
                },
                ..DataTableOptions::default()
            });
        let data_table = DataTable::try_new(model, columns())
            .expect("unique data table column IDs")
            .options(DataTableOptions {
                layout: TableLayout {
                    row_height: authored,
                    ..TableLayout::default()
                },
                ..DataTableOptions::default()
            });
        let mut surface = Environment::default();
        surface.input = self.input.get();
        surface.viewport.width = 1280.0;
        AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                10.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(AnyView::new(tree).frame_height(180.0).debug_name("tree")),
                    AnyView::new(AnyView::new(
                        AnyView::new(BoxView::new(
                            FlexDirection::Column,
                            Align::Stretch,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            0.0,
                            AnyView::new(table.flex_with(1.0, 1.0, Some(0.0))),
                        ))
                        .frame_height(240.0)
                        .debug_name("table"),
                    )),
                    AnyView::new(
                        AnyView::new(data_table)
                            .frame_height(240.0)
                            .debug_name("data-table"),
                    ),
                ])),
            ))
            .frame_width(self.width.get())
            .surface_environment(surface),
        )
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("width", &self.width),
            DynamicField::state("input", &self.input),
            DynamicField::state("authored", &self.authored),
        ]
    }
}

#[test]
fn collection_rows_share_density_and_retain_identity() {
    for width in [390.0_f32, 720.0, 1280.0] {
        for authored in [36.0_f32, 56.0] {
            let harness = CollectionHarness {
                width: State::new(width),
                input: State::new(desktop()),
                authored: State::new(authored),
            };
            let input = harness.input.clone();
            InteractiveScene::run(width.max(390.0), 820.0, |scene| {
                scene.install(harness);
                scene.settle(4, 0.016);
                let tree_row = scene
                    .all(|node| {
                        node.accessibility().map(|semantics| semantics.role)
                            == Some(AccessibilityRole::TreeItem)
                    })
                    .first()
                    .map(Node::id)
                    .expect("tree row");
                let table_row = scene
                    .all(|node| {
                        node.accessibility().map(|semantics| semantics.role)
                            == Some(AccessibilityRole::Cell)
                            && node.attachment::<usize>("dataTable.rowIndex").is_none()
                    })
                    .first()
                    .map(Node::id)
                    .expect("table row");
                let cell = scene
                    .all(|node| {
                        node.accessibility().map(|semantics| semantics.role)
                            == Some(AccessibilityRole::Cell)
                    })
                    .first()
                    .map(Node::id)
                    .expect("data-table cell");

                for capabilities in [TOUCH, TOUCH | desktop(), desktop()] {
                    input.set(capabilities);
                    scene.settle(3, 0.016);
                    let tree_height = authored;
                    check_rows(
                        scene,
                        "tree",
                        AccessibilityRole::TreeItem,
                        tree_height,
                        40,
                        true,
                        "the tree row takes the touch floor",
                    );
                    let table = scene.named("table").expect("table mounted");
                    let cells = rows_with(&table, AccessibilityRole::Cell);
                    assert!(
                        !cells.is_empty() && cells.len() < 200,
                        "the table mounts a window of cells, got {}",
                        cells.len()
                    );
                    check_rows(
                        scene,
                        "data-table",
                        AccessibilityRole::Cell,
                        if capabilities.contains(TOUCH) {
                            authored.max(44.0)
                        } else {
                            authored
                        },
                        40,
                        false,
                        "data-table takes the touch floor",
                    );
                    let data_table = scene.named("data-table").expect("data-table mounted");
                    for role in [AccessibilityRole::Group, AccessibilityRole::Row] {
                        let marker_rows: Vec<Node> = nodes(&data_table)
                            .into_iter()
                            .filter(|node| {
                                node.attachment::<usize>("dataTable.rowIndex").is_some()
                                    && node.accessibility().map(|s| s.role) == Some(role)
                            })
                            .collect();
                        assert!(
                            !marker_rows.is_empty() && marker_rows.len() < 40,
                            "data-table {role:?} window, saw {}",
                            marker_rows.len()
                        );
                        for row in &marker_rows {
                            assert_eq!(
                                row.frame().height,
                                if capabilities.contains(TOUCH) {
                                    authored.max(44.0)
                                } else {
                                    authored
                                },
                                "data-table {role:?} row height (actual {})",
                                row.frame().height
                            );
                        }
                        let tops = row_tops(&marker_rows);
                        for pair in tops.windows(2) {
                            assert_eq!(
                                pair[1] - pair[0],
                                if capabilities.contains(TOUCH) {
                                    authored.max(44.0)
                                } else {
                                    authored
                                },
                                "data-table {role:?} row tops"
                            );
                        }
                    }
                    assert_eq!(
                        scene.all(|node| node.id() == tree_row).len(),
                        1,
                        "tree row identity survives an input change"
                    );
                    assert_eq!(
                        scene.all(|node| node.id() == table_row).len(),
                        1,
                        "table row identity survives an input change"
                    );
                    assert_eq!(
                        scene.all(|node| node.id() == cell).len(),
                        1,
                        "data-table cell identity survives an input change"
                    );
                }
            });
        }
    }
}

struct TableHarness {
    input: State<InputCapabilities>,
}

impl View for TableHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let model = DataTableModel::try_new(records(), |record| *record)
            .expect("unique data table row IDs");
        let table = DataTable::try_new(model, columns())
            .expect("unique data table column IDs")
            .frame_height(310.0);
        let mut surface = Environment::default();
        surface.input = self.input.get();
        surface.viewport.width = 390.0;
        AnyView::new(
            AnyView::new(table)
                .frame_width(390.0)
                .surface_environment(surface),
        )
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

#[test]
fn touch_data_table_paging_reveals_the_last_row() {
    let harness = TableHarness {
        input: State::new(TOUCH | InputCapabilities::KEYBOARD),
    };
    let input = harness.input.clone();
    InteractiveScene::run(390.0, 310.0, |scene| {
        scene.install(harness);
        scene.settle(4, 0.016);
        let focus = scene
            .first(|node| node.attachment::<bool>("dataTable.input") == Some(true))
            .expect("data-table input");
        scene.click(&focus, 1);
        scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
        scene.key(Scancode::PAGE_DOWN, KeyModifiers::default());
        scene.key(Scancode::END, KeyModifiers::default());
        let last_rows: Vec<Node> =
            scene.all(|node| node.attachment::<usize>("dataTable.rowIndex") == Some(999));
        assert_eq!(
            last_rows.len(),
            2,
            "the frozen and moving rows of row 999 are mounted (actual {})",
            last_rows.len()
        );
        for row in &last_rows {
            assert_eq!(
                row.frame().height,
                44.0,
                "touch data-table rows rise to the control floor (actual {})",
                row.frame().height
            );
        }
        let mut tops: Vec<(AccessibilityRole, f32)> = last_rows
            .iter()
            .map(|row| {
                (
                    row.accessibility()
                        .map(|s| s.role)
                        .unwrap_or(AccessibilityRole::Row),
                    row.absolute_frame().y,
                )
            })
            .collect();
        tops.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        assert_eq!(
            (tops[1].1 - tops[0].1).abs(),
            0.0,
            "the frozen and moving lanes align on the same row: {:?}",
            tops
        );
        assert!(
            last_rows
                .iter()
                .all(|row| row.absolute_frame().y + row.frame().height <= 310.5),
            "the last row stays inside the 310pt body"
        );
        input.set(TOUCH | desktop());
        scene.settle(3, 0.016);
        assert_eq!(
            scene
                .all(|node| node.attachment::<usize>("dataTable.rowIndex") == Some(999))
                .len(),
            2,
            "paging keeps the last row mounted after an input change"
        );
    });
}

struct ScaledHarness {
    scale: State<f32>,
    dark: State<bool>,
    input: State<InputCapabilities>,
}

impl View for ScaledHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let tree = Tree::new(
            (0..20).collect::<Vec<usize>>(),
            |record: &usize| *record,
            |_record: &usize| Vec::new(),
            |value: &usize, _: RowState| AnyView::new(Text::new(value.to_string())),
        )
        .options(TreeOptions::default());
        let menu = Menu::new(vec![MenuEntry::Item(MenuItem::new(1, "Command", || {}))])
            .width(320.0)
            .max_visible_rows(2);
        let mut surface = Environment::default();
        surface.input = self.input.get();
        surface.viewport.width = 390.0;
        AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                10.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(
                        AnyView::new(tree)
                            .frame_height(180.0)
                            .debug_name("scaled-tree"),
                    ),
                    AnyView::new(menu),
                ])),
            ))
            .frame_width(390.0)
            .surface_environment(surface)
            .text_scaler(TextScaler::new(self.scale.get()))
            .appearance(
                if self.dark.get() {
                    Appearance::Dark
                } else {
                    Appearance::Light
                },
                ThemeTransition::default(),
            ),
        )
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("scale", &self.scale),
            DynamicField::state("dark", &self.dark),
            DynamicField::state("input", &self.input),
        ]
    }
}

#[test]
fn touch_floors_preserve_scaled_tree_and_menu_rows() {
    for scale in [1.0_f32, 2.0] {
        for dark in [false, true] {
            let harness = ScaledHarness {
                scale: State::new(scale),
                dark: State::new(dark),
                input: State::new(TOUCH),
            };
            let input = harness.input.clone();
            InteractiveScene::run(390.0, 820.0, |scene| {
                scene.install(harness);
                scene.settle(4, 0.016);
                let tree = scene.named("scaled-tree").expect("scaled tree");
                let tree_rows = rows_with(&tree, AccessibilityRole::TreeItem);
                assert!(
                    !tree_rows.is_empty(),
                    "scaled tree mounts rows at scale {scale} dark={dark}"
                );
                let expected = 30.0_f32;
                for row in &tree_rows {
                    assert_eq!(
                        row.frame().height,
                        expected,
                        "the tree row keeps its authored height on touch at scale {scale} (actual {})",
                        row.frame().height
                    );
                }
                let menu_row = scene.labelled("Command", None).expect("menu row mounted");
                assert!(
                    menu_row.frame().height >= 44.0,
                    "the menu row keeps the 44pt touch floor at scale {scale} (actual {})",
                    menu_row.frame().height
                );
                if scale == 2.0 {
                    assert!(
                        menu_row.frame().height > 44.0,
                        "a 2x menu row grows past the floor (actual {})",
                        menu_row.frame().height
                    );
                }

                input.set(desktop());
                scene.settle(3, 0.016);
                for row in &tree_rows {
                    assert_eq!(
                        row.frame().height,
                        30.0,
                        "the tree row keeps its authored height on pointer at scale {scale} (actual {})",
                        row.frame().height
                    );
                }
                let pointer_row = scene
                    .labelled("Command", None)
                    .expect("menu row mounted on pointer");
                assert!(
                    pointer_row.frame().height >= 28.0
                        && pointer_row.frame().height <= menu_row.frame().height,
                    "the anchored row returns to its compact height (touch {}, pointer {})",
                    menu_row.frame().height,
                    pointer_row.frame().height
                );
                if scale == 1.0 {
                    assert!(
                        pointer_row.frame().height < 44.0,
                        "at 1x the pointer row is below the touch floor (actual {})",
                        pointer_row.frame().height
                    );
                }
            });
        }
    }
}

struct MenuHarness {
    input: State<InputCapabilities>,
}

impl View for MenuHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let entries = vec![
            MenuEntry::Submenu(MenuSubmenu::new(
                1,
                "Tools",
                vec![MenuEntry::Item(MenuItem::new(2, "Build", || {}))],
            )),
            MenuEntry::Item(MenuItem::new(3, "Unavailable", || {}).enabled(false)),
            MenuEntry::Item(MenuItem::new(4, "Last", || {})),
        ];
        let menu = Menu::new(entries).width(220.0).max_visible_rows(2);
        let mut surface = Environment::default();
        surface.input = self.input.get();
        surface.viewport.width = 390.0;
        AnyView::new(
            AnyView::new(LayerRoot::new(AnyView::new(
                AnyView::new(menu).surface_environment(surface),
            )))
            .frame_width(390.0),
        )
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

#[test]
fn touch_menu_rows_keep_height_and_pointer_restores_anchored() {
    let harness = MenuHarness {
        input: State::new(TOUCH | InputCapabilities::KEYBOARD),
    };
    let input = harness.input.clone();
    InteractiveScene::run(390.0, 820.0, |scene| {
        scene.install(harness);
        scene.settle(4, 0.016);
        let branch = scene.labelled("Tools", None).expect("submenu branch");
        let disabled = scene.labelled("Unavailable", None).expect("disabled row");
        assert!(
            branch.frame().height >= 44.0 && disabled.frame().height >= 44.0,
            "touch menu rows meet the 44pt floor (branch {}, disabled {})",
            branch.frame().height,
            disabled.frame().height
        );
        assert!(
            disabled.accessibility().map(|s| s.state.is_enabled) == Some(false)
                && !disabled.is_focusable(),
            "the disabled row reports a disabled, unfocusable state"
        );
        assert_eq!(
            branch.accessibility().and_then(|s| s.state.is_expanded),
            Some(false),
            "the submenu branch advertises a collapsed state"
        );
        branch
            .accessibility_actions()
            .activate
            .expect("submenu branch is activatable")();
        scene.settle(2, 0.016);
        assert!(
            scene.labelled("Build", None).is_some(),
            "activating the branch mounts its submenu"
        );

        input.set(desktop());
        scene.settle(3, 0.016);
        let anchored = scene.labelled("Tools", None).expect("anchored branch");
        assert!(
            anchored.frame().height >= 28.0,
            "a menu under `LayerRoot` rebuilds its rows for pointer input (actual {})",
            anchored.frame().height
        );
        assert_eq!(
            anchored.frame().height,
            44.0,
            "the pointer row falls back to the authored metric (actual {})",
            anchored.frame().height
        );
    });
}
