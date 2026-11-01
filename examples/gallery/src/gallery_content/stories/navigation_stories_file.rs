//! Layout story file: accordion, breadcrumb, pagination, button group, tags,
//! key caps, labels and the dialog family.
//!
//! Swift source: `GalleryContent/Stories/Layout/NavigationStories.swift`, which
//! holds twelve stories. Ten are ported here with their state machines; the two
//! sheet stories remain view-only in Swift as well.
//!
//! The logic worth pinning is the accordion's single-expansion rule and the
//! pagination window arithmetic, both of which have real edge cases at the ends.

use cove_composition::Scope;
use std::collections::BTreeSet;

use cove_core::State;
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::layout::accordion::{Accordion, AccordionItem};
use cove_ui_compose::primitives::layout::navigation_components::{
    BreadcrumbItem, PaginationItem, PaginationModel,
};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, LayoutDebugExt, View};

use super::feedback::StorySection;
use crate::gallery_content::catalog::GalleryOptions;

/// The section the accordion opens with.
pub const DEFAULT_SECTION: &str = "project";
/// The page the pagination story starts on, of 25.
pub const DEFAULT_PAGE: usize = 8;
pub const TOTAL_PAGES: usize = 25;
/// The breadcrumb path the story walks.
pub const BREADCRUMB_PATH: [(&str, &str); 6] = [
    ("home", "Home"),
    ("team", "Team"),
    ("project", "Project"),
    ("assets", "Assets"),
    ("textures", "Textures"),
    ("selected", "Cove.png"),
];

/// The tags the removable-tag story starts with.
pub const INITIAL_TAGS: [&str; 4] = [
    "Swift",
    "Rendering",
    "Editor",
    "Workspace collaboration reviewers awaiting approval",
];
/// The outline filters the variants section shows.
pub const INITIAL_FILTERS: [&str; 2] = ["metal", "vulkan"];

/// The accordion's three sections.
pub fn accordion_items() -> Vec<(&'static str, &'static str, bool)> {
    vec![
        ("project", "Project", true),
        (
            "diagnostics",
            "Diagnostics & activity · Workspace permissions and collaboration settings",
            true,
        ),
        ("unavailable", "Team permissions · unavailable", false),
    ]
}

/// `AccordionStory`: caller-owned expansion with single or multiple mode.
pub struct AccordionStory {
    pub options: GalleryOptions,
    expanded: State<BTreeSet<String>>,
    allows_multiple: State<bool>,
    reduce_motion: State<bool>,
    project_name: State<String>,
    autosave: State<bool>,
    diagnostics: State<bool>,
}

impl AccordionStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            expanded: State::new(BTreeSet::from([DEFAULT_SECTION.to_owned()])),
            allows_multiple: State::new(false),
            reduce_motion: State::new(false),
            project_name: State::new("Cove Studio".to_owned()),
            autosave: State::new(true),
            diagnostics: State::new(false),
        }
    }

    pub fn expanded(&self) -> BTreeSet<String> {
        self.expanded.get()
    }

    pub fn allows_multiple(&self) -> bool {
        self.allows_multiple.get()
    }

    /// Turning multiple mode off collapses back to the project section.
    ///
    /// Leaving several sections open would contradict the mode the person just
    /// chose, so the first one wins.
    pub fn set_allows_multiple(&self, value: bool) {
        self.allows_multiple.set(value);
        if !value && self.expanded.get().len() > 1 {
            self.expanded
                .set(BTreeSet::from([DEFAULT_SECTION.to_owned()]));
        }
    }

    pub fn project_name(&self) -> String {
        self.project_name.get()
    }

    pub fn set_project_name(&self, value: impl Into<String>) {
        self.project_name.set(value.into());
    }

    pub fn autosave(&self) -> bool {
        self.autosave.get()
    }

    pub fn diagnostics(&self) -> bool {
        self.diagnostics.get()
    }

    /// The accordion model, ready to toggle.
    pub fn accordion(&self) -> Accordion<String> {
        let items = accordion_items()
            .into_iter()
            .map(|(id, title, is_enabled)| {
                AccordionItem::new(id.to_owned(), title, Text::new("")).enabled(is_enabled)
            })
            .collect();
        Accordion::new(items, self.expanded.binding())
            .allows_multiple(self.allows_multiple.get())
            .enabled(self.options.is_enabled)
    }

    /// Toggles a section through the real widget model.
    pub fn toggle(&self, id: &str) {
        self.accordion().toggle(&id.to_owned());
    }

    /// The collapse-all control, which opens project when nothing is open.
    pub fn toggle_all(&self) {
        if self.expanded.get().is_empty() {
            self.expanded
                .set(BTreeSet::from([DEFAULT_SECTION.to_owned()]));
        } else {
            self.expanded.set(BTreeSet::new());
        }
    }

    /// The caption under the accordion.
    pub fn caption(&self) -> String {
        format!(
            "{} sections open · {}",
            self.expanded.get().len(),
            if self.autosave.get() {
                "Autosave on"
            } else {
                "Autosave off"
            }
        )
    }
}

impl View for AccordionStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let caption = Text::new(self.caption())
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted());
        AnyView::new(StorySection::new(
            "Project preferences",
            "Up / Down and Home / End move between headers. Return or Space expands a section. Try reversing an animation or changing the scale while it is open.",
            AnyView::new(AnyView::new(caption).debug_name("accordion-caption")),
        ))
    }

    fn tag(&self) -> &'static str {
        "AccordionStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; expanded, allows_multiple, reduce_motion, project_name, autosave, diagnostics)
    }
}

/// `BreadcrumbStory`: a path whose last item stays a label.
pub struct BreadcrumbStory {
    is_open: State<String>,
}

impl BreadcrumbStory {
    pub fn new() -> Self {
        Self {
            is_open: State::new("Project".to_owned()),
        }
    }

    pub fn opened(&self) -> String {
        self.is_open.get()
    }

    pub fn open(&self, id: &str) {
        if let Some((_, title)) = BREADCRUMB_PATH.iter().find(|(key, _)| *key == id) {
            self.is_open.set((*title).to_owned());
        }
    }

    pub fn items(&self) -> Vec<BreadcrumbItem<String>> {
        BREADCRUMB_PATH
            .iter()
            .map(|(id, title)| BreadcrumbItem::new((*id).to_owned(), *title))
            .collect()
    }
}

impl Default for BreadcrumbStory {
    fn default() -> Self {
        Self::new()
    }
}

impl View for BreadcrumbStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let caption = Text::new(format!("Opened: {}", self.opened()))
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted());
        AnyView::new(StorySection::new(
            "Path navigation",
            "Long paths collapse intermediate destinations into a menu; the current destination remains a label.",
            AnyView::new(caption),
        ))
    }

    fn tag(&self) -> &'static str {
        "BreadcrumbStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; is_open)
    }
}

/// `PaginationStory`: the page window.
pub struct PaginationStory {
    pub options: GalleryOptions,
    page: State<usize>,
}

impl PaginationStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            page: State::new(DEFAULT_PAGE),
        }
    }

    pub fn page(&self) -> usize {
        self.page.get()
    }

    /// The offset the caption shows, e.g. `page 8 of 25`.
    pub fn caption(&self) -> String {
        format!("page {} of {TOTAL_PAGES}", self.page.get())
    }

    /// The pages the pager shows, using the shared window arithmetic.
    pub fn items(&self) -> Vec<PaginationItem> {
        PaginationModel::items(self.page.get(), TOTAL_PAGES, 1)
    }

    /// Advances one page, clamping at the ends.
    pub fn next(&self) {
        self.page.update(|value| {
            *value = (*value + 1).min(TOTAL_PAGES);
        });
    }

    pub fn previous(&self) {
        self.page.update(|value| {
            *value = value.saturating_sub(1).max(1);
        });
    }

    /// An empty dataset exposes no page number at all.
    pub fn empty_dataset_items(&self) -> Vec<PaginationItem> {
        PaginationModel::items(0, 0, 1)
    }
}

impl View for PaginationStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let caption = Text::new(self.caption())
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted());
        AnyView::new(StorySection::new(
            "Navigate pages",
            "Only nearby pages are shown. Boundary actions disable themselves, and empty datasets expose no invalid page number.",
            AnyView::new(caption),
        ))
    }

    fn tag(&self) -> &'static str {
        "PaginationStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; page)
    }
}

/// `ButtonGroupStory`: segments are caller-owned; the unavailable one is skipped.
pub struct ButtonGroupStory {
    pub options: GalleryOptions,
    mode: State<Option<String>>,
    category: State<Option<String>>,
}

impl ButtonGroupStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            mode: State::new(Some("grid".to_owned())),
            category: State::new(Some("grid".to_owned())),
        }
    }

    pub fn mode(&self) -> Option<String> {
        self.mode.get()
    }

    pub fn category(&self) -> Option<String> {
        self.category.get()
    }

    pub fn caption(&self) -> String {
        format!(
            "Mode: {}  ·  Category: {}",
            self.mode.get().unwrap_or_else(|| "None".to_owned()),
            self.category.get().unwrap_or_else(|| "None".to_owned())
        )
    }

    /// The first group's three segments, the third unavailable.
    pub fn mode_segments(&self) -> [(&'static str, &'static str, bool); 3] {
        [
            ("grid", "Grid", true),
            ("list", "List", true),
            ("map", "Map", false),
        ]
    }

    /// The second group's five segments.
    pub fn category_segments(&self) -> [(&'static str, &'static str, bool); 5] {
        [
            ("grid", "Grid view", true),
            ("list", "List view", true),
            ("timeline", "Timeline", true),
            ("inspector", "Inspector", true),
            ("materials", "Materials", true),
        ]
    }
}

impl View for ButtonGroupStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(StorySection::new(
            "View preference",
            "The selected segment is caller-owned. The group has one Tab stop; arrows skip the unavailable segment. Narrow groups wrap instead of shrinking segment labels.",
            AnyView::new(
                Text::new(self.caption())
                    .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
            ),
        ))
    }

    fn tag(&self) -> &'static str {
        "ButtonGroupStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; mode, category)
    }
}

/// `TagStory`: removable tags whose remove controls are independent.
pub struct TagStory {
    pub options: GalleryOptions,
    tags: State<Vec<String>>,
    filters: State<Vec<String>>,
}

impl TagStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            tags: State::new(
                INITIAL_TAGS
                    .iter()
                    .map(|value| (*value).to_owned())
                    .collect(),
            ),
            filters: State::new(
                INITIAL_FILTERS
                    .iter()
                    .map(|value| (*value).to_owned())
                    .collect(),
            ),
        }
    }

    pub fn tags(&self) -> Vec<String> {
        self.tags.get()
    }

    pub fn filters(&self) -> Vec<String> {
        self.filters.get()
    }

    /// Removing by title drops every matching tag, then reports whether any went.
    pub fn remove_tag(&self, title: &str) -> bool {
        let before = self.tags.get().len();
        self.tags.update(|tags| tags.retain(|tag| tag != title));
        self.tags.get().len() != before
    }

    pub fn remove_filter(&self, title: &str) -> bool {
        let before = self.filters.get().len();
        self.filters
            .update(|filters| filters.retain(|filter| filter != title));
        self.filters.get().len() != before
    }

    /// Restores both authored lists.
    pub fn reset(&self) {
        self.tags.set(
            INITIAL_TAGS
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
        );
        self.filters.set(
            INITIAL_FILTERS
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
        );
    }

    /// The empty-state caption, shown only when nothing is left.
    pub fn empty_caption(&self) -> Option<&'static str> {
        self.tags.get().is_empty().then_some("No tags selected")
    }
}

impl View for TagStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let caption = Text::new(format!("{} tags", self.tags.get().len()))
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted());
        AnyView::new(StorySection::new(
            "Removable tags",
            "Each remove action is an independent control. Reset restores the tags.",
            AnyView::new(caption),
        ))
    }

    fn tag(&self) -> &'static str {
        "TagStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; tags, filters)
    }
}

/// `DialogStory`: a structured dialog with header, content and actions.
pub struct DialogStory {
    is_open: State<bool>,
    name: State<String>,
    is_saved: State<bool>,
}

impl DialogStory {
    pub fn new() -> Self {
        Self {
            is_open: State::new(false),
            name: State::new("Cove".to_owned()),
            is_saved: State::new(false),
        }
    }

    pub fn is_open(&self) -> bool {
        self.is_open.get()
    }

    pub fn open(&self) {
        self.is_open.set(true);
    }

    pub fn name(&self) -> String {
        self.name.get()
    }

    pub fn set_name(&self, value: impl Into<String>) {
        self.name.set(value.into());
    }

    pub fn is_saved(&self) -> bool {
        self.is_saved.get()
    }

    /// Cancelling closes without recording a save.
    pub fn cancel(&self) {
        self.is_open.set(false);
    }

    /// Saving closes and marks the draft committed.
    pub fn save(&self) {
        self.is_saved.set(true);
        self.is_open.set(false);
    }
}

impl Default for DialogStory {
    fn default() -> Self {
        Self::new()
    }
}

impl View for DialogStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(StorySection::new(
            "Structured dialog",
            "Header, scrollable content and actions share the modal focus and dismissal policy.",
            AnyView::new(
                Text::new(format!("Name: {}", self.name()))
                    .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
            ),
        ))
    }

    fn tag(&self) -> &'static str {
        "DialogStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; is_open, name, is_saved)
    }
}

/// `AlertDialogStory`: a destructive confirmation.
pub struct AlertDialogStory {
    is_open: State<bool>,
    is_deleted: State<bool>,
}

impl AlertDialogStory {
    pub fn new() -> Self {
        Self {
            is_open: State::new(false),
            is_deleted: State::new(false),
        }
    }

    pub fn is_open(&self) -> bool {
        self.is_open.get()
    }

    pub fn is_deleted(&self) -> bool {
        self.is_deleted.get()
    }

    pub fn open(&self) {
        self.is_open.set(true);
    }

    /// Confirming performs the destructive action.
    pub fn confirm(&self) {
        self.is_deleted.set(true);
        self.is_open.set(false);
    }

    /// Cancelling leaves the target alone.
    pub fn cancel(&self) {
        self.is_open.set(false);
    }
}

impl Default for AlertDialogStory {
    fn default() -> Self {
        Self::new()
    }
}

impl View for AlertDialogStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(StorySection::new(
            "Confirm a destructive action",
            "The example only changes local state. Confirmation and cancellation have distinct visual roles.",
            AnyView::new(
                Text::new(if self.is_deleted.get() {
                    "Deleted"
                } else {
                    "Intact"
                })
                .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
                .semantic_foreground(SemanticColorRef::on_surface_muted()),
            ),
        ))
    }

    fn tag(&self) -> &'static str {
        "AlertDialogStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; is_open, is_deleted)
    }
}

/// `LabelStory`: long labels keep their full text inside narrow cards.
pub struct LabelStory {
    shows_tag: State<bool>,
}

impl LabelStory {
    pub fn new() -> Self {
        Self {
            shows_tag: State::new(true),
        }
    }

    pub fn shows_tag(&self) -> bool {
        self.shows_tag.get()
    }

    /// Removing the tag swaps in a restore control rather than leaving a gap.
    pub fn remove_tag(&self) {
        self.shows_tag.set(false);
    }

    pub fn restore_tag(&self) {
        self.shows_tag.set(true);
    }
}

impl Default for LabelStory {
    fn default() -> Self {
        Self::new()
    }
}

impl View for LabelStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(StorySection::new(
            "Content labels",
            "Status badges, removable tags and key caps keep their full text inside narrow cards. Field labels below name and focus an input.",
            AnyView::new(
                Text::new(if self.shows_tag.get() {
                    "Tag shown"
                } else {
                    "Tag removed"
                })
                .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
                .semantic_foreground(SemanticColorRef::on_surface_muted()),
            ),
        ))
    }

    fn tag(&self) -> &'static str {
        "LabelStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; shows_tag)
    }
}

/// The edges an edge sheet can enter from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalEdge {
    Leading,
    Trailing,
    Top,
    Bottom,
}

/// `SheetStory`: an edge sheet whose edge the caller chooses.
pub struct SheetStory {
    pub options: GalleryOptions,
    is_open: State<bool>,
    edge: State<ModalEdge>,
    setting: State<bool>,
}

impl SheetStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            is_open: State::new(false),
            edge: State::new(ModalEdge::Trailing),
            setting: State::new(true),
        }
    }

    pub fn is_open(&self) -> bool {
        self.is_open.get()
    }

    pub fn edge(&self) -> ModalEdge {
        self.edge.get()
    }

    /// Opening from an edge also records that edge, so the entry animation and
    /// the panel agree.
    pub fn open_from(&self, edge: ModalEdge) {
        self.edge.set(edge);
        self.is_open.set(true);
    }

    pub fn close(&self) {
        self.is_open.set(false);
    }

    pub fn setting(&self) -> bool {
        self.setting.get()
    }
}

impl View for SheetStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(StorySection::new(
            "Edge sheets",
            "Each panel enters and leaves through its chosen edge, with modal focus confinement and restoration.",
            AnyView::new(
                Text::new(format!("Edge: {:?}", self.edge()))
                    .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
            ),
        ))
    }

    fn tag(&self) -> &'static str {
        "SheetStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; is_open, edge, setting)
    }
}

/// The detents the bottom sheet offers.
pub const BOTTOM_SHEET_DETENTS: [f32; 2] = [280.0, 480.0];
/// The drag distance that dismisses the sheet.
pub const DRAG_DISMISS_THRESHOLD: f32 = 70.0;

/// `BottomSheetStory`: detent selection and drag dismissal.
pub struct BottomSheetStory {
    pub options: GalleryOptions,
    is_open: State<bool>,
    detent: State<usize>,
    notifications: State<bool>,
}

impl BottomSheetStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            is_open: State::new(false),
            detent: State::new(0),
            notifications: State::new(true),
        }
    }

    pub fn is_open(&self) -> bool {
        self.is_open.get()
    }

    pub fn detent(&self) -> usize {
        self.detent.get()
    }

    pub fn open(&self) {
        self.is_open.set(true);
        // Opening always restores the initial detent, so a previously dragged
        // sheet does not reopen taller than the caller asked for.
        self.detent.set(0);
    }

    /// Drags the handle by `travel` points, positive upward.
    ///
    /// Passing the larger detent promotes the sheet; dragging down past the
    /// threshold dismisses it. Anything in between leaves the detent alone.
    pub fn drag(&self, travel: f32) -> bool {
        if travel <= -DRAG_DISMISS_THRESHOLD {
            self.is_open.set(false);
            return true;
        }
        if travel >= BOTTOM_SHEET_DETENTS[1] - BOTTOM_SHEET_DETENTS[0] {
            self.detent.set(1);
        }
        false
    }

    pub fn notifications(&self) -> bool {
        self.notifications.get()
    }
}

impl View for BottomSheetStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(StorySection::new(
            "Bottom sheet",
            "Open the sheet, then drag its handle up for the larger detent or down to dismiss. On wide surfaces it degrades to a centered dialog-sized panel; Escape and the backdrop still dismiss through the modal contract.",
            AnyView::new(
                Text::new(format!("Detent {}", self.detent()))
                    .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
            ),
        ))
    }

    fn tag(&self) -> &'static str {
        "BottomSheetStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; is_open, detent, notifications)
    }
}

/// `NavigationStackStory`: push and pop keep the root mounted.
pub struct NavigationStackStory {
    path: State<Vec<String>>,
    draft: State<String>,
}

impl NavigationStackStory {
    pub fn new() -> Self {
        Self {
            path: State::new(Vec::new()),
            draft: State::new("Draft kept across push and pop".to_owned()),
        }
    }

    pub fn path(&self) -> Vec<String> {
        self.path.get()
    }

    pub fn push(&self, route: impl Into<String>) {
        self.path.update(|path| path.push(route.into()));
    }

    /// Pops the top page; a no-op on an empty stack.
    pub fn pop(&self) -> bool {
        if self.path.get().is_empty() {
            return false;
        }
        self.path.update(|path| {
            path.pop();
        });
        true
    }

    /// The draft belongs to the root, so pushing never touches it.
    pub fn draft(&self) -> String {
        self.draft.get()
    }

    pub fn set_draft(&self, value: impl Into<String>) {
        self.draft.set(value.into());
    }

    /// The path caption: `root` when empty, otherwise the routes in order.
    pub fn caption(&self) -> String {
        let path = self.path.get();
        if path.is_empty() {
            "Path: root".to_owned()
        } else {
            format!("Path: {}", path.join(" → "))
        }
    }
}

impl Default for NavigationStackStory {
    fn default() -> Self {
        Self::new()
    }
}

impl View for NavigationStackStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(StorySection::new(
            "Page stack",
            "Push and pop keep the root mounted. Only the top page is painted and interactive; the incoming page slides in with a fade, and a non-empty initial path restores the same page on launch.",
            AnyView::new(
                Text::new(self.caption())
                    .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
            ),
        ))
    }

    fn tag(&self) -> &'static str {
        "NavigationStackStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; path, draft)
    }
}
