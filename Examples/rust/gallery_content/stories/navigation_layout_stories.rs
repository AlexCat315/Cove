//! Layout stories: the modal navigation drawer and the navigation rail.
//!
//! Swift sources:
//! `GalleryContent/Stories/Layout/{DrawerStory,RailStory}.swift`.
//!
//! Both stories exercise `SidebarPresentation` and the shared item model, so the
//! measurable behaviour (rail width, label visibility, disabled-item skipping and
//! badge preservation) lives here as plain data the tests can assert against.

use cove_core::State;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::controls::Checkbox;
use cove_ui_compose::primitives::layout::sidebar::drawer::{
    NavigationDrawer, NavigationDrawerOptions,
};
use cove_ui_compose::primitives::layout::sidebar::presentation::SidebarPresentation;
use cove_ui_compose::primitives::layout::sidebar::{SidebarItem, SidebarSection};
use cove_ui_compose::primitives::media::ui_icon_catalog as icons;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View, ViewExt};

use super::feedback::StorySection;
use crate::gallery_content::catalog::GalleryOptions;

/// The number of drawer destinations the story generates.
pub const DRAWER_DESTINATION_COUNT: usize = 20;
/// The destination the story starts on.
pub const DRAWER_DEFAULT_SELECTION: usize = 8;
/// The destination that is present but unavailable.
pub const DRAWER_DISABLED_INDEX: usize = 4;
/// The destination that carries a badge.
pub const DRAWER_BADGED_INDEX: usize = 8;
/// The drawer always leaves this much of the window visible outside it.
pub const DRAWER_MINIMUM_OUTSIDE: f32 = 56.0;

/// `NavigationDrawerStory`: a modal drawer with 20 destinations.
pub struct DrawerStory {
    pub options: GalleryOptions,
    is_open: State<bool>,
    selection: State<Option<usize>>,
    closes_on_selection: State<bool>,
    allows_swipe: State<bool>,
}

impl DrawerStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            is_open: State::new(false),
            selection: State::new(Some(DRAWER_DEFAULT_SELECTION)),
            closes_on_selection: State::new(true),
            allows_swipe: State::new(true),
        }
    }

    /// A drawer that starts with nothing chosen, for the empty-state caption.
    pub fn new_without_selection(options: GalleryOptions) -> Self {
        let story = Self::new(options);
        story.selection.set(None);
        story
    }

    pub fn is_open(&self) -> bool {
        self.is_open.get()
    }

    pub fn open(&self) {
        self.is_open.set(true);
    }

    pub fn selection(&self) -> Option<usize> {
        self.selection.get()
    }

    /// Chooses a destination. Closing afterwards follows `closes_on_selection`,
    /// so the story can keep the drawer open to inspect it.
    pub fn choose(&self, destination: usize) {
        self.selection.set(Some(destination));
        if self.closes_on_selection.get() {
            self.is_open.set(false);
        }
    }

    pub fn closes_on_selection(&self) -> bool {
        self.closes_on_selection.get()
    }

    pub fn set_closes_on_selection(&self, value: bool) {
        self.closes_on_selection.set(value);
    }

    pub fn allows_swipe(&self) -> bool {
        self.allows_swipe.get()
    }

    pub fn set_allows_swipe(&self, value: bool) {
        self.allows_swipe.set(value);
    }

    /// The selection caption, which reads `none` when nothing is chosen.
    pub fn selection_caption(&self) -> String {
        format!(
            "Selected destination: {}",
            self.selection
                .get()
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_owned())
        )
    }

    pub fn sections(&self) -> Vec<SidebarSection<usize>> {
        let enabled = self.options.is_enabled;
        let items = (0..DRAWER_DESTINATION_COUNT)
            .map(|index| {
                let mut item = SidebarItem::new(
                    index,
                    if index == DRAWER_DISABLED_INDEX {
                        "Unavailable destination".to_owned()
                    } else {
                        format!("Destination {index}")
                    },
                )
                .icon(if index % 2 == 0 {
                    icons::file::folder()
                } else {
                    icons::layout::dashboard()
                })
                // The disabled destination stays listed but cannot be chosen.
                .enabled(enabled && index != DRAWER_DISABLED_INDEX);
                if index == DRAWER_BADGED_INDEX {
                    item = item.badge("8");
                }
                item
            })
            .collect();
        vec![SidebarSection::new("workspace", items).title("Workspace")]
    }

    pub fn drawer_options(&self) -> NavigationDrawerOptions {
        NavigationDrawerOptions {
            closes_on_selection: self.closes_on_selection.get(),
            allows_swipe_dismissal: self.allows_swipe.get(),
            ..NavigationDrawerOptions::default()
        }
    }

    /// Every selectable destination id, in order.
    pub fn selectable(&self) -> Vec<usize> {
        self.sections()
            .into_iter()
            .flat_map(|section| section.items)
            .filter(|item| item.is_enabled)
            .map(|item| item.id)
            .collect()
    }
}

impl View for DrawerStory {
    fn body(&self) -> AnyView {
        let enabled = self.options.is_enabled;
        let open_state = self.is_open.clone();
        let label = Text::new("Browse destinations without losing the current workspace.");

        let drawer = NavigationDrawer::new(
            "Cove workspace",
            self.is_open.binding(),
            self.selection.binding(),
            self.sections(),
        )
        .options(self.drawer_options())
        .header(label);

        let caption = Text::new(self.selection_caption())
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted());

        let controls = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::Wrap,
            16.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    cove_ui_compose::primitives::controls::button::Button::title(
                        "Open navigation",
                        move || open_state.set(true),
                    )
                    .enabled(enabled)
                    .debug_name("drawer-open"),
                ),
                AnyView::new(Checkbox::from_boolean(self.closes_on_selection.binding())),
                AnyView::new(
                    Text::new("Close after choosing")
                        .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption()),
                ),
                AnyView::new(Checkbox::from_boolean(self.allows_swipe.binding())),
                AnyView::new(
                    Text::new("Swipe to dismiss")
                        .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption()),
                ),
            ])),
        ));

        AnyView::new(StorySection::new(
            "Modal workspace navigation",
            "Open a leading drawer, then collapse it with the sidebar toggle or tap outside. Its width follows the window, leaving at least 56 points outside. Arrow keys browse; Enter chooses. Swipe left or press Escape to dismiss; focus returns to the opener.",
            AnyView::new(TupleView::new(vec![
                controls,
                AnyView::new(AnyView::new(caption).debug_name("drawer-selection")),
                AnyView::new(drawer),
            ])),
        ))
    }

    fn tag(&self) -> &'static str {
        "DrawerStory"
    }
}

/// The rail's five destinations, in authored order.
pub const RAIL_DESTINATIONS: [(&str, bool); 5] = [
    ("Overview", true),
    ("Projects", true),
    ("Notifications", true),
    ("Unavailable", false),
    ("Workspace permissions", true),
];

/// `NavigationRailStory`: the narrow rail and its label toggle.
pub struct RailStory {
    pub options: GalleryOptions,
    selection: State<Option<usize>>,
    shows_labels: State<bool>,
}

impl RailStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            selection: State::new(Some(0)),
            shows_labels: State::new(true),
        }
    }

    pub fn selection(&self) -> Option<usize> {
        self.selection.get()
    }

    pub fn shows_labels(&self) -> bool {
        self.shows_labels.get()
    }

    pub fn set_shows_labels(&self, value: bool) {
        self.shows_labels.set(value);
    }

    /// The presentation the toggle currently asks for.
    pub fn presentation(&self) -> SidebarPresentation {
        SidebarPresentation::Rail {
            shows_labels: self.shows_labels.get(),
        }
    }

    /// The rail's width in points: 72 with labels, 56 without.
    pub fn rail_width(&self) -> f32 {
        self.presentation().rail_width().unwrap_or(0.0)
    }

    pub fn items(&self) -> Vec<SidebarItem<usize>> {
        let enabled = self.options.is_enabled;
        RAIL_DESTINATIONS
            .iter()
            .enumerate()
            .map(|(index, (title, is_available))| {
                let mut item = SidebarItem::new(index, *title)
                    .icon(match index {
                        0 => icons::layout::dashboard(),
                        1 => icons::file::folder(),
                        2 => icons::status::bell(),
                        3 => icons::status::lock(),
                        _ => icons::file::file(),
                    })
                    .enabled(enabled && *is_available);
                // A long badge keeps its full value rather than truncating to fit.
                item = match index {
                    1 => item.badge("12"),
                    2 => item.badge("999"),
                    _ => item,
                };
                item
            })
            .collect()
    }

    /// Ids the arrow keys can reach: disabled destinations are skipped.
    pub fn navigable(&self) -> Vec<usize> {
        self.items()
            .into_iter()
            .filter(|item| item.is_enabled)
            .map(|item| item.id)
            .collect()
    }
}

impl View for RailStory {
    fn body(&self) -> AnyView {
        let labels = Text::new("Show labels")
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption());
        let rows = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::NoWrap,
            8.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    Checkbox::from_boolean(self.shows_labels.binding())
                        .debug_name("rail-show-labels"),
                ),
                AnyView::new(labels),
            ])),
        ));

        AnyView::new(StorySection::new(
            "Rail destinations",
            "The 72-point rail keeps labels; hiding them uses 56 points and retains complete names in tooltips and accessibility. Use Up / Down and Home / End. Disabled destinations are skipped; badges keep their full value.",
            rows,
        ))
    }

    fn tag(&self) -> &'static str {
        "RailStory"
    }
}
