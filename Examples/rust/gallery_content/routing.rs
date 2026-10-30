//! Page-to-story dispatch and the honest record of pages that cannot be built yet.
//!
//! Swift source: the `GalleryPage.makeStory` / `baseStory` switch in
//! `GalleryContent/Stories/Feedback/FeedbackStories.swift`.
//!
//! Only pages whose story can be written against widgets that exist today have a
//! dispatch arm. Every other page falls through to [`StoryDispatch::pending`],
//! which names the exact missing widgets instead of rendering a placeholder. The
//! table in [`MigrationPending::entries`] is generated from the Swift sources and
//! the real `cove-ui-compose` symbol set, so it is an accurate work queue for the
//! Compose migration rather than a guess.

use crate::gallery_content::catalog::{GalleryOptions, GalleryPage};
use crate::gallery_content::stories::overview::IntroductionStory;
use cove_ui_compose::AnyView;

/// What the shell should render for a page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoryDispatch {
    /// A ported story exists.
    Story(GalleryPage),
    /// The story needs widgets that have not been migrated yet.
    Pending {
        page: GalleryPage,
        story_file: &'static str,
        missing_widgets: &'static [&'static str],
    },
}

impl StoryDispatch {
    /// Resolves the dispatch for `page`.
    pub fn new(page: GalleryPage) -> Self {
        match page {
            // Pages whose story is fully portable today.
            GalleryPage::Introduction => Self::Story(page),
            _ => Self::pending(page),
        }
    }

    fn pending(page: GalleryPage) -> Self {
        let (story_file, missing_widgets) = MigrationPending::entry(page);
        Self::Pending {
            page,
            story_file,
            missing_widgets,
        }
    }

    pub fn is_implemented(&self) -> bool {
        matches!(self, Self::Story(_))
    }

    /// The story view for an implemented page.
    pub fn view(&self, options: GalleryOptions) -> Option<AnyView> {
        match self {
            Self::Story(GalleryPage::Introduction) => {
                Some(AnyView::new(IntroductionStory::new(options)))
            }
            Self::Story(_) | Self::Pending { .. } => None,
        }
    }

    /// The operator-facing message for an unimplemented page.
    pub fn pending_message(&self) -> Option<String> {
        match self {
            Self::Story(_) => None,
            Self::Pending {
                page,
                story_file,
                missing_widgets,
            } => Some(format!(
                "{} needs widgets that are not migrated to Rust yet ({}): {}",
                page.raw_value(),
                story_file,
                missing_widgets.join(", ")
            )),
        }
    }
}

/// The generated record of every page's story file and its blocking widgets.
pub struct MigrationPending;

impl MigrationPending {
    /// `(page, story file, missing widget type names)` for every page.
    ///
    /// An empty widget list means the page is portable; use
    /// [`MigrationPending::is_portable`].
    pub fn entries() -> &'static [(GalleryPage, &'static str, &'static [&'static str])] {
        &[
            (GalleryPage::Introduction, "Overview/Dashboard.swift", &["ButtonGroup", "ButtonGroupItem"]),
            (GalleryPage::Button, "Controls/ControlStories.swift", &["DatePicker", "MenuButton", "Select"]),
            (GalleryPage::Checkbox, "Controls/ControlStories.swift", &["Checkbox"]),
            (GalleryPage::Toggle, "Controls/ControlStories.swift", &["Toggle"]),
            (GalleryPage::Slider, "Feedback/FeedbackStories.swift", &["Slider"]),
            (GalleryPage::RangeSlider, "Controls/ControlStories.swift", &[]),
            (GalleryPage::Rating, "Feedback/RatingStory.swift", &["Rating"]),
            (GalleryPage::TextField, "Controls/ControlStories.swift", &["TextField"]),
            (GalleryPage::SearchField, "Controls/ControlStories.swift", &["TextField", "Timeline", "Toolbar"]),
            (GalleryPage::TextArea, "Controls/ControlStories.swift", &[]),
            (GalleryPage::TextEditor, "Controls/TextEditorStory.swift", &["Checkbox", "TextField", "Tooltip"]),
            (GalleryPage::WheelPicker, "Controls/WheelPickerStory.swift", &["Calendar", "Checkbox"]),
            (GalleryPage::NumberField, "Controls/ControlStories.swift", &[]),
            (GalleryPage::Vec3Field, "Controls/ControlStories.swift", &[]),
            (GalleryPage::ColorField, "Controls/ControlStories.swift", &[]),
            (GalleryPage::Select, "Controls/ControlStories.swift", &["Select"]),
            (GalleryPage::JsonField, "Controls/ControlStories.swift", &[]),
            (GalleryPage::AssetRef, "Data/DataStories.swift", &[]),
            (GalleryPage::List, "Data/DataStories.swift", &["List", "ListRow"]),
            (GalleryPage::SectionedList, "Data/DataStories.swift", &["ListRow", "SectionedList"]),
            (GalleryPage::ReorderableList, "Data/DataStories.swift", &[]),
            (GalleryPage::SwipeList, "Data/DataStories.swift", &[]),
            (GalleryPage::RefreshableList, "Data/DataStories.swift", &["List"]),
            (GalleryPage::Scrollbar, "Data/DataStories.swift", &[]),
            (GalleryPage::ViewportTraits, "Foundation/SurfaceTraits.swift", &[]),
            (GalleryPage::ViewThatFits, "Foundation/IntrinsicFitting.swift", &["ViewThatFits"]),
            (GalleryPage::DragAndDrop, "Data/DataStories.swift", &[]),
            (GalleryPage::Tree, "Data/DataStories.swift", &["TextField", "Tree"]),
            (GalleryPage::VirtualStack, "Data/DataStories.swift", &[]),
            (GalleryPage::Chart, "Data/DataStories.swift", &["Chart", "Checkbox"]),
            (GalleryPage::Icon, "Data/DataStories.swift", &["Slider", "TextField"]),
            (GalleryPage::Avatar, "Feedback/AvatarStory.swift", &["ButtonGroup", "Select", "Slider"]),
            (GalleryPage::Image, "Layout/ImageStory.swift", &["ButtonGroup", "TextField"]),
            (GalleryPage::Web, "Data/WebStory.swift", &["ButtonGroup", "ButtonGroupItem"]),
            (GalleryPage::RichText, "Data/RichTextStory.swift", &["RichText"]),
            (GalleryPage::Markdown, "Data/MarkdownStory.swift", &["Table", "TextField"]),
            (GalleryPage::Menu, "Layout/OverlayStories.swift", &["MenuButton", "Workspace"]),
            (GalleryPage::MenuButton, "Controls/ControlStories.swift", &["MenuButton"]),
            (GalleryPage::ContextMenu, "Layout/OverlayStories.swift", &[]),
            (GalleryPage::Popover, "Layout/OverlayStories.swift", &["Popover", "TextField"]),
            (GalleryPage::Modal, "Layout/OverlayStories.swift", &["Dialog", "TextField"]),
            (GalleryPage::Disclosure, "Layout/OverlayStories.swift", &["TextField", "Workspace"]),
            (GalleryPage::Tabs, "Layout/OverlayStories.swift", &["TabView", "Workspace"]),
            (GalleryPage::Panel, "Layout/LayoutStories.swift", &["Panel"]),
            (GalleryPage::SplitView, "Layout/LayoutStories.swift", &["Sidebar", "Slider", "SplitView", "TextField"]),
            (GalleryPage::ScrollView, "Layout/LayoutStories.swift", &[]),
            (GalleryPage::BoundedScroll, "Layout/LayoutStories.swift", &["Select"]),
            (GalleryPage::PropertyGrid, "Layout/LayoutStories.swift", &["PropertyGrid", "Select", "TextField", "Toggle"]),
            (GalleryPage::Workspace, "Layout/LayoutStories.swift", &[]),
            (GalleryPage::Layout, "Layout/LayoutStories.swift", &[]),
            (GalleryPage::Masonry, "Layout/MasonryStory.swift", &[]),
            (GalleryPage::ZStack, "Layout/LayoutStories.swift", &["Checkbox"]),
            (GalleryPage::Animation, "Layout/LayoutStories.swift", &[]),
            (GalleryPage::Theme, "Foundation/ThemeStory.swift", &["Checkbox", "Select", "Slider", "TextField"]),
            (GalleryPage::Badge, "Feedback/FeedbackStories.swift", &["Workspace"]),
            (GalleryPage::Alert, "Feedback/FeedbackStories.swift", &[]),
            (GalleryPage::Progress, "Feedback/FeedbackStories.swift", &["Slider"]),
            (GalleryPage::Empty, "Feedback/FeedbackStories.swift", &[]),
            (GalleryPage::Radio, "Layout/ProductStories.swift", &["RadioGroup"]),
            (GalleryPage::Form, "Layout/ProductStories.swift", &["TextField"]),
            (GalleryPage::Combobox, "Layout/ProductStories.swift", &["Combobox"]),
            (GalleryPage::Notification, "Layout/ProductStories.swift", &["Select"]),
            (GalleryPage::Tooltip, "Layout/ProductStories.swift", &[]),
            (GalleryPage::Sidebar, "Layout/SidebarStory.swift", &["Checkbox", "Sidebar", "SidebarItem", "SidebarSection", "Workspace"]),
            (GalleryPage::NavigationDrawer, "Layout/DrawerStory.swift", &["Checkbox", "SidebarItem", "SidebarSection", "Workspace"]),
            (GalleryPage::NavigationRail, "Layout/RailStory.swift", &["Checkbox", "NavigationRail", "Select", "Sidebar", "SidebarItem", "SidebarSection", "Workspace"]),
            (GalleryPage::AppBar, "Navigation/AppBarStory.swift", &["AppBar", "Checkbox", "SafeArea", "ToolbarItem", "Workspace"]),
            (GalleryPage::Table, "Data/DateTableStories.swift", &["Table"]),
            (GalleryPage::DataTable, "Data/DataTableStory.swift", &["Checkbox", "DataTable"]),
            (GalleryPage::Calendar, "Data/DateTableStories.swift", &[]),
            (GalleryPage::DatePicker, "Data/DateTableStories.swift", &["DatePicker"]),
            (GalleryPage::DateRangePicker, "Controls/ControlStories.swift", &[]),
            (GalleryPage::TimeField, "Data/DateTableStories.swift", &[]),
            (GalleryPage::TimePicker, "Controls/TimePickerStory.swift", &["Calendar", "Checkbox", "Popover", "Sheet", "TimePicker"]),
            (GalleryPage::Popconfirm, "Overlays/PopconfirmStory.swift", &["Checkbox"]),
            (GalleryPage::ActionSheet, "Overlays/ActionSheetStory.swift", &["ActionSheet", "Checkbox", "Workspace"]),
            (GalleryPage::Accordion, "Layout/NavigationStories.swift", &["Accordion", "Checkbox", "TextField", "Workspace"]),
            (GalleryPage::Breadcrumb, "Layout/NavigationStories.swift", &["Breadcrumb"]),
            (GalleryPage::Pagination, "Layout/NavigationStories.swift", &["Pagination"]),
            (GalleryPage::ButtonGroup, "Layout/NavigationStories.swift", &["ButtonGroup", "ButtonGroupItem", "List", "Timeline"]),
            (GalleryPage::SplitButton, "Controls/ControlStories.swift", &["MenuButton"]),
            (GalleryPage::Tag, "Layout/NavigationStories.swift", &["Workspace"]),
            (GalleryPage::KeyCap, "Layout/NavigationStories.swift", &["Workspace"]),
            (GalleryPage::Label, "Layout/NavigationStories.swift", &["TextField", "Workspace"]),
            (GalleryPage::Dialog, "Layout/NavigationStories.swift", &["Dialog", "TextField"]),
            (GalleryPage::AlertDialog, "Layout/NavigationStories.swift", &[]),
            (GalleryPage::Sheet, "Layout/NavigationStories.swift", &["Sheet", "Toggle"]),
            (GalleryPage::BottomSheet, "Layout/NavigationStories.swift", &["BottomSheet", "Toggle"]),
            (GalleryPage::CommandPalette, "Overlays/CommandStory.swift", &["CommandPalette", "Toggle"]),
            (GalleryPage::NavigationStack, "Layout/NavigationStories.swift", &["TextField", "Workspace"]),
            (GalleryPage::HoverCard, "Overlays/HoverStory.swift", &["Popover"]),
            (GalleryPage::Timeline, "Data/DataStories.swift", &["Calendar", "Timeline"]),
            (GalleryPage::Toolbar, "Controls/ToolbarStory.swift", &["TextField", "Toolbar", "ToolbarItem"]),
            (GalleryPage::Link, "Controls/ControlStories.swift", &[]),
            (GalleryPage::StatusBar, "Layout/LayoutStories.swift", &[]),
            (GalleryPage::Carousel, "Layout/LayoutStories.swift", &["Carousel", "Checkbox"]),
            (GalleryPage::Skeleton, "Feedback/SkeletonStory.swift", &["Checkbox"]),
            (GalleryPage::Stepper, "Controls/StepperStory.swift", &[]),
        ]
    }

    /// The story file and blocking widgets for one page.
    pub fn entry(page: GalleryPage) -> (&'static str, &'static [&'static str]) {
        let entries = Self::entries();
        match entries.iter().find(|(candidate, _, _)| *candidate == page) {
            Some((_, file, missing)) => (file, missing),
            None => ("", &[]),
        }
    }

    /// Pages that can be ported with the widgets that exist today.
    pub fn portable() -> Vec<GalleryPage> {
        Self::entries()
            .iter()
            .filter(|(_, _, missing)| missing.is_empty())
            .map(|(page, _, _)| *page)
            .collect()
    }

    /// Pages still waiting on Compose.
    pub fn blocked() -> Vec<(GalleryPage, &'static str, &'static [&'static str])> {
        Self::entries()
            .iter()
            .filter(|(_, _, missing)| !missing.is_empty())
            .copied()
            .collect()
    }

    /// Every missing widget name with the pages that need it, most-needed first.
    ///
    /// This is the work queue for the Compose migration.
    pub fn widget_backlog() -> Vec<(&'static str, Vec<GalleryPage>)> {
        let mut backlog: Vec<(&'static str, Vec<GalleryPage>)> = Vec::new();
        for (page, _, missing) in Self::entries() {
            for widget in *missing {
                match backlog.iter_mut().find(|(name, _)| name == widget) {
                    Some((_, pages)) => pages.push(*page),
                    None => backlog.push((widget, vec![*page])),
                }
            }
        }
        backlog.sort_by(|left, right| {
            right
                .1
                .len()
                .cmp(&left.1.len())
                .then_with(|| left.0.cmp(right.0))
        });
        backlog
    }
}

#[cfg(test)]
mod checks {
    use super::*;
    use crate::gallery_content::catalog::GalleryCatalog;

    #[test]
    fn every_catalog_page_has_exactly_one_entry() {
        let entries = MigrationPending::entries();
        assert_eq!(entries.len(), 98, "one row per catalog page");
        for entry in GalleryCatalog::entries() {
            assert!(
                entries.iter().any(|(page, _, _)| *page == entry.page),
                "{} is missing from the migration table",
                entry.page.raw_value()
            );
        }
        let unique: std::collections::BTreeSet<_> =
            entries.iter().map(|(page, _, _)| *page).collect();
        assert_eq!(unique.len(), 98, "no page appears twice");
    }

    #[test]
    fn every_entry_names_a_real_story_file() {
        for (page, file, _) in MigrationPending::entries() {
            assert!(!file.is_empty(), "{} has no story file", page.raw_value());
            assert!(
                file.ends_with(".swift"),
                "{} must name a Swift source (got {file})",
                page.raw_value()
            );
            assert!(
                file.contains('/'),
                "{} must name a path inside Stories/ (got {file})",
                page.raw_value()
            );
        }
    }

    #[test]
    fn portable_and_blocked_partition_every_page() {
        let entries = MigrationPending::entries();
        let portable = MigrationPending::portable();
        let blocked = MigrationPending::blocked();
        assert_eq!(portable.len() + blocked.len(), entries.len());
        assert_eq!(portable.len(), 29, "29 pages are portable today");
        assert_eq!(blocked.len(), 69, "69 pages wait on Compose");
        for page in &portable {
            assert!(MigrationPending::entry(*page).1.is_empty());
        }
        for (page, _, missing) in &blocked {
            assert!(!missing.is_empty(), "{} is listed as blocked", page.raw_value());
        }
    }

    #[test]
    fn introduction_dispatches_to_its_story() {
        let dispatch = StoryDispatch::new(GalleryPage::Introduction);
        assert!(dispatch.is_implemented());
        assert!(dispatch.view(GalleryOptions::default()).is_some());
        assert!(dispatch.pending_message().is_none());
    }

    #[test]
    fn blocked_pages_report_their_missing_widgets() {
        let dispatch = StoryDispatch::new(GalleryPage::Checkbox);
        assert!(!dispatch.is_implemented());
        assert!(dispatch.view(GalleryOptions::default()).is_none());
        let message = dispatch.pending_message().expect("a pending page reports");
        assert!(message.contains("checkbox"));
        assert!(message.contains("Checkbox"));
        assert!(message.contains("Controls/ControlStories.swift"));

        // Portable-but-not-yet-ported pages still report, with an empty widget
        // list, because the dispatch arm does not exist yet.
        let dispatch = StoryDispatch::new(GalleryPage::Stepper);
        assert!(!dispatch.is_implemented());
        assert_eq!(
            dispatch,
            StoryDispatch::Pending {
                page: GalleryPage::Stepper,
                story_file: "Controls/StepperStory.swift",
                missing_widgets: &[],
            }
        );
    }

    #[test]
    fn widget_backlog_is_ordered_by_pages_blocked() {
        let backlog = MigrationPending::widget_backlog();
        assert!(!backlog.is_empty());
        // The most-demanded widget comes first and the order is monotonic.
        for pair in backlog.windows(2) {
            assert!(
                pair[0].1.len() >= pair[1].1.len(),
                "backlog must be ordered by demand: {} vs {}",
                pair[0].0,
                pair[1].0
            );
        }
        let (most_needed, pages) = &backlog[0];
        assert!(!pages.is_empty(), "{most_needed} blocks no pages");
        // Every listed page really does name that widget.
        for page in pages {
            let (_, missing) = MigrationPending::entry(*page);
            assert!(
                missing.contains(most_needed),
                "{} must name {most_needed}",
                page.raw_value()
            );
        }
    }
}
