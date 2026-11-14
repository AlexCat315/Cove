use std::path::{Path, PathBuf};
/// Rust resource roots replace Foundation.Bundle; lookup falls back to the bundle root.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BundleImageResource {
    pub name: String,
    pub file_extension: String,
    pub bundle: PathBuf,
    pub subdirectory: Option<PathBuf>,
    /// Bytes compiled into the binary. A packaged app carries no source tree,
    /// so an icon that is only reachable through `path()` disappears on
    /// Android; embedding keeps one lookup rule for every platform.
    embedded: Option<&'static [u8]>,
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
            embedded: None,
        }
    }
    /// A vector asset that travels inside the binary instead of on disk.
    pub fn svg_embedded(name: impl Into<String>, data: &'static [u8]) -> Self {
        Self {
            name: name.into(),
            file_extension: "svg".to_owned(),
            bundle: PathBuf::from("<embedded>"),
            subdirectory: None,
            embedded: Some(data),
        }
    }
    /// The compiled-in bytes, when this resource is embedded.
    pub fn embedded(&self) -> Option<&'static [u8]> {
        self.embedded
    }
    pub fn svg(
        name: impl Into<String>,
        bundle: impl Into<PathBuf>,
        subdirectory: Option<PathBuf>,
    ) -> Self {
        Self::new(name, "svg", bundle, subdirectory)
    }
    /// The file name this resource asks for, with its extension.
    fn file_name(&self) -> String {
        if self.file_extension.is_empty() {
            self.name.clone()
        } else {
            format!("{}.{}", self.name, self.file_extension)
        }
    }
    /// The path this resource names, whether or not it exists on disk.
    ///
    /// Diagnostics and extension checks use it for embedded assets too, which
    /// keeps one code path for icons that ship inside the binary.
    pub fn synthetic_path(&self) -> PathBuf {
        match &self.subdirectory {
            Some(subdirectory) => self.bundle.join(subdirectory).join(self.file_name()),
            None => self.bundle.join(self.file_name()),
        }
    }
    pub fn path(&self) -> Option<PathBuf> {
        let file = self.file_name();
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
