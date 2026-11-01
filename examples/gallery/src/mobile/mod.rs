//! Mobile application content.
//!
//! Swift sources: `examples/mobile/Sources/AppContent/**`.

pub mod app;
pub mod model;
pub mod today;

pub use app::{MobileRoot, MobileTab, make_application_content};
pub use model::{APPEARANCE_CHOICES, MobileModel, Project};
pub use today::{ProjectCard, TodayScreen};
pub mod settings;
pub use settings::{SettingsRow, SettingsScreen, SettingsSection, sections};
