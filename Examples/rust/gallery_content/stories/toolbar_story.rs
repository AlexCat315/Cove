//! Controls story: toolbars and command overflow.
//!
//! Swift source: `GalleryContent/Stories/Controls/ToolbarStory.swift`.
//!
//! The interesting behaviour is which commands stay inline as the bar narrows:
//! `Always` never moves, `Overflow` never inlines, and larger automatic
//! priorities survive longer while ties keep source order.

use cove_core::State;
use cove_ui_compose::modifiers::layout::{LayoutFrameExt, PaddingExt};
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::controls::toolbar::overflow_layout::{
    ToolbarArrangement, ToolbarOverflowLayout,
};
use cove_ui_compose::primitives::controls::toolbar::{
    Toolbar, ToolbarAxis, ToolbarDividerEdge, ToolbarItem, ToolbarItemPlacement, ToolbarOptions,
};
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::overlays::menu::MenuItem;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, BoxView, TupleView, View, ViewExt};

use crate::gallery_content::catalog::GalleryOptions;

/// Spacing between toolbar items, in points.
pub const TOOLBAR_SPACING: f32 = 6.0;

/// `ToolbarStory`: an adaptive command bar plus a content bar.
pub struct ToolbarStory {
    pub options: GalleryOptions,
    is_playing: State<bool>,
    applied_action: State<String>,
}

impl ToolbarStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            is_playing: State::new(false),
            applied_action: State::new("None".to_owned()),
        }
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing.get()
    }

    pub fn applied_action(&self) -> String {
        self.applied_action.get()
    }

    pub fn record(&self, action: impl Into<String>) {
        self.applied_action.set(action.into());
    }

    /// The run command's title flips with playback state.
    pub fn run_title(&self) -> &'static str {
        if self.is_playing.get() { "Pause" } else { "Run" }
    }

    /// The five commands the adaptive bar declares, in authored order.
    ///
    /// `Run` is `Always`; `Project settings` is `Overflow`; the middle three are
    /// automatic, with `Save workspace` at priority 10 so it outlives the rest.
    pub fn command_placements(&self) -> Vec<ToolbarItemPlacement> {
        vec![
            ToolbarItemPlacement::Always,
            ToolbarItemPlacement::AutomaticWithPriority(10),
            ToolbarItemPlacement::Automatic,
            ToolbarItemPlacement::Automatic,
            ToolbarItemPlacement::Overflow,
        ]
    }

    /// Which commands stay inline at a given width, with every item the same size.
    ///
    /// The layout expects one extra subview for the overflow button, so a uniform
    /// row is `commands + 1` equal sizes.
    pub fn arrangement(&self, available_extent: f32, item_extent: f32) -> ToolbarArrangement {
        let layout =
            ToolbarOverflowLayout::new(available_extent, self.command_placements(), TOOLBAR_SPACING);
        let sizes = vec![cove_core::Size { width: item_extent, height: 28.0 }; self.command_placements().len() + 1];
        layout.arrangement(&sizes)
    }

    pub fn items(&self) -> Vec<ToolbarItem> {
        let playing = self.is_playing.clone();
        let record = self.applied_action.clone();
        vec![
            ToolbarItem::new(MenuItem::new(1, self.run_title(), move || {
                playing.update(|value| *value = !*value)
            }))
            .placement(ToolbarItemPlacement::Always),
            ToolbarItem::new(MenuItem::new(2, "Save workspace", {
                let record = record.clone();
                move || record.set("Saved".to_owned())
            }))
            .placement(ToolbarItemPlacement::AutomaticWithPriority(10)),
            ToolbarItem::new(MenuItem::new(3, "Export selection", move || {
                record.set("Exported".to_owned())
            })),
            ToolbarItem::new(MenuItem::new(4, "Share project", || {})),
            ToolbarItem::new(MenuItem::new(5, "Project settings", || {}))
                .placement(ToolbarItemPlacement::Overflow),
        ]
    }
}

impl View for ToolbarStory {
    fn body(&self) -> AnyView {
        let enabled = self.options.is_enabled;
        let apply = self.applied_action.clone();
        let note = Text::new("Resize this bar: Run stays visible; lower priority commands move into More actions.")
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted());

        let content_bar = Toolbar::new(
            BoxView::new(
                cove_core::scene::FlexDirection::Row,
                cove_core::scene::Align::Center,
                cove_core::scene::Justify::FlexStart,
                cove_core::scene::Wrap::NoWrap,
                TOOLBAR_SPACING,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(Text::new("A bar below its content puts the divider on top.")),
                    AnyView::new(Button::title("Apply", move || {
                        apply.set("Applied".to_owned())
                    })),
                ])),
            ),
        )
        .options(ToolbarOptions {
            divider: ToolbarDividerEdge::Top,
            is_enabled: enabled,
            label: "Apply bar".to_owned(),
            axis: ToolbarAxis::Horizontal,
        });

        let summary = Text::new(format!("Last action: {}", self.applied_action()))
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted());

        AnyView::new(BoxView::new(
            cove_core::scene::FlexDirection::Column,
            cove_core::scene::Align::Stretch,
            cove_core::scene::Justify::FlexStart,
            cove_core::scene::Wrap::NoWrap,
            16.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(AnyView::new(note).padding_edges(0.0, 4.0)),
                AnyView::new(content_bar),
                AnyView::new(summary),
            ])),
        ))
    }

    fn tag(&self) -> &'static str {
        "ToolbarStory"
    }
}
