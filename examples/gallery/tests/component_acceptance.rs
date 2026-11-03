mod support;

use cove_composition::AnyView;
use cove_core::scene::{AccessibilityRole, TextInputAttachmentKey};
use cove_core::{DrawList, Point};
use cove_examples::gallery_content::{GalleryView, catalog::GalleryPage};
use cove_platform::{InputEvent, TextEditingEvent, TextInputArea};
use cove_ui_compose::{Scancode, theme::styles::appearance_modifier::Appearance};
use support::GalleryScene;

#[test]
fn range_endpoints_have_visible_bounds_and_follow_pointer_focus_in_both_directions() {
    use cove_composition::layout::LayoutDirectionExt;
    use cove_core::scene::Direction;
    use cove_ui_compose::prelude::*;
    for direction in [Direction::Ltr, Direction::Rtl] {
        let values = State::new(RangeValue::new(20.0, 80.0));
        GalleryScene::run(
            320.0,
            100.0,
            |_| {
                AnyView::new(
                    RangeSlider::new(values.binding())
                        .range((0.0, 100.0))
                        .step(1.0)
                        .frame_width(240.0)
                        .layout_direction(direction),
                )
            },
            |scene| {
                let lower = scene
                    .first(|node| {
                        node.accessibility().is_some_and(|s| {
                            s.role == AccessibilityRole::Slider && s.label == "Range start"
                        })
                    })
                    .unwrap();
                let upper = scene
                    .first(|node| {
                        node.accessibility().is_some_and(|s| {
                            s.role == AccessibilityRole::Slider && s.label == "Range end"
                        })
                    })
                    .unwrap();
                assert!(lower.frame().width > 0.0 && lower.frame().height > 0.0);
                assert!(upper.frame().width > 0.0 && upper.frame().height > 0.0);
                assert_eq!(
                    lower.absolute_frame().x < upper.absolute_frame().x,
                    direction == Direction::Ltr
                );
                scene.click(&lower);
                assert_eq!(
                    values.get().lower,
                    20.0,
                    "the accessible thumb centre matches its value"
                );
                assert_eq!(scene.focus.focused(), Some(lower));
                scene.key(Scancode::ARROW_RIGHT);
                scene.settle_n(1);
                assert_eq!(
                    values.get().lower,
                    if direction == Direction::Rtl {
                        19.0
                    } else {
                        21.0
                    }
                );
                assert_eq!(values.get().upper, 80.0);
            },
        );
    }
}

#[test]
fn editable_text_supports_pointer_caret_and_drag_selection() {
    use cove_ui_compose::prelude::*;
    use cove_ui_compose::primitives::text::text_field::text_field_layout_engine::FIELD_STATE_KEY;
    use std::{cell::RefCell, rc::Rc};
    let text = State::new(TextBuffer::new("abcdef"));
    GalleryScene::run(
        320.0,
        100.0,
        |_| AnyView::new(TextField::new("", text.binding()).frame_size(240.0, 36.0)),
        |scene| {
            let field = scene
                .first(|node| {
                    node.accessibility()
                        .is_some_and(|s| s.role == AccessibilityRole::TextField)
                })
                .unwrap();
            let rect = field.absolute_frame();
            let from = Point::new(rect.x + 9.0, rect.y + 18.0);
            scene.click_at(from);
            let state: Rc<RefCell<FieldState>> = field.attachment(FIELD_STATE_KEY).unwrap();
            assert_eq!(
                state.borrow().selection.cursor_index,
                0,
                "click at the beginning moves the caret"
            );
            let button = |x| cove_platform::MouseButtonEvent {
                button: cove_platform::MouseButton::Left,
                x,
                y: from.y,
                clicks: 1,
                ..Default::default()
            };
            let input = scene.dispatcher();
            input.dispatch(&InputEvent::MouseButtonDown(button(from.x)));
            input.dispatch(&InputEvent::MouseMotion(cove_platform::MouseMotionEvent {
                x: from.x + 32.0,
                y: from.y,
                delta_x: 32.0,
                ..Default::default()
            }));
            input.dispatch(&InputEvent::MouseButtonUp(button(from.x + 32.0)));
            let range = state
                .borrow()
                .selection_range()
                .expect("dragging selects text");
            assert_eq!(range.0, 0);
            assert!(range.1 > range.0 && range.1 <= 6);
            text.set(TextBuffer::new("hello world"));
            scene.settle_n(1);
            let click = |clicks| cove_platform::MouseButtonEvent {
                clicks,
                ..button(from.x + 4.0)
            };
            input.dispatch(&InputEvent::MouseButtonDown(click(2)));
            input.dispatch(&InputEvent::MouseButtonUp(click(2)));
            assert_eq!(
                state.borrow().selection_range(),
                Some((0, 5)),
                "double click selects a word using real text metrics"
            );
            input.dispatch(&InputEvent::MouseButtonDown(click(3)));
            input.dispatch(&InputEvent::MouseButtonUp(click(3)));
            assert_eq!(
                state.borrow().selection_range(),
                Some((0, 11)),
                "triple click selects the field"
            );
        },
    );
}

#[test]
fn search_click_enables_native_input_then_filters_per_character_and_clears() {
    let gallery = GalleryView::new(GalleryPage::Introduction, Appearance::Light, 1.0);
    let query = gallery.search.clone();
    GalleryScene::run(
        1280.0,
        820.0,
        |_| AnyView::new(gallery),
        |scene| {
            let field = scene
                .first(|node| {
                    node.accessibility().is_some_and(|s| {
                        s.role == AccessibilityRole::TextField && s.label == "Search components…"
                    })
                })
                .unwrap();
            assert!(
                field
                    .attachment::<TextInputArea>(TextInputAttachmentKey::AREA)
                    .is_none()
            );
            scene.click(&field);
            scene.settle_n(1);
            assert_eq!(scene.focus.focused(), Some(field.clone()));
            let area = field
                .attachment::<TextInputArea>(TextInputAttachmentKey::AREA)
                .expect(
                    "native host can enable text input after a click, before any painted frame",
                );
            assert!(area.width > 100.0 && area.height > 0.0);
            for character in ["R", "a", "t", "i", "n", "g"] {
                scene.text_input(character);
                scene.settle_n(1);
            }
            assert_eq!(query.get().string_value(), "Rating");
            let navigation = |label: &str| {
                scene.all(|node| {
                    node.accessibility()
                        .is_some_and(|s| s.role == AccessibilityRole::Button && s.label == label)
                })
            };
            assert_eq!(navigation("Rating").len(), 1);
            assert!(navigation("Slider").is_empty());
            scene
                .dispatcher()
                .dispatch(&InputEvent::TextEditing(TextEditingEvent {
                    text: "ni".into(),
                    start: 2,
                    length: 0,
                }));
            assert_eq!(
                query.get().string_value(),
                "Rating",
                "IME preview does not commit a search"
            );
            scene
                .dispatcher()
                .dispatch(&InputEvent::TextEditing(TextEditingEvent {
                    text: String::new(),
                    start: 0,
                    length: 0,
                }));
            let mut drawing = DrawList::default();
            field.draw().unwrap()(&mut drawing, field.absolute_origin());
            let hit = field
                .attachment::<f32>("__textfield_clear_hit")
                .expect("nonempty search exposes a clear affordance");
            scene.click_at(Point::new(
                hit + 7.0,
                field.absolute_frame().y + field.frame().height * 0.5,
            ));
            scene.settle_n(2);
            assert!(
                query.get().is_empty(),
                "clear writes through the query binding"
            );
            assert_eq!(navigation("Slider").len(), 1);
            scene.text_input("Rating");
            scene.settle_n(1);
            scene.key(Scancode::ESCAPE);
            scene.settle_n(1);
            assert!(
                query.get().is_empty(),
                "Escape restores the full navigation"
            );
        },
    );
}

#[test]
fn rating_density_and_custom_samples_keep_independent_live_values() {
    GalleryScene::run(
        1280.0,
        1800.0,
        |_| {
            AnyView::new(GalleryView::new(
                GalleryPage::Rating,
                Appearance::Light,
                1.0,
            ))
        },
        |scene| {
            for label in [
                "Mini rating",
                "Small rating",
                "Regular rating",
                "Large rating",
                "Seven stars, minimum half",
            ] {
                let node = scene
                    .first(|node| {
                        node.accessibility().is_some_and(|s| {
                            s.role == AccessibilityRole::Slider && s.label == label
                        })
                    })
                    .unwrap();
                let before = node.accessibility().unwrap().value;
                node.accessibility_actions().increment.unwrap()();
                scene.settle_n(2);
                assert_ne!(
                    node.accessibility().unwrap().value,
                    before,
                    "{label} writes its own displayed state"
                );
            }
        },
    );
}
