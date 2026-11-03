//! Translation of CoveUIScene/Layout/CustomLayout.swift.
//!
//! A custom layout is a measured leaf to flexbox. Its children are laid out
//! one at a time inside a probe: a flex-start column the size of the proposal,
//! so an intrinsic measurement gets an at-most proposal while authored sizes
//! and minimums stay authoritative. Placement runs after the flex pass and
//! only when the bounds or direction changed.
use super::{LayoutNode, engine};
use crate::{Point, Rect, Size};
use std::cell::RefCell;
use std::rc::Rc;

pub type CustomMeasure = Rc<dyn Fn(Option<f32>, Option<f32>, &[LayoutNode]) -> Size>;
pub type CustomPlace = Rc<dyn Fn(Rect, &[LayoutNode])>;

pub struct CustomLayoutState {
    pub measure: RefCell<CustomMeasure>,
    pub place: RefCell<CustomPlace>,
    placement: RefCell<Option<(Rect, super::Direction)>>,
}
impl CustomLayoutState {
    pub fn new(measure: CustomMeasure, place: CustomPlace) -> Self {
        Self {
            measure: RefCell::new(measure),
            place: RefCell::new(place),
            placement: RefCell::new(None),
        }
    }
    pub fn invalidate(&self) {
        *self.placement.borrow_mut() = None;
    }
}

impl LayoutNode {
    pub fn set_custom_layout(&self, measure: CustomMeasure, place: CustomPlace) {
        let updated = {
            let data = self.data();
            match &data.custom {
                Some(state) => {
                    *state.measure.borrow_mut() = measure.clone();
                    *state.place.borrow_mut() = place.clone();
                    true
                }
                None => false,
            }
        };
        if !updated {
            let mut data = self.data_mut();
            data.custom = Some(CustomLayoutState::new(measure, place));
            data.measure = None;
        }
        for child in self.children() {
            child.0.engine.forget_measurements();
        }
        self.mark_layout_dirty_hint();
    }
    pub(super) fn is_custom(&self) -> bool {
        self.data().custom.is_some()
    }
    pub fn custom_layout_direction(&self) -> super::Direction {
        let direction = self.direction();
        if direction != super::Direction::Inherit {
            return direction;
        }
        self.parent().map_or_else(
            || engine::resolved_direction(&self.0),
            |parent| parent.custom_layout_direction(),
        )
    }
    /// Size of this child of a custom layout under `width` x `height`.
    pub fn measure_for_custom_layout(&self, width: Option<f32>, height: Option<f32>) -> Size {
        let Some(owner) = self.parent().filter(LayoutNode::is_custom) else {
            return Size::default();
        };
        let direction = owner.custom_layout_direction();
        if let Some(size) = self.0.engine.measurement(width, height, direction) {
            return size;
        }
        // Probing relays out this child's subtree, so the owner must place
        // again even if its own bounds stay equal.
        if let Some(state) = &owner.data().custom {
            state.invalidate();
        }
        let size = engine::layout_probe(self, width, height, direction);
        self.0
            .engine
            .store_measurement(width, height, direction, size);
        size
    }
    pub fn custom_layout_is_visible(&self) -> bool {
        self.data().placement_is_visible
    }
    pub fn place_for_custom_layout(
        &self,
        at: Point,
        width: Option<f32>,
        height: Option<f32>,
        is_visible: bool,
    ) {
        let Some(owner) = self.parent().filter(LayoutNode::is_custom) else {
            return;
        };
        self.data_mut().placement_is_visible = is_visible;
        if !is_visible {
            self.data_mut().placed_frame = Some(Rect::default());
            return;
        }
        let size = engine::layout_probe(self, width, height, owner.custom_layout_direction());
        self.place_custom_layouts();
        // The probe reports the child's intrinsic size, which can be narrower
        // than the slot the algorithm assigned it. Honour an explicit proposal
        // so the frame matches the box the layout measured against; otherwise
        // a wrapped cell reports the height of a wider box and clips its last
        // line.
        let placed = Size {
            width: width.unwrap_or(size.width),
            height: height.unwrap_or(size.height),
        };
        self.data_mut().placed_frame = Some(Rect::new(at.x, at.y, placed.width, placed.height));
    }
    pub(super) fn prepare_custom_layouts(&self) {
        if !self.subtree_is_dirty() {
            return;
        }
        if let Some(custom) = &self.data().custom {
            custom.invalidate();
        }
        for child in self.children() {
            child.prepare_custom_layouts();
        }
    }
    pub(super) fn place_custom_layouts(&self) {
        let place = {
            let data = self.data();
            data.custom
                .as_ref()
                .map(|state| state.place.borrow().clone())
        };
        let Some(place) = place else {
            for child in self.children() {
                child.place_custom_layouts();
            }
            return;
        };
        let layout = self.0.engine.layout();
        let left = layout.padding.left + layout.border.left;
        let top = layout.padding.top + layout.border.top;
        let bounds = Rect::new(
            left,
            top,
            (layout.size.width - left - layout.padding.right - layout.border.right).max(0.0),
            (layout.size.height - top - layout.padding.bottom - layout.border.bottom).max(0.0),
        );
        let direction = self.custom_layout_direction();
        let unchanged = {
            let data = self.data();
            let state = data.custom.as_ref().expect("custom layout state");
            let mut placement = state.placement.borrow_mut();
            if *placement == Some((bounds, direction)) {
                true
            } else {
                *placement = Some((bounds, direction));
                false
            }
        };
        if !unchanged {
            place(bounds, &self.children());
        }
    }
}
