//! Translation of Primitives/Text/Markdown/CodeBlock.swift.
//!
//! A fenced code block: authored lines in the mono face that scroll sideways
//! rather than wrap, under a header with the language and a copy action.
use crate::Scope;
use std::cell::Cell;
use std::rc::Rc;

use crate::modifiers::layout::{FlexExt, PaddingExt};
use crate::modifiers::style::SemanticStyleViewExt;
use crate::primitives::controls::button::{Button, ButtonIconSource, ButtonRole};
use crate::primitives::layout::{ScrollAxis, ScrollView, ScrollbarGutter};
use crate::primitives::text::Text;
use crate::theme::styles::buttons::{ButtonStyleViewExt, GhostButtonStyle};
use crate::theme::{SemanticColorRef, SemanticFontRef, theme_for_node};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View, impl_primitive_view};
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, Align, FlexDirection, Justify, Node, NodeResource,
    Wrap,
};

/// Copy confirmation hold, matching the Swift reset delay.
const COPIED_HOLD_SECONDS: f64 = 1.6;

/// A fenced code block with its language header and copy action.
pub struct MarkdownCodeBlock {
    pub language: Option<String>,
    pub text: String,
    pub copied: cove_core::State<bool>,
}

impl MarkdownCodeBlock {
    pub fn new(language: Option<String>, text: impl Into<String>) -> Self {
        Self {
            language,
            text: text.into(),
            copied: cove_core::State::new(false),
        }
    }
}

impl View for MarkdownCodeBlock {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(MarkdownCodeBlockBody {
            language: self.language.clone(),
            text: self.text.clone(),
            copied: self.copied.clone(),
        })
    }

    fn dynamic_properties(&self) -> Vec<crate::DynamicField> {
        crate::dynamic_fields!(self; copied)
    }
}

struct MarkdownCodeBlockBody {
    language: Option<String>,
    text: String,
    copied: cove_core::State<bool>,
}

impl PrimitiveView for MarkdownCodeBlockBody {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let _theme = theme_for_node(node);
        let is_copied = self.copied.get();
        let language_label = self.language.clone().unwrap_or_else(|| "Code".to_string());
        let tooltip = if is_copied { "Copied" } else { "Copy code" };
        let glyph = if is_copied {
            crate::primitives::media::ui_icon_catalog::action::check()
        } else {
            crate::primitives::media::ui_icon_catalog::action::copy()
        };
        let copy_text = self.text.clone();
        let copied = self.copied.clone();
        let copy_button = Button::icon_with_options(
            ButtonIconSource::resource(glyph),
            Some(14.0),
            None,
            ButtonRole::Normal,
            true,
            false,
            false,
            Some(tooltip.to_string()),
            move || {
                cove_core::scene::ClipboardHolder::write(&copy_text);
                copied.set(true);
            },
        )
        .button_style(GhostButtonStyle);
        let copy_button = AnyView::new(copy_button);

        let header = AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                8.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(
                        Text::new(language_label.clone())
                            .line_limit(Some(1))
                            .semantic_font(SemanticFontRef::caption())
                            .semantic_foreground(SemanticColorRef::on_surface_muted())
                            .flex_with(1.0, 1.0, Some(0.0)),
                    ),
                    AnyView::new(crate::theme::control_size_environment(
                        AnyView::new(copy_button),
                        crate::theme::ControlSize::Small,
                    )),
                    AnyView::new(DelayedReset {
                        after: COPIED_HOLD_SECONDS,
                        is_active: is_copied,
                        on_reset: {
                            let copied = self.copied.clone();
                            Rc::new(move || copied.set(false))
                        },
                    }),
                ])),
            )
            .padding_insets(crate::modifiers::layout::EdgeInsets::new(
                4.0, 12.0, 4.0, 4.0,
            )),
        );

        let scroll = ScrollView::new(
            ScrollAxis::Horizontal,
            AnyView::new(
                Text::new(self.text.clone())
                    .semantic_font(SemanticFontRef::mono())
                    .padding_edges(14.0, 12.0),
            ),
        )
        .scrollbar_gutter(ScrollbarGutter::Overlay)
        .accessibility_label(match self.language.clone() {
            Some(language) => format!("{language} code"),
            None => "Code".to_string(),
        });
        let code = AnyView::new(scroll);

        vec![AnyView::new(CodeBlockCard {
            content: AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(vec![
                    header,
                    AnyView::new(crate::primitives::text::Divider::new(
                        None,
                        1.0,
                        crate::primitives::text::DividerAxis::Horizontal,
                    )),
                    code,
                ])),
            )),
        })]
    }
}

/// The card itself: sunken fill, hairline border and group semantics.
struct CodeBlockCard {
    content: AnyView,
}

impl PrimitiveView for CodeBlockCard {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        let theme = theme_for_node(node);
        node.set_background_color(Some(theme.colors.surface_sunken));
        node.set_corner_radius(10.0);
        node.set_border_color(Some(theme.colors.border));
        node.set_border_width(1.0);
        node.set_clips_to_bounds(true);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Group,
            ..Default::default()
        }));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(CodeBlockCard);
impl_primitive_view!(MarkdownCodeBlockBody);

/// Sunken card chrome and group semantics for one code block.
#[expect(dead_code, reason = "ported from Swift but not wired in yet")]
struct CodeBlockChrome;

impl cove_composition::ViewModifier for CodeBlockChrome {
    fn apply_node(&self, node: &Node) {
        let theme = theme_for_node(node);
        node.set_background_color(Some(theme.colors.surface_sunken));
        node.set_corner_radius(10.0);
        node.set_border_color(Some(theme.colors.border));
        node.set_border_width(1.0);
        node.set_clips_to_bounds(true);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Group,
            ..Default::default()
        }));
    }
}

/// Calls `on_reset` once `after` seconds have passed while `is_active`, used for
/// short-lived confirmations such as "Copied".
pub struct DelayedReset {
    pub after: f64,
    pub is_active: bool,
    pub on_reset: Rc<dyn Fn()>,
}

impl PrimitiveView for DelayedReset {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        if self.is_active {
            let clock = DelayedResetClock::new(self.after, self.on_reset.clone());
            let lifetime: Rc<dyn NodeResource> = clock;
            node.add_resource(lifetime);
        }
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        Vec::new()
    }
}
impl_primitive_view!(DelayedReset);

/// Fires one callback after a delay, then stops.
struct DelayedResetClock {
    after: f64,
    elapsed: Cell<f64>,
    on_reset: Rc<dyn Fn()>,
    is_finished: Cell<bool>,
    scheduler: std::cell::RefCell<Option<cove_core::scene::AnimatorScheduler>>,
    self_ref: std::cell::RefCell<std::rc::Weak<DelayedResetClock>>,
}

impl DelayedResetClock {
    fn new(after: f64, on_reset: Rc<dyn Fn()>) -> Rc<Self> {
        let clock = Rc::new(Self {
            after: if after.is_finite() {
                after.max(0.0)
            } else {
                COPIED_HOLD_SECONDS
            },
            elapsed: Cell::new(0.0),
            on_reset,
            is_finished: Cell::new(false),
            scheduler: std::cell::RefCell::new(None),
            self_ref: std::cell::RefCell::new(std::rc::Weak::new()),
        });
        *clock.self_ref.borrow_mut() = Rc::downgrade(&clock);
        clock
    }
}

impl cove_core::scene::AnyAnimationController for DelayedResetClock {
    fn is_finished(&self) -> bool {
        self.is_finished.get()
    }

    fn tick(&self, delta_time: f64) {
        if self.is_finished.get() || !delta_time.is_finite() || delta_time < 0.0 {
            return;
        }
        self.elapsed.set(self.elapsed.get() + delta_time);
        if self.elapsed.get() < self.after {
            return;
        }
        self.is_finished.set(true);
        (self.on_reset)();
    }

    fn finish_immediately(&self) {
        self.is_finished.set(true);
    }

    fn cancel(&self) {
        self.is_finished.set(true);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl NodeResource for DelayedResetClock {
    fn mount(&self, _node: &Node) {
        *self.scheduler.borrow_mut() = Some(cove_core::scene::AnimatorScheduler::current());
        let any: Rc<dyn cove_core::scene::AnyAnimationController> =
            match self.self_ref.borrow().upgrade() {
                Some(clock) => clock,
                None => Rc::new(FinishedClock),
            };
        if let Some(scheduler) = self.scheduler.borrow().as_ref() {
            scheduler.register(any);
        }
    }

    fn unmount(&self, _node: &Node) {
        self.is_finished.set(true);
        *self.scheduler.borrow_mut() = None;
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

/// Placeholder controller used when the clock is already gone.
struct FinishedClock;

impl cove_core::scene::AnyAnimationController for FinishedClock {
    fn is_finished(&self) -> bool {
        true
    }
    fn tick(&self, _delta_time: f64) {}
    fn finish_immediately(&self) {}
    fn cancel(&self) {}
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
