//! Translation of `Examples/Mobile/Sources/AppContent/Settings.swift`.
//!
//! The settings screen: three toggle groups, a disclosure row that opens an
//! appearance sheet, and a destructive sign-out row.
//!
//! Behaviour checklist:
//! - each toggle writes through to the shared mobile model
//! - the appearance row shows the current choice and opens a sheet
//! - choosing an appearance records it and closes the sheet
//! - the sheet offers exactly System, Light and Dark
//! - sign-out is destructive and carries no toggle

use std::cell::RefCell;
use std::rc::Rc;

use cove_core::State;
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::controls::Toggle;
use cove_ui_compose::primitives::controls::button::ButtonRole;
use cove_ui_compose::primitives::data::lists::inset_group::ListGroup;
use cove_ui_compose::primitives::data::lists::row::{ListRow, ListRowOptions};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View, ViewExt};

use crate::mobile::model::MobileModel;

/// The appearance choices the sheet offers, in authored order.
pub const APPEARANCE_CHOICES: [&str; 3] = ["System", "Light", "Dark"];
/// The sheet's detent height, in points.
pub const APPEARANCE_SHEET_HEIGHT: f32 = 260.0;

/// A settings section, which is just a named group of rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsSection {
    pub title: Option<String>,
    pub rows: Vec<SettingsRow>,
}

/// One settings row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingsRow {
    /// A label with a toggle on the trailing edge.
    Toggle { label: &'static str },
    /// A label with a disclosure indicator and a value.
    Disclosure { label: &'static str },
    /// A destructive action with no accessory.
    Destructive { label: &'static str },
}

impl SettingsRow {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Toggle { label } | Self::Disclosure { label } | Self::Destructive { label } => {
                label
            }
        }
    }

    pub fn shows_disclosure(&self) -> bool {
        matches!(self, Self::Disclosure { .. })
    }

    pub fn is_destructive(&self) -> bool {
        matches!(self, Self::Destructive { .. })
    }
}

/// The settings screen's sections, in authored order.
pub fn sections() -> Vec<SettingsSection> {
    vec![
        SettingsSection {
            title: Some("General".to_owned()),
            rows: vec![
                SettingsRow::Toggle {
                    label: "Notifications",
                },
                SettingsRow::Toggle {
                    label: "Sync over cellular",
                },
                SettingsRow::Disclosure {
                    label: "Appearance",
                },
            ],
        },
        SettingsSection {
            title: Some("Accessibility".to_owned()),
            rows: vec![SettingsRow::Toggle {
                label: "Reduce motion",
            }],
        },
        SettingsSection {
            // The sign-out group has no header, which is what makes it read as a
            // separate destructive affordance rather than a titled section.
            title: None,
            rows: vec![SettingsRow::Destructive { label: "Sign out" }],
        },
    ]
}

/// `SettingsScreen`: the mobile settings list.
pub struct SettingsScreen {
    model: Rc<RefCell<MobileModel>>,
    is_choosing_appearance: State<bool>,
    notifications: State<bool>,
    sync_over_cellular: State<bool>,
    reduce_motion: State<bool>,
}

impl SettingsScreen {
    pub fn new(model: MobileModel) -> Self {
        Self {
            notifications: State::new(model.notifications),
            sync_over_cellular: State::new(model.sync_over_cellular),
            reduce_motion: State::new(model.reduce_motion),
            model: Rc::new(RefCell::new(model)),
            is_choosing_appearance: State::new(false),
        }
    }

    pub fn model(&self) -> MobileModel {
        self.model.borrow().clone()
    }

    pub fn is_choosing_appearance(&self) -> bool {
        self.is_choosing_appearance.get()
    }

    pub fn open_appearance_sheet(&self) {
        self.is_choosing_appearance.set(true);
    }

    /// Records the appearance choice and closes the sheet.
    ///
    /// An unrecognised value is ignored rather than stored, so the model can only
    /// ever hold one of the three offered choices.
    pub fn choose_appearance(&self, choice: &str) -> bool {
        if !APPEARANCE_CHOICES.contains(&choice) {
            return false;
        }
        self.model.borrow_mut().appearance = choice.to_owned();
        self.is_choosing_appearance.set(false);
        true
    }

    pub fn set_notifications(&self, value: bool) {
        self.model.borrow_mut().notifications = value;
    }

    pub fn set_sync_over_cellular(&self, value: bool) {
        self.model.borrow_mut().sync_over_cellular = value;
    }

    pub fn set_reduce_motion(&self, value: bool) {
        self.model.borrow_mut().reduce_motion = value;
    }

    pub fn sections(&self) -> Vec<SettingsSection> {
        sections()
    }
}

impl View for SettingsScreen {
    fn body(&self) -> AnyView {
        let title =
            Text::new("Settings").semantic_font(cove_ui_compose::theme::SemanticFontRef::display());

        let general = {
            let model = self.model.clone();
            let _ = &model;
            let notifications = ListRow::new("Notifications")
                .trailing(AnyView::new(Toggle::new(self.notifications.binding())));
            let sync = ListRow::new("Sync over cellular")
                .trailing(AnyView::new(Toggle::new(self.sync_over_cellular.binding())));
            let appearance = ListRow::new("Appearance")
                .options(ListRowOptions {
                    shows_disclosure: true,
                    ..ListRowOptions::new()
                })
                .trailing(AnyView::new(
                    Text::new(model.borrow().appearance.clone())
                        .semantic_foreground(SemanticColorRef::on_surface_muted()),
                ));
            AnyView::new(
                ListGroup::new(AnyView::new(TupleView::new(vec![
                    AnyView::new(notifications),
                    AnyView::new(sync),
                    AnyView::new(appearance),
                ])))
                .title("General"),
            )
        };

        let accessibility = {
            let reduce = ListRow::new("Reduce motion")
                .trailing(AnyView::new(Toggle::new(self.reduce_motion.binding())));
            AnyView::new(ListGroup::new(AnyView::new(reduce)).title("Accessibility"))
        };

        let sign_out = ListRow::new("Sign out").options(ListRowOptions {
            role: ButtonRole::Destructive,
            ..ListRowOptions::new()
        });
        let account = AnyView::new(ListGroup::new(AnyView::new(sign_out)));

        AnyView::new(
            AnyView::new(
                AnyView::new(BoxView::new(
                    cove_core::scene::FlexDirection::Column,
                    cove_core::scene::Align::Stretch,
                    cove_core::scene::Justify::FlexStart,
                    cove_core::scene::Wrap::NoWrap,
                    16.0,
                    AnyView::new(TupleView::new(vec![
                        AnyView::new(title),
                        general,
                        accessibility,
                        account,
                    ])),
                ))
                .padding_edges(20.0, 8.0),
            )
            .debug_name("settings-screen"),
        )
    }

    fn tag(&self) -> &'static str {
        "SettingsScreen"
    }
}
