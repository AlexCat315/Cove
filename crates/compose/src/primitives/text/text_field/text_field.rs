//! Translation of Primitives/Text/TextField/TextField.swift.
//!
//! Text input field. The default horizontal axis is single-line; the vertical
//! axis accepts explicit newline insertion and grows in height to fit those
//! lines. Editing state survives recomposition in the host node.
//!
//! Reads from the text environment for shaping; without one installed the field
//! still accepts input but renders no glyphs.
use crate::Scope;
use std::cell::RefCell;
use std::rc::Rc;

use crate::primitives::overlays::portal::PortalResource;
use crate::primitives::text::code::minimap::{
    MINIMAP_WIDTH, MinimapGeometry, ink_runs, strip_color,
};
use crate::primitives::text::text_buffer::TextBuffer;
use crate::primitives::text::text_document_layout::TextDocumentLayout;
use crate::primitives::text::text_edit_history::{Snapshot, TextEditHistory, TextEditKind};
use crate::primitives::text::text_editing_commands::TextEditingCommands;
use crate::theme::styles::components::AnyTextFieldStyle;
use crate::theme::styles::components::TextFieldStyleConfiguration;
use crate::theme::styles::components::text_field_style::TextFieldStyleEnvironment;
use crate::theme::{SemanticFontRef, Theme, theme_for_node};
use crate::{AnyView, PrimitiveView, View, impl_primitive_view};
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, AccessibilityState, EventResult, FocusChainHolder,
    HoverPhase, InputHandlerRoute, InteractionRegistryHolder, LayoutNode, MeasureMode, Node,
    NodeResource, Overflow, PointerCaptureHolder, PointerPhase, TextEditCommand,
    TextInputAttachmentKey, WheelRoutingAttachmentKey, WheelRoutingPriority,
};
use cove_core::{Binding, Color, Point, Rect, Size, TextAlignment};
use cove_platform::{MouseButton, MouseWheelEvent, SystemCursor};
use cove_text::{DrawListText, TextEnvironment, TextEnvironmentHolder};

use super::indent_guides::TextIndentGuides;
use super::snippet::TextSnippetSession;
use super::text_completion_session::TextCompletionSession;
use super::text_field_code_editing::indented_newline;
use super::text_field_editing::{
    delete_selection, edit_clock, insert_replacing_selection, insert_replacing_selection_limited,
};
use super::text_field_layout_engine::{
    DOCUMENT_LAYOUT_KEY, FIELD_STATE_KEY, FieldCompositionState, FieldLayoutInputs,
    FieldScrollState, FieldSelectionState, RENDER_CACHE_KEY, RenderCacheEntry, RenderCacheKey,
    RenderDraftState, document_geometry, horizontal_scrollbar_metrics, make_render_state,
    scrollbar_metrics, selection_range, update_scroll,
};
use super::text_field_navigation::{
    line_boundary, move_cursor_by_page, move_cursor_vertically, word_boundary_after,
    word_boundary_before,
};
use super::text_field_observation::{TextFieldCaretState, TextFieldHoverAnchor};
use super::text_field_options::{
    TextFieldAxis, TextFieldBehavior, TextFieldCodeEditing, TextFieldDecoration, TextFieldLayout,
    TextFieldNavigationRequest, TextFieldSize,
};

const SCROLLBAR_HOVERED: &str = "__textfield_scrollbar_hovered";
const FOCUS_REQUEST_ID: &str = "__textfield_focus_request_id";
const CARET_REQUEST_ID: &str = "__textfield_caret_request_id";
const MINIMAP_GEOMETRY: &str = "__textfield_minimap_geometry";
const MINIMAP_DRAGGING: &str = "__textfield_minimap_dragging";
const MEASURE_INPUTS_KEY: &str = "__textfield_measure_inputs";
const MEASURE_DOCUMENT_KEY: &str = "__textfield_measure_document";
/// One line increment for a wheel notch in a multiline field.
pub const MULTILINE_WHEEL_STEP: f32 = 30.0;
/// Space between the line-number column and the document text.
const GUTTER_GAP: f32 = 24.0;
pub const SCROLLBAR_TRACK_THICKNESS: f32 = 6.0;
pub const SCROLLBAR_INSET: f32 = 3.0;

/// One open `begin_edit` … `end_edit` bracket. Nested brackets coalesce into
/// the outermost transaction, so a helper that records its own edit inside a
/// caller's bracket does not split the undo step.
#[derive(Clone, Debug, Default)]
struct EditTransaction {
    history: TextEditHistory,
    depth: usize,
    before: Option<Snapshot>,
    kind: TextEditKind,
}

/// Widths the authored decorations reserve from the text column.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AddonMetrics {
    /// `prepend` slab, including both of its paddings.
    pub prepend: f32,
    /// `append` slab, including both of its paddings.
    pub append: f32,
    /// Inline `prefix` glyphs and their gap.
    pub prefix: f32,
    /// Inline `suffix` glyphs and their gap.
    pub suffix: f32,
    /// Decorative leading glyph and its gap.
    pub icon: f32,
    /// Trailing clear affordance.
    pub clear: f32,
    /// Word-limit counter and its gap.
    pub counter: f32,
    /// Line-number gutter and its gap to the document.
    pub gutter: f32,
}

/// Measured decorations for one pass: the shapes to paint and the space they
/// take.
pub struct AddonLayouts {
    pub metrics: AddonMetrics,
    pub prepend: Option<std::sync::Arc<cove_core::TextLayoutResult>>,
    pub append: Option<std::sync::Arc<cove_core::TextLayoutResult>>,
    pub prefix: Option<std::sync::Arc<cove_core::TextLayoutResult>>,
    pub suffix: Option<std::sync::Arc<cove_core::TextLayoutResult>>,
    pub counter: Option<std::sync::Arc<cove_core::TextLayoutResult>>,
    pub counter_text: Option<String>,
    pub gutter_geometry: Option<crate::primitives::text::code::TextGutterGeometry>,
}

impl AddonLayouts {
    /// No decorations: the plain field chrome.
    pub fn empty() -> Self {
        Self {
            metrics: AddonMetrics::default(),
            prepend: None,
            append: None,
            prefix: None,
            suffix: None,
            counter: None,
            counter_text: None,
            gutter_geometry: None,
        }
    }

    /// Space taken on the leading edge, before the editable text.
    pub fn leading(&self) -> f32 {
        self.metrics.gutter + self.metrics.prepend + self.metrics.icon + self.metrics.prefix
    }

    /// Space taken on the trailing edge, after the editable text.
    pub fn trailing(&self) -> f32 {
        self.metrics.suffix + self.metrics.append
    }
}

/// Editing state owned by the surface node and shared across recompositions.
pub struct FieldState {
    pub buffer: TextBuffer,
    pub selection: FieldSelectionState,
    pub composition: FieldCompositionState,
    pub scroll: FieldScrollState,
    pub draft: RenderDraftState,
    pub is_focused: bool,
    pub is_dragging: bool,
    pub last_draw_origin: Point,
    pub clear_hit_x: Option<f32>,
    pub needs_caret_reveal: bool,
    transaction: EditTransaction,
    last_activity: std::time::Instant,
}

impl FieldState {
    pub fn new(buffer: TextBuffer) -> Self {
        Self {
            selection: FieldSelectionState {
                cursor_index: buffer.character_count(),
                ..Default::default()
            },
            buffer,
            composition: FieldCompositionState::default(),
            scroll: FieldScrollState::default(),
            draft: RenderDraftState::default(),
            is_focused: false,
            is_dragging: false,
            last_draw_origin: Point::default(),
            clear_hit_x: None,
            needs_caret_reveal: true,
            transaction: EditTransaction::default(),
            last_activity: std::time::Instant::now(),
        }
    }

    pub fn clear_composition(&mut self) {
        self.composition = FieldCompositionState::default();
    }

    /// Adopt the caller's history when one is authored. Swift replaces the
    /// session history every pass so the same object backs `canUndo` and the
    /// menu bridge.
    pub fn adopt_history(&mut self, history: Option<TextEditHistory>) {
        if let Some(history) = history {
            self.transaction.history = history;
        }
    }

    pub fn history(&self) -> &TextEditHistory {
        &self.transaction.history
    }

    pub fn history_mut(&mut self) -> &mut TextEditHistory {
        &mut self.transaction.history
    }

    /// Open an undo bracket at the current text. `kind` decides whether the
    /// transaction may coalesce with the previous one.
    pub fn begin_edit(&mut self, current: &TextBuffer, kind: TextEditKind) {
        if self.transaction.depth == 0 {
            self.normalize(current);
            let bound = self.buffer.clone();
            self.transaction.history.synchronize(&bound);
            self.transaction.before = Some(Snapshot::new(
                bound,
                self.selection.cursor_index,
                self.selection.anchor,
            ));
            self.transaction.kind = kind;
        }
        self.transaction.depth += 1;
    }

    /// Close the bracket opened by `begin_edit` and record the difference.
    pub fn end_edit(&mut self) {
        self.transaction.depth = self.transaction.depth.saturating_sub(1);
        if self.transaction.depth != 0 {
            return;
        }
        let Some(before) = self.transaction.before.take() else {
            return;
        };
        let after = Snapshot::new(
            self.buffer.clone(),
            self.selection.cursor_index,
            self.selection.anchor,
        );
        self.transaction
            .history
            .record(before, after, self.transaction.kind, edit_clock());
    }

    /// Restore the previous (or next) snapshot. Returns the restored buffer so
    /// the caller can publish it through the binding.
    pub fn restore_history(&mut self, redo: bool) -> Option<TextBuffer> {
        let bound = self.buffer.clone();
        self.transaction.history.synchronize(&bound);
        let snapshot = if redo {
            self.transaction.history.redo()
        } else {
            self.transaction.history.undo()
        }?;
        self.clear_composition();
        self.buffer = snapshot.buffer.clone();
        self.selection.cursor_index = snapshot.cursor.min(self.buffer.character_count());
        self.selection.anchor = snapshot.anchor;
        self.selection.set_preferred_caret_x(None);
        self.record_activity();
        Some(snapshot.buffer)
    }

    /// End the current coalescing run: the next edit starts a new undo step.
    pub fn break_history_group(&mut self) {
        self.transaction.history.break_group();
    }

    /// Clamp persisted indices into the current text. State outlives recompose
    /// while the bound text can be rewritten underneath it (entity switch,
    /// formatter, programmatic set); a stale cursor past the new end must never
    /// reach index math.
    pub fn normalize(&mut self, buffer: &TextBuffer) {
        if !self.buffer.has_same_contents(buffer) {
            self.selection.line_end_affinity = false;
        }
        self.buffer = buffer.clone();
        let count = buffer.character_count();
        self.selection.cursor_index = self.selection.cursor_index.min(count);
        if let Some(anchor) = self.selection.anchor {
            let bounded = anchor.min(count);
            self.selection.anchor = if bounded == self.selection.cursor_index {
                None
            } else {
                Some(bounded)
            };
        }
    }

    pub fn selection_range(&self) -> Option<(usize, usize)> {
        selection_range(&self.buffer, &self.selection)
    }

    /// Move the caret, optionally growing a selection.
    pub fn move_cursor(&mut self, target: usize, extend_selection: bool, preferred_x: Option<f32>) {
        let count = self.buffer.character_count();
        let bounded = target.min(count);
        if extend_selection {
            if self.selection.anchor.is_none() {
                self.selection.anchor = Some(self.selection.cursor_index);
            }
        } else {
            self.selection.anchor = None;
        }
        self.selection.cursor_index = bounded;
        self.selection.set_preferred_caret_x(preferred_x);
        self.record_activity();
    }

    /// Every caret/selection mutation funnels through here: it clears the
    /// line-end affinity, requests a reveal and reports the new caret.
    pub fn record_activity(&mut self) {
        self.last_activity = std::time::Instant::now();
        self.selection.line_end_affinity = false;
        self.needs_caret_reveal = true;
        self.scroll.needs_caret_reveal = true;
    }

    pub fn caret_state(&self) -> TextFieldCaretState {
        TextFieldCaretState::new(self.selection.cursor_index, self.selection_range())
    }
}

impl NodeResource for FieldState {
    fn mount(&self, _node: &Node) {}

    fn unmount(&self, _node: &Node) {}

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

/// Callbacks the host forwards to its owner.
#[derive(Clone, Default)]
pub struct TextFieldEvents {
    pub on_submit: Option<Rc<dyn Fn()>>,
    pub on_key_down: Option<Rc<dyn Fn(&cove_platform::KeyEvent) -> bool>>,
    pub on_cancel: Option<Rc<dyn Fn()>>,
    pub on_change: Option<Rc<dyn Fn(&TextBuffer)>>,
    pub on_hover_change: Option<Rc<dyn Fn(Option<TextFieldHoverAnchor>)>>,
    pub on_caret_change: Option<Rc<dyn Fn(TextFieldCaretState)>>,
    pub on_focus: Option<Rc<dyn Fn()>>,
    pub on_blur: Option<Rc<dyn Fn()>>,
    pub on_clear: Option<Rc<dyn Fn()>>,
}

/// Interaction snapshot the style configuration observes, mirroring Swift's
/// `_TextFieldInteractionState`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextFieldInteraction {
    pub focused: bool,
    pub composing: bool,
    pub hovered: bool,
}

impl TextFieldInteraction {
    fn editing(self) -> bool {
        self.focused || self.composing
    }
}

/// Reactive handle shared by the surface and the style host.
pub(super) struct TextFieldInteractionState {
    flags: cove_core::Signal<TextFieldInteraction>,
}

impl TextFieldInteractionState {
    fn update(&self, change: impl FnOnce(&mut TextFieldInteraction)) {
        // Handlers run while a body may be recomposing. Reading untracked
        // avoids invalidating the very observer that is rebuilding the host.
        let Some(current) = cove_core::reactive::untrack(|| self.flags.try_get()) else {
            return;
        };
        let mut next = current;
        change(&mut next);
        if next != current {
            self.flags.set(next);
        }
    }
}

/// Resolves the `TextFieldStyle` and asks it for the field chrome, the way
/// Swift's `_TextFieldStyleHost` does.
struct TextFieldChrome {
    field: TextField,
    interaction: Rc<TextFieldInteractionState>,
    snapshot: TextFieldInteraction,
}

impl PrimitiveView for TextFieldChrome {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_is_focusable(false);
        node
    }

    fn update_node(&self, node: &Node) {
        node.set_is_hit_testable(false);
        node.set_is_focusable(false);
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(cove_core::scene::FlexDirection::Column);
        layout.set_align_items(cove_core::scene::Align::Stretch);
        Some(layout)
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        // A field-level style overrides the environment, the same way Swift's
        // `.textFieldStyle` resolves through its composition local.
        let style = self
            .field
            .style
            .clone()
            .unwrap_or_else(|| node.composition_value(&TextFieldStyleEnvironment::key()));
        let theme = theme_for_node(node);
        let field = &self.field;
        let interaction = self.snapshot;
        let disabled = field.behavior.disabled;
        let supports_hover = node
            .composition_value(&crate::adaptive::SurfaceEnvironment::key())
            .supports_hover();
        // Swift's `_TextFieldStyleHost` resolves an `Automatic` decoration from
        // the inherited control size before building the surface.
        let mut resolved = field.snapshot();
        if resolved.decoration.size == TextFieldSize::Automatic {
            resolved.decoration.size = node
                .composition_value(&crate::theme::ControlSizeEnvironment::key())
                .text_field_size();
        }
        let configuration = TextFieldStyleConfiguration {
            content: AnyView::new(TextFieldHost {
                field: resolved,
                interaction: self.interaction.clone(),
            }),
            placeholder: field.placeholder.clone(),
            is_focused: interaction.focused && !disabled,
            is_editing: interaction.editing() && !disabled,
            is_error: false,
            is_enabled: !disabled,
            theme,
            is_hovered: interaction.hovered && supports_hover && !disabled,
        };
        vec![style.make_body(configuration)]
    }
}
impl_primitive_view!(TextFieldChrome);

/// Text input field.
#[derive(Clone)]
pub struct TextField {
    pub text: Binding<TextBuffer>,
    pub placeholder: String,
    pub layout: TextFieldLayout,
    pub behavior: TextFieldBehavior,
    pub decoration: TextFieldDecoration,
    pub code_editing: TextFieldCodeEditing,
    pub navigation: TextFieldNavigationRequest,
    pub events: TextFieldEvents,
    pub style: Option<AnyTextFieldStyle>,
}

impl TextField {
    pub fn new(placeholder: impl Into<String>, text: Binding<TextBuffer>) -> Self {
        Self {
            text,
            placeholder: placeholder.into(),
            layout: TextFieldLayout::default(),
            behavior: TextFieldBehavior::default(),
            decoration: TextFieldDecoration::default(),
            code_editing: TextFieldCodeEditing::default(),
            navigation: TextFieldNavigationRequest::default(),
            events: TextFieldEvents::default(),
            style: None,
        }
    }

    pub fn multiline(mut self) -> Self {
        self.layout.axis = TextFieldAxis::Vertical;
        self.layout.max_visible_lines = self.layout.max_visible_lines.max(2);
        self
    }

    pub fn layout(mut self, layout: TextFieldLayout) -> Self {
        self.layout = layout.validated();
        self
    }

    pub fn behavior(mut self, behavior: TextFieldBehavior) -> Self {
        self.behavior = behavior.validated();
        self
    }

    pub fn decoration(mut self, decoration: TextFieldDecoration) -> Self {
        self.decoration = decoration;
        self
    }

    pub fn code_editing(mut self, code_editing: TextFieldCodeEditing) -> Self {
        self.code_editing = code_editing.validated();
        self
    }

    pub fn events(mut self, events: TextFieldEvents) -> Self {
        self.events = events;
        self
    }

    pub fn style(mut self, style: AnyTextFieldStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// Minimum field height for the authored size variant.
    pub fn minimum_field_height(&self) -> f32 {
        match self.decoration.size {
            TextFieldSize::Large => 40.0,
            TextFieldSize::Automatic | TextFieldSize::Regular => 32.0,
            TextFieldSize::Small => 28.0,
            TextFieldSize::Mini => 22.0,
        }
    }

    fn horizontal_inset(&self, theme: &Theme) -> f32 {
        match self.decoration.size {
            TextFieldSize::Large => 12.0,
            TextFieldSize::Automatic | TextFieldSize::Regular => theme.spacing.sm,
            TextFieldSize::Small => 6.0,
            TextFieldSize::Mini => 4.0,
        }
    }

    /// Decorative leading glyph size, scaled with the line box.
    fn leading_icon_size(&self, line_height: f32) -> f32 {
        (line_height * 0.8).max(12.0)
    }

    /// The `n / limit` counter text, or `None` while no limit is authored or
    /// shown.
    fn counter_text(&self, character_count: usize) -> Option<String> {
        if !self.decoration.show_word_limit {
            return None;
        }
        let limit = self.behavior.max_length?;
        Some(format!("{character_count}/{limit}"))
    }

    /// Measure the authored decorations once per pass. Every consumer of the
    /// text column — hit testing, the IME anchor, the caret and the paint —
    /// reads these same reservations so nothing collides.
    fn addon_layouts(
        &self,
        node: &Node,
        theme: &Theme,
        env: &TextEnvironment,
        font: &cove_core::Font,
        line_height: f32,
        letter_spacing: f32,
        character_count: usize,
        rows: usize,
    ) -> AddonLayouts {
        let inset = self.horizontal_inset(theme);
        let has_text = character_count > 0;
        let measure = |text: &str| -> std::sync::Arc<cove_core::TextLayoutResult> {
            env.cached_layout(
                text,
                Some(font),
                Some(line_height),
                f32::INFINITY,
                TextAlignment::Leading,
                letter_spacing,
            )
        };
        let prepend = self
            .decoration
            .prepend
            .as_deref()
            .filter(|text| !text.is_empty())
            .map(measure);
        let append = self
            .decoration
            .append
            .as_deref()
            .filter(|text| !text.is_empty())
            .map(measure);
        let prefix = self
            .decoration
            .prefix
            .as_deref()
            .filter(|text| !text.is_empty())
            .map(measure);
        let suffix = self
            .decoration
            .suffix
            .as_deref()
            .filter(|text| !text.is_empty())
            .map(measure);
        let counter_text = self.counter_text(character_count);
        let counter = counter_text.as_deref().map(measure);
        // Three digit slots keep the document column still while the visible
        // line numbers grow; narrower documents never need more.
        let mut gutter_geometry = None;
        let gutter = if self.code_editing.shows_line_numbers
            && self.layout.axis == TextFieldAxis::Vertical
        {
            let digits = rows.max(1).to_string().len().max(3);
            let slot = measure("0").total_width.max(1.0);
            let number_width = slot * digits as f32;
            if self.code_editing.gutter.is_enabled()
                || !self.code_editing.folding.regions.is_empty()
            {
                let gutter = &self.code_editing.gutter;
                let geometry = crate::primitives::text::code::TextGutterGeometry::new(
                    gutter.is_enabled(),
                    gutter.breakpoints.is_some(),
                    gutter.execution_line.is_some(),
                    !self.code_editing.folding.regions.is_empty(),
                    number_width,
                    font,
                    theme,
                    !crate::primitives::controls::adaptive::supports_control_hover(node),
                );
                gutter_geometry = Some(geometry);
                geometry.width
            } else {
                number_width + GUTTER_GAP
            }
        } else {
            0.0
        };
        let metrics = AddonMetrics {
            gutter,
            prepend: prepend.as_ref().map_or(0.0, |layout| {
                layout.total_width + inset * 2.0 + theme.spacing.sm
            }),
            append: append.as_ref().map_or(0.0, |layout| {
                layout.total_width + inset * 2.0 + theme.spacing.sm
            }),
            prefix: prefix
                .as_ref()
                .map_or(0.0, |layout| layout.total_width + theme.spacing.xs),
            suffix: suffix
                .as_ref()
                .map_or(0.0, |layout| layout.total_width + theme.spacing.xs),
            icon: if self.decoration.leading_icon.is_some() {
                self.leading_icon_size(line_height) + theme.spacing.xs
            } else {
                0.0
            },
            clear: if self.shows_clear_affordance(has_text) {
                line_height + theme.spacing.xs
            } else {
                0.0
            },
            counter: counter
                .as_ref()
                .map_or(0.0, |layout| layout.total_width + theme.spacing.xs),
        };
        AddonLayouts {
            metrics,
            gutter_geometry,
            prepend,
            append,
            prefix,
            suffix,
            counter,
            counter_text,
        }
    }

    /// The trailing lane the clear control uses while the field can be emptied.
    /// The control itself only appears while the field is focused.
    fn shows_clear_affordance(&self, has_text: bool) -> bool {
        self.behavior.clearable && !self.behavior.read_only && !self.behavior.disabled && has_text
    }

    /// Leading reservation: the authored slabs, icon and inline prefix.
    fn leading_addon(&self, addons: &AddonLayouts) -> f32 {
        addons.metrics.gutter + addons.metrics.prepend + addons.metrics.icon + addons.metrics.prefix
    }

    /// Text-column width for a frame, after every reservation.
    fn available_text_width(&self, theme: &Theme, frame_width: f32, addons: &AddonLayouts) -> f32 {
        (frame_width
            - self.horizontal_inset(theme) * 2.0
            - self.leading_addon(addons)
            - addons.metrics.suffix
            - addons.metrics.append
            - addons.metrics.clear
            - addons.metrics.counter
            - self.minimap_reservation())
        .max(1.0)
    }

    fn vertical_inset(&self, line_height: f32) -> f32 {
        let field = self.minimum_field_height();
        ((field - line_height) / 2.0).max(4.0)
    }

    /// Width the overview takes from the text column.
    fn minimap_reservation(&self) -> f32 {
        if self.code_editing.shows_minimap && self.layout.axis == TextFieldAxis::Vertical {
            MINIMAP_WIDTH
        } else {
            0.0
        }
    }

    fn inputs(&self, theme: &Theme) -> FieldLayoutInputs {
        FieldLayoutInputs {
            is_vertical: self.layout.axis == TextFieldAxis::Vertical,
            wraps_lines: self.layout.wraps_lines,
            secure: self.behavior.secure,
            horizontal_inset: self.horizontal_inset(theme),
            vertical_inset: 0.0,
            text_origin_y_offset: 0.0,
        }
    }
}

impl View for TextField {
    fn body(&self, cx: &Scope) -> AnyView {
        let flags = cx.state(TextFieldInteraction::default);
        let interaction = Rc::new(TextFieldInteractionState { flags });
        // Swift's @State invalidates _TextFieldStyleHost on focus/composition
        // changes. Reading here subscribes the body's observer, so a write
        // recomposes the style host instead of only repainting the surface.
        let snapshot = flags.get();
        AnyView::new(TextFieldChrome {
            field: self.snapshot(),
            interaction,
            snapshot,
        })
    }
}

impl TextField {
    pub(super) fn clone_for_session(&self) -> TextField {
        self.snapshot()
    }

    fn snapshot(&self) -> TextField {
        TextField {
            text: self.text.clone(),
            placeholder: self.placeholder.clone(),
            layout: self.layout,
            behavior: self.behavior.clone(),
            decoration: self.decoration.clone(),
            code_editing: self.code_editing.clone(),
            navigation: self.navigation.clone(),
            events: self.events.clone(),
            style: self.style.clone(),
        }
    }
}

struct TextFieldHost {
    field: TextField,
    interaction: Rc<TextFieldInteractionState>,
}

/// The text style for one field node: the theme token with the caller's
/// authored font, line height and spacing applied.
struct AuthoredTextStyle {
    font: cove_core::Font,
    line_height: f32,
    letter_spacing: f32,
}

/// Resolve the authored text style at use time, the way Swift's
/// `resolvedFont(node:env:)` reads it from the node ancestry.
fn resolved_text_style(node: &Node, theme: &Theme) -> AuthoredTextStyle {
    use crate::modifiers::style::style_modifiers::{
        text_style_font, text_style_letter_spacing, text_style_line_height,
    };
    let base = SemanticFontRef::body().resolve(theme);
    let fallback_line_height = base.line_height_for(None);
    let letter_spacing = text_style_letter_spacing(node).unwrap_or(base.letter_spacing);
    let font = text_style_font(node).unwrap_or(base.font);
    let line_height = text_style_line_height(node).unwrap_or(fallback_line_height);
    AuthoredTextStyle {
        font,
        line_height,
        letter_spacing,
    }
}

impl PrimitiveView for TextFieldHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(!self.field.behavior.disabled);
        node.set_is_focusable(!self.field.behavior.disabled);
        node.set_clips_to_bounds(true);
        let portal = PortalResource::new();
        let snippet = TextSnippetSession::new();
        node.add_resource(portal.clone());
        node.add_resource(snippet.clone());
        node.add_resource(TextCompletionSession::new(portal, snippet));
        node.add_resource(Rc::new(
            crate::primitives::text::code::TextGutterSession::new(),
        ));
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_min_height(Some(self.field.minimum_field_height()));
        layout.set_overflow(Overflow::Hidden);
        layout.set_min_width(Some(0.0));
        layout.set_flex_grow(1.0);
        layout.set_flex_shrink(1.0);
        install_measure(&layout, self.field.snapshot());
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_min_height(Some(self.field.minimum_field_height()));
        layout.set_overflow(Overflow::Hidden);
        layout.set_min_width(Some(0.0));
        layout.set_flex_grow(1.0);
        layout.set_flex_shrink(1.0);
        install_measure(layout, self.field.snapshot());
    }

    fn update_node(&self, node: &Node) {
        // Swift's `updateSurfaceNode` calls `node.syncTextStyleEnvironment()`
        // first so shaping sees the ambient scaler and body token. Authored
        // font/line height stay on the modifier chain above the style host and
        // are resolved lazily through `text_style_*`, exactly like
        // `TextField.resolvedFont(node:env:)` — the surface must not cache them,
        // because the caller's modifier is applied to the style host after this
        // node is materialised.
        crate::theme::sync_text_style_environment(node);
        let focused = FocusChainHolder::current()
            .and_then(|chain| chain.focused())
            .is_some_and(|focused| focused == *node);
        let hovered = node.attachment::<bool>(SCROLLBAR_HOVERED).unwrap_or(false);
        self.interaction.update(|state| {
            state.focused = focused;
            state.hovered = hovered && !self.field.behavior.disabled;
        });
        node.set_cursor(Some(if self.field.behavior.disabled {
            SystemCursor::Arrow
        } else {
            SystemCursor::Ibeam
        }));
        node.set_is_focusable(!self.field.behavior.disabled);
        node.set_is_hit_testable(!self.field.behavior.disabled);
        if self.field.behavior.disabled
            && let Some(chain) = FocusChainHolder::current()
            && chain.focused().is_some_and(|focused| focused == *node)
        {
            chain.clear();
        }

        let current = self.field.text.get();
        if node
            .attachment::<Rc<RefCell<FieldState>>>(FIELD_STATE_KEY)
            .is_none()
        {
            node.set_attachment(
                FIELD_STATE_KEY,
                Rc::new(RefCell::new(FieldState::new(current.clone()))),
            );
        }
        let Some(state_handle) = node.attachment::<Rc<RefCell<FieldState>>>(FIELD_STATE_KEY) else {
            return;
        };
        {
            let mut state = state_handle.borrow_mut();
            state.normalize(&current);
            // An external rewrite of the bound text invalidates both stacks;
            // the session adopts the caller's history when one is authored.
            state.adopt_history(self.field.code_editing.edit_history.clone());
            let bound = current.clone();
            state.history_mut().synchronize(&bound);
            if self.field.behavior.disabled {
                state.clear_composition();
            }
        }
        self.interaction
            .update(|flags| flags.composing = state_handle.borrow().composition.is_active());
        install_history_commands(node, &self.field, state_handle.clone());
        // Publish the scroll limits during the update pass as well, so a
        // nested field knows it can scroll before its first paint.
        refresh_scroll_metrics(node, &self.field, &state_handle);

        let is_focused = FocusChainHolder::current()
            .and_then(|chain| chain.focused())
            .is_some_and(|focused| focused == *node);
        state_handle.borrow_mut().is_focused = is_focused;
        let is_hovered = node.attachment::<bool>(SCROLLBAR_HOVERED).unwrap_or(false);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::TextField,
            label: self.field.placeholder.clone(),
            value: current.string_value(),
            state: AccessibilityState {
                is_enabled: !self.field.behavior.disabled,
                is_read_only: self.field.behavior.read_only,
                is_secure: self.field.behavior.secure,
                ..Default::default()
            },
            ..Default::default()
        }));

        // Focus requests arrive as one-shot IDs so repeated updates cannot
        // re-focus a field the user already left.
        if let Some(request) = self.field.navigation.focus_request_id
            && node.attachment::<u64>(FOCUS_REQUEST_ID) != Some(request)
        {
            node.set_attachment(FOCUS_REQUEST_ID, request);
            if let Some(chain) = FocusChainHolder::current() {
                chain.focus(Some(node), true);
            }
            // A focus request selects the current value so typing replaces it.
            let mut state = state_handle.borrow_mut();
            state.selection.anchor = Some(0);
            state.selection.cursor_index = state.buffer.character_count();
            state.record_activity();
        }
        if let (Some(request), Some(index)) = (
            self.field.navigation.caret_request_id,
            self.field.navigation.caret_request_index,
        ) && node.attachment::<u64>(CARET_REQUEST_ID) != Some(request)
        {
            node.set_attachment(CARET_REQUEST_ID, request);
            let handle = state_handle.clone();
            {
                let mut state = handle.borrow_mut();
                state.selection.anchor = None;
                state.move_cursor(index, false, None);
            }
            // A caret request also focuses the field, so the host can place the
            // IME anchor on the requested caret.
            if let Some(chain) = FocusChainHolder::current() {
                chain.focus(Some(node), false);
            }
        }

        if let Some(session) = node.first_resource::<TextCompletionSession>() {
            session.configure(self.field.snapshot(), state_handle.clone());
        }
        install_interaction(node, &self.field, state_handle.clone(), &self.interaction);
        if let Some(gutter) =
            node.first_resource::<crate::primitives::text::code::TextGutterSession>()
        {
            gutter.configure(
                self.field.code_editing.gutter.clone(),
                &self.field.code_editing.folding,
                &current,
            );
            let field = self.field.snapshot();
            node.set_layout_did_update(Some(Rc::new(move |owner| {
                update_gutter_controls(owner, &field)
            })));
        }

        let overlay_node = node.downgrade();
        node.update_overlay_draw(
            (
                is_focused,
                is_hovered,
                state_handle.borrow().scroll.max_y.to_bits(),
                state_handle.borrow().scroll.horizontal.maximum().to_bits(),
            ),
            move |list, origin| {
                let Some(owner) = Node::upgrade(&overlay_node) else {
                    return;
                };
                draw_scrollbar_overlay(&owner, list, origin);
            },
        );

        let draw_node = node.downgrade();
        let field = self.field.snapshot();
        let state_for_draw = state_handle.clone();
        node.update_draw(
            (
                current.string_value(),
                self.field.placeholder.clone(),
                self.field.layout,
                self.field.behavior.clone(),
                is_focused,
            ),
            move |list, origin| {
                let Some(owner) = Node::upgrade(&draw_node) else {
                    return;
                };
                render(&owner, &field, &state_for_draw, list, origin);
            },
        );
    }
    fn children(&self, node: &Node) -> Vec<AnyView> {
        use crate::primitives::text::code::{TextGutterControls, TextGutterSession};
        let Some(session) = node.first_resource::<TextGutterSession>() else {
            return Vec::new();
        };
        if !self.field.code_editing.shows_line_numbers
            || self.field.behavior.secure
            || self.field.layout.axis != TextFieldAxis::Vertical
        {
            return Vec::new();
        }
        let current = self.field.text.get();
        let cursor = node
            .attachment::<Rc<RefCell<FieldState>>>(FIELD_STATE_KEY)
            .map_or(0, |s| s.borrow().selection.cursor_index);
        vec![AnyView::new(
            TextGutterControls::new(session, self.field.code_editing.gutter.clone())
                .is_enabled(!self.field.behavior.disabled)
                .caret(
                    current.line_index_for_character_index(cursor),
                    current.character_count(),
                    self.field.behavior.read_only,
                ),
        )]
    }
}
impl_primitive_view!(TextFieldHost);

fn update_gutter_controls(node: &Node, field: &TextField) {
    use crate::primitives::text::code::{
        TextGutterControl, TextGutterControlKind as Kind, TextGutterSession,
    };
    let Some(session) = node.first_resource::<TextGutterSession>() else {
        return;
    };
    if !field.code_editing.shows_line_numbers
        || field.behavior.secure
        || field.layout.axis != TextFieldAxis::Vertical
    {
        session.update(Vec::new());
        return;
    }
    let buffer = field.text.get();
    let style = resolved_text_style(node, &theme_for_node(node));
    let touch = !crate::primitives::controls::adaptive::supports_control_hover(node);
    let Some(environment) = TextEnvironmentHolder::current() else {
        session.update(Vec::new());
        return;
    };
    let theme = theme_for_node(node);
    let addons = field.addon_layouts(
        node,
        &theme,
        &environment,
        &style.font,
        style.line_height,
        style.letter_spacing,
        buffer.character_count(),
        buffer.line_count(),
    );
    let Some(geometry) = addons.gutter_geometry else {
        session.update(Vec::new());
        return;
    };
    let left = field.horizontal_inset(&theme);
    let top = field.vertical_inset(style.line_height);
    let offset = node
        .attachment::<Rc<RefCell<FieldState>>>(FIELD_STATE_KEY)
        .map_or(0.0, |s| s.borrow().scroll.offset_y);
    let hidden = session.folds().hidden_lines();
    let mut controls = Vec::new();
    let line_height = style.line_height.max(1.0);
    let first_row = ((offset - top) / line_height).floor().max(0.0) as usize;
    let end_row = ((offset + node.frame().height - top) / line_height)
        .ceil()
        .max(0.0) as usize;
    let mut line = first_row;
    // Translate the first visible row through the merged folded ranges, then
    // visit only the viewport's rows even for a large document.
    for range in &hidden {
        if range.start <= line {
            line += range.len();
        } else {
            break;
        }
    }
    let markers = session.markers_by_line();
    let mut row = first_row;
    while line < buffer.line_count() && row < end_row {
        if let Some(range) = hidden.iter().find(|r| r.contains(&line)) {
            line = range.end;
            continue;
        }
        let y = top + row as f32 * line_height - offset;
        let mut push = |kind, x| {
            controls.push(TextGutterControl::new(
                line,
                kind,
                left + x,
                y,
                geometry.control_width,
                style.line_height.max(if touch { 44.0 } else { 24.0 }),
                0,
            ))
        };
        if geometry.combined {
            push(Kind::Combined, 0.0);
        } else {
            if let Some(x) = geometry.breakpoint_x {
                push(Kind::Breakpoint, x);
            }
            if let Some(x) = geometry.marker_x
                && markers.contains_key(&line)
            {
                push(Kind::Markers, x);
            }
            if let Some(x) = geometry.fold_x
                && session.folds().region(line).is_some()
            {
                push(Kind::Fold, x);
            }
        }
        line += 1;
        row += 1;
    }
    session.update(controls);
}

/// Inputs that change the measured size of a field. Cached on the layout node
/// so a text edit can mark the node dirty instead of silently keeping a stale
/// height.
#[derive(Clone, PartialEq)]
struct MeasureInputs {
    text: String,
    placeholder: String,
    axis: TextFieldAxis,
    wraps_lines: bool,
    max_visible_lines: usize,
    secure: bool,
}

/// Install the intrinsic-size measure for a text field.
///
/// A vertical field grows with its content up to `max_visible_lines`; a
/// horizontal one keeps one line but still grows while the text carries
/// explicit newlines. Both clamp to the authored maximum, after which the
/// field scrolls.
fn install_measure(layout: &LayoutNode, field: TextField) {
    let inputs = MeasureInputs {
        text: field.text.get().string_value(),
        placeholder: field.placeholder.clone(),
        axis: field.layout.axis,
        wraps_lines: field.layout.wraps_lines,
        max_visible_lines: field.layout.max_visible_lines,
        secure: field.behavior.secure,
    };
    let previous = layout.attachment::<MeasureInputs>(MEASURE_INPUTS_KEY);
    if previous.as_ref() == Some(&inputs) {
        return;
    }
    if previous.is_some() {
        // Only a change after the first measure invalidates: the initial pass
        // already measured the value it was installed with.
        layout.mark_dirty();
    }
    layout.set_attachment(MEASURE_INPUTS_KEY, inputs);

    let weak_layout = layout.downgrade();
    layout.set_measure_func(Some(Rc::new(
        move |width, width_mode, _height, _height_mode| {
            let Some(layout) = cove_core::scene::LayoutNode::upgrade(&weak_layout) else {
                return Size::default();
            };
            measure_field(&field, &layout, width, width_mode)
        },
    )));
}

/// Default line-box inset used while measuring, before a theme is resolvable.
const MEASURE_INSET_X: f32 = 16.0;

fn measure_field(
    field: &TextField,
    layout: &LayoutNode,
    width: f32,
    width_mode: MeasureMode,
) -> Size {
    let minimum = field.minimum_field_height();
    let Some(environment) = TextEnvironmentHolder::current() else {
        return Size {
            width: width.max(0.0),
            height: minimum,
        };
    };
    let font = environment.default_font.clone();
    let line_height = environment.resolved_line_height(&font, None);
    let inset_y = field.vertical_inset(line_height);
    let maximum =
        (line_height * field.layout.max_visible_lines as f32 + inset_y * 2.0).max(minimum);
    if field.layout.axis != TextFieldAxis::Vertical {
        let ideal = if field.layout.ideal_width.is_finite() {
            field.layout.ideal_width.max(1.0)
        } else {
            160.0
        };
        let resolved = match width_mode {
            MeasureMode::Exactly => width,
            MeasureMode::AtMost | MeasureMode::MinContent => ideal.min(width.max(0.0)),
            MeasureMode::Undefined => ideal,
        };
        let current = field.text.get();
        let buffer = if current.is_empty() {
            TextBuffer::new(&field.placeholder)
        } else {
            current
        };
        let content = buffer.line_count() as f32 * line_height;
        return Size {
            width: resolved,
            height: (content + inset_y * 2.0).clamp(minimum, maximum),
        };
    }
    let current = field.text.get();
    let is_empty = current.is_empty();
    let buffer = if is_empty {
        TextBuffer::new(&field.placeholder)
    } else {
        current
    };
    let width_limit = if matches!(width_mode, MeasureMode::Undefined) || !field.layout.wraps_lines {
        f32::INFINITY
    } else {
        (width - MEASURE_INSET_X).max(1.0)
    };
    let inputs = FieldLayoutInputs {
        is_vertical: true,
        wraps_lines: field.layout.wraps_lines,
        secure: field.behavior.secure,
        horizontal_inset: MEASURE_INSET_X * 0.5,
        vertical_inset: inset_y,
        text_origin_y_offset: 0.0,
    };
    let geometry = document_geometry(
        &inputs,
        font.clone(),
        line_height,
        0.0,
        environment.atlas_texture_id as u64,
        width_limit,
        is_empty,
    );
    let document = layout.attachment::<Rc<RefCell<TextDocumentLayout>>>(MEASURE_DOCUMENT_KEY);
    let document = document.unwrap_or_else(|| {
        let document = Rc::new(RefCell::new(TextDocumentLayout::new(
            buffer.clone(),
            geometry.clone(),
        )));
        layout.set_attachment(MEASURE_DOCUMENT_KEY, document.clone());
        document
    });
    document.borrow_mut().update(buffer.clone(), geometry);
    let rows =
        document
            .borrow_mut()
            .visible_layout(0, field.layout.max_visible_lines + 1, &environment);
    let content_height = rows.total_height.max(line_height);
    let measured_width = rows.total_width + MEASURE_INSET_X;
    let resolved_width = match width_mode {
        MeasureMode::Exactly => width,
        MeasureMode::AtMost | MeasureMode::MinContent => measured_width.min(width.max(0.0)),
        MeasureMode::Undefined => measured_width,
    };
    Size {
        width: resolved_width,
        height: (content_height + inset_y * 2.0).clamp(minimum, maximum),
    }
}

/// Publish the undo/redo bridge the platform menu and the scene shortcuts read.
///
/// Swift installs both the plain closures and a `TextEditActions` pair
/// (`TextField.swift:236,262`); without them `TextEditingCommands::undo` finds
/// nothing on the command target and a Primary-Z falls through to the scene
/// fallback.
fn install_history_commands(node: &Node, field: &TextField, state: Rc<RefCell<FieldState>>) {
    let keys = [
        TextEditingCommands::UNDO_KEY,
        TextEditingCommands::REDO_KEY,
        TextEditingCommands::CAN_UNDO_KEY,
        TextEditingCommands::CAN_REDO_KEY,
    ];
    if field.behavior.disabled || field.behavior.read_only {
        for key in keys {
            node.remove_attachment(key);
        }
        return;
    }
    let binding = field.text.clone();
    let undo_node = node.downgrade();
    let undo_state = state.clone();
    let undo_binding = binding.clone();
    node.set_attachment(
        TextEditingCommands::UNDO_KEY,
        Rc::new(move || {
            if let Some(owner) = Node::upgrade(&undo_node) {
                restore_history(&owner, &undo_state, &undo_binding, false);
            }
        }) as Rc<dyn Fn()>,
    );
    let redo_node = node.downgrade();
    let redo_state = state.clone();
    let redo_binding = binding.clone();
    node.set_attachment(
        TextEditingCommands::REDO_KEY,
        Rc::new(move || {
            if let Some(owner) = Node::upgrade(&redo_node) {
                restore_history(&owner, &redo_state, &redo_binding, true);
            }
        }) as Rc<dyn Fn()>,
    );
    let can_undo_state = state.clone();
    node.set_attachment(
        TextEditingCommands::CAN_UNDO_KEY,
        Rc::new(move || can_undo_state.borrow().history().can_undo()) as Rc<dyn Fn() -> bool>,
    );
    let can_redo_state = state.clone();
    node.set_attachment(
        TextEditingCommands::CAN_REDO_KEY,
        Rc::new(move || can_redo_state.borrow().history().can_redo()) as Rc<dyn Fn() -> bool>,
    );
    let actions_node = node.downgrade();
    let query_state = state.clone();
    let query_binding = binding.clone();
    let actions_state = state;
    let actions_binding = binding;
    node.set_attachment(
        TextInputAttachmentKey::EDIT_ACTIONS,
        cove_core::scene::TextEditActions::new(
            move |command| {
                let mut state = query_state.borrow_mut();
                let bound = query_binding.get();
                state.history_mut().synchronize(&bound);
                match command {
                    TextEditCommand::Undo => state.history().can_undo(),
                    TextEditCommand::Redo => state.history().can_redo(),
                }
            },
            move |command| {
                let Some(owner) = Node::upgrade(&actions_node) else {
                    return;
                };
                restore_history(
                    &owner,
                    &actions_state,
                    &actions_binding,
                    command == TextEditCommand::Redo,
                );
            },
        ),
    );
}

/// Restore one history snapshot and publish it through the caller's binding.
fn restore_history(
    node: &Node,
    state: &Rc<RefCell<FieldState>>,
    binding: &Binding<TextBuffer>,
    redo: bool,
) {
    let Some(buffer) = state.borrow_mut().restore_history(redo) else {
        return;
    };
    if !buffer.has_same_contents(&binding.get()) {
        binding.set(buffer.clone());
        if let Some(handler) = node.attachment::<Rc<dyn Fn(&TextBuffer)>>("__textfield_change") {
            handler(&buffer);
        }
    }
    node.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
        "textFieldUndo",
    ));
}

fn install_text_input_client(node: &Node, field: &TextField, state: &Rc<RefCell<FieldState>>) {
    if field.behavior.disabled {
        node.remove_attachment(TextInputAttachmentKey::CLIENT);
        node.remove_attachment(TextInputAttachmentKey::AREA_RESOLVER);
        node.remove_attachment(TextInputAttachmentKey::AREA);
        return;
    }
    let configuration = field.snapshot();
    let area_state = state.clone();
    node.set_attachment(
        TextInputAttachmentKey::AREA_RESOLVER,
        Rc::new(move |node: &Node, origin: Point| {
            input_area(node, &configuration, &area_state, origin)
        }) as cove_core::scene::TextInputAreaResolver,
    );
    let client = super::platform_input::platform_input_client(
        state.clone(),
        field.behavior.clone(),
        field.layout.axis,
    );
    let publish_node = node.downgrade();
    let publish_state = state.clone();
    let publish_binding = field.text.clone();
    let publish = Rc::new(move || {
        let Some(owner) = Node::upgrade(&publish_node) else {
            return;
        };
        let buffer = publish_state.borrow().buffer.clone();
        notify_composition(&owner, &publish_state);
        if !buffer.has_same_contents(&publish_binding.get()) {
            publish_binding.set(buffer.clone());
            if let Some(handler) = owner.attachment::<Rc<dyn Fn(&TextBuffer)>>("__textfield_change")
            {
                handler(&buffer);
            }
        }
        owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
            "textFieldPlatformEdit",
        ));
    });
    let select_publish = publish.clone();
    let select_state = state.clone();
    let select_binding = field.text.clone();
    let delete_state = state.clone();
    let delete_binding = field.text.clone();
    node.set_attachment(
        TextInputAttachmentKey::CLIENT,
        cove_core::scene::TextInputClient::new(
            move || (client.snapshot)(),
            move |start, end| {
                select_state.borrow_mut().normalize(&select_binding.get());
                (client.select)(start, end);
                select_publish();
            },
            move |before, after| {
                delete_state.borrow_mut().normalize(&delete_binding.get());
                (client.delete_surrounding)(before, after);
                publish();
            },
        ),
    );
}

/// Resolve the platform anchor during layout/focus updates, independently of paint.
fn input_area(
    node: &Node,
    field: &TextField,
    state: &Rc<RefCell<FieldState>>,
    origin: Point,
) -> Option<cove_platform::TextInputArea> {
    if field.behavior.disabled || field.behavior.read_only || !node.is_layout_visible() {
        return None;
    }
    committed_input_area(node, field, state, origin)
}

/// The caret's committed text area, also for a read-only editor.
///
/// A read-only code field still answers F1 with the visible caret so a hover
/// card can anchor there; nothing is inserted either way.
fn committed_input_area(
    node: &Node,
    field: &TextField,
    state: &Rc<RefCell<FieldState>>,
    origin: Point,
) -> Option<cove_platform::TextInputArea> {
    if field.behavior.disabled || !node.is_layout_visible() {
        return None;
    }
    let focused = FocusChainHolder::current()
        .and_then(|chain| chain.focused())
        .is_some_and(|focused| focused == *node);
    let frame = node.frame();
    let size = (frame.width.to_bits(), frame.height.to_bits());
    // A field that never held focus has no anchor to publish. Once it has,
    // a blurred field republishes only after invalidation or a size change,
    // so typing cannot land on a stale anchor.
    let republish = {
        let mut state = state.borrow_mut();
        if focused {
            state.scroll.has_been_focused = true;
        }
        state.scroll.has_been_focused
            && (node.render_dirty() || state.scroll.committed_size != size)
    };
    if !focused && !republish {
        return None;
    }
    let environment = TextEnvironmentHolder::current()?;
    let theme = theme_for_node(node);
    let token = resolved_text_style(node, &theme);
    let line_height = token.line_height;
    let vertical = field.layout.axis == TextFieldAxis::Vertical;
    let mut state = state.borrow_mut();
    let addons = field.addon_layouts(
        node,
        &theme,
        &environment,
        &token.font,
        line_height,
        token.letter_spacing,
        state.buffer.character_count(),
        state.buffer.line_count(),
    );
    let inset = field.horizontal_inset(&theme);
    let width = field.available_text_width(&theme, node.frame().width, &addons);
    let selection = state.selection;
    let composition = state.composition.clone();
    let current = state.buffer.clone();
    let rendered = make_render_state(
        &field.inputs(&theme),
        &mut state.draft,
        &current,
        &selection,
        &composition,
        &field.placeholder,
        true,
    );
    let geometry = document_geometry(
        &field.inputs(&theme),
        token.font,
        line_height,
        token.letter_spacing,
        environment.atlas_texture_id as u64,
        if vertical { width } else { f32::INFINITY },
        rendered.shows_placeholder,
    );
    let mut document = TextDocumentLayout::new(rendered.buffer, geometry);
    let (caret_x, row) = document.caret(
        rendered.cursor_index,
        selection.line_end_affinity && !rendered.is_composing,
        &environment,
    );
    let height = (node.frame().height - 2.0 * field.vertical_inset(line_height)).max(line_height);
    state.scroll.visible_height = height;
    state.scroll.content_height = document.index().row_count() as f32 * line_height;
    state.scroll.max_y = (state.scroll.content_height - height).max(0.0);
    state.scroll.horizontal.viewport_width = width;
    state.scroll.horizontal.leading_inset = inset + addons.leading();
    if state.scroll.needs_caret_reveal {
        if caret_x - state.scroll.horizontal.offset > width - 1.0 {
            state.scroll.horizontal.offset = (caret_x - width + 1.0).max(0.0);
        }
        if caret_x < state.scroll.horizontal.offset {
            state.scroll.horizontal.offset = caret_x.max(0.0);
        }
        let top = row as f32 * line_height;
        if top < state.scroll.offset_y {
            state.scroll.offset_y = top;
        }
        if top + line_height > state.scroll.offset_y + height {
            state.scroll.offset_y = (top + line_height - height).max(0.0);
        }
        state.scroll.offset_y = state.scroll.offset_y.clamp(0.0, state.scroll.max_y);
    }
    state.scroll.committed_size = (frame.width.to_bits(), frame.height.to_bits());
    Some(cove_platform::TextInputArea {
        x: origin.x + inset + addons.leading(),
        y: origin.y + field.vertical_inset(line_height) + row as f32 * line_height
            - state.scroll.offset_y,
        width,
        height: line_height,
        cursor_x: (caret_x - state.scroll.horizontal.offset).clamp(0.0, width),
    })
}

fn pointer_character(
    node: &Node,
    field: &TextField,
    state: &Rc<RefCell<FieldState>>,
    point: Point,
) -> Option<usize> {
    let environment = TextEnvironmentHolder::current()?;
    let theme = theme_for_node(node);
    let token = resolved_text_style(node, &theme);
    let line_height = token.line_height;
    let mut state = state.borrow_mut();
    let addons = field.addon_layouts(
        node,
        &theme,
        &environment,
        &token.font,
        line_height,
        token.letter_spacing,
        state.buffer.character_count(),
        state.buffer.line_count(),
    );
    let leading = field.horizontal_inset(&theme) + addons.leading();
    let width = field.available_text_width(&theme, node.frame().width, &addons);
    let current = state.buffer.clone();
    let selection = state.selection;
    let composition = state.composition.clone();
    let rendered = make_render_state(
        &field.inputs(&theme),
        &mut state.draft,
        &current,
        &selection,
        &composition,
        &field.placeholder,
        true,
    );
    if rendered.shows_placeholder {
        return Some(0);
    }
    let geometry = document_geometry(
        &field.inputs(&theme),
        token.font,
        line_height,
        token.letter_spacing,
        environment.atlas_texture_id as u64,
        if field.layout.axis == TextFieldAxis::Vertical {
            width
        } else {
            f32::INFINITY
        },
        false,
    );
    let mut document = TextDocumentLayout::new(rendered.buffer, geometry);
    let origin = node.absolute_origin();
    let x = point.x - origin.x - leading + state.scroll.horizontal.offset;
    let row = ((point.y - origin.y - field.vertical_inset(line_height) + state.scroll.offset_y)
        .max(0.0)
        / line_height.max(1.0)) as usize;
    Some(
        document
            .character(row, x, &environment)
            .min(current.character_count()),
    )
}

fn notify_composition(node: &Node, state: &Rc<RefCell<FieldState>>) {
    let composing = state.borrow().composition.is_active();
    if let Some(handler) = node.attachment::<cove_core::scene::TextInputEditingChangeHandler>(
        TextInputAttachmentKey::EDITING_CHANGE_HANDLER,
    ) {
        handler(composing);
    }
}

fn install_interaction(
    node: &Node,
    field: &TextField,
    state: Rc<RefCell<FieldState>>,
    interaction: &Rc<TextFieldInteractionState>,
) {
    install_text_input_client(node, field, &state);
    // Publish the caller's `TextFieldEvents` callbacks on the node. Every handler
    // below *reads* these attachments, but nothing ever wrote them, so
    // `on_submit`/`on_change`/`on_blur` and friends were silently dead — a
    // `TextField` with an `on_submit` could never commit on Return. Swift calls
    // them directly from the host (`TextField.swift:588`,
    // `TextFieldHost.swift:134`). Swift installs these unconditionally, including
    // while disabled, because the disabled path still publishes chrome state.
    if let Some(handler) = field.events.on_submit.clone() {
        node.set_attachment("__textfield_submit", handler);
    }
    if let Some(handler) = field.events.on_change.clone() {
        node.set_attachment("__textfield_change", handler);
    }
    if let Some(handler) = field.events.on_cancel.clone() {
        node.set_attachment("__textfield_cancel", handler);
    }
    if let Some(handler) = field.events.on_focus.clone() {
        node.set_attachment("__textfield_focus", handler);
    }
    if let Some(handler) = field.events.on_blur.clone() {
        node.set_attachment("__textfield_blur", handler);
    }
    if let Some(handler) = field.events.on_clear.clone() {
        node.set_attachment("__textfield_clear", handler);
    }
    if let Some(handler) = field.events.on_key_down.clone() {
        node.set_attachment("__textfield_key", handler);
    }
    if let Some(handler) = field.events.on_hover_change.clone() {
        node.set_attachment("__textfield_hover", handler);
    }
    if let Some(handler) = field.events.on_caret_change.clone() {
        node.set_attachment("__textfield_caret", handler);
    }
    let Some(registry) = InteractionRegistryHolder::current() else {
        return;
    };
    if field.behavior.disabled {
        registry.remove(node);
        return;
    }
    let hover_node = node.downgrade();
    let hover_interaction = interaction.clone();
    registry.set_hover(
        node,
        Rc::new(move |phase| {
            let Some(owner) = Node::upgrade(&hover_node) else {
                return;
            };
            // Touch surfaces never send a hover phase, so the event itself is
            // the capability signal; the overlay simply stays hidden there.
            let hovered = phase == HoverPhase::Enter
                && owner
                    .composition_value(&crate::adaptive::SurfaceEnvironment::key())
                    .supports_hover();
            owner.set_attachment(SCROLLBAR_HOVERED, hovered);
            hover_interaction.update(|state| state.hovered = hovered);
            if phase == HoverPhase::Leave
                && let Some(handler) = owner
                    .attachment::<Rc<dyn Fn(Option<TextFieldHoverAnchor>)>>("__textfield_hover")
            {
                handler(None);
            }
            // The owning style controls chrome; resetting fill here would restart
            // an interpolation from transparent on every hover.
            owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                "textFieldHover",
            ));
        }),
    );

    let pointer_node = node.downgrade();
    // Composition state must reach the host so it can show marked text and
    // defer commit-time side effects while an IME is live.
    let editing_interaction = interaction.clone();
    node.set_attachment(
        TextInputAttachmentKey::EDITING_CHANGE_HANDLER,
        Rc::new(move |is_composing: bool| {
            editing_interaction.update(|state| state.composing = is_composing);
        }) as cove_core::scene::TextInputEditingChangeHandler,
    );
    let report_node = node.downgrade();
    let focus_state = state.clone();
    let focus_interaction = interaction.clone();
    node.set_attachment(
        TextInputAttachmentKey::FOCUS_CHANGE_HANDLER,
        Rc::new(move |focused: bool| {
            let Some(owner) = Node::upgrade(&report_node) else {
                return;
            };
            {
                let mut state = focus_state.borrow_mut();
                state.is_focused = focused;
                if !focused {
                    state.clear_composition();
                }
                state.record_activity();
            }
            focus_interaction.update(|state| {
                state.focused = focused;
                if !focused {
                    state.composing = false;
                }
            });
            if !focused {
                owner.remove_attachment(TextInputAttachmentKey::AREA);
            }
            owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                "textFieldFocus",
            ));
            if !focused && let Some(session) = owner.first_resource::<TextCompletionSession>() {
                session.dismiss();
            }
            if let Some(handler) = owner.attachment::<Rc<dyn Fn()>>(if focused {
                "__textfield_focus"
            } else {
                "__textfield_blur"
            }) {
                handler();
            }
        }) as cove_core::scene::TextInputFocusChangeHandler,
    );

    let pointer_state = state.clone();
    let pointer_field = field.snapshot();
    let clear_binding = field.text.clone();
    let behavior = field.behavior.clone();
    registry.set_pointer(
        node,
        InputHandlerRoute::text_input(),
        Rc::new(move |event, phase, event_phase| {
            if event_phase == cove_core::scene::EventPhase::Capture
                || event.button != MouseButton::Left
            {
                return EventResult::Ignored;
            }
            let Some(owner) = Node::upgrade(&pointer_node) else {
                return EventResult::Ignored;
            };
            match phase {
                PointerPhase::Down => {
                    pointer_state
                        .borrow_mut()
                        .normalize(&pointer_field.text.get());
                    if let Some(session) = owner.first_resource::<TextCompletionSession>() {
                        session.dismiss();
                    }
                    if scroll_minimap(&owner, &pointer_state, event.x, event.y, true) {
                        owner.set_attachment(MINIMAP_DRAGGING, true);
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.acquire(&owner, true);
                        }
                        return EventResult::Handled;
                    }
                    if behavior.clearable
                        && owner
                            .attachment::<f32>("__textfield_clear_hit")
                            .is_some_and(|hit| event.x >= hit)
                    {
                        let mut state = pointer_state.borrow_mut();
                        state.buffer = TextBuffer::empty();
                        state.selection.cursor_index = 0;
                        state.selection.anchor = None;
                        state.clear_composition();
                        state.record_activity();
                        drop(state);
                        notify_composition(&owner, &pointer_state);
                        clear_binding.set(TextBuffer::empty());
                        if let Some(handler) =
                            owner.attachment::<Rc<dyn Fn(&TextBuffer)>>("__textfield_change")
                        {
                            handler(&TextBuffer::empty());
                        }
                        if let Some(handler) = owner.attachment::<Rc<dyn Fn()>>("__textfield_clear")
                        {
                            handler();
                        }
                        owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                            "textFieldClear",
                        ));
                        return EventResult::Handled;
                    }
                    if let Some(chain) = FocusChainHolder::current() {
                        chain.focus(Some(&owner), true);
                    }
                    if let Some(capture) = PointerCaptureHolder::current() {
                        capture.acquire(&owner, true);
                    }
                    if event.clicks >= 3 {
                        let mut state = pointer_state.borrow_mut();
                        state.clear_composition();
                        state.selection.anchor = Some(0);
                        state.selection.cursor_index = state.buffer.character_count();
                        state.record_activity();
                    } else if let Some(index) = pointer_character(
                        &owner,
                        &pointer_field,
                        &pointer_state,
                        Point::new(event.x, event.y),
                    ) {
                        let mut state = pointer_state.borrow_mut();
                        state.clear_composition();
                        if event.clicks >= 2 {
                            let text = state.buffer.string_value();
                            let offset = state.buffer.utf8_offset_for_character_index(index);
                            let range =
                                crate::primitives::text::rich::text_selection::TextBoundaries::word(
                                    &text, offset,
                                );
                            state.selection.anchor =
                                Some(state.buffer.character_index_for_utf8_offset(range.start));
                            state.selection.cursor_index =
                                state.buffer.character_index_for_utf8_offset(range.end);
                            state.record_activity();
                        } else {
                            state.move_cursor(index, event.modifiers.has_shift(), None);
                        }
                    }
                    let mut state = pointer_state.borrow_mut();
                    state.is_dragging = true;
                    state.is_focused = true;
                    drop(state);
                    notify_composition(&owner, &pointer_state);
                    owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                        "textFieldPointer",
                    ));
                    EventResult::Handled
                }
                PointerPhase::Up => {
                    owner.set_attachment(MINIMAP_DRAGGING, false);
                    pointer_state.borrow_mut().is_dragging = false;
                    if PointerCaptureHolder::current()
                        .and_then(|capture| capture.target())
                        .is_some_and(|target| target == owner)
                        && let Some(capture) = PointerCaptureHolder::current()
                    {
                        capture.release();
                    }
                    EventResult::Handled
                }
            }
        }),
    );

    // A drag that began on the overview follows the pointer anywhere.
    let motion_node = node.downgrade();
    let motion_state = state.clone();
    let motion_field = field.snapshot();
    registry.set_motion(
        node,
        InputHandlerRoute::text_input(),
        Rc::new(move |event, _phase| {
            let Some(owner) = Node::upgrade(&motion_node) else {
                return EventResult::Ignored;
            };
            if owner.attachment::<bool>(MINIMAP_DRAGGING) != Some(true) {
                if !motion_state.borrow().is_dragging
                    || PointerCaptureHolder::current()
                        .and_then(|capture| capture.target())
                        .as_ref()
                        != Some(&owner)
                {
                    return EventResult::Ignored;
                }
                if let Some(index) = pointer_character(
                    &owner,
                    &motion_field,
                    &motion_state,
                    Point::new(event.x, event.y),
                ) {
                    motion_state.borrow_mut().move_cursor(index, true, None);
                    owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                        "textFieldSelection",
                    ));
                }
                return EventResult::Handled;
            }
            scroll_minimap(&owner, &motion_state, event.x, event.y, false);
            EventResult::Handled
        }),
    );

    // Typed text and IME composition. Swift registers both on the same
    // `.textInput` route (`TextFieldInputController.swift:23,32`). Without them
    // `EventDispatcher::dispatch_text` finds no handler and every keystroke is a
    // no-op — the platform layer cannot enter text on desktop or Android.
    let text_node = node.downgrade();
    let text_state = state.clone();
    let text_behavior = field.behavior.clone();
    let text_limit = field.behavior.max_length;
    let text_binding = field.text.clone();
    registry.set_text(
        node,
        InputHandlerRoute::text_input(),
        Rc::new(move |incoming: &str, _phase| {
            if text_behavior.read_only {
                return EventResult::Handled;
            }
            let Some(owner) = Node::upgrade(&text_node) else {
                return EventResult::Ignored;
            };
            text_state.borrow_mut().normalize(&text_binding.get());
            insert_replacing_selection_limited(&mut text_state.borrow_mut(), incoming, text_limit);
            // Publish the edit back to the caller's binding. Nothing ever wrote
            // `field.text`, so `TextFieldHost::update_node` re-read the unchanged
            // binding on the next pass and normalised the internal buffer back —
            // the user's keystrokes were discarded before any commit could see
            // them. Swift assigns through the binding itself.
            let buffer = text_state.borrow().buffer.clone();
            text_state.borrow_mut().clear_composition();
            notify_composition(&owner, &text_state);
            text_binding.set(buffer.clone());
            if let Some(handler) = owner.attachment::<Rc<dyn Fn(&TextBuffer)>>("__textfield_change")
            {
                handler(&buffer);
            }
            if let Some(session) = owner.first_resource::<TextCompletionSession>() {
                session.activity(false);
            }
            owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                "textFieldInput",
            ));
            EventResult::Handled
        }),
    );

    let editing_node = node.downgrade();
    let editing_state = state.clone();
    let editing_behavior = field.behavior.clone();
    registry.set_editing(
        node,
        InputHandlerRoute::text_input(),
        Rc::new(move |event: &cove_platform::TextEditingEvent, _phase| {
            if editing_behavior.read_only {
                return EventResult::Handled;
            }
            let Some(owner) = Node::upgrade(&editing_node) else {
                return EventResult::Ignored;
            };
            let mut state = editing_state.borrow_mut();
            // `FieldCompositionState` stores `usize` offsets while the platform
            // event carries `i32`, so clamp in the event's domain and convert
            // once. The subtraction saturates: a length past the text end is
            // simply the remainder.
            // SDL editing offsets count Unicode scalars, while field offsets
            // count graphemes. Map through bytes before storing the selection.
            let pending = TextBuffer::new(&event.text);
            let scalar_byte = |offset: i32| {
                event
                    .text
                    .char_indices()
                    .nth(offset.max(0) as usize)
                    .map_or(event.text.len(), |(byte, _)| byte)
            };
            let first_byte = scalar_byte(event.start);
            let last_byte = scalar_byte(event.start.max(0).saturating_add(event.length.max(0)));
            let mut start = pending.character_index_for_utf8_offset(first_byte);
            if pending.utf8_offset_for_character_index(start) > first_byte {
                start = start.saturating_sub(1);
            }
            let end = if event.length <= 0 {
                start
            } else {
                pending.character_index_for_utf8_offset(last_byte)
            };
            state.composition.text = event.text.clone();
            state.composition.start = start;
            state.composition.length = end.saturating_sub(start);
            drop(state);
            notify_composition(&owner, &editing_state);
            editing_state.borrow_mut().record_activity();
            if let Some(handler) = owner.attachment::<Rc<dyn Fn()>>("__textfield_editing") {
                handler();
            }
            owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                "textFieldComposition",
            ));
            EventResult::Handled
        }),
    );

    let key_node = node.downgrade();
    let key_state = state.clone();
    let key_behavior = field.behavior.clone();
    let key_binding = field.text.clone();
    let key_field = field.snapshot();
    let indentation = field.code_editing.indentation_width;
    registry.set_key(
        node,
        InputHandlerRoute::text_input(),
        Rc::new(move |event, _phase| {
            let Some(owner) = Node::upgrade(&key_node) else {
                return EventResult::Ignored;
            };
            if let Some(custom) =
                owner.attachment::<Rc<dyn Fn(&cove_platform::KeyEvent) -> bool>>("__textfield_key")
                && custom(event)
            {
                return EventResult::Handled;
            }
            let session = owner.first_resource::<TextCompletionSession>();
            if !key_behavior.read_only
                && session
                    .as_ref()
                    .is_some_and(|session| session.handle_key(event))
            {
                return EventResult::Handled;
            }
            if let Some(snippet) = owner.first_resource::<TextSnippetSession>() {
                let buffer = key_state.borrow().buffer.clone();
                if let Some(command) = snippet.command(event, &buffer) {
                    if let Some((start, end)) = command {
                        let mut state = key_state.borrow_mut();
                        state.selection.anchor = (start != end).then_some(start);
                        state.selection.cursor_index = end;
                        state.record_activity();
                    }
                    return EventResult::Handled;
                }
            }
            // F1 asks the language service for documentation at the visible
            // caret. A read-only code editor still answers it, since nothing is
            // mutated and reviewers read code they cannot edit.
            if event.scancode == crate::Scancode::F1
                && event.modifiers.is_empty()
                && let Some(request) = key_field.code_editing.on_request_hover.clone()
                && let Some(area) =
                    committed_input_area(&owner, &key_field, &key_state, owner.absolute_origin())
            {
                let character_index = key_state.borrow().selection.cursor_index;
                // The anchor sits on the caret itself: the area's column
                // origin plus the caret's offset inside the scrolled column,
                // and its bottom edge is the caret's baseline.
                request(TextFieldHoverAnchor::new(
                    character_index,
                    area.x + area.cursor_x,
                    area.y + area.height,
                ));
                return EventResult::Handled;
            }
            // Escape cancels through the caller's callback without holding the
            // editing state, so the callback may safely recompose this field.
            if event.scancode == crate::Scancode::ESCAPE
                && let Some(cancel) = owner.attachment::<Rc<dyn Fn()>>("__textfield_cancel")
            {
                if let Ok(mut state) = key_state.try_borrow_mut() {
                    state.clear_composition();
                }
                cancel();
                // Resigning focus hands the container back its focus, the way
                // Swift returns focus when the editor closes.
                if let Some(chain) = FocusChainHolder::current() {
                    let mut ancestor = owner.parent();
                    while let Some(node) = ancestor {
                        if node.is_focusable() {
                            chain.focus(Some(&node), false);
                            break;
                        }
                        ancestor = node.parent();
                    }
                }
                return EventResult::Handled;
            }
            if _phase != cove_core::scene::EventPhase::Capture
                && key_field.code_editing.shows_line_numbers
                && key_field.layout.axis == TextFieldAxis::Vertical
                && !key_behavior.disabled
                && !key_behavior.secure
                && let Some(gutter) =
                    owner.first_resource::<crate::primitives::text::code::TextGutterSession>()
            {
                let state = key_state.borrow();
                let cursor = state.selection.cursor_index;
                let line = state.buffer.line_index_for_character_index(cursor);
                let count = state.buffer.character_count();
                let range = state.buffer.line_range(line);
                let boundary = match event.scancode {
                    crate::Scancode::BACKSPACE => cursor == range.start,
                    crate::Scancode::DELETE => cursor.saturating_add(1) >= range.end,
                    _ => true,
                };
                let available = boundary && !state.composition.is_active();
                drop(state);
                if available && gutter.handle_key(event, line, count, key_behavior.read_only) {
                    if let Some(hidden) = gutter
                        .folds()
                        .hidden_lines()
                        .iter()
                        .find(|range| range.contains(&line))
                    {
                        let mut state = key_state.borrow_mut();
                        let header = state.buffer.line_range(hidden.start.saturating_sub(1));
                        state.move_cursor(header.end.saturating_sub(1), false, None);
                    }
                    owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                        "textFieldGutter",
                    ));
                    return EventResult::Handled;
                }
            }
            let primary = event.modifiers.has_gui() || event.modifiers.has_ctrl();
            // Undo / redo take priority over every other binding. Primary-Z
            // steps back, Primary-Y and Primary-Shift-Z step forward — the
            // latter is the Windows/Linux spelling, and Swift accepts both
            // (`TextField.swift:450`).
            if primary
                && (event.scancode == crate::Scancode::Z || event.scancode == crate::Scancode::Y)
            {
                if key_behavior.read_only {
                    return EventResult::Handled;
                }
                restore_history(
                    &owner,
                    &key_state,
                    &key_binding,
                    event.scancode == crate::Scancode::Y || event.modifiers.has_shift(),
                );
                return EventResult::Handled;
            }
            if primary && event.scancode == crate::Scancode::V {
                if key_behavior.read_only {
                    return EventResult::Handled;
                }
                let paste_state = key_state.clone();
                let paste_binding = key_binding.clone();
                let paste_node = owner.downgrade();
                let paste_limit = key_behavior.max_length;
                cove_core::scene::ClipboardHolder::request_read(Box::new(move |text| {
                    let (Some(text), Some(owner)) = (text, Node::upgrade(&paste_node)) else {
                        return;
                    };
                    let buffer = {
                        let mut state = paste_state.borrow_mut();
                        state.normalize(&paste_binding.get());
                        insert_replacing_selection_limited(&mut state, &text, paste_limit);
                        state.buffer.clone()
                    };
                    if !buffer.has_same_contents(&paste_binding.get()) {
                        paste_binding.set(buffer.clone());
                        if let Some(handler) =
                            owner.attachment::<Rc<dyn Fn(&TextBuffer)>>("__textfield_change")
                        {
                            handler(&buffer);
                        }
                    }
                    owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                        "textFieldPaste",
                    ));
                }));
                return EventResult::Handled;
            }
            // Return submits unless Shift asks for a newline in the buffer:
            // Primary-Return always submits, Shift-Return inserts anywhere, and
            // a vertical field takes the newline for a plain Return.
            if matches!(
                event.scancode,
                crate::Scancode::RETURN | crate::Scancode::KEYPAD_ENTER
            ) && !event.modifiers.has_shift()
                && (key_field.layout.axis != TextFieldAxis::Vertical || primary)
            {
                return match owner.attachment::<Rc<dyn Fn()>>("__textfield_submit") {
                    Some(submit) => {
                        submit();
                        EventResult::Handled
                    }
                    None => EventResult::Ignored,
                };
            }
            let edit = || {
                let mut state = key_state.borrow_mut();
                // The caller may have rewritten the text since the last pass. Editing
                // the old copy would write it back over theirs.
                state.normalize(&key_binding.get());
                // Backspace and Delete coalesce with the neighbouring deletion;
                // every other command is its own undo step.
                let kind = if !primary
                    && matches!(
                        event.scancode,
                        crate::Scancode::BACKSPACE | crate::Scancode::DELETE
                    ) {
                    TextEditKind::Deletion
                } else {
                    TextEditKind::Atomic
                };
                state.begin_edit(&key_binding.get(), kind);
                let result = apply_key(&mut state, &owner, &key_field, event, indentation, primary);
                state.end_edit();
                result
            };
            let result = edit();
            if result == EventResult::Handled {
                notify_composition(&owner, &key_state);
                owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                    "textFieldKey",
                ));
                // Edits reach the caller's binding; otherwise the next pass reads
                // the old text back over them.
                let buffer = key_state.borrow().buffer.clone();
                if !buffer.has_same_contents(&key_binding.get()) {
                    key_binding.set(buffer.clone());
                    if let Some(handler) =
                        owner.attachment::<Rc<dyn Fn(&TextBuffer)>>("__textfield_change")
                    {
                        handler(&buffer);
                    }
                }
                if let Some(session) = session {
                    session.activity(false);
                }
            }
            result
        }),
    );

    let wheel_node = node.downgrade();
    let wheel_state = state.clone();
    let wheel_field = field.snapshot();
    registry.set_wheel(
        node,
        InputHandlerRoute::text_input(),
        Rc::new(move |event: &MouseWheelEvent, _phase| {
            let Some(owner) = Node::upgrade(&wheel_node) else {
                return EventResult::Ignored;
            };
            if !event.input.is_continuation()
                && let (Some(mouse_x), Some(mouse_y)) = (event.mouse_x, event.mouse_y)
            {
                let frame = owner.absolute_frame();
                if mouse_x < frame.x
                    || mouse_x > frame.x + frame.width
                    || mouse_y < frame.y
                    || mouse_y > frame.y + frame.height
                {
                    return EventResult::Ignored;
                }
            }
            // A field consumes the wheel only while its rows overflow the
            // frame — the same condition that caps a growing field. Anything
            // shorter keeps bubbling to the enclosing scroll view.
            refresh_scroll_metrics(&owner, &wheel_field, &wheel_state);
            let mut state = wheel_state.borrow_mut();
            if state.composition.is_active() {
                return EventResult::Ignored;
            }
            let frame = owner.frame();
            let (dx, dy) =
                event.displacement(MULTILINE_WHEEL_STEP, frame.width, frame.height.max(1.0));
            // Wheel deltas report content movement; the viewport moves the
            // other way, matching the scroll view's convention.
            let (delta_x, delta_y) = (-dx, -dy);
            let max_x = state.scroll.horizontal.maximum();
            let max_y = state.scroll.max_y.max(0.0);
            if (max_x <= 0.0 && max_y <= 0.0) || (delta_x == 0.0 && delta_y == 0.0) {
                return EventResult::Ignored;
            }
            let previous = (state.scroll.horizontal.offset, state.scroll.offset_y);
            state.scroll.horizontal.offset =
                (state.scroll.horizontal.offset + delta_x).clamp(0.0, max_x);
            state.scroll.offset_y = (state.scroll.offset_y + delta_y).clamp(0.0, max_y);
            let changed = previous != (state.scroll.horizontal.offset, state.scroll.offset_y);
            // The wheel moves the viewport, not the caret: the next paint must
            // not snap the visible region back to the caret.
            state.scroll.needs_caret_reveal = false;
            let offset = Point::new(state.scroll.horizontal.offset, state.scroll.offset_y);
            drop(state);
            owner.set_content_offset(offset);
            if let Some(session) = owner.first_resource::<TextCompletionSession>() {
                session.dismiss();
            }
            if changed {
                owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                    "textFieldWheel",
                ));
            }
            // An overflowing field keeps the wheel at its boundary instead of
            // forwarding the leftover delta to the enclosing scroll view.
            EventResult::Handled
        }),
    );
}

/// Displayed-row geometry for caret commands that step by row instead of by
/// logical line: soft-wrapped fields, Home/End affinity and page moves.
struct NavigationContext {
    document: TextDocumentLayout,
    environment: TextEnvironment,
    line_height: f32,
    vertical_inset: f32,
}

/// Build the row geometry from the live buffer, so navigation sees the text as
/// typed instead of the last painted pass.
fn navigation_context(
    node: &Node,
    field: &TextField,
    state: &FieldState,
) -> Option<NavigationContext> {
    let environment = TextEnvironmentHolder::current()?;
    let theme = theme_for_node(node);
    let token = resolved_text_style(node, &theme);
    let line_height = token.line_height;
    let addons = field.addon_layouts(
        node,
        &theme,
        &environment,
        &token.font,
        line_height,
        token.letter_spacing,
        state.buffer.character_count(),
        state.buffer.line_count(),
    );
    let width = field.available_text_width(&theme, node.frame().width, &addons);
    let geometry = document_geometry(
        &field.inputs(&theme),
        token.font.clone(),
        line_height,
        token.letter_spacing,
        environment.atlas_texture_id as u64,
        if field.layout.axis == TextFieldAxis::Vertical && field.layout.wraps_lines {
            width
        } else {
            f32::INFINITY
        },
        state.buffer.is_empty(),
    );
    Some(NavigationContext {
        document: TextDocumentLayout::new(state.buffer.clone(), geometry),
        environment,
        line_height,
        vertical_inset: field.vertical_inset(line_height),
    })
}

/// Resolve the caller's font and line height from the style host.
///
/// Text modifiers land on the style host (the only node with a layout node in
/// that chain), so the surface reads them from its layout ancestry, like
/// Swift's `resolvedFont(node:)`.
/// Hover scrollbar: the thumb appears while the pointer rests on a field whose
/// rows or columns overflow.
fn draw_scrollbar_overlay(node: &Node, list: &mut cove_core::DrawList, origin: Point) {
    if node.attachment::<bool>(SCROLLBAR_HOVERED) != Some(true) {
        return;
    }
    let Some(state) = node.attachment::<Rc<RefCell<FieldState>>>(FIELD_STATE_KEY) else {
        return;
    };
    let state = state.borrow();
    if state.scroll.max_y <= 0.0 && state.scroll.horizontal.maximum() <= 0.0 {
        return;
    }
    let theme = theme_for_node(node);
    let thickness = 3.0;
    let inset = theme.spacing.xs;
    let track = theme.colors.on_surface_muted.multiplied_alpha(0.18);
    let thumb = theme.colors.on_surface_muted.multiplied_alpha(0.55);
    if let Some(metrics) = scrollbar_metrics(&state.scroll, node, origin, thickness, inset) {
        let _ = list.add_rounded_rect(metrics.track_rect, thickness * 0.5, track);
        let _ = list.add_rounded_rect(metrics.thumb_rect, thickness * 0.5, thumb);
    }
    if let Some(metrics) =
        horizontal_scrollbar_metrics(&state.scroll, node, origin, thickness, inset)
    {
        let _ = list.add_rounded_rect(metrics.track_rect, thickness * 0.5, track);
        let _ = list.add_rounded_rect(metrics.thumb_rect, thickness * 0.5, thumb);
    }
}

/// Fill the scroll limits from the laid-out rows when no paint has run yet.
///
/// The wheel can arrive before the first draw of a freshly mounted field;
/// without this the viewport claims zero content and swallows the gesture.
fn refresh_scroll_metrics(node: &Node, field: &TextField, state: &Rc<RefCell<FieldState>>) {
    if state.borrow().scroll.max_y > 0.0 {
        return;
    }
    let context = {
        let borrowed = state.borrow();
        navigation_context(node, field, &borrowed)
    };
    let Some(context) = context else {
        return;
    };
    let rows = context.document.index().row_count() as f32;
    let visible = (node.frame().height - 2.0 * context.vertical_inset).max(context.line_height);
    let content = rows * context.line_height;
    let mut state = state.borrow_mut();
    state.scroll.visible_height = visible;
    state.scroll.content_height = content;
    state.scroll.max_y = (content - visible).max(0.0);
}

/// Logical-line fallback for Home/End when no row geometry is available.
fn logical_line_boundary(buffer: &TextBuffer, caret: usize, ending: bool, vertical: bool) -> usize {
    if !vertical {
        return if ending { buffer.character_count() } else { 0 };
    }
    let range = buffer.line_range(buffer.line_index_for_character_index(caret));
    if ending { range.end } else { range.start }
}

/// Apply one command key to the editing state.
///
/// The caller has already opened an undo bracket and normalised the buffer, so
/// every buffer mutation here is one step in the history.
#[allow(
    clippy::too_many_arguments,
    reason = "one dispatch table mirrors the Swift key switch"
)]
fn apply_key(
    state: &mut FieldState,
    owner: &Node,
    field: &TextField,
    event: &cove_platform::KeyEvent,
    indentation: Option<usize>,
    primary: bool,
) -> EventResult {
    let shift = event.modifiers.has_shift();
    let command = event.modifiers.has_gui();
    let word = event.modifiers.has_alt() || (event.modifiers.has_ctrl() && !command);
    let vertical = field.layout.axis == TextFieldAxis::Vertical;
    let scancode = event.scancode;
    let caret = state.selection.cursor_index;
    let preferred_x = state.selection.preferred_caret_x();
    let mut context = if vertical {
        navigation_context(owner, field, state)
    } else {
        None
    };

    match scancode {
        // A code field consumes Tab to indent; every other field yields it to
        // focus traversal.
        crate::Scancode::TAB if vertical && indentation.is_some() && !primary => {
            let width = indentation.unwrap_or(4);
            if field.behavior.read_only {
                return EventResult::Handled;
            }
            let selection = state.selection_range();
            let result = super::text_field_code_editing::indent_lines(
                &state.buffer.string_value(),
                state.selection.cursor_index,
                state.selection.anchor,
                selection,
                width,
                shift,
                field.behavior.max_length,
            );
            if result.changed {
                state.buffer = TextBuffer::new(&result.text);
                state.selection.cursor_index = result.caret;
                state.selection.anchor = result.anchor;
                state.clear_composition();
                state.selection.set_preferred_caret_x(None);
                state.record_activity();
            }
            EventResult::Handled
        }
        crate::Scancode::A if primary => {
            state.selection.anchor = Some(0);
            state.selection.cursor_index = state.buffer.character_count();
            state.record_activity();
            EventResult::Handled
        }
        crate::Scancode::C | crate::Scancode::X if primary => {
            if !field.behavior.secure
                && let Some((low, high)) = state.selection_range()
            {
                let text = state.buffer.substring(low..high);
                cove_core::scene::ClipboardHolder::write(&text);
                if scancode == crate::Scancode::X && !field.behavior.read_only {
                    delete_selection(state);
                }
            }
            EventResult::Handled
        }
        crate::Scancode::ARROW_LEFT | crate::Scancode::ARROW_RIGHT => {
            let forward = scancode == crate::Scancode::ARROW_RIGHT;
            let target = if command {
                // Command-arrow spans one line, not the whole document:
                // Command-Up/Down own the document bounds.
                match context.as_mut() {
                    Some(ctx) => {
                        let document = &mut ctx.document;
                        let environment = &ctx.environment;
                        line_boundary(
                            &state.buffer,
                            caret,
                            forward,
                            vertical,
                            field.layout.wraps_lines,
                            Some(document),
                            environment,
                            state.selection.line_end_affinity,
                        )
                    }
                    None => logical_line_boundary(&state.buffer, caret, forward, vertical),
                }
            } else if word {
                if forward {
                    word_boundary_after(&state.buffer, caret)
                } else {
                    word_boundary_before(&state.buffer, caret)
                }
            } else if forward {
                grapheme_after(&state.buffer, caret)
            } else {
                grapheme_before(&state.buffer, caret)
            };
            state.move_cursor(target, shift, None);
            EventResult::Handled
        }
        crate::Scancode::ARROW_UP | crate::Scancode::ARROW_DOWN => {
            let delta = if scancode == crate::Scancode::ARROW_UP {
                -1
            } else {
                1
            };
            if command {
                let target = if delta < 0 {
                    0
                } else {
                    state.buffer.character_count()
                };
                state.move_cursor(target, shift, None);
                return EventResult::Handled;
            }
            let moved = match context.as_mut() {
                Some(ctx) => {
                    let document = &mut ctx.document;
                    let environment = &ctx.environment;
                    move_cursor_vertically(
                        &state.buffer,
                        caret,
                        delta,
                        preferred_x,
                        state.selection.line_end_affinity,
                        Some(document),
                        Some(environment),
                    )
                }
                None => move_cursor_vertically(
                    &state.buffer,
                    caret,
                    delta,
                    preferred_x,
                    state.selection.line_end_affinity,
                    None,
                    None,
                ),
            };
            state.move_cursor(
                moved.target,
                shift,
                moved.preferred_caret_x.map(f32::from_bits),
            );
            state.selection.line_end_affinity = moved.line_end_affinity;
            EventResult::Handled
        }
        crate::Scancode::PAGE_UP | crate::Scancode::PAGE_DOWN => {
            let direction = if scancode == crate::Scancode::PAGE_UP {
                -1
            } else {
                1
            };
            let frame_height = owner.frame().height;
            let moved = match context.as_mut() {
                Some(ctx) => {
                    let line_height = ctx.line_height;
                    let vertical_inset = ctx.vertical_inset;
                    let document = &mut ctx.document;
                    let environment = &ctx.environment;
                    move_cursor_by_page(
                        &state.buffer,
                        caret,
                        direction,
                        frame_height,
                        line_height,
                        vertical_inset,
                        preferred_x,
                        Some(document),
                        Some(environment),
                    )
                }
                None => move_cursor_by_page(
                    &state.buffer,
                    caret,
                    direction,
                    frame_height,
                    16.0,
                    0.0,
                    preferred_x,
                    None,
                    None,
                ),
            };
            if let Some(moved) = moved {
                state.move_cursor(
                    moved.target,
                    shift,
                    moved.preferred_caret_x.map(f32::from_bits),
                );
                state.selection.line_end_affinity = moved.line_end_affinity;
            }
            EventResult::Handled
        }
        crate::Scancode::HOME | crate::Scancode::END => {
            let ending = scancode == crate::Scancode::END;
            // Primary-Home/End reach the document bounds; the plain and Shift
            // spellings stop at the current line.
            if primary && (command || event.modifiers.has_ctrl()) {
                let target = if ending {
                    state.buffer.character_count()
                } else {
                    0
                };
                state.move_cursor(target, shift, None);
                state.selection.line_end_affinity = false;
                return EventResult::Handled;
            }
            let target = match context.as_mut() {
                Some(ctx) => {
                    let document = &mut ctx.document;
                    let environment = &ctx.environment;
                    line_boundary(
                        &state.buffer,
                        caret,
                        ending,
                        vertical,
                        field.layout.wraps_lines,
                        Some(document),
                        environment,
                        state.selection.line_end_affinity,
                    )
                }
                None => logical_line_boundary(&state.buffer, caret, ending, vertical),
            };
            state.move_cursor(target, shift, None);
            // A wrapped row keeps its displayed row for the next vertical move.
            state.selection.line_end_affinity = ending && vertical;
            EventResult::Handled
        }
        crate::Scancode::BACKSPACE | crate::Scancode::DELETE => {
            if field.behavior.read_only {
                return EventResult::Handled;
            }
            if state.selection_range().is_some() {
                delete_selection(state);
                return EventResult::Handled;
            }
            let forward = scancode == crate::Scancode::DELETE;
            let (low, high) = if forward {
                let end = if command {
                    let line = state.buffer.line_index_for_character_index(caret);
                    state.buffer.line_range(line).end
                } else if word {
                    word_boundary_after(&state.buffer, caret)
                } else {
                    grapheme_after(&state.buffer, caret)
                };
                (caret, end)
            } else {
                let start = if command {
                    let line = state.buffer.line_index_for_character_index(caret);
                    state.buffer.line_range(line).start
                } else if word {
                    word_boundary_before(&state.buffer, caret)
                } else {
                    grapheme_before(&state.buffer, caret)
                };
                (start, caret)
            };
            if high <= low {
                return EventResult::Handled;
            }
            state.buffer = state.buffer.delete(low..high);
            state.clear_composition();
            state.selection.anchor = None;
            state.selection.cursor_index = low;
            state.selection.set_preferred_caret_x(None);
            state.record_activity();
            EventResult::Handled
        }
        crate::Scancode::RETURN | crate::Scancode::KEYPAD_ENTER => {
            if primary || field.behavior.read_only {
                return EventResult::Handled;
            }
            let newline = indented_newline(
                &state.buffer.string_value(),
                state.selection.cursor_index,
                indentation,
                true,
            );
            insert_replacing_selection(state, &newline);
            EventResult::Handled
        }
        _ => EventResult::Ignored,
    }
}

/// Caret steps are one extended grapheme cluster, the Swift `Character` unit.
fn grapheme_before(buffer: &TextBuffer, index: usize) -> usize {
    index.min(buffer.character_count()).saturating_sub(1)
}

/// The end of the user-perceived character after `index`.
fn grapheme_after(buffer: &TextBuffer, index: usize) -> usize {
    (index + 1).min(buffer.character_count())
}

/// Scrolls so the visible region centres on the overview point. With
/// `requires_hit`, only a point on the overview scrolls.
fn scroll_minimap(
    node: &Node,
    state: &Rc<RefCell<FieldState>>,
    x: f32,
    y: f32,
    requires_hit: bool,
) -> bool {
    let Some(geometry) = node.attachment::<MinimapGeometry>(MINIMAP_GEOMETRY) else {
        return false;
    };
    if requires_hit && !geometry.contains(x, y) {
        return false;
    }
    let mut state = state.borrow_mut();
    state.scroll.offset_y = geometry.offset_at(y, state.scroll.visible_height, state.scroll.max_y);
    state.scroll.needs_caret_reveal = false;
    state.needs_caret_reveal = false;
    drop(state);
    if let Some(session) = node.first_resource::<TextCompletionSession>() {
        session.dismiss();
    }
    node.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
        "textFieldMinimap",
    ));
    true
}

/// Paints the overview over the trailing reservation and records where it went
/// for pointer hits.
fn draw_minimap(
    node: &Node,
    field: &TextField,
    state: &FieldState,
    document: &TextDocumentLayout,
    buffer: &TextBuffer,
    list: &mut cove_core::DrawList,
    origin: Point,
    line_height: f32,
    text_color: Color,
    theme: &Theme,
) {
    let reservation = field.minimap_reservation();
    if reservation <= 0.0 {
        node.remove_attachment(MINIMAP_GEOMETRY);
        return;
    }
    let frame = node.frame();
    let rect = Rect::new(
        origin.x + frame.width - reservation,
        origin.y,
        reservation,
        frame.height,
    );
    let rows = document.index().row_count();
    let geometry = MinimapGeometry::new(
        rect,
        rows,
        state.scroll.offset_y,
        state.scroll.max_y,
        line_height,
    );
    let absolute = node.absolute_frame();
    let mut hit = geometry;
    hit.rect.x = absolute.x + frame.width - reservation;
    hit.rect.y = absolute.y;
    node.set_attachment(MINIMAP_GEOMETRY, hit);

    list.push_clip_rect(rect);
    let opacity = node.opacity();
    let tab_width = field.code_editing.indentation_width.unwrap_or(4);
    let columns = geometry.columns();
    let syntax = field.code_editing.syntax_color_at_utf8_offset.as_ref();
    for row in geometry.visible_rows(rows) {
        let (line, subrow) = document.index().location_at_row(row);
        if subrow != 0 {
            continue;
        }
        let characters = buffer.line_range(line);
        let start = buffer.utf8_offset_for_character_index(characters.start);
        let end = buffer
            .utf8_offset_for_character_index(characters.end)
            .min(start + columns);
        if end <= start {
            continue;
        }
        let bytes = buffer.utf8_bytes(start..end);
        for (run, offset) in ink_runs(&bytes, tab_width, columns) {
            let color = syntax
                .and_then(|lookup| lookup(buffer, start + offset))
                .unwrap_or(text_color);
            let _ = list.add_rect(geometry.strip(row, run), strip_color(color, opacity));
        }
    }
    let _ = list.add_rect(
        geometry.visible_band(state.scroll.offset_y, state.scroll.visible_height),
        theme.colors.on_surface.multiplied_alpha(opacity * 0.08),
    );
    list.pop_clip_rect();
}

/// Advance width of one space in `font`, used to align indent guides.
fn space_width_for(font: &cove_core::Font, environment: &TextEnvironment) -> f32 {
    let shaped = environment.shape(" ", Some(font));
    shaped
        .iter()
        .map(|glyph| glyph.x_advance)
        .sum::<f32>()
        .max(1.0)
}

fn render(
    node: &Node,
    field: &TextField,
    state_handle: &Rc<RefCell<FieldState>>,
    list: &mut cove_core::DrawList,
    origin: Point,
) {
    let theme = theme_for_node(node);
    let frame = node.frame();
    if frame.width <= 0.0 || frame.height <= 0.0 {
        return;
    }
    // The style paints the chrome; the surface paints text, selection, caret.

    let Some(environment) = TextEnvironmentHolder::current() else {
        return;
    };
    let token = resolved_text_style(node, &theme);
    let line_height = token.line_height;
    let font = token.font.clone();
    let mut state = state_handle.borrow_mut();
    state.last_draw_origin = origin;
    let current = field.text.get();
    state.normalize(&current);
    let addons = field.addon_layouts(
        node,
        &theme,
        &environment,
        &font,
        line_height,
        token.letter_spacing,
        current.character_count(),
        current.line_count(),
    );
    let inset = field.horizontal_inset(&theme);
    let metrics = field.inputs(&theme);

    // Split borrows: `make_render_state` needs the draft mutably while the
    // selection and composition are read from the same struct.
    let selection = state.selection;
    let composition = state.composition.clone();
    let is_focused = state.is_focused;
    let render_state = make_render_state(
        &metrics,
        &mut state.draft,
        &current,
        &selection,
        &composition,
        &field.placeholder,
        is_focused,
    );
    let available_width = field.available_text_width(&theme, frame.width, &addons);
    let is_vertical = field.layout.axis == TextFieldAxis::Vertical;
    let geometry = document_geometry(
        &metrics,
        font.clone(),
        line_height,
        token.letter_spacing,
        environment.atlas_texture_id as u64,
        if is_vertical {
            available_width
        } else {
            f32::INFINITY
        },
        render_state.shows_placeholder,
    );
    let mut document = TextDocumentLayout::new(render_state.buffer.clone(), geometry.clone());
    if let Some(gutter) = node.first_resource::<crate::primitives::text::code::TextGutterSession>()
    {
        document.set_hidden_lines(gutter.folds().hidden_lines());
    }
    let vertical_inset = field.vertical_inset(line_height);
    state.scroll.visible_height = (frame.height - vertical_inset * 2.0).max(line_height);
    update_scroll(&mut state.scroll, &document, line_height);
    // Reveal the caret before windowing so the painted rows match the scroll a
    // fresh caret position asks for.
    let mut reveal = None;
    if state.scroll.needs_caret_reveal {
        let (caret_x, caret_row) = document.caret(
            render_state.cursor_index,
            !render_state.is_composing && selection.line_end_affinity,
            &environment,
        );
        let top = caret_row as f32 * line_height;
        if top < state.scroll.offset_y {
            state.scroll.offset_y = top;
        }
        if top + line_height > state.scroll.offset_y + state.scroll.visible_height {
            state.scroll.offset_y = (top + line_height - state.scroll.visible_height).max(0.0);
        }
        state.scroll.offset_y = state.scroll.offset_y.clamp(0.0, state.scroll.max_y);
        reveal = Some(caret_x);
    }
    let first_row =
        ((state.scroll.offset_y / line_height.max(1.0)).floor() as usize).saturating_sub(1);
    let row_count = ((state.scroll.visible_height / line_height.max(1.0)).ceil() as usize) + 3;
    let layout = document.visible_layout(first_row, row_count, &environment);
    state.scroll.horizontal.synchronize(
        &render_state.buffer,
        &geometry,
        available_width,
        layout.total_width,
        reveal,
    );
    state.scroll.needs_caret_reveal = false;
    // Mirror the viewport so hit testing, accessibility and tests observe the
    // same scroll the glyphs were painted with.
    node.set_content_offset(Point::new(
        state.scroll.horizontal.offset,
        state.scroll.offset_y,
    ));
    let scrollable = state.scroll.max_y > 0.0 || state.scroll.horizontal.maximum() > 0.0;
    if is_focused && scrollable {
        node.set_attachment(
            WheelRoutingAttachmentKey::PRIORITY,
            WheelRoutingPriority::PreferFocused,
        );
    } else {
        node.remove_attachment(WheelRoutingAttachmentKey::PRIORITY);
    }

    let text_origin = Point::new(
        origin.x + inset + addons.leading() - state.scroll.horizontal.offset,
        origin.y + vertical_inset - state.scroll.offset_y,
    );
    let text_color = field
        .decoration
        .text_color
        .or_else(|| node.inherited_foreground_color())
        .unwrap_or(theme.colors.on_surface);
    let placeholder_color = field
        .decoration
        .placeholder_color
        .unwrap_or(theme.colors.on_surface_muted);
    let clip_left = origin.x + inset + addons.leading();
    let text_column_right = clip_left + available_width;
    list.push_clip_rect(Rect::new(
        clip_left,
        origin.y,
        available_width,
        frame.height,
    ));

    // Line numbers: the gutter keeps its column while the horizontal scroll
    // moves the document underneath.
    if is_vertical && field.code_editing.shows_line_numbers && addons.metrics.gutter > 0.0 {
        if let Some(background) = field.code_editing.line_number_gutter_color {
            let _ = list.add_rect(
                Rect::new(
                    origin.x + inset,
                    origin.y,
                    addons.metrics.gutter,
                    frame.height,
                ),
                background,
            );
        }
        let number_color = field
            .code_editing
            .line_number_color
            .unwrap_or(theme.colors.on_surface_muted);
        let first = ((state.scroll.offset_y / line_height.max(1.0)).floor() as usize).max(1) - 1;
        let visible_rows =
            ((state.scroll.visible_height / line_height.max(1.0)).ceil() as usize) + 1;
        for offset in 0..visible_rows {
            let row = first + offset;
            if row >= document.index().row_count() {
                break;
            }
            let (line, _) = document.index().location_at_row(row);
            let label = (line + 1).to_string();
            let number = environment.cached_layout(
                &label,
                Some(&font),
                Some(line_height),
                f32::INFINITY,
                TextAlignment::Trailing,
                token.letter_spacing,
            );
            let right = origin.x
                + inset
                + addons
                    .gutter_geometry
                    .map_or(addons.metrics.gutter - GUTTER_GAP, |geometry| {
                        geometry.number_right
                    });
            let y = origin.y + vertical_inset + row as f32 * line_height - state.scroll.offset_y;
            let _ = list.add_text(
                &number,
                Point::new(right - number.total_width, y),
                number_color,
                environment.atlas_texture_id,
                Some(&environment.atlas),
                None,
            );
        }
    }

    // Indent guides sit under the glyphs so they never dim text.
    if is_vertical && field.code_editing.indentation_width.is_some() {
        let segments = TextIndentGuides::segments(
            &layout,
            &render_state.buffer,
            field.code_editing.indentation_width.unwrap_or(4),
            space_width_for(&font, &environment),
            line_height,
            (
                state.scroll.horizontal.offset,
                state.scroll.horizontal.offset + available_width,
            ),
        );
        for segment in segments {
            let _ = list.add_rect(
                Rect::new(
                    text_origin.x + segment.x,
                    text_origin.y + segment.top,
                    1.0,
                    segment.height,
                ),
                theme.colors.divider.multiplied_alpha(0.6),
            );
        }
    }

    // Selection band under the glyphs.
    if let Some((low, high)) = render_state
        .composition_range
        .or_else(|| state.selection_range())
    {
        let color = field
            .decoration
            .selection_color
            .unwrap_or(theme.colors.selection);
        draw_range(
            list,
            &render_state.buffer,
            &layout,
            text_origin,
            line_height,
            low,
            high,
            color.multiplied_alpha(node.opacity()),
        );
    }

    // Inline prefix sits in the text column; the glyphs after it start at the
    // shifted text origin so the caret and hit testing agree.
    if let Some(prefix) = &addons.prefix
        && !render_state.shows_placeholder
    {
        let _ = list.add_text(
            prefix,
            Point::new(clip_left, text_origin.y),
            theme.colors.on_surface_muted,
            environment.atlas_texture_id,
            Some(&environment.atlas),
            None,
        );
    }
    if let Some(suffix) = &addons.suffix
        && !render_state.shows_placeholder
    {
        let x = text_column_right - suffix.total_width;
        let _ = list.add_text(
            suffix,
            Point::new(x, text_origin.y),
            theme.colors.on_surface_muted,
            environment.atlas_texture_id,
            Some(&environment.atlas),
            None,
        );
    }

    // Per-glyph colouring: a code-editing lookup may recolour part of the text
    // (keywords, literals, diagnostics) while the rest keeps the field colour.
    let syntax = field
        .code_editing
        .syntax_color_at_utf8_offset
        .as_ref()
        .filter(|_| !render_state.shows_placeholder);
    let colors = syntax.map(|lookup| {
        let buffer = &render_state.buffer;
        move |glyph: &cove_core::PositionedGlyph| lookup(buffer, glyph.cluster as usize)
    });
    if let Ok(()) = list.add_text(
        &layout,
        text_origin,
        if render_state.shows_placeholder {
            placeholder_color
        } else {
            text_color
        },
        environment.atlas_texture_id,
        Some(&environment.atlas),
        colors
            .as_ref()
            .map(|resolve| resolve as &dyn Fn(&cove_core::PositionedGlyph) -> Option<Color>),
    ) {}

    // Caret: a one-pixel bar at the shaped caret position.
    if state.is_focused
        && (state.composition.is_active() || state.selection_range().is_none())
        && (state.composition.is_active()
            || (state.last_activity.elapsed().as_millis() / 500).is_multiple_of(2))
    {
        let (x, row) = document.caret(
            render_state.cursor_index,
            !render_state.is_composing && state.selection.line_end_affinity,
            &environment,
        );
        let caret_color = field.decoration.cursor_color.unwrap_or(theme.colors.accent);
        let top = text_origin.y + (row as f32 - first_row as f32) * line_height;
        if let Some(session) = node.first_resource::<TextCompletionSession>() {
            let absolute = node.absolute_frame();
            session.update_anchor(Rect::new(
                absolute.x + (text_origin.x - origin.x) + x,
                absolute.y + (top - origin.y),
                1.0,
                line_height,
            ));
        }
        let _ = list.add_rect(
            Rect::new(text_origin.x + x, top, 1.0, line_height),
            caret_color,
        );
    }

    list.pop_clip_rect();

    // Trailing reservations, outermost first: `append` slab, word-limit
    // counter, then the clear affordance. Each one was subtracted from the
    // text column above, so painting here cannot overlap the glyphs.
    let mut edge = origin.x + frame.width - field.minimap_reservation();
    if let Some(append) = &addons.append {
        let x = edge - field.horizontal_inset(&theme) - append.total_width;
        paint_slab(
            list,
            append,
            x,
            origin.y,
            frame.height,
            &theme,
            &environment,
            placeholder_color,
        );
        edge = x - theme.spacing.sm;
    }
    if let Some(counter) = &addons.counter {
        let x = edge - field.horizontal_inset(&theme) - counter.total_width;
        let _ = list.add_text(
            counter,
            Point::new(x, origin.y + (frame.height - counter.total_height) * 0.5),
            theme.colors.on_surface_muted,
            environment.atlas_texture_id,
            Some(&environment.atlas),
            None,
        );
        edge = x - theme.spacing.xs;
    }
    if let Some(prepend) = &addons.prepend {
        let x = origin.x + field.horizontal_inset(&theme);
        paint_slab(
            list,
            prepend,
            x,
            origin.y,
            frame.height,
            &theme,
            &environment,
            placeholder_color,
        );
    }
    if let Some(resource) = &field.decoration.leading_icon {
        let image = crate::primitives::media::image::Image::resolve_resource(resource, 16.0, 16.0);
        image.retain(list);
        let _ = list.add_image_mask_quad(
            Rect::new(
                origin.x + field.horizontal_inset(&theme),
                origin.y + (frame.height - 16.0) * 0.5,
                16.0,
                16.0,
            ),
            image.texture_id(),
            placeholder_color,
            Point::default(),
            Point::new(1.0, 1.0),
        );
    }
    // Apple fields offer clearing only while they are being edited: at rest a
    // field shows its text alone. The lane stays reserved, so focusing never
    // reflows the text.
    if field.behavior.clearable
        && !field.behavior.disabled
        && !field.behavior.read_only
        && !current.is_empty()
        && state.is_focused
    {
        let start = edge - 20.0;
        // `edge` already includes the paint origin; adding it again here moved
        // the glyph and its hit region one field offset to the right, so a
        // field inset from the window edge clipped the clear affordance.
        node.set_attachment("__textfield_clear_hit", start);
        // The system clear control: a filled gray disc with the cross knocked
        // out in the field's own fill.
        let disc = Rect::new(
            start + 3.0,
            origin.y + (frame.height - 14.0) * 0.5,
            14.0,
            14.0,
        );
        let _ = list.add_rounded_rect(
            disc,
            7.0,
            theme.colors.on_surface_muted.multiplied_alpha(0.55),
        );
        let image = crate::primitives::media::image::Image::resolve_resource(
            &crate::primitives::media::ui_icon_catalog::action::x(),
            8.0,
            8.0,
        );
        image.retain(list);
        let _ = list.add_image_mask_quad(
            Rect::new(disc.x + 3.0, disc.y + 3.0, 8.0, 8.0),
            image.texture_id(),
            theme.inputs.background,
            Point::default(),
            Point::new(1.0, 1.0),
        );
    } else {
        node.remove_attachment("__textfield_clear_hit");
    }

    draw_minimap(
        node,
        field,
        &state,
        &document,
        &render_state.buffer,
        list,
        origin,
        line_height,
        text_color,
        &theme,
    );
    let shared_document = Rc::new(RefCell::new(document));
    node.set_attachment(DOCUMENT_LAYOUT_KEY, shared_document.clone());
    // The last measure pass, keyed by everything that changes its glyphs. A
    // later pass reuses it instead of reshaping, and observers can read what
    // the field actually shaped with (the authored tracking included).
    node.set_attachment(
        RENDER_CACHE_KEY,
        RenderCacheEntry {
            key: RenderCacheKey {
                root: first_row,
                visible_first_row: first_row,
                visible_row_count: row_count,
                total_rows: shared_document.borrow().index().row_count(),
                font: font.clone(),
                line_height,
                letter_spacing: token.letter_spacing,
                atlas: environment.atlas_texture_id as u64,
                available_text_width: available_width,
                secure: field.behavior.secure,
                hidden_lines: shared_document.borrow().hidden_lines().to_vec(),
            },
            layout,
            document: shared_document.clone(),
        },
    );
}

/// Paint one sourced slab (`prepend` / `append`) with its glyphs centred
/// vertically in the field.
#[allow(
    clippy::too_many_arguments,
    reason = "slab paint needs the measured run plus the field's chrome metrics"
)]
fn paint_slab(
    list: &mut cove_core::DrawList,
    layout: &cove_core::TextLayoutResult,
    x: f32,
    top: f32,
    height: f32,
    theme: &Theme,
    environment: &TextEnvironment,
    text_color: Color,
) {
    let padding = theme.spacing.xs;
    let _ = list.add_rounded_rect(
        Rect::new(
            x,
            top + 2.0,
            layout.total_width + padding * 2.0,
            (height - 4.0).max(0.0),
        ),
        theme.inputs.radius,
        theme.colors.surface_variant,
    );
    let _ = list.add_text(
        layout,
        Point::new(x + padding, top + (height - layout.total_height) * 0.5),
        text_color,
        environment.atlas_texture_id,
        Some(&environment.atlas),
        None,
    );
}

fn draw_range(
    list: &mut cove_core::DrawList,
    buffer: &TextBuffer,
    layout: &cove_core::TextLayoutResult,
    text_origin: Point,
    line_height: f32,
    low: usize,
    high: usize,
    color: Color,
) {
    let lower = buffer.utf8_offset_for_character_index(low);
    let upper = buffer.utf8_offset_for_character_index(high);
    let line_break_at = |offset: usize| {
        let character = buffer.character_index_for_utf8_offset(offset);
        buffer
            .character_at(character)
            .is_some_and(|value| value.contains('\n'))
    };
    for line in &layout.lines {
        let line_start = line.start_cluster as usize;
        let line_end = line.end_cluster as usize;
        let overlaps_text = lower < line_end && upper > line_start;
        let selects_line_break = lower <= line_end && upper > line_end && line_break_at(line_end);
        let x_low = super::text_field_layout_engine::row_x_at_byte(line, lower.max(line_start));
        let x_high = super::text_field_layout_engine::row_x_at_byte(line, upper.min(line_end));
        if overlaps_text || selects_line_break {
            crate::primitives::text::selection_input::paint_selection(
                list,
                Rect::new(
                    text_origin.x + x_low,
                    text_origin.y + line.top_y,
                    (x_high - x_low).max(1.0),
                    line_height,
                ),
                color,
            );
        }
    }
}
