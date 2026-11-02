//! Translation of Primitives/Data/Lists/Row.swift.
//!
//! Title and fixed slots. Touch rows stay within their proposal; desktop rows
//! retain horizontal overflow for dense, independently authored accessory slots.
use crate::Scope;
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::{ButtonIcon, ButtonIconSource};
use crate::primitives::layout::{ScrollAxis, ScrollView};
use crate::primitives::text::Text;
use crate::theme::styles::buttons::ButtonRole;
use crate::theme::{SemanticFontRef, ThemeReader, theme_for_node};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View, ViewExt, impl_primitive_view};
use cove_core::scene::{Align, FlexDirection, Justify, LayoutNode, Node, Wrap};

/// Authored options for one list row.
#[derive(Clone, Debug, PartialEq)]
pub struct ListRowOptions {
    pub subtitle: Option<String>,
    pub role: ButtonRole,
    pub shows_disclosure: bool,
}

impl Default for ListRowOptions {
    fn default() -> Self {
        Self {
            subtitle: None,
            role: ButtonRole::Normal,
            shows_disclosure: false,
        }
    }
}

impl ListRowOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn role(mut self, role: ButtonRole) -> Self {
        self.role = role;
        self
    }

    pub fn disclosure(mut self, shows_disclosure: bool) -> Self {
        self.shows_disclosure = shows_disclosure;
        self
    }
}

/// Title with leading and trailing accessory slots.
pub struct ListRow {
    pub title: String,
    pub options: ListRowOptions,
    leading: AnyView,
    trailing: AnyView,
}

impl ListRow {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            options: ListRowOptions::default(),
            leading: AnyView::new(crate::EmptyView),
            trailing: AnyView::new(crate::EmptyView),
        }
    }

    pub fn options(mut self, options: ListRowOptions) -> Self {
        self.options = options;
        self
    }

    pub fn leading(mut self, leading: impl View + 'static) -> Self {
        self.leading = AnyView::new(leading);
        self
    }

    pub fn trailing(mut self, trailing: impl View + 'static) -> Self {
        self.trailing = AnyView::new(trailing);
        self
    }
}

impl View for ListRow {
    fn body(&self, _cx: &Scope) -> AnyView {
        let title = self.title.clone();
        let options = self.options.clone();
        let leading = self.leading.clone();
        let trailing = self.trailing.clone();
        AnyView::new(crate::adaptive::SurfaceReader::new(
            move |surface: &cove_core::Environment| {
                let touch = surface.input.contains(cove_core::InputCapabilities::TOUCH);
                let title = title.clone();
                let options = options.clone();
                let leading = leading.clone();
                let trailing = trailing.clone();
                AnyView::new(ThemeReader::new(move |theme: &crate::theme::Theme| {
                    let title_color = if options.role == ButtonRole::Destructive {
                        theme.colors.error
                    } else {
                        theme.colors.on_surface
                    };
                    let title_content = match options.subtitle.clone() {
                        Some(subtitle) => AnyView::new(BoxView::new(
                            FlexDirection::Column,
                            Align::Stretch,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            4.0,
                            AnyView::new(TupleView::new(vec![
                                AnyView::new(
                                    Text::new(title.clone())
                                        .truncated()
                                        .clipped()
                                        .foreground_color(title_color),
                                ),
                                AnyView::new(
                                    Text::new(subtitle)
                                        .truncated()
                                        .clipped()
                                        .semantic_font(SemanticFontRef::caption())
                                        .semantic_foreground(
                                            crate::theme::SemanticColorRef::on_surface_muted(),
                                        ),
                                ),
                            ])),
                        )),
                        None => AnyView::new(
                            Text::new(title.clone())
                                .truncated()
                                .clipped()
                                .foreground_color(title_color),
                        ),
                    };
                    let trailing_content = if options.shows_disclosure {
                        AnyView::new(BoxView::new(
                            FlexDirection::Row,
                            Align::Center,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            8.0,
                            AnyView::new(TupleView::new(vec![
                                AnyView::new(trailing.clone().flex(0.0).shrink(0.0)),
                                AnyView::new(AnyView::new(ListDisclosure).modifier(
                                    crate::modifiers::interaction::AccessibilityModifier::new(
                                        |semantics| semantics.is_hidden = true,
                                    ),
                                )),
                            ])),
                        ))
                    } else {
                        AnyView::new(BoxView::new(
                            FlexDirection::Row,
                            Align::Center,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            0.0,
                            trailing.clone(),
                        ))
                    };
                    let contents = AnyView::new(
                        crate::LayoutContainer::new(
                            ListRowSlots { touch },
                            TupleView::new(vec![
                                AnyView::new(BoxView::new(
                                    FlexDirection::Row,
                                    Align::Center,
                                    Justify::FlexStart,
                                    Wrap::NoWrap,
                                    0.0,
                                    leading.clone(),
                                )),
                                title_content,
                                trailing_content,
                            ]),
                        )
                        .accessibility(|semantics| semantics.combines_children = true),
                    );
                    let scroll = ScrollView::new(ScrollAxis::Horizontal, contents)
                        .combine_accessibility(true)
                        .scroll_enabled(!touch);
                    AnyView::new(AnyView::new(scroll).frame(
                        if touch {
                            Some(crate::modifiers::layout::FrameDimension::Percent(100.0))
                        } else {
                            None
                        },
                        None,
                        Some(0.0),
                        None,
                        None,
                        None,
                    ))
                }))
            },
        ))
    }
}

/// Measure fixed slots before assigning the title its remaining width.
#[derive(Clone, Copy)]
struct ListRowSlots {
    touch: bool,
}

impl ListRowSlots {
    fn sizes(
        &self,
        width: Option<f32>,
        children: &[crate::LayoutSubview],
    ) -> (f32, Vec<cove_core::Size>) {
        if children.len() != 3 {
            return (0.0, Vec::new());
        }
        let natural: Vec<_> = children
            .iter()
            .map(|child| child.size_that_fits(crate::ProposedViewSize::UNSPECIFIED))
            .collect();
        let floor = natural[1].width.min(80.0);
        let natural_width = natural.iter().map(|size| size.width).sum();
        let width = width.unwrap_or(natural_width);
        let width = if self.touch {
            width
        } else {
            width.max(natural[0].width + floor + natural[2].width)
        };
        let leading = natural[0].width.min(width);
        let trailing = if self.touch {
            natural[2].width.min((width - leading - floor).max(0.0))
        } else {
            natural[2].width
        };
        let title = (width - leading - trailing).max(0.0);
        let sizes = [leading, title, trailing]
            .into_iter()
            .zip(children)
            .map(|(width, child)| {
                let size = child.size_that_fits(crate::ProposedViewSize::new(Some(width), None));
                cove_core::Size {
                    width,
                    height: size.height,
                }
            })
            .collect();
        (width, sizes)
    }
}

impl crate::Layout for ListRowSlots {
    fn size_that_fits(
        &self,
        proposal: crate::ProposedViewSize,
        children: &[crate::LayoutSubview],
    ) -> cove_core::Size {
        let (width, sizes) = self.sizes(proposal.width, children);
        cove_core::Size {
            width,
            height: sizes
                .iter()
                .map(|size| size.height)
                .fold(if self.touch { 44.0 } else { 0.0 }, f32::max),
        }
    }
    fn place_subviews(
        &self,
        bounds: cove_core::Rect,
        _: crate::ProposedViewSize,
        children: &[crate::LayoutSubview],
    ) {
        let (_, sizes) = self.sizes(Some(bounds.width), children);
        let rtl = children
            .first()
            .is_some_and(|child| child.layout_direction == cove_core::scene::Direction::Rtl);
        let mut cursor = if rtl { bounds.max_x() } else { bounds.x };
        for (child, size) in children.iter().zip(sizes) {
            if rtl {
                cursor -= size.width;
            }
            child.place(
                cove_core::Point::new(cursor, bounds.y + (bounds.height - size.height) / 2.0),
                crate::ProposedViewSize::new(Some(size.width), Some(size.height)),
                true,
            );
            if !rtl {
                cursor += size.width;
            }
        }
    }
}

/// Trailing chevron for a row that opens a detail screen.
struct ListDisclosure;

impl PrimitiveView for ListDisclosure {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_width(Some(12.0));
        layout.set_height(Some(12.0));
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        let theme = theme_for_node(node);
        node.set_foreground_color(Some(theme.colors.on_surface_muted));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![AnyView::new(ButtonIcon::new(
            ButtonIconSource::resource(
                crate::primitives::media::ui_icon_catalog::navigation::chevron_right(),
            ),
            Some(12.0),
            None,
        ))]
    }
}
impl_primitive_view!(ListDisclosure);
