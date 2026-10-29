//! Translation of `System/Sources/CoveFileSystem/AtomicFile.swift`.
use super::permissions::FilePermissions;
use super::replacement::FileReplacement;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub enum AtomicFile {}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Access {
    #[default]
    Standard,
    OwnerOnly,
}

impl AtomicFile {
    /// Writes a sibling staging file and publishes it with one replacement
    /// operation. The staging file is removed whether publication succeeds or
    /// fails.
    pub fn write(data: &[u8], destination: &Path, access: Access) -> io::Result<()> {
        let parent = destination
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let staging = parent.join(format!(".cove-{}", unique_identifier()));
        let result = (|| {
            if access == Access::OwnerOnly {
                OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&staging)?;
                FilePermissions::restrict_to_current_user(&staging)?;
                let mut handle = OpenOptions::new().append(true).open(&staging)?;
                handle.write_all(data)?;
                handle.sync_all()?;
            } else {
                let mut handle = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&staging)?;
                handle.write_all(data)?;
                #[cfg(not(target_os = "windows"))]
                {
                    // Preserve the published destination's POSIX mode when it
                    // already exists.
                    if let Ok(metadata) = fs::metadata(destination) {
                        let _ = fs::set_permissions(&staging, metadata.permissions());
                    }
                }
            }
            FileReplacement::replace(destination, &staging)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&staging);
        }
        result
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
