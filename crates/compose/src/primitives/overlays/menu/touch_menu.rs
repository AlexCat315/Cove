//! Translation of Primitives/Overlays/Menu/TouchMenu.swift.
//!
//! A sheet navigates branches in place instead of opening another anchored
//! portal alongside the current level, because a touch sheet has no room for a
//! second floating surface.
//!
//! Behaviour checklist:
//! - the path resolves to a level; an unresolvable or disabled branch stops the
//!   walk where it is rather than dropping the caller back to the root
//! - only rows with a presentation and `is_enabled` are roving-focus stops
//! - the active row is the highlighted one when still enabled, else the first
//! - a submenu row pushes its id and clears the highlight; the back row pops one
//! - activating an item runs its action and then the sheet's dismissal
//! - labels, separators and plain submenu entries are not actionable rows

use crate::Scope;
use std::rc::Rc;

use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::SemanticStyleViewExt;
use crate::primitives::media::ui_icon_catalog;
use crate::primitives::overlays::menu::{MenuEntry, MenuItemRole, MenuRowPresentation};
use crate::primitives::overlays::roving_focus::{RovingFocusHost, RovingItem, SelectionNavigation};
use crate::primitives::text::Text;
use crate::theme::styles::buttons::{ButtonRole, ButtonStyleViewExt};
use crate::theme::{SemanticColorRef, SemanticFontRef};
use crate::{AnyView, BoxView, TupleView, View, ViewExt};
use cove_composition::LayoutDebugExt;
use cove_composition::Scancode;
use cove_core::Signal;
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, Align, FlexDirection, Justify, Wrap,
};

/// One resolved sheet level: its entries and the title of the branch it shows.
#[derive(Clone)]
pub struct TouchMenuLevel {
    pub entries: Vec<MenuEntry>,
    pub path: Vec<u64>,
    pub title: Option<String>,
}

impl TouchMenuLevel {
    pub fn root(entries: Vec<MenuEntry>) -> Self {
        Self {
            entries,
            path: Vec::new(),
            title: None,
        }
    }

    /// Walks the path, stopping at the first entry that is not an enabled
    /// submenu so a stale id cannot strand the sheet on an empty level.
    pub fn resolve(entries: &[MenuEntry], path: &[u64]) -> Self {
        let mut level = Self::root(entries.to_vec());
        for id in path {
            let Some(MenuEntry::Submenu(branch)) =
                level.entries.iter().find(|entry| entry.id() == *id)
            else {
                break;
            };
            if !branch.is_enabled {
                break;
            }
            let branch = branch.clone();
            level = Self {
                entries: branch.entries,
                path: {
                    let mut next = level.path.clone();
                    next.push(*id);
                    next
                },
                title: Some(branch.title),
            };
        }
        level
    }

    /// Ids that roving focus may land on.
    pub fn enabled_ids(&self) -> Vec<u64> {
        self.entries
            .iter()
            .filter(|entry| entry.row().is_some_and(|row| row.is_enabled))
            .map(|entry| entry.id())
            .collect()
    }
}

/// A sheet that navigates menu branches in place.
pub struct TouchMenu {
    pub entries: Vec<MenuEntry>,
    pub on_item_activated: Option<Rc<dyn Fn()>>,
}

impl TouchMenu {
    pub fn new(entries: Vec<MenuEntry>) -> Self {
        Self {
            entries,
            on_item_activated: None,
        }
    }

    pub fn on_item_activated(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_item_activated = Some(Rc::new(handler));
        self
    }

    fn entry_view(
        &self,
        menu: TouchMenuSession,
        entry: &MenuEntry,
        active: Option<u64>,
    ) -> AnyView {
        let Some(row) = entry.row() else {
            return match entry {
                MenuEntry::Label { title, .. } => AnyView::new(
                    AnyView::new(
                        Text::new(title.clone())
                            .semantic_font(SemanticFontRef::caption())
                            .semantic_foreground(SemanticColorRef::on_surface_muted()),
                    )
                    .padding(12.0),
                ),
                _ => super::menu::separator_view(),
            };
        };

        let activate = {
            let entry = entry.clone();
            let on_item = self.on_item_activated.clone();
            Rc::new(move || match &entry {
                MenuEntry::Item(item) => {
                    (item.action)();
                    if let Some(handler) = &on_item {
                        handler();
                    }
                }
                MenuEntry::Submenu(branch) => menu.push(branch.id),
                _ => {}
            })
        };
        let role = if row.is_link() {
            AccessibilityRole::Link
        } else {
            AccessibilityRole::Button
        };
        let label = row.title.clone();
        let value = row.content.detail.clone().unwrap_or_default();
        let is_selected = row.is_selected;
        let is_tab_stop = active == Some(row.id);
        let identity: Rc<dyn Fn(&u64) -> u64> = Rc::new(|id: &u64| *id);

        AnyView::new(
            AnyView::new(
                TouchMenuRow {
                    row: row.clone(),
                    activate,
                }
                .modifier(RovingItem::new(row.id, is_tab_stop, identity)),
            )
            .accessibility(move |semantics: &mut AccessibilitySemantics| {
                semantics.role = role;
                semantics.label = label.clone();
                semantics.value = value.clone();
                semantics.state.is_selected = is_selected;
            }),
        )
    }
}

/// The navigation path and roving highlight, owned by the menu's scope.
#[derive(Clone, Copy)]
struct TouchMenuSession {
    path: Signal<Vec<u64>>,
    highlighted: Signal<Option<u64>>,
}

impl TouchMenuSession {
    fn level(&self, entries: &[MenuEntry]) -> TouchMenuLevel {
        self.path
            .with(|path| TouchMenuLevel::resolve(entries, path))
    }

    /// The roving stop: the highlighted row while it is still enabled, else the
    /// first enabled row.
    fn active(&self, level: &TouchMenuLevel) -> Option<u64> {
        let enabled = level.enabled_ids();
        match self.highlighted.get() {
            Some(id) if enabled.contains(&id) => Some(id),
            _ => enabled.first().copied(),
        }
    }

    fn push(&self, id: u64) {
        cove_core::batch(|| {
            self.path.update(|path| path.push(id));
            self.highlighted.set(None);
        });
    }

    fn pop(&self) {
        cove_core::batch(|| {
            self.path.update(|path| {
                path.pop();
            });
            self.highlighted.set(None);
        });
    }
}

impl View for TouchMenu {
    fn body(&self, cx: &Scope) -> AnyView {
        let menu = TouchMenuSession {
            path: cx.state(Vec::new),
            highlighted: cx.state(|| None),
        };
        let level = menu.level(&self.entries);
        let enabled = level.enabled_ids();
        let active = menu.active(&level);

        let mut children = Vec::new();
        if let Some(title) = &level.title {
            let back = menu;
            children.push(AnyView::new(
                AnyView::new(
                    crate::primitives::controls::Button::new(
                        AnyView::new(BoxView::new(
                            FlexDirection::Row,
                            Align::Center,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            8.0,
                            AnyView::new(TupleView::new(vec![
                                AnyView::new(
                                    crate::primitives::media::Icon::with_semantic(
                                        ui_icon_catalog::navigation::arrow_left(),
                                        16.0,
                                        SemanticColorRef::on_surface(),
                                    )
                                    .accessibility_hidden(true),
                                ),
                                AnyView::new(
                                    Text::new(title.clone())
                                        .truncated()
                                        .semantic_font(SemanticFontRef::body_strong()),
                                ),
                            ])),
                        )),
                        move || back.pop(),
                    )
                    .role(ButtonRole::Normal),
                )
                .button_style(crate::theme::styles::buttons::plain())
                .accessibility(|semantics: &mut AccessibilitySemantics| {
                    semantics.label = "Back".to_string();
                })
                .debug_name("touch-menu-back"),
            ));
            children.push(AnyView::new(crate::primitives::text::Divider::horizontal()));
        }

        for entry in &level.entries {
            children.push(self.entry_view(menu, entry, active));
        }

        let body = AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(children)),
            ))
            .frame(None, None, Some(0.0), None, None, None)
            .debug_name("touch-menu"),
        );

        let highlighted = menu.highlighted;
        AnyView::new(
            RovingFocusHost::new(
                enabled,
                move || highlighted.get(),
                move |id: u64| highlighted.set(Some(id)),
                |id: &u64| *id,
                body,
            )
            .command(|event| {
                matches!(
                    event.scancode,
                    Scancode::ARROW_UP | Scancode::ARROW_DOWN | Scancode::HOME | Scancode::END
                )
                .then(|| SelectionNavigation::command_for(event.scancode))
                .flatten()
            }),
        )
    }
}

/// One actionable sheet row: icon, title/detail, and a trailing affordance.
/// The roving focus host (via the `RovingItem` modifier in `entry_view`) owns
/// the tab-stop policy, so the row itself carries no focus-state field.
struct TouchMenuRow {
    row: MenuRowPresentation,
    activate: Rc<dyn Fn()>,
}

impl View for TouchMenuRow {
    fn body(&self, _cx: &Scope) -> AnyView {
        let row = self.row.clone();
        let activate = self.activate.clone();
        let role = if row.role == MenuItemRole::Destructive {
            ButtonRole::Destructive
        } else {
            ButtonRole::Normal
        };
        let mut leading = Vec::new();
        if let Some(icon) = &row.content.icon {
            leading.push(AnyView::new(
                crate::primitives::media::Icon::new(icon.clone(), 20.0).accessibility_hidden(true),
            ));
        }

        let mut text = vec![AnyView::new(
            Text::new(row.title.clone())
                .truncated()
                .semantic_font(SemanticFontRef::body()),
        )];
        if let Some(detail) = &row.content.detail {
            text.push(AnyView::new(
                Text::new(detail.clone())
                    .line_limit(Some(2))
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
            ));
        }

        let trailing = if row.has_submenu {
            Some(ui_icon_catalog::navigation::chevron_right())
        } else if row.is_selected {
            Some(ui_icon_catalog::action::check())
        } else if row.is_link() {
            Some(ui_icon_catalog::action::external_link())
        } else {
            None
        };

        let mut row_children = leading;
        row_children.push(AnyView::new(
            AnyView::new(
                AnyView::new(BoxView::new(
                    FlexDirection::Column,
                    Align::FlexStart,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    4.0,
                    AnyView::new(TupleView::new(text)),
                ))
                .frame(None, None, Some(0.0), None, None, None),
            )
            .flex_with(1.0, 1.0, Some(0.0)),
        ));
        if let Some(icon) = trailing {
            row_children.push(AnyView::new(
                crate::primitives::media::Icon::with_semantic(
                    icon,
                    16.0,
                    SemanticColorRef::on_surface_muted(),
                )
                .accessibility_hidden(true),
            ));
        }

        let label = row.title.clone();
        AnyView::new(
            AnyView::new(
                crate::primitives::controls::Button::new(
                    AnyView::new(BoxView::new(
                        FlexDirection::Row,
                        Align::Center,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        12.0,
                        AnyView::new(TupleView::new(row_children)),
                    )),
                    move || activate(),
                )
                .enabled(row.is_enabled)
                .selected(row.is_selected)
                .role(role),
            )
            .button_style(crate::theme::styles::buttons::plain())
            .padding_edges(12.0, 12.0)
            .frame(None, None, Some(0.0), Some(48.0), None, None)
            .flex_with(1.0, 1.0, None)
            .accessibility(move |semantics: &mut AccessibilitySemantics| {
                semantics.label = label.clone();
            }),
        )
    }
}
