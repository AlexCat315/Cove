//! Translation of Primitives/Text/Code/GutterSession.swift.
//!
//! The node-owned gutter session: which lines have breakpoints, markers and
//! folds, which controls are currently painted, and the keyboard rules that act
//! on them. Reading state lives on the node so a parent recompose mid-drag keeps
//! the pointer transaction.
//!
//! Behaviour checklist:
//! - markers are grouped by line
//! - editing rebases authored breakpoint lines through the line mapping; a
//!   cursor-less edit that reports nothing clears them only when the contents
//!   actually changed
//! - F9 toggles a breakpoint, Shift+F9 toggles its enabled state
//! - Alt+Cmd/Ctrl+Left/Right toggles the innermost fold containing the caret
//! - Escape returns focus to the editor
//! - Backspace/Delete at a folded boundary first reveals the hidden text
//!   rather than silently deleting an invisible character

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::rc::Rc;

use crate::primitives::text::code::folding_options::{
    TextEditorFolding, TextFoldRegion, TextLineRebase,
};
use crate::primitives::text::code::gutter_geometry::TextGutterControl;
use crate::primitives::text::code::gutter_options::{
    TextBreakpoint, TextBreakpointState, TextEditorGutter, TextGutterMarker,
};
use crate::primitives::text::text_buffer::TextBuffer;
use cove_composition::Scancode;
use cove_core::Binding;
use cove_core::scene::{FocusChainHolder, Node};
use cove_platform::KeyEvent;

/// Language-service fold regions plus the caller's collapsed set.
#[derive(Default)]
pub struct TextFoldSession {
    regions: RefCell<BTreeMap<usize, TextFoldRegion>>,
    collapsed: RefCell<BTreeSet<usize>>,
    binding: RefCell<Option<Binding<BTreeSet<usize>>>>,
    authored: RefCell<Vec<TextFoldRegion>>,
    buffer: RefCell<Option<TextBuffer>>,
    previous_bound: RefCell<BTreeSet<usize>>,
    hidden_lines: RefCell<Vec<Range<usize>>>,
}

impl TextFoldSession {
    pub fn new() -> Self {
        Self::default()
    }

    /// Regions keyed by their header line, as the gutter addresses them.
    pub fn regions(&self) -> BTreeMap<usize, TextFoldRegion> {
        self.regions.borrow().clone()
    }

    pub fn region(&self, header: usize) -> Option<TextFoldRegion> {
        self.regions.borrow().get(&header).copied()
    }

    pub fn collapsed(&self) -> BTreeSet<usize> {
        match &*self.binding.borrow() {
            Some(binding) => binding.get(),
            None => self.collapsed.borrow().clone(),
        }
    }

    pub fn is_collapsed(&self, header: usize) -> bool {
        self.collapsed().contains(&header)
    }

    /// The line ranges a collapsed fold hides, excluding each header. Adjacent
    /// or overlapping ranges merge, so the row index consumes one projection.
    pub fn hidden_lines(&self) -> Vec<Range<usize>> {
        self.hidden_lines.borrow().clone()
    }

    /// Recomputes the hidden-line projection from the collapsed headers.
    fn update_projection(&self) {
        let collapsed = self.collapsed();
        let regions = self.regions.borrow();
        let mut projected: Vec<Range<usize>> = Vec::new();
        for header in collapsed.iter().copied() {
            let Some(region) = regions.get(&header) else {
                continue;
            };
            let range = (header + 1)..(region.last_line + 1);
            match projected.last_mut() {
                Some(last) if range.start <= last.end => {
                    last.end = last.end.max(range.end);
                }
                _ => projected.push(range),
            }
        }
        *self.hidden_lines.borrow_mut() = projected;
    }

    /// Installs the authored configuration. A missing bound set keeps the
    /// node-local session.
    ///
    /// Invalid (single-line), out-of-document and crossing regions are dropped;
    /// properly nested regions keep their own header.
    pub fn synchronize(&self, folding: &TextEditorFolding, buffer: &TextBuffer) {
        *self.binding.borrow_mut() = folding.collapsed_lines.clone();
        let mut collapsed = self.collapsed();
        let previous_buffer = self.buffer.borrow().clone();
        if let Some(previous) = &previous_buffer
            && previous != buffer
        {
            if let Some(change) = TextLineRebase::new(previous, buffer) {
                if folding.collapsed_lines.is_none() || collapsed == *self.previous_bound.borrow() {
                    collapsed = collapsed
                        .into_iter()
                        .filter_map(|line| change.line(line))
                        .collect();
                }
                let regions = self
                    .regions
                    .borrow()
                    .values()
                    .filter_map(|region| {
                        let start = change.line(region.first_line)?;
                        let end = change.line(region.last_line)?;
                        (start < end).then_some((start, TextFoldRegion::new(start, end)))
                    })
                    .collect();
                *self.regions.borrow_mut() = regions;
            } else if previous.string_value() != buffer.string_value() {
                collapsed.clear();
                self.regions.borrow_mut().clear();
            }
        }
        let line_count = buffer.line_count();
        if previous_buffer.is_none() || *self.authored.borrow() != folding.regions {
            let mut regions = BTreeMap::new();
            let mut ancestors: Vec<TextFoldRegion> = Vec::new();
            let mut sorted = folding.regions.clone();
            sorted.sort_by(|left, right| {
                left.first_line
                    .cmp(&right.first_line)
                    .then_with(|| right.last_line.cmp(&left.last_line))
            });
            for region in sorted {
                if region.first_line >= region.last_line
                    || region.last_line >= line_count
                    || regions.contains_key(&region.first_line)
                {
                    continue;
                }
                while ancestors
                    .last()
                    .is_some_and(|parent| region.first_line > parent.last_line)
                {
                    ancestors.pop();
                }
                if ancestors
                    .last()
                    .is_some_and(|parent| region.last_line > parent.last_line)
                {
                    continue;
                }
                regions.insert(region.first_line, region);
                ancestors.push(region);
            }
            *self.regions.borrow_mut() = regions;
            *self.authored.borrow_mut() = folding.regions.clone();
        }
        if folding.is_ready {
            collapsed.retain(|line| self.regions.borrow().contains_key(line));
        }
        if let Some(binding) = &*self.binding.borrow()
            && binding.get() != collapsed
        {
            binding.set(collapsed.clone());
        }
        *self.collapsed.borrow_mut() = collapsed.clone();
        *self.previous_bound.borrow_mut() = collapsed;
        *self.buffer.borrow_mut() = Some(buffer.clone());
        self.update_projection();
    }

    pub fn toggle(&self, header: usize) -> bool {
        if !self.regions.borrow().contains_key(&header) {
            return false;
        }
        let mut next = self.collapsed();
        if !next.remove(&header) {
            next.insert(header);
        }
        match &*self.binding.borrow() {
            Some(binding) => binding.set(next),
            None => *self.collapsed.borrow_mut() = next,
        }
        self.update_projection();
        true
    }

    /// Reveals a folded line so an edit at its boundary is visible first.
    pub fn reveal(&self, line: usize) -> bool {
        let mut changed = false;
        let mut next = self.collapsed();
        let headers: Vec<usize> = self
            .regions
            .borrow()
            .values()
            .filter(|region| region.contains(line) && region.first_line != line)
            .map(|region| region.first_line)
            .collect();
        for header in headers {
            if next.remove(&header) {
                changed = true;
            }
        }
        if changed {
            match &*self.binding.borrow() {
                Some(binding) => binding.set(next),
                None => *self.collapsed.borrow_mut() = next,
            }
            self.update_projection();
        }
        changed
    }
}

const F9: u32 = Scancode::F9;

/// Node-owned gutter session.
pub struct TextGutterSession {
    node: RefCell<Option<Node>>,
    gutter: RefCell<TextEditorGutter>,
    folds: TextFoldSession,
    controls: RefCell<Vec<TextGutterControl>>,
    markers_by_line: RefCell<BTreeMap<usize, Vec<TextGutterMarker>>>,
    revision: Cell<u64>,
    changed: cove_core::Signal<()>,
    previous_breakpoints: RefCell<Vec<TextBreakpoint>>,
    buffer: RefCell<Option<TextBuffer>>,
    pub selected_control: RefCell<Option<String>>,
    subscribers: RefCell<Vec<Rc<dyn Fn()>>>,
}

impl TextGutterSession {
    pub fn new() -> Self {
        Self {
            node: RefCell::new(None),
            gutter: RefCell::new(TextEditorGutter::new()),
            folds: TextFoldSession::new(),
            controls: RefCell::new(Vec::new()),
            markers_by_line: RefCell::new(BTreeMap::new()),
            revision: Cell::new(0),
            changed: cove_core::Signal::detached(()),
            previous_breakpoints: RefCell::new(Vec::new()),
            buffer: RefCell::new(None),
            selected_control: RefCell::new(None),
            subscribers: RefCell::new(Vec::new()),
        }
    }

    pub fn mount(&self, node: Node) {
        *self.node.borrow_mut() = Some(node);
    }

    pub fn unmount(&self) {
        *self.node.borrow_mut() = None;
        self.controls.borrow_mut().clear();
        self.markers_by_line.borrow_mut().clear();
    }

    pub fn folds(&self) -> &TextFoldSession {
        &self.folds
    }

    pub fn controls(&self) -> Vec<TextGutterControl> {
        self.changed.track();
        self.controls.borrow().clone()
    }

    /// The painted controls actually changed.
    pub fn update(&self, controls: Vec<TextGutterControl>) {
        if *self.controls.borrow() == controls {
            return;
        }
        *self.controls.borrow_mut() = controls;
        self.publish();
    }

    pub fn markers_by_line(&self) -> BTreeMap<usize, Vec<TextGutterMarker>> {
        self.markers_by_line.borrow().clone()
    }

    pub fn revision(&self) -> u64 {
        self.revision.get()
    }

    /// Installs the field's gutter and folding configuration, groups markers by
    /// line and rebases any authored breakpoint lines.
    pub fn configure(
        &self,
        gutter: TextEditorGutter,
        folding: &TextEditorFolding,
        buffer: &TextBuffer,
    ) {
        let mut grouped: BTreeMap<usize, Vec<TextGutterMarker>> = BTreeMap::new();
        for marker in &gutter.markers {
            grouped.entry(marker.line).or_default().push(marker.clone());
        }
        *self.markers_by_line.borrow_mut() = grouped;
        *self.gutter.borrow_mut() = gutter;
        self.synchronize_source(buffer);
        self.folds.synchronize(folding, buffer);
        self.publish();
    }

    fn synchronize_source(&self, next: &TextBuffer) {
        let previous = self.previous_breakpoints.borrow().clone();
        let current = self
            .gutter
            .borrow()
            .breakpoints
            .as_ref()
            .map(|binding| binding.get())
            .unwrap_or_default();
        let line_count = next.line_count();
        let previous_buffer = self.buffer.borrow().clone();
        let rebased: Vec<TextBreakpoint> = match previous_buffer.as_ref() {
            Some(buffer) if buffer != next => match TextLineRebase::new(buffer, next) {
                Some(change) => previous
                    .iter()
                    .filter_map(|point| {
                        let line = change.line(point.line)?;
                        (line < line_count).then_some(TextBreakpoint {
                            line,
                            state: point.state,
                        })
                    })
                    .collect(),
                None if buffer.string_value() == next.string_value() => previous.clone(),
                None => Vec::new(),
            },
            _ => previous
                .iter()
                .filter(|point| point.line < line_count)
                .copied()
                .collect(),
        };
        if let Some(binding) = self.gutter.borrow().breakpoints.clone() {
            let unchanged = current == previous;
            if unchanged && current != rebased {
                binding.set(rebased.clone());
                if let Some(handler) = &self.gutter.borrow().on_breakpoints_change {
                    handler(rebased.clone());
                }
            }
        }
        *self.previous_breakpoints.borrow_mut() = self
            .gutter
            .borrow()
            .breakpoints
            .as_ref()
            .map(|binding| binding.get())
            .unwrap_or_default();
        *self.buffer.borrow_mut() = Some(next.clone());
    }

    fn publish(&self) {
        self.changed.notify();
        self.revision.set(self.revision.get().wrapping_add(1));
        for handler in self.subscribers.borrow().iter() {
            handler();
        }
    }

    pub fn subscribe(&self, handler: Rc<dyn Fn()>) {
        self.subscribers.borrow_mut().push(handler);
    }

    /// The gutter accepts input only where the field does.
    pub fn is_available(
        &self,
        disabled: bool,
        secure: bool,
        shows_line_numbers: bool,
        vertical: bool,
    ) -> bool {
        let node_accepts = self
            .node
            .borrow()
            .as_ref()
            .map(|node| node.accepts_subtree_input())
            .unwrap_or(false);
        node_accepts && !disabled && shows_line_numbers && vertical && !secure
    }

    pub fn toggle_breakpoint(&self, line: usize) {
        let Some(binding) = self.gutter.borrow().breakpoints.clone() else {
            return;
        };
        let mut next = binding.get();
        if next.iter().any(|point| point.line == line) {
            next.retain(|point| point.line != line);
        } else {
            next.push(TextBreakpoint::new(line));
        }
        next.sort_by_key(|point| point.line);
        binding.set(next.clone());
        if let Some(handler) = &self.gutter.borrow().on_breakpoints_change {
            handler(next);
        }
        self.return_to_editor();
        self.mark_dirty();
        self.publish();
    }

    pub fn toggle_breakpoint_enabled(&self, line: usize) {
        let Some(binding) = self.gutter.borrow().breakpoints.clone() else {
            return;
        };
        let mut next = binding.get();
        let Some(point) = next.iter_mut().find(|point| point.line == line) else {
            return;
        };
        point.state = if point.state == TextBreakpointState::Disabled {
            TextBreakpointState::Enabled
        } else {
            TextBreakpointState::Disabled
        };
        binding.set(next.clone());
        if let Some(handler) = &self.gutter.borrow().on_breakpoints_change {
            handler(next);
        }
        self.return_to_editor();
        self.mark_dirty();
        self.publish();
    }

    pub fn activate_marker(&self, id: &str) {
        let marker = self
            .gutter
            .borrow()
            .markers
            .iter()
            .find(|marker| marker.id == id)
            .cloned();
        if let Some(marker) = marker {
            (marker.action)();
        }
    }

    /// Toggles the fold whose header is `line`. A caret inside the region moves
    /// to the header's end so it does not land in hidden text.
    ///
    /// Returns the line the caret must move to when the collapse hid it.
    pub fn toggle_fold(&self, line: usize) -> Option<usize> {
        let region = self.folds.region(line)?;
        if !self.folds.toggle(line) {
            return None;
        }
        if self.folds.is_collapsed(line) {
            Some(region.first_line)
        } else {
            None
        }
    }

    pub fn return_to_editor(&self) {
        let Some(node) = self.node.borrow().clone() else {
            return;
        };
        if let Some(chain) = FocusChainHolder::current() {
            let visible = chain.is_focus_visible();
            chain.focus(Some(&node), visible);
        }
    }

    fn mark_dirty(&self) {
        if let Some(node) = self.node.borrow().as_ref() {
            node.mark_render_dirty(cove_core::scene::InvalidationSource::style_set("gutter"));
        }
    }

    /// The gutter's keyboard contract. Returns whether the event was consumed.
    pub fn handle_key(
        &self,
        event: &KeyEvent,
        cursor_line: usize,
        character_count: usize,
        read_only: bool,
    ) -> bool {
        let breakpoints = self.gutter.borrow().breakpoints.is_some();

        if event.scancode == F9
            && !event.modifiers.has_gui()
            && !event.modifiers.has_ctrl()
            && !event.modifiers.has_alt()
            && breakpoints
        {
            if !event.modifiers.has_shift() {
                self.toggle_breakpoint(cursor_line);
            } else {
                self.toggle_breakpoint_enabled(cursor_line);
            }
            return true;
        }

        let arrow =
            event.scancode == Scancode::ARROW_LEFT || event.scancode == Scancode::ARROW_RIGHT;
        if event.modifiers.has_alt()
            && (event.modifiers.has_gui() || event.modifiers.has_ctrl())
            && arrow
        {
            let mut candidates: Vec<TextFoldRegion> = self
                .folds
                .regions()
                .values()
                .copied()
                .filter(|region| region.contains(cursor_line))
                .collect();
            candidates.sort_by_key(|region| region.last_line - region.first_line);
            let Some(region) = candidates.first().copied() else {
                return false;
            };
            let wants_collapsed = event.scancode == Scancode::ARROW_LEFT;
            if self.folds.is_collapsed(region.first_line) == wants_collapsed {
                return true;
            }
            self.toggle_fold(region.first_line);
            return true;
        }

        if event.scancode == Scancode::ESCAPE
            && FocusChainHolder::current()
                .and_then(|chain| chain.focused())
                .map(|focused| Some(&focused) != self.node.borrow().as_ref())
                .unwrap_or(true)
        {
            self.return_to_editor();
            return true;
        }

        // Removing a newline at a folded boundary first reveals what would be
        // edited, rather than silently deleting an invisible closing character.
        if !read_only && (event.scancode == Scancode::BACKSPACE || event.scancode == 76) {
            let target = if event.scancode == Scancode::BACKSPACE {
                cursor_line.saturating_sub(1)
            } else {
                cursor_line + 1
            };
            let target = target.min(character_count);
            if self.folds.reveal(target) {
                self.mark_dirty();
                self.publish();
                return true;
            }
        }

        false
    }
}

impl Default for TextGutterSession {
    fn default() -> Self {
        Self::new()
    }
}

impl cove_core::scene::NodeResource for TextGutterSession {
    fn mount(&self, node: &Node) {
        TextGutterSession::mount(self, node.clone());
    }
    fn unmount(&self, _node: &Node) {
        TextGutterSession::unmount(self);
    }
    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

impl Drop for TextGutterSession {
    fn drop(&mut self) {
        self.changed.dispose();
    }
}
