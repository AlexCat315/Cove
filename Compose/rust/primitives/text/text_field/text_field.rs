//! Translation of Primitives/Text/TextField/TextField.swift.
//!
//! Text input field. The default horizontal axis is single-line; the vertical
//! axis accepts explicit newline insertion and grows in height to fit those
//! lines. Editing state survives recomposition in the host node.
//!
//! Reads from the text environment for shaping; without one installed the field
//! still accepts input but renders no glyphs.
use std::cell::RefCell;
use std::rc::Rc;

use crate::modifiers::interaction::CursorExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::text::text_buffer::TextBuffer;
use crate::primitives::text::text_document_layout::{TextDocumentLayout, TextLineGeometry};
use crate::theme::styles::components::{
    AnyTextFieldStyle, TextFieldStyleConfiguration, TextFieldStyleEnvironment,
};
use crate::theme::{SemanticColorRef, SemanticFontRef, Theme, ThemeReader, theme_for_node};
use crate::{AnyView, PrimitiveView, View, impl_primitive_view};
use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, AccessibilityState,
    EventPhase, EventResult, FocusChainHolder, HoverPhase, InputHandlerRoute,
    InteractionRegistryHolder, LayoutNode, MeasureMode, Node, NodeResource, Overflow,
    PointerCaptureHolder, PointerPhase, TextInputAttachmentKey, TextInputClient, TextInputSnapshot,
};
use cove_core::{Binding, Color, InputCapabilities, Point, Rect, Size, TextAlignment};
use cove_platform::{MouseButton, MouseWheelEvent, SystemCursor};
use cove_text::{DrawListText, TextEnvironment, TextEnvironmentHolder};

use super::indent_guides::TextIndentGuides;
use super::text_field_code_editing::indented_newline;
use super::text_field_layout_engine::{
    FIELD_STATE_KEY, FieldCompositionState, FieldLayoutInputs, FieldScrollState,
    FieldSelectionState, RenderDraftState, RenderState, document_geometry, make_render_state,
    selection_range, update_scroll,
};
use super::text_field_observation::{TextFieldCaretState, TextFieldHoverAnchor};
use super::text_field_options::{
    TextFieldAxis, TextFieldBehavior, TextFieldCodeEditing, TextFieldDecoration, TextFieldLayout,
    TextFieldNavigationRequest, TextFieldSize,
};

const SCROLLBAR_HOVERED: &str = "__textfield_scrollbar_hovered";
const FOCUS_REQUEST_ID: &str = "__textfield_focus_request_id";
const CARET_REQUEST_ID: &str = "__textfield_caret_request_id";
/// One line increment for a wheel notch in a multiline field.
pub const MULTILINE_WHEEL_STEP: f32 = 30.0;
pub const SCROLLBAR_TRACK_THICKNESS: f32 = 6.0;
pub const SCROLLBAR_INSET: f32 = 3.0;

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
        }
    }

    pub fn clear_composition(&mut self) {
        self.composition = FieldCompositionState::default();
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

/// Text input field.
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

    fn vertical_inset(&self, line_height: f32) -> f32 {
        let field = self.minimum_field_height();
        ((field - line_height) / 2.0).max(4.0)
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
    fn body(&self) -> AnyView {
        AnyView::new(TextFieldHost {
            field: self.snapshot(),
        })
    }
}

impl TextField {
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
}

fn border_color_for(field: &TextField, theme: &Theme, is_focused: bool, is_hovered: bool) -> Color {
    let inputs = &theme.inputs;
    if field.behavior.disabled {
        inputs.border_disabled
    } else if is_focused {
        inputs.border_focused
    } else if is_hovered {
        inputs.border_hover
    } else {
        inputs.border_color
    }
}

impl PrimitiveView for TextFieldHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(!self.field.behavior.disabled);
        node.set_is_focusable(!self.field.behavior.disabled);
        node.set_clips_to_bounds(true);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_min_height(Some(self.field.minimum_field_height()));
        layout.set_overflow(Overflow::Hidden);
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_min_height(Some(self.field.minimum_field_height()));
        layout.set_overflow(Overflow::Hidden);
    }

    fn update_node(&self, node: &Node) {
        let theme = theme_for_node(node);
        node.set_cursor(Some(if self.field.behavior.disabled {
            SystemCursor::Arrow
        } else {
            SystemCursor::Ibeam
        }));
        node.set_is_focusable(!self.field.behavior.disabled);
        node.set_is_hit_testable(!self.field.behavior.disabled);

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
        }

        let is_focused = FocusChainHolder::current()
            .and_then(|chain| chain.focused())
            .is_some_and(|focused| focused == *node);
        state_handle.borrow_mut().is_focused = is_focused;
        let is_hovered = node.attachment::<bool>(SCROLLBAR_HOVERED).unwrap_or(false);
        let border = border_color_for(&self.field, &theme, is_focused, is_hovered);
        let width = if is_focused {
            theme.inputs.focus_ring_width
        } else {
            theme.inputs.border_width
        };
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
        }
        if let (Some(request), Some(index)) = (
            self.field.navigation.caret_request_id,
            self.field.navigation.caret_request_index,
        ) && node.attachment::<u64>(CARET_REQUEST_ID) != Some(request)
        {
            node.set_attachment(CARET_REQUEST_ID, request);
            let handle = state_handle.clone();
            let mut state = handle.borrow_mut();
            state.selection.anchor = None;
            state.move_cursor(index, false, None);
        }

        install_interaction(node, &self.field, state_handle.clone());

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
                border,
                width.to_bits(),
            ),
            move |list, origin| {
                let Some(owner) = Node::upgrade(&draw_node) else {
                    return;
                };
                render(&owner, &field, &state_for_draw, list, origin, border, width);
            },
        );
    }
}
impl_primitive_view!(TextFieldHost);

fn install_text_input_client(node: &Node, field: &TextField, state: &Rc<RefCell<FieldState>>) {
    if field.behavior.disabled {
        node.remove_attachment(TextInputAttachmentKey::CLIENT);
        return;
    }
    node.set_attachment(
        TextInputAttachmentKey::CLIENT,
        crate::primitives::text::text_field::platform_input::platform_input_client(
            state.clone(),
            field.behavior.clone(),
            field.layout.axis,
        ),
    );
}

fn install_interaction(node: &Node, field: &TextField, state: Rc<RefCell<FieldState>>) {
    install_text_input_client(node, field, &state);
    let Some(registry) = InteractionRegistryHolder::current() else {
        return;
    };
    if field.behavior.disabled {
        registry.remove(node);
        return;
    }
    let hover_node = node.downgrade();
    registry.set_hover(
        node,
        Rc::new(move |phase| {
            let Some(owner) = Node::upgrade(&hover_node) else {
                return;
            };
            let hovered = phase == HoverPhase::Enter
                && owner
                    .composition_value(&crate::adaptive::SurfaceEnvironment::key())
                    .supports_hover();
            owner.set_attachment(SCROLLBAR_HOVERED, hovered);
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
    let editing_state = state.clone();
    let reported_composing = Rc::new(std::cell::Cell::new(false));
    let reported_for_editing = reported_composing.clone();
    node.set_attachment(
        TextInputAttachmentKey::EDITING_CHANGE_HANDLER,
        Rc::new(move |is_composing: bool| {
            let _ = &editing_state;
            reported_for_editing.set(is_composing);
        }) as cove_core::scene::TextInputEditingChangeHandler,
    );
    let report_node = node.downgrade();
    let report_composing = reported_composing;
    node.set_attachment(
        TextInputAttachmentKey::FOCUS_CHANGE_HANDLER,
        Rc::new(move |focused: bool| {
            let Some(owner) = Node::upgrade(&report_node) else {
                return;
            };
            if !focused && report_composing.get() {
                if let Some(handle) = owner.attachment::<Rc<RefCell<FieldState>>>(FIELD_STATE_KEY) {
                    handle.borrow_mut().clear_composition();
                }
                report_composing.set(false);
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
    let behavior = field.behavior.clone();
    registry.set_pointer(
        node,
        InputHandlerRoute::text_input(),
        Rc::new(move |event, phase, _event_phase| {
            if event.button != MouseButton::Left {
                return EventResult::Ignored;
            }
            let Some(owner) = Node::upgrade(&pointer_node) else {
                return EventResult::Ignored;
            };
            match phase {
                PointerPhase::Down => {
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
                        if let Some(handler) =
                            owner.attachment::<Rc<dyn Fn(&TextBuffer)>>("__textfield_change")
                        {
                            handler(&TextBuffer::empty());
                        }
                        return EventResult::Handled;
                    }
                    if let Some(chain) = FocusChainHolder::current() {
                        chain.focus(Some(&owner), true);
                    }
                    if let Some(capture) = PointerCaptureHolder::current() {
                        capture.acquire(&owner, true);
                    }
                    let mut state = pointer_state.borrow_mut();
                    state.is_dragging = true;
                    state.is_focused = true;
                    drop(state);
                    if let Some(handler) = owner.attachment::<Rc<dyn Fn()>>("__textfield_focus") {
                        handler();
                    }
                    EventResult::Handled
                }
                PointerPhase::Up => {
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

    let key_node = node.downgrade();
    let key_state = state;
    let key_behavior = field.behavior.clone();
    let key_vertical = field.layout.axis == TextFieldAxis::Vertical;
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
            {
                if custom(event) {
                    return EventResult::Handled;
                }
            }
            let mut state = key_state.borrow_mut();
            match event.scancode {
                crate::Scancode::ARROW_LEFT => {
                    let target = state.selection.cursor_index.saturating_sub(1);
                    state.move_cursor(target, event.modifiers.has_shift(), None);
                    EventResult::Handled
                }
                crate::Scancode::ARROW_RIGHT => {
                    let target =
                        (state.selection.cursor_index + 1).min(state.buffer.character_count());
                    state.move_cursor(target, event.modifiers.has_shift(), None);
                    EventResult::Handled
                }
                crate::Scancode::HOME => {
                    let line = state
                        .buffer
                        .line_index_for_character_index(state.selection.cursor_index);
                    let start = state.buffer.line_range(line).start;
                    state.move_cursor(start, event.modifiers.has_shift(), None);
                    EventResult::Handled
                }
                crate::Scancode::END => {
                    let line = state
                        .buffer
                        .line_index_for_character_index(state.selection.cursor_index);
                    let end = state.buffer.line_range(line).end;
                    state.move_cursor(end, event.modifiers.has_shift(), None);
                    EventResult::Handled
                }
                crate::Scancode::RETURN | crate::Scancode::KEYPAD_ENTER => {
                    if key_vertical {
                        let newline = indented_newline(
                            &state.buffer.string_value(),
                            state.selection.cursor_index,
                            indentation,
                            true,
                        );
                        insert_replacing_selection(&mut state, &newline);
                        EventResult::Handled
                    } else if let Some(submit) =
                        owner.attachment::<Rc<dyn Fn()>>("__textfield_submit")
                    {
                        drop(state);
                        submit();
                        EventResult::Handled
                    } else {
                        EventResult::Ignored
                    }
                }
                crate::Scancode::BACKSPACE | crate::Scancode::DELETE => {
                    if key_behavior.read_only {
                        return EventResult::Handled;
                    }
                    if state.selection_range().is_none() {
                        let cursor = state.selection.cursor_index;
                        if cursor > 0 {
                            let buffer = state.buffer.delete((cursor - 1)..cursor);
                            state.buffer = buffer;
                            state.selection.cursor_index = cursor - 1;
                            state.record_activity();
                        }
                    } else {
                        delete_selection(&mut state);
                    }
                    EventResult::Handled
                }
                _ => EventResult::Ignored,
            }
        }),
    );

    let wheel_node = node.downgrade();
    registry.set_wheel(
        node,
        InputHandlerRoute::text_input(),
        Rc::new(move |event: &MouseWheelEvent, _phase| {
            let Some(_owner) = Node::upgrade(&wheel_node) else {
                return EventResult::Ignored;
            };
            if !event.input.is_continuation()
                && let (Some(mouse_x), Some(mouse_y)) = (event.mouse_x, event.mouse_y)
            {
                let frame = _owner.absolute_frame();
                if mouse_x < frame.x
                    || mouse_x > frame.x + frame.width
                    || mouse_y < frame.y
                    || mouse_y > frame.y + frame.height
                {
                    return EventResult::Ignored;
                }
            }
            EventResult::Ignored
        }),
    );
}

fn insert_replacing_selection(state: &mut FieldState, incoming: &str) {
    if incoming.is_empty() {
        return;
    }
    let selection = state.selection_range();
    let cursor = selection.map_or(state.selection.cursor_index, |(low, _)| low);
    let removed = selection.map_or(0, |(low, high)| high - low);
    state.buffer = state.buffer.replace(cursor..(cursor + removed), incoming);
    state.clear_composition();
    state.selection.cursor_index =
        (cursor + incoming.chars().count()).min(state.buffer.character_count());
    state.selection.anchor = None;
    state.selection.set_preferred_caret_x(None);
    state.record_activity();
}

fn delete_selection(state: &mut FieldState) -> bool {
    let Some((low, high)) = state.selection_range() else {
        return false;
    };
    state.buffer = state.buffer.delete(low..high);
    state.selection.cursor_index = low;
    state.selection.anchor = None;
    state.record_activity();
    true
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
    border: Color,
    border_width: f32,
) {
    let theme = theme_for_node(node);
    let frame = node.frame();
    if frame.width <= 0.0 || frame.height <= 0.0 {
        return;
    }
    let radius = theme.inputs.radius;
    let background = if field.behavior.disabled {
        theme.inputs.background_disabled
    } else {
        theme.inputs.background
    };
    let _ = list.add_rounded_rect(
        Rect::new(origin.x, origin.y, frame.width, frame.height),
        radius,
        background,
    );
    if border_width > 0.0 {
        let _ = list.add_rounded_rect_stroke(
            Rect::new(origin.x, origin.y, frame.width, frame.height),
            radius,
            border_width,
            border,
        );
    }

    let Some(environment) = TextEnvironmentHolder::current() else {
        return;
    };
    let token = SemanticFontRef::body().resolve(&theme);
    let line_height = token.line_height;
    let font = token.font.clone();
    let inset = field.horizontal_inset(&theme);
    let metrics = field.inputs(&theme);
    let mut state = state_handle.borrow_mut();
    state.last_draw_origin = origin;
    let current = field.text.get();
    state.normalize(&current);

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
    let available_width = (frame.width - inset * 2.0).max(1.0);
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
    let mut document = TextDocumentLayout::new(render_state.buffer.clone(), geometry);
    let vertical_inset = field.vertical_inset(line_height);
    state.scroll.visible_height = (frame.height - vertical_inset * 2.0).max(line_height);
    update_scroll(&mut state.scroll, &document, line_height);
    let first_row =
        ((state.scroll.offset_y / line_height.max(1.0)).floor() as usize).saturating_sub(1);
    let row_count = ((state.scroll.visible_height / line_height.max(1.0)).ceil() as usize) + 3;
    let layout = document.visible_layout(first_row, row_count, &environment);

    let text_origin = Point::new(
        origin.x + inset - state.scroll.horizontal.offset,
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
            color,
        );
    }

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
        None,
    ) {}

    // Caret: a one-pixel bar at the shaped caret position.
    if state.is_focused {
        let (x, row) = document.caret(
            render_state.cursor_index,
            !render_state.is_composing && state.selection.line_end_affinity,
            &environment,
        );
        let caret_color = field.decoration.cursor_color.unwrap_or(theme.colors.accent);
        let top = text_origin.y + (row as f32 - first_row as f32) * line_height;
        let _ = list.add_rect(
            Rect::new(text_origin.x + x, top, 1.0, line_height),
            caret_color,
        );
    }

    if state.scroll.needs_caret_reveal {
        state.scroll.needs_caret_reveal = false;
    }
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
    for line in &layout.lines {
        if lower > line.end_cluster as usize || upper <= line.start_cluster as usize {
            continue;
        }
        let x_low = super::text_field_layout_engine::row_x_at_byte(
            line,
            lower.max(line.start_cluster as usize),
        );
        let x_high = super::text_field_layout_engine::row_x_at_byte(
            line,
            upper.min(line.end_cluster as usize),
        );
        let _ = list.add_rect(
            Rect::new(
                text_origin.x + x_low,
                text_origin.y + line.top_y,
                (x_high - x_low).max(1.0),
                line_height,
            ),
            color.multiplied_alpha(0.35),
        );
    }
}
