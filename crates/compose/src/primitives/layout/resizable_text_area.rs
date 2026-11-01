//! Translation of Primitives/Layout/ResizableTextArea.swift.
//!
//! Multiline input with a draggable lower edge and internal text scrolling.
use crate::Scope;
use std::rc::Rc;

use cove_composition::{AnyView, View, adaptive::SurfaceReader};
use cove_core::Binding;
use cove_core::scene::{Align, FlexDirection, Wrap};

use crate::BoxView;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt};
use crate::primitives::layout::resize::{EditorHeightHandle, EditorHeightLimits};
use crate::primitives::text::text_buffer::TextBuffer;
use crate::primitives::text::text_field::{
    TextField, TextFieldAxis, TextFieldBehavior, TextFieldCodeEditing, TextFieldEvents,
    TextFieldLayout,
};

/// Multiline input with a draggable lower edge and internal text scrolling.
pub struct ResizableTextArea {
    text: Binding<TextBuffer>,
    placeholder: String,
    limits: EditorHeightLimits,
    resize_label: String,
    disabled: bool,
    on_submit: Option<Rc<dyn Fn()>>,
    on_focus: Option<Rc<dyn Fn()>>,
    on_blur: Option<Rc<dyn Fn()>>,
    code_editing: TextFieldCodeEditing,
}

impl ResizableTextArea {
    pub fn new(
        placeholder: impl Into<String>,
        text: Binding<TextBuffer>,
        min_height: f32,
        max_height: f32,
        disabled: bool,
        resize_label: impl Into<String>,
        code_editing: TextFieldCodeEditing,
        on_submit: Option<Rc<dyn Fn()>>,
        on_focus: Option<Rc<dyn Fn()>>,
        on_blur: Option<Rc<dyn Fn()>>,
    ) -> Self {
        let limits = EditorHeightLimits::new(min_height.max(32.0), max_height);
        Self {
            text,
            placeholder: placeholder.into(),
            limits,
            resize_label: resize_label.into(),
            disabled,
            on_submit,
            on_focus,
            on_blur,
            code_editing,
        }
    }

    pub fn simple(
        text: Binding<TextBuffer>,
        placeholder: impl Into<String>,
        disabled: bool,
    ) -> Self {
        Self::new(
            placeholder,
            text,
            96.0,
            360.0,
            disabled,
            "Editor height",
            TextFieldCodeEditing::default(),
            None,
            None,
            None,
        )
    }
}

impl View for ResizableTextArea {
    fn body(&self, cx: &Scope) -> AnyView {
        let text = self.text.clone();
        let placeholder = self.placeholder.clone();
        let limits = self.limits;
        let resize_label = self.resize_label.clone();
        let disabled = self.disabled;
        let on_submit = self.on_submit.clone();
        let on_focus = self.on_focus.clone();
        let on_blur = self.on_blur.clone();
        let code_editing = self.code_editing.clone();
        let height_state = cx.state(|| self.limits.minimum);
        let reader = SurfaceReader::new(move |surface| {
            let extent = surface.minimum_control_extent();
            let viewport_limits =
                EditorHeightLimits::new(extent.max(limits.minimum), limits.maximum);
            let viewport = height_state.binding();
            let clamped = viewport_limits.clamp(viewport.get());

            let layout = TextFieldLayout {
                axis: TextFieldAxis::Vertical,
                max_visible_lines: 128,
                ..TextFieldLayout::default()
            };
            let behavior = TextFieldBehavior {
                disabled,
                ..TextFieldBehavior::default()
            };
            let events = TextFieldEvents {
                on_submit: on_submit.clone(),
                on_focus: on_focus.clone(),
                on_blur: on_blur.clone(),
                ..TextFieldEvents::default()
            };
            let field = TextField::new(placeholder.clone(), text.clone())
                .layout(layout)
                .behavior(behavior)
                .code_editing(code_editing.clone())
                .events(events)
                .frame(
                    None,
                    Some(crate::modifiers::layout::FrameDimension::Points(clamped)),
                    None,
                    None,
                    None,
                    None,
                )
                .flex_with(0.0, 0.0, None);

            let handle = EditorHeightHandle::new(
                viewport,
                viewport_limits,
                resize_label.clone(),
                !disabled,
                extent,
            );

            let column = BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                cove_core::scene::Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(crate::TupleView::new(vec![
                    AnyView::new(field),
                    AnyView::new(handle),
                ])),
            );
            AnyView::new(column)
        });
        AnyView::new(reader)
    }
}
