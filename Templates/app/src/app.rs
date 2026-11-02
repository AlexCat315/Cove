use cove_ui::ApplicationLaunch;
use cove_ui::compose::modifiers::layout::PaddingExt;
use cove_ui::compose::primitives::controls::Button;
use cove_ui::compose::primitives::layout::{SafeArea, SafeAreaEdges};
use cove_ui::compose::primitives::text::text_buffer::TextBuffer;
use cove_ui::compose::primitives::text::{Text, TextField};
use cove_ui::composition::{
    AnyView, Column, HorizontalAlignment, Row, Scope, VerticalAlignment, View, views,
};

pub fn application() -> ApplicationLaunch {
    ApplicationLaunch::new(RootView).configure(|app| {
        app.window.title = "{{application_name}}".into();
    })
}

struct RootView;

impl View for RootView {
    fn body(&self, cx: &Scope) -> AnyView {
        let count = cx.state(|| 0_i64);
        let name = cx.state(|| TextBuffer::from("CoveUI"));

        AnyView::new(SafeArea::new(
            SafeAreaEdges::ALL,
            Column::new(
                HorizontalAlignment::Leading,
                20.0,
                AnyView::new(views!(
                    Text::new(crate::resources::welcome()),
                    Text::new(format!("Count: {}", count.get())),
                    Row::new(
                        VerticalAlignment::Center,
                        12.0,
                        AnyView::new(views!(
                            Button::title("Add one", move || count.update(|value| *value += 1)),
                            Button::title("Reset", move || count.set(0)),
                        ))
                    ),
                    TextField::new("Your name", name.binding()),
                )),
            )
            .padding(20.0),
        ))
    }
}
