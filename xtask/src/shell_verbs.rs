//! Install and remove OS file-menu verbs.
//!
//! Windows writes the static-verb registry keys planned by `cove-shell-verbs`;
//! Linux writes Nautilus scripts and Dolphin service menus into the user's XDG
//! data home. Both paths are driven by an explicit plan so `--dry-run` and the
//! unit tests can inspect every action without touching the machine.

use crate::{Result, process};
use clap::ValueEnum;
use cove_shell_verbs::{
    LinuxManager, MultiSelectModel, OwnedVerb, RegistryEntry, RegistryScope, VerbTarget,
    linux_action_files, validate_id, windows_verb_entries_with, windows_verb_key,
};
#[cfg(windows)]
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum ScopeArg {
    /// `HKCU\Software\Classes`; no elevation required.
    User,
    /// `HKCR`; requires elevation.
    Machine,
}

impl From<ScopeArg> for RegistryScope {
    fn from(value: ScopeArg) -> Self {
        match value {
            ScopeArg::User => Self::CurrentUser,
            ScopeArg::Machine => Self::Machine,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum MultiSelectArg {
    Single,
    Document,
    Player,
}

impl From<MultiSelectArg> for MultiSelectModel {
    fn from(value: MultiSelectArg) -> Self {
        match value {
            MultiSelectArg::Single => Self::Single,
            MultiSelectArg::Document => Self::Document,
            MultiSelectArg::Player => Self::Player,
        }
    }
}

#[derive(Debug)]
pub(crate) struct InstallOptions {
    pub id: String,
    pub label: Option<String>,
    pub targets: Vec<String>,
    pub mime_types: Vec<String>,
    pub exe: PathBuf,
    pub scope: ScopeArg,
    pub multi_select: MultiSelectArg,
    pub data_home: Option<PathBuf>,
    pub dry_run: bool,
}

#[derive(Debug)]
pub(crate) struct UninstallOptions {
    pub id: String,
    pub targets: Vec<String>,
    pub scope: ScopeArg,
    pub data_home: Option<PathBuf>,
    pub dry_run: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PlanAction {
    SetRegistryValue {
        key: String,
        name: Option<String>,
        value: String,
    },
    DeleteRegistryKey {
        key: String,
    },
    WriteFile {
        path: PathBuf,
        contents: String,
        executable: bool,
    },
    RemoveFile {
        path: PathBuf,
    },
    RefreshShell,
}

impl PlanAction {
    fn describe(&self) -> String {
        match self {
            // The key is already fully qualified (`HKCU\Software\Classes\...`).
            Self::SetRegistryValue {
                key, name, value, ..
            } => {
                format!(
                    "{key} [{}] = {value}",
                    name.as_deref().unwrap_or("(default)")
                )
            }
            Self::DeleteRegistryKey { key, .. } => format!("delete {key}"),
            Self::WriteFile {
                path, executable, ..
            } => {
                format!(
                    "write {}{}",
                    path.display(),
                    if *executable { " (executable)" } else { "" }
                )
            }
            Self::RemoveFile { path } => format!("remove {}", path.display()),
            Self::RefreshShell => "refresh shell".to_string(),
        }
    }

    fn apply(&self) -> Result<()> {
        match self {
            Self::SetRegistryValue { key, name, value } => {
                set_registry_value(key, name.as_deref(), value)
            }
            Self::DeleteRegistryKey { key } => delete_registry_key(key),
            Self::WriteFile {
                path,
                contents,
                executable,
            } => {
                process::write(path, contents)?;
                if *executable {
                    make_executable(path)?;
                }
                Ok(())
            }
            Self::RemoveFile { path } => match std::fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(format!("{}: {error}", path.display())),
            },
            Self::RefreshShell => refresh_shell(),
        }
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let permissions = std::fs::Permissions::from_mode(0o755);
    std::fs::set_permissions(path, permissions)
        .map_err(|error| format!("{}: {error}", path.display()))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<()> {
    Ok(())
}

fn registry_command() -> Command {
    Command::new(if cfg!(windows) { "reg" } else { "reg.exe" })
}

fn set_registry_value(key: &str, name: Option<&str>, value: &str) -> Result<()> {
    let mut command = registry_command();
    command.arg("add").arg(key);
    match name {
        Some(name) => {
            command.arg("/v").arg(name);
        }
        None => {
            command.arg("/ve");
        }
    }
    command.arg("/d").arg(value).arg("/f");
    process::run(&mut command)
}

fn registry_key_exists(key: &str) -> bool {
    let mut command = registry_command();
    command.arg("query").arg(key);
    command
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn delete_registry_key(key: &str) -> Result<()> {
    if !registry_key_exists(key) {
        eprintln!("    key absent, skipping: {key}");
        return Ok(());
    }
    let mut command = registry_command();
    command.arg("delete").arg(key).arg("/f");
    process::run(&mut command)
}

#[cfg(windows)]
fn refresh_shell() -> Result<()> {
    const SHCNE_ASSOCCHANGED: i32 = 0x0800_0000;
    const SHCNF_IDLIST: u32 = 0;
    // SAFETY: documented shell notification; the IDLIST flag ignores both
    // item pointers for SHCNE_ASSOCCHANGED, and no pointer is dereferenced.
    unsafe {
        sh_change_notify(
            SHCNE_ASSOCCHANGED,
            SHCNF_IDLIST,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    Ok(())
}

#[cfg(windows)]
#[link(name = "shell32")]
unsafe extern "system" {
    #[link_name = "SHChangeNotify"]
    fn sh_change_notify(event: i32, flags: u32, item1: *const c_void, item2: *const c_void);
}

#[cfg(not(windows))]
fn refresh_shell() -> Result<()> {
    Ok(())
}

fn parse_targets(targets: &[String]) -> Result<Vec<VerbTarget>> {
    if targets.is_empty() {
        return Ok(vec![VerbTarget::AnyFile, VerbTarget::Directory]);
    }
    targets
        .iter()
        .map(|value| match value.as_str() {
            "any-file" => Ok(VerbTarget::AnyFile),
            "directory" => Ok(VerbTarget::Directory),
            "background" => Ok(VerbTarget::Background),
            other if other.starts_with("extension:") => {
                let extension = &other["extension:".len()..];
                if extension.is_empty() {
                    Err(format!("target '{other}' has no extension"))
                } else {
                    // The installer is short-lived; leaking the extension lets
                    // us reuse the runtime's borrowed VerbTarget descriptor.
                    Ok(VerbTarget::Extension(Box::leak(extension.to_string().into_boxed_str())))
                }
            }
            other => Err(format!(
                "unknown target '{other}'; expected any-file, directory, background or extension:<ext>"
            )),
        })
        .collect()
}

fn verb(id: &str, label: Option<&str>) -> OwnedVerb {
    OwnedVerb {
        id: id.to_string(),
        label: label.map(str::to_string),
    }
}

fn data_home(options: Option<&PathBuf>) -> PathBuf {
    if let Some(path) = options {
        return path.clone();
    }
    if let Some(path) = std::env::var_os("XDG_DATA_HOME")
        && !path.is_empty()
    {
        return PathBuf::from(path);
    }
    PathBuf::from(
        std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .unwrap_or_default(),
    )
    .join(".local")
    .join("share")
}

/// Builds the Windows registry actions for installing one verb.
pub(crate) fn plan_windows_install(
    options: &InstallOptions,
    targets: &[VerbTarget],
) -> Result<Vec<PlanAction>> {
    let verb = verb(&options.id, options.label.as_deref());
    let mut actions = Vec::new();
    for target in targets {
        for entry in windows_verb_entries_with(
            *target,
            &verb,
            &options.exe,
            options.scope.into(),
            options.multi_select.into(),
        ) {
            actions.push(registry_action(entry));
        }
    }
    actions.push(PlanAction::RefreshShell);
    Ok(actions)
}

/// Builds the Windows actions for removing one verb.
pub(crate) fn plan_windows_uninstall(
    options: &UninstallOptions,
    targets: &[VerbTarget],
) -> Result<Vec<PlanAction>> {
    let scope: RegistryScope = options.scope.into();
    let mut actions = Vec::new();
    for target in targets {
        actions.push(PlanAction::DeleteRegistryKey {
            key: windows_verb_key(*target, &options.id, scope),
        });
    }
    actions.push(PlanAction::RefreshShell);
    Ok(actions)
}

fn registry_action(entry: RegistryEntry) -> PlanAction {
    PlanAction::SetRegistryValue {
        key: entry.key,
        name: entry.name,
        value: entry.value,
    }
}

const LINUX_MANAGERS: [LinuxManager; 2] = [LinuxManager::Nautilus, LinuxManager::Dolphin];

/// Builds the Linux file actions for installing one verb.
pub(crate) fn plan_linux_install(options: &InstallOptions) -> Result<Vec<PlanAction>> {
    let files = linux_action_files(
        &verb(&options.id, options.label.as_deref()),
        &options.exe,
        &options.mime_types,
        &LINUX_MANAGERS,
    )
    .map_err(|error| error.to_string())?;
    let root = data_home(options.data_home.as_ref());
    let mut actions: Vec<PlanAction> = files
        .into_iter()
        .map(|file| PlanAction::WriteFile {
            path: root.join(file.relative_path),
            contents: file.contents,
            executable: file.executable,
        })
        .collect();
    actions.push(PlanAction::RefreshShell);
    Ok(actions)
}

/// Builds the Linux file actions for removing one verb.
pub(crate) fn plan_linux_uninstall(options: &UninstallOptions) -> Result<Vec<PlanAction>> {
    let files = linux_action_files(
        &verb(&options.id, None),
        Path::new("unused"),
        &[],
        &LINUX_MANAGERS,
    )
    .map_err(|error| error.to_string())?;
    let root = data_home(options.data_home.as_ref());
    let mut actions: Vec<PlanAction> = files
        .into_iter()
        .map(|file| PlanAction::RemoveFile {
            path: root.join(file.relative_path),
        })
        .collect();
    actions.push(PlanAction::RefreshShell);
    Ok(actions)
}

fn unsupported_platform() -> Result<()> {
    Err(
        "shell verbs are managed by Info.plist on macOS; no install/uninstall action is available"
            .to_string(),
    )
}

pub(crate) fn install(options: InstallOptions) -> Result<()> {
    validate_id(&options.id).map_err(|error| error.to_string())?;
    let actions = if cfg!(windows) {
        plan_windows_install(&options, &parse_targets(&options.targets)?)?
    } else if cfg!(target_os = "linux") {
        plan_linux_install(&options)?
    } else {
        return unsupported_platform();
    };
    run_plan(actions, options.dry_run)
}

pub(crate) fn uninstall(options: UninstallOptions) -> Result<()> {
    validate_id(&options.id).map_err(|error| error.to_string())?;
    let actions = if cfg!(windows) {
        plan_windows_uninstall(&options, &parse_targets(&options.targets)?)?
    } else if cfg!(target_os = "linux") {
        plan_linux_uninstall(&options)?
    } else {
        return unsupported_platform();
    };
    run_plan(actions, options.dry_run)
}

fn run_plan(actions: Vec<PlanAction>, dry_run: bool) -> Result<()> {
    for action in actions {
        eprintln!("    {}", action.describe());
        if !dry_run {
            action.apply()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn install_options() -> InstallOptions {
        InstallOptions {
            id: "extract".to_string(),
            label: Some("Extract here".to_string()),
            targets: vec![],
            mime_types: vec!["application/zip".to_string()],
            exe: PathBuf::from("C:/Apps/zip.exe"),
            scope: ScopeArg::User,
            multi_select: MultiSelectArg::Single,
            data_home: None,
            dry_run: true,
        }
    }

    #[test]
    fn windows_install_plans_four_values_per_target() {
        let options = install_options();
        let targets = parse_targets(&options.targets).unwrap();
        let actions = plan_windows_install(&options, &targets).unwrap();
        assert_eq!(actions.len(), 2 * 4 + 1);
        assert!(matches!(actions[0], PlanAction::SetRegistryValue { .. }));
        assert!(matches!(actions.last(), Some(PlanAction::RefreshShell)));
        assert!(
            actions
                .iter()
                .any(|action| action.describe().contains("MultiSelectModel"))
        );
    }

    #[test]
    fn windows_uninstall_removes_the_parent_of_every_written_value() {
        let install = install_options();
        let targets = parse_targets(&install.targets).unwrap();
        let written: Vec<String> = plan_windows_install(&install, &targets)
            .unwrap()
            .into_iter()
            .filter_map(|action| match action {
                PlanAction::SetRegistryValue { key, .. } => Some(key),
                _ => None,
            })
            .collect();
        let options = UninstallOptions {
            id: install.id.clone(),
            targets: install.targets.clone(),
            scope: install.scope,
            data_home: None,
            dry_run: true,
        };
        let deleted: Vec<String> = plan_windows_uninstall(&options, &targets)
            .unwrap()
            .into_iter()
            .filter_map(|action| match action {
                PlanAction::DeleteRegistryKey { key } => Some(key),
                _ => None,
            })
            .collect();
        // `reg delete` removes the key and its `command` subkey recursively, so
        // one delete per target covers all four written values.
        assert_eq!(deleted.len(), targets.len());
        for key in written {
            assert!(
                deleted.iter().any(|parent| key.starts_with(parent)),
                "{key} not covered"
            );
        }
    }

    #[test]
    fn windows_extension_targets_are_parsed() {
        let targets = parse_targets(&[
            "extension:zip".to_string(),
            "background".to_string(),
            "any-file".to_string(),
            "directory".to_string(),
        ])
        .unwrap();
        assert_eq!(targets.len(), 4);
        assert!(matches!(targets[0], VerbTarget::Extension("zip")));
        assert_eq!(targets[0].key_suffix(), "SystemFileAssociations\\.zip");
    }

    #[test]
    fn unknown_target_is_reported() {
        assert!(parse_targets(&["menu".to_string()]).is_err());
    }

    #[test]
    fn linux_install_and_uninstall_address_the_same_files() {
        let temp = tempfile::tempdir().unwrap();
        let mut install = install_options();
        install.exe = PathBuf::from("/opt/app/bin");
        install.data_home = Some(temp.path().to_path_buf());
        let written: Vec<PathBuf> = plan_linux_install(&install)
            .unwrap()
            .into_iter()
            .filter_map(|action| match action {
                PlanAction::WriteFile { path, .. } => Some(path),
                _ => None,
            })
            .collect();
        assert_eq!(written.len(), 2);
        assert!(written[0].ends_with("nautilus/scripts/extract"));
        assert!(written[1].ends_with("kio/servicemenus/cove-extract.desktop"));

        let options = UninstallOptions {
            id: install.id.clone(),
            targets: vec![],
            scope: install.scope,
            data_home: install.data_home.clone(),
            dry_run: true,
        };
        let removed: Vec<PathBuf> = plan_linux_uninstall(&options)
            .unwrap()
            .into_iter()
            .filter_map(|action| match action {
                PlanAction::RemoveFile { path } => Some(path),
                _ => None,
            })
            .collect();
        assert_eq!(removed, written);
    }

    #[test]
    fn linux_files_are_written_and_removed() {
        let temp = tempfile::tempdir().unwrap();
        let mut install = install_options();
        install.exe = PathBuf::from("/opt/app/bin");
        install.data_home = Some(temp.path().to_path_buf());
        let actions = plan_linux_install(&install).unwrap();
        run_plan(actions, false).unwrap();
        let script = temp.path().join("nautilus/scripts/extract");
        let menu = temp.path().join("kio/servicemenus/cove-extract.desktop");
        assert!(script.is_file());
        assert!(menu.is_file());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&script).unwrap().permissions().mode() & 0o111,
                0o111
            );
        }

        let options = UninstallOptions {
            id: install.id.clone(),
            targets: vec![],
            scope: install.scope,
            data_home: install.data_home.clone(),
            dry_run: false,
        };
        run_plan(plan_linux_uninstall(&options).unwrap(), false).unwrap();
        assert!(!script.exists());
        assert!(!menu.exists());
    }
}
