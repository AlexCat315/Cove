//! Optional, explicit shell integration.
//!
//! `cargo cove setup` installs pinned tools into Cove's private cache and records
//! their absolute paths in the setup state. Build scripts read that state through
//! `setup::apply`, so Cargo builds work without touching the user's environment.
//! That is deliberate: silently editing a user's PATH would leak project-specific
//! tool versions into every other shell on the machine.
//!
//! A developer who also wants `zig`/`slangc` in their own terminal can opt in
//! with `cargo cove setup --persist-path` (reversible with `--unpersist-path`),
//! or load the environment for one shell with `cargo cove env`. Only directories
//! under the Cove cache are ever persisted; host SDK/compiler paths stay in the
//! setup state and are injected into child processes there.

use serde_json::Value;
use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

const BEGIN: &str = "# >>> cove setup (managed) >>>";
const END: &str = "# <<< cove setup (managed) <<<";

/// Environment syntax a shell needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shell {
    Posix,
    PowerShell,
}

impl Shell {
    /// The syntax matching the host's default shell.
    pub fn host_default() -> Self {
        if cfg!(windows) {
            Self::PowerShell
        } else {
            Self::Posix
        }
    }
}

/// Pinned-tool directories that Cove installed into its own cache.
///
/// Host SDK paths (MSVC, Homebrew, MSYS2, …) are intentionally excluded: they
/// belong to the host toolchain and are already merged into the setup state's
/// PATH for build scripts. Copying them into a user's persistent PATH would
/// freeze one process's snapshot of the machine into every future shell.
pub fn tool_directories(saved: &Value) -> Vec<PathBuf> {
    let Ok(cache) = super::cache_dir() else {
        return Vec::new();
    };
    let mut directories: Vec<PathBuf> = Vec::new();
    for key in ["COVE_ZIG", "SLANGC"] {
        let Some(binary) = saved["environment"][key].as_str() else {
            continue;
        };
        let Some(parent) = Path::new(binary).parent() else {
            continue;
        };
        if parent.starts_with(&cache) && !directories.contains(&parent.to_path_buf()) {
            directories.push(parent.to_path_buf());
        }
    }
    directories
}

/// Statements that add Cove's pinned tools to the *current* shell only.
pub fn exports(saved: &Value, shell: Shell) -> String {
    let directories = tool_directories(saved);
    let mut lines = Vec::new();
    match shell {
        Shell::Posix => {
            // A POSIX shell (Git Bash/MSYS on Windows included) separates PATH
            // with ':' and cannot use Windows drive letters. `env::join_paths`
            // uses the host separator, so it would emit a broken PATH here.
            let joined = directories
                .iter()
                .map(|path| to_posix_path(path))
                .collect::<Vec<_>>()
                .join(":");
            lines.push(format!(
                "export PATH=\"{}:${{PATH}}\"",
                quote_posix(&joined)
            ));
            for key in ["COVE_ZIG", "SLANGC"] {
                if let Some(value) = saved["environment"][key].as_str() {
                    lines.push(format!("export {key}=\"{}\"", quote_posix(value)));
                }
            }
        }
        Shell::PowerShell => {
            let joined = env::join_paths(&directories)
                .map(|value| value.to_string_lossy().into_owned())
                .unwrap_or_default();
            lines.push(format!(
                "$env:PATH = \"{};\" + $env:PATH",
                quote_ps(&joined)
            ));
            for key in ["COVE_ZIG", "SLANGC"] {
                if let Some(value) = saved["environment"][key].as_str() {
                    lines.push(format!("$env:{key} = \"{}\"", quote_ps(value)));
                }
            }
        }
    }
    lines.join("\n")
}

/// Converts a Windows path to the MSYS form a POSIX shell can put in PATH
/// (`C:\Users\me\Cove` → `/c/Users/me/Cove`). Other hosts keep their paths.
fn to_posix_path(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let bytes = text.as_bytes();
    if bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/' {
        let drive = (bytes[0] as char).to_ascii_lowercase();
        return format!("/{drive}{}", &text[2..]);
    }
    text
}

fn quote_posix(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('`', "\\`")
}

fn quote_ps(value: &str) -> String {
    // PowerShell expands `$` and treats backticks as escapes inside double quotes.
    value
        .replace('`', "``")
        .replace('$', "`$")
        .replace('"', "`\"")
}

/// The marked profile block used on macOS and Linux.
pub fn profile_block(directories: &[PathBuf]) -> String {
    let joined = env::join_paths(directories)
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_default();
    format!(
        "{BEGIN}\n# Added by `cargo cove setup --persist-path`; remove with `--unpersist-path`.\nexport PATH=\"{}:$PATH\"\n{END}\n",
        quote_posix(&joined)
    )
}

/// Replaces (or removes) the managed block in a shell profile. Pure so it can
/// be tested without touching the developer's dotfiles.
pub fn replace_block(content: &str, block: Option<&str>) -> String {
    let mut result = content.to_owned();
    if let Some(start) = result.find(BEGIN) {
        let end = result[start..]
            .find(END)
            .map(|offset| start + offset + END.len())
            .unwrap_or(result.len());
        let mut end = end.min(result.len());
        if result[end..].starts_with("\r\n") {
            end += 2;
        } else if result[end..].starts_with('\n') {
            end += 1;
        }
        result.replace_range(start..end, "");
    }
    if let Some(block) = block {
        if !result.is_empty() && !result.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(block);
    }
    result
}

/// Shell profile that receives the managed block.
pub fn profile_path() -> io::Result<PathBuf> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("HOME is not set; cannot locate a shell profile"))?;
    let shell = env::var("SHELL").unwrap_or_default();
    Ok(if shell.contains("zsh") {
        home.join(".zshrc")
    } else {
        home.join(".bashrc")
    })
}

/// PowerShell that rewrites the *user-level* PATH.
///
/// `setx` and `[Environment]::SetEnvironmentVariable` are both avoided: the
/// former truncates PATH at 1024 characters, and the latter expands `%VAR%`
/// placeholders before writing them back, permanently turning a user's
/// `REG_EXPAND_SZ` PATH into literal paths. Reading the registry value with
/// `DoNotExpandEnvironmentNames` and writing it with its original kind keeps
/// the user's environment intact. Only entries under the Cove cache are
/// removed, so this is idempotent and reversible. Nothing is written when the
/// resulting value is unchanged (e.g. `--unpersist-path` on a clean machine).
pub fn powershell_script(cache: &Path, directories: &[PathBuf], remove: bool) -> String {
    let literal = |value: &str| format!("'{}'", value.replace('\'', "''"));
    let additions = if remove {
        "@()".to_owned()
    } else {
        format!(
            "@({})",
            directories
                .iter()
                .map(|path| literal(&path.to_string_lossy()))
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    format!(
        "$ErrorActionPreference = 'Stop'; \
         $cache = {}; \
         $add = {additions}; \
         $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true); \
         if ($null -eq $key) {{ throw 'HKCU\\Environment is unavailable' }}; \
         try {{ $kind = $key.GetValueKind('PATH') }} catch {{ $kind = [Microsoft.Win32.RegistryValueKind]::ExpandString }}; \
         $raw = $key.GetValue('PATH', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames); \
         if ($null -eq $raw) {{ $raw = '' }}; \
         $keep = @($raw.Split(';') | Where-Object {{ $_ -and -not $_.StartsWith($cache, [StringComparison]::OrdinalIgnoreCase) }}); \
         $updated = (($add + $keep) -join ';'); \
         if ($updated -ne $raw) {{ $key.SetValue('PATH', $updated, $kind) }}; \
         $key.Close()",
        literal(&cache.to_string_lossy()),
    )
}

/// Persists Cove's pinned-tool directories for future shells.
pub fn persist(saved: &Value) -> io::Result<Vec<PathBuf>> {
    let directories = tool_directories(saved);
    if directories.is_empty() {
        return Err(io::Error::other(
            "no pinned Cove tools in the setup state; run `cargo cove setup` first",
        ));
    }
    let cache = super::cache_dir()?;
    if cfg!(windows) {
        let script = powershell_script(&cache, &directories, false);
        super::checked(super::command("powershell")?.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &script,
        ]))?;
    } else {
        let path = profile_path()?;
        let existing = fs::read_to_string(&path).unwrap_or_default();
        let updated = replace_block(&existing, Some(&profile_block(&directories)));
        write_profile(&path, &updated)?;
    }
    Ok(directories)
}

/// Removes only Cove's entries from the persistent PATH.
pub fn unpersist() -> io::Result<()> {
    let cache = super::cache_dir()?;
    if cfg!(windows) {
        let script = powershell_script(&cache, &[], true);
        super::checked(super::command("powershell")?.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &script,
        ]))?;
    } else {
        let path = profile_path()?;
        if path.is_file() {
            let existing = fs::read_to_string(&path)?;
            write_profile(&path, &replace_block(&existing, None))?;
        }
    }
    Ok(())
}

fn write_profile(path: &Path, content: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("cove-tmp");
    fs::write(&temporary, content)?;
    fs::rename(temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn state() -> Value {
        let cache = super::super::cache_dir().expect("cache");
        json!({"environment": {
            "COVE_ZIG": cache.join("tools/zig-0.17.0/zig.exe").to_string_lossy(),
            "PATH": "C:\\Windows\\System32;/usr/bin"
        }})
    }

    #[test]
    fn only_cove_cache_directories_are_persisted() {
        let directories = tool_directories(&state());
        assert_eq!(directories.len(), 1);
        assert!(directories[0].ends_with("tools/zig-0.17.0"));
        assert!(tool_directories(&json!({"environment":{}})).is_empty());
    }

    #[test]
    fn replacing_the_managed_block_is_idempotent() {
        let directories = tool_directories(&state());
        let original = "export FOO=1\n";
        let once = replace_block(original, Some(&profile_block(&directories)));
        let twice = replace_block(&once, Some(&profile_block(&directories)));
        assert_eq!(once, twice);
        assert!(once.contains("export FOO=1"));
        assert_eq!(once.matches(BEGIN).count(), 1);
    }

    #[test]
    fn unpersisting_restores_the_original_profile() {
        let directories = tool_directories(&state());
        let original = "export FOO=1\n";
        let with_block = replace_block(original, Some(&profile_block(&directories)));
        assert_eq!(replace_block(&with_block, None), original);
    }

    #[test]
    fn powershell_script_is_idempotent_and_reversible() {
        let cache = Path::new("C:/Users/dev/AppData/Local/Cove");
        let script = powershell_script(cache, &[cache.join("tools/zig-0.17.0")], false);
        // Preserve REG_EXPAND_SZ and unexpanded %VAR% entries.
        assert!(script.contains("DoNotExpandEnvironmentNames"));
        assert!(script.contains("GetValueKind('PATH')"));
        assert!(script.contains("SetValue('PATH', $updated, $kind)"));
        // No write when nothing changed; only Cove's own entries are removed.
        assert!(script.contains("if ($updated -ne $raw)"));
        assert!(script.contains("StartsWith($cache"));
        let removal = powershell_script(cache, &[], true);
        assert!(removal.contains("$add = @()"));
    }

    #[test]
    fn exports_cover_pinned_tools_and_escape_shell_metacharacters() {
        let posix = exports(&state(), Shell::Posix);
        assert!(posix.contains("export PATH="));
        assert!(posix.contains("export COVE_ZIG="));
        assert!(posix.contains("${PATH}"));
        let ps = exports(&state(), Shell::PowerShell);
        assert!(ps.contains("$env:PATH"));
        assert!(ps.contains("$env:COVE_ZIG"));
    }

    #[test]
    fn posix_path_line_is_usable_from_git_bash_on_windows() {
        let line = exports(&state(), Shell::Posix)
            .lines()
            .next()
            .expect("PATH statement")
            .to_owned();
        assert!(line.starts_with("export PATH=\""));
        if cfg!(windows) {
            // A drive letter or backslash would split/escape the POSIX PATH.
            assert!(line.contains("/c/"), "{line}");
            assert!(!line.contains("C:"), "{line}");
            assert!(!line.contains('\\'), "{line}");
        }
        assert_eq!(
            to_posix_path(Path::new("/opt/cove/tools")),
            "/opt/cove/tools"
        );
        assert_eq!(to_posix_path(Path::new("D:\\tools\\zig")), "/d/tools/zig");
    }
}
