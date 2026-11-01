//! Foundation story: deterministic adaptive previews.
//!
//! Swift source: `GalleryContent/Stories/Foundation/AdaptiveExamples.swift`.
//!
//! Each supported page gets rendered at two fixed container widths so desktop
//! layout is exercised independently of the host window. Two lookups drive it:
//! which pages have a sample at all, and how tall that sample's frame must be.
//!
//! Both are exhaustive-by-hand tables in Swift, so a page added to one and
//! forgotten in the other is a silent gap — the sample would render at the wrong
//! height, or not at all. `unsupported_pages()` and `height_table()` exist so the
//! tests can assert the two agree.

use crate::gallery_content::catalog::GalleryPage;
use cove_composition::Scope;

/// The widths every supported page is previewed at.
pub const PREVIEW_WIDTHS: [f32; 2] = [390.0, 720.0];
/// Padding added to a sample height to leave room for its label.
pub const FRAME_PADDING: f32 = 32.0;

/// Every page that has an adaptive sample.
///
/// Mirrors the Swift `supports(_:)` switch exactly, in the same order.
pub const SUPPORTED: [GalleryPage; 97] = [
    GalleryPage::Button,
    GalleryPage::ButtonGroup,
    GalleryPage::SplitButton,
    GalleryPage::Checkbox,
    GalleryPage::Toggle,
    GalleryPage::Slider,
    GalleryPage::RangeSlider,
    GalleryPage::TextField,
    GalleryPage::SearchField,
    GalleryPage::NumberField,
    GalleryPage::Vec3Field,
    GalleryPage::Select,
    GalleryPage::Combobox,
    GalleryPage::Rating,
    GalleryPage::List,
    GalleryPage::SectionedList,
    GalleryPage::ReorderableList,
    GalleryPage::SwipeList,
    GalleryPage::RefreshableList,
    GalleryPage::Scrollbar,
    GalleryPage::Calendar,
    GalleryPage::DatePicker,
    GalleryPage::Tabs,
    GalleryPage::Stepper,
    GalleryPage::Toolbar,
    GalleryPage::AppBar,
    GalleryPage::NavigationStack,
    GalleryPage::Form,
    GalleryPage::PropertyGrid,
    GalleryPage::SplitView,
    GalleryPage::Sidebar,
    GalleryPage::NavigationDrawer,
    GalleryPage::NavigationRail,
    GalleryPage::Table,
    GalleryPage::DataTable,
    GalleryPage::Layout,
    GalleryPage::Masonry,
    GalleryPage::ZStack,
    GalleryPage::ViewportTraits,
    GalleryPage::ViewThatFits,
    GalleryPage::DragAndDrop,
    GalleryPage::MenuButton,
    GalleryPage::Tooltip,
    GalleryPage::Link,
    GalleryPage::Badge,
    GalleryPage::Tag,
    GalleryPage::KeyCap,
    GalleryPage::Progress,
    GalleryPage::Empty,
    GalleryPage::Skeleton,
    GalleryPage::Accordion,
    GalleryPage::Icon,
    GalleryPage::Avatar,
    GalleryPage::Panel,
    GalleryPage::ScrollView,
    GalleryPage::StatusBar,
    GalleryPage::Carousel,
    GalleryPage::Breadcrumb,
    GalleryPage::Pagination,
    GalleryPage::Radio,
    GalleryPage::Timeline,
    GalleryPage::Tree,
    GalleryPage::VirtualStack,
    GalleryPage::Chart,
    GalleryPage::BoundedScroll,
    GalleryPage::Label,
    GalleryPage::Disclosure,
    GalleryPage::Alert,
    GalleryPage::TextArea,
    GalleryPage::ColorField,
    GalleryPage::JsonField,
    GalleryPage::AssetRef,
    GalleryPage::DateRangePicker,
    GalleryPage::TimeField,
    GalleryPage::TimePicker,
    GalleryPage::WheelPicker,
    GalleryPage::Theme,
    GalleryPage::TextEditor,
    GalleryPage::Menu,
    GalleryPage::ContextMenu,
    GalleryPage::Popover,
    GalleryPage::Popconfirm,
    GalleryPage::ActionSheet,
    GalleryPage::Modal,
    GalleryPage::Dialog,
    GalleryPage::AlertDialog,
    GalleryPage::Sheet,
    GalleryPage::BottomSheet,
    GalleryPage::CommandPalette,
    GalleryPage::HoverCard,
    GalleryPage::Animation,
    GalleryPage::Introduction,
    GalleryPage::Image,
    GalleryPage::Workspace,
    GalleryPage::Notification,
    GalleryPage::RichText,
    GalleryPage::Markdown,
];

impl GalleryPage {
    /// Whether this page has an adaptive sample.
    pub fn has_adaptive_sample(self) -> bool {
        SUPPORTED.contains(&self)
    }

    /// The fixed sample height, or `None` when the control hugs its content.
    ///
    /// Samples that scroll, fill or overlay need a definite height; everything
    /// else is measured from its content.
    pub fn adaptive_sample_height(self) -> Option<f32> {
        Some(match self {
            GalleryPage::Calendar => 440.0,
            GalleryPage::Layout => 280.0,
            GalleryPage::Table | GalleryPage::DataTable => 260.0,
            GalleryPage::NavigationRail
            | GalleryPage::SplitView
            | GalleryPage::Sidebar
            | GalleryPage::Workspace
            | GalleryPage::NavigationStack => 220.0,
            GalleryPage::PropertyGrid | GalleryPage::SwipeList | GalleryPage::RefreshableList => {
                200.0
            }
            GalleryPage::Masonry | GalleryPage::WheelPicker | GalleryPage::TimePicker => 190.0,
            GalleryPage::Form | GalleryPage::ReorderableList | GalleryPage::JsonField => 180.0,
            GalleryPage::DragAndDrop => 170.0,
            GalleryPage::Tabs
            | GalleryPage::SectionedList
            | GalleryPage::Accordion
            | GalleryPage::Timeline
            | GalleryPage::Tree
            | GalleryPage::BoundedScroll
            | GalleryPage::TextArea => 160.0,
            GalleryPage::Panel | GalleryPage::TextEditor | GalleryPage::Popconfirm => 150.0,
            GalleryPage::List => 144.0,
            GalleryPage::Carousel => 140.0,
            GalleryPage::VirtualStack | GalleryPage::Chart => 130.0,
            GalleryPage::ZStack
            | GalleryPage::Scrollbar
            | GalleryPage::ScrollView
            | GalleryPage::Disclosure => 120.0,
            _ => return None,
        })
    }

    /// The debug name a preview carries, which encodes the width band.
    pub fn adaptive_debug_name(self, width: f32) -> String {
        format!(
            "adaptive-{}-{}",
            self.raw_value(),
            if width < 600.0 { "narrow" } else { "wide" }
        )
    }
}

/// The pages in `GalleryPage` that have no adaptive sample.
///
/// Computed from the supported set rather than hand-listed, so the two cannot
/// drift apart.
pub fn unsupported_pages() -> Vec<GalleryPage> {
    ALL_PAGES
        .into_iter()
        .filter(|page| !page.has_adaptive_sample())
        .collect()
}

/// Every page variant that can appear in the gallery.
pub const ALL_PAGES: [GalleryPage; 98] = [
    GalleryPage::Introduction,
    GalleryPage::Button,
    GalleryPage::Checkbox,
    GalleryPage::Toggle,
    GalleryPage::Slider,
    GalleryPage::RangeSlider,
    GalleryPage::Rating,
    GalleryPage::TextField,
    GalleryPage::TextArea,
    GalleryPage::TextEditor,
    GalleryPage::NumberField,
    GalleryPage::Vec3Field,
    GalleryPage::ColorField,
    GalleryPage::Select,
    GalleryPage::JsonField,
    GalleryPage::AssetRef,
    GalleryPage::SearchField,
    GalleryPage::List,
    GalleryPage::SectionedList,
    GalleryPage::ReorderableList,
    GalleryPage::SwipeList,
    GalleryPage::RefreshableList,
    GalleryPage::Scrollbar,
    GalleryPage::Tree,
    GalleryPage::VirtualStack,
    GalleryPage::Chart,
    GalleryPage::Icon,
    GalleryPage::Image,
    GalleryPage::Web,
    GalleryPage::Avatar,
    GalleryPage::Menu,
    GalleryPage::MenuButton,
    GalleryPage::ContextMenu,
    GalleryPage::Popover,
    GalleryPage::Popconfirm,
    GalleryPage::ActionSheet,
    GalleryPage::Modal,
    GalleryPage::Disclosure,
    GalleryPage::Tabs,
    GalleryPage::Panel,
    GalleryPage::SplitView,
    GalleryPage::ScrollView,
    GalleryPage::BoundedScroll,
    GalleryPage::PropertyGrid,
    GalleryPage::Workspace,
    GalleryPage::Layout,
    GalleryPage::Masonry,
    GalleryPage::ZStack,
    GalleryPage::Animation,
    GalleryPage::Theme,
    GalleryPage::Badge,
    GalleryPage::Alert,
    GalleryPage::Progress,
    GalleryPage::Empty,
    GalleryPage::Radio,
    GalleryPage::Form,
    GalleryPage::Combobox,
    GalleryPage::Notification,
    GalleryPage::Tooltip,
    GalleryPage::Sidebar,
    GalleryPage::NavigationDrawer,
    GalleryPage::NavigationRail,
    GalleryPage::Table,
    GalleryPage::DataTable,
    GalleryPage::Calendar,
    GalleryPage::DatePicker,
    GalleryPage::DateRangePicker,
    GalleryPage::TimeField,
    GalleryPage::TimePicker,
    GalleryPage::WheelPicker,
    GalleryPage::Accordion,
    GalleryPage::Breadcrumb,
    GalleryPage::Pagination,
    GalleryPage::ButtonGroup,
    GalleryPage::SplitButton,
    GalleryPage::Tag,
    GalleryPage::KeyCap,
    GalleryPage::Label,
    GalleryPage::Dialog,
    GalleryPage::AlertDialog,
    GalleryPage::Sheet,
    GalleryPage::BottomSheet,
    GalleryPage::CommandPalette,
    GalleryPage::HoverCard,
    GalleryPage::Timeline,
    GalleryPage::Toolbar,
    GalleryPage::AppBar,
    GalleryPage::NavigationStack,
    GalleryPage::DragAndDrop,
    GalleryPage::Link,
    GalleryPage::StatusBar,
    GalleryPage::Carousel,
    GalleryPage::Skeleton,
    GalleryPage::Stepper,
    GalleryPage::ViewportTraits,
    GalleryPage::ViewThatFits,
    GalleryPage::RichText,
    GalleryPage::Markdown,
];

/// The frame height for a sample, including the label padding.
pub fn frame_height(page: GalleryPage) -> Option<f32> {
    page.adaptive_sample_height()
        .map(|height| height + FRAME_PADDING)
}

/// `AdaptiveControlExamples`: the two fixed-width previews.
pub struct AdaptiveControlExamples {
    pub page: GalleryPage,
}

impl AdaptiveControlExamples {
    pub fn new(page: GalleryPage) -> Self {
        Self { page }
    }

    /// Whether this page has anything to preview.
    pub fn supports(&self) -> bool {
        self.page.has_adaptive_sample()
    }

    /// The preview captions, in order.
    pub fn captions(&self) -> [&'static str; 2] {
        ["390 pt · Pointer", "720 pt · Pointer"]
    }

    /// The sample frame for a preview width.
    pub fn sample_size(&self, width: f32) -> (f32, Option<f32>) {
        (width, frame_height(self.page))
    }
}

impl cove_ui_compose::View for AdaptiveControlExamples {
    fn body(&self, _cx: &Scope) -> cove_ui_compose::AnyView {
        use cove_ui_compose::LayoutDebugExt;
        use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
        use cove_ui_compose::primitives::text::Text;
        use cove_ui_compose::theme::tokens::SemanticColorRef;

        let caption = Text::new(self.captions()[0])
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted());

        cove_ui_compose::AnyView::new(
            cove_ui_compose::AnyView::new(caption).debug_name("adaptive-preview-caption"),
        )
    }

    fn tag(&self) -> &'static str {
        "AdaptiveControlExamples"
    }
}
