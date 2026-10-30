//! Layout story: the workspace sidebar.
//!
//! Swift source: `GalleryContent/Stories/Layout/SidebarStory.swift`.
//!
//! Two sections, one disabled destination, a row with inline slots (leading dot,
//! trailing count, interactive pin) and a detail pane that follows the selection.
//! The story also embeds the resizable and nested examples directly.

use cove_core::State;
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::layout::sidebar::drawer::NavigationDrawer;
use cove_ui_compose::primitives::layout::sidebar::{SidebarItem, SidebarSection};
use cove_ui_compose::primitives::media::ui_icon_catalog as icons;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View, ViewExt};

use super::feedback::StorySection;
use super::sidebar_stories::{NestedSidebarExample, ResizableSidebarExample};
use crate::gallery_content::catalog::GalleryOptions;

/// The section ids, in authored order.
pub const SECTION_IDS: [&str; 2] = ["workspace", "preferences"];
/// The destination the story starts on.
pub const DEFAULT_SELECTION: &str = "downloads";
/// The destination that cannot be activated.
pub const DISABLED_DESTINATION: &str = "permissions";
/// The width of the side-by-side preview.
pub const PREVIEW_WIDTH: f32 = 280.0;
pub const PREVIEW_HEIGHT: f32 = 360.0;
/// The accessory lane width for the slotted row.
pub const ACCESSORY_WIDTH: f32 = 36.0;

/// What the detail pane shows for a destination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DestinationDetail {
    pub title: &'static str,
    pub icon: DestinationIcon,
}

/// The icon set a destination can use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DestinationIcon {
    Dashboard,
    Download,
    Folder,
    Activity,
    Bell,
    Settings,
}

impl DestinationIcon {
    /// The slug resolved for `id`, matching the story's switch.
    pub fn slug(self) -> &'static str {
        match self {
            Self::Dashboard => "dashboard",
            Self::Download => "download",
            Self::Folder => "folder",
            Self::Activity => "activity",
            Self::Bell => "bell",
            Self::Settings => "settings",
        }
    }
}

/// The detail pane for `selection`.
///
/// An unknown or absent selection falls back to the prompt rather than to an
/// arbitrary destination, so the pane never claims a row is selected.
pub fn destination_detail(selection: Option<&str>) -> DestinationDetail {
    match selection {
        Some("overview") => DestinationDetail { title: "Overview", icon: DestinationIcon::Dashboard },
        Some("downloads") => DestinationDetail { title: "Downloads", icon: DestinationIcon::Download },
        Some("projects") => DestinationDetail { title: "Projects", icon: DestinationIcon::Folder },
        Some("recent") => {
            DestinationDetail { title: "Recent activity", icon: DestinationIcon::Activity }
        }
        Some("notifications") => {
            DestinationDetail { title: "Notifications", icon: DestinationIcon::Bell }
        }
        Some("settings") => DestinationDetail { title: "Settings", icon: DestinationIcon::Settings },
        _ => DestinationDetail { title: "Choose a destination", icon: DestinationIcon::Settings },
    }
}

/// `SidebarStory`: the workspace sidebar plus the two embedded examples.
pub struct SidebarStory {
    pub options: GalleryOptions,
    selection: State<Option<String>>,
    shows_icons: State<bool>,
    shows_badges: State<bool>,
    is_drawer_open: State<bool>,
    is_pinned: State<bool>,
}

impl SidebarStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            selection: State::new(Some(DEFAULT_SELECTION.to_owned())),
            shows_icons: State::new(true),
            shows_badges: State::new(true),
            is_drawer_open: State::new(false),
            is_pinned: State::new(false),
        }
    }

    pub fn selection(&self) -> Option<String> {
        self.selection.get()
    }

    /// Selecting a destination updates the detail pane.
    ///
    /// A disabled destination is refused, matching the sidebar's own behaviour.
    pub fn select(&self, id: &str) -> bool {
        if id == DISABLED_DESTINATION {
            return false;
        }
        if !self.all_ids().iter().any(|known| known == id) {
            return false;
        }
        self.selection.set(Some(id.to_owned()));
        true
    }

    pub fn detail(&self) -> DestinationDetail {
        destination_detail(self.selection().as_deref())
    }

    pub fn shows_icons(&self) -> bool {
        self.shows_icons.get()
    }

    pub fn set_shows_icons(&self, value: bool) {
        self.shows_icons.set(value);
    }

    pub fn shows_badges(&self) -> bool {
        self.shows_badges.get()
    }

    pub fn set_shows_badges(&self, value: bool) {
        self.shows_badges.set(value);
    }

    pub fn is_pinned(&self) -> bool {
        self.is_pinned.get()
    }

    pub fn toggle_pinned(&self) {
        self.is_pinned.update(|value| *value = !*value);
    }

    pub fn is_drawer_open(&self) -> bool {
        self.is_drawer_open.get()
    }

    pub fn open_drawer(&self) {
        self.is_drawer_open.set(true);
    }

    fn all_ids(&self) -> Vec<String> {
        self.sections()
            .into_iter()
            .flat_map(|section| section.items)
            .map(|item| item.id)
            .collect()
    }

    /// Every destination id the sidebar lists, including the disabled one.
    pub fn destination_ids(&self) -> Vec<String> {
        self.all_ids()
    }

    /// Ids the arrow keys can reach.
    pub fn navigable(&self) -> Vec<String> {
        self.sections()
            .into_iter()
            .flat_map(|section| section.items)
            .filter(|item| item.is_enabled)
            .map(|item| item.id)
            .collect()
    }

    pub fn sections(&self) -> Vec<SidebarSection<String>> {
        let enabled = self.options.is_enabled;
        let icons_on = self.shows_icons.get();
        let badges_on = self.shows_badges.get();
        let mut item = |id: &str, title: &str, icon: Option<cove_imaging::BundleImageResource>, badge: Option<&str>, is_enabled: bool| {
            let mut row = SidebarItem::new(id.to_owned(), title.to_owned())
                .enabled(enabled && is_enabled);
            if icons_on && let Some(icon) = icon {
                row = row.icon(icon);
            }
            if badges_on && let Some(badge) = badge {
                row = row.badge(badge);
            }
            row
        };

        let recent = {
            // Inline slots: presence dot, muted count, interactive pin outside the
            // destination button.
            let mut row = item("recent", "Recent activity", None, None, true);
            row.slots.trailing = Some(AnyView::new(
                Text::new("12").semantic_foreground(SemanticColorRef::on_surface_muted()),
            ));
            row.slots.accessory_width = ACCESSORY_WIDTH;
            row
        };

        vec![
            SidebarSection::new(
                "workspace",
                vec![
                    item("overview", "Overview", Some(icons::layout::dashboard()), None, true),
                    item("downloads", "Downloads", Some(icons::action::download()), Some("8"), true),
                    item("projects", "Projects", Some(icons::file::folder()), None, true),
                    recent,
                ],
            )
            .title("Workspace"),
            SidebarSection::new(
                "preferences",
                vec![
                    item("notifications", "Notifications", Some(icons::status::bell()), Some("3"), true),
                    item("settings", "Settings", Some(icons::action::settings()), None, true),
                    item(
                        "permissions",
                        "Team permissions unavailable",
                        Some(icons::status::lock()),
                        None,
                        false,
                    ),
                ],
            )
            .title("Preferences"),
        ]
    }
}

impl View for SidebarStory {
    fn body(&self) -> AnyView {
        let detail = self.detail();
        let caption = Text::new(
            "Selection belongs to the application. This content updates when a destination is clicked or reached with the keyboard.",
        )
        .semantic_foreground(SemanticColorRef::on_surface_muted());

        let pane = AnyView::new(BoxView::new(
            cove_core::scene::FlexDirection::Column,
            cove_core::scene::Align::Stretch,
            cove_core::scene::Justify::FlexStart,
            cove_core::scene::Wrap::NoWrap,
            12.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    AnyView::new(
                        Text::new(detail.title)
                            .semantic_font(cove_ui_compose::theme::SemanticFontRef::headline()),
                    )
                    .debug_name("sidebar-destination"),
                ),
                AnyView::new(caption),
            ])),
        ))
        .frame_width(PREVIEW_WIDTH)
        .frame_height(PREVIEW_HEIGHT);

        let drawer = NavigationDrawer::new(
            "Cove workspace",
            self.is_drawer_open.binding(),
            self.selection.binding(),
            self.sections(),
        );

        AnyView::new(TupleView::new(vec![
            AnyView::new(StorySection::new(
                "Workspace navigation",
                "Native SidebarItem icons follow IconTheme and Scale. Up / Down and Home / End select enabled destinations and reveal them inside the scrolling pane.",
                AnyView::new(TupleView::new(vec![
                    AnyView::new(pane),
                    AnyView::new(drawer),
                ])),
            )),
            AnyView::new(ResizableSidebarExample::new(self.options.clone())),
            AnyView::new(NestedSidebarExample::new(self.options.clone())),
        ]))
    }

    fn tag(&self) -> &'static str {
        "SidebarStory"
    }
}
