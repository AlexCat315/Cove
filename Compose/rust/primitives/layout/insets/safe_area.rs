//! Translation of Primitives/Layout/Insets/SafeArea.swift.
//!
//! Insets supplied by the host are consumed once, by the selected logical edges.
//! Nested safe-area containers receive only the remaining edges.
use std::rc::Rc;

use cove_core::scene::{CompositionLocal, Edge, Node};
use crate::modifiers::layout::EdgeInsets;
use crate::{AnyView, PrimitiveView, View, ViewExt};

/// Which logical edges consume the host inset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SafeAreaEdges(pub u8);

impl SafeAreaEdges {
    pub const TOP: Self = Self(1 << 0);
    pub const LEADING: Self = Self(1 << 1);
    pub const BOTTOM: Self = Self(1 << 2);
    pub const TRAILING: Self = Self(1 << 3);
    pub const HORIZONTAL: Self = Self(Self::LEADING.0 | Self::TRAILING.0);
    pub const VERTICAL: Self = Self(Self::TOP.0 | Self::BOTTOM.0);
    pub const ALL: Self = Self(Self::HORIZONTAL.0 | Self::VERTICAL.0);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl Default for SafeAreaEdges {
    fn default() -> Self {
        Self::ALL
    }
}

/// Environment carrying the unconsumed host insets.
pub struct SafeAreaEnvironment;
impl SafeAreaEnvironment {
    pub fn insets() -> Rc<CompositionLocal<EdgeInsets>> {
        thread_local! {
            static KEY: Rc<CompositionLocal<EdgeInsets>> =
                Rc::new(CompositionLocal::constant(EdgeInsets::default()));
        }
        KEY.with(Rc::clone)
    }
}

/// Consumes the selected insets for its content.
pub struct SafeArea {
    pub edges: SafeAreaEdges,
    content: AnyView,
}

impl SafeArea {
    pub fn new(edges: SafeAreaEdges, content: impl View + 'static) -> Self {
        Self { edges, content: AnyView::new(content) }
    }

    pub fn all(content: impl View + 'static) -> Self {
        Self::new(SafeAreaEdges::ALL, content)
    }
}

fn valid(value: f32) -> f32 {
    if value.is_finite() { value.max(0.0) } else { 0.0 }
}

impl PrimitiveView for SafeArea {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, node: &Node) {
        let insets: EdgeInsets = node.composition_value(&SafeAreaEnvironment::insets());
        let Some(layout) = node.layout_node() else { return };
        layout.set_padding(
            if self.edges.contains(SafeAreaEdges::TOP) { valid(insets.top) } else { 0.0 },
            Edge::Top,
        );
        layout.set_padding(
            if self.edges.contains(SafeAreaEdges::LEADING) { valid(insets.leading) } else { 0.0 },
            Edge::Start,
        );
        layout.set_padding(
            if self.edges.contains(SafeAreaEdges::BOTTOM) { valid(insets.bottom) } else { 0.0 },
            Edge::Bottom,
        );
        layout.set_padding(
            if self.edges.contains(SafeAreaEdges::TRAILING) { valid(insets.trailing) } else { 0.0 },
            Edge::End,
        );
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let mut remaining: EdgeInsets = node.composition_value(&SafeAreaEnvironment::insets());
        if self.edges.contains(SafeAreaEdges::TOP) { remaining.top = 0.0; }
        if self.edges.contains(SafeAreaEdges::LEADING) { remaining.leading = 0.0; }
        if self.edges.contains(SafeAreaEdges::BOTTOM) { remaining.bottom = 0.0; }
        if self.edges.contains(SafeAreaEdges::TRAILING) { remaining.trailing = 0.0; }
        vec![AnyView::new(
            self.content.clone().composition_local(SafeAreaEnvironment::insets(), remaining),
        )]
    }
}
crate::impl_primitive_view!(SafeArea);

/// Provide host insets in logical points. Desktop hosts default to zero;
/// platform chrome and keyboard geometry must be supplied by the owning host.
pub fn safe_area_insets(view: AnyView, insets: EdgeInsets) -> AnyView {
    AnyView::new(view.composition_local(SafeAreaEnvironment::insets(), insets))
}
