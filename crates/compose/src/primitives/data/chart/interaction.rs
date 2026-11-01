//! Translation of `Primitives/Data/Chart/Interaction.swift`.
//!
//! Gesture state is transient; only completed/live intervals go to bindings.

use std::cell::RefCell;
use std::rc::Rc;

use cove_core::Point;
use cove_core::scene::{
    EventPhase, EventResult, FocusChainHolder, HoverPhase, InputHandlerRoute,
    InteractionRegistryHolder, PointerCaptureHolder, PointerPhase,
};

use crate::Scancode;
use crate::primitives::data::chart::axis::ChartText;
use crate::primitives::data::chart::coordinates::ChartGeometry;
use crate::primitives::data::chart::model::{ChartOrientation, ChartPoint, ChartRange};
use crate::primitives::data::chart::plot::{ChartContent, ChartPlot, ChartSession};
use crate::primitives::data::chart::viewport::ChartWindow;

/// Drag state for brush and pan navigation. Lives on the session so a
/// recomposition does not interrupt a live gesture.
#[derive(Clone, Debug)]
pub enum ChartDrag {
    Brush { anchor: f32, window: ChartWindow },
    Pan { anchor: f32, window: ChartWindow },
}

#[derive(Default)]
pub struct ChartNavigation {
    pub drag: Option<ChartDrag>,
    pub window: Option<ChartWindow>,
}
impl ChartNavigation {
    pub fn cancel(&mut self) {
        self.drag = None;
        self.window = None;
    }
}

impl ChartPlot {
    pub fn is_cartesian(&self) -> bool {
        matches!(self.content, ChartContent::Cartesian { .. })
    }

    pub fn is_interactive_plot(&self) -> bool {
        self.input.is_enabled
            || (self.is_cartesian() && (self.viewport.range.is_some() || self.brush.is_some()))
    }

    pub fn install_input(&self, node: &cove_core::scene::Node, session: &Rc<ChartSession>) {
        let Some(registry) = InteractionRegistryHolder::current() else {
            session.navigation.borrow_mut().cancel();
            return;
        };
        if !self.is_interactive_plot() {
            session.navigation.borrow_mut().cancel();
            return;
        }

        let weak_node = node.downgrade();
        let choose = {
            let session = session.clone();
            let select = self.input.select.clone();
            move |next: Option<ChartPoint>| {
                session.current.set(next);
                select(next);
            }
        };

        // Hover: a pointer leaving clears an unpinned selection.
        {
            let session = session.clone();
            let weak_node = weak_node.clone();
            let input = self.input.clone();
            let choose = choose.clone();
            registry.set_hover(
                node,
                Rc::new(move |phase: HoverPhase| {
                    let Some(owner) = cove_core::scene::Node::upgrade(&weak_node) else {
                        return;
                    };
                    if phase != HoverPhase::Leave
                        || session.is_pinned.get()
                        || session.is_pressed.get()
                        || !crate::primitives::controls::adaptive::supports_control_hover(&owner)
                    {
                        return;
                    }
                    if input.is_enabled {
                        choose(None);
                    }
                }),
            );
        }

        // Motion: brush/pan while pressed, otherwise hit-test for selection.
        {
            let session = session.clone();
            let weak_node = weak_node.clone();
            let plot = self.clone();
            let choose = choose.clone();
            registry.set_motion(
                node,
                InputHandlerRoute::scroll(),
                Rc::new(move |event, phase| {
                    let Some(owner) = cove_core::scene::Node::upgrade(&weak_node) else {
                        return EventResult::Ignored;
                    };
                    if phase != EventPhase::Target {
                        return EventResult::Ignored;
                    }
                    if session.is_pressed.get()
                        && move_navigation(&plot, event.x, event.y, &owner, &session)
                    {
                        return EventResult::Handled;
                    }
                    if !plot.input.is_enabled {
                        return EventResult::Ignored;
                    }
                    let hovering =
                        crate::primitives::controls::adaptive::supports_control_hover(&owner);
                    if !(session.is_pressed.get() || (hovering && !session.is_pinned.get())) {
                        return EventResult::Ignored;
                    }
                    let hit = plot.hit(event.x, event.y, &owner);
                    if (hit.is_some() || !session.is_pressed.get()) && hit != session.current.get()
                    {
                        choose(hit);
                    }
                    if session.is_pressed.get() {
                        EventResult::Handled
                    } else {
                        EventResult::Ignored
                    }
                }),
            );
        }

        // Pointer: press/click selection and the start of a navigation drag.
        {
            let session = session.clone();
            let weak_node = weak_node.clone();
            let plot = self.clone();
            let choose = choose.clone();
            registry.set_pointer(
                node,
                InputHandlerRoute::control(),
                Rc::new(move |event, pointer_phase, event_phase| {
                    let Some(owner) = cove_core::scene::Node::upgrade(&weak_node) else {
                        return EventResult::Ignored;
                    };
                    if event.button != cove_platform::MouseButton::Left
                        || event_phase != EventPhase::Target
                    {
                        return EventResult::Ignored;
                    }
                    match pointer_phase {
                        PointerPhase::Down => {
                            if plot.is_cartesian()
                                && event.clicks == 2
                                && let Some(range) = &plot.viewport.range
                            {
                                range.set(None);
                                session.navigation.borrow_mut().cancel();
                                return EventResult::Handled;
                            }
                            if begin_navigation(&plot, event.x, event.y, &owner, &session) {
                                session.press(&owner);
                                return EventResult::Handled;
                            }
                            if !plot.input.is_enabled {
                                return EventResult::Ignored;
                            }
                            let hit = plot.hit(event.x, event.y, &owner);
                            let releases = session.is_pinned.get()
                                && hit.is_some()
                                && hit == session.current.get();
                            session.releases_pin.set(releases);
                            session.press(&owner);
                            choose(if releases { None } else { hit });
                        }
                        PointerPhase::Up => {
                            if !session.is_pressed.get() {
                                return EventResult::Ignored;
                            }
                            let navigated =
                                move_navigation(&plot, event.x, event.y, &owner, &session);
                            session.release(&owner);
                            session.navigation.borrow_mut().drag = None;
                            if !navigated {
                                session.is_pinned.set(
                                    !session.releases_pin.get() && session.current.get().is_some(),
                                );
                                session.releases_pin.set(false);
                            }
                        }
                    }
                    EventResult::Handled
                }),
            );
        }

        // Wheel: pan or zoom the viewport window around the pointer.
        {
            let session = session.clone();
            let weak_node = weak_node.clone();
            let plot = self.clone();
            registry.set_wheel(
                node,
                InputHandlerRoute::scroll(),
                Rc::new(move |event, phase| {
                    let Some(owner) = cove_core::scene::Node::upgrade(&weak_node) else {
                        return EventResult::Ignored;
                    };
                    if phase != EventPhase::Target
                        || !plot.is_cartesian()
                        || plot.viewport.range.is_none()
                    {
                        return EventResult::Ignored;
                    }
                    let Some(geometry) = plot_geometry(&plot, &owner, &session) else {
                        return EventResult::Ignored;
                    };
                    let window = session
                        .navigation
                        .borrow()
                        .window
                        .clone()
                        .or_else(|| geometry.window.clone());
                    let Some(window) = window else {
                        return EventResult::Ignored;
                    };
                    let x = event
                        .mouse_x
                        .unwrap_or(geometry.plot.x + geometry.plot.width / 2.0);
                    let y = event
                        .mouse_y
                        .unwrap_or(geometry.plot.y + geometry.plot.height / 2.0);
                    if !contains(x, y, geometry.plot) {
                        return EventResult::Ignored;
                    }
                    let (dx, dy) =
                        event.displacement(24.0, geometry.plot.width, geometry.plot.height);
                    if !dx.is_finite() || !dy.is_finite() {
                        return EventResult::Ignored;
                    }
                    let next = if dx.abs() > dy.abs() {
                        window.pan(-dx / geometry.plot.width.max(1.0))
                    } else {
                        let anchor =
                            window.fraction_at(x, y, geometry.plot, plot.style.orientation);
                        let exponent = (-dy).clamp(-500.0, 500.0) as f64 * 0.005;
                        window.zoom(exponent.exp() as f32, anchor)
                    };
                    if next.range == window.range {
                        return EventResult::Ignored;
                    }
                    set_window(&plot, next, &session);
                    EventResult::Handled
                }),
            );
        }

        // Keyboard: reset/zoom the window, clear the brush, step the selection.
        {
            let session = session.clone();
            let weak_node = weak_node.clone();
            let plot = self.clone();
            let choose = choose.clone();
            registry.set_key(
                node,
                InputHandlerRoute::control(),
                Rc::new(move |event, phase| {
                    let Some(owner) = cove_core::scene::Node::upgrade(&weak_node) else {
                        return EventResult::Ignored;
                    };
                    if phase != EventPhase::Target {
                        return EventResult::Ignored;
                    }
                    if event.modifiers.has_ctrl()
                        || event.modifiers.has_alt()
                        || event.modifiers.has_gui()
                    {
                        return EventResult::Ignored;
                    }
                    if plot.is_cartesian() && plot.viewport.range.is_some() {
                        match event.scancode {
                            Scancode::DIGIT0 => {
                                if let Some(range) = &plot.viewport.range {
                                    range.set(None);
                                }
                                session.navigation.borrow_mut().window = None;
                                return EventResult::Handled;
                            }
                            Scancode::EQUALS | Scancode::MINUS => {
                                let window =
                                    session.navigation.borrow().window.clone().or_else(|| {
                                        plot_geometry(&plot, &owner, &session)
                                            .and_then(|geometry| geometry.window)
                                    });
                                let Some(window) = window else {
                                    return EventResult::Ignored;
                                };
                                let factor = if event.scancode == Scancode::EQUALS {
                                    0.8
                                } else {
                                    1.25
                                };
                                set_window(&plot, window.zoom(factor, 0.5), &session);
                                return EventResult::Handled;
                            }
                            _ => {}
                        }
                    }
                    if event.scancode == Scancode::ESCAPE
                        && let Some(brush) = &plot.brush
                        && brush.get().is_some()
                    {
                        brush.set(None);
                        session.navigation.borrow_mut().cancel();
                        session.release(&owner);
                        return EventResult::Handled;
                    }
                    if !plot.input.is_enabled {
                        return EventResult::Ignored;
                    }
                    let current = session.current.get();
                    match event.scancode {
                        Scancode::ARROW_RIGHT => choose(plot.stepped_index(1, current)),
                        Scancode::ARROW_LEFT => choose(plot.stepped_index(-1, current)),
                        Scancode::ARROW_DOWN => choose(plot.stepped_series(1, current)),
                        Scancode::ARROW_UP => choose(plot.stepped_series(-1, current)),
                        Scancode::HOME => choose(plot.jumped(false, current)),
                        Scancode::END => choose(plot.jumped(true, current)),
                        Scancode::ESCAPE => {
                            if current.is_none() {
                                return EventResult::Ignored;
                            }
                            session.is_pinned.set(false);
                            session.reading.set(None);
                            choose(None);
                            return EventResult::Handled;
                        }
                        _ => return EventResult::Ignored,
                    }
                    session.is_pinned.set(session.current.get().is_some());
                    EventResult::Handled
                }),
            );
        }
    }
}

fn plot_geometry(
    plot: &ChartPlot,
    node: &cove_core::scene::Node,
    session: &Rc<ChartSession>,
) -> Option<ChartGeometry> {
    let frame = node.frame();
    let text = ChartText::new(&plot.theme(node));
    plot.resolve(
        cove_core::Size {
            width: frame.width,
            height: frame.height,
        },
        node.absolute_origin(),
        &text,
        Some(session.cache.clone()),
    )
    .cartesian
}

fn contains(x: f32, y: f32, plot: cove_core::Rect) -> bool {
    x >= plot.x && x <= plot.x + plot.width && y >= plot.y && y <= plot.y + plot.height
}

fn begin_navigation(
    plot: &ChartPlot,
    x: f32,
    y: f32,
    node: &cove_core::scene::Node,
    session: &Rc<ChartSession>,
) -> bool {
    if !plot.is_cartesian() {
        return false;
    }
    let Some(geometry) = plot_geometry(plot, node, session) else {
        return false;
    };
    let Some(window) = geometry.window.clone() else {
        return false;
    };
    if !contains(x, y, geometry.plot) {
        return false;
    }
    let fraction = window.fraction_at(x, y, geometry.plot, plot.style.orientation);
    let mut navigation = session.navigation.borrow_mut();
    if plot.viewport.range.is_some() && (window_pan_by_default(plot) || !plot.input.is_enabled) {
        navigation.drag = Some(ChartDrag::Pan {
            anchor: fraction,
            window,
        });
    } else if plot.brush.is_some() {
        let value = window.value_at(fraction.clamp(0.0, 1.0));
        navigation.drag = Some(ChartDrag::Brush {
            anchor: value,
            window,
        });
        drop(navigation);
        if let Some(brush) = &plot.brush {
            brush.set(Some(ChartRange::new(value, value)));
        }
    } else {
        return false;
    }
    true
}

/// The Swift path pans when the alt modifier is held. The Rust input event
/// carries modifiers only on pointer/press, so the default (no alt) decides.
fn window_pan_by_default(plot: &ChartPlot) -> bool {
    plot.brush.is_none()
}

fn move_navigation(
    plot: &ChartPlot,
    x: f32,
    y: f32,
    node: &cove_core::scene::Node,
    session: &Rc<ChartSession>,
) -> bool {
    let drag = session.navigation.borrow().drag.clone();
    let Some(drag) = drag else { return false };
    let Some(geometry) = plot_geometry(plot, node, session) else {
        return false;
    };
    match drag {
        ChartDrag::Brush { anchor, window } => {
            let fraction = window.fraction_at(x, y, geometry.plot, plot.style.orientation);
            let value = window.value_at(fraction.clamp(0.0, 1.0));
            if let Some(brush) = &plot.brush {
                brush.set(Some(ChartRange::new(anchor.min(value), anchor.max(value))));
            }
        }
        ChartDrag::Pan { anchor, window } => {
            let fraction = window.fraction_at(x, y, geometry.plot, plot.style.orientation);
            set_window(plot, window.pan(anchor - fraction), session);
        }
    }
    true
}

fn set_window(plot: &ChartPlot, window: ChartWindow, session: &Rc<ChartSession>) {
    session.navigation.borrow_mut().window = Some(window.clone());
    if let Some(range) = &plot.viewport.range {
        range.set(if window.range == window.full {
            None
        } else {
            Some(window.range)
        });
    }
}

/// Keeps the orientation enumerant reachable for callers wiring their own
/// pointer handling, mirroring the Swift extension point.
pub fn plot_orientation(plot: &ChartPlot) -> ChartOrientation {
    plot.style.orientation
}

/// Node-space point helper for hosts that need to convert window coordinates
/// before calling into the chart.
pub fn local_point(node: &cove_core::scene::Node, x: f32, y: f32) -> Point {
    node.convert_from_window(Point::new(x, y))
}

/// Focus/capture cleanup shared by unmount paths.
pub fn release_session(node: &cove_core::scene::Node) {
    if let Some(capture) = PointerCaptureHolder::current()
        && capture.target().is_some_and(|target| target == *node)
    {
        capture.release();
    }
    if let Some(chain) = FocusChainHolder::current()
        && chain.focused().is_some_and(|focused| focused == *node)
    {
        chain.clear();
    }
}

/// The session's navigation cell, exposed for hosts that need to reset it.
pub fn cancel_navigation(session: &Rc<ChartSession>) {
    session.navigation.borrow_mut().cancel();
}

/// Shared cell so callers can inspect a live drag without borrowing the
/// session through the plot.
pub type ChartNavigationCell = RefCell<ChartNavigation>;
