//! Translation of CoveUIComposeCore/Layout/Direction.swift.
use crate::{ModifiedContent, View, ViewExt, ViewModifier};
use cove_core::scene::{Direction, LayoutNode};

struct LayoutDirectionModifier {
    direction: Direction,
}
impl ViewModifier for LayoutDirectionModifier {
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_direction(self.direction);
    }
}

pub trait LayoutDirectionExt: View + Sized + 'static {
    /// Logical leading/trailing edges, flex rows and custom layouts inherit
    /// this direction. Explicit physical left/right positions stay physical.
    fn layout_direction(self, direction: Direction) -> ModifiedContent;
}
impl<T: View + Sized + 'static> LayoutDirectionExt for T {
    fn layout_direction(self, direction: Direction) -> ModifiedContent {
        self.modifier(LayoutDirectionModifier { direction })
    }
}
