//! Translation of Primitives/Layout/BoundedScrollView.swift.
//!
//! A scroll view whose height is the content's extent clamped into a range.
use cove_core::scene::ScrollConsumePolicy;
use crate::modifiers::layout::LayoutFrameExt;
use crate::primitives::layout::{ScrollAxis, ScrollView, ScrollbarGutter};
use crate::{AnyView, View};

/// Scroll view sized to its content within a height range.
pub struct BoundedScrollView {
    pub axes: ScrollAxis,
    pub content_height: f32,
    pub min_height: f32,
    pub max_height: f32,
    pub consume_policy: ScrollConsumePolicy,
    pub scrollbar_gutter: ScrollbarGutter,
    content: AnyView,
}

impl BoundedScrollView {
    pub fn new(content: impl View + 'static) -> Self {
        Self {
            axes: ScrollAxis::Vertical,
            content_height: 0.0,
            min_height: 0.0,
            max_height: f32::INFINITY,
            consume_policy: ScrollConsumePolicy::WhenScrollable,
            scrollbar_gutter: ScrollbarGutter::Stable,
            content: AnyView::new(content),
        }
    }

    pub fn content_height(mut self, content_height: f32) -> Self {
        self.content_height = content_height;
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
        Self::resolved_viewport_height(self.content_height, self.min_height, self.max_height)
    }

    pub fn resolved_viewport_height(content_height: f32, min_height: f32, max_height: f32) -> f32 {
        content_height.max(min_height).min(max_height)
    }
}

impl View for BoundedScrollView {
    fn body(&self) -> AnyView {
        let viewport = self.viewport_height();
        AnyView::new(
            ScrollView::new(self.axes, self.content.clone())
                .consume_policy(self.consume_policy)
                .scrollbar_gutter(self.scrollbar_gutter)
                .frame_height(self.content_height)
                .frame_height(viewport),
        )
    }
}
