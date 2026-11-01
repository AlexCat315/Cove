//! Translation of Primitives/Controls/CompletionPopover.swift.
//!
//! Nonmodal completion list. Keyboard focus stays in the editor while the
//! highlighted row and the scroll window follow Up/Down, Return, Tab and Escape.
//!
//! The request and item models already live in
//! `primitives::text::text_completion_rebase`; this module re-exports them
//! alongside the provider/resolver callback types and the view half.
use crate::Scope;
use std::ops::Range;
use std::sync::Arc;

pub use crate::primitives::text::text_completion_rebase::{
    TextCompletionEdit, TextCompletionItem, TextCompletionRequest, TextInsertFormat,
};

use crate::modifiers::layout::{FlexExt, PaddingExt};
use crate::modifiers::style::{SemanticStyleViewExt, SurfaceStyleExt};
use crate::primitives::text::Text;
use crate::theme::{SemanticColorRef, SemanticFontRef};
use crate::{
    AnyView, BoxView, PrimitiveView, Row, TupleView, VerticalAlignment, View, impl_primitive_view,
};
use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, AccessibilityState, Align,
    EventPhase, EventResult, FlexDirection, InputHandlerRoute, InteractionRegistryHolder, Justify,
    LayoutNode, Node, Wrap,
};

/// Answers with the item, its documentation filled in when there is any.
pub type TextCompletionResolver =
    Arc<dyn Fn(TextCompletionItem, Box<dyn FnOnce(TextCompletionItem)>) + Send + Sync>;

/// Asks the host for candidates for one request, then delivers them.
///
/// The reply closure is invoked once; a caller that never answers simply leaves
/// the list as it was.
pub type TextCompletionProvider = Arc<
    dyn Fn(TextCompletionRequest, Box<dyn FnOnce(Vec<TextCompletionItem>) + Send>) + Send + Sync,
>;

/// Rows shown before the list scrolls.
const VISIBLE_ROWS: usize = 10;
/// Rows of context kept above the highlight.
const CONTEXT_ROWS: usize = 5;
/// List padding.
const LIST_PADDING: f32 = 3.0;
/// Row padding.
const ROW_PADDING_HORIZONTAL: f32 = 9.0;
const ROW_PADDING_VERTICAL: f32 = 5.0;
/// Documentation line limit.
const DOCUMENTATION_LINES: usize = 8;

/// Nonmodal completion list.
pub struct CompletionPopover {
    pub items: Vec<TextCompletionItem>,
    pub selected_index: usize,
    pub on_accept: Option<Arc<dyn Fn(TextCompletionItem) + Send + Sync>>,
}

impl CompletionPopover {
    pub fn new(items: Vec<TextCompletionItem>, selected_index: usize) -> Self {
        let selected_index = clamp_index(selected_index, items.len());
        Self {
            items,
            selected_index,
            on_accept: None,
        }
    }

    pub fn on_accept(
        mut self,
        handler: impl Fn(TextCompletionItem) + Send + Sync + 'static,
    ) -> Self {
        self.on_accept = Some(Arc::new(handler));
        self
    }

    fn snapshot(&self) -> CompletionPopover {
        CompletionPopover {
            items: self.items.clone(),
            selected_index: self.selected_index,
            on_accept: self.on_accept.clone(),
        }
    }

    /// The visible window: at most ten rows, keeping five rows of context above
    /// the highlight where the ends allow.
    pub fn visible_range(&self, count: usize, selected_index: usize) -> Range<usize> {
        visible_range(count, selected_index)
    }
}

/// The first index shown for a list of `count` rows with `selected_index`
/// highlighted.
pub fn visible_range(count: usize, selected_index: usize) -> Range<usize> {
    let start = selected_index.saturating_sub(CONTEXT_ROWS);
    let start = if count > VISIBLE_ROWS {
        start.min(count - VISIBLE_ROWS)
    } else {
        0
    };
    start..(start + VISIBLE_ROWS).min(count)
}

fn clamp_index(index: usize, count: usize) -> usize {
    if count == 0 { 0 } else { index.min(count - 1) }
}

impl Clone for CompletionPopover {
    fn clone(&self) -> Self {
        self.snapshot()
    }
}

impl View for CompletionPopover {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(CompletionPanel {
            view: self.snapshot(),
        })
    }
}

/// The floating panel: rows, a position counter and documentation.
struct CompletionPanel {
    view: CompletionPopover,
}

impl PrimitiveView for CompletionPanel {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::List,
            label: "Completions".to_string(),
            ..Default::default()
        }));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let view = &self.view;
        let count = view.items.len();
        let window = visible_range(count, view.selected_index);
        let mut column: Vec<AnyView> = Vec::new();
        for index in window {
            let Some(item) = view.items.get(index).cloned() else {
                continue;
            };
            let is_selected = index == view.selected_index;
            let on_accept = view.on_accept.clone();
            column.push(AnyView::new(CompletionRow {
                item,
                is_selected,
                on_accept,
            }));
        }
        if count > VISIBLE_ROWS {
            column.push(AnyView::new(
                Text::new(format!("{} / {}", view.selected_index + 1, count))
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted())
                    .padding_edges(10.0, 4.0),
            ));
        }
        if let Some(documentation) = view
            .items
            .get(view.selected_index)
            .and_then(|item| item.documentation.clone())
            .filter(|documentation| !documentation.is_empty())
        {
            column.push(AnyView::new(
                Text::new(documentation)
                    .line_limit(Some(DOCUMENTATION_LINES))
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted())
                    .padding_edges(9.0, 6.0),
            ));
        }
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(column)),
            )
            .padding(LIST_PADDING)
            .floating_surface_metric(crate::theme::ThemeMetric::control_radius()),
        )]
    }
}
impl_primitive_view!(CompletionPanel);

/// One completion row: label, optional detail and its own press target.
struct CompletionRow {
    item: TextCompletionItem,
    is_selected: bool,
    on_accept: Option<Arc<dyn Fn(TextCompletionItem) + Send + Sync>>,
}

impl PrimitiveView for CompletionRow {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node.set_is_focusable(true);
        node.set_is_tab_stop(false);
        node.set_automatically_focus_on_pointer_down(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_align_items(Align::Stretch);
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::ListItem,
            label: self.item.label.clone(),
            state: AccessibilityState {
                is_selected: self.is_selected,
                ..Default::default()
            },
            ..Default::default()
        }));
        let accept = self.on_accept.clone();
        let item = self.item.clone();
        let activate_item = item.clone();
        node.set_accessibility_actions(AccessibilityActions {
            activate: accept.clone().map(|handler| {
                let item = activate_item.clone();
                std::rc::Rc::new(move || handler(item.clone())) as std::rc::Rc<dyn Fn()>
            }),
            ..Default::default()
        });

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        registry.set_pointer(
            node,
            InputHandlerRoute::control(),
            std::rc::Rc::new(move |event, phase, event_phase| {
                if event.button != cove_platform::MouseButton::Left
                    || phase != cove_core::scene::PointerPhase::Down
                    || event_phase == EventPhase::Capture
                {
                    return EventResult::Ignored;
                }
                if let Some(handler) = accept.as_ref() {
                    handler(item.clone());
                }
                EventResult::Handled
            }),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let mut row: Vec<AnyView> = vec![AnyView::new(
            AnyView::new(
                Text::new(self.item.label.clone())
                    .line_limit(Some(1))
                    .semantic_font(SemanticFontRef::body())
                    .semantic_foreground(SemanticColorRef::on_surface()),
            )
            .flex_with(1.0, 1.0, None),
        )];
        if let Some(detail) = self.item.detail.clone() {
            row.push(AnyView::new(
                Text::new(detail)
                    .line_limit(Some(1))
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
            ));
        }
        // Unselected rows stay transparent so the panel's material shows
        // through; an opaque row fill would tile the glass out of existence.
        vec![AnyView::new(
            Row::new(
                VerticalAlignment::Center,
                10.0,
                AnyView::new(TupleView::new(row)),
            )
            .padding_edges(ROW_PADDING_HORIZONTAL, ROW_PADDING_VERTICAL)
            .semantic_background(if self.is_selected {
                SemanticColorRef::selection()
            } else {
                SemanticColorRef::clear()
            })
            .corner_radius_metric(crate::theme::ThemeMetric::row_radius()),
        )]
    }
}
impl_primitive_view!(CompletionRow);

impl CompletionPopover {
    /// Moves the highlight with wrapping, as Up/Down do in the editor.
    pub fn step(&mut self, delta: isize) -> bool {
        let count = self.items.len();
        if count == 0 {
            return false;
        }
        let next = (self.selected_index as isize + delta).rem_euclid(count as isize) as usize;
        if next == self.selected_index {
            return false;
        }
        self.selected_index = next;
        true
    }

    /// The highlighted item, when the list has one.
    pub fn selected(&self) -> Option<&TextCompletionItem> {
        self.items.get(self.selected_index)
    }

    /// Routes one editor key to the list's behaviour.
    ///
    /// Returns whether the key was consumed. Ctrl+Space is an explicit request
    /// and is left to the caller's provider.
    pub fn handle_key(&mut self, event: &cove_platform::KeyEvent) -> bool {
        if event.scancode == crate::Scancode::SPACE && event.modifiers.has_ctrl() {
            return false;
        }
        if self.items.is_empty() {
            return false;
        }
        match event.scancode {
            crate::Scancode::ARROW_UP => {
                self.step(-1);
                true
            }
            crate::Scancode::ARROW_DOWN => {
                self.step(1);
                true
            }
            crate::Scancode::RETURN | crate::Scancode::KEYPAD_ENTER | crate::Scancode::TAB => {
                if let Some(item) = self.selected().cloned()
                    && let Some(handler) = self.on_accept.as_ref()
                {
                    handler(item);
                }
                true
            }
            crate::Scancode::ESCAPE => true,
            _ => false,
        }
    }
}

impl TextCompletionRequest {
    /// The query the provider receives: the identifier prefix under the caret.
    pub fn query(&self) -> &str {
        &self.prefix
    }

    /// Whether the caret sits after a member-access dot, which opens the list
    /// even without a prefix.
    pub fn follows_dot(&self) -> bool {
        self.caret_index > 0 && self.buffer.character_at(self.caret_index - 1) == Some('.')
    }
}
