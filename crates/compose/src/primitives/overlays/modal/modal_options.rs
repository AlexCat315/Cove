//! Translation of Primitives/Overlays/Modal/ModalOptions.swift.
use crate::animation::transition::Transition;
use crate::theme::SemanticColorRef;
use cove_core::{Size, Viewport};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ModalPlacement {
    #[default]
    Center,
    Leading,
    Trailing,
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModalGeometry {
    pub width: f32,
    pub height: f32,
    pub inset: f32,
    pub corner_radius: f32,
    pub placement: ModalPlacement,
}

impl Default for ModalGeometry {
    fn default() -> Self {
        Self {
            width: 760.0,
            height: 560.0,
            inset: 16.0,
            corner_radius: 10.0,
            placement: ModalPlacement::Center,
        }
    }
}

impl ModalGeometry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate(&mut self) {
        self.width = if self.width.is_finite() {
            self.width.max(64.0)
        } else {
            760.0
        };
        self.height = if self.height.is_finite() {
            self.height.max(64.0)
        } else {
            560.0
        };
        self.inset = if self.inset.is_finite() {
            self.inset.max(0.0)
        } else {
            16.0
        };
        self.corner_radius = if self.corner_radius.is_finite() {
            self.corner_radius.max(0.0)
        } else {
            10.0
        };
    }

    pub fn validated(mut self) -> Self {
        self.validate();
        self
    }

    /// Centered dialogs keep an 8 pt margin on a compact window.
    pub fn fit_compact_window(&mut self, size: Size) {
        if self.placement != ModalPlacement::Center || size.width >= Viewport::COMPACT_MAXIMUM_WIDTH
        {
            return;
        }
        self.inset = self.inset.min(8.0);
        self.width = (size.width - 2.0 * self.inset).max(0.0);
    }

    pub fn transition(&self, size: Size) -> Transition {
        match self.placement {
            ModalPlacement::Center => Transition::opacity().combined(Transition::offset(0.0, 12.0)),
            ModalPlacement::Leading => Transition::offset(-size.width, 0.0),
            ModalPlacement::Trailing => Transition::offset(size.width, 0.0),
            ModalPlacement::Top => Transition::offset(0.0, -size.height),
            ModalPlacement::Bottom => Transition::offset(0.0, size.height),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModalDismissal {
    pub closes_on_backdrop: bool,
    pub closes_on_escape: bool,
}

impl Default for ModalDismissal {
    fn default() -> Self {
        Self {
            closes_on_backdrop: true,
            closes_on_escape: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ModalBackdrop {
    #[default]
    Standard,
    Dimmed,
}

#[derive(Clone)]
pub struct ModalOptions {
    pub geometry: ModalGeometry,
    pub dismissal: ModalDismissal,
    pub backdrop: ModalBackdrop,
    pub surface: SemanticColorRef,
    pub fits_content_height: bool,
    pub translation_y: f32,
    /// Accessible name for the dialog panel; empty leaves the panel unnamed.
    pub label: String,
}

impl std::fmt::Debug for ModalOptions {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModalOptions")
            .field("geometry", &self.geometry)
            .field("dismissal", &self.dismissal)
            .field("backdrop", &self.backdrop)
            .field("fits_content_height", &self.fits_content_height)
            .field("translation_y", &self.translation_y)
            .field("label", &self.label)
            .finish()
    }
}

impl PartialEq for ModalOptions {
    /// A semantic color is a resolved-per-theme closure, so options compare on
    /// everything else; two surfaces differ only by palette, not by value.
    fn eq(&self, other: &Self) -> bool {
        self.geometry == other.geometry
            && self.dismissal == other.dismissal
            && self.backdrop == other.backdrop
            && self.fits_content_height == other.fits_content_height
            && self.translation_y == other.translation_y
            && self.label == other.label
    }
}

impl Default for ModalOptions {
    fn default() -> Self {
        Self {
            geometry: ModalGeometry::default(),
            dismissal: ModalDismissal::default(),
            backdrop: ModalBackdrop::Standard,
            surface: SemanticColorRef::surface_floating(),
            fits_content_height: false,
            translation_y: 0.0,
            label: String::new(),
        }
    }
}

impl ModalOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validated(mut self) -> Self {
        self.geometry.validate();
        self
    }
}
