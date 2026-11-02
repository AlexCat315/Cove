//! Translation of the `SheetOptions`/`Sheet` half of
//! Primitives/Overlays/Modal/Dialog.swift.
//!
//! An edge sheet: a rounded panel flush to one window edge, sized along the
//! axis it slides on. The bottom sheet keeps its own detent model; this is the
//! simpler single-extent form used for inspector panels and settings trays.
use crate::Scope;
use cove_core::Binding;

use crate::primitives::overlays::modal::Modal;
use crate::primitives::overlays::modal::modal_options::{
    ModalDismissal, ModalGeometry, ModalOptions, ModalPlacement,
};
use crate::{AnyView, View};

/// Edge, extent and chrome for an edge sheet.
#[derive(Clone, Debug, PartialEq)]
pub struct SheetOptions {
    pub edge: ModalPlacement,
    pub extent: f32,
    /// Margin from the window edge; keeps the rounded sheet visually floating.
    pub inset: f32,
    pub corner_radius: f32,
    pub dismissal: ModalDismissal,
}

impl Default for SheetOptions {
    fn default() -> Self {
        Self {
            edge: ModalPlacement::Trailing,
            extent: 400.0,
            inset: 12.0,
            corner_radius: 14.0,
            dismissal: ModalDismissal::default(),
        }
    }
}

impl SheetOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn edge(mut self, edge: ModalPlacement) -> Self {
        self.edge = edge;
        self
    }

    pub fn extent(mut self, extent: f32) -> Self {
        self.extent = extent;
        self
    }

    pub fn inset(mut self, inset: f32) -> Self {
        self.inset = inset;
        self
    }

    pub fn corner_radius(mut self, corner_radius: f32) -> Self {
        self.corner_radius = corner_radius;
        self
    }

    pub fn dismissal(mut self, dismissal: ModalDismissal) -> Self {
        self.dismissal = dismissal;
        self
    }

    pub fn validate(&mut self) {
        self.extent = if self.extent.is_finite() {
            self.extent.max(64.0)
        } else {
            400.0
        };
        self.inset = if self.inset.is_finite() {
            self.inset.max(0.0)
        } else {
            12.0
        };
        self.corner_radius = if self.corner_radius.is_finite() {
            self.corner_radius.max(0.0)
        } else {
            14.0
        };
        // A centered sheet has no edge to slide from.
        if self.edge == ModalPlacement::Center {
            self.edge = ModalPlacement::Trailing;
        }
    }

    pub fn validated(mut self) -> Self {
        self.validate();
        self
    }

    /// The modal geometry this sheet presents with.
    pub fn geometry(&self) -> ModalGeometry {
        let horizontal_edge = matches!(self.edge, ModalPlacement::Top | ModalPlacement::Bottom);
        ModalGeometry {
            width: if horizontal_edge {
                f32::MAX
            } else {
                self.extent
            },
            height: if horizontal_edge {
                self.extent
            } else {
                f32::MAX
            },
            inset: self.inset,
            corner_radius: self.corner_radius,
            placement: self.edge,
        }
    }
}

/// Rounded panel flush to one window edge.
pub struct Sheet {
    pub is_presented: Binding<bool>,
    pub(crate) options: SheetOptions,
    content: AnyView,
}

impl Sheet {
    pub fn new(is_presented: Binding<bool>, content: impl View + 'static) -> Self {
        Self {
            is_presented,
            options: SheetOptions::new(),
            content: AnyView::new(content),
        }
    }

    /// Edit the current options in builder order. Earlier edits to other fields are retained.
    pub fn configure(mut self, configure: impl FnOnce(&mut SheetOptions)) -> Self {
        configure(&mut self.options);
        self.options.validate();
        self
    }
}

impl View for Sheet {
    fn body(&self, _cx: &Scope) -> AnyView {
        let mut geometry = self.options.geometry();
        geometry.validate();
        let options = ModalOptions {
            geometry,
            dismissal: self.options.dismissal,
            ..Default::default()
        };
        AnyView::new(Modal::new(self.is_presented.clone(), self.content.clone()).options(options))
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
