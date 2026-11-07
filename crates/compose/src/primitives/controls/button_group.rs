//! Translation of Primitives/Layout/NavigationComponents.swift (button group).
//!
//! A segmented single-selection control: one Tab stop across its items and
//! wrapping arrow-key navigation, so the group reads as one control to the
//! keyboard while each segment keeps its authored width.
use crate::Scope;
use crate::primitives::{DuplicateId, check_ids};
use std::rc::Rc;

use crate::animation::geometry::{SlidingSelection, SlidingSelectionStyle};
use crate::primitives::controls::button::{Button, ButtonRole};
use crate::primitives::overlays::roving_focus::{RovingFocusHost, RovingItem, roving_identity};
use crate::theme::styles::buttons::{ButtonStyleViewExt, SegmentButtonStyle};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View};
use cove_core::Binding;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};

/// One segment of a button group.
#[derive(Clone, Debug, PartialEq)]
pub struct ButtonGroupItem<Id: PartialEq + Clone> {
    pub id: Id,
    pub title: String,
    pub is_enabled: bool,
}

impl<Id: PartialEq + Clone> ButtonGroupItem<Id> {
    pub fn new(id: Id, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            is_enabled: true,
        }
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }
}

/// Segmented single-selection control.
pub struct ButtonGroup<Id: PartialEq + Clone + std::hash::Hash + Send + Sync + 'static> {
    pub(crate) items: Vec<ButtonGroupItem<Id>>,
    pub selection: Binding<Option<Id>>,
    pub is_enabled: bool,
}

impl<Id: PartialEq + Clone + std::hash::Hash + Send + Sync + 'static> ButtonGroup<Id> {
    pub fn try_new(
        items: Vec<ButtonGroupItem<Id>>,
        selection: Binding<Option<Id>>,
    ) -> Result<Self, DuplicateId>
    where
        Id: Eq,
    {
        check_ids(items.iter().map(|item| &item.id), "Button group")?;
        Ok(Self {
            items,
            selection,
            is_enabled: true,
        })
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    fn enabled_ids(&self) -> Vec<Id> {
        self.items
            .iter()
            .filter(|item| self.is_enabled && item.is_enabled)
            .map(|item| item.id.clone())
            .collect()
    }
}

impl<Id: PartialEq + Clone + std::hash::Hash + Send + Sync + 'static> View for ButtonGroup<Id> {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled = self.enabled_ids();
        let tab_stop = self
            .selection
            .get()
            .filter(|selected| enabled.contains(selected))
            .or_else(|| enabled.first().cloned());
        let selected = self.selection.get();
        let mut segments: Vec<AnyView> = Vec::new();
        for item in &self.items {
            let is_item_enabled = self.is_enabled && item.is_enabled;
            let is_selected = selected.as_ref() == Some(&item.id);
            let selection = self.selection.clone();
            let id = item.id.clone();
            let button = Button::with_options(
                crate::primitives::text::Text::new(item.title.clone()),
                move || selection.set(Some(id.clone())),
                ButtonRole::Normal,
                is_item_enabled,
                false,
                is_selected,
                None,
            )
            .button_style(SegmentButtonStyle);
            let identity: Rc<dyn Fn(&Id) -> u64> = Rc::new(|value: &Id| roving_identity(value));
            let is_tab_stop = tab_stop.as_ref() == Some(&item.id);
            let roving = RovingItem::new(item.id.clone(), is_tab_stop, identity);
            segments.push(AnyView::new(crate::ModifiedContent::new(
                AnyView::new(button),
                roving,
            )));
        }
        let selection_for_get = self.selection.clone();
        let selection_for_set = self.selection.clone();
        let enabled_for_host = enabled.clone();
        let identity: Rc<dyn Fn(&Id) -> u64> = Rc::new(|value: &Id| roving_identity(value));
        let host = RovingFocusHost::new(
            enabled,
            move || selection_for_get.get(),
            move |value: Id| {
                if enabled_for_host.contains(&value) {
                    selection_for_set.set(Some(value));
                }
            },
            move |value: &Id| identity(value),
            // Segments keep their authored width; when the row cannot hold them,
            // wrap to a second line instead of squeezing labels or shrinking the
            // touch target.
            AnyView::new(SlidingSelection::new(
                |theme| SlidingSelectionStyle {
                    fill: segment_thumb_fill(theme),
                    corner_radius: theme.radius.pill,
                    shadow: theme.elevation.low,
                },
                AnyView::new(BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::Wrap,
                    2.0,
                    AnyView::new(TupleView::new(segments)),
                )),
            )),
        );
        AnyView::new(ButtonGroupChrome {
            content: AnyView::new(host),
        })
    }
}

/// The thumb is the raised face of the well: white on a light well, a lighter
/// gray on a dark one.
fn segment_thumb_fill(theme: &crate::theme::Theme) -> cove_core::Color {
    let well = theme.colors.surface_pressed;
    if well.r + well.g + well.b > 1.5 {
        theme.colors.surface
    } else {
        theme.colors.border_strong
    }
}

/// Group chrome: a capsule well behind the segments.
struct ButtonGroupChrome {
    content: AnyView,
}

impl PrimitiveView for ButtonGroupChrome {
    fn make_node(&self) -> cove_core::scene::Node {
        let node = cove_core::scene::Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_padding(3.0, cove_core::scene::Edge::All);
        Some(layout)
    }

    fn update_layout(&self, layout: &cove_core::scene::LayoutNode) {
        layout.set_padding(3.0, cove_core::scene::Edge::All);
    }

    fn update_node(&self, node: &cove_core::scene::Node) {
        let theme = crate::theme::theme_for_node(node);
        node.set_background_color(Some(theme.colors.surface_pressed));
        node.set_corner_radius(theme.radius.pill);
    }

    fn children(&self, _node: &cove_core::scene::Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
crate::impl_primitive_view!(ButtonGroupChrome);
