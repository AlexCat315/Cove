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
use crate::gallery_content::stories;
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
            // Pages whose story is fully ported today.
            GalleryPage::Introduction
            | GalleryPage::Button
            | GalleryPage::Checkbox
            | GalleryPage::Toggle
            | GalleryPage::Badge
            | GalleryPage::Alert
            | GalleryPage::Progress
            | GalleryPage::Empty
            | GalleryPage::Animation
            | GalleryPage::ViewportTraits
            | GalleryPage::Masonry
            | GalleryPage::RichText
            | GalleryPage::Calendar
            | GalleryPage::Stepper
            | GalleryPage::ViewThatFits => Self::Story(page),
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
                Some(AnyView::new(stories::IntroductionStory::new(options)))
            }
            Self::Story(GalleryPage::Button) => {
                Some(AnyView::new(stories::ButtonStory::new(options)))
            }
            Self::Story(GalleryPage::Checkbox) => {
                Some(AnyView::new(stories::CheckboxStory::new(options)))
            }
            Self::Story(GalleryPage::Toggle) => {
                Some(AnyView::new(stories::ToggleStory::new(options)))
            }
            Self::Story(GalleryPage::Masonry) => {
                Some(AnyView::new(stories::MasonryStory::new(options)))
            }
            Self::Story(GalleryPage::RichText) => Some(AnyView::new(stories::RichTextStory::new())),
            Self::Story(GalleryPage::Calendar) => {
                Some(AnyView::new(stories::CalendarStory::new(options)))
            }
            Self::Story(GalleryPage::Stepper) => {
                Some(AnyView::new(stories::StepperStory::new(options)))
            }
            Self::Story(GalleryPage::ViewThatFits) => {
                Some(AnyView::new(stories::ViewThatFitsStory::new()))
            }
            Self::Story(GalleryPage::Badge) => Some(AnyView::new(stories::BadgeStory)),
            Self::Story(GalleryPage::Alert) => Some(AnyView::new(stories::AlertStory::new())),
            Self::Story(GalleryPage::Progress) => {
                Some(AnyView::new(stories::ProgressStory::new(options)))
            }
            Self::Story(GalleryPage::Empty) => Some(AnyView::new(stories::EmptyStory::new())),
            Self::Story(GalleryPage::Animation) => {
                Some(AnyView::new(stories::AnimationStory::new(options)))
            }
            Self::Story(GalleryPage::ViewportTraits) => {
                Some(AnyView::new(stories::ViewportTraitsStory::new()))
            }
            Self::Story(GalleryPage::Skeleton) => {
                Some(AnyView::new(stories::SkeletonStory::new(options)))
            }
            Self::Story(GalleryPage::Rating) => {
                Some(AnyView::new(stories::RatingSample::new(options)))
            }
            Self::Story(GalleryPage::Popconfirm) => {
                Some(AnyView::new(stories::PopconfirmStory::new(options)))
            }
            Self::Story(GalleryPage::HoverCard) => {
                Some(AnyView::new(stories::HoverCardStory::new(options)))
            }
            Self::Story(GalleryPage::Chart) => {
                Some(AnyView::new(stories::ChartExplorationStory::new()))
            }
            Self::Story(GalleryPage::AppBar) => {
                Some(AnyView::new(stories::AppBarStory::new(options)))
            }
            Self::Story(GalleryPage::Avatar) => {
                Some(AnyView::new(stories::AvatarStory::new(options)))
            }
            Self::Story(GalleryPage::ActionSheet) => {
                Some(AnyView::new(stories::ActionSheetStory::new(options)))
            }
            Self::Story(GalleryPage::Toolbar) => {
                Some(AnyView::new(stories::ToolbarStory::new(options)))
            }
            Self::Story(GalleryPage::NavigationDrawer) => {
                Some(AnyView::new(stories::DrawerStory::new(options)))
            }
            Self::Story(GalleryPage::NavigationRail) => {
                Some(AnyView::new(stories::RailStory::new(options)))
            }
            Self::Story(GalleryPage::TimePicker) => {
                Some(AnyView::new(stories::TimePickerStory::new(options)))
            }
            Self::Story(GalleryPage::Markdown) => {
                Some(AnyView::new(stories::MarkdownStory::new(options)))
            }
            Self::Story(GalleryPage::WheelPicker) => {
                Some(AnyView::new(stories::WheelPickerStory::new(options)))
            }
            Self::Story(GalleryPage::CommandPalette) => {
                Some(AnyView::new(stories::CommandStory::new(options)))
            }
            Self::Story(GalleryPage::Image) => {
                Some(AnyView::new(stories::ImageStory::new(options)))
            }
            Self::Story(GalleryPage::Sidebar) => {
                Some(AnyView::new(stories::SidebarStory::new(options)))
            }
            Self::Story(GalleryPage::DataTable) => {
                Some(AnyView::new(stories::DataTableStory::new(options)))
            }
            Self::Story(GalleryPage::Web) => Some(AnyView::new(stories::WebStory::new(options))),
            Self::Story(GalleryPage::Menu) => Some(AnyView::new(stories::MenuStory::new(options))),
            Self::Story(GalleryPage::ContextMenu) => {
                Some(AnyView::new(stories::MenuStory::new(options)))
            }
            Self::Story(GalleryPage::Popover) => {
                Some(AnyView::new(stories::PopoverStory::new(options)))
            }
            Self::Story(GalleryPage::Modal) => {
                Some(AnyView::new(stories::ModalStory::new(options)))
            }
            Self::Story(GalleryPage::Disclosure) => {
                Some(AnyView::new(stories::DisclosureStory::new(options)))
            }
            Self::Story(GalleryPage::Tabs) => Some(AnyView::new(stories::TabsStory::new(options))),
            Self::Story(GalleryPage::TextEditor) => {
                Some(AnyView::new(stories::TextEditorStory::new(options)))
            }
            Self::Story(GalleryPage::Accordion) => {
                Some(AnyView::new(stories::AccordionStory::new(options)))
            }
            Self::Story(GalleryPage::Breadcrumb) => {
                Some(AnyView::new(stories::BreadcrumbStory::new()))
            }
            Self::Story(GalleryPage::Pagination) => {
                Some(AnyView::new(stories::PaginationStory::new(options)))
            }
            Self::Story(GalleryPage::ButtonGroup) => {
                Some(AnyView::new(stories::ButtonGroupStory::new(options)))
            }
            Self::Story(GalleryPage::Tag) => Some(AnyView::new(stories::TagStory::new(options))),
            Self::Story(GalleryPage::Label) => Some(AnyView::new(stories::LabelStory::new())),
            Self::Story(GalleryPage::Dialog) => Some(AnyView::new(stories::DialogStory::new())),
            Self::Story(GalleryPage::AlertDialog) => {
                Some(AnyView::new(stories::AlertDialogStory::new()))
            }
            Self::Story(GalleryPage::Sheet) => {
                Some(AnyView::new(stories::SheetStory::new(options)))
            }
            Self::Story(GalleryPage::BottomSheet) => {
                Some(AnyView::new(stories::BottomSheetStory::new(options)))
            }
            Self::Story(GalleryPage::NavigationStack) => {
                Some(AnyView::new(stories::NavigationStackStory::new()))
            }
            Self::Story(GalleryPage::Tree) => Some(AnyView::new(stories::TreeStory::new(options))),
            Self::Story(GalleryPage::VirtualStack) => {
                Some(AnyView::new(stories::VirtualStackStory::new()))
            }
            Self::Story(GalleryPage::SwipeList) => {
                Some(AnyView::new(stories::SwipeListStory::new()))
            }
            Self::Story(GalleryPage::RefreshableList) => {
                Some(AnyView::new(stories::RefreshListStory::new()))
            }
            Self::Story(GalleryPage::DragAndDrop) => {
                Some(AnyView::new(stories::DragAndDropStory::new()))
            }
            Self::Story(GalleryPage::SectionedList) => {
                Some(AnyView::new(stories::SectionedListStory::new()))
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
            (GalleryPage::Introduction, "Overview/Dashboard.swift", &[]),
            (GalleryPage::Button, "Controls/ControlStories.swift", &[]),
            (GalleryPage::Checkbox, "Controls/ControlStories.swift", &[]),
            (GalleryPage::Toggle, "Controls/ControlStories.swift", &[]),
            (GalleryPage::Slider, "Feedback/FeedbackStories.swift", &[]),
            (
                GalleryPage::RangeSlider,
                "Controls/ControlStories.swift",
                &[],
            ),
            (
                GalleryPage::Rating,
                "Feedback/RatingStory.swift",
                &["Rating"],
            ),
            (GalleryPage::TextField, "Controls/ControlStories.swift", &[]),
            (
                GalleryPage::SearchField,
                "Controls/ControlStories.swift",
                &[],
            ),
            (GalleryPage::TextArea, "Controls/ControlStories.swift", &[]),
            (
                GalleryPage::TextEditor,
                "Controls/TextEditorStory.swift",
                &["Tooltip"],
            ),
            (
                GalleryPage::WheelPicker,
                "Controls/WheelPickerStory.swift",
                &["WheelPickerOption"],
            ),
            (
                GalleryPage::NumberField,
                "Controls/ControlStories.swift",
                &[],
            ),
            (GalleryPage::Vec3Field, "Controls/ControlStories.swift", &[]),
            (
                GalleryPage::ColorField,
                "Controls/ControlStories.swift",
                &[],
            ),
            (GalleryPage::Select, "Controls/ControlStories.swift", &[]),
            (GalleryPage::JsonField, "Controls/ControlStories.swift", &[]),
            (GalleryPage::AssetRef, "Data/DataStories.swift", &[]),
            (GalleryPage::List, "Data/DataStories.swift", &[]),
            (GalleryPage::SectionedList, "Data/DataStories.swift", &[]),
            (GalleryPage::ReorderableList, "Data/DataStories.swift", &[]),
            (GalleryPage::SwipeList, "Data/DataStories.swift", &[]),
            (GalleryPage::RefreshableList, "Data/DataStories.swift", &[]),
            (GalleryPage::Scrollbar, "Data/DataStories.swift", &[]),
            (
                GalleryPage::ViewportTraits,
                "Foundation/SurfaceTraits.swift",
                &[],
            ),
            (
                GalleryPage::ViewThatFits,
                "Foundation/IntrinsicFitting.swift",
                &[],
            ),
            (GalleryPage::DragAndDrop, "Data/DataStories.swift", &[]),
            (GalleryPage::Tree, "Data/DataStories.swift", &["Tree"]),
            (GalleryPage::VirtualStack, "Data/DataStories.swift", &[]),
            (GalleryPage::Chart, "Data/DataStories.swift", &[]),
            (GalleryPage::Icon, "Data/DataStories.swift", &[]),
            (GalleryPage::Avatar, "Feedback/AvatarStory.swift", &[]),
            (GalleryPage::Image, "Layout/ImageStory.swift", &[]),
            (GalleryPage::Web, "Data/WebStory.swift", &[]),
            (GalleryPage::RichText, "Data/RichTextStory.swift", &[]),
            (GalleryPage::Markdown, "Data/MarkdownStory.swift", &[]),
            (GalleryPage::Menu, "Layout/OverlayStories.swift", &[]),
            (
                GalleryPage::MenuButton,
                "Controls/ControlStories.swift",
                &[],
            ),
            (GalleryPage::ContextMenu, "Layout/OverlayStories.swift", &[]),
            (GalleryPage::Popover, "Layout/OverlayStories.swift", &[]),
            (GalleryPage::Modal, "Layout/OverlayStories.swift", &[]),
            (GalleryPage::Disclosure, "Layout/OverlayStories.swift", &[]),
            (
                GalleryPage::Tabs,
                "Layout/OverlayStories.swift",
                &["TabItem"],
            ),
            (GalleryPage::Panel, "Layout/LayoutStories.swift", &[]),
            (GalleryPage::SplitView, "Layout/LayoutStories.swift", &[]),
            (GalleryPage::ScrollView, "Layout/LayoutStories.swift", &[]),
            (
                GalleryPage::BoundedScroll,
                "Layout/LayoutStories.swift",
                &[],
            ),
            (
                GalleryPage::PropertyGrid,
                "Layout/LayoutStories.swift",
                &["PropertyGrid", "PropertyGridRow", "PropertyGridSection"],
            ),
            (GalleryPage::Workspace, "Layout/LayoutStories.swift", &[]),
            (GalleryPage::Layout, "Layout/LayoutStories.swift", &[]),
            (GalleryPage::Masonry, "Layout/MasonryStory.swift", &[]),
            (GalleryPage::ZStack, "Layout/LayoutStories.swift", &[]),
            (GalleryPage::Animation, "Layout/LayoutStories.swift", &[]),
            (GalleryPage::Theme, "Foundation/ThemeStory.swift", &[]),
            (GalleryPage::Badge, "Feedback/FeedbackStories.swift", &[]),
            (GalleryPage::Alert, "Feedback/FeedbackStories.swift", &[]),
            (GalleryPage::Progress, "Feedback/FeedbackStories.swift", &[]),
            (GalleryPage::Empty, "Feedback/FeedbackStories.swift", &[]),
            (GalleryPage::Radio, "Layout/ProductStories.swift", &[]),
            (GalleryPage::Form, "Layout/ProductStories.swift", &[]),
            (GalleryPage::Combobox, "Layout/ProductStories.swift", &[]),
            (
                GalleryPage::Notification,
                "Layout/ProductStories.swift",
                &[],
            ),
            (GalleryPage::Tooltip, "Layout/ProductStories.swift", &[]),
            (GalleryPage::Sidebar, "Layout/SidebarStory.swift", &[]),
            (
                GalleryPage::NavigationDrawer,
                "Layout/DrawerStory.swift",
                &[],
            ),
            (GalleryPage::NavigationRail, "Layout/RailStory.swift", &[]),
            (
                GalleryPage::AppBar,
                "Navigation/AppBarStory.swift",
                &["AppBar"],
            ),
            (GalleryPage::Table, "Data/DateTableStories.swift", &[]),
            (GalleryPage::DataTable, "Data/DataTableStory.swift", &[]),
            (GalleryPage::Calendar, "Data/DateTableStories.swift", &[]),
            (GalleryPage::DatePicker, "Data/DateTableStories.swift", &[]),
            (
                GalleryPage::DateRangePicker,
                "Controls/ControlStories.swift",
                &["DateRangePicker"],
            ),
            (GalleryPage::TimeField, "Data/DateTableStories.swift", &[]),
            (
                GalleryPage::TimePicker,
                "Controls/TimePickerStory.swift",
                &["TimeInput", "TimePicker"],
            ),
            (
                GalleryPage::Popconfirm,
                "Overlays/PopconfirmStory.swift",
                &["Popconfirm"],
            ),
            (
                GalleryPage::ActionSheet,
                "Overlays/ActionSheetStory.swift",
                &[],
            ),
            (
                GalleryPage::Accordion,
                "Layout/NavigationStories.swift",
                &[],
            ),
            (
                GalleryPage::Breadcrumb,
                "Layout/NavigationStories.swift",
                &["Breadcrumb", "BreadcrumbItem"],
            ),
            (
                GalleryPage::Pagination,
                "Layout/NavigationStories.swift",
                &["Pagination"],
            ),
            (
                GalleryPage::ButtonGroup,
                "Layout/NavigationStories.swift",
                &[],
            ),
            (
                GalleryPage::SplitButton,
                "Controls/ControlStories.swift",
                &[],
            ),
            (GalleryPage::Tag, "Layout/NavigationStories.swift", &[]),
            (GalleryPage::KeyCap, "Layout/NavigationStories.swift", &[]),
            (GalleryPage::Label, "Layout/NavigationStories.swift", &[]),
            (GalleryPage::Dialog, "Layout/NavigationStories.swift", &[]),
            (
                GalleryPage::AlertDialog,
                "Layout/NavigationStories.swift",
                &[],
            ),
            (GalleryPage::Sheet, "Layout/NavigationStories.swift", &[]),
            (
                GalleryPage::BottomSheet,
                "Layout/NavigationStories.swift",
                &[],
            ),
            (
                GalleryPage::CommandPalette,
                "Overlays/CommandStory.swift",
                &["CommandItem", "CommandPalette"],
            ),
            (
                GalleryPage::NavigationStack,
                "Layout/NavigationStories.swift",
                &[],
            ),
            (
                GalleryPage::HoverCard,
                "Overlays/HoverStory.swift",
                &["HoverCard"],
            ),
            (GalleryPage::Timeline, "Data/DataStories.swift", &[]),
            (GalleryPage::Toolbar, "Controls/ToolbarStory.swift", &[]),
            (GalleryPage::Link, "Controls/ControlStories.swift", &[]),
            (GalleryPage::StatusBar, "Layout/LayoutStories.swift", &[]),
            (
                GalleryPage::Carousel,
                "Layout/LayoutStories.swift",
                &["CarouselPage"],
            ),
            (GalleryPage::Skeleton, "Feedback/SkeletonStory.swift", &[]),
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
        assert_eq!(portable.len(), 83, "83 pages are portable today");
        assert_eq!(blocked.len(), 15, "15 pages wait on Compose");
        for page in &portable {
            assert!(MigrationPending::entry(*page).1.is_empty());
        }
        for (page, _, missing) in &blocked {
            assert!(
                !missing.is_empty(),
                "{} is listed as blocked",
                page.raw_value()
            );
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
        // `tree` still needs the Tree widget.
        let dispatch = StoryDispatch::new(GalleryPage::Tree);
        assert!(!dispatch.is_implemented());
        assert!(dispatch.view(GalleryOptions::default()).is_none());
        let message = dispatch.pending_message().expect("a pending page reports");
        assert!(message.contains("tree"));
        assert!(message.contains("Tree"));
        assert!(message.contains("Data/DataStories.swift"));

        // Portable-but-not-yet-ported pages still report, with an empty widget
        // list, because the dispatch arm does not exist yet.
        let dispatch = StoryDispatch::new(GalleryPage::RangeSlider);
        assert!(!dispatch.is_implemented());
        assert_eq!(
            dispatch,
            StoryDispatch::Pending {
                page: GalleryPage::RangeSlider,
                story_file: "Controls/ControlStories.swift",
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
