//! Translation of `System/Sources/CoveFileSystem/Permissions.swift`.
use crate::files::restrict_path;
use std::fs;
use std::io;
use std::path::Path;

pub enum FilePermissions {}

impl FilePermissions {
    /// Creates the directory, applying `0700` at creation time on POSIX, then
    /// replaces inherited access with access for the current user.
    pub fn create_private_directory(
        path: &Path,
        with_intermediate_directories: bool,
    ) -> io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = fs::DirBuilder::new();
            builder.recursive(with_intermediate_directories).mode(0o700);
            builder.create(path)?;
        }
        #[cfg(not(unix))]
        {
            if with_intermediate_directories {
                fs::create_dir_all(path)?;
            } else {
                fs::create_dir(path)?;
            }
        }
        Self::restrict_to_current_user(path)
    }

    /// Replaces inherited access with access for the current user. Symbolic
    /// links are rejected rather than followed.
    pub fn restrict_to_current_user(path: &Path) -> io::Result<()> {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "cannot restrict a symbolic link",
            ));
        }
        restrict_path(path, metadata.is_dir())
    }
}
