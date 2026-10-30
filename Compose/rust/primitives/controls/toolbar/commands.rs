//! Translation of Primitives/Controls/Toolbar/Commands.swift.
//!
//! Measure commands at their natural size, then move lower-priority commands into
//! a menu. Icon commands retain their full title as help and semantics.
use std::cell::RefCell;
use std::rc::Rc;

use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::SemanticStyleViewExt;
use crate::primitives::controls::button::{Button, ButtonRole};
use crate::primitives::overlays::menu::{Menu, MenuEntry};
use crate::primitives::overlays::popover::Popover;
use crate::primitives::overlays::anchored::{PopoverOverflow, PopoverPlacement, PopoverSurface};
use crate::primitives::media::ui_icon_catalog;
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View};

use super::{ToolbarAxis, ToolbarItem, ToolbarItemPlacement, ToolbarOverflowLayout};

/// Presentation values for a command row.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToolbarCommandPresentation {
    pub minimum_extent: Option<f32>,
    pub maximum_button_width: Option<f32>,
    pub uses_overflow_icon: bool,
}

/// Measure, inline and overflow one run of commands.
pub struct ToolbarCommands {
    pub items: Vec<ToolbarItem>,
    pub is_enabled: bool,
    pub available_width: Option<f32>,
    pub presentation: ToolbarCommandPresentation,
    pub on_content_width: Option<Rc<dyn Fn(f32)>>,
    pub axis: ToolbarAxis,
    overflow: Rc<RefCell<Vec<u64>>>,
    overflow_focus: Rc<crate::primitives::layout::split::SplitFocusTransfer>,
    is_presented: Rc<RefCell<bool>>,
    overflow_hovered: Rc<RefCell<bool>>,
}

impl ToolbarCommands {
    pub fn new(items: Vec<ToolbarItem>, is_enabled: bool, axis: ToolbarAxis) -> Self {
        assert!(
            items.iter().enumerate().all(|(index, item)| items
                .iter()
                .enumerate()
                .all(|(other, candidate)| other == index || candidate.command.id != item.command.id)),
            "Toolbar command IDs must be unique"
        );
        Self {
            items,
            is_enabled,
            available_width: None,
            presentation: ToolbarCommandPresentation::default(),
            on_content_width: None,
            axis,
            overflow: Rc::new(RefCell::new(Vec::new())),
            overflow_focus: crate::primitives::layout::split::SplitFocusTransfer::new(),
            is_presented: Rc::new(RefCell::new(false)),
            overflow_hovered: Rc::new(RefCell::new(false)),
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
            return AnyView::new(
                AnyView::new(button).with_modifier(ActionControlLabel { label: command.title.clone() }),
            );
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
            return AnyView::new(
                {
                    let labelled = AnyView::new(button)
                        .with_modifier(ActionControlLabel { label: command.title.clone() });
                    AnyView::new(labelled).frame(None, None, None, None, Some(limit), None)
                },
            );
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
        AnyView::new(AnyView::new(button).with_modifier(ActionControlLabel { label: command.title.clone() }))
    }
}

/// Exposes the command's title as the control's accessible label.
struct ActionControlLabel {
    label: String,
}

impl cove_composition::ViewModifier for ActionControlLabel {
    fn apply_node(&self, node: &Node) {
        let mut semantics = node
            .accessibility()
            .unwrap_or_else(|| cove_core::scene::AccessibilitySemantics::new(
                cove_core::scene::AccessibilityRole::Button,
            ));
        semantics.label = self.label.clone();
        node.set_accessibility(Some(semantics));
    }
}

impl View for ToolbarCommands {
    fn body(&self) -> AnyView {
        AnyView::new(ToolbarCommandsBody {
            commands: self.snapshot(),
            overflow_focus: self.overflow_focus.clone(),
        })
    }
}

struct ToolbarCommandsBody {
    commands: ToolbarCommands,
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
        let overflow = commands.overflow.borrow().clone();
        for item in &commands.items {
            let visible = !overflow.contains(&item.command.id);
            children.push(AnyView::new(ToolbarRetainedCommand {
                is_visible: visible,
                is_overflow: false,
                focus: self.overflow_focus.clone(),
                content: AnyView::new(
                    commands
                        .command(item)
                        .frame(None, None, Some(minimum), Some(minimum), None, None),
                ),
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
            let presented = *is_presented.borrow();
            let hovered = *overflow_hovered.borrow();
            AnyView::new(OverflowButton {
                vertical,
                is_enabled: commands.is_enabled,
                is_presented: presented,
                is_hovered: hovered,
                minimum_extent: minimum,
                on_hover: {
                    let overflow_hovered = overflow_hovered.clone();
                    Rc::new(move |hovered: bool| {
                        *overflow_hovered.borrow_mut() = hovered;
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
                move || *presented.borrow_mut() = false
            })),
        )
        .placement(if vertical {
            PopoverPlacement::TrailingBottom
        } else {
            PopoverPlacement::BottomTrailing
        })
        .surface(PopoverSurface::Content)
        .overflow(PopoverOverflow::Flip)
        .enabled(commands.is_enabled);
        children.push(AnyView::new(ToolbarRetainedCommand {
            is_visible: !overflow.is_empty(),
            is_overflow: true,
            focus: self.overflow_focus.clone(),
            content: AnyView::new(popover),
        }));

        // The overflow layout measures each command and parks the ones that do not
        // fit, publishing the moved set during placement.
        let overflow_for_layout = commands.overflow.clone();
        let presented_for_layout = commands.is_presented.clone();
        let items_for_layout = commands.items.clone();
        let content_extent = commands.on_content_width.clone();
        let mut layout = ToolbarOverflowLayout::new(
            commands.available_width.unwrap_or(0.0),
            commands.items.iter().map(|item| item.placement).collect(),
            4.0,
        );
        layout.axis = commands.axis;
        layout.fits_content = commands.available_width.is_some();
        layout.on_overflow = Some(Rc::new(move |indices: &[usize]| {
            let next: Vec<u64> = indices
                .iter()
                .filter_map(|index| items_for_layout.get(*index).map(|item| item.command.id))
                .collect();
            if next != *overflow_for_layout.borrow() {
                if *presented_for_layout.borrow() && next.is_empty() {
                    // Closing the menu because nothing overflowed returns focus to
                    // the bar only when focus was visible.
                    *presented_for_layout.borrow_mut() = false;
                }
                *overflow_for_layout.borrow_mut() = next;
                *presented_for_layout.borrow_mut() = false;
            }
        }));
        layout.on_content_extent = content_extent;
        if commands.available_width.is_some() {
            // A fixed available width means the row hugs its measured content.
            vec![AnyView::new(crate::LayoutContainer::<ToolbarOverflowLayout, AnyView>::with_erased_content(
                layout,
                AnyView::new(TupleView::new(children)),
            ))]
        } else {
            // No allocation yet: report the row so the host can decide.
            vec![AnyView::new(ToolbarCommandProbe {
                content: AnyView::new(TupleView::new(children)),
                axis: commands.axis,
            })]
        }
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

        // Once the overflow row lays out, hand focus to its first tab stop.
        if self.is_overflow {
            let focus = self.focus.clone();
            let is_visible = self.is_visible;
            node.set_layout_did_update(Some(Rc::new(move |owner: &Node| {
                let Some(visible) = *focus.pending_visibility.borrow() else { return };
                let mut pending = vec![owner.clone()];
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
                let _ = is_visible;
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
            label: "More actions".to_string(),
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
        let Some(registry) = cove_core::scene::InteractionRegistryHolder::current() else { return };
        let on_hover = self.on_hover.clone();
        registry.set_hover(node, Rc::new(move |phase| {
            let hovered = phase == cove_core::scene::HoverPhase::Enter;
            on_hover(hovered);
        }));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let icon = if self.vertical {
            ui_icon_catalog::action::more_vertical()
        } else {
            ui_icon_catalog::action::more_horizontal()
        };
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Column, Align::Center, Justify::Center, Wrap::NoWrap, 0.0,
                AnyView::new(crate::primitives::media::Icon::new(icon, 20.0)),
            )
            .frame_size(self.minimum_extent, self.minimum_extent),
        )]
    }
}
crate::impl_primitive_view!(OverflowButton);

/// Measures the command row before an available width is known, so the host can
/// feed the result back in.
struct ToolbarCommandProbe {
    content: AnyView,
    axis: ToolbarAxis,
}

impl PrimitiveView for ToolbarCommandProbe {
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
        // A vertical bar measures height, so it must fill the cross axis.
        let content = if self.axis == ToolbarAxis::Vertical {
            AnyView::new(self.content.clone().frame(None, None, None, Some(0.0), None, None))
        } else {
            self.content.clone()
        };
        vec![content]
    }
}
crate::impl_primitive_view!(ToolbarCommandProbe);
