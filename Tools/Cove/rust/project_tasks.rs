//! `cove.workspace.json` tasks and the SwiftPM workspace around them.
//!
//! Swift sources: `Project/Tasks/Dependencies.swift`,
//! `Project/Tasks/Runner.swift`, `Project/Tasks/Workspace.swift`.
//!
//! SwiftPM still owns resolution and lockfiles; Cove only decides when declared
//! branches refresh.

use crate::cli::{standardized, AppPlatform, Arguments};
use crate::platforms_desktop::SwiftPackage;
use crate::project::{is_entry_name, is_module_name, is_product_name, is_task_product};
use crate::project_sdk::CoveSDK;
use crate::support::{Files, Shell, StdoutTarget, ToolError};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const WORKSPACE_FILENAME: &str = "cove.workspace.json";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub package: String,
    pub product: String,
    pub entry: String,
    #[serde(default)]
    pub module: Option<String>,
    #[serde(default)]
    pub arguments: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub platforms: Option<Vec<AppPlatform>>,
}

impl Task {
    pub fn module(&self) -> &str {
        self.module.as_deref().unwrap_or(&self.product)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencyUpdates {    Locked,
    Branches,
}

impl Default for DependencyUpdates {
    fn default() -> Self {
        Self::Locked
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceConfiguration {
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    pub dependency_updates: DependencyUpdates,
    #[serde(default)]
    pub before_build: Vec<String>,
    #[serde(default)]
    pub tasks: BTreeMap<String, Task>,
}

pub struct ProjectWorkspace {
    pub directory: PathBuf,
    pub configuration: WorkspaceConfiguration,
}

impl ProjectWorkspace {
    pub fn new(at: &Path) -> Result<Self, ToolError> {
        let directory = resolve(at)?;
        let data = std::fs::read(directory.join(WORKSPACE_FILENAME)).map_err(|error| {
            ToolError::invalid(format!(
                "{}: {error}",
                directory.join(WORKSPACE_FILENAME).display()
            ))
        })?;
        let configuration: WorkspaceConfiguration = serde_json::from_slice(&data)
            .map_err(|error| ToolError::invalid(format!("{WORKSPACE_FILENAME}: {error}")))?;
        let workspace = Self {
            directory,
            configuration,
        };
        workspace.validate()?;
        Ok(workspace)
    }

    pub fn find(from: &Path) -> Result<Option<Self>, ToolError> {
        let mut current = resolve(from)?;
        loop {
            if current.join(WORKSPACE_FILENAME).is_file() {
                return Self::new(&current).map(Some);
            }
            let Some(parent) = current.parent() else {
                return Ok(None);
            };
            if parent == current {
                return Ok(None);
            }
            current = parent.to_path_buf();
        }
    }

    pub fn package(&self, relative: &str) -> Result<PathBuf, ToolError> {
        if relative.is_empty()
            || Path::new(relative).is_absolute()
            || relative.starts_with('/')
            || relative.starts_with('\\')
        {
            return Err(ToolError::invalid(
                "Task packages must be relative to the workspace",
            ));
        }
        let result = resolve(&self.directory.join(relative))?;
        let inside = result == self.directory
            || result
                .strip_prefix(&self.directory)
                .map(|rest| !rest.as_os_str().is_empty())
                .unwrap_or(false);
        if !inside || !result.join("Package.swift").is_file() {
            return Err(ToolError::invalid(format!(
                "Workspace package must exist inside the project: {relative}"
            )));
        }
        Ok(result)
    }

    pub fn ordered_tasks(&self, names: &[String]) -> Result<Vec<String>, ToolError> {
        fn visit(
            workspace: &ProjectWorkspace,
            name: &str,
            active: &mut BTreeSet<String>,
            complete: &mut BTreeSet<String>,
            ordered: &mut Vec<String>,
        ) -> Result<(), ToolError> {
            if complete.contains(name) {
                return Ok(());
            }
            let Some(task) = workspace.configuration.tasks.get(name) else {
                return Err(ToolError::invalid(format!("Unknown project task: {name}")));
            };
            if !active.insert(name.to_string()) {
                return Err(ToolError::invalid(format!(
                    "Project task dependency cycle at {name}"
                )));
            }
            for dependency in &task.depends_on {
                visit(workspace, dependency, active, complete, ordered)?;
            }
            active.remove(name);
            complete.insert(name.to_string());
            ordered.push(name.to_string());
            Ok(())
        }

        let mut active = BTreeSet::new();
        let mut complete = BTreeSet::new();
        let mut ordered = Vec::new();
        for name in names {
            visit(self, name, &mut active, &mut complete, &mut ordered)?;
        }
        Ok(ordered)
    }

    fn validate(&self) -> Result<(), ToolError> {
        let mut target_names = BTreeSet::new();
        for (name, task) in &self.configuration.tasks {
            let valid = is_product_name(name)
                && is_task_product(&task.product)
                && is_module_name(task.module())
                && is_entry_name(&task.entry)
                && target_names.insert(name.replace('-', "_"));
            if !valid {
                return Err(ToolError::invalid(format!(
                    "Invalid project task declaration: {name}"
                )));
            }
            let _ = self.package(&task.package)?;
        }
        for path in &self.configuration.packages {
            let _ = self.package(path)?;
        }
        let names: Vec<String> = self
            .configuration
            .tasks
            .keys()
            .cloned()
            .chain(self.configuration.before_build.iter().cloned())
            .collect();
        let _ = self.ordered_tasks(&names)?;
        Ok(())
    }
}

/// Resolves symlinks where possible, matching `resolvingSymlinksInPath`.
fn resolve(path: &Path) -> Result<PathBuf, ToolError> {
    let path = standardized(path);
    Ok(std::fs::canonicalize(&path).unwrap_or(path))
}

/// SwiftPM owns resolution and lockfiles; only declared branches are refreshed.
pub struct WorkspaceDependencies;

impl WorkspaceDependencies {
    #[derive(Deserialize)]
    struct Lockfile {
        #[serde(default)]
        pins: Vec<Pin>,
    }

    #[derive(Deserialize)]
    struct Pin {
        identity: String,
        location: String,
        state: State,
    }

    #[derive(Deserialize)]
    struct State {
        #[serde(default)]
        branch: Option<String>,
        #[serde(default)]
        revision: Option<String>,
    }

    pub fn refresh(
        workspace: &ProjectWorkspace,
        sdk: &Path,
        environment: &BTreeMap<String, String>,
    ) -> Result<(), ToolError> {
        if workspace.configuration.dependency_updates != DependencyUpdates::Branches
            || !environment
                .get("CI")
                .map(String::is_empty)
                .unwrap_or(true)
        {
            return Ok(());
        }
        let mut heads: BTreeMap<String, String> = BTreeMap::new();
        let build_environment = SwiftPackage::environment(sdk)?;
        for path in &workspace.configuration.packages {
            let package = workspace.package(path)?;
            let Ok(data) = std::fs::read(package.join("Package.resolved")) else {
                continue;
            };
            let Ok(lock) = serde_json::from_slice::<Lockfile>(&data) else {
                continue;
            };
            let mut changed = Vec::new();
            for pin in lock.pins {
                let Some(branch) = pin.state.branch else {
                    continue;
                };
                let reference = format!("refs/heads/{branch}");
                let key = format!("{}\n{}", pin.location, reference);
                if !heads.contains_key(&key) {
                    let Ok(output) = Shell::run(
                        &[
                            "git".into(),
                            "-c".into(),
                            "http.lowSpeedLimit=1000".into(),
                            "-c".into(),
                            "http.lowSpeedTime=5".into(),
                            "ls-remote".into(),
                            pin.location.clone(),
                            reference.clone(),
                        ],
                        None,
                        environment,
                        true,
                        true,
                        StdoutTarget::StandardOutput,
                    ) else {
                        continue;
                    };
                    let Some(head) = output.split_whitespace().next() else {
                        continue;
                    };
                    heads.insert(key.clone(), head.to_string());
                }
                if heads.get(&key) == pin.state.revision.as_ref() {
                    continue;
                }
                let repositories = package.join(".build/repositories");
                let cached = Files::children(&repositories).unwrap_or_default();
                let prefix = format!("{}-", pin.identity.to_lowercase());
                for repository in cached {
                    let name = repository
                        .file_name()
                        .map(|name| name.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    if name.starts_with(&prefix) {
                        Shell::run(
                            &[
                                "git".into(),
                                "-C".into(),
                                repository.display().to_string(),
                                "fetch".into(),
                                "origin".into(),
                            ],
                            None,
                            &build_environment,
                            false,
                            false,
                            StdoutTarget::StandardOutput,
                        )?;
                    }
                }
                changed.push(pin.identity);
            }
            if !changed.is_empty() {
                let mut command = vec![
                    CoveSDK::swift(),
                    "package".into(),
                    "--package-path".into(),
                    package.display().to_string(),
                    "--skip-update".into(),
                    "update".into(),
                ];
                command.extend(changed);
                Shell::run(
                    &command,
                    None,
                    &build_environment,
                    false,
                    false,
                    StdoutTarget::StandardOutput,
                )?;
            }
        }
        Ok(())
    }
}

use crate::project_sdk::CoveSDK;

pub struct ProjectTaskRunner {
    pub arguments: Arguments,
    pub sdk: PathBuf,
}

impl ProjectTaskRunner {
    pub fn new(arguments: &Arguments, sdk: &Path) -> Self {
        Self {
            arguments: arguments.clone(),
            sdk: sdk.to_path_buf(),
        }
    }

    pub fn run(&self) -> Result<(), ToolError> {
        let Some(workspace) = ProjectWorkspace::find(&self.arguments.directory())? else {
            return Err(ToolError::invalid(format!(
                "No {WORKSPACE_FILENAME} found for this project"
            )));
        };
        let Some(name) = self.arguments.task_name.clone() else {
            for name in workspace.configuration.tasks.keys() {
                println!("{name}");
            }
            return Ok(());
        };
        for task in workspace.ordered_tasks(&[name.clone()])? {
            let extra = if task == name {
                self.arguments.application_arguments.clone()
            } else {
                Vec::new()
            };
            self.execute(&task, &workspace, &extra)?;
        }
        Ok(())
    }

    pub fn run_before_build(&self, workspace: &ProjectWorkspace) -> Result<(), ToolError> {
        for task in workspace.ordered_tasks(&workspace.configuration.before_build)? {
            self.execute(&task, workspace, &[])?;
        }
        Ok(())
    }

    pub fn prepare(&self, workspace: &ProjectWorkspace) -> Result<PathBuf, ToolError> {
        let cache = workspace.directory.join(".build/tasks");
        let resolved = resolve(&cache)?;
        let inside = resolved
            .strip_prefix(&workspace.directory)
            .map(|rest| !rest.as_os_str().is_empty())
            .unwrap_or(false);
        if !inside {
            return Err(ToolError::invalid(
                "Project task cache must remain inside the workspace",
            ));
        }
        let host = AppPlatform::host();
        let tasks: BTreeMap<&String, &Task> = workspace
            .configuration
            .tasks
            .iter()
            .filter(|(_, task)| {
                task.platforms
                    .as_ref()
                    .map(|platforms| platforms.contains(&host))
                    .unwrap_or(true)
            })
            .collect();
        let paths: BTreeSet<String> = tasks.values().map(|task| task.package.clone()).collect();
        let aliases: BTreeMap<String, String> = paths
            .iter()
            .enumerate()
            .map(|(index, path)| (path.clone(), format!("ProjectTasks{index}")))
            .collect();
        let mut dependencies = Vec::new();
        for path in &paths {
            dependencies.push(format!(
                ".package(name: {}, path: {})",
                Files::swift_literal(&aliases[path]),
                Files::swift_literal(&workspace.package(path)?.display().to_string())
            ));
        }
        let mut targets = Vec::new();
        let mut products = Vec::new();
        for (name, task) in &tasks {
            let target = target_name(name);
            products.push(format!(
                ".executable(name: {}, targets: [{}])",
                Files::swift_literal(&target),
                Files::swift_literal(&target)
            ));
            targets.push(format!(
                ".executableTarget(name: {}, dependencies: [.product(name: {}, package: {})])",
                Files::swift_literal(&target),
                Files::swift_literal(&task.product),
                Files::swift_literal(&aliases[&task.package])
            ));
            let source = format!(
                "import Foundation\nimport {}\ndo {{ try {}(Array(CommandLine.arguments.dropFirst())) }}\ncatch {{\n    FileHandle.standardError.write(Data((String(describing: error) + \"\\n\").utf8))\n    exit(1)\n}}\n",
                task.module(),
                task.entry
            );
            self.write(
                &source,
                &cache.join(format!("Sources/{target}/main.swift")),
            )?;
        }
        let manifest = format!(
            "// swift-tools-version: 6.4\n// Generated by Cove SDK from cove.workspace.json.\nimport PackageDescription\nlet package = Package(name: \"CoveProjectTasks\", platforms: [.macOS(.v26)],\n    products: [{}],\n    dependencies: [{}],\n    targets: [{}])\n",
            products.join(",\n"),
            dependencies.join(",\n"),
            targets.join(",\n")
        );
        self.write(&manifest, &cache.join("Package.swift"))?;
        Ok(cache)
    }

    fn execute(
        &self,
        name: &str,
        workspace: &ProjectWorkspace,
        extra: &[String],
    ) -> Result<(), ToolError> {
        let task = workspace
            .configuration
            .tasks
            .get(name)
            .ok_or_else(|| ToolError::invalid(format!("Unknown project task: {name}")))?;
        let host = AppPlatform::host();
        if !task
            .platforms
            .as_ref()
            .map(|platforms| platforms.contains(&host))
            .unwrap_or(true)
        {
            return Err(ToolError::invalid(format!(
                "Project task {name} is unavailable on this host"
            )));
        }
        let cache = self.prepare(workspace)?;
        let product = target_name(name);
        let mut build = SwiftPackage::command(
            &cache,
            self.arguments.configuration,
            "build",
            None,
        );
        build.push("--skip-update".into());
        let mut environment = SwiftPackage::environment(&self.sdk)?;
        environment.insert(
            "COVE_ROOT".into(),
            workspace.directory.display().to_string(),
        );
        let key = format!("{}:{name}", workspace.directory.display());
        let active: Vec<String> = environment
            .get("COVE_PROJECT_TASK_STACK")
            .map(|value| value.split('\n').map(str::to_string).collect())
            .unwrap_or_default();
        if active.contains(&key) {
            return Err(ToolError::invalid(format!(
                "Recursive project task invocation: {name}"
            )));
        }
        let mut stack = active;
        stack.push(key);
        environment.insert("COVE_PROJECT_TASK_STACK".into(), stack.join("\n"));
        let mut build_product = build.clone();
        build_product.push("--product".into());
        build_product.push(product.clone());
        Shell::run(
            &build_product,
            Some(&workspace.directory),
            &environment,
            false,
            false,
            StdoutTarget::StandardError,
        )?;
        let mut path_command = build;
        path_command.push("--show-bin-path".into());
        let binaries = Shell::run(
            &path_command,
            None,
            &environment,
            true,
            true,
            StdoutTarget::StandardOutput,
        )?;
        let suffix = if host == AppPlatform::Win { ".exe" } else { "" };
        let mut command = vec![Path::new(binaries.trim())
            .join(format!("{product}{suffix}"))
            .display()
            .to_string()];
        command.extend(task.arguments.iter().cloned());
        command.extend(extra.iter().cloned());
        Shell::run(
            &command,
            Some(&workspace.directory),
            &environment,
            false,
            false,
            StdoutTarget::StandardOutput,
        )?;
        Ok(())
    }

    fn write(&self, value: &str, file: &Path) -> Result<(), ToolError> {
        if std::fs::read_to_string(file).ok().as_deref() != Some(value) {
            Files::text(value, file)?;
        }
        Ok(())
    }
}

fn target_name(name: &str) -> String {
    format!("CoveTask_{}", name.replace('-', "_"))
}
