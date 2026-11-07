//! Translation of Theme/ThemeEnvironment.swift.
use super::theme::Theme;
use super::tokens::TextStyleToken;
use crate::{ModifiedContent, View, ViewExt};
use cove_core::scene::{CompositionLocal, Node};
use std::rc::Rc;

/// `CompositionLocal` key carrying the active `Theme` down the composition
/// tree. Always resolves: descendants without an ancestor `.theme(_:)`
/// modifier see `Theme::default_dark`.
pub struct ThemeEnvironment;
impl ThemeEnvironment {
    pub fn key() -> Rc<CompositionLocal<Theme>> {
        thread_local! {
            static KEY: Rc<CompositionLocal<Theme>> = Rc::new(CompositionLocal::new(Theme::default_dark));
        }
        KEY.with(|key| key.clone())
    }
    pub fn body_style() -> Rc<CompositionLocal<Option<TextStyleToken>>> {
        thread_local! {
            static KEY: Rc<CompositionLocal<Option<TextStyleToken>>> = Rc::new(CompositionLocal::constant(None));
        }
        KEY.with(|key| key.clone())
    }
    pub const BODY_STYLE_ATTACHMENT: &'static str = "__theme_body_style";
}

/// The `Theme` resolved for this node by walking its ancestor chain.
/// Falls back to `Theme::default_dark` when no provider is reachable.
pub fn theme_for_node(node: &Node) -> Theme {
    node.composition_value(&ThemeEnvironment::key())
}

/// Nearest explicitly provided theme at or below this node.
///
/// The search is bounded: applications provide their theme near the root, so a
/// deep scan would only cost frames.
pub fn surface_theme(node: &Node) -> Option<Theme> {
    provided_theme_within(node, 12, 512)
}

/// Nearest explicitly provided theme within `max_depth` levels and `budget`
/// visited nodes, breadth first. Hosts that wrap the application root in
/// several providers (surface, text scale, portals) need a deeper reach than
/// [`surface_theme`].
pub fn provided_theme_within(node: &Node, max_depth: usize, budget: i32) -> Option<Theme> {
    let mut level = vec![node.clone()];
    let mut depth = 0;
    let mut budget = budget;
    while !level.is_empty() && depth < max_depth && budget > 0 {
        let mut next = Vec::new();
        for current in level {
            budget -= 1;
            if budget < 0 {
                break;
            }
            if let Some(theme) = current.composition_value_here(&ThemeEnvironment::key()) {
                return Some(theme);
            }
            next.extend(current.children());
        }
        level = next;
        depth += 1;
    }
    None
}

pub trait ThemeViewExt: View + Sized + 'static {
    /// Provide `theme` to every descendant of this view. Nearer providers
    /// override farther ones; absent providers fall back to
    /// `Theme::default_dark`.
    fn theme(self, theme: Theme) -> ModifiedContent {
        let body = theme.typography.resolved_body();
        self.composition_local(ThemeEnvironment::key(), theme)
            .composition_local(ThemeEnvironment::body_style(), Some(body))
    }
}
impl<T: View + Sized + 'static> ThemeViewExt for T {}
