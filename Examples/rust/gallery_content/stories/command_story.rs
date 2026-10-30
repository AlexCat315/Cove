//! Overlay story: the command palette.
//!
//! Swift source: `GalleryContent/Stories/Overlays/CommandStory.swift`.
//!
//! Eight commands across four groups, one disabled. Ranking and projection come
//! from `CommandSearch`, so this story pins the authored catalogue and the
//! recent-command ordering the prose promises.

use cove_core::State;
use cove_ui_compose::modifiers::layout::FlexExt;
use cove_ui_compose::LayoutDebugExt;
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::overlays::{
    destination, display_entries, ranked, record_recent, CommandEntry, CommandItem,
};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, BoxView, TupleView, View, ViewExt};

use super::feedback::StorySection;
use crate::gallery_content::catalog::GalleryOptions;

/// The recent-command cap the palette uses.
pub const RECENT_LIMIT: usize = 3;

/// The authored commands, in declaration order.
///
/// `Deploy to Device` is disabled so the arrow-key skip is observable.
pub fn commands() -> Vec<CommandItem> {
    vec![
        CommandItem::new("Toggle Theme", "Toggle Theme", "View")
            .with_keywords(["dark", "light", "appearance"]),
        CommandItem::new("Go to File", "Go to File", "Navigate").with_keywords(["jump", "search"]),
        CommandItem::new("Go to Line", "Go to Line", "Navigate").with_keywords(["jump"]),
        CommandItem::new("Format Document", "Format Document", "Edit")
            .with_keywords(["pretty", "reindent"]),
        CommandItem::new("Rename Symbol", "Rename Symbol", "Edit").with_keywords(["refactor"]),
        CommandItem::new("Run Build", "Run Build", "Build").with_keywords(["compile"]),
        CommandItem::new("Clean Build Folder", "Clean Build Folder", "Build")
            .with_keywords(["wipe"]),
        CommandItem::new("Deploy to Device", "Deploy to Device", "Build")
            .with_keywords(["ship"])
            .disabled(),
    ]
}

/// `CommandPaletteStory`: the palette with search, recents and execution.
pub struct CommandStory {
    pub options: GalleryOptions,
    is_presented: State<bool>,
    is_custom_presented: State<bool>,
    last_run: State<String>,
    recent: State<Vec<String>>,
}

impl CommandStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            is_presented: State::new(false),
            is_custom_presented: State::new(false),
            last_run: State::new("none".to_owned()),
            recent: State::new(Vec::new()),
        }
    }

    pub fn last_run(&self) -> String {
        self.last_run.get()
    }

    pub fn recent(&self) -> Vec<String> {
        self.recent.get()
    }

    pub fn is_presented(&self) -> bool {
        self.is_presented.get()
    }

    pub fn open(&self) {
        self.is_presented.set(true);
    }

    pub fn close(&self) {
        self.is_presented.set(false);
    }

    /// Runs a command by id, records it as recent and closes the palette.
    ///
    /// A disabled command is ignored rather than run, matching the sheet's
    /// behaviour for an unavailable row.
    pub fn run(&self, id: &str) -> bool {
        let Some(item) = commands().into_iter().find(|item| item.id == id) else {
            return false;
        };
        if !item.is_enabled {
            return false;
        }
        self.last_run.set(item.title.clone());
        let mut recent = self.recent.get();
        record_recent(&mut recent, id, RECENT_LIMIT);
        self.recent.set(recent);
        self.close();
        true
    }

    /// Ranks the catalogue for a query.
    pub fn ranked(&self, query: &str) -> Vec<CommandItem> {
        ranked(&commands(), query)
    }

    /// The rows the palette shows for a query: recents first when the query is
    /// empty, otherwise the ranked matches grouped by section.
    pub fn entries(&self, query: &str) -> Vec<CommandEntry> {
        let trimmed = query.trim();
        // Recents show only when the query is empty, matching Swift's
        // `showsRecentCommands && trimmedQuery.isEmpty`.
        display_entries(
            &self.ranked(trimmed),
            &self.recent.get(),
            trimmed.is_empty(),
            RECENT_LIMIT,
            trimmed.is_empty(),
        )
    }

    /// The next highlight for an arrow press.
    pub fn next_highlight(&self, count: usize, current: Option<usize>, delta: i64, wraps: bool) -> Option<usize> {
        destination(count, current, delta, wraps)
    }
}

impl View for CommandStory {
    fn body(&self) -> AnyView {
        let enabled = self.options.is_enabled;
        let open = self.is_presented.clone();
        let open_custom = self.is_custom_presented.clone();

        let caption = Text::new(format!("Last run: {}", self.last_run()))
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted());

        let rows = AnyView::new(BoxView::new(
            cove_core::scene::FlexDirection::Row,
            cove_core::scene::Align::Center,
            cove_core::scene::Justify::FlexStart,
            cove_core::scene::Wrap::Wrap,
            12.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    AnyView::new(
                        Button::title("Open command palette", move || open.set(true))
                            .enabled(enabled),
                    )
                    .debug_name("command-open"),
                ),
                AnyView::new(
                    AnyView::new(
                        Button::title("Open with project footer", move || open_custom.set(true))
                            .enabled(enabled),
                    )
                    .debug_name("command-open-custom"),
                ),
                AnyView::new(caption),
            ])),
        ));

        AnyView::new(StorySection::new(
            "Command palette",
            "Try “GO”, “go f” or “appearance”. Arrows move, Page Up/Down jump, Return runs and Escape closes. Disabled commands are skipped; run one and reopen to see it under Recent.",
            rows,
        ))
    }

    fn tag(&self) -> &'static str {
        "CommandStory"
    }
}
