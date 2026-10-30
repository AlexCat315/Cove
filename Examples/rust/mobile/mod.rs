//! Mobile application content.
//!
//! Swift sources: `Examples/Mobile/Sources/AppContent/**`.

pub mod app;
pub mod model;
pub mod today;

pub use app::{make_application_content, MobileRoot, MobileTab};
pub use model::{MobileModel, Project, APPEARANCE_CHOICES};
pub use today::{ProjectCard, TodayScreen};
pub mod settings;
pub use settings::{sections, SettingsRow, SettingsScreen, SettingsSection};
