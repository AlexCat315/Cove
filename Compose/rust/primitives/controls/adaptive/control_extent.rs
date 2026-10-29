//! Translation of Primitives/Controls/Adaptive/ControlExtent.swift.
use cove_core::InputCapabilities;
use cove_core::Size;
use cove_core::scene::Node;
use crate::SurfaceEnvironment;

/// Whether the surrounding surface can deliver hover transitions.
pub fn supports_control_hover(node: &Node) -> bool {
    node.composition_value(&SurfaceEnvironment::key()).supports_hover()
}

/// Gives touch surfaces a minimum hit extent without changing desktop density.
pub fn apply_touch_control_extent(node: &Node, enabled: bool) {
    let environment = node.composition_value(&SurfaceEnvironment::key());
    let extent = environment.minimum_control_extent();
    if let Some(layout_node) = node.layout_node() {
        layout_node.set_minimum_size_floor(if enabled && environment.input.contains(InputCapabilities::TOUCH) {
            Some(Size { width: extent, height: extent })
        } else {
            None
        });
    }
}
