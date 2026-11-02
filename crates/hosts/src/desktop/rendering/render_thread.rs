//! Presentation on a dedicated render thread.
//!
//! The UI thread composes, lays out and records frame N+1 while the render
//! thread encodes, submits and presents frame N. A frame crosses as an owned
//! [`DrawList`] (an immutable snapshot once sent); its buffer comes back with
//! the result and is reused, so steady-state frames allocate nothing.
//!
//! One frame is in flight at a time: handing over a frame first waits for the
//! previous one, which bounds latency to a single frame and keeps the swapchain
//! from queueing work the UI cannot see. Configuration, resizing and teardown
//! are synchronous and ordered with frames by the command channel, as are font
//! atlas uploads, so an upload always lands before the frame that samples it.
use crate::desktop::application::memory::AppWindowMemorySnapshot;
use crate::desktop::window::native_renderer::AppWindowRenderer;
use cove_core::DrawList;
use cove_graphics::rhi::{RhiError, RhiResult};
use cove_graphics::ui::FontAtlasUpload;
use cove_shell::NativeRenderSurface;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

type Reply<T> = Sender<T>;

enum Command {
    Configure(NativeRenderSurface, (u32, u32), bool, Reply<RhiResult<()>>),
    Resize((u32, u32), bool, Reply<RhiResult<()>>),
    Upload(FontAtlasUpload),
    Frame(DrawList, (f32, f32)),
    SetPresentationActive(bool, Reply<RhiResult<()>>),
    Snapshot(Reply<AppWindowMemorySnapshot>),
    Close(Reply<RhiResult<()>>),
}

struct FrameDone {
    list: DrawList,
    result: RhiResult<bool>,
}

fn stopped() -> RhiError {
    RhiError::invalid_argument("the render thread has stopped")
}

/// An [`AppWindowRenderer`] that runs another renderer on its own thread.
pub struct ThreadedWindowRenderer {
    commands: Sender<Command>,
    frames: Receiver<FrameDone>,
    in_flight: bool,
    recycled: Option<DrawList>,
    configured: bool,
    closed: bool,
    thread: Option<JoinHandle<()>>,
}

impl ThreadedWindowRenderer {
    /// Moves `renderer` onto a new render thread named after `label`.
    pub fn spawn(renderer: Box<dyn AppWindowRenderer + Send>, label: &str) -> Self {
        let (commands, inbox) = channel::<Command>();
        let (done, frames) = channel::<FrameDone>();
        let thread = std::thread::Builder::new()
            .name(format!("cove-render-{label}"))
            .spawn(move || run(renderer, inbox, done))
            .expect("the render thread could not start");
        Self {
            commands,
            frames,
            in_flight: false,
            recycled: None,
            configured: false,
            closed: false,
            thread: Some(thread),
        }
    }

    fn request<T>(&mut self, command: impl FnOnce(Reply<T>) -> Command) -> Option<T> {
        let (reply, answer) = channel();
        self.commands.send(command(reply)).ok()?;
        answer.recv().ok()
    }

    /// Waits for the frame in flight and returns its result.
    fn finish_in_flight(&mut self) -> RhiResult<bool> {
        if !self.in_flight {
            return Ok(true);
        }
        self.in_flight = false;
        let done = self.frames.recv().map_err(|_| stopped())?;
        self.recycled = Some(done.list);
        done.result
    }

    /// Blocks until the GPU work handed over so far has been submitted.
    pub fn flush(&mut self) -> RhiResult<bool> {
        self.finish_in_flight()
    }
}

impl AppWindowRenderer for ThreadedWindowRenderer {
    fn is_configured(&self) -> bool {
        self.configured
    }

    fn memory_snapshot(&self) -> AppWindowMemorySnapshot {
        let (reply, answer) = channel();
        if self.commands.send(Command::Snapshot(reply)).is_err() {
            return AppWindowMemorySnapshot::default();
        }
        answer.recv().unwrap_or_default()
    }

    fn configure(
        &mut self,
        native: NativeRenderSurface,
        size: (u32, u32),
        vsync: bool,
    ) -> RhiResult<()> {
        let result = self
            .request(|reply| Command::Configure(native, size, vsync, reply))
            .unwrap_or_else(|| Err(stopped()));
        self.configured = result.is_ok();
        result
    }

    fn resize(&mut self, size: (u32, u32), vsync: bool) -> RhiResult<()> {
        self.request(|reply| Command::Resize(size, vsync, reply))
            .unwrap_or_else(|| Err(stopped()))
    }

    /// Hands `list` to the render thread and returns the previous frame's
    /// result; `list` is left holding a recycled buffer.
    fn draw(&mut self, list: &mut DrawList, logical: (f32, f32)) -> RhiResult<bool> {
        let previous = self.finish_in_flight();
        let mut frame = self.recycled.take().unwrap_or_default();
        std::mem::swap(&mut frame, list);
        self.commands
            .send(Command::Frame(frame, logical))
            .map_err(|_| stopped())?;
        self.in_flight = true;
        previous
    }

    fn upload_font_atlas(&mut self, upload: FontAtlasUpload) -> RhiResult<()> {
        self.commands
            .send(Command::Upload(upload))
            .map_err(|_| stopped())
    }

    fn set_presentation_active(&mut self, active: bool) -> RhiResult<()> {
        self.request(|reply| Command::SetPresentationActive(active, reply))
            .unwrap_or_else(|| Err(stopped()))
    }

    fn close(&mut self) -> RhiResult<()> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        let _ = self.finish_in_flight();
        let result = self
            .request(Command::Close)
            .unwrap_or_else(|| Err(stopped()));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        self.configured = false;
        result
    }
}

impl Drop for ThreadedWindowRenderer {
    fn drop(&mut self) {
        let _ = AppWindowRenderer::close(self);
    }
}

fn run(
    mut renderer: Box<dyn AppWindowRenderer + Send>,
    inbox: Receiver<Command>,
    done: Sender<FrameDone>,
) {
    // An upload that fails is reported with the next frame, where the UI
    // thread already handles render errors.
    let mut pending_error: Option<RhiError> = None;
    while let Ok(command) = inbox.recv() {
        match command {
            Command::Configure(native, size, vsync, reply) => {
                let _ = reply.send(renderer.configure(native, size, vsync));
            }
            Command::Resize(size, vsync, reply) => {
                let _ = reply.send(renderer.resize(size, vsync));
            }
            Command::Upload(upload) => {
                if let Err(error) = renderer.upload_font_atlas(upload) {
                    pending_error.get_or_insert(error);
                }
            }
            Command::Frame(mut list, logical) => {
                let result = match pending_error.take() {
                    Some(error) => Err(error),
                    None => renderer.draw(&mut list, logical),
                };
                if done.send(FrameDone { list, result }).is_err() {
                    break;
                }
            }
            Command::SetPresentationActive(active, reply) => {
                let _ = reply.send(renderer.set_presentation_active(active));
            }
            Command::Snapshot(reply) => {
                let _ = reply.send(renderer.memory_snapshot());
            }
            Command::Close(reply) => {
                let _ = reply.send(renderer.close());
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop::application::memory::AppWindowRendererState;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Log {
        events: Vec<String>,
        threads: Vec<std::thread::ThreadId>,
    }

    struct Recording(Arc<Mutex<Log>>);

    impl AppWindowRenderer for Recording {
        fn is_configured(&self) -> bool {
            true
        }
        fn memory_snapshot(&self) -> AppWindowMemorySnapshot {
            AppWindowMemorySnapshot {
                state: AppWindowRendererState::Active,
                ..Default::default()
            }
        }
        fn configure(&mut self, _: NativeRenderSurface, size: (u32, u32), _: bool) -> RhiResult<()> {
            self.0.lock().unwrap().events.push(format!("configure {}x{}", size.0, size.1));
            Ok(())
        }
        fn resize(&mut self, size: (u32, u32), _: bool) -> RhiResult<()> {
            self.0.lock().unwrap().events.push(format!("resize {}x{}", size.0, size.1));
            Ok(())
        }
        fn draw(&mut self, list: &mut DrawList, _: (f32, f32)) -> RhiResult<bool> {
            let mut log = self.0.lock().unwrap();
            log.events.push(format!("draw {}", list.pixel_scale));
            log.threads.push(std::thread::current().id());
            Ok(true)
        }
        fn upload_font_atlas(&mut self, _: FontAtlasUpload) -> RhiResult<()> {
            self.0.lock().unwrap().events.push("upload".into());
            Ok(())
        }
        fn set_presentation_active(&mut self, active: bool) -> RhiResult<()> {
            self.0.lock().unwrap().events.push(format!("active {active}"));
            Ok(())
        }
        fn close(&mut self) -> RhiResult<()> {
            self.0.lock().unwrap().events.push("close".into());
            Ok(())
        }
    }

    #[test]
    fn frames_run_on_the_render_thread_in_order_with_commands() {
        let log = Arc::new(Mutex::new(Log::default()));
        let mut renderer = ThreadedWindowRenderer::spawn(Box::new(Recording(log.clone())), "test");
        renderer
            .configure(NativeRenderSurface::MetalLayer(0), (10, 10), true)
            .unwrap();
        assert!(renderer.is_configured());
        for scale in [1.0_f32, 2.0, 3.0] {
            let mut list = DrawList::default();
            list.pixel_scale = scale;
            assert!(renderer.draw(&mut list, (10.0, 10.0)).unwrap());
            assert_eq!(list.pixel_scale, if scale == 1.0 { 0.0 } else { scale - 1.0 }, "the list comes back recycled");
        }
        renderer.resize((20, 20), true).unwrap();
        assert_eq!(
            renderer.memory_snapshot().state,
            AppWindowRendererState::Active
        );
        renderer.close().unwrap();
        let log = log.lock().unwrap();
        assert_eq!(
            log.events,
            ["configure 10x10", "draw 1", "draw 2", "draw 3", "resize 20x20", "close"]
        );
        assert!(log.threads.iter().all(|thread| *thread != std::thread::current().id()));
    }
}
