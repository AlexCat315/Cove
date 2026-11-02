//! Translation of Primitives/Layout/TabView.swift.
//!
//! Multi-panel container with a single active panel.
//!
//! Three variants cover the three places tabs realistically appear: `Bar` is a
//! document strip (editor panels, inspector pages), `Segmented` is a value picker
//! that happens to look like tabs, and `Card` is a folder metaphor for content
//! that sits on the same surface as the page.
//!
//! The strip keeps one Tab stop for the whole group: arrows, Home and End move
//! between enabled tabs, and disabled tabs are skipped rather than announced as
//! unavailable one by one.
//!
//! **Child materialisation is eager and delegated.** `TabItem::content` is an
//! `AnyView` built by the caller at construction, exactly as the Swift
//! `TabItem.init` does with `AnyView(content())`; this view only decides *which*
//! one to mount. Nothing here builds a tab's body lazily, and nothing here holds
//! a builder closure, so the caller owns materialisation and lifecycle.
use crate::Scope;
use crate::primitives::{DuplicateId, check_ids};
use std::cell::RefCell;
use std::rc::Rc;

use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::SemanticStyleViewExt;
use crate::primitives::controls::button::{Button, ButtonRole};
use crate::primitives::media::Icon;
use crate::primitives::overlays::roving_focus::{RovingFocusHost, RovingItem, roving_identity};
use crate::primitives::text::Text;
use crate::theme::styles::buttons::{ButtonStyleViewExt, TabButtonStyle};
use crate::theme::{SemanticColorRef, SemanticFontRef, ThemeMetric, theme_for_node};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View, ViewExt};
use cove_core::Binding;
use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};
use cove_imaging::BundleImageResource;

/// Visual treatment of a tab strip.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TabVariant {
    /// Underlined document strip sitting flush with the content.
    #[default]
    Bar,
    /// Single pill that selects a value rather than a document.
    Segmented,
    /// Raised card for the active tab.
    Card,
}

impl TabVariant {
    fn horizontal_padding(self) -> f32 {
        if self == Self::Segmented { 12.0 } else { 14.0 }
    }
}

/// Which edge of a tab view holds the strip.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TabStripPlacement {
    /// Bottom on compact surfaces; top on medium and expanded surfaces.
    #[default]
    Adaptive,
    Top,
    Bottom,
}

/// What a tab view does when its tabs are wider than the strip.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TabOverflow {
    /// Tabs wrap onto further lines.
    #[default]
    Wrap,
    /// The strip scrolls horizontally.
    Scroll,
    /// Keep the selected tab in the strip and list hidden tabs in a menu.
    Menu,
}

/// Where the selection indicator sits for a variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndicatorPlacement {
    Top,
    Bottom,
    None,
}

/// One tab: its identity, label and already-materialised content.
pub struct TabItem<Id: PartialEq + Clone + 'static> {
    pub id: Id,
    pub label: String,
    pub content: AnyView,
    pub is_enabled: bool,
    /// Leading glyph. Tabs read faster with one, and it survives translation.
    pub icon: Option<BundleImageResource>,
    /// Short trailing count or status. Keep it to two or three characters.
    pub badge: Option<String>,
    /// When set the tab renders a close control; `None` makes the tab permanent.
    pub on_close: Option<Rc<dyn Fn()>>,
}

impl<Id: PartialEq + Clone + 'static> Clone for TabItem<Id> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            label: self.label.clone(),
            content: self.content.clone(),
            is_enabled: self.is_enabled,
            icon: self.icon.clone(),
            badge: self.badge.clone(),
            on_close: self.on_close.clone(),
        }
    }
}

impl<Id: PartialEq + Clone + 'static> TabItem<Id> {
    /// Build a tab. `content` is materialised by the caller, matching the Swift
    /// `AnyView(content())` in `TabItem.init`.
    pub fn new(id: Id, label: impl Into<String>, content: impl View + 'static) -> Self {
        Self {
            id,
            label: label.into(),
            content: AnyView::new(content),
            is_enabled: true,
            icon: None,
            badge: None,
            on_close: None,
        }
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn icon(mut self, icon: BundleImageResource) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    pub fn on_close(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }
}

/// Multi-panel container with a single active panel.
pub struct TabView<Id: PartialEq + Clone + std::hash::Hash + 'static> {
    pub selection: Binding<Id>,
    pub(crate) tabs: Vec<TabItem<Id>>,
    pub variant: TabVariant,
    pub placement: TabStripPlacement,
    pub overflow: TabOverflow,
    pub is_enabled: bool,
    pub compact: bool,
    pub mobile: bool,
    focus_request: Rc<RefCell<crate::primitives::overlays::roving_focus::RovingFocusRequest>>,
}

impl<Id: PartialEq + Clone + std::hash::Hash + 'static> TabView<Id> {
    ///
    /// Tab IDs must be unique.
    pub fn try_new(selection: Binding<Id>, tabs: Vec<TabItem<Id>>) -> Result<Self, DuplicateId>
    where
        Id: Eq,
    {
        check_ids(tabs.iter().map(|item| &item.id), "Tab")?;
        Ok(Self {
            selection,
            tabs,
            variant: TabVariant::Bar,
            placement: TabStripPlacement::Adaptive,
            overflow: TabOverflow::Wrap,
            is_enabled: true,
            compact: false,
            mobile: false,
            focus_request: Rc::new(RefCell::new(
                crate::primitives::overlays::roving_focus::RovingFocusRequest::new(),
            )),
        })
    }

    pub fn variant(mut self, variant: TabVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn placement(mut self, placement: TabStripPlacement) -> Self {
        self.placement = placement;
        self
    }

    pub fn overflow(mut self, overflow: TabOverflow) -> Self {
        self.overflow = overflow;
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn snapshot(&self) -> TabView<Id> {
        TabView {
            selection: self.selection.clone(),
            tabs: self.tabs.clone(),
            variant: self.variant,
            placement: self.placement,
            overflow: self.overflow,
            is_enabled: self.is_enabled,
            compact: self.compact,
            mobile: self.mobile,
            focus_request: self.focus_request.clone(),
        }
    }

    fn enabled_ids(&self) -> Vec<Id> {
        self.tabs
            .iter()
            .filter(|tab| self.is_enabled && tab.is_enabled)
            .map(|tab| tab.id.clone())
            .collect()
    }

    /// The active tab: the bound one when enabled, else the first enabled one.
    fn active(&self, enabled: &[Id]) -> Option<Id> {
        // No substitution: Swift mounts nothing when the selection matches no
        // enabled tab (`TabView.swift:127`). Falling back to the first enabled tab
        // rendered tab 0's content for a selection the caller never authored.
        let selected = self.selection.get();
        enabled.contains(&selected).then_some(selected)
    }

    /// `Adaptive` resolves against the surface: a phone bar of plain tabs goes to
    /// the bottom, a compact width anywhere goes to the bottom, else the top.
    fn resolved_placement(&self) -> TabStripPlacement {
        let all_plain = self.tabs.iter().all(|tab| tab.on_close.is_none());
        let mobile = self.placement == TabStripPlacement::Adaptive
            && self.variant == TabVariant::Bar
            && all_plain
            && self.mobile;
        if mobile {
            return TabStripPlacement::Bottom;
        }
        match self.placement {
            TabStripPlacement::Adaptive => {
                if self.compact {
                    TabStripPlacement::Bottom
                } else {
                    TabStripPlacement::Top
                }
            }
            other => other,
        }
    }

    fn indicator(&self, placement: TabStripPlacement) -> IndicatorPlacement {
        if self.variant == TabVariant::Bar {
            if placement == TabStripPlacement::Top {
                IndicatorPlacement::Bottom
            } else {
                IndicatorPlacement::Top
            }
        } else {
            IndicatorPlacement::None
        }
    }
}

impl<Id: PartialEq + Clone + std::hash::Hash + 'static> View for TabView<Id> {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(TabViewBody {
            tabs_view: self.snapshot(),
        })
    }
}

struct TabViewBody<Id: PartialEq + Clone + std::hash::Hash + 'static> {
    tabs_view: TabView<Id>,
}

impl<Id: PartialEq + Clone + std::hash::Hash + 'static> PrimitiveView for TabViewBody<Id> {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let view = &self.tabs_view;
        let enabled = view.enabled_ids();
        let active = view.active(&enabled);
        let tab_stop = active.clone().or_else(|| enabled.first().cloned());
        let placement = view.resolved_placement();
        let indicator = view.indicator(placement);

        // Strip items, each with its roving identity.
        let identity: Rc<dyn Fn(&Id) -> u64> = Rc::new(|id: &Id| roving_identity(id));
        let mut strip_items: Vec<AnyView> = Vec::new();
        for tab in &view.tabs {
            let is_tab_stop = tab_stop.as_ref() == Some(&tab.id);
            let item = TabStripItem {
                tab: tab.clone(),
                selection: view.selection.clone(),
                is_enabled: view.is_enabled && tab.is_enabled,
                variant: view.variant,
                indicator,
                is_selected: active.as_ref() == Some(&tab.id),
            };
            strip_items.push(AnyView::new(crate::ModifiedContent::new(
                AnyView::new(item),
                RovingItem::new(tab.id.clone(), is_tab_stop, identity.clone()),
            )));
        }

        let strip = AnyView::new(TabStrip {
            variant: view.variant,
            overflow: view.overflow,
            content: AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                if view.overflow == TabOverflow::Wrap {
                    Wrap::Wrap
                } else {
                    Wrap::NoWrap
                },
                if view.variant == TabVariant::Segmented {
                    2.0
                } else {
                    0.0
                },
                AnyView::new(TupleView::new(strip_items)),
            )),
        });

        // Only the active tab's content is mounted; the rest were already built
        // by the caller and simply are not in this tree.
        let mut content: Vec<AnyView> = Vec::new();
        if let Some(active_id) = active.as_ref()
            && let Some(tab) = view
                .tabs
                .iter()
                .find(|tab| &tab.id == active_id && tab.is_enabled)
        {
            // Give the panel a per-tab identity. Without it the content slot is
            // reused across a switch, so two tabs hosting the same panel type
            // shared one `State` — bumping tab A's counter made tab B read it
            // too. Swift does `.id(active.id)` on the content slot
            // (`TabView.swift:128`). A hash-derived key avoids requiring `Eq` on
            // `Id`, which `TabView` does not bound.
            let identity = {
                use std::hash::Hasher;
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                tab.id.hash(&mut hasher);
                hasher.finish()
            };
            content.push(AnyView::new(
                AnyView::new(tab.content.clone().flex_with(1.0, 1.0, Some(0.0))).id(identity),
            ));
        }

        let mut column: Vec<AnyView> = Vec::new();
        if placement == TabStripPlacement::Top {
            column.push(strip);
            column.extend(content);
        } else {
            column.extend(content);
            column.push(strip);
        }

        let selection = view.selection.clone();
        let enabled_for_host = enabled.clone();
        let selection_for_get = view.selection.clone();
        let focus_request = view.focus_request.clone();
        let mut host = RovingFocusHost::new(
            enabled.clone(),
            // Read the binding at call time, not an `active` snapshot: two key
            // events can arrive inside one render pass, and a captured value would
            // send the second arrow from the previous selection.
            move || Some(selection_for_get.get()),
            move |id: Id| {
                if enabled_for_host.contains(&id) {
                    selection.set(id);
                }
            },
            move |id: &Id| roving_identity(id),
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(column)),
            )),
        );
        host.focus_request = Some(focus_request);
        let _ = tab_stop;
        vec![AnyView::new(host)]
    }
}
impl<Id: PartialEq + Clone + std::hash::Hash + 'static> View for TabViewBody<Id> {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
}

/// The strip chrome: canvas behind a bar, a pill tray for segmented, nothing for
/// cards.
struct TabStrip {
    variant: TabVariant,
    overflow: TabOverflow,
    content: AnyView,
}

impl PrimitiveView for TabStrip {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, node: &Node) {
        if self.variant == TabVariant::Segmented {
            let theme = theme_for_node(node);
            node.set_background_color(Some(theme.colors.surface_variant));
            node.set_corner_radius(theme.radius.md);
        } else {
            node.set_background_color(Some(theme_for_node(node).colors.surface));
            node.set_corner_radius(0.0);
        }
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let padding = if self.variant == TabVariant::Segmented {
            3.0
        } else {
            0.0
        };
        let styled =
            AnyView::new(AnyView::new(self.content.clone()).padding_edges(padding, padding));
        let _ = ThemeMetric::control_radius();
        if self.overflow == TabOverflow::Scroll {
            // A strip that must stay one line scrolls rather than wrapping.
            return vec![AnyView::new(
                crate::primitives::layout::ScrollView::new(
                    crate::primitives::layout::ScrollAxis::Horizontal,
                    styled,
                )
                .scrollbar_gutter(crate::primitives::layout::ScrollbarGutter::Overlay)
                .frame(None, None, None, Some(40.0), None, None),
            )];
        }
        vec![styled]
    }
}
crate::impl_primitive_view!(TabStrip);

/// One tab in the strip: icon, label, badge and optional close control.
pub(crate) struct TabStripItem<Id: PartialEq + Clone + 'static> {
    tab: TabItem<Id>,
    selection: Binding<Id>,
    is_enabled: bool,
    variant: TabVariant,
    indicator: IndicatorPlacement,
    is_selected: bool,
}

impl<Id: PartialEq + Clone + std::hash::Hash + 'static> TabStripItem<Id> {
    pub(crate) fn new(
        tab: TabItem<Id>,
        selection: Binding<Id>,
        is_enabled: bool,
        variant: TabVariant,
        indicator: IndicatorPlacement,
        is_selected: bool,
    ) -> Self {
        Self {
            tab,
            selection,
            is_enabled,
            variant,
            indicator,
            is_selected,
        }
    }
}

impl<Id: PartialEq + Clone + std::hash::Hash + 'static> PrimitiveView for TabStripItem<Id> {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_attachment("tab.id", self.tab.label.clone());
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let mut label_row: Vec<AnyView> = Vec::new();
        if let Some(icon) = self.tab.icon.clone() {
            label_row.push(AnyView::new(Icon::new(icon, 14.0)));
        }
        label_row.push(AnyView::new(
            Text::new(self.tab.label.clone()).semantic_font(SemanticFontRef::label()),
        ));
        if let Some(badge) = self.tab.badge.clone() {
            label_row.push(AnyView::new(
                Text::new(badge)
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
            ));
        }

        let selection = self.selection.clone();
        let id = self.tab.id.clone();
        let button = Button::with_options(
            AnyView::new(
                BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    6.0,
                    AnyView::new(TupleView::new(label_row)),
                )
                .padding_edges(self.variant.horizontal_padding(), 0.0),
            ),
            move || selection.set(id.clone()),
            ButtonRole::Normal,
            self.is_enabled,
            false,
            self.is_selected,
            None,
        )
        .button_style(TabButtonStyle::default());
        let button = AnyView::new(button);

        let mut row: Vec<AnyView> = vec![button];
        if let Some(on_close) = self.tab.on_close.clone() {
            let close = Button::with_options(
                AnyView::new(crate::primitives::media::Icon::with_semantic(
                    crate::primitives::media::ui_icon_catalog::action::x(),
                    10.0,
                    SemanticColorRef::on_surface_muted(),
                )),
                move || on_close(),
                ButtonRole::Normal,
                self.is_enabled,
                false,
                false,
                Some("Close tab".to_string()),
            );
            row.push(AnyView::new(close));
        }

        vec![AnyView::new(TabItemChrome {
            content: AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(row)),
            )),
            indicator: self.indicator,
            is_selected: self.is_selected,
            variant: self.variant,
        })]
    }
}
impl<Id: PartialEq + Clone + std::hash::Hash + 'static> View for TabStripItem<Id> {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
}

/// Selection indicator under (or over) a bar tab.
struct TabItemChrome {
    content: AnyView,
    indicator: IndicatorPlacement,
    is_selected: bool,
    variant: TabVariant,
}

impl PrimitiveView for TabItemChrome {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, node: &Node) {
        let theme = theme_for_node(node);
        if self.variant == TabVariant::Card && self.is_selected {
            node.set_background_color(Some(theme.colors.surface));
            node.set_corner_radius(theme.radius.md);
        }
        if self.indicator != IndicatorPlacement::None && self.is_selected {
            // A 2pt rule on the content-facing edge marks the active document tab.
            node.set_border_color(Some(theme.colors.accent));
            node.set_border_width(0.0);
        }
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
crate::impl_primitive_view!(TabItemChrome);

const _: fn(&Node) -> bool = |node| node.resolves_right_to_left();

#[cfg(test)]
mod tests {
    use super::*;
    use cove_core::Binding;

    fn tab(id: &str, is_enabled: bool) -> TabItem<String> {
        TabItem::new(id.to_string(), id, crate::EmptyView).enabled(is_enabled)
    }

    fn view(selection: &str, tabs: Vec<TabItem<String>>) -> TabView<String> {
        let selected = selection.to_string();
        TabView::try_new(
            Binding::new(move || selected.clone(), |_value: String| {}),
            tabs,
        )
        .expect("unique tab IDs")
    }

    #[test]
    fn a_selection_on_a_disabled_tab_mounts_no_content() {
        // Swift mounts nothing here (`TabView.swift:127`); only the *tab stop*
        // falls back, which `children` handles separately.
        let v = view("b", vec![tab("a", true), tab("b", false), tab("c", true)]);
        let enabled = v.enabled_ids();
        assert_eq!(v.active(&enabled), None);
    }

    #[test]
    fn active_is_the_bound_tab_when_it_is_enabled() {
        let v = view("c", vec![tab("a", true), tab("b", true), tab("c", true)]);
        let enabled = v.enabled_ids();
        assert_eq!(v.active(&enabled), Some("c".to_string()));
    }

    #[test]
    fn a_disabled_group_has_no_active_tab() {
        let v = view("a", vec![tab("a", true), tab("b", true)]).enabled(false);
        let enabled = v.enabled_ids();
        assert!(enabled.is_empty());
        assert_eq!(v.active(&enabled), None);
    }

    #[test]
    fn adaptive_placement_goes_top_when_not_compact() {
        let v = view("a", vec![tab("a", true)]);
        assert_eq!(v.resolved_placement(), TabStripPlacement::Top);
    }

    #[test]
    fn adaptive_placement_goes_bottom_when_compact() {
        let mut v = view("a", vec![tab("a", true)]);
        v.compact = true;
        assert_eq!(v.resolved_placement(), TabStripPlacement::Bottom);
    }

    #[test]
    fn explicit_placement_wins_over_compactness() {
        let mut v = view("a", vec![tab("a", true)]).placement(TabStripPlacement::Top);
        v.compact = true;
        assert_eq!(v.resolved_placement(), TabStripPlacement::Top);
    }
}
