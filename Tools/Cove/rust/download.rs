//! Downloading pinned SDK artifacts with SHA-256 verification.
//!
//! `cove-cli` has no HTTP/TLS dependency, so the transfer is delegated to the
//! platform `curl` executable, exactly as the `ToolSupport` translation does.
//! Redirect resolution, digest verification and atomic publication stay here.

use crate::sha256;
use crate::support::{environment, Files, Shell, StdoutTarget, ToolError};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Resolves `remote` after redirects using a HEAD request, without a body.
pub fn redirected_url(remote: &str) -> Result<String, ToolError> {
    require_https(remote)?;
    let output = Shell::run(
        &[
            "curl".into(),
            "--fail".into(),
            "--location".into(),
            "--silent".into(),
            "--show-error".into(),
            "--head".into(),
            "--output".into(),
            null_device().into(),
            "--write-out".into(),
            "%{url_effective}".into(),
            remote.into(),
        ],
        None,
        &environment(),
        true,
        true,
        StdoutTarget::StandardOutput,
    )
    .map_err(|error| {
        ToolError::invalid(format!("HTTP request for {remote} failed: {error}"))
    })?;
    let url = output.trim().to_string();
    if url.is_empty() {
        return Err(ToolError::invalid(format!(
            "Redirect did not complete: {remote}"
        )));
    }
    Ok(url)
}

/// Downloads metadata beside `destination`, then publishes it atomically.
pub fn metadata(from: &str, to: &Path) -> Result<(), ToolError> {
    require_https(from)?;
    if let Some(parent) = to.parent() {
        Files::directory(parent)?;
    }
    let downloaded = fetch(from, to)?;
    let result = replace(&downloaded, to);
    if result.is_err() {
        let _ = Files::remove(&downloaded);
    }
    result
}

/// Verifies the digest of an existing file.
pub fn verify(file: &Path, expected: &str) -> Result<(), ToolError> {
    let actual = sha256::hex_file(file).map_err(io_error)?;
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(ToolError::invalid(format!(
            "SHA-256 mismatch for {}: expected {expected}, got {actual}",
            file.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        )));
    }
    Ok(())
}

/// Downloads `remote` to `destination` unless the cached file already matches.
pub fn file(from: &str, to: &Path, sha256: &str) -> Result<(), ToolError> {
    if to.is_file() {
        if let Ok(actual) = sha256::hex_file(to) {
            if actual.eq_ignore_ascii_case(sha256) {
                return Ok(());
            }
        }
    }
    if let Some(parent) = to.parent() {
        Files::directory(parent)?;
    }
    println!("Downloading {from}");
    let downloaded = fetch(from, to)?;
    let result = (|| {
        verify(&downloaded, sha256)?;
        replace(&downloaded, to)
    })();
    let _ = Files::remove(&downloaded);
    result
}

fn fetch(remote: &str, destination: &Path) -> Result<PathBuf, ToolError> {
    let parent = destination
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    Files::directory(&parent)?;
    let staging = parent.join(format!(".cove-download-{}", unique_token()));
    let output = Shell::run(
        &[
            "curl".into(),
            "--fail".into(),
            "--location".into(),
            "--silent".into(),
            "--show-error".into(),
            "--output".into(),
            staging.display().to_string(),
            remote.into(),
        ],
        None,
        &environment(),
        true,
        true,
        StdoutTarget::StandardOutput,
    );
    match output {
        Ok(_) => {
            if !staging.is_file() {
                return Err(ToolError::invalid(format!("No data for {remote}")));
            }
            Ok(staging)
        }
        Err(error) => {
            let _ = Files::remove(&staging);
            Err(ToolError::invalid(format!("{remote}: {error}")))
        }
    }
}

fn replace(source: &Path, destination: &Path) -> Result<(), ToolError> {
    if let Some(parent) = destination.parent() {
        Files::directory(parent)?;
    }
    Files::remove(destination)?;
    std::fs::rename(source, destination).map_err(io_error)
}

fn require_https(remote: &str) -> Result<(), ToolError> {
    if remote.starts_with("https://") {
        Ok(())
    } else {
        Err(ToolError::invalid("Metadata requires HTTPS"))
    }
}

fn null_device() -> &'static str {
    if cfg!(windows) {
        "NUL"
    } else {
        "/dev/null"
    }
}

fn io_error(error: io::Error) -> ToolError {
    ToolError::invalid(error.to_string())
}

fn unique_token() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default();
    format!(
        "{:x}{:x}{:x}",
        std::process::id(),
        nanos,
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}
