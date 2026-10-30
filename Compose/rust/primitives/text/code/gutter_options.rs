//! Translation of Primitives/Text/Code/GutterOptions.swift.
//!
//! Optional caller-owned document-session annotations. This value is
//! deliberately separate from source text, undo snapshots and persisted project
//! configuration.
use std::collections::BTreeSet;
use std::rc::Rc;

use cove_core::{Binding, Color};
use cove_imaging::BundleImageResource;
use crate::primitives::text::text_buffer::TextBuffer;

use super::folding_options::TextLineRebase;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextBreakpointState {
    #[default]
    Enabled,
    Disabled,
    Unverified,
}

impl TextBreakpointState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Enabled => "Enabled",
            Self::Disabled => "Disabled",
            Self::Unverified => "Unverified",
        }
    }
}

/// Zero-based logical source line, independent of wrapping and folding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextBreakpoint {
    pub line: usize,
    pub state: TextBreakpointState,
}

impl TextBreakpoint {
    pub fn new(line: usize) -> Self {
        Self { line, state: TextBreakpointState::Enabled }
    }

    pub fn with_state(line: usize, state: TextBreakpointState) -> Self {
        Self { line, state }
    }
}

/// Caller-owned document-session annotations.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextEditorAnnotations {
    pub breakpoints: Vec<TextBreakpoint>,
    pub collapsed_lines: BTreeSet<usize>,
}

impl TextEditorAnnotations {
    pub fn new() -> Self {
        Self::default()
    }

    /// Carry annotations across an edit; drops entries whose line disappeared.
    pub fn rebase(&mut self, previous: &TextBuffer, next: &TextBuffer) {
        if previous.has_same_contents(next) {
            return;
        }
        let Some(change) = TextLineRebase::new(previous, next) else {
            if !previous.has_same_contents(next) {
                *self = Self::default();
            }
            return;
        };
        let line_count = next.line_count();
        self.breakpoints = self
            .breakpoints
            .iter()
            .filter_map(|point| {
                let line = change.line(point.line)?;
                if line >= line_count { None } else { Some(TextBreakpoint { line, state: point.state }) }
            })
            .collect();
        self.collapsed_lines = self
            .collapsed_lines
            .iter()
            .filter_map(|line| change.line(*line))
            .filter(|line| *line < line_count)
            .collect();
    }
}

/// One interactive marker in the gutter.
#[derive(Clone)]
pub struct TextGutterMarker {
    pub id: String,
    pub line: usize,
    pub label: String,
    pub icon: BundleImageResource,
    pub color: Option<Color>,
    pub action: Rc<dyn Fn()>,
}

impl TextGutterMarker {
    pub fn new(
        id: impl Into<String>,
        line: usize,
        label: impl Into<String>,
        icon: BundleImageResource,
        color: Option<Color>,
        action: impl Fn() + 'static,
    ) -> Self {
        Self { id: id.into(), line, label: label.into(), icon, color, action: Rc::new(action) }
    }
}

/// Names are functions so applications can localize word order and line numbers.
pub struct TextGutterLabels {
    pub line_actions: Box<dyn Fn(usize) -> String>,
    pub add_breakpoint: Box<dyn Fn(usize) -> String>,
    pub remove_breakpoint: Box<dyn Fn(usize) -> String>,
    pub enable_breakpoint: Box<dyn Fn(usize) -> String>,
    pub disable_breakpoint: Box<dyn Fn(usize) -> String>,
    pub execution: Box<dyn Fn(usize) -> String>,
    pub breakpoint_state: Box<dyn Fn(TextBreakpointState) -> String>,
}

impl Default for TextGutterLabels {
    fn default() -> Self {
        Self {
            line_actions: Box::new(|line| format!("Line {} actions", line + 1)),
            add_breakpoint: Box::new(|line| format!("Add breakpoint at line {}", line + 1)),
            remove_breakpoint: Box::new(|line| format!("Remove breakpoint at line {}", line + 1)),
            enable_breakpoint: Box::new(|line| format!("Enable breakpoint at line {}", line + 1)),
            disable_breakpoint: Box::new(|line| format!("Disable breakpoint at line {}", line + 1)),
            execution: Box::new(|line| format!("Execution paused at line {}", line + 1)),
            breakpoint_state: Box::new(|state| state.label().to_string()),
        }
    }
}

/// Breakpoints belong to the caller's document session, never the source text.
/// Editing rebases them; callbacks can forward the resulting list to a debugger.
#[derive(Clone, Default)]
pub struct TextEditorGutter {
    pub breakpoints: Option<Binding<Vec<TextBreakpoint>>>,
    pub on_breakpoints_change: Option<Rc<dyn Fn(Vec<TextBreakpoint>)>>,
    pub markers: Vec<TextGutterMarker>,
    pub execution_line: Option<usize>,
    pub labels: std::rc::Rc<TextGutterLabels>,
}

impl TextEditorGutter {
    pub fn new() -> Self {
        Self { labels: Rc::new(TextGutterLabels::default()), ..Self::default() }
    }

    pub fn is_enabled(&self) -> bool {
        self.breakpoints.is_some() || !self.markers.is_empty() || self.execution_line.is_some()
    }
}
