//! Translation of Primitives/Layout/Sidebar/Destination.swift.
//!
//! One navigation row: the hit/selection surface spans the whole row, while an
//! optional accessory and an actions menu sit outside the button at the trailing
//! edge so they never shrink a short destination's hover band.
use std::rc::Rc;

use crate::Scope;
use crate::modifiers::layout::{FlexExt, FrameDimension, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::{Button, ButtonIcon, ButtonIconSource, ButtonRole};
use crate::primitives::feedback::Badge;
use crate::primitives::overlays::context_menu::ContextMenu;
use crate::primitives::overlays::{Popover, PopoverTriggerDescription};
use crate::primitives::text::Text;
use crate::theme::styles::buttons::ButtonStyleViewExt;
use crate::theme::tokens::{IconScaling, IconScalingViewExt};
use crate::theme::{SemanticFontRef, theme_for_node};
use crate::{
    AnyView, BoxView, LayoutDebugExt, PrimitiveView, TupleView, View, impl_primitive_view,
};
use cove_core::Binding;
use cove_core::scene::{
    AccessibilityRole, Align, Edge, FlexDirection, Justify, Node, PositionType, Wrap,
};

use super::hierarchy::SidebarEntry;
use super::presentation::SidebarPresentation;
pub use super::sidebar::SidebarItemStyle;

/// Accessibility: `true` while this row is the selected destination.
const SELECTED: &str = "__sidebar.selected";
const ACTIONS_WIDTH: f32 = 44.0;
/// Marker key the button host publishes, used to find the row's real hit node.
const BUTTON_HOST_MARKER: &str = "__button_host";

/// Reveal bookkeeping for one row.
#[derive(Default)]
struct RevealState {
    /// Last `(is_selected, presentation)` the host acted on.
    signature: Option<(bool, u8)>,
    /// A selection change asked for a reveal that has not happened yet.
    pending: bool,
    /// Last geometry the reveal ran against, so an unchanged row does not
    /// re-scroll on every layout pass.
    geometry: Option<(u32, u32, u32, u32, u32, u32)>,
}

/// One navigation destination row.
pub struct SidebarDestination<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static>
{
    pub entry: SidebarEntry<Id>,
    pub selection: Binding<Option<Id>>,
    pub expanded: Binding<std::collections::HashSet<Id>>,
    pub is_selected: bool,
    pub is_expanded: bool,
    pub is_tab_stop: bool,
    pub icon_width: Option<f32>,
    pub minimum_row_height: f32,
    pub on_activate: Rc<dyn Fn(Id)>,
    pub presentation: SidebarPresentation,
    pub icon_extent: f32,
    pub disclosure_lane: bool,
    pub context_label: String,
}

impl<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> SidebarDestination<Id> {
    fn actions_width(&self) -> f32 {
        if self.entry.item.context_menu.is_empty() {
            0.0
        } else {
            ACTIONS_WIDTH
        }
    }

    /// Title inset that keeps text clear of the absolutely placed accessory and
    /// actions.
    fn trailing_reservation(&self) -> f32 {
        if self.presentation.is_rail() {
            0.0
        } else {
            self.actions_width()
                + if self.entry.item.slots.accessory.is_some() {
                    self.entry.item.slots.accessory_width
                } else {
                    0.0
                }
        }
    }
}

impl<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> PrimitiveView
    for SidebarDestination<Id>
{
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_attachment(SELECTED, self.is_selected);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        // Reveal only on a real state change: arrowing or clicking a row must
        // scroll it into view, but an unrelated relayout must not.
        let signature = (self.is_selected, presentation_code(self.presentation));
        let existing = node.attachment::<Rc<std::cell::RefCell<RevealState>>>(REVEAL_STATE);
        let state = match existing {
            Some(state) => state,
            None => {
                let created = Rc::new(std::cell::RefCell::new(RevealState::default()));
                node.set_attachment(REVEAL_STATE, created.clone());
                created
            }
        };
        {
            let mut state = state.borrow_mut();
            if state.signature != Some(signature) {
                state.pending = self.is_selected;
                state.signature = Some(signature);
            }
        }
        let weak = node.downgrade();
        let state_for_layout = state.clone();
        node.set_layout_did_update(Some(Rc::new(move |_owner: &Node| {
            let Some(owner) = Node::upgrade(&weak) else {
                return;
            };
            let mut state = state_for_layout.borrow_mut();
            if !state.signature.is_some_and(|(selected, _)| selected) {
                return;
            }
            let mut pending = owner.children();
            while let Some(candidate) = pending.pop() {
                if candidate.attachment::<bool>(BUTTON_HOST_MARKER) == Some(true) {
                    let Some(scroll) = scroll_ancestor(&candidate) else {
                        return;
                    };
                    let scroll_frame = scroll.frame();
                    if scroll_frame.height <= 0.0 || candidate.frame().height <= 0.0 {
                        return;
                    }
                    let offset = scroll.content_offset();
                    let scroll_origin = scroll.absolute_origin();
                    let frame = candidate.absolute_frame();
                    let geometry = (
                        (frame.x + offset.x - scroll_origin.x).to_bits(),
                        (frame.y + offset.y - scroll_origin.y).to_bits(),
                        frame.width.to_bits(),
                        frame.height.to_bits(),
                        scroll_frame.width.to_bits(),
                        scroll_frame.height.to_bits(),
                    );
                    if state.pending || state.geometry != Some(geometry) {
                        state.pending = false;
                        state.geometry = Some(geometry);
                        crate::primitives::layout::ScrollReveal::reveal(&candidate, true);
                    }
                    return;
                }
                pending.extend(candidate.children().into_iter().rev());
            }
        }) as cove_core::scene::LayoutUpdateHook));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let is_rail = self.presentation.is_rail();
        let surface =
            _node.composition_value(&cove_composition::adaptive::SurfaceEnvironment::key());
        let minimum_height = self.minimum_row_height.max(if is_rail {
            44.0
        } else if surface.input.contains(cove_core::InputCapabilities::TOUCH) {
            surface.minimum_control_extent()
        } else {
            0.0
        });
        let label = if is_rail {
            self.context_label.clone()
        } else {
            self.entry.item.title.clone()
        };
        let value = self.entry.item.badge.clone().unwrap_or_default();
        let is_branch = self.entry.is_branch();
        let item_id = self.entry.item.id.clone();
        let selection = self.selection.clone();
        let expanded = self.expanded.clone();
        let on_activate = self.on_activate.clone();
        let is_expanded = self.is_expanded;
        let is_selected = self.is_selected;
        let is_enabled = self.entry.is_enabled;

        let content = if is_rail {
            // Swift clamps the glyph to `railWidth - 16` so an oversized
            // authored icon cannot widen or overhang the rail.
            let icon_extent = self
                .presentation
                .rail_width()
                .map_or(self.icon_extent, |rail_width| {
                    self.icon_extent.min(rail_width - 16.0)
                });
            AnyView::new(RailDestinationLabel {
                item: self.entry.item.clone(),
                shows_labels: self.presentation.shows_labels(),
                icon_extent,
                is_expanded: is_branch.then_some(is_expanded),
            })
        } else {
            let mut lanes: Vec<AnyView> = Vec::new();
            if self.disclosure_lane {
                let glyph = if is_branch {
                    AnyView::new(ButtonIcon::new(
                        ButtonIconSource::resource(if is_expanded {
                            crate::primitives::media::ui_icon_catalog::action::minus()
                        } else {
                            crate::primitives::media::ui_icon_catalog::action::plus()
                        }),
                        Some(12.0),
                        None,
                    ))
                } else {
                    AnyView::new(crate::EmptyView)
                };
                lanes.push(AnyView::new(
                    BoxView::new(
                        FlexDirection::Row,
                        Align::Center,
                        Justify::Center,
                        Wrap::NoWrap,
                        0.0,
                        glyph,
                    )
                    .frame_width(16.0)
                    .flex_with(0.0, 0.0, None),
                ));
            }
            if let Some(width) = self.icon_width {
                // Source-list glyphs carry the accent, as macOS sidebars do; a
                // disabled destination keeps the row's muted ink.
                let theme = theme_for_node(_node);
                let glyph = match self.entry.item.icon.clone() {
                    Some(icon) if self.entry.is_enabled => AnyView::new(
                        AnyView::new(ButtonIcon::new(
                            ButtonIconSource::resource(icon),
                            Some(self.entry.item.icon_size.resolve(&theme)),
                            None,
                        ))
                        .foreground_color(theme.colors.accent),
                    ),
                    Some(icon) => AnyView::new(ButtonIcon::new(
                        ButtonIconSource::resource(icon),
                        Some(self.entry.item.icon_size.resolve(&theme)),
                        None,
                    )),
                    None => AnyView::new(crate::EmptyView),
                };
                lanes.push(AnyView::new(
                    BoxView::new(
                        FlexDirection::Row,
                        Align::Center,
                        Justify::Center,
                        Wrap::NoWrap,
                        0.0,
                        glyph,
                    )
                    .frame_width(width)
                    .flex_with(0.0, 0.0, None),
                ));
            }
            lanes.push(AnyView::new(
                // The destination title wraps inside its row instead of
                // truncating, so a long name stays readable at 200% text.
                Text::new(self.entry.item.title.clone())
                    .semantic_font(SemanticFontRef::label())
                    .flex_with(1.0, 1.0, None)
                    .frame_min_width(0.0),
            ));
            if let Some(badge) = self.entry.item.badge.clone() {
                lanes.push(AnyView::new(
                    Badge::new(badge).tone(crate::primitives::feedback::StatusTone::Neutral),
                ));
            }
            if let Some(trailing) = self.entry.item.slots.trailing.clone() {
                lanes.push(AnyView::new(trailing.flex_with(0.0, 0.0, None)));
            }
            AnyView::new(
                BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    10.0,
                    AnyView::new(TupleView::new(lanes)),
                )
                .padding_insets(crate::modifiers::layout::EdgeInsets::new(
                    0.0,
                    10.0 + self.entry.ancestors.len().min(5) as f32 * 16.0,
                    0.0,
                    self.trailing_reservation().max(10.0),
                )),
            )
        };

        let button = Button::with_options(
            content,
            move || {
                if is_branch {
                    let mut next = expanded.get();
                    if !next.remove(&item_id) {
                        next.insert(item_id.clone());
                    }
                    expanded.set(next);
                } else {
                    selection.set(Some(item_id.clone()));
                    on_activate(item_id.clone());
                }
            },
            ButtonRole::Normal,
            is_enabled,
            false,
            is_selected,
            // A rail row has no title text, so its tooltip carries the whole
            // breadcrumb and, when present, the badge count.
            is_rail.then(|| match self.entry.item.badge.as_ref() {
                Some(badge) => format!("{} ({badge})", self.context_label),
                None => self.context_label.clone(),
            }),
        )
        .button_style(SidebarItemStyle {
            minimum_height,
            presentation: self.presentation,
        })
        .flex_with(1.0, 1.0, None)
        .frame(None, None, Some(0.0), Some(minimum_height), None, None);

        let button = AnyView::new(button);
        let button = match &self.entry.item.debug_name {
            Some(name) => AnyView::new(button.debug_name(name)),
            None => button,
        };
        let row_label = label.clone();
        let mut column: Vec<AnyView> = vec![AnyView::new(AnyView::new(button).with_modifier(
            SidebarRowIdentity {
                label: row_label,
                value,
                is_expanded: is_branch.then_some(is_expanded),
            },
        ))];
        // The accessory and actions menu sit outside the button so they never
        // shrink the destination's hover band.
        if !is_rail && let Some(accessory) = self.entry.item.slots.accessory.clone() {
            column.push(AnyView::new(SidebarTrailingLane {
                content: accessory,
                width: self.entry.item.slots.accessory_width,
                trailing: self.actions_width(),
            }));
        }
        if !is_rail && !self.entry.item.context_menu.is_empty() {
            column.push(AnyView::new(SidebarActionsTrigger {
                entries: self.entry.item.context_menu.clone(),
                is_enabled: self.entry.is_enabled,
                title: self.entry.item.title.clone(),
                name: format!(
                    "sidebar-actions-{}",
                    crate::primitives::overlays::roving_focus::roving_identity(&self.entry.item.id)
                ),
            }));
        }
        let row = AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(column)),
            )
            .frame(None, None, Some(0.0), None, None, None),
        );
        // A right-click anywhere on the row opens the same menu the ellipsis
        // does, so the actions stay reachable without hunting for the trigger.
        if self.entry.item.context_menu.is_empty() {
            vec![row]
        } else {
            vec![AnyView::new(
                ContextMenu::new(
                    {
                        let entries = self.entry.item.context_menu.clone();
                        move || entries.clone()
                    },
                    row,
                )
                .width(220.0),
            )]
        }
    }
}

/// The trailing ellipsis that opens one destination's actions.
struct SidebarActionsTrigger {
    entries: Vec<crate::primitives::overlays::menu::MenuEntry>,
    is_enabled: bool,
    title: String,
    name: String,
}

impl SidebarActionsTrigger {
    const WIDTH: f32 = ACTIONS_WIDTH;
}

impl View for SidebarActionsTrigger {
    fn body(&self, cx: &Scope) -> AnyView {
        // Presentation belongs to the composition scope, so the menu survives a
        // recompose instead of reopening on every pass.
        let presented = cx.state(|| false);
        let entries = self.entries.clone();
        let title = self.title.clone();
        let is_enabled = self.is_enabled;
        let toggle = presented;
        let dismiss = presented;
        let menu = crate::primitives::overlays::menu::Menu::new(entries)
            .width(220.0)
            .on_dismiss(move || dismiss.set(false));
        let trigger = AnyView::new(
            Button::with_options(
                AnyView::new(ButtonIcon::new(
                    ButtonIconSource::resource(
                        crate::primitives::media::ui_icon_catalog::action::more_horizontal(),
                    ),
                    Some(16.0),
                    None,
                )),
                move || toggle.set(!toggle.get()),
                ButtonRole::Normal,
                is_enabled,
                false,
                false,
                Some(format!("Actions for {title}")),
            )
            .button_style(crate::theme::styles::buttons::GhostButtonStyle),
        );
        let popover = Popover::new(trigger, menu)
            .width(220.0)
            .placement(crate::primitives::overlays::PopoverPlacement::BottomTrailing)
            .enabled(is_enabled)
            .trigger_description(PopoverTriggerDescription {
                label: format!("Actions for {title}"),
                help: Some(crate::i18n::tr("Open item actions")),
                ..Default::default()
            })
            .is_presented(Binding::new(
                move || presented.get(),
                move |value: bool| presented.set(value),
            ));
        AnyView::new(SidebarActionsLane {
            content: AnyView::new(popover.debug_name(&self.name)),
        })
    }
}

/// Absolutely placed trailing lane that hosts the actions trigger.
struct SidebarActionsLane {
    content: AnyView,
}

impl PrimitiveView for SidebarActionsLane {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        Some(Self::layout())
    }

    fn update_layout(&self, layout: &cove_core::scene::LayoutNode) {
        Self::apply(layout);
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}

impl SidebarActionsLane {
    fn layout() -> cove_core::scene::LayoutNode {
        let layout = cove_core::scene::LayoutNode::new();
        Self::apply(&layout);
        layout
    }

    fn apply(layout: &cove_core::scene::LayoutNode) {
        layout.set_position_type(PositionType::Absolute);
        layout.set_position(0.0, Edge::Top);
        layout.set_position(0.0, Edge::End);
        layout.set_position(0.0, Edge::Bottom);
        layout.set_width(Some(SidebarActionsTrigger::WIDTH));
    }
}
impl_primitive_view!(SidebarActionsLane);

const REVEAL_STATE: &str = "__sidebar.revealState";

fn presentation_code(presentation: SidebarPresentation) -> u8 {
    match presentation {
        SidebarPresentation::Expanded => 0,
        SidebarPresentation::Rail { shows_labels: true } => 1,
        SidebarPresentation::Rail {
            shows_labels: false,
        } => 2,
        SidebarPresentation::List { grouped: true } => 3,
        SidebarPresentation::List { grouped: false } => 4,
    }
}

/// Nearest ancestor that owns a reveal hook.
fn scroll_ancestor(node: &Node) -> Option<Node> {
    let mut ancestor = node.parent();
    while let Some(current) = ancestor {
        if current.scroll_reveal().is_some() {
            return Some(current);
        }
        ancestor = current.parent();
    }
    None
}

impl<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> View
    for SidebarDestination<Id>
{
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
}

/// Row semantics applied to the erased button.
struct SidebarRowIdentity {
    label: String,
    value: String,
    is_expanded: Option<bool>,
}

impl cove_composition::ViewModifier for SidebarRowIdentity {
    fn apply_node(&self, node: &Node) {
        let mut semantics = node.accessibility().unwrap_or_else(|| {
            cove_core::scene::AccessibilitySemantics::new(AccessibilityRole::Button)
        });
        semantics.label = self.label.clone();
        semantics.value = self.value.clone();
        semantics.state.is_expanded = self.is_expanded;
        node.set_accessibility(Some(semantics));
    }
}

/// Absolutely placed trailing accessory, inset from the actions menu.
struct SidebarTrailingLane {
    content: AnyView,
    width: f32,
    trailing: f32,
}

impl PrimitiveView for SidebarTrailingLane {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_position_type(PositionType::Absolute);
        layout.set_position(0.0, Edge::Top);
        layout.set_position(self.trailing, Edge::End);
        layout.set_position(0.0, Edge::Bottom);
        layout.set_width(Some(self.width));
        Some(layout)
    }

    fn update_layout(&self, layout: &cove_core::scene::LayoutNode) {
        layout.set_position_type(PositionType::Absolute);
        layout.set_position(0.0, Edge::Top);
        layout.set_position(self.trailing, Edge::End);
        layout.set_position(0.0, Edge::Bottom);
        layout.set_width(Some(self.width));
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::Center,
            Wrap::NoWrap,
            0.0,
            self.content.clone(),
        ))]
    }
}
impl_primitive_view!(SidebarTrailingLane);

/// Rail presentation of one destination: glyph, optional badge dot, optional
/// expand marker and optional label.
pub struct RailDestinationLabel<
    Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static,
> {
    pub item: super::items::SidebarItem<Id>,
    pub shows_labels: bool,
    pub icon_extent: f32,
    pub is_expanded: Option<bool>,
}

impl<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> PrimitiveView
    for RailDestinationLabel<Id>
{
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let theme = theme_for_node(node);
        let tint = if self.item.is_enabled {
            theme.colors.on_surface_variant
        } else {
            theme.colors.on_surface_muted
        };
        let glyph = match self.item.icon.clone() {
            Some(icon) => AnyView::new(
                // `.iconScaling(.fixed)`: the authored extent has already been
                // resolved against the icon roles and clamped to the rail, so a
                // second application of text scaling would grow it again.
                ButtonIcon::new(
                    ButtonIconSource::resource(icon),
                    Some(self.icon_extent),
                    Some(tint),
                )
                .icon_scaling(IconScaling::Fixed),
            ),
            None => AnyView::new(
                ButtonIcon::new(
                    ButtonIconSource::resource(
                        crate::primitives::media::ui_icon_catalog::file::file(),
                    ),
                    Some(self.icon_extent),
                    Some(tint),
                )
                .icon_scaling(IconScaling::Fixed),
            ),
        };
        let mut column: Vec<AnyView> = vec![AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Center,
                Justify::Center,
                Wrap::NoWrap,
                0.0,
                glyph,
            )
            .frame_size(self.icon_extent, self.icon_extent)
            .flex_with(0.0, 0.0, None),
        )];
        // A branch keeps its disclosure marker in the rail, where there is no
        // gutter lane for it.
        if let Some(is_expanded) = self.is_expanded {
            column.push(AnyView::new(
                BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::Center,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(ButtonIcon::new(
                        ButtonIconSource::resource(if is_expanded {
                            crate::primitives::media::ui_icon_catalog::action::minus()
                        } else {
                            crate::primitives::media::ui_icon_catalog::action::plus()
                        }),
                        Some(10.0),
                        Some(tint),
                    )),
                )
                .flex_with(0.0, 0.0, None),
            ));
        }
        if self.shows_labels {
            column.push(AnyView::new(
                // The rail caption wraps onto as many lines as the rail needs;
                // it is not truncated the way an expanded row's title is.
                AnyView::new(
                    Text::new(self.item.title.clone())
                        .alignment(cove_core::TextAlignment::Center)
                        .semantic_font(SemanticFontRef::caption())
                        .semantic_foreground(crate::theme::SemanticColorRef::on_surface_variant()),
                )
                .frame(
                    Some(FrameDimension::Percent(100.0)),
                    None,
                    None,
                    None,
                    None,
                    None,
                ),
            ));
        }
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                4.0,
                AnyView::new(TupleView::new(column)),
            )
            .frame(
                Some(FrameDimension::Percent(100.0)),
                None,
                Some(0.0),
                None,
                None,
                None,
            ),
        )]
    }
}
impl<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> View
    for RailDestinationLabel<Id>
{
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
}
