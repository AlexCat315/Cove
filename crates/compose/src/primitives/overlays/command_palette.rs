//! Translation of Primitives/Overlays/CommandPalette.swift (the view half; the
//! ranking and result projection live in `command_palette_model`).
//!
//! Searchable command surface presented as a modal overlay. A palette is worth
//! more than the menu it replaces only if finding a command is faster than
//! looking for it, so ranking is deliberately aggressive: a title prefix
//! outranks a word start, which outranks a substring, which outranks a
//! subsequence match.
//!
//! Keyboard is the primary path — arrows move, Return runs, Escape closes — and
//! the search field keeps focus throughout, so the hands never leave the
//! keyboard.
use crate::Scope;
use std::rc::Rc;

use cove_core::scene::{
    AccessibilityRole, AccessibilityState, Align, FlexDirection, Justify, Node, Wrap,
};
use cove_core::{Binding, Signal};
use cove_imaging::BundleImageResource;
use cove_platform::KeyEvent;

use crate::modifiers::interaction::{AccessibilityExt, HoverExt};
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::{Button, ButtonRole};
use crate::primitives::controls::fields::select::{KeyboardShortcut, KeyboardShortcutPlatform};
use crate::primitives::controls::key_cap::KeyCap;
use crate::primitives::feedback::{EmptyState, EmptyStateProminence};
use crate::primitives::layout::virtualization::VariableVirtualStack;
use crate::primitives::media::ui_icon_catalog::{action, navigation};
use crate::primitives::overlays::command_palette_model::{
    CommandEntry, CommandItem, destination, display_entries, ranked, record_recent,
};
use crate::primitives::overlays::modal::{Modal, ModalGeometry, ModalOptions, ModalPlacement};
use crate::primitives::text::text_buffer::TextBuffer;
use crate::primitives::text::text_field::{
    TextField, TextFieldDecoration, TextFieldEvents, TextFieldSize,
};
use crate::primitives::text::{Divider, Text};
use crate::theme::styles::buttons::{ButtonStyleViewExt, GhostButtonStyle};
use crate::theme::{SemanticColorRef, SemanticFontRef};
use crate::{AnyView, BoxView, Row, Spacer, TupleView, VerticalAlignment, View};

/// One entry in a [`CommandPalette`].
///
/// `keywords` are the search-only aliases nobody sees — the abbreviation, the
/// old name, the menu path. Without them a palette only finds commands whose
/// title the person already remembers, which is exactly when they do not need
/// one.
#[derive(Clone)]
pub struct PaletteCommand {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub group: String,
    pub keywords: Vec<String>,
    pub icon: Option<BundleImageResource>,
    pub shortcut: Option<KeyboardShortcut>,
    pub is_enabled: bool,
    pub action: Rc<dyn Fn()>,
}

impl PaletteCommand {
    pub fn new(title: impl Into<String>, action: impl Fn() + 'static) -> Self {
        let title = title.into();
        Self {
            id: title.clone(),
            title,
            subtitle: String::new(),
            group: String::new(),
            keywords: Vec::new(),
            icon: None,
            shortcut: None,
            is_enabled: true,
            action: Rc::new(action),
        }
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = subtitle.into();
        self
    }

    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = group.into();
        self
    }

    pub fn keywords(mut self, keywords: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.keywords = keywords.into_iter().map(Into::into).collect();
        self
    }

    pub fn icon(mut self, icon: BundleImageResource) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn shortcut(mut self, shortcut: KeyboardShortcut) -> Self {
        self.shortcut = Some(shortcut);
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    /// The projection the shared ranking model consumes.
    pub fn item(&self) -> CommandItem {
        CommandItem {
            id: self.id.clone(),
            title: self.title.clone(),
            subtitle: self.subtitle.clone(),
            group: self.group.clone(),
            keywords: self.keywords.clone(),
            is_enabled: self.is_enabled,
        }
    }
}

/// Session state owned by the palette's scope, so it survives recomposition.
#[derive(Clone, Copy)]
struct PaletteState {
    query: Signal<TextBuffer>,
    highlighted_id: Signal<Option<String>>,
    focus_token: Signal<u64>,
    recent_ids: Signal<Vec<String>>,
}

impl PaletteState {
    fn new(cx: &Scope) -> Self {
        Self {
            query: cx.state(TextBuffer::empty),
            highlighted_id: cx.state(|| None),
            focus_token: cx.state(|| 0),
            recent_ids: cx.state(Vec::new),
        }
    }
}

/// Searchable command surface presented as a modal overlay.
pub struct CommandPalette {
    pub is_presented: Binding<bool>,
    pub items: Vec<PaletteCommand>,
    pub placeholder: String,
    pub empty_title: String,
    pub empty_message: String,
    /// Shows the commands run in this palette above their normal groups when the
    /// query is empty. History is session-scoped and never persisted.
    pub shows_recent_commands: bool,
    pub recent_command_limit: usize,
    footer: Option<AnyView>,
}

impl CommandPalette {
    pub const ROW_HEIGHT: f32 = 40.0;
    pub const HEADER_HEIGHT: f32 = 24.0;

    pub fn new(is_presented: Binding<bool>, items: Vec<PaletteCommand>) -> Self {
        Self {
            is_presented,
            items,
            placeholder: "Type a command…".to_string(),
            empty_title: "No matching commands".to_string(),
            empty_message: "Try a different search term.".to_string(),
            shows_recent_commands: true,
            recent_command_limit: 3,
            footer: None,
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn empty_state(mut self, title: impl Into<String>, message: impl Into<String>) -> Self {
        self.empty_title = title.into();
        self.empty_message = message.into();
        self
    }

    pub fn shows_recent_commands(mut self, shows_recent_commands: bool) -> Self {
        self.shows_recent_commands = shows_recent_commands;
        self
    }

    pub fn recent_command_limit(mut self, limit: usize) -> Self {
        self.recent_command_limit = limit.min(10);
        self
    }

    /// Replaces the default shortcut-hint footer with caller content.
    pub fn footer(mut self, footer: impl View + 'static) -> Self {
        self.footer = Some(AnyView::new(footer));
        self
    }

    fn trimmed_query(state: PaletteState) -> String {
        state
            .query
            .with(|query| query.string_value().trim().to_string())
    }

    fn visible(&self, state: PaletteState) -> Vec<PaletteCommand> {
        let items: Vec<CommandItem> = self
            .items
            .iter()
            .filter(|item| item.is_enabled)
            .map(PaletteCommand::item)
            .collect();
        ranked(&items, &Self::trimmed_query(state))
            .into_iter()
            .filter_map(|ranked_item| {
                self.items
                    .iter()
                    .find(|item| item.id == ranked_item.id)
                    .cloned()
            })
            .collect()
    }

    fn entries(&self, state: PaletteState, visible: &[PaletteCommand]) -> Vec<CommandEntry> {
        let items: Vec<CommandItem> = visible.iter().map(PaletteCommand::item).collect();
        let query_empty = Self::trimmed_query(state).is_empty();
        display_entries(
            &items,
            &state.recent_ids.get(),
            self.shows_recent_commands && query_empty,
            self.recent_command_limit,
            query_empty,
        )
    }

    fn selectable(entries: &[CommandEntry]) -> Vec<CommandItem> {
        entries
            .iter()
            .filter_map(CommandEntry::item)
            .filter(|item| item.is_enabled)
            .cloned()
            .collect()
    }

    fn effective_highlight(state: PaletteState, selectable: &[CommandItem]) -> Option<String> {
        state
            .highlighted_id
            .get()
            .filter(|id| selectable.iter().any(|item| &item.id == id))
            .or_else(|| selectable.first().map(|item| item.id.clone()))
    }
}

impl View for CommandPalette {
    fn body(&self, cx: &Scope) -> AnyView {
        let state = PaletteState::new(cx);
        let visible = self.visible(state);
        let entries = self.entries(state, &visible);
        let selectable = Self::selectable(&entries);
        let effective = Self::effective_highlight(state, &selectable);
        let on_highlight = { Rc::new(move |id: String| state.highlighted_id.set(Some(id))) };
        let highlighted_entry_index = effective.as_ref().and_then(|id| {
            entries.iter().position(|entry| match entry {
                CommandEntry::Item(item) => &item.id == id,
                CommandEntry::Header(_) => false,
            })
        });

        let is_presented = self.is_presented.clone();
        let footer = self.footer.clone();
        let limit = self.recent_command_limit;
        let total = self.items.len();
        let visible_count = visible.len();
        let items: Rc<Vec<PaletteCommand>> = Rc::new(self.items.clone());

        let on_present = {
            let binding = is_presented.clone();
            let items = items.clone();
            move || {
                let next = if binding.get() {
                    first_selectable(&items)
                } else {
                    None
                };
                state.query.set(TextBuffer::empty());
                state.highlighted_id.set(next);
                let token = state.focus_token.get().wrapping_add(1);
                state.focus_token.set(token);
            }
        };

        let on_move = {
            let items = items.clone();
            Rc::new(move |delta: i64, wraps: bool| {
                let selectable = first_selectable_all(&items);
                let index = state
                    .highlighted_id
                    .get()
                    .and_then(|id| selectable.iter().position(|item| item.id == id));
                match destination(selectable.len(), index, delta, wraps) {
                    Some(next) => state
                        .highlighted_id
                        .set(selectable.get(next).map(|item| item.id.clone())),
                    None => state.highlighted_id.set(None),
                }
            })
        };

        let run_command = {
            let items = items.clone();
            let binding = is_presented.clone();
            Rc::new(move |id: &str| {
                let Some(command) = items
                    .iter()
                    .find(|command| command.id == id && command.is_enabled)
                else {
                    return;
                };
                let mut recents = state.recent_ids.get();
                record_recent(&mut recents, &command.id, limit);
                state.recent_ids.set(recents);
                let action = command.action.clone();
                binding.set(false);
                action();
            })
        };

        let on_run_highlighted = {
            let items = items.clone();
            let run_command = run_command.clone();
            Rc::new(move || {
                let selectable = first_selectable_all(&items);
                let effective = state
                    .highlighted_id
                    .get()
                    .filter(|id| selectable.iter().any(|item| &item.id == id))
                    .or_else(|| selectable.first().map(|item| item.id.clone()));
                if let Some(id) = effective {
                    run_command(&id);
                }
            })
        };

        let on_close = {
            let binding = is_presented.clone();
            Rc::new(move || binding.set(false))
        };

        let on_change = {
            let items = items.clone();
            Rc::new(move |buffer: &TextBuffer| {
                let query = buffer.string_value().trim().to_string();
                let projected: Vec<CommandItem> = items.iter().map(PaletteCommand::item).collect();
                let next = ranked(&projected, &query)
                    .into_iter()
                    .find(|item| item.is_enabled);
                state.highlighted_id.set(next.map(|item| item.id));
            })
        };

        let palette = PaletteBody {
            commands: items,
            entries,
            effective,
            highlighted_entry_index,
            placeholder: self.placeholder.clone(),
            empty_title: self.empty_title.clone(),
            empty_message: self.empty_message.clone(),
            footer,
            query: state.query,
            focus_token: state.focus_token,
            total,
            visible_count,
            on_move,
            on_run_highlighted,
            on_run: run_command,
            on_close,
            on_change,
            on_highlight,
            on_present: Rc::new(on_present),
        };

        let options = ModalOptions {
            geometry: ModalGeometry {
                width: 640.0,
                height: 560.0,
                inset: 72.0,
                corner_radius: 8.0,
                placement: ModalPlacement::Top,
            },
            ..Default::default()
        };
        AnyView::new(
            Modal::new(is_presented, palette)
                .options(options)
                .fitting_geometry(|geometry, size| {
                    if size.height > 0.0 {
                        geometry.height = geometry.height.min((size.height - 144.0).max(240.0));
                    }
                }),
        )
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
/// First enabled command's id, for the mount-time highlight.
fn first_selectable(items: &[PaletteCommand]) -> Option<String> {
    items
        .iter()
        .find(|item| item.is_enabled)
        .map(|item| item.id.clone())
}

/// All enabled commands, in authored order.
fn first_selectable_all(items: &[PaletteCommand]) -> Vec<CommandItem> {
    items
        .iter()
        .filter(|item| item.is_enabled)
        .map(PaletteCommand::item)
        .collect()
}

/// The panel: search field, results, footer.
struct PaletteBody {
    commands: Rc<Vec<PaletteCommand>>,
    entries: Vec<CommandEntry>,
    effective: Option<String>,
    highlighted_entry_index: Option<usize>,
    placeholder: String,
    empty_title: String,
    empty_message: String,
    footer: Option<AnyView>,
    query: Signal<TextBuffer>,
    focus_token: Signal<u64>,
    total: usize,
    visible_count: usize,
    on_move: Rc<dyn Fn(i64, bool)>,
    on_run_highlighted: Rc<dyn Fn()>,
    on_run: Rc<dyn Fn(&str)>,
    on_close: Rc<dyn Fn()>,
    on_change: Rc<dyn Fn(&TextBuffer)>,
    on_highlight: Rc<dyn Fn(String)>,
    on_present: Rc<dyn Fn()>,
}

impl View for PaletteBody {
    fn body(&self, _cx: &Scope) -> AnyView {
        // Fires once per presentation so the palette resets its transient state.
        let settled = PresentWatcher {
            on_present: self.on_present.clone(),
        };

        let move_down = {
            let on_move = self.on_move.clone();
            move |_| on_move(1, true)
        };
        let move_up = {
            let on_move = self.on_move.clone();
            move |_| on_move(-1, true)
        };
        let page_down = {
            let on_move = self.on_move.clone();
            move |_| on_move(10, false)
        };
        let page_up = {
            let on_move = self.on_move.clone();
            move |_| on_move(-10, false)
        };
        let run = self.on_run_highlighted.clone();
        let close = self.on_close.clone();
        let change = self.on_change.clone();
        let clear_change = self.on_change.clone();

        let token = self.focus_token.get() as usize;
        let mut events = TextFieldEvents::default();
        events.on_key_down = Some(Rc::new(move |event: &KeyEvent| match event.scancode {
            crate::Scancode::ARROW_DOWN => {
                move_down(());
                true
            }
            crate::Scancode::ARROW_UP => {
                move_up(());
                true
            }
            crate::Scancode::PAGE_DOWN => {
                page_down(());
                true
            }
            crate::Scancode::PAGE_UP => {
                page_up(());
                true
            }
            crate::Scancode::RETURN | crate::Scancode::KEYPAD_ENTER => {
                run();
                true
            }
            crate::Scancode::ESCAPE => {
                close();
                true
            }
            _ => false,
        }));
        events.on_change = Some(Rc::new(move |buffer: &TextBuffer| change(buffer)));
        events.on_clear = Some(Rc::new(move || clear_change(&TextBuffer::empty())));

        let mut behavior = crate::primitives::text::text_field::TextFieldBehavior::default();
        behavior.clearable = true;
        let mut decoration = TextFieldDecoration::default();
        decoration.size = TextFieldSize::Large;
        let mut navigation =
            crate::primitives::text::text_field::TextFieldNavigationRequest::default();
        navigation.focus_request_id = Some(token as u64);

        let field = TextField::new(self.placeholder.clone(), self.query.binding())
            .behavior(behavior)
            .decoration(decoration)
            .events(events);
        let mut field = field;
        field.navigation = navigation;

        let search = AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(field),
            )
            .frame(None, None, None, None, Some(f32::INFINITY), None)
            .padding(12.0),
        );

        let results = AnyView::new(PaletteResults {
            commands: self.commands.clone(),
            entries: self.entries.clone(),
            effective: self.effective.clone(),
            empty_title: self.empty_title.clone(),
            empty_message: self.empty_message.clone(),
            scroll_to: self.highlighted_entry_index,
            on_run: self.on_run.clone(),
            on_highlight: self.on_highlight.clone(),
        });

        let separator = AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(crate::EmptyView),
            ))
            .frame_height(1.0)
            .semantic_background(SemanticColorRef::border())
            .padding_edges(12.0, 0.0),
        );

        let footer = self.footer.clone().unwrap_or_else(|| {
            AnyView::new(PaletteFooter {
                visible_count: self.visible_count,
                total: self.total,
            })
        });

        let column = AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(settled),
                    search,
                    AnyView::new(Divider::horizontal()),
                    results,
                    separator,
                    footer,
                ])),
            )
            .frame(None, None, None, None, None, None),
        );
        AnyView::new(column)
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Fires once per presentation so the palette can reset its transient state.
struct PresentWatcher {
    on_present: Rc<dyn Fn()>,
}

impl crate::PrimitiveView for PresentWatcher {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_width(Some(0.0));
        layout.set_height(Some(0.0));
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        const SETTLED: &str = "commandPalette.settled";
        if node.attachment::<bool>(SETTLED) == Some(true) {
            return;
        }
        node.set_attachment(SETTLED, true);
        (self.on_present)();
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        Vec::new()
    }
}
crate::impl_primitive_view!(PresentWatcher);

/// Grouped result rows, scrolled to keep the highlight visible.
struct PaletteResults {
    entries: Vec<CommandEntry>,
    commands: Rc<Vec<PaletteCommand>>,
    effective: Option<String>,
    empty_title: String,
    empty_message: String,
    scroll_to: Option<usize>,
    on_run: Rc<dyn Fn(&str)>,
    on_highlight: Rc<dyn Fn(String)>,
}

impl PaletteResults {
    fn snapshot(&self) -> Self {
        Self {
            commands: self.commands.clone(),
            entries: self.entries.clone(),
            effective: self.effective.clone(),
            empty_title: self.empty_title.clone(),
            empty_message: self.empty_message.clone(),
            scroll_to: self.scroll_to,
            on_run: self.on_run.clone(),
            on_highlight: self.on_highlight.clone(),
        }
    }

    fn row(&self, entry: &CommandEntry) -> AnyView {
        match entry {
            CommandEntry::Header(title) => AnyView::new(
                AnyView::new(
                    AnyView::new(
                        Text::new(title.clone())
                            .semantic_font(SemanticFontRef::caption())
                            .semantic_foreground(SemanticColorRef::on_surface_muted()),
                    )
                    .padding_edges(14.0, 0.0)
                    .frame_height(CommandPalette::HEADER_HEIGHT),
                )
                .accessibility(|semantics| {
                    semantics.role = AccessibilityRole::StaticText;
                }),
            ),
            CommandEntry::Item(item) => {
                let is_highlighted = self.effective.as_ref() == Some(&item.id);
                let command = self
                    .commands
                    .iter()
                    .find(|command| command.id == item.id)
                    .cloned()
                    .unwrap_or_else(|| PaletteCommand::new(item.title.clone(), || {}));
                let row = CommandRow {
                    item: item.clone(),
                    command,
                    is_highlighted,
                    on_run: self.on_run.clone(),
                    on_highlight: self.on_highlight.clone(),
                };
                AnyView::new(AnyView::new(row).frame(
                    None,
                    None,
                    None,
                    Some(CommandPalette::ROW_HEIGHT),
                    None,
                    None,
                ))
            }
        }
    }
}

impl View for PaletteResults {
    fn body(&self, _cx: &Scope) -> AnyView {
        if self.entries.is_empty() {
            return AnyView::new(
                EmptyState::new(self.empty_title.clone(), self.empty_message.clone())
                    .prominence(EmptyStateProminence::Compact)
                    .icon(action::search())
                    .padding_edges(6.0, 0.0)
                    .flex_with(1.0, 1.0, Some(0.0))
                    .frame(None, None, None, Some(0.0), None, None),
            );
        }
        // Rows are keyed by entry position: filtering rebuilds the list anyway,
        // and a header's title need not be unique.
        let entries = self.entries.clone();
        let heights = self.entries.clone();
        let results = self.snapshot();
        let mut stack = VariableVirtualStack::new(
            (0..entries.len()).collect(),
            move |index: &usize, _width, _scale| match heights.get(*index) {
                Some(CommandEntry::Header(_)) => CommandPalette::HEADER_HEIGHT,
                _ => CommandPalette::ROW_HEIGHT,
            },
            move |index: &usize| match entries.get(*index) {
                Some(entry) => results.row(entry),
                None => AnyView::new(crate::EmptyView),
            },
        );
        if let Some(index) = self.scroll_to {
            stack = stack.scroll_to_index(index);
        }
        AnyView::new(
            AnyView::new(stack)
                .padding_edges(8.0, 0.0)
                .flex_with(1.0, 1.0, Some(0.0))
                .frame(None, None, None, Some(0.0), None, None),
        )
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// One command row. The highlight is a cursor, not a press: only Return or a
/// click runs the command.
struct CommandRow {
    item: CommandItem,
    command: PaletteCommand,
    is_highlighted: bool,
    on_run: Rc<dyn Fn(&str)>,
    on_highlight: Rc<dyn Fn(String)>,
}

impl View for CommandRow {
    fn body(&self, _cx: &Scope) -> AnyView {
        let item = self.item.clone();
        let on_run = self.on_run.clone();
        let id = item.id.clone();
        let row_id = id.clone();
        let hover_id = id.clone();
        let on_highlight = self.on_highlight.clone();
        let highlighted = self.is_highlighted;
        let enabled = item.is_enabled;
        let label = AnyView::new(
            Row::new(
                VerticalAlignment::Center,
                10.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(
                        AnyView::new(crate::primitives::media::Icon::semantic_with_ref(
                            command_icon(&self.command),
                            crate::theme::SemanticIconSize::medium(),
                            if highlighted {
                                SemanticColorRef::on_surface()
                            } else {
                                SemanticColorRef::on_surface_muted()
                            },
                        ))
                        .accessibility_hidden(true),
                    ),
                    AnyView::new(
                        Text::new(item.title.clone()).semantic_font(SemanticFontRef::label()),
                    ),
                    {
                        let subtitle = item.subtitle.clone();
                        if subtitle.is_empty() {
                            AnyView::new(crate::EmptyView)
                        } else {
                            AnyView::new(
                                Text::new(subtitle)
                                    .semantic_font(SemanticFontRef::caption())
                                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
                            )
                        }
                    },
                    AnyView::new(Spacer::new(0.0)),
                    shortcut_chip(self.command.shortcut.clone()),
                ])),
            )
            .frame(None, None, None, None, Some(f32::INFINITY), None),
        );
        let row = Button::with_options(
            label,
            move || on_run(&row_id),
            ButtonRole::Normal,
            enabled,
            false,
            highlighted,
            None,
        )
        .accessibility(move |semantics| {
            semantics.role = AccessibilityRole::Button;
            semantics.label = item.title.clone();
            semantics.help = if item.subtitle.is_empty() {
                String::new()
            } else {
                item.subtitle.clone()
            };
            semantics.state = AccessibilityState {
                is_enabled: item.is_enabled,
                is_selected: highlighted,
                ..Default::default()
            };
        })
        .frame(None, None, None, None, Some(f32::INFINITY), None);
        AnyView::new(
            AnyView::new(row.button_style(GhostButtonStyle)).on_hover(move |hovered| {
                if hovered && enabled {
                    on_highlight(hover_id.clone());
                }
            }),
        )
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

fn command_icon(command: &PaletteCommand) -> BundleImageResource {
    command.icon.clone().unwrap_or_else(navigation::arrow_right)
}

fn shortcut_chip(shortcut: Option<KeyboardShortcut>) -> AnyView {
    match shortcut {
        Some(shortcut) => {
            let caps = shortcut.caps(shortcut_platform());
            // One quiet chip reads faster than a row of tiny keycaps and keeps
            // the trailing slot from dominating a command row.
            AnyView::new(
                Text::new(caps.join(" "))
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_variant())
                    .padding_edges(7.0, 4.0)
                    .semantic_background(SemanticColorRef::surface_variant())
                    .corner_radius(5.0)
                    .accessibility_hidden(true),
            )
        }
        None => AnyView::new(crate::EmptyView),
    }
}

fn shortcut_platform() -> KeyboardShortcutPlatform {
    KeyboardShortcutPlatform::current()
}

/// Shortcut hints plus the visible/total command count.
struct PaletteFooter {
    visible_count: usize,
    total: usize,
}

impl View for PaletteFooter {
    fn body(&self, _cx: &Scope) -> AnyView {
        let hint = |key: &str, label: &str| {
            AnyView::new(Row::new(
                VerticalAlignment::Center,
                5.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(KeyCap::title(key)),
                    AnyView::new(
                        Text::new(label)
                            .semantic_font(SemanticFontRef::caption())
                            .semantic_foreground(SemanticColorRef::on_surface_muted()),
                    ),
                ])),
            ))
        };
        AnyView::new(
            Row::new(
                VerticalAlignment::Center,
                14.0,
                AnyView::new(TupleView::new(vec![
                    hint("↑↓", "navigate"),
                    hint("↵", "run"),
                    hint("esc", "close"),
                    AnyView::new(Spacer::new(0.0)),
                    AnyView::new(
                        Text::new(format!("{} of {}", self.visible_count, self.total))
                            .semantic_font(SemanticFontRef::caption())
                            .semantic_foreground(SemanticColorRef::on_surface_muted()),
                    ),
                ])),
            )
            .padding_edges(14.0, 8.0)
            .accessibility(|semantics| {
                semantics.role = AccessibilityRole::Group;
                semantics.label = "Keyboard shortcuts".to_string();
            }),
        )
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
