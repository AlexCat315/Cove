//! Child-process lifecycle with hard bounds and process-tree cleanup.
use crate::error::{DebuggerError, Result};
use std::io::Read;
use std::path::Path;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Output kept from a short diagnostic command.
const CAPTURE_LIMIT: usize = 1024 * 1024;
/// Stderr kept from a debug adapter, used to explain failures.
pub(crate) const STDERR_LIMIT: usize = 16 * 1024;
const TERM_GRACE: Duration = Duration::from_millis(400);
const KILL_GRACE: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub(crate) struct CapturedOutput {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

fn spawn_error(program: &Path, error: std::io::Error) -> DebuggerError {
    DebuggerError::new(
        "process_spawn",
        format!("Cannot start {}: {error}", program.display()),
    )
}

fn command(program: &Path) -> Command {
    let mut command = Command::new(program);
    command.stdin(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(CREATE_NEW_PROCESS_GROUP);
    }
    command
}

fn read_bounded(mut reader: impl Read + Send + 'static, limit: usize) -> JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut chunk = [0u8; 4096];
        while bytes.len() < limit {
            match reader.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(read) => {
                    let keep = (limit - bytes.len()).min(read);
                    bytes.extend_from_slice(&chunk[..keep]);
                }
            }
        }
        bytes
    })
}

fn wait_until(child: &mut Child, until: Instant) -> Option<ExitStatus> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) if Instant::now() < until => thread::sleep(Duration::from_millis(5)),
            Ok(None) | Err(_) => return None,
        }
    }
}

/// Runs a short diagnostic command with a hard timeout and bounded output.
///
/// A command that overruns is killed with its process group; it never blocks
/// the caller past `timeout`.
pub(crate) fn run_capture(
    program: &Path,
    args: &[&str],
    timeout: Duration,
) -> Result<CapturedOutput> {
    let mut child = command(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| spawn_error(program, error))?;
    let stdout = child
        .stdout
        .take()
        .map(|io| read_bounded(io, CAPTURE_LIMIT));
    let stderr = child
        .stderr
        .take()
        .map(|io| read_bounded(io, CAPTURE_LIMIT));
    let deadline = Instant::now() + timeout;
    let status = wait_until(&mut child, deadline);
    let timed_out = status.is_none();
    if timed_out {
        terminate_tree(&mut child);
    }
    let status = child
        .try_wait()
        .ok()
        .flatten()
        .or_else(|| wait_until(&mut child, Instant::now() + Duration::from_millis(200)))
        .or(status)
        .ok_or_else(|| DebuggerError::new("process_status", "The process has no exit status."))?;
    let stdout = stdout
        .and_then(|handle| handle.join().ok())
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default();
    let stderr = stderr
        .and_then(|handle| handle.join().ok())
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default();
    Ok(CapturedOutput {
        status,
        stdout,
        stderr,
        timed_out,
    })
}

/// The adapter process and its stdio pipes.
pub(crate) struct AdapterProcess {
    child: Child,
    pid: u32,
    terminated: bool,
}

pub(crate) struct AdapterIo {
    pub stdin: ChildStdin,
    pub stdout: ChildStdout,
    pub stderr: ChildStderr,
}

impl AdapterProcess {
    pub(crate) fn spawn(program: &Path) -> Result<(Self, AdapterIo)> {
        let mut child = command(program)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| spawn_error(program, error))?;
        let pid = child.id();
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| DebuggerError::new("process_io", "The adapter has no stdin."))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| DebuggerError::new("process_io", "The adapter has no stdout."))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| DebuggerError::new("process_io", "The adapter has no stderr."))?;
        Ok((
            Self {
                child,
                pid,
                terminated: false,
            },
            AdapterIo {
                stdin,
                stdout,
                stderr,
            },
        ))
    }

    pub(crate) fn pid(&self) -> u32 {
        self.pid
    }

    /// Ends the adapter and every process it started.
    ///
    /// The target is not part of this tree; detaching is the session's job.
    pub(crate) fn terminate(&mut self) {
        if self.terminated {
            return;
        }
        self.terminated = true;
        terminate_tree(&mut self.child);
    }
}

impl Drop for AdapterProcess {
    fn drop(&mut self) {
        self.terminate();
    }
}

/// Collects `(pid, ppid)` pairs from the platform process table.
#[cfg(unix)]
fn process_table() -> Vec<(u32, u32)> {
    let Ok(output) = run_capture(
        Path::new("ps"),
        &["-axo", "pid=,ppid="],
        Duration::from_secs(2),
    ) else {
        return Vec::new();
    };
    output
        .stdout
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let pid = parts.next()?.parse().ok()?;
            let ppid = parts.next()?.parse().ok()?;
            Some((pid, ppid))
        })
        .collect()
}

#[cfg(unix)]
fn descendants(root: u32) -> Vec<u32> {
    let table = process_table();
    let mut found = Vec::new();
    let mut frontier = vec![root];
    while let Some(parent) = frontier.pop() {
        for (pid, ppid) in &table {
            if *ppid == parent && !found.contains(pid) {
                found.push(*pid);
                frontier.push(*pid);
            }
        }
    }
    found
}

#[cfg(unix)]
fn signal(pid: u32, signal: i32) {
    if pid <= 1 || pid > i32::MAX as u32 {
        return;
    }
    // SAFETY: `kill` takes plain integers; a stale PID only yields ESRCH.
    unsafe {
        libc::kill(pid as i32, signal);
    }
}

#[cfg(unix)]
fn signal_group(pgid: u32, signal_number: i32) {
    if pgid <= 1 || pgid > i32::MAX as u32 {
        return;
    }
    // SAFETY: the adapter is started with `process_group(0)`, so its PGID is
    // its PID and a negative target cannot reach an unrelated group.
    unsafe {
        libc::kill(-(pgid as i32), signal_number);
    }
}

#[cfg(unix)]
fn terminate_unix(child: &mut Child) {
    let pid = child.id();
    let mut targets = descendants(pid);
    targets.push(pid);
    // The adapter is its own group leader. Its helpers (debugserver) may have
    // their own groups, so they are signalled individually as well.
    signal(pid, libc::SIGTERM);
    signal_group(pid, libc::SIGTERM);
    for target in &targets {
        signal(*target, libc::SIGTERM);
    }
    let until = Instant::now() + TERM_GRACE;
    while Instant::now() < until {
        if child.try_wait().ok().flatten().is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    signal(pid, libc::SIGKILL);
    signal_group(pid, libc::SIGKILL);
    for target in &targets {
        signal(*target, libc::SIGKILL);
    }
    let _ = child.kill();
    let _ = wait_until(child, Instant::now() + KILL_GRACE);
    // A helper that outlived its parent is reparented; kill the recorded PIDs.
    for target in targets {
        signal(target, libc::SIGKILL);
    }
}

#[cfg(windows)]
fn terminate_windows(child: &mut Child) {
    let pid = child.id();
    let _ = run_capture(
        Path::new("taskkill"),
        &["/PID", &pid.to_string(), "/T", "/F"],
        Duration::from_secs(3),
    );
    let _ = child.kill();
    let _ = wait_until(child, Instant::now() + KILL_GRACE);
}

fn terminate_tree(child: &mut Child) {
    #[cfg(unix)]
    terminate_unix(child);
    #[cfg(windows)]
    terminate_windows(child);
    #[cfg(not(any(unix, windows)))]
    {
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// Keeps the tail of a stream without growing without bound.
pub(crate) fn read_stderr_tail(mut reader: ChildStderr) -> (Arc<Mutex<Vec<u8>>>, JoinHandle<()>) {
    let tail = Arc::new(Mutex::new(Vec::new()));
    let shared = Arc::clone(&tail);
    let handle = thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        while let Ok(read) = reader.read(&mut chunk) {
            if read == 0 {
                break;
            }
            let mut buffer = shared.lock().unwrap();
            buffer.extend_from_slice(&chunk[..read]);
            if buffer.len() > STDERR_LIMIT {
                let drop = buffer.len() - STDERR_LIMIT;
                buffer.drain(..drop);
            }
        }
    });
    (tail, handle)
}

/// Waits briefly for a background reader to finish after its stream closed.
pub(crate) fn join_bounded(handle: JoinHandle<()>, timeout: Duration) {
    let until = Instant::now() + timeout;
    while !handle.is_finished() && Instant::now() < until {
        thread::sleep(Duration::from_millis(5));
    }
    if handle.is_finished() {
        let _ = handle.join();
    }
}
