//! Translation of Primitives/Layout/Tabs/MenuStrip.swift and the
//! `TabOverflowLayout` in Primitives/Layout/Tabs/StripLayout.swift.
//!
//! Measures the same retained headers for both the strip and menu policy. Only
//! the selected header may shrink; its non-text chrome stays intact. Tabs that do
//! not fit are collected into a "more" menu.
use crate::Scope;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use crate::{Layout, LayoutContainer, LayoutSubview, ProposedViewSize};
use cove_composition::{AnyView, View};
use cove_core::Binding;
use cove_core::scene::{Align, Direction, FlexDirection, Point, Rect, Size, Wrap};

use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt};
use crate::primitives::controls::button::{Button, ButtonIconSource};
use crate::primitives::layout::scroll_view::ScrollView;
use crate::primitives::layout::tab_view::TabStripItem;
use crate::primitives::layout::tab_view::{IndicatorPlacement, TabItem, TabVariant};
use crate::primitives::media::ui_icon_catalog;
use crate::primitives::overlays::anchored::PopoverPlacement;
use crate::primitives::overlays::menu::{MenuEntry, MenuItem};
use crate::primitives::overlays::popover::Popover;
use crate::primitives::overlays::roving_focus::{RovingFocusRequest, RovingItem, roving_identity};
use crate::theme::styles::buttons::ButtonStyleViewExt;
use crate::{BoxView, TupleView};

/// Custom layout that measures which headers fit and reports the hidden ones.
#[derive(Clone)]
pub struct TabOverflowLayout {
    selected_index: Option<usize>,
    spacing: f32,
    on_hidden: Rc<dyn Fn(&[usize])>,
}

impl TabOverflowLayout {
    pub fn new(
        selected_index: Option<usize>,
        spacing: f32,
        on_hidden: impl Fn(&[usize]) + 'static,
    ) -> Self {
        Self {
            selected_index,
            spacing,
            on_hidden: Rc::new(on_hidden),
        }
    }

    fn arrangement(&self, proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> Plan {
        let mut sizes: Vec<Size> = subviews
            .iter()
            .map(|subview| safe_size(subview.size_that_fits(ProposedViewSize::UNSPECIFIED)))
            .collect();
        // The last subview is the "more" column; the rest are the tab headers.
        let count = if subviews.is_empty() {
            0
        } else {
            subviews.len() - 1
        };
        if count == 0 {
            return Plan {
                sizes,
                visible: HashSet::new(),
                hidden: Vec::new(),
                size: Size::default(),
            };
        }
        let width_of = |sizes: &[Size], indices: &HashSet<usize>| -> f32 {
            indices.iter().map(|index| sizes[*index].width).sum::<f32>()
                + self.spacing * indices.len().saturating_sub(1) as f32
        };
        let all: HashSet<usize> = (0..count).collect();
        let available = proposal.width.unwrap_or_else(|| width_of(&sizes, &all));
        let mut visible = all.clone();
        if width_of(&sizes, &all) > available {
            let selected = self
                .selected_index
                .filter(|index| all.contains(index))
                .unwrap_or(0);
            let more_width = if count > 1 {
                sizes[count].width + self.spacing
            } else {
                0.0
            };
            let selected_proposal =
                ProposedViewSize::new(Some((available - more_width).max(44.0)), None);
            sizes[selected] = safe_size(subviews[selected].size_that_fits(selected_proposal));
            visible = HashSet::new();
            visible.insert(selected);
            if count > 1 {
                visible.insert(count);
            }
            for index in 0..count {
                if index == selected {
                    continue;
                }
                let mut candidate = visible.clone();
                candidate.insert(index);
                if width_of(&sizes, &candidate) <= available {
                    visible = candidate;
                }
            }
        }
        let hidden: Vec<usize> = (0..count)
            .filter(|index| !visible.contains(index))
            .collect();
        let visible_width = width_of(&sizes, &visible);
        let height = visible
            .iter()
            .map(|index| sizes[*index].height)
            .fold(0.0_f32, f32::max);
        Plan {
            sizes,
            visible,
            hidden,
            size: Size {
                width: available.max(visible_width),
                height,
            },
        }
    }
}

impl Layout for TabOverflowLayout {
    fn size_that_fits(&self, proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> Size {
        self.arrangement(proposal, subviews).size
    }

    fn place_subviews(
        &self,
        bounds: Rect,
        _proposal: ProposedViewSize,
        subviews: &[LayoutSubview],
    ) {
        let plan = self.arrangement(ProposedViewSize::new(Some(bounds.width), None), subviews);
        let rtl = subviews
            .first()
            .map(|subview| subview.layout_direction == Direction::Rtl)
            .unwrap_or(false);
        let mut cursor = if rtl { bounds.max_x() } else { bounds.x };
        for index in 0..subviews.len() {
            if !plan.visible.contains(&index) {
                subviews[index].place(
                    Point::new(bounds.x, bounds.y),
                    ProposedViewSize::new(Some(0.0), Some(0.0)),
                    false,
                );
                continue;
            }
            let size = plan.sizes[index];
            let x = if rtl { cursor - size.width } else { cursor };
            subviews[index].place(
                Point::new(x, bounds.y + (plan.size.height - size.height) / 2.0),
                ProposedViewSize::new(Some(size.width), Some(size.height)),
                true,
            );
            cursor += if rtl { -1.0 } else { 1.0 } * (size.width + self.spacing);
        }
        (self.on_hidden)(&plan.hidden);
    }
}

fn safe_size(size: Size) -> Size {
    Size {
        width: if size.width.is_finite() {
            size.width.max(0.0)
        } else {
            0.0
        },
        height: if size.height.is_finite() {
            size.height.max(0.0)
        } else {
            0.0
        },
    }
}

struct Plan {
    sizes: Vec<Size>,
    visible: HashSet<usize>,
    hidden: Vec<usize>,
    size: Size,
}

/// Horizontal strip that keeps fitting tabs and lists the rest in a menu.
pub struct TabOverflowStrip<Id: std::hash::Hash + Clone + PartialEq + 'static> {
    tabs: Vec<TabItem<Id>>,
    selection: Binding<Id>,
    tab_stop: Option<Id>,
    is_enabled: bool,
    variant: TabVariant,
    indicator: IndicatorPlacement,
    focus_request: Rc<RefCell<RovingFocusRequest>>,
}

impl<Id: std::hash::Hash + Clone + PartialEq + 'static> TabOverflowStrip<Id> {
    pub fn new(
        tabs: Vec<TabItem<Id>>,
        selection: Binding<Id>,
        tab_stop: Option<Id>,
        is_enabled: bool,
        variant: TabVariant,
        indicator: IndicatorPlacement,
        focus_request: Rc<RefCell<RovingFocusRequest>>,
    ) -> Self {
        Self {
            tabs,
            selection,
            tab_stop,
            is_enabled,
            variant,
            indicator,
            focus_request,
        }
    }
}

impl<Id: std::hash::Hash + Clone + PartialEq + 'static> View for TabOverflowStrip<Id> {
    fn body(&self, cx: &Scope) -> AnyView {
        let tabs = self.tabs.clone();
        let selection = self.selection.clone();
        let is_enabled = self.is_enabled;
        let variant = self.variant;
        let indicator = self.indicator;
        let tab_stop = self.tab_stop.clone();
        let focus_request = self.focus_request.clone();
        let hidden_state = cx.state(Vec::<usize>::new);
        let presented_state = cx.state(|| false);
        if !is_enabled && presented_state.get() {
            presented_state.set(false);
        }

        let reader = crate::adaptive::ContainerSizeReader::new(move |size: Size| {
            let spacing = if variant == TabVariant::Segmented {
                2.0
            } else {
                0.0
            };
            let maximum_header_width = if size.width > 0.0 {
                let reserved = if tabs.len() > 1 { 44.0 } else { 0.0 }
                    + if variant == TabVariant::Segmented {
                        2.0
                    } else {
                        0.0
                    };
                (size.width - reserved).max(44.0)
            } else {
                f32::MAX
            };

            let selected_index = tabs.iter().position(|tab| tab.id == selection.get());

            let layout_hidden = hidden_state.clone();
            let layout_presented = presented_state.clone();
            let layout_selection = selection.clone();
            let layout_focus = focus_request.clone();
            let layout = TabOverflowLayout::new(selected_index, spacing, move |indices| {
                if layout_hidden.get() == indices {
                    return;
                }
                // A resize that reshuffles the menu closes it and returns focus
                // to the selected tab.
                if layout_presented.get() {
                    layout_focus.borrow_mut().target =
                        Some(roving_identity(&layout_selection.get()));
                    layout_presented.set(false);
                }
                layout_hidden.set(indices.to_vec());
            });

            let mut content_items: Vec<AnyView> = Vec::new();
            let identity_roving: Rc<dyn Fn(&Id) -> u64> = Rc::new(|id| roving_identity(id));
            for tab in &tabs {
                let is_selected = selected_index
                    .map(|index| {
                        tabs.get(index)
                            .map(|active| active.id == tab.id)
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
                let item = TabStripItem::new(
                    tab.clone(),
                    selection.clone(),
                    is_enabled && tab.is_enabled,
                    variant,
                    indicator,
                    is_selected,
                )
                .maximum_width(maximum_header_width);
                let is_tab_stop = tab_stop.as_ref() == Some(&tab.id);
                let item = crate::ModifiedContent::new(
                    AnyView::new(item),
                    RovingItem::new(tab.id.clone(), is_tab_stop, identity_roving.clone()),
                );
                let mut item_view = AnyView::new(item);
                if maximum_header_width.is_finite() {
                    item_view = AnyView::new(item_view.frame(
                        None,
                        None,
                        Some(0.0),
                        None,
                        Some(maximum_header_width),
                        None,
                    ));
                }
                content_items.push(item_view);
            }

            // More button + overflow menu.
            let more_presented = presented_state.binding();
            let hidden_for_menu = hidden_state.clone();
            let menu_tabs = tabs.clone();
            let menu_selection = selection.clone();
            let menu_presented = presented_state.clone();
            let mut menu_entries: Vec<MenuEntry> = Vec::new();
            for hidden_index in hidden_for_menu.get() {
                let Some(tab) = menu_tabs.get(hidden_index) else {
                    continue;
                };
                let id = roving_identity(&tab.id);
                let title = tab.label.clone();
                let item_enabled = is_enabled && tab.is_enabled;
                let item_selected = tab.id == menu_selection.get();
                let action_selection = menu_selection.clone();
                let action_target = tab.id.clone();
                let action_presented = menu_presented.clone();
                let item = MenuItem::new(id, title, move || {
                    action_selection.set(action_target.clone());
                    action_presented.set(false);
                })
                .enabled(item_enabled)
                .selected(item_selected);
                menu_entries.push(MenuEntry::Item(item));
            }
            let more_button = crate::primitives::media::Icon::new(ui_icon_catalog::action::more_horizontal(), 16.0)
                .frame(None, None, Some(44.0), Some(44.0), None, None);
            let more_column = BoxView::new(
                FlexDirection::Column,
                Align::FlexStart,
                cove_core::scene::Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(
                    Popover::new(
                        more_button,
                        AnyView::new(crate::primitives::overlays::menu::Menu::new(menu_entries)),
                    )
                    .is_presented(more_presented)
                    .enabled(is_enabled)
                    .trigger_style(MoreTabsTriggerStyle)
                    .trigger_description(crate::primitives::overlays::popover::PopoverTriggerDescription {
                        label: "More tabs".to_string(), value: String::new(), help: Some("More tabs".to_string()),
                    })
                    .placement(PopoverPlacement::BottomTrailing)
                    .width(220.0),
                ),
            );
            content_items.push(AnyView::new(more_column));

            let fitted = LayoutContainer::new(layout, AnyView::new(TupleView::new(content_items)))
                .frame(
                    None,
                    None,
                    Some(if tabs.is_empty() {
                        0.0
                    } else if tabs.len() > 1 {
                        90.0
                    } else {
                        44.0
                    }),
                    None,
                    None,
                    None,
                )
                .flex_with(0.0, 0.0, None);

            let scrolled = ScrollView::horizontal(fitted)
                .scrollbar_gutter(crate::primitives::layout::ScrollbarGutter::Overlay)
                .flex_with(0.0, 0.0, None);
            AnyView::new(scrolled)
        });
        AnyView::new(reader)
    }
}

struct MoreTabsTriggerStyle;
impl crate::theme::styles::buttons::ButtonStyle for MoreTabsTriggerStyle {
    fn make_body(&self, configuration: crate::theme::styles::buttons::ButtonStyleConfiguration) -> AnyView {
        let body = crate::theme::styles::buttons::ButtonStyle::make_body(&crate::theme::styles::buttons::GhostButtonStyle, configuration);
        AnyView::new(body.frame(None, None, Some(44.0), Some(44.0), None, None))
    }
}
