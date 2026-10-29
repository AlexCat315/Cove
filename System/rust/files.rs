use std::{
    ffi::CString,
    fs,
    io::{self, Write},
    path::{Component, Path, PathBuf},
};

unsafe extern "C" {
    fn cove_restrict_path(path: *const std::ffi::c_char, directory: i32) -> u32;
    fn cove_same_file(lhs: *const std::ffi::c_char, rhs: *const std::ffi::c_char) -> i32;
    fn cove_file_lock(path: *const std::ffi::c_char, error: *mut u32) -> isize;
    fn cove_file_unlock(handle: isize);
    fn cove_replace_file(
        source: *const std::ffi::c_char,
        destination: *const std::ffi::c_char,
    ) -> u32;
}

fn native_path(path: &Path) -> io::Result<CString> {
    #[cfg(unix)]
    let bytes = {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes()
    };
    #[cfg(not(unix))]
    let bytes = path
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path is not valid UTF-8"))?
        .as_bytes();
    CString::new(bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Access {
    #[default]
    Inherit,
    OwnerOnly,
}

pub fn restrict_path(path: &Path, directory: bool) -> io::Result<()> {
    let path = native_path(path)?;
    // SAFETY: NUL-terminated path lives for the duration of the call.
    let error = unsafe { cove_restrict_path(path.as_ptr(), i32::from(directory)) };
    if error == 0 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(error as i32))
    }
}

/// Stages beside the destination, applies access before writing, then atomically replaces it.
pub fn atomic_write(path: &Path, bytes: &[u8], access: Access) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut staged = tempfile::Builder::new()
        .prefix(".cove-")
        .tempfile_in(parent)?;
    if access == Access::OwnerOnly {
        restrict_path(staged.path(), false)?;
    }
    staged.write_all(bytes)?;
    staged.as_file().sync_all()?;
    let staging_path = staged.into_temp_path();
    replace_file(&staging_path, path)?;
    Ok(())
}

/// Same-volume publication. Failure preserves the destination and staging file.
pub fn replace_file(staging: &Path, destination: &Path) -> io::Result<()> {
    if same_file(staging, destination)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "staging and destination identify the same file",
        ));
    }
    let source = native_path(staging)?;
    let target = native_path(destination)?;
    // SAFETY: both arguments are live NUL-terminated paths; the native function borrows them.
    let error = unsafe { cove_replace_file(source.as_ptr(), target.as_ptr()) };
    if error == 0 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(error as i32))
    }
}

/// Resolves the longest existing ancestor before normalizing missing components.
pub fn resolve_path(path: &Path) -> io::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    match fs::canonicalize(&absolute) {
        Ok(path) => return Ok(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let parent = absolute
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "path has no existing ancestor"))?;
    let mut resolved = resolve_path(parent)?;
    match absolute.components().next_back() {
        Some(Component::ParentDir) => {
            resolved.pop();
        }
        Some(Component::Normal(name)) => resolved.push(name),
        Some(Component::CurDir) => {}
        _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid path")),
    }
    Ok(resolved)
}

pub fn same_file(lhs: &Path, rhs: &Path) -> io::Result<bool> {
    let left = native_path(lhs)?;
    let right = native_path(rhs)?;
    // SAFETY: both arguments are live NUL-terminated paths.
    match unsafe { cove_same_file(left.as_ptr(), right.as_ptr()) } {
        0 => Ok(false),
        1 => Ok(true),
        _ => Ok(resolve_path(lhs)? == resolve_path(rhs)?),
    }
}

pub fn contains_path(directory: &Path, candidate: &Path) -> io::Result<bool> {
    let directory = resolve_path(directory)?;
    let mut candidate = resolve_path(candidate)?;
    loop {
        if same_file(&directory, &candidate)? {
            return Ok(true);
        }
        if !candidate.pop() {
            return Ok(false);
        }
    }
}

/// Exclusive cross-process lock. Dropping the guard closes its native handle.
pub struct FileLock {
    handle: isize,
}
impl FileLock {
    pub fn acquire(path: &Path) -> io::Result<Self> {
        let path = native_path(path)?;
        let mut error = 0;
        // SAFETY: valid path and writable error location; handle is exclusively owned.
        let handle = unsafe { cove_file_lock(path.as_ptr(), &mut error) };
        if handle == -1 {
            Err(io::Error::from_raw_os_error(error as i32))
        } else {
            Ok(Self { handle })
        }
    }
}
impl Drop for FileLock {
    fn drop(&mut self) {
        // SAFETY: handle was obtained by acquire and is closed exactly once.
        unsafe {
            cove_file_unlock(self.handle);
        }
    }
}

/// Moves to an explicit recovery directory. A failed rename leaves the source intact.
pub fn move_to_recovery(source: &Path, directory: &Path) -> io::Result<PathBuf> {
    if contains_path(source, directory)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "recovery directory is inside the source",
        ));
    }
    fs::create_dir_all(directory)?;
    let reservation = tempfile::Builder::new()
        .prefix("recovered-")
        .tempdir_in(directory)?;
    let target = reservation.path().join(source.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "cannot move a filesystem root")
    })?);
    fs::rename(source, &target)?;
    reservation.keep();
    Ok(target)
}

/// Native macOS Trash, or an explicitly recoverable move on other platforms.
pub fn trash(source: &Path, recovery_directory: &Path) -> io::Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        use std::ffi::CStr;
        use std::os::unix::ffi::OsStrExt;
        unsafe extern "C" {
            fn cove_trash_item(
                path: *const std::ffi::c_char,
                error: *mut u32,
            ) -> *mut std::ffi::c_char;
        }
        let source = native_path(source)?;
        let mut error = 0;
        let _ = recovery_directory;
        // SAFETY: path/error remain live. The returned allocation is copied and freed exactly once.
        let result = unsafe { cove_trash_item(source.as_ptr(), &mut error) };
        if result.is_null() {
            return Err(io::Error::from_raw_os_error(if error == 0 {
                12
            } else {
                error as i32
            }));
        }
        // SAFETY: a successful native call returns a NUL-terminated malloc allocation.
        let path = unsafe {
            PathBuf::from(std::ffi::OsStr::from_bytes(
                CStr::from_ptr(result).to_bytes(),
            ))
        };
        // SAFETY: allocation originated from strdup in the native bridge and is no longer borrowed.
        unsafe {
            libc::free(result.cast());
        }
        Ok(path)
    }
    #[cfg(not(target_os = "macos"))]
    {
        move_to_recovery(source, recovery_directory)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atomic_replacement_and_owner_access() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("秘密.txt");
        atomic_write(&path, b"old", Access::Inherit).unwrap();
        atomic_write(&path, b"new", Access::OwnerOnly).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        assert!(atomic_write(&root.path().join("absent/file"), b"bad", Access::Inherit).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"new");
    }
    #[test]
    fn identities_hardlinks_and_component_boundaries() {
        let root = tempfile::tempdir().unwrap();
        let a = root.path().join("a");
        let b = root.path().join("b");
        fs::write(&a, b"test").unwrap();
        fs::hard_link(&a, &b).unwrap();
        assert!(same_file(&a, &b).unwrap());
        assert!(contains_path(root.path(), &root.path().join("missing/deep")).unwrap());
        assert!(!contains_path(&root.path().join("foo"), &root.path().join("foobar")).unwrap());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.path(), root.path().join("link")).unwrap();
            assert_eq!(
                resolve_path(&root.path().join("link/missing")).unwrap(),
                resolve_path(&root.path().join("missing")).unwrap()
            );
        }
    }
    #[test]
    fn recovery_is_non_destructive() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        fs::write(&source, b"retain").unwrap();
        let target = move_to_recovery(&source, &root.path().join("recovery")).unwrap();
        assert!(!source.exists());
        assert_eq!(fs::read(target).unwrap(), b"retain");
    }
    #[test]
    fn file_lock_serializes_threads() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("lock");
        let lock = FileLock::acquire(&path).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let _lock = FileLock::acquire(&path).unwrap();
            tx.send(()).unwrap();
        });
        assert!(
            rx.recv_timeout(std::time::Duration::from_millis(30))
                .is_err()
        );
        drop(lock);
        rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap();
        waiter.join().unwrap();
    }
}
