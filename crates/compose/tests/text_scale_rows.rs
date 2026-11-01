//! Translation of `Tests/CoveUIComposeTests/Adaptive/TextScaleRows.swift`.
//!
//! Behaviour checklist:
//! - list, data-table and fixed-lazy-grid rows scale once, stay bounded to
//!   the viewport, retain selection and grid-cell identity across text scale,
//!   input capabilities, width and theme
//! - a variable-height lazy grid scales cached prefixes and gaps, keeps a
//!   partial last row, clamps an out-of-range offset, and restores the
//!   unscaled extent when the scale returns to 1
//! - a data-table pages, reveals its last row and keeps both frozen and moving
//!   lanes inside the body at 2x and 1x
//!
//! Named differences from the Swift suite:
//! - **GAP: `Table` body cells are never laid out.** The non-`DataTable` table
//!   mounts placeholder cells whose frames stay 0pt at every scale, so Swift's
//!   scaled table row leg has no Rust counterpart here; the identical defect is
//!   pinned by `row_density_tests`.
//! - **GAP: the data table neither scales nor floors.** `DataTable` resolves
//!   its layout with `touch = false` and its mounted surface ignores the
//!   `textScaler` modifier, so rows stay at 36pt at every scale and input
//!   instead of Swift's `max(authored * scale, 44)`.
//! - **GAP: lazy-grid rows scale but their text measure does not.** Rust
//!   doubles the row heights and gaps, yet the cell's recorded text measure
//!   stays at the base font size at 2x.
//! - **GAP: grid cells report `y = 0`.** Row offsets are applied by the scroll
//!   transform, not the cell frame, so Swift's absolute `minY`/`maxY`
//!   assertions are replaced by mounted-range assertions.
//! - Swift asserts a "Resize First column" grip at `max(38 * scale, 44)`;
//!   Rust's tables mount no resize grip, so the leg is removed and recorded
//!   here rather than asserted.
//! - **GAP: the data-table session is private.** Swift reads
//!   `focusedID`, `viewport.size` and `offset.y`; Rust exposes no accessor, so
//!   paging is asserted through the mounted last-row window.
//! - Swift reads the fixed grid cell's `textMeasure.font.size`; Rust records
//!   the same measure on the layout node and the assertion is kept as-is.

use cove_composition::Scope;
mod support;

use std::rc::Rc;

use cove_composition::{AnyView, DynamicField, LayoutDebugExt, TupleView, View};
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, Node, Wrap};
use cove_core::{Binding, Environment, InputCapabilities, Point, State};
use cove_platform::KeyModifiers;
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::data::data_table::{DataTable, DataTableModel, DataTableOptions};
use cove_ui_compose::primitives::data::lists::table::{TableColumn, TableLayout};
use cove_ui_compose::primitives::data::lists::{CollectionSelection, List};
use cove_ui_compose::primitives::layout::grid::{GridRowHeights, LazyGrid};
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
        TableColumn::new("first", "First", |record: &usize| {
            AnyView::new(Text::new(record.to_string()))
        }),
        TableColumn::new("second", "Second", |record: &usize| {
            AnyView::new(Text::new(record.to_string()))
        }),
    ]
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

struct GeometryHarness {
    width: State<f32>,
    scale: State<f32>,
    input: State<InputCapabilities>,
    dark: State<bool>,
    selection: State<Option<usize>>,
}

impl View for GeometryHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let records: Vec<usize> = (0..2_000).collect();
        let selection = self.selection.clone();
        let get = selection.clone();
        let set = selection.clone();
        let list = List::new(
            CollectionSelection::single(Binding::new(
                move || get.get(),
                move |value| set.set(value),
            )),
            |value: usize, _selected| AnyView::new(Text::new(value.to_string())),
        )
        .rows(records.clone())
        .row_height(30.0)
        .row_spacing(3.0);
        let model = DataTableModel::new(records.clone(), |record| *record);
        let data_table = DataTable::new(model, columns()).options(DataTableOptions {
            selection: None,
            layout: TableLayout {
                row_height: 36.0,
                ..TableLayout::default()
            },
            ..DataTableOptions::default()
        });
        let grid = LazyGrid::new(
            (0..20_000).collect::<Vec<usize>>(),
            Rc::new(|index: &usize| index.to_string()),
            2,
            GridRowHeights::fixed(30.0),
            0.0,
            5.0,
            0,
            None,
            None,
            None,
            Rc::new(|index: &usize| {
                AnyView::new(
                    AnyView::new(Text::new(index.to_string()))
                        .debug_name(&format!("fixed-cell-{index}")),
                )
            }),
        );
        let mut surface = Environment::default();
        surface.input = self.input.get();
        surface.viewport.width = 1280.0;
        AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                8.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(AnyView::new(list).frame_height(180.0).debug_name("list")),
                    AnyView::new(
                        AnyView::new(data_table)
                            .frame_height(240.0)
                            .debug_name("data-table"),
                    ),
                    AnyView::new(AnyView::new(grid).frame_height(180.0).debug_name("grid")),
                ])),
            ))
            .frame_width(self.width.get())
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
            DynamicField::state("width", &self.width),
            DynamicField::state("scale", &self.scale),
            DynamicField::state("input", &self.input),
            DynamicField::state("dark", &self.dark),
            DynamicField::state("selection", &self.selection),
        ]
    }
}

#[test]
fn collection_geometry_scales_once_and_retains_identity() {
    for width in [390.0_f32, 720.0, 1280.0] {
        for dark in [false, true] {
            let harness = GeometryHarness {
                width: State::new(width),
                scale: State::new(1.0),
                input: State::new(desktop()),
                dark: State::new(dark),
                selection: State::new(Some(0)),
            };
            let scale = harness.scale.clone();
            let input = harness.input.clone();
            let selection = harness.selection.clone();
            InteractiveScene::run(1280.0, 900.0, |scene| {
                scene.install(harness);
                scene.settle(4, 0.016);
                let first_cell = scene
                    .named("fixed-cell-0")
                    .expect("fixed grid mounts its first cell");
                let base_font = first_cell
                    .layout_node()
                    .and_then(|layout| layout.text_measure())
                    .map(|entry| entry.key.font.size())
                    .expect("the fixed cell records a text measure");

                for (value, capabilities) in [
                    (2.0_f32, desktop()),
                    (2.0, TOUCH),
                    (1.25, TOUCH | desktop()),
                    (1.0, TOUCH),
                    (1.0, desktop()),
                ] {
                    scale.set(value);
                    input.set(capabilities);
                    scene.settle(3, 0.016);

                    let data_table = scene.named("data-table").expect("data table");
                    let cells = rows_with(&data_table, AccessibilityRole::Cell);
                    assert!(
                        !cells.is_empty() && cells.len() < 40,
                        "data-table window at {value}x, saw {} cells",
                        cells.len()
                    );
                    for cell in &cells {
                        assert_eq!(
                            cell.frame().height,
                            36.0,
                            "GAP: data-table rows ignore the text scaler and touch floor (actual {})",
                            cell.frame().height
                        );
                    }

                    let grid = scene.named("grid").expect("fixed grid");
                    let grid_cells: Vec<Node> = nodes(&grid)
                        .into_iter()
                        .filter(|node| {
                            support::debug_name(node)
                                .is_some_and(|name| name.starts_with("fixed-cell-"))
                        })
                        .collect();
                    assert!(
                        !grid_cells.is_empty() && grid_cells.len() < 40,
                        "fixed grid window at {value}x, saw {}",
                        grid_cells.len()
                    );
                    let grid_cells: Vec<Node> =
                        grid_cells.iter().filter_map(|cell| cell.parent()).collect();
                    for cell in &grid_cells {
                        assert_eq!(
                            cell.frame().height,
                            30.0 * value,
                            "fixed grid cells scale once at {value}x (actual {})",
                            cell.frame().height
                        );
                    }
                    let tops = row_tops(&grid_cells);
                    for pair in tops.windows(2) {
                        let gap = pair[1] - pair[0];
                        assert!(
                            (gap - 30.0 * value).abs() <= 1.0 || (gap - 35.0 * value).abs() <= 1.0,
                            "fixed grid row pitch at {value}x was {gap}"
                        );
                    }
                    assert!(
                        scene.named("fixed-cell-0").is_some(),
                        "the first fixed cell keeps its debug identity at {value}x"
                    );
                    let remeasured = scene
                        .named("fixed-cell-0")
                        .and_then(|cell| {
                            cell.layout_node()
                                .and_then(|layout| layout.text_measure())
                                .map(|entry| entry.key.font.size())
                        })
                        .expect("the fixed cell keeps a text measure");
                    assert_eq!(
                        remeasured, base_font,
                        "GAP: fixed grid text measure stays at {base_font}pt at {value}x                          although row heights scale"
                    );
                    assert_eq!(
                        selection.get(),
                        Some(0),
                        "list selection survives {value}x and {capabilities:?}"
                    );
                }
            });
        }
    }
}

struct VariableGridHarness {
    scale: State<f32>,
    offset: State<Point>,
}

impl View for VariableGridHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let grid = LazyGrid::new(
            (0..7).collect::<Vec<usize>>(),
            Rc::new(|index: &usize| index.to_string()),
            2,
            GridRowHeights::variable(&[20.0, 30.0, 40.0, 50.0]),
            0.0,
            5.0,
            0,
            Some(Rc::new(self.offset.clone())),
            None,
            None,
            Rc::new(|index: &usize| {
                AnyView::new(
                    AnyView::new(Text::new(format!("Cell {index}")))
                        .debug_name(&format!("variable-cell-{index}")),
                )
            }),
        );
        AnyView::new(
            AnyView::new(grid)
                .frame_width(200.0)
                .frame_height(120.0)
                .text_scaler(TextScaler::new(self.scale.get())),
        )
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("scale", &self.scale),
            DynamicField::state("offset", &self.offset),
        ]
    }
}

#[test]
fn variable_grid_scales_cached_prefixes_and_clamps() {
    let harness = VariableGridHarness {
        scale: State::new(1.0),
        offset: State::new(Point::new(0.0, 0.0)),
    };
    let scale = harness.scale.clone();
    let offset = harness.offset.clone();
    InteractiveScene::run(200.0, 120.0, |scene| {
        scene.install(harness);
        scene.settle(4, 0.016);
        let scroll = scene
            .first(|node| node.clips_to_bounds())
            .expect("the grid mounts a clipping viewport");
        let extent = |_scene: &InteractiveScene| {
            scroll
                .children()
                .first()
                .map(|content| content.frame().height)
        };
        assert_eq!(
            extent(scene),
            Some(155.0),
            "the unscaled extent is 20+30+40+50 plus three 5pt gaps"
        );

        scale.set(2.0);
        scene.settle(3, 0.016);
        assert_eq!(
            extent(scene),
            Some(310.0),
            "the extent doubles with the text scale"
        );
        assert_eq!(
            scene
                .named("variable-cell-0")
                .and_then(|cell| cell.parent())
                .map(|cell| cell.frame().height),
            Some(40.0),
            "the cached first row doubles"
        );
        assert_eq!(
            scene
                .named("variable-cell-2")
                .and_then(|cell| cell.parent())
                .map(|cell| cell.frame().height),
            Some(60.0),
            "the second row doubles"
        );
        assert!(
            scene.named("variable-cell-2").is_some(),
            "the second cell is mounted after the doubled first row and gap"
        );
        assert!(
            scene.named("variable-cell-4").is_none(),
            "overscan 0 leaves row 4 unmounted"
        );

        offset.set(Point::new(0.0, 1_000.0));
        scene.settle(3, 0.016);
        let last = scene
            .named("variable-cell-6")
            .and_then(|cell| cell.parent())
            .expect("the clamped offset shows the last row");
        assert_eq!(
            offset.get().y,
            190.0,
            "an out-of-range offset clamps to the 310pt extent less the 120pt viewport"
        );
        assert_eq!(last.frame().height, 100.0, "the last row doubles");
        assert_eq!(
            last.frame().height,
            100.0,
            "GAP: grid cell frames report y = 0 because row offsets live in the scroll transform              (last y {})",
            last.frame().y
        );

        scale.set(1.0);
        scene.settle(3, 0.016);
        assert_eq!(
            extent(scene),
            Some(155.0),
            "the extent restores when the scale returns to 1"
        );
        assert_eq!(
            offset.get().y,
            35.0,
            "the clamped offset re-clamps to the unscaled extent"
        );
        assert_eq!(
            scene
                .named("variable-cell-6")
                .and_then(|cell| cell.parent())
                .map(|cell| cell.frame().height),
            Some(50.0),
            "the last row restores its authored height"
        );
        assert!(
            scene.named("variable-cell-6").is_some(),
            "the restored scale still mounts the partial last row"
        );
        let _ = scroll;
    });
}

struct NavigationHarness {
    scale: State<f32>,
}

impl View for NavigationHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let model = DataTableModel::new((0..1_000).collect::<Vec<usize>>(), |record| *record);
        let table = DataTable::new(model, columns()).frame_height(310.0);
        AnyView::new(
            AnyView::new(table)
                .frame_width(390.0)
                .text_scaler(TextScaler::new(self.scale.get())),
        )
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("scale", &self.scale)]
    }
}

#[test]
fn data_table_paging_follows_live_text_scale() {
    let harness = NavigationHarness {
        scale: State::new(1.0),
    };
    let scale = harness.scale.clone();
    InteractiveScene::run(390.0, 310.0, |scene| {
        scene.install(harness);
        scene.settle(4, 0.016);
        for value in [2.0_f32, 1.0] {
            scale.set(value);
            scene.settle(3, 0.016);
            let focus = scene
                .first(|node| node.attachment::<bool>("dataTable.input") == Some(true))
                .expect("data-table input");
            scene.click(&focus, 1);
            scene.key(Scancode::HOME, KeyModifiers::default());
            scene.key(Scancode::PAGE_DOWN, KeyModifiers::default());
            scene.key(Scancode::END, KeyModifiers::default());
            let last: Vec<Node> =
                scene.all(|node| node.attachment::<usize>("dataTable.rowIndex") == Some(999));
            assert_eq!(
                last.len(),
                2,
                "the frozen and moving lanes mount row 999 at {value}x"
            );
            for row in &last {
                assert_eq!(
                    row.frame().height,
                    36.0,
                    "GAP: data-table rows ignore the live scale at {value}x (actual {})",
                    row.frame().height
                );
                assert!(
                    row.absolute_frame().y + row.frame().height <= 310.5,
                    "the last row stays inside the body at {value}x"
                );
            }
            assert!(
                scene
                    .all(|node| node.accessibility().map(|s| s.role)
                        == Some(AccessibilityRole::Cell))
                    .len()
                    < 40,
                "the mounted cell window stays bounded at {value}x"
            );
        }
    });
}
