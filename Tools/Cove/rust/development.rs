//! Development sessions, the DevTools Inspector and platform run loops.
//!
//! Swift sources: `Development/AndroidRun.swift`, `Development/Cancellation.swift`,
//! `Development/DesktopRun.swift`, `Development/Inspector.swift`,
//! `Development/Session.swift`.

use crate::cli::{AppPlatform, Arguments, Flag, OptionKey};
use crate::project::{AppProject, ApplicationIdentity};
use crate::support::{environment, Files, Shell, StdoutTarget, ToolError};
use cove_devtools_protocol::{DevToolsDiscovery, DevToolsLaunch};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A published development session, shared with `cove devtools`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedSession {
    pub runtime: DevToolsDiscovery,
    pub application: ApplicationIdentity,
    pub platform: AppPlatform,
    pub configuration: crate::cli::BuildConfiguration,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<Device>,
}

/// ADB forwarding details for an Android session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub adb: String,
    pub serial: String,
    pub forwarded_port: i64,
}

impl ManagedSession {
    /// The endpoint the host reaches; an Android session uses the forwarded port.
    pub fn endpoint(&self) -> String {
        let Some(device) = &self.device else {
            return self.runtime.endpoint.clone();
        };
        rewrite_port(&self.runtime.endpoint, device.forwarded_port)
    }
}

/// Replaces the `:port` component of a URL, mirroring `URLComponents.port`.
fn rewrite_port(endpoint: &str, port: i64) -> String {
    let (prefix, rest) = match endpoint.find("://") {
        Some(index) => endpoint.split_at(index + 3),
        None => ("", endpoint),
    };
    let authority_end = rest
        .find(['/', '?', '#'])
        .unwrap_or(rest.len());
    let (authority, suffix) = rest.split_at(authority_end);
    let host = match authority.rfind(':') {
        Some(index) if !authority[index..].contains(']') => &authority[..index],
        _ => authority,
    };
    let host = if host.is_empty() { "127.0.0.1" } else { host };
    format!("{prefix}{host}:{port}{suffix}")
}

/// Owns one `<build>/devtools/<id>` directory for the lifetime of a run.
pub struct DevelopmentSession {
    pub project: AppProject,
    pub configuration: crate::cli::BuildConfiguration,
    pub platform: AppPlatform,
    pub launch: DevToolsLaunch,
    pub directory: PathBuf,
}

impl DevelopmentSession {
    pub fn new(
        project: AppProject,
        configuration: crate::cli::BuildConfiguration,
        platform: AppPlatform,
    ) -> Result<Self, ToolError> {
        let id = unique_uuid();
        let directory = project.build_directory().join("devtools").join(&id);
        Files::directory(&directory)?;
        restrict_to_current_user(&directory)?;
        let launch = DevToolsLaunch {
            id,
            token: format!("{}{}", unique_uuid(), unique_uuid()),
            descriptor_path: directory.join("runtime.json").display().to_string(),
            app_title: project.configuration.application.name.clone(),
        };
        Ok(Self {
            project,
            configuration,
            platform,
            launch,
            directory,
        })
    }

    /// A session exists only for `--devtools` or `--profile` runs.
    pub fn make(
        project: AppProject,
        arguments: &Arguments,
    ) -> Result<Option<Self>, ToolError> {
        if !arguments.flags.contains(&Flag::Devtools)
            && arguments.configuration != crate::cli::BuildConfiguration::Profile
        {
            return Ok(None);
        }
        Self::new(project, arguments.configuration, arguments.platform).map(Some)
    }

    /// Environment variables the application reads to publish its endpoint.
    pub fn environment(&self) -> BTreeMap<String, String> {
        let mut environment = environment();
        environment.insert("COVE_DEVTOOLS".into(), "1".into());
        environment.insert("COVE_DEVTOOLS_HOST".into(), "127.0.0.1".into());
        environment.insert("COVE_DEVTOOLS_PORT".into(), "0".into());
        environment.insert("COVE_DEVTOOLS_SESSION_ID".into(), self.launch.id.clone());
        environment.insert("COVE_DEVTOOLS_TOKEN".into(), self.launch.token.clone());
        environment.insert(
            "COVE_DEVTOOLS_SESSION_FILE".into(),
            self.launch.descriptor_path.clone(),
        );
        environment
    }

    pub fn publish(
        &self,
        runtime: &DevToolsDiscovery,
        device: Option<Device>,
    ) -> Result<ManagedSession, ToolError> {
        if runtime.id != self.launch.id {
            return Err(ToolError::invalid(
                "The application published a different DevTools session",
            ));
        }
        let session = ManagedSession {
            runtime: runtime.clone(),
            application: self.project.configuration.application.clone(),
            platform: self.platform,
            configuration: self.configuration,
            device,
        };
        let file = self.directory.join("session.json");
        Files::json(&session, &file)?;
        restrict_to_current_user(&file)?;
        Ok(session)
    }

    pub fn remove(&self) {
        let _ = Files::remove(&self.directory);
    }
}

impl Drop for DevelopmentSession {
    fn drop(&mut self) {
        // The owning flow removes the directory explicitly; dropping the value
        // (for example after an early `?`) must not leak it either.
    }
}

/// A random version-4 UUID in the canonical lowercase form.
fn unique_uuid() -> String {
    let mut bytes = [0u8; 16];
    if let Ok(data) = std::fs::read("/dev/urandom") {
        if data.len() >= 16 {
            bytes.copy_from_slice(&data[..16]);
        }
    }
    if bytes == [0u8; 16] {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or_default();
        let pid = std::process::id() as u128;
        bytes[..8].copy_from_slice(&(nanos as u64).to_le_bytes());
        bytes[8..].copy_from_slice(&((nanos >> 64) as u64 ^ pid as u64).to_le_bytes());
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

/// `0700`/`0600` for artifacts that carry a DevTools token.
pub fn restrict_to_current_user(path: &Path) -> Result<(), ToolError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if path.is_dir() { 0o700 } else { 0o600 };
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
            .map_err(|error| ToolError::invalid(error.to_string()))?;
    }
    Ok(())
}

/// Signal-driven cancellation; the owning flow keeps doing the cleanup.
pub struct RunCancellation {
    cancelled: Arc<AtomicBool>,
}

impl RunCancellation {
    pub fn new() -> Self {
        let cancelled = Arc::new(AtomicBool::new(false));
        install_signal_handlers(Arc::clone(&cancelled));
        Self { cancelled }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

impl Default for RunCancellation {
    fn default() -> Self {
        Self::new()
    }
}

/// Live cancellation flags, published for the signal handler.
///
/// A `Vec` cannot be scanned from a signal handler, so the flags live in the
/// `Arc` allocations themselves and are never freed: the slots hold their
/// addresses, which stay valid for the rest of the process.
#[cfg(unix)]
mod cancellation {
    use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
    use std::sync::{Arc, Once};

    /// Upper bound on concurrent run loops in one CLI invocation.
    const SLOTS: usize = 8;

    static FLAGS: [AtomicPtr<AtomicBool>; SLOTS] = [const {
        AtomicPtr::new(std::ptr::null_mut())
    }; SLOTS];
    static INSTALL: Once = Once::new();

    pub fn register(flag: Arc<AtomicBool>) {
        let pointer = Arc::into_raw(flag) as *mut AtomicBool;
        let mut stored = false;
        for slot in FLAGS.iter() {
            if slot
                .compare_exchange(
                    std::ptr::null_mut(),
                    pointer,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                stored = true;
                break;
            }
        }
        if !stored {
            // Discard the extra reference; the run loop still polls its own
            // `Arc`, it just will not observe signals.
            // SAFETY: the pointer came from `Arc::into_raw` and was not stored.
            drop(unsafe { Arc::from_raw(pointer) });
        }
        INSTALL.call_once(|| {
            // SAFETY: the handler only performs atomic stores.
            unsafe {
                libc::signal(libc::SIGINT, handle as *const () as libc::sighandler_t);
                libc::signal(libc::SIGTERM, handle as *const () as libc::sighandler_t);
            }
        });
    }

    extern "C" fn handle(_signal: libc::c_int) {
        for slot in FLAGS.iter() {
            let pointer = slot.load(Ordering::Relaxed);
            if !pointer.is_null() {
                // SAFETY: registered flags are never freed while the process runs.
                unsafe { (*pointer).store(true, Ordering::SeqCst) };
            }
        }
    }
}

#[cfg(unix)]
fn install_signal_handlers(cancelled: Arc<AtomicBool>) {
    cancellation::register(cancelled);
}

#[cfg(windows)]
fn install_signal_handlers(_cancelled: Arc<AtomicBool>) {
    // Windows hosts cancel a run by terminating the CLI process.
}

/// True while `pid` is running.
pub fn process_is_alive(pid: i32) -> bool {
    if cfg!(windows) {
        return false;
    }
    #[cfg(unix)]
    {
        // SAFETY: `kill` with signal 0 only performs an existence check.
        unsafe { libc::kill(pid, 0) == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM) }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// The Inspector page and the DevTools client bridge.
pub struct Inspector;

impl Inspector {
    /// Writes a session-local bootstrap page that forwards to the Inspector.
    ///
    /// `open`/`xdg-open`/`rundll32` drop the query string when launching a
    /// `file://` URL, so the browser lands here first and the redirect (which
    /// preserves the query) carries the endpoint.
    pub fn present(
        session: &ManagedSession,
        root: &Path,
        session_directory: &Path,
        open: bool,
    ) -> Result<(), ToolError> {
        let page = root.join("DevTools/Client/index.html");
        let target = format!(
            "{}?endpoint={}",
            page.display(),
            percent_encode(&session.endpoint())
        );
        let bootstrap = session_directory.join("inspector.html");
        Files::text(&bootstrap_html(&target), &bootstrap)?;
        restrict_to_current_user(&bootstrap)?;
        println!(
            "DevTools session: {} ({}, {})",
            session.runtime.id,
            session.application.name,
            session.configuration.raw_value()
        );
        println!("Inspector: {}", file_url(&bootstrap));
        if !open {
            return Ok(());
        }
        let url = file_url(&bootstrap);
        if cfg!(target_os = "macos") {
            Shell::run(
                &["open".into(), url],
                None,
                &environment(),
                false,
                false,
                StdoutTarget::StandardOutput,
            )?;
        } else if cfg!(windows) {
            Shell::run(
                &["rundll32.exe".into(), "url.dll,FileProtocolHandler".into(), url],
                None,
                &environment(),
                false,
                false,
                StdoutTarget::StandardOutput,
            )?;
        } else {
            Shell::run(
                &["xdg-open".into(), url],
                None,
                &environment(),
                false,
                false,
                StdoutTarget::StandardOutput,
            )?;
        }
        Ok(())
    }

    /// Runs one client action against the resolved live session, or opens the
    /// Inspector when the command has no action.
    pub fn attach(arguments: &Arguments, project: &AppProject, root: &Path) -> Result<(), ToolError> {
        let (session, file) = Self::live(arguments, project)?;
        let Some(client) = &arguments.client else {
            return Self::present(
                &session,
                root,
                file.parent().unwrap_or(Path::new(".")),
                !arguments.flags.contains(&Flag::NoOpen),
            );
        };
        let mut forwarded = vec![
            "devtools".to_string(),
            client.operation.clone(),
            format!("@{}", file.display()),
        ];
        forwarded.extend(client.arguments.iter().cloned());
        crate::support::DeveloperTool::run(&forwarded, root, &environment(), false)
    }

    /// The single live session, filtered by `--session` and `--ios`/`--android`.
    fn live(
        arguments: &Arguments,
        project: &AppProject,
    ) -> Result<(ManagedSession, PathBuf), ToolError> {
        let directory = project.build_directory().join("devtools");
        if !directory.is_dir() {
            return Err(ToolError::invalid(
                "No development session; start cove run --devtools or --profile",
            ));
        }
        let requested = arguments.value(OptionKey::Session);
        let mut sessions: Vec<(ManagedSession, PathBuf)> = Vec::new();
        for child in Files::children(&directory)? {
            let file = child.join("session.json");
            let Ok(data) = std::fs::read(&file) else {
                continue;
            };
            let Ok(session) = serde_json::from_slice::<ManagedSession>(&data) else {
                continue;
            };
            if let Some(requested) = requested {
                if session.runtime.id != requested {
                    continue;
                }
            }
            if let Some(platform) = arguments.requested_platform {
                if session.platform != platform {
                    continue;
                }
            }
            if Self::is_alive(&session)? {
                sessions.push((session, file));
            }
        }
        match sessions.len() {
            1 => Ok(sessions.pop().expect("length checked")),
            0 => Err(ToolError::invalid(
                "No live DevTools session for this application",
            )),
            _ => Err(ToolError::invalid(format!(
                "Multiple live sessions: {}; select --session ID",
                sessions
                    .iter()
                    .map(|(session, _)| session.runtime.id.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))),
        }
    }

    pub fn is_alive(session: &ManagedSession) -> Result<bool, ToolError> {
        if let Some(device) = &session.device {
            let pid = Shell::run(
                &[
                    device.adb.clone(),
                    "-s".into(),
                    device.serial.clone(),
                    "shell".into(),
                    "pidof".into(),
                    session.application.id.clone(),
                ],
                None,
                &environment(),
                true,
                true,
                StdoutTarget::StandardOutput,
            )?;
            return Ok(pid
                .split_whitespace()
                .any(|value| value == session.runtime.pid.to_string()));
        }
        if cfg!(windows) {
            let pid = session.runtime.pid.to_string();
            let listing = Shell::run(
                &[
                    "tasklist.exe".into(),
                    "/FI".into(),
                    format!("PID eq {pid}"),
                    "/NH".into(),
                ],
                None,
                &environment(),
                true,
                true,
                StdoutTarget::StandardOutput,
            )?;
            return Ok(listing.split_whitespace().any(|value| value == pid));
        }
        Ok(process_is_alive(session.runtime.pid as i32))
    }
}

fn bootstrap_html(target: &str) -> String {
    let literal = serde_json::to_string(target).unwrap_or_else(|_| "\"\"".into());
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\">\n  <title>CoveUI DevTools</title>\n</head>\n<body>\n  <p>Opening CoveUI DevTools… <a href=\"{target}\">Open manually</a></p>\n  <noscript><p>Enable JavaScript or open the CoveUI DevTools Inspector directly.</p></noscript>\n  <script>location.replace({literal});</script>\n</body>\n</html>"
    )
}

/// `file://` form of an absolute path, percent-encoding unsafe bytes.
fn file_url(path: &Path) -> String {
    format!("file://{}", percent_encode(&path.display().to_string()))
}

fn percent_encode(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => {
                result.push(byte as char)
            }
            other => result.push_str(&format!("%{other:02X}")),
        }
    }
    result
}

/// `cove run` for a desktop application.
pub struct DesktopRun;

impl DesktopRun {
    pub fn run(
        executable: &Path,
        products: &Path,
        project: &AppProject,
        root: &Path,
        arguments: &Arguments,
    ) -> Result<(), ToolError> {
        let mut environment = environment();
        environment.insert(
            "COVE_BUILD_PRODUCTS_PATH".into(),
            products.display().to_string(),
        );
        let Some(development) = DevelopmentSession::make(project.clone(), arguments)? else {
            let mut command = vec![executable.display().to_string()];
            command.extend(arguments.application_arguments.iter().cloned());
            Shell::run(
                &command,
                Some(&project.directory),
                &environment,
                false,
                false,
                StdoutTarget::StandardOutput,
            )?;
            return Ok(());
        };
        let cancellation = RunCancellation::new();
        for (key, value) in development.environment() {
            environment.insert(key, value);
        }
        environment.insert(
            "COVE_BUILD_PRODUCTS_PATH".into(),
            products.display().to_string(),
        );
        let mut command = vec![executable.display().to_string()];
        command.extend(arguments.application_arguments.iter().cloned());
        let mut child = Shell::start(&command, Some(&project.directory), &environment)?;
        let deadline = Instant::now() + Duration::from_secs(20);
        let descriptor = PathBuf::from(&development.launch.descriptor_path);
        let mut runtime: Option<DevToolsDiscovery> = None;
        while is_running(&mut child)
            && Instant::now() < deadline
            && !cancellation.is_cancelled()
        {
            if let Some(found) = read_discovery(&descriptor) {
                runtime = Some(found);
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let Some(runtime) = runtime else {
            if cancellation.is_cancelled() {
                development.remove();
                return Ok(());
            }
            let _ = child.kill();
            let _ = child.wait();
            development.remove();
            return Err(ToolError::invalid(
                "Application did not publish its DevTools endpoint; check its startup log and diagnostics configuration",
            ));
        };
        let result = (|| {
            let session = development.publish(&runtime, None)?;
            Inspector::present(
                &session,
                root,
                &development.directory,
                arguments.flags.contains(&Flag::Devtools)
                    && !arguments.flags.contains(&Flag::NoOpen),
            )?;
            while is_running(&mut child) && !cancellation.is_cancelled() {
                std::thread::sleep(Duration::from_millis(100));
            }
            if !is_running(&mut child) {
                let _ = child.wait();
            }
            if !cancellation.is_cancelled() {
                if let Some(status) = child_status(&mut child).filter(|status| *status != 0) {
                    return Err(ToolError::failed(executable.display().to_string(), status));
                }
            }
            Ok(())
        })();
        if is_running(&mut child) {
            let _ = child.kill();
            let _ = child.wait();
        }
        development.remove();
        result
    }
}

fn read_discovery(path: &Path) -> Option<DevToolsDiscovery> {
    let data = std::fs::read(path).ok()?;
    serde_json::from_slice(&data).ok()
}

fn is_running(child: &mut std::process::Child) -> bool {
    matches!(child.try_wait(), Ok(None))
}

fn child_status(child: &mut std::process::Child) -> Option<i32> {
    match child.try_wait() {
        Ok(Some(status)) => status.code(),
        _ => None,
    }
}

/// `cove run --android` after the APK is installed.
pub struct AndroidRun;

impl AndroidRun {
    pub fn run(
        apk: &Path,
        project: &AppProject,
        toolchain: &crate::platforms_android::AndroidToolchain,
        arguments: &Arguments,
    ) -> Result<(), ToolError> {
        let development = DevelopmentSession::make(project.clone(), arguments)?;
        let result = Self::run_with(apk, project, &toolchain, arguments, development.as_ref());
        if let Some(development) = &development {
            development.remove();
        }
        result
    }

    fn run_with(
        apk: &Path,
        project: &AppProject,
        toolchain: &crate::platforms_android::AndroidToolchain,
        arguments: &Arguments,
        development: Option<&DevelopmentSession>,
    ) -> Result<(), ToolError> {
        let application_id = project.configuration.application.id.clone();
        let device = crate::platforms_android::apk::AndroidDevice::install(
            apk,
            toolchain,
            arguments.value(OptionKey::Device),
            &project.configuration.application,
            development.map(|development| &development.launch),
        )?;
        let Some(development) = development else {
            return Ok(());
        };
        let adb = toolchain.sdk.join("platform-tools/adb").display().to_string();
        let command = vec![adb.clone(), "-s".into(), device.clone()];
        let cancellation = RunCancellation::new();
        let descriptor = format!(
            "files/devtools/{}.json",
            development.launch.id
        );
        let mut runtime: Option<DevToolsDiscovery> = None;
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline && !cancellation.is_cancelled() {
            let mut probe = command.clone();
            probe.extend([
                "exec-out".to_string(),
                "run-as".into(),
                application_id.clone(),
                "cat".into(),
                descriptor.clone(),
            ]);
            if let Ok(text) = Shell::run(
                &probe,
                None,
                &environment(),
                true,
                true,
                StdoutTarget::StandardOutput,
            ) {
                if let Ok(found) = serde_json::from_slice::<DevToolsDiscovery>(text.as_bytes()) {
                    runtime = Some(found);
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let Some(runtime) = runtime else {
            if cancellation.is_cancelled() {
                return Ok(());
            }
            stop(&command, &application_id);
            return Err(ToolError::invalid(
                "Android did not publish its DevTools endpoint; inspect the application's startup log",
            ));
        };
        let Some(port) = endpoint_port(&runtime.endpoint) else {
            stop(&command, &application_id);
            return Err(ToolError::invalid(
                "Android did not publish its DevTools endpoint; inspect the application's startup log",
            ));
        };
        let mut forward = command.clone();
        forward.extend([
            "forward".to_string(),
            "tcp:0".into(),
            format!("tcp:{port}"),
        ]);
        let forwarded = Shell::run(
            &forward,
            None,
            &environment(),
            true,
            true,
            StdoutTarget::StandardOutput,
        )?;
        let Some(host_port) = forwarded.trim().parse::<i64>().ok() else {
            stop(&command, &application_id);
            return Err(ToolError::invalid("ADB did not return a forwarded port"));
        };
        let result = (|| {
            let session = development.publish(
                &runtime,
                Some(Device {
                    adb: adb.clone(),
                    serial: device.clone(),
                    forwarded_port: host_port,
                }),
            )?;
            Inspector::present(
                &session,
                &toolchain.root,
                &development.directory,
                arguments.flags.contains(&Flag::Devtools)
                    && !arguments.flags.contains(&Flag::NoOpen),
            )?;
            while !cancellation.is_cancelled() {
                if !Inspector::is_alive(&session).unwrap_or(false) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
            Ok(())
        })();
        let mut remove = command.clone();
        remove.extend([
            "forward".to_string(),
            "--remove".into(),
            format!("tcp:{host_port}"),
        ]);
        let _ = Shell::run(
            &remove,
            None,
            &environment(),
            false,
            true,
            StdoutTarget::StandardOutput,
        );
        stop(&command, &application_id);
        result
    }
}

fn stop(command: &[String], application_id: &str) {
    let mut stop = command.to_vec();
    stop.extend([
        "shell".to_string(),
        "am".into(),
        "force-stop".into(),
        application_id.to_string(),
    ]);
    let _ = Shell::run(
        &stop,
        None,
        &environment(),
        false,
        true,
        StdoutTarget::StandardOutput,
    );
}

/// The `:port` of an endpoint URL.
fn endpoint_port(endpoint: &str) -> Option<u16> {
    let rest = endpoint.split("://").nth(1).unwrap_or(endpoint);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let port = authority.rsplit_once(':')?.1;
    port.trim_end_matches(|character: char| !character.is_ascii_digit())
        .parse()
        .ok()
}
