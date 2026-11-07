//! Translation of Primitives/Overlays/Menu/MenuNavigation.swift.
//!
//! Row presentation, hover intent and type-ahead. The session keeps navigation
//! identity across recomposition so child portals retain their callbacks.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::primitives::overlays::menu::row_content::MenuItemContent;
use cove_core::Rect;
use cove_core::scene::{AnimatorScheduler, AnyAnimationController, Node, NodeResource};

use super::menu::{MenuItemRole, MenuSubmenu};

/// Presentation of one rendered menu row.
#[derive(Clone)]
pub struct MenuRowPresentation {
    pub id: u64,
    pub title: String,
    pub shortcut: Option<String>,
    pub is_enabled: bool,
    pub is_selected: bool,
    pub role: MenuItemRole,
    pub has_submenu: bool,
    pub content: MenuItemContent,
}

impl MenuRowPresentation {
    pub fn is_link(&self) -> bool {
        self.content.link.is_some() && !self.has_submenu
    }
}

/// Row-type-agnostic projection used by the type-ahead search.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuTypeaheadRow {
    pub id: u64,
    pub title: String,
}

/// Type-ahead prefix matching, reset after a short idle gap.
#[derive(Clone, Debug, Default)]
pub struct MenuTypeahead {
    prefix: String,
    last_input: f64,
}

impl MenuTypeahead {
    pub fn new() -> Self {
        Self::default()
    }

    /// Next enabled row whose title starts with the accumulated prefix,
    /// searching forward from the current row and wrapping once.
    pub fn destination(
        &mut self,
        text: &str,
        rows: &[MenuTypeaheadRow],
        current: Option<u64>,
        time: f64,
    ) -> Option<u64> {
        if text.is_empty() || rows.is_empty() {
            return None;
        }
        if time - self.last_input > 0.8 {
            self.prefix.clear();
        }
        self.last_input = time;
        // Swift folds case and diacritics here, so a leading "e" matches
        // "Éclair" as well as "Echo".
        let input = super::super::command_palette_model::normalize(text);
        if self.prefix == input && input.chars().count() == 1 {
            self.prefix = input;
        } else {
            self.prefix.push_str(&input);
        }
        let start = rows
            .iter()
            .position(|row| Some(row.id) == current)
            .map_or(0, |index| index + 1);
        for distance in 0..rows.len() {
            let row = &rows[(start + distance) % rows.len()];
            if super::super::command_palette_model::normalize(&row.title).starts_with(&self.prefix)
            {
                return Some(row.id);
            }
        }
        None
    }
}

/// Navigation state of one menu level.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuNavigationState {
    pub highlighted: Option<u64>,
    pub opened: Option<u64>,
    pub reveal_token: u64,
}

/// Shared session for a menu level and its child portals.
pub struct MenuSession {
    navigation: RefCell<MenuNavigationState>,
    /// Advances with every navigation change, so the rows repaint their highlight.
    pub revision: cove_core::State<u64>,
    scroll_offset: Cell<(u32, u32)>,
    pub hover_intent: Rc<MenuHoverIntent>,
    pub typeahead: RefCell<MenuTypeahead>,
    pub pointer: RefCell<super::row_content::MenuPointerIntent>,
    /// One navigation slot per open branch. Swift gives every `MenuLevel` its
    /// own `@State session`, so a child level opening a branch of its own never
    /// overwrites its parent's `opened`; caching by branch id keeps that
    /// identity across recomposition without sharing one slot tree-wide.
    children: RefCell<std::collections::HashMap<u64, Rc<MenuSession>>>,
}

impl MenuSession {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            navigation: RefCell::new(MenuNavigationState::default()),
            revision: cove_core::State::new(0),
            scroll_offset: Cell::new((0, 0)),
            hover_intent: MenuHoverIntent::new(),
            typeahead: RefCell::new(MenuTypeahead::new()),
            pointer: RefCell::new(super::row_content::MenuPointerIntent::new()),
            children: RefCell::new(std::collections::HashMap::new()),
        })
    }

    /// The navigation slot for the branch level rooted at `id`, created once and
    /// then retained for as long as the level stays open.
    pub fn child(&self, id: u64) -> Rc<MenuSession> {
        if let Some(existing) = self.children.borrow().get(&id) {
            return existing.clone();
        }
        let session = MenuSession::new();
        self.children.borrow_mut().insert(id, session.clone());
        session
    }

    pub fn navigation(&self) -> MenuNavigationState {
        *self.navigation.borrow()
    }

    pub fn set_navigation(&self, value: MenuNavigationState) {
        if *self.navigation.borrow() == value {
            return;
        }
        *self.navigation.borrow_mut() = value;
        self.revision
            .update(|revision| *revision = revision.wrapping_add(1));
    }

    pub fn scroll_offset(&self) -> cove_core::Point {
        let (x, y) = self.scroll_offset.get();
        cove_core::Point::new(f32::from_bits(x), f32::from_bits(y))
    }

    pub fn set_scroll_offset(&self, value: cove_core::Point) {
        let packed = (value.x.to_bits(), value.y.to_bits());
        if self.scroll_offset.get() == packed {
            return;
        }
        self.scroll_offset.set(packed);
    }
}

/// A short intent delay lets a pointer cross sibling rows while travelling into
/// an already-open child. Leaving a pending row cancels its switch.
pub struct MenuHoverIntent {
    pending_id: Cell<Option<u64>>,
    action: RefCell<Option<Rc<dyn Fn()>>>,
    elapsed: Cell<f64>,
    delay: Cell<f64>,
    is_finished: Cell<bool>,
    scheduler: RefCell<Option<AnimatorScheduler>>,
    controller: RefCell<Option<Rc<dyn AnyAnimationController>>>,
}

impl MenuHoverIntent {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            pending_id: Cell::new(None),
            action: RefCell::new(None),
            elapsed: Cell::new(0.0),
            delay: Cell::new(0.18),
            is_finished: Cell::new(true),
            scheduler: RefCell::new(None),
            controller: RefCell::new(None),
        })
    }

    pub fn is_finished(&self) -> bool {
        self.is_finished.get()
    }

    pub fn schedule(self: &Rc<Self>, id: u64, delay: f64, action: Rc<dyn Fn()>) {
        self.pending_id.set(Some(id));
        *self.action.borrow_mut() = Some(action);
        self.delay.set(delay);
        self.elapsed.set(0.0);
        if !self.is_finished.get() {
            return;
        }
        self.is_finished.set(false);
        let controller: Rc<dyn AnyAnimationController> = self.clone();
        *self.controller.borrow_mut() = Some(controller.clone());
        if let Some(scheduler) = self.scheduler.borrow().as_ref() {
            scheduler.register(controller);
        }
    }

    pub fn cancel_all(&self) {
        self.cancel_with(None, true);
    }

    /// Cancel the pending switch unless it belongs to a different row.
    pub fn cancel(&self, id: u64) {
        if self.pending_id.get() != Some(id) {
            return;
        }
        self.cancel_with(None, true);
    }

    fn cancel_with(&self, keep: Option<u64>, clear_action: bool) {
        if keep.is_some() && self.pending_id.get() == keep {
            return;
        }
        self.is_finished.set(true);
        self.pending_id.set(None);
        if clear_action {
            *self.action.borrow_mut() = None;
        }
        if let (Some(scheduler), Some(controller)) = (
            self.scheduler.borrow().as_ref(),
            self.controller.borrow().as_ref(),
        ) {
            scheduler.unregister(controller);
        }
        self.controller.borrow_mut().take();
    }

    fn tick(&self, delta_time: f64) {
        if self.is_finished.get() {
            return;
        }
        self.elapsed.set(self.elapsed.get() + delta_time.max(0.0));
        if self.elapsed.get() < self.delay.get() {
            return;
        }
        let action = self.action.borrow().clone();
        self.cancel_with(None, true);
        if let Some(action) = action {
            action();
        }
    }
}

impl AnyAnimationController for MenuHoverIntent {
    fn is_finished(&self) -> bool {
        self.is_finished.get()
    }

    fn tick(&self, delta_time: f64) {
        MenuHoverIntent::tick(self, delta_time);
    }

    fn finish_immediately(&self) {
        self.cancel_all();
    }

    fn cancel(&self) {
        self.cancel_all();
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl NodeResource for MenuHoverIntent {
    fn mount(&self, _node: &Node) {
        *self.scheduler.borrow_mut() = Some(AnimatorScheduler::current());
    }

    fn unmount(&self, _node: &Node) {
        self.cancel_all();
        *self.scheduler.borrow_mut() = None;
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

/// Finds the row node carrying `id` under `node`.
pub fn menu_row(id: u64, node: &Node) -> Option<Node> {
    if node.attachment::<u64>(MENU_ITEM_ID) == Some(id) {
        return Some(node.clone());
    }
    for child in node.children() {
        if let Some(row) = menu_row(id, &child) {
            return Some(row);
        }
    }
    None
}

pub const MENU_ITEM_ID: &str = "__menu_item_id";
pub const MENU_LEVEL_MARKER: &str = "__menu.level";
pub const MENU_SCROLL_MARKER: &str = "__menu.scroll";
pub const MENU_SCROLL_INSET: &str = "__menu.scroll.inset";

/// Row-node facts the level needs when it schedules hover and activation.
#[derive(Clone)]
pub struct MenuRowEvents {
    pub activate: Rc<dyn Fn(&Node)>,
    pub hover: Rc<dyn Fn(&Node, bool)>,
    pub focus: Rc<dyn Fn()>,
    pub reveal: Rc<dyn Fn(&Node)>,
    pub close_submenu: Rc<dyn Fn()>,
}

impl MenuRowEvents {
    pub fn is_inside(node: &Node, point: cove_core::Point) -> bool {
        let frame = node.absolute_frame();
        point.x >= frame.x
            && point.x <= frame.x + frame.width
            && point.y >= frame.y
            && point.y <= frame.y + frame.height
    }

    pub fn frame(node: &Node) -> Rect {
        node.absolute_frame()
    }
}

const _: fn(&MenuSubmenu) -> f32 = |submenu| submenu.width;
