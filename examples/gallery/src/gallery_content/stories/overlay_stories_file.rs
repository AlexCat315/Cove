//! Layout story file: menus, popovers, modal, disclosure and tabs.
//!
//! Swift source: `GalleryContent/Stories/Layout/OverlayStories.swift`, which holds
//! six independent stories (`MenuStory`, `ContextMenuStory`, `PopoverStory`,
//! `ModalStory`, `DisclosureStory`, `TabsStory`).
//!
//! The logic worth pinning is the menu tree (depth, disabled propagation, the
//! 100-command scroll case) and tab closing, where the fallback must pick the
//! nearest remaining tab rather than jumping to the first.

use cove_composition::Scope;
use cove_core::State;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::layout::tab_view::{TabItem, TabOverflow, TabVariant, TabView};
use cove_ui_compose::primitives::overlays::menu::{MenuEntry, MenuItem, MenuSubmenu};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View};

use super::feedback::StorySection;
use crate::gallery_content::catalog::GalleryOptions;

/// The command count in the "scrollable menu" section.
pub const MANY_COMMANDS: usize = 100;
/// Every tenth command is a group submenu.
pub const GROUP_STRIDE: usize = 10;
/// The submenu width the workspace menu declares.
pub const SUBMENU_WIDTH: f32 = 240.0;
/// The modal's declared geometry.
pub const MODAL_WIDTH: f32 = 440.0;
pub const MODAL_HEIGHT: f32 = 220.0;

/// The initial document tabs.
pub const INITIAL_DOCUMENTS: [&str; 3] = ["Scene.cove", "Material.cove", "Notes.md"];

/// How deep a menu entry nests, counting from the root.
pub fn menu_depth(entries: &[MenuEntry]) -> usize {
    entries
        .iter()
        .map(|entry| match entry {
            MenuEntry::Submenu(submenu) => 1 + menu_depth(&submenu.entries),
            MenuEntry::Item(_) | MenuEntry::Label { .. } | MenuEntry::Separator { .. } => 0,
        })
        .max()
        .unwrap_or(0)
}

/// The command count a menu contains, counting leaf items only.
///
/// Labels and separators are decoration, so they are excluded; a submenu counts
/// its leaves.
pub fn leaf_count(entries: &[MenuEntry]) -> usize {
    entries
        .iter()
        .map(|entry| match entry {
            MenuEntry::Item(_) => 1,
            MenuEntry::Submenu(submenu) => leaf_count(&submenu.entries),
            MenuEntry::Label { .. } | MenuEntry::Separator { .. } => 0,
        })
        .sum()
}

/// Builds the story's workspace menu.
///
/// `is_enabled` gates the interactive commands; `checked` drives the pin's check
/// mark. Disabled entries stay visible so the menu can explain itself.
pub fn workspace_entries(is_enabled: bool, checked: bool) -> Vec<MenuEntry> {
    vec![
        MenuEntry::Label {
            id: 100,
            title: "Workspace".to_owned(),
        },
        MenuEntry::Item(MenuItem::new(101, "Copy", || {}).enabled(is_enabled)),
        MenuEntry::Item(
            MenuItem::new(102, "Pin item", || {})
                .enabled(is_enabled)
                .selected(checked),
        ),
        MenuEntry::separator(103),
        MenuEntry::Submenu(MenuSubmenu {
            id: 104,
            title: "Settings".to_owned(),
            entries: vec![
                MenuEntry::Item(MenuItem::new(105, "General", || {})),
                MenuEntry::Submenu(MenuSubmenu {
                    id: 106,
                    title: "Appearance".to_owned(),
                    entries: vec![
                        MenuEntry::Item(MenuItem::new(107, "System theme", || {}).selected(true)),
                        MenuEntry::Submenu(MenuSubmenu {
                            id: 108,
                            title: "Accent color".to_owned(),
                            entries: vec![
                                MenuEntry::Item(MenuItem::new(109, "Blue", || {})),
                                MenuEntry::Item(MenuItem::new(110, "Green", || {})),
                            ],
                            is_enabled: true,
                            width: SUBMENU_WIDTH,
                            icon: None,
                        }),
                    ],
                    is_enabled: true,
                    width: SUBMENU_WIDTH,
                    icon: None,
                }),
                MenuEntry::Item(MenuItem::new(111, "Restricted", || {}).enabled(false)),
            ],
            is_enabled,
            width: SUBMENU_WIDTH,
            icon: None,
        }),
        MenuEntry::Item(MenuItem::new(112, "Unavailable", || {}).enabled(false)),
        MenuEntry::Item(MenuItem::new(113, "Delete", || {}).enabled(is_enabled)),
    ]
}

/// Builds the 100-entry menu, where every tenth row is a group submenu.
pub fn many_entries() -> Vec<MenuEntry> {
    (0..MANY_COMMANDS)
        .map(|index| {
            if index % GROUP_STRIDE == 0 {
                MenuEntry::Submenu(MenuSubmenu {
                    id: 1_000 + index as u64,
                    title: format!("Group {index}"),
                    entries: vec![
                        MenuEntry::Item(MenuItem::new(
                            2_000 + index as u64,
                            "Inspect group",
                            || {},
                        )),
                        MenuEntry::Item(MenuItem::new(3_000 + index as u64, "Export group", || {})),
                    ],
                    is_enabled: true,
                    width: SUBMENU_WIDTH,
                    icon: None,
                })
            } else {
                MenuEntry::Item(MenuItem::new(
                    4_000 + index as u64,
                    format!("Command {index}"),
                    || {},
                ))
            }
        })
        .collect()
}

/// Which tab to select after closing the one at `closed`.
///
/// The nearest remaining tab wins: the one that took the closed tab's slot, or
/// the previous one when the last tab was closed. Falling back to index 0 would
/// jump the reader to the far end of the strip.
pub fn tab_after_close(documents: &[String], closed: usize) -> Option<String> {
    if documents.is_empty() {
        return None;
    }
    let index = closed.min(documents.len() - 1);
    Some(documents[index].clone())
}

/// `MenuStory`: the native menu with nested branches.
pub struct MenuStory {
    pub options: GalleryOptions,
    status: State<String>,
    checked: State<bool>,
}

impl MenuStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            status: State::new("Choose a command".to_owned()),
            checked: State::new(true),
        }
    }

    pub fn status(&self) -> String {
        self.status.get()
    }

    pub fn record(&self, status: impl Into<String>) {
        self.status.set(status.into());
    }

    pub fn is_checked(&self) -> bool {
        self.checked.get()
    }

    /// The pin command toggles its check mark.
    pub fn toggle_checked(&self) {
        self.checked.update(|value| *value = !*value);
    }

    pub fn entries(&self) -> Vec<MenuEntry> {
        workspace_entries(self.options.is_enabled, self.checked.get())
    }

    pub fn many(&self) -> Vec<MenuEntry> {
        many_entries()
    }
}

impl View for MenuStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let status = Text::new(self.status())
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted());
        AnyView::new(StorySection::new(
            "Native menu",
            "The system menu opens at the button's corner. It supports labels, separators, submenus, disabled and destructive actions, check marks and keyboard shortcuts.",
            AnyView::new(AnyView::new(status).debug_name("menu-status")),
        ))
    }

    fn tag(&self) -> &'static str {
        "MenuStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; status, checked)
    }
}

/// `PopoverStory`: an anchored form that escapes clipping.
pub struct PopoverStory {
    pub options: GalleryOptions,
    is_open: State<bool>,
    name: State<String>,
}

impl PopoverStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            is_open: State::new(false),
            name: State::new("New project".to_owned()),
        }
    }

    pub fn is_open(&self) -> bool {
        self.is_open.get()
    }

    pub fn open(&self) {
        self.is_open.set(true);
    }

    /// The Done action records nothing and closes the popover.
    pub fn done(&self) {
        self.is_open.set(false);
    }

    pub fn name(&self) -> String {
        self.name.get()
    }

    pub fn set_name(&self, value: impl Into<String>) {
        self.name.set(value.into());
    }
}

impl View for PopoverStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let caption = Text::new(format!("Name: {}", self.name()))
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted());
        AnyView::new(StorySection::new(
            "Anchored form",
            "The overlay escapes clipping and repositions near viewport edges.",
            AnyView::new(caption),
        ))
    }

    fn tag(&self) -> &'static str {
        "PopoverStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; is_open, name)
    }
}

/// `ModalStory`: a dialog with its own draft.
pub struct ModalStory {
    pub options: GalleryOptions,
    is_open: State<bool>,
    status: State<String>,
    name: State<String>,
}

impl ModalStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            is_open: State::new(false),
            status: State::new("No changes".to_owned()),
            name: State::new("Untitled".to_owned()),
        }
    }

    pub fn is_open(&self) -> bool {
        self.is_open.get()
    }

    pub fn open(&self) {
        self.is_open.set(true);
    }

    /// Cancelling closes without recording anything.
    pub fn cancel(&self) {
        self.is_open.set(false);
    }

    /// Creating records the draft's name and closes.
    pub fn create(&self) {
        self.status.set(format!("Created {}", self.name.get()));
        self.is_open.set(false);
    }

    pub fn status(&self) -> String {
        self.status.get()
    }

    pub fn set_name(&self, value: impl Into<String>) {
        self.name.set(value.into());
    }
}

impl View for ModalStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let status = Text::new(self.status())
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted());
        AnyView::new(StorySection::new(
            "Dialog",
            "Open the dialog, cycle focus with Tab, then close with Escape or the cancel action.",
            AnyView::new(AnyView::new(status).debug_name("modal-status")),
        ))
    }

    fn tag(&self) -> &'static str {
        "ModalStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; is_open, status, name)
    }
}

/// `DisclosureStory`: caller-owned expansion, so a draft survives collapsing.
pub struct DisclosureStory {
    pub options: GalleryOptions,
    is_expanded: State<bool>,
    draft: State<String>,
}

impl DisclosureStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            is_expanded: State::new(true),
            draft: State::new("The draft survives collapse".to_owned()),
        }
    }

    pub fn is_expanded(&self) -> bool {
        self.is_expanded.get()
    }

    /// Collapsing leaves the draft untouched, which is the whole point of
    /// caller-owned expansion.
    pub fn toggle(&self) {
        self.is_expanded.update(|value| *value = !*value);
    }

    pub fn draft(&self) -> String {
        self.draft.get()
    }

    pub fn set_draft(&self, value: impl Into<String>) {
        self.draft.set(value.into());
    }
}

impl View for DisclosureStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(StorySection::new(
            "Collapsible content",
            "Expansion is caller-owned, so search and Expand All can control it.",
            AnyView::new(
                Text::new(self.draft())
                    .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption()),
            ),
        ))
    }

    fn tag(&self) -> &'static str {
        "DisclosureStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; is_expanded, draft)
    }
}

/// `TabsStory`: content tabs, variants and closable documents.
pub struct TabsStory {
    pub options: GalleryOptions,
    selection: State<String>,
    segment: State<String>,
    documents: State<Vec<String>>,
    document_selection: State<String>,
}

impl TabsStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            selection: State::new("overview".to_owned()),
            segment: State::new("day".to_owned()),
            documents: State::new(
                INITIAL_DOCUMENTS
                    .iter()
                    .map(|name| (*name).to_owned())
                    .collect(),
            ),
            document_selection: State::new("Scene.cove".to_owned()),
        }
    }

    pub fn selection(&self) -> String {
        self.selection.get()
    }

    pub fn segment(&self) -> String {
        self.segment.get()
    }

    pub fn documents(&self) -> Vec<String> {
        self.documents.get()
    }

    pub fn document_selection(&self) -> String {
        self.document_selection.get()
    }

    /// The five content tabs, one of which is disabled.
    pub fn content_tabs(&self) -> Vec<TabItem<String>> {
        let enabled = self.options.is_enabled;
        vec![
            TabItem::new(
                "overview".to_owned(),
                "Overview",
                Text::new("Project overview"),
            ),
            TabItem::new(
                "disabled".to_owned(),
                "Unavailable",
                Text::new("Disabled content"),
            )
            .enabled(false),
            TabItem::new(
                "activity".to_owned(),
                "Activity",
                Text::new("Recent activity"),
            )
            .badge("3"),
            TabItem::new(
                "settings".to_owned(),
                "Settings",
                Text::new("Project settings"),
            )
            .enabled(enabled),
            TabItem::new(
                "permissions".to_owned(),
                "Workspace permissions and international collaboration 👩🏽‍💻",
                Text::new("Permission settings"),
            ),
        ]
    }

    /// Closes the document at `index` and picks the fallback selection.
    ///
    /// Returns the new selection, or `None` when every document is gone.
    pub fn close_document(&self, index: usize) -> Option<String> {
        let mut documents = self.documents.get();
        if index >= documents.len() {
            let current = self.document_selection.get();
            return (!current.is_empty()).then_some(current);
        }
        let closed = documents.remove(index);
        let fallback = tab_after_close(&documents, index);
        self.documents.set(documents);
        let closing_selected = self.document_selection.get() == closed;
        if closing_selected {
            match &fallback {
                Some(next) => self.document_selection.set(next.clone()),
                None => self.document_selection.set(String::new()),
            }
        }
        fallback
    }

    /// Restores the initial documents.
    pub fn reset_documents(&self) {
        self.documents.set(
            INITIAL_DOCUMENTS
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
        );
        self.document_selection.set(INITIAL_DOCUMENTS[0].to_owned());
    }

    pub fn overflow(&self) -> TabOverflow {
        TabOverflow::Menu
    }

    pub fn variant(&self) -> TabVariant {
        TabVariant::Segmented
    }
}

impl View for TabsStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let tabs = AnyView::new(
            TabView::new(self.selection.binding(), self.content_tabs())
                .variant(self.variant())
                .overflow(self.overflow())
                .enabled(self.options.is_enabled)
                .frame_height(180.0)
                .debug_name("tabs-content"),
        );

        let segments = vec![
            TabItem::new("day".to_owned(), "Day", Text::new("Daily agenda")),
            TabItem::new("week".to_owned(), "Week", Text::new("Weekly agenda")),
            TabItem::new("month".to_owned(), "Month", Text::new("Monthly agenda")),
        ];
        let segmented = AnyView::new(
            TabView::new(self.segment.binding(), segments)
                .variant(TabVariant::Segmented)
                .enabled(self.options.is_enabled)
                .frame_height(120.0)
                .debug_name("tabs-segmented"),
        );

        let close = {
            let documents = self.documents.clone();
            let selection = self.document_selection.clone();
            AnyView::new(
                cove_ui_compose::primitives::controls::Button::title("Close selected", move || {
                    let current = documents.get();
                    let selected = selection.get();
                    let index = current.iter().position(|name| *name == selected).unwrap_or(0);
                    let mut next = current.clone();
                    if !next.is_empty() {
                        next.remove(index.min(next.len() - 1));
                        let fallback = next
                            .get(index.saturating_sub(1))
                            .or_else(|| next.first())
                            .cloned()
                            .unwrap_or_default();
                        selection.set(fallback);
                        documents.set(next);
                    }
                })
                .enabled(self.options.is_enabled && !self.documents.get().is_empty()),
            )
        };
        let document_tabs: Vec<TabItem<String>> = self
            .documents
            .get()
            .into_iter()
            .map(|name| {
                let label = name.clone();
                TabItem::new(name.clone(), label, Text::new(format!("Contents of {name}")))
            })
            .collect();
        let documents = AnyView::new(
            TabView::new(self.document_selection.binding(), document_tabs)
                .variant(TabVariant::Card)
                .overflow(TabOverflow::Scroll)
                .frame_height(120.0)
                .debug_name("tabs-documents"),
        );

        let caption = Text::new(format!(
            "Active tab: {} · segment: {} · document: {}",
            self.selection(),
            self.segment(),
            self.document_selection()
        ))
        .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
        .semantic_foreground(SemanticColorRef::on_surface_muted());

        AnyView::new(
            BoxView::new(
                cove_core::scene::FlexDirection::Column,
                cove_core::scene::Align::Stretch,
                cove_core::scene::Justify::FlexStart,
                cove_core::scene::Wrap::NoWrap,
                20.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(StorySection::new(
                        "Content tabs",
                        "The selected tab stays visible. More tabs lists hidden pages. Arrow keys, Home and End reach every enabled tab; long titles retain their full tooltip and name.",
                        tabs,
                    )),
                    AnyView::new(StorySection::new(
                        "Segmented variant",
                        "Three exclusive segments keep the same keyboard contract as the bar.",
                        segmented,
                    )),
                    AnyView::new(StorySection::new(
                        "Closable documents",
                        "Closing the selected document selects its neighbour, not the first tab.",
                        AnyView::new(TupleView::new(vec![documents, close])),
                    )),
                    AnyView::new(caption),
                ])),
            )
            .debug_name("tabs-story"),
        )
    }

    fn tag(&self) -> &'static str {
        "TabsStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; selection, segment, documents, document_selection)
    }
}
