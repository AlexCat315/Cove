//! Feedback story: avatars, status and groups.
//!
//! Swift source: `GalleryContent/Stories/Feedback/AvatarStory.swift`.
//!
//! The group geometry (`AvatarGroupGeometry`) is the shared truth for measuring,
//! painting and hit testing, so the story's limit/overflow/overlap controls drive
//! it directly and the tests assert against it.

use cove_composition::Scope;
use cove_core::State;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::controls::fields::Slider;
use cove_ui_compose::primitives::data::avatar::{
    Avatar, AvatarGroup, AvatarGroupGeometry, AvatarGroupOptions, AvatarMemberList, AvatarOverflow,
    AvatarShape, AvatarStatus,
};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::ControlSize;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View, ViewExt};

use super::feedback::StorySection;
use crate::gallery_content::catalog::GalleryOptions;

/// The six members the story lists, in authored order.
pub const MEMBER_NAMES: [&str; 6] = [
    "Jason Lee",
    "Alice Johnson",
    "陈珊",
    "Diana Prince",
    "Émile Zola",
    "Bob Smith",
];

/// The names whose initials the story checks against Unicode corner cases.
pub const INITIAL_CASES: [&str; 7] = [
    "Jason Lee",
    "huacnlee",
    "李 小龙",
    "陈珊",
    "Émile Zola",
    "👩🏽‍💻 coder",
    "",
];

/// The sources the image/fallback switch offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AvatarImageSource {
    Bundled,
    Remote,
    Missing,
    Initials,
}

impl AvatarImageSource {
    pub const ALL: [Self; 4] = [Self::Bundled, Self::Remote, Self::Missing, Self::Initials];

    pub fn id(self) -> &'static str {
        match self {
            Self::Bundled => "bundled",
            Self::Remote => "remote",
            Self::Missing => "missing",
            Self::Initials => "initials",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Bundled => "Bundled",
            Self::Remote => "Remote",
            Self::Missing => "Missing",
            Self::Initials => "Initials",
        }
    }

    pub fn from_id(id: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|source| source.id() == id)
            .unwrap_or(Self::Bundled)
    }

    /// Whether this source has no image at all, so initials show directly.
    pub fn shows_initials_only(self) -> bool {
        matches!(self, Self::Initials)
    }

    /// The URL this source resolves to, when it has one.
    pub fn url(self) -> Option<&'static str> {
        match self {
            Self::Bundled => Some("avatar-portrait.svg"),
            Self::Remote => Some("https://avatars.githubusercontent.com/u/5518?v=4"),
            Self::Missing => Some("/cove-gallery-missing-avatar.png"),
            Self::Initials => None,
        }
    }
}

/// `AvatarStory`: image/fallback, Unicode initials, group overflow, status.
pub struct AvatarStory {
    pub options: GalleryOptions,
    source: State<String>,
    limit: State<usize>,
    overflow: State<AvatarOverflow>,
    overlap: State<f64>,
    selected_member: State<String>,
}

impl AvatarStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            source: State::new(AvatarImageSource::Bundled.id().to_owned()),
            limit: State::new(3),
            overflow: State::new(AvatarOverflow::Count),
            overlap: State::new(0.3),
            selected_member: State::new("None".to_owned()),
        }
    }

    pub fn source(&self) -> AvatarImageSource {
        AvatarImageSource::from_id(&self.source.get())
    }

    /// Changing the source cancels stale image work; the state simply swaps.
    pub fn set_source(&self, source: AvatarImageSource) {
        self.source.set(source.id().to_owned());
    }

    pub fn limit(&self) -> usize {
        self.limit.get()
    }

    pub fn set_limit(&self, limit: usize) {
        self.limit.set(limit);
    }

    pub fn overflow(&self) -> AvatarOverflow {
        self.overflow.get()
    }

    pub fn set_overflow(&self, overflow: AvatarOverflow) {
        self.overflow.set(overflow);
    }

    pub fn selected_member(&self) -> String {
        self.selected_member.get()
    }

    pub fn members(&self) -> Vec<Avatar> {
        MEMBER_NAMES.iter().map(|name| Avatar::new(*name)).collect()
    }

    /// The group geometry for the current limit/overflow/overlap controls.
    pub fn group_geometry(&self, diameter: f32) -> AvatarGroupGeometry {
        AvatarGroupGeometry::new(
            MEMBER_NAMES.len(),
            self.limit.get(),
            diameter,
            self.overlap.get() as f32,
            self.overflow.get(),
        )
    }

    /// Every status the story renders, in `allCases` order.
    pub fn statuses(&self) -> [AvatarStatus; 4] {
        AvatarStatus::ALL
    }
}

impl View for AvatarStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled = self.options.is_enabled;

        // Unicode initials: two-word initials or two graphemes from one name.
        let initials_row = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::Wrap,
            16.0,
            AnyView::new(TupleView::new(
                INITIAL_CASES
                    .iter()
                    .map(|name| {
                        AnyView::new(Text::new(if name.is_empty() { "Anonymous" } else { name }))
                    })
                    .collect(),
            )),
        ));

        // Status sits on the outline at the logical bottom-trailing corner.
        let status_row = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::Wrap,
            20.0,
            AnyView::new(TupleView::new(
                AvatarStatus::ALL
                    .iter()
                    .map(|status| {
                        AnyView::new(
                            Text::new(status.label())
                                .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption()),
                        )
                    })
                    .collect(),
            )),
        ));

        let mut group_options = AvatarGroupOptions::default();
        group_options.limit = self.limit.get();
        group_options.overflow = self.overflow.get();
        group_options.overlap = self.overlap.get() as f32;
        let member_list = AvatarMemberList::default();
        group_options.member_list = Some(member_list);
        let mut group = AvatarGroup::new(self.members());
        group.options = group_options;

        let overlap_slider = Slider::new(self.overlap.binding()).enabled(enabled);

        let rows = AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            24.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(StorySection::new(
                    "Unicode initials",
                    "Two word initials or two graphemes from a single name. Accented names, Chinese characters and emoji share the same text layout.",
                    initials_row,
                )),
                AnyView::new(StorySection::new(
                    "Avatar groups",
                    "Earlier members paint above later members. An explicit identity keeps image state with each person when a group reorders.",
                    AnyView::new(TupleView::new(vec![
                        AnyView::new(group),
                        AnyView::new(overlap_slider.frame_width(180.0)),
                    ])),
                )),
                AnyView::new(StorySection::new(
                    "Presence & members",
                    "Status sits on the outline at the logical bottom-trailing corner, uses a distinct shape per state and is read with the name. The overflow marker opens the member list.",
                    status_row,
                )),
                AnyView::new(StorySection::new(
                    "Sizes & shapes",
                    "Named control sizes and the three shapes share one geometry, so a rounded square keeps its radius at every diameter.",
                    AnyView::new(BoxView::new(
                        FlexDirection::Row,
                        Align::Center,
                        Justify::FlexStart,
                        Wrap::Wrap,
                        16.0,
                        AnyView::new(TupleView::new({
                            let mut tiles: Vec<AnyView> = ControlSize::ALL
                                .iter()
                                .map(|size| {
                                    let size = *size;
                                    AnyView::new(
                                        Avatar::new("Ada Lovelace").configure(move |appearance| {
                                            appearance.size = Some(size);
                                        }),
                                    )
                                })
                                .collect();
                            for shape in [
                                AvatarShape::Circle,
                                AvatarShape::RoundedRectangle(10.0),
                                AvatarShape::Square,
                            ] {
                                tiles.push(AnyView::new(
                                    Avatar::new("Ada Lovelace").configure(move |appearance| {
                                        appearance.shape = shape.clone();
                                    }),
                                ));
                            }
                            tiles
                        })),
                    )),
                )),
                AnyView::new(
                    Text::new(format!("Selected member: {}", self.selected_member()))
                        .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
                        .semantic_foreground(SemanticColorRef::on_surface_muted())
                        .debug_name("avatar-selected-member"),
                ),
            ])),
        ));

        AnyView::new(StorySection::new(
            "Image & fallback",
            "Images load off the UI loop. Initials remain visible during loading and after an error; changing the source cancels stale work.",
            rows,
        ))
    }

    fn tag(&self) -> &'static str {
        "AvatarStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; source, limit, overflow, overlap, selected_member)
    }
}
