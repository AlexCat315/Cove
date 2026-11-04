//! Translation of Primitives/Controls/Fields/JsonField.swift (the view half).
//!
//! A JSON editor with a format/revert/expand toolbar, inline validation and an
//! expanded modal. The draft is committed only while it is acceptable, so
//! malformed text never replaces the bound value.
//!
//! Validation, normalization, pretty-printing and the labels live in
//! [`JsonField`](super::json_field_model) helpers: [`validate`],
//! [`pretty_printed`], [`normalized_commit_text`] and [`JsonFieldLabels`].
use crate::Scope;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use cove_core::Binding;
use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};

use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::primitives::controls::button::{Button, ButtonIconSource, ButtonRole};
use crate::primitives::controls::fields::json_field_model::{
    JsonFieldLabels, JsonFieldValidation, normalized_commit_text, pretty_printed, validate,
};
use crate::primitives::layout::resizable_text_area::ResizableTextArea;
use crate::primitives::overlays::modal::{Modal, ModalOptions};
use crate::primitives::text::Text;
use crate::primitives::text::text_buffer::TextBuffer;
use crate::primitives::text::text_field::TextFieldCodeEditing;
use crate::theme::styles::buttons::{ButtonStyleViewExt, GhostButtonStyle};
use crate::theme::styles::components::InputChromeState;
use crate::theme::tokens::SemanticStyleViewExt;
use crate::theme::{SemanticColorRef, SemanticFontRef, theme_for_node};
use crate::{
    AnyView, BoxView, PrimitiveView, Row, TupleView, VerticalAlignment, View, impl_primitive_view,
};
use cove_composition::LayoutDebugExt;

/// Toolbar spacing.
const TOOLBAR_SPACING: f32 = 2.0;

/// Edit session shared by the inline editor and the expanded modal.
#[derive(Clone, Debug)]
struct JsonDraft {
    text: TextBuffer,
    has_draft: bool,
    baseline: TextBuffer,
    expanded: TextBuffer,
    is_expanded: bool,
}

impl JsonDraft {
    fn new(baseline: TextBuffer) -> Self {
        Self {
            text: baseline.clone(),
            has_draft: false,
            baseline,
            expanded: TextBuffer::empty(),
            is_expanded: false,
        }
    }

    fn current(&self) -> &TextBuffer {
        if self.has_draft {
            &self.text
        } else {
            &self.baseline
        }
    }
}

/// JSON text editor with validation feedback.
pub struct JsonField {
    pub text: Binding<TextBuffer>,
    pub placeholder: String,
    pub min_height: f32,
    pub is_enabled: bool,
    pub labels: JsonFieldLabels,
    pub on_commit: Option<Rc<dyn Fn(TextBuffer)>>,
    draft: Arc<Mutex<JsonDraft>>,
}

impl JsonField {
    pub fn new(text: Binding<TextBuffer>) -> Self {
        let baseline = text.get();
        Self {
            text,
            placeholder: "{}".to_string(),
            min_height: 96.0,
            is_enabled: true,
            labels: JsonFieldLabels::default(),
            on_commit: None,
            draft: Arc::new(Mutex::new(JsonDraft::new(baseline))),
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn min_height(mut self, min_height: f32) -> Self {
        self.min_height = min_height;
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn labels(mut self, labels: JsonFieldLabels) -> Self {
        self.labels = labels;
        self
    }

    pub fn on_commit(mut self, handler: impl Fn(TextBuffer) + 'static) -> Self {
        self.on_commit = Some(Rc::new(handler));
        self
    }

    fn snapshot(&self) -> JsonField {
        JsonField {
            text: self.text.clone(),
            placeholder: self.placeholder.clone(),
            min_height: self.min_height,
            is_enabled: self.is_enabled,
            labels: self.labels.clone(),
            on_commit: self.on_commit.clone(),
            draft: self.draft.clone(),
        }
    }

    /// The text the field currently shows: the draft while editing, else the
    /// bound value.
    fn current_text(&self) -> TextBuffer {
        let draft = self
            .draft
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        draft.current().clone()
    }

    fn validation(&self) -> JsonFieldValidation {
        validate(&self.current_text().string_value())
    }

    /// Starts editing from the bound value, seeding `{}` for an empty field.
    fn begin_editing(&self) {
        let mut draft = self
            .draft
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if draft.has_draft {
            return;
        }
        let authored = self.text.get();
        draft.baseline = authored.clone();
        draft.text = if authored.is_empty() {
            TextBuffer::new("{}")
        } else {
            authored
        };
        draft.has_draft = true;
    }

    /// Commits the draft when it validates. Returns whether it committed.
    fn commit_draft(&self) -> bool {
        let current = self.current_text();
        if !validate(&current.string_value()).is_acceptable() {
            return false;
        }
        let normalized_text = normalized_commit_text(&current.string_value());
        let authored = self.text.get();
        let normalized = if normalized_text == authored.string_value() {
            authored
        } else if normalized_text == current.string_value() {
            current
        } else {
            TextBuffer::new(&normalized_text)
        };
        if self.text.get() != normalized {
            self.text.set(normalized.clone());
            if let Some(handler) = self.on_commit.as_ref() {
                handler(normalized.clone());
            }
        }
        let mut draft = self
            .draft
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        draft.text = normalized;
        draft.has_draft = false;
        true
    }

    /// Restores the edit baseline.
    fn revert_draft(&self) {
        {
            let mut draft = self
                .draft
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            draft.text = draft.baseline.clone();
            draft.has_draft = true;
        }
        self.commit_draft();
    }

    /// Pretty-prints the current text when it is valid.
    fn format_draft(&self) {
        if let Some(pretty) = pretty_printed(&self.current_text().string_value()) {
            let mut draft = self
                .draft
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            draft.text = TextBuffer::new(&pretty);
            draft.has_draft = true;
        }
    }

    /// Applies the expanded modal's text and closes it.
    fn apply_expanded(&self) {
        let expanded = self
            .draft
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .expanded
            .clone();
        if !validate(&expanded.string_value()).is_acceptable() {
            return;
        }
        {
            let mut draft = self
                .draft
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            draft.text = expanded;
            draft.has_draft = true;
        }
        if self.commit_draft() {
            self.draft
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .is_expanded = false;
        }
    }

    fn draft_binding(&self) -> Binding<TextBuffer> {
        Binding::new(
            {
                let draft = self.draft.clone();
                move || {
                    let draft = draft
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    draft.current().clone()
                }
            },
            {
                let draft = self.draft.clone();
                move |value: TextBuffer| {
                    let mut draft = draft
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    draft.text = value;
                    draft.has_draft = true;
                }
            },
        )
    }
}

impl View for JsonField {
    fn body(&self, cx: &Scope) -> AnyView {
        // The edit session outlives every body: callers rebuild their
        // `JsonField` value on each recomposition, so the draft lives in this
        // scope. Swift keeps it in `@State`.
        let draft = cx.remember(|| self.draft.clone());
        let draft_handle = draft.clone();
        let field = JsonField {
            text: self.text.clone(),
            placeholder: self.placeholder.clone(),
            min_height: self.min_height,
            is_enabled: self.is_enabled,
            labels: self.labels.clone(),
            on_commit: self.on_commit.clone(),
            draft,
        };
        let validation = field.validation();
        // The presented flag must be reactive: writing it from the Expand
        // button has to recompose this body so the `Modal` presents.
        let is_expanded = cx.state(|| {
            draft_handle
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .is_expanded
        });

        let format = {
            let field = field.clone();
            Button::icon_with_options(
                ButtonIconSource::resource(
                    crate::primitives::media::ui_icon_catalog::action::format(),
                ),
                Some(12.0),
                None,
                ButtonRole::Normal,
                self.is_enabled && validation.is_acceptable(),
                false,
                false,
                Some(self.labels.format.clone()),
                move || field.format_draft(),
            )
            .button_style(GhostButtonStyle)
        };
        let revert = {
            let field = field.clone();
            let dirty = {
                let draft = self
                    .draft
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                draft.current() != &draft.baseline
            };
            Button::icon_with_options(
                ButtonIconSource::resource(
                    crate::primitives::media::ui_icon_catalog::action::reset(),
                ),
                Some(12.0),
                None,
                ButtonRole::Normal,
                self.is_enabled && dirty,
                false,
                false,
                Some(self.labels.revert.clone()),
                move || field.revert_draft(),
            )
            .button_style(GhostButtonStyle)
        };
        let expand = {
            let field = field.clone();
            let expanded_flag = is_expanded.clone();
            Button::icon_with_options(
                ButtonIconSource::resource(
                    crate::primitives::media::ui_icon_catalog::navigation::expand(),
                ),
                Some(12.0),
                None,
                ButtonRole::Normal,
                self.is_enabled,
                false,
                false,
                Some(self.labels.expand.clone()),
                move || {
                    let current = field.current_text();
                    let mut draft = field
                        .draft
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    draft.expanded = current;
                    draft.is_expanded = true;
                    drop(draft);
                    expanded_flag.set(true);
                },
            )
            .button_style(GhostButtonStyle)
        };

        let mut header: Vec<AnyView> = vec![AnyView::new(
            Text::new("JSON")
                .semantic_font(SemanticFontRef::caption())
                .semantic_foreground(SemanticColorRef::on_surface_muted()),
        )];
        if validation.is_acceptable() {
            header.push(AnyView::new(
                Text::new("✓")
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::success()),
            ));
        }
        header.push(AnyView::new(crate::Spacer::new(0.0)));
        header.push(AnyView::new(format));
        header.push(AnyView::new(revert));
        header.push(AnyView::new(expand));

        let draft = draft_handle.clone();
        let submit = {
            let field = field.clone();
            Rc::new(move || {
                field.commit_draft();
            }) as Rc<dyn Fn()>
        };
        let focus = {
            let field = field.clone();
            Rc::new(move || field.begin_editing()) as Rc<dyn Fn()>
        };
        let blur = {
            let field = field.clone();
            let draft = draft.clone();
            Rc::new(move || {
                let expanded = draft
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .is_expanded;
                if !expanded {
                    field.commit_draft();
                }
            }) as Rc<dyn Fn()>
        };

        let editor = ResizableTextArea::new(
            self.placeholder.clone(),
            self.draft_binding(),
            self.min_height,
            360.0,
            !self.is_enabled,
            self.labels.resize.clone(),
            TextFieldCodeEditing {
                shows_line_numbers: true,
                indentation_width: Some(2),
                ..Default::default()
            },
            Some(submit),
            Some(focus),
            Some(blur),
        );

        let mut column: Vec<AnyView> = vec![
            AnyView::new(Row::new(
                VerticalAlignment::Center,
                TOOLBAR_SPACING,
                AnyView::new(TupleView::new(header)),
            )),
            AnyView::new(RestorableEditor {
                is_enabled: self.is_enabled,
                is_acceptable: validation.is_acceptable(),
                editor: AnyView::new(editor),
            }),
        ];
        if let JsonFieldValidation::Invalid(message) = &validation {
            column.push(AnyView::new(
                Text::new(message.clone())
                    .line_limit(Some(2))
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::error()),
            ));
        }

        // Reading the flag here subscribes this body, so a write from Expand
        // recomposes it and the `Modal` presents; the draft keeps the mirror
        // the buttons consult.
        let _ = is_expanded.get();
        let expanded_binding = Binding::new(
            {
                let flag = is_expanded.clone();
                move || flag.get()
            },
            {
                let draft = draft_handle.clone();
                let flag = is_expanded.clone();
                move |value: bool| {
                    draft
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .is_expanded = value;
                    flag.set(value);
                }
            },
        );
        let modal_field = field.clone();
        // The expanded draft is reactive state: the modal's footer must follow
        // it so Apply enables only while the text parses.
        let expanded_state = cx.state(|| {
            draft_handle
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .expanded
                .clone()
        });
        let expanded_text = {
            let mirror = draft_handle.clone();
            let state = expanded_state.clone();
            Binding::new(
                {
                    let state = state.clone();
                    move || state.get()
                },
                move |value: TextBuffer| {
                    mirror
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .expanded = value.clone();
                    state.set(value);
                },
            )
        };
        let labels = self.labels.clone();
        // The dialog is named by the expand label, the way Swift labels its
        // `json-expanded-editor` panel with `labels.expand`.
        let modal_options = ModalOptions {
            label: labels.expand.clone(),
            ..ModalOptions::default()
        };
        let modal = Modal::new(
            expanded_binding,
            AnyView::new(JsonExpandedEditor {
                text: expanded_text,
                draft: expanded_state.clone(),
                labels,
                apply: {
                    let field = modal_field.clone();
                    Rc::new(move || field.apply_expanded())
                },
                cancel: {
                    let field = modal_field.clone();
                    let flag = is_expanded.clone();
                    Rc::new(move || {
                        field
                            .draft
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .is_expanded = false;
                        flag.set(false);
                    })
                },
            }),
        )
        .options(modal_options);

        column.push(AnyView::new(modal));
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                4.0,
                AnyView::new(TupleView::new(column)),
            )
            .frame_min_width(0.0),
        )
    }
}

impl Clone for JsonField {
    fn clone(&self) -> Self {
        self.snapshot()
    }
}

/// The editor surface: sunken fill, commit-state border and clipping.
struct RestorableEditor {
    is_enabled: bool,
    is_acceptable: bool,
    editor: AnyView,
}

impl PrimitiveView for RestorableEditor {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_clips_to_bounds(true);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_overflow(cove_core::scene::Overflow::Hidden);
        Some(layout)
    }

    fn update_layout(&self, layout: &cove_core::scene::LayoutNode) {
        layout.set_overflow(cove_core::scene::Overflow::Hidden);
    }

    fn update_node(&self, node: &Node) {
        let theme = theme_for_node(node);
        let state = InputChromeState::new(self.is_enabled).error(!self.is_acceptable);
        node.set_background_color(Some(theme.inputs.background_for(state)));
        node.set_corner_radius(theme.inputs.radius);
        node.set_border_color(Some(theme.inputs.border_for(state)));
        node.set_border_width(theme.inputs.border_width_for(state));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.editor.clone()]
    }
}

impl_primitive_view!(RestorableEditor);

/// The expanded editor shown in the modal.
struct JsonExpandedEditor {
    text: Binding<TextBuffer>,
    /// The observed draft, read by the footer so Apply tracks validity.
    draft: cove_core::Signal<TextBuffer>,
    labels: JsonFieldLabels,
    apply: Rc<dyn Fn()>,
    cancel: Rc<dyn Fn()>,
}

impl View for JsonExpandedEditor {
    fn body(&self, _cx: &Scope) -> AnyView {
        // A body, not a primitive's `children`, so the footer follows the
        // draft: Apply enables only once the text parses.
        AnyView::new(JsonExpandedEditorBody {
            text: self.text.clone(),
            draft: self.draft.clone(),
            labels: self.labels.clone(),
            apply: self.apply.clone(),
            cancel: self.cancel.clone(),
        })
    }
}

struct JsonExpandedEditorBody {
    text: Binding<TextBuffer>,
    draft: cove_core::Signal<TextBuffer>,
    labels: JsonFieldLabels,
    apply: Rc<dyn Fn()>,
    cancel: Rc<dyn Fn()>,
}

impl PrimitiveView for JsonExpandedEditorBody {
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
        let editor = ResizableTextArea::new(
            "{}",
            self.text.clone(),
            320.0,
            720.0,
            false,
            self.labels.resize.clone(),
            TextFieldCodeEditing {
                shows_line_numbers: true,
                indentation_width: Some(2),
                ..Default::default()
            },
            None,
            None,
            None,
        );
        let apply = self.apply.clone();
        let cancel = self.cancel.clone();
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                8.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(AnyView::new(editor).flex(1.0)),
                    AnyView::new(Row::new(
                        VerticalAlignment::Center,
                        8.0,
                        AnyView::new(TupleView::new(vec![
                            AnyView::new(crate::Spacer::new(0.0)),
                            AnyView::new(
                                Button::with_options(
                                    AnyView::new(Text::new(self.labels.cancel.clone())),
                                    move || cancel(),
                                    ButtonRole::Normal,
                                    true,
                                    false,
                                    false,
                                    None,
                                )
                                .button_style(GhostButtonStyle)
                                .debug_name("json-cancel"),
                            ),
                            AnyView::new(
                                Button::with_options(
                                    AnyView::new(Text::new(self.labels.apply.clone())),
                                    move || apply(),
                                    ButtonRole::Normal,
                                    // The draft decides: invalid text cannot commit, so
                                    // Apply announces itself as disabled (Swift:
                                    // `isEnabled: validation.isAcceptable`).
                                    validate(&self.draft.get().string_value()).is_acceptable(),
                                    false,
                                    false,
                                    None,
                                )
                                .debug_name("json-apply"),
                            ),
                        ])),
                    )),
                ])),
            )
            .padding(16.0)
            .frame_min_width(0.0)
            .debug_name("json-expanded-editor"),
        )]
    }
}
impl_primitive_view!(JsonExpandedEditorBody);

impl JsonField {
    /// Whether the current text may be committed.
    pub fn is_acceptable(&self) -> bool {
        self.validation().is_acceptable()
    }

    /// Pretty-prints the current text, when it is valid.
    pub fn formatted(&self) -> Option<String> {
        pretty_printed(&self.current_text().string_value())
    }
}
