//! Translation of Primitives/Controls/Toolbar/Commands.swift.
//!
//! Measure commands at their natural size, then move lower-priority commands into
//! a menu. Icon commands retain their full title as help and semantics.
use crate::Scope;
use crate::primitives::{DuplicateId, check_ids};
use std::rc::Rc;

use crate::modifiers::layout::{LayoutFrameExt, PaddingExt};
use crate::modifiers::style::SemanticStyleViewExt;
use crate::primitives::controls::button::{Button, ButtonRole};
use crate::primitives::layout::scroll_view::{ScrollAxis, ScrollView, ScrollbarGutter};
use crate::primitives::media::ui_icon_catalog;
use crate::primitives::overlays::anchored::{PopoverOverflow, PopoverPlacement, PopoverSurface};
use crate::primitives::overlays::menu::{Menu, MenuEntry};
use crate::primitives::overlays::popover::Popover;
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View, ViewExt};
use cove_core::State;
use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};

use super::{ToolbarAxis, ToolbarItem, ToolbarOverflowLayout};

/// Presentation values for a command row.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToolbarCommandPresentation {
    pub minimum_extent: Option<f32>,
    pub maximum_button_width: Option<f32>,
    pub uses_overflow_icon: bool,
}

/// Measure, inline and overflow one run of commands.
pub struct ToolbarCommands {
    pub(crate) items: Vec<ToolbarItem>,
    pub is_enabled: bool,
    pub available_width: Option<f32>,
    pub presentation: ToolbarCommandPresentation,
    pub on_content_width: Option<Rc<dyn Fn(f32)>>,
    pub axis: ToolbarAxis,
    overflow: State<Vec<u64>>,
    overflow_focus: Rc<crate::primitives::layout::split::SplitFocusTransfer>,
    is_presented: State<bool>,
    overflow_hovered: State<bool>,
}

impl ToolbarCommands {
    pub fn try_new(
        items: Vec<ToolbarItem>,
        is_enabled: bool,
        axis: ToolbarAxis,
    ) -> Result<Self, DuplicateId> {
        check_ids(items.iter().map(|item| &item.command.id), "Toolbar command")?;
        Ok(Self::from_unique_items(items, is_enabled, axis))
    }

    pub(crate) fn from_unique_items(
        items: Vec<ToolbarItem>,
        is_enabled: bool,
        axis: ToolbarAxis,
    ) -> Self {
        Self {
            items,
            is_enabled,
            available_width: None,
            presentation: ToolbarCommandPresentation::default(),
            on_content_width: None,
            axis,
            overflow: State::new(Vec::new()),
            overflow_focus: crate::primitives::layout::split::SplitFocusTransfer::new(),
            is_presented: State::new(false),
            overflow_hovered: State::new(false),
        }
    }

    pub fn available_width(mut self, width: f32) -> Self {
        self.available_width = Some(width);
        self
    }

    pub fn presentation(mut self, presentation: ToolbarCommandPresentation) -> Self {
        self.presentation = presentation;
        self
    }

    pub fn on_content_width(mut self, handler: impl Fn(f32) + 'static) -> Self {
        self.on_content_width = Some(Rc::new(handler));
        self
    }

    fn snapshot(&self) -> Self {
        Self {
            items: self.items.clone(),
            is_enabled: self.is_enabled,
            available_width: self.available_width,
            presentation: self.presentation.clone(),
            on_content_width: self.on_content_width.clone(),
            axis: self.axis,
            overflow: self.overflow.clone(),
            overflow_focus: self.overflow_focus.clone(),
            is_presented: self.is_presented.clone(),
            overflow_hovered: self.overflow_hovered.clone(),
        }
    }

    /// One command as a button, honouring its icon, role and width cap.
    fn command(&self, item: &ToolbarItem) -> AnyView {
        let command = &item.command;
        let role = if command.role == crate::primitives::overlays::menu::MenuItemRole::Destructive {
            ButtonRole::Destructive
        } else {
            ButtonRole::Normal
        };
        let is_enabled = self.is_enabled && command.is_enabled;
        if let Some(icon) = item.icon.clone() {
            let button = Button::icon_with_options(
                icon,
                Some(20.0),
                None,
                role,
                is_enabled,
                command.is_selected,
                false,
                Some(command.title.clone()),
                {
                    let action = command.action.clone();
                    move || action()
                },
            );
            return AnyView::new(AnyView::new(button).with_modifier(ActionControlLabel {
                label: command.title.clone(),
            }));
        }
        if let Some(limit) = self.presentation.maximum_button_width {
            let button = Button::with_options(
                AnyView::new(
                    crate::primitives::text::Text::new(command.title.clone())
                        .truncated()
                        .semantic_font(crate::theme::SemanticFontRef::label()),
                ),
                {
                    let action = command.action.clone();
                    move || action()
                },
                role,
                is_enabled,
                false,
                command.is_selected,
                Some(command.title.clone()),
            );
            return AnyView::new({
                let labelled = AnyView::new(button).with_modifier(ActionControlLabel {
                    label: command.title.clone(),
                });
                AnyView::new(labelled).frame(None, None, None, None, Some(limit), None)
            });
        }
        let button = Button::with_options(
            AnyView::new(crate::primitives::text::Text::new(command.title.clone())),
            {
                let action = command.action.clone();
                move || action()
            },
            role,
            is_enabled,
            false,
            command.is_selected,
            Some(command.title.clone()),
        );
        AnyView::new(AnyView::new(button).with_modifier(ActionControlLabel {
            label: command.title.clone(),
        }))
    }
}

/// Exposes the command's title as the control's accessible label.
struct ActionControlLabel {
    label: String,
}

impl cove_composition::ViewModifier for ActionControlLabel {
    fn apply_node(&self, node: &Node) {
        let mut semantics = node.accessibility().unwrap_or_else(|| {
            cove_core::scene::AccessibilitySemantics::new(
                cove_core::scene::AccessibilityRole::Button,
            )
        });
        semantics.label = self.label.clone();
        node.set_accessibility(Some(semantics));
    }
}

impl View for ToolbarCommands {
    fn body(&self, cx: &Scope) -> AnyView {
        if !self.is_enabled {
            if self.is_presented.get() {
                self.is_presented.set(false);
            }
            *self.overflow_focus.pending_visibility.borrow_mut() = None;
        }
        let mut commands = self.snapshot();
        commands.overflow_focus = cx.remember(|| self.overflow_focus.clone());
        let vertical = self.axis == ToolbarAxis::Vertical;
        let reader = crate::adaptive::ContainerSizeReader::new(move |size: cove_core::Size| {
            let mut commands = commands.snapshot();
            let fits_content = commands.available_width.is_some();
            commands.available_width = Some(commands.available_width.unwrap_or(if vertical {
                size.height
            } else {
                size.width
            }));
            let overflow_focus = commands.overflow_focus.clone();
            let body = ToolbarCommandsBody {
                commands,
                fits_content,
                overflow_focus,
            };
            ScrollView::new(
                if vertical {
                    ScrollAxis::Vertical
                } else {
                    ScrollAxis::Horizontal
                },
                body,
            )
            .scrollbar_gutter(ScrollbarGutter::Overlay)
        });
        // The probe fills only the width by default; a vertical bar measures height.
        if vertical {
            AnyView::new(AnyView::new(reader).frame(
                None,
                Some(crate::modifiers::layout::FrameDimension::Percent(100.0)),
                None,
                Some(0.0),
                None,
                None,
            ))
        } else {
            AnyView::new(reader)
        }
    }

    fn dynamic_properties(&self) -> Vec<crate::DynamicField> {
        vec![
            crate::DynamicField::state("overflow", &self.overflow),
            crate::DynamicField::state("is_presented", &self.is_presented),
            crate::DynamicField::state("overflow_hovered", &self.overflow_hovered),
        ]
    }
}

struct ToolbarCommandsBody {
    commands: ToolbarCommands,
    fits_content: bool,
    /// Shared with `ToolbarRetainedCommand` so a hidden command remembers the
    /// focus visibility it had, and the overflow row can restore it.
    overflow_focus: Rc<crate::primitives::layout::split::SplitFocusTransfer>,
}

impl PrimitiveView for ToolbarCommandsBody {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let commands = &self.commands;
        let vertical = commands.axis == ToolbarAxis::Vertical;
        let minimum = commands.presentation.minimum_extent.unwrap_or(0.0);
        let mut children: Vec<AnyView> = Vec::new();
        let overflow = commands.overflow.get();
        for item in &commands.items {
            let visible = !overflow.contains(&item.command.id);
            children.push(AnyView::new(ToolbarRetainedCommand {
                is_visible: visible,
                is_overflow: false,
                command_id: Some(item.command.id),
                focus: self.overflow_focus.clone(),
                content: AnyView::new(commands.command(item).frame(
                    None,
                    None,
                    Some(minimum),
                    Some(minimum),
                    None,
                    None,
                )),
            }));
        }

        // The overflow button: an icon, or an ellipsis when the row cannot spare
        // the icon's width.
        let is_presented = commands.is_presented.clone();
        let overflow_hovered = commands.overflow_hovered.clone();
        let menu_items: Vec<MenuEntry> = commands
            .items
            .iter()
            .filter(|item| overflow.contains(&item.command.id))
            .map(|item| MenuEntry::Item(item.command.clone()))
            .collect();
        let trigger: AnyView = if commands.presentation.uses_overflow_icon || vertical {
            let presented = is_presented.get();
            let hovered = overflow_hovered.get();
            AnyView::new(OverflowButton {
                vertical,
                is_enabled: commands.is_enabled,
                is_presented: presented,
                is_hovered: hovered,
                minimum_extent: minimum,
                on_hover: {
                    let overflow_hovered = overflow_hovered.clone();
                    Rc::new(move |hovered: bool| {
                        if overflow_hovered.get() != hovered {
                            overflow_hovered.set(hovered);
                        }
                    })
                },
            })
        } else {
            AnyView::new(
                AnyView::new(
                    crate::primitives::text::Text::new("…")
                        .semantic_font(crate::theme::SemanticFontRef::body_strong()),
                )
                .padding_edges(10.0, 6.0),
            )
        };
        let popover = Popover::new(
            trigger,
            AnyView::new(Menu::new(menu_items).on_item_activated({
                let presented = is_presented.clone();
                move || presented.set(false)
            })),
        )
        .is_presented(is_presented.binding())
        .placement(if vertical {
            PopoverPlacement::TrailingBottom
        } else {
            PopoverPlacement::BottomTrailing
        })
        .surface(PopoverSurface::Content)
        .overflow(PopoverOverflow::Flip)
        .enabled(commands.is_enabled);
        let popover = AnyView::new(popover).composition_local(
            crate::primitives::controls::button::ActionControlAccessibility::label(),
            Some("More actions".to_string()),
        );
        children.push(AnyView::new(ToolbarRetainedCommand {
            is_visible: !overflow.is_empty(),
            is_overflow: true,
            command_id: None,
            focus: self.overflow_focus.clone(),
            content: AnyView::new(popover),
        }));

        // The overflow layout measures each command and parks the ones that do not
        // fit, publishing the moved set during placement.
        let overflow_for_layout = commands.overflow.clone();
        let presented_for_layout = commands.is_presented.clone();
        let focus_for_layout = self.overflow_focus.clone();
        let items_for_layout = commands.items.clone();
        let mut layout = ToolbarOverflowLayout::new(
            commands.available_width.unwrap_or(0.0),
            commands.items.iter().map(|item| item.placement).collect(),
            4.0,
        );
        layout.axis = commands.axis;
        layout.fits_content = self.fits_content;
        layout.on_overflow = Some(Rc::new(move |indices: &[usize]| {
            let next: Vec<u64> = indices
                .iter()
                .filter_map(|index| items_for_layout.get(*index).map(|item| item.command.id))
                .collect();
            if next != overflow_for_layout.get() {
                if let Some(chain) = cove_core::scene::FocusChainHolder::current() {
                    let mut ancestor = chain.focused();
                    while let Some(node) = ancestor {
                        if node
                            .attachment::<u64>("toolbar.commandID")
                            .is_some_and(|id| next.contains(&id))
                        {
                            *focus_for_layout.pending_visibility.borrow_mut() =
                                Some(chain.is_focus_visible());
                            break;
                        }
                        ancestor = node.parent();
                    }
                }
                if presented_for_layout.get() && next.is_empty() {
                    // Closing the menu because nothing overflowed hands focus back
                    // to the bar with the visibility it had.
                    *focus_for_layout.pending_visibility.borrow_mut() = Some(
                        cove_core::scene::FocusChainHolder::current()
                            .map(|chain| chain.is_focus_visible())
                            .unwrap_or(true),
                    );
                }
                overflow_for_layout.set(next);
                presented_for_layout.set(false);
            }
        }));
        layout.on_content_extent = commands.on_content_width.clone();
        vec![AnyView::new(crate::LayoutContainer::<
            ToolbarOverflowLayout,
            AnyView,
        >::with_erased_content(
            layout,
            AnyView::new(TupleView::new(children)),
        ))]
    }
}
crate::impl_primitive_view!(ToolbarCommandsBody);

/// A command that keeps its measured size even while parked off-screen.
/// Keep command nodes measured and mounted while their menu representation is
/// active. Hidden commands receive neither input nor accessibility traversal.
struct ToolbarRetainedCommand {
    is_visible: bool,
    /// Set on the overflow representation, which is what receives focus back.
    is_overflow: bool,
    command_id: Option<u64>,
    focus: Rc<crate::primitives::layout::split::SplitFocusTransfer>,
    content: AnyView,
}

impl PrimitiveView for ToolbarRetainedCommand {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(self.is_visible);
        node
    }

    fn update_node(&self, node: &Node) {
        if let Some(id) = self.command_id {
            node.set_attachment("toolbar.commandID", id);
        }
        node.set_is_interaction_enabled(self.is_visible);
        node.set_opacity(if self.is_visible { 1.0 } else { 0.0 });
        node.set_accessibility(Some(cove_core::scene::AccessibilitySemantics {
            role: cove_core::scene::AccessibilityRole::Group,
            is_hidden: !self.is_visible,
            ..Default::default()
        }));

        if !self.is_visible {
            // A hidden command must not keep focus or pointer capture, and the
            // restored focus state is remembered for the next reveal.
            if let Some(chain) = cove_core::scene::FocusChainHolder::current()
                && let Some(focused) = chain.focused()
                && chain.contains(&focused, node)
            {
                *self.focus.pending_visibility.borrow_mut() = Some(chain.is_focus_visible());
                chain.clear();
            }
            if let Some(capture) = cove_core::scene::PointerCaptureHolder::current()
                && let Some(target) = capture.target()
                && cove_core::scene::FocusChainHolder::current()
                    .is_some_and(|chain| chain.contains(&target, node))
            {
                capture.release();
            }
        }

        // Once the overflow row lays out, hand focus to its first tab stop. A
        // hidden row hands it to the first tab stop left in the toolbar.
        if self.is_overflow {
            let focus = self.focus.clone();
            let is_visible = self.is_visible;
            node.set_layout_did_update(Some(Rc::new(move |owner: &Node| {
                let Some(visible) = *focus.pending_visibility.borrow() else {
                    return;
                };
                let mut root = owner.clone();
                if !is_visible {
                    while root.accessibility().map(|semantics| semantics.role)
                        != Some(cove_core::scene::AccessibilityRole::Toolbar)
                    {
                        let Some(parent) = root.parent() else { break };
                        root = parent;
                    }
                }
                let mut pending = vec![root];
                let mut target = None;
                while let Some(candidate) = pending.pop() {
                    if !candidate.accepts_subtree_input() {
                        continue;
                    }
                    if candidate.is_focusable() && candidate.is_tab_stop() {
                        target = Some(candidate);
                        break;
                    }
                    pending.extend(candidate.children().into_iter().rev());
                }
                if let Some(target) = target {
                    if let Some(chain) = cove_core::scene::FocusChainHolder::current() {
                        chain.focus(Some(&target), visible);
                    }
                    crate::primitives::layout::ScrollReveal::reveal(&target, false);
                    *focus.pending_visibility.borrow_mut() = None;
                }
            })));
        }
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
crate::impl_primitive_view!(ToolbarRetainedCommand);

/// The "more actions" affordance.
struct OverflowButton {
    vertical: bool,
    is_enabled: bool,
    is_presented: bool,
    is_hovered: bool,
    minimum_extent: f32,
    on_hover: Rc<dyn Fn(bool)>,
}

impl PrimitiveView for OverflowButton {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_accessibility(Some(cove_core::scene::AccessibilitySemantics {
            role: cove_core::scene::AccessibilityRole::Button,
            label: crate::i18n::tr("More actions"),
            state: cove_core::scene::AccessibilityState {
                is_enabled: self.is_enabled,
                is_expanded: Some(self.is_presented),
                ..Default::default()
            },
            ..Default::default()
        }));
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_min_width(Some(self.minimum_extent));
        layout.set_min_height(Some(self.minimum_extent));
        Some(layout)
    }

    fn update_layout(&self, layout: &cove_core::scene::LayoutNode) {
        layout.set_min_width(Some(self.minimum_extent));
        layout.set_min_height(Some(self.minimum_extent));
    }

    fn update_node(&self, node: &Node) {
        let theme = crate::theme::theme_for_node(node);
        node.set_background_color(Some(if !self.is_enabled {
            cove_core::Color::CLEAR
        } else if self.is_presented {
            theme.colors.state_layer_selected
        } else if self.is_hovered {
            theme.colors.state_layer_hover
        } else {
            cove_core::Color::CLEAR
        }));
        node.set_corner_radius(theme.inputs.radius);
        let Some(registry) = cove_core::scene::InteractionRegistryHolder::current() else {
            return;
        };
        let on_hover = self.on_hover.clone();
        registry.set_hover(
            node,
            Rc::new(move |phase| {
                let hovered = phase == cove_core::scene::HoverPhase::Enter;
                on_hover(hovered);
            }),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let icon = if self.vertical {
            ui_icon_catalog::action::more_vertical()
        } else {
            ui_icon_catalog::action::more_horizontal()
        };
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Center,
                Justify::Center,
                Wrap::NoWrap,
                0.0,
                AnyView::new(crate::primitives::media::Icon::new(icon, 20.0)),
            )
            .frame_size(self.minimum_extent, self.minimum_extent),
        )]
    }
}
crate::impl_primitive_view!(OverflowButton);
