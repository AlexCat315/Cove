//! The sample data the mobile screens share.
//!
//! Swift source: `Examples/Mobile/Sources/AppContent/Model.swift`. Edits stay
//! for the session, so the model is a plain value behind `State` rather than a
//! persisted store.

use serde::{Deserialize, Serialize};

/// One row in the Projects list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub updated: String,
    pub progress: f64,
}

impl Project {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        kind: impl Into<String>,
        updated: impl Into<String>,
        progress: f64,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind: kind.into(),
            updated: updated.into(),
            progress,
        }
    }
}

/// Appearance choices offered by the Settings sheet, in presentation order.
pub const APPEARANCE_CHOICES: [&str; 3] = ["System", "Light", "Dark"];

/// The sessional model behind every mobile screen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MobileModel {
    pub projects: Vec<Project>,
    pub notifications: bool,
    pub sync_over_cellular: bool,
    pub reduce_motion: bool,
    pub appearance: String,
}

impl Default for MobileModel {
    fn default() -> Self {
        Self::new()
    }
}

impl MobileModel {
    pub fn new() -> Self {
        Self {
            projects: vec![
                Project::new("crystal", "Crystal Rush", "Game", "2 min ago", 0.72),
                Project::new("harbor", "Harbor Night", "Animation", "1 h ago", 0.4),
                Project::new("trailer", "Launch Trailer", "Video", "Yesterday", 0.9),
                Project::new("coveui", "CoveUI", "Code", "Mon", 0.55),
            ],
            notifications: true,
            sync_over_cellular: false,
            reduce_motion: false,
            appearance: "System".into(),
        }
    }

    /// Projects whose name contains `query` case-insensitively.
    ///
    /// A blank query returns every project, matching the Swift screen's
    /// `text.isEmpty ? projects : filter`.
    pub fn matches(&self, query: &str) -> Vec<Project> {
        let text = query.trim();
        if text.is_empty() {
            return self.projects.clone();
        }
        let needle = text.to_lowercase();
        self.projects
            .iter()
            .filter(|project| project.name.to_lowercase().contains(&needle))
            .cloned()
            .collect()
    }

    /// The first two projects, as the Today screen shows them.
    pub fn continue_projects(&self) -> Vec<Project> {
        self.projects.iter().take(2).cloned().collect()
    }

    /// Sets the sheet's selection and closes it, as `choiceButton` does.
    pub fn select_appearance(&mut self, choice: &str) {
        if APPEARANCE_CHOICES.contains(&choice) {
            self.appearance = choice.to_string();
        }
    }
}

#[cfg(test)]
mod checks {
    use super::*;

    #[test]
    fn defaults_match_the_swift_sample_data() {
        let model = MobileModel::new();
        assert_eq!(model.projects.len(), 4);
        assert_eq!(model.projects[0].id, "crystal");
        assert_eq!(model.projects[0].name, "Crystal Rush");
        assert_eq!(model.projects[0].kind, "Game");
        assert_eq!(model.projects[0].updated, "2 min ago");
        assert_eq!(model.projects[0].progress, 0.72);
        assert_eq!(model.projects[3].id, "coveui");
        assert_eq!(model.projects[3].name, "CoveUI");
        assert_eq!(model.projects[3].progress, 0.55);
        assert!(model.notifications);
        assert!(!model.sync_over_cellular);
        assert!(!model.reduce_motion);
        assert_eq!(model.appearance, "System");
    }

    #[test]
    fn search_is_case_insensitive_and_blank_matches_all() {
        let model = MobileModel::new();
        assert_eq!(model.matches("").len(), 4);
        assert_eq!(model.matches("   ").len(), 4, "whitespace is trimmed first");
        assert_eq!(model.matches("crystal").len(), 1);
        assert_eq!(model.matches("CRYSTAL").len(), 1);
        assert_eq!(model.matches("r").len(), 4, "every name contains 'r'");
        assert!(model.matches("nope").is_empty());
    }

    #[test]
    fn today_shows_the_first_two_projects() {
        let model = MobileModel::new();
        let shown = model.continue_projects();
        assert_eq!(shown.len(), 2);
        assert_eq!(shown[0].id, "crystal");
        assert_eq!(shown[1].id, "harbor");
    }

    #[test]
    fn appearance_selection_accepts_only_offered_choices() {
        let mut model = MobileModel::new();
        model.select_appearance("Dark");
        assert_eq!(model.appearance, "Dark");
        model.select_appearance("Purple");
        assert_eq!(model.appearance, "Dark", "an unknown choice is ignored");
        assert_eq!(APPEARANCE_CHOICES, ["System", "Light", "Dark"]);
    }
}
