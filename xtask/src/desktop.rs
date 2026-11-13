//! Windows and Linux application folders.

use crate::process;
use crate::project::Project;
use crate::{Result, Target};
use std::path::{Path, PathBuf};
use std::process::Command;

/// `<output>/<Name>/` with the executable, resources and an embedded manifest.
pub fn bundle_windows(project: &Project, target: &Target, output: &Path) -> Result<PathBuf> {
    let executable = project.build_bin(target.mode(), None)?;
    let directory = folder(project, output, &executable)?;
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

/// `<output>/<Name>/` with the executable, resources and a `.desktop` entry.
pub fn bundle_linux(project: &Project, target: &Target, output: &Path) -> Result<PathBuf> {
    let executable = project.build_bin(target.mode(), None)?;
    let directory = folder(project, output, &executable)?;
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

fn folder(project: &Project, output: &Path, executable: &Path) -> Result<PathBuf> {
    let directory = output.join(&project.name);
    process::remove_dir(&directory)?;
    process::copy(
        executable,
        &directory.join(executable.file_name().expect("executable has a name")),
    )?;
    project.stage_resources(&directory)?;
    let platform = if cfg!(windows) { "Windows" } else { "Linux" };
    process::copy_tree(&project.platforms(platform).join("Resources"), &directory)?;
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
