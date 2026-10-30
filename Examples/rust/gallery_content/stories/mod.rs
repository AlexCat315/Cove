//! Gallery stories, grouped the way the Swift sources group them.
//!
//! Swift sources: `GalleryContent/Stories/**`. Each submodule ports one
//! directory. `GalleryPage::make_story` in `super::routing` is the dispatch
//! table the shell drives; a page whose story needs a primitive that has not
//! been migrated yet is listed in `MIGRATION_PENDING` rather than stubbed.

pub mod animation;
pub mod animation_curves;
pub mod animation_system;
pub mod controls;
pub mod feedback;
pub mod layout;
pub mod overview;
pub mod rich_text;
pub mod stepper;
pub mod viewport_traits;

pub use animation::AnimationStory;
pub use animation_curves::{AnimationCurveExamples, CurveChoice};
pub use animation_system::{AnimationSystemExamples, DemoPhase};
pub use controls::{ButtonStory, CheckboxStory, ToggleStory};
pub use layout::{LayoutGridStory, LazyGridStory, MasonrySample, MasonryStory};
pub use feedback::{AlertStory, BadgeStory, EmptyStory, ProgressStory, StorySection};
pub use rich_text::RichTextStory;
pub use stepper::StepperStory;
pub use overview::{DashboardCard, IntroductionStory, MetricCard};
pub use viewport_traits::{ViewportDebugName, ViewportTraits, ViewportTraitsStory};
