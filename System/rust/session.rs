//! Translation of `System/Sources/CoveProcess/Session.swift`.
//!
//! One native process lifetime shared by LSP, tasks and extension imports.
//! Pipe readers drain concurrently; queued writes never block the caller.
use super::command::{ProcessBudget, ProcessCommand, ProcessError, ProcessEvent};
use super::event_buffer::{Failure, ProcessEventBuffer, ProcessFailure};
use super::io_state::{ProcessIOState, ProcessTimer};
use super::native_process::NativeProcess;
use super::pipe_read::{PipeRead, PipeReadError};
use super::platform::ProcessPlatform;
use os_pipe::{PipeReader, PipeWriter};
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const READ_CHUNK: usize = 65_536;

enum WriterJob {
    Write(Vec<u8>),
    Close,
}

pub struct ProcessSession {
    pub id: String,
    core: Arc<Core>,
    writer: Sender<WriterJob>,
}

struct Core {
    events: Arc<ProcessEventBuffer>,
    state: ProcessIOState,
    process: Arc<NativeProcess>,
    budget: ProcessBudget,
    writer_closed: AtomicBool,
}

impl Core {
    fn finish(self: &Arc<Self>, status: i32) {
        let _ = self.state.stop();
        self.state.cancel_timer();
        let _ = self.events.append(ProcessEvent::Exited(status));
        self.events.finish();
    }

    fn terminated(self: &Arc<Self>, status: i32) {
        if self.state.terminated(status) {
            self.finish(status);
        }
    }

    fn fail(self: &Arc<Self>, error: ProcessFailure) {
        if !self.state.stop() {
            return;
        }
        self.events.finish_with(Some(Arc::new(error)));
        self.state.cancel_timer();
        self.process.terminate(false);
        let process = self.process.clone();
        let _ = std::thread::Builder::new()
            .name("cove.process.terminate".into())
            .spawn(move || {
                std::thread::sleep(Duration::from_millis(300));
                process.terminate(true);
            });
    }

    fn closed(self: &Arc<Self>, is_error: bool) {
        let _ = self.events.append(if is_error {
            ProcessEvent::StderrClosed
        } else {
            ProcessEvent::StdoutClosed
        });
        if let Some(status) = self.state.closed_pipe() {
            self.finish(status);
        }
    }

    fn install_deadline(self: &Arc<Self>) {
        let Some(milliseconds) = self.budget.lifetime_milliseconds else {
            return;
        };
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let core = self.clone();
        let handle = std::thread::Builder::new()
            .name("cove.process.deadline".into())
            .spawn(move || {
                let deadline = Instant::now() + Duration::from_millis(milliseconds);
                while Instant::now() < deadline {
                    if flag.load(Ordering::Acquire) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                if !flag.load(Ordering::Acquire) {
                    core.fail(ProcessError::TimedOut.into());
                }
            });
        if let Ok(handle) = handle {
            self.state.install_timer(ProcessTimer::new(cancelled, handle));
        }
    }
}

impl ProcessSession {
    pub fn new(command: &ProcessCommand, budget: ProcessBudget) -> Result<Self, ProcessFailure> {
        Self::validate(command, &budget)?;
        let events = Arc::new(ProcessEventBuffer::new(budget.output_bytes));
        let process = Arc::new(NativeProcess::new());
        let core = Arc::new(Core {
            events: events.clone(),
            state: ProcessIOState::new(),
            process: process.clone(),
            budget: budget.clone(),
            writer_closed: AtomicBool::new(false),
        });

        let callback_core = core.clone();
        let pipes = process.start(
            command,
            Arc::new(move |status| callback_core.terminated(status)),
        )?;
        ProcessPlatform::prepare_input(&pipes.input);
        Self::install_reader(core.clone(), pipes.output, false);
        Self::install_reader(core.clone(), pipes.errors, true);
        core.install_deadline();

        let (writer, jobs) = mpsc::channel();
        Self::install_writer(core.clone(), pipes.input, jobs);
        Ok(Self {
            id: unique_identifier(),
            core,
            writer,
        })
    }

    fn validate(command: &ProcessCommand, budget: &ProcessBudget) -> Result<(), ProcessFailure> {
        if command.executable.as_os_str().is_empty()
            || path_contains_nul(&command.executable)
            || path_contains_nul(&command.directory)
            || command.arguments.iter().any(|value| value.contains('\0'))
            || command.environment.iter().any(|(key, value)| {
                key.contains('=') || key.contains('\0') || value.contains('\0')
            })
        {
            return Err(ProcessError::InvalidCommand.into());
        }
        let maximum = ProcessBudget::MAXIMUM_BYTES;
        if budget.output_bytes == 0
            || budget.output_bytes > maximum
            || budget.pending_input_bytes == 0
            || budget.pending_input_bytes > maximum
            || budget
                .lifetime_milliseconds
                .is_some_and(|milliseconds| milliseconds == 0 || milliseconds > 86_400_000)
        {
            return Err(ProcessError::InvalidBudget.into());
        }
        Ok(())
    }

    fn install_reader(core: Arc<Core>, mut handle: PipeReader, is_error: bool) {
        let worker = core.clone();
        let _ = std::thread::Builder::new()
            .name(if is_error {
                "cove.process.stderr".into()
            } else {
                "cove.process.stdout".into()
            })
            .spawn(move || {
                loop {
                    match PipeRead::available(&mut handle, READ_CHUNK) {
                        Ok(data) if data.is_empty() => break,
                        Ok(data) => {
                            let event = if is_error {
                                ProcessEvent::Stderr(data)
                            } else {
                                ProcessEvent::Stdout(data)
                            };
                            if !core.events.append(event) {
                                core.fail(ProcessError::OutputLimit.into());
                                return;
                            }
                        }
                        Err(PipeReadError::Process(error)) => {
                            core.fail(error.into());
                            return;
                        }
                        Err(PipeReadError::Io(error)) => {
                            core.fail(ProcessFailure::Io(error));
                            return;
                        }
                    }
                }
                // The reader owns its handle; it closes when the blocking read
                // reaches EOF, which avoids racing descriptor access on
                // teardown.
                worker.closed(is_error);
            });
    }

    fn install_writer(core: Arc<Core>, mut handle: PipeWriter, jobs: mpsc::Receiver<WriterJob>) {
        let _ = std::thread::Builder::new()
            .name("cove.process.input".into())
            .spawn(move || {
                for job in jobs {
                    match job {
                        WriterJob::Write(data) => {
                            let count = data.len();
                            let result = ProcessPlatform::write(&data, &mut handle);
                            core.state.wrote(count);
                            if let Err(error) = result {
                                core.fail(ProcessFailure::Io(error));
                                return;
                            }
                        }
                        WriterJob::Close => break,
                    }
                }
                drop(handle);
            });
    }

    pub fn is_running(&self) -> bool {
        self.core.process.is_running()
    }

    pub fn process_identifier(&self) -> i32 {
        self.core.process.process_identifier()
    }

    pub fn write(&self, data: &[u8]) -> Result<(), ProcessFailure> {
        self.core
            .state
            .enqueue(data.len(), self.core.budget.pending_input_bytes)?;
        if self.writer.send(WriterJob::Write(data.to_vec())).is_err() {
            self.core.state.wrote(data.len());
            return Err(ProcessFailure::Io(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "process input writer stopped",
            )));
        }
        Ok(())
    }

    pub fn close_input(&self) {
        self.core.state.close_input();
        let _ = self.writer.send(WriterJob::Close);
    }

    pub fn cancel(&self) {
        self.core.fail(ProcessFailure::Cancelled);
    }

    pub fn next_event(&self, until: Option<Instant>) -> Result<Option<ProcessEvent>, Failure> {
        self.core.events.next_synchronously(until)
    }

    pub fn events(&self) -> ProcessEventStream {
        ProcessEventStream {
            buffer: self.core.events.clone(),
        }
    }
}

impl Drop for ProcessSession {
    fn drop(&mut self) {
        if !self.core.writer_closed.swap(true, Ordering::AcqRel) {
            self.close_input();
        }
        self.core.state.cancel_timer();
        let _ = self.core.state.stop();
        self.core.events.finish();
        self.core.process.terminate(true);
    }
}

/// Blocking replacement for the Swift `AsyncThrowingStream`.
pub struct ProcessEventStream {
    buffer: Arc<ProcessEventBuffer>,
}

impl Iterator for ProcessEventStream {
    type Item = Result<ProcessEvent, Failure>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.buffer.next() {
            Ok(Some(event)) => Some(Ok(event)),
            Ok(None) => None,
            Err(error) => Some(Err(error)),
        }
    }
}

fn path_contains_nul(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().contains(&0)
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        path.as_os_str().encode_wide().any(|unit| unit == 0)
    }
    #[cfg(not(any(unix, windows)))]
    {
        path.to_string_lossy().contains('\0')
    }
}

fn unique_identifier() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or_default();
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = u64::from(std::process::id());
    let high = nanos ^ (pid << 32) ^ counter.rotate_left(17);
    let low = nanos.rotate_left(23) ^ counter.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ pid;
    format!(
        "{:08x}-{:04x}-4{:03x}-a{:03x}-{:012x}",
        (high >> 32) as u32,
        (high >> 16) as u16,
        high as u16 & 0x0fff,
        (low >> 48) as u16 & 0x0fff,
        low & 0x0000_ffff_ffff_ffff
    )
}
