//! Translation of Theme/Styles/Components/DefaultPanelStyle.swift.
//!
//! Panel default — the macOS grouped box: the panel body is a rounded slab of
//! `surface_variant`, one step of fill off the white window, with no border
//! and no header fill. Separation between neighbouring panels comes from the canvas
//! showing through the gutters, not from lines. The header has a 34 pt minimum
//! and grows with its title and accessories. Active panels brighten the title
//! rather than repainting the header.
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::text::Text;
use crate::theme::{SemanticColorRef, SemanticFontRef};
use crate::{AnyView, BoxView, Row, Spacer, TupleView, VerticalAlignment};
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};

use super::panel_style::{PanelStyle, PanelStyleConfiguration};

#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultPanelStyle;

impl PanelStyle for DefaultPanelStyle {
    fn make_body(&self, configuration: PanelStyleConfiguration) -> AnyView {
        let theme = configuration.theme.clone();
        let title_color = if configuration.is_active {
            SemanticColorRef::on_surface()
        } else {
            SemanticColorRef::on_surface_muted()
        };
        // Header and content consume the allocated panel width. Intrinsic
        // leading alignment would strand container-query content at zero width.
        let header = AnyView::new(
            Row::new(
                VerticalAlignment::Center,
                theme.spacing.sm,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(
                        Text::new(configuration.title.clone())
                            .truncated()
                            .semantic_font(SemanticFontRef::label())
                            .semantic_foreground(title_color)
                            .frame_min_width(0.0)
                            .flex_with(0.0, 1.0, None),
                    ),
                    AnyView::new(Spacer::new(0.0)),
                    configuration.accessory.clone(),
                ])),
            )
            .padding_edges(theme.spacing.lg, 8.0)
            .frame(None, None, None, Some(34.0), None, None),
        );
        let body = AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(configuration.content.clone().flex(1.0)),
            )
            .flex(1.0)
            .padding_insets(crate::modifiers::layout::EdgeInsets::new(
                0.0,
                theme.spacing.md,
                theme.spacing.md,
                theme.spacing.md,
            )),
        );
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(vec![header, body])),
            )
            .semantic_background(SemanticColorRef::surface_variant())
            .corner_radius(theme.radius.xl)
            .clipped(),
        )
    }
}
