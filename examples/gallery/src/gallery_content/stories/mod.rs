//! Gallery stories, grouped the way the Swift sources group them.
//!
//! Swift sources: `GalleryContent/Stories/**`. Each submodule ports one
//! directory. `GalleryPage::make_story` in `super::routing` is the dispatch
//! table the shell drives; a page whose story needs a primitive that has not
//! been migrated yet is listed in `MIGRATION_PENDING` rather than stubbed.

pub mod action_sheet_story;
pub mod adaptive_navigation_story;
pub mod animation;
pub mod animation_curves;
pub mod animation_system;
pub mod avatar_story;
pub mod calendar;
pub mod command_story;
pub mod container_stories;
pub mod controls;
pub mod data_list_stories;
pub mod data_stories;
pub mod data_stories_file;
pub mod data_table_story;
pub mod drag_upload_story;
pub mod feedback;
pub mod field_stories;
pub mod image_story;
pub mod interaction_gap_stories;
pub mod intrinsic_fitting;
pub mod layout;
pub mod markdown_story;
pub mod media_player_story;
pub mod media_stories;
pub mod navigation_layout_stories;
pub mod navigation_stories;
pub mod navigation_stories_file;
pub mod overlay_stories;
pub mod overlay_stories_file;
pub mod overview;
pub mod picker_stories;
pub mod placement_example;
pub mod primitive_gap_stories;
pub mod product_stories;
pub mod rich_text;
pub mod sidebar_stories;
pub mod sidebar_story;
pub mod stepper;
pub mod table_story;
pub mod text_editor_story;
pub mod theme;
pub mod time_picker_story;
pub mod toolbar_story;
pub mod viewport_traits;
pub mod web_story;
pub mod wheel_picker_story;
pub mod workspace_story;

pub use action_sheet_story::ActionSheetStory;
pub use adaptive_navigation_story::AdaptiveNavigationStory;
pub use animation::AnimationStory;
pub use animation_curves::{AnimationCurveExamples, CurveChoice};
pub use animation_system::{AnimationSystemExamples, DemoPhase};
pub use avatar_story::AvatarStory;
pub use calendar::CalendarStory;
pub use command_story::CommandStory;
pub use container_stories::{
    BoundedScrollStory, CarouselStory, PanelStory, ScrollViewStory, SplitViewStory, StatusBarStory,
    ZStackStory,
};
pub use controls::{ButtonStory, CheckboxStory, ToggleStory};
pub use data_list_stories::{
    ListStory, PropertyGridStory, ReorderableStory, ScrollbarStory, TimelineStory,
};
pub use data_stories::{AdaptiveTableSample, ChartExplorationStory, ChartStory};
pub use data_stories_file::{
    DragAndDropStory, RefreshListStory, SectionedListStory, SwipeListStory, TreeStory,
    VirtualStackStory,
};
pub use data_table_story::DataTableStory;
pub use drag_upload_story::DragUploadStory;
pub use feedback::{AlertStory, BadgeStory, EmptyStory, ProgressStory, StorySection};
pub use field_stories::{
    AssetRefStory, ColorFieldStory, JsonFieldStory, NumberFieldStory, NumericStory,
    RangeSliderStory, SearchFieldStory, SelectStory, SliderStory, TextAreaStory, TextFieldStory,
    TextFieldsStory, Vec3FieldStory,
};
pub use image_story::ImageStory;
pub use interaction_gap_stories::{
    ActionGroupLayoutStory, EdgeAutoScrollStory, HorizontalSwipeStory, ScrollRevealStory,
};
pub use intrinsic_fitting::{FIT_CHIPS, ViewThatFitsStory};
pub use layout::{LayoutGridStory, LazyGridStory, MasonrySample, MasonryStory};
pub use markdown_story::MarkdownStory;
pub use media_player_story::MediaPlayerStory;
pub use media_stories::{RatingSample, RatingStory, SkeletonStory};
pub use navigation_layout_stories::{DrawerStory, RailStory};
pub use navigation_stories::AppBarStory;
pub use navigation_stories_file::{
    AccordionStory, AlertDialogStory, BottomSheetStory, BreadcrumbStory, ButtonGroupStory,
    DialogStory, LabelStory, NavigationStackStory, PaginationStory, SheetStory, TagStory,
};
pub use overlay_stories::{ContextMenuStory, HoverCardStory, PopconfirmStory};
pub use overlay_stories_file::{DisclosureStory, MenuStory, ModalStory, PopoverStory, TabsStory};
pub use overview::{DashboardCard, IntroductionStory, MetricCard};
pub use picker_stories::{
    DatePickerStory, DateRangeStory, IconStory, KeyCapStory, LinkStory, MenuButtonStory,
    SplitButtonStory, TimeFieldStory,
};
pub use placement_example::PlacementExample;
pub use primitive_gap_stories::{
    CompletionPopoverStory, DemoRoute, ProgressValueStory, RouteOutletStory, SingleLineTextStory,
};
pub use product_stories::{ComboboxStory, FormStory, NotificationStory, RadioStory, TooltipStory};
pub use rich_text::TextStory;
pub use sidebar_stories::{NestedSidebarExample, ResizableSidebarExample};
pub use sidebar_story::SidebarStory;
pub use stepper::StepperStory;
pub use table_story::{InventoryRecord, TABLE_RECORDS, TableStory};
pub use text_editor_story::TextEditorStory;
pub use theme::ThemeStory;
pub use time_picker_story::TimePickerStory;
pub use toolbar_story::ToolbarStory;
pub use viewport_traits::{ViewportDebugName, ViewportTraits, ViewportTraitsStory};
pub use web_story::WebStory;
pub use wheel_picker_story::WheelPickerStory;

mod showcase;
pub use workspace_story::WorkspaceStory;
