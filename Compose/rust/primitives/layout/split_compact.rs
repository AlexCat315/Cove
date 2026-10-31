//! Translation of Primitives/Layout/Split/Compact.swift.
//!
//! Opt-in single-pane presentation for a split that has room for only one pane.
use std::rc::Rc;

use cove_composition::{AnyView, View};
use cove_core::Binding;
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Wrap};

use crate::BoxView;
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt};
use crate::primitives::controls::button::Button;
use crate::primitives::overlays::roving_focus::{RovingFocusHost, RovingItem, roving_identity};
use crate::primitives::text::Text;
use crate::theme::styles::buttons::{ButtonStyleViewExt, TabButtonStyle};

/// The pane presented when a split has room for only one pane.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SplitPane {
    #[default]
    First,
    Second,
}

impl SplitPane {
    pub fn all() -> [Self; 2] {
        [Self::First, Self::Second]
    }
}

/// Opt-in single-pane presentation, measured along the split's allocated axis.
#[derive(Clone)]
pub struct SplitCompactPresentation {
    pub first_title: String,
    pub second_title: String,
    pub threshold: f32,
    /// Omit to let the split retain its own selection, initially `First`.
    pub selection: Option<Binding<SplitPane>>,
}

impl SplitCompactPresentation {
    pub fn new(first_title: impl Into<String>, second_title: impl Into<String>) -> Self {
        Self {
            first_title: first_title.into(),
            second_title: second_title.into(),
            threshold: 600.0,
            selection: None,
        }
    }

    pub fn threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold;
        self
    }

    pub fn selection(mut self, selection: Binding<SplitPane>) -> Self {
        self.selection = Some(selection);
        self
    }

    pub fn resolved_threshold(&self) -> f32 {
        if self.threshold.is_finite() {
            self.threshold.max(0.0)
        } else {
            600.0
        }
    }
}

/// Segmented control that chooses which pane is shown.
pub struct SplitPaneSelector {
    pub presentation: SplitCompactPresentation,
    pub selection: Binding<SplitPane>,
}

impl SplitPaneSelector {
    pub fn new(presentation: SplitCompactPresentation, selection: Binding<SplitPane>) -> Self {
        Self {
            presentation,
            selection,
        }
    }
}

impl View for SplitPaneSelector {
    fn body(&self) -> AnyView {
        let presentation = self.presentation.clone();
        let selection = self.selection.clone();

        let host_selection = selection.clone();
        let host_on_select = selection.clone();
        let identity: Rc<dyn Fn(&SplitPane) -> u64> = Rc::new(|pane| roving_identity(pane));
        let host = RovingFocusHost::new(
            SplitPane::all().to_vec(),
            move || Some(host_selection.get()),
            move |pane| host_on_select.set(pane),
            {
                let identity = identity.clone();
                move |pane: &SplitPane| identity(pane)
            },
            {
                let row_selection = selection.clone();
                let first_title = presentation.first_title.clone();
                let second_title = presentation.second_title.clone();
                let row = BoxView::new(
                    FlexDirection::Row,
                    Align::Stretch,
                    cove_core::scene::Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(crate::TupleView::new(vec![
                        pane_item(
                            SplitPane::First,
                            first_title,
                            row_selection.clone(),
                            identity.clone(),
                        ),
                        pane_item(SplitPane::Second, second_title, row_selection, identity),
                    ])),
                );
                row
            },
        );

        AnyView::new(host.accessibility(move |semantics| {
            semantics.role = AccessibilityRole::Group;
            semantics.label = "Choose split pane".to_string();
        }))
    }
}

fn pane_item(
    pane: SplitPane,
    title: String,
    selection: Binding<SplitPane>,
    identity: Rc<dyn Fn(&SplitPane) -> u64>,
) -> AnyView {
    let is_selected = selection.get() == pane;
    let action_selection = selection.clone();
    let action_pane = pane;
    let label = Text::new(title.clone())
        .frame_min_width(0.0)
        .flex_with(1.0, 1.0, Some(0.0));
    let button = Button::new(label, move || action_selection.set(action_pane))
        .button_style(TabButtonStyle::new(0.0))
        .accessibility(move |semantics| {
            semantics.role = AccessibilityRole::Tab;
            semantics.label = title.clone();
            semantics.state.is_selected = is_selected;
        })
        .frame_min_width(0.0)
        .flex_with(1.0, 1.0, Some(0.0));
    AnyView::new(crate::ModifiedContent::new(
        AnyView::new(button),
        RovingItem::new(pane, is_selected, identity),
    ))
}
