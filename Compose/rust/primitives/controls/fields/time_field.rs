//! Translation of Primitives/Controls/Fields/TimeField.swift.
//!
//! An editable draft commits on Return or blur. Invalid text never overwrites
//! the bound time: the field keeps the draft and shows the accepted range.
use std::rc::Rc;
use std::sync::{Arc, Mutex, MutexGuard};

use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_core::Binding;

use crate::modifiers::layout::LayoutFrameExt;
use crate::modifiers::style::SemanticStyleViewExt;
use crate::primitives::controls::date_values::TimeOfDay;
use crate::primitives::text::text_buffer::TextBuffer;
use crate::primitives::text::text_field::{TextField, TextFieldEvents};
use crate::primitives::text::Text;
use crate::theme::SemanticFontRef;
use crate::{AnyView, BoxView, TupleView, View};

/// Authored time-field settings.
#[derive(Clone, Debug, PartialEq)]
pub struct TimeFieldOptions {
    pub show_seconds: bool,
    pub is_enabled: bool,
    pub minute_step: i32,
    pub minimum: TimeOfDay,
    pub maximum: TimeOfDay,
}

impl Default for TimeFieldOptions {
    fn default() -> Self {
        Self {
            show_seconds: false,
            is_enabled: true,
            minute_step: 1,
            minimum: TimeOfDay::new(0, 0, 0),
            maximum: TimeOfDay::new(23, 59, 59),
        }
    }
}

impl TimeFieldOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate(&mut self) {
        self.minute_step = self.minute_step.clamp(1, 60);
        if self.maximum < self.minimum {
            self.maximum = self.minimum;
        }
    }

    pub fn validated(mut self) -> Self {
        self.validate();
        self
    }

    /// The placeholder text for the authored precision.
    pub fn placeholder(&self) -> &'static str {
        if self.show_seconds { "HH:MM:SS" } else { "HH:MM" }
    }
}

/// Edit session: the typed draft and its commit error.
#[derive(Clone, Debug, Default)]
struct TimeFieldSession {
    draft: TextBuffer,
    is_editing: bool,
    changed: bool,
    error: String,
}

/// Editable time text field.
pub struct TimeField {
    pub value: Binding<Option<TimeOfDay>>,
    pub options: TimeFieldOptions,
    session: Arc<Mutex<TimeFieldSession>>,
}

impl TimeField {
    pub fn new(value: Binding<Option<TimeOfDay>>, options: TimeFieldOptions) -> Self {
        Self { value, options: options.validated(), session: Arc::new(Mutex::new(TimeFieldSession::default())) }
    }

    fn snapshot(&self) -> TimeField {
        TimeField {
            value: self.value.clone(),
            options: self.options.clone(),
            session: self.session.clone(),
        }
    }

    fn committed(&self) -> TextBuffer {
        TextBuffer::new(
            &self
                .value
                .get()
                .map(|time| time.formatted(self.options.show_seconds))
                .unwrap_or_default(),
        )
    }

    /// Commits the draft. Empty text clears the value; text outside the
    /// authored range or unparseable stays in the field with an error.
    fn commit(&self) {
        let mut session = self.session.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if !session.changed {
            return;
        }
        let text = session.draft.string_value();
        if text.trim().is_empty() {
            self.value.set(None);
            session.error.clear();
            session.changed = false;
            return;
        }
        let parsed = TimeOfDay::parse(&text, self.options.show_seconds).filter(|time| {
            *time >= self.options.minimum && *time <= self.options.maximum
        });
        match parsed {
            Some(time) => {
                self.value.set(Some(time));
                session.error.clear();
                session.changed = false;
            }
            None => {
                session.error = format!(
                    "Enter a valid time from {} to {}.",
                    self.options.minimum.formatted(self.options.show_seconds),
                    self.options.maximum.formatted(self.options.show_seconds),
                );
            }
        }
    }
}

impl View for TimeField {
    fn body(&self) -> AnyView {
        let field = self.snapshot();
        let session = self.session.clone();
        let committed = self.committed();

        let text = Binding::new(
            {
                let session = session.clone();
                let committed = committed.clone();
                move || {
                    if lock(&session).is_editing {
                        lock(&session).draft.clone()
                    } else {
                        committed.clone()
                    }
                }
            },
            {
                let session = session.clone();
                move |buffer: TextBuffer| {
                    let mut session = lock(&session);
                    session.draft = buffer;
                    session.changed = true;
                }
            },
        );

        let input = TextField::new(self.options.placeholder(), text)
            .behavior(crate::primitives::text::text_field::TextFieldBehavior {
                disabled: !self.options.is_enabled,
                ..Default::default()
            })
            .events(TextFieldEvents {
                on_submit: Some({
                    let field = field.clone();
                    Rc::new(move || field.commit())
                }),
                on_focus: Some({
                    let session = session.clone();
                    let committed = committed.clone();
                    Rc::new(move || {
                        let mut session = lock(&session);
                        if !session.is_editing {
                            session.draft = committed.clone();
                            session.is_editing = true;
                            session.changed = false;
                        }
                    })
                }),
                on_blur: Some({
                    let field = field.clone();
                    let session = session.clone();
                    Rc::new(move || {
                        field.commit();
                        lock(&session).is_editing = false;
                    })
                }),
                on_cancel: Some({
                    let session = session.clone();
                    let committed = committed.clone();
                    Rc::new(move || {
                        let mut session = lock(&session);
                        session.error.clear();
                        session.changed = false;
                        session.draft = committed.clone();
                    })
                }),
                on_key_down: Some({
                    let field = field.clone();
                    Rc::new(move |event: &cove_platform::KeyEvent| {
                        if event.scancode != crate::Scancode::ARROW_UP
                            && event.scancode != crate::Scancode::ARROW_DOWN
                        {
                            return false;
                        }
                        // Shift steps an hour; the plain key steps a minute.
                        let step = if event.modifiers.has_shift() { 3600 } else { field.options.minute_step * 60 };
                        let delta = if event.scancode == crate::Scancode::ARROW_UP { step } else { -step };
                        let current = field.value.get().unwrap_or(field.options.minimum);
                        let seconds = (current.seconds() + delta)
                            .clamp(field.options.minimum.seconds(), field.options.maximum.seconds());
                        let next = TimeOfDay::new(seconds / 3600, (seconds / 60) % 60, seconds % 60);
                        field.value.set(Some(next));
                        let mut session = field.session.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                        session.draft = TextBuffer::new(&next.formatted(field.options.show_seconds));
                        session.changed = false;
                        session.error.clear();
                        true
                    })
                }),
                ..Default::default()
            });

        let error = self.session.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).error.clone();
        let label = AnyView::new(TimeFieldLabel { input: AnyView::new(input) });
        if error.is_empty() {
            return label;
        }
        AnyView::new(
            BoxView::new(
                FlexDirection::Column, Align::Stretch, Justify::FlexStart, Wrap::NoWrap, 4.0,
                AnyView::new(TupleView::new(vec![
                    label,
                    AnyView::new(
                        Text::new(error)
                            .semantic_font(SemanticFontRef::caption())
                            .semantic_foreground(crate::theme::SemanticColorRef::error()),
                    ),
                ])),
            )
            .frame_min_width(0.0),
        )
    }
}

/// Applies the field's accessible name.
struct TimeFieldLabel {
    input: AnyView,
}

impl crate::PrimitiveView for TimeFieldLabel {
    fn make_node(&self) -> cove_core::scene::Node {
        let node = cove_core::scene::Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, node: &cove_core::scene::Node) {
        node.set_accessibility(Some(cove_core::scene::AccessibilitySemantics {
            role: cove_core::scene::AccessibilityRole::Group,
            label: "Time".to_string(),
            ..Default::default()
        }));
    }

    fn children(&self, _node: &cove_core::scene::Node) -> Vec<AnyView> {
        vec![self.input.clone()]
    }
}
crate::impl_primitive_view!(TimeFieldLabel);

impl TimeField {
    /// The error text currently shown under the field.
    pub fn error(&self) -> String {
        self.session.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).error.clone()
    }
}

impl Clone for TimeField {
    fn clone(&self) -> Self {
        self.snapshot()
    }
}


fn lock(session: &Arc<Mutex<TimeFieldSession>>) -> MutexGuard<'_, TimeFieldSession> {
    session.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
