//! Translation of Primitives/Layout/ResizableEditor.swift.
//!
//! Vertical resize surface for multiline/code inputs. Drag the bottom grip;
//! content owns its text scrolling and the parent owns its surrounding layout.
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_core::{Binding, State};

use crate::modifiers::layout::{FlexExt, LayoutFrameExt};
use crate::primitives::layout::resize::{EditorHeightHandle, EditorHeightLimits};
use crate::{AnyView, BoxView, TupleView, View};

/// Vertical resize surface for multiline and code inputs.
pub struct ResizableEditor {
    limits: EditorHeightLimits,
    resize_label: String,
    is_resizable: bool,
    height: State<f32>,
    content: AnyView,
}

impl ResizableEditor {
    pub fn new(content: impl View + 'static) -> Self {
        let limits = EditorHeightLimits::new(96.0, 520.0);
        Self {
            limits,
            resize_label: "Editor height".to_string(),
            is_resizable: true,
            height: State::new(limits.clamp(160.0)),
            content: AnyView::new(content),
        }
    }

    pub fn initial_height(self, initial_height: f32) -> Self {
        self.height.set(self.limits.clamp(initial_height));
        self
    }

    pub fn limits(mut self, minimum: f32, maximum: f32) -> Self {
        self.limits = EditorHeightLimits::new(minimum.max(32.0), maximum);
        self.height.set(self.limits.clamp(self.height.get()));
        self
    }

    pub fn resize_label(mut self, label: impl Into<String>) -> Self {
        self.resize_label = label.into();
        self
    }

    pub fn resizable(mut self, is_resizable: bool) -> Self {
        self.is_resizable = is_resizable;
        self
    }

    pub fn height(&self) -> Binding<f32> {
        self.height.binding()
    }
}

impl View for ResizableEditor {
    fn body(&self) -> AnyView {
        // The grip lane takes its own height out of the editor's viewport, so
        // the content never hides behind it.
        let extent = 12.0;
        let viewport_limits = EditorHeightLimits::new(
            (self.limits.minimum - extent).max(extent),
            (self.limits.maximum - extent).max(extent),
        );
        let height = self.height.clone();
        let viewport = Binding::new(
            {
                let height = height.clone();
                move || viewport_limits.clamp(height.get() - extent)
            },
            {
                let height = height.clone();
                move |value: f32| height.set(viewport_limits.clamp(value) + extent)
            },
        );
        let current = viewport.get();
        let handle = EditorHeightHandle::new(
            viewport,
            viewport_limits,
            self.resize_label.clone(),
            self.is_resizable,
            extent,
        );
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(
                        AnyView::new(self.content.clone())
                            .flex_with(1.0, 1.0, None)
                            .frame(None, None, None, Some(0.0), None, None),
                    ),
                    AnyView::new(handle),
                ])),
            )
            .frame_height(current + extent)
            .frame_min_width(0.0),
        )
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
