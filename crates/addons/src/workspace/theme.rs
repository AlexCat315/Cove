//! Workspace metrics. Panes are borderless rounded blocks resting on the window
//! canvas; the gutter between two panes is what separates them, so no rule is
//! drawn. The grab area of a split overhangs both panes and reserves no layout
//! space beyond the gutter.

use cove_core::scene::{CompositionLocal, Node};
use std::rc::Rc;

/// Workspace metrics carried as a composition-local value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorkspaceTheme {
    pub side_rail_width: f32,
    pub bottom_rail_height: f32,
    pub tab_bar_height: f32,
    /// Empty canvas between two panes.
    pub gutter: f32,
    /// Extra grab area on each side of a gutter. It overhangs the panes.
    pub split_divider_hit_slop: f32,
    pub animates_layout: bool,
    /// A collapsed group leaves a strip of its tab icons on its edge. Hosts
    /// with their own panel launcher turn the strips off; a collapsed group
    /// then takes no space until it is expanded again.
    pub shows_collapsed_rails: bool,
}

impl Default for WorkspaceTheme {
    fn default() -> Self {
        Self {
            side_rail_width: 40.0,
            bottom_rail_height: 40.0,
            tab_bar_height: 34.0,
            gutter: 6.0,
            split_divider_hit_slop: 6.0,
            animates_layout: false,
            shows_collapsed_rails: true,
        }
    }
}

impl WorkspaceTheme {
    pub const COMPOSITION_LOCAL_KEY: &'static str = "cove.workspace.theme";

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        side_rail_width: f32,
        bottom_rail_height: f32,
        tab_bar_height: f32,
        gutter: f32,
        split_divider_hit_slop: f32,
        animates_layout: bool,
        shows_collapsed_rails: bool,
    ) -> Self {
        Self {
            side_rail_width,
            bottom_rail_height,
            tab_bar_height,
            gutter,
            split_divider_hit_slop,
            animates_layout,
            shows_collapsed_rails,
        }
    }
}

/// Composition-local carrying [`WorkspaceTheme`]. A host provides it once near
/// the root; every descendant reads the nearest provided value.
pub struct WorkspaceThemeEnvironment;

impl WorkspaceThemeEnvironment {
    pub fn key() -> Rc<CompositionLocal<WorkspaceTheme>> {
        thread_local! {
            static KEY: Rc<CompositionLocal<WorkspaceTheme>> =
                Rc::new(CompositionLocal::constant(WorkspaceTheme::default()));
        }
        KEY.with(Rc::clone)
    }
}

/// The theme resolved for `node`, falling back to the default metrics.
pub fn resolve_workspace_theme(node: &Node) -> WorkspaceTheme {
    node.composition_value(&WorkspaceThemeEnvironment::key())
}

/// Region sizing used by the collapse/expand transition. `Divider` reserves
/// only the hairline; the grab area is added as a negative margin so it
/// overhangs the neighbouring panes instead of pushing them apart.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WorkspaceRegionSizing {
    Fraction(f32),
    Points(f32),
    Divider,
}

impl WorkspaceRegionSizing {
    pub fn extent(self, theme: &WorkspaceTheme) -> f32 {
        match self {
            Self::Fraction(value) | Self::Points(value) => value.max(0.0),
            Self::Divider => theme.gutter.max(1.0),
        }
    }
    pub fn is_divider(self) -> bool {
        matches!(self, Self::Divider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn divider_extent_is_never_below_one_point() {
        let theme = WorkspaceTheme {
            gutter: 0.0,
            ..WorkspaceTheme::default()
        };
        assert_eq!(WorkspaceRegionSizing::Divider.extent(&theme), 1.0);
        assert_eq!(WorkspaceRegionSizing::Points(-4.0).extent(&theme), 0.0);
    }
}
