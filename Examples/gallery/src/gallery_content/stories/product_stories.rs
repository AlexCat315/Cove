//! Layout story file: radio, form, combobox, notifications and tooltip.
//!
//! Swift source: `GalleryContent/Stories/Layout/ProductStories.swift`, which holds
//! five independent stories (`RadioStory`, `FormStory`, `ComboboxStory`,
//! `NotificationStory`, `TooltipStory`).
//!
//! The behaviour worth pinning here is validation and the simulated remote
//! directory: the email rule, the required-name rule, the airport matcher and its
//! deliberate failure for one query.

use cove_core::State;
use cove_ui_compose::LayoutDebugExt;
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::controls::form_validation::{
    FormController, FormRule, FormSubmissionState,
};
use cove_ui_compose::primitives::overlays::toast::controller::ToastController;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, View, ViewExt};

use super::feedback::StorySection;
use crate::gallery_content::catalog::GalleryOptions;

/// The performance preference the radio story starts on.
pub const DEFAULT_MODE: &str = "balanced";
/// The simulated server latency, in milliseconds.
pub const AIRPORT_LATENCY_MS: u64 = 600;
/// The query that makes the simulated service fail.
pub const OFFLINE_QUERY: &str = "offline";
/// The toast queue's visible cap.
pub const MAX_VISIBLE_TOASTS: usize = 3;

/// One radio option with an optional detail line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadioChoice {
    pub id: &'static str,
    pub title: &'static str,
    pub detail: Option<&'static str>,
    pub is_enabled: bool,
}

/// The radio story's four options, in authored order.
pub const RADIO_CHOICES: [RadioChoice; 4] = [
    RadioChoice {
        id: "fast",
        title: "Fast",
        detail: Some("Prefer lower latency"),
        is_enabled: true,
    },
    RadioChoice {
        id: "balanced",
        title: "Balanced",
        detail: Some("Balance quality and speed"),
        is_enabled: true,
    },
    RadioChoice {
        id: "offline",
        title: "Unavailable",
        detail: None,
        is_enabled: false,
    },
    RadioChoice {
        id: "quality",
        title: "High quality",
        detail: None,
        is_enabled: true,
    },
];

/// Whether `value` looks like an email address the form accepts.
///
/// The authored rule requires both `@` and `.`, which is deliberately loose: it
/// only needs to reject obvious mistakes for a gallery story.
pub fn is_valid_email(value: &str) -> bool {
    value.contains('@') && value.contains('.')
}

/// The email rule's message, or `None` when the value passes.
pub fn validate_email(value: &str) -> Option<&'static str> {
    if is_valid_email(value) {
        None
    } else {
        Some("Enter a valid email address.")
    }
}

/// One airport the simulated directory knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Airport {
    pub code: &'static str,
    pub name: &'static str,
}

/// The directory's twelve airports.
pub const AIRPORTS: [Airport; 12] = [
    Airport {
        code: "PEK",
        name: "Beijing Capital",
    },
    Airport {
        code: "PVG",
        name: "Shanghai Pudong",
    },
    Airport {
        code: "HKG",
        name: "Hong Kong",
    },
    Airport {
        code: "LHR",
        name: "London Heathrow",
    },
    Airport {
        code: "CDG",
        name: "Paris Charles de Gaulle",
    },
    Airport {
        code: "FRA",
        name: "Frankfurt",
    },
    Airport {
        code: "BER",
        name: "Berlin Brandenburg",
    },
    Airport {
        code: "NRT",
        name: "Tokyo Narita",
    },
    Airport {
        code: "HND",
        name: "Tokyo Haneda",
    },
    Airport {
        code: "SFO",
        name: "San Francisco",
    },
    Airport {
        code: "JFK",
        name: "New York JFK",
    },
    Airport {
        code: "SIN",
        name: "Singapore Changi",
    },
];

/// The label the combobox shows for an airport.
pub fn airport_label(airport: Airport) -> String {
    format!("{} · {}", airport.code, airport.name)
}

/// The simulated search: matches the code or the name, case-insensitively.
///
/// Returns `Err` for the one query the story uses to demonstrate failure
/// recovery, so the failure path is reachable without a real network.
pub fn search_airports(query: &str) -> Result<Vec<Airport>, &'static str> {
    let lowered = query.to_lowercase();
    if lowered.contains(OFFLINE_QUERY) {
        return Err("The airport service is unreachable.");
    }
    if query.is_empty() {
        return Ok(AIRPORTS.to_vec());
    }
    Ok(AIRPORTS
        .into_iter()
        .filter(|airport| {
            airport.code.to_lowercase().contains(&lowered)
                || airport.name.to_lowercase().contains(&lowered)
        })
        .collect())
}

/// The cities the local combobox offers.
pub const CITY_OPTIONS: [(&str, &str, bool); 5] = [
    ("beijing", "北京 · Beijing", true),
    (
        "shanghai",
        "上海 · Shanghai · International collaboration workspace 👩🏽‍💻",
        true,
    ),
    ("hongkong", "香港 · Hong Kong", true),
    ("london", "London", true),
    ("unavailable", "Unavailable", false),
];

/// Which of the five product stories a request refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProductStory {
    Radio,
    Form,
    Combobox,
    Notification,
    Tooltip,
}

impl ProductStory {
    /// A stable debug name for each story's root.
    pub fn debug_name(self) -> &'static str {
        match self {
            Self::Radio => "product-radio",
            Self::Form => "product-form",
            Self::Combobox => "product-combobox",
            Self::Notification => "product-notification",
            Self::Tooltip => "product-tooltip",
        }
    }
}

/// `RadioStory`: four options, one disabled.
pub struct RadioStory {
    pub options: GalleryOptions,
    selection: State<Option<String>>,
}

impl RadioStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            selection: State::new(Some(DEFAULT_MODE.to_owned())),
        }
    }

    pub fn selection(&self) -> Option<String> {
        self.selection.get()
    }

    /// Selects an option unless it is disabled.
    pub fn select(&self, id: &str) -> bool {
        match RADIO_CHOICES.iter().find(|choice| choice.id == id) {
            Some(choice) if choice.is_enabled => {
                self.selection.set(Some(id.to_owned()));
                true
            }
            _ => false,
        }
    }

    /// The caption under the group.
    pub fn caption(&self) -> String {
        format!(
            "Selected: {}",
            self.selection.get().unwrap_or_else(|| "None".to_owned())
        )
    }

    /// Ids the arrow keys can reach.
    pub fn navigable(&self) -> Vec<&'static str> {
        RADIO_CHOICES
            .iter()
            .filter(|choice| choice.is_enabled)
            .map(|choice| choice.id)
            .collect()
    }
}

impl View for RadioStory {
    fn body(&self) -> AnyView {
        let caption = Text::new(self.caption())
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted());
        AnyView::new(StorySection::new(
            "Performance preference",
            "Tab enters once. Arrows choose an enabled option and move focus; the disabled option is skipped.",
            AnyView::new(AnyView::new(caption).debug_name("product-radio-caption")),
        ))
    }

    fn tag(&self) -> &'static str {
        "RadioStory"
    }
}

/// `FormStory`: a required name, a required email, and a submit that surfaces
/// field errors.
pub struct FormStory {
    pub options: GalleryOptions,
    name: State<String>,
    email: State<String>,
    controller: FormController,
    status: State<String>,
}

impl FormStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            name: State::new(String::new()),
            email: State::new(String::new()),
            controller: FormController::new(),
            status: State::new("Complete the required fields".to_owned()),
        }
    }

    pub fn name(&self) -> String {
        self.name.get()
    }

    pub fn set_name(&self, value: impl Into<String>) {
        self.name.set(value.into());
    }

    pub fn email(&self) -> String {
        self.email.get()
    }

    pub fn set_email(&self, value: impl Into<String>) {
        self.email.set(value.into());
    }

    pub fn status(&self) -> String {
        self.status.get()
    }

    pub fn submission(&self) -> FormSubmissionState {
        self.controller.submission()
    }

    pub fn error(&self, field_id: &str) -> Option<String> {
        self.controller.error(field_id)
    }

    /// The rules, reading the live field values.
    pub fn rules(&self) -> Vec<FormRule> {
        let name = self.name.clone();
        let email = self.email.clone();
        vec![
            FormRule::required("name", "Name", move || name.get()),
            FormRule::new("email", "Email", move || {
                validate_email(&email.get()).map(str::to_owned)
            }),
        ]
    }

    /// Validates and records the outcome in the status line.
    pub fn submit(&self) -> bool {
        let ok = self.controller.validate(&self.rules());
        if ok {
            self.status.set("Account created".to_owned());
        } else {
            self.status.set("Complete the required fields".to_owned());
        }
        ok
    }
}

impl View for FormStory {
    fn body(&self) -> AnyView {
        AnyView::new(StorySection::new(
            "Create an account",
            "Labels move above their inputs when space is limited. Resize or change text scale while editing; drafts and focus stay intact. Submit to see field errors.",
            AnyView::new(
                Text::new(self.status())
                    .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
            ),
        ))
    }

    fn tag(&self) -> &'static str {
        "FormStory"
    }
}

/// `ComboboxStory`: a local list plus a simulated remote directory.
pub struct ComboboxStory {
    pub options: GalleryOptions,
    city: State<Option<String>>,
    airport: State<Option<String>>,
}

impl ComboboxStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            city: State::new(Some("shanghai".to_owned())),
            airport: State::new(None),
        }
    }

    pub fn city(&self) -> Option<String> {
        self.city.get()
    }

    pub fn airport(&self) -> Option<String> {
        self.airport.get()
    }

    /// Chooses a city unless the option is unavailable.
    pub fn select_city(&self, id: &str) -> bool {
        match CITY_OPTIONS.iter().find(|(value, _, _)| *value == id) {
            Some((value, _, true)) => {
                self.city.set(Some((*value).to_owned()));
                true
            }
            _ => false,
        }
    }

    /// Clears the city selection.
    pub fn clear_city(&self) {
        self.city.set(None);
    }

    pub fn select_airport(&self, code: impl Into<String>) {
        self.airport.set(Some(code.into()));
    }

    pub fn caption(&self) -> String {
        format!(
            "Selected: {}",
            self.city.get().unwrap_or_else(|| "None".to_owned())
        )
    }
}

impl View for ComboboxStory {
    fn body(&self) -> AnyView {
        let caption = Text::new(self.caption())
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted());
        AnyView::new(StorySection::new(
            "Searchable selection",
            "Search in English or Chinese, use Up/Down and Return, or clear the selection. Unavailable options cannot be chosen.",
            AnyView::new(AnyView::new(caption).debug_name("combobox-local-caption")),
        ))
    }

    fn tag(&self) -> &'static str {
        "ComboboxStory"
    }
}

/// `NotificationStory`: a bounded toast queue.
pub struct NotificationStory {
    controller: std::rc::Rc<ToastController>,
    count: State<u64>,
}

impl NotificationStory {
    pub fn new() -> Self {
        Self {
            controller: ToastController::new(MAX_VISIBLE_TOASTS),
            count: State::new(0),
        }
    }

    pub fn controller(&self) -> std::rc::Rc<ToastController> {
        self.controller.clone()
    }

    pub fn posted(&self) -> u64 {
        self.count.get()
    }

    /// The next notification number, which the titles embed.
    pub fn next_count(&self) -> u64 {
        self.count.get() + 1
    }

    pub fn bump(&self) {
        self.count.update(|value| *value += 1);
    }

    pub fn visible(&self) -> usize {
        self.controller.notifications().len()
    }

    pub fn queued(&self) -> usize {
        self.controller.queued_count()
    }

    pub fn clear(&self) {
        self.controller.clear();
    }

    pub fn caption(&self) -> String {
        format!("Visible: {} · Queued: {}", self.visible(), self.queued())
    }
}

impl Default for NotificationStory {
    fn default() -> Self {
        Self::new()
    }
}

impl View for NotificationStory {
    fn body(&self) -> AnyView {
        let caption = Text::new(self.caption())
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted());
        AnyView::new(StorySection::new(
            "Toast queue",
            "Notifications appear at the window edge. Hover pauses expiry. Add several to see the bounded queue, replace the same stable ID, or use an inline action. Automatic positions the banner at the top trailing edge at every width.",
            AnyView::new(AnyView::new(caption).debug_name("toast-caption")),
        ))
    }

    fn tag(&self) -> &'static str {
        "NotificationStory"
    }
}

/// `TooltipStory`: the shared tooltip surface.
pub struct TooltipStory {
    pub options: GalleryOptions,
}

impl TooltipStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self { options }
    }
}

impl View for TooltipStory {
    fn body(&self) -> AnyView {
        AnyView::new(StorySection::new(
            "Tooltips",
            "Hover or focus a target to reveal its label. Long labels wrap inside the bubble.",
            AnyView::new(
                Text::new("Hover a target")
                    .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
            ),
        ))
    }

    fn tag(&self) -> &'static str {
        "TooltipStory"
    }
}

/// Keeps the five story names stable for the dispatch table.
pub fn product_stories() -> [ProductStory; 5] {
    [
        ProductStory::Radio,
        ProductStory::Form,
        ProductStory::Combobox,
        ProductStory::Notification,
        ProductStory::Tooltip,
    ]
}
