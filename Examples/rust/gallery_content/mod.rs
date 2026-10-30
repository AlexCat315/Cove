//! Gallery content: the page catalog, navigation icons, story routing and stories.
//!
//! Swift sources: `GalleryContent/GalleryCatalog.swift`,
//! `GalleryContent/NavigationIcons.swift`, `GalleryContent/CounterContent.swift`,
//! `GalleryContent/Stories/**`.

pub mod catalog;
pub mod counter_content;
pub mod icons;
pub mod routing;
pub mod stories;

pub use catalog::{CowSize, GalleryCatalog, GalleryEntry, GalleryOptions, GalleryPage, StorySection};
pub use counter_content::CounterContent;
pub use routing::{MigrationPending, StoryDispatch};
