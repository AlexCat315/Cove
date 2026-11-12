//! Translation of Primitives/Data/Lists/Row.swift.
//!
//! Title and fixed slots. Touch rows stay within their proposal; desktop rows
//! retain horizontal overflow for dense, independently authored accessory slots.
use crate::Scope;
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::{ButtonIcon, ButtonIconSource};
use crate::primitives::layout::{ScrollAxis, ScrollView};
use crate::primitives::media::Icon;
use crate::primitives::text::Text;
use crate::theme::styles::buttons::ButtonRole;
use crate::theme::styles::components::list_row_style::{ListRowContext, ListRowContextReader};
use crate::theme::tokens::{SemanticColorRef, SemanticIconSize};
use crate::theme::{SemanticFontRef, ThemeReader};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View, ViewExt, impl_primitive_view};
use cove_core::scene::{Align, FlexDirection, Justify, LayoutNode, Node, Wrap};
use cove_imaging::BundleImageResource;

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

/// What closes a row on its trailing edge.
#[derive(Clone, Default)]
pub enum ListRowAccessory {
    #[default]
    None,
    /// A chevron: the row opens a detail view.
    Disclosure,
    /// A checkmark in the accent: the row is the chosen option.
    Checkmark,
    /// Any trailing view, after the value and the badge.
    Custom(AnyView),
}

/// A macOS list row: an optional tinted icon, a title with an optional
/// subtitle, and trailing value, badge and accessory. Every part inks itself
/// from the band it sits on, so a selected row needs no caller-side colours.
pub struct ListRow {
    pub title: String,
    pub options: ListRowOptions,
    icon: Option<BundleImageResource>,
    icon_tint: Option<SemanticColorRef>,
    value: Option<String>,
    badge: Option<String>,
    accessory: ListRowAccessory,
    is_disabled: bool,
    leading: AnyView,
    trailing: AnyView,
}

impl ListRow {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            options: ListRowOptions::default(),
            icon: None,
            icon_tint: None,
            value: None,
            badge: None,
            accessory: ListRowAccessory::None,
            is_disabled: false,
            leading: AnyView::new(crate::EmptyView),
            trailing: AnyView::new(crate::EmptyView),
        }
    }

    pub fn options(mut self, options: ListRowOptions) -> Self {
        self.options = options;
        self
    }

    /// Secondary line under the title.
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.options.subtitle = Some(subtitle.into());
        self
    }

    /// Leading glyph in the accent (or `tint`), light on an emphasized band.
    pub fn icon(mut self, icon: BundleImageResource) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Tint for the leading glyph when the row is not on the accent band.
    pub fn icon_tint(mut self, tint: SemanticColorRef) -> Self {
        self.icon_tint = Some(tint);
        self
    }

    /// Trailing secondary text, such as a count, a date or a setting's value.
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Trailing count capsule, as a mailbox shows unread messages.
    pub fn badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    pub fn accessory(mut self, accessory: ListRowAccessory) -> Self {
        self.options.shows_disclosure = matches!(accessory, ListRowAccessory::Disclosure);
        self.accessory = accessory;
        self
    }

    /// A destructive action row: the title and icon take the error colour.
    pub fn destructive(mut self) -> Self {
        self.options.role = ButtonRole::Destructive;
        self
    }

    /// Dims the row's content; selection and activation stay with the list.
    pub fn disabled(mut self, is_disabled: bool) -> Self {
        self.is_disabled = is_disabled;
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

fn row_stack(spacing: f32, items: Vec<AnyView>) -> AnyView {
    AnyView::new(BoxView::new(
        FlexDirection::Row,
        Align::Center,
        Justify::FlexStart,
        Wrap::NoWrap,
        spacing,
        AnyView::new(TupleView::new(items)),
    ))
}

impl View for ListRow {
    fn body(&self, _cx: &Scope) -> AnyView {
        let title = self.title.clone();
        let options = self.options.clone();
        let icon = self.icon.clone();
        let icon_tint = self.icon_tint.clone();
        let value = self.value.clone();
        let badge = self.badge.clone();
        let accessory = self.accessory.clone();
        let is_disabled = self.is_disabled;
        let leading = self.leading.clone();
        let trailing = self.trailing.clone();
        AnyView::new(crate::adaptive::SurfaceReader::new(
            move |surface: &cove_core::Environment| {
                let touch = surface.input.contains(cove_core::InputCapabilities::TOUCH);
                let title = title.clone();
                let options = options.clone();
                let icon = icon.clone();
                let icon_tint = icon_tint.clone();
                let value = value.clone();
                let badge = badge.clone();
                let accessory = accessory.clone();
                let leading = leading.clone();
                let trailing = trailing.clone();
                AnyView::new(ListRowContextReader::new(move |context: ListRowContext| {
                    let on_accent = context.is_on_accent();
                    let destructive = options.role == ButtonRole::Destructive;
                    // On the accent band everything takes the band's light ink;
                    // elsewhere the title keeps the row ink, secondary text is
                    // muted and glyphs carry their tint.
                    let mut title_text = Text::new(title.clone()).truncated().clipped();
                    if destructive && !on_accent {
                        title_text = title_text.semantic_foreground(SemanticColorRef::error());
                    }
                    let secondary = |text: String| {
                        let view = Text::new(text).truncating_tail().clipped();
                        if on_accent {
                            AnyView::new(view.opacity(0.8))
                        } else {
                            AnyView::new(
                                view.semantic_foreground(SemanticColorRef::on_surface_muted()),
                            )
                        }
                    };
                    let title_content = match options.subtitle.clone() {
                        Some(subtitle) => AnyView::new(BoxView::new(
                            FlexDirection::Column,
                            Align::Stretch,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            2.0,
                            AnyView::new(TupleView::new(vec![
                                AnyView::new(title_text),
                                AnyView::new(
                                    AnyView::new(secondary(subtitle))
                                        .semantic_font(SemanticFontRef::caption()),
                                ),
                            ])),
                        )),
                        None => AnyView::new(title_text),
                    };

                    let mut leading_items = Vec::new();
                    if let Some(icon) = icon.clone() {
                        let glyph = if on_accent {
                            Icon::semantic(icon, SemanticIconSize::medium())
                        } else {
                            Icon::semantic_with_ref(
                                icon,
                                SemanticIconSize::medium(),
                                icon_tint.clone().unwrap_or(if destructive {
                                    SemanticColorRef::error()
                                } else {
                                    SemanticColorRef::accent()
                                }),
                            )
                        };
                        leading_items.push(AnyView::new(glyph.accessibility_hidden(true)));
                    }
                    leading_items.push(leading.clone());

                    let mut trailing_items = Vec::new();
                    if let Some(value) = value.clone() {
                        trailing_items.push(secondary(value));
                    }
                    if let Some(badge) = badge.clone() {
                        trailing_items.push(AnyView::new(ListRowBadge {
                            text: badge,
                            on_accent,
                        }));
                    }
                    trailing_items.push(AnyView::new(trailing.clone().flex(0.0).shrink(0.0)));
                    match &accessory {
                        ListRowAccessory::None if options.shows_disclosure => {
                            trailing_items.push(disclosure(on_accent));
                        }
                        ListRowAccessory::None => {}
                        ListRowAccessory::Disclosure => trailing_items.push(disclosure(on_accent)),
                        ListRowAccessory::Checkmark => trailing_items.push(AnyView::new(
                            if on_accent {
                                Icon::semantic(
                                    crate::primitives::media::ui_icon_catalog::action::check(),
                                    SemanticIconSize::small(),
                                )
                            } else {
                                Icon::semantic_with_ref(
                                    crate::primitives::media::ui_icon_catalog::action::check(),
                                    SemanticIconSize::small(),
                                    SemanticColorRef::accent(),
                                )
                            }
                            .accessibility_hidden(true),
                        )),
                        ListRowAccessory::Custom(view) => trailing_items.push(view.clone()),
                    }

                    let contents = AnyView::new(
                        crate::LayoutContainer::new(
                            ListRowSlots { touch },
                            TupleView::new(vec![
                                row_stack(8.0, leading_items),
                                title_content,
                                row_stack(8.0, trailing_items),
                            ]),
                        )
                        .accessibility(|semantics| semantics.combines_children = true),
                    );
                    let scroll = ScrollView::new(ScrollAxis::Horizontal, contents)
                        .combine_accessibility(true)
                        .scroll_enabled(!touch);
                    let row = AnyView::new(AnyView::new(scroll).frame(
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
                    ));
                    if is_disabled || !context.is_enabled {
                        AnyView::new(row.opacity(0.4))
                    } else {
                        row
                    }
                }))
            },
        ))
    }
}

fn disclosure(on_accent: bool) -> AnyView {
    let chevron = AnyView::new(AnyView::new(ListDisclosure).modifier(
        crate::modifiers::interaction::AccessibilityModifier::new(|semantics| {
            semantics.is_hidden = true
        }),
    ));
    // A muted chevron reads as navigation without competing with the title;
    // on the accent band it follows the light ink.
    if on_accent {
        chevron
    } else {
        AnyView::new(chevron.semantic_foreground(SemanticColorRef::on_surface_muted()))
    }
}

/// A count capsule: muted fill on a plain row, translucent light on the band.
struct ListRowBadge {
    text: String,
    on_accent: bool,
}

impl View for ListRowBadge {
    fn body(&self, _cx: &Scope) -> AnyView {
        let text = self.text.clone();
        let on_accent = self.on_accent;
        AnyView::new(ThemeReader::new(move |theme: &crate::theme::Theme| {
            let (fill, ink) = if on_accent {
                (
                    theme.colors.on_accent.multiplied_alpha(0.25),
                    theme.colors.on_accent,
                )
            } else {
                (
                    theme.colors.surface_pressed,
                    theme.colors.on_surface_variant,
                )
            };
            AnyView::new(
                Text::new(text.clone())
                    .semantic_font(SemanticFontRef::caption())
                    .foreground_color(ink)
                    .padding_edges(7.0, 1.0)
                    .background_color(fill)
                    .corner_radius(theme.radius.pill),
            )
        }))
    }
}

/// Measure fixed slots before assigning the title its remaining width.
#[derive(Clone, Copy)]
struct ListRowSlots {
    touch: bool,
}

impl ListRowSlots {
    /// Fixed slots keep their intrinsic width; the title takes the remainder
    /// with a breathing gap on either side. Without the gaps an icon or badge
    /// sits flush against the text, which reads as one crowded token.
    fn sizes(
        &self,
        width: Option<f32>,
        children: &[crate::LayoutSubview],
    ) -> (f32, Vec<cove_core::Size>) {
        const SPACING: f32 = 8.0;
        if children.len() != 3 {
            return (0.0, Vec::new());
        }
        let natural: Vec<_> = children
            .iter()
            .map(|child| child.size_that_fits(crate::ProposedViewSize::UNSPECIFIED))
            .collect();
        let leading_gap = if natural[0].width > 0.0 { SPACING } else { 0.0 };
        let trailing_gap = if natural[2].width > 0.0 { SPACING } else { 0.0 };
        let gaps = leading_gap + trailing_gap;
        let natural_width = natural.iter().map(|size| size.width).sum::<f32>() + gaps;
        let width = width.unwrap_or(natural_width);
        let minimum_title = natural[1].width.min(80.0);
        let width = if self.touch {
            width
        } else {
            width.max(natural[0].width + natural[2].width + gaps + minimum_title)
        };

        let fixed_budget = (width - minimum_title.min(width) - gaps).max(0.0);
        let total_fixed = natural[0].width + natural[2].width;
        let (leading, trailing) = if self.touch && total_fixed > fixed_budget {
            let leading_budget = fixed_budget * natural[0].width / total_fixed;
            let trailing_budget = fixed_budget - leading_budget;
            (
                natural[0].width.min(leading_budget),
                natural[2].width.min(trailing_budget),
            )
        } else {
            (natural[0].width.min(width), natural[2].width)
        };
        let title = (width - leading - trailing - gaps).max(0.0);
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
        let trailing_present = sizes.get(2).is_some_and(|size| size.width > 0.0);
        let rtl = children
            .first()
            .is_some_and(|child| child.layout_direction == cove_core::scene::Direction::Rtl);
        let mut cursor = if rtl { bounds.max_x() } else { bounds.x };
        for (index, (child, size)) in children.iter().zip(sizes).enumerate() {
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
            let gap = if (index == 0 && size.width > 0.0) || (index == 1 && trailing_present) {
                8.0
            } else {
                0.0
            };
            cursor += if rtl { -gap } else { gap };
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
        // Inherit the row ink so a selected band's chevron follows `on_accent`
        // instead of staying muted against the fill.
        node.set_foreground_color(None);
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
