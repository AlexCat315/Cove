//! The shared demonstration counter.
//!
//! Swift source: `Examples/Counter/Sources/CounterExample/CounterView.swift`.
//!
//! One Compose source used by the desktop, browser and Android hosts. Each host
//! shapes text into the shared font atlas at the painting boundary, so the view
//! stores a plain [`CounterTextRun`] attachment and leaves painting to a
//! [`CounterText::painter`] supplied by the host through a composition local.

use cove_composition::Scope;
use cove_core::scene::{
    Align, CompositionLocal, EventPhase, EventResult, FlexDirection, FocusChainHolder, HoverPhase,
    InputHandlerRoute, InteractionRegistryHolder, Justify, LayoutNode, Node, PointerCaptureHolder,
    PointerPhase, Wrap,
};
use cove_core::{Color, Point, State};
use cove_platform::{MouseButton, SystemCursor};
use cove_ui_compose::{
    AnyView, BoxView, DynamicField, LayoutDebugExt, PrimitiveView, TupleView, View, ViewExt,
    impl_primitive_view,
};

use std::collections::BTreeMap;
use std::rc::Rc;
use unicode_segmentation::UnicodeSegmentation;

/// One text run the host paints with its own font service.
#[derive(Clone, Debug, PartialEq)]
pub struct CounterTextRun {
    pub text: String,
    pub size: f32,
    pub color: Color,
    pub inset: f32,
}

impl CounterTextRun {
    pub fn new(text: impl Into<String>, size: f32, color: Color, inset: f32) -> Self {
        Self {
            text: text.into(),
            size,
            color,
            inset,
        }
    }
}

/// Host-supplied text painter; the origin is the node's top-left corner.
pub type CounterTextPainter = dyn Fn(&CounterTextRun, &mut cove_core::DrawList, Point);

/// Shared attachment key and painter local for the counter's text runs.
pub struct CounterText;

impl CounterText {
    /// Attachment carrying the [`CounterTextRun`] a host should paint.
    pub const ATTACHMENT: &'static str = "CoveUI.sharedDemo.text";

    /// The painter every host provides for its own font service.
    ///
    /// `Provided` carries the host's painter; `PartialEq` compares by identity
    /// so re-providing the same painter does not invalidate descendants.
    pub fn painter() -> Rc<CompositionLocal<Option<ProvidedPainter>>> {
        thread_local! {
            static KEY: Rc<CompositionLocal<Option<ProvidedPainter>>> =
                Rc::new(CompositionLocal::constant(None));
        }
        KEY.with(Rc::clone)
    }
}

/// A painter wrapped for `CompositionLocal` comparison.
#[derive(Clone)]
pub struct ProvidedPainter(pub Rc<CounterTextPainter>);

impl ProvidedPainter {
    pub fn new(
        painter: impl Fn(&CounterTextRun, &mut cove_core::DrawList, Point) + 'static,
    ) -> Self {
        Self(Rc::new(painter))
    }

    pub fn paint(&self, run: &CounterTextRun, list: &mut cove_core::DrawList, origin: Point) {
        (self.0)(run, list, origin)
    }
}

impl PartialEq for ProvidedPainter {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for ProvidedPainter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ProvidedPainter")
    }
}

/// Counter state limits shared by the view, the checkpoint and the note editor.
pub const MAXIMUM_COUNT: i32 = 999_999;
/// Longest accepted note, counted in grapheme clusters as Swift's `String.count`.
pub const MAXIMUM_NOTE_LENGTH: usize = 2048;

/// Swift's `String.count` is the number of extended grapheme clusters.
fn character_count(value: &str) -> usize {
    value.graphemes(true).count()
}

/// Debug names resolved by DevTools `select.node` and recording replay.
pub struct CounterDebugName;
impl CounterDebugName {
    pub const ROOT: &'static str = "counter.root";
    pub const CARD: &'static str = "counter.card";
    pub const VALUE: &'static str = "counter.value";
    pub const NOTE: &'static str = "counter.note";
    pub const INCREMENT: &'static str = "counter.increment";
    pub const RESET: &'static str = "counter.reset";
    pub const THEME: &'static str = "counter.theme";
}

/// The shared counter: count, theme and a short note.
///
/// Cloning keeps the same three `State` handles, so a clone is the same
/// counter rather than a copy — the Swift value-type view and the retained
/// state behave identically.
#[derive(Clone)]
pub struct CounterView {
    pub count: State<i32>,
    pub dark: State<bool>,
    pub note: State<String>,
}

impl CounterView {
    pub fn new() -> Self {
        Self {
            count: State::new(0),
            dark: State::new(false),
            note: State::new(String::new()),
        }
    }

    /// The portable state document exchanged with a host or a recording.
    pub fn checkpoint(&self) -> BTreeMap<String, String> {
        let mut values = BTreeMap::new();
        values.insert("count".into(), self.count.get().to_string());
        values.insert("dark".into(), self.dark.get().to_string());
        values.insert("note".into(), self.note.get());
        values
    }

    /// Applies a checkpoint, or leaves the state untouched and returns `false`.
    pub fn restore(&self, values: &BTreeMap<String, String>) -> bool {
        let Some(count) = values
            .get("count")
            .and_then(|value| value.parse::<i32>().ok())
        else {
            return false;
        };
        if !(0..=MAXIMUM_COUNT).contains(&count) {
            return false;
        }
        let Some(dark) = values.get("dark") else {
            return false;
        };
        if dark != "true" && dark != "false" {
            return false;
        }
        let note = values.get("note");
        if note.map(|value| character_count(value)).unwrap_or(0) > MAXIMUM_NOTE_LENGTH {
            return false;
        }
        self.count.set(count);
        self.dark.set(dark == "true");
        if let Some(note) = note {
            self.note.set(note.clone());
        }
        true
    }

    pub fn increment(&self) {
        self.count
            .update(|count| *count = (*count + 1).min(MAXIMUM_COUNT));
    }

    pub fn reset(&self) {
        self.count.set(0);
    }

    pub fn toggle_theme(&self) {
        self.dark.update(|dark| *dark = !*dark);
    }
}

impl Default for CounterView {
    fn default() -> Self {
        Self::new()
    }
}

impl View for CounterView {
    fn body(&self, _cx: &Scope) -> AnyView {
        let dark = self.dark.clone();
        let foreground = if dark.get() {
            Color::from_rgba8(241, 245, 249, 255)
        } else {
            Color::from_rgba8(22, 35, 56, 255)
        };
        let background = if dark.get() {
            Color::from_rgba8(20, 29, 45, 255)
        } else {
            Color::from_rgba8(244, 247, 252, 255)
        };

        let count = self.count.clone();
        let note = self.note.clone();
        let theme = self.dark.clone();

        let increment = {
            let count = count.clone();
            move || count.update(|value| *value = (*value + 1).min(MAXIMUM_COUNT))
        };
        let reset = {
            let count = count.clone();
            move || count.set(0)
        };
        let toggle = {
            let theme = theme.clone();
            move || theme.update(|dark| *dark = !*dark)
        };

        // `debug_name` is a `View` extension, so each named node is wrapped
        // before it is erased into the parent's child list.
        let counter_card = AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                8.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(DemoLabel::new("Counter", 14.0, foreground, 24.0)),
                    AnyView::new(
                        DemoLabel::new(count.get().to_string(), 42.0, foreground, 50.0)
                            .debug_name(CounterDebugName::VALUE),
                    ),
                ])),
            )
            .modifier(DemoStyle::new(
                Some(126.0),
                20.0,
                Some(foreground.multiplied_alpha(0.06)),
                0.0,
            ))
            .debug_name(CounterDebugName::CARD),
        );

        let buttons = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            12.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(DemoButton::new(
                    "+ Increment",
                    CounterDebugName::INCREMENT,
                    1.0,
                    increment,
                )),
                AnyView::new(DemoButton::new(
                    "Reset",
                    CounterDebugName::RESET,
                    1.0,
                    reset,
                )),
            ])),
        ));

        let theme_label = if dark.get() {
            "Light theme"
        } else {
            "Dark theme"
        };
        let theme_button = AnyView::new(DemoButton::new(
            theme_label,
            CounterDebugName::THEME,
            0.0,
            toggle,
        ));

        let note_field = AnyView::new(DemoNote::new(note, foreground));

        let column = AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                12.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(DemoLabel::new(
                        "CoveUI · Shared Counter",
                        22.0,
                        foreground,
                        36.0,
                    )),
                    AnyView::new(DemoLabel::new(
                        "Shared Compose · ViewGraph · Yoga",
                        14.0,
                        foreground,
                        24.0,
                    )),
                    counter_card,
                    buttons,
                    theme_button,
                    note_field,
                ])),
            )
            .modifier(DemoStyle::new(None, 0.0, None, 1.0))
            .modifier(DemoInsets::new(24.0))
            .debug_name(CounterDebugName::ROOT),
        );

        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    column,
                )),
            )
            .modifier(DemoStyle::new(None, 0.0, Some(background), 1.0)),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("count", &self.count),
            DynamicField::state("dark", &self.dark),
            DynamicField::state("note", &self.note),
        ]
    }
}

/// Authored box styling: fixed height, padding, background and flex growth.
struct DemoStyle {
    height: Option<f32>,
    padding: f32,
    background: Option<Color>,
    grow: f32,
}

impl DemoStyle {
    fn new(height: Option<f32>, padding: f32, background: Option<Color>, grow: f32) -> Self {
        Self {
            height,
            padding,
            background,
            grow,
        }
    }
}

impl cove_ui_compose::ViewModifier for DemoStyle {
    fn apply_node(&self, node: &Node) {
        node.set_background_color(self.background);
        node.set_corner_radius(12.0);
        node.set_is_hit_testable(false);
    }

    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_height(self.height);
        layout.set_padding(self.padding, cove_core::scene::Edge::All);
        layout.set_flex_grow(self.grow);
        layout.set_width_percent(100.0);
    }
}

/// Pads content by the host safe area plus authored padding while the
/// background still paints behind the system bars.
struct DemoInsets {
    padding: f32,
}

impl DemoInsets {
    fn new(padding: f32) -> Self {
        Self { padding }
    }
}

impl cove_ui_compose::ViewModifier for DemoInsets {
    fn scope_apply(&self, node: &Node) -> Option<bool> {
        let environment =
            node.composition_value(&cove_ui_compose::adaptive::SurfaceEnvironment::key());
        let safe_area = environment.safe_area.normalized();
        let Some(layout) = node.layout_node() else {
            return Some(false);
        };
        layout.set_padding(
            self.padding + safe_area.top.max(0.0),
            cove_core::scene::Edge::Top,
        );
        layout.set_padding(
            self.padding + safe_area.leading.max(0.0),
            cove_core::scene::Edge::Start,
        );
        layout.set_padding(
            self.padding + safe_area.bottom.max(0.0),
            cove_core::scene::Edge::Bottom,
        );
        layout.set_padding(
            self.padding + safe_area.trailing.max(0.0),
            cove_core::scene::Edge::End,
        );
        Some(true)
    }

    fn is_scope_applying(&self) -> bool {
        true
    }
}

/// A non-interactive text run painted by the host.
struct DemoLabel {
    text: String,
    size: f32,
    color: Color,
    height: f32,
}

impl DemoLabel {
    fn new(text: impl Into<String>, size: f32, color: Color, height: f32) -> Self {
        Self {
            text: text.into(),
            size,
            color,
            height,
        }
    }
}

impl PrimitiveView for DemoLabel {
    fn make_node(&self) -> Node {
        Node::new()
    }

    fn update_node(&self, node: &Node) {
        node.set_is_hit_testable(false);
        install_text(
            &CounterTextRun::new(self.text.clone(), self.size, self.color, 0.0),
            node,
        );
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_height(Some(self.height));
    }
}

impl_primitive_view!(DemoLabel);

/// Stores the run for the host and paints it through the provided local.
fn install_text(text: &CounterTextRun, node: &Node) {
    node.set_attachment(CounterText::ATTACHMENT, text.clone());
    let painter = node.composition_value(&CounterText::painter());
    let text = text.clone();
    let weak = node.downgrade();
    let draw: cove_core::scene::DrawHook = Rc::new(move |list, origin| {
        let mut painted = text.clone();
        if let Some(node) = Node::upgrade(&weak) {
            if let Some(color) = node.inherited_foreground_color() {
                painted.color = color;
            }
            let padding = node
                .layout_node()
                .map(|layout| layout.resolved_padding())
                .unwrap_or_default();
            let offset = Point::new(origin.x + padding.left, origin.y + padding.top);
            if let Some(painter) = &painter {
                painter.paint(&painted, list, offset);
            }
        } else if let Some(painter) = &painter {
            painter.paint(&painted, list, origin);
        }
    });
    node.set_draw(Some(draw));
}

/// A pressable button with hover, pointer capture and keyboard activation.
struct DemoButton {
    title: String,
    name: &'static str,
    grow: f32,
    action: Rc<dyn Fn()>,
}

const PRESSED: &str = "__counter_pressed";

impl DemoButton {
    fn new(
        title: impl Into<String>,
        name: &'static str,
        grow: f32,
        action: impl Fn() + 'static,
    ) -> Self {
        Self {
            title: title.into(),
            name,
            grow,
            action: Rc::new(action),
        }
    }
}

impl PrimitiveView for DemoButton {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_focusable(true);
        node.set_is_hit_testable(true);
        node
    }

    fn update_node(&self, node: &Node) {
        node.set_cursor(Some(SystemCursor::Pointer));
        node.set_corner_radius(9.0);
        node.set_background_color(Some(BUTTON_BACKGROUND));
        node.set_attachment(
            cove_ui_compose::LayoutDebugAttachmentKey::DEBUG_NAME,
            self.name.to_string(),
        );
        install_text(
            &CounterTextRun::new(self.title.clone(), 16.0, Color::WHITE, 16.0),
            node,
        );

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        let press_node = node.downgrade();
        let capture_node = node.downgrade();
        let action = self.action.clone();
        registry.set_pointer(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase, delivery| {
                if delivery != EventPhase::Target || event.button != MouseButton::Left {
                    return EventResult::Ignored;
                }
                let Some(node) = Node::upgrade(&press_node) else {
                    return EventResult::Ignored;
                };
                match phase {
                    PointerPhase::Down => {
                        node.set_attachment(PRESSED, true);
                        if let Some(capture) = PointerCaptureHolder::current()
                            && let Some(target) = Node::upgrade(&capture_node)
                        {
                            capture.acquire(&target, true);
                        }
                        EventResult::Handled
                    }
                    PointerPhase::Up => {
                        let pressed = node.attachment::<bool>(PRESSED) == Some(true);
                        node.remove_attachment(PRESSED);
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.release();
                        }
                        let inside = node.absolute_frame().contains(Point::new(event.x, event.y));
                        if pressed && inside {
                            action();
                        }
                        EventResult::Handled
                    }
                }
            }),
        );

        let _key_node = node.downgrade();
        let key_action = self.action.clone();
        registry.set_key(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, delivery| {
                if delivery != EventPhase::Target
                    || event.is_repeat
                    || !matches!(event.scancode, 40 | 44)
                {
                    return EventResult::Ignored;
                }
                key_action();
                EventResult::Handled
            }),
        );

        let hover_node = node.clone();
        registry.set_hover(
            node,
            Rc::new(move |phase| {
                hover_node.set_background_color(Some(if phase == HoverPhase::Enter {
                    BUTTON_BACKGROUND_HOVER
                } else {
                    BUTTON_BACKGROUND
                }));
            }),
        );

        // A focused button draws the theme's focus ring above its own chrome.
        let focus_node = node.downgrade();
        let overlay: cove_core::scene::DrawHook = Rc::new(move |list, origin| {
            let Some(node) = Node::upgrade(&focus_node) else {
                return;
            };
            let visible = FocusChainHolder::current().is_some_and(|chain| chain.is_focus_visible());
            if !visible {
                return;
            }
            let frame = node.frame();
            let _ = list.add_rounded_rect_stroke(
                cove_core::Rect::new(
                    origin.x + frame.x,
                    origin.y + frame.y,
                    frame.width,
                    frame.height,
                ),
                9.0,
                2.0,
                FOCUS_RING,
            );
        });
        node.set_overlay_draw(Some(overlay));
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_height(Some(48.0));
        layout.set_flex_grow(self.grow);
        if self.grow > 0.0 {
            layout.set_flex_basis(0.0);
        } else {
            layout.set_flex_basis_auto();
        }
    }
}

impl_primitive_view!(DemoButton);

const BUTTON_BACKGROUND: Color = Color {
    r: 40.0 / 255.0,
    g: 130.0 / 255.0,
    b: 235.0 / 255.0,
    a: 1.0,
};
const BUTTON_BACKGROUND_HOVER: Color = Color {
    r: 29.0 / 255.0,
    g: 110.0 / 255.0,
    b: 210.0 / 255.0,
    a: 1.0,
};
const FOCUS_RING: Color = Color {
    r: 250.0 / 255.0,
    g: 180.0 / 255.0,
    b: 40.0 / 255.0,
    a: 1.0,
};

impl_primitive_view!(DemoNote);

/// Small append-at-caret-end text example, not the native full text editor.
struct DemoNote {
    value: State<String>,
    color: Color,
}

impl DemoNote {
    fn new(value: State<String>, color: Color) -> Self {
        Self { value, color }
    }
}

const PREEDIT: &str = "__counter_preedit";

impl PrimitiveView for DemoNote {
    fn make_node(&self) -> Node {
        Node::new()
    }

    fn update_node(&self, node: &Node) {
        node.set_is_hit_testable(true);
        node.set_is_focusable(true);
        node.set_cursor(Some(SystemCursor::Ibeam));
        node.set_corner_radius(9.0);
        node.set_background_color(Some(self.color.multiplied_alpha(0.06)));
        node.set_attachment(
            cove_ui_compose::LayoutDebugAttachmentKey::DEBUG_NAME,
            CounterDebugName::NOTE,
        );

        let current = self.value.get();
        let label = CounterTextRun::new(
            if current.is_empty() {
                "Type a note · Unicode / IME"
            } else {
                current.as_str()
            },
            16.0,
            self.color,
            16.0,
        );
        node.set_attachment(CounterText::ATTACHMENT, label.clone());

        let painter = node.composition_value(&CounterText::painter());
        let value = self.value.clone();
        let weak = node.downgrade();
        let draw: cove_core::scene::DrawHook = Rc::new(move |list, origin| {
            let mut text = label.clone();
            if let Some(node) = Node::upgrade(&weak)
                && let Some(preedit) = node.attachment::<String>(PREEDIT)
                && !preedit.is_empty()
            {
                text.text = format!("{}{preedit}", value.get());
            }
            let flat = value.get();
            if flat.is_empty() {
                text.text = label.text.clone();
            }
            if let Some(painter) = &painter {
                painter.paint(&text, list, origin);
            }
        });
        node.set_draw(Some(draw));

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        let text_value = self.value.clone();
        let text_node = node.downgrade();
        registry.set_text(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |text, phase| {
                if phase != EventPhase::Target {
                    return EventResult::Ignored;
                }
                let mut next = text_value.get();
                next.push_str(text);
                text_value.set(next.graphemes(true).take(MAXIMUM_NOTE_LENGTH).collect());
                if let Some(node) = Node::upgrade(&text_node) {
                    node.remove_attachment(PREEDIT);
                }
                EventResult::Handled
            }),
        );

        let editing_node = node.downgrade();
        registry.set_editing(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if phase != EventPhase::Target {
                    return EventResult::Ignored;
                }
                let Some(node) = Node::upgrade(&editing_node) else {
                    return EventResult::Ignored;
                };
                node.set_attachment(PREEDIT, event.text.clone());
                node.mark_render_dirty(cove_core::scene::InvalidationSource::style_set("preedit"));
                EventResult::Handled
            }),
        );

        let delete_value = self.value.clone();
        registry.set_key(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if phase != EventPhase::Target || event.scancode != 42 {
                    return EventResult::Ignored;
                }
                let mut current = delete_value.get();
                current.pop();
                delete_value.set(current);
                EventResult::Handled
            }),
        );
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_height(Some(48.0));
    }
}
