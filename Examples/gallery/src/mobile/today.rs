//! The Today screen and the project card it shares with the Projects list.
//!
//! Swift source: `Examples/Mobile/Sources/AppContent/Today.swift`
//! (`TodayScreen`, `ProjectCard`).

use cove_core::Binding;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui_compose::modifiers::layout::{FlexExt, PaddingExt};
use cove_ui_compose::modifiers::style::AppearanceExt;
use cove_ui_compose::primitives::data::avatar::Avatar;
use cove_ui_compose::primitives::feedback::{Badge, ProgressView};
use cove_ui_compose::primitives::layout::{ScrollAxis, ScrollView};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticStyleViewExt};
use cove_ui_compose::{AnyView, BoxView, TupleView, View};

use crate::mobile::model::{MobileModel, Project};

/// The three summary metrics, in the order the Swift screen renders them.
pub const SUMMARY_METRICS: [(&str, &str); 3] =
    [("4", "Projects"), ("12", "Renders"), ("3", "Reviews")];

/// `TodayScreen`.
pub struct TodayScreen {
    pub model: Binding<MobileModel>,
}

impl TodayScreen {
    pub fn new(model: Binding<MobileModel>) -> Self {
        Self { model }
    }

    /// The first two projects, as "Continue" shows them.
    pub fn continue_projects(&self) -> Vec<Project> {
        self.model.get().continue_projects()
    }
}

impl View for TodayScreen {
    fn body(&self) -> AnyView {
        let summary = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::NoWrap,
            12.0,
            AnyView::new(TupleView::new(
                SUMMARY_METRICS
                    .iter()
                    .map(|(value, label)| AnyView::new(SummaryMetric::new(value, label)))
                    .collect(),
            )),
        ));

        let projects = self.continue_projects();
        let mut children = vec![
            AnyView::new(Text::new("Today")),
            summary,
            AnyView::new(Text::new("Continue")),
        ];
        for project in projects {
            children.push(AnyView::new(ProjectCard::new(project)));
        }

        AnyView::new(
            ScrollView::new(
                ScrollAxis::Vertical,
                AnyView::new(
                    BoxView::new(
                        FlexDirection::Column,
                        Align::FlexStart,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        16.0,
                        AnyView::new(TupleView::new(children)),
                    )
                    .padding_edges(20.0, 8.0)
                    .flex_with(0.0, 0.0, None),
                ),
            )
            .flex_with(1.0, 1.0, Some(0.0)),
        )
    }
}

/// One summary metric tile.
struct SummaryMetric {
    value: &'static str,
    label: &'static str,
}

impl SummaryMetric {
    fn new(value: &'static str, label: &'static str) -> Self {
        Self { value, label }
    }
}

impl View for SummaryMetric {
    fn body(&self) -> AnyView {
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::FlexStart,
                Justify::FlexStart,
                Wrap::NoWrap,
                2.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(Text::new(self.value)),
                    AnyView::new(
                        Text::new(self.label)
                            .semantic_foreground(SemanticColorRef::on_surface_muted()),
                    ),
                ])),
            )
            .padding(16.0)
            .flex_with(1.0, 1.0, Some(0.0))
            .semantic_background(SemanticColorRef::surface_raised())
            .corner_radius(16.0),
        )
    }
}

/// `ProjectCard`, shared by Today and Projects.
pub struct ProjectCard {
    pub project: Project,
}

impl ProjectCard {
    pub fn new(project: Project) -> Self {
        Self { project }
    }

    /// `Edited {updated}`
    pub fn edited_line(&self) -> String {
        format!("Edited {}", self.project.updated)
    }
}

impl View for ProjectCard {
    fn body(&self) -> AnyView {
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::FlexStart,
                Justify::FlexStart,
                Wrap::NoWrap,
                10.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(BoxView::new(
                        FlexDirection::Row,
                        Align::Center,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        8.0,
                        AnyView::new(TupleView::new(vec![
                            AnyView::new(
                                Text::new(self.project.name.clone()).flex_with(1.0, 1.0, None),
                            ),
                            AnyView::new(Badge::new(self.project.kind.clone())),
                        ])),
                    )),
                    AnyView::new(ProgressView::new(Some(self.project.progress))),
                    AnyView::new(
                        Text::new(self.edited_line())
                            .semantic_foreground(SemanticColorRef::on_surface_muted()),
                    ),
                ])),
            )
            .padding(16.0)
            .semantic_background(SemanticColorRef::surface_raised())
            .corner_radius(16.0),
        )
    }
}

/// The `Avatar` used by the Projects list rows.
pub fn project_avatar(project: &Project) -> Avatar {
    Avatar::new(project.name.clone())
}

#[cfg(test)]
mod checks {
    use super::*;

    fn model() -> MobileModel {
        MobileModel::new()
    }

    #[test]
    fn summary_metrics_match_the_swift_screen() {
        assert_eq!(
            SUMMARY_METRICS,
            [("4", "Projects"), ("12", "Renders"), ("3", "Reviews")]
        );
    }

    #[test]
    fn today_continues_with_the_first_two_projects() {
        let model = model();
        let binding = Binding::new(
            {
                let model = model.clone();
                move || model.clone()
            },
            |_| {},
        );
        let screen = TodayScreen::new(binding);
        let projects = screen.continue_projects();
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].name, "Crystal Rush");
        assert_eq!(projects[1].name, "Harbor Night");
    }

    #[test]
    fn project_card_formats_the_edited_line() {
        let project = Project::new("crystal", "Crystal Rush", "Game", "2 min ago", 0.72);
        let card = ProjectCard::new(project);
        assert_eq!(card.edited_line(), "Edited 2 min ago");
    }

    #[test]
    fn project_avatar_uses_the_project_name() {
        let project = Project::new("crystal", "Crystal Rush", "Game", "2 min ago", 0.72);
        let avatar = project_avatar(&project);
        // `Avatar::new` trims its name; the full name is retained here.
        assert!(!avatar.name.is_empty());
        assert_eq!(avatar.name, "Crystal Rush");
    }
}
