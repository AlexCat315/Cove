//! Node-owned glue between one terminal surface and its session.
//!
//! Observes the session while mounted and keeps per-view state: IME preedit,
//! pointer gestures, cursor blink and image textures.

use crate::session::TerminalSession;
use crate::view::{
    blink_timer::BlinkTimer,
    cell_metrics::CellMetrics,
    image_textures::ImageTextures,
    pointer_router::{PointerGeometry, PointerRouter},
    url_detector::UrlMatch,
};
use cove_core::scene::{AnimatorScheduler, InvalidationSource, Node, NodeResource};
use cove_platform::TextInputArea;
use std::{
    any::Any,
    cell::RefCell,
    rc::{Rc, Weak},
};

pub struct SurfaceState {
    session: RefCell<Option<Rc<TerminalSession>>>,
    node: RefCell<Weak<Node>>,
    /// Cell size of the last paint; input waits for it.
    metrics: RefCell<Option<CellMetrics>>,
    /// IME composition shown at the cursor until committed.
    preedit: RefCell<String>,
    /// Plain-text URL under the pointer (OSC 8 links are marked by the program).
    hovered_link: RefCell<Option<UrlMatch>>,
    open_link: RefCell<Rc<dyn Fn(&str)>>,
    pub pointer: PointerRouter,
    pub blink: Rc<BlinkTimer>,
    pub text_blink: Rc<BlinkTimer>,
    pub textures: RefCell<ImageTextures>,
    observer: RefCell<Option<usize>>,
}

impl SurfaceState {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            session: RefCell::new(None),
            node: RefCell::new(Weak::new()),
            metrics: RefCell::new(None),
            preedit: RefCell::new(String::new()),
            hovered_link: RefCell::new(None),
            open_link: RefCell::new(Rc::new(|link: &str| crate::view::link_opener::open(link))),
            pointer: PointerRouter::new(),
            blink: BlinkTimer::new(),
            text_blink: BlinkTimer::new(),
            textures: RefCell::new(ImageTextures::new()),
            observer: RefCell::new(None),
        })
    }

    pub fn session(&self) -> Option<Rc<TerminalSession>> {
        self.session.borrow().clone()
    }

    pub fn node(&self) -> Option<Node> {
        self.node.borrow().upgrade()
    }

    pub fn metrics(&self) -> Option<CellMetrics> {
        self.metrics.borrow().clone()
    }

    pub fn set_metrics(&self, metrics: CellMetrics) {
        *self.metrics.borrow_mut() = Some(metrics);
    }

    pub fn preedit(&self) -> String {
        self.preedit.borrow().clone()
    }

    pub fn set_preedit(&self, text: impl Into<String>) {
        let text = text.into();
        let changed = *self.preedit.borrow() != text;
        *self.preedit.borrow_mut() = text;
        if changed {
            self.redraw();
        }
    }

    pub fn hovered_link(&self) -> Option<UrlMatch> {
        self.hovered_link.borrow().clone()
    }

    pub fn set_hovered_link(&self, link: Option<UrlMatch>) {
        let changed = *self.hovered_link.borrow() != link;
        *self.hovered_link.borrow_mut() = link;
        if changed {
            self.redraw();
        }
    }

    pub fn set_open_link(&self, open: Rc<dyn Fn(&str)>) {
        *self.open_link.borrow_mut() = open;
    }

    pub fn open_link(&self, link: &str) {
        let open = self.open_link.borrow().clone();
        open(link);
    }

    pub fn bind(&self, session: Rc<TerminalSession>) {
        if let Some(current) = self.session.borrow().as_ref()
            && Rc::ptr_eq(current, &session)
        {
            return;
        }
        self.unobserve();
        self.set_preedit("");
        self.pointer.reset();
        self.textures.borrow_mut().remove_all();
        let weak = Rc::downgrade(self);
        // The session is single-threaded like the scene; observation is what
        // turns new output into a repaint.
        let token = session.observe(move || {
            if let Some(state) = weak.upgrade() {
                // New output may have moved or replaced the hovered text.
                state.set_hovered_link(None);
                state.blink.restart();
                state.redraw();
            }
        });
        *self.observer.borrow_mut() = Some(token);
        *self.session.borrow_mut() = Some(session);
        self.observe_now();
    }

    /// Grid position under a window-space point, clamped to the grid.
    pub fn grid_pointer(&self, x: f32, y: f32) -> Option<crate::engine::TerminalPointer> {
        let node = self.node()?;
        let metrics = self.metrics.borrow().clone()?;
        let session = self.session.borrow().clone()?;
        let frame = session.frame();
        let origin = node.absolute_origin();
        let geometry = PointerGeometry {
            origin: (origin.x, origin.y),
            cell: (metrics.width, metrics.height),
            engine_cell: session.engine_cell_size(),
            frame_columns: frame.columns,
            frame_rows: frame.rows,
            height: node.frame().height,
        };
        self.pointer.pointer(&geometry, x, y, 0)
    }

    /// The geometry the pointer router needs, from the node and current metrics.
    pub fn geometry(&self) -> Option<PointerGeometry> {
        let node = self.node()?;
        let metrics = self.metrics.borrow().clone()?;
        let session = self.session.borrow().clone()?;
        let frame = session.frame();
        let origin = node.absolute_origin();
        Some(PointerGeometry {
            origin: (origin.x, origin.y),
            cell: (metrics.width, metrics.height),
            engine_cell: session.engine_cell_size(),
            frame_columns: frame.columns,
            frame_rows: frame.rows,
            height: node.frame().height,
        })
    }

    pub fn redraw(&self) {
        if let Some(node) = self.node() {
            node.mark_render_dirty(InvalidationSource::style_set("terminal.surface"));
        }
    }

    /// The cursor cell in window coordinates, where the IME composes.
    pub fn text_input_area(&self, origin: (f32, f32), metrics: &CellMetrics) -> Option<TextInputArea> {
        let session = self.session.borrow().clone()?;
        let frame = session.frame();
        let cursor = frame.cursor;
        let x = origin.0 + cursor.column as f32 * metrics.width;
        let y = origin.1 + cursor.row as f32 * metrics.height;
        Some(TextInputArea { x, y, width: metrics.width, height: metrics.height, cursor_x: x })
    }

    fn observe_now(&self) {
        self.redraw();
    }

    fn unobserve(&self) {
        if let Some(token) = self.observer.borrow_mut().take()
            && let Some(session) = self.session.borrow().as_ref()
        {
            session.remove_observer(token);
        }
        *self.session.borrow_mut() = None;
    }
}

impl Default for SurfaceState {
    fn default() -> Self {
        // Only used through `new`, which returns the `Rc` the node needs.
        Self {
            session: RefCell::new(None),
            node: RefCell::new(Weak::new()),
            metrics: RefCell::new(None),
            preedit: RefCell::new(String::new()),
            hovered_link: RefCell::new(None),
            open_link: RefCell::new(Rc::new(|link: &str| crate::view::link_opener::open(link))),
            pointer: PointerRouter::new(),
            blink: Rc::new(BlinkTimer::default()),
            text_blink: Rc::new(BlinkTimer::default()),
            textures: RefCell::new(ImageTextures::new()),
            observer: RefCell::new(None),
        }
    }
}

impl NodeResource for SurfaceState {
    fn mount(&self, node: &Node) {
        *self.node.borrow_mut() = node.downgrade();
        self.blink.attach(AnimatorScheduler::current());
        self.text_blink.attach(AnimatorScheduler::current());
        let redraw = Rc::downgrade(self);
        self.blink.set_on_toggle(Some(Rc::new(move || {
            if let Some(state) = redraw.upgrade() {
                state.redraw();
            }
        })));
        let redraw = Rc::downgrade(self);
        self.text_blink.set_on_toggle(Some(Rc::new(move || {
            if let Some(state) = redraw.upgrade() {
                state.redraw();
            }
        })));
        self.redraw();
    }

    fn unmount(&self, _node: &Node) {
        self.unobserve();
        self.blink.detach();
        self.text_blink.detach();
        self.textures.borrow_mut().remove_all();
        *self.node.borrow_mut() = Weak::new();
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }
}
