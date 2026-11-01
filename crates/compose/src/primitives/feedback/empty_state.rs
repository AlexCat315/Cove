//! Translation of Primitives/Feedback/EmptyState.swift.
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::{ButtonIcon, ButtonIconSource};
use crate::primitives::text::Text;
use crate::theme::{SemanticColorRef, SemanticFontRef, Theme};
use crate::{AnyView, BoxView, TupleView, View};
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, Wrap};
use cove_imaging::BundleImageResource;

/// Vertical weight of an `EmptyState` block.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum EmptyStateProminence {
    /// Fits a table body or a short card: small glyph, tight padding.
    Compact,
    /// A panel-sized region. The default.
    #[default]
    Regular,
    /// A whole page: large glyph, generous padding.
    Spacious,
}

/// Explains why a region is empty and offers the one action that fixes it.
pub struct EmptyState {
    pub title: String,
    pub message: String,
    /// Glyph override. `None` uses the bundled "empty container" glyph.
    pub icon: Option<BundleImageResource>,
    pub shows_icon: bool,
    pub prominence: EmptyStateProminence,
    action: Option<AnyView>,
}

impl EmptyState {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            icon: None,
            shows_icon: true,
            prominence: EmptyStateProminence::Regular,
            action: None,
        }
    }

    pub fn with_action<Action: View + 'static>(
        title: impl Into<String>,
        message: impl Into<String>,
        action: Action,
    ) -> Self {
        let mut value = Self::new(title, message);
        value.action = Some(AnyView::new(action));
        value
    }

    pub fn icon(mut self, icon: BundleImageResource) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn shows_icon(mut self, shows_icon: bool) -> Self {
        self.shows_icon = shows_icon;
        self
    }

    pub fn prominence(mut self, prominence: EmptyStateProminence) -> Self {
        self.prominence = prominence;
        self
    }

    pub fn configure(mut self, configure: impl FnOnce(&mut Self)) -> Self {
        configure(&mut self);
        self
    }
}

impl View for EmptyState {
    fn body(&self) -> AnyView {
        let title = self.title.clone();
        let message = self.message.clone();
        let icon = self.icon.clone();
        let shows_icon = self.shows_icon;
        let prominence = self.prominence;
        let action = self.action.clone();
        AnyView::new(crate::theme::ThemeReader::new(move |theme: &Theme| {
            let metrics = EmptyStateMetrics::new(prominence, theme);
            let mut column: Vec<AnyView> = Vec::new();
            if shows_icon {
                let resource = icon
                    .clone()
                    .unwrap_or_else(crate::primitives::media::ui_icon_catalog::file::inbox);
                column.push(AnyView::new(
                    ButtonIcon::new(
                        ButtonIconSource::resource(resource),
                        Some(metrics.icon_size),
                        None,
                    )
                    .padding(metrics.chip_padding)
                    .background_color(theme.colors.surface_variant)
                    .corner_radius(theme.radius.pill)
                    .accessibility_hidden(true),
                ));
            }
            column.push(AnyView::new(
                Text::new(title.clone())
                    .semantic_font(metrics.title_font())
                    .semantic_foreground(SemanticColorRef::on_surface()),
            ));
            if !message.is_empty() {
                column.push(AnyView::new(
                    Text::new(message.clone())
                        .semantic_font(SemanticFontRef::body())
                        .semantic_foreground(SemanticColorRef::on_surface_muted()),
                ));
            }
            if let Some(action) = action.clone() {
                let insets =
                    crate::modifiers::layout::EdgeInsets::new(metrics.action_gap, 0.0, 0.0, 0.0);
                column.push(AnyView::new(action.padding_insets(insets)));
            }
            let label = title.clone();
            let help = message.clone();
            AnyView::new(
                BoxView::new(
                    FlexDirection::Column,
                    Align::Center,
                    Justify::Center,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(
                        BoxView::new(
                            FlexDirection::Column,
                            Align::Center,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            metrics.gap,
                            AnyView::new(TupleView::new(column)),
                        )
                        .frame(
                            None,
                            None,
                            None,
                            None,
                            Some(metrics.max_width),
                            None,
                        ),
                    ),
                )
                .padding(metrics.padding)
                .accessibility(move |semantics| {
                    semantics.role = AccessibilityRole::Group;
                    semantics.label = label.clone();
                    semantics.help = help.clone();
                }),
            )
        }))
    }
}

struct EmptyStateMetrics {
    icon_size: f32,
    chip_padding: f32,
    gap: f32,
    padding: f32,
    action_gap: f32,
    title_font: SemanticFontRef,
    max_width: f32,
}

impl EmptyStateMetrics {
    fn new(prominence: EmptyStateProminence, theme: &Theme) -> Self {
        match prominence {
            EmptyStateProminence::Compact => Self {
                icon_size: 18.0,
                chip_padding: 8.0,
                gap: 4.0,
                padding: theme.spacing.md,
                action_gap: theme.spacing.xs,
                title_font: SemanticFontRef::body_strong(),
                max_width: 320.0,
            },
            EmptyStateProminence::Regular => Self {
                icon_size: 24.0,
                chip_padding: 12.0,
                gap: 6.0,
                padding: theme.spacing.lg,
                action_gap: theme.spacing.sm,
                title_font: SemanticFontRef::headline(),
                max_width: 380.0,
            },
            EmptyStateProminence::Spacious => Self {
                icon_size: 32.0,
                chip_padding: 16.0,
                gap: 8.0,
                padding: theme.spacing.xl,
                action_gap: theme.spacing.md,
                title_font: SemanticFontRef::title(),
                max_width: 440.0,
            },
        }
    }

    fn title_font(&self) -> SemanticFontRef {
        self.title_font.clone()
    }
}
