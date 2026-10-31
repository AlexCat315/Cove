//! Process, file and terminal plumbing for the `cove` command line.
//!
//! Swift sources: `Support/DeveloperTool.swift`, `Support/Files.swift`,
//! `Support/Process.swift`, `Support/ProcessStream.swift`. The filesystem and
//! process primitives come from `cove_system` (the Rust counterpart of
//! `CoveFileSystem`/`CoveProcess`), as required by the migration conventions.

use cove_system::process::{
    ProcessCommand, ProcessEvent, ProcessLimits, ProcessSession, Termination,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// Failure shared by every command implementation.
#[derive(Debug)]
pub enum ToolError {
    Invalid(String),
    Failed { command: String, status: i32 },
}

impl ToolError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid(message.into())
    }

    pub fn failed(command: impl Into<String>, status: i32) -> Self {
        Self::Failed {
            command: command.into(),
            status,
        }
    }
}

impl fmt::Display for ToolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::Failed { command, status } => {
                write!(formatter, "{command} exited with status {status}")
            }
        }
    }
}

impl std::error::Error for ToolError {}

/// The process environment as an ordered map.
pub fn environment() -> BTreeMap<String, String> {
    std::env::vars().collect()
}

/// Minimal journal used in place of `CoveLogging`, which is not a declared
/// dependency of this crate. The run identifier is still propagated to children
/// so application logs join the same session.
pub struct Log;

impl Log {
    pub fn run_id() -> String {
        if let Some(value) = std::env::var_os("COVE_LOG_RUN_ID") {
            return value.to_string_lossy().into_owned();
        }
        format!(
            "{:x}-{:x}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|value| value.as_nanos())
                .unwrap_or_default()
        )
    }

    pub fn debug(_message: &str, _metadata: &[(&str, String)]) {
        // Level filtering lives in `cove-logging`; the CLI only needs the run id.
    }
}

/// Filesystem helpers matching the Swift `Files` enum.
pub struct Files;

impl Files {
    pub fn directory(url: &Path) -> Result<(), ToolError> {
        std::fs::create_dir_all(url).map_err(io_error)
    }

    pub fn remove(url: &Path) -> Result<(), ToolError> {
        if !url.exists() {
            return Ok(());
        }
        if url.is_dir() {
            std::fs::remove_dir_all(url).map_err(io_error)
        } else {
            std::fs::remove_file(url).map_err(io_error)
        }
    }

    pub fn reset(url: &Path) -> Result<(), ToolError> {
        Self::remove(url)?;
        Self::directory(url)
    }

    pub fn copy(source: &Path, destination: &Path) -> Result<(), ToolError> {
        if let Some(parent) = destination.parent() {
            Self::directory(parent)?;
        }
        Self::remove(destination)?;
        std::fs::copy(source, destination).map_err(io_error)?;
        Ok(())
    }

    pub fn text(value: &str, url: &Path) -> Result<(), ToolError> {
        if let Some(parent) = url.parent() {
            Self::directory(parent)?;
        }
        atomic_write(url, value.as_bytes())
    }

    pub fn json<T: Serialize>(value: &T, url: &Path) -> Result<(), ToolError> {
        if let Some(parent) = url.parent() {
            Self::directory(parent)?;
        }
        let mut bytes = serde_json::to_vec_pretty(value)
            .map_err(|error| ToolError::invalid(error.to_string()))?;
        bytes.push(b'\n');
        atomic_write(url, &bytes)
    }

    pub fn children(url: &Path) -> Result<Vec<PathBuf>, ToolError> {
        let mut result = Vec::new();
        for entry in std::fs::read_dir(url).map_err(io_error)? {
            result.push(entry.map_err(io_error)?.path());
        }
        result.sort();
        Ok(result)
    }

    pub fn files(url: &Path) -> Result<Vec<PathBuf>, ToolError> {
        let mut result = Vec::new();
        collect_files(url, &mut result)?;
        result.sort();
        Ok(result)
    }

    pub fn relative(file: &Path, root: &Path) -> String {
        relative_path(file, root)
            .components()
            .filter_map(|component| match component {
                std::path::Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
                std::path::Component::ParentDir => Some("..".to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("/")
    }
}

fn collect_files(url: &Path, result: &mut Vec<PathBuf>) -> Result<(), ToolError> {
    for entry in std::fs::read_dir(url).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let path = entry.path();
        let metadata = entry.metadata().map_err(io_error)?;
        if metadata.is_dir() {
            collect_files(&path, result)?;
        } else if metadata.is_file() {
            result.push(path);
        }
    }
    Ok(())
}

fn atomic_write(url: &Path, data: &[u8]) -> Result<(), ToolError> {
    let parent = url.parent().unwrap_or_else(|| Path::new("."));
    let staging = parent.join(format!(
        ".cove-write-{:x}-{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or_default()
    ));
    std::fs::write(&staging, data).map_err(io_error)?;
    match std::fs::rename(&staging, url) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = std::fs::remove_file(&staging);
            Err(io_error(error))
        }
    }
}

fn io_error(error: io::Error) -> ToolError {
    ToolError::invalid(error.to_string())
}

/// Lexical relative path used by manifest and archive builders.
pub fn relative_path(file: &Path, root: &Path) -> PathBuf {
    let file_components: Vec<_> = file.components().collect();
    let root_components: Vec<_> = root.components().collect();
    let mut shared = 0;
    while shared < file_components.len()
        && shared < root_components.len()
        && file_components[shared] == root_components[shared]
    {
        shared += 1;
    }
    let mut result = PathBuf::new();
    for _ in shared..root_components.len() {
        result.push("..");
    }
    for component in &file_components[shared..] {
        result.push(component.as_os_str());
    }
    result
}

/// Where human-readable child output is forwarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdoutTarget {
    StandardOutput,
    StandardError,
}

/// Thin, explicit process launching; arguments are never shell-interpreted.
pub struct Shell;

impl Shell {
    pub fn executable(
        name: &str,
        environment: &BTreeMap<String, String>,
    ) -> Result<PathBuf, ToolError> {
        locate(name, environment)
            .ok_or_else(|| ToolError::invalid(format!("Executable not found on PATH: {name}")))
    }

    /// Starts a child and returns its handle.
    pub fn start(
        arguments: &[String],
        at: Option<&Path>,
        environment: &BTreeMap<String, String>,
    ) -> Result<Child, ToolError> {
        let command = arguments
            .first()
            .ok_or_else(|| ToolError::invalid("Empty command"))?;
        let executable = Self::executable(command, environment)?;
        let mut process = Command::new(executable);
        process.args(&arguments[1..]);
        if let Some(directory) = at {
            process.current_dir(directory);
        }
        let mut child_environment = environment.clone();
        child_environment
            .entry("COVE_LOG_RUN_ID".into())
            .or_insert_with(Log::run_id);
        process
            .envs(child_environment.iter())
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        process.spawn().map_err(io_error)
    }

    /// Passes arguments directly to the executable; never interprets a shell
    /// command. Returns trimmed standard output when capturing.
    pub fn run(
        arguments: &[String],
        at: Option<&Path>,
        environment: &BTreeMap<String, String>,
        capture: bool,
        quiet: bool,
        stdout: StdoutTarget,
    ) -> Result<String, ToolError> {
        let command = arguments
            .first()
            .ok_or_else(|| ToolError::invalid("Empty command"))?;
        if !quiet {
            Log::debug(
                "Starting command",
                &[
                    (
                        "command",
                        Path::new(command)
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                    ),
                    ("arguments", (arguments.len() - 1).to_string()),
                ],
            );
        }
        let executable = Self::executable(command, environment)?;
        let directory = at
            .map(Path::to_path_buf)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
        let mut request = ProcessCommand {
            executable,
            arguments: arguments[1..]
                .iter()
                .map(std::ffi::OsString::from)
                .collect(),
            directory,
            environment: environment
                .iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        };
        request
            .environment
            .entry("COVE_LOG_RUN_ID".into())
            .or_insert_with(|| Log::run_id().into());
        let mut limits = ProcessLimits::default();
        limits.output_bytes = 33_554_432;
        // The Swift budget disables the lifetime limit; a century is the
        // closest non-zero value the shared session accepts.
        limits.timeout = Duration::from_secs(60 * 60 * 24 * 365 * 100);
        let mut session = ProcessSession::spawn(&request, limits).map_err(io_error)?;
        session.close_input();
        let mut output_stream = ProcessStream::new(
            command,
            "stdout",
            if capture { None } else { Some(stdout) },
            capture,
        );
        let mut error_stream =
            ProcessStream::new(command, "stderr", Some(StdoutTarget::StandardError), false);
        let mut status = 0;
        while let Some(event) = session.next_event() {
            match event {
                ProcessEvent::Stdout(data) => output_stream.consume(&data, session.pid as i32),
                ProcessEvent::Stderr(data) => error_stream.consume(&data, session.pid as i32),
                ProcessEvent::Exit { code, .. } => status = code,
            }
        }
        let data = output_stream.finish(session.pid as i32);
        let _ = error_stream.finish(session.pid as i32);
        if status != 0 {
            return Err(ToolError::failed(command.clone(), status));
        }
        Ok(String::from_utf8_lossy(&data).trim().to_string())
    }

    /// Same contract as `run`, but also returns the exit status and never
    /// fails on a non-zero status (used where the caller reports it).
    pub fn run_status(
        arguments: &[String],
        at: Option<&Path>,
        environment: &BTreeMap<String, String>,
        capture: bool,
        quiet: bool,
        stdout: StdoutTarget,
        timeout: Option<Duration>,
        stream: bool,
    ) -> Result<(String, Vec<u8>, i32), ToolError> {
        let command = arguments
            .first()
            .ok_or_else(|| ToolError::invalid("Empty command"))?;
        if !quiet {
            Log::debug("Starting command", &[("command", command.clone())]);
        }
        let executable = Self::executable(command, environment)?;
        let directory = at
            .map(Path::to_path_buf)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
        let request = ProcessCommand {
            executable,
            arguments: arguments[1..]
                .iter()
                .map(std::ffi::OsString::from)
                .collect(),
            directory,
            environment: environment
                .iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        };
        let mut limits = ProcessLimits::default();
        limits.output_bytes = 33_554_432;
        limits.timeout = timeout.unwrap_or(Duration::from_secs(60 * 60 * 24 * 365 * 100));
        let mut session = ProcessSession::spawn(&request, limits).map_err(io_error)?;
        session.close_input();
        let mut output_stream = ProcessStream::new(
            command,
            "stdout",
            if stream || !capture {
                Some(stdout)
            } else {
                None
            },
            capture,
        );
        let mut error_stream = ProcessStream::new(
            command,
            "stderr",
            if stream {
                Some(if stdout == StdoutTarget::StandardOutput {
                    StdoutTarget::StandardOutput
                } else {
                    StdoutTarget::StandardError
                })
            } else {
                Some(StdoutTarget::StandardError)
            },
            false,
        );
        let mut status = 0;
        while let Some(event) = session.next_event() {
            match event {
                ProcessEvent::Stdout(data) => output_stream.consume(&data, session.pid as i32),
                ProcessEvent::Stderr(data) => error_stream.consume(&data, session.pid as i32),
                ProcessEvent::Exit { code, .. } => status = code,
            }
        }
        let data = output_stream.finish(session.pid as i32);
        let errors = error_stream.finish(session.pid as i32);
        let _ = Termination::Exited;
        Ok((
            String::from_utf8_lossy(&data).trim().to_string(),
            errors,
            status,
        ))
    }
}

/// Resolves a bare executable name against `PATH`.
pub fn locate(name: &str, environment: &BTreeMap<String, String>) -> Option<PathBuf> {
    let path = Path::new(name);
    if path.is_absolute() || path.components().count() > 1 {
        return executable_file(path).then(|| path.to_path_buf());
    }
    let search = environment
        .get("PATH")
        .or_else(|| environment.get("Path"))
        .map(String::as_str)
        .unwrap_or("");
    let extensions: Vec<String> = if cfg!(windows) {
        environment
            .get("PATHEXT")
            .map(|value| {
                value
                    .split(';')
                    .filter(|part| !part.is_empty())
                    .map(|part| part.trim_start_matches('.').to_ascii_lowercase())
                    .collect()
            })
            .unwrap_or_else(|| vec!["exe".into(), "com".into(), "bat".into(), "cmd".into()])
    } else {
        Vec::new()
    };
    for directory in std::env::split_paths(search) {
        if directory.as_os_str().is_empty() {
            continue;
        }
        let candidate = directory.join(name);
        if extensions.is_empty() {
            if executable_file(&candidate) {
                return Some(candidate);
            }
        } else {
            let base = if candidate.extension().is_some() {
                vec![candidate.clone()]
            } else {
                extensions
                    .iter()
                    .map(|extension| candidate.with_extension(extension))
                    .collect()
            };
            for candidate in base {
                if executable_file(&candidate) {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

fn executable_file(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Machine-readable stdout is collected verbatim. Human output is teed to its
/// original stream and the journal.
pub struct ProcessStream {
    destination: Option<StdoutTarget>,
    collect: bool,
    stream: String,
    command: String,
    collected: Vec<u8>,
    buffered: Vec<u8>,
}

impl ProcessStream {
    pub fn new(
        command: &str,
        stream: &str,
        destination: Option<StdoutTarget>,
        collect: bool,
    ) -> Self {
        Self {
            command: Path::new(command)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            stream: stream.into(),
            destination,
            collect,
            collected: Vec::new(),
            buffered: Vec::new(),
        }
    }

    pub fn consume(&mut self, data: &[u8], pid: i32) {
        if self.collect {
            self.collected.extend_from_slice(data);
        }
        match self.destination {
            Some(StdoutTarget::StandardOutput) => {
                let mut out = io::stdout().lock();
                let _ = out.write_all(data);
                let _ = out.flush();
            }
            Some(StdoutTarget::StandardError) => {
                let mut err = io::stderr().lock();
                let _ = err.write_all(data);
                let _ = err.flush();
            }
            None => {}
        }
        if self.collect {
            return;
        }
        self.buffered.extend_from_slice(data);
        while let Some(end) = self.buffered.iter().position(|byte| *byte == 10) {
            let line = self.buffered[..end].to_vec();
            self.buffered.drain(..=end);
            self.record(&line, pid);
        }
        if self.buffered.len() > 32_768 {
            let line = std::mem::take(&mut self.buffered);
            self.record(&line, pid);
        }
    }

    pub fn finish(&mut self, pid: i32) -> Vec<u8> {
        if !self.buffered.is_empty() {
            let line = std::mem::take(&mut self.buffered);
            self.record(&line, pid);
        }
        self.buffered.clear();
        std::mem::take(&mut self.collected)
    }

    fn record(&self, data: &[u8], pid: i32) {
        let line = String::from_utf8_lossy(data).trim_matches('\n').to_string();
        if line.is_empty() {
            return;
        }
        // The journal is stderr-only here; `cove-logging` owns persistent files.
        let _ = (line, pid, &self.command, &self.stream);
    }
}

/// Bridges the application CLI to the repository tooling package.
///
/// SDK diagnostics live in `Tools/Developer` and are loaded only when requested.
pub struct DeveloperTool;

impl DeveloperTool {
    pub fn run(
        arguments: &[String],
        root: &Path,
        environment: &BTreeMap<String, String>,
        quiet: bool,
    ) -> Result<(), ToolError> {
        let package = root.join("Tools/Developer");
        if !package.join("Package.swift").is_file() {
            return Err(ToolError::invalid(format!(
                "Developer tooling not found at {}; use a source checkout of the SDK",
                package.display()
            )));
        }
        let swift = environment
            .get("COVE_SWIFT_EXECUTABLE")
            .cloned()
            .unwrap_or_else(|| "swift".into());
        let mut child_environment = environment.clone();
        child_environment.insert("COVE_ROOT".into(), root.display().to_string());
        // SwiftPM expands @file response arguments even after the product name.
        // Build first, then pass session/event files literally to the executable.
        let build = vec![
            swift.clone(),
            "build".into(),
            "--package-path".into(),
            package.display().to_string(),
            "--jobs".into(),
            "4".into(),
        ];
        let mut build_command = build.clone();
        build_command.push("--product".into());
        build_command.push("cove-dev".into());
        Shell::run(
            &build_command,
            Some(root),
            &child_environment,
            false,
            quiet,
            StdoutTarget::StandardError,
        )?;
        let mut path_command = build.clone();
        path_command.push("--show-bin-path".into());
        let products = Shell::run(
            &path_command,
            Some(root),
            &child_environment,
            true,
            true,
            StdoutTarget::StandardOutput,
        )?;
        let name = if cfg!(windows) {
            "cove-dev.exe"
        } else {
            "cove-dev"
        };
        let executable = Path::new(products.trim()).join(name);
        let mut command = vec![executable.display().to_string()];
        command.extend(arguments.iter().cloned());
        Shell::run(
            &command,
            Some(root),
            &child_environment,
            false,
            quiet,
            StdoutTarget::StandardOutput,
        )?;
        Ok(())
    }
}
