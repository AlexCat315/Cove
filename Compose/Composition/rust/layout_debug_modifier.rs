//! Translation of CoveUIComposeCore/LayoutDebugModifier.swift.
use cove_core::scene::Node;
use crate::{ModifiedContent, View, ViewExt, ViewModifier};

pub struct LayoutDebugAttachmentKey;
impl LayoutDebugAttachmentKey {
    pub const LAYOUT_ROLE: &'static str = "CoveUI.layout.role";
    pub const SEMANTIC_ROLE: &'static str = "CoveUI.semantic.role";
    pub const DEBUG_NAME: &'static str = "CoveUI.debug.name";
}

pub struct LayoutDebugModifier {
    pub layout_role: Option<String>,
    pub semantic_role: Option<String>,
    pub debug_name: Option<String>,
}
impl LayoutDebugModifier {
    pub fn new(layout_role: Option<&str>, semantic_role: Option<&str>, debug_name: Option<&str>) -> Self {
        Self {
            layout_role: layout_role.map(str::to_string),
            semantic_role: semantic_role.map(str::to_string),
            debug_name: debug_name.map(str::to_string),
        }
    }
    pub fn layout_role(value: &str) -> Self {
        Self::new(Some(value), None, None)
    }
    pub fn semantic_role(value: &str) -> Self {
        Self::new(None, Some(value), None)
    }
    pub fn debug_name(value: &str) -> Self {
        Self::new(None, None, Some(value))
    }
}
impl ViewModifier for LayoutDebugModifier {
    fn apply_node(&self, node: &Node) {
        if let Some(role) = &self.layout_role {
            node.set_attachment(LayoutDebugAttachmentKey::LAYOUT_ROLE, role.clone());
        }
        if let Some(role) = &self.semantic_role {
            node.set_attachment(LayoutDebugAttachmentKey::SEMANTIC_ROLE, role.clone());
        }
        if let Some(name) = &self.debug_name {
            node.set_attachment(LayoutDebugAttachmentKey::DEBUG_NAME, name.clone());
        }
    }
}

pub trait LayoutDebugExt: View + Sized + 'static {
    fn layout_role(self, value: &str) -> ModifiedContent;
    fn semantic_role(self, value: &str) -> ModifiedContent;
    fn debug_name(self, value: &str) -> ModifiedContent;
}
impl<T: View + Sized + 'static> LayoutDebugExt for T {
    fn layout_role(self, value: &str) -> ModifiedContent {
        self.modifier(LayoutDebugModifier::layout_role(value))
    }
    fn semantic_role(self, value: &str) -> ModifiedContent {
        self.modifier(LayoutDebugModifier::semantic_role(value))
    }
    fn debug_name(self, value: &str) -> ModifiedContent {
        self.modifier(LayoutDebugModifier::debug_name(value))
    }
}
