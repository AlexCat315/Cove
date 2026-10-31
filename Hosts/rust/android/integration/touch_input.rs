//! Translation of Android/Integration/TouchInput.swift.

use cove_core::Point;
use cove_core::scene::{
    EventDispatcher, HitTester, InputHandlerRole, NodeTree, PlatformInputContext,
};
use cove_platform::{
    InputEvent, MouseButton, MouseButtonEvent, MouseMotionEvent, MouseWheelEvent, ScrollDevice,
    ScrollInput, ScrollPhase, ScrollUnit,
};
use std::time::{SystemTime, UNIX_EPOCH};

fn now_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .unwrap_or(0.0)
}

/// Classifies a primary touch as a control drag or a scroll gesture.
#[derive(Clone, Debug, Default)]
pub struct TouchInput {
    down: Point,
    previous: Point,
    scrolling: bool,
}

impl TouchInput {
    pub fn new() -> Self {
        Self {
            down: Point::default(),
            previous: Point::default(),
            scrolling: false,
        }
    }

    pub fn route(
        &mut self,
        kind: i32,
        point: Point,
        tree: &NodeTree,
        context: &PlatformInputContext,
        dispatcher: &EventDispatcher,
    ) {
        let button = MouseButtonEvent {
            button: MouseButton::Left,
            x: point.x,
            y: point.y,
            clicks: 1,
            ..MouseButtonEvent::default()
        };
        let down = self.down;
        let wheel = move |delta: Point, phase: ScrollPhase| {
            dispatcher.dispatch(&InputEvent::MouseWheel(MouseWheelEvent {
                x: delta.x,
                y: delta.y,
                mouse_x: Some(down.x),
                mouse_y: Some(down.y),
                input: ScrollInput {
                    unit: ScrollUnit::Points,
                    device: ScrollDevice::Continuous,
                    phase,
                    timestamp: now_seconds(),
                    ..ScrollInput::default()
                },
            }));
        };
        match kind {
            0 => {
                self.down = point;
                self.previous = point;
                self.scrolling = false;
                dispatcher.dispatch(&InputEvent::MouseButtonDown(button));
            }
            2 => {
                let delta = Point::new(point.x - self.previous.x, point.y - self.previous.y);
                if !self.scrolling
                    && (point.x - self.down.x).abs() + (point.y - self.down.y).abs() > 8.0
                    && let Some(root) = tree.root()
                    && let Some(hit) = HitTester::hit_test(&root, self.down)
                {
                    let capture = context.pointer_capture.target();
                    let dragging_control = capture
                        .map(|node| context.interactions.handlers(&node).motion.is_some())
                        .unwrap_or(false);
                    let mut node = Some(hit.node);
                    let mut can_scroll = false;
                    while let Some(current) = node {
                        if context
                            .interactions
                            .handlers(&current)
                            .wheel_route
                            .is_some_and(|route| route.role == InputHandlerRole::Scroll)
                        {
                            can_scroll = true;
                            break;
                        }
                        node = current.parent();
                    }
                    if can_scroll && !dragging_control {
                        context.pointer_capture.release();
                        self.scrolling = true;
                        wheel(
                            Point::new(point.x - self.down.x, point.y - self.down.y),
                            ScrollPhase::Began,
                        );
                    }
                } else if self.scrolling {
                    wheel(delta, ScrollPhase::Changed);
                }
                if !self.scrolling {
                    dispatcher.dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
                        x: point.x,
                        y: point.y,
                        delta_x: delta.x,
                        delta_y: delta.y,
                        ..MouseMotionEvent::default()
                    }));
                }
                self.previous = point;
            }
            1 => {
                if self.scrolling {
                    wheel(Point::default(), ScrollPhase::Ended);
                } else {
                    dispatcher.dispatch(&InputEvent::MouseButtonUp(button));
                }
                self.scrolling = false;
            }
            3 => {
                if self.scrolling {
                    wheel(Point::default(), ScrollPhase::Cancelled);
                }
                context.pointer_capture.release();
                self.scrolling = false;
            }
            _ => {}
        }
    }
}
