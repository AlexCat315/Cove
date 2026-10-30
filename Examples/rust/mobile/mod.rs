//! Mobile application content.
//!
//! Swift sources: `Examples/Mobile/Sources/AppContent/**`.

pub mod model;
pub mod today;

pub use model::{MobileModel, Project, APPEARANCE_CHOICES};
pub use today::{ProjectCard, TodayScreen};
