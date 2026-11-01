//! Translation of Primitives/Layout/ResizableEditor.swift.
//!
//! Vertical resize surface for multiline/code inputs. Drag the bottom grip;
//! content owns its text scrolling and the parent owns its surrounding layout.
use crate::Scope;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};

use crate::modifiers::layout::{FlexExt, LayoutFrameExt};
use crate::primitives::layout::resize::{EditorHeightHandle, EditorHeightLimits};
use crate::adaptive::SurfaceReader;
use crate::{AnyView, BoxView, LayoutDebugExt, TupleView, View};
use cove_core::Environment;

/// Vertical resize surface for multiline and code inputs.
pub struct ResizableEditor {
    limits: EditorHeightLimits,
    resize_label: String,
    is_resizable: bool,
    initial_height: f32,
    content: AnyView,
}

impl ResizableEditor {
    pub fn new(content: impl View + 'static) -> Self {
        let limits = EditorHeightLimits::new(96.0, 520.0);
        Self {
            limits,
            resize_label: "Editor height".to_string(),
            is_resizable: true,
            initial_height: 160.0,
            content: AnyView::new(content),
        }
    }

    pub fn initial_height(mut self, initial_height: f32) -> Self {
        self.initial_height = initial_height;
        self
    }

    pub fn limits(mut self, minimum: f32, maximum: f32) -> Self {
        self.limits = EditorHeightLimits::new(minimum.max(32.0), maximum);
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
}

impl View for ResizableEditor {
    fn body(&self, cx: &Scope) -> AnyView {
        let height = cx.state(|| self.limits.clamp(self.initial_height));
        let limits = self.limits;
        let content = self.content.clone();
        let resize_label = self.resize_label.clone();
        let is_resizable = self.is_resizable;
        AnyView::new(SurfaceReader::new(move |surface: &Environment| {
            // The grip lane takes its own height out of the editor's viewport,
            // so the content never hides behind it; it is one control tall.
            let extent = surface.minimum_control_extent();
            let viewport_limits = EditorHeightLimits::new(
                extent.max(limits.minimum - extent),
                limits.maximum - extent,
            );
            let viewport = height.binding().map(
                move |height| viewport_limits.clamp(*height - extent),
                move |height, value| *height = viewport_limits.clamp(value) + extent,
            );
            let current = viewport.get();
            let handle = EditorHeightHandle::new(
                viewport,
                viewport_limits,
                resize_label.clone(),
                is_resizable,
                extent,
            )
            .debug_name("editor-height-handle");
            AnyView::new(
                BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(TupleView::new(vec![
                        AnyView::new(
                            AnyView::new(content.clone())
                                .flex_with(1.0, 1.0, None)
                                .frame(None, None, None, Some(0.0), None, None),
                        ),
                        AnyView::new(handle),
                    ])),
                )
                .frame_height(current + extent)
                .frame_min_width(0.0),
            )
        }))
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
