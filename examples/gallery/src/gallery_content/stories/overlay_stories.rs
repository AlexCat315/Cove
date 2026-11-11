//! Overlay stories: anchored confirmation and interactive hover cards.
//!
//! Swift sources: `GalleryContent/Stories/Overlays/{PopconfirmStory,HoverStory}.swift`.

use cove_composition::Scope;
use cove_core::State;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui_compose::modifiers::interaction::AccessibilityExt;
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use cove_ui_compose::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::controls::ButtonRole;
use cove_ui_compose::primitives::controls::Checkbox;
use cove_ui_compose::primitives::data::avatar::Avatar;
use cove_ui_compose::primitives::media::Icon;
use cove_ui_compose::primitives::media::ui_icon_catalog as icons;
use cove_ui_compose::primitives::overlays::PopoverPlacement;
use cove_ui_compose::primitives::overlays::hover_card::HoverCard;
use cove_ui_compose::primitives::overlays::menu::{MenuEntry, MenuItem, MenuSubmenu};
use cove_ui_compose::primitives::overlays::{ContextMenuExt, Popconfirm};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::theme::{Theme, ThemeReader};
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View};

use super::feedback::StorySection;
use super::placement_example::PlacementExample;
use crate::gallery_content::catalog::GalleryOptions;

fn caption(text: impl Into<String>) -> AnyView {
    AnyView::new(
        Text::new(text)
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted()),
    )
}

fn body_strong(text: impl Into<String>) -> AnyView {
    AnyView::new(
        Text::new(text).semantic_font(cove_ui_compose::theme::SemanticFontRef::body_strong()),
    )
}

/// A labelled row: leading icon, title + detail, trailing control. The leading
/// group flexes from a 200-pt basis so the control wraps below it when narrow.
fn labelled_row(
    icon: Option<cove_imaging::BundleImageResource>,
    icon_color: SemanticColorRef,
    title: impl Into<String>,
    detail: impl Into<String>,
    trailing: AnyView,
) -> AnyView {
    let mut leading: Vec<AnyView> = Vec::new();
    if let Some(icon) = icon {
        leading.push(AnyView::new(Icon::with_semantic(icon, 12.0, icon_color)));
    }
    leading.push(AnyView::new(
        AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            2.0,
            AnyView::new(TupleView::new(vec![body_strong(title), caption(detail)])),
        ))
        .flex_with(1.0, 1.0, Some(0.0))
        .frame_min_width(0.0),
    ));
    AnyView::new(BoxView::new(
        FlexDirection::Row,
        Align::Center,
        Justify::SpaceBetween,
        Wrap::Wrap,
        12.0,
        AnyView::new(TupleView::new(vec![
            AnyView::new(
                AnyView::new(BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    10.0,
                    AnyView::new(TupleView::new(leading)),
                ))
                .flex_with(1.0, 1.0, Some(200.0))
                .frame_min_width(0.0),
            ),
            trailing,
        ])),
    ))
}

/// `PopconfirmStory`: destructive actions require the explicit confirm button.
pub struct PopconfirmStory {
    pub options: GalleryOptions,
    last_action: State<String>,
    enabled: State<bool>,
}

impl PopconfirmStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            last_action: State::new("None".to_owned()),
            enabled: State::new(true),
        }
    }

    pub fn last_action(&self) -> String {
        self.last_action.get()
    }

    /// Records that a confirmation completed.
    pub fn record(&self, action: impl Into<String>) {
        self.last_action.set(action.into());
    }
}

impl View for PopconfirmStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled = self.options.is_enabled && self.enabled.get();

        let delete_sink = self.last_action.clone();
        let archive_sink = self.last_action.clone();
        let card = AnyView::new(ThemeReader::new(move |theme: &Theme| {
            let delete_sink = delete_sink.clone();
            let archive_sink = archive_sink.clone();
            let delete = Popconfirm::new(
                "Delete",
                Some(icons::action::trash()),
                "Delete “Archived mockup”?",
                "This action cannot be undone. The project and its local history will be removed.",
                "Delete",
                "Cancel",
                ButtonRole::Destructive,
                PopoverPlacement::Bottom,
                enabled,
                move || delete_sink.set("Deleted".to_owned()),
            );
            let archive = Popconfirm::new(
                "Archive",
                Some(icons::file::archive()),
                "Archive this project?",
                "You can restore it from the archive at any time.",
                "Archive",
                "Cancel",
                ButtonRole::Normal,
                PopoverPlacement::Bottom,
                enabled,
                move || archive_sink.set("Archived".to_owned()),
            );
            AnyView::new(
                BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    10.0,
                    AnyView::new(TupleView::new(vec![
                        labelled_row(
                            Some(icons::action::trash()),
                            SemanticColorRef::error(),
                            "Delete project",
                            "This removes the project and its local history.",
                            AnyView::new(delete),
                        ),
                        labelled_row(
                            Some(icons::file::archive()),
                            SemanticColorRef::on_surface_muted(),
                            "Archive project",
                            "Keeps the project; it can be restored later.",
                            AnyView::new(archive),
                        ),
                    ])),
                )
                .padding(14.0)
                .background_color(theme.colors.surface)
                .border_color(theme.colors.border, 1.0)
                .corner_radius(theme.radius.md),
            )
        }));

        let enabled_binding = self.enabled.clone();
        let controls = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::Wrap,
            8.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    Checkbox::from_boolean(enabled_binding.binding())
                        .enabled(self.options.is_enabled)
                        .accessibility_label("Enable confirmation actions"),
                ),
                caption("Enabled"),
                AnyView::new(
                    caption(format!("Last action: {}", self.last_action.get()))
                        .debug_name("popconfirm-last-action"),
                ),
                AnyView::new(
                    BoxView::new(
                        FlexDirection::Row,
                        Align::Center,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        0.0,
                        AnyView::new(TupleView::new(Vec::new())),
                    )
                    .flex_with(1.0, 1.0, None),
                ),
            ])),
        ));

        let placement_sink = self.last_action.clone();
        let placement = AnyView::new(PlacementExample::new(self.options.clone(), move |label| {
            placement_sink.set(label);
        }));

        AnyView::new(StorySection::new(
            "Anchored confirmation",
            "The trigger is a real button; the popover owns focus, Escape and outside dismissal. Destructive actions require the explicit red confirm button.",
            AnyView::new(TupleView::new(vec![
                card,
                controls,
                caption(
                    "Cancel or Escape closes the bubble without invoking the action. The destructive trigger is styled separately from the confirm button.",
                ),
                placement,
            ])),
        ))
    }

    fn tag(&self) -> &'static str {
        "PopconfirmStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; last_action, enabled)
    }
}

/// `HoverCardStory`: hover reveals a detail card the keyboard can also reach.
pub struct HoverCardStory {
    pub options: GalleryOptions,
    following: State<bool>,
}

impl HoverCardStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            following: State::new(false),
        }
    }

    pub fn is_following(&self) -> bool {
        self.following.get()
    }

    /// Toggles the follow action the card exposes.
    pub fn toggle_following(&self) {
        self.following.update(|value| *value = !*value);
    }
}

impl View for HoverCardStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled = self.options.is_enabled;
        let following = self.following.get();
        let toggle = self.following.clone();

        let profile = AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            10.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    10.0,
                    AnyView::new(TupleView::new(vec![
                        AnyView::new(Avatar::new("Ada Lovelace")),
                        AnyView::new(BoxView::new(
                            FlexDirection::Column,
                            Align::Stretch,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            3.0,
                            AnyView::new(TupleView::new(vec![
                                body_strong("Ada Lovelace"),
                                caption("Analytical Engine · maintainer"),
                            ])),
                        )),
                    ])),
                )),
                caption(
                    "Contributor since 1843. Open the profile with a click or keyboard for the same actions.",
                ),
                AnyView::new(
                    Button::title(if following { "Unfollow" } else { "Follow" }, move || {
                        toggle.update(|value| *value = !*value)
                    })
                    .enabled(enabled)
                    .debug_name("hover-follow"),
                ),
            ])),
        ));

        let card = HoverCard::new(
            AnyView::new(
                Text::new("@ada")
                    .semantic_font(cove_ui_compose::theme::SemanticFontRef::body_strong())
                    .semantic_foreground(SemanticColorRef::accent())
                    .debug_name("hover-profile-trigger"),
            ),
            profile.clone(),
        )
        .width(Some(340.0))
        .enabled(enabled);

        let content = AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            12.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(card),
                caption(if following {
                    "Following Ada"
                } else {
                    "Not following Ada"
                }),
            ])),
        ));

        AnyView::new(StorySection::new(
            "Interactive detail",
            "Move from the name into the card and use Follow. Escape or clicking outside dismisses it. Open profile offers the same information and action from the keyboard.",
            content,
        ))
    }

    fn tag(&self) -> &'static str {
        "HoverCardStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; following)
    }
}

/// The right-click target the context-menu story annotates.
pub const CONTEXT_TARGET: &str = "Right-click this card";
/// The last command the context menu reported.
pub const CONTEXT_INITIAL: &str = "No command yet";

/// `ContextMenuStory`: a right-click target with a real menu tree.
///
/// The menu is the `Menu` surface the sidebar rows use, opened by a secondary
/// click (or Control-click) on the card, with the same keyboard path.
pub struct ContextMenuStory {
    pub options: GalleryOptions,
    last_action: State<String>,
}

impl ContextMenuStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            last_action: State::new(CONTEXT_INITIAL.to_owned()),
        }
    }

    pub fn last_action(&self) -> String {
        self.last_action.get()
    }

    /// The entries the card offers, in authored order.
    pub fn entries(&self) -> Vec<MenuEntry> {
        let record = self.last_action.clone();
        vec![
            MenuEntry::Item(
                MenuItem::new(201, "Open", {
                    let record = record.clone();
                    move || record.set("Open".to_owned())
                })
                .enabled(self.options.is_enabled),
            ),
            MenuEntry::Item(MenuItem::new(202, "Duplicate", {
                let record = record.clone();
                move || record.set("Duplicate".to_owned())
            })),
            MenuEntry::separator(203),
            MenuEntry::Submenu(MenuSubmenu {
                id: 204,
                title: "Move to".to_owned(),
                entries: vec![
                    MenuEntry::Item(MenuItem::new(205, "Projects", {
                        let record = record.clone();
                        move || record.set("Move to Projects".to_owned())
                    })),
                    MenuEntry::Item(MenuItem::new(206, "Archive", {
                        let record = record.clone();
                        move || record.set("Move to Archive".to_owned())
                    })),
                ],
                is_enabled: self.options.is_enabled,
                width: 200.0,
                icon: None,
            }),
            MenuEntry::Item(MenuItem::new(207, "Delete", || {}).enabled(false)),
        ]
    }
}

impl View for ContextMenuStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let target = AnyView::new(
            AnyView::new(
                AnyView::new(Text::new(CONTEXT_TARGET))
                    .padding_edges(16.0, 24.0)
                    .semantic_background(SemanticColorRef::surface_variant()),
            )
            .context_menu(self.entries()),
        );
        AnyView::new(StorySection::new(
            "Right-click menu",
            "Control-click or right-click the card for the menu. Submenus open on hover or Right, Escape dismisses the innermost level first, and unavailable commands stay visible but skipped.",
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::FlexStart,
                Justify::FlexStart,
                Wrap::NoWrap,
                12.0,
                AnyView::new(TupleView::new(vec![
                    target,
                    caption(format!("Last command: {}", self.last_action())),
                ])),
            )),
        ))
    }

    fn tag(&self) -> &'static str {
        "ContextMenuStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; last_action)
    }
}
