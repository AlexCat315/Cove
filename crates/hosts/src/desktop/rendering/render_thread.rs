//! Presentation on a dedicated render thread.
//!
//! The UI thread composes, lays out and records frame N+1 while the render
//! thread encodes, submits and presents earlier frames. A frame crosses as an
//! owned [`DrawList`] (an immutable snapshot once sent); its buffer comes back
//! with the result and is reused, so steady-state frames allocate nothing.
//!
//! Handing over a frame never waits. Up to [`MAX_FRAMES_IN_FLIGHT`] frames may be
//! queued, which bounds latency; with none free the UI records nothing and tries
//! at the next opportunity. Every wait on the GPU and the display stays on the
//! render thread. Results, including errors from reconfiguration, come back
//! through [`ThreadedWindowRenderer::take_presented`]. Configuration, resizing,
//! font uploads and frames are ordered by one command channel, so an upload
//! always lands before the frame that samples it.
use crate::desktop::application::memory::AppWindowMemorySnapshot;
use crate::desktop::window::native_renderer::AppWindowRenderer;
use cove_core::DrawList;
use cove_core::scene::wake_scene_thread;
use cove_graphics::rhi::{RhiError, RhiResult};
use cove_graphics::ui::FontAtlasUpload;
use cove_shell::NativeRenderSurface;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// One frame encoding plus one queued: the UI never runs more than two frames
/// ahead of the display.
pub const MAX_FRAMES_IN_FLIGHT: usize = 2;

/// The native window is destroyed right after the renderer closes, so close
/// waits for the swapchain to go, but a wedged GPU must not hang the caller.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(2);

type Reply<T> = Sender<T>;

/// Identifies a frame handed to the render thread.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FrameToken(pub u64);

/// The outcome of a frame that left the render thread.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FramePresented {
    pub token: FrameToken,
    /// False when the renderer skipped presentation (occluded or locked window).
    pub presented: bool,
    pub cpu_ms: f64,
}

enum Command {
    Configure(NativeRenderSurface, (u32, u32), bool, Reply<RhiResult<()>>),
    Resize((u32, u32), bool),
    Upload(FontAtlasUpload),
    Frame(FrameToken, DrawList, (f32, f32)),
    SetPresentationActive(bool),
    Trim,
    Close(Reply<RhiResult<()>>),
}

enum Completion {
    Frame {
        token: FrameToken,
        list: DrawList,
        result: RhiResult<bool>,
        cpu_ms: f64,
    },
    Failed(RhiError),
}

fn stopped() -> RhiError {
    RhiError::invalid_argument("the render thread has stopped")
}

/// Owns the presentation renderer and executes commands in order.
struct Worker {
    renderer: Box<dyn AppWindowRenderer + Send>,
    done: Sender<Completion>,
    snapshot: Arc<Mutex<AppWindowMemorySnapshot>>,
}

impl Worker {
    /// Returns false once the renderer closed.
    fn handle(&mut self, command: Command) -> bool {
        match command {
            Command::Configure(native, size, vsync, reply) => {
                let result = self.renderer.configure(native, size, vsync);
                self.publish_snapshot();
                let _ = reply.send(result);
            }
            Command::Resize(size, vsync) => {
                let result = self.renderer.resize(size, vsync);
                self.report(result);
            }
            Command::Upload(upload) => {
                let result = self.renderer.upload_font_atlas(upload);
                self.report(result);
            }
            Command::Frame(token, mut list, logical) => {
                let start = Instant::now();
                let result = self.renderer.draw(&mut list, logical);
                self.publish_snapshot();
                let _ = self.done.send(Completion::Frame {
                    token,
                    list,
                    result,
                    cpu_ms: start.elapsed().as_secs_f64() * 1000.0,
                });
                wake_scene_thread();
            }
            Command::SetPresentationActive(active) => {
                let result = self.renderer.set_presentation_active(active);
                self.report(result);
            }
            Command::Trim => {
                let result = self.renderer.trim_memory();
                self.report(result);
            }
            Command::Close(reply) => {
                let result = self.renderer.close();
                let _ = reply.send(result);
                return false;
            }
        }
        true
    }

    fn report(&self, result: RhiResult<()>) {
        self.publish_snapshot();
        if let Err(error) = result {
            let _ = self.done.send(Completion::Failed(error));
            wake_scene_thread();
        }
    }

    fn publish_snapshot(&self) {
        *self.snapshot.lock().unwrap_or_else(|e| e.into_inner()) = self.renderer.memory_snapshot();
    }
}

enum Transport {
    Thread {
        commands: Sender<Command>,
        thread: Option<JoinHandle<()>>,
    },
    /// Commands run on the caller's thread: a debugging fallback.
    Inline(Worker),
}

/// One window's presentation, driven without waiting on the GPU or the display.
pub struct ThreadedWindowRenderer {
    transport: Transport,
    frames: Receiver<Completion>,
    in_flight: usize,
    next_token: u64,
    recycled: Vec<DrawList>,
    snapshot: Arc<Mutex<AppWindowMemorySnapshot>>,
    configured: bool,
    closed: bool,
}

impl ThreadedWindowRenderer {
    /// Moves `renderer` onto a new render thread named after `label`.
    pub fn spawn(renderer: Box<dyn AppWindowRenderer + Send>, label: &str) -> Self {
        let (commands, inbox) = channel::<Command>();
        let (mut this, mut worker) = Self::parts(renderer);
        let thread = std::thread::Builder::new()
            .name(format!("cove-render-{label}"))
            .spawn(move || {
                while let Ok(command) = inbox.recv() {
                    if !worker.handle(command) {
                        break;
                    }
                }
            })
            .expect("the render thread could not start");
        this.transport = Transport::Thread {
            commands,
            thread: Some(thread),
        };
        this
    }

    /// Presents on the caller's thread with the same interface.
    pub fn inline(renderer: Box<dyn AppWindowRenderer + Send>) -> Self {
        let (mut this, worker) = Self::parts(renderer);
        this.transport = Transport::Inline(worker);
        this
    }

    fn parts(renderer: Box<dyn AppWindowRenderer + Send>) -> (Self, Worker) {
        let (done, frames) = channel::<Completion>();
        let snapshot = Arc::new(Mutex::new(renderer.memory_snapshot()));
        let worker = Worker {
            renderer,
            done,
            snapshot: snapshot.clone(),
        };
        let (idle, _) = channel();
        let this = Self {
            transport: Transport::Thread {
                commands: idle,
                thread: None,
            },
            frames,
            in_flight: 0,
            next_token: 0,
            recycled: Vec::new(),
            snapshot,
            configured: false,
            closed: false,
        };
        (this, worker)
    }

    fn send(&mut self, command: Command) -> RhiResult<()> {
        match &mut self.transport {
            Transport::Thread { commands, .. } => commands.send(command).map_err(|_| stopped()),
            Transport::Inline(worker) => {
                worker.handle(command);
                Ok(())
            }
        }
    }

    pub fn is_configured(&self) -> bool {
        self.configured
    }

    /// The renderer's memory as of the last command it finished.
    pub fn memory_snapshot(&self) -> AppWindowMemorySnapshot {
        *self.snapshot.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Waits for swapchain creation, which only happens while no frame exists.
    pub fn configure(
        &mut self,
        native: NativeRenderSurface,
        size: (u32, u32),
        vsync: bool,
    ) -> RhiResult<()> {
        let (reply, answer) = channel();
        self.send(Command::Configure(native, size, vsync, reply))?;
        let result = answer.recv().unwrap_or_else(|_| Err(stopped()));
        self.configured = result.is_ok();
        result
    }

    pub fn resize(&mut self, size: (u32, u32), vsync: bool) -> RhiResult<()> {
        self.send(Command::Resize(size, vsync))
    }

    pub fn upload_font_atlas(&mut self, upload: FontAtlasUpload) -> RhiResult<()> {
        self.send(Command::Upload(upload))
    }

    pub fn set_presentation_active(&mut self, active: bool) -> RhiResult<()> {
        self.send(Command::SetPresentationActive(active))
    }

    pub fn trim_memory(&mut self) -> RhiResult<()> {
        self.send(Command::Trim)
    }

    /// Takes every finished frame, recycling its buffer. The first failure since
    /// the last call wins.
    pub fn take_presented(&mut self) -> RhiResult<Vec<FramePresented>> {
        let mut presented = Vec::new();
        let mut failure = None;
        loop {
            match self.frames.try_recv() {
                Ok(completion) => self.record(completion, &mut presented, &mut failure),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if !self.closed && matches!(self.transport, Transport::Thread { .. }) {
                        failure.get_or_insert_with(stopped);
                    }
                    break;
                }
            }
        }
        match failure {
            Some(error) => Err(error),
            None => Ok(presented),
        }
    }

    fn record(
        &mut self,
        completion: Completion,
        presented: &mut Vec<FramePresented>,
        failure: &mut Option<RhiError>,
    ) {
        match completion {
            Completion::Frame {
                token,
                list,
                result,
                cpu_ms,
            } => {
                self.in_flight = self.in_flight.saturating_sub(1);
                self.recycled.push(list);
                match result {
                    Ok(value) => presented.push(FramePresented {
                        token,
                        presented: value,
                        cpu_ms,
                    }),
                    Err(error) => {
                        failure.get_or_insert(error);
                    }
                }
            }
            Completion::Failed(error) => {
                failure.get_or_insert(error);
            }
        }
    }

    /// Whether a frame recorded now can be handed over. The caller checks before
    /// recording so a dropped frame costs no work.
    pub fn has_frame_capacity(&self) -> bool {
        self.in_flight < MAX_FRAMES_IN_FLIGHT
    }

    /// Hands `list` to the render thread and returns at once; `list` is left
    /// holding a recycled buffer. `None` means every frame is in flight, the
    /// frame was dropped and `list` is untouched.
    pub fn submit(
        &mut self,
        list: &mut DrawList,
        logical: (f32, f32),
    ) -> RhiResult<Option<FrameToken>> {
        if !self.has_frame_capacity() {
            return Ok(None);
        }
        let mut frame = self.recycled.pop().unwrap_or_default();
        std::mem::swap(&mut frame, list);
        // Settings that outlive one frame stay with the recording side.
        list.geometry_antialiasing = frame.geometry_antialiasing;
        list.clip_revision = frame.clip_revision;
        self.next_token += 1;
        let token = FrameToken(self.next_token);
        self.send(Command::Frame(token, frame, logical))?;
        self.in_flight += 1;
        Ok(Some(token))
    }

    /// Stops the renderer. Waits for the swapchain to go, within a bound.
    pub fn close(&mut self) -> RhiResult<()> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        self.configured = false;
        let (reply, answer) = channel();
        self.send(Command::Close(reply))?;
        let result = match answer.recv_timeout(CLOSE_TIMEOUT) {
            Ok(result) => result,
            Err(RecvTimeoutError::Timeout) => {
                // Give up on the thread; the process keeps running without it.
                if let Transport::Thread { thread, .. } = &mut self.transport {
                    thread.take();
                }
                return Err(RhiError::invalid_argument(
                    "the render thread did not stop in time",
                ));
            }
            Err(RecvTimeoutError::Disconnected) => Err(stopped()),
        };
        if let Transport::Thread { thread, .. } = &mut self.transport
            && let Some(thread) = thread.take()
        {
            let _ = thread.join();
        }
        result
    }
}

impl Drop for ThreadedWindowRenderer {
    fn drop(&mut self) {
        let _ = self.close();
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
        fn configure(
            &mut self,
            _: NativeRenderSurface,
            size: (u32, u32),
            _: bool,
        ) -> RhiResult<()> {
            self.0
                .lock()
                .unwrap()
                .events
                .push(format!("configure {}x{}", size.0, size.1));
            Ok(())
        }
        fn resize(&mut self, size: (u32, u32), _: bool) -> RhiResult<()> {
            self.0
                .lock()
                .unwrap()
                .events
                .push(format!("resize {}x{}", size.0, size.1));
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
            self.0
                .lock()
                .unwrap()
                .events
                .push(format!("active {active}"));
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
            while !renderer.has_frame_capacity() {
                renderer.take_presented().unwrap();
                std::thread::yield_now();
            }
            assert!(renderer.submit(&mut list, (10.0, 10.0)).unwrap().is_some());
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
            [
                "configure 10x10",
                "draw 1",
                "draw 2",
                "draw 3",
                "resize 20x20",
                "close"
            ]
        );
        assert!(
            log.threads
                .iter()
                .all(|thread| *thread != std::thread::current().id())
        );
    }

    /// Draws block until the test releases them, standing in for a GPU that waits
    /// for the display.
    struct Gated {
        release: Arc<(Mutex<usize>, std::sync::Condvar)>,
        drawn: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl AppWindowRenderer for Gated {
        fn is_configured(&self) -> bool {
            true
        }
        fn memory_snapshot(&self) -> AppWindowMemorySnapshot {
            AppWindowMemorySnapshot::default()
        }
        fn configure(&mut self, _: NativeRenderSurface, _: (u32, u32), _: bool) -> RhiResult<()> {
            Ok(())
        }
        fn resize(&mut self, _: (u32, u32), _: bool) -> RhiResult<()> {
            Err(RhiError::invalid_argument("resize refused"))
        }
        fn draw(&mut self, _: &mut DrawList, _: (f32, f32)) -> RhiResult<bool> {
            let (permits, changed) = &*self.release;
            let mut permits = permits.lock().unwrap();
            while *permits == 0 {
                permits = changed.wait(permits).unwrap();
            }
            *permits -= 1;
            self.drawn.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(false)
        }
        fn upload_font_atlas(&mut self, _: FontAtlasUpload) -> RhiResult<()> {
            Ok(())
        }
        fn set_presentation_active(&mut self, _: bool) -> RhiResult<()> {
            Ok(())
        }
        fn close(&mut self) -> RhiResult<()> {
            Ok(())
        }
    }

    fn gated() -> (
        ThreadedWindowRenderer,
        Arc<(Mutex<usize>, std::sync::Condvar)>,
        Arc<std::sync::atomic::AtomicUsize>,
    ) {
        let release = Arc::new((Mutex::new(0), std::sync::Condvar::new()));
        let drawn = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let renderer = ThreadedWindowRenderer::spawn(
            Box::new(Gated {
                release: release.clone(),
                drawn: drawn.clone(),
            }),
            "gated",
        );
        (renderer, release, drawn)
    }

    fn allow(release: &(Mutex<usize>, std::sync::Condvar), frames: usize) {
        *release.0.lock().unwrap() += frames;
        release.1.notify_all();
    }

    fn settle(renderer: &mut ThreadedWindowRenderer, frames: usize) -> Vec<FramePresented> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut presented = Vec::new();
        while presented.len() < frames && Instant::now() < deadline {
            presented.extend(renderer.take_presented().unwrap());
            std::thread::yield_now();
        }
        presented
    }

    #[test]
    fn submitting_never_waits_for_a_blocked_gpu_and_drops_past_the_bound() {
        let (mut renderer, release, drawn) = gated();
        let start = Instant::now();
        for _ in 0..MAX_FRAMES_IN_FLIGHT {
            let mut list = DrawList::default();
            assert!(renderer.submit(&mut list, (1.0, 1.0)).unwrap().is_some());
        }
        assert!(!renderer.has_frame_capacity());
        let mut dropped = DrawList::default();
        dropped.pixel_scale = 7.0;
        assert_eq!(renderer.submit(&mut dropped, (1.0, 1.0)).unwrap(), None);
        assert_eq!(dropped.pixel_scale, 7.0, "a dropped frame keeps its list");
        assert!(start.elapsed() < Duration::from_secs(1));
        assert_eq!(drawn.load(std::sync::atomic::Ordering::SeqCst), 0);

        allow(&release, MAX_FRAMES_IN_FLIGHT);
        let presented = settle(&mut renderer, MAX_FRAMES_IN_FLIGHT);
        assert_eq!(presented.len(), MAX_FRAMES_IN_FLIGHT);
        assert!(presented.iter().all(|frame| !frame.presented));
        assert!(renderer.has_frame_capacity());
        renderer.close().unwrap();
    }

    #[test]
    fn a_recycled_list_keeps_settings_that_outlive_one_frame() {
        let (mut renderer, release, _) = gated();
        let mut list = DrawList::default();
        list.geometry_antialiasing = cove_core::GeometryAntialiasing::Analytic;
        list.clip_revision = 9;
        renderer.submit(&mut list, (1.0, 1.0)).unwrap();
        assert_eq!(
            list.geometry_antialiasing,
            cove_core::GeometryAntialiasing::Analytic
        );
        assert_eq!(list.clip_revision, 9);
        allow(&release, 1);
        settle(&mut renderer, 1);
        renderer.close().unwrap();
    }

    #[test]
    fn a_failed_reconfiguration_reaches_the_ui_without_waiting() {
        let (mut renderer, release, _) = gated();
        renderer.resize((1, 1), true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut failure = None;
        while failure.is_none() && Instant::now() < deadline {
            failure = renderer.take_presented().err();
            std::thread::yield_now();
        }
        assert!(failure.is_some());
        allow(&release, 1);
        renderer.close().unwrap();
    }

    #[test]
    fn close_gives_up_on_a_wedged_render_thread() {
        let (mut renderer, _release, _) = gated();
        let mut list = DrawList::default();
        renderer.submit(&mut list, (1.0, 1.0)).unwrap();
        let start = Instant::now();
        assert!(renderer.close().is_err());
        assert!(start.elapsed() < CLOSE_TIMEOUT + Duration::from_secs(1));
    }

    #[test]
    fn the_inline_transport_presents_on_the_calling_thread() {
        let log = Arc::new(Mutex::new(Log::default()));
        let mut renderer = ThreadedWindowRenderer::inline(Box::new(Recording(log.clone())));
        let mut list = DrawList::default();
        renderer.submit(&mut list, (1.0, 1.0)).unwrap();
        assert_eq!(renderer.take_presented().unwrap().len(), 1);
        renderer.close().unwrap();
        assert_eq!(log.lock().unwrap().threads, [std::thread::current().id()]);
    }
}
