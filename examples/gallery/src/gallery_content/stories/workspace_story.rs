//! `WorkspaceStory`: a docked editor layout with persistent panel sizing.
//!
//! Swift source: `GalleryContent/Stories/Layout/LayoutStories.swift`
//! (`WorkspaceStory`). The controller is story state, so dragging a tab or a
//! divider survives recomposition; Save keeps the encoded document and Restore
//! replaces the live one with it.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use cove_composition::Scope;
use cove_composition::ViewExt;
use cove_core::State;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui_addons::workspace::{
    WorkspaceAxisSizing, WorkspaceController, WorkspaceDocument, WorkspaceLayoutNode,
    WorkspacePanel, WorkspacePanelID, WorkspacePanelSizing, WorkspaceSlot, WorkspaceSlotID,
    WorkspaceSplitAxis, WorkspaceSplitFractions, WorkspaceTabGroup, WorkspaceTabGroupID,
    WorkspaceTheme, WorkspaceThemeEnvironment, WorkspaceView,
};
use cove_ui_compose::modifiers::layout::{LayoutFrameExt, PaddingExt};
use cove_ui_compose::modifiers::style::AppearanceExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::SemanticFontRef;
use cove_ui_compose::theme::styles::buttons::{
    ButtonStyleViewExt, GhostButtonStyle, SecondaryButtonStyle,
};
use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticStyleViewExt};
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View};

use super::feedback::StorySection;
use crate::gallery_content::catalog::GalleryOptions;

/// The workspace page: one dock layout plus layout persistence.
pub struct WorkspaceStory {
    pub options: GalleryOptions,
    controller: Rc<RefCell<WorkspaceController>>,
    saved: State<Option<String>>,
    status: State<String>,
}

impl WorkspaceStory {
    pub const HEIGHT: f32 = 360.0;

    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            controller: Rc::new(RefCell::new(WorkspaceController::new(
                Self::default_document(),
            ))),
            saved: State::new(None),
            status: State::new("Drag a tab or a split divider".to_owned()),
        }
    }

    fn id(value: &str) -> WorkspacePanelID {
        WorkspacePanelID::new(value)
    }

    fn panel(id: &str, title: &str, sizing: WorkspacePanelSizing) -> WorkspacePanel {
        WorkspacePanel {
            sizing,
            ..WorkspacePanel::new(Self::id(id), title)
        }
    }

    fn width(minimum: f32, preferred: f32, maximum: f32) -> WorkspacePanelSizing {
        WorkspacePanelSizing::new(
            WorkspaceAxisSizing::new(minimum, Some(preferred), Some(maximum)),
            WorkspaceAxisSizing::default(),
        )
    }

    fn height(minimum: f32, preferred: f32, maximum: f32) -> WorkspacePanelSizing {
        WorkspacePanelSizing::new(
            WorkspaceAxisSizing::default(),
            WorkspaceAxisSizing::new(minimum, Some(preferred), Some(maximum)),
        )
    }

    /// Files, search and history share the leading dock; the editor sits over
    /// a preview strip in the centre.
    pub fn default_document() -> WorkspaceDocument {
        let panels = BTreeMap::from([
            (
                Self::id("files"),
                Self::panel("files", "Files", Self::width(160.0, 240.0, 340.0)),
            ),
            (
                Self::id("search"),
                Self::panel(
                    "search",
                    "Search across the entire workspace",
                    Self::width(180.0, 260.0, 360.0),
                ),
            ),
            (
                Self::id("history"),
                WorkspacePanel::new(Self::id("history"), "Version history and incoming changes"),
            ),
            (
                Self::id("editor"),
                WorkspacePanel {
                    is_closable: false,
                    is_collapsible: false,
                    ..WorkspacePanel::new(Self::id("editor"), "Editor")
                },
            ),
            (
                Self::id("preview"),
                Self::panel("preview", "Preview", Self::height(80.0, 140.0, 240.0)),
            ),
        ]);
        let group = |id: &str, panels: &[&str]| {
            (
                WorkspaceTabGroupID::new(id),
                WorkspaceTabGroup::new(
                    WorkspaceTabGroupID::new(id),
                    panels.iter().map(|panel| Self::id(panel)).collect(),
                ),
            )
        };
        let groups = BTreeMap::from([
            group("left", &["files", "search", "history"]),
            group("main", &["editor"]),
            group("preview", &["preview"]),
        ]);
        let mut slots = WorkspaceSlot::standard_editor_slots();
        slots.insert(
            WorkspaceSlotID::leading(),
            WorkspaceSlot::new(
                WorkspaceSlotID::leading(),
                cove_ui_addons::workspace::WorkspaceSlotKind::Content,
                Some(WorkspaceLayoutNode::group(WorkspaceTabGroupID::new("left"))),
            ),
        );
        slots.insert(
            WorkspaceSlotID::center(),
            WorkspaceSlot::new(
                WorkspaceSlotID::center(),
                cove_ui_addons::workspace::WorkspaceSlotKind::Content,
                Some(WorkspaceLayoutNode::split(
                    WorkspaceSplitAxis::Vertical,
                    0.58,
                    WorkspaceLayoutNode::group(WorkspaceTabGroupID::new("main")),
                    WorkspaceLayoutNode::group(WorkspaceTabGroupID::new("preview")),
                )),
            ),
        );
        WorkspaceDocument {
            split_fractions: WorkspaceSplitFractions::default(),
            ..WorkspaceDocument::new(panels, groups, slots)
        }
    }

    /// The text each panel shows.
    pub fn panel_text(id: &WorkspacePanelID) -> &'static str {
        match id.raw_value() {
            "files" => "Sources\nResources\nTests",
            "editor" => "Editable document workspace",
            "search" => "Search panel",
            "history" => "Version history",
            _ => "Preview panel",
        }
    }

    pub fn controller(&self) -> Rc<RefCell<WorkspaceController>> {
        self.controller.clone()
    }

    pub fn status(&self) -> String {
        self.status.get()
    }

    pub fn save(&self) {
        let encoded = serde_json::to_string(self.controller.borrow().document());
        match encoded {
            Ok(json) => {
                self.saved.set(Some(json));
                self.status.set("Layout saved".to_owned());
            }
            Err(error) => self.status.set(format!("Save failed: {error}")),
        }
    }

    pub fn restore(&self) {
        let Some(json) = self.saved.get() else {
            return;
        };
        match serde_json::from_str::<WorkspaceDocument>(&json) {
            Ok(document) => {
                self.controller.borrow_mut().replace(document);
                self.status.set("Layout restored".to_owned());
            }
            Err(error) => self.status.set(format!("Restore failed: {error}")),
        }
    }

    pub fn reset(&self) {
        self.controller
            .borrow_mut()
            .replace(Self::default_document());
        self.status.set("Layout reset".to_owned());
    }

    fn handle(&self) -> Self {
        Self {
            options: self.options.clone(),
            controller: self.controller.clone(),
            saved: self.saved.clone(),
            status: self.status.clone(),
        }
    }
}

impl View for WorkspaceStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled = self.options.is_enabled;
        let save = self.handle();
        let restore = self.handle();
        let reset = self.handle();
        let actions = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::Wrap,
            10.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    Button::title("Save layout", move || save.save())
                        .enabled(enabled)
                        .button_style(SecondaryButtonStyle)
                        .debug_name("workspace-save"),
                ),
                AnyView::new(
                    Button::title("Restore", move || restore.restore())
                        .enabled(enabled && self.saved.get().is_some())
                        .button_style(SecondaryButtonStyle)
                        .debug_name("workspace-restore"),
                ),
                AnyView::new(
                    Button::title("Reset", move || reset.reset())
                        .enabled(enabled)
                        .button_style(GhostButtonStyle)
                        .debug_name("workspace-reset"),
                ),
            ])),
        ));
        let content: Rc<dyn Fn(WorkspacePanelID) -> AnyView> = Rc::new(|id: WorkspacePanelID| {
            AnyView::new(
                Text::new(WorkspaceStory::panel_text(&id))
                    .semantic_foreground(SemanticColorRef::on_surface_variant())
                    .padding(18.0),
            )
        });
        // Regions animate as they collapse, expand and close.
        let workspace = AnyView::new(
            WorkspaceView::new(self.controller.clone(), content)
                .frame_height(Self::HEIGHT)
                .corner_radius(10.0)
                .clipped()
                .composition_local(
                    WorkspaceThemeEnvironment::key(),
                    WorkspaceTheme {
                        animates_layout: true,
                        ..WorkspaceTheme::default()
                    },
                )
                .debug_name("workspace-dock"),
        );
        let status = AnyView::new(
            Text::new(self.status.get())
                .semantic_font(SemanticFontRef::caption())
                .semantic_foreground(SemanticColorRef::on_surface_muted()),
        );
        AnyView::new(StorySection::new(
            "Dock workspace",
            "Every panel owns persistent minimum, preferred and maximum sizes. Focus any divider for arrow/Home/End adjustment. Overflowing tabs scroll horizontally, including at 200%. Save and restore persists authored sizes and user splits.",
            AnyView::new(TupleView::new(vec![actions, workspace, status])),
        ))
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; saved, status)
    }
}

#[cfg(test)]
mod checks {
    use super::*;

    #[test]
    fn the_default_document_docks_five_panels() {
        let document = WorkspaceStory::default_document();
        assert_eq!(document.panels.len(), 5);
        assert_eq!(
            document
                .group_containing(&WorkspacePanelID::new("search"))
                .map(|group| group.id.raw_value().to_owned()),
            Some("left".to_owned())
        );
        let editor = &document.panels[&WorkspacePanelID::new("editor")];
        assert!(!editor.is_closable && !editor.is_collapsible);
        let files = &document.panels[&WorkspacePanelID::new("files")];
        assert_eq!(files.sizing.width.preferred, Some(240.0));
    }

    #[test]
    fn save_and_restore_round_trip_the_document() {
        let story = WorkspaceStory::new(GalleryOptions::default());
        story.save();
        assert_eq!(story.status(), "Layout saved");
        story
            .controller()
            .borrow_mut()
            .replace(WorkspaceDocument::new(
                BTreeMap::new(),
                BTreeMap::new(),
                BTreeMap::new(),
            ));
        story.restore();
        assert_eq!(story.status(), "Layout restored");
        assert_eq!(
            story.controller().borrow().document(),
            &WorkspaceStory::default_document()
        );
    }
}
