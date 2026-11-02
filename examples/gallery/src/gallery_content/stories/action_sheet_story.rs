//! Overlay story: bottom action sheets.
//!
//! Swift source: `GalleryContent/Stories/Overlays/ActionSheetStory.swift`.
//!
//! The action list is the interesting part: a disabled action is skipped by
//! arrow navigation but still rendered, and the close control sits outside the
//! scrolling list so it stays reachable.

use cove_composition::Scope;
use cove_core::State;
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::controls::ButtonRole;
use cove_ui_compose::primitives::data::data_table::table_column::ComparisonResult;
use cove_ui_compose::primitives::media::ui_icon_catalog as icons;
use cove_ui_compose::primitives::overlays::modal::action_sheet::{
    ActionSheet, ActionSheetAction, ActionSheetQuestion,
};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, LayoutDebugExt, View};

use crate::gallery_content::catalog::GalleryOptions;

/// The number of shortcuts the "long action lists" section generates.
pub const SHORTCUT_COUNT: usize = 24;

/// `ActionSheetStory`: a four-command project sheet and a 24-command sheet.
pub struct ActionSheetStory {
    pub options: GalleryOptions,
    open: State<bool>,
    many_open: State<bool>,
    last_action: State<String>,
    export_enabled: State<bool>,
}

impl ActionSheetStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            open: State::new(false),
            many_open: State::new(false),
            last_action: State::new("None".to_owned()),
            export_enabled: State::new(false),
        }
    }

    pub fn is_open(&self) -> bool {
        self.open.get()
    }

    pub fn set_open(&self, open: bool) {
        self.open.set(open);
    }

    pub fn last_action(&self) -> String {
        self.last_action.get()
    }

    pub fn record(&self, action: impl Into<String>) {
        self.last_action.set(action.into());
    }

    pub fn export_enabled(&self) -> bool {
        self.export_enabled.get()
    }

    pub fn set_export_enabled(&self, enabled: bool) {
        self.export_enabled.set(enabled);
    }

    /// The project's four commands, in authored order.
    ///
    /// `Export` is gated on the story's checkbox so the disabled styling and the
    /// arrow-key skip are both visible without changing the list.
    pub fn project_actions(&self) -> Vec<ActionSheetAction> {
        let record = self.last_action.clone();
        vec![
            ActionSheetAction::new("duplicate", "Duplicate project", {
                let record = record.clone();
                move || record.set("Duplicated".to_owned())
            })
            .icon(icons::action::copy()),
            ActionSheetAction::new("archive", "Archive project", {
                let record = record.clone();
                move || record.set("Archived".to_owned())
            })
            .icon(icons::file::archive()),
            ActionSheetAction::new("export", "Export project", {
                let record = record.clone();
                move || record.set("Exported".to_owned())
            })
            .icon(icons::action::download())
            .enabled(self.export_enabled.get()),
            ActionSheetAction::new("delete", "Delete project", move || {
                record.set("Deleted".to_owned())
            })
            .icon(icons::action::trash())
            .role(ButtonRole::Destructive),
        ]
    }

    pub fn project_question(&self) -> ActionSheetQuestion {
        ActionSheetQuestion::try_new(
            "Aurora project",
            "Choose an action for this project.",
            self.project_actions(),
            "Close project actions",
        )
        .expect("unique action sheet action IDs")
    }

    /// The generated shortcut commands, numbered from one.
    pub fn shortcut_actions(&self) -> Vec<ActionSheetAction> {
        (1..=SHORTCUT_COUNT)
            .map(|index| {
                let record = self.last_action.clone();
                ActionSheetAction::new(
                    format!("shortcut-{index}"),
                    format!("Workspace shortcut {index}"),
                    move || record.set(format!("Shortcut {index}")),
                )
            })
            .collect()
    }
}

impl View for ActionSheetStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled = self.options.is_enabled;
        let trigger = Text::new("Project actions");

        let sheet = ActionSheet::new(self.open.binding(), self.project_question(), trigger)
            .is_enabled(enabled);

        let long_sheet = ActionSheet::new(
            self.many_open.binding(),
            ActionSheetQuestion::try_new(
                "Workspace shortcuts",
                "The list scrolls while the close control remains visible.",
                self.shortcut_actions(),
                "Close workspace actions",
            )
            .expect("unique action sheet action IDs"),
            Text::new("Browse workspace actions"),
        )
        .is_enabled(enabled);

        let summary = Text::new(format!("Last action: {}", self.last_action()))
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted());

        AnyView::new(cove_ui_compose::BoxView::new(
            cove_core::scene::FlexDirection::Column,
            cove_core::scene::Align::Stretch,
            cove_core::scene::Justify::FlexStart,
            cove_core::scene::Wrap::NoWrap,
            24.0,
            AnyView::new(cove_ui_compose::TupleView::new(vec![
                AnyView::new(
                    AnyView::new(Text::new("Action sheets"))
                        .debug_name("gallery-action-sheet-story"),
                ),
                AnyView::new(sheet),
                AnyView::new(long_sheet),
                AnyView::new(summary),
            ])),
        ))
    }

    fn tag(&self) -> &'static str {
        "ActionSheetStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; open, many_open, last_action, export_enabled)
    }
}

/// The ids arrow navigation can reach: enabled actions only.
pub fn navigable_ids(actions: &[ActionSheetAction]) -> Vec<String> {
    actions
        .iter()
        .filter(|action| action.is_enabled)
        .map(|action| action.id.clone())
        .collect()
}

/// The first action in the list, enabled or not, so a caller can tell whether a
/// sheet opens on a usable command.
pub fn first_available(actions: &[ActionSheetAction]) -> Option<&ActionSheetAction> {
    actions.iter().find(|action| action.is_enabled)
}

/// Whether a command should read as destructive.
pub fn is_destructive(action: &ActionSheetAction) -> bool {
    action.role == ButtonRole::Destructive
}

/// The comparison a sheet uses to order its actions, matching the table's
/// shared `ComparisonResult` convention.
pub fn alphabetical(left: &str, right: &str) -> ComparisonResult {
    match left.cmp(right) {
        std::cmp::Ordering::Less => ComparisonResult::Ascending,
        std::cmp::Ordering::Greater => ComparisonResult::Descending,
        std::cmp::Ordering::Equal => ComparisonResult::Same,
    }
}
