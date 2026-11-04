//! The gallery page catalog, its entries and the shared story options.
//!
//! Swift source: `GalleryContent/GalleryCatalog.swift`. The entry order,
//! titles, categories and summaries are reproduced verbatim because the
//! gallery shell lists them and the search index matches against them.

use serde::{Deserialize, Serialize};

/// Every gallery page, in catalog order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum GalleryPage {
    Introduction,
    Button,
    Checkbox,
    Toggle,
    Slider,
    RangeSlider,
    Rating,
    TextField,
    TextEditor,
    NumberField,
    ColorField,
    Select,
    AssetRef,
    List,
    SectionedList,
    ReorderableList,
    SwipeList,
    RefreshableList,
    Scrollbar,
    Tree,
    VirtualStack,
    Chart,
    Icon,
    Image,
    Web,
    Avatar,
    Menu,
    MenuButton,
    ContextMenu,
    Popover,
    Popconfirm,
    ActionSheet,
    Modal,
    Disclosure,
    Tabs,
    Panel,
    SplitView,
    ScrollView,
    BoundedScroll,
    PropertyGrid,
    Workspace,
    Layout,
    Masonry,
    ZStack,
    Animation,
    Theme,
    Badge,
    Alert,
    Progress,
    Empty,
    Radio,
    Form,
    Combobox,
    Notification,
    Tooltip,
    Sidebar,
    NavigationDrawer,
    NavigationRail,
    Table,
    DataTable,
    Calendar,
    DatePicker,
    DateRangePicker,
    TimeField,
    TimePicker,
    WheelPicker,
    Accordion,
    Breadcrumb,
    Pagination,
    ButtonGroup,
    SplitButton,
    Tag,
    KeyCap,
    Label,
    Dialog,
    AlertDialog,
    Sheet,
    BottomSheet,
    CommandPalette,
    HoverCard,
    Timeline,
    Toolbar,
    AppBar,
    NavigationStack,
    DragAndDrop,
    Link,
    StatusBar,
    Carousel,
    Skeleton,
    Stepper,
    ViewportTraits,
    ViewThatFits,
    TextRuns,
    Markdown,
    SingleLineText,
    CompletionPopover,
    RouteOutlet,
    ProgressValue,
    HorizontalSwipe,
    EdgeAutoScroll,
    ScrollReveal,
    ActionGroupLayout,
}

impl GalleryPage {
    /// The `rawValue` used by `--component` matching and story identity.
    pub const fn raw_value(self) -> &'static str {
        match self {
            Self::Introduction => "introduction",
            Self::Button => "button",
            Self::Checkbox => "checkbox",
            Self::Toggle => "toggle",
            Self::Slider => "slider",
            Self::RangeSlider => "rangeSlider",
            Self::Rating => "rating",
            Self::TextField => "textFields",
            Self::TextEditor => "textEditor",
            Self::NumberField => "numeric",
            Self::ColorField => "colorField",
            Self::Select => "select",
            Self::AssetRef => "assetRef",
            Self::List => "list",
            Self::SectionedList => "sectionedList",
            Self::ReorderableList => "reorderableList",
            Self::SwipeList => "swipeList",
            Self::RefreshableList => "refreshableList",
            Self::Scrollbar => "scrollbar",
            Self::Tree => "tree",
            Self::VirtualStack => "virtualStack",
            Self::Chart => "chart",
            Self::Icon => "icon",
            Self::Image => "image",
            Self::Web => "web",
            Self::Avatar => "avatar",
            Self::Menu => "menu",
            Self::MenuButton => "menuButton",
            Self::ContextMenu => "contextMenu",
            Self::Popover => "popover",
            Self::Popconfirm => "popconfirm",
            Self::ActionSheet => "actionSheet",
            Self::Modal => "modal",
            Self::Disclosure => "disclosure",
            Self::Tabs => "tabs",
            Self::Panel => "panel",
            Self::SplitView => "splitView",
            Self::ScrollView => "scrollView",
            Self::BoundedScroll => "boundedScroll",
            Self::PropertyGrid => "propertyGrid",
            Self::Workspace => "workspace",
            Self::Layout => "layout",
            Self::Masonry => "masonry",
            Self::ZStack => "zStack",
            Self::Animation => "animation",
            Self::Theme => "theme",
            Self::Badge => "badge",
            Self::Alert => "alert",
            Self::Progress => "progress",
            Self::Empty => "empty",
            Self::Radio => "radio",
            Self::Form => "form",
            Self::Combobox => "combobox",
            Self::Notification => "notification",
            Self::Tooltip => "tooltip",
            Self::Sidebar => "sidebar",
            Self::NavigationDrawer => "navigationDrawer",
            Self::NavigationRail => "navigationRail",
            Self::Table => "table",
            Self::DataTable => "dataTable",
            Self::Calendar => "calendar",
            Self::DatePicker => "datePicker",
            Self::DateRangePicker => "dateRangePicker",
            Self::TimeField => "timeField",
            Self::TimePicker => "timePicker",
            Self::WheelPicker => "wheelPicker",
            Self::Accordion => "accordion",
            Self::Breadcrumb => "breadcrumb",
            Self::Pagination => "pagination",
            Self::ButtonGroup => "buttonGroup",
            Self::SplitButton => "splitButton",
            Self::Tag => "tag",
            Self::KeyCap => "keyCap",
            Self::Label => "label",
            Self::Dialog => "dialog",
            Self::AlertDialog => "alertDialog",
            Self::Sheet => "sheet",
            Self::BottomSheet => "bottomSheet",
            Self::CommandPalette => "commandPalette",
            Self::HoverCard => "hoverCard",
            Self::Timeline => "timeline",
            Self::Toolbar => "toolbar",
            Self::AppBar => "appBar",
            Self::NavigationStack => "navigationStack",
            Self::DragAndDrop => "dragAndDrop",
            Self::Link => "link",
            Self::StatusBar => "statusBar",
            Self::Carousel => "carousel",
            Self::Skeleton => "skeleton",
            Self::Stepper => "stepper",
            Self::ViewportTraits => "viewportTraits",
            Self::ViewThatFits => "viewThatFits",
            Self::TextRuns => "text",
            Self::Markdown => "markdown",
            Self::SingleLineText => "singleLineText",
            Self::CompletionPopover => "completionPopover",
            Self::RouteOutlet => "routeOutlet",
            Self::ProgressValue => "progressValue",
            Self::HorizontalSwipe => "horizontalSwipe",
            Self::EdgeAutoScroll => "edgeAutoScroll",
            Self::ScrollReveal => "scrollReveal",
            Self::ActionGroupLayout => "actionGroupLayout",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        GalleryCatalog::entries()
            .iter()
            .find(|entry| entry.page.raw_value() == value)
            .map(|entry| entry.page)
    }

    /// The catalog entry for this page.
    pub fn entry(self) -> GalleryEntry {
        GalleryCatalog::entries()
            .into_iter()
            .find(|entry| entry.page == self)
            .expect("every page has a catalog entry")
    }
}

/// One catalog row: page identity plus the text the shell displays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GalleryEntry {
    pub page: GalleryPage,
    pub title: &'static str,
    pub category: &'static str,
    pub summary: &'static str,
}

impl GalleryEntry {
    const fn new(
        page: GalleryPage,
        title: &'static str,
        category: &'static str,
        summary: &'static str,
    ) -> Self {
        Self {
            page,
            title,
            category,
            summary,
        }
    }
}

/// The page catalog.
pub struct GalleryCatalog;

impl GalleryCatalog {
    /// Every story, in the original declaration order.
    pub fn entries() -> Vec<GalleryEntry> {
        vec![
            GalleryEntry::new(
                GalleryPage::Introduction,
                "Dashboard",
                "Overview",
                "A working composition of CoveUI typography, surfaces, controls and live state.",
            ),
            GalleryEntry::new(
                GalleryPage::Button,
                "Button",
                "Controls",
                "Action hierarchy, semantic variants, icons, loading and control density.",
            ),
            GalleryEntry::new(
                GalleryPage::Checkbox,
                "Checkbox",
                "Controls",
                "Unchecked, checked, mixed and disabled selection.",
            ),
            GalleryEntry::new(
                GalleryPage::Toggle,
                "Toggle / Switch",
                "Controls",
                "Boolean settings with pointer capture and keyboard activation.",
            ),
            GalleryEntry::new(
                GalleryPage::Slider,
                "Slider",
                "Controls",
                "Continuous and stepped values with pointer capture and keyboard control.",
            ),
            GalleryEntry::new(
                GalleryPage::RangeSlider,
                "RangeSlider",
                "Controls",
                "Two thumbs, active-thumb pointer control, minimum span and accessibility value.",
            ),
            GalleryEntry::new(
                GalleryPage::Rating,
                "Rating",
                "Controls",
                "Whole and half stars, pointer preview, keyboard selection, read-only scores and density.",
            ),
            GalleryEntry::new(
                GalleryPage::TextField,
                "Text Fields",
                "Inputs",
                "Single-line, search, multiline and JSON entry: password and adornments, query submit and suggestions, wrapping with a resize grip, JSON validation, formatting and revert.",
            ),
            GalleryEntry::new(
                GalleryPage::TextEditor,
                "Text Editor",
                "Inputs",
                "Multiline prose and code editing: syntax colours, indent guides, bracket matching, current-line treatment, completion and undo.",
            ),
            GalleryEntry::new(
                GalleryPage::NumberField,
                "Numeric",
                "Inputs",
                "Numeric commit on submit or blur, bounded stepping, and three-axis vector editing with constraints and mixed values.",
            ),
            GalleryEntry::new(
                GalleryPage::ColorField,
                "ColorField",
                "Inputs",
                "RGBA channels and hexadecimal color input in a popover.",
            ),
            GalleryEntry::new(
                GalleryPage::Select,
                "Select",
                "Inputs",
                "Typed selection, disabled options and keyboard navigation.",
            ),
            GalleryEntry::new(
                GalleryPage::AssetRef,
                "AssetRef / DropTarget",
                "Inputs",
                "Typed asset references, picker, clear action and accepted kinds.",
            ),
            GalleryEntry::new(
                GalleryPage::List,
                "List",
                "Data",
                "Single selection, arrow-key navigation and activation.",
            ),
            GalleryEntry::new(
                GalleryPage::SectionedList,
                "SectionedList",
                "Data",
                "Sticky section headers and footers over the virtualized list contract.",
            ),
            GalleryEntry::new(
                GalleryPage::ReorderableList,
                "ReorderableList",
                "Data",
                "Drag handles, a drop indicator and keyboard moves over a caller-owned order.",
            ),
            GalleryEntry::new(
                GalleryPage::SwipeList,
                "SwipeList",
                "Data",
                "Trailing swipe actions on touch, plus an always-available row actions button and context menu.",
            ),
            GalleryEntry::new(
                GalleryPage::RefreshableList,
                "RefreshableList",
                "Data",
                "Touch pull-to-refresh with threshold damping, an idle/frozen indicator and Cmd+R.",
            ),
            GalleryEntry::new(
                GalleryPage::Scrollbar,
                "Scrollbar",
                "Data",
                "A focusable slider thumb bound to a ScrollView offset.",
            ),
            GalleryEntry::new(
                GalleryPage::Tree,
                "Tree",
                "Data",
                "Hierarchy, expansion, selection and search with automatic expansion.",
            ),
            GalleryEntry::new(
                GalleryPage::VirtualStack,
                "VirtualStack",
                "Data",
                "Fixed or variable row heights, viewport-driven materialization and scroll-to-index.",
            ),
            GalleryEntry::new(
                GalleryPage::Chart,
                "Chart",
                "Data",
                "Line, area, grouped or stacked bars, scatter and donut charts with selection and a toggleable legend.",
            ),
            GalleryEntry::new(
                GalleryPage::Icon,
                "Icon",
                "Data",
                "Bundled SVG resources tinted by the current semantic theme.",
            ),
            GalleryEntry::new(
                GalleryPage::Avatar,
                "Avatar",
                "Data",
                "Asynchronous images, Unicode initials, stable theme colors and overlapping member groups.",
            ),
            GalleryEntry::new(
                GalleryPage::Image,
                "Image",
                "Data",
                "Fit/fill/stretch, asynchronous loading, explicit errors and retry.",
            ),
            GalleryEntry::new(
                GalleryPage::TextRuns,
                "Text",
                "Data",
                "Styled runs in one wrapping paragraph: weight, slant, inline code, strike lines and links.",
            ),
            GalleryEntry::new(
                GalleryPage::Markdown,
                "Markdown",
                "Data",
                "CommonMark and GFM documents: headings, lists, quotes, scrolling code blocks, tables and links.",
            ),
            GalleryEntry::new(
                GalleryPage::Web,
                "WebView",
                "Data",
                "A native browser view hosted above the composed surface, with navigation, clipping and lifecycle.",
            ),
            GalleryEntry::new(
                GalleryPage::Menu,
                "Menu",
                "Overlays",
                "Nested commands, keyboard navigation, checked states and scrollable menus.",
            ),
            GalleryEntry::new(
                GalleryPage::MenuButton,
                "MenuButton",
                "Controls",
                "A button-triggered menu with regular trigger chrome, keyboard dismissal and action callbacks.",
            ),
            GalleryEntry::new(
                GalleryPage::ContextMenu,
                "ContextMenu",
                "Overlays",
                "Right-click actions attached to a real target.",
            ),
            GalleryEntry::new(
                GalleryPage::Popover,
                "Popover",
                "Overlays",
                "Anchored overlays, flipping, dismissal and nested input.",
            ),
            GalleryEntry::new(
                GalleryPage::Popconfirm,
                "Popconfirm",
                "Overlays",
                "Anchored destructive and normal confirmation with explicit confirm/cancel.",
            ),
            GalleryEntry::new(
                GalleryPage::ActionSheet,
                "ActionSheet",
                "Overlays",
                "Bottom action drawer with drag and outside dismissal; wide drawers add a caller-localized close control.",
            ),
            GalleryEntry::new(
                GalleryPage::Modal,
                "Modal",
                "Overlays",
                "Focus confinement, Escape dismissal and restoration.",
            ),
            GalleryEntry::new(
                GalleryPage::Disclosure,
                "DisclosureGroup",
                "Navigation",
                "Caller-owned expansion with animated content.",
            ),
            GalleryEntry::new(
                GalleryPage::Tabs,
                "TabView",
                "Navigation",
                "Selected content, disabled tabs and arrow/Home/End navigation.",
            ),
            GalleryEntry::new(
                GalleryPage::Panel,
                "Panel",
                "Layout",
                "Themed panel headers and content slots.",
            ),
            GalleryEntry::new(
                GalleryPage::SplitView,
                "SplitView",
                "Layout",
                "Horizontal and vertical proportional layouts.",
            ),
            GalleryEntry::new(
                GalleryPage::ScrollView,
                "ScrollView",
                "Layout",
                "Nested scrolling, wheel routing and stable scrollbar gutters.",
            ),
            GalleryEntry::new(
                GalleryPage::BoundedScroll,
                "BoundedScrollView",
                "Layout",
                "Content-driven viewport height with minimum and maximum bounds.",
            ),
            GalleryEntry::new(
                GalleryPage::PropertyGrid,
                "PropertyGrid",
                "Layout",
                "Inspector rows, sections, custom sizing and empty state.",
            ),
            GalleryEntry::new(
                GalleryPage::Workspace,
                "Workspace / Dock",
                "Layout",
                "Draggable tabs, split layouts and document serialization.",
            ),
            GalleryEntry::new(
                GalleryPage::Layout,
                "Box / Row / Column",
                "Layout",
                "Alignment, spacing, flex sizing, spacers and dividers.",
            ),
            GalleryEntry::new(
                GalleryPage::Masonry,
                "Masonry",
                "Layout",
                "Balanced one- and two-column waterfall packing for mixed-height cards.",
            ),
            GalleryEntry::new(
                GalleryPage::ZStack,
                "ZStack / Overlay",
                "Layout",
                "Layered content, alignment, topmost hit testing and retained hidden branches.",
            ),
            GalleryEntry::new(
                GalleryPage::Animation,
                "Animation",
                "Foundation",
                "Visibility, curves, springs, keyframes, phases, loops, shared geometry, numeric transitions and completion.",
            ),
            GalleryEntry::new(
                GalleryPage::ViewportTraits,
                "Viewport Traits",
                "Foundation",
                "Width/height classes and orientation derived from the surface viewport.",
            ),
            GalleryEntry::new(
                GalleryPage::ViewThatFits,
                "ViewThatFits",
                "Foundation",
                "Threshold and intrinsic-width branch selection with retained subtrees.",
            ),
            GalleryEntry::new(
                GalleryPage::Theme,
                "Theme / Typography",
                "Foundation",
                "Light and dark semantic surfaces, text and status colors.",
            ),
            GalleryEntry::new(
                GalleryPage::Badge,
                "Badge",
                "Feedback",
                "Compact labels using semantic status tones.",
            ),
            GalleryEntry::new(
                GalleryPage::Alert,
                "Alert",
                "Feedback",
                "Inline feedback with a title, detail and status tone.",
            ),
            GalleryEntry::new(
                GalleryPage::Progress,
                "Progress / Spinner",
                "Feedback",
                "Bounded progress and a lifecycle-managed loading indicator.",
            ),
            GalleryEntry::new(
                GalleryPage::Empty,
                "EmptyState",
                "Feedback",
                "A useful explanation and a recovery action for empty content.",
            ),
            GalleryEntry::new(
                GalleryPage::Radio,
                "RadioGroup",
                "Controls",
                "One Tab stop, controlled selection and arrow-key navigation.",
            ),
            GalleryEntry::new(
                GalleryPage::Form,
                "Form",
                "Inputs",
                "Field layout, required values, validation summaries and submission state.",
            ),
            GalleryEntry::new(
                GalleryPage::Combobox,
                "Combobox",
                "Inputs",
                "Searchable typed options, keyboard highlight, empty results and clearing.",
            ),
            GalleryEntry::new(
                GalleryPage::Notification,
                "Notification",
                "Feedback",
                "Stable-ID replacement, queued toasts, expiry, hover pause and dismissal.",
            ),
            GalleryEntry::new(
                GalleryPage::Tooltip,
                "Tooltip",
                "Overlays",
                "Delayed pointer and keyboard-focus descriptions outside clipping.",
            ),
            GalleryEntry::new(
                GalleryPage::Sidebar,
                "Sidebar",
                "Navigation",
                "Retained adaptive sidebar, rail and drawer layouts, icons, badges and keyboard navigation.",
            ),
            GalleryEntry::new(
                GalleryPage::NavigationDrawer,
                "NavigationDrawer",
                "Navigation",
                "Modal destinations, fitted width, focus restoration, swipe dismissal and shared selection.",
            ),
            GalleryEntry::new(
                GalleryPage::NavigationRail,
                "NavigationRail",
                "Navigation",
                "Labeled and icon-only destinations, retained presentations, keyboard navigation and full accessible names.",
            ),
            GalleryEntry::new(
                GalleryPage::AppBar,
                "AppBar / NavigationBar",
                "Navigation",
                "Adaptive headings, back navigation, primary actions, overflow commands and host-owned safe areas.",
            ),
            GalleryEntry::new(
                GalleryPage::Table,
                "Table",
                "Data",
                "Virtualized rows, stable sorting, selection, fixed headers and resizable columns.",
            ),
            GalleryEntry::new(
                GalleryPage::DataTable,
                "DataTable",
                "Data",
                "Persistent large-data model, background sorting, frozen columns, multi-selection and cell editing.",
            ),
            GalleryEntry::new(
                GalleryPage::Calendar,
                "Calendar",
                "Inputs",
                "Calendar-aware month grids, disabled dates and keyboard day navigation.",
            ),
            GalleryEntry::new(
                GalleryPage::DatePicker,
                "DatePicker",
                "Inputs",
                "Optional date input with an anchored calendar and a clear action.",
            ),
            GalleryEntry::new(
                GalleryPage::DateRangePicker,
                "DateRangePicker",
                "Inputs",
                "Two-click range selection in a real calendar with endpoint and in-range highlighting.",
            ),
            GalleryEntry::new(
                GalleryPage::TimeField,
                "TimeField",
                "Inputs",
                "Validated time drafts, bounded values, seconds and keyboard stepping.",
            ),
            GalleryEntry::new(
                GalleryPage::TimePicker,
                "TimePicker",
                "Inputs",
                "Inline hour, minute, seconds and AM/PM wheels with range clamping and accessibility actions.",
            ),
            GalleryEntry::new(
                GalleryPage::WheelPicker,
                "WheelPicker",
                "Inputs",
                "Five-row wheel selection with drag, wheel, keyboard, disabled rows and accessibility value actions.",
            ),
            GalleryEntry::new(
                GalleryPage::Accordion,
                "Accordion",
                "Navigation",
                "Controlled single or multiple expansion with grouped keyboard focus.",
            ),
            GalleryEntry::new(
                GalleryPage::Breadcrumb,
                "Breadcrumb",
                "Navigation",
                "Destination paths with a collapsed intermediate-item menu.",
            ),
            GalleryEntry::new(
                GalleryPage::Pagination,
                "Pagination",
                "Navigation",
                "Bounded page navigation, boundary states and compact page windows.",
            ),
            GalleryEntry::new(
                GalleryPage::ButtonGroup,
                "ButtonGroup",
                "Controls",
                "Controlled segments with one Tab stop and disabled-item navigation.",
            ),
            GalleryEntry::new(
                GalleryPage::SplitButton,
                "SplitButton",
                "Controls",
                "Primary action plus an adjacent menu trigger with independent disabled behavior.",
            ),
            GalleryEntry::new(
                GalleryPage::Tag,
                "Tag",
                "Data",
                "Semantic tags with independent removal actions.",
            ),
            GalleryEntry::new(
                GalleryPage::KeyCap,
                "Kbd / KeyCap",
                "Data",
                "Platform-aware keyboard hints.",
            ),
            GalleryEntry::new(
                GalleryPage::Label,
                "Label",
                "Inputs",
                "Associated field labels, focus activation, required state and help.",
            ),
            GalleryEntry::new(
                GalleryPage::Dialog,
                "Dialog",
                "Overlays",
                "Structured headers, scrollable content and action slots.",
            ),
            GalleryEntry::new(
                GalleryPage::AlertDialog,
                "AlertDialog",
                "Overlays",
                "Confirmation and cancellation with loading and destructive roles.",
            ),
            GalleryEntry::new(
                GalleryPage::Sheet,
                "Sheet",
                "Overlays",
                "An edge panel using modal focus and dismissal policies.",
            ),
            GalleryEntry::new(
                GalleryPage::BottomSheet,
                "BottomSheet",
                "Overlays",
                "Detent heights, drag-to-expand/dismiss and a centered wide-surface fallback.",
            ),
            GalleryEntry::new(
                GalleryPage::CommandPalette,
                "Command Palette",
                "Overlays",
                "Ranked fuzzy search over commands with keyboard-first selection.",
            ),
            GalleryEntry::new(
                GalleryPage::NavigationStack,
                "NavigationStack",
                "Navigation",
                "A session-owned page stack with retained pages, push/pop motion and deep-link restore.",
            ),
            GalleryEntry::new(
                GalleryPage::DragAndDrop,
                "Drag & Drop",
                "Data",
                "Typed drag payloads, portal previews and highlighted drop targets.",
            ),
            GalleryEntry::new(
                GalleryPage::HoverCard,
                "HoverCard",
                "Overlays",
                "Delayed hover detail that survives the pointer moving onto the card.",
            ),
            GalleryEntry::new(
                GalleryPage::Timeline,
                "Timeline",
                "Data",
                "Ordered events on a rail with per-event outcome glyphs.",
            ),
            GalleryEntry::new(
                GalleryPage::Toolbar,
                "Toolbar",
                "Controls",
                "One height and spacing rhythm, grouped controls and separators.",
            ),
            GalleryEntry::new(
                GalleryPage::Link,
                "Link",
                "Controls",
                "In-flow navigation with contextual underlines, external hints and a link role.",
            ),
            GalleryEntry::new(
                GalleryPage::StatusBar,
                "StatusBar",
                "Layout",
                "A single-height window footer with leading and trailing context cells.",
            ),
            GalleryEntry::new(
                GalleryPage::Carousel,
                "Carousel",
                "Layout",
                "Paged content with drag, arrows, indicators and keyboard paging.",
            ),
            GalleryEntry::new(
                GalleryPage::Skeleton,
                "Skeleton",
                "Feedback",
                "Shaped loading placeholders, reduced motion and content transitions.",
            ),
            GalleryEntry::new(
                GalleryPage::Stepper,
                "Stepper",
                "Navigation",
                "Controlled workflow steps, independent outcomes, locked steps and keyboard navigation.",
            ),
            GalleryEntry::new(
                GalleryPage::SingleLineText,
                "Text · Truncation",
                "Controls",
                "Single-line fitting: measure, truncate and fold line breaks before layout.",
            ),
            GalleryEntry::new(
                GalleryPage::CompletionPopover,
                "CompletionPopover",
                "Controls",
                "Nonmodal completion list with a scrolling window that follows the highlight.",
            ),
            GalleryEntry::new(
                GalleryPage::RouteOutlet,
                "RouteOutlet",
                "Navigation",
                "A composition boundary that rebuilds its subtree for the selected route.",
            ),
            GalleryEntry::new(
                GalleryPage::ProgressValue,
                "ProgressValue",
                "Feedback",
                "The shared animated value behind the progress controls: repaint, never relayout.",
            ),
            GalleryEntry::new(
                GalleryPage::HorizontalSwipe,
                "HorizontalSwipe",
                "Controls",
                "A horizontal drag turns one page; taps inside still reach their button.",
            ),
            GalleryEntry::new(
                GalleryPage::EdgeAutoScroll,
                "EdgeAutoScroll",
                "Data",
                "Drag-while-scrolling near a viewport edge, with speed that grows quadratically.",
            ),
            GalleryEntry::new(
                GalleryPage::ScrollReveal,
                "ScrollReveal",
                "Layout",
                "The scroll delta that brings a row into view inside its scroll ancestor.",
            ),
            GalleryEntry::new(
                GalleryPage::ActionGroupLayout,
                "ActionGroupLayout",
                "Layout",
                "Action controls that stack as soon as their natural widths overflow.",
            ),
        ]
    }

    /// Categories in first-appearance order.
    pub fn categories() -> Vec<&'static str> {
        let mut result: Vec<&'static str> = Vec::new();
        for entry in Self::entries() {
            if !result.contains(&entry.category) {
                result.push(entry.category);
            }
        }
        result
    }

    /// Every entry whose title, page name, category or summary contains all
    /// whitespace-separated terms, case-insensitively.
    pub fn search(query: &str) -> Vec<GalleryEntry> {
        let terms: Vec<String> = query
            .split_whitespace()
            .map(|term| term.to_lowercase())
            .collect();
        Self::entries()
            .into_iter()
            .filter(|entry| {
                let text = format!(
                    "{} {} {} {}",
                    entry.title,
                    entry.page.raw_value(),
                    entry.category,
                    entry.summary
                )
                .to_lowercase();
                terms.iter().all(|term| text.contains(term.as_str()))
            })
            .collect()
    }

    /// Matches a page by raw value or title, case-insensitively.
    pub fn page(named: &str) -> Option<GalleryPage> {
        Self::entries()
            .into_iter()
            .find(|entry| {
                entry.page.raw_value().eq_ignore_ascii_case(named)
                    || entry.title.eq_ignore_ascii_case(named)
            })
            .map(|entry| entry.page)
    }
}

/// The control-size and enablement overrides the gallery toolbar owns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GalleryOptions {
    pub size: CowSize,
    pub is_enabled: bool,
}

/// Control density; mirrors Swift's `ControlSize`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CowSize {
    Mini,
    Small,
    #[default]
    Regular,
    Large,
}

impl CowSize {
    pub fn raw_value(self) -> &'static str {
        match self {
            Self::Mini => "mini",
            Self::Small => "small",
            Self::Regular => "regular",
            Self::Large => "large",
        }
    }
}

impl Default for GalleryOptions {
    fn default() -> Self {
        Self {
            size: CowSize::Regular,
            is_enabled: true,
        }
    }
}

impl GalleryOptions {
    pub fn new() -> Self {
        Self::default()
    }
}

/// A titled story group rendered as a hairline card.
#[derive(Clone, Debug)]
pub struct StorySection {
    pub title: String,
    pub detail: String,
}

impl StorySection {
    pub fn new(title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
        }
    }
}

#[cfg(test)]
mod checks {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn catalog_has_one_entry_per_page_in_swift_order() {
        let entries = GalleryCatalog::entries();
        // `GalleryPage` declares 106 cases and the catalog lists them all.
        assert_eq!(entries.len(), 106);
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.page)
                .collect::<BTreeSet<_>>()
                .len(),
            106,
            "every page appears exactly once"
        );
        assert_eq!(entries[0].page, GalleryPage::Introduction);
        assert_eq!(entries[0].title, "Dashboard");
        assert_eq!(entries[0].category, "Overview");
        assert_eq!(entries[97].page, GalleryPage::Stepper);
        assert_eq!(entries[97].title, "Stepper");
        // Raw values keep the Swift case spelling, including interior capitals.
        assert_eq!(GalleryPage::RangeSlider.raw_value(), "rangeSlider");
        assert_eq!(GalleryPage::ViewThatFits.raw_value(), "viewThatFits");
        assert_eq!(GalleryPage::DragAndDrop.raw_value(), "dragAndDrop");
    }

    #[test]
    fn every_page_resolves_to_its_own_entry() {
        for entry in GalleryCatalog::entries() {
            assert_eq!(entry.page.entry(), entry);
        }
    }

    #[test]
    fn categories_follow_first_appearance_order() {
        assert_eq!(
            GalleryCatalog::categories(),
            vec![
                "Overview",
                "Controls",
                "Inputs",
                "Data",
                "Overlays",
                "Navigation",
                "Layout",
                "Foundation",
                "Feedback"
            ]
        );
    }

    #[test]
    fn search_requires_every_term_case_insensitively() {
        assert_eq!(GalleryCatalog::search("command palette").len(), 1);
        assert_eq!(GalleryCatalog::search("COMMAND PALETTE").len(), 1);
        assert_eq!(
            GalleryCatalog::search("palette command").len(),
            1,
            "term order is irrelevant"
        );
        assert!(GalleryCatalog::search("zzzznotathing").is_empty());
        assert_eq!(
            GalleryCatalog::search("").len(),
            106,
            "an empty query matches everything"
        );
    }

    #[test]
    fn page_lookup_matches_raw_value_or_title() {
        assert_eq!(
            GalleryCatalog::page("introduction"),
            Some(GalleryPage::Introduction)
        );
        assert_eq!(
            GalleryCatalog::page("Introduction"),
            Some(GalleryPage::Introduction)
        );
        assert_eq!(
            GalleryCatalog::page("Dashboard"),
            Some(GalleryPage::Introduction)
        );
        assert_eq!(
            GalleryCatalog::page("rangeSLIDER"),
            Some(GalleryPage::RangeSlider)
        );
        assert_eq!(
            GalleryCatalog::page("TimePicker"),
            Some(GalleryPage::TimePicker)
        );
        assert_eq!(GalleryCatalog::page("nope"), None);
    }

    #[test]
    fn options_default_to_regular_and_enabled() {
        let options = GalleryOptions::default();
        assert_eq!(options.size, CowSize::Regular);
        assert!(options.is_enabled);
        assert_eq!(CowSize::Regular.raw_value(), "regular");
        assert_eq!(CowSize::Mini.raw_value(), "mini");
    }
}
