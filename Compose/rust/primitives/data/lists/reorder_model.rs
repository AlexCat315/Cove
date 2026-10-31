//! Translation of `Primitives/Data/Lists/Reorder/{Preview,Reorderable}.swift`
//! (the model half).
//!
//! Transient drag geometry shared by the handle, the list and the floating
//! preview, plus the index arithmetic that turns a drag distance into a
//! destination. Pointer moves update the session in place; only starting and
//! ending a drag recompose the list.
//!
//! Behaviour checklist:
//! - the preview follows the pointer but never leaves the viewport
//! - the drop boundary is the source row shifted by whole row strides, clamped
//! - finishing converts the boundary to a destination index after removal
//! - a drop on the row's own position reports no move
//! - keyboard stepping clamps instead of wrapping
//! - mounted rows always include the dragged and last-moved rows

use std::cell::{Cell, RefCell};

/// Live drag geometry for one reorder.
///
/// Deliberately a shared mutable session rather than per-frame state: moving the
/// preview must not recompose the list.
#[derive(Default)]
pub struct ReorderDragSession {
    /// Latest scroll offset of the list viewport.
    scroll_y: Cell<f32>,
    /// Latest height of the list viewport.
    viewport_height: Cell<f32>,
    /// Dragged row's top in viewport coordinates when the drag began.
    start_top: Cell<f32>,
    row_height: Cell<f32>,
    preview_top: Cell<f32>,
    /// Whether a drag is in progress.
    active: Cell<bool>,
}

impl ReorderDragSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_scroll_y(&self, value: f32) {
        self.scroll_y.set(value);
    }

    pub fn set_viewport_height(&self, value: f32) {
        self.viewport_height.set(value);
    }

    pub fn is_active(&self) -> bool {
        self.active.get()
    }

    /// Where the lifted row is drawn, in viewport coordinates.
    pub fn preview_top(&self) -> f32 {
        self.preview_top.get()
    }

    /// Starts a drag with the row's top in *content* coordinates.
    pub fn begin(&self, row_top_in_content: f32, row_height: f32) {
        self.row_height.set(row_height);
        self.start_top.set(row_top_in_content - self.scroll_y.get());
        self.active.set(true);
        self.move_by(0.0);
    }

    /// Follows the pointer, staying inside the viewport so the preview rests at
    /// an edge while automatic scrolling moves the content beneath it.
    pub fn move_by(&self, travel: f32) {
        let row_height = self.row_height.get();
        let ceiling = (self.viewport_height.get() - row_height).max(0.0);
        self.preview_top
            .set((self.start_top.get() + travel).clamp(0.0, ceiling));
    }

    pub fn end(&self) {
        self.active.set(false);
        self.preview_top.set(0.0);
        self.row_height.set(0.0);
        self.start_top.set(0.0);
    }
}

/// The column-style reorder rules for a vertical list.
pub enum ReorderableListModel {}

impl ReorderableListModel {
    /// The drop boundary for `source` dragged by `delta` points.
    ///
    /// `travel` is pointer movement and `scrolled` is automatic scrolling since
    /// the drag began; their sum is the displacement in content coordinates.
    pub fn drop_boundary(
        source: usize,
        travel: f32,
        scrolled: f32,
        row_stride: f32,
        count: usize,
    ) -> usize {
        let stride = row_stride.max(1.0);
        let offset = ((travel + scrolled) / stride).round() as i64;
        let boundary = source as i64 + offset;
        boundary.clamp(0, count as i64) as usize
    }

    /// The final index after the row is removed, or `None` when the row would
    /// stay where it is.
    ///
    /// A boundary past the source shifts back by one because removing the row
    /// closes the gap it left.
    pub fn destination(source: usize, boundary: usize) -> Option<usize> {
        let destination = if boundary > source {
            boundary - 1
        } else {
            boundary
        };
        (destination != source).then_some(destination)
    }

    /// The boundary a drop indicator should draw at, or `None` when the drop
    /// would be a no-op (the indicator must not appear in that case).
    pub fn visible_boundary(source: usize, boundary: usize) -> Option<usize> {
        Self::destination(source, boundary).map(|_| boundary)
    }

    /// Keyboard stepping clamps at both ends rather than wrapping.
    pub fn stepped(index: usize, delta: i64, count: usize) -> Option<usize> {
        if count == 0 {
            return None;
        }
        let last = count - 1;
        let destination = (index as i64 + delta).clamp(0, last as i64) as usize;
        (destination != index).then_some(destination)
    }

    /// Rows to keep mounted: the visible window plus the dragged and last
    /// keyboard-moved rows, so pointer capture and focus survive auto-scrolling.
    pub fn mounted_indices(
        visible: std::ops::Range<usize>,
        count: usize,
        retained: &[usize],
    ) -> Vec<usize> {
        let start = visible.start.min(count);
        let end = visible.end.min(count);
        let mut indices: Vec<usize> = (start..end).collect();
        for index in retained {
            if *index < count && !indices.contains(index) {
                indices.push(*index);
            }
        }
        indices
    }

    /// The accessible label for a handle, matching the authored-title form when
    /// the caller supplied one.
    pub fn handle_label(title: Option<&str>, index: usize, count: usize) -> String {
        match title.filter(|value| !value.is_empty()) {
            Some(title) => format!("Reorder {title}, position {} of {count}", index + 1),
            None => format!("Reorder item {} of {count}", index + 1),
        }
    }
}

/// Row height: the authored height after text scaling, raised to the minimum
/// control extent on a touch surface so the row is at least a finger target.
pub fn reorder_row_height(scaled: f32, is_touch: bool, minimum_control_extent: f32) -> f32 {
    // `max(scaler.scale(rowHeight), isTouch ? minimumControlExtent : 0)`.
    let floor = if is_touch {
        minimum_control_extent
    } else {
        0.0
    };
    scaled.max(floor)
}

/// The painted y of the drop indicator for `boundary`.
pub fn drop_indicator_y(
    boundary: usize,
    count: usize,
    row_stride: f32,
    content_height: f32,
) -> f32 {
    if boundary >= count {
        (content_height - 2.0).max(0.0)
    } else {
        boundary as f32 * row_stride - 1.0
    }
}

/// Shared handle for the preview node's scroll offset, so following the pointer
/// neither relayouts nor recomposes.
#[derive(Clone, Default)]
pub struct ReorderPreviewOffset(std::rc::Rc<RefCell<Option<std::rc::Rc<cove_core::scene::Node>>>>);

impl ReorderPreviewOffset {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bind(&self, node: std::rc::Rc<cove_core::scene::Node>) {
        *self.0.borrow_mut() = Some(node);
    }

    /// Applies `preview_top` through the node's content offset.
    pub fn apply(&self, preview_top: f32) {
        if let Some(node) = self.0.borrow().as_ref() {
            node.set_content_offset(cove_core::Point {
                x: 0.0,
                y: -preview_top,
            });
        }
    }
}
