//! Cargo needs the Android linker before build scripts run. Setup writes it once;
//! consumers use ordinary Cargo commands afterwards.
use crate::Result;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn configure_android(project: &Path, targets: &[String]) -> Result<Option<PathBuf>> {
    let targets: Vec<_> = targets
        .iter()
        .filter(|target| target.contains("android"))
        .collect();
    if targets.is_empty() {
        return Ok(None);
    }
    let ndk = cove_native_build::setup::setting("ANDROID_NDK_HOME")
        .map_err(|e| e.to_string())?
        .ok_or("Android NDK has not been prepared")?;
    let host = if cfg!(target_os = "macos") {
        "darwin-x86_64"
    } else if cfg!(windows) {
        "windows-x86_64"
    } else {
        "linux-x86_64"
    };
    let bin = PathBuf::from(&ndk)
        .join("toolchains/llvm/prebuilt")
        .join(host)
        .join("bin");
    let mut linkers = BTreeMap::new();
    let mut environment = BTreeMap::new();
    for target in targets {
        if !target.starts_with("aarch64-") && !target.starts_with("x86_64-") {
            return Err(format!("unsupported Android media ABI: {target}"));
        }
        let suffix = if cfg!(windows) { ".cmd" } else { "" };
        let prefix = format!("{target}{}", cove_native_build::ANDROID_PLATFORM);
        let compiler = bin.join(format!("{prefix}-clang{suffix}"));
        if !compiler.is_file() {
            return Err(format!(
                "Android compiler not found: {}",
                compiler.display()
            ));
        }
        linkers.insert(target.clone(), compiler.to_string_lossy().into_owned());
        let key = target.replace('-', "_");
        environment.insert(format!("CC_{key}"), compiler.to_string_lossy().into_owned());
        environment.insert(
            format!("CXX_{key}"),
            bin.join(format!("{prefix}-clang++{suffix}"))
                .to_string_lossy()
                .into_owned(),
        );
        environment.insert(
            format!("AR_{key}"),
            bin.join(if cfg!(windows) {
                "llvm-ar.exe"
            } else {
                "llvm-ar"
            })
            .to_string_lossy()
            .into_owned(),
        );
    }
    for key in [
        "ANDROID_HOME",
        "ANDROID_SDK_ROOT",
        "ANDROID_NDK_HOME",
        "JAVA_HOME",
    ] {
        if let Some(value) = cove_native_build::setup::setting(key).map_err(|e| e.to_string())? {
            environment.insert(key.into(), value);
        }
    }
    apply(project, &linkers, &environment)
}

/// Merges into the project's Cargo config; `linkers`/`environment` only fill
/// missing entries. Returns the path when written.
fn apply(
    project: &Path,
    linkers: &BTreeMap<String, String>,
    environment: &BTreeMap<String, String>,
) -> Result<Option<PathBuf>> {
    let directory = project.join(".cargo");
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let path = if directory.join("config").exists() {
        directory.join("config")
    } else {
        directory.join("config.toml")
    };
    let original = if path.exists() {
        fs::read_to_string(&path).map_err(|e| e.to_string())?
    } else {
        String::new()
    };
    let configured = merge(&original, linkers, environment)?;
    if configured != original {
        fs::write(&path, configured)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        return Ok(Some(path));
    }
    Ok(None)
}

fn merge(
    original: &str,
    linkers: &BTreeMap<String, String>,
    environment: &BTreeMap<String, String>,
) -> Result<String> {
    let mut document: toml::Table =
        toml::from_str(original).map_err(|e| format!("cannot read Cargo config: {e}"))?;
    let before = document.clone();
    if !linkers.is_empty() {
        let targets = document
            .entry("target")
            .or_insert_with(|| toml::Value::Table(Default::default()))
            .as_table_mut()
            .ok_or("Cargo target config must be a table")?;
        for (target, linker) in linkers {
            let target = targets
                .entry(target)
                .or_insert_with(|| toml::Value::Table(Default::default()))
                .as_table_mut()
                .ok_or("Cargo target must be a table")?;
            target
                .entry("linker")
                .or_insert_with(|| toml::Value::String(linker.clone()));
        }
    }
    if environment.is_empty() {
        return Ok(original.into());
    }
    let values = document
        .entry("env")
        .or_insert_with(|| toml::Value::Table(Default::default()))
        .as_table_mut()
        .ok_or("Cargo env config must be a table")?;
    for (key, value) in environment {
        values
            .entry(key)
            .or_insert_with(|| toml::Value::String(value.clone()));
    }
    if document == before {
        Ok(original.into())
    } else {
        toml::to_string_pretty(&document).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> (PathBuf, tempfile::TempDir) {
        let directory = tempfile::tempdir().expect("project fixture");
        let path = directory.path().to_path_buf();
        let project = path.join("project");
        fs::create_dir_all(&project).expect("project fixture");
        (project, directory)
    }

    #[test]
    fn preserves_existing_aliases_flags_and_explicit_linkers_and_is_idempotent() {
        let original = "# project config\n[alias]\ncheck-all = 'check --workspace'\n[target.aarch64-linux-android]\nlinker='custom-clang'\nrustflags=['-C', 'debuginfo=1']\n[env]\nUSER_SETTING='preserved'\n";
        let linkers = BTreeMap::from([
            ("aarch64-linux-android".into(), "prepared-clang".into()),
            ("x86_64-linux-android".into(), "x64-clang".into()),
        ]);
        let environment = BTreeMap::from([("ANDROID_NDK_HOME".into(), "/sdk/ndk".into())]);
        let configured = merge(original, &linkers, &environment).expect("configure");
        let document: toml::Value = toml::from_str(&configured).expect("Cargo config");
        assert_eq!(
            document["alias"]["check-all"].as_str(),
            Some("check --workspace")
        );
        assert_eq!(
            document["target"]["aarch64-linux-android"]["linker"].as_str(),
            Some("custom-clang")
        );
        assert_eq!(
            document["target"]["x86_64-linux-android"]["linker"].as_str(),
            Some("x64-clang")
        );
        assert_eq!(
            document["target"]["aarch64-linux-android"]["rustflags"]
                .as_array()
                .expect("flags")
                .len(),
            2
        );
        assert_eq!(document["env"]["USER_SETTING"].as_str(), Some("preserved"));
        assert_eq!(
            merge(&configured, &linkers, &environment).expect("second setup"),
            configured
        );
    }
    #[test]
    fn malformed_configuration_is_rejected_before_writing() {
        assert!(
            merge("[env", &BTreeMap::new(), &BTreeMap::new()).is_err(),
            "a broken config must not be rewritten"
        );
    }
    #[test]
    fn non_android_targets_never_touch_the_cargo_config() {
        assert_eq!(
            configure_android(Path::new("/nonexistent"), &["aarch64-apple-darwin".into()])
                .expect("no Android target"),
            None
        );
    }
    #[test]
    fn writes_the_linker_once_and_reports_the_changed_file() {
        let (project, _fixture) = project();
        let linkers = BTreeMap::from([("aarch64-linux-android".into(), "/ndk/bin/clang".into())]);
        let environment = BTreeMap::from([("ANDROID_NDK_HOME".into(), "/ndk".into())]);
        let changed = apply(&project, &linkers, &environment).expect("first setup");
        let config = project.join(".cargo/config.toml");
        assert_eq!(changed, Some(config.clone()));
        assert!(
            fs::read_to_string(&config)
                .expect("config")
                .contains("aarch64-linux-android")
        );
        assert_eq!(
            apply(&project, &linkers, &environment).expect("second setup"),
            None,
            "an up-to-date config must not be reported as changed"
        );
    }
}
