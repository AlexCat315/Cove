//! Translation of Primitives/Controls/Fields/AssetRefField.swift (the view half).
//!
//! A drop target that shows the referenced asset's preview tile, name and
//! subtitle, with a clear button and an optional picker menu.
//!
//! Kind-to-glyph matching, picker filtering and the field description live in
//! [`icon_for`], [`picker_options`], [`placeholder_kind`],
//! [`field_description`] and [`field_tooltip`].
use crate::Scope;
use std::collections::BTreeSet;
use std::rc::Rc;

use cove_core::Binding;
use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};
use cove_imaging::BundleImageResource;

use crate::LayoutDebugExt;
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::SemanticStyleViewExt;
use crate::primitives::controls::button::{Button, ButtonIcon, ButtonIconSource, ButtonRole};
use crate::primitives::controls::fields::asset_ref_model::{
    AssetIconKind, AssetRef, field_description, field_tooltip, has_preview, icon_for,
    picker_options, placeholder_kind,
};
use crate::primitives::media::Icon;
use crate::primitives::overlays::anchored::{PopoverOverflow, PopoverPlacement};
use crate::primitives::overlays::asset_drop_target::{AssetDropPayload, AssetDropTarget};
use crate::primitives::overlays::menu::{Menu, MenuEntry, MenuItem};
use crate::primitives::overlays::popover::Popover;
use crate::primitives::text::Text;
use crate::theme::styles::buttons::{ButtonStyleViewExt, GhostButtonStyle};
use crate::theme::{
    ControlSize, ControlSizeViewExt, SemanticColorRef, SemanticFontRef, theme_for_node,
};
use crate::{
    AnyView, BoxView, PrimitiveView, Row, TupleView, VerticalAlignment, View, impl_primitive_view,
};

/// Preview tile size.
const TILE_WIDTH: f32 = 34.0;
const TILE_HEIGHT: f32 = 24.0;
/// Field padding.
const FIELD_PADDING_HORIZONTAL: f32 = 6.0;
const FIELD_PADDING_VERTICAL: f32 = 4.0;
/// Pointer-surface minimum height.
const FIELD_MIN_HEIGHT: f32 = 32.0;
/// Touch-surface minimum height.
const TOUCH_MIN_HEIGHT: f32 = 52.0;
/// Picker popover width.
const PICKER_WIDTH: f32 = 240.0;
/// Picker rows shown before scrolling.
const PICKER_ROWS: usize = 9;

/// An asset slot with a preview tile, clear affordance and optional picker.
pub struct AssetRefField {
    pub value: Binding<Option<AssetRef>>,
    pub active_payload: Binding<Option<AssetDropPayload>>,
    pub accepted_kinds: BTreeSet<String>,
    pub picker_options: Vec<AssetRef>,
    pub placeholder: String,
    pub is_enabled: bool,
}

impl AssetRefField {
    pub fn new(value: Binding<Option<AssetRef>>) -> Self {
        Self {
            value,
            active_payload: Binding::constant(None),
            accepted_kinds: BTreeSet::new(),
            picker_options: Vec::new(),
            placeholder: "Drop asset".to_string(),
            is_enabled: true,
        }
    }

    pub fn active_payload(mut self, active_payload: Binding<Option<AssetDropPayload>>) -> Self {
        self.active_payload = active_payload;
        self
    }

    /// Kinds the slot accepts; empty means anything.
    pub fn accepted_kinds(mut self, accepted_kinds: BTreeSet<String>) -> Self {
        self.accepted_kinds = accepted_kinds;
        self
    }

    pub fn picker_options(mut self, options: Vec<AssetRef>) -> Self {
        self.picker_options = options;
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    fn snapshot(&self) -> AssetRefField {
        AssetRefField {
            value: self.value.clone(),
            active_payload: self.active_payload.clone(),
            accepted_kinds: self.accepted_kinds.clone(),
            picker_options: self.picker_options.clone(),
            placeholder: self.placeholder.clone(),
            is_enabled: self.is_enabled,
        }
    }

    /// The picker options this slot accepts, in authored order.
    pub fn accepted_picker_options(&self) -> Vec<AssetRef> {
        picker_options(&self.picker_options, &self.accepted_kinds)
            .into_iter()
            .cloned()
            .collect()
    }

    /// The description shown to assistive technology.
    pub fn description(&self) -> String {
        field_description(self.value.get().as_ref())
    }

    fn menu_entries(&self) -> Vec<MenuEntry> {
        let current = self.value.get();
        self.accepted_picker_options()
            .into_iter()
            .enumerate()
            .map(|(index, option)| {
                let selected = current.as_ref().is_some_and(|value| value.id == option.id);
                let title = menu_title(&option);
                let target = self.value.clone();
                let value = option.clone();
                MenuEntry::Item(
                    MenuItem::new(index as u64, title, move || target.set(Some(value.clone())))
                        .selected(selected),
                )
            })
            .collect()
    }
}

fn menu_title(option: &AssetRef) -> String {
    match option
        .subtitle
        .as_deref()
        .filter(|subtitle| !subtitle.is_empty())
    {
        Some(subtitle) => format!("{}  {}", option.name, subtitle),
        None => option.name.clone(),
    }
}

/// The glyph for an asset icon kind.
pub fn icon_resource(kind: AssetIconKind) -> BundleImageResource {
    use crate::primitives::media::ui_icon_catalog::{editor, file, media};
    match kind {
        AssetIconKind::Image => media::image(),
        AssetIconKind::Mesh => editor::box_(),
        AssetIconKind::Material => editor::brush(),
        AssetIconKind::Audio => media::music(),
        AssetIconKind::Video => media::video(),
        AssetIconKind::Scene => editor::layers(),
        AssetIconKind::Script => file::file_code(),
        AssetIconKind::Text => file::file_text(),
        AssetIconKind::Package => file::package(),
        AssetIconKind::File => file::file(),
    }
}

impl View for AssetRefField {
    fn body(&self, _cx: &Scope) -> AnyView {
        let field = self.snapshot();
        let value = self.value.clone();
        let content = AnyView::new(AssetFieldContent {
            field: field.clone(),
        });
        let target = AssetDropTarget::new(
            self.active_payload.clone(),
            self.accepted_kinds.iter().cloned().collect(),
            self.is_enabled,
            Rc::new(move |payload: AssetDropPayload| {
                value.set(Some(AssetRef {
                    id: payload.id.clone(),
                    name: payload.name.clone(),
                    subtitle: payload.subtitle.clone(),
                    kind: payload.kind.clone(),
                    preview_path: payload.preview_path.clone(),
                }));
            }),
            content,
        );
        AnyView::new(AssetFieldChrome {
            field,
            content: AnyView::new(target),
        })
    }
}

impl Clone for AssetRefField {
    fn clone(&self) -> Self {
        self.snapshot()
    }
}

/// Field chrome and group semantics for the whole slot.
struct AssetFieldChrome {
    field: AssetRefField,
    content: AnyView,
}

impl PrimitiveView for AssetFieldChrome {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        let current = self.field.value.get();
        node.set_accessibility(Some(cove_core::scene::AccessibilitySemantics {
            role: cove_core::scene::AccessibilityRole::Group,
            label: current
                .as_ref()
                .map(|value| value.name.clone())
                .unwrap_or_else(|| self.field.placeholder.clone()),
            value: self.field.description(),
            state: cove_core::scene::AccessibilityState {
                is_enabled: self.field.is_enabled,
                ..Default::default()
            },
            combines_children: true,
            ..Default::default()
        }));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(AssetFieldChrome);

/// Preview, name/subtitle stack, clear button and picker.
struct AssetFieldContent {
    field: AssetRefField,
}

impl PrimitiveView for AssetFieldContent {
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
        let field = &self.field;
        let current = field.value.get();
        let title = current
            .as_ref()
            .map(|value| value.name.clone())
            .unwrap_or_else(|| field.placeholder.clone());
        let title_tint = if !field.is_enabled || current.is_none() {
            SemanticColorRef::on_surface_muted()
        } else {
            SemanticColorRef::on_surface()
        };

        let mut stack: Vec<AnyView> = vec![AnyView::new(
            Text::new(title)
                .truncated()
                .semantic_font(SemanticFontRef::caption())
                .semantic_foreground(title_tint)
                .flex_with(0.0, 1.0, None),
        )];
        if let Some(subtitle) = current.as_ref().and_then(|value| value.subtitle.clone())
            && !subtitle.is_empty()
        {
            stack.push(AnyView::new(
                Text::new(subtitle)
                    .truncated()
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted())
                    .flex_with(0.0, 1.0, None),
            ));
        }

        let mut row: Vec<AnyView> = vec![
            AnyView::new(
                AnyView::new(AssetTile {
                    reference: current.clone(),
                    accepted_kinds: field.accepted_kinds.clone(),
                    is_enabled: field.is_enabled,
                })
                .flex(0.0)
                .shrink(0.0),
            ),
            AnyView::new(
                BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    1.0,
                    AnyView::new(TupleView::new(stack)),
                )
                .flex_with(1.0, 1.0, Some(0.0))
                .frame_min_width(0.0),
            ),
        ];
        if current.is_some() {
            let value = field.value.clone();
            row.push(AnyView::new(
                Button::icon_with_options(
                    ButtonIconSource::resource(
                        crate::primitives::media::ui_icon_catalog::action::x(),
                    ),
                    Some(12.0),
                    None,
                    ButtonRole::Normal,
                    field.is_enabled,
                    false,
                    false,
                    Some("Clear asset".to_string()),
                    move || value.set(None),
                )
                .button_style(GhostButtonStyle)
                .flex(0.0)
                .shrink(0.0),
            ));
        }
        if !field.picker_options.is_empty() {
            row.push(AnyView::new(
                AnyView::new(AssetPicker {
                    field: field.clone(),
                })
                .flex(0.0)
                .shrink(0.0),
            ));
        }

        vec![AnyView::new(
            Row::new(
                VerticalAlignment::Center,
                8.0,
                AnyView::new(TupleView::new(row)),
            )
            .padding_edges(FIELD_PADDING_HORIZONTAL, FIELD_PADDING_VERTICAL)
            .frame(None, None, None, Some(FIELD_MIN_HEIGHT), None, None)
            .semantic_background(SemanticColorRef::surface())
            .corner_radius_metric(crate::theme::ThemeMetric::input_radius())
            .semantic_border(SemanticColorRef::border(), 1.0),
        )]
    }
}
impl_primitive_view!(AssetFieldContent);

/// The preview tile: the asset's image when it has one, else its kind glyph.
struct AssetTile {
    reference: Option<AssetRef>,
    accepted_kinds: BTreeSet<String>,
    is_enabled: bool,
}

impl PrimitiveView for AssetTile {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_width(Some(TILE_WIDTH));
        layout.set_height(Some(TILE_HEIGHT));
        Some(layout)
    }

    fn update_layout(&self, layout: &cove_core::scene::LayoutNode) {
        layout.set_width(Some(TILE_WIDTH));
        layout.set_height(Some(TILE_HEIGHT));
    }

    fn update_node(&self, node: &Node) {
        let theme = theme_for_node(node);
        node.set_background_color(Some(theme.colors.surface_variant));
        node.set_corner_radius(theme.radius.sm);
        node.set_opacity(if self.is_enabled { 1.0 } else { 0.55 });
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        // An empty slot that accepts exactly one kind hints at that kind; a
        // multi-kind slot shows the generic file glyph instead.
        let kind = match self.reference.as_ref() {
            Some(reference) => Some(reference.kind.as_str()),
            None => placeholder_kind(&self.accepted_kinds),
        };
        let icon = icon_resource(icon_for(kind));
        let tint = if !self.is_enabled
            || self.reference.is_none()
            || !has_preview(self.reference.as_ref())
        {
            SemanticColorRef::on_surface_muted()
        } else {
            SemanticColorRef::on_surface_variant()
        };
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::Center,
                Wrap::NoWrap,
                0.0,
                AnyView::new(
                    ButtonIcon::new(ButtonIconSource::resource(icon), Some(14.0), None)
                        .semantic_foreground(tint),
                ),
            )
            .frame_size(TILE_WIDTH, TILE_HEIGHT)
            .accessibility_label(field_tooltip(kind).to_string()),
        )]
    }
}
impl_primitive_view!(AssetTile);

/// The popover that lists the accepted picker options.
struct AssetPicker {
    field: AssetRefField,
}

impl View for AssetPicker {
    fn body(&self, cx: &Scope) -> AnyView {
        let presented = cx.state(|| false);
        let entries = self.field.menu_entries();
        let is_enabled = self.field.is_enabled && !entries.is_empty();
        let chevron = if presented.get() {
            crate::primitives::media::ui_icon_catalog::navigation::dropdown_chevron_up()
        } else {
            crate::primitives::media::ui_icon_catalog::navigation::dropdown_chevron_down()
        };
        let trigger = Icon::with_semantic(chevron, 12.0, SemanticColorRef::on_surface_muted());
        let current = self.field.value.get();
        AnyView::new(
            Popover::new(
                trigger,
                Menu::new(entries)
                    .width(PICKER_WIDTH)
                    .max_visible_rows(PICKER_ROWS)
                    .on_item_activated(move || presented.set(false)),
            )
            .is_presented(presented.into())
            .enabled(is_enabled)
            .width(PICKER_WIDTH)
            .placement(PopoverPlacement::BottomTrailing)
            .surface(crate::primitives::overlays::PopoverSurface::Content)
            .overflow(PopoverOverflow::Flip)
            .trigger_style(GhostButtonStyle)
            .trigger_description(
                crate::primitives::overlays::popover::PopoverTriggerDescription {
                    label: crate::i18n::tr("Choose asset"),
                    value: current
                        .map(|value| value.name)
                        .unwrap_or_else(|| self.field.placeholder.clone()),
                    help: None,
                },
            )
            .control_size(ControlSize::Mini)
            .debug_name("asset-picker"),
        )
    }
}

impl AssetRefField {
    /// The touch-aware minimum height of the field.
    pub fn minimum_height(touch: bool) -> f32 {
        if touch {
            TOUCH_MIN_HEIGHT
        } else {
            FIELD_MIN_HEIGHT
        }
    }
}
