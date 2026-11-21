//! Windows and Linux application folders.

use crate::process;
use crate::project::Project;
use crate::{Mode, Result, Target};
use std::path::{Path, PathBuf};
use std::process::Command;

/// `<output>/<Name>/` with the executable, resources and an embedded manifest.
pub fn bundle_windows(project: &Project, target: &Target, output: &Path) -> Result<PathBuf> {
    let executable = project.build_bin(target.mode(), None)?;
    let directory = folder(project, output, &executable, target.mode())?;
    stage_msvc_runtime(&directory)?;
    let installed = directory.join(executable.file_name().expect("executable has a name"));
    let manifest = project.platforms("Windows").join("Application.manifest");
    if manifest.is_file() {
        let tool = process::which("mt").ok_or("mt.exe (Windows SDK) is not on PATH")?;
        process::run(
            Command::new(tool)
                .arg("-nologo")
                .arg("-manifest")
                .arg(&manifest)
                .arg(format!("-outputresource:{};#1", installed.display())),
        )?;
    }
    Ok(directory)
}

/// Copies the MSVC C runtime beside the executable. Rust and C++ libraries
/// link it dynamically by default, so a folder built on a machine with Visual
/// Studio only runs on another machine once these DLLs travel with it.
fn stage_msvc_runtime(directory: &Path) -> Result<()> {
    if !cfg!(all(windows, target_env = "msvc")) {
        return Ok(());
    }
    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else {
        "x64"
    };
    let root = redist_root().ok_or_else(|| {
        "找不到 Visual C++ 运行库（MSVC Redist）。请在 Visual Studio 的“x64 Native Tools \
         Command Prompt”中运行 cargo xtask package，或安装“使用 C++ 的桌面开发”工作负载。"
            .to_owned()
    })?;
    let crt = newest_crt(&root, arch).ok_or_else(|| {
        format!(
            "{} 下没有 {arch} 的 MSVC C 运行库（Microsoft.VC14*.CRT）；请修复或更新 Visual Studio。",
            root.display()
        )
    })?;
    let mut copied = 0;
    for entry in std::fs::read_dir(&crt).map_err(|error| format!("{}: {error}", crt.display()))? {
        let path = entry.map_err(|error| error.to_string())?.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if !path.is_file() || !is_runtime_dll(name) {
            continue;
        }
        process::copy(&path, &directory.join(name))?;
        copied += 1;
    }
    if copied == 0 {
        return Err(format!("{} 里没有可复制的运行库 DLL", crt.display()));
    }
    eprintln!("    runtime: {copied} MSVC DLLs from {}", crt.display());
    Ok(())
}

/// `msvcp140.dll`, `vcruntime140_1.dll`, `concrt140.dll` and their siblings.
fn is_runtime_dll(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.ends_with(".dll")
        && (name.starts_with("vcruntime")
            || name.starts_with("msvcp")
            || name.starts_with("concrt"))
}

/// The `VC\Redist\MSVC` folder of the Visual Studio that built the app.
fn redist_root() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("VCToolsRedistDir") {
        let path = PathBuf::from(dir);
        if path.is_dir() {
            return Some(path);
        }
    }
    if let Some(dir) = std::env::var_os("VCToolsInstallDir") {
        // `VC\Tools\MSVC\<version>` -> `VC\Redist\MSVC`
        if let Some(vc) = PathBuf::from(dir).ancestors().nth(3) {
            let path = vc.join("Redist").join("MSVC");
            if path.is_dir() {
                return Some(path);
            }
        }
    }
    let installation = vs_installation()?;
    let path = installation.join("VC").join("Redist").join("MSVC");
    path.is_dir().then_some(path)
}

fn vs_installation() -> Option<PathBuf> {
    let programs = std::env::var_os("ProgramFiles(x86)")?;
    let vswhere = PathBuf::from(programs)
        .join("Microsoft Visual Studio")
        .join("Installer")
        .join("vswhere.exe");
    if !vswhere.is_file() {
        return None;
    }
    let output = Command::new(vswhere)
        .args([
            "-latest",
            "-products",
            "*",
            "-requires",
            "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
            "-property",
            "installationPath",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8(output.stdout).ok()?;
    let path = PathBuf::from(path.trim());
    path.is_dir().then_some(path)
}

/// The newest `Microsoft.VC14*.CRT` folder for `arch` below `root`.
fn newest_crt(root: &Path, arch: &str) -> Option<PathBuf> {
    let mut best: Option<(String, PathBuf)> = None;
    for version in std::fs::read_dir(root).ok()?.flatten() {
        let Ok(entries) = std::fs::read_dir(version.path().join(arch)) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !entry.path().is_dir()
                || !name.starts_with("Microsoft.VC14")
                || !name.ends_with(".CRT")
            {
                continue;
            }
            let key = version.file_name().to_string_lossy().into_owned();
            if best.as_ref().is_none_or(|(current, _)| key > *current) {
                best = Some((key, entry.path()));
            }
        }
    }
    best.map(|(_, path)| path)
}

/// `<output>/<Name>/` with the executable, resources and a `.desktop` entry.
pub fn bundle_linux(project: &Project, target: &Target, output: &Path) -> Result<PathBuf> {
    let executable = project.build_bin(target.mode(), None)?;
    let directory = folder(project, output, &executable, target.mode())?;
    let template = project.platforms("Linux").join("Application.desktop");
    let mut entry = std::fs::read_to_string(&template)
        .unwrap_or_else(|_| "[Desktop Entry]\nType=Application\nTerminal=false\n".into());
    if !entry.ends_with('\n') {
        entry.push('\n');
    }
    entry.push_str(&format!("Name={}\nExec={}\n", project.name, project.bin()?));
    process::write(
        &directory.join(format!("{}.desktop", project.bin()?)),
        entry,
    )?;
    Ok(directory)
}

fn folder(project: &Project, output: &Path, executable: &Path, mode: Mode) -> Result<PathBuf> {
    let directory = output.join(&project.name);
    process::remove_dir(&directory)?;
    process::copy(
        executable,
        &directory.join(executable.file_name().expect("executable has a name")),
    )?;
    project.stage_resources(&directory)?;
    let platform = if cfg!(windows) { "Windows" } else { "Linux" };
    process::copy_tree(&project.platforms(platform).join("Resources"), &directory)?;
    project.stage_extra_resources(&directory)?;
    project.stage_helpers(&directory, mode, None)?;
    Ok(directory)
}

pub fn launch(
    project: &Project,
    directory: &Path,
    arguments: &[String],
    devtools: Option<&crate::devtools::Session>,
) -> Result<()> {
    let name = if cfg!(windows) {
        format!("{}.exe", project.bin()?)
    } else {
        project.bin()?.to_owned()
    };
    crate::devtools::launch_desktop(Command::new(directory.join(name)).args(arguments), devtools)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A throwaway directory for a test tree.
    struct Tree(PathBuf);

    impl Tree {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("cove-package-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("create test tree");
            Self(path)
        }

        fn directory(&self, relative: &str) -> PathBuf {
            let path = self.0.join(relative);
            std::fs::create_dir_all(&path).expect("create test directory");
            path
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn runtime_dlls_are_selected_by_name() {
        assert!(is_runtime_dll("MSVCP140.dll"));
        assert!(is_runtime_dll("msvcp140_atomic_wait.dll"));
        assert!(is_runtime_dll("VCRUNTIME140_1.dll"));
        assert!(is_runtime_dll("concrt140.dll"));
        assert!(!is_runtime_dll("vccorlib140.dll"));
        assert!(!is_runtime_dll("ruwriter.exe"));
        assert!(!is_runtime_dll("Ruwriter.dll"));
    }

    #[test]
    fn newest_crt_prefers_the_highest_toolset_version() {
        let tree = Tree::new("crt-version");
        let old = tree.directory("14.29.30133/x64/Microsoft.VC142.CRT");
        tree.directory("14.29.30133/x64/Microsoft.VC142.CRT/neutral");
        let new = tree.directory("14.44.35207/x64/Microsoft.VC143.CRT");
        tree.directory("14.44.35207/arm64/Microsoft.VC143.CRT");

        assert_eq!(newest_crt(&tree.0, "x64"), Some(new));
        assert_eq!(
            newest_crt(&tree.0, "arm64"),
            Some(tree.0.join("14.44.35207/arm64/Microsoft.VC143.CRT"))
        );
        assert_eq!(newest_crt(&tree.0, "x86"), None);
        assert_eq!(old, tree.0.join("14.29.30133/x64/Microsoft.VC142.CRT"));
    }
}
