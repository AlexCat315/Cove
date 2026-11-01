//! Translation of Primitives/Controls/Fields/SearchField.swift (the view half).
//!
//! Search input built on the regular text field chrome: a decorative leading
//! search icon, the field's clear affordance, submit and Escape handling, and an
//! optional suggestion list that supports arrow keys, Return and pointer.
//!
//! Matching, visibility, highlighting and key decisions live in
//! [`SearchFieldState`]; this module renders that state and forwards events.
use crate::Scope;
use std::rc::Rc;

use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};
use cove_core::{Binding, Signal};

use crate::modifiers::layout::{LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::{Button, ButtonRole};
use crate::primitives::controls::fields::search_field_model::{
    SearchAction, SearchFieldState, SearchKey,
};
use crate::primitives::text::Text;
use crate::primitives::text::text_buffer::TextBuffer;
use crate::primitives::text::text_field::{TextField, TextFieldDecoration, TextFieldEvents};
use crate::theme::SemanticColorRef;
use crate::theme::styles::buttons::{ButtonStyle, ButtonStyleConfiguration, ButtonStyleViewExt};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View, impl_primitive_view};

/// Gap between the field and the suggestion list.
const LIST_SPACING: f32 = 4.0;
/// Suggestion row padding.
const ROW_PADDING_HORIZONTAL: f32 = 10.0;
const ROW_PADDING_VERTICAL: f32 = 8.0;

/// Search field with an optional suggestion list.
pub struct SearchField {
    pub text: Binding<TextBuffer>,
    pub placeholder: String,
    pub suggestions: Vec<String>,
    pub is_enabled: bool,
    pub clears_on_escape: bool,
    pub on_submit: Option<Rc<dyn Fn(String)>>,
    pub on_suggestion_selected: Option<Rc<dyn Fn(String)>>,
}

impl SearchField {
    pub fn new(text: Binding<TextBuffer>) -> Self {
        Self {
            text,
            placeholder: "Search".to_string(),
            suggestions: Vec::new(),
            is_enabled: true,
            clears_on_escape: true,
            on_submit: None,
            on_suggestion_selected: None,
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn suggestions(mut self, suggestions: Vec<String>) -> Self {
        self.suggestions = suggestions;
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn clears_on_escape(mut self, clears_on_escape: bool) -> Self {
        self.clears_on_escape = clears_on_escape;
        self
    }

    pub fn on_submit(mut self, handler: impl Fn(String) + 'static) -> Self {
        self.on_submit = Some(Rc::new(handler));
        self
    }

    pub fn on_suggestion_selected(mut self, handler: impl Fn(String) + 'static) -> Self {
        self.on_suggestion_selected = Some(Rc::new(handler));
        self
    }

    fn snapshot(&self) -> SearchField {
        SearchField {
            text: self.text.clone(),
            placeholder: self.placeholder.clone(),
            suggestions: self.suggestions.clone(),
            is_enabled: self.is_enabled,
            clears_on_escape: self.clears_on_escape,
            on_submit: self.on_submit.clone(),
            on_suggestion_selected: self.on_suggestion_selected.clone(),
        }
    }
}

/// The field's configuration plus its scope-owned suggestion state.
#[derive(Clone)]
struct SearchSession {
    field: SearchField,
    state: Signal<SearchFieldState>,
}

impl SearchSession {
    fn submit(&self) {
        if let Some(handler) = self.field.on_submit.as_ref() {
            handler(self.field.text.get().string_value());
        }
    }

    fn choose(&self, suggestion: &str) {
        self.field.text.set(TextBuffer::new(suggestion));
        self.state.update(SearchFieldState::choose);
        if let Some(handler) = self.field.on_suggestion_selected.as_ref() {
            handler(suggestion.to_string());
        }
    }

    /// Routes one intercepted key through the model's decision table.
    fn handle_key(&self, event: &cove_platform::KeyEvent) -> bool {
        let key = match event.scancode {
            crate::Scancode::ARROW_DOWN => SearchKey::ArrowDown,
            crate::Scancode::ARROW_UP => SearchKey::ArrowUp,
            crate::Scancode::ESCAPE => SearchKey::Escape,
            crate::Scancode::RETURN => SearchKey::Return,
            crate::Scancode::KEYPAD_ENTER => SearchKey::KeypadEnter,
            _ => return false,
        };
        let current = self.field.text.get().string_value();
        let mut action = SearchAction::None;
        self.state.update(|state| {
            action = state.handle_key(
                key,
                &current,
                &self.field.suggestions,
                self.field.clears_on_escape,
            );
        });
        match action {
            SearchAction::None => false,
            SearchAction::MoveHighlight => true,
            SearchAction::Clear => {
                self.field.text.set(TextBuffer::empty());
                true
            }
            SearchAction::Choose(index) => {
                let matches = SearchFieldState::matches(&current, &self.field.suggestions);
                if let Some(suggestion) = matches.get(index) {
                    self.choose(&suggestion.clone());
                }
                true
            }
            SearchAction::Submit => {
                self.submit();
                true
            }
        }
    }
}

impl Clone for SearchField {
    fn clone(&self) -> Self {
        self.snapshot()
    }
}

impl View for SearchField {
    fn body(&self, cx: &Scope) -> AnyView {
        let state = cx.state(SearchFieldState::new);
        let field = SearchSession {
            field: self.snapshot(),
            state,
        };
        let suggestions = self.suggestions.clone();

        let input = TextField::new(self.placeholder.clone(), self.text.clone())
            .decoration(TextFieldDecoration {
                leading_icon: Some(crate::primitives::media::ui_icon_catalog::action::search()),
                ..Default::default()
            })
            .behavior(crate::primitives::text::text_field::TextFieldBehavior {
                clearable: true,
                disabled: !self.is_enabled,
                ..Default::default()
            })
            .events(TextFieldEvents {
                on_submit: Some({
                    let field = field.clone();
                    Rc::new(move || field.submit())
                }),
                on_key_down: Some({
                    let field = field.clone();
                    Rc::new(move |event: &cove_platform::KeyEvent| field.handle_key(event))
                }),
                on_change: Some({
                    let suggestions = suggestions.clone();
                    Rc::new(move |buffer: &TextBuffer| {
                        state.update(|state| state.on_change(&buffer.string_value(), &suggestions));
                    })
                }),
                on_focus: Some({
                    let suggestions = suggestions.clone();
                    let text = field.field.text.clone();
                    Rc::new(move || {
                        let current = text.get().string_value();
                        state.update(|state| state.on_focus(&current, &suggestions));
                    })
                }),
                on_blur: Some(Rc::new(move || state.update(SearchFieldState::on_blur))),
                on_clear: Some(Rc::new(move || state.update(SearchFieldState::on_clear))),
                ..Default::default()
            });

        let text = self.text.get().string_value();
        let shows =
            state.with(|state| state.shows_suggestions(&text, &self.suggestions, self.is_enabled));

        let mut column: Vec<AnyView> = vec![AnyView::new(input)];
        if shows {
            column.push(AnyView::new(SuggestionList { session: field }));
        }
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                LIST_SPACING,
                AnyView::new(TupleView::new(column)),
            )
            .frame_min_width(0.0),
        )
    }
}

/// The suggestion list beneath the field.
struct SuggestionList {
    session: SearchSession,
}

impl PrimitiveView for SuggestionList {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        let theme = crate::theme::theme_for_node(node);
        node.set_background_color(Some(theme.colors.surface_floating));
        node.set_corner_radius(theme.radius.md);
        node.set_border_color(Some(theme.colors.border));
        node.set_border_width(1.0);
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let field = &self.session.field;
        let text = field.text.get().string_value();
        let matches = SearchFieldState::matches(&text, &field.suggestions);
        let highlighted = self.session.state.with(|state| state.highlighted);
        let mut rows: Vec<AnyView> = Vec::new();
        for (index, suggestion) in matches.into_iter().enumerate() {
            let session = self.session.clone();
            let value = suggestion.clone();
            rows.push(AnyView::new(
                Button::with_options(
                    AnyView::new(Text::new(suggestion)),
                    move || session.choose(&value),
                    ButtonRole::Normal,
                    field.is_enabled,
                    false,
                    highlighted == Some(index),
                    None,
                )
                .button_style(SuggestionRowStyle),
            ));
        }
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(rows)),
            )
            .semantic_border(SemanticColorRef::border(), 1.0),
        )]
    }
}
impl_primitive_view!(SuggestionList);

/// Fills the row and keeps the label at the leading edge; a plain ghost button
/// centers its label, which reads as a menu title rather than a suggestion.
struct SuggestionRowStyle;

impl ButtonStyle for SuggestionRowStyle {
    fn make_body(&self, configuration: ButtonStyleConfiguration) -> AnyView {
        let theme = configuration.theme.clone();
        let fill = if configuration.is_selected {
            theme.colors.state_layer_selected
        } else if configuration.is_hovered {
            theme.colors.state_layer_hover
        } else {
            cove_core::Color::CLEAR
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
            .frame(
                Some(crate::modifiers::layout::FrameDimension::Percent(100.0)),
                None,
                None,
                None,
                None,
                None,
            )
            .padding_edges(ROW_PADDING_HORIZONTAL, ROW_PADDING_VERTICAL)
            .background_color(fill)
            .foreground_color(if configuration.is_enabled {
                theme.colors.on_surface
            } else {
                theme.colors.on_surface_muted
            }),
        )
    }
}
