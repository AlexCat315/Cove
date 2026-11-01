//! Overlay stories: anchored confirmation and interactive hover cards.
//!
//! Swift sources: `GalleryContent/Stories/Overlays/{PopconfirmStory,HoverStory}.swift`.

use cove_core::State;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::controls::ButtonRole;
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::data::avatar::Avatar;
use cove_ui_compose::primitives::media::Icon;
use cove_ui_compose::primitives::media::ui_icon_catalog as icons;
use cove_ui_compose::primitives::overlays::PopoverPlacement;
use cove_ui_compose::primitives::overlays::hover_card::HoverCard;
use cove_ui_compose::primitives::overlays::popconfirm::Popconfirm;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View};

use super::feedback::StorySection;
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

/// A labelled row: leading icon, title + detail, trailing control.
fn labelled_row(
    icon: Option<cove_imaging::BundleImageResource>,
    title: impl Into<String>,
    detail: impl Into<String>,
    trailing: AnyView,
    is_enabled: bool,
) -> AnyView {
    let mut leading: Vec<AnyView> = Vec::new();
    if let Some(icon) = icon {
        leading.push(AnyView::new(Icon::new(icon, 14.0)));
    }
    leading.push(AnyView::new(BoxView::new(
        FlexDirection::Column,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        2.0,
        AnyView::new(TupleView::new(vec![body_strong(title), caption(detail)])),
    )));
    let _ = is_enabled;
    AnyView::new(BoxView::new(
        FlexDirection::Row,
        Align::Center,
        Justify::SpaceBetween,
        Wrap::Wrap,
        12.0,
        AnyView::new(TupleView::new(vec![
            AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                10.0,
                AnyView::new(TupleView::new(leading)),
            )),
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
    fn body(&self) -> AnyView {
        let enabled = self.options.is_enabled && self.enabled.get();

        let delete_sink = self.last_action.clone();
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

        let archive_sink = self.last_action.clone();
        let archive = Popconfirm::new(
            "Archive",
            Some(icons::file::archive()),
            "Archive “Archived mockup”?",
            "The project moves to the archive and can be restored later.",
            "Archive",
            "Cancel",
            ButtonRole::Normal,
            PopoverPlacement::Bottom,
            enabled,
            move || archive_sink.set("Archived".to_owned()),
        );

        let rows = AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            10.0,
            AnyView::new(TupleView::new(vec![
                labelled_row(
                    Some(icons::action::trash()),
                    "Delete project",
                    "This removes the project and its local history.",
                    AnyView::new(delete),
                    enabled,
                ),
                labelled_row(
                    Some(icons::file::archive()),
                    "Archive project",
                    "Hide it from the active list without deleting anything.",
                    AnyView::new(archive),
                    enabled,
                ),
                AnyView::new(
                    BoxView::new(
                        FlexDirection::Row,
                        Align::Center,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        10.0,
                        AnyView::new(TupleView::new(vec![
                            caption("Last action"),
                            body_strong(self.last_action.get()),
                        ])),
                    )
                    .debug_name("popconfirm-last-action"),
                ),
            ])),
        ));

        AnyView::new(StorySection::new(
            "Anchored confirmation",
            "The trigger is a real button; the popover owns focus, Escape and outside dismissal. Destructive actions require the explicit red confirm button.",
            rows,
        ))
    }

    fn tag(&self) -> &'static str {
        "PopconfirmStory"
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
    fn body(&self) -> AnyView {
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
}
