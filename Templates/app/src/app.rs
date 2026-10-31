//! Shared application root, installed once per session by every platform host.

use cove_ui::compose::primitives::controls::Button;
use cove_ui::compose::primitives::layout::{SafeArea, SafeAreaEdges};
use cove_ui::compose::primitives::text::text_buffer::TextBuffer;
use cove_ui::compose::primitives::text::{Text, TextField};
use cove_ui::composition::{AnyView, BoxView, TupleView, View};
use cove_ui::core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui::core::State;

/// The application content shared by every platform host.
pub fn make_application_content() -> impl View + 'static {
    RootView::default()
}

pub struct RootView {
    count: State<i64>,
    name: State<TextBuffer>,
}

impl Default for RootView {
    fn default() -> Self {
        // `State` owns its storage and has no `Default`; the authored initial
        // values are what the Swift template's `@State` declarations carry.
        Self { count: State::new(0), name: State::new(TextBuffer::from("CoveUI")) }
    }
}

impl View for RootView {
    fn body(&self) -> AnyView {
        let count = self.count.clone();
        let increment = count.clone();
        let reset = count.clone();
        let label = format!("Count: {}", count.get());

        AnyView::new(SafeArea::new(
            SafeAreaEdges::ALL,
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                20.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(Text::new(crate::resources::welcome())),
                    AnyView::new(Text::new(label)),
                    AnyView::new(BoxView::new(
                        FlexDirection::Row,
                        Align::Center,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        12.0,
                        AnyView::new(TupleView::new(vec![
                            AnyView::new(Button::new(Text::new("Add one"), move || {
                                increment.set(increment.get() + 1);
                            })),
                            AnyView::new(Button::new(Text::new("Reset"), move || {
                                reset.set(0);
                            })),
                        ])),
                    )),
                    AnyView::new(TextField::new("Your name", self.name.binding())),
                ])),
            ),
        ))
    }
}
