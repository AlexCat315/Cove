//! Translation of Primitives/Controls/FormValidation.swift.
use cove_core::scene::{ObservableStateRegistrar, StateAccessKey};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormIssue {
    pub field_id: String,
    pub label: String,
    pub message: String,
}

impl FormIssue {
    pub fn new(field_id: impl Into<String>, label: impl Into<String>, message: impl Into<String>) -> Self {
        Self { field_id: field_id.into(), label: label.into(), message: message.into() }
    }
    pub fn id(&self) -> &str {
        &self.field_id
    }
}

/// Rules read application-owned values when validation runs, avoiding a parallel field-value model.
#[derive(Clone)]
pub struct FormRule {
    pub field_id: String,
    pub label: String,
    pub validate: Rc<dyn Fn() -> Option<String>>,
}

impl FormRule {
    pub fn new(field_id: impl Into<String>, label: impl Into<String>, validate: impl Fn() -> Option<String> + 'static) -> Self {
        Self { field_id: field_id.into(), label: label.into(), validate: Rc::new(validate) }
    }

    pub fn required(field_id: impl Into<String>, label: impl Into<String>, value: impl Fn() -> String + 'static) -> Self {
        Self::new(field_id, label, move || {
            if value().trim().is_empty() { Some("This field is required.".to_string()) } else { None }
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormSubmissionState {
    Editing,
    Submitting,
    Succeeded,
    Failed(String),
}

#[derive(Clone)]
struct FormSession {
    issues: Vec<FormIssue>,
    submission: FormSubmissionState,
    submit_count: u32,
}

impl Default for FormSession {
    fn default() -> Self {
        Self { issues: Vec::new(), submission: FormSubmissionState::Editing, submit_count: 0 }
    }
}

/// Transient validation/submission state. Authored values and persistence stay in the application.
pub struct FormController {
    session: RefCell<FormSession>,
    registrar: ObservableStateRegistrar,
}

impl Default for FormController {
    fn default() -> Self {
        Self::new()
    }
}

impl FormController {
    pub fn new() -> Self {
        Self { session: RefCell::new(FormSession::default()), registrar: ObservableStateRegistrar::new() }
    }

    pub fn issues(&self) -> Vec<FormIssue> {
        self.registrar.access(&StateAccessKey::text("form"));
        self.session.borrow().issues.clone()
    }

    pub fn submission(&self) -> FormSubmissionState {
        self.registrar.access(&StateAccessKey::text("form"));
        self.session.borrow().submission.clone()
    }

    pub fn submit_count(&self) -> u32 {
        self.registrar.access(&StateAccessKey::text("form"));
        self.session.borrow().submit_count
    }

    pub fn is_submitting(&self) -> bool {
        self.submission() == FormSubmissionState::Submitting
    }

    pub fn error(&self, field_id: &str) -> Option<String> {
        self.issues().into_iter().find(|issue| issue.field_id == field_id).map(|issue| issue.message)
    }

    pub fn validate(&self, rules: &[FormRule]) -> bool {
        let unique: std::collections::HashSet<&str> = rules.iter().map(|rule| rule.field_id.as_str()).collect();
        assert_eq!(unique.len(), rules.len(), "Form field IDs must be unique");
        let issues: Vec<FormIssue> = rules
            .iter()
            .filter_map(|rule| {
                let message = (rule.validate)()?;
                if message.is_empty() {
                    None
                } else {
                    Some(FormIssue::new(rule.field_id.clone(), rule.label.clone(), message))
                }
            })
            .collect();
        let is_empty = issues.is_empty();
        self.session.borrow_mut().issues = issues;
        self.registrar.invalidate(&StateAccessKey::text("form"));
        is_empty
    }

    pub fn begin_submit(&self, rules: &[FormRule]) -> bool {
        if self.is_submitting() {
            return false;
        }
        self.session.borrow_mut().submit_count += 1;
        if !self.validate(rules) {
            self.session.borrow_mut().submission = FormSubmissionState::Editing;
            self.registrar.invalidate(&StateAccessKey::text("form"));
            return false;
        }
        self.session.borrow_mut().submission = FormSubmissionState::Submitting;
        self.registrar.invalidate(&StateAccessKey::text("form"));
        true
    }

    pub fn finish_submit(&self, error: Option<String>) {
        if !self.is_submitting() {
            return;
        }
        self.session.borrow_mut().submission = match error {
            Some(message) => FormSubmissionState::Failed(message),
            None => FormSubmissionState::Succeeded,
        };
        self.registrar.invalidate(&StateAccessKey::text("form"));
    }

    pub fn reset(&self) {
        *self.session.borrow_mut() = FormSession::default();
        self.registrar.invalidate(&StateAccessKey::text("form"));
    }
}
