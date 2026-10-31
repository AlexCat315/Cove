//! `TerminalView`: shows a `TerminalSession`, starting it if it is still idle.
//!
//! The grid fills the space the view is given; the program is told the new size
//! whenever layout changes.
//!
//! - Keyboard and IME text go to the program; Cmd-C copies the selection and
//!   Cmd-V pastes. Other Command shortcuts stay with the host.
//! - Drag selects (double-click a word, triple-click a line). Programs that ask
//!   for mouse reports receive the pointer instead; hold Shift to select.
//! - Cmd-click opens OSC 8 hyperlinks with `on_open_link`, by default the system.
//! - Kitty graphics images are drawn in place; the cursor blinks while focused.
//!
//! Default colors follow the theme's surface colors; programs can still set their
//! own palette.

use crate::engine::TerminalRGB;
use crate::session::TerminalSession;
use crate::view::{
    cell_metrics::CellMetrics,
    key_routing::{self, KeyRouting},
    painter::TerminalPainter,
    surface_state::SurfaceState,
};
use cove_composition::{PrimitiveView, impl_primitive_view};
use cove_core::{
    Color, Font, FontWeight, Rect,
    scene::{
        EventResult, FocusChainHolder, InteractionRegistryHolder, Node, PointerPhase,
        TextInputAreaResolver, TextInputAttachmentKey,
    },
};
use cove_platform::{
    KeyEvent, MouseButtonEvent, MouseMotionEvent, MouseWheelEvent, SystemCursor, TextEditingEvent,
};
use cove_text::TextEnvironmentHolder;
use std::rc::Rc;

pub type LinkOpener = Rc<dyn Fn(&str)>;

pub struct TerminalView {
    session: Rc<TerminalSession>,
    font_size: f32,
    on_open_link: Option<LinkOpener>,
}

impl TerminalView {
    pub fn new(session: Rc<TerminalSession>) -> Self {
        Self {
            session,
            font_size: 13.0,
            on_open_link: None,
        }
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = if size.is_finite() {
            size.clamp(6.0, 72.0)
        } else {
            13.0
        };
        self
    }

    pub fn on_open_link(mut self, open: LinkOpener) -> Self {
        self.on_open_link = Some(open);
        self
    }

    fn font(&self) -> Font {
        Font::monospaced(self.font_size, FontWeight::Regular)
    }
}

impl PrimitiveView for TerminalView {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_focusable(true);
        node.set_clips_to_bounds(true);
        node.set_cursor(Some(SystemCursor::Ibeam));
        let state = SurfaceState::new();
        node.add_resource(state);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        // Fill both axes of any stack: grow along it, stretch across it.
        layout.set_flex_grow(1.0);
        layout.set_flex_shrink(1.0);
        layout.set_align_self(cove_core::scene::Align::Stretch);
        // Without a sized container the terminal still shows a few rows.
        layout.set_min_width(Some(0.0));
        layout.set_min_height(Some((self.font_size * 1.3 * 4.0).ceil()));
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        let Some(state) = node.first_resource::<SurfaceState>() else {
            return;
        };
        state.bind(self.session.clone());
        if let Some(open) = self.on_open_link.clone() {
            state.set_open_link(open);
        }
        let foreground = node.inherited_foreground_color();
        let background = node.background_color();
        if foreground.is_some() || background.is_some() {
            self.session.set_default_colors(
                TerminalRGB::from_color(foreground.unwrap_or(Color::WHITE)),
                TerminalRGB::from_color(background.unwrap_or(Color::BLACK)),
            );
        }
        self.install_input(node, &state);
        self.install_text_input_area(node, &state);

        let session = self.session.clone();
        let font = self.font();
        let weak_node = node.downgrade();
        let draw_state = state.clone();
        node.update_draw(
            (
                font.size().to_bits(),
                font.weight(),
                font.design(),
                font.is_italic(),
            ),
            move |list, origin| {
                let Some(node) = Node::upgrade(&weak_node) else {
                    return;
                };
                let Some(environment) = TextEnvironmentHolder::current() else {
                    return;
                };
                let metrics = CellMetrics::new(font.clone(), &environment);
                draw_state.set_metrics(metrics.clone());
                let frame_rect = node.frame();
                let (columns, rows) = metrics.grid(frame_rect.width, frame_rect.height);
                session.fit(
                    columns,
                    rows,
                    metrics.width.round() as usize,
                    metrics.height.round() as usize,
                );
                // Spawn only after the first fitted grid exists, so programs see
                // the actual window size even when they query it immediately.
                if !session.is_running() && session.status() == crate::session::SessionStatus::Idle
                {
                    let _ = session.start();
                }
                let frame = session.frame();
                let is_focused = FocusChainHolder::current()
                    .and_then(|chain| chain.focused())
                    .is_some_and(|focused| focused == node);
                let reduced_motion = node.prefers_reduced_motion();
                draw_state
                    .blink
                    .update(is_focused && frame.cursor.is_blinking && !reduced_motion);
                draw_state.text_blink.update(
                    !reduced_motion
                        && frame.cells.iter().any(|cell| {
                            cell.attributes
                                .contains(crate::engine::TerminalCellAttributes::BLINK)
                        }),
                );
                let images = draw_state
                    .textures
                    .borrow_mut()
                    .resolve(&frame.images, |image_id| session.image_pixels(image_id))
                    .into_iter()
                    .map(|(placement, asset)| (placement, asset.texture_id))
                    .collect::<Vec<_>>();
                let painter = TerminalPainter {
                    frame: &frame,
                    metrics: &metrics,
                    environment: &environment,
                    is_focused,
                    opacity: node.opacity(),
                    preedit: draw_state.preedit(),
                    is_cursor_shown: draw_state.blink.is_visible(),
                    is_blinking_text_shown: draw_state.text_blink.is_visible(),
                    hovered_link: draw_state.hovered_link(),
                    images,
                    cell_pixels: session.engine_cell_size(),
                };
                let bounds = Rect::new(origin.x, origin.y, frame_rect.width, frame_rect.height);
                painter.paint(list, bounds);
                // Output moves the cursor without a layout pass; keep the IME
                // anchor on it.
                if let Some(area) = draw_state.text_input_area((origin.x, origin.y), &metrics) {
                    node.set_attachment(TextInputAttachmentKey::AREA, area);
                }
            },
        );
    }
}

impl TerminalView {
    fn install_text_input_area(&self, node: &Node, state: &Rc<SurfaceState>) {
        // The host enables OS text input (and places the IME window) from this
        // area, read during layout. It must not wait for a paint to exist.
        let weak = Rc::downgrade(state);
        let font = self.font();
        let resolver: TextInputAreaResolver =
            Rc::new(move |_node: &Node, origin: cove_core::Point| {
                let state = weak.upgrade()?;
                let environment = TextEnvironmentHolder::current()?;
                let metrics = state
                    .metrics()
                    .unwrap_or_else(|| CellMetrics::new(font.clone(), &environment));
                state.text_input_area((origin.x, origin.y), &metrics)
            });
        node.set_attachment(TextInputAttachmentKey::AREA_RESOLVER, resolver);
        let weak = Rc::downgrade(state);
        node.set_attachment(
            TextInputAttachmentKey::FOCUS_CHANGE_HANDLER,
            Rc::new(move |focused: bool| {
                if let Some(state) = weak.upgrade() {
                    if !focused {
                        state.set_preedit("");
                    }
                    state.blink.restart();
                    state.redraw();
                }
            }),
        );
    }

    fn install_input(&self, node: &Node, state: &Rc<SurfaceState>) {
        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };

        let weak_node = node.downgrade();
        let weak_state = Rc::downgrade(state);
        let down_session = self.session.clone();
        registry.set_pointer(
            node,
            cove_core::scene::InputHandlerRoute::control(),
            Rc::new(
                move |event: &MouseButtonEvent,
                      phase: PointerPhase,
                      _: cove_core::scene::EventPhase|
                      -> EventResult {
                    let (Some(_node), Some(state)) =
                        (Node::upgrade(&weak_node), weak_state.upgrade())
                    else {
                        return EventResult::Ignored;
                    };
                    let Some(geometry) = state.geometry() else {
                        return EventResult::Ignored;
                    };
                    let handled = match phase {
                        PointerPhase::Down => state.pointer.down(event, &down_session, &geometry),
                        PointerPhase::Up => state.pointer.up(event, &down_session, &geometry),
                    };
                    if handled {
                        if phase == PointerPhase::Down && event.modifiers.has_gui() {
                            if let Some(pointer) =
                                state.pointer.pointer(&geometry, event.x, event.y, 0)
                                && let Some(link) = state.pointer.link_at(&pointer, &down_session)
                            {
                                state.open_link(&link);
                            }
                        }
                        state.redraw();
                        EventResult::Handled
                    } else {
                        EventResult::Ignored
                    }
                },
            ),
        );

        let weak_node = node.downgrade();
        let weak_state = Rc::downgrade(state);
        let motion_session = self.session.clone();
        registry.set_motion(
            node,
            cove_core::scene::InputHandlerRoute::control(),
            Rc::new(
                move |event: &MouseMotionEvent, _: cove_core::scene::EventPhase| -> EventResult {
                    let (Some(node), Some(state)) =
                        (Node::upgrade(&weak_node), weak_state.upgrade())
                    else {
                        return EventResult::Ignored;
                    };
                    let Some(geometry) = state.geometry() else {
                        return EventResult::Ignored;
                    };
                    let handled = state.pointer.motion(event, &motion_session, &geometry);
                    if let Some(pointer) = state.pointer.pointer(&geometry, event.x, event.y, 0) {
                        let frame = motion_session.frame();
                        let marked = frame
                            .cell(
                                pointer.row.min(frame.rows.saturating_sub(1)),
                                pointer.column.min(frame.columns.saturating_sub(1)),
                            )
                            .is_hyperlink;
                        let hovered = state.pointer.hovered_link();
                        let is_link = marked || hovered.is_some();
                        state.set_hovered_link(hovered);
                        node.set_cursor(Some(if is_link {
                            SystemCursor::Pointer
                        } else if motion_session.is_mouse_tracking() {
                            SystemCursor::Arrow
                        } else {
                            SystemCursor::Ibeam
                        }));
                    }
                    if handled {
                        EventResult::Handled
                    } else {
                        EventResult::Ignored
                    }
                },
            ),
        );

        let weak_state = Rc::downgrade(state);
        let wheel_session = self.session.clone();
        registry.set_wheel(
            node,
            cove_core::scene::InputHandlerRoute::scroll(),
            Rc::new(
                move |event: &MouseWheelEvent, _: cove_core::scene::EventPhase| -> EventResult {
                    let Some(state) = weak_state.upgrade() else {
                        return EventResult::Ignored;
                    };
                    let Some(metrics) = state.metrics() else {
                        return EventResult::Ignored;
                    };
                    if state.pointer.wheel(event, &wheel_session, metrics.height) {
                        state.redraw();
                        EventResult::Handled
                    } else {
                        EventResult::Ignored
                    }
                },
            ),
        );

        let weak_state = Rc::downgrade(state);
        let key_session = self.session.clone();
        registry.set_key(
            node,
            cove_core::scene::InputHandlerRoute::text_input(),
            Rc::new(
                move |event: &KeyEvent, phase: cove_core::scene::EventPhase| -> EventResult {
                    let _ = phase;
                    {
                        // Key-up is not a separate route; the encoder only emits on down.
                        let Some(state) = weak_state.upgrade() else {
                            return EventResult::Ignored;
                        };
                        // While an input method composes, Return, Backspace and arrows
                        // edit the composition; the program must not see them.
                        if !state.preedit().is_empty() {
                            return EventResult::Handled;
                        }
                        state.blink.restart();
                        match key_routing::KeyRouting::new(event) {
                            KeyRouting::Text => EventResult::Handled,
                            KeyRouting::Host => EventResult::Ignored,
                            KeyRouting::Paste => {
                                if let Some(text) = cove_core::scene::ClipboardHolder::read()
                                    && !text.is_empty()
                                {
                                    key_session.paste(&text);
                                }
                                EventResult::Handled
                            }
                            KeyRouting::Copy => {
                                let Some(text) = key_session.selected_text() else {
                                    return EventResult::Ignored;
                                };
                                if text.is_empty() {
                                    return EventResult::Ignored;
                                }
                                cove_core::scene::ClipboardHolder::write(&text);
                                EventResult::Handled
                            }
                            KeyRouting::Encode(key) => {
                                if key_session.press(&key) {
                                    EventResult::Handled
                                } else {
                                    EventResult::Ignored
                                }
                            }
                        }
                    }
                },
            ),
        );

        let weak_state = Rc::downgrade(state);
        let text_session = self.session.clone();
        registry.set_text(
            node,
            cove_core::scene::InputHandlerRoute::text_input(),
            Rc::new(
                move |text: &str, _: cove_core::scene::EventPhase| -> EventResult {
                    let Some(state) = weak_state.upgrade() else {
                        return EventResult::Ignored;
                    };
                    state.set_preedit("");
                    state.blink.restart();
                    text_session.send(text);
                    EventResult::Handled
                },
            ),
        );

        let weak_state = Rc::downgrade(state);
        registry.set_editing(
            node,
            cove_core::scene::InputHandlerRoute::text_input(),
            Rc::new(
                move |event: &TextEditingEvent, _: cove_core::scene::EventPhase| -> EventResult {
                    let Some(state) = weak_state.upgrade() else {
                        return EventResult::Ignored;
                    };
                    state.set_preedit(event.text.clone());
                    EventResult::Handled
                },
            ),
        );
    }
}

impl_primitive_view!(TerminalView);
