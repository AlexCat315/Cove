//! Translation of Primitives/Text/Code/GutterControls.swift.
//!
//! The interactive gutter overlay: one absolutely positioned control per line,
//! a roving focus ring over them, and a key boundary so the gutter handles F9
//! and its fold chords before the field does.
//!
//! Behaviour checklist:
//! - controls are absolutely positioned from the geometry the layout published,
//!   so targets cannot drift from what was painted
//! - the first control is the tab stop
//! - the key boundary runs on the bubble phase, leaving capture to the field
//! - a breakpoint control shows the glyph; a fold/summary control shows a
//!   chevron or the hidden-line badge; a markers control opens its actions
//! - every control carries a label and, for breakpoints, its selected state
use crate::Scope;
use std::rc::Rc;

use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{AbsolutePositionModifier, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::Button;
use crate::primitives::media::{Icon, ui_icon_catalog};
use crate::primitives::overlays::roving_focus::{RovingFocusHost, RovingItem};
use crate::primitives::text::Text;
use crate::primitives::text::code::folding_options::TextFoldingLabels;
use crate::primitives::text::code::gutter_geometry::{TextGutterControl, TextGutterControlKind};
use crate::primitives::text::code::gutter_options::{
    TextBreakpoint, TextBreakpointState, TextEditorGutter,
};
use crate::primitives::text::code::gutter_session::TextGutterSession;
use crate::theme::styles::buttons::ButtonStyleViewExt;
use crate::theme::{SemanticColorRef, SemanticFontRef};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View};
use cove_composition::ViewExt;
use cove_core::State;
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, Align, EventPhase, EventResult, FlexDirection,
    InputHandlerRoute, InteractionRegistryHolder, Justify, LayoutNode, Node, Wrap,
};
use cove_platform::KeyEvent;

/// A stable roving identity for a control id.
fn control_token(id: &String) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut hasher);
    hasher.finish()
}

/// The gutter's interactive overlay.
pub struct TextGutterControls {
    session: Rc<TextGutterSession>,
    gutter: TextEditorGutter,
    folding_labels: Rc<TextFoldingLabels>,
    is_enabled: bool,
    cursor_line: usize,
    character_count: usize,
    read_only: bool,
    selected: State<Option<String>>,
}

impl TextGutterControls {
    pub fn new(session: Rc<TextGutterSession>, gutter: TextEditorGutter) -> Self {
        Self {
            session,
            gutter,
            folding_labels: Rc::new(TextFoldingLabels::default()),
            is_enabled: true,
            cursor_line: 0,
            character_count: 0,
            read_only: false,
            selected: State::new(None),
        }
    }

    pub fn folding_labels(mut self, labels: Rc<TextFoldingLabels>) -> Self {
        self.folding_labels = labels;
        self
    }

    pub fn is_enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    /// The caret's line and the document length, so the key handler can resolve
    /// fold chords and boundary deletes.
    pub fn caret(mut self, cursor_line: usize, character_count: usize, read_only: bool) -> Self {
        self.cursor_line = cursor_line;
        self.character_count = character_count;
        self.read_only = read_only;
        self
    }

    pub fn selected(&self) -> Option<String> {
        self.selected.get()
    }

    fn breakpoint(&self, line: usize) -> Option<TextBreakpoint> {
        self.gutter
            .breakpoints
            .as_ref()
            .and_then(|binding| binding.get().into_iter().find(|point| point.line == line))
    }

    /// One control: its glyph, accessible label and selected state.
    fn control_view(&self, control: &TextGutterControl, first: Option<&str>) -> AnyView {
        let session = self.session.clone();
        let line = control.line;
        let is_tab_stop = first == Some(control.id.as_str());

        let (content, label, is_selected, selected_state): (
            AnyView,
            String,
            Option<bool>,
            Option<bool>,
        ) = match control.kind {
            TextGutterControlKind::Breakpoint => {
                let breakpoint = self.breakpoint(line);
                let label = match &breakpoint {
                    Some(_) => (self.gutter.labels.remove_breakpoint)(line),
                    None => (self.gutter.labels.add_breakpoint)(line),
                };
                let glyph = breakpoint_glyph(
                    breakpoint.as_ref(),
                    self.gutter.execution_line == Some(line),
                );
                let toggle_session = session.clone();
                (
                    AnyView::new(Button::new(glyph, move || {
                        toggle_session.toggle_breakpoint(line)
                    })),
                    label,
                    Some(breakpoint.is_some()),
                    None,
                )
            }
            TextGutterControlKind::Fold | TextGutterControlKind::Summary => {
                let collapsed = self.session.folds().is_collapsed(line);
                let label = if collapsed {
                    (self.folding_labels.expand)(line)
                } else {
                    (self.folding_labels.collapse)(line)
                };
                let glyph = if control.kind == TextGutterControlKind::Summary {
                    AnyView::new(
                        AnyView::new(BoxView::new(
                            FlexDirection::Row,
                            Align::Center,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            4.0,
                            AnyView::new(
                                Text::new(format!("⋯ {}", control.count))
                                    .semantic_font(SemanticFontRef::caption())
                                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
                            ),
                        ))
                        .padding_edges(6.0, 0.0)
                        .semantic_background(SemanticColorRef::state_layer_hover())
                        .corner_radius(4.0),
                    )
                } else {
                    AnyView::new(Icon::with_semantic(
                        if collapsed {
                            ui_icon_catalog::navigation::chevron_right()
                        } else {
                            ui_icon_catalog::navigation::chevron_down()
                        },
                        16.0,
                        SemanticColorRef::on_surface_muted(),
                    ))
                };
                let toggle_session = session.clone();
                (
                    AnyView::new(Button::new(glyph, move || {
                        toggle_session.toggle_fold(line);
                    })),
                    label,
                    None,
                    Some(!collapsed),
                )
            }
            TextGutterControlKind::Markers => {
                let markers = self
                    .session
                    .markers_by_line()
                    .get(&line)
                    .cloned()
                    .unwrap_or_default();
                if let [marker] = markers.as_slice() {
                    let activate_session = session.clone();
                    let id = marker.id.clone();
                    let glyph = AnyView::new(Icon::new(marker.icon.clone(), 20.0));
                    let label = marker.label.clone();
                    (
                        AnyView::new(Button::new(glyph, move || {
                            activate_session.activate_marker(&id);
                        })),
                        label,
                        None,
                        None,
                    )
                } else {
                    (
                        AnyView::new(
                            Text::new(markers.len().to_string())
                                .semantic_font(SemanticFontRef::caption())
                                .semantic_foreground(SemanticColorRef::accent()),
                        ),
                        (self.gutter.labels.line_actions)(line),
                        None,
                        None,
                    )
                }
            }
            TextGutterControlKind::Combined => (
                AnyView::new(Icon::with_semantic(
                    ui_icon_catalog::action::more_horizontal(),
                    16.0,
                    SemanticColorRef::on_surface_muted(),
                )),
                (self.gutter.labels.line_actions)(line),
                None,
                None,
            ),
        };

        let role = AccessibilityRole::Button;
        AnyView::new(
            AnyView::new(
                AnyView::new(content)
                    .button_style(crate::theme::styles::buttons::plain())
                    .frame(
                        Some(crate::modifiers::layout::FrameDimension::Points(
                            control.frame.width,
                        )),
                        Some(crate::modifiers::layout::FrameDimension::Points(
                            control.frame.height,
                        )),
                        None,
                        None,
                        None,
                        None,
                    ),
            )
            .accessibility(move |semantics: &mut AccessibilitySemantics| {
                semantics.role = role;
                semantics.label = label.clone();
                if let Some(selected) = is_selected {
                    semantics.state.is_selected = selected;
                }
                if let Some(expanded) = selected_state {
                    semantics.state.is_expanded = Some(expanded);
                }
            })
            .modifier(RovingItem::new(
                control.id.clone(),
                is_tab_stop,
                Rc::new(control_token),
            ))
            .modifier(AbsolutePositionModifier::new(
                Some(control.frame.x),
                Some(control.frame.y),
                None,
                None,
            ))
            .modifier(cove_composition::LayoutDebugModifier::debug_name(&format!(
                "editor-gutter-{}",
                control.id
            ))),
        )
    }
}

impl View for TextGutterControls {
    fn body(&self, _cx: &Scope) -> AnyView {
        let controls = self.session.controls();
        let first = controls.first().map(|control| control.id.clone());
        let ids: Vec<String> = controls.iter().map(|control| control.id.clone()).collect();
        let layers: Vec<AnyView> = controls
            .iter()
            .map(|control| self.control_view(control, first.as_deref()))
            .collect();

        let selected = self.selected.clone();
        let selected_for_select = self.selected.clone();
        let host = RovingFocusHost::new(
            ids,
            move || selected.get(),
            move |id: String| selected_for_select.set(Some(id)),
            control_token,
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(layers)),
            )),
        );

        AnyView::new(
            AnyView::new(GutterKeyBoundary {
                session: self.session.clone(),
                cursor_line: self.cursor_line,
                character_count: self.character_count,
                read_only: self.read_only,
                is_enabled: self.is_enabled,
                content: AnyView::new(host),
            })
            .frame(
                Some(crate::modifiers::layout::FrameDimension::Percent(100.0)),
                Some(crate::modifiers::layout::FrameDimension::Percent(100.0)),
                None,
                None,
                None,
                None,
            ),
        )
    }

    fn dynamic_properties(&self) -> Vec<crate::DynamicField> {
        crate::dynamic_fields!(self; selected)
    }
}

/// A breakpoint dot: filled while enabled, hollow while disabled, and an arrow
/// on the execution line when no breakpoint is set there.
fn breakpoint_glyph(breakpoint: Option<&TextBreakpoint>, is_execution_line: bool) -> AnyView {
    if breakpoint.is_none() && is_execution_line {
        return AnyView::new(Icon::with_semantic(
            ui_icon_catalog::navigation::arrow_right(),
            20.0,
            SemanticColorRef::warning(),
        ));
    }
    let dot = AnyView::new(
        AnyView::new(crate::EmptyView)
            .frame_size(10.0, 10.0)
            .corner_radius(5.0),
    );
    let dot = match breakpoint {
        Some(point) if point.state == TextBreakpointState::Enabled => {
            AnyView::new(dot.semantic_background(SemanticColorRef::error()))
        }
        // A disabled breakpoint keeps a hollow ring in the muted tone; the
        // semantic border helper resolves it against the node's own theme.
        Some(_) => {
            AnyView::new(dot.border_color(cove_core::Color::from_rgba8(120, 128, 140, 255), 1.5))
        }
        None => dot,
    };
    AnyView::new(
        AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::Center,
            Wrap::NoWrap,
            0.0,
            dot,
        ))
        .frame(None, None, Some(0.0), Some(0.0), None, None),
    )
}

/// Key boundary so the gutter sees F9 and fold chords on the bubble phase.
struct GutterKeyBoundary {
    session: Rc<TextGutterSession>,
    cursor_line: usize,
    character_count: usize,
    read_only: bool,
    is_enabled: bool,
    content: AnyView,
}

impl PrimitiveView for GutterKeyBoundary {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_node(&self, node: &Node) {
        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        let session = self.session.clone();
        let cursor_line = self.cursor_line;
        let character_count = self.character_count;
        let read_only = self.read_only;
        let is_enabled = self.is_enabled;
        registry.set_key(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event: &KeyEvent, phase: EventPhase| {
                if !is_enabled || phase != EventPhase::Bubble {
                    return EventResult::Ignored;
                }
                if session.handle_key(event, cursor_line, character_count, read_only) {
                    EventResult::Handled
                } else {
                    EventResult::Ignored
                }
            }),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}

impl View for GutterKeyBoundary {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
