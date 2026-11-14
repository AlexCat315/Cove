//! Options and hosts that exist on a component must reach the screen.
//!
//! Behaviour checklist:
//! - an avatar without an image shows its initials, or an icon when anonymous
//! - a menu tab strip lists tabs that do not fit
//! - the highlighted palette row stays mounted while the keyboard walks the list
//! - a swipe list moves its selection with the keyboard and mounts a window
//! - the minimap reserves its column and scrolls on a press
//! - a bounded scroll view measures its content when no height is authored
//! - a data table reorders columns by header drag and paints the hovered row

use crate::support;

use cove_composition::Scope;
use cove_composition::{AnyView, TupleView, View};
use cove_core::Point;
use cove_core::State;
use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};
use cove_platform::{InputEvent, KeyModifiers, MouseMotionEvent};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::data::avatar::avatar::Avatar;
use cove_ui_compose::primitives::data::lists::{RowState, Selection, SwipeList};
use cove_ui_compose::primitives::data::table::table_column::{TableColumn, TableColumnLayout};
use cove_ui_compose::primitives::data::table::{Table, TableOptions};
use cove_ui_compose::primitives::layout::{BoundedScrollView, TabItem, TabOverflow, TabView};
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::overlays::{CommandPalette, PaletteCommand};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_field::text_field::{FieldState, TextField};
use cove_ui_compose::primitives::text::text_field::text_field_options::{
    TextFieldAxis, TextFieldCodeEditing, TextFieldLayout,
};
use cove_ui_compose::{BoxView, Scancode};
use std::cell::RefCell;
use std::rc::Rc;

use support::InteractiveScene;

fn text_nodes(scene: &InteractiveScene, needle: &str) -> usize {
    scene
        .all(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.label == needle)
        })
        .len()
}

#[test]
fn an_avatar_without_an_image_shows_its_initials() {
    InteractiveScene::run(200.0, 200.0, |scene| {
        scene.install(Avatar::new("Ada Lovelace"));
        assert!(
            text_nodes(scene, "AL") > 0,
            "the fallback disc draws the initials"
        );
    });
}

#[test]
fn an_anonymous_avatar_shows_its_placeholder_icon() {
    InteractiveScene::run(200.0, 200.0, |scene| {
        scene.install(Avatar::new("  "));
        let disc = scene
            .first(|node| {
                node.view_tag()
                    .is_some_and(|tag| tag.contains("AvatarFallback"))
            })
            .expect("the fallback disc mounts");
        assert!(
            !disc.children().is_empty(),
            "the anonymous disc holds the placeholder icon"
        );
    });
}

#[test]
fn a_menu_tab_strip_lists_what_does_not_fit() {
    InteractiveScene::run(220.0, 200.0, |scene| {
        let selection = State::new(0_i32);
        let tabs = (0..12)
            .map(|index| {
                TabItem::new(
                    index,
                    format!("Long tab title {index}"),
                    AnyView::new(Text::new(format!("Panel {index}"))),
                )
            })
            .collect();
        scene.install(LayerRoot::new(
            TabView::try_new(selection.binding(), tabs)
                .expect("unique tab IDs")
                .overflow(TabOverflow::Menu),
        ));
        scene.settle(6, 0.1);
        let frames: Vec<_> = scene
            .all(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.label.starts_with("Long tab title"))
            })
            .into_iter()
            .filter(|node| node.frame().width > 0.0 && node.is_layout_visible())
            .collect();
        assert!(
            !frames.is_empty() && frames.len() < 12,
            "only the tabs that fit stay visible, got {}",
            frames.len()
        );
    });
}

#[test]
fn the_highlighted_palette_row_stays_mounted() {
    InteractiveScene::run(900.0, 700.0, |scene| {
        let open = State::new(true);
        let items = (0..200)
            .map(|index| PaletteCommand::new(format!("Command {index}"), || {}))
            .collect();
        scene.install(LayerRoot::new(CommandPalette::new(open.binding(), items)));
        scene.settle(10, 0.1);
        assert_eq!(
            text_nodes(scene, "Command 199"),
            0,
            "the last row starts unmounted"
        );
        for _ in 0..199 {
            scene
                .dispatcher()
                .dispatch(&InputEvent::KeyDown(cove_platform::KeyEvent {
                    scancode: Scancode::ARROW_DOWN,
                    keycode: 0,
                    modifiers: KeyModifiers::default(),
                    is_repeat: false,
                }));
            scene.graph.recomposer.commit_all();
        }
        scene.settle(10, 0.1);
        assert!(
            text_nodes(scene, "Command 199") > 0,
            "the keyboard scrolled the highlighted row into the window"
        );
    });
}

#[test]
fn a_swipe_list_moves_with_the_keyboard_and_mounts_a_window() {
    InteractiveScene::run(300.0, 240.0, |scene| {
        let selected = State::new(None::<i32>);
        let list = SwipeList::new(
            (0..500).collect::<Vec<i32>>(),
            |item: &i32| *item,
            |item: &i32, _: RowState| AnyView::new(Text::new(format!("Row {item}"))),
        )
        .row_label(|item: &i32| format!("Row {item}"))
        .selection(Selection::single(selected.binding()));
        scene.install(list.frame_size(300.0, 240.0));
        scene.settle(6, 0.1);
        assert!(
            text_nodes(scene, "Row 499") == 0 && text_nodes(scene, "Row 0") > 0,
            "only the window mounts"
        );
        let host = scene
            .first(|node| node.is_focusable())
            .expect("the list takes focus");
        scene.focus.focus(Some(&host), false);
        scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
        assert_eq!(selected.get(), Some(0));
        scene.key(Scancode::END, KeyModifiers::default());
        assert_eq!(selected.get(), Some(499));
    });
}

#[test]
fn the_minimap_reserves_its_column_and_scrolls_on_a_press() {
    InteractiveScene::run(500.0, 200.0, |scene| {
        let lines = (0..400)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let text = State::new(TextBuffer::new(&lines));
        let mut code = TextFieldCodeEditing::default();
        code.shows_minimap = true;
        scene.install(
            TextField::new("", text.binding())
                .layout(TextFieldLayout {
                    axis: TextFieldAxis::Vertical,
                    ..TextFieldLayout::default()
                })
                .code_editing(code)
                .frame_size(500.0, 200.0),
        );
        scene.settle(3, 0.1);
        let field = scene
            .first(|node| {
                node.attachment::<Rc<RefCell<FieldState>>>("__textfield_state")
                    .is_some()
            })
            .expect("the field mounts");
        // The draw hook records where the overview went.
        let mut list = cove_core::DrawList::default();
        let draw = field.draw().expect("the field draws");
        support::with_test_text_environment(|| draw(&mut list, Point::default()));
        let state = field
            .attachment::<Rc<RefCell<FieldState>>>("__textfield_state")
            .expect("field state");
        assert!(state.borrow().scroll.max_y > 0.0, "the document overflows");
        let frame = field.absolute_frame();
        scene.click_at_point(
            Point::new(frame.x + frame.width - 20.0, frame.y + frame.height - 4.0),
            1,
        );
        assert!(
            state.borrow().scroll.offset_y > 0.0,
            "a press low on the overview scrolls the document"
        );
    });
}

#[test]
fn a_bounded_scroll_view_measures_its_content() {
    InteractiveScene::run(300.0, 600.0, |scene| {
        let rows = (0..3)
            .map(|index| {
                AnyView::new(AnyView::new(Text::new(format!("Row {index}"))).frame_height(40.0))
            })
            .collect();
        scene.install(
            BoundedScrollView::new(AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(rows)),
            )))
            .min_height(20.0)
            .max_height(300.0)
            .frame_width(300.0),
        );
        scene.settle(6, 0.1);
        let viewport = scene
            .first(|node| node.is_scroll_viewport())
            .expect("the scroll viewport mounts");
        assert_eq!(
            viewport.frame().height,
            120.0,
            "three 40pt rows size the viewport"
        );
    });
}

#[derive(Clone)]
struct Fixture {
    order: State<Vec<String>>,
}

impl View for Fixture {
    fn body(&self, _cx: &Scope) -> AnyView {
        let columns = ["a", "b", "c"]
            .into_iter()
            .map(|id| {
                TableColumn::new(id, id.to_uppercase(), move |record: &i32| {
                    AnyView::new(Text::new(format!("{id}{record}")))
                })
                .layout(TableColumnLayout {
                    width: 100.0,
                    is_resizable: false,
                    ..TableColumnLayout::default()
                })
            })
            .collect();
        AnyView::new(
            Table::new((0..5).collect(), |record: &i32| *record, columns)
                .expect("unique IDs")
                .options(TableOptions {
                    column_order: Some(self.order.binding()),
                    frozen_column_count: 0,
                    ..TableOptions::default()
                })
                .frame_size(400.0, 300.0),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state("order", &self.order)]
    }
}

fn header(scene: &InteractiveScene, title: &str) -> Node {
    scene
        .first(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.label == format!("Sort by {title}"))
        })
        .expect("the header mounts")
}

#[test]
fn a_header_drag_reorders_columns() {
    InteractiveScene::run(500.0, 400.0, |scene| {
        let order = State::new(Vec::<String>::new());
        scene.install(Fixture {
            order: order.clone(),
        });
        scene.settle(4, 0.1);
        let frame = header(scene, "A").absolute_frame();
        let y = frame.y + frame.height / 2.0;
        scene.drag(
            Point::new(frame.x + 20.0, y),
            Point::new(frame.x + 150.0, y),
            4,
        );
        assert_eq!(
            order.get(),
            vec!["b".to_string(), "a".to_string(), "c".to_string()],
            "dragging A past B's midpoint writes the new order"
        );
        let small = header(scene, "B").absolute_frame();
        scene.click_at_point(Point::new(small.x + 20.0, y), 1);
        assert_eq!(
            order.get().len(),
            3,
            "a click on a header without a comparator leaves the order alone"
        );
    });
}

#[test]
fn the_hovered_row_paints_across_lanes() {
    InteractiveScene::run(500.0, 400.0, |scene| {
        scene.install(Fixture {
            order: State::new(Vec::new()),
        });
        scene.settle(4, 0.1);
        let row = scene
            .first(|node| node.attachment::<usize>("dataTable.rowIndex") == Some(1))
            .expect("row 1 mounts");
        let resting = row.background_color();
        let frame = row.absolute_frame();
        // One dispatcher: the hover path lives on it.
        let dispatcher = scene.dispatcher();
        dispatcher.dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
            x: frame.x + 40.0,
            y: frame.y + frame.height / 2.0,
            delta_x: 0.0,
            delta_y: 0.0,
            kind: Default::default(),
        }));
        assert_ne!(row.background_color(), resting, "the row paints its hover");
        dispatcher.dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
            x: frame.x + 40.0,
            y: 390.0,
            delta_x: 0.0,
            delta_y: 0.0,
            kind: Default::default(),
        }));
        assert_eq!(row.background_color(), resting, "leaving restores the fill");
    });
}
