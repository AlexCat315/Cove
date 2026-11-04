//! Page-to-story dispatch and the record of where each page's story comes from.
//!
//! Swift source: the `GalleryPage.makeStory` / `baseStory` switch in
//! `GalleryContent/Stories/Feedback/FeedbackStories.swift`.
//!
//! Every page whose story is written has a dispatch arm. The remaining page
//! falls through to [`StoryDispatch::pending`], which names the Swift file it
//! still has to be ported from — not a missing widget, because every widget the
//! Swift stories use now exists in `cove-ui-compose`.

use crate::gallery_content::catalog::{GalleryOptions, GalleryPage};
use crate::gallery_content::stories;
use cove_ui_compose::AnyView;

/// What the shell should render for a page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoryDispatch {
    /// A ported story exists.
    Story(GalleryPage),
    /// The page has no ported story yet.
    Pending {
        page: GalleryPage,
        story_file: &'static str,
    },
}

impl StoryDispatch {
    /// Resolves the dispatch for `page`.
    pub fn new(page: GalleryPage) -> Self {
        match page {
            // `Workspace` is the one page without a story: the addon view needs a
            // controller, a panel registry and authored content, so it is ported
            // separately from the gallery stories.
            GalleryPage::Workspace => Self::Pending {
                page,
                story_file: StorySource::entry(page),
            },
            _ => Self::Story(page),
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
            Self::Story(GalleryPage::Slider) => {
                Some(AnyView::new(stories::SliderStory::new(options)))
            }
            Self::Story(GalleryPage::RangeSlider) => {
                Some(AnyView::new(stories::RangeSliderStory::new(options)))
            }
            Self::Story(GalleryPage::TextField) => {
                Some(AnyView::new(stories::TextFieldsStory::new(options)))
            }
            Self::Story(GalleryPage::NumberField) => {
                Some(AnyView::new(stories::NumericStory::new(options)))
            }
            Self::Story(GalleryPage::ColorField) => {
                Some(AnyView::new(stories::ColorFieldStory::new(options)))
            }
            Self::Story(GalleryPage::Select) => {
                Some(AnyView::new(stories::SelectStory::new(options)))
            }
            Self::Story(GalleryPage::AssetRef) => {
                Some(AnyView::new(stories::AssetRefStory::new(options)))
            }
            Self::Story(GalleryPage::List) => Some(AnyView::new(stories::ListStory::new(options))),
            Self::Story(GalleryPage::ReorderableList) => {
                Some(AnyView::new(stories::ReorderableStory::new(options)))
            }
            Self::Story(GalleryPage::Scrollbar) => {
                Some(AnyView::new(stories::ScrollbarStory::new(options)))
            }
            Self::Story(GalleryPage::Icon) => Some(AnyView::new(stories::IconStory::new(options))),
            Self::Story(GalleryPage::MenuButton) => {
                Some(AnyView::new(stories::MenuButtonStory::new(options)))
            }
            Self::Story(GalleryPage::SplitButton) => {
                Some(AnyView::new(stories::SplitButtonStory::new(options)))
            }
            Self::Story(GalleryPage::KeyCap) => {
                Some(AnyView::new(stories::KeyCapStory::new(options)))
            }
            Self::Story(GalleryPage::Link) => Some(AnyView::new(stories::LinkStory::new(options))),
            Self::Story(GalleryPage::SingleLineText) => {
                Some(AnyView::new(stories::SingleLineTextStory::new(options)))
            }
            Self::Story(GalleryPage::CompletionPopover) => {
                Some(AnyView::new(stories::CompletionPopoverStory::new(options)))
            }
            Self::Story(GalleryPage::RouteOutlet) => {
                Some(AnyView::new(stories::RouteOutletStory::new(options)))
            }
            Self::Story(GalleryPage::ProgressValue) => {
                Some(AnyView::new(stories::ProgressValueStory::new(options)))
            }
            Self::Story(GalleryPage::HorizontalSwipe) => {
                Some(AnyView::new(stories::HorizontalSwipeStory::new(options)))
            }
            Self::Story(GalleryPage::EdgeAutoScroll) => {
                Some(AnyView::new(stories::EdgeAutoScrollStory::new(options)))
            }
            Self::Story(GalleryPage::ScrollReveal) => {
                Some(AnyView::new(stories::ScrollRevealStory::new(options)))
            }
            Self::Story(GalleryPage::ActionGroupLayout) => {
                Some(AnyView::new(stories::ActionGroupLayoutStory::new(options)))
            }
            Self::Story(GalleryPage::Timeline) => {
                Some(AnyView::new(stories::TimelineStory::new(options)))
            }
            Self::Story(GalleryPage::Panel) => {
                Some(AnyView::new(stories::PanelStory::new(options)))
            }
            Self::Story(GalleryPage::SplitView) => {
                Some(AnyView::new(stories::SplitViewStory::new(options)))
            }
            Self::Story(GalleryPage::ScrollView) => {
                Some(AnyView::new(stories::ScrollViewStory::new(options)))
            }
            Self::Story(GalleryPage::BoundedScroll) => {
                Some(AnyView::new(stories::BoundedScrollStory::new(options)))
            }
            Self::Story(GalleryPage::PropertyGrid) => {
                Some(AnyView::new(stories::PropertyGridStory::new(options)))
            }
            Self::Story(GalleryPage::ZStack) => {
                Some(AnyView::new(stories::ZStackStory::new(options)))
            }
            Self::Story(GalleryPage::StatusBar) => {
                Some(AnyView::new(stories::StatusBarStory::new(options)))
            }
            Self::Story(GalleryPage::Carousel) => {
                Some(AnyView::new(stories::CarouselStory::new(options)))
            }
            Self::Story(GalleryPage::Layout) => {
                Some(AnyView::new(stories::LayoutGridStory::new(options)))
            }
            Self::Story(GalleryPage::Theme) => Some(AnyView::new(stories::ThemeStory::new())),
            Self::Story(GalleryPage::Radio) => {
                Some(AnyView::new(stories::RadioStory::new(options)))
            }
            Self::Story(GalleryPage::Form) => Some(AnyView::new(stories::FormStory::new(options))),
            Self::Story(GalleryPage::Combobox) => {
                Some(AnyView::new(stories::ComboboxStory::new(options)))
            }
            Self::Story(GalleryPage::Notification) => {
                Some(AnyView::new(stories::NotificationStory::new()))
            }
            Self::Story(GalleryPage::Tooltip) => {
                Some(AnyView::new(stories::TooltipStory::new(options)))
            }
            Self::Story(GalleryPage::Table) => {
                Some(AnyView::new(stories::TableStory::new(options)))
            }
            Self::Story(GalleryPage::DatePicker) => {
                Some(AnyView::new(stories::DatePickerStory::new(options)))
            }
            Self::Story(GalleryPage::DateRangePicker) => {
                Some(AnyView::new(stories::DateRangeStory::new(options)))
            }
            Self::Story(GalleryPage::TimeField) => {
                Some(AnyView::new(stories::TimeFieldStory::new(options)))
            }
            Self::Story(GalleryPage::Masonry) => {
                Some(AnyView::new(stories::MasonryStory::new(options)))
            }
            Self::Story(GalleryPage::TextRuns) => Some(AnyView::new(stories::TextStory::new())),
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
                Some(AnyView::new(stories::RatingStory::new(options)))
            }
            Self::Story(GalleryPage::Popconfirm) => {
                Some(AnyView::new(stories::PopconfirmStory::new(options)))
            }
            Self::Story(GalleryPage::HoverCard) => {
                Some(AnyView::new(stories::HoverCardStory::new(options)))
            }
            Self::Story(GalleryPage::Chart) => Some(AnyView::new(stories::ChartStory::new())),
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
                Some(AnyView::new(stories::ContextMenuStory::new(options)))
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

    /// The operator-facing message for a page without a story.
    pub fn pending_message(&self) -> Option<String> {
        match self {
            Self::Story(_) => None,
            Self::Pending { page, story_file } => Some(format!(
                "{} has no Rust story yet; port it from Stories/{}.",
                page.raw_value(),
                story_file
            )),
        }
    }
}

/// Where every page's story comes from in the Swift gallery.
pub struct StorySource;

impl StorySource {
    /// `(page, story file)` for every catalog page, in catalog order.
    pub fn entries() -> &'static [(GalleryPage, &'static str)] {
        &[
            (GalleryPage::Introduction, "Overview/Dashboard.swift"),
            (GalleryPage::Button, "Controls/ControlStories.swift"),
            (GalleryPage::Checkbox, "Controls/ControlStories.swift"),
            (GalleryPage::Toggle, "Controls/ControlStories.swift"),
            (GalleryPage::Slider, "Feedback/FeedbackStories.swift"),
            (GalleryPage::RangeSlider, "Controls/ControlStories.swift"),
            (GalleryPage::Rating, "Feedback/RatingStory.swift"),
            (GalleryPage::TextField, "Controls/ControlStories.swift"),
            (GalleryPage::TextEditor, "Controls/TextEditorStory.swift"),
            (GalleryPage::WheelPicker, "Controls/WheelPickerStory.swift"),
            (GalleryPage::NumberField, "Controls/ControlStories.swift"),
            (GalleryPage::ColorField, "Controls/ControlStories.swift"),
            (GalleryPage::Select, "Controls/ControlStories.swift"),
            (GalleryPage::AssetRef, "Data/DataStories.swift"),
            (GalleryPage::List, "Data/DataStories.swift"),
            (GalleryPage::SectionedList, "Data/DataStories.swift"),
            (GalleryPage::ReorderableList, "Data/DataStories.swift"),
            (GalleryPage::SwipeList, "Data/DataStories.swift"),
            (GalleryPage::RefreshableList, "Data/DataStories.swift"),
            (GalleryPage::Scrollbar, "Data/DataStories.swift"),
            (
                GalleryPage::ViewportTraits,
                "Foundation/SurfaceTraits.swift",
            ),
            (
                GalleryPage::ViewThatFits,
                "Foundation/IntrinsicFitting.swift",
            ),
            (GalleryPage::DragAndDrop, "Data/DataStories.swift"),
            (GalleryPage::Tree, "Data/DataStories.swift"),
            (GalleryPage::VirtualStack, "Data/DataStories.swift"),
            (GalleryPage::Chart, "Data/DataStories.swift"),
            (GalleryPage::Icon, "Data/DataStories.swift"),
            (GalleryPage::Avatar, "Feedback/AvatarStory.swift"),
            (GalleryPage::Image, "Layout/ImageStory.swift"),
            (GalleryPage::Web, "Data/WebStory.swift"),
            (GalleryPage::TextRuns, "Data/RichTextStory.swift"),
            (GalleryPage::Markdown, "Data/MarkdownStory.swift"),
            (GalleryPage::Menu, "Layout/OverlayStories.swift"),
            (GalleryPage::MenuButton, "Controls/ControlStories.swift"),
            (GalleryPage::ContextMenu, "Layout/OverlayStories.swift"),
            (GalleryPage::Popover, "Layout/OverlayStories.swift"),
            (GalleryPage::Modal, "Layout/OverlayStories.swift"),
            (GalleryPage::Disclosure, "Layout/OverlayStories.swift"),
            (GalleryPage::Tabs, "Layout/OverlayStories.swift"),
            (GalleryPage::Panel, "Layout/LayoutStories.swift"),
            (GalleryPage::SplitView, "Layout/LayoutStories.swift"),
            (GalleryPage::ScrollView, "Layout/LayoutStories.swift"),
            (GalleryPage::BoundedScroll, "Layout/LayoutStories.swift"),
            (GalleryPage::PropertyGrid, "Layout/LayoutStories.swift"),
            (GalleryPage::Workspace, "Layout/LayoutStories.swift"),
            (GalleryPage::Layout, "Layout/LayoutStories.swift"),
            (GalleryPage::Masonry, "Layout/MasonryStory.swift"),
            (GalleryPage::ZStack, "Layout/LayoutStories.swift"),
            (GalleryPage::Animation, "Layout/LayoutStories.swift"),
            (GalleryPage::Theme, "Foundation/ThemeStory.swift"),
            (GalleryPage::Badge, "Feedback/FeedbackStories.swift"),
            (GalleryPage::Alert, "Feedback/FeedbackStories.swift"),
            (GalleryPage::Progress, "Feedback/FeedbackStories.swift"),
            (GalleryPage::Empty, "Feedback/FeedbackStories.swift"),
            (GalleryPage::Radio, "Layout/ProductStories.swift"),
            (GalleryPage::Form, "Layout/ProductStories.swift"),
            (GalleryPage::Combobox, "Layout/ProductStories.swift"),
            (GalleryPage::Notification, "Layout/ProductStories.swift"),
            (GalleryPage::Tooltip, "Layout/ProductStories.swift"),
            (GalleryPage::Sidebar, "Layout/SidebarStory.swift"),
            (GalleryPage::NavigationDrawer, "Layout/DrawerStory.swift"),
            (GalleryPage::NavigationRail, "Layout/RailStory.swift"),
            (GalleryPage::AppBar, "Navigation/AppBarStory.swift"),
            (GalleryPage::Table, "Data/DateTableStories.swift"),
            (GalleryPage::DataTable, "Data/DataTableStory.swift"),
            (GalleryPage::Calendar, "Data/DateTableStories.swift"),
            (GalleryPage::DatePicker, "Data/DateTableStories.swift"),
            (
                GalleryPage::DateRangePicker,
                "Controls/ControlStories.swift",
            ),
            (GalleryPage::TimeField, "Data/DateTableStories.swift"),
            (GalleryPage::TimePicker, "Controls/TimePickerStory.swift"),
            (GalleryPage::Popconfirm, "Overlays/PopconfirmStory.swift"),
            (GalleryPage::ActionSheet, "Overlays/ActionSheetStory.swift"),
            (GalleryPage::Accordion, "Layout/NavigationStories.swift"),
            (GalleryPage::Breadcrumb, "Layout/NavigationStories.swift"),
            (GalleryPage::Pagination, "Layout/NavigationStories.swift"),
            (GalleryPage::ButtonGroup, "Layout/NavigationStories.swift"),
            (GalleryPage::SplitButton, "Controls/ControlStories.swift"),
            (GalleryPage::Tag, "Layout/NavigationStories.swift"),
            (GalleryPage::KeyCap, "Layout/NavigationStories.swift"),
            (GalleryPage::Label, "Layout/NavigationStories.swift"),
            (GalleryPage::Dialog, "Layout/NavigationStories.swift"),
            (GalleryPage::AlertDialog, "Layout/NavigationStories.swift"),
            (GalleryPage::Sheet, "Layout/NavigationStories.swift"),
            (GalleryPage::BottomSheet, "Layout/NavigationStories.swift"),
            (GalleryPage::CommandPalette, "Overlays/CommandStory.swift"),
            (
                GalleryPage::NavigationStack,
                "Layout/NavigationStories.swift",
            ),
            (GalleryPage::HoverCard, "Overlays/HoverStory.swift"),
            (GalleryPage::Timeline, "Data/DataStories.swift"),
            (GalleryPage::Toolbar, "Controls/ToolbarStory.swift"),
            (GalleryPage::Link, "Controls/ControlStories.swift"),
            (GalleryPage::SingleLineText, "Text/SingleLineText.swift"),
            (
                GalleryPage::CompletionPopover,
                "Controls/CompletionPopover.swift",
            ),
            (GalleryPage::RouteOutlet, "Navigation/RouteOutlet.swift"),
            (GalleryPage::ProgressValue, "Feedback/FeedbackStories.swift"),
            (
                GalleryPage::HorizontalSwipe,
                "Primitives/Interaction/HorizontalSwipe.swift",
            ),
            (
                GalleryPage::EdgeAutoScroll,
                "Primitives/Interaction/EdgeAutoScroll.swift",
            ),
            (
                GalleryPage::ScrollReveal,
                "Primitives/Layout/ScrollReveal.swift",
            ),
            (
                GalleryPage::ActionGroupLayout,
                "Layout/Responsive/ActionGroupLayout.swift",
            ),
            (GalleryPage::StatusBar, "Layout/LayoutStories.swift"),
            (GalleryPage::Carousel, "Layout/LayoutStories.swift"),
            (GalleryPage::Skeleton, "Feedback/SkeletonStory.swift"),
            (GalleryPage::Stepper, "Controls/StepperStory.swift"),
        ]
    }

    /// The Swift story file for one page.
    pub fn entry(page: GalleryPage) -> &'static str {
        Self::entries()
            .iter()
            .find(|(candidate, _)| *candidate == page)
            .map(|(_, file)| *file)
            .unwrap_or("")
    }
}

#[cfg(test)]
mod checks {
    use super::*;
    use crate::gallery_content::catalog::GalleryCatalog;

    #[test]
    fn every_catalog_page_has_exactly_one_entry() {
        let entries = StorySource::entries();
        assert_eq!(
            entries.len(),
            crate::gallery_content::catalog::GalleryPage::ALL.len(),
            "one row per catalog page"
        );
        for entry in GalleryCatalog::entries() {
            assert!(
                entries.iter().any(|(page, _)| *page == entry.page),
                "{} is missing from the story table",
                entry.page.raw_value()
            );
        }
        let unique: std::collections::BTreeSet<_> = entries.iter().map(|(page, _)| *page).collect();
        assert_eq!(
            unique.len(),
            crate::gallery_content::catalog::GalleryPage::ALL.len(),
            "no page appears twice"
        );
    }

    #[test]
    fn every_entry_names_a_real_story_file() {
        for (page, file) in StorySource::entries() {
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
    fn introduction_dispatches_to_its_story() {
        let dispatch = StoryDispatch::new(GalleryPage::Introduction);
        assert!(dispatch.is_implemented());
        assert!(dispatch.view(GalleryOptions::default()).is_some());
        assert!(dispatch.pending_message().is_none());
    }

    #[test]
    fn every_implemented_view_arm_is_reachable() {
        // A page with a real story must resolve to `Story`; a page dispatched as
        // `Pending` must have no story. Both directions are checked so neither a
        // dead arm nor a mislabelled page can pass.
        for entry in GalleryCatalog::entries() {
            let page = entry.page;
            let dispatch = StoryDispatch::new(page);
            let has_story = dispatch.view(GalleryOptions::default()).is_some();
            assert_eq!(
                has_story,
                dispatch.is_implemented(),
                "{} disagrees between dispatch and its story arm",
                page.raw_value()
            );
        }
    }

    #[test]
    fn only_the_workspace_page_is_pending() {
        let pending: Vec<_> = GalleryCatalog::entries()
            .into_iter()
            .map(|entry| entry.page)
            .filter(|page| !StoryDispatch::new(*page).is_implemented())
            .collect();
        assert_eq!(pending, vec![GalleryPage::Workspace]);
        let message = StoryDispatch::new(GalleryPage::Workspace)
            .pending_message()
            .expect("a pending page explains itself");
        assert!(message.contains("Layout/LayoutStories.swift"), "{message}");
    }
}
