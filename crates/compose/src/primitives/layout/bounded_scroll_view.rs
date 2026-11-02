//! Translation of Primitives/Layout/BoundedScrollView.swift.
//!
//! A scroll view whose height is the content's extent clamped into a range. The
//! content is measured unless the caller authors its height.
use crate::Scope;
use crate::modifiers::layout::LayoutFrameExt;
use crate::primitives::layout::{ScrollAxis, ScrollGeometry, ScrollView, ScrollbarGutter};
use crate::{AnyView, View};
use cove_core::State;
use cove_core::scene::ScrollConsumePolicy;

/// Scroll view sized to its content within a height range.
pub struct BoundedScrollView {
    pub axes: ScrollAxis,
    /// Authored content height; `None` measures the content.
    pub content_height: Option<f32>,
    pub min_height: f32,
    pub max_height: f32,
    pub consume_policy: ScrollConsumePolicy,
    pub scrollbar_gutter: ScrollbarGutter,
    content: AnyView,
    measured_height: State<f32>,
}

impl BoundedScrollView {
    pub fn new(content: impl View + 'static) -> Self {
        Self {
            axes: ScrollAxis::Vertical,
            content_height: None,
            min_height: 0.0,
            max_height: f32::INFINITY,
            consume_policy: ScrollConsumePolicy::WhenScrollable,
            scrollbar_gutter: ScrollbarGutter::Stable,
            content: AnyView::new(content),
            measured_height: State::new(0.0),
        }
    }

    pub fn content_height(mut self, content_height: f32) -> Self {
        self.content_height = Some(content_height);
        self
    }

    pub fn min_height(mut self, min_height: f32) -> Self {
        self.min_height = min_height;
        self
    }

    pub fn max_height(mut self, max_height: f32) -> Self {
        self.max_height = max_height;
        self
    }

    pub fn axes(mut self, axes: ScrollAxis) -> Self {
        self.axes = axes;
        self
    }

    pub fn consume_policy(mut self, policy: ScrollConsumePolicy) -> Self {
        self.consume_policy = policy;
        self
    }

    pub fn scrollbar_gutter(mut self, gutter: ScrollbarGutter) -> Self {
        self.scrollbar_gutter = gutter;
        self
    }

    /// The height the viewport occupies for the authored extents.
    pub fn viewport_height(&self) -> f32 {
        Self::resolved_viewport_height(
            self.content_height
                .unwrap_or_else(|| self.measured_height.get()),
            self.min_height,
            self.max_height,
        )
    }

    pub fn resolved_viewport_height(content_height: f32, min_height: f32, max_height: f32) -> f32 {
        content_height.max(min_height).min(max_height)
    }
}

impl View for BoundedScrollView {
    fn body(&self, _cx: &Scope) -> AnyView {
        let viewport = self.viewport_height();
        let content = match self.content_height {
            Some(height) => AnyView::new(self.content.clone().frame_height(height)),
            None => self.content.clone(),
        };
        let measured = self.measured_height.clone();
        let measures = self.content_height.is_none();
        AnyView::new(
            ScrollView::new(self.axes, content)
                .consume_policy(self.consume_policy)
                .scrollbar_gutter(self.scrollbar_gutter)
                .on_geometry_change(move |geometry: ScrollGeometry| {
                    if measures && (geometry.content_size.height - measured.get()).abs() > 0.5 {
                        measured.set(geometry.content_size.height);
                    }
                })
                .frame_height(viewport),
        )
    }

    fn dynamic_properties(&self) -> Vec<crate::DynamicField> {
        vec![crate::DynamicField::state(
            "measured_height",
            &self.measured_height,
        )]
    }
}
