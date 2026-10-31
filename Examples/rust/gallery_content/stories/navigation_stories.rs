//! Navigation stories: the application header.
//!
//! Swift source: `GalleryContent/Stories/Navigation/AppBarStory.swift`.
//!
//! The header's state machine (long title toggle, text-vs-icon primary action,
//! last performed action) is what the gallery drives, so it lives here as a type.

use std::cell::RefCell;
use std::rc::Rc;

use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
use cove_ui_compose::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use cove_ui_compose::primitives::controls::Checkbox;
use cove_ui_compose::primitives::controls::app_bar::{AppBar, AppBarOptions};
use cove_ui_compose::primitives::controls::toolbar::{ToolbarItem, ToolbarItemPlacement};
use cove_ui_compose::primitives::media::ui_icon_catalog as icons;
use cove_ui_compose::primitives::overlays::menu::MenuItem;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View, ViewExt};

use super::feedback::StorySection;
use crate::gallery_content::catalog::GalleryOptions;

/// The long title the story uses, exercising a wide glyph run and an emoji.
pub const LONG_TITLE: &str = "Workspace permissions and project access · 用户设置 👩🏽‍💻";
/// The short title the story swaps to.
pub const SHORT_TITLE: &str = "Workspace";

/// `AppBarStory`: a header that sheds actions as it narrows.
pub struct AppBarStory {
    pub options: GalleryOptions,
    // `State` is `Arc`-backed, so its binding satisfies `Binding`'s
    // `Send + Sync` requirement; `Rc<Cell>` does not.
    long_title: cove_core::State<bool>,
    text_primary: cove_core::State<bool>,
    // `State` (not `Rc<RefCell<_>>`) so the result line recomposes when an
    // action is recorded — otherwise the Save button visibly does nothing.
    last_action: cove_core::State<String>,
}

impl AppBarStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            long_title: cove_core::State::new(true),
            text_primary: cove_core::State::new(false),
            last_action: cove_core::State::new("No action yet".to_owned()),
        }
    }

    pub fn title(&self) -> &'static str {
        if self.long_title.get() {
            LONG_TITLE
        } else {
            SHORT_TITLE
        }
    }

    pub fn last_action(&self) -> String {
        self.last_action.get()
    }

    /// Records the action the header performed.
    pub fn record(&self, action: impl Into<String>) {
        self.last_action.set(action.into());
    }

    pub fn uses_text_primary(&self) -> bool {
        self.text_primary.get()
    }

    /// The overflow commands, in authored order.
    ///
    /// The first is an automatic-placement command at priority 10, so it stays
    /// inline longest; the last two are explicitly overflow-only, and one is
    /// disabled to exercise that styling.
    fn actions(&self) -> Vec<ToolbarItem> {
        let record = self.last_action.clone();
        vec![
            ToolbarItem::new(MenuItem::new(1, "Find in workspace", move || {
                record.set("Find".to_owned());
            }))
            .icon(
                cove_ui_compose::primitives::controls::button::ButtonIconSource::Resource(
                    icons::action::search(),
                ),
            )
            .placement(ToolbarItemPlacement::AutomaticWithPriority(10)),
            ToolbarItem::new(MenuItem::new(2, "Export project", {
                let record = self.last_action.clone();
                move || record.set("Exported".to_owned())
            }))
            .icon(
                cove_ui_compose::primitives::controls::button::ButtonIconSource::Resource(
                    icons::action::download(),
                ),
            ),
            ToolbarItem::new(MenuItem::new(3, "Share project", {
                let record = self.last_action.clone();
                move || record.set("Shared".to_owned())
            }))
            .icon(
                cove_ui_compose::primitives::controls::button::ButtonIconSource::Resource(
                    icons::action::share(),
                ),
            ),
            ToolbarItem::new(MenuItem::new(4, "Pinned", {
                let record = self.last_action.clone();
                move || record.set("Pinned".to_owned())
            }))
            .placement(ToolbarItemPlacement::Overflow),
            // A disabled command stays listed so the menu can explain itself.
            ToolbarItem::new(MenuItem::new(5, "Unavailable", || {}).enabled(false))
                .placement(ToolbarItemPlacement::Overflow),
        ]
    }
}

impl View for AppBarStory {
    fn body(&self) -> AnyView {
        let enabled = self.options.is_enabled;

        let long_title = self.long_title.clone();
        let text_primary = self.text_primary.clone();
        let toggles = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::Wrap,
            8.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    Checkbox::from_boolean(long_title.binding())
                        .enabled(enabled)
                        .debug_name("appbar-long-title"),
                ),
                AnyView::new(
                    Text::new("Long title")
                        .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption()),
                ),
                AnyView::new(
                    Checkbox::from_boolean(text_primary.binding())
                        .enabled(enabled)
                        .debug_name("appbar-text-primary"),
                ),
                AnyView::new(
                    Text::new("Text primary action")
                        .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption()),
                ),
            ])),
        ));

        let record_back = self.last_action.clone();
        let record_save = self.last_action.clone();
        let header = AppBar::new(self.title())
            .subtitle("All changes saved locally")
            .back(MenuItem::new(10, "Back to projects", move || {
                record_back.set("Back".to_owned());
            }))
            .primary(
                ToolbarItem::new(MenuItem::new(11, "Save workspace", move || {
                    record_save.set("Saved".to_owned());
                }))
                .icon(
                    cove_ui_compose::primitives::controls::button::ButtonIconSource::Resource(
                        icons::action::check(),
                    ),
                ),
            )
            .actions(self.actions())
            .options(AppBarOptions {
                is_enabled: enabled,
                ..AppBarOptions::default()
            });

        let rows = AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            10.0,
            AnyView::new(TupleView::new(vec![
                toggles,
                AnyView::new(AnyView::new(header).debug_name("gallery-appbar")),
                AnyView::new(
                    Text::new(self.last_action())
                        .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
                        .semantic_foreground(SemanticColorRef::on_surface_muted())
                        .debug_name("appbar-result"),
                ),
            ])),
        ));

        AnyView::new(StorySection::new(
            "Application header",
            "Resize the window: compact headers keep the primary action and a menu; wider headers show up to three actions. The complete title remains available to assistive technology and in a tooltip.",
            rows,
        ))
    }

    fn tag(&self) -> &'static str {
        "AppBarStory"
    }

    /// The story reads three pieces of live state, so all three must be
    /// declared or the toggles and the action result never recompose. This is
    /// the same defect class that silently froze the timeline, SplitView and
    /// ten other views.
    fn dynamic_properties(&self) -> Vec<cove_ui_compose::DynamicField> {
        vec![
            cove_ui_compose::DynamicField::state("long_title", &self.long_title),
            cove_ui_compose::DynamicField::state("text_primary", &self.text_primary),
            cove_ui_compose::DynamicField::state("last_action", &self.last_action),
        ]
    }
}

/// The declared header height for a control size, matching the toolbar contract.
pub fn header_height(button_height: f32) -> f32 {
    button_height + 8.0
}

/// The number of actions a header shows inline.
///
/// One when compact, three otherwise — the capacity AppBar itself uses.
pub fn inline_capacity(is_compact: bool) -> usize {
    if is_compact { 1 } else { 3 }
}

/// Keeps the story's back command label available to tests.
pub fn back_title() -> &'static str {
    "Back to projects"
}

/// A disabled overflow command is still listed, so the menu shows why it cannot run.
pub const UNAVAILABLE_TITLE: &str = "Unavailable";
