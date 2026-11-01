//! Translation of Primitives/Layout/StatusBar.swift.
//!
//! One line of persistent context at the bottom of a window. Enforces a single
//! height for every cell and leading/trailing slots that never fight for space.

use crate::Scope;
use crate::{Layout, LayoutContainer, LayoutSubview, ProposedViewSize};
use cove_composition::{AnyView, View};
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Point, Rect, Size, Wrap};

use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::feedback::StatusTone;
use crate::primitives::media::Icon;
use crate::primitives::text::{Divider, DividerAxis, Text};
use crate::theme::{SemanticColorRef, SemanticFontRef, Theme, ThemeReader};
use crate::{BoxView, Row, TupleView};

/// Shared height so swapping cells never changes the window's content height.
pub struct StatusBarMetrics;

impl StatusBarMetrics {
    pub const HEIGHT: f32 = 26.0;
}

/// One line of persistent context at the bottom of a window.
pub struct StatusBar {
    leading: AnyView,
    trailing: AnyView,
    shows_divider: bool,
    is_enabled: bool,
}

impl StatusBar {
    pub fn new(leading: AnyView, trailing: AnyView) -> Self {
        Self {
            leading,
            trailing,
            shows_divider: true,
            is_enabled: true,
        }
    }

    pub fn leading_only(leading: AnyView) -> Self {
        Self::new(leading, AnyView::new(crate::EmptyView))
    }

    pub fn trailing_only(trailing: AnyView) -> Self {
        Self::new(AnyView::new(crate::EmptyView), trailing)
    }

    pub fn shows_divider(mut self, shows_divider: bool) -> Self {
        self.shows_divider = shows_divider;
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }
}

impl View for StatusBar {
    fn body(&self, _cx: &Scope) -> AnyView {
        let leading = self.leading.clone();
        let trailing = self.trailing.clone();
        let shows_divider = self.shows_divider;
        let is_enabled = self.is_enabled;
        let content = ThemeReader::new(move |theme: &Theme| {
            let mut column_items: Vec<AnyView> = Vec::new();
            if shows_divider {
                column_items.push(AnyView::new(Divider::new(
                    None,
                    1.0,
                    DividerAxis::Horizontal,
                )));
            }
            let leading_row = Row::new(
                crate::VerticalAlignment::Center,
                theme.spacing.md,
                leading.clone(),
            )
            .frame_min_width(0.0);
            let trailing_row = Row::new(
                crate::VerticalAlignment::Center,
                theme.spacing.md,
                trailing.clone(),
            )
            .frame_min_width(0.0);
            let layout = LayoutContainer::new(
                StatusBarLayout {
                    spacing: theme.spacing.md,
                },
                AnyView::new(TupleView::new(vec![
                    AnyView::new(leading_row),
                    AnyView::new(trailing_row),
                ])),
            );
            let bar = AnyView::new(layout)
                .frame(
                    None,
                    Some(crate::modifiers::layout::FrameDimension::Points(
                        StatusBarMetrics::HEIGHT,
                    )),
                    None,
                    None,
                    None,
                    None,
                )
                .padding_edges(theme.spacing.md, 0.0)
                .semantic_background(SemanticColorRef::surface_raised())
                .opacity(if is_enabled { 1.0 } else { 0.6 });
            column_items.push(AnyView::new(bar));
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                cove_core::scene::Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(column_items)),
            )
            .accessibility(move |semantics| {
                semantics.role = AccessibilityRole::Group;
                semantics.label = "Status bar".to_string();
                semantics.state.is_enabled = is_enabled;
            })
        });
        AnyView::new(content)
    }
}

/// A single status-bar cell: one short fact, optionally toned and monospaced.
pub struct StatusBarText {
    text: String,
    detail: String,
    icon: Option<cove_imaging::BundleImageResource>,
    tone: Option<StatusTone>,
    is_monospaced: bool,
}

impl StatusBarText {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            detail: String::new(),
            icon: None,
            tone: None,
            is_monospaced: false,
        }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }
    pub fn icon(mut self, icon: cove_imaging::BundleImageResource) -> Self {
        self.icon = Some(icon);
        self
    }
    pub fn tone(mut self, tone: StatusTone) -> Self {
        self.tone = Some(tone);
        self
    }
    pub fn monospaced(mut self, is_monospaced: bool) -> Self {
        self.is_monospaced = is_monospaced;
        self
    }
}

impl View for StatusBarText {
    fn body(&self, _cx: &Scope) -> AnyView {
        let text = self.text.clone();
        let detail = self.detail.clone();
        let icon = self.icon.clone();
        let tone = self.tone;
        let is_monospaced = self.is_monospaced;
        let content = ThemeReader::new(move |_theme: &Theme| {
            let ink = tone
                .map(|tone| tone.color())
                .unwrap_or_else(SemanticColorRef::on_surface_variant);
            let mut items: Vec<AnyView> = Vec::new();
            if let Some(icon) = &icon {
                let icon_view =
                    Icon::with_semantic(icon.clone(), 11.0, ink.clone()).accessibility_hidden(true);
                items.push(AnyView::new(icon_view));
            }
            let font = if is_monospaced {
                SemanticFontRef::mono()
            } else {
                SemanticFontRef::caption()
            };
            let main = Text::new(text.clone())
                .truncated()
                .semantic_font(font.clone())
                .semantic_foreground(ink.clone())
                .frame_min_width(0.0)
                .flex_with(0.0, 1.0, None);
            items.push(AnyView::new(main));
            if !detail.is_empty() {
                let detail_view = Text::new(detail.clone())
                    .truncated()
                    .semantic_font(font)
                    .semantic_foreground(SemanticColorRef::on_surface_muted())
                    .frame_min_width(0.0)
                    .flex_with(0.0, 1.0, None);
                items.push(AnyView::new(detail_view));
            }

            Row::new(
                crate::VerticalAlignment::Center,
                4.0,
                AnyView::new(TupleView::new(items)),
            )
            .frame(None, None, Some(0.0), None, None, None)
            .accessibility({
                let text = text.clone();
                let detail = detail.clone();
                move |semantics| {
                    semantics.role = AccessibilityRole::StaticText;
                    semantics.label = text.clone();
                    let mut value = text.clone();
                    if !detail.is_empty() {
                        value.push_str(", ");
                        value.push_str(&detail);
                    }
                    semantics.value = value;
                }
            })
        });
        AnyView::new(content)
    }
}

/// Custom layout: the trailing cell keeps its natural width; the leading cell
/// takes the remaining space and truncates rather than painting into trailing.
#[derive(Clone, Copy)]
struct StatusBarLayout {
    spacing: f32,
}

/// Extra gutter beyond the authored spacing so a truncated leading cell cannot
/// paint its clipped tail into the trailing state.
const TRAILING_GAP: f32 = 12.0;

impl StatusBarLayout {
    fn safe(size: Size) -> Size {
        Size {
            width: if size.width.is_finite() {
                size.width.max(0.0)
            } else {
                0.0
            },
            height: if size.height.is_finite() {
                size.height.max(0.0)
            } else {
                0.0
            },
        }
    }
}

impl Layout for StatusBarLayout {
    fn size_that_fits(&self, proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> Size {
        if subviews.len() != 2 {
            return Size::default();
        }
        let trailing = Self::safe(subviews[1].size_that_fits(ProposedViewSize::UNSPECIFIED));
        if proposal.width.is_none() {
            let leading = Self::safe(subviews[0].size_that_fits(ProposedViewSize::UNSPECIFIED));
            return Size {
                width: leading.width + trailing.width + self.spacing,
                height: leading.height.max(trailing.height),
            };
        }
        let width = proposal.width.unwrap_or(0.0);
        let leading_width = (width - trailing.width - self.spacing - TRAILING_GAP).max(0.0);
        let leading = if leading_width > 0.0 {
            Self::safe(subviews[0].size_that_fits(ProposedViewSize::new(Some(leading_width), None)))
        } else {
            Size::default()
        };
        Size {
            width: width.max(0.0),
            height: leading.height.max(trailing.height),
        }
    }

    fn place_subviews(
        &self,
        bounds: Rect,
        _proposal: ProposedViewSize,
        subviews: &[LayoutSubview],
    ) {
        if subviews.len() != 2 {
            return;
        }
        let width = if bounds.width.is_finite() {
            bounds.width.max(0.0)
        } else {
            0.0
        };
        let height = if bounds.height.is_finite() {
            bounds.height.max(0.0)
        } else {
            0.0
        };
        let trailing = Self::safe(subviews[1].size_that_fits(ProposedViewSize::UNSPECIFIED));
        let leading_width = (width - trailing.width - self.spacing - TRAILING_GAP).max(0.0);
        let leading = if leading_width > 0.0 {
            Self::safe(subviews[0].size_that_fits(ProposedViewSize::new(Some(leading_width), None)))
        } else {
            Size::default()
        };
        let leading_x = bounds.x;
        let trailing_x = bounds.x + width - trailing.width;
        let leading_y = bounds.y + (height - leading.height).max(0.0) / 2.0;
        let trailing_y = bounds.y + (height - trailing.height).max(0.0) / 2.0;
        if leading_width > 0.0 {
            subviews[0].place(
                Point::new(leading_x, leading_y),
                ProposedViewSize::new(Some(leading_width), Some(leading.height)),
                true,
            );
        } else {
            subviews[0].place(
                Point::new(bounds.x, bounds.y),
                ProposedViewSize::new(Some(0.0), Some(0.0)),
                false,
            );
        }
        if trailing.width > 0.0 {
            subviews[1].place(
                Point::new(trailing_x, trailing_y),
                ProposedViewSize::new(Some(trailing.width), Some(trailing.height)),
                true,
            );
        } else {
            subviews[1].place(
                Point::new(bounds.x, bounds.y),
                ProposedViewSize::new(Some(0.0), Some(0.0)),
                false,
            );
        }
    }
}
