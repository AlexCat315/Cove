use std::{
    collections::BTreeSet,
    io,
    path::{Component, Path, PathBuf},
};

/// Resolves development and packaged resources without a platform framework dependency.
#[derive(Clone, Debug)]
pub struct ResourceBundle {
    root: PathBuf,
}
impl ResourceBundle {
    pub fn locate(names: &[&str], roots: &[PathBuf]) -> Option<Self> {
        for name in names {
            if Path::new(name).components().count() != 1
                || !matches!(
                    Path::new(name).components().next(),
                    Some(Component::Normal(_))
                )
            {
                continue;
            }
            let mut visited = BTreeSet::new();
            for root in roots {
                if !visited.insert(root) {
                    continue;
                }
                for suffix in ["bundle", "resources"] {
                    let candidate = root.join(format!("{name}.{suffix}"));
                    if candidate.is_dir() {
                        return Some(Self { root: candidate });
                    }
                }
            }
        }
        None
    }
    pub fn beside_executable(names: &[&str]) -> io::Result<Option<Self>> {
        let executable = std::env::current_exe()?;
        let parent = executable
            .parent()
            .ok_or_else(|| io::Error::other("executable has no parent"))?;
        let mut roots = Vec::new();
        if let Some(path) = std::env::var_os("COVE_RESOURCE_PATH")
            .or_else(|| std::env::var_os("PACKAGE_RESOURCE_BUNDLE_PATH"))
        {
            roots.push(path.into());
        }
        roots.push(parent.to_owned());
        if parent.file_name().is_some_and(|n| n == "MacOS") {
            if let Some(contents) = parent
                .parent()
                .filter(|p| p.file_name().is_some_and(|n| n == "Contents"))
            {
                roots.push(contents.join("Resources"));
            }
        }
        Ok(Self::locate(names, &roots))
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn read(&self, relative: &Path) -> io::Result<Vec<u8>> {
        if relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "resource path must stay inside its bundle",
            ));
        }
        let root = self.root.canonicalize()?;
        let target = root.join(relative).canonicalize()?;
        if !target.starts_with(&root) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "resource symlink escapes its bundle",
            ));
        }
        std::fs::read(target)
    }
}
