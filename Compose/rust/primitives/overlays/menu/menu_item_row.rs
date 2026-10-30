//! Translation of Primitives/Overlays/Menu/MenuItemRow.swift.
//!
//! One interactive row: highlight band, leading selection and icon lanes, title
//! or rich title, detail line, shortcut or submenu chevron, and the link glyph.
use std::rc::Rc;

use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, AccessibilityState, Align,
    EventResult, FlexDirection, FocusChainHolder, HoverPhase, InputHandlerRoute,
    InteractionRegistryHolder, Justify, LayoutNode, Node, PointerCaptureHolder, PointerPhase, Wrap,
};
use cove_core::Color;
use cove_platform::{MouseButton, SystemCursor};
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::interaction::CursorExt;
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::{ButtonIcon, ButtonIconSource};
use crate::primitives::text::Text;
use crate::theme::{theme_for_node, SemanticFontRef};
use crate::{impl_primitive_view, AnyView, BoxView, PrimitiveView, Row, TupleView, VerticalAlignment, View};

use super::menu::MenuItemRole;
use super::menu_navigation::{MenuRowEvents, MenuRowPresentation, MENU_ITEM_ID};

const HOVERED: &str = "__menu_item_hovered";
const PRESSED: &str = "__menu_item_pressed";
const ACTIVE_PRESS: &str = "__menu_item_active_press";
const HIGHLIGHTED: &str = "__menu_highlighted";
const ENABLED: &str = "__menu_enabled";
const VISUAL_FILL: &str = "__menu_visual_fill";
const REVEAL_TOKEN: &str = "__menu.revealToken";

/// Minimum row height, before touch extents and text scaling.
const MINIMUM_ROW_HEIGHT: f32 = 28.0;

#[derive(Clone, Copy, PartialEq)]
struct PaintIdentity {
    id: u64,
    is_enabled: bool,
    is_selected: bool,
    role: MenuItemRole,
    is_highlighted: bool,
}

/// One menu row.
pub struct MenuItemRowHost {
    pub item: MenuRowPresentation,
    pub is_highlighted: bool,
    pub shows_selection_column: bool,
    pub shows_icon_column: bool,
    pub submenu: Option<AnyView>,
    pub events: MenuRowEvents,
    pub reveal_token: u64,
}

fn update_fill(node: &Node) {
    let enabled = node.attachment::<bool>(ENABLED) == Some(true);
    let pressed = node.attachment::<bool>(PRESSED) == Some(true);
    let hovered = node.attachment::<bool>(HOVERED) == Some(true);
    let highlighted = node.attachment::<bool>(HIGHLIGHTED) == Some(true);
    let colors = theme_for_node(node).colors;
    // The select paints the row the pointer or the arrows are on with the
    // accent and swaps the ink to its paired colour; it snaps rather than
    // easing, so the band follows the highlight without a trailing fade.
    let fill = if !enabled {
        Color::CLEAR
    } else if pressed {
        colors.accent_pressed
    } else if hovered || highlighted {
        colors.accent
    } else {
        Color::CLEAR
    };
    node.set_attachment(VISUAL_FILL, fill);
    node.mark_render_dirty(cove_core::scene::InvalidationSource::style_set("menu.fill"));
}

fn set_interaction(node: &Node, key: &str, value: bool) {
    if node.attachment::<bool>(key) == Some(value) {
        return;
    }
    node.set_attachment(key, value);
    update_fill(node);
    node.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(key));
}

fn anchor_is_visible(node: &Node) -> bool {
    let mut visible = node.absolute_frame();
    let mut ancestor = node.parent();
    while let Some(candidate) = ancestor {
        if candidate.clips_to_bounds() {
            let frame = candidate.absolute_frame();
            let x = visible.x.max(frame.x);
            let y = visible.y.max(frame.y);
            let max_x = (visible.x + visible.width).min(frame.x + frame.width);
            let max_y = (visible.y + visible.height).min(frame.y + frame.height);
            visible = cove_core::Rect::new(x, y, (max_x - x).max(0.0), (max_y - y).max(0.0));
        }
        if visible.width <= 0.0 || visible.height <= 0.0 {
            return false;
        }
        ancestor = candidate.parent();
    }
    true
}

impl PrimitiveView for MenuItemRowHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node.set_is_focusable(true);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_min_height(Some(MINIMUM_ROW_HEIGHT));
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_min_height(Some(MINIMUM_ROW_HEIGHT));
    }

    fn update_node(&self, node: &Node) {
        node.set_is_focusable(self.item.is_enabled);
        node.set_is_tab_stop(self.item.is_enabled && self.is_highlighted);
        let focus_events = self.events.focus.clone();
        node.set_attachment(
            cove_core::scene::TextInputAttachmentKey::FOCUS_CHANGE_HANDLER,
            Rc::new(move |focused: bool| {
                if focused {
                    focus_events();
                }
            }) as Rc<dyn Fn(bool)>,
        );
        let weak = node.downgrade();
        let reveal_events = self.events.reveal.clone();
        let close_submenu = self.events.close_submenu.clone();
        let reveal_token = self.reveal_token;
        let is_highlighted = self.is_highlighted;
        let has_submenu = self.item.has_submenu;
        node.set_layout_did_update(Some(Rc::new(move |owner: &Node| {
            let Some(node) = Node::upgrade(&weak) else { return };
            if is_highlighted && node.attachment::<u64>(REVEAL_TOKEN) != Some(reveal_token) {
                node.set_attachment(REVEAL_TOKEN, reveal_token);
                reveal_events(&node);
            }
            if has_submenu && !anchor_is_visible(&node) {
                close_submenu();
            }
            let _ = owner;
        }) as cove_core::scene::LayoutUpdateHook));

        let role = if self.item.is_link() {
            AccessibilityRole::Link
        } else {
            AccessibilityRole::Button
        };
        node.set_accessibility(Some(AccessibilitySemantics {
            role,
            label: self.item.title.clone(),
            value: self.item.content.detail.clone().unwrap_or_default(),
            help: self
                .item
                .content
                .link
                .clone()
                .or_else(|| self.item.shortcut.clone())
                .unwrap_or_default(),
            state: AccessibilityState {
                is_enabled: self.item.is_enabled,
                is_selected: self.item.is_selected,
                is_expanded: if self.item.has_submenu { Some(self.submenu.is_some()) } else { None },
                ..Default::default()
            },
            combines_children: true,
            ..Default::default()
        }));
        let activate = self.events.activate.clone();
        let activate_node = node.downgrade();
        node.set_accessibility_actions(AccessibilityActions {
            activate: self.item.is_enabled.then(|| {
                Rc::new({
                    let activate = activate.clone();
                    let activate_node = activate_node.clone();
                    move || {
                        if let Some(owner) = Node::upgrade(&activate_node) {
                            activate(&owner);
                        }
                    }
                }) as Rc<dyn Fn()>
            }),
            ..Default::default()
        });

        node.set_attachment(MENU_ITEM_ID, self.item.id);
        if self.item.is_enabled {
            if node.attachment::<bool>(HOVERED).is_none() {
                node.set_attachment(HOVERED, false);
            }
            if node.attachment::<bool>(PRESSED).is_none() {
                node.set_attachment(PRESSED, false);
            }
        } else {
            if let Some(chain) = FocusChainHolder::current()
                && chain.focused().is_some_and(|focused| focused == *node)
            {
                chain.clear();
            }
            node.set_attachment(HOVERED, false);
            node.set_attachment(PRESSED, false);
            node.remove_attachment(ACTIVE_PRESS);
            if PointerCaptureHolder::current()
                .and_then(|capture| capture.target())
                .is_some_and(|target| target == *node)
                && let Some(capture) = PointerCaptureHolder::current()
            {
                capture.release();
            }
        }
        node.set_cursor(Some(if self.item.is_enabled { SystemCursor::Pointer } else { SystemCursor::NotAllowed }));
        node.set_attachment(HIGHLIGHTED, self.is_highlighted);
        node.set_attachment(ENABLED, self.item.is_enabled);
        update_fill(node);

        let identity = PaintIdentity {
            id: self.item.id,
            is_enabled: self.item.is_enabled,
            is_selected: self.item.is_selected,
            role: self.item.role,
            is_highlighted: self.is_highlighted,
        };
        let weak = node.downgrade();
        node.update_draw(identity, move |list, origin| {
            let Some(owner) = Node::upgrade(&weak) else { return };
            let Some(background) = owner.attachment::<Color>(VISUAL_FILL) else { return };
            let frame = owner.frame();
            // The panel insets the rows; the highlight fills the row so the
            // accent band runs to the same gutter on both sides.
            let rect = cove_core::Rect::new(origin.x, origin.y, frame.width, frame.height);
            let radius = theme_for_node(&owner).radius.md;
            let _ = list.add_rounded_rect(rect, radius, background.multiplied_alpha(owner.opacity()));
        });

        let Some(registry) = InteractionRegistryHolder::current() else { return };
        if !self.item.is_enabled {
            registry.remove(node);
            return;
        }

        let hover_events = self.events.hover.clone();
        let hover_node = node.downgrade();
        registry.set_hover(node, Rc::new(move |phase| {
            let Some(owner) = Node::upgrade(&hover_node) else { return };
            match phase {
                HoverPhase::Enter => {
                    set_interaction(&owner, HOVERED, true);
                    hover_events(&owner, true);
                }
                HoverPhase::Leave => {
                    hover_events(&owner, false);
                    owner.remove_attachment(ACTIVE_PRESS);
                    if PointerCaptureHolder::current()
                        .and_then(|capture| capture.target())
                        .is_some_and(|target| target == owner)
                        && let Some(capture) = PointerCaptureHolder::current()
                    {
                        capture.release();
                    }
                    set_interaction(&owner, HOVERED, false);
                    set_interaction(&owner, PRESSED, false);
                }
            }
        }));

        let pointer_events = self.events.activate.clone();
        let pointer_node = node.downgrade();
        registry.set_pointer(node, InputHandlerRoute::control(), Rc::new(move |event, phase, _event_phase| {
            if event.button != MouseButton::Left {
                return EventResult::Ignored;
            }
            let Some(owner) = Node::upgrade(&pointer_node) else { return EventResult::Ignored };
            match phase {
                PointerPhase::Down => {
                    owner.set_attachment(ACTIVE_PRESS, true);
                    if let Some(capture) = PointerCaptureHolder::current() {
                        capture.acquire(&owner, true);
                    }
                    set_interaction(&owner, PRESSED, true);
                    EventResult::Handled
                }
                PointerPhase::Up => {
                    let was_active = owner.attachment::<bool>(ACTIVE_PRESS) == Some(true);
                    owner.remove_attachment(ACTIVE_PRESS);
                    set_interaction(&owner, PRESSED, false);
                    if PointerCaptureHolder::current()
                        .and_then(|capture| capture.target())
                        .is_some_and(|target| target == owner)
                        && let Some(capture) = PointerCaptureHolder::current()
                    {
                        capture.release();
                    }
                    if !was_active {
                        return EventResult::Ignored;
                    }
                    let frame = owner.absolute_frame();
                    if !(event.x >= frame.x
                        && event.x <= frame.x + frame.width
                        && event.y >= frame.y
                        && event.y <= frame.y + frame.height)
                    {
                        return EventResult::Handled;
                    }
                    pointer_events(&owner);
                    EventResult::Handled
                }
            }
        }));

        let key_events = self.events.activate.clone();
        let key_node = node.downgrade();
        registry.set_key(node, InputHandlerRoute::control(), Rc::new(move |event, _phase| {
            match event.scancode {
                crate::Scancode::RETURN | crate::Scancode::SPACE | crate::Scancode::KEYPAD_ENTER => {
                    if !event.is_repeat
                        && let Some(owner) = Node::upgrade(&key_node)
                    {
                        key_events(&owner);
                    }
                    EventResult::Handled
                }
                _ => EventResult::Ignored,
            }
        }));
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let theme = theme_for_node(node);
        let touch = node
            .composition_value(&crate::adaptive::SurfaceEnvironment::key())
            .input
            .contains(cove_core::InputCapabilities::TOUCH);
        let minimum_height = if touch { MINIMUM_ROW_HEIGHT.max(44.0) } else { MINIMUM_ROW_HEIGHT };
        // A highlighted row is an accent band with light ink: the whole row
        // (label, icon, shortcut) inverts together instead of keeping dark text
        // on an accent face.
        let highlighted = self.item.is_enabled && self.is_highlighted;
        let title_color = if !self.item.is_enabled {
            theme.text_emphasis.disabled.unwrap_or(theme.colors.on_surface_muted)
        } else if highlighted {
            theme.colors.on_accent
        } else if self.item.role == MenuItemRole::Destructive {
            theme.colors.error
        } else {
            theme.colors.on_surface
        };
        let secondary_color = if highlighted { theme.colors.on_accent } else { theme.colors.on_surface_muted };

        let mut lanes: Vec<AnyView> = Vec::new();
        if self.shows_selection_column {
            // Fixed-width slot — a growable spacer here pushes every unchecked
            // title toward the center of the menu.
            let check = if self.item.is_selected {
                AnyView::new(ButtonIcon::new(
                    ButtonIconSource::resource(crate::primitives::media::ui_icon_catalog::action::check()),
                    Some(12.0),
                    Some(title_color),
                ))
            } else {
                AnyView::new(crate::EmptyView)
            };
            lanes.push(AnyView::new(
                BoxView::new(
                    FlexDirection::Row, Align::Center, Justify::Center, Wrap::NoWrap, 0.0, check,
                )
                .frame_width(12.0),
            ));
        }
        if self.shows_icon_column {
            let glyph = match self.item.content.icon.clone() {
                Some(icon) => AnyView::new(
                    ButtonIcon::new(ButtonIconSource::resource(icon), Some(12.0), Some(title_color)),
                ),
                None => AnyView::new(crate::EmptyView),
            };
            lanes.push(AnyView::new(
                BoxView::new(
                    FlexDirection::Row, Align::Center, Justify::Center, Wrap::NoWrap, 0.0, glyph,
                )
                .frame_width(12.0)
                .flex(0.0)
                .shrink(0.0),
            ));
        }
        let mut labels: Vec<AnyView> = Vec::new();
        if self.item.content.rich_title.is_empty() {
            labels.push(AnyView::new(
                Text::new(self.item.title.clone())
                    .semantic_font(SemanticFontRef::body())
                    .foreground_color(title_color),
            ));
        } else {
            // Styled spans render as real runs, display-only: the row is the only
            // hit target, so links inside a title must not take focus.
            labels.push(AnyView::new(
                crate::primitives::text::rich::RichText::new(self.item.content.rich_title.clone())
                    .display_only(true),
            ));
        }
        if let Some(detail) = self.item.content.detail.clone() {
            labels.push(AnyView::new(
                Text::new(detail)
                    .line_limit(Some(2))
                    .semantic_font(SemanticFontRef::caption())
                    .foreground_color(secondary_color),
            ));
        }
        lanes.push(AnyView::new(
            BoxView::new(
                FlexDirection::Column, Align::Stretch, Justify::FlexStart, Wrap::NoWrap, 1.0,
                AnyView::new(TupleView::new(labels)),
            )
            .flex_with(1.0, 1.0, None)
            .frame_min_width(0.0),
        ));
        if self.item.has_submenu {
            lanes.push(AnyView::new(ButtonIcon::new(
                ButtonIconSource::resource(crate::primitives::media::ui_icon_catalog::navigation::chevron_right()),
                Some(12.0),
                Some(title_color),
            )));
        } else if let Some(shortcut) = self.item.shortcut.clone() {
            lanes.push(AnyView::new(
                Text::new(shortcut)
                    .line_limit(Some(1))
                    .semantic_font(SemanticFontRef::caption())
                    .foreground_color(secondary_color)
                    .clipped(),
            ));
        }
        if self.item.is_link() {
            lanes.push(AnyView::new(ButtonIcon::new(
                ButtonIconSource::resource(crate::primitives::media::ui_icon_catalog::action::external_link()),
                Some(12.0),
                Some(secondary_color),
            )));
        }

        vec![AnyView::new(
            Row::new(
                VerticalAlignment::Center,
                8.0,
                AnyView::new(TupleView::new(lanes)),
            )
            .padding_edges(10.0, 6.0)
            .frame(None, None, None, Some(minimum_height), None, None),
        )]
    }
}
impl_primitive_view!(MenuItemRowHost);
