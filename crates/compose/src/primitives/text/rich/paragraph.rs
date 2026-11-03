//! Translation of Primitives/Text/Rich/Paragraph.swift.
//!
//! Paints one paragraph — code chips, glyphs, link underlines and strike lines —
//! and resolves pointer input against the same flow.
use std::cell::RefCell;
use std::rc::Rc;

use crate::theme::theme_for_node;
use crate::{AnyView, PrimitiveView, impl_primitive_view};
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, EventResult, InputHandlerRoute,
    InteractionRegistryHolder, LayoutNode, MeasureMode, Node, PointerPhase, Rect,
};
use cove_core::{Color, Point};
use cove_platform::{MouseButton, SystemCursor};
use cove_text::{DrawListText, TextEnvironmentHolder};

use super::flow::{RichParagraph, RichTextFlow};
use super::link_anchor::{FOCUSED_LINK, RichTextLinkAnchor};
use super::text_selection::{TextSelectionChrome, TextSelectionScope};

/// How a paragraph participates in its document.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum TextRole {
    #[default]
    Paragraph,
    Heading(usize),
    /// Monospaced, unwrapped lines inside a horizontally scrolling block.
    CodeBlock,
}

/// Resolved colors for one paragraph.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RichTextPalette {
    pub text: Color,
    pub link: Color,
    pub link_hover: Color,
    pub code_background: Color,
    pub selection: Color,
    pub focus_ring: Color,
}

impl RichTextPalette {
    /// Read from the theme, honouring a foreground color applied by a modifier.
    pub fn resolve(node: &Node) -> Self {
        let theme = theme_for_node(node);
        let text = node
            .inherited_foreground_color()
            .unwrap_or(theme.colors.on_surface);
        Self {
            text,
            link: theme.colors.accent,
            link_hover: theme.colors.accent_hover,
            code_background: theme.colors.surface_variant,
            selection: theme.colors.selection,
            focus_ring: theme.colors.focus_ring,
        }
    }
}

/// What the paragraph surface paints, and the identity that keys a repaint.
#[derive(Clone, PartialEq)]
struct PaintIdentity {
    paragraph: RichParagraph,
    palette: RichTextPalette,
    focused_link: Option<usize>,
    line_limit: Option<usize>,
    truncates: bool,
}

/// Paints one paragraph and hit-tests pointers against the same flow.
pub struct RichTextSurface {
    pub paragraph: RichParagraph,
    pub role: TextRole,
    /// Link destinations, indexed by each span's `link`.
    pub destinations: Vec<String>,
    pub on_open_link: Option<Rc<dyn Fn(&str)>>,
    pub line_limit: Option<usize>,
    pub truncates: bool,
    pub is_selectable: bool,
}

impl RichTextSurface {
    pub fn new(paragraph: RichParagraph) -> Self {
        Self {
            paragraph,
            role: TextRole::Paragraph,
            destinations: Vec::new(),
            on_open_link: None,
            line_limit: None,
            truncates: false,
            is_selectable: true,
        }
    }

    fn accessibility_role(&self) -> AccessibilityRole {
        match self.role {
            TextRole::Paragraph | TextRole::CodeBlock => AccessibilityRole::StaticText,
            TextRole::Heading(_) => AccessibilityRole::Heading,
        }
    }
}

impl PrimitiveView for RichTextSurface {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        let paragraph = self.paragraph.clone();
        let weak_layout = layout.downgrade();
        let line_limit = self.line_limit;
        let truncates = self.truncates;
        layout.set_measure_func(Some(Rc::new(move |width, mode, _, _| {
            let Some(environment) = TextEnvironmentHolder::current() else {
                return cove_core::Size::default();
            };
            let limit = if mode == MeasureMode::Undefined {
                f32::INFINITY
            } else {
                width
            };
            let paragraph = LayoutNode::upgrade(&weak_layout)
                .map(|layout| styled_paragraph(&paragraph, &layout))
                .unwrap_or_else(|| paragraph.clone());
            let paragraph = super::display::visible_paragraph(
                &paragraph,
                limit,
                line_limit,
                truncates,
                &environment,
            );
            let flow = RichTextFlow::layout(&paragraph, limit, &environment);
            cove_core::Size {
                width: flow.width,
                height: flow.height,
            }
        })));
    }

    fn update_node(&self, node: &Node) {
        node.set_attachment("__text_display_options", (self.line_limit, self.truncates));
        let text = self.paragraph.text();
        node.set_accessibility(Some(AccessibilitySemantics {
            role: self.accessibility_role(),
            label: text,
            value: match self.role {
                TextRole::Heading(level) => format!("level {level}"),
                _ => String::new(),
            },
            ..Default::default()
        }));
        // Selectable paragraphs take focus on click for copy shortcuts, but are
        // not Tab stops: links are the keyboard targets inside text.
        node.set_is_focusable(false);
        node.set_is_tab_stop(false);

        install_pointer(
            node,
            &self.paragraph,
            self.destinations.clone(),
            self.on_open_link.clone(),
        );
        let scope = node
            .composition_value(&TextSelectionScope::key())
            .filter(|_| self.is_selectable && !TextSelectionChrome::is_inside_control(node));
        if let Some(scope) = scope {
            let source = self.paragraph.clone();
            let owner = node.downgrade();
            TextSelectionScope::register(
                &scope.0,
                node,
                Rc::new(move || source.text()),
                Rc::new({
                    let paragraph = self.paragraph.clone();
                    move |point| {
                        let (Some(owner), Some(environment)) =
                            (Node::upgrade(&owner), TextEnvironmentHolder::current())
                        else {
                            return 0;
                        };
                        let frame = owner.absolute_frame();
                        RichTextFlow::layout(
                            &paragraph_at_node(&paragraph, &owner),
                            frame.width,
                            &environment,
                        )
                        .offset(point.x - frame.x, point.y - frame.y)
                    }
                }),
            );
            install_selection(node, &scope.0);
            node.set_attachment("__selectable_text", true);
        } else {
            TextSelectionScope::unregister(node);
            node.remove_attachment("__selectable_text");
            node.set_cursor(Some(SystemCursor::Arrow));
            if TextSelectionChrome::is_inside_control(node) {
                if let Some(registry) = InteractionRegistryHolder::current() {
                    registry.remove(node);
                }
                node.set_is_hit_testable(false);
                node.set_cursor(None);
            }
        }

        let weak = node.downgrade();
        let paragraph = self.paragraph.clone();
        let destinations = self.destinations.clone();
        node.update_draw(
            PaintIdentity {
                paragraph: self.paragraph.clone(),
                palette: RichTextPalette::resolve(node),
                focused_link: node.attachment::<usize>(FOCUSED_LINK),
                line_limit: self.line_limit,
                truncates: self.truncates,
            },
            move |list, origin| {
                let Some(owner) = Node::upgrade(&weak) else {
                    return;
                };
                paint(&owner, &paragraph, &destinations, list, origin);
            },
        );
    }
}
impl_primitive_view!(RichTextSurface);

fn styled_paragraph(paragraph: &RichParagraph, layout: &LayoutNode) -> RichParagraph {
    let font = crate::modifiers::style::layout_text_style_font(layout);
    let line_height = crate::modifiers::style::layout_text_style_line_height(layout);
    let mut resolved = paragraph.clone();
    for span in &mut resolved.spans {
        let ratio = font
            .as_ref()
            .map_or(1.0, |font| font.size() / span.font.size().max(1.0));
        if let Some(font) = &font {
            span.font = font.clone();
        }
        span.line_height = line_height.unwrap_or(span.line_height * ratio);
    }
    resolved
}

fn paragraph_at_node(paragraph: &RichParagraph, node: &Node) -> RichParagraph {
    let resolved = node.layout_node().map_or_else(
        || paragraph.clone(),
        |layout| styled_paragraph(paragraph, &layout),
    );
    let (limit, truncates) = node
        .attachment::<(Option<usize>, bool)>("__text_display_options")
        .unwrap_or_default();
    if limit.is_none() && !truncates {
        return resolved;
    }
    TextEnvironmentHolder::current().map_or(resolved.clone(), |environment| {
        super::display::visible_paragraph(
            &resolved,
            node.frame().width,
            limit,
            truncates,
            &environment,
        )
    })
}

fn paint(
    node: &Node,
    paragraph: &RichParagraph,
    destinations: &[String],
    list: &mut cove_core::DrawList,
    origin: Point,
) {
    let Some(environment) = TextEnvironmentHolder::current() else {
        return;
    };
    let frame = node.frame();
    if frame.width <= 0.0 || frame.height <= 0.0 {
        return;
    }
    let palette = RichTextPalette::resolve(node);
    let paragraph = paragraph_at_node(paragraph, node);
    let flow = RichTextFlow::layout(&paragraph, frame.width, &environment);
    let opacity = node.opacity();
    let focused_link = node.attachment::<usize>(FOCUSED_LINK);

    // Code chips behind their spans, so the glyphs sit on top.
    for fragment in &flow.fragments {
        let Some(span) = paragraph.spans.get(fragment.span) else {
            continue;
        };
        if !span
            .traits
            .contains(crate::primitives::text::rich::run::TextTraits::CODE)
        {
            continue;
        }
        let line = flow.lines.get(fragment.line);
        let top = line.map_or(0.0, |line| line.top);
        let height = line.map_or(span.line_height, |line| line.height);
        let _ = list.add_rounded_rect(
            Rect::new(
                origin.x + fragment.min_x - 2.0,
                origin.y + top,
                (fragment.max_x - fragment.min_x) + 4.0,
                height,
            ),
            3.0,
            palette.code_background.multiplied_alpha(opacity),
        );
    }

    let scope = node.composition_value(&TextSelectionScope::key());
    let (selection, endpoints) = scope
        .as_ref()
        .map(|scope| {
            let mut scope = scope.0.borrow_mut();
            (scope.range_for(node), scope.endpoint_offsets(node))
        })
        .unwrap_or((None, (None, None)));
    if let Some(range) = selection {
        for (x, y, width, height) in flow.selection_rects(range) {
            crate::primitives::text::selection_input::paint_selection(
                list,
                Rect::new(origin.x + x, origin.y + y, width, height),
                palette.selection.multiplied_alpha(opacity),
            );
        }
    }

    // The shaped glyph run.
    let _ = list.add_text(
        &flow.glyphs,
        Point::new(origin.x, origin.y),
        palette.text.multiplied_alpha(opacity),
        environment.atlas_texture_id,
        Some(&environment.atlas),
        Some(&|glyph: &cove_core::PositionedGlyph| {
            let span_index = flow.span(glyph_cluster(&flow, glyph));
            let span = span_index.and_then(|index| paragraph.spans.get(index))?;
            if span.link.is_some() {
                Some(palette.link.multiplied_alpha(opacity))
            } else {
                None
            }
        }),
    );

    let caret = |offset, trailing| {
        let (x, y, width, height) = flow.caret_rect_with_affinity(offset, trailing);
        Rect::new(x, y, width, height)
    };
    TextSelectionChrome::draw_endpoint_bars(
        list,
        origin,
        endpoints.0.map(|offset| caret(offset, false)),
        endpoints.1.map(|offset| caret(offset, true)),
        theme_for_node(node).colors.accent.multiplied_alpha(opacity),
    );

    // Link underlines, plus a focus ring on the focused link's fragments.
    for fragment in &flow.fragments {
        let Some(span) = paragraph.spans.get(fragment.span) else {
            continue;
        };
        let Some(link) = span.link else { continue };
        let line = flow.lines.get(fragment.line);
        let bottom = line.map_or(0.0, |line| line.baseline + line.height * 0.12);
        let focused = focused_link == Some(link);
        let color = if focused {
            palette.focus_ring
        } else {
            palette.link
        };
        let _ = list.add_rect(
            Rect::new(
                origin.x + fragment.min_x,
                origin.y + bottom,
                fragment.max_x - fragment.min_x,
                if focused { 2.0 } else { 1.0 },
            ),
            color.multiplied_alpha(opacity),
        );
        let _ = destinations.get(link);
    }

    // Strike lines through struck spans.
    for fragment in &flow.fragments {
        let Some(span) = paragraph.spans.get(fragment.span) else {
            continue;
        };
        if !span
            .traits
            .contains(crate::primitives::text::rich::run::TextTraits::STRIKETHROUGH)
        {
            continue;
        }
        let line = flow.lines.get(fragment.line);
        let middle = line.map_or(0.0, |line| line.baseline - line.height * 0.28);
        let _ = list.add_rect(
            Rect::new(
                origin.x + fragment.min_x,
                origin.y + middle,
                fragment.max_x - fragment.min_x,
                1.0,
            ),
            palette.text.multiplied_alpha(opacity),
        );
    }
}

/// Character offset that a positioned glyph belongs to.
fn glyph_cluster(flow: &RichTextFlow, glyph: &cove_core::PositionedGlyph) -> usize {
    let _ = flow;
    glyph.cluster as usize
}

/// Resolve pointer presses and hovers against the shaped flow.
fn install_pointer(
    node: &Node,
    paragraph: &RichParagraph,
    destinations: Vec<String>,
    on_open_link: Option<Rc<dyn Fn(&str)>>,
) {
    let Some(registry) = InteractionRegistryHolder::current() else {
        return;
    };
    node.set_cursor(Some(SystemCursor::Ibeam));
    let pointer_paragraph = paragraph.clone();
    let pointer_destinations = destinations.clone();
    let open = on_open_link.clone();
    let weak = node.downgrade();
    registry.set_pointer(
        node,
        InputHandlerRoute::control(),
        Rc::new(move |event, phase, _event_phase| {
            if event.button != MouseButton::Left || phase != PointerPhase::Up {
                return EventResult::Ignored;
            }
            let Some(owner) = Node::upgrade(&weak) else {
                return EventResult::Ignored;
            };
            let Some(environment) = TextEnvironmentHolder::current() else {
                return EventResult::Ignored;
            };
            let frame = owner.absolute_frame();
            let resolved = paragraph_at_node(&pointer_paragraph, &owner);
            let flow = RichTextFlow::layout(&resolved, frame.width, &environment);
            let Some(fragment) = flow.fragment(event.x - frame.x, event.y - frame.y) else {
                return EventResult::Ignored;
            };
            let Some(span) = pointer_paragraph.spans.get(fragment.span) else {
                return EventResult::Ignored;
            };
            let Some(link) = span.link else {
                return EventResult::Ignored;
            };
            let Some(destination) = pointer_destinations.get(link) else {
                return EventResult::Ignored;
            };
            match open.as_ref() {
                Some(handler) => {
                    handler(destination);
                    EventResult::Handled
                }
                None => EventResult::Ignored,
            }
        }),
    );

    let hover_paragraph = paragraph.clone();
    let hover_node = node.downgrade();
    let hover_registry = InteractionRegistryHolder::current();
    if let Some(registry) = hover_registry {
        let hover_flow = Rc::new(RefCell::new(false));
        registry.set_motion(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, _phase| {
                let Some(owner) = Node::upgrade(&hover_node) else {
                    return EventResult::Ignored;
                };
                let Some(environment) = TextEnvironmentHolder::current() else {
                    return EventResult::Ignored;
                };
                let frame = owner.absolute_frame();
                let resolved = paragraph_at_node(&hover_paragraph, &owner);
                let flow = RichTextFlow::layout(&resolved, frame.width, &environment);
                let over_link = flow
                    .fragment(event.x - frame.x, event.y - frame.y)
                    .and_then(|fragment| hover_paragraph.spans.get(fragment.span))
                    .is_some_and(|span| span.link.is_some());
                if *hover_flow.borrow() != over_link {
                    *hover_flow.borrow_mut() = over_link;
                    owner.set_cursor(Some(if over_link {
                        SystemCursor::Pointer
                    } else {
                        if owner.is_focusable() {
                            SystemCursor::Ibeam
                        } else {
                            SystemCursor::Arrow
                        }
                    }));
                }
                EventResult::Ignored
            }),
        );
    }
}

/// A press may become either a link activation or a selection. The selection
/// owns capture; an actual range on release suppresses opening the link.
fn install_selection(node: &Node, scope: &super::text_selection::SharedTextSelectionScope) {
    let Some(registry) = InteractionRegistryHolder::current() else {
        return;
    };
    let links = registry.handlers(node);
    crate::primitives::text::selection_input::install(node, scope);
    let selection = registry.handlers(node);
    if let Some(pointer) = selection.pointer {
        let scope = scope.clone();
        registry.set_pointer(
            node,
            InputHandlerRoute::text_input(),
            Rc::new(move |event, phase, event_phase| {
                let selected = pointer(event, phase, event_phase);
                if phase == PointerPhase::Up
                    && !scope.borrow().has_selection()
                    && let Some(link) = &links.pointer
                {
                    return link(event, phase, event_phase);
                }
                selected
            }),
        );
    }
    if let Some(motion) = selection.motion {
        registry.set_motion(
            node,
            InputHandlerRoute::text_input(),
            Rc::new(move |event, phase| {
                if let Some(link) = &links.motion {
                    let _ = link(event, phase);
                }
                motion(event, phase)
            }),
        );
    }
}

/// Anchor nodes for each link, so links are Tab-reachable and announced.
pub fn link_anchors(
    paragraph: &RichParagraph,
    destinations: &[String],
    on_open_link: Option<Rc<dyn Fn(&str)>>,
    surface: Option<Node>,
) -> Vec<AnyView> {
    let mut anchors: Vec<AnyView> = Vec::new();
    for (index, destination) in destinations.iter().enumerate() {
        // Full visible text of this link, across its spans.
        let label: String = paragraph
            .spans
            .iter()
            .filter(|span| span.link == Some(index))
            .map(|span| span.text.as_str())
            .collect();
        if label.is_empty() {
            continue;
        }
        anchors.push(AnyView::new(RichTextLinkAnchor {
            destination: destination.clone(),
            label,
            index,
            on_open_link: on_open_link.clone(),
            surface: surface.clone(),
        }));
    }
    anchors
}

/// Link anchors are semantic overlays and never participate in text layout.
pub(crate) fn update_link_frames(node: &Node, paragraph: &RichParagraph) {
    let Some(environment) = TextEnvironmentHolder::current() else {
        return;
    };
    let paragraph = paragraph_at_node(paragraph, node);
    let flow = RichTextFlow::layout(&paragraph, node.frame().width, &environment);
    for child in node.children() {
        let Some(index) = child.attachment::<usize>("__text_link_index") else {
            continue;
        };
        let frame = flow
            .fragments
            .iter()
            .find_map(|fragment| {
                if paragraph.spans.get(fragment.span)?.link != Some(index) {
                    return None;
                }
                let line = flow.lines.get(fragment.line)?;
                Some(Rect::new(
                    fragment.min_x,
                    line.top,
                    fragment.max_x - fragment.min_x,
                    line.height,
                ))
            })
            .unwrap_or_default();
        child.set_frame(frame);
    }
}
