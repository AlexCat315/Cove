use std::path::{Path, PathBuf};
/// Rust resource roots replace Foundation.Bundle; lookup falls back to the bundle root.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BundleImageResource {
    pub name: String,
    pub file_extension: String,
    pub bundle: PathBuf,
    pub subdirectory: Option<PathBuf>,
}
impl BundleImageResource {
    pub fn new(
        name: impl Into<String>,
        extension: impl Into<String>,
        bundle: impl Into<PathBuf>,
        subdirectory: Option<PathBuf>,
    ) -> Self {
        Self {
            name: name.into(),
            file_extension: extension.into(),
            bundle: bundle.into(),
            subdirectory,
        }
    }
    pub fn svg(
        name: impl Into<String>,
        bundle: impl Into<PathBuf>,
        subdirectory: Option<PathBuf>,
    ) -> Self {
        Self::new(name, "svg", bundle, subdirectory)
    }
    pub fn path(&self) -> Option<PathBuf> {
        let file = if self.file_extension.is_empty() {
            self.name.clone()
        } else {
            format!("{}.{}", self.name, self.file_extension)
        };
        if let Some(subdirectory) = &self.subdirectory {
            let path = self.bundle.join(subdirectory).join(&file);
            if path.is_file() {
                return Some(path);
            }
        }
        let path = self.bundle.join(file);
        path.is_file().then_some(path)
    }
    pub fn bundle_root(&self) -> &Path {
        &self.bundle
    }
}
impl std::fmt::Display for BundleImageResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}#", self.bundle.display())?;
        if let Some(directory) = &self.subdirectory {
            write!(f, "{}/", directory.display())?;
        }
        write!(f, "{}", self.name)?;
        if !self.file_extension.is_empty() {
            write!(f, ".{}", self.file_extension)?;
        }
        Ok(())
    }
}
