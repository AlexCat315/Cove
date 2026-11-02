//! Translation of `TextCompletionSession` in Primitives/Controls/CompletionPopover.swift.
//!
//! Debounces provider requests while the caret moves through an identifier,
//! filters the reply by prefix, shows the nonmodal list in a portal and applies
//! the accepted item: the replacement, its additional edits and any snippet.
//! Keyboard focus stays in the editor throughout.
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};

use cove_core::scene::{
    AnimatorScheduler, AnyAnimationController, FocusChainHolder, Node, NodeResource, NodeWeak,
};
use cove_core::{Point, Rect};
use cove_platform::KeyEvent;

use super::snippet::{TextSnippet, TextSnippetSession};
use super::text_field::{FieldState, TextField};
use crate::AnyView;
use crate::primitives::controls::completion_popover::CompletionPopover;
use crate::primitives::overlays::portal::{
    PortalPlacementMode, PortalResource, PortalStoreEnvironment, PortalStoreHolder,
};
use crate::primitives::text::text_completion_rebase::{
    TextCompletionEdit, TextCompletionItem, TextCompletionRebase, TextCompletionRequest,
    TextInsertFormat,
};

/// Typing pauses this long before the provider is asked.
const DEBOUNCE: f64 = 0.3;
const POPOVER_WIDTH: f32 = 380.0;
const ANCHOR_GAP: f32 = 3.0;

/// One field's completion state.
pub struct TextCompletionSession {
    me: RefCell<Weak<Self>>,
    node: RefCell<Option<NodeWeak>>,
    state: RefCell<Option<Rc<RefCell<FieldState>>>>,
    field: RefCell<Option<TextField>>,
    portal: Rc<PortalResource>,
    snippet: Rc<TextSnippetSession>,
    request: RefCell<Option<TextCompletionRequest>>,
    generation: Cell<u64>,
    elapsed: Cell<f64>,
    candidates: RefCell<Vec<TextCompletionItem>>,
    /// Items already sent to the resolver in this session.
    resolving: RefCell<HashSet<String>>,
    items: RefCell<Vec<TextCompletionItem>>,
    selected_index: Cell<usize>,
    anchor: Cell<Rect>,
    is_finished: Cell<bool>,
}

impl TextCompletionSession {
    pub fn new(portal: Rc<PortalResource>, snippet: Rc<TextSnippetSession>) -> Rc<Self> {
        let session = Rc::new(Self {
            me: RefCell::new(Weak::new()),
            node: RefCell::new(None),
            state: RefCell::new(None),
            field: RefCell::new(None),
            portal,
            snippet,
            request: RefCell::new(None),
            generation: Cell::new(0),
            elapsed: Cell::new(0.0),
            candidates: RefCell::new(Vec::new()),
            resolving: RefCell::new(HashSet::new()),
            items: RefCell::new(Vec::new()),
            selected_index: Cell::new(0),
            anchor: Cell::new(Rect::ZERO),
            is_finished: Cell::new(true),
        });
        *session.me.borrow_mut() = Rc::downgrade(&session);
        session
    }

    fn owner(&self) -> Option<Node> {
        self.node.borrow().as_ref().and_then(Node::upgrade)
    }

    /// Items currently offered, after filtering.
    pub fn items(&self) -> Vec<TextCompletionItem> {
        self.items.borrow().clone()
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index.get()
    }

    /// Takes the latest field and drops anything the new props invalidate.
    pub fn configure(&self, field: TextField, state: Rc<RefCell<FieldState>>) {
        let unavailable = field.code_editing.on_request_completion.is_none()
            || field.behavior.read_only
            || field.behavior.disabled;
        let stale = self.request.borrow().as_ref().is_some_and(|request| {
            !request.buffer.has_same_contents(&field.text.get())
                || request.caret_index != state.borrow().selection.cursor_index
        });
        *self.field.borrow_mut() = Some(field);
        *self.state.borrow_mut() = Some(state);
        if unavailable || stale {
            self.dismiss();
        }
    }

    /// The caret or text changed. `explicit` is Ctrl+Space, which asks at once
    /// and also opens on an empty prefix.
    pub fn activity(&self, explicit: bool) {
        let Some(next) = self.next_request() else {
            self.dismiss();
            return;
        };
        let after_dot = next.follows_dot();
        if !explicit && next.prefix.is_empty() && !after_dot {
            self.dismiss();
            return;
        }
        self.generation.set(self.generation.get().wrapping_add(1));
        self.elapsed.set(0.0);
        let previous = self.request.borrow().clone();
        match previous {
            Some(previous) if !previous.buffer.has_same_contents(&next.buffer) => {
                let rebased =
                    TextCompletionRebase::candidates(&self.candidates.borrow(), &previous, &next);
                *self.candidates.borrow_mut() = rebased;
            }
            Some(previous) if previous.prefix_range.start == next.prefix_range.start => {}
            _ => self.candidates.borrow_mut().clear(),
        }
        *self.request.borrow_mut() = Some(next);
        self.filter();
        self.is_finished.set(false);
        if let Some(me) = self.me.borrow().upgrade() {
            AnimatorScheduler::current().register(me);
        }
        if explicit {
            self.elapsed.set(DEBOUNCE);
            self.tick(0.0);
        }
    }

    fn next_request(&self) -> Option<TextCompletionRequest> {
        let node = self.owner()?;
        let field = self.field.borrow();
        let field = field.as_ref()?;
        field.code_editing.on_request_completion.as_ref()?;
        let state = self.state.borrow();
        let state = state.as_ref()?.borrow();
        if !state.composition.text.is_empty() || state.selection.anchor.is_some() {
            return None;
        }
        let focused = FocusChainHolder::current().and_then(|chain| chain.focused());
        if focused.as_ref() != Some(&node) {
            return None;
        }
        Some(TextCompletionRequest::new(
            field.text.get(),
            state.selection.cursor_index,
        ))
    }

    fn filter(&self) {
        let prefix = self
            .request
            .borrow()
            .as_ref()
            .map(|request| request.prefix.to_lowercase())
            .unwrap_or_default();
        let filtered: Vec<TextCompletionItem> = self
            .candidates
            .borrow()
            .iter()
            .filter(|item| {
                prefix.is_empty()
                    || item
                        .filter_text
                        .as_deref()
                        .unwrap_or(&item.label)
                        .to_lowercase()
                        .starts_with(&prefix)
            })
            .cloned()
            .collect();
        let last = filtered.len().saturating_sub(1);
        *self.items.borrow_mut() = filtered;
        self.selected_index.set(self.selected_index.get().min(last));
        self.present();
    }

    fn present(&self) {
        let Some(node) = self.owner() else {
            return;
        };
        let items = self.items.borrow().clone();
        if items.is_empty() {
            self.portal.unmount(&node);
            return;
        }
        let store = node
            .composition_value(&PortalStoreEnvironment::key())
            .unwrap_or_else(PortalStoreHolder::current);
        let accept = self.me.borrow().clone();
        let content =
            CompletionPopover::new(items, self.selected_index.get()).on_accept(move |item| {
                if let Some(session) = accept.upgrade() {
                    session.accept(item);
                }
            });
        self.portal.present(
            Some(store),
            self.position(),
            Some(POPOVER_WIDTH),
            AnyView::new(content),
            false,
            PortalPlacementMode::Fixed,
            None,
            None,
        );
        self.install_dismissal();
        self.resolve_selected();
    }

    fn position(&self) -> Point {
        let anchor = self.anchor.get();
        Point::new(anchor.x, anchor.y + anchor.height + ANCHOR_GAP)
    }

    fn install_dismissal(&self) {
        let frame = self.node.borrow().clone();
        let dismiss = self.me.borrow().clone();
        self.portal.set_dismissal(
            Rc::new(move || {
                frame
                    .as_ref()
                    .and_then(Node::upgrade)
                    .map_or(Rect::ZERO, |node| node.absolute_frame())
            }),
            Rc::new(move || {
                if let Some(session) = dismiss.upgrade() {
                    session.dismiss();
                }
            }),
        );
    }

    fn resolve_selected(&self) {
        let Some(resolver) = self
            .field
            .borrow()
            .as_ref()
            .and_then(|field| field.code_editing.on_resolve_completion.clone())
        else {
            return;
        };
        let Some(item) = self.items.borrow().get(self.selected_index.get()).cloned() else {
            return;
        };
        if item.documentation.is_some() || !self.resolving.borrow_mut().insert(item.id.clone()) {
            return;
        }
        let me = self.me.borrow().clone();
        let id = item.id.clone();
        resolver(
            item,
            Box::new(move |resolved| {
                // Dismissal clears `resolving`, so a late answer finds nothing to update.
                let Some(session) = me.upgrade() else {
                    return;
                };
                if !session.resolving.borrow().contains(&id) || resolved.id != id {
                    return;
                }
                for list in [&session.candidates, &session.items] {
                    for item in list.borrow_mut().iter_mut() {
                        if item.id == id {
                            *item = resolved.clone();
                        }
                    }
                }
                if resolved.documentation.is_some() {
                    session.present();
                }
            }),
        );
    }

    /// Follows the caret, in window coordinates.
    pub fn update_anchor(&self, rect: Rect) {
        if self.anchor.get() == rect {
            return;
        }
        self.anchor.set(rect);
        if !self.items.borrow().is_empty() {
            self.portal.update_position(self.position());
        }
    }

    /// Routes one editor key to the list. Returns whether it was consumed.
    pub fn handle_key(&self, event: &KeyEvent) -> bool {
        if event.scancode == crate::Scancode::SPACE && event.modifiers.has_ctrl() {
            self.activity(true);
            return true;
        }
        let count = self.items.borrow().len();
        if count == 0 {
            return false;
        }
        match event.scancode {
            crate::Scancode::ARROW_UP => {
                self.selected_index
                    .set((self.selected_index.get() + count - 1) % count);
                self.present();
            }
            crate::Scancode::ARROW_DOWN => {
                self.selected_index
                    .set((self.selected_index.get() + 1) % count);
                self.present();
            }
            crate::Scancode::RETURN | crate::Scancode::KEYPAD_ENTER | crate::Scancode::TAB => {
                let item = self.items.borrow().get(self.selected_index.get()).cloned();
                if let Some(item) = item {
                    self.accept(item);
                }
            }
            crate::Scancode::ESCAPE => self.dismiss(),
            _ => return false,
        }
        true
    }

    /// Applies an item to the text the request was made against.
    fn accept(&self, item: TextCompletionItem) {
        let field = self
            .field
            .borrow()
            .as_ref()
            .map(TextField::clone_for_session);
        let state = self.state.borrow().clone();
        let request = self.request.borrow().clone();
        let (Some(field), Some(state), Some(request)) = (field, state, request) else {
            self.dismiss();
            return;
        };
        if !request.buffer.has_same_contents(&field.text.get()) {
            self.dismiss();
            return;
        }
        let Some(applied) = apply_completion(&request, &item, field.behavior.max_length) else {
            self.dismiss();
            return;
        };
        self.dismiss();

        field.text.set(applied.buffer.clone());
        {
            let mut state = state.borrow_mut();
            state.buffer = applied.buffer.clone();
            state.clear_composition();
            state.selection.anchor = None;
            state.selection.cursor_index = applied.caret;
            if let Some(snippet) = applied.snippet.as_ref()
                && let Some((start, end)) =
                    self.snippet
                        .start(snippet, applied.insertion, &applied.buffer)
            {
                state.selection.anchor = (start != end).then_some(start);
                state.selection.cursor_index = end;
            }
            state.record_activity();
        }
        if let Some(handler) = field.events.on_change.as_ref() {
            handler(&applied.buffer);
        }
        if let Some(handler) = field.code_editing.on_accept_completion.as_ref() {
            handler(item);
        }
    }

    pub fn dismiss(&self) {
        self.generation.set(self.generation.get().wrapping_add(1));
        self.is_finished.set(true);
        *self.request.borrow_mut() = None;
        self.candidates.borrow_mut().clear();
        self.items.borrow_mut().clear();
        self.selected_index.set(0);
        self.resolving.borrow_mut().clear();
        if let Some(node) = self.owner() {
            self.portal.unmount(&node);
        }
    }
}

/// The outcome of accepting a completion.
#[derive(Clone, Debug, PartialEq)]
pub struct AppliedCompletion {
    pub buffer: crate::primitives::text::text_buffer::TextBuffer,
    pub caret: usize,
    /// Character offset where the inserted text begins.
    pub insertion: usize,
    pub snippet: Option<TextSnippet>,
}

/// Merges an item's replacement and additional edits into one new buffer.
///
/// Returns `None` when a range is outside the request's text, two edits overlap
/// or the result would exceed `max_length`.
pub fn apply_completion(
    request: &TextCompletionRequest,
    item: &TextCompletionItem,
    max_length: Option<usize>,
) -> Option<AppliedCompletion> {
    let buffer = &request.buffer;
    let count = buffer.character_count();
    let range = item
        .replacement
        .clone()
        .unwrap_or_else(|| request.prefix_range.clone());
    if range.end > count {
        return None;
    }
    let snippet = (item.insert_text_format == TextInsertFormat::Snippet)
        .then(|| TextSnippet::new(&item.insert_text));
    let insert_text = snippet
        .as_ref()
        .map_or(item.insert_text.as_str(), |snippet| snippet.text.as_str());
    let mut edits: Vec<TextCompletionEdit> = item.additional_edits.clone();
    edits.push(TextCompletionEdit::new(range.clone(), insert_text));
    edits.sort_by_key(|edit| edit.range.start);
    for (index, edit) in edits.iter().enumerate() {
        if edit.range.end > count {
            return None;
        }
        if index > 0 {
            let previous = &edits[index - 1].range;
            if previous.end > edit.range.start || *previous == edit.range {
                return None;
            }
        }
    }
    let shift: i64 = item
        .additional_edits
        .iter()
        .filter(|edit| edit.range.end <= range.start)
        .map(|edit| {
            let removed = buffer.utf8_offset_for_character_index(edit.range.end)
                - buffer.utf8_offset_for_character_index(edit.range.start);
            edit.text.len() as i64 - removed as i64
        })
        .sum();
    let insertion_byte =
        (buffer.utf8_offset_for_character_index(range.start) as i64 + shift) as usize;
    let mut next = buffer.clone();
    for edit in edits.iter().rev() {
        next = next.replace(edit.range.clone(), &edit.text);
    }
    if max_length.is_some_and(|maximum| next.character_count() > maximum) {
        return None;
    }
    Some(AppliedCompletion {
        caret: next.character_index_for_utf8_offset(insertion_byte + insert_text.len()),
        insertion: next.character_index_for_utf8_offset(insertion_byte),
        buffer: next,
        snippet,
    })
}

impl AnyAnimationController for TextCompletionSession {
    fn is_finished(&self) -> bool {
        self.is_finished.get()
    }

    fn tick(&self, delta_time: f64) {
        if self.is_finished.get() {
            return;
        }
        self.elapsed.set(self.elapsed.get() + delta_time.max(0.0));
        if self.elapsed.get() < DEBOUNCE {
            return;
        }
        let Some(request) = self.request.borrow().clone() else {
            return;
        };
        let Some(provider) = self
            .field
            .borrow()
            .as_ref()
            .and_then(|field| field.code_editing.on_request_completion.clone())
        else {
            return;
        };
        self.is_finished.set(true);
        let token = self.generation.get();
        let me = self.me.borrow().clone();
        let asked = request.clone();
        provider(
            request,
            Box::new(move |candidates| {
                if let Some(session) = me.upgrade() {
                    session.receive(token, &asked, candidates);
                }
            }),
        );
    }

    fn finish_immediately(&self) {
        self.dismiss();
    }

    fn cancel(&self) {
        self.dismiss();
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl TextCompletionSession {
    /// A provider's reply counts only for the request it answers.
    fn receive(
        &self,
        token: u64,
        request: &TextCompletionRequest,
        candidates: Vec<TextCompletionItem>,
    ) {
        if self.generation.get() != token {
            return;
        }
        let still_current = {
            let field = self.field.borrow();
            let state = self.state.borrow();
            field
                .as_ref()
                .is_some_and(|field| request.buffer.has_same_contents(&field.text.get()))
                && state.as_ref().is_some_and(|state| {
                    state.borrow().selection.cursor_index == request.caret_index
                })
        };
        if !still_current {
            return;
        }
        // Keep documentation already resolved for items the new list repeats.
        let known: HashMap<String, String> = self
            .candidates
            .borrow()
            .iter()
            .filter_map(|item| Some((item.id.clone(), item.documentation.clone()?)))
            .collect();
        *self.candidates.borrow_mut() = candidates
            .into_iter()
            .map(|mut item| {
                if item.documentation.is_none() {
                    item.documentation = known.get(&item.id).cloned();
                }
                item
            })
            .collect();
        self.filter();
    }
}

impl NodeResource for TextCompletionSession {
    fn mount(&self, node: &Node) {
        *self.node.borrow_mut() = Some(node.downgrade());
    }

    fn unmount(&self, _node: &Node) {
        self.dismiss();
        *self.node.borrow_mut() = None;
        *self.field.borrow_mut() = None;
        *self.state.borrow_mut() = None;
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}
