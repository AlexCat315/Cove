//! One selection shared by every paragraph of a document, so a drag can run
//! from a heading through a list into a code block and copy as one string.
//! Transient view state: owned by the mounted root, never persisted.

use super::flow::RichTextFlow;
use crate::primitives::text::text_buffer::TextBuffer;
use cove_core::scene::{
    AccessibilityRevision, CompositionLocal, EventResult, InputHandlerRoute, InvalidationSource,
    Node, NodeResource, NodeWeak, PointerPhase,
};
use cove_core::scene::{FocusChainHolder, InputRoutingPriority, InteractionRegistryHolder};
use cove_core::{Clip, Color, DrawList, Painter, Point, Rect, RoundedRect, paint_scaled};
use cove_platform::SystemCursor;
use cove_text::TextEnvironmentHolder;
use std::any::Any;
use std::cell::RefCell;
use std::ops::Range;
use std::rc::{Rc, Weak};
use unicode_segmentation::UnicodeSegmentation;

/// Mirrors `CTextSelection.CoveSelectionFrame`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CoveSelectionFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub member: usize,
}

unsafe extern "C" {
    fn cove_selection_sort(frames: *mut CoveSelectionFrame, count: usize);
    fn cove_selection_find(
        frames: *const CoveSelectionFrame,
        count: usize,
        x: f64,
        y: f64,
        snap: bool,
        edge: *mut i32,
    ) -> isize;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextSelectionPosition {
    pub member: u64,
    pub offset: usize,
}

#[derive(Clone)]
struct SelectionMember {
    node: NodeWeak,
    text: Rc<dyn Fn() -> String>,
    /// Caret offset nearest to a window-space point.
    offset_at: Rc<dyn Fn(Point) -> usize>,
    /// Whether the point is over a painted line of glyphs. The node's frame
    /// covers its padding and whatever the parent stretched it to, so a label
    /// in a dock panel owned the text cursor across the whole panel.
    caret_at: Rc<dyn Fn(Point) -> bool>,
}

impl SelectionMember {
    fn node(&self) -> Option<Node> {
        Node::upgrade(&self.node)
    }

    fn text(&self) -> String {
        (self.text)()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SelectionSpan {
    lower: usize,
    upper: usize,
    start: usize,
    end: usize,
}

struct SelectionRegistration {
    scope: RefCell<Weak<RefCell<TextSelectionScope>>>,
}

impl SelectionRegistration {
    /// Removes `node` from the scope it joined.
    fn leave(&self, node: &Node) {
        let Some(scope) = self.scope.replace(Weak::new()).upgrade() else {
            return;
        };
        let mut scope = scope.borrow_mut();
        scope.members.remove(&node.id().0);
        scope.geometry_revision = None;
    }
}

impl NodeResource for SelectionRegistration {
    fn mount(&self, _node: &Node) {}
    /// The node has already released its resources by now, so this cannot go
    /// through [`TextSelectionScope::unregister`], which looks them up.
    fn unmount(&self, node: &Node) {
        self.leave(node);
    }
    fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }
}

#[derive(Clone)]
pub struct TextSelectionScope {
    members: std::collections::HashMap<u64, SelectionMember>,
    ordered_ids: Vec<u64>,
    member_indices: std::collections::HashMap<u64, usize>,
    frames: Vec<CoveSelectionFrame>,
    geometry_revision: Option<u64>,
    pub anchor: Option<TextSelectionPosition>,
    pub focus: Option<TextSelectionPosition>,
    /// Opens the root's context menu at a window-space point.
    pub request_menu: Option<Rc<dyn Fn(Point)>>,
    /// Optional hooks wired by the host so the selection toolbar can offer Look
    /// Up and Share.
    pub lookup: Option<Rc<dyn Fn(String)>>,
    pub share: Option<Rc<dyn Fn(String)>>,
}

impl Default for TextSelectionScope {
    fn default() -> Self {
        Self::new()
    }
}

impl TextSelectionScope {
    pub fn new() -> Self {
        Self {
            members: std::collections::HashMap::new(),
            ordered_ids: Vec::new(),
            member_indices: std::collections::HashMap::new(),
            frames: Vec::new(),
            geometry_revision: None,
            anchor: None,
            focus: None,
            request_menu: None,
            lookup: None,
            share: None,
        }
    }

    pub fn key() -> Rc<CompositionLocal<Option<TextSelectionHandle>>> {
        thread_local! {
            static KEY: Rc<CompositionLocal<Option<TextSelectionHandle>>> =
                Rc::new(CompositionLocal::constant(None));
        }
        KEY.with(|key| key.clone())
    }

    pub fn register(
        scope: &SharedTextSelectionScope,
        node: &Node,
        text: Rc<dyn Fn() -> String>,
        offset_at: Rc<dyn Fn(Point) -> usize>,
        caret_at: Rc<dyn Fn(Point) -> bool>,
    ) {
        let registration = if let Some(existing) = node.first_resource::<SelectionRegistration>() {
            existing
        } else {
            let registration = Rc::new(SelectionRegistration {
                scope: RefCell::new(Weak::new()),
            });
            node.add_resource(registration.clone());
            registration
        };
        let same_scope = registration
            .scope
            .borrow()
            .upgrade()
            .is_some_and(|existing| Rc::ptr_eq(&existing, scope));
        if !same_scope {
            Self::unregister(node);
            *registration.scope.borrow_mut() = Rc::downgrade(scope);
        }
        let id = node.id().0;
        {
            let mut scope = scope.borrow_mut();
            let replaced = scope
                .members
                .get(&id)
                .and_then(SelectionMember::node)
                .is_none_or(|existing| existing.id() != node.id());
            if replaced {
                scope.geometry_revision = None;
            }
            scope.members.insert(
                id,
                SelectionMember {
                    node: node.downgrade(),
                    text,
                    offset_at,
                    caret_at,
                },
            );
        }
    }

    pub fn unregister(node: &Node) {
        if let Some(registration) = node.first_resource::<SelectionRegistration>() {
            registration.leave(node);
        }
    }

    /// Geometry and structure share the scene's revision funnel. Capture each
    /// absolute frame once per change, never inside a sort comparator or
    /// painter.
    fn update_order(&mut self) {
        if self.geometry_revision == Some(AccessibilityRevision::geometry()) {
            return;
        }
        let previous_ids = self.ordered_ids.clone();
        let previous_span = self.selection_span();
        self.members
            .retain(|_, member| member.node().is_some_and(|node| node.parent().is_some()));
        let mut ids: Vec<u64> = Vec::with_capacity(self.members.len());
        self.frames.clear();
        self.frames.reserve(self.members.len());
        for (id, member) in &self.members {
            let Some(node) = member.node() else { continue };
            let frame = node.absolute_frame();
            self.frames.push(CoveSelectionFrame {
                x: frame.x as f64,
                y: frame.y as f64,
                width: frame.width as f64,
                height: frame.height as f64,
                member: ids.len(),
            });
            ids.push(*id);
        }
        if !self.frames.is_empty() {
            // SAFETY: the buffer is valid for `count` frames and the C helper
            // only reorders it.
            unsafe {
                cove_selection_sort(self.frames.as_mut_ptr(), self.frames.len());
            }
        }
        self.ordered_ids = self.frames.iter().map(|frame| ids[frame.member]).collect();
        self.member_indices = self
            .ordered_ids
            .iter()
            .enumerate()
            .map(|(index, id)| (*id, index))
            .collect();
        self.geometry_revision = Some(AccessibilityRevision::geometry());
        // A reflow or insertion can move paragraphs across the endpoints even
        // without a pointer event. Invalidate any highlights that changed sides.
        if previous_ids != self.ordered_ids {
            let previous = self.ranges_in(previous_span, &previous_ids);
            let current = self.ranges_in(self.selection_span(), &self.ordered_ids);
            let mut changed: std::collections::HashSet<u64> = previous.keys().copied().collect();
            changed.extend(current.keys().copied());
            for id in changed {
                if previous.get(&id) != current.get(&id)
                    && let Some(node) = self.members.get(&id).and_then(SelectionMember::node)
                {
                    node.mark_render_dirty(InvalidationSource::style_set("textSelection"));
                }
            }
        }
    }

    /// Mounted members in reading order: top to bottom, then leading to
    /// trailing.
    fn ordered(&mut self) -> Vec<(u64, SelectionMember)> {
        self.update_order();
        self.ordered_ids
            .iter()
            .filter_map(|id| self.members.get(id).map(|member| (*id, member.clone())))
            .collect()
    }

    fn selection_span(&self) -> Option<SelectionSpan> {
        let (anchor, focus) = (self.anchor?, self.focus?);
        if anchor == focus {
            return None;
        }
        let a = *self.member_indices.get(&anchor.member)?;
        let f = *self.member_indices.get(&focus.member)?;
        Some(if (a, anchor.offset) <= (f, focus.offset) {
            SelectionSpan {
                lower: a,
                upper: f,
                start: anchor.offset,
                end: focus.offset,
            }
        } else {
            SelectionSpan {
                lower: f,
                upper: a,
                start: focus.offset,
                end: anchor.offset,
            }
        })
    }

    fn range_at(
        &self,
        index: usize,
        span: Option<SelectionSpan>,
        ids: &[u64],
    ) -> Option<Range<usize>> {
        let span = span?;
        if index < span.lower || index > span.upper {
            return None;
        }
        let member = self.members.get(ids.get(index)?)?;
        let length = member.text().len();
        let lower = if index == span.lower {
            length.min(span.start)
        } else {
            0
        };
        let upper = if index == span.upper {
            length.min(span.end)
        } else {
            length
        };
        (lower < upper).then_some(lower..upper)
    }

    fn ranges_in(
        &self,
        span: Option<SelectionSpan>,
        ids: &[u64],
    ) -> std::collections::HashMap<u64, Range<usize>> {
        let Some(span) = span else {
            return std::collections::HashMap::new();
        };
        let mut result = std::collections::HashMap::new();
        for index in span.lower..=span.upper {
            if let Some(range) = self.range_at(index, Some(span), ids)
                && let Some(id) = ids.get(index)
            {
                result.insert(*id, range);
            }
        }
        result
    }

    /// Whether anything is actually selected (an anchor with no focus is a
    /// caret, not a selection).
    pub fn has_selection(&self) -> bool {
        match (self.anchor, self.focus) {
            (Some(anchor), Some(focus)) => anchor != focus,
            _ => false,
        }
    }

    pub fn select(&mut self, anchor: TextSelectionPosition, focus: TextSelectionPosition) {
        // Layout adapters report UTF-8 bytes. Keep the selection model safe
        // even when a fitted display (for example an ellipsis) has a different
        // byte length from its source, or a caller supplies an interior offset.
        let bounded = |mut position: TextSelectionPosition| {
            if let Some(member) = self.members.get(&position.member) {
                let text = member.text();
                position.offset = text
                    .grapheme_indices(true)
                    .map(|(offset, _)| offset)
                    .chain(std::iter::once(text.len()))
                    .take_while(|offset| *offset <= position.offset)
                    .last()
                    .unwrap_or(0);
            }
            position
        };
        let anchor = bounded(anchor);
        let focus = bounded(focus);
        if self.anchor == Some(anchor) && self.focus == Some(focus) {
            return;
        }
        self.update_order();
        let previous = self.selection_span();
        self.anchor = Some(anchor);
        self.focus = Some(focus);
        self.invalidate(previous, self.selection_span());
    }

    pub fn extend(&mut self, focus: TextSelectionPosition) {
        let anchor = self.anchor.unwrap_or(focus);
        self.select(anchor, focus);
    }

    pub fn clear(&mut self) {
        if self.anchor.is_none() && self.focus.is_none() {
            return;
        }
        self.update_order();
        let previous = self.selection_span();
        self.anchor = None;
        self.focus = None;
        self.invalidate(previous, None);
    }

    /// Moves the endpoint of the selection that belongs to `node`, keeping the
    /// other endpoint fixed.
    pub fn replace_range(&mut self, node: &Node, range: Range<usize>) {
        let (Some(anchor), Some(focus)) = (self.anchor, self.focus) else {
            return;
        };
        let node_id = node.id().0;
        if anchor.member == node_id {
            self.select(
                TextSelectionPosition {
                    member: node_id,
                    offset: range.start,
                },
                TextSelectionPosition {
                    member: focus.member,
                    offset: focus.offset,
                },
            );
        } else if focus.member == node_id {
            self.select(
                TextSelectionPosition {
                    member: anchor.member,
                    offset: anchor.offset,
                },
                TextSelectionPosition {
                    member: node_id,
                    offset: range.end,
                },
            );
        }
    }

    pub fn select_all(&mut self) {
        let list = self.ordered();
        let (Some(first), Some(last)) = (list.first(), list.last()) else {
            return;
        };
        self.select(
            TextSelectionPosition {
                member: first.0,
                offset: 0,
            },
            TextSelectionPosition {
                member: last.0,
                offset: last.1.text().len(),
            },
        );
    }

    /// Moves the focus by whole grapheme clusters, keeping the anchor fixed,
    /// and crosses paragraph boundaries in reading order. Returns false when
    /// there is no caret to extend from.
    pub fn extend_focus(&mut self, clusters: isize) -> bool {
        if clusters == 0 {
            return false;
        }
        let list = self.ordered();
        let Some(focus) = self.focus else {
            return false;
        };
        let Some(mut index) = list.iter().position(|(id, _)| *id == focus.member) else {
            return false;
        };
        let mut offset = focus.offset;
        let mut remaining = clusters;
        while remaining != 0 {
            let step: isize = if remaining > 0 { 1 } else { -1 };
            let text = list[index].1.text();
            if let Some(next) = Self::stepped(&text, offset, step) {
                offset = next;
            } else {
                // Step over the paragraph edge into the neighbour, when there is
                // one.
                let neighbour = index as isize + step;
                if neighbour < 0 || neighbour >= list.len() as isize {
                    break;
                }
                index = neighbour as usize;
                offset = if step > 0 {
                    0
                } else {
                    list[index].1.text().len()
                };
            }
            remaining -= step;
        }
        let target = &list[index];
        self.extend(TextSelectionPosition {
            member: target.0,
            offset,
        });
        true
    }

    /// Extends the focus to the start or end of the paragraph holding it.
    pub fn extend_focus_to_edge(&mut self, start: bool) -> bool {
        let Some(focus) = self.focus else {
            return false;
        };
        let Some(member) = self.members.get(&focus.member) else {
            return false;
        };
        let offset = if start { 0 } else { member.text().len() };
        self.extend(TextSelectionPosition {
            member: focus.member,
            offset,
        });
        true
    }

    /// The next caret offset one grapheme away, or none at the paragraph edge.
    fn stepped(text: &str, offset: usize, delta: isize) -> Option<usize> {
        if offset > text.len() || !text.is_char_boundary(offset) {
            return None;
        }
        if delta > 0 {
            let cluster = text[offset..].graphemes(true).next()?;
            Some(offset + cluster.len())
        } else {
            let cluster = text[..offset].graphemes(true).next_back()?;
            Some(offset - cluster.len())
        }
    }

    pub fn select_word(&mut self, point: Point) {
        let Some(position) = self.position(point) else {
            return;
        };
        let Some(member) = self.members.get(&position.member) else {
            return;
        };
        let word = TextBoundaries::word(&member.text(), position.offset);
        self.select(
            TextSelectionPosition {
                member: position.member,
                offset: word.start,
            },
            TextSelectionPosition {
                member: position.member,
                offset: word.end,
            },
        );
    }

    /// Selects one whole paragraph.
    pub fn select_paragraph(&mut self, node: &Node) {
        let id = node.id().0;
        let Some(member) = self.members.get(&id) else {
            return;
        };
        let length = member.text().len();
        self.select(
            TextSelectionPosition {
                member: id,
                offset: 0,
            },
            TextSelectionPosition {
                member: id,
                offset: length,
            },
        );
    }

    /// The position under a window-space point; points between or beyond
    /// paragraphs snap to the nearest paragraph edge in reading order.
    pub fn position(&mut self, point: Point) -> Option<TextSelectionPosition> {
        self.update_order();
        let mut edge: i32 = 0;
        // SAFETY: frames is a live repr(C) slice for the supplied count and edge
        // is a writable local. The synchronous C search retains neither pointer.
        let index = unsafe {
            cove_selection_find(
                self.frames.as_ptr(),
                self.frames.len(),
                point.x as f64,
                point.y as f64,
                true,
                &mut edge,
            )
        };
        if index < 0 {
            return None;
        }
        let id = *self.ordered_ids.get(index as usize)?;
        let member = self.members.get(&id)?;
        let offset = if edge < 0 {
            0
        } else if edge > 0 {
            member.text().len()
        } else {
            (member.offset_at)(point)
        };
        Some(TextSelectionPosition { member: id, offset })
    }

    /// Whether a window-space point lands on a paragraph this scope owns.
    pub fn text_under(&mut self, point: Point) -> bool {
        self.update_order();
        // The frames above answer "which paragraph comes first", not "is there
        // a glyph here": they are the members' whole node rectangles.
        self.members.values().any(|member| {
            member
                .node()
                .filter(|node| node.parent().is_some())
                .is_some_and(|_| (member.caret_at)(point))
        })
    }

    /// The selected UTF-8 range within one paragraph.
    pub fn range_for(&mut self, node: &Node) -> Option<Range<usize>> {
        if !self.has_selection() {
            return None;
        }
        self.update_order();
        let index = *self.member_indices.get(&node.id().0)?;
        let ids = self.ordered_ids.clone();
        self.range_at(index, self.selection_span(), &ids)
    }

    /// Paragraphs on one row (table cells) join with tabs, rows with newlines.
    pub fn selected_text(&mut self) -> String {
        if !self.has_selection() {
            return String::new();
        }
        self.update_order();
        let Some(span) = self.selection_span() else {
            return String::new();
        };
        let ids = self.ordered_ids.clone();
        let mut result = String::new();
        let mut previous_y: Option<f64> = None;
        for index in span.lower..=span.upper {
            let Some(member) = self.members.get(&ids[index]) else {
                continue;
            };
            let Some(range) = self.range_at(index, Some(span), &ids) else {
                continue;
            };
            let text = member.text();
            let part = &text[range.clone()];
            if let Some(previous_y) = previous_y {
                result.push_str(if (self.frames[index].y - previous_y).abs() < 1.0 {
                    "\t"
                } else {
                    "\n"
                });
            }
            result.push_str(part);
            previous_y = Some(self.frames[index].y);
        }
        result
    }

    pub fn copy(&mut self) {
        if !self.has_selection() {
            return;
        }
        let text = self.selected_text();
        cove_core::scene::ClipboardHolder::write(&text);
    }

    /// Interior paragraphs shared by both spans keep their cached drawing.
    /// Only entering/leaving paragraphs and the four possible endpoints change.
    fn invalidate(&mut self, previous: Option<SelectionSpan>, current: Option<SelectionSpan>) {
        let mut endpoints: std::collections::HashSet<usize> = std::collections::HashSet::new();
        for value in [
            previous.map(|span| span.lower),
            previous.map(|span| span.upper),
            current.map(|span| span.lower),
            current.map(|span| span.upper),
        ]
        .into_iter()
        .flatten()
        {
            endpoints.insert(value);
        }
        for (span, other) in [(previous, current), (current, previous)] {
            let Some(span) = span else { continue };
            let low_end = (span.upper + 1).min(other.map_or(span.upper + 1, |span| span.lower));
            if span.lower < low_end {
                for index in span.lower..low_end {
                    if !endpoints.contains(&index) {
                        self.mark_at(index);
                    }
                }
            }
            if let Some(other) = other {
                let high_start = span.lower.max(other.upper + 1);
                if high_start <= span.upper {
                    for index in high_start..=span.upper {
                        if !endpoints.contains(&index) {
                            self.mark_at(index);
                        }
                    }
                }
            }
        }
        let ids = self.ordered_ids.clone();
        for index in endpoints {
            if self.range_at(index, previous, &ids) != self.range_at(index, current, &ids) {
                self.mark_at(index);
            }
        }
    }

    fn mark_at(&self, index: usize) {
        if let Some(node) = self
            .ordered_ids
            .get(index)
            .and_then(|id| self.members.get(id))
            .and_then(SelectionMember::node)
        {
            node.mark_render_dirty(InvalidationSource::style_set("textSelection"));
        }
    }
}

pub type SharedTextSelectionScope = Rc<RefCell<TextSelectionScope>>;

/// Composition-local wrapper. The ambient value is compared by identity — the
/// scope itself mutates as members register and the selection moves, which is
/// not a change of the environment.
#[derive(Clone)]
pub struct TextSelectionHandle(pub SharedTextSelectionScope);
impl PartialEq for TextSelectionHandle {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// One endpoint of a plain-text selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextSelectionEndpoint {
    Start,
    End,
}

/// Chrome shared by every selectable text primitive: the two draggable
/// endpoint handles and the magnifier that follows a finger while it refines a
/// selection.
pub struct TextSelectionChrome;

impl TextSelectionChrome {
    /// Radius of the round endpoint handles.
    pub const HANDLE_RADIUS: f32 = 6.0;
    /// A press within this distance of a handle centre grabs it.
    pub const HIT_SLOP: f32 = 16.0;
    pub const LOUPE_RADIUS: f32 = 44.0;
    pub const LOUPE_SCALE: f32 = 1.8;
    /// Gap between the lens and the finger holding it.
    const LOUPE_LIFT: f32 = 28.0;

    /// The start handle hangs below its line and the end handle above, so a
    /// one-line selection keeps them apart.
    pub fn handle_center(caret: Rect, is_start: bool) -> Point {
        Point::new(
            caret.x + caret.width / 2.0,
            if is_start {
                caret.y + caret.height + Self::HANDLE_RADIUS
            } else {
                caret.y - Self::HANDLE_RADIUS
            },
        )
    }

    /// The endpoint under a node-local point, or none when the press missed
    /// both.
    pub fn endpoint(
        local: Point,
        start: Option<Rect>,
        end: Option<Rect>,
    ) -> Option<TextSelectionEndpoint> {
        if let Some(start) = start
            && Self::distance(local, Self::handle_center(start, true)) <= Self::HIT_SLOP
        {
            return Some(TextSelectionEndpoint::Start);
        }
        if let Some(end) = end
            && Self::distance(local, Self::handle_center(end, false)) <= Self::HIT_SLOP
        {
            return Some(TextSelectionEndpoint::End);
        }
        None
    }

    pub fn draw_handles(
        list: &mut DrawList,
        origin: Point,
        start: Option<Rect>,
        end: Option<Rect>,
        fill: Color,
        border: Color,
    ) {
        for (caret, is_start) in [(start, true), (end, false)] {
            let Some(caret) = caret else { continue };
            let center = Self::handle_center(caret, is_start);
            let rect = Rect::new(
                origin.x + center.x - Self::HANDLE_RADIUS,
                origin.y + center.y - Self::HANDLE_RADIUS,
                Self::HANDLE_RADIUS * 2.0,
                Self::HANDLE_RADIUS * 2.0,
            );
            let _ = list.add_rounded_rect(rect, Self::HANDLE_RADIUS, fill);
            let _ = list.add_rounded_rect_stroke(rect, Self::HANDLE_RADIUS, 1.0, border);
        }
    }

    /// Circular magnifier above the finger. `draw_text` receives the draw list
    /// and the vertical shift to add to the text block origin, which is what
    /// puts the character under the finger at the centre of the lens once
    /// scaled.
    pub fn draw_loupe(
        list: &mut DrawList,
        finger: Point,
        surface: Color,
        border: Color,
        draw_text: Rc<dyn Fn(&mut DrawList, f32)>,
    ) {
        let radius = Self::LOUPE_RADIUS;
        let center = Point::new(finger.x, finger.y - radius - Self::LOUPE_LIFT);
        let rect = Rect::new(
            center.x - radius,
            center.y - radius,
            radius * 2.0,
            radius * 2.0,
        );
        let _ = list.add_rounded_rect(rect, radius, surface);
        let clip = Clip {
            rect,
            mask: Some(RoundedRect::new(rect, radius)),
        };
        list.push_clip(clip);
        let shift = (center.y - finger.y) / Self::LOUPE_SCALE;
        let painter: Painter = Rc::new(move |temp, _| draw_text(temp, shift));
        paint_scaled(&painter, list, finger, Self::LOUPE_SCALE);
        list.pop_clip(clip);
        let _ = list.add_rounded_rect_stroke(rect, radius, 1.5, border);
        // Stem from the lens down to the finger.
        let stem: f32 = 3.0;
        let stem_top = center.y + radius;
        let _ = list.add_rounded_rect(
            Rect::new(
                center.x - stem / 2.0,
                stem_top,
                stem,
                (finger.y - stem_top).max(0.0),
            ),
            stem / 2.0,
            border,
        );
    }

    fn distance(lhs: Point, rhs: Point) -> f32 {
        ((lhs.x - rhs.x).powi(2) + (lhs.y - rhs.y).powi(2)).sqrt()
    }

    /// True when an ancestor already owns the pointer gesture — a button, list
    /// row, table cell or workspace chrome.
    ///
    /// Two things count. A normal-priority pointer route (a control), and any
    /// ancestor that declares a cursor of its own: a workspace tab or split
    /// divider registers at capture priority yet still says the pointer means
    /// "hand" or "resize", and its title must not turn that into a caret. The
    /// workspace canvas declares no cursor, so selectable text inside panels
    /// keeps its caret. A scroll view's route exists only to scroll, so it does
    /// not count.
    pub fn is_inside_control(node: &Node) -> bool {
        let Some(registry) = InteractionRegistryHolder::current() else {
            return false;
        };
        let mut ancestor = node.parent();
        while let Some(current) = ancestor {
            let chrome_cursor = current
                .cursor()
                .is_some_and(|cursor| cursor != SystemCursor::Ibeam);
            if chrome_cursor {
                return true;
            }
            if let Some(route) = registry.handlers(&current).pointer_route
                && route.priority <= InputRoutingPriority::NORMAL
                && route.role != cove_core::scene::InputHandlerRole::Scroll
            {
                return true;
            }
            ancestor = current.parent();
        }
        false
    }
}

pub struct TextBoundaries;

impl TextBoundaries {
    /// The word (or, outside words, the single character) around a UTF-8
    /// offset.
    pub fn word(text: &str, offset: usize) -> Range<usize> {
        let mut word_start: Option<usize> = None;
        let mut word_end = 0usize;
        for (byte, character) in text.char_indices() {
            let is_word = character.is_alphanumeric()
                || character == '_'
                || character == '\''
                || character == '\u{2019}'
                || is_combining_mark(character);
            if is_word {
                word_start.get_or_insert(byte);
                word_end = byte + character.len_utf8();
            } else if let Some(start) = word_start.take() {
                if offset >= start && offset < word_end {
                    return start..word_end;
                }
                if offset == word_end && word_end == text.len() {
                    return start..word_end;
                }
            }
        }
        if let Some(start) = word_start {
            if offset >= start && offset < word_end {
                return start..word_end;
            }
            if offset == word_end && word_end == text.len() {
                return start..word_end;
            }
        }
        let mut lower = 0usize;
        for grapheme in text.graphemes(true) {
            let upper = lower + grapheme.len();
            if offset >= lower && (offset < upper || offset == upper && upper == text.len()) {
                return lower..upper;
            }
            lower = upper;
        }
        let end = offset.min(text.len());
        end..end
    }
}

fn is_combining_mark(character: char) -> bool {
    matches!(character as u32,
        0x0300..=0x036F
            | 0x1AB0..=0x1AFF
            | 0x1DC0..=0x1DFF
            | 0x20D0..=0x20FF
            | 0xFE20..=0xFE2F)
}

/// Owns the selection for its content. The host's context-menu overlay is not
/// yet ported; the scope, regions and actions are.
pub struct TextSelectionRoot {
    content: cove_composition::AnyView,
    scope: SharedTextSelectionScope,
}

impl TextSelectionRoot {
    pub fn new(content: cove_composition::AnyView) -> Self {
        Self {
            content,
            scope: Rc::new(RefCell::new(TextSelectionScope::new())),
        }
    }

    pub fn scope(&self) -> SharedTextSelectionScope {
        self.scope.clone()
    }
}

struct MountedSelectionScope(SharedTextSelectionScope);
impl NodeResource for MountedSelectionScope {
    fn mount(&self, _node: &Node) {}
    fn unmount(&self, _node: &Node) {}
    fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }
}

impl cove_composition::PrimitiveView for TextSelectionRoot {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.add_resource(Rc::new(MountedSelectionScope(self.scope.clone())));
        node
    }
    fn update_node(&self, node: &Node) {
        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        let scope = node
            .first_resource::<MountedSelectionScope>()
            .map_or_else(|| self.scope.clone(), |mounted| mounted.0.clone());
        registry.set_pointer(
            node,
            InputHandlerRoute::overlay(),
            Rc::new(move |event, phase, _| {
                if phase != PointerPhase::Down || event.button != cove_platform::MouseButton::Left {
                    return EventResult::Ignored;
                }

                let point = Point::new(event.x, event.y);
                let outside_selection = {
                    let mut scope = scope.borrow_mut();
                    scope.has_selection() && !scope.text_under(point)
                };
                if outside_selection {
                    scope.borrow_mut().clear();
                }
                EventResult::Ignored
            }),
        );
    }
    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }
    fn children(&self, node: &Node) -> Vec<cove_composition::AnyView> {
        use cove_composition::ViewExt;
        let scope = node
            .first_resource::<MountedSelectionScope>()
            .map_or_else(|| self.scope.clone(), |mounted| mounted.0.clone());
        vec![cove_composition::AnyView::new(
            self.content
                .clone()
                .composition_local(TextSelectionScope::key(), Some(TextSelectionHandle(scope))),
        )]
    }
}

/// Publishes the host's Look Up / Share actions onto the enclosing selection
/// scope.
pub struct TextSelectionActions {
    pub lookup: Option<Rc<dyn Fn(String)>>,
    pub share: Option<Rc<dyn Fn(String)>>,
    pub content: cove_composition::AnyView,
}

impl cove_composition::PrimitiveView for TextSelectionActions {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &Node) {}
    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }
    fn children(&self, node: &Node) -> Vec<cove_composition::AnyView> {
        if let Some(scope) = node.composition_value(&TextSelectionScope::key()) {
            let mut scope = scope.0.borrow_mut();
            scope.lookup = self.lookup.clone();
            scope.share = self.share.clone();
        }
        vec![self.content.clone()]
    }
}
cove_composition::impl_primitive_view!(TextSelectionRoot);
cove_composition::impl_primitive_view!(TextSelectionActions);

/// `.textSelection(_:)` and `.textSelectionActions(lookup:share:)` for the
/// composition layer.
pub trait TextSelectionViewExt: cove_composition::View + Sized + 'static {
    fn text_selection(self, enabled: bool) -> cove_composition::AnyView {
        use cove_composition::ViewExt;
        if enabled {
            cove_composition::AnyView::new(TextSelectionRoot::new(cove_composition::AnyView::new(
                self,
            )))
        } else {
            cove_composition::AnyView::new(
                self.composition_local(TextSelectionScope::key(), None::<TextSelectionHandle>),
            )
        }
    }

    fn text_selection_actions(
        self,
        lookup: Option<Rc<dyn Fn(String)>>,
        share: Option<Rc<dyn Fn(String)>>,
    ) -> cove_composition::AnyView {
        cove_composition::AnyView::new(TextSelectionActions {
            lookup,
            share,
            content: cove_composition::AnyView::new(self),
        })
    }
}

impl<T: cove_composition::View + Sized + 'static> TextSelectionViewExt for T {}

/// Keeps the imported flow type reachable for callers that build selection
/// surfaces, mirroring the Swift file's extension point.
pub type SelectionFlow = RichTextFlow;
pub type SelectionBuffer = TextBuffer;
pub type SelectionFocusChain = FocusChainHolder;
pub type SelectionTextEnvironment = TextEnvironmentHolder;

/// Convenience for tests and hosts that need the current focus visibility.
pub fn focus_is_visible() -> bool {
    FocusChainHolder::current().is_some_and(|chain| chain.is_focus_visible())
}

/// Whether a window-space point lands on one of a text block's painted lines.
///
/// A caret belongs to glyphs. A member's node frame covers its padding and
/// whatever the parent stretched it to, so testing the frame let a one-line
/// label claim the text cursor across a whole dock panel and steal it from the
/// control underneath. Each line contributes its own `(top, height, width)`
/// box, measured from the block's content origin.
pub(crate) fn point_on_text_line(
    point: Point,
    origin: Point,
    lines: impl Iterator<Item = (f32, f32, f32)>,
) -> bool {
    lines
        .filter(|(_, height, width)| *height > 0.0 && *width > 0.0)
        .any(|(top, height, width)| {
            let local_y = point.y - origin.y;
            let local_x = point.x - origin.x;
            local_y >= top
                && local_y < top + height
                && local_x >= 0.0
                && local_x <= width + cove_core::TEXT_FIT_TOLERANCE
        })
}
