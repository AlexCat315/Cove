//! Translation of Primitives/Navigation/Stepper/Values.swift.
use crate::primitives::feedback::StatusTone;

/// Authored workflow outcomes are independent from the current destination.
/// Going back to an earlier step never silently rewrites a later outcome.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StepStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
    Failed,
}

impl StepStatus {
    pub const ALL: [Self; 4] = [Self::Pending, Self::InProgress, Self::Completed, Self::Failed];

    pub fn description(self) -> &'static str {
        match self {
            Self::Pending => "Not completed",
            Self::InProgress => "In progress",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
        }
    }

    pub fn tone(self) -> StatusTone {
        match self {
            Self::Pending => StatusTone::Neutral,
            Self::InProgress => StatusTone::Info,
            Self::Completed => StatusTone::Success,
            Self::Failed => StatusTone::Danger,
        }
    }
}

/// One authored workflow step.
#[derive(Clone, Debug, PartialEq)]
pub struct StepperItem<Id: PartialEq + Clone> {
    pub id: Id,
    pub title: String,
    pub detail: String,
    pub status: StepStatus,
    pub is_enabled: bool,
}

impl<Id: PartialEq + Clone> StepperItem<Id> {
    pub fn new(id: Id, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            detail: String::new(),
            status: StepStatus::Pending,
            is_enabled: true,
        }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    pub fn status(mut self, status: StepStatus) -> Self {
        self.status = status;
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StepperAxis {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StepperOptions {
    pub axis: StepperAxis,
    pub is_enabled: bool,
    pub allows_wrap: bool,
    pub label: String,
    /// Width budget one horizontal step needs before the component switches to
    /// its vertical presentation. Titles longer than the budget should raise
    /// this instead of letting the step compress.
    pub minimum_horizontal_step_width: f32,
}

impl Default for StepperOptions {
    fn default() -> Self {
        Self {
            axis: StepperAxis::Horizontal,
            is_enabled: true,
            allows_wrap: false,
            label: "Steps".to_string(),
            minimum_horizontal_step_width: 148.0,
        }
    }
}

impl StepperOptions {
    pub fn new() -> Self {
        Self::default()
    }
}
