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
use crate::theme::styles::components::AnyTextFieldStyle;
use crate::theme::{SemanticFontRef, Theme, theme_for_node};
use crate::{AnyView, PrimitiveView, View, impl_primitive_view};
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, AccessibilityState, EventResult, FocusChainHolder,
    HoverPhase, InputHandlerRoute, InteractionRegistryHolder, LayoutNode, Node, NodeResource,
    Overflow, PointerCaptureHolder, PointerPhase, TextInputAttachmentKey,
};
use cove_core::{Binding, Color, Point, Rect};
use cove_platform::{MouseButton, MouseWheelEvent, SystemCursor};
use cove_text::{DrawListText, TextEnvironment, TextEnvironmentHolder};
use unicode_segmentation::UnicodeSegmentation;

use super::indent_guides::TextIndentGuides;
use super::snippet::TextSnippetSession;
use super::text_completion_session::TextCompletionSession;
use super::text_field_code_editing::indented_newline;
use super::text_field_layout_engine::{
    FIELD_STATE_KEY, FieldCompositionState, FieldLayoutInputs, FieldScrollState,
    FieldSelectionState, RenderDraftState, document_geometry, make_render_state, selection_range,
    update_scroll,
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
            last_activity: std::time::Instant::now(),
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

    fn leading_inset(&self, theme: &Theme) -> f32 {
        self.horizontal_inset(theme)
            + if self.decoration.leading_icon.is_some() {
                22.0
            } else {
                0.0
            }
    }

    fn trailing_inset(&self, theme: &Theme, has_text: bool) -> f32 {
        self.horizontal_inset(theme)
            + if self.behavior.clearable && !self.behavior.read_only && has_text {
                22.0
            } else {
                0.0
            }
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
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(TextFieldHost {
            field: self.snapshot(),
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
        let portal = PortalResource::new();
        let snippet = TextSnippetSession::new();
        node.add_resource(portal.clone());
        node.add_resource(snippet.clone());
        node.add_resource(TextCompletionSession::new(portal, snippet));
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

        if let Some(session) = node.first_resource::<TextCompletionSession>() {
            session.configure(self.field.snapshot(), state_handle.clone());
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
    if field.behavior.disabled
        || field.behavior.read_only
        || !node.is_layout_visible()
        || !FocusChainHolder::current()
            .and_then(|chain| chain.focused())
            .is_some_and(|focused| focused == *node)
    {
        return None;
    }
    let environment = TextEnvironmentHolder::current()?;
    let theme = theme_for_node(node);
    let token = SemanticFontRef::body().resolve(&theme);
    let line_height = token.line_height;
    let inset = field.leading_inset(&theme);
    let vertical = field.layout.axis == TextFieldAxis::Vertical;
    let width = (node.frame().width
        - inset
        - field.trailing_inset(&theme, !state.borrow().buffer.is_empty())
        - field.minimap_reservation())
    .max(1.0);
    let mut state = state.borrow_mut();
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
    }
    Some(cove_platform::TextInputArea {
        x: origin.x + inset,
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
    let token = SemanticFontRef::body().resolve(&theme);
    let line_height = token.line_height;
    let mut state = state.borrow_mut();
    let width = (node.frame().width
        - field.leading_inset(&theme)
        - field.trailing_inset(&theme, !state.buffer.is_empty())
        - field.minimap_reservation())
    .max(1.0);
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
    let x = point.x - origin.x - field.leading_inset(&theme) + state.scroll.horizontal.offset;
    let row = ((point.y - origin.y - field.vertical_inset(line_height) + state.scroll.offset_y)
        .max(0.0)
        / line_height.max(1.0)) as usize;
    Some(
        document
            .character(row, x, &environment)
            .min(current.character_count()),
    )
}

fn install_interaction(node: &Node, field: &TextField, state: Rc<RefCell<FieldState>>) {
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
    let focus_state = state.clone();
    let report_composing = reported_composing;
    node.set_attachment(
        TextInputAttachmentKey::FOCUS_CHANGE_HANDLER,
        Rc::new(move |focused: bool| {
            let Some(owner) = Node::upgrade(&report_node) else {
                return;
            };
            {
                let mut state = focus_state.borrow_mut();
                state.is_focused = focused;
                state.record_activity();
            }
            if !focused {
                owner.remove_attachment(TextInputAttachmentKey::AREA);
            }
            owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                "textFieldFocus",
            ));
            if !focused && let Some(session) = owner.first_resource::<TextCompletionSession>() {
                session.dismiss();
            }
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
    let pointer_field = field.snapshot();
    let clear_binding = field.text.clone();
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
            crate::primitives::text::text_field::text_field_editing::insert_replacing_selection(
                &mut text_state.borrow_mut(),
                incoming,
            );
            text_state.borrow_mut().record_activity();
            // Publish the edit back to the caller's binding. Nothing ever wrote
            // `field.text`, so `TextFieldHost::update_node` re-read the unchanged
            // binding on the next pass and normalised the internal buffer back —
            // the user's keystrokes were discarded before any commit could see
            // them. Swift assigns through the binding itself.
            let buffer = text_state.borrow().buffer.clone();
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
            let count = event.text.chars().count() as i32;
            let start = event.start.clamp(0, count) as usize;
            state.composition.text = event.text.clone();
            state.composition.start = start;
            state.composition.length =
                event.length.clamp(0, (count - start as i32).max(0)) as usize;
            drop(state);
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
    let key_state = state;
    let key_behavior = field.behavior.clone();
    let key_binding = field.text.clone();
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
            let primary = event.modifiers.has_gui() || event.modifiers.has_ctrl();
            if primary && event.scancode == crate::Scancode::V {
                if key_behavior.read_only {
                    return EventResult::Handled;
                }
                let paste_state = key_state.clone();
                let paste_binding = key_binding.clone();
                let paste_node = owner.downgrade();
                cove_core::scene::ClipboardHolder::request_read(Box::new(move |text| {
                    let (Some(text), Some(owner)) = (text, Node::upgrade(&paste_node)) else {
                        return;
                    };
                    let buffer = {
                        let mut state = paste_state.borrow_mut();
                        state.normalize(&paste_binding.get());
                        insert_replacing_selection(&mut state, &text);
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
            let edit = || {
                let mut state = key_state.borrow_mut();
                // The caller may have rewritten the text since the last pass. Editing
                // the old copy would write it back over theirs.
                state.normalize(&key_binding.get());
                match event.scancode {
                    crate::Scancode::A if primary => {
                        state.selection.anchor = Some(0);
                        state.selection.cursor_index = state.buffer.character_count();
                        state.record_activity();
                        EventResult::Handled
                    }
                    crate::Scancode::C | crate::Scancode::X if primary => {
                        if !key_behavior.secure
                            && let Some((low, high)) = state.selection_range()
                        {
                            cove_core::scene::ClipboardHolder::write(
                                &state.buffer.substring(low..high),
                            );
                            if event.scancode == crate::Scancode::X && !key_behavior.read_only {
                                delete_selection(&mut state);
                            }
                        }
                        EventResult::Handled
                    }
                    crate::Scancode::ARROW_LEFT => {
                        let target = grapheme_before(&state.buffer, state.selection.cursor_index);
                        state.move_cursor(target, event.modifiers.has_shift(), None);
                        EventResult::Handled
                    }
                    crate::Scancode::ARROW_RIGHT => {
                        let target = grapheme_after(&state.buffer, state.selection.cursor_index);
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
                        let is_primary = event.modifiers.has_gui() || event.modifiers.has_ctrl();
                        if key_vertical && !is_primary {
                            if key_behavior.read_only {
                                return EventResult::Handled;
                            }
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
                                let start = grapheme_before(&state.buffer, cursor);
                                let buffer = state.buffer.delete(start..cursor);
                                state.buffer = buffer;
                                state.selection.cursor_index = start;
                                state.record_activity();
                            }
                        } else {
                            delete_selection(&mut state);
                        }
                        EventResult::Handled
                    }
                    _ => EventResult::Ignored,
                }
            };
            let result = edit();
            if result == EventResult::Handled {
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

/// Characters the caret steps by inspect at most this far around it, which covers
/// the longest emoji sequences.
const GRAPHEME_WINDOW: usize = 64;

/// The start of the user-perceived character before `index`.
fn grapheme_before(buffer: &TextBuffer, index: usize) -> usize {
    let start = index.saturating_sub(GRAPHEME_WINDOW);
    let text = buffer.substring(start..index);
    let last = text
        .graphemes(true)
        .next_back()
        .map_or(0, |grapheme| grapheme.chars().count());
    index.saturating_sub(last.max(usize::from(index > 0)))
}

/// The end of the user-perceived character after `index`.
fn grapheme_after(buffer: &TextBuffer, index: usize) -> usize {
    let count = buffer.character_count();
    let text = buffer.substring(index..(index + GRAPHEME_WINDOW).min(count));
    let first = text
        .graphemes(true)
        .next()
        .map_or(0, |grapheme| grapheme.chars().count());
    (index + first.max(1)).min(count)
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
    let inset = field.leading_inset(&theme);
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
    let available_width = (frame.width
        - inset
        - field.trailing_inset(&theme, !current.is_empty())
        - field.minimap_reservation())
    .max(1.0);
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
    list.push_clip_rect(Rect::new(
        origin.x + inset,
        origin.y,
        available_width,
        frame.height,
    ));

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
    if state.is_focused
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

    if state.scroll.needs_caret_reveal {
        state.scroll.needs_caret_reveal = false;
    }

    list.pop_clip_rect();
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
    if field.behavior.clearable
        && !field.behavior.disabled
        && !field.behavior.read_only
        && !current.is_empty()
    {
        let start = frame.width - field.horizontal_inset(&theme) - 20.0;
        node.set_attachment("__textfield_clear_hit", node.absolute_origin().x + start);
        let image = crate::primitives::media::image::Image::resolve_resource(
            &crate::primitives::media::ui_icon_catalog::action::x(),
            14.0,
            14.0,
        );
        image.retain(list);
        let _ = list.add_image_mask_quad(
            Rect::new(
                origin.x + start + 3.0,
                origin.y + (frame.height - 14.0) * 0.5,
                14.0,
                14.0,
            ),
            image.texture_id(),
            placeholder_color,
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
