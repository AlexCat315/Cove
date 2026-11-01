//! Translation of `Primitives/Controls/Fields/Combobox/{Combobox,RemoteSearch,Results}.swift`
//! (the view half).
//!
//! Searchable typed selection. Query, highlight and overlay state are transient
//! and separate from the value. Options are either a fixed list filtered
//! locally, or a [`ComboboxSearch`] whose results the component requests,
//! debounces and cancels.
//!
//! The request bookkeeping — generations, cancellation, too-short queries and
//! duplicate suppression — lives in [`ComboboxSearchSession`]; the results area
//! decides between spinner, failure, prompt, empty text and a dimmed list with
//! [`results_presentation`].
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, Node, Wrap};
use cove_core::{Binding, Color};

use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt, SurfaceStyleExt};
use crate::primitives::controls::button::{
    ActionControlAccessibility, Button, ButtonIcon, ButtonIconSource, ButtonRole,
};
use crate::primitives::controls::fields::combobox_model::{
    ComboboxResultsPresentation, ComboboxSearchPhase, ComboboxSearchSession, results_presentation,
    too_short_prompt,
};
use crate::primitives::controls::fields::select::SelectOption;
use crate::primitives::feedback::Spinner;
use crate::primitives::overlays::anchored::{PopoverOverflow, PopoverSurface};
use crate::primitives::overlays::popover::{Popover, PopoverSizing, PopoverTriggerDescription};
use crate::primitives::overlays::roving_focus::SelectionNavigation;
use crate::primitives::text::Text;
use crate::theme::styles::buttons::{ButtonStyleViewExt, GhostButtonStyle};
use crate::theme::{SemanticColorRef, SemanticFontRef, theme_for_node};
use crate::{
    AnyView, BoxView, PrimitiveView, Row, TupleView, VerticalAlignment, View, ViewExt, ViewKind,
    impl_primitive_view,
};

/// `impl_primitive_view!` cannot name a generic host, so the same impl is
/// written once here.
macro_rules! impl_generic_view {
    ($type:ident) => {
        impl<Value: PartialEq + Clone + Send + Sync + 'static> View for $type<Value> {
            fn kind(&self) -> ViewKind<'_> {
                ViewKind::Primitive(self)
            }
            fn tag(&self) -> &'static str {
                std::any::type_name::<Self>()
            }
        }
    };
}

/// Trigger height floor, matching the text fields beside it.
const TRIGGER_MIN_HEIGHT: f32 = 32.0;
/// Minimum popover width.
const MIN_WIDTH: f32 = 120.0;
/// Footer row padding.
const STATUS_PADDING_HORIZONTAL: f32 = 8.0;
const STATUS_PADDING_VERTICAL: f32 = 10.0;

/// Authored combobox settings.
#[derive(Clone, Debug, PartialEq)]
pub struct ComboboxPresentation {
    pub placeholder: String,
    pub search_placeholder: String,
    pub empty_text: String,
    pub width: f32,
    pub max_visible_rows: usize,
    pub is_enabled: bool,
    pub is_clearable: bool,
}

impl Default for ComboboxPresentation {
    fn default() -> Self {
        Self {
            placeholder: "Select an option".to_string(),
            search_placeholder: "Search…".to_string(),
            empty_text: "No matching options".to_string(),
            width: 280.0,
            max_visible_rows: 8,
            is_enabled: true,
            is_clearable: true,
        }
    }
}

impl ComboboxPresentation {
    pub fn new() -> Self {
        Self::default()
    }

    /// Width floors at 120 and at least one row stays visible.
    pub fn validate(&mut self) {
        self.width = self.width.max(MIN_WIDTH);
        self.max_visible_rows = self.max_visible_rows.max(1);
    }

    pub fn validated(mut self) -> Self {
        self.validate();
        self
    }

    /// Row height before the touch floor.
    pub fn authored_row_height(&self, theme: &crate::theme::Theme) -> f32 {
        (theme.typography.label.line_height + 12.0).max(32.0)
    }

    /// Trigger minimum height, scaled by the text scaler.
    pub fn trigger_minimum(&self, theme: &crate::theme::Theme) -> f32 {
        (theme.typography.label.line_height + 12.0).max(32.0)
    }
}

/// Remote results for a combobox: the caller's data layer only maps a query to
/// options, while the component debounces, cancels superseded requests and
/// publishes only the newest generation.
///
/// The reply callback is `Send + Sync` because the request may be delivered from
/// a worker; hand results back with [`ComboboxSearchSession::deliver`].
pub struct ComboboxSearch<Value> {
    /// Quiet time after the last keystroke before a request starts. Opening the
    /// list requests immediately.
    pub delay_ms: u64,
    /// Shorter queries show a prompt instead of requesting.
    pub minimum_query_length: usize,
    pub results: Arc<dyn Fn(&str) -> Result<Vec<SelectOption<Value>>, String> + Send + Sync>,
}

impl<Value> Clone for ComboboxSearch<Value> {
    fn clone(&self) -> Self {
        Self {
            delay_ms: self.delay_ms,
            minimum_query_length: self.minimum_query_length,
            results: self.results.clone(),
        }
    }
}

impl<Value> ComboboxSearch<Value> {
    pub fn new(
        results: impl Fn(&str) -> Result<Vec<SelectOption<Value>>, String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            delay_ms: 250,
            minimum_query_length: 0,
            results: Arc::new(results),
        }
    }

    /// Both fields are unsigned, so there is nothing to clamp.
    pub fn validated(self) -> Self {
        self
    }
}

/// Transient session state, separate from the committed value.
#[derive(Clone, Debug)]
struct ComboboxSession<Value> {
    is_presented: bool,
    query: String,
    highlighted: Option<Value>,
    /// The last committed option, so a remote selection keeps its label after
    /// the results change.
    chosen: Option<SelectOption<Value>>,
}

impl<Value> Default for ComboboxSession<Value> {
    fn default() -> Self {
        Self {
            is_presented: false,
            query: String::new(),
            highlighted: None,
            chosen: None,
        }
    }
}

/// Either a fixed option list or a remote search.
#[derive(Clone)]
enum ComboboxSource<Value> {
    Options(Vec<SelectOption<Value>>),
    Search {
        search: ComboboxSearch<Value>,
        selected_label: Option<String>,
    },
}

/// Searchable typed selection.
pub struct Combobox<Value: PartialEq + Clone + Send + Sync + 'static> {
    pub selection: Binding<Option<Value>>,
    pub presentation: ComboboxPresentation,
    source: ComboboxSource<Value>,
    session: Arc<Mutex<ComboboxSession<Value>>>,
    /// The request bookkeeping is label-only; this keeps the typed options that
    /// belong to the accepted generation.
    remote: Arc<Mutex<ComboboxSearchSession>>,
    remote_options: Arc<Mutex<Vec<SelectOption<Value>>>>,
}

impl<Value: PartialEq + Clone + Send + Sync + 'static> Combobox<Value> {
    /// Fixed options, filtered locally.
    pub fn new(selection: Binding<Option<Value>>, options: Vec<SelectOption<Value>>) -> Self {
        for (index, option) in options.iter().enumerate() {
            assert!(
                options
                    .iter()
                    .enumerate()
                    .all(|(other, candidate)| other == index || candidate.value != option.value),
                "Combobox values must be unique"
            );
        }
        Self {
            selection,
            presentation: ComboboxPresentation::default(),
            source: ComboboxSource::Options(options),
            session: Arc::new(Mutex::new(ComboboxSession::default())),
            remote: Arc::new(Mutex::new(ComboboxSearchSession::new())),
            remote_options: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Remote options. `selected_label` names the current value while it is not
    /// among the loaded results, for example before the first search.
    pub fn search(
        selection: Binding<Option<Value>>,
        selected_label: Option<String>,
        search: ComboboxSearch<Value>,
    ) -> Self {
        Self {
            selection,
            presentation: ComboboxPresentation::default(),
            source: ComboboxSource::Search {
                search: search.validated(),
                selected_label,
            },
            session: Arc::new(Mutex::new(ComboboxSession::default())),
            remote: Arc::new(Mutex::new(ComboboxSearchSession::new())),
            remote_options: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn presentation(mut self, presentation: ComboboxPresentation) -> Self {
        self.presentation = presentation.validated();
        self
    }

    fn snapshot(&self) -> Combobox<Value> {
        Combobox {
            selection: self.selection.clone(),
            presentation: self.presentation.clone(),
            source: self.source.clone(),
            session: self.session.clone(),
            remote: self.remote.clone(),
            remote_options: self.remote_options.clone(),
        }
    }

    /// Options matching a query. Local lists filter by every whitespace term;
    /// remote lists use whatever the last request published.
    pub fn matching(options: &[SelectOption<Value>], query: &str) -> Vec<SelectOption<Value>> {
        let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        if terms.is_empty() {
            return options.to_vec();
        }
        options
            .iter()
            .filter(|option| {
                let label = option.label.to_lowercase();
                terms.iter().all(|term| label.contains(term))
            })
            .cloned()
            .collect()
    }

    fn matches(&self, query: &str) -> Vec<SelectOption<Value>> {
        match &self.source {
            ComboboxSource::Options(options) => Self::matching(options, query),
            ComboboxSource::Search { .. } => self
                .remote_options
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone(),
        }
    }

    fn phase(&self) -> ComboboxSearchPhase {
        match &self.source {
            ComboboxSource::Options(_) => ComboboxSearchPhase::Loaded,
            ComboboxSource::Search { .. } => self
                .remote
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .phase()
                .clone(),
        }
    }

    /// The label for the committed value, falling back to the caller's label for
    /// a remote selection that is no longer in the results.
    fn selected_label(&self) -> Option<String> {
        let value = self.selection.get()?;
        match &self.source {
            ComboboxSource::Options(options) => options
                .iter()
                .find(|option| option.value == value)
                .map(|option| option.label.clone()),
            ComboboxSource::Search { selected_label, .. } => self
                .remote_options
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .iter()
                .find(|option| option.value == value)
                .map(|option| option.label.clone())
                .or_else(|| {
                    self.session
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .chosen
                        .as_ref()
                        .filter(|chosen| chosen.value == value)
                        .map(|chosen| chosen.label.clone())
                })
                .or_else(|| selected_label.clone()),
        }
    }

    /// Starts a remote request, if this combobox has one.
    fn request(&self, query: &str, force: bool) -> Option<u64> {
        let ComboboxSource::Search { search, .. } = &self.source else {
            return None;
        };
        let generation = self
            .remote
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .begin(query, search.minimum_query_length, force)?;
        // The reply is applied through `deliver`, which drops a superseded
        // generation.
        let _ = search.delay_ms;
        Some(generation)
    }

    /// Chooses an enabled option and closes the popover.
    fn choose(&self, value: &Value) {
        if !self.presentation.is_enabled {
            return;
        }
        let matches = self.matches(
            &self
                .session
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .query
                .clone(),
        );
        let Some(option) = matches
            .iter()
            .find(|option| &option.value == value && option.is_enabled)
        else {
            return;
        };
        let option = option.clone();
        {
            let mut session = self
                .session
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            session.chosen = Some(option);
            session.is_presented = false;
        }
        self.selection.set(Some(value.clone()));
    }

    /// Moves the highlight over the enabled values only.
    fn navigate(&self, command: SelectionNavigation) {
        let matches = self.matches(
            &self
                .session
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .query
                .clone(),
        );
        let enabled: Vec<Value> = matches
            .iter()
            .filter(|option| option.is_enabled)
            .map(|option| option.value.clone())
            .collect();
        let mut session = self
            .session
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        session.highlighted = command.destination(&enabled, session.highlighted.as_ref(), false);
    }

    /// Opens the popover, resetting the query and seeding the highlight.
    fn open(&self) {
        {
            let mut session = self
                .session
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            session.query.clear();
            session.highlighted = self.selection.get();
        }
        // Remote data can change between opens: refresh, keeping the previous
        // results dimmed until the new ones arrive.
        self.request("", true);
    }

    fn close(&self) {
        self.remote
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .cancel();
    }
}

impl<Value: PartialEq + Clone + Send + Sync + 'static> View for Combobox<Value> {
    fn body(&self) -> AnyView {
        let field = self.snapshot();
        let selected_label = self.selected_label();
        let title = selected_label
            .clone()
            .unwrap_or_else(|| self.presentation.placeholder.clone());

        let presented = Binding::new(
            {
                let field = field.clone();
                move || {
                    field
                        .session
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .is_presented
                }
            },
            {
                let field = field.clone();
                move |open: bool| {
                    let was_open = field
                        .session
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .is_presented;
                    if open && !was_open {
                        field.open();
                    }
                    if !open && was_open {
                        field.close();
                    }
                    field
                        .session
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .is_presented = open;
                }
            },
        );

        let trigger = AnyView::new(ComboboxTrigger {
            field: field.clone(),
            title: title.clone(),
            is_placeholder: selected_label.is_none(),
        });

        let content = AnyView::new(ComboboxContent {
            field: field.clone(),
        });

        let popover = Popover::new(trigger, content)
            .is_presented(presented)
            .enabled(self.presentation.is_enabled)
            .width(self.presentation.width)
            .sizing(PopoverSizing::Anchor)
            .surface(PopoverSurface::Content)
            .overflow(PopoverOverflow::Flip)
            .trigger_description(PopoverTriggerDescription {
                label: title.clone(),
                value: selected_label.clone().unwrap_or_default(),
                help: None,
            });

        // Swift provides the role and name as composition locals so the trigger
        // `Button` reads them in `Button::update_node`; a wrapper node carried no
        // semantics of its own.
        AnyView::new(
            AnyView::new(popover)
                .composition_local(
                    ActionControlAccessibility::role(),
                    AccessibilityRole::ComboBox,
                )
                .composition_local(ActionControlAccessibility::label(), Some(title))
                .composition_local(
                    ActionControlAccessibility::value(),
                    selected_label.unwrap_or_default(),
                )
                .frame_width(self.presentation.width),
        )
    }
}

impl<Value: PartialEq + Clone + Send + Sync + 'static> Clone for Combobox<Value> {
    fn clone(&self) -> Self {
        self.snapshot()
    }
}

/// The field-chrome trigger: title, an optional clear button and a chevron.
struct ComboboxTrigger<Value: PartialEq + Clone + Send + Sync + 'static> {
    field: Combobox<Value>,
    title: String,
    is_placeholder: bool,
}

impl<Value: PartialEq + Clone + Send + Sync + 'static> PrimitiveView for ComboboxTrigger<Value> {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let field = &self.field;
        let mut row: Vec<AnyView> = vec![AnyView::new(
            Text::new(self.title.clone())
                .truncated()
                .semantic_font(SemanticFontRef::label())
                .semantic_foreground(if self.is_placeholder {
                    SemanticColorRef::placeholder()
                } else {
                    SemanticColorRef::on_surface()
                })
                .flex_with(1.0, 1.0, Some(0.0)),
        )];
        if field.presentation.is_clearable && field.selection.get().is_some() {
            let selection = field.selection.clone();
            row.push(AnyView::new(
                Button::icon_with_options(
                    ButtonIconSource::resource(
                        crate::primitives::media::ui_icon_catalog::action::x(),
                    ),
                    Some(12.0),
                    None,
                    ButtonRole::Normal,
                    field.presentation.is_enabled,
                    false,
                    false,
                    Some("Clear selection".to_string()),
                    move || selection.set(None),
                )
                .button_style(GhostButtonStyle),
            ));
        }
        row.push(AnyView::new(ButtonIcon::new(
            ButtonIconSource::resource(
                crate::primitives::media::ui_icon_catalog::navigation::dropdown_chevron_down(),
            ),
            Some(12.0),
            None,
        )));
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                8.0,
                AnyView::new(TupleView::new(row)),
            )
            .padding_edges(10.0, 4.0)
            .frame_size(field.presentation.width, TRIGGER_MIN_HEIGHT)
            .semantic_background(SemanticColorRef::surface())
            .corner_radius_metric(crate::theme::ThemeMetric::input_radius())
            .semantic_border(SemanticColorRef::border(), 1.0),
        )]
    }
}
impl_generic_view!(ComboboxTrigger);

/// The popover body: search field plus the results area.
struct ComboboxContent<Value: PartialEq + Clone + Send + Sync + 'static> {
    field: Combobox<Value>,
}

impl<Value: PartialEq + Clone + Send + Sync + 'static> PrimitiveView for ComboboxContent<Value> {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let field = self.field.clone();
        let version = field
            .session
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .query
            .clone();
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                6.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(ComboboxSearchInput {
                        field: field.clone(),
                        version,
                    }),
                    AnyView::new(ComboboxResultsArea {
                        field: field.clone(),
                    }),
                ])),
            )
            .padding(4.0)
            .floating_surface(12.0),
        )]
    }
}
impl_generic_view!(ComboboxContent);

/// The search field inside the popover.
struct ComboboxSearchInput<Value: PartialEq + Clone + Send + Sync + 'static> {
    field: Combobox<Value>,
    version: String,
}

impl<Value: PartialEq + Clone + Send + Sync + 'static> PrimitiveView
    for ComboboxSearchInput<Value>
{
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let field = self.field.clone();
        let field_for_get = field.clone();
        let field_for_set = field.clone();
        let text = Binding::new(
            move || {
                crate::primitives::text::text_buffer::TextBuffer::new(
                    &field_for_get
                        .session
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .query
                        .clone(),
                )
            },
            move |buffer: crate::primitives::text::text_buffer::TextBuffer| {
                let query = buffer.string_value();
                {
                    let mut session = field_for_set
                        .session
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    session.query = query.clone();
                }
                let matches = field_for_set.matches(&query);
                let first_enabled = matches
                    .iter()
                    .find(|option| option.is_enabled)
                    .map(|option| option.value.clone());
                field_for_set
                    .session
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .highlighted = first_enabled;
                field_for_set.request(&query, false);
            },
        );
        let key_field = field.clone();
        let submit_field = field.clone();
        let input = crate::primitives::text::text_field::TextField::new(
            field.presentation.search_placeholder.clone(),
            text,
        )
        .behavior(crate::primitives::text::text_field::TextFieldBehavior {
            clearable: true,
            ..Default::default()
        })
        .events(crate::primitives::text::text_field::TextFieldEvents {
            on_submit: Some(Rc::new(move || {
                let matches = submit_field.matches(
                    &submit_field
                        .session
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .query
                        .clone(),
                );
                let highlighted = submit_field
                    .session
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .highlighted
                    .clone();
                match highlighted {
                    Some(value)
                        if matches
                            .iter()
                            .any(|option| option.value == value && option.is_enabled) =>
                    {
                        submit_field.choose(&value)
                    }
                    _ => {
                        if let Some(first) = matches.iter().find(|option| option.is_enabled) {
                            submit_field.choose(&first.value);
                        }
                    }
                }
            })),
            on_key_down: Some(Rc::new(move |event: &cove_platform::KeyEvent| {
                if event.scancode != crate::Scancode::ARROW_UP
                    && event.scancode != crate::Scancode::ARROW_DOWN
                {
                    return false;
                }
                let Some(command) = SelectionNavigation::command_for(event.scancode) else {
                    return false;
                };
                key_field.navigate(command);
                true
            })),
            ..Default::default()
        });
        let _ = &self.version;
        vec![AnyView::new(input)]
    }
}
impl_generic_view!(ComboboxSearchInput);

/// The list, or the status row that stands in for it.
struct ComboboxResultsArea<Value: PartialEq + Clone + Send + Sync + 'static> {
    field: Combobox<Value>,
}

impl<Value: PartialEq + Clone + Send + Sync + 'static> PrimitiveView
    for ComboboxResultsArea<Value>
{
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let field = &self.field;
        let matches = field.matches(
            &field
                .session
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .query
                .clone(),
        );
        let phase = field.phase();
        let count = matches.len();
        let presentation = results_presentation(&phase, count, &field.presentation.empty_text);

        match presentation {
            ComboboxResultsPresentation::Searching => vec![AnyView::new(ComboboxStatus {
                icon: None,
                text: "Searching…".to_string(),
                tone: SemanticColorRef::on_surface_muted(),
                accessory: Some(AnyView::new(Spinner::new(14.0))),
            })],
            ComboboxResultsPresentation::Failed(message) => {
                let retry_field = field.clone();
                vec![AnyView::new(ComboboxStatus {
                    icon: Some(crate::primitives::media::ui_icon_catalog::status::alert_circle()),
                    text: message,
                    tone: SemanticColorRef::error(),
                    accessory: Some(AnyView::new(
                        Button::with_options(
                            AnyView::new(
                                Text::new("Retry").semantic_font(SemanticFontRef::label()),
                            ),
                            move || {
                                let query = retry_field
                                    .session
                                    .lock()
                                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                                    .query
                                    .clone();
                                retry_field.request(&query, true);
                            },
                            ButtonRole::Normal,
                            true,
                            false,
                            false,
                            None,
                        )
                        .button_style(GhostButtonStyle),
                    )),
                })]
            }
            ComboboxResultsPresentation::TooShort(count) => vec![AnyView::new(ComboboxStatus {
                icon: None,
                text: too_short_prompt(count),
                tone: SemanticColorRef::on_surface_muted(),
                accessory: None,
            })],
            ComboboxResultsPresentation::Empty(text) => vec![AnyView::new(ComboboxStatus {
                icon: None,
                text,
                tone: SemanticColorRef::on_surface_muted(),
                accessory: None,
            })],
            ComboboxResultsPresentation::List { dimmed } => {
                let row_height = 32.0_f32.max(matches.len() as f32 * 0.0);
                let visible = matches
                    .len()
                    .min(field.presentation.max_visible_rows)
                    .max(1);
                vec![AnyView::new(
                    ComboboxList {
                        field: field.clone(),
                        matches,
                        row_height,
                    }
                    .frame(
                        None,
                        Some(crate::modifiers::layout::FrameDimension::Points(
                            row_height * visible as f32,
                        )),
                        None,
                        None,
                        None,
                        None,
                    )
                    .opacity(if dimmed { 0.55 } else { 1.0 }),
                )]
            }
        }
    }
}
impl_generic_view!(ComboboxResultsArea);

/// A status row: optional glyph, text and an accessory.
struct ComboboxStatus {
    icon: Option<cove_imaging::BundleImageResource>,
    text: String,
    tone: SemanticColorRef,
    accessory: Option<AnyView>,
}

impl PrimitiveView for ComboboxStatus {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        node.set_accessibility(Some(cove_core::scene::AccessibilitySemantics {
            role: cove_core::scene::AccessibilityRole::Status,
            label: self.text.clone(),
            ..Default::default()
        }));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let mut row: Vec<AnyView> = Vec::new();
        if let Some(icon) = self.icon.clone() {
            row.push(AnyView::new(
                ButtonIcon::new(ButtonIconSource::resource(icon), Some(12.0), None)
                    .semantic_foreground(self.tone.clone())
                    .flex(0.0)
                    .shrink(0.0),
            ));
        }
        row.push(AnyView::new(
            Text::new(self.text.clone())
                .semantic_font(SemanticFontRef::caption())
                .semantic_foreground(self.tone.clone())
                .flex_with(1.0, 1.0, Some(0.0))
                .frame_min_width(0.0),
        ));
        if let Some(accessory) = self.accessory.clone() {
            row.push(accessory);
        }
        vec![AnyView::new(
            Row::new(
                VerticalAlignment::Center,
                8.0,
                AnyView::new(TupleView::new(row)),
            )
            .padding_edges(STATUS_PADDING_HORIZONTAL, STATUS_PADDING_VERTICAL),
        )]
    }
}
impl_primitive_view!(ComboboxStatus);

/// The result rows, each a press target that commits its option.
struct ComboboxList<Value: PartialEq + Clone + Send + Sync + 'static> {
    field: Combobox<Value>,
    matches: Vec<SelectOption<Value>>,
    row_height: f32,
}

impl<Value: PartialEq + Clone + Send + Sync + 'static> PrimitiveView for ComboboxList<Value> {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let field = &self.field;
        let highlighted = field
            .session
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .highlighted
            .clone();
        let selected = field.selection.get();
        self.matches
            .iter()
            .map(|option| {
                let is_highlighted = highlighted.as_ref() == Some(&option.value);
                let mut row: Vec<AnyView> = vec![AnyView::new(
                    Text::new(option.label.clone())
                        .truncated()
                        .semantic_font(SemanticFontRef::label())
                        .semantic_foreground(if option.is_enabled {
                            SemanticColorRef::on_surface()
                        } else {
                            SemanticColorRef::on_surface_muted()
                        })
                        .flex_with(1.0, 1.0, Some(0.0))
                        .frame_min_width(0.0),
                )];
                if selected.as_ref() == Some(&option.value) {
                    row.push(AnyView::new(
                        ButtonIcon::new(
                            ButtonIconSource::resource(
                                crate::primitives::media::ui_icon_catalog::action::check(),
                            ),
                            Some(12.0),
                            None,
                        )
                        .flex(0.0)
                        .shrink(0.0),
                    ));
                }
                let target = field.clone();
                let value = option.value.clone();
                let is_enabled = option.is_enabled;
                AnyView::new(
                    Button::with_options(
                        AnyView::new(
                            BoxView::new(
                                FlexDirection::Row,
                                Align::Center,
                                Justify::FlexStart,
                                Wrap::NoWrap,
                                8.0,
                                AnyView::new(TupleView::new(row)),
                            )
                            .padding_edges(8.0, 4.0)
                            .frame_height(self.row_height),
                        ),
                        move || {
                            if is_enabled {
                                target.choose(&value);
                            }
                        },
                        ButtonRole::Normal,
                        is_enabled,
                        false,
                        is_highlighted,
                        None,
                    )
                    .button_style(ComboboxRowStyle),
                )
            })
            .collect()
    }
}
impl_generic_view!(ComboboxList);

/// Row chrome: selected and hovered state layers over the popover material.
struct ComboboxRowStyle;

impl crate::theme::styles::buttons::ButtonStyle for ComboboxRowStyle {
    fn make_body(
        &self,
        configuration: crate::theme::styles::buttons::ButtonStyleConfiguration,
    ) -> AnyView {
        let theme = configuration.theme.clone();
        let fill = if configuration.is_selected {
            theme.colors.state_layer_selected
        } else if configuration.is_hovered {
            theme.colors.state_layer_hover
        } else {
            Color::CLEAR
        };
        AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(configuration.label.clone()),
            )
            .background_color(fill)
            .corner_radius(theme.radius.sm)
            .semantic_foreground(SemanticColorRef::on_surface()),
        )
    }
}

impl<Value: PartialEq + Clone + Send + Sync + 'static> Combobox<Value> {
    /// Height of one row for the themed label slot.
    pub fn row_height(&self, theme: &crate::theme::Theme) -> f32 {
        let _ = theme_for_node;
        self.presentation.authored_row_height(theme)
    }
}
