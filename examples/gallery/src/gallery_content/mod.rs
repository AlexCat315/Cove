//! Gallery content: the page catalog, navigation icons, story routing and stories.
//!
//! Swift sources: `GalleryContent/GalleryCatalog.swift`,
//! `GalleryContent/NavigationIcons.swift`, `GalleryContent/CounterContent.swift`,
//! `GalleryContent/GalleryView.swift`, `GalleryContent/Stories/**`.

pub mod catalog;
pub mod counter_content;
pub mod gallery_view;
pub mod icons;
pub mod routing;
pub mod stories;

pub use catalog::{
    CowSize, GalleryCatalog, GalleryEntry, GalleryOptions, GalleryPage, StorySection,
};
pub use counter_content::CounterContent;
pub use gallery_view::{GalleryView, current_entry, size_choices};
pub use routing::{StoryDispatch, StorySource};
