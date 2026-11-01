//! Translation of Primitives/Overlays/OverlayHost.swift.
//!
//! Compatibility wrapper for the portal system. New roots should use
//! `LayerRoot` with an explicit portal host, so overlays never participate in
//! normal flex layout.
use crate::Scope;
use crate::primitives::overlays::portal::PortalHost;
use crate::{AnyView, View};

/// Compatibility wrapper that renders the window's portal entries.
pub struct OverlayHost;

impl OverlayHost {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OverlayHost {
    fn default() -> Self {
        Self::new()
    }
}

impl View for OverlayHost {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(PortalHost::new())
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
