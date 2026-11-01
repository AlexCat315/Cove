//! Translation of `Primitives/Data/Avatar/MemberList.swift`.
//!
//! Popover presentation is not available in Rust yet (`Popover` has no
//! counterpart). The roster content and its row primitives are complete so the
//! trigger can be hosted by whichever overlay primitive lands; `AvatarMemberListTrigger`
//! exposes `content()` for that host.
use std::cell::Cell;
use std::rc::Rc;

use crate::{AnyView, PrimitiveView, View, impl_primitive_view};
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, Align, FlexDirection, LayoutNode, Node,
};

use super::avatar::Avatar;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AvatarMemberListScope {
    /// Every member, so the list is the full roster.
    All,
    /// Only the members hidden behind the overflow marker.
    Remaining,
}

#[derive(Clone)]
pub struct AvatarMemberList {
    pub title: String,
    pub scope: AvatarMemberListScope,
    pub on_select: Option<Rc<dyn Fn(&Avatar)>>,
}
impl std::fmt::Debug for AvatarMemberList {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AvatarMemberList")
            .field("title", &self.title)
            .field("scope", &self.scope)
            .field("on_select", &self.on_select.is_some())
            .finish()
    }
}
impl Default for AvatarMemberList {
    fn default() -> Self {
        Self {
            title: "Members".to_string(),
            scope: AvatarMemberListScope::All,
            on_select: None,
        }
    }
}

/// Overflow marker that owns the popover's session-only presentation state.
pub struct AvatarMemberListTrigger {
    pub(crate) marker: super::group::AvatarOverflowMarker,
    pub(crate) members: Vec<Avatar>,
    pub(crate) list: AvatarMemberList,
    presented: Rc<Cell<bool>>,
}
impl AvatarMemberListTrigger {
    pub fn new(
        marker: super::group::AvatarOverflowMarker,
        members: Vec<Avatar>,
        list: AvatarMemberList,
    ) -> Self {
        Self {
            marker,
            members,
            list,
            presented: Rc::new(Cell::new(false)),
        }
    }
    pub fn is_presented(&self) -> bool {
        self.presented.get()
    }
    pub fn dismiss(&self) {
        self.presented.set(false);
    }
    /// Body of the popover. The host owns sizing (260 pt), bottom-leading
    /// placement and corner radius (`marker.diameter / 2`).
    pub fn content(&self) -> AvatarMemberListContent {
        let on_select = self.list.on_select.clone();
        let presented = self.presented.clone();
        AvatarMemberListContent {
            title: self.list.title.clone(),
            members: self.members.clone(),
            on_select: on_select.map(|select| {
                let presented = presented.clone();
                Rc::new(move |member: &Avatar| {
                    presented.set(false);
                    select(member);
                }) as Rc<dyn Fn(&Avatar)>
            }),
        }
    }
}
impl View for AvatarMemberListTrigger {
    fn body(&self) -> AnyView {
        // Popover is unresolved; the marker remains the visual/accessible
        // trigger and content() is the complete popover body.
        AnyView::new(self.marker.clone())
    }
}

pub struct AvatarMemberListContent {
    pub title: String,
    pub members: Vec<Avatar>,
    pub on_select: Option<Rc<dyn Fn(&Avatar)>>,
}
impl PrimitiveView for AvatarMemberListContent {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::FlexStart);
        layout.set_gap(2.0, cove_core::scene::Gutter::Row);
        layout.set_padding(4.0, cove_core::scene::Edge::All);
        Some(layout)
    }
    fn update_node(&self, node: &Node) {
        let mut semantics = AccessibilitySemantics::new(AccessibilityRole::List);
        semantics.label = self.title.clone();
        semantics.value = format!("{} members", self.members.len());
        node.set_accessibility(Some(semantics));
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let mut children: Vec<AnyView> = Vec::with_capacity(self.members.len() + 1);
        children.push(AnyView::new(AvatarMemberListHeading(self.title.clone())));
        for (index, member) in self.members.iter().enumerate() {
            let row = AvatarMemberRow {
                member: member.clone(),
                on_select: self.on_select.clone(),
            };
            let key = member
                .id
                .clone()
                .unwrap_or_else(|| format!("member-{index}"));
            children.push(AnyView::new(crate::IdentifiedView::new(row, key)));
        }
        children
    }
}
impl_primitive_view!(AvatarMemberListContent);

/// Heading line; text rendering arrives with the Text primitive.
struct AvatarMemberListHeading(String);
impl PrimitiveView for AvatarMemberListHeading {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, node: &Node) {
        let mut semantics = AccessibilitySemantics::new(AccessibilityRole::Heading);
        semantics.label = self.0.clone();
        node.set_accessibility(Some(semantics));
        node.set_attachment("avatar.member_list.title", self.0.clone());
        if let Some(layout) = node.layout_node() {
            layout.set_padding(8.0, cove_core::scene::Edge::Horizontal);
            layout.set_padding(4.0, cove_core::scene::Edge::Vertical);
        }
    }
}
impl View for AvatarMemberListHeading {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
}

/// One roster line. The avatar copy keeps source, colors and status; only the
/// diameter is unified so long rosters stay compact.
pub struct AvatarMemberRow {
    pub member: Avatar,
    pub on_select: Option<Rc<dyn Fn(&Avatar)>>,
}
impl PrimitiveView for AvatarMemberRow {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(self.on_select.is_some());
        if self.on_select.is_some() {
            node.set_is_focusable(true);
            let mut semantics = AccessibilitySemantics::new(AccessibilityRole::Button);
            semantics.label = if self.member.name.is_empty() {
                "Anonymous user".to_string()
            } else {
                self.member.name.clone()
            };
            semantics.value = self.member.status_text();
            node.set_accessibility(Some(semantics));
        } else {
            let mut semantics = AccessibilitySemantics::new(AccessibilityRole::ListItem);
            semantics.label = if self.member.name.is_empty() {
                "Anonymous user".to_string()
            } else {
                self.member.name.clone()
            };
            semantics.value = self.member.status_text();
            semantics.combines_children = true;
            node.set_accessibility(Some(semantics));
        }
        node
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Row);
        layout.set_align_items(Align::Center);
        layout.set_gap(10.0, cove_core::scene::Gutter::Column);
        layout.set_min_height(Some(44.0));
        layout.set_padding(8.0, cove_core::scene::Edge::Horizontal);
        layout.set_padding(4.0, cove_core::scene::Edge::Vertical);
        Some(layout)
    }
    fn update_node(&self, node: &Node) {
        if self.on_select.is_some() {
            let member = self.member.clone();
            let on_select = self.on_select.clone().unwrap();
            if let Some(registry) = cove_core::scene::InteractionRegistryHolder::current() {
                registry.set_pointer(
                    node,
                    cove_core::scene::InputHandlerRoute::control(),
                    Rc::new(move |_event, phase, event_phase| {
                        if phase == cove_core::scene::PointerPhase::Up
                            && event_phase == cove_core::scene::EventPhase::Target
                        {
                            on_select(&member);
                            return cove_core::scene::EventResult::Handled;
                        }
                        cove_core::scene::EventResult::Ignored
                    }),
                );
            }
            node.set_cursor(Some(cove_platform::SystemCursor::Pointer));
        }
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let mut avatar = self.member.clone();
        avatar.appearance.diameter = Some(28.0);
        vec![AnyView::new(avatar)]
    }
}
impl_primitive_view!(AvatarMemberRow);
