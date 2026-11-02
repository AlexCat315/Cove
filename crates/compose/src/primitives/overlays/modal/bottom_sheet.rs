//! Translation of `Primitives/Overlays/Modal/BottomSheet.swift`.
//!
//! Phone-first bottom sheet: full-width bottom panel on compact/medium
//! surfaces, centered dialog-like panel on expanded surfaces. The content owns
//! its layout; this component owns the handle, detents and dismissal.

use crate::Scope;
use std::cell::Cell;
use std::rc::Rc;

use crate::adaptive::WidthClassReader;
use crate::animation::geometry::transforms::TransformViewExt;
use crate::primitives::overlays::modal::{Modal, ModalDismissal, ModalPlacement, SheetDragHandle};
use crate::{AnyView, BoxView, TupleView, View};
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_core::{Binding, SizeClass};

/// Detents and drag behavior for a bottom sheet.
#[derive(Clone, Debug, PartialEq)]
pub struct BottomSheetOptions {
    /// Sheet heights in logical points, from smallest to largest.
    pub detents: Vec<f32>,
    pub initial_detent: usize,
    pub shows_handle: bool,
    pub allows_drag_to_dismiss: bool,
    pub drag_dismiss_threshold: f32,
    pub drag_dismiss_velocity: f32,
    pub dismissal: ModalDismissal,
    /// Width used when the surface is expanded and the sheet degrades to a
    /// centered dialog-like panel.
    pub dialog_width: f32,
    pub dialog_inset: f32,
}

impl Default for BottomSheetOptions {
    fn default() -> Self {
        Self {
            detents: vec![320.0, 560.0],
            initial_detent: 0,
            shows_handle: true,
            allows_drag_to_dismiss: true,
            drag_dismiss_threshold: 80.0,
            drag_dismiss_velocity: 900.0,
            dismissal: ModalDismissal::default(),
            dialog_width: 560.0,
            dialog_inset: 16.0,
        }
    }
}

impl BottomSheetOptions {
    pub fn new() -> Self {
        let value = Self::default();
        value.validated()
    }

    pub fn validated(mut self) -> Self {
        self.detents
            .retain(|height| height.is_finite() && *height > 0.0);
        if self.detents.is_empty() {
            self.detents = vec![320.0, 560.0];
        }
        self.drag_dismiss_threshold = if self.drag_dismiss_threshold.is_finite() {
            self.drag_dismiss_threshold.max(0.0)
        } else {
            80.0
        };
        self.drag_dismiss_velocity = if self.drag_dismiss_velocity.is_finite() {
            self.drag_dismiss_velocity.max(0.0)
        } else {
            900.0
        };
        self.dialog_width = if self.dialog_width.is_finite() {
            self.dialog_width.max(120.0)
        } else {
            560.0
        };
        self.dialog_inset = if self.dialog_inset.is_finite() {
            self.dialog_inset.max(0.0)
        } else {
            16.0
        };
        self
    }
}

/// Sheet state that has to survive recomposition: the chosen detent and the
/// live drag offset.
struct BottomSheetState {
    detent: Cell<usize>,
    drag_offset: Cell<f32>,
}

impl BottomSheetState {
    fn new(initial_detent: usize, detent_count: usize) -> Rc<Self> {
        Rc::new(Self {
            detent: Cell::new(if detent_count == 0 {
                0
            } else {
                initial_detent.min(detent_count - 1)
            }),
            drag_offset: Cell::new(0.0),
        })
    }
}

/// Bottom sheet with detents and drag-to-dismiss.
pub struct BottomSheet {
    pub is_presented: Binding<bool>,
    pub(crate) options: BottomSheetOptions,
    content: AnyView,
    state: Rc<BottomSheetState>,
}

impl BottomSheet {
    pub fn new(is_presented: Binding<bool>, content: impl View + 'static) -> Self {
        let options = BottomSheetOptions::new();
        let state = BottomSheetState::new(options.initial_detent, options.detents.len());
        Self {
            is_presented,
            options,
            content: AnyView::new(content),
            state,
        }
    }

    /// Edit the current options in builder order. Earlier edits to other fields are retained.
    pub fn configure(mut self, configure: impl FnOnce(&mut BottomSheetOptions)) -> Self {
        configure(&mut self.options);
        self.options = self.options.validated();
        self.state = BottomSheetState::new(self.options.initial_detent, self.options.detents.len());
        self
    }
}

impl View for BottomSheet {
    fn body(&self, _cx: &Scope) -> AnyView {
        let is_presented = self.is_presented.clone();
        let options = self.options.clone();
        let content = self.content.clone();
        let state = self.state.clone();
        AnyView::new(WidthClassReader::new(move |width_class: SizeClass| {
            let compact = width_class != SizeClass::Expanded;
            let index = state
                .detent
                .get()
                .min(options.detents.len().saturating_sub(1));
            let height = options.detents.get(index).copied().unwrap_or(320.0);

            let mut modal_options = crate::primitives::overlays::modal::ModalOptions::default();
            modal_options.dismissal = options.dismissal;
            modal_options.geometry.corner_radius = 12.0;
            if compact {
                modal_options.geometry.placement = ModalPlacement::Bottom;
                modal_options.geometry.inset = 0.0;
                modal_options.geometry.width = f32::MAX;
                modal_options.geometry.height = height;
            } else {
                modal_options.geometry.placement = ModalPlacement::Center;
                modal_options.geometry.inset = options.dialog_inset;
                modal_options.geometry.width = options.dialog_width;
                modal_options.geometry.height = height;
            }

            let mut column: Vec<AnyView> = Vec::new();
            if options.shows_handle {
                let drag_state = state.clone();
                let settle_options = options.clone();
                let settle_presented = is_presented.clone();
                let handle = SheetDragHandle::new(
                    {
                        let drag_state = drag_state.clone();
                        move |translation: f32| {
                            drag_state.drag_offset.set(translation.clamp(-120.0, 800.0));
                        }
                    },
                    move |translation: f32, velocity: f32| {
                        let threshold = settle_options.drag_dismiss_threshold;
                        let detent = drag_state.detent.get();
                        if translation < -threshold && detent + 1 < settle_options.detents.len() {
                            drag_state.detent.set(detent + 1);
                        } else if translation > threshold
                            || (velocity > settle_options.drag_dismiss_velocity
                                && translation > 0.0)
                        {
                            if detent > 0 {
                                drag_state.detent.set(detent - 1);
                            } else if settle_options.allows_drag_to_dismiss {
                                settle_presented.set(false);
                            }
                        }
                        drag_state.drag_offset.set(0.0);
                    },
                );
                column.push(AnyView::new(handle));
            }
            column.push(content.clone());

            let body = AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(column)),
            ))
            .offset(0.0, state.drag_offset.get());

            AnyView::new(Modal::new(is_presented.clone(), body).options(modal_options))
        }))
    }
}
