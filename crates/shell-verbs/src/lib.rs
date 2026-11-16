//! Pure descriptors and plan generators for OS file-menu verbs.
//!
//! This crate has no native dependencies and performs no I/O. The runtime
//! shell uses it for the verb descriptor; the packaging tool uses it to plan
//! registry keys and file-manager action files. Keeping both on one definition
//! is what lets the installer and the tests agree on the exact key layout.

use std::fmt;
use std::path::{Path, PathBuf};

/// A verb published to the OS file menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShellVerb {
    /// Stable id: the registry key name and the id handed to the handler.
    pub id: &'static str,
    /// Menu label; falls back to `id` when absent.
    pub label: Option<&'static str>,
    /// macOS: the `NSMessage` declared in the bundle's `NSServices`.
    pub selector: &'static str,
}

/// An owned verb descriptor for installer manifests and CLI input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnedVerb {
    pub id: String,
    pub label: Option<String>,
}

/// What a generator needs from a verb descriptor.
pub trait VerbDescriptor {
    fn id(&self) -> &str;
    fn label(&self) -> Option<&str>;
}

impl VerbDescriptor for ShellVerb {
    fn id(&self) -> &str {
        self.id
    }
    fn label(&self) -> Option<&str> {
        self.label
    }
}

impl VerbDescriptor for OwnedVerb {
    fn id(&self) -> &str {
        &self.id
    }
    fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
}

/// Why a verb descriptor could not be turned into an install plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerbError {
    /// Id contains characters that are unsafe in a registry key or file name.
    InvalidId(String),
}

impl fmt::Display for VerbError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId(id) => write!(
                formatter,
                "invalid verb id '{id}': use letters, digits, '-' and '_'"
            ),
        }
    }
}

impl std::error::Error for VerbError {}

/// Validates an id before it becomes a registry key name or file stem.
pub fn validate_id(id: &str) -> Result<(), VerbError> {
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(VerbError::InvalidId(id.to_string()));
    }
    Ok(())
}

/// Registry hive a verb key lives under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistryScope {
    /// `HKCU\Software\Classes` — per-user, no elevation.
    CurrentUser,
    /// `HKCR` — machine-wide; writing it requires elevation.
    Machine,
}

impl RegistryScope {
    fn root(self) -> &'static str {
        match self {
            Self::CurrentUser => "HKCU\\Software\\Classes",
            Self::Machine => "HKCR",
        }
    }

    /// Registry hive name used in `reg.exe` commands.
    pub fn hive(self) -> &'static str {
        match self {
            Self::CurrentUser => "HKCU",
            Self::Machine => "HKCR",
        }
    }
}

/// Where Explorer offers the verb.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerbTarget {
    /// Right-click on any file.
    AnyFile,
    /// Right-click on a folder.
    Directory,
    /// Right-click on empty space inside a folder.
    Background,
    /// Right-click on one extension.
    Extension(&'static str),
}

impl VerbTarget {
    /// Path below the hive's `Classes` root (without `\shell\<verb>`).
    pub fn key_suffix(self) -> String {
        match self {
            Self::AnyFile => "*".to_string(),
            Self::Directory => "Directory".to_string(),
            Self::Background => "Directory\\Background".to_string(),
            Self::Extension(extension) => {
                format!(
                    "SystemFileAssociations\\.{}",
                    extension.trim_start_matches('.')
                )
            }
        }
    }
}

/// How Explorer passes a multi-file selection to a static verb.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MultiSelectModel {
    /// One process per selected item (the static-verb default).
    #[default]
    Single,
    /// One process per item, with a document per item.
    Document,
    /// One process carrying the whole selection in its arguments.
    Player,
}

impl MultiSelectModel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Single => "Single",
            Self::Document => "Document",
            Self::Player => "Player",
        }
    }
}

/// One registry value an installer has to write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryEntry {
    pub hive: &'static str,
    /// Full key below the hive; `name: None` means the key's default value.
    pub key: String,
    pub name: Option<String>,
    pub value: String,
}

/// Full key that owns one static verb.
///
/// Shared with the uninstall planner so both sides address the same key.
pub fn windows_verb_key(target: VerbTarget, id: &str, scope: RegistryScope) -> String {
    format!("{}\\{}\\shell\\{id}", scope.root(), target.key_suffix())
}

/// Registry values publishing `verb` as a static Explorer verb.
///
/// Static verbs are legacy entries: Windows 11 folds them into "Show more
/// options", so a top-level entry needs an `IExplorerCommand` COM handler.
/// `MultiSelectModel` defaults to `Single` because a static verb is launched
/// once per selected item; use [`windows_verb_entries_with`] for `Player`.
pub fn windows_verb_entries<V: VerbDescriptor>(
    target: VerbTarget,
    verb: &V,
    exe: &Path,
    scope: RegistryScope,
) -> Vec<RegistryEntry> {
    windows_verb_entries_with(target, verb, exe, scope, MultiSelectModel::Single)
}

/// [`windows_verb_entries`] with an explicit multi-select model.
pub fn windows_verb_entries_with<V: VerbDescriptor>(
    target: VerbTarget,
    verb: &V,
    exe: &Path,
    scope: RegistryScope,
    multi_select: MultiSelectModel,
) -> Vec<RegistryEntry> {
    let id = verb.id();
    let key = windows_verb_key(target, id, scope);
    let command_key = format!("{key}\\command");
    let exe = exe.display().to_string();
    vec![
        RegistryEntry {
            hive: scope.hive(),
            key: key.clone(),
            name: Some("MUIVerb".to_string()),
            value: verb.label().unwrap_or(id).to_string(),
        },
        RegistryEntry {
            hive: scope.hive(),
            key: key.clone(),
            name: Some("Icon".to_string()),
            value: format!("\"{exe}\",0"),
        },
        RegistryEntry {
            hive: scope.hive(),
            key: key.clone(),
            name: Some("MultiSelectModel".to_string()),
            value: multi_select.as_str().to_string(),
        },
        RegistryEntry {
            hive: scope.hive(),
            key: command_key,
            name: None,
            value: format!("\"{exe}\" \"%1\""),
        },
    ]
}

/// File manager whose context menu can be extended without a COM extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinuxManager {
    /// `~/.local/share/nautilus/scripts/<id>`; paths arrive in an env var.
    Nautilus,
    /// `~/.local/share/kio/servicemenus/<id>.desktop`; paths arrive as `%F`.
    Dolphin,
}

/// A file an installer writes below the user's XDG data home.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionFile {
    /// Path relative to the XDG data home.
    pub relative_path: PathBuf,
    pub contents: String,
    /// Whether the installed file needs the executable bit (Nautilus scripts).
    pub executable: bool,
}

/// Plans the action files for the requested file managers.
///
/// `mime_types` filters the Dolphin menu; an empty slice means every file.
pub fn linux_action_files<V: VerbDescriptor>(
    verb: &V,
    exe: &Path,
    mime_types: &[String],
    managers: &[LinuxManager],
) -> Result<Vec<ActionFile>, VerbError> {
    validate_id(verb.id())?;
    let id = verb.id();
    let label = verb.label().unwrap_or(id);
    let mut files = Vec::new();
    for manager in managers {
        match manager {
            LinuxManager::Nautilus => files.push(ActionFile {
                relative_path: PathBuf::from("nautilus").join("scripts").join(id),
                contents: nautilus_script(label, exe),
                executable: true,
            }),
            LinuxManager::Dolphin => files.push(ActionFile {
                relative_path: PathBuf::from("kio")
                    .join("servicemenus")
                    .join(format!("cove-{id}.desktop")),
                contents: dolphin_service_menu(id, label, exe, mime_types),
                executable: false,
            }),
        }
    }
    Ok(files)
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn nautilus_script(label: &str, exe: &Path) -> String {
    format!(
        "#!/bin/sh\n# {label} — generated by Cove; edits will be overwritten.\n\
         old_ifs=$IFS\nIFS='\n'\nset -- $NAUTILUS_SCRIPT_SELECTED_FILE_PATHS\nIFS=$old_ifs\n\
         exec {} \"$@\"\n",
        shell_quote(&exe.display().to_string())
    )
}

fn desktop_quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn dolphin_service_menu(id: &str, label: &str, exe: &Path, mime_types: &[String]) -> String {
    let mimetype = if mime_types.is_empty() {
        "all/allfiles".to_string()
    } else {
        format!("{};", mime_types.join(";"))
    };
    let exe = exe.display().to_string();
    format!(
        "[Desktop Entry]\n\
         Type=Service\n\
         ServiceTypes=KonqPopupMenu/Plugin\n\
         MimeType={mimetype}\n\
         Actions={id};\n\
         X-KDE-StartupNotify=false\n\
         \n\
         [Desktop Action {id}]\n\
         Name={label}\n\
         Icon={exe}\n\
         Exec={} %F\n",
        desktop_quote(&exe)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const VERB: ShellVerb = ShellVerb {
        id: "extract",
        label: Some("Extract here"),
        selector: "coveExtractService:",
    };

    #[test]
    fn registry_key_includes_target_and_verb() {
        assert_eq!(
            windows_verb_key(VerbTarget::AnyFile, "extract", RegistryScope::CurrentUser),
            "HKCU\\Software\\Classes\\*\\shell\\extract"
        );
        assert_eq!(
            windows_verb_key(VerbTarget::Background, "extract", RegistryScope::Machine),
            "HKCR\\Directory\\Background\\shell\\extract"
        );
    }

    #[test]
    fn registry_entries_cover_values_and_command() {
        let entries = windows_verb_entries(
            VerbTarget::Extension(".zip"),
            &VERB,
            Path::new("C:/Apps/zip.exe"),
            RegistryScope::CurrentUser,
        );
        assert_eq!(entries.len(), 4);
        assert_eq!(
            entries[0].key,
            "HKCU\\Software\\Classes\\SystemFileAssociations\\.zip\\shell\\extract"
        );
        assert_eq!(entries[0].name.as_deref(), Some("MUIVerb"));
        assert_eq!(entries[0].value, "Extract here");
        assert_eq!(entries[2].value, "Single");
        assert_eq!(
            entries[3].key,
            "HKCU\\Software\\Classes\\SystemFileAssociations\\.zip\\shell\\extract\\command"
        );
        assert_eq!(entries[3].name, None);
        assert_eq!(entries[3].value, "\"C:/Apps/zip.exe\" \"%1\"");
    }

    #[test]
    fn multi_select_model_is_emitted() {
        let entries = windows_verb_entries_with(
            VerbTarget::AnyFile,
            &VERB,
            Path::new("C:/zip.exe"),
            RegistryScope::CurrentUser,
            MultiSelectModel::Player,
        );
        assert_eq!(entries[2].value, "Player");
    }

    #[test]
    fn label_falls_back_to_id() {
        let verb = OwnedVerb {
            id: "open".into(),
            label: None,
        };
        let entries = windows_verb_entries(
            VerbTarget::AnyFile,
            &verb,
            Path::new("app.exe"),
            RegistryScope::CurrentUser,
        );
        assert_eq!(entries[0].value, "open");
    }

    #[test]
    fn linux_plans_nautilus_script_and_dolphin_menu() {
        let files = linux_action_files(
            &VERB,
            Path::new("/opt/app/bin"),
            &["application/zip".to_string()],
            &[LinuxManager::Nautilus, LinuxManager::Dolphin],
        )
        .unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(
            files[0].relative_path,
            Path::new("nautilus/scripts/extract")
        );
        assert!(files[0].executable);
        assert!(
            files[0]
                .contents
                .contains("NAUTILUS_SCRIPT_SELECTED_FILE_PATHS")
        );
        assert!(files[0].contents.contains("'/opt/app/bin'"));
        assert_eq!(
            files[1].relative_path,
            Path::new("kio/servicemenus/cove-extract.desktop")
        );
        assert!(!files[1].executable);
        assert!(files[1].contents.contains("MimeType=application/zip;"));
        assert!(files[1].contents.contains("[Desktop Action extract]"));
        assert!(files[1].contents.contains("Exec=\"/opt/app/bin\" %F"));
    }

    #[test]
    fn dolphin_menu_defaults_to_all_files() {
        let files =
            linux_action_files(&VERB, Path::new("app"), &[], &[LinuxManager::Dolphin]).unwrap();
        assert!(files[0].contents.contains("MimeType=all/allfiles"));
    }

    #[test]
    fn unsafe_ids_are_rejected() {
        assert_eq!(
            linux_action_files(&VERB, Path::new("app"), &[], &[]).map(|_| ()),
            Ok(())
        );
        let bad = ShellVerb {
            id: "../../evil",
            label: None,
            selector: "",
        };
        assert!(matches!(
            linux_action_files(&bad, Path::new("app"), &[], &[LinuxManager::Nautilus]),
            Err(VerbError::InvalidId(_))
        ));
    }
}
