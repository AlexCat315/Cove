//! Controls story: toolbars and command overflow.
//!
//! Swift source: `GalleryContent/Stories/Controls/ToolbarStory.swift`.
//!
//! The interesting behaviour is which commands stay inline as the bar narrows:
//! `Always` never moves, `Overflow` never inlines, and larger automatic
//! priorities survive longer while ties keep source order.

use super::{
    feedback::StorySection,
    showcase::{note, row, stack},
};
use cove_composition::Scope;
use cove_core::State;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui_compose::modifiers::layout::FlexExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::controls::ButtonIconSource;
use cove_ui_compose::primitives::controls::toolbar::commands::ToolbarCommands;
use cove_ui_compose::primitives::controls::toolbar::overflow_layout::{
    ToolbarArrangement, ToolbarOverflowLayout,
};
use cove_ui_compose::primitives::controls::toolbar::{
    Toolbar, ToolbarAxis, ToolbarDividerEdge, ToolbarItem, ToolbarItemPlacement, ToolbarOptions,
};
use cove_ui_compose::primitives::controls::toolbar::{ToolbarGroup, ToolbarSeparator};
use cove_ui_compose::primitives::media::ui_icon_catalog as icons;
use cove_ui_compose::primitives::overlays::menu::MenuItem;
use cove_ui_compose::primitives::text::text_field::{TextField, TextFieldBehavior};
use cove_ui_compose::{AnyView, View};
use cove_ui_compose::{BoxView, LayoutDebugExt, TupleView};

use crate::gallery_content::catalog::GalleryOptions;

/// Spacing between toolbar items, in points.
pub const TOOLBAR_SPACING: f32 = 6.0;

/// `ToolbarStory`: an adaptive command bar plus a content bar.
pub struct ToolbarStory {
    pub options: GalleryOptions,
    is_playing: State<bool>,
    applied_action: State<String>,
    is_bold: State<bool>,
    search: State<cove_ui_compose::primitives::text::text_buffer::TextBuffer>,
    zoom_percent: State<i32>,
}

impl ToolbarStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            is_playing: State::new(false),
            applied_action: State::new("None".to_owned()),
            is_bold: State::new(true),
            search: State::new(cove_ui_compose::primitives::text::text_buffer::TextBuffer::empty()),
            zoom_percent: State::new(100),
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
        if self.is_playing.get() {
            "Pause"
        } else {
            "Run"
        }
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
        let layout = ToolbarOverflowLayout::new(
            available_extent,
            self.command_placements(),
            TOOLBAR_SPACING,
        );
        let sizes = vec![
            cove_core::Size {
                width: item_extent,
                height: 28.0
            };
            self.command_placements().len() + 1
        ];
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
            ToolbarItem::new(MenuItem::new(3, "Export selection", {
                let record = record.clone();
                move || record.set("Exported".to_owned())
            })),
            ToolbarItem::new(MenuItem::new(4, "Share project", {
                let record = record.clone();
                move || record.set("Shared".to_owned())
            })),
            ToolbarItem::new(MenuItem::new(5, "Project settings", move || {
                record.set("Settings".to_owned())
            }))
            .placement(ToolbarItemPlacement::Overflow),
        ]
    }
}

impl ToolbarStory {
    fn playback(&self) -> AnyView {
        let play = self.is_playing.clone();
        let stop = self.is_playing.clone();
        AnyView::new(
            ToolbarGroup::new(TupleView::new(vec![
                AnyView::new(
                    Button::icon_default(
                        ButtonIconSource::resource(icons::editor::play()),
                        move || play.set(true),
                    )
                    .selected(self.is_playing())
                    .tooltip("Play"),
                ),
                AnyView::new(
                    Button::icon_default(
                        ButtonIconSource::resource(icons::editor::stop()),
                        move || stop.set(false),
                    )
                    .selected(!self.is_playing())
                    .tooltip("Stop"),
                ),
            ]))
            .label("Playback"),
        )
    }
    fn formatting(&self) -> AnyView {
        let bold = self.is_bold.clone();
        AnyView::new(
            ToolbarGroup::new(TupleView::new(vec![
                AnyView::new(
                    Button::icon_default(
                        ButtonIconSource::resource(icons::text::bold()),
                        move || bold.update(|b| *b = !*b),
                    )
                    .selected(self.is_bold.get())
                    .tooltip("Bold"),
                ),
                AnyView::new(
                    Button::icon_default(ButtonIconSource::resource(icons::text::italic()), || {})
                        .enabled(false)
                        .tooltip("Italic unavailable"),
                ),
            ]))
            .bordered(true)
            .label("Formatting"),
        )
    }
    fn zoom(&self) -> AnyView {
        let out = self.zoom_percent.clone();
        let into = self.zoom_percent.clone();
        AnyView::new(
            ToolbarGroup::new(TupleView::new(vec![
                AnyView::new(
                    Button::icon_default(
                        ButtonIconSource::resource(icons::action::zoom_out()),
                        move || out.set((out.get() - 10).max(50)),
                    )
                    .enabled(self.zoom_percent.get() > 50)
                    .tooltip("Zoom out"),
                ),
                AnyView::new(
                    Button::icon_default(
                        ButtonIconSource::resource(icons::action::zoom_in()),
                        move || into.set((into.get() + 10).min(200)),
                    )
                    .enabled(self.zoom_percent.get() < 200)
                    .tooltip("Zoom in"),
                ),
            ]))
            .label("Zoom"),
        )
    }
}

impl View for ToolbarStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled = self.options.is_enabled;
        let options = |label: &str, axis, divider| ToolbarOptions {
            label: label.into(),
            axis,
            divider,
            is_enabled: enabled,
        };
        let commands = ToolbarCommands::try_new(self.items(), enabled, ToolbarAxis::Horizontal)
            .expect("unique command IDs");
        let adaptive = Toolbar::new(commands)
            .options(options(
                "Adaptive project commands",
                ToolbarAxis::Horizontal,
                ToolbarDividerEdge::Bottom,
            ))
            .debug_name("toolbar-commands");
        let spacer = || {
            AnyView::new(
                BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(cove_ui_compose::EmptyView),
                )
                .flex(1.0),
            )
        };
        let playback = Toolbar::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::Wrap,
                TOOLBAR_SPACING,
                AnyView::new(TupleView::new(vec![
                    self.playback(),
                    AnyView::new(ToolbarSeparator),
                    self.formatting(),
                    spacer(),
                    AnyView::new(
                        TextField::new("Find in project", self.search.binding())
                            .behavior(TextFieldBehavior {
                                disabled: !enabled,
                                ..Default::default()
                            })
                            .frame_width(160.0),
                    ),
                    self.zoom(),
                ])),
            )
            .flex_with(1.0, 1.0, Some(0.0))
            .frame_min_width(0.0),
        )
        .options(options(
            "Playback and formatting",
            ToolbarAxis::Horizontal,
            ToolbarDividerEdge::Bottom,
        ));
        let search = self.search.clone();
        let applied = self.applied_action.clone();
        let apply = Toolbar::new(TupleView::new(vec![
            AnyView::new(
                note("A bar below its content puts the divider on top.")
                    .flex_with(1.0, 1.0, Some(0.0))
                    .frame_min_width(0.0),
            ),
            spacer(),
            AnyView::new(Button::title("Apply", move || {
                applied.set(if search.get().is_empty() {
                    "None".into()
                } else {
                    search.get().string_value()
                })
            })),
        ]))
        .options(options(
            "Apply bar",
            ToolbarAxis::Horizontal,
            ToolbarDividerEdge::Top,
        ));
        let tools = Toolbar::new(TupleView::new(vec![
            self.playback(),
            AnyView::new(ToolbarSeparator),
            self.formatting(),
        ]))
        .options(options(
            "Tools",
            ToolbarAxis::Vertical,
            ToolbarDividerEdge::Trailing,
        ))
        .frame_height(220.0);
        let vertical = (1..=8)
            .map(|i| {
                let applied = self.applied_action.clone();
                ToolbarItem::new(MenuItem::new(i, format!("Tool {i}"), move || {
                    applied.set(format!("Tool {i}"))
                }))
                .icon(ButtonIconSource::resource(icons::action::plus()))
                .placement(if i == 1 {
                    ToolbarItemPlacement::Always
                } else {
                    ToolbarItemPlacement::AutomaticWithPriority(-(i as i32))
                })
            })
            .collect();
        let vertical = Toolbar::new(
            ToolbarCommands::try_new(vertical, enabled, ToolbarAxis::Vertical)
                .expect("unique tool IDs"),
        )
        .options(options(
            "Vertical commands",
            ToolbarAxis::Vertical,
            ToolbarDividerEdge::Bottom,
        ))
        .frame_height(180.0);
        AnyView::new(StorySection::new(
            "Toolbar",
            "Left/Right and Home/End move focus between controls; the search field keeps its caret keys. Disabled blocks the entire bar.",
            stack(vec![
                AnyView::new(adaptive),
                note(
                    "Resize this bar: Run stays visible; lower priority commands move into More actions.",
                ),
                AnyView::new(playback),
                AnyView::new(apply),
                row(vec![
                    AnyView::new(tools),
                    AnyView::new(vertical),
                    note(
                        "Vertical bars use Up/Down and Home/End; commands overflow by height into a menu beside the bar.",
                    ),
                ]),
                note(if self.is_playing() {
                    "Playback running"
                } else {
                    "Playback stopped"
                }),
                note(format!(
                    "Zoom: {}% · Applied filter: {}",
                    self.zoom_percent.get(),
                    self.applied_action()
                )),
            ]),
        ))
    }
    fn tag(&self) -> &'static str {
        "ToolbarStory"
    }
    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; is_playing,applied_action,is_bold,search,zoom_percent)
    }
}
