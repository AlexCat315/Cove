//! Translation of Theme/Styles/Components/DefaultDividerStyle.swift.
use cove_core::Color;
use cove_core::scene::{LayoutNode, Node};

use crate::{AnyView, impl_primitive_view};

use super::divider_style::{DividerOrientation, DividerStyle, DividerStyleConfiguration};

/// Divider default: 1-px line drawn with the `divider` color slot. Spans 100 %
/// of the cross-axis.
///
/// GAP: Swift returns the legacy `Divider` primitive directly
/// (Primitives/Text/Text.swift). That primitive has no Rust counterpart yet,
/// so `DividerLine` reproduces its documented layout contract here: width 100 %
/// for horizontal dividers, height 100 % for vertical ones, thickness on the
/// cross axis. When `crate::primitives::Divider` lands this body should return
/// it instead.
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultDividerStyle;

impl DividerStyle for DefaultDividerStyle {
    fn make_body(&self, configuration: DividerStyleConfiguration) -> AnyView {
        AnyView::new(DividerLine {
            color: configuration.theme.colors.divider,
            thickness: configuration.thickness,
            orientation: configuration.orientation,
        })
    }
}

struct DividerLine {
    color: Color,
    thickness: f32,
    orientation: DividerOrientation,
}

impl crate::PrimitiveView for DividerLine {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, node: &Node) {
        node.set_background_color(Some(self.color));
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        match self.orientation {
            DividerOrientation::Horizontal => {
                layout.set_width_percent(100.0);
                layout.set_height(Some(self.thickness));
            }
            DividerOrientation::Vertical => {
                layout.set_height_percent(100.0);
                layout.set_width(Some(self.thickness));
            }
        }
    }
}

impl_primitive_view!(DividerLine);
