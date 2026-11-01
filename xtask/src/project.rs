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
}

impl Project {
    pub fn load(package: Option<&str>) -> Result<Self> {
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
        Ok(Self {
            cove,
            root: manifest(entry)
                .parent()
                .ok_or("manifest has no directory")?
                .to_path_buf(),
            target_dir: PathBuf::from(metadata["target_directory"].as_str().unwrap_or("target")),
            bin: target("bin"),
            lib: target("lib").map(|name| name.replace('-', "_")),
            id: cove_metadata["id"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("com.example.{}", name.replace('-', "_"))),
            name: cove_metadata["name"].as_str().unwrap_or(&name).to_owned(),
            version: entry["version"].as_str().unwrap_or("0.1.0").to_owned(),
            build: cove_metadata["build"].as_u64().unwrap_or(1),
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

    pub fn output(&self, requested: Option<&Path>, platform: &str, mode: Mode) -> PathBuf {
        requested.map(Path::to_path_buf).unwrap_or_else(|| {
            self.target_dir
                .join("cove")
                .join(platform)
                .join(mode.name())
        })
    }
}
