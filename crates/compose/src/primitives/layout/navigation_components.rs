//! Translation of Primitives/Layout/NavigationComponents.swift.
//!
//! Breadcrumbs and pagination share one idea: a bounded row of navigation targets
//! whose middle collapses when the row cannot hold them all.
use crate::Scope;
use std::cell::RefCell;
use std::rc::Rc;

use crate::modifiers::layout::PaddingExt;
use crate::modifiers::style::SemanticStyleViewExt;
use crate::primitives::controls::button::{Button, ButtonRole};
use crate::primitives::media::ui_icon_catalog;
use crate::primitives::overlays::anchored::{PopoverOverflow, PopoverSurface};
use crate::primitives::overlays::menu::{Menu, MenuEntry, MenuItem};
use crate::primitives::overlays::popover::Popover;
use crate::primitives::text::Text;
use crate::theme::styles::buttons::{ButtonStyleViewExt, GhostButtonStyle, ToggleButtonStyle};
use crate::theme::{SemanticColorRef, SemanticFontRef};
use crate::{AnyView, PrimitiveView, Row, TupleView, VerticalAlignment, View};
use cove_core::Binding;
use cove_core::scene::{AccessibilityRole, AccessibilitySemantics, Node};

/// Default visible items before a breadcrumb collapses its middle.
pub const DEFAULT_MAX_VISIBLE_ITEMS: usize = 4;
/// Breadcrumb trail gap.
const TRAIL_SPACING: f32 = 4.0;

/// One step in a breadcrumb trail.
#[derive(Clone, Debug, PartialEq)]
pub struct BreadcrumbItem<Id: PartialEq + Clone> {
    pub id: Id,
    pub title: String,
    pub is_enabled: bool,
}

impl<Id: PartialEq + Clone> BreadcrumbItem<Id> {
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

/// A trail of ancestors, with a collapsed middle when it is too long.
pub struct Breadcrumb<Id: PartialEq + Clone + std::hash::Hash + 'static> {
    pub items: Vec<BreadcrumbItem<Id>>,
    pub on_select: Rc<dyn Fn(Id)>,
    pub max_visible_items: usize,
    is_presented: Rc<RefCell<bool>>,
}

impl<Id: PartialEq + Clone + std::hash::Hash + 'static> Breadcrumb<Id> {
    ///
    /// IDs must be unique.
    pub fn new(
        items: Vec<BreadcrumbItem<Id>>,
        max_visible_items: usize,
        on_select: impl Fn(Id) + 'static,
    ) -> Self {
        assert!(
            items.iter().enumerate().all(|(index, item)| items
                .iter()
                .enumerate()
                .all(|(other, candidate)| other == index || candidate.id != item.id)),
            "Breadcrumb IDs must be unique"
        );
        Self {
            items,
            on_select: Rc::new(on_select),
            max_visible_items: max_visible_items.max(2),
            is_presented: Rc::new(RefCell::new(false)),
        }
    }

    fn snapshot(&self) -> Breadcrumb<Id> {
        Breadcrumb {
            items: self.items.clone(),
            on_select: self.on_select.clone(),
            max_visible_items: self.max_visible_items,
            is_presented: self.is_presented.clone(),
        }
    }
}

impl<Id: PartialEq + Clone + std::hash::Hash + 'static> View for Breadcrumb<Id> {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(BreadcrumbBody {
            trail: self.snapshot(),
        })
    }
}

struct BreadcrumbBody<Id: PartialEq + Clone + std::hash::Hash + 'static> {
    trail: Breadcrumb<Id>,
}

impl<Id: PartialEq + Clone + std::hash::Hash + 'static> PrimitiveView for BreadcrumbBody<Id> {
    fn update_layout(&self, _layout: &cove_core::scene::LayoutNode) {}
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Group,
            label: "Breadcrumb".to_string(),
            ..Default::default()
        }));
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let trail = &self.trail;
        let count = trail.items.len();
        let collapsed = count > trail.max_visible_items;
        // The trailing window keeps the last `maxVisibleItems - 1` items plus the
        // ellipsis slot, so the root and the current page always survive.
        let trailing_start = if collapsed {
            count - trail.max_visible_items + 1
        } else {
            count
        };

        let chevron = AnyView::new(
            AnyView::new(crate::primitives::media::Icon::with_semantic(
                ui_icon_catalog::navigation::chevron_right(),
                12.0,
                SemanticColorRef::on_surface_muted(),
            ))
            .with_modifier(HiddenSemantics),
        );

        let mut row: Vec<AnyView> = Vec::new();
        for (index, item) in trail.items.iter().enumerate() {
            if collapsed && index != 0 && index < trailing_start {
                continue;
            }
            let mut cell: Vec<AnyView> = Vec::new();
            if index > 0 {
                cell.push(chevron.clone());
            }
            if collapsed && index == trailing_start {
                let hidden: Vec<MenuEntry> = trail.items[1..trailing_start]
                    .iter()
                    .map(|entry| {
                        let on_select = trail.on_select.clone();
                        let is_presented = trail.is_presented.clone();
                        let id = entry.id.clone();
                        MenuEntry::Item(
                            MenuItem::new(menu_id(&entry.id), entry.title.clone(), move || {
                                on_select(id.clone());
                                *is_presented.borrow_mut() = false;
                            })
                            .enabled(entry.is_enabled),
                        )
                    })
                    .collect();
                let popover = Popover::new(
                    AnyView::new(
                        Text::new("…")
                            .semantic_font(SemanticFontRef::label())
                            .padding_edges(8.0, 0.0),
                    ),
                    AnyView::new(Menu::new(hidden)),
                )
                .width(240.0)
                .surface(PopoverSurface::Content)
                .overflow(PopoverOverflow::Flip);
                cell.push(AnyView::new(
                    AnyView::new(popover).with_modifier(AccessibleLabel::new("Hidden path items")),
                ));
                cell.push(chevron.clone());
            }
            let is_current = index + 1 == count;
            if is_current {
                cell.push(AnyView::new(
                    Text::new(item.title.clone())
                        .semantic_font(SemanticFontRef::label())
                        .semantic_foreground(SemanticColorRef::on_surface()),
                ));
            } else {
                let on_select = trail.on_select.clone();
                let id = item.id.clone();
                cell.push(AnyView::new(
                    Button::with_options(
                        AnyView::new(Text::new(item.title.clone())),
                        move || on_select(id.clone()),
                        ButtonRole::Normal,
                        item.is_enabled,
                        false,
                        false,
                        None,
                    )
                    .button_style(GhostButtonStyle),
                ));
            }
            row.push(AnyView::new(Row::new(
                VerticalAlignment::Center,
                TRAIL_SPACING,
                AnyView::new(TupleView::new(cell)),
            )));
        }
        vec![AnyView::new(Row::new(
            VerticalAlignment::Center,
            TRAIL_SPACING,
            AnyView::new(TupleView::new(row)),
        ))]
    }
}

impl<Id: PartialEq + Clone + std::hash::Hash + 'static> View for BreadcrumbBody<Id> {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
}

fn menu_id(id: &impl std::hash::Hash) -> u64 {
    use std::hash::Hasher;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut hasher);
    hasher.finish()
}

/// Hides decorative glyphs from assistive technology.
struct HiddenSemantics;

impl cove_composition::ViewModifier for HiddenSemantics {
    fn apply_node(&self, node: &Node) {
        let mut semantics = node.accessibility().unwrap_or_default();
        semantics.is_hidden = true;
        node.set_accessibility(Some(semantics));
    }
}

/// Sets an accessible label on an erased view.
struct AccessibleLabel {
    label: String,
}

impl AccessibleLabel {
    fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }
}

impl cove_composition::ViewModifier for AccessibleLabel {
    fn apply_node(&self, node: &Node) {
        let mut semantics = node
            .accessibility()
            .unwrap_or_else(|| AccessibilitySemantics::new(AccessibilityRole::Button));
        semantics.label = self.label.clone();
        node.set_accessibility(Some(semantics));
    }
}

/// One slot in a pagination row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PaginationItem {
    Page(usize),
    LeadingGap,
    TrailingGap,
}

/// Page-window arithmetic: which page numbers a pager shows.
pub struct PaginationModel;

impl PaginationModel {
    pub fn normalized(current: usize, total: usize) -> usize {
        if total == 0 {
            0
        } else {
            current.clamp(1, total)
        }
    }

    /// The pages to show, with gaps where numbers were skipped.
    pub fn items(current: usize, total: usize, siblings: usize) -> Vec<PaginationItem> {
        if total == 0 {
            return Vec::new();
        }
        let siblings = siblings.min(10);
        let current = Self::normalized(current, total);
        // Short pagers show every page; no gaps to justify.
        if total <= siblings * 2 + 5 {
            return (1..=total).map(PaginationItem::Page).collect();
        }
        let start = 2.max(current.saturating_sub(siblings));
        let end = (total - 1).min(current + siblings.min(total.saturating_sub(current)));
        let mut result: Vec<PaginationItem> = vec![PaginationItem::Page(1)];
        if start == 3 {
            result.push(PaginationItem::Page(2));
        } else if start > 3 {
            result.push(PaginationItem::LeadingGap);
        }
        if start <= end {
            result.extend((start..=end).map(PaginationItem::Page));
        }
        if end == total.saturating_sub(2) {
            result.push(PaginationItem::Page(total - 1));
        } else if end < total.saturating_sub(2) {
            result.push(PaginationItem::TrailingGap);
        }
        result.push(PaginationItem::Page(total));
        result
    }
}

/// A bounded page selector.
pub struct Pagination {
    pub page: Binding<usize>,
    pub total_pages: usize,
    pub siblings: usize,
    pub is_enabled: bool,
}

impl Pagination {
    pub fn new(page: Binding<usize>, total_pages: usize) -> Self {
        Self {
            page,
            total_pages,
            siblings: 1,
            is_enabled: true,
        }
    }

    pub fn siblings(mut self, siblings: usize) -> Self {
        self.siblings = siblings.min(10);
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }
}

impl View for Pagination {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(PaginationBody {
            pager: self.snapshot(),
        })
    }
}

impl Pagination {
    fn snapshot(&self) -> Pagination {
        Pagination {
            page: self.page.clone(),
            total_pages: self.total_pages,
            siblings: self.siblings,
            is_enabled: self.is_enabled,
        }
    }
}

struct PaginationBody {
    pager: Pagination,
}

impl PrimitiveView for PaginationBody {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Group,
            label: "Pagination".to_string(),
            ..Default::default()
        }));
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let pager = &self.pager;
        let total = pager.total_pages;
        let current = PaginationModel::normalized(pager.page.get(), total);
        let mut row: Vec<AnyView> = Vec::new();

        let previous = pager.page.clone();
        row.push(AnyView::new(
            Button::with_options(
                AnyView::new(Text::new("Previous")),
                move || previous.set(current.saturating_sub(1)),
                ButtonRole::Normal,
                pager.is_enabled && current > 1,
                false,
                false,
                None,
            )
            .button_style(GhostButtonStyle),
        ));

        for item in PaginationModel::items(current, total, pager.siblings) {
            row.push(match item {
                PaginationItem::Page(value) => {
                    let page = pager.page.clone();
                    AnyView::new(PaginationPageButton {
                        value,
                        total,
                        is_selected: value == current,
                        is_enabled: pager.is_enabled,
                        on_select: Rc::new(move || page.set(value)),
                    })
                }
                PaginationItem::LeadingGap | PaginationItem::TrailingGap => AnyView::new(
                    AnyView::new(
                        Text::new("…")
                            .semantic_font(SemanticFontRef::label())
                            .padding_edges(5.0, 0.0),
                    )
                    .with_modifier(HiddenSemantics),
                ),
            });
        }

        if total == 0 {
            row.push(AnyView::new(
                Text::new("No pages")
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
            ));
        }

        let next = pager.page.clone();
        row.push(AnyView::new(
            Button::with_options(
                AnyView::new(Text::new("Next")),
                move || next.set((current + 1).min(total)),
                ButtonRole::Normal,
                pager.is_enabled && current < total,
                false,
                false,
                None,
            )
            .button_style(GhostButtonStyle),
        ));

        vec![AnyView::new(Row::new(
            VerticalAlignment::Center,
            4.0,
            AnyView::new(TupleView::new(row)),
        ))]
    }
}
crate::impl_primitive_view!(PaginationBody);

/// One numbered page target.
struct PaginationPageButton {
    value: usize,
    total: usize,
    is_selected: bool,
    is_enabled: bool,
    on_select: Rc<dyn Fn()>,
}

impl PrimitiveView for PaginationPageButton {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Button,
            label: format!("Page {} of {}", self.value, self.total),
            state: cove_core::scene::AccessibilityState {
                is_enabled: self.is_enabled,
                is_selected: self.is_selected,
                ..Default::default()
            },
            ..Default::default()
        }));
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let on_select = self.on_select.clone();
        let value = self.value;
        vec![AnyView::new(
            Button::with_options(
                AnyView::new(Text::new(value.to_string())),
                move || on_select(),
                ButtonRole::Normal,
                self.is_enabled,
                false,
                self.is_selected,
                None,
            )
            .button_style(ToggleButtonStyle {
                min_width: 32.0,
                height: 28.0,
            }),
        )]
    }
}
crate::impl_primitive_view!(PaginationPageButton);

#[cfg(test)]
mod tests {
    use super::PaginationItem;
    use super::PaginationModel;

    #[test]
    fn short_pagers_show_every_page() {
        // total <= siblings*2+5 shows all pages with no gaps.
        assert_eq!(
            PaginationModel::items(3, 7, 1),
            (1..=7).map(PaginationItem::Page).collect::<Vec<_>>()
        );
    }

    #[test]
    fn long_pagers_collapse_around_the_current_page() {
        let items = PaginationModel::items(10, 20, 1);
        assert_eq!(items.first(), Some(&PaginationItem::Page(1)));
        assert_eq!(items.last(), Some(&PaginationItem::Page(20)));
        assert!(items.contains(&PaginationItem::Page(10)));
        assert!(items.contains(&PaginationItem::LeadingGap));
        assert!(items.contains(&PaginationItem::TrailingGap));
    }

    #[test]
    fn a_page_next_to_the_first_shows_two_instead_of_a_gap() {
        // start == 3 must expand to page 2 rather than an ellipsis.
        let items = PaginationModel::items(3, 20, 1);
        assert!(items.contains(&PaginationItem::Page(2)));
        assert!(!items.contains(&PaginationItem::LeadingGap));
    }

    #[test]
    fn normalization_clamps_to_the_page_range() {
        assert_eq!(PaginationModel::normalized(0, 5), 1);
        assert_eq!(PaginationModel::normalized(9, 5), 5);
        assert_eq!(PaginationModel::normalized(3, 0), 0);
    }
}
