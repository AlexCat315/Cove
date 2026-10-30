//! Gallery stories, grouped the way the Swift sources group them.
//!
//! Swift sources: `GalleryContent/Stories/**`. Each submodule ports one
//! directory. `GalleryPage::make_story` in `super::routing` is the dispatch
//! table the shell drives; a page whose story needs a primitive that has not
//! been migrated yet is listed in `MIGRATION_PENDING` rather than stubbed.

pub mod animation;
pub mod animation_curves;
pub mod animation_system;
pub mod calendar;
pub mod action_sheet_story;
pub mod avatar_story;
pub mod command_story;
pub mod controls;
pub mod data_stories;
pub mod data_table_story;
pub mod feedback;
pub mod image_story;
pub mod intrinsic_fitting;
pub mod layout;
pub mod markdown_story;
pub mod media_stories;
pub mod sidebar_stories;
pub mod sidebar_story;
pub mod navigation_layout_stories;
pub mod navigation_stories;
pub mod overlay_stories;
pub mod overview;
pub mod placement_example;
pub mod product_stories;
pub mod rich_text;
pub mod stepper;
pub mod theme;
pub mod time_picker_story;
pub mod toolbar_story;
pub mod wheel_picker_story;
pub mod viewport_traits;

pub use animation::AnimationStory;
pub use animation_curves::{AnimationCurveExamples, CurveChoice};
pub use animation_system::{AnimationSystemExamples, DemoPhase};
pub use calendar::CalendarStory;
pub use action_sheet_story::ActionSheetStory;
pub use avatar_story::AvatarStory;
pub use command_story::CommandStory;
pub use controls::{ButtonStory, CheckboxStory, ToggleStory};
pub use data_stories::{AdaptiveTableSample, ChartExplorationStory};
pub use data_table_story::DataTableStory;
pub use image_story::ImageStory;
pub use intrinsic_fitting::{ViewThatFitsStory, FIT_CHIPS};
pub use markdown_story::MarkdownStory;
pub use media_stories::{RatingSample, SkeletonStory};
pub use sidebar_stories::{NestedSidebarExample, ResizableSidebarExample};
pub use sidebar_story::SidebarStory;
pub use navigation_layout_stories::{DrawerStory, RailStory};
pub use navigation_stories::AppBarStory;
pub use overlay_stories::{HoverCardStory, PopconfirmStory};
pub use layout::{LayoutGridStory, LazyGridStory, MasonrySample, MasonryStory};
pub use feedback::{AlertStory, BadgeStory, EmptyStory, ProgressStory, StorySection};
pub use placement_example::PlacementExample;
pub use product_stories::{
    ComboboxStory, FormStory, NotificationStory, RadioStory, TooltipStory,
};
pub use rich_text::RichTextStory;
pub use stepper::StepperStory;
pub use theme::ThemeStory;
pub use time_picker_story::TimePickerStory;
pub use toolbar_story::ToolbarStory;
pub use wheel_picker_story::WheelPickerStory;
pub use overview::{DashboardCard, IntroductionStory, MetricCard};
pub use viewport_traits::{ViewportDebugName, ViewportTraits, ViewportTraitsStory};
