//! The application package, its `[package.metadata.cove]` identity and the
//! Cove checkout Cargo resolved for it.

use crate::process::{self, cargo};
use crate::{Mode, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub struct Project {
    /// Root of the Cove checkout (`crates/hosts/`, `crates/text/fonts/`, ...).
    pub cove: PathBuf,
    /// Directory holding the application's `Cargo.toml`.
    pub root: PathBuf,
    pub target_dir: PathBuf,
    pub package: String,
    pub bin: Option<String>,
    pub lib: Option<String>,
    pub id: String,
    pub name: String,
    pub version: String,
    pub build: u64,
    /// Extra executables shipped beside the application binary, from
    /// `[package.metadata.cove] helpers`. Built with `--bin <name>`.
    pub helpers: Vec<String>,
    /// Additional resource directories copied into the bundle, from
    /// `[package.metadata.cove] resources`. Each declared relative path is
    /// reproduced below the bundle's resource root.
    pub resources: Vec<PathBuf>,
}

impl Project {
    pub fn load(package: Option<&str>) -> Result<Self> {
        Self::load_for_run(package, false)
    }

    pub fn load_for_run(package: Option<&str>, gallery_default: bool) -> Result<Self> {
        let text = process::output(cargo().args(["metadata", "--format-version", "1"]))?;
        let metadata: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
        let packages = metadata["packages"]
            .as_array()
            .ok_or("cargo metadata has no packages")?;
        let manifest =
            |entry: &Value| PathBuf::from(entry["manifest_path"].as_str().unwrap_or_default());
        let workspace_root = PathBuf::from(metadata["workspace_root"].as_str().unwrap_or_default());
        let members: Vec<&str> = metadata["workspace_members"]
            .as_array()
            .map(|ids| ids.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();

        let entry = match package {
            Some(name) => packages
                .iter()
                .find(|entry| entry["name"] == name)
                .ok_or_else(|| format!("no package named {name}"))?,
            None => packages
                .iter()
                .filter(|entry| members.contains(&entry["id"].as_str().unwrap_or_default()))
                .find(|entry| manifest(entry) == workspace_root.join("Cargo.toml"))
                .and_then(|root| {
                    if gallery_default && root["name"] == "cove-ui" {
                        packages.iter().find(|entry| {
                            manifest(entry) == workspace_root.join("examples/gallery/Cargo.toml")
                        })
                    } else {
                        Some(root)
                    }
                })
                .ok_or("the workspace root is not an application package; pass --package")?,
        };
        let hosts = packages
            .iter()
            .find(|entry| entry["name"] == "cove-hosts")
            .ok_or("the application does not depend on cove-ui")?;
        let cove = manifest(hosts)
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .ok_or("cannot locate the Cove checkout")?
            .to_path_buf();

        let name = entry["name"].as_str().unwrap_or_default().to_owned();
        let target = |kind: &str| {
            entry["targets"].as_array().and_then(|targets| {
                targets
                    .iter()
                    .find(|target| {
                        target["kind"]
                            .as_array()
                            .is_some_and(|kinds| kinds.iter().any(|value| value == kind))
                    })
                    .and_then(|target| target["name"].as_str())
                    .map(str::to_owned)
            })
        };
        let cove_metadata = &entry["metadata"]["cove"];
        let declared = |key: &str| -> Vec<String> {
            cove_metadata[key]
                .as_array()
                .map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default()
        };
        Ok(Self {
            cove,
            root: manifest(entry)
                .parent()
                .ok_or("manifest has no directory")?
                .to_path_buf(),
            target_dir: PathBuf::from(metadata["target_directory"].as_str().unwrap_or("target")),
            bin: entry["default_run"]
                .as_str()
                .map(str::to_owned)
                .or_else(|| target("bin")),
            lib: target("lib").map(|name| name.replace('-', "_")),
            id: cove_metadata["id"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("com.example.{}", name.replace('-', "_"))),
            name: cove_metadata["name"].as_str().unwrap_or(&name).to_owned(),
            version: entry["version"].as_str().unwrap_or("0.1.0").to_owned(),
            build: cove_metadata["build"].as_u64().unwrap_or(1),
            helpers: declared("helpers"),
            resources: declared("resources").into_iter().map(PathBuf::from).collect(),
            package: name,
        })
    }

    pub fn bin(&self) -> Result<&str> {
        self.bin
            .as_deref()
            .ok_or_else(|| format!("{} has no binary target", self.package))
    }

    pub fn lib(&self) -> Result<&str> {
        self.lib
            .as_deref()
            .ok_or_else(|| format!("{} has no library target", self.package))
    }

    pub fn platforms(&self, platform: &str) -> PathBuf {
        self.root.join("Platforms").join(platform)
    }

    /// `cargo build` of the binary; returns the built executable.
    pub fn build_bin(&self, mode: Mode, target: Option<&str>) -> Result<PathBuf> {
        let bin = self.bin()?;
        let mut command = cargo();
        command
            .current_dir(&self.root)
            .args(["build", "--package", &self.package, "--bin", bin]);
        if mode.optimized() {
            command.arg("--release");
        }
        if let Some(target) = target {
            command.args(["--target", target]);
            if target.contains("apple-ios") {
                command.env("IPHONEOS_DEPLOYMENT_TARGET", crate::apple::MINIMUM_VERSION);
            }
        }
        process::run(&mut command)?;
        let windows = target.map_or(cfg!(windows), |target| target.contains("windows"));
        Ok(self.artifacts(mode, target).join(if windows {
            format!("{bin}.exe")
        } else {
            bin.to_owned()
        }))
    }

    pub fn artifacts(&self, mode: Mode, target: Option<&str>) -> PathBuf {
        let mut directory = self.target_dir.clone();
        if let Some(target) = target {
            directory.push(target);
        }
        directory.join(if mode.optimized() { "release" } else { "debug" })
    }

    /// Stages `Assets/` as the `AppContent.resources` bundle inside `directory`.
    pub fn stage_resources(&self, directory: &Path) -> Result<()> {
        let bundle = directory.join("AppContent.resources");
        process::remove_dir(&bundle)?;
        process::copy_tree(&self.root.join("Assets"), &bundle.join("Assets"))
    }

    /// Builds one declared helper with `--bin <helper>` and returns its path.
    pub fn build_helper(&self, helper: &str, mode: Mode, target: Option<&str>) -> Result<PathBuf> {
        let mut command = cargo();
        command
            .current_dir(&self.root)
            .args(["build", "--bin", helper]);
        if mode.optimized() {
            command.arg("--release");
        }
        if let Some(target) = target {
            command.args(["--target", target]);
        }
        process::run(&mut command)?;
        let windows = target.map_or(cfg!(windows), |target| target.contains("windows"));
        let executable = self.artifacts(mode, target).join(if windows {
            format!("{helper}.exe")
        } else {
            helper.to_owned()
        });
        if !executable.is_file() {
            return Err(format!(
                "helper {helper} did not produce {}",
                executable.display()
            ));
        }
        Ok(executable)
    }

    /// Stages every declared helper beside the application binary. Cocoa
    /// bundles keep helpers in `Contents/MacOS`, where the signing step covers
    /// them; desktop folders and iOS app roots hold executables directly.
    pub fn stage_helpers(
        &self,
        directory: &Path,
        mode: Mode,
        target: Option<&str>,
    ) -> Result<()> {
        for helper in &self.helpers {
            let executable = self.build_helper(helper, mode, target)?;
            process::copy(
                &executable,
                &directory.join(executable.file_name().expect("helper has a name")),
            )?;
        }
        Ok(())
    }

    /// Copies every declared resource directory into `directory`, keeping the
    /// relative path that was declared so the application can look assets up
    /// with the same path it uses in the source tree.
    pub fn stage_extra_resources(&self, directory: &Path) -> Result<()> {
        for resource in &self.resources {
            let source = self.root.join(resource);
            if !source.is_dir() {
                return Err(format!(
                    "declared resource directory {} does not exist",
                    source.display()
                ));
            }
            process::copy_tree(&source, &directory.join(resource))?;
        }
        Ok(())
    }

    /// Android packages run code out of the APK, so a helper executable has no
    /// place to run. Fail instead of shipping an inert file.
    pub fn reject_helpers_for_android(&self) -> Result<()> {
        if self.helpers.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "Android packages cannot run helper executables (declared: {})",
                self.helpers.join(", ")
            ))
        }
    }

    pub fn output(&self, requested: Option<&Path>, platform: &str, mode: Mode) -> PathBuf {
        requested.map(Path::to_path_buf).unwrap_or_else(|| {
            self.target_dir
                .join("cove")
                .join(platform)
                .join(mode.name())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Mode;

    fn project(root: &Path, target_dir: &Path) -> Project {
        Project {
            cove: root.to_path_buf(),
            root: root.to_path_buf(),
            target_dir: target_dir.to_path_buf(),
            package: "app".to_owned(),
            bin: Some("app".to_owned()),
            lib: None,
            id: "com.example.app".to_owned(),
            name: "App".to_owned(),
            version: "0.1.0".to_owned(),
            build: 1,
            helpers: vec!["plugin-host".to_owned()],
            resources: vec![PathBuf::from("RenderBackend/Resources")],
        }
    }

    #[test]
    fn extra_resources_keep_the_declared_relative_path() {
        let root = tempfile::tempdir().expect("source tree");
        let output = tempfile::tempdir().expect("bundle");
        std::fs::create_dir_all(root.path().join("RenderBackend/Resources/shaders")).expect("dirs");
        std::fs::write(
            root.path().join("RenderBackend/Resources/shaders/main.slang"),
            "shader",
        )
        .expect("write");
        let project = project(root.path(), output.path());
        let bundle = output.path().join("App.app");
        project.stage_extra_resources(&bundle).expect("staged");
        assert!(
            bundle
                .join("RenderBackend/Resources/shaders/main.slang")
                .is_file()
        );
    }

    #[test]
    fn a_missing_declared_resource_directory_is_reported() {
        let root = tempfile::tempdir().expect("source tree");
        let output = tempfile::tempdir().expect("bundle");
        let project = project(root.path(), output.path());
        let error = project
            .stage_extra_resources(&output.path().join("App.app"))
            .expect_err("missing directory");
        assert!(error.contains("RenderBackend/Resources"));
    }

    #[test]
    fn android_rejects_helper_executables() {
        let root = tempfile::tempdir().expect("source tree");
        let project = project(root.path(), root.path());
        let error = project
            .reject_helpers_for_android()
            .expect_err("helpers cannot run on Android");
        assert!(error.contains("plugin-host"));
    }

    #[test]
    fn a_project_without_helpers_is_accepted_by_android() {
        let root = tempfile::tempdir().expect("source tree");
        let mut project = project(root.path(), root.path());
        project.helpers.clear();
        project.reject_helpers_for_android().expect("no helpers");
    }

    #[test]
    fn declared_resources_are_relative_to_the_package_root() {
        let root = tempfile::tempdir().expect("source tree");
        let project = project(root.path(), root.path());
        assert_eq!(
            project.root.join(&project.resources[0]),
            root.path().join("RenderBackend/Resources")
        );
        assert_eq!(Mode::Debug.name(), "debug");
    }

    #[test]
    fn a_declared_helper_is_built_and_staged_beside_the_app() {
        let root = tempfile::tempdir().expect("source tree");
        std::fs::write(
            root.path().join("Cargo.toml"),
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
             [[bin]]\nname = \"plugin-host\"\npath = \"src/main.rs\"\n\n[workspace]\n",
        )
        .expect("manifest");
        std::fs::create_dir_all(root.path().join("src")).expect("src");
        std::fs::write(root.path().join("src/main.rs"), "fn main() {}\n").expect("source");

        let target_dir = root.path().join("target");
        let project = project(root.path(), &target_dir);
        let bundle = tempfile::tempdir().expect("bundle");

        project
            .stage_helpers(bundle.path(), Mode::Debug, None)
            .expect("the helper builds and stages");

        assert!(
            bundle.path().join("plugin-host").is_file(),
            "the helper lands beside the app binary"
        );
    }
}
