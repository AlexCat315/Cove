//! Translation of `System/Sources/CoveFileSystem/Trash.swift`.
use super::paths::PathIdentity;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub enum FileTrash {}

impl FileTrash {
    /// Uses the native macOS Trash, or the caller's recoverable storage on
    /// other hosts. Never falls back to permanent deletion.
    pub fn r#move(source: &Path, recovery_directory: &Path) -> io::Result<PathBuf> {
        #[cfg(target_os = "macos")]
        {
            return crate::files::trash(source, recovery_directory);
        }
        #[cfg(not(target_os = "macos"))]
        {
            if PathIdentity::contains(recovery_directory, source, true) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "recovery directory is inside the item",
                ));
            }
            fs::create_dir_all(recovery_directory)?;
            let name = source.file_name().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "cannot move a filesystem root")
            })?;
            let destination = recovery_directory.join(format!(
                "{}-{}",
                name.to_string_lossy(),
                unique_identifier()
            ));
            fs::rename(source, &destination)?;
            Ok(destination)
        }
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
