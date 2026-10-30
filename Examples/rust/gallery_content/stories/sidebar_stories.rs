//! Layout stories: resizable and nested navigation.
//!
//! Swift sources:
//! `GalleryContent/Stories/Layout/{ResizableSidebarExample,NestedSidebarExample}.swift`.
//!
//! Both examples exercise the same adaptive sidebar at a 600pt expanded
//! threshold, so the thresholds and section builders live here as plain data the
//! tests can assert against.

use cove_core::State;
use cove_ui_compose::primitives::layout::sidebar::{SidebarItem, SidebarSection};
use cove_ui_compose::primitives::overlays::menu::MenuEntry;

use crate::gallery_content::catalog::GalleryOptions;

/// The compact→rail breakpoint both examples declare.
pub const COMPACT_MAXIMUM_WIDTH: f32 = 400.0;
/// The rail→expanded breakpoint both examples declare.
pub const EXPANDED_MINIMUM_WIDTH: f32 = 600.0;
/// The width the resizable example starts at and resets to.
pub const DEFAULT_SIDEBAR_WIDTH: f32 = 240.0;
/// The minimum content width the resize handle respects.
pub const MINIMUM_CONTENT_WIDTH: f32 = 240.0;

/// `ResizableSidebarExample`: drag the separator, or focus it and use the
/// keyboard, to change the sidebar width.
pub struct ResizableSidebarExample {
    pub options: GalleryOptions,
    width: State<f32>,
    selected: State<Option<usize>>,
    draft: State<String>,
}

impl ResizableSidebarExample {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            width: State::new(DEFAULT_SIDEBAR_WIDTH),
            selected: State::new(Some(0)),
            draft: State::new("A retained document draft".to_owned()),
        }
    }

    pub fn width(&self) -> f32 {
        self.width.get()
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected.get()
    }

    /// Restores the authored width, which is what the reset button does.
    pub fn reset_width(&self) {
        self.width.set(DEFAULT_SIDEBAR_WIDTH);
    }

    /// The requested width rounded to whole points, as the caption shows it.
    pub fn requested_width_label(&self) -> String {
        format!("Requested width: {} pt", self.width.get().round() as i32)
    }

    /// Two documents, both gated on the story's enabled flag.
    pub fn sections(&self) -> Vec<SidebarSection<usize>> {
        let enabled = self.options.is_enabled;
        vec![SidebarSection::new(
            "documents",
            vec![
                SidebarItem::new(0, "Overview").enabled(enabled),
                SidebarItem::new(1, "Documents").enabled(enabled),
            ],
        )]
    }
}

impl cove_ui_compose::View for ResizableSidebarExample {
    fn body(&self) -> cove_ui_compose::AnyView {
        use cove_ui_compose::modifiers::layout::LayoutFrameExt;
        use cove_ui_compose::ViewExt;

        let content = cove_ui_compose::primitives::text::Text::new(self.requested_width_label());
        let sidebar = cove_ui_compose::primitives::layout::sidebar::AdaptiveSidebar::new(
            "Document navigation",
            self.selected.binding(),
            self.sections(),
        )
        .content(content)
        .width(self.width.binding())
        .options(sidebar_options());
        cove_ui_compose::AnyView::new(
            cove_ui_compose::AnyView::new(sidebar).frame_height(220.0),
        )
    }

    fn tag(&self) -> &'static str {
        "ResizableSidebarExample"
    }
}

/// The layout thresholds both examples declare, in one place.
fn sidebar_options() -> cove_ui_compose::primitives::layout::sidebar::AdaptiveSidebarOptions {
    let mut options =
        cove_ui_compose::primitives::layout::sidebar::AdaptiveSidebarOptions::default();
    options.layout.compact_maximum_width = COMPACT_MAXIMUM_WIDTH;
    options.layout.expanded_minimum_width = EXPANDED_MINIMUM_WIDTH;
    options
}

/// The nested workspace's selection id.
pub const NESTED_DEFAULT_SELECTION: &str = "nested-overview";
/// The group the nested example starts expanded.
pub const NESTED_DEFAULT_EXPANDED: &str = "nested-projects";

/// `NestedSidebarExample`: groups expand without changing the page.
pub struct NestedSidebarExample {
    pub options: GalleryOptions,
    selection: State<Option<String>>,
    expanded: State<Vec<String>>,
    is_open: State<bool>,
    draft: State<String>,
    last_action: State<String>,
}

impl NestedSidebarExample {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            selection: State::new(Some(NESTED_DEFAULT_SELECTION.to_owned())),
            expanded: State::new(vec![NESTED_DEFAULT_EXPANDED.to_owned()]),
            is_open: State::new(false),
            draft: State::new("A retained nested document".to_owned()),
            last_action: State::new("None".to_owned()),
        }
    }

    pub fn selection(&self) -> Option<String> {
        self.selection.get()
    }

    pub fn expanded(&self) -> Vec<String> {
        self.expanded.get()
    }

    pub fn is_open(&self) -> bool {
        self.is_open.get()
    }

    pub fn last_action(&self) -> String {
        self.last_action.get()
    }

    /// Expands both authored groups.
    pub fn expand_groups(&self) {
        self.expanded.set(vec!["nested-projects".to_owned(), "nested-team".to_owned()]);
    }

    /// Collapses every group. The selection is deliberately preserved.
    pub fn collapse_groups(&self) {
        self.expanded.set(Vec::new());
    }

    /// Opens the drawer that hosts the sidebar on compact layouts.
    pub fn open_drawer(&self) {
        self.is_open.set(true);
    }

    pub fn record(&self, action: impl Into<String>) {
        self.last_action.set(action.into());
    }

    /// The workspace tree: two top-level items, one with two levels of children.
    pub fn sections(&self) -> Vec<SidebarSection<String>> {
        let enabled = self.options.is_enabled;
        let audit = SidebarItem::new("nested-audit".to_owned(), "Audit log").context_menu(vec![
            MenuEntry::Item(cove_ui_compose::primitives::overlays::menu::MenuItem::new(20, "Archive", || {})),
            MenuEntry::Item(cove_ui_compose::primitives::overlays::menu::MenuItem::new(21, "Duplicate", || {})),
        ]);
        let team = SidebarItem::new("nested-team".to_owned(), "Design team")
            .children(vec![audit, SidebarItem::new("nested-billing".to_owned(), "Billing").enabled(false)]);
        let projects = SidebarItem::new("nested-projects".to_owned(), "Project spaces")
            .children(vec![SidebarItem::new("nested-welcome".to_owned(), "Getting started"), team]);
        vec![SidebarSection::new(
            "nested-workspace",
            vec![
                SidebarItem::new("nested-overview".to_owned(), "Workspace overview").enabled(enabled),
                projects,
            ],
        )]
    }

    /// Every id that can be selected: leaves only, since a group click expands.
    pub fn selectable_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        fn walk(items: &[SidebarItem<String>], ids: &mut Vec<String>) {
            for item in items {
                if item.children.is_empty() {
                    if item.is_enabled {
                        ids.push(item.id.clone());
                    }
                } else {
                    walk(&item.children, ids);
                }
            }
        }
        for section in self.sections() {
            walk(&section.items, &mut ids);
        }
        ids
    }
}

impl cove_ui_compose::View for NestedSidebarExample {
    fn body(&self) -> cove_ui_compose::AnyView {
        use cove_ui_compose::modifiers::layout::LayoutFrameExt;
        use cove_ui_compose::ViewExt;

        let selection = self.selection.get();
        let content = cove_ui_compose::primitives::text::Text::new(format!(
            "Last action: {}",
            self.last_action()
        ));
        let sidebar = cove_ui_compose::primitives::layout::sidebar::AdaptiveSidebar::new(
            "Nested workspace",
            self.selection.binding(),
            self.sections(),
        )
        .content(content)
        .presentation(self.is_open.binding())
        .options(sidebar_options());
        let _ = selection;
        cove_ui_compose::AnyView::new(
            cove_ui_compose::AnyView::new(sidebar).frame_height(280.0),
        )
    }

    fn tag(&self) -> &'static str {
        "NestedSidebarExample"
    }
}
