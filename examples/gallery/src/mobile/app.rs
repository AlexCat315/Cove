//! Translation of Examples/Mobile/Sources/AppContent/App.swift.
//!
//! The shared mobile root: desktop and mobile hosts install this once per
//! application session. Step up to a phone surface on desktop with
//! `COVE_PREVIEW_DEVICE=phone`.
//!
//! Sheets, dialogs and menus present in the layer root's portal layer, so the
//! root wraps `LayerRoot` rather than a plain column.
use cove_composition::Scope;
use cove_core::State;
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::layout::TabView;
use cove_ui_compose::primitives::layout::{SafeArea, SafeAreaEdges};
use cove_ui_compose::primitives::media::ui_icon_catalog;
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::theme::SemanticColorRef;
use cove_ui_compose::{AnyView, View};

use crate::mobile::model::MobileModel;
use crate::mobile::settings::SettingsScreen;
use crate::mobile::today::TodayScreen;

/// The three mobile tabs, in the order the strip shows them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MobileTab {
    Today,
    Settings,
}

impl MobileTab {
    pub const ALL: [Self; 2] = [Self::Today, Self::Settings];

    pub fn label(self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::Settings => "Settings",
        }
    }
}

/// The shared mobile root.
pub struct MobileRoot {
    tab: State<MobileTab>,
    model: State<MobileModel>,
}

impl MobileRoot {
    pub fn new() -> Self {
        Self {
            tab: State::new(MobileTab::Today),
            model: State::new(MobileModel::default()),
        }
    }

    pub fn selected_tab(&self) -> MobileTab {
        self.tab.get()
    }

    pub fn tab(&self) -> State<MobileTab> {
        self.tab.clone()
    }

    pub fn model(&self) -> State<MobileModel> {
        self.model.clone()
    }
}

impl Default for MobileRoot {
    fn default() -> Self {
        Self::new()
    }
}

/// The shared root every mobile and desktop host installs.
pub fn make_application_content() -> MobileRoot {
    MobileRoot::new()
}

impl View for MobileRoot {
    fn body(&self, _cx: &Scope) -> AnyView {
        let tabs = vec![
            cove_ui_compose::primitives::layout::TabItem::new(
                MobileTab::Today,
                MobileTab::Today.label(),
                TodayScreen::new(self.model.binding()),
            )
            .icon(ui_icon_catalog::shape::home()),
            cove_ui_compose::primitives::layout::TabItem::new(
                MobileTab::Settings,
                MobileTab::Settings.label(),
                SettingsScreen::new(self.model.get()),
            )
            .icon(ui_icon_catalog::action::settings()),
        ];

        let mut tabs_view = TabView::try_new(self.tab.binding(), tabs).expect("unique tab IDs");
        tabs_view.mobile = true;
        let view = tabs_view.flex_with(1.0, 1.0, Some(0.0)).frame(
            None,
            None,
            Some(0.0),
            Some(0.0),
            None,
            None,
        );

        let safe = SafeArea::new(
            SafeAreaEdges(SafeAreaEdges::TOP.0 | SafeAreaEdges::HORIZONTAL.0),
            AnyView::new(view),
        )
        .flex_with(1.0, 1.0, Some(0.0))
        .frame(None, None, Some(0.0), Some(0.0), None, None);

        AnyView::new(
            AnyView::new(LayerRoot::new(AnyView::new(safe)))
                .semantic_background(SemanticColorRef::background()),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_ui_compose::DynamicField> {
        vec![
            cove_ui_compose::DynamicField::state("tab", &self.tab),
            cove_ui_compose::DynamicField::state("model", &self.model),
        ]
    }
}
