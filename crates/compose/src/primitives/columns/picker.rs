//! State and menu entries for the column visibility picker, plus the picker
//! trigger that both the `Grid` and the table presentations mount above their
//! content.
//!
//! Translation of `Primitives/Columns/Picker.swift`.
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt};
use crate::modifiers::layout::PaddingExt;
use crate::primitives::overlays::{MenuEntry, MenuItem};
use crate::theme::SemanticStyleViewExt;
use crate::{View, ViewExt};
use cove_core::Binding;

use super::{ColumnVisibilityPlan, ColumnVisibilityTrack};
use std::collections::{HashMap, HashSet};

/// Label advertised by the picker trigger, so one query finds it in every
/// column container.
pub const COLUMN_VISIBILITY_LABEL: &str = "Column visibility";
/// The menu entry that clears every override.
pub const AUTOMATIC_COLUMNS_TITLE: &str = "Automatic columns";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColumnPickerEntry {
    Automatic {
        is_enabled: bool,
    },
    Separator,
    Column {
        id: String,
        title: String,
        is_enabled: bool,
        is_selected: bool,
    },
}

/// UI-independent state consumed by the Rust menu host once it is available.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnVisibilityPickerModel {
    pub entries: Vec<ColumnPickerEntry>,
    pub summary: String,
    pub is_enabled: bool,
    pub is_expanded: bool,
}

impl ColumnVisibilityPickerModel {
    pub fn new(
        columns: &[ColumnVisibilityTrack],
        plan: &ColumnVisibilityPlan,
        overrides: &HashMap<String, bool>,
        is_enabled: bool,
        is_expanded: bool,
    ) -> Self {
        let visible: HashSet<&str> = plan
            .indices
            .iter()
            .filter_map(|index| columns.get(*index).map(|column| column.id.as_str()))
            .collect();
        let mut entries = vec![ColumnPickerEntry::Automatic {
            is_enabled: !overrides.is_empty(),
        }];
        entries.push(ColumnPickerEntry::Separator);
        entries.extend(columns.iter().map(|column| {
            let selected = visible.contains(column.id.as_str());
            let required =
                plan.required_ids.contains(&column.id) || (visible.len() == 1 && selected);
            ColumnPickerEntry::Column {
                id: column.id.clone(),
                title: column.title.clone(),
                is_enabled: !required,
                is_selected: selected,
            }
        }));
        Self {
            entries,
            summary: format!("{} of {} columns", visible.len(), columns.len()),
            is_enabled,
            is_expanded: is_enabled && is_expanded,
        }
    }

    pub fn reset_to_automatic(overrides: &mut HashMap<String, bool>) -> bool {
        if overrides.is_empty() {
            return false;
        }
        overrides.clear();
        true
    }

    pub fn set_column_visible(id: &str, visible: bool, overrides: &mut HashMap<String, bool>) {
        overrides.insert(id.to_owned(), visible);
    }
}

/// The picker both column containers mount above their content: a 44pt
/// "Column visibility" trigger that opens a menu with the automatic reset and
/// one entry per column.
///
/// Translation of `Primitives/Columns/Picker.swift`. The trigger is a real
/// `Button`, so it inherits the touch control extent and the disabled state
/// instead of re-deriving them, and the menu is presented through `Popover`,
/// which owns the portal and dismisses itself when the container is disabled
/// while open.
pub struct ColumnVisibilityPicker {
    columns: Vec<ColumnVisibilityTrack>,
    plan: ColumnVisibilityPlan,
    overrides: Binding<HashMap<String, bool>>,
    is_enabled: bool,
}

impl ColumnVisibilityPicker {
    pub fn new(
        columns: Vec<ColumnVisibilityTrack>,
        plan: ColumnVisibilityPlan,
        overrides: Binding<HashMap<String, bool>>,
        is_enabled: bool,
    ) -> Self {
        Self {
            columns,
            plan,
            overrides,
            is_enabled,
        }
    }

    /// Menu entries in the order Swift authors them: the automatic reset, a
    /// separator, then one entry per column.
    fn entries(&self, model: &ColumnVisibilityPickerModel) -> Vec<MenuEntry> {
        let entries: Vec<MenuEntry> = model
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| match entry {
                ColumnPickerEntry::Automatic { is_enabled } => {
                    let overrides = self.overrides.clone();
                    MenuEntry::Item(
                        MenuItem::new(index as u64, AUTOMATIC_COLUMNS_TITLE, move || {
                            let mut next = overrides.get();
                            if ColumnVisibilityPickerModel::reset_to_automatic(&mut next) {
                                overrides.set(next);
                            }
                        })
                        .enabled(*is_enabled),
                    )
                }
                ColumnPickerEntry::Separator => MenuEntry::separator(index as u64),
                ColumnPickerEntry::Column {
                    id,
                    title,
                    is_enabled,
                    is_selected,
                } => {
                    let column_id = id.clone();
                    let shown = *is_selected;
                    let overrides = self.overrides.clone();
                    MenuEntry::Item(
                        MenuItem::new(index as u64, title.clone(), move || {
                            let mut next = overrides.get();
                            ColumnVisibilityPickerModel::set_column_visible(
                                &column_id, !shown, &mut next,
                            );
                            overrides.set(next);
                        })
                        .enabled(*is_enabled)
                        .selected(*is_selected),
                    )
                }
            })
            .collect();
        entries
    }
}

impl View for ColumnVisibilityPicker {
    fn body(&self, cx: &crate::Scope) -> crate::AnyView {
        let model = ColumnVisibilityPickerModel::new(
            &self.columns,
            &self.plan,
            &self.overrides.get(),
            self.is_enabled,
            false,
        );
        let entries = self.entries(&model);
        // Scope-owned, so the presentation survives the recomposition the
        // choice itself triggers.
        let presented = cx.state(|| false);
        let dismiss = presented;
        let menu = crate::primitives::overlays::Menu::new(entries)
            .on_item_activated(move || dismiss.set(false));
        // One control, one chrome. The trigger used to be a `Button` handed to
        // `Popover`, which wraps its label in a button of its own: two nested
        // capsule backgrounds, one inside the other. The popover now owns the
        // whole trigger, and the trigger is a text-only ghost label, so the
        // picker reads as a toolbar action instead of a pill in a pill.
        let trigger = crate::AnyView::new(
            crate::BoxView::new(
                cove_core::scene::FlexDirection::Row,
                cove_core::scene::Align::Center,
                cove_core::scene::Justify::FlexStart,
                cove_core::scene::Wrap::NoWrap,
                4.0,
                crate::AnyView::new(crate::TupleView::new(vec![
                    crate::AnyView::new(
                        crate::primitives::text::Text::new("Columns")
                            .semantic_font(crate::theme::SemanticFontRef::label())
                            .semantic_foreground(crate::theme::SemanticColorRef::on_surface()),
                    ),
                    crate::AnyView::new(
                        crate::primitives::text::Text::new("\u{25BE}")
                            .semantic_font(crate::theme::SemanticFontRef::caption())
                            .semantic_foreground(
                                crate::theme::SemanticColorRef::on_surface_muted(),
                            ),
                    ),
                ])),
            )
            .padding_edges(6.0, 0.0),
        );
        let popover = crate::primitives::overlays::Popover::new(trigger, menu)
            .is_presented(presented.binding())
            .enabled(self.is_enabled)
            .placement(crate::primitives::overlays::PopoverPlacement::BottomLeading)
            .surface(crate::primitives::overlays::PopoverSurface::Content)
            .overflow(crate::primitives::overlays::PopoverOverflow::Flip)
            .trigger_style(crate::theme::styles::buttons::GhostButtonStyle)
            .width(220.0)
            .composition_local(
                crate::primitives::controls::button::ActionControlAccessibility::label(),
                Some(COLUMN_VISIBILITY_LABEL.to_string()),
            );
        let summary = crate::primitives::text::Text::new(model.summary.clone())
            .semantic_font(crate::theme::SemanticFontRef::caption())
            .semantic_foreground(crate::theme::SemanticColorRef::on_surface_muted())
            .accessibility(|semantics| {
                semantics.role = cove_core::scene::AccessibilityRole::Status;
            })
            .flex_with(0.0, 0.0, None);
        crate::AnyView::new(
            // The caption explains the trigger, so it sits right before it:
            // as a pair they read as one control at the trailing end.
            crate::AnyView::new(crate::BoxView::new(
                cove_core::scene::FlexDirection::Row,
                cove_core::scene::Align::Center,
                cove_core::scene::Justify::FlexStart,
                cove_core::scene::Wrap::NoWrap,
                8.0,
                crate::AnyView::new(crate::TupleView::new(vec![
                    crate::AnyView::new(summary),
                    crate::AnyView::new(popover),
                ])),
            ))
            .frame_min_width(0.0),
        )
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
