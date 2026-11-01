//! Translation of Primitives/Media/Icon.swift.
//!
//! Themed square icon: the asset's alpha channel is used as coverage and the
//! foreground color as fill, so icons follow theme tokens instead of their
//! source colors.
use crate::primitives::media::{Image, ImageContentMode, ImageRenderingMode};
use crate::theme::{
    IconMetrics, IconMetricsReader, SemanticColorRef, SemanticIconSize, Theme, ThemeReader,
};
use crate::{AnyView, BoxView, View};
use cove_core::Color;
use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};
use cove_imaging::BundleImageResource;

#[derive(Clone)]
enum IconFill {
    Inherited,
    Semantic(SemanticColorRef),
    Concrete(Color),
}

#[derive(Clone)]
enum IconSize {
    Points(f32),
    Semantic(SemanticIconSize),
}

/// Themed square icon rendered through the alpha-mask path.
pub struct Icon {
    resource: BundleImageResource,
    size: IconSize,
    fill: IconFill,
}

impl Icon {
    pub fn new(resource: BundleImageResource, size: f32) -> Self {
        Self {
            resource,
            size: IconSize::Points(size),
            fill: IconFill::Inherited,
        }
    }

    pub fn with_color(resource: BundleImageResource, size: f32, color: Color) -> Self {
        Self {
            resource,
            size: IconSize::Points(size),
            fill: IconFill::Concrete(color),
        }
    }

    pub fn with_semantic(
        resource: BundleImageResource,
        size: f32,
        color: SemanticColorRef,
    ) -> Self {
        Self {
            resource,
            size: IconSize::Points(size),
            fill: IconFill::Semantic(color),
        }
    }

    pub fn semantic(resource: BundleImageResource, size: SemanticIconSize) -> Self {
        Self {
            resource,
            size: IconSize::Semantic(size),
            fill: IconFill::Inherited,
        }
    }

    pub fn semantic_with_color(
        resource: BundleImageResource,
        size: SemanticIconSize,
        color: Color,
    ) -> Self {
        Self {
            resource,
            size: IconSize::Semantic(size),
            fill: IconFill::Concrete(color),
        }
    }

    pub fn semantic_with_ref(
        resource: BundleImageResource,
        size: SemanticIconSize,
        color: SemanticColorRef,
    ) -> Self {
        Self {
            resource,
            size: IconSize::Semantic(size),
            fill: IconFill::Semantic(color),
        }
    }

    #[expect(dead_code, reason = "ported from Swift but not wired in yet")]
    fn resolved_fill(&self, theme: &Theme) -> Option<Color> {
        match &self.fill {
            IconFill::Inherited => None,
            IconFill::Semantic(reference) => Some(reference.resolve(theme)),
            IconFill::Concrete(color) => Some(*color),
        }
    }
}

impl View for Icon {
    fn body(&self) -> AnyView {
        let resource = self.resource.clone();
        let size = self.size.clone();
        let fill = self.fill.clone();
        AnyView::new(ThemeReader::new(move |theme: &Theme| {
            let resource = resource.clone();
            let size = size.clone();
            let fill = fill.clone();
            let theme = theme.clone();
            IconMetricsReader::new(move |node: &Node| {
                let metrics = IconMetrics::new(node);
                let resolved = match &size {
                    IconSize::Points(value) => metrics.size(*value),
                    IconSize::Semantic(role) => metrics.size_role(role),
                };
                let themed = theme.icons.resolve(&resource);
                let image = Image::from_resource(
                    &themed,
                    resolved,
                    resolved,
                    Color::WHITE,
                    ImageContentMode::Fit,
                    ImageRenderingMode::AlphaMask,
                );
                let image = match &fill {
                    IconFill::Inherited => image,
                    IconFill::Semantic(reference) => image.foreground(reference.resolve(&theme)),
                    IconFill::Concrete(color) => image.foreground(*color),
                };
                // Keep layout slots/padding separate from the SVG's own size.
                // A surrounding frame is a container, not a glyph resize.
                BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::Center,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(image),
                )
            })
        }))
    }
}
