use crate::native;
use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    io,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU8, AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct ProcessCommand {
    pub executable: PathBuf,
    pub arguments: Vec<OsString>,
    pub directory: PathBuf,
    pub environment: BTreeMap<OsString, OsString>,
}
impl ProcessCommand {
    /// Resolves PATH at construction so launch never depends on implicit OS search rules.
    pub fn new(executable: impl AsRef<OsStr>) -> io::Result<Self> {
        let environment = std::env::vars_os().collect();
        let directory = std::env::current_dir()?;
        let executable = locate(executable.as_ref(), &environment, &directory)?;
        Ok(Self {
            executable,
            arguments: vec![],
            directory,
            environment,
        })
    }
    pub fn arg(mut self, argument: impl Into<OsString>) -> Self {
        self.arguments.push(argument.into());
        self
    }
}

fn env_value<'a>(
    environment: &'a BTreeMap<OsString, OsString>,
    name: &str,
) -> Option<&'a OsString> {
    environment
        .iter()
        .find(|(key, _)| {
            if cfg!(windows) {
                key.to_string_lossy().eq_ignore_ascii_case(name)
            } else {
                key.as_os_str() == OsStr::new(name)
            }
        })
        .map(|(_, value)| value)
}

pub fn locate(
    name: &OsStr,
    environment: &BTreeMap<OsString, OsString>,
    directory: &Path,
) -> io::Result<PathBuf> {
    let name = Path::new(name);
    let mut candidates = Vec::new();
    if name.is_absolute() || name.components().count() > 1 {
        candidates.push(if name.is_absolute() {
            name.to_owned()
        } else {
            directory.join(name)
        });
    } else if let Some(path) = env_value(environment, "PATH") {
        for parent in std::env::split_paths(path) {
            let parent = if parent.is_absolute() {
                parent
            } else {
                directory.join(parent)
            };
            candidates.push(parent.join(name));
        }
    }
    for candidate in candidates {
        let mut alternatives = vec![candidate.clone()];
        if cfg!(windows) && candidate.extension().is_none() {
            for extension in ["exe", "com"] {
                alternatives.push(candidate.with_extension(extension));
            }
        }
        for candidate in alternatives {
            if !candidate.is_file() {
                continue;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if std::fs::metadata(&candidate)?.permissions().mode() & 0o111 == 0 {
                    continue;
                }
            }
            return candidate.canonicalize();
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("executable not found: {}", name.display()),
    ))
}

#[derive(Clone, Debug)]
pub struct ProcessLimits {
    pub timeout: Duration,
    pub output_bytes: usize,
    pub input_bytes: usize,
    pub queued_chunks: usize,
}
impl Default for ProcessLimits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            output_bytes: 16 * 1024 * 1024,
            input_bytes: 1024 * 1024,
            queued_chunks: 128,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Termination {
    Exited,
    Cancelled,
    TimedOut,
    OutputLimit,
    IoError,
}
#[derive(Debug)]
pub enum ProcessEvent {
    Stdout(Vec<u8>),
    Stderr(Vec<u8>),
    Exit { code: i32, reason: Termination },
}
#[derive(Debug)]
pub struct ProcessOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub code: i32,
    pub reason: Termination,
}

/// One event consumer. stdout/stderr drain concurrently into a bounded channel.
/// Drop cancels the process tree before releasing its native resources.
pub struct ProcessSession {
    pub pid: u32,
    input: Option<os_pipe::PipeWriter>,
    events: Option<Receiver<ProcessEvent>>,
    control: Arc<AtomicU8>,
    supervisor: Option<JoinHandle<()>>,
    input_remaining: usize,
}

impl ProcessSession {
    pub fn spawn(command: &ProcessCommand, limits: ProcessLimits) -> io::Result<Self> {
        if !command.executable.is_absolute()
            || !command.directory.is_absolute()
            || limits.timeout.is_zero()
            || limits.queued_chunks == 0
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "use absolute paths and nonzero process limits",
            ));
        }
        #[cfg(windows)]
        {
            if command.executable.extension().is_some_and(|s| {
                s.to_string_lossy().eq_ignore_ascii_case("cmd")
                    || s.to_string_lossy().eq_ignore_ascii_case("bat")
            }) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "batch files must be invoked through an explicit command interpreter",
                ));
            }
        }
        let spawned = native::spawn(command)?;
        let pid = spawned.child.pid();
        let (sender, events) = mpsc::sync_channel(limits.queued_chunks);
        let control = Arc::new(AtomicU8::new(0));
        let signal = control.clone();
        let input_remaining = limits.input_bytes;
        let supervisor = thread::spawn(move || {
            let mut child = spawned.child;
            let bytes = Arc::new(AtomicUsize::new(0));
            let mut readers = Vec::new();
            for (pipe, stderr) in [(spawned.output, false), (spawned.errors, true)] {
                let sender = sender.clone();
                let control = signal.clone();
                let bytes = bytes.clone();
                let limit = limits.output_bytes;
                readers.push(thread::spawn(move || {
                    drain(pipe, stderr, sender, control, bytes, limit)
                }));
            }
            let started = Instant::now();
            let (code, reason) = loop {
                let signal_value = signal.load(Ordering::Acquire);
                let reason = match signal_value {
                    1 => Some(Termination::Cancelled),
                    2 => Some(Termination::OutputLimit),
                    3 => Some(Termination::IoError),
                    _ if started.elapsed() >= limits.timeout => Some(Termination::TimedOut),
                    _ => None,
                };
                if let Some(reason) = reason {
                    let _ = signal.compare_exchange(0, 4, Ordering::AcqRel, Ordering::Acquire);
                    child.terminate();
                    break (child.wait().unwrap_or(1), reason);
                }
                match child.try_wait() {
                    Ok(Some(code)) if readers.iter().all(JoinHandle::is_finished) => {
                        break (code, Termination::Exited);
                    }
                    Ok(_) => {}
                    Err(_) => {
                        child.terminate();
                        break (child.wait().unwrap_or(1), Termination::IoError);
                    }
                }
                thread::sleep(Duration::from_millis(5));
            };
            // Parent exit is not enough: descendants can still own inherited pipes.
            child.terminate();
            for reader in readers {
                let _ = reader.join();
            }
            let _ = sender.send(ProcessEvent::Exit { code, reason });
        });
        Ok(Self {
            pid,
            input: Some(spawned.input),
            events: Some(events),
            control,
            supervisor: Some(supervisor),
            input_remaining,
        })
    }
    pub fn write_input(&mut self, bytes: &[u8]) -> io::Result<()> {
        if bytes.len() > self.input_remaining {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "process input limit exceeded",
            ));
        }
        self.input_remaining -= bytes.len();
        let input = self
            .input
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "stdin is closed"))?;
        native::write_pipe(input, bytes, &self.control)
    }
    pub fn close_input(&mut self) {
        self.input.take();
    }
    pub fn cancel(&self) {
        let _ = self
            .control
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire);
    }
    pub fn next_event(&self) -> Option<ProcessEvent> {
        self.events.as_ref()?.recv().ok()
    }
    pub fn collect(mut self) -> io::Result<ProcessOutput> {
        self.close_input();
        let mut output = ProcessOutput {
            stdout: vec![],
            stderr: vec![],
            code: 1,
            reason: Termination::IoError,
        };
        while let Some(event) = self.next_event() {
            match event {
                ProcessEvent::Stdout(bytes) => output.stdout.extend(bytes),
                ProcessEvent::Stderr(bytes) => output.stderr.extend(bytes),
                ProcessEvent::Exit { code, reason } => {
                    output.code = code;
                    output.reason = reason;
                    return Ok(output);
                }
            }
        }
        Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "process supervisor disconnected",
        ))
    }
}
impl Drop for ProcessSession {
    fn drop(&mut self) {
        self.cancel();
        self.input.take();
        self.events.take();
        if let Some(supervisor) = self.supervisor.take() {
            let _ = supervisor.join();
        }
    }
}

fn drain(
    mut pipe: os_pipe::PipeReader,
    stderr: bool,
    sender: SyncSender<ProcessEvent>,
    control: Arc<AtomicU8>,
    bytes: Arc<AtomicUsize>,
    limit: usize,
) {
    let mut buffer = [0_u8; 8192];
    loop {
        match native::read_pipe(&mut pipe, &mut buffer, &control) {
            Ok(0) => break,
            Ok(n) => {
                let used = bytes.fetch_add(n, Ordering::Relaxed);
                let allowed = n.min(limit.saturating_sub(used));
                if allowed != n {
                    let _ = control.compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire);
                }
                if allowed > 0 {
                    let data = buffer[..allowed].to_vec();
                    if sender
                        .send(if stderr {
                            ProcessEvent::Stderr(data)
                        } else {
                            ProcessEvent::Stdout(data)
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                if allowed != n {
                    break;
                }
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => {
                let _ = control.compare_exchange(0, 3, Ordering::AcqRel, Ordering::Acquire);
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    fn fixture(mode: &str) -> ProcessCommand {
        let executable = std::env::current_exe().unwrap();
        let mut command = ProcessCommand::new(executable).unwrap();
        command.arguments = vec![
            "--exact".into(),
            "process::tests::process_fixture".into(),
            "--nocapture".into(),
        ];
        command
            .environment
            .insert("COVE_RUST_PROCESS_FIXTURE".into(), mode.into());
        command
    }
    #[test]
    #[allow(
        clippy::zombie_processes,
        reason = "the fixture intentionally exits before its descendant to exercise pipe-drain cancellation"
    )]
    fn process_fixture() {
        let Some(mode) = std::env::var_os("COVE_RUST_PROCESS_FIXTURE") else {
            return;
        };
        match mode.to_str().unwrap() {
            "echo" => {
                let mut input = String::new();
                io::stdin().read_to_string(&mut input).unwrap();
                print!("out:{input}");
                eprint!("err:{input}");
            }
            "flood" => {
                for _ in 0..10000 {
                    println!("{}", "x".repeat(100));
                    eprintln!("{}", "y".repeat(100));
                }
            }
            "sleep" => thread::sleep(Duration::from_secs(60)),
            "descendant" => {
                let mut child = std::process::Command::new(std::env::current_exe().unwrap());
                child
                    .args(["--exact", "process::tests::process_fixture", "--nocapture"])
                    .env("COVE_RUST_PROCESS_FIXTURE", "sleep");
                let _ = child.spawn().unwrap();
            }
            _ => panic!("unknown fixture"),
        }
    }
    #[test]
    fn drains_both_streams_and_closes_stdin() {
        let mut session =
            ProcessSession::spawn(&fixture("echo"), ProcessLimits::default()).unwrap();
        session.write_input("你好🦀".as_bytes()).unwrap();
        let output = session.collect().unwrap();
        assert_eq!((output.code, output.reason), (0, Termination::Exited));
        assert!(
            String::from_utf8(output.stdout)
                .unwrap()
                .contains("out:你好🦀")
        );
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("err:你好🦀")
        );
    }
    #[test]
    fn enforces_total_timeout_and_reaps_descendant_pipes() {
        for mode in ["sleep", "descendant"] {
            let start = Instant::now();
            let result = ProcessSession::spawn(
                &fixture(mode),
                ProcessLimits {
                    timeout: Duration::from_millis(150),
                    ..Default::default()
                },
            )
            .unwrap()
            .collect()
            .unwrap();
            assert_eq!(result.reason, Termination::TimedOut);
            assert!(start.elapsed() < Duration::from_secs(3));
        }
    }
    #[test]
    fn bounds_output_and_drop_cancels_without_consumer() {
        let result = ProcessSession::spawn(
            &fixture("flood"),
            ProcessLimits {
                output_bytes: 4096,
                queued_chunks: 2,
                ..Default::default()
            },
        )
        .unwrap()
        .collect()
        .unwrap();
        assert_eq!(result.reason, Termination::OutputLimit);
        assert!(result.stdout.len() + result.stderr.len() <= 4096);
        let start = Instant::now();
        let session = ProcessSession::spawn(
            &fixture("flood"),
            ProcessLimits {
                queued_chunks: 1,
                ..Default::default()
            },
        )
        .unwrap();
        thread::sleep(Duration::from_millis(30));
        drop(session);
        assert!(start.elapsed() < Duration::from_secs(3));
    }
}
