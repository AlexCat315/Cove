//! Translation of `Logging/Sources/CoveLogging/Outputs/File.swift`.
//!
//! One directory per process session with a `.lease` marker locked for the
//! lifetime of the file. Pruning only removes directories whose marker it can
//! lock, so another live process is never deleted.
use super::configuration::{LoggingError, Storage};
use cove_system::files::FileLock;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const LEASE_CONTENTS: &[u8] = b"CoveLogging/1\n";

pub struct LogFile {
    pub directory: PathBuf,
    configuration: Storage,
    /// Held for the lifetime of the file; dropping it releases the lease.
    _lease: FileLock,
    handle: Option<File>,
    size: u64,
}

impl LogFile {
    /// `root` is the user-facing log root; `application` names the child
    /// directory and `configuration` supplies rotation limits.
    pub fn new(root: &Path, application: &str, configuration: Storage) -> io::Result<Self> {
        let safe = sanitize_component(application);
        let parent = root.join(if safe.is_empty() || safe == "." || safe == ".." {
            "Cove".to_owned()
        } else {
            safe
        });
        let directory = parent.join(format!("session-{}", session_identifier()));
        create_private_directory(&directory)?;
        let marker = directory.join(".lease");
        write_private_file(&marker, LEASE_CONTENTS)?;
        let lease = FileLock::acquire(&marker)?;
        let mut file = Self {
            directory,
            configuration,
            _lease: lease,
            handle: None,
            size: 0,
        };
        if let Err(error) = file.open().and_then(|()| file.prune(&parent)) {
            drop(file);
            return Err(error);
        }
        Ok(file)
    }

    pub fn path(&self) -> PathBuf {
        self.directory.join("events.jsonl")
    }

    pub fn append(&mut self, data: &[u8]) -> io::Result<()> {
        if data.len() as u64 > self.configuration.maximum_file_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                LoggingError::InvalidConfiguration.to_string(),
            ));
        }
        if self.size > 0
            && self.size.saturating_add(data.len() as u64) > self.configuration.maximum_file_bytes
        {
            self.rotate()?;
        }
        let handle = self
            .handle
            .as_mut()
            .ok_or_else(|| io::Error::other("log file is closed"))?;
        handle.write_all(data)?;
        self.size += data.len() as u64;
        Ok(())
    }

    pub fn flush(&mut self) -> io::Result<()> {
        if let Some(handle) = &mut self.handle {
            // `FileHandle.synchronize` is fsync, not a user-space flush.
            handle.sync_all()?;
        }
        Ok(())
    }

    fn open(&mut self) -> io::Result<()> {
        let path = self.path();
        create_private_file(&path)?;
        self.handle = Some(OpenOptions::new().append(true).open(path)?);
        self.size = 0;
        Ok(())
    }

    fn rotate(&mut self) -> io::Result<()> {
        self.handle.take();
        let files = self.configuration.files_per_session;
        if files > 1 {
            for index in (1..files).rev() {
                let target = self.directory.join(format!("events.{index}.jsonl"));
                if target.exists() {
                    fs::remove_file(&target)?;
                }
                let source = if index == 1 {
                    self.path()
                } else {
                    self.directory.join(format!("events.{}.jsonl", index - 1))
                };
                if source.exists() {
                    fs::rename(source, target)?;
                }
            }
        }
        let path = self.path();
        if path.exists() {
            fs::remove_file(&path)?;
        }
        self.open()
    }

    /// Keeps the newest `retained_sessions` directories, never the live one and
    /// never a directory that is not a Cove session.
    fn prune(&self, parent: &Path) -> io::Result<()> {
        let mut sessions = Vec::new();
        for entry in fs::read_dir(parent)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Some(identifier) = name.strip_prefix("session-") else {
                continue;
            };
            if !is_uuid(identifier) {
                continue;
            }
            let created = entry
                .metadata()
                .and_then(|metadata| metadata.created().or_else(|_| metadata.modified()))
                .unwrap_or(UNIX_EPOCH);
            sessions.push((created, path));
        }
        sessions.sort_by(|lhs, rhs| rhs.0.cmp(&lhs.0));
        for (_, old) in sessions.iter().skip(self.configuration.retained_sessions) {
            if *old == self.directory {
                continue;
            }
            let Ok(metadata) = fs::symlink_metadata(old) else {
                continue;
            };
            if metadata.file_type().is_symlink() {
                continue;
            }
            let marker = old.join(".lease");
            match fs::read(&marker) {
                Ok(contents) if contents == LEASE_CONTENTS => {}
                _ => continue,
            }
            let Some(_lock) = FileLock::try_acquire(&marker) else {
                continue;
            };
            // Do not delete another process's active session or unrelated
            // directory. Lock acquisition proves no live process holds it.
            let _ = fs::remove_dir_all(old);
        }
        Ok(())
    }
}

fn sanitize_component(application: &str) -> String {
    application
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || ".-_".contains(character) {
                character
            } else {
                '_'
            }
        })
        .take(128)
        .collect()
}

/// Foundation's `UUID(uuidString:)` accepts 8-4-4-4-12 hexadecimal groups.
fn is_uuid(value: &str) -> bool {
    let groups: Vec<&str> = value.split('-').collect();
    groups.len() == 5
        && [8, 4, 4, 4, 12]
            .iter()
            .zip(groups.iter())
            .all(|(length, group)| {
                group.len() == *length && group.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
}

fn session_identifier() -> String {
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

fn create_private_directory(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)?;
    restrict(path, true)
}

fn create_private_file(path: &Path) -> io::Result<()> {
    if !path.exists() {
        OpenOptions::new().write(true).create_new(true).open(path)?;
    }
    restrict(path, false)
}

fn write_private_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    restrict(path, false)?;
    file.write_all(contents)
}

fn restrict(path: &Path, directory: bool) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            path,
            fs::Permissions::from_mode(if directory { 0o700 } else { 0o600 }),
        )?;
    }
    cove_system::files::restrict_path(path, directory)
}
