//! Top-level destinations that follow platform conventions: a bottom tab bar
//! on compact windows, a navigation rail on wider touch surfaces, tabs above
//! the content on pointer desktops.
//!
//! The selected destination's content keeps one slot in every presentation,
//! so rotating, folding or resizing never rebuilds it.
use std::hash::Hash;

use cove_composition::Scope;
use cove_core::scene::FlexDirection;
use cove_core::{Binding, Environment, WidthClass};

use crate::adaptive::SurfaceReader;
use crate::primitives::DuplicateId;
use crate::primitives::layout::sidebar::SidebarItem;
use crate::primitives::layout::tab_view::{TabItem, TabView};
use crate::primitives::layout::{MobileTabBar, NavigationRail};
use crate::primitives::navigation::list_detail::{Pane, PaneContainer, PaneLayout};
use crate::{AnyView, TupleView, View};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabsPresentation {
    BottomBar,
    Rail,
    TopTabs,
}

impl TabsPresentation {
    pub fn resolve(environment: &Environment) -> Self {
        if environment.width_class() == WidthClass::Compact {
            Self::BottomBar
        } else if environment.device_class.is_handheld()
            || environment.device_class == cove_core::DeviceClass::Tablet
            || !environment.supports_hover()
        {
            Self::Rail
        } else {
            Self::TopTabs
        }
    }
}

/// Destinations with one selected at a time.
pub struct AdaptiveTabs<Id: Hash + Eq + Clone + Send + Sync + 'static> {
    selection: Binding<Id>,
    tabs: Vec<TabItem<Id>>,
}

impl<Id: Hash + Eq + Clone + Send + Sync + 'static> AdaptiveTabs<Id> {
    pub fn try_new(selection: Binding<Id>, tabs: Vec<TabItem<Id>>) -> Result<Self, DuplicateId> {
        crate::primitives::check_ids(tabs.iter().map(|tab| &tab.id), "Tab")?;
        Ok(Self { selection, tabs })
    }
}

impl<Id: Hash + Eq + Clone + Send + Sync + 'static> View for AdaptiveTabs<Id> {
    fn body(&self, _cx: &Scope) -> AnyView {
        let selection = self.selection.clone();
        let tabs = self.tabs.clone();
        AnyView::new(SurfaceReader::new(move |environment: &Environment| {
            let presentation = TabsPresentation::resolve(environment);
            let selected = selection.get();
            let content = tabs
                .iter()
                .find(|tab| tab.id == selected)
                .or_else(|| tabs.first())
                .map(|tab| tab.content.clone())
                .unwrap_or_else(|| AnyView::new(crate::EmptyView));
            let navigation_only: Vec<TabItem<Id>> = tabs
                .iter()
                .map(|tab| TabItem {
                    content: AnyView::new(crate::EmptyView),
                    ..tab.clone()
                })
                .collect();
            let top = match presentation {
                TabsPresentation::TopTabs => {
                    TabView::try_new(selection.clone(), navigation_only.clone())
                        .map(AnyView::new)
                        .unwrap_or_else(|_| AnyView::new(crate::EmptyView))
                }
                _ => AnyView::new(crate::EmptyView),
            };
            let rail = match presentation {
                TabsPresentation::Rail => {
                    let optional = {
                        let selection = selection.clone();
                        Binding::new(
                            {
                                let selection = selection.clone();
                                move || Some(selection.get())
                            },
                            move |value: Option<Id>| {
                                if let Some(value) = value {
                                    selection.set(value);
                                }
                            },
                        )
                    };
                    let items = tabs
                        .iter()
                        .map(|tab| {
                            let item = SidebarItem::new(tab.id.clone(), tab.label.clone());
                            match &tab.icon {
                                Some(icon) => item.icon(icon.clone()),
                                None => item,
                            }
                        })
                        .collect();
                    NavigationRail::try_new(optional, items)
                        .map(AnyView::new)
                        .unwrap_or_else(|_| AnyView::new(crate::EmptyView))
                }
                _ => AnyView::new(crate::EmptyView),
            };
            let bottom = match presentation {
                TabsPresentation::BottomBar => {
                    AnyView::new(MobileTabBar::new(navigation_only, selection.clone(), true))
                }
                _ => AnyView::new(crate::EmptyView),
            };
            let middle = AnyView::new(PaneContainer {
                direction: FlexDirection::Row,
                content: AnyView::new(TupleView::new(vec![
                    AnyView::new(Pane {
                        layout: PaneLayout::intrinsic(presentation == TabsPresentation::Rail),
                        role: "adaptive-tabs-rail",
                        content: rail,
                    }),
                    AnyView::new(Pane {
                        layout: PaneLayout::fill(true),
                        role: "adaptive-tabs-content",
                        content,
                    }),
                ])),
            });
            AnyView::new(PaneContainer {
                direction: FlexDirection::Column,
                content: AnyView::new(TupleView::new(vec![
                    AnyView::new(Pane {
                        layout: PaneLayout::intrinsic(presentation == TabsPresentation::TopTabs),
                        role: "adaptive-tabs-top",
                        content: top,
                    }),
                    AnyView::new(Pane {
                        layout: PaneLayout::fill(true),
                        role: "adaptive-tabs-body",
                        content: middle,
                    }),
                    AnyView::new(Pane {
                        layout: PaneLayout::intrinsic(presentation == TabsPresentation::BottomBar),
                        role: "adaptive-tabs-bottom",
                        content: bottom,
                    }),
                ])),
            })
        }))
    }
}
