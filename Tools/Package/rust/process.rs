//! Child processes and file staging shared by every platform.

use crate::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn cargo() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
}

/// Runs a command to completion, echoing it first.
pub fn run(command: &mut Command) -> Result<()> {
    eprintln!("    {}", describe(command));
    let status = command
        .status()
        .map_err(|error| format!("{}: {error}", describe(command)))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{} failed with {status}", describe(command)))
    }
}

/// Runs a command and returns its standard output.
pub fn output(command: &mut Command) -> Result<String> {
    let output = command
        .output()
        .map_err(|error| format!("{}: {error}", describe(command)))?;
    if !output.status.success() {
        return Err(format!(
            "{} failed with {}: {}",
            describe(command),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout).map_err(|error| error.to_string())
}

fn describe(command: &Command) -> String {
    std::iter::once(command.get_program())
        .chain(command.get_args())
        .map(|part| part.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn create_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path).map_err(|error| format!("{}: {error}", path.display()))
}

pub fn remove_dir(path: &Path) -> Result<()> {
    match std::fs::remove_dir_all(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            Err(format!("{}: {error}", path.display()))
        }
        _ => Ok(()),
    }
}

pub fn copy(source: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        create_dir(parent)?;
    }
    std::fs::copy(source, destination)
        .map(drop)
        .map_err(|error| format!("{} -> {}: {error}", source.display(), destination.display()))
}

pub fn write(path: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    if let Some(parent) = path.parent() {
        create_dir(parent)?;
    }
    std::fs::write(path, contents).map_err(|error| format!("{}: {error}", path.display()))
}

/// Every regular file below `root`, as paths relative to it, sorted.
pub fn files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries = std::fs::read_dir(&directory)
            .map_err(|error| format!("{}: {error}", directory.display()))?;
        for entry in entries {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.is_file() {
                found.push(
                    path.strip_prefix(root)
                        .expect("walked below root")
                        .to_path_buf(),
                );
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Copies the tree at `source` into `destination`, skipping `README.md` notes.
pub fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    if !source.is_dir() {
        return Ok(());
    }
    for relative in files(source)? {
        if relative == Path::new("README.md") {
            continue;
        }
        copy(&source.join(&relative), &destination.join(&relative))?;
    }
    Ok(())
}

/// The first executable on `PATH` with this name.
pub fn which(name: &str) -> Option<PathBuf> {
    let suffixes: &[&str] = if cfg!(windows) {
        &[".exe", ".cmd", ""]
    } else {
        &[""]
    };
    std::env::split_paths(&std::env::var_os("PATH")?).find_map(|directory| {
        suffixes
            .iter()
            .map(|suffix| directory.join(format!("{name}{suffix}")))
            .find(|path| path.is_file())
    })
}
