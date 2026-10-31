//! Translation of Primitives/Overlays/Menu/Menu.swift.
//!
//! Authored menu tree plus the level renderer. Rows own their hover, press and
//! focus handling; the level owns highlight, submenu timing and type-ahead.
use std::rc::Rc;

use crate::modifiers::style::SemanticStyleViewExt;
use crate::primitives::controls::KeyboardShortcut;
use crate::{AnyView, PrimitiveView, View};
use cove_core::scene::{LayoutNode, Node};
use cove_imaging::BundleImageResource;

use super::menu_descriptor::{MenuDescriptor, MenuItemDescriptor};
use super::menu_item_row::MenuItemRowHost;
use super::menu_navigation::{
    MENU_ITEM_ID, MENU_SCROLL_MARKER, MenuRowEvents, MenuRowPresentation, MenuSession,
};
use super::row_content::MenuItemContent;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MenuItemRole {
    #[default]
    Normal,
    Destructive,
}

/// One actionable menu command.
#[derive(Clone)]
pub struct MenuItem {
    pub id: u64,
    pub title: String,
    pub shortcut: Option<String>,
    pub is_enabled: bool,
    pub is_selected: bool,
    pub role: MenuItemRole,
    pub content: MenuItemContent,
    pub action: Rc<dyn Fn()>,
}

impl MenuItem {
    pub fn new(id: u64, title: impl Into<String>, action: impl Fn() + 'static) -> Self {
        Self {
            id,
            title: title.into(),
            shortcut: None,
            is_enabled: true,
            is_selected: false,
            role: MenuItemRole::Normal,
            content: MenuItemContent::default(),
            action: Rc::new(action),
        }
    }

    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn selected(mut self, is_selected: bool) -> Self {
        self.is_selected = is_selected;
        self
    }

    pub fn role(mut self, role: MenuItemRole) -> Self {
        self.role = role;
        self
    }

    pub fn content(mut self, content: MenuItemContent) -> Self {
        self.content = content;
        self
    }
}

/// A branch has children rather than a command. Its authored settings do not
/// contain the hovered row, focus, timers or portal identifiers.
#[derive(Clone)]
pub struct MenuSubmenu {
    pub id: u64,
    pub title: String,
    pub entries: Vec<MenuEntry>,
    pub is_enabled: bool,
    pub width: f32,
    pub icon: Option<BundleImageResource>,
}

impl MenuSubmenu {
    pub fn new(id: u64, title: impl Into<String>, entries: Vec<MenuEntry>) -> Self {
        Self {
            id,
            title: title.into(),
            entries,
            is_enabled: true,
            width: 220.0,
            icon: None,
        }
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = if width.is_finite() {
            width.max(80.0)
        } else {
            220.0
        };
        self
    }

    pub fn icon(mut self, icon: BundleImageResource) -> Self {
        self.icon = Some(icon);
        self
    }
}

/// One entry in a menu level.
#[derive(Clone)]
pub enum MenuEntry {
    Item(MenuItem),
    Submenu(MenuSubmenu),
    Label { id: u64, title: String },
    Separator { id: u64 },
}

impl MenuEntry {
    pub fn separator(id: u64) -> Self {
        Self::Separator { id }
    }

    pub fn label(id: u64, title: impl Into<String>) -> Self {
        Self::Label {
            id,
            title: title.into(),
        }
    }

    pub fn id(&self) -> u64 {
        match self {
            Self::Item(item) => item.id,
            Self::Submenu(branch) => branch.id,
            Self::Label { id, .. } | Self::Separator { id } => *id,
        }
    }

    pub fn row(&self) -> Option<MenuRowPresentation> {
        match self {
            Self::Item(item) => Some(MenuRowPresentation {
                id: item.id,
                title: item.title.clone(),
                shortcut: item.shortcut.clone(),
                is_enabled: item.is_enabled,
                is_selected: item.is_selected,
                role: item.role,
                has_submenu: false,
                content: item.content.clone(),
            }),
            Self::Submenu(branch) => Some(MenuRowPresentation {
                id: branch.id,
                title: branch.title.clone(),
                shortcut: None,
                is_enabled: branch.is_enabled,
                is_selected: false,
                role: MenuItemRole::Normal,
                has_submenu: true,
                content: MenuItemContent {
                    icon: branch.icon.clone(),
                    ..MenuItemContent::default()
                },
            }),
            _ => None,
        }
    }
}

/// Authored settings for one menu level.
#[derive(Clone)]
pub struct MenuLevelConfiguration {
    pub entries: Vec<MenuEntry>,
    pub width: Option<f32>,
    pub max_visible_rows: usize,
    pub on_item_activated: Rc<dyn Fn()>,
    pub on_dismiss: Rc<dyn Fn()>,
    pub on_close_level: Option<Rc<dyn Fn()>>,
    pub on_frame: Option<Rc<dyn Fn(cove_core::Rect)>>,
}

impl MenuLevelConfiguration {
    pub fn new(entries: Vec<MenuEntry>) -> Self {
        Self {
            entries,
            width: None,
            max_visible_rows: 12,
            on_item_activated: Rc::new(|| {}),
            on_dismiss: Rc::new(|| {}),
            on_close_level: None,
            on_frame: None,
        }
    }
}

/// A menu panel for one level of entries.
pub struct Menu {
    pub entries: Vec<MenuEntry>,
    pub width: Option<f32>,
    pub max_visible_rows: usize,
    pub on_item_activated: Option<Rc<dyn Fn()>>,
    pub on_dismiss: Option<Rc<dyn Fn()>>,
}

impl Menu {
    pub fn new(entries: Vec<MenuEntry>) -> Self {
        Self {
            entries,
            width: None,
            max_visible_rows: 8,
            on_item_activated: None,
            on_dismiss: None,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(if width.is_finite() {
            width.max(80.0)
        } else {
            220.0
        });
        self
    }

    pub fn max_visible_rows(mut self, rows: usize) -> Self {
        self.max_visible_rows = rows.max(1);
        self
    }

    pub fn on_item_activated(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_item_activated = Some(Rc::new(handler));
        self
    }

    pub fn on_dismiss(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(handler));
        self
    }

    /// Build from a platform-facing descriptor tree.
    pub fn from_descriptor(descriptor: &MenuDescriptor) -> Self {
        Self::new(Self::entries(&descriptor.items))
    }

    fn entries(items: &[MenuItemDescriptor]) -> Vec<MenuEntry> {
        items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let id = index as u64;
                match item {
                    MenuItemDescriptor::Separator => MenuEntry::separator(id),
                    MenuItemDescriptor::Label(title) => MenuEntry::label(id, title.clone()),
                    MenuItemDescriptor::Action {
                        title,
                        shortcut,
                        is_enabled,
                        action,
                    } => {
                        let shortcut = shortcut.as_ref().map(KeyboardShortcut::display_string);
                        let action = action.clone();
                        MenuEntry::Item(
                            MenuItem::new(id, title.clone(), move || action())
                                .with_shortcut_opt(shortcut)
                                .enabled(*is_enabled),
                        )
                    }
                    MenuItemDescriptor::Submenu {
                        title,
                        is_enabled,
                        items,
                    } => MenuEntry::Submenu(
                        MenuSubmenu::new(id, title.clone(), Self::entries(items))
                            .enabled(*is_enabled),
                    ),
                }
            })
            .collect()
    }

    fn configuration(&self) -> MenuLevelConfiguration {
        let on_item_activated = self
            .on_item_activated
            .clone()
            .unwrap_or_else(|| Rc::new(|| {}));
        let on_dismiss = self.on_dismiss.clone().unwrap_or_else(|| {
            self.on_item_activated
                .clone()
                .unwrap_or_else(|| Rc::new(|| {}))
        });
        MenuLevelConfiguration {
            entries: self.entries.clone(),
            width: self.width,
            max_visible_rows: self.max_visible_rows,
            on_item_activated,
            on_dismiss,
            on_close_level: None,
            on_frame: None,
        }
    }
}

impl MenuItem {
    /// Runs the command.
    ///
    /// Public so a caller can invoke a declared command from a keyboard shortcut
    /// or a test rather than only through a presented menu.
    pub fn perform(&self) {
        (self.action)();
    }

    fn with_shortcut_opt(mut self, shortcut: Option<String>) -> Self {
        self.shortcut = shortcut;
        self
    }
}

impl View for Menu {
    fn body(&self) -> AnyView {
        AnyView::new(MenuPresentationHost {
            configuration: self.configuration(),
        })
    }
}

struct MenuPresentationHost {
    configuration: MenuLevelConfiguration,
}

impl PrimitiveView for MenuPresentationHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![AnyView::new(MenuLevel::new(self.configuration.clone()))]
    }
}
crate::impl_primitive_view!(MenuPresentationHost);

/// The panel plus its scrollable row stack.
pub struct MenuLevel {
    configuration: MenuLevelConfiguration,
    session: Rc<MenuSession>,
}

impl MenuLevel {
    pub fn new(configuration: MenuLevelConfiguration) -> Self {
        Self {
            configuration,
            session: MenuSession::new(),
        }
    }

    fn with_session(configuration: MenuLevelConfiguration, session: Rc<MenuSession>) -> Self {
        Self {
            configuration,
            session,
        }
    }

    fn enabled(&self) -> Vec<u64> {
        self.configuration
            .entries
            .iter()
            .filter_map(|entry| entry.row().filter(|row| row.is_enabled).map(|row| row.id))
            .collect()
    }

    fn effective_highlight(&self) -> Option<u64> {
        let enabled = self.enabled();
        if let Some(highlighted) = self.session.navigation().highlighted
            && enabled.contains(&highlighted)
        {
            return Some(highlighted);
        }
        self.configuration
            .entries
            .iter()
            .find_map(|entry| {
                entry
                    .row()
                    .filter(|row| row.is_enabled && row.is_selected)
                    .map(|row| row.id)
            })
            .or_else(|| enabled.first().copied())
    }

    fn rows(&self) -> Vec<AnyView> {
        let selection_column = self
            .configuration
            .entries
            .iter()
            .any(|entry| entry.row().is_some_and(|row| row.is_selected));
        let icon_column = self
            .configuration
            .entries
            .iter()
            .any(|entry| entry.row().is_some_and(|row| row.content.icon.is_some()));
        let highlight = self.effective_highlight();
        let reveal_token = self.session.navigation().reveal_token;
        self.configuration
            .entries
            .iter()
            .map(|entry| {
                let Some(row) = entry.row() else {
                    return match entry {
                        MenuEntry::Label { title, .. } => AnyView::new(
                            crate::primitives::text::Text::new(title.clone())
                                .semantic_font(crate::theme::SemanticFontRef::caption())
                                .semantic_foreground(
                                    crate::theme::SemanticColorRef::on_surface_muted(),
                                ),
                        ),
                        _ => AnyView::new(crate::primitives::text::Divider::new(
                            None,
                            1.0,
                            crate::primitives::text::DividerAxis::Horizontal,
                        )),
                    };
                };
                let session = self.session.clone();
                let configuration = self.configuration.clone();
                let entry_clone = entry.clone_for_events();
                let activate_entry = entry_clone.clone();
                let hover_entry = entry_clone.clone();
                let activate_session = session.clone();
                let activate_configuration = configuration.clone();
                let hover_session = session.clone();
                let events = MenuRowEvents {
                    activate: Rc::new(move |node: &Node| {
                        activate_entry.activate(&activate_session, &activate_configuration, node);
                    }),
                    hover: Rc::new(move |node: &Node, inside: bool| {
                        hover_entry.hover(&hover_session, inside, node);
                    }),
                    focus: Rc::new({
                        let session = self.session.clone();
                        let id = row.id;
                        move || {
                            if session.navigation().highlighted != Some(id) {
                                let mut state = session.navigation();
                                state.highlighted = Some(id);
                                session.set_navigation(state);
                            }
                        }
                    }),
                    reveal: Rc::new({
                        let session = self.session.clone();
                        move |node: &Node| reveal_row(&session, node)
                    }),
                    close_submenu: Rc::new({
                        let session = self.session.clone();
                        let id = row.id;
                        move || {
                            if session.navigation().opened == Some(id) {
                                let mut state = session.navigation();
                                state.opened = None;
                                session.set_navigation(state);
                            }
                        }
                    }),
                };
                let submenu = match entry {
                    MenuEntry::Submenu(branch)
                        if self.session.navigation().opened == Some(branch.id) =>
                    {
                        Some(AnyView::new(MenuLevel::with_session(
                            MenuLevelConfiguration {
                                entries: branch.entries.clone(),
                                width: Some(branch.width),
                                max_visible_rows: 12,
                                on_item_activated: Rc::new({
                                    let session = self.session.clone();
                                    let handler = self.configuration.on_item_activated.clone();
                                    move || {
                                        let mut state = session.navigation();
                                        state.opened = None;
                                        session.set_navigation(state);
                                        handler();
                                    }
                                }),
                                on_dismiss: Rc::new({
                                    let session = self.session.clone();
                                    let handler = self.configuration.on_dismiss.clone();
                                    move || {
                                        let mut state = session.navigation();
                                        state.opened = None;
                                        session.set_navigation(state);
                                        handler();
                                    }
                                }),
                                on_close_level: Some(Rc::new({
                                    let session = self.session.clone();
                                    move || {
                                        let mut state = session.navigation();
                                        state.opened = None;
                                        session.set_navigation(state);
                                    }
                                })),
                                on_frame: None,
                            },
                            self.session.clone(),
                        )))
                    }
                    _ => None,
                };
                AnyView::new(MenuItemRowHost {
                    item: row,
                    is_highlighted: highlight == Some(entry.id()),
                    shows_selection_column: selection_column,
                    shows_icon_column: icon_column,
                    submenu,
                    events,
                    reveal_token,
                })
            })
            .collect()
    }
}

impl MenuEntry {
    pub fn clone_for_events(&self) -> MenuEntryRef {
        MenuEntryRef {
            id: self.id(),
            is_item: matches!(self, Self::Item(_)),
            is_submenu: matches!(self, Self::Submenu(_)),
            action: match self {
                Self::Item(item) => Some(item.action.clone()),
                _ => None,
            },
        }
    }
}

/// The minimum an event closure needs from an entry.
#[derive(Clone)]
pub struct MenuEntryRef {
    pub id: u64,
    pub is_item: bool,
    pub is_submenu: bool,
    pub action: Option<Rc<dyn Fn()>>,
}

impl MenuEntryRef {
    fn activate(
        &self,
        session: &Rc<MenuSession>,
        configuration: &MenuLevelConfiguration,
        node: &Node,
    ) {
        session.hover_intent.cancel_all();
        if session.navigation().highlighted != Some(self.id) {
            let mut state = session.navigation();
            state.highlighted = Some(self.id);
            session.set_navigation(state);
        }
        if self.is_submenu {
            if let Some(chain) = cove_core::scene::FocusChainHolder::current() {
                let visible = chain.is_focus_visible();
                chain.focus(Some(node), visible);
            }
            let mut state = session.navigation();
            state.opened = Some(self.id);
            session.set_navigation(state);
        } else if let Some(action) = self.action.clone() {
            let mut state = session.navigation();
            state.opened = None;
            session.set_navigation(state);
            action();
            (configuration.on_item_activated)();
        }
    }

    fn hover(&self, session: &Rc<MenuSession>, inside: bool, node: &Node) {
        if !inside {
            session.hover_intent.cancel(self.id);
            return;
        }
        // The accent band and its light ink follow the pointer at once; only a
        // branch waits for the crossing delay, so a diagonal path cannot swap it
        // out under the pointer.
        if session.navigation().highlighted != Some(self.id) {
            let mut state = session.navigation();
            state.highlighted = Some(self.id);
            session.set_navigation(state);
            if let Some(chain) = cove_core::scene::FocusChainHolder::current() {
                chain.focus(Some(node), false);
            }
        }
        let opened = session.navigation().opened;
        let crossing = opened.is_some()
            && opened != Some(self.id)
            && session.pointer.borrow().is_heading_to_submenu();
        let delay = if crossing { 0.45 } else { 0.18 };
        let session_inner = session.clone();
        let id = self.id;
        let is_submenu = self.is_submenu;
        let weak = node.downgrade();
        session.hover_intent.schedule(
            id,
            delay,
            Rc::new(move || {
                let Some(owner) = Node::upgrade(&weak) else {
                    return;
                };
                if !owner.accepts_subtree_input() {
                    return;
                }
                let next = if is_submenu { Some(id) } else { None };
                if next != session_inner.navigation().opened {
                    session_inner.pointer.borrow_mut().submenu_frame = None;
                }
                let mut state = session_inner.navigation();
                state.opened = next;
                session_inner.set_navigation(state);
            }),
        );
    }
}

fn reveal_row(session: &Rc<MenuSession>, row: &Node) {
    let rect = row.absolute_frame();
    let mut ancestor = row.parent();
    while let Some(node) = ancestor {
        if node.attachment::<bool>(MENU_SCROLL_MARKER) == Some(true) {
            let viewport = node.absolute_frame();
            // The panel insets its rows, and revealing keeps that same gutter:
            // aligning the row flush to the viewport edge would hide the padding
            // the highlight is meant to sit inside.
            let inset = node
                .attachment::<f32>(super::menu_navigation::MENU_SCROLL_INSET)
                .unwrap_or(0.0);
            let mut next = session.scroll_offset();
            if rect.y < viewport.y {
                next.y = (next.y + rect.y - viewport.y - inset).max(0.0);
            } else if rect.y + rect.height > viewport.y + viewport.height {
                next.y += rect.y + rect.height - (viewport.y + viewport.height) + inset;
            }
            if next != session.scroll_offset() {
                session.set_scroll_offset(next);
            }
            return;
        }
        ancestor = node.parent();
    }
}

impl MenuLevel {
    fn handle_key(&self, event: &cove_platform::KeyEvent, root: &Node) -> bool {
        use crate::primitives::overlays::roving_focus::SelectionNavigation;
        let command = match event.scancode {
            crate::Scancode::ARROW_DOWN => Some(SelectionNavigation::Next),
            crate::Scancode::ARROW_UP => Some(SelectionNavigation::Previous),
            crate::Scancode::HOME => Some(SelectionNavigation::First),
            crate::Scancode::END => Some(SelectionNavigation::Last),
            crate::Scancode::ESCAPE => {
                self.session.hover_intent.cancel_all();
                let mut state = self.session.navigation();
                state.opened = None;
                self.session.set_navigation(state);
                match self.configuration.on_close_level.as_ref() {
                    Some(close) => close(),
                    None => (self.configuration.on_dismiss)(),
                }
                return true;
            }
            crate::Scancode::TAB => {
                self.session.hover_intent.cancel_all();
                let mut state = self.session.navigation();
                state.opened = None;
                self.session.set_navigation(state);
                (self.configuration.on_dismiss)();
                return true;
            }
            crate::Scancode::ARROW_LEFT => {
                let Some(close) = self.configuration.on_close_level.as_ref() else {
                    return false;
                };
                self.session.hover_intent.cancel_all();
                let mut state = self.session.navigation();
                state.opened = None;
                self.session.set_navigation(state);
                close();
                return true;
            }
            crate::Scancode::ARROW_RIGHT
            | crate::Scancode::RETURN
            | crate::Scancode::KEYPAD_ENTER
            | crate::Scancode::SPACE => {
                if event.is_repeat {
                    return true;
                }
                let Some(id) = self.effective_highlight() else {
                    return false;
                };
                let Some(entry) = self
                    .configuration
                    .entries
                    .iter()
                    .find(|entry| entry.id() == id)
                else {
                    return false;
                };
                let Some(row) = super::menu_navigation::menu_row(id, root) else {
                    return false;
                };
                if event.scancode == crate::Scancode::ARROW_RIGHT
                    && !matches!(entry, MenuEntry::Submenu(_))
                {
                    return false;
                }
                entry
                    .clone_for_events()
                    .activate(&self.session, &self.configuration, &row);
                return true;
            }
            _ => return false,
        };
        let enabled = self.enabled();
        let Some(destination) = command.and_then(|command| {
            command.destination(&enabled, self.effective_highlight().as_ref(), true)
        }) else {
            return true;
        };
        self.select(destination, root);
        true
    }

    fn handle_text(&self, text: &str, root: &Node) -> bool {
        let rows: Vec<super::menu_navigation::MenuTypeaheadRow> = self
            .configuration
            .entries
            .iter()
            .filter_map(|entry| entry.row().filter(|row| row.is_enabled))
            .map(|row| super::menu_navigation::MenuTypeaheadRow {
                id: row.id,
                title: row.title.clone(),
            })
            .collect();
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs_f64())
            .unwrap_or(0.0);
        let target = self.session.typeahead.borrow_mut().destination(
            text,
            &rows,
            self.effective_highlight(),
            time,
        );
        if let Some(target) = target {
            self.select(target, root);
        }
        true
    }

    fn select(&self, id: u64, root: &Node) {
        self.session.hover_intent.cancel_all();
        let mut state = self.session.navigation();
        state.opened = None;
        state.highlighted = Some(id);
        state.reveal_token = state.reveal_token.wrapping_add(1);
        self.session.set_navigation(state);
        if let Some(row) = super::menu_navigation::menu_row(id, root) {
            if let Some(chain) = cove_core::scene::FocusChainHolder::current() {
                chain.focus(Some(&row), true);
            }
            reveal_row(&self.session, &row);
        }
    }
}

impl View for MenuLevel {
    fn body(&self) -> AnyView {
        let rows = self.rows();
        let on_key: Rc<dyn Fn(&cove_platform::KeyEvent, &Node) -> bool> = Rc::new({
            let level = self.snapshot();
            move |event, root| level.handle_key(event, root)
        });
        let on_text: Rc<dyn Fn(&str, &Node) -> bool> = Rc::new({
            let level = self.snapshot();
            move |text, root| level.handle_text(text, root)
        });
        AnyView::new(super::menu_level_host::MenuLevelHost {
            session: self.session.clone(),
            width: self.configuration.width,
            max_visible_rows: self.configuration.max_visible_rows,
            rows: AnyView::new(crate::TupleView::new(rows)),
            on_key,
            on_text,
        })
    }
}

impl MenuLevel {
    fn snapshot(&self) -> MenuLevel {
        MenuLevel {
            configuration: self.configuration.clone(),
            session: self.session.clone(),
        }
    }
}
