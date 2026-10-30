//! Translation of Primitives/Controls/Form.swift.
use cove_core::scene::{AccessibilitySemantics, AccessibilityState, Align, FlexDirection, Justify, Wrap};
use crate::{AnyView, BoxView, TupleView, View, ViewExt};
use crate::adaptive::ContainerSizeReader;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt};
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::style::SemanticStyleViewExt;
use crate::primitives::controls::field_label::FieldLabel;
use crate::primitives::controls::form::label_placement::FieldLabelPlacement;
use crate::primitives::controls::form::label_scope::FormLabelScope;
use crate::primitives::controls::form_validation::FormController;
use crate::primitives::layout::grid::{Grid, GridColumn};
use crate::primitives::text::Text;
use crate::theme::{SemanticColorRef, SemanticFontRef, TextScalerReader};
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct FormLayout {
    pub columns: usize,
    pub label_placement: FieldLabelPlacement,
    pub label_width: f32,
    /// Preferred value-slot width used for column count and label reflow.
    /// Below this width a single stacked field fills the available width.
    pub min_value_width: f32,
    pub spacing: f32,
}

impl Default for FormLayout {
    fn default() -> Self {
        Self { columns: 1, label_placement: FieldLabelPlacement::Adaptive, label_width: 120.0, min_value_width: 220.0, spacing: 18.0 }
    }
}

impl FormLayout {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate(&mut self) {
        self.columns = self.columns.max(1);
        self.label_width = if self.label_width.is_finite() { self.label_width.max(0.0) } else { 120.0 };
        self.min_value_width = if self.min_value_width.is_finite() { self.min_value_width.max(1.0) } else { 220.0 };
        self.spacing = if self.spacing.is_finite() { self.spacing.max(0.0) } else { 18.0 };
    }
}

#[derive(Clone, Debug, Default)]
pub struct FormFieldPresentation {
    pub is_required: bool,
    pub help: String,
    pub error: Option<String>,
    pub is_visible: bool,
}

#[derive(Clone)]
pub struct FormField {
    pub id: String,
    pub label: String,
    pub content: AnyView,
    pub presentation: FormFieldPresentation,
}

impl FormField {
    pub fn new(id: impl Into<String>, label: impl Into<String>, content: impl View + 'static) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            content: AnyView::new(content),
            presentation: FormFieldPresentation { is_visible: true, ..Default::default() },
        }
    }

    pub fn configure(mut self, configure: impl FnOnce(&mut FormFieldPresentation)) -> Self {
        configure(&mut self.presentation);
        self
    }
}

/// Layout and field descriptions; values, validation policy and submission remain application-owned.
pub struct Form {
    pub fields: Vec<FormField>,
    pub controller: Option<Rc<FormController>>,
    pub layout: FormLayout,
    footer: AnyView,
}

impl Form {
    pub fn new(
        fields: Vec<FormField>,
        controller: Option<Rc<FormController>>,
        configure: impl FnOnce(&mut FormLayout),
        footer: impl View + 'static,
    ) -> Self {
        let mut layout = FormLayout::default();
        configure(&mut layout);
        layout.validate();
        let unique: std::collections::HashSet<&str> = fields.iter().map(|field| field.id.as_str()).collect();
        assert_eq!(unique.len(), fields.len(), "Form field IDs must be unique");
        Self { fields, controller, layout, footer: AnyView::new(footer) }
    }

    pub fn without_footer(fields: Vec<FormField>, controller: Option<Rc<FormController>>, configure: impl FnOnce(&mut FormLayout)) -> Self {
        Self::new(fields, controller, configure, crate::EmptyView)
    }
}

impl View for Form {
    fn body(&self) -> AnyView {
        let visible: Vec<FormField> = self.fields.iter().filter(|field| field.presentation.is_visible).cloned().collect();
        let layout = self.layout.clone();
        let controller = self.controller.clone();
        let footer = self.footer.clone();
        let content = ContainerSizeReader::new(move |size| {
            let columns = layout.columns.min(
                (((size.width + layout.spacing) / (layout.min_value_width + layout.spacing)).floor() as usize).max(1),
            );
            let mut stack: Vec<AnyView> = Vec::new();
            if let Some(controller) = &controller {
                let issues = controller.issues();
                if !issues.is_empty() {
                    let message = issues.iter().map(|issue| format!("{}: {}", issue.label, issue.message)).collect::<Vec<_>>().join("\n");
                    stack.push(AnyView::new(
                        crate::primitives::feedback::Alert::new("Please check the form")
                            .message(message)
                            .tone(crate::primitives::feedback::StatusTone::Danger),
                    ));
                }
            }
            let mut cells: Vec<AnyView> = Vec::new();
            for field in &visible {
                let field_id = field.id.clone();
                let error = controller.as_ref().and_then(|controller| controller.error(&field_id)).or_else(|| field.presentation.error.clone());
                cells.push(AnyView::new(AnyView::new(FormFieldView { field: field.clone(), layout: layout.clone(), error }).id(field_id)));
            }
            stack.push(AnyView::new(Grid::new(
                vec![GridColumn::flexible(0.0, f32::INFINITY); columns],
                layout.spacing,
                layout.spacing,
                crate::Alignment::TopLeading,
                None,
                true,
                AnyView::new(TupleView::new(cells)),
            )));
            stack.push(AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexEnd,
                Wrap::Wrap,
                10.0,
                footer.clone(),
            )));
            AnyView::new(
                BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    layout.spacing,
                    AnyView::new(TupleView::new(stack)),
                ),
            )
        });
        AnyView::new(FormLabelScope::new(content))
    }
}

struct FormFieldView {
    field: FormField,
    layout: FormLayout,
    error: Option<String>,
}

impl View for FormFieldView {
    fn body(&self) -> AnyView {
        let field = self.field.clone();
        let layout = self.layout.clone();
        let error = self.error.clone();
        AnyView::new(TextScalerReader::new(move |scaler| {
            let field = field.clone();
            let layout = layout.clone();
            let error = error.clone();
            ContainerSizeReader::new(move |size| {
                let label_width = scaler.scale(layout.label_width);
                let leading = layout.label_placement.is_leading(size.width, label_width, layout.min_value_width, 7.0);
                let label = FieldLabel::new(field.label.clone())
                    .configure(|options| {
                        options.is_required = field.presentation.is_required;
                        options.target_id = Some(field.id.clone());
                        options.help = field.presentation.help.clone();
                    });
                let label = if leading {
                    AnyView::new(AnyView::new(label.frame_width(label_width)).flex(0.0).shrink(0.0))
                } else {
                    AnyView::new(AnyView::new(label).flex(0.0).shrink(0.0))
                };
                let field_label = field.label.clone();
                let field_id = field.id.clone();
                let field_help = field.presentation.help.clone();
                let field_required = field.presentation.is_required;
                let field_error = error.clone();
                let mut value_stack: Vec<AnyView> = vec![AnyView::new(field.content.clone().accessibility(move |semantics| {
                    semantics.label = field_label.clone();
                    semantics.identifier = field_id.clone();
                    semantics.help = field_error.clone().unwrap_or_else(|| field_help.clone());
                    semantics.state = AccessibilityState {
                        is_required: field_required,
                        is_invalid: field_error.is_some(),
                        ..Default::default()
                    };
                }))];
                if let Some(error) = &error {
                    value_stack.push(AnyView::new(
                        Text::new(error.clone())
                            .semantic_font(SemanticFontRef::caption())
                            .semantic_foreground(SemanticColorRef::error()),
                    ));
                } else if !field.presentation.help.is_empty() {
                    value_stack.push(AnyView::new(
                        Text::new(field.presentation.help.clone())
                            .semantic_font(SemanticFontRef::caption())
                            .semantic_foreground(SemanticColorRef::on_surface_muted()),
                    ));
                }
                let values = AnyView::new(
                    BoxView::new(
                        FlexDirection::Column,
                        Align::Stretch,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        5.0,
                        AnyView::new(TupleView::new(value_stack)),
                    )
                    .flex(if leading { 1.0 } else { 0.0 })
                    .shrink(1.0)
                    .flex_basis(if leading { Some(0.0) } else { None }),
                );
                AnyView::new(
                    BoxView::new(
                        if leading { FlexDirection::Row } else { FlexDirection::Column },
                        Align::Stretch,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        7.0,
                        AnyView::new(TupleView::new(vec![label, values])),
                    )
                    .frame_min_width(0.0),
                )
            })
        }))
    }
}
