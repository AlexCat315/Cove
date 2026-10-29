//! Translation of `Primitives/Data/Avatar/Group.swift`.
//!
//! Shared sizing, stable optional IDs, first-person-on-top overlap and a
//! count/ellipsis overflow slot. A zero limit can display only the overflow.
use cove_core::scene::{
    Align, Edge, LayoutNode, Node, Point, PositionType, Rect,
};
use crate::theme::tokens::ControlSize;
use crate::theme::theme_environment::theme_for_node;
use crate::{impl_primitive_view, AnyView, PrimitiveView};

use super::avatar::{Avatar, AvatarAppearance, AvatarColors};
use super::member_list::{AvatarMemberList, AvatarMemberListTrigger};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AvatarOverflow {
    Count,
    Ellipsis,
    Hidden,
}

#[derive(Clone, Debug)]
pub struct AvatarGroupOptions {
    pub limit: usize,
    pub overflow: AvatarOverflow,
    pub overlap: f32,
    pub size: Option<ControlSize>,
    pub diameter: Option<f32>,
    pub label: String,
    /// Makes the overflow marker open a member list; `None` keeps it static.
    pub member_list: Option<AvatarMemberList>,
}
impl Default for AvatarGroupOptions {
    fn default() -> Self {
        Self {
            limit: 3,
            overflow: AvatarOverflow::Count,
            overlap: 0.3,
            size: None,
            diameter: None,
            label: "Members".to_string(),
            member_list: None,
        }
    }
}
impl AvatarGroupOptions {
    pub fn validate(&mut self) {
        self.overlap = if self.overlap.is_finite() {
            self.overlap.clamp(0.0, 0.7)
        } else {
            0.3
        };
        if let Some(diameter) = self.diameter {
            self.diameter = if diameter.is_finite() {
                Some(diameter.clamp(12.0, 512.0))
            } else {
                None
            };
        }
    }
}

pub struct AvatarGroup {
    pub avatars: Vec<Avatar>,
    pub options: AvatarGroupOptions,
}
impl AvatarGroup {
    pub fn new(avatars: Vec<Avatar>) -> Self {
        Self { avatars, options: AvatarGroupOptions::default() }
    }
    pub fn configure(mut self, configure: impl FnOnce(&mut AvatarGroupOptions)) -> Self {
        configure(&mut self.options);
        self.options.validate();
        self
    }

    pub fn geometry(&self, node: &Node) -> AvatarGroupGeometry {
        let mut options = self.options.clone();
        options.validate();
        let mut appearance = AvatarAppearance::default();
        appearance.size = options.size;
        appearance.diameter = options.diameter;
        AvatarGroupGeometry::new(
            self.avatars.len(),
            options.limit,
            appearance.resolved_diameter(node),
            options.overlap,
            options.overflow,
        )
    }

    fn resolved_colors(&self, node: &Node) -> AvatarColors {
        let theme = theme_for_node(node);
        AvatarColors::new(
            theme.colors.surface_variant,
            theme.colors.on_surface_variant,
            theme.colors.border,
        )
    }
}

impl PrimitiveView for AvatarGroup {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }
    fn update_node(&self, node: &Node) {
        let geometry = self.geometry(node);
        if let Some(layout) = node.layout_node() {
            layout.set_width(Some(geometry.width));
            layout.set_height(Some(if geometry.item_count() == 0 {
                0.0
            } else {
                geometry.diameter
            }));
        }
        let mut semantics = cove_core::scene::AccessibilitySemantics::new(
            cove_core::scene::AccessibilityRole::Group,
        );
        semantics.label = self.options.label.clone();
        semantics.value = format!("{} members", self.avatars.len());
        node.set_accessibility(Some(semantics));
        node.set_attachment("avatar.group.count", self.avatars.len());
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let geometry = self.geometry(node);
        let visible = geometry.visible_count.min(self.avatars.len());
        let mut children: Vec<AnyView> = (0..visible)
            .map(|index| {
                let mut item = Avatar {
                    name: self.avatars[index].name.clone(),
                    id: self.avatars[index].id.clone(),
                    image_url: self.avatars[index].image_url.clone(),
                    appearance: self.avatars[index].appearance.clone(),
                    image: self.avatars[index].image.clone(),
                    fallback: self.avatars[index].fallback.clone(),
                };
                item.appearance.diameter = Some(geometry.diameter);
                let slot = AvatarGroupSlot {
                    x: index as f32 * geometry.step,
                    diameter: geometry.diameter,
                    z_index: (visible - index) as f32,
                    content: AnyView::new(item),
                };
                let key = self.avatars[index]
                    .id
                    .clone()
                    .unwrap_or_else(|| format!("slot-{index}"));
                AnyView::new(crate::IdentifiedView::new(slot, key))
            })
            .collect();

        if geometry.has_overflow {
            let remaining = self.avatars.len().saturating_sub(visible);
            let text = if self.options.overflow == AvatarOverflow::Ellipsis {
                "\u{22ef}".to_string()
            } else {
                format!("+{remaining}")
            };
            let names = self
                .avatars
                .iter()
                .skip(visible)
                .take(20)
                .map(|avatar| avatar.name.clone())
                .filter(|name| !name.is_empty())
                .collect::<Vec<_>>()
                .join(", ");
            let colors = self.resolved_colors(node);
            let marker = AvatarOverflowMarker {
                text,
                diameter: geometry.diameter,
                count: remaining,
                names,
                colors,
                is_interactive: self.options.member_list.is_some(),
            };
            let content: AnyView = if let Some(list) = &self.options.member_list {
                let members = if list.scope == super::member_list::AvatarMemberListScope::All {
                    self.avatars.clone()
                } else {
                    self.avatars.iter().skip(visible).cloned().collect()
                };
                AnyView::new(AvatarMemberListTrigger {
                    marker,
                    members,
                    list: list.clone(),
                })
            } else {
                AnyView::new(marker)
            };
            children.push(AnyView::new(crate::IdentifiedView::new(
                AvatarGroupSlot {
                    x: geometry.overflow_x(),
                    diameter: geometry.diameter,
                    z_index: 0.0,
                    content,
                },
                "avatar.overflow",
            )));
        }
        children
    }
}
impl_primitive_view!(AvatarGroup);

/// Layout math shared by measurement, painting and hit testing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AvatarGroupGeometry {
    pub visible_count: usize,
    pub has_overflow: bool,
    pub diameter: f32,
    pub step: f32,
}
impl AvatarGroupGeometry {
    pub fn new(
        total: usize,
        limit: usize,
        diameter: f32,
        overlap: f32,
        overflow: AvatarOverflow,
    ) -> Self {
        let visible_count = total.min(limit);
        let has_overflow = total > visible_count && overflow != AvatarOverflow::Hidden;
        Self {
            visible_count,
            has_overflow,
            diameter,
            step: diameter * (1.0 - overlap),
        }
    }
    pub fn item_count(&self) -> usize {
        self.visible_count + usize::from(self.has_overflow)
    }
    pub fn visible_width(&self) -> f32 {
        if self.visible_count == 0 {
            0.0
        } else {
            self.diameter + (self.visible_count - 1) as f32 * self.step
        }
    }
    pub fn overflow_x(&self) -> f32 {
        if self.visible_count == 0 {
            0.0
        } else {
            self.visible_width() + 4.0
        }
    }
    pub fn width(&self) -> f32 {
        if self.has_overflow {
            self.overflow_x() + self.diameter
        } else {
            self.visible_width()
        }
    }
}

/// Absolutely positioned member that paints a surface-colored separation ring.
pub struct AvatarGroupSlot {
    pub x: f32,
    pub diameter: f32,
    pub z_index: f32,
    pub content: AnyView,
}
impl PrimitiveView for AvatarGroupSlot {
    fn make_node(&self) -> Node {
        Node::new()
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_position_type(PositionType::Absolute);
        layout.set_position(self.x, Edge::Left);
        layout.set_position(0.0, Edge::Top);
        layout.set_width(Some(self.diameter));
        layout.set_height(Some(self.diameter));
        Some(layout)
    }
    fn update_node(&self, node: &Node) {
        node.set_z_index(self.z_index);
        let ring = theme_for_node(node).colors.surface;
        let diameter = self.diameter;
        node.update_overlay_draw(ring.rgba8(), move |list, origin: Point| {
            let _ = list.add_rounded_rect_stroke(
                Rect::new(origin.x - 1.0, origin.y - 1.0, diameter + 2.0, diameter + 2.0),
                diameter / 2.0 + 1.0,
                2.0,
                ring,
            );
        });
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(AvatarGroupSlot);

/// Count/ellipsis marker used in the overflow slot.
#[derive(Clone)]
pub struct AvatarOverflowMarker {
    pub text: String,
    pub diameter: f32,
    pub count: usize,
    pub names: String,
    pub colors: AvatarColors,
    /// Inside a trigger the button owns the semantics and tooltip.
    pub is_interactive: bool,
}
impl AvatarOverflowMarker {
    pub fn accessibility_label(&self) -> String {
        format!("{} more members", self.count)
    }
}
impl PrimitiveView for AvatarOverflowMarker {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }
    fn update_node(&self, node: &Node) {
        node.set_background_color(Some(self.colors.background));
        node.set_corner_radius(self.diameter / 2.0);
        node.set_border_color(Some(self.colors.border));
        node.set_border_width(1.0);
        node.set_attachment("avatar.overflow.text", self.text.clone());
        node.set_attachment("avatar.overflow.count", self.count);
        node.set_attachment("avatar.overflow.foreground", self.colors.foreground);
        if !self.is_interactive {
            let mut semantics = cove_core::scene::AccessibilitySemantics::new(
                cove_core::scene::AccessibilityRole::Image,
            );
            semantics.label = self.accessibility_label();
            semantics.help = self.names.clone();
            semantics.combines_children = true;
            node.set_accessibility(Some(semantics));
        } else {
            let mut semantics = cove_core::scene::AccessibilitySemantics::new(
                cove_core::scene::AccessibilityRole::Button,
            );
            semantics.label = self.accessibility_label();
            semantics.help = self.names.clone();
            node.set_accessibility(Some(semantics));
        }
        if let Some(layout) = node.layout_node() {
            layout.set_width(Some(self.diameter));
            layout.set_height(Some(self.diameter));
            layout.set_align_items(Align::Center);
            layout.set_justify_content(cove_core::scene::Justify::Center);
        }
    }
}
impl_primitive_view!(AvatarOverflowMarker);
