//! Cargo needs the Android linker before build scripts run. Setup writes it once;
//! consumers use ordinary Cargo commands afterwards.
use crate::Result;
use serde_json::{Value, json};
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
    apply(project, &linkers, &environment, &BTreeMap::new())
}

/// Merges into the project's Cargo config: `linkers`/`environment` only fill
/// missing entries, `owned` keys are rewritten. Returns the path when written.
fn apply(
    project: &Path,
    linkers: &BTreeMap<String, String>,
    environment: &BTreeMap<String, String>,
    owned: &BTreeMap<String, String>,
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
    let configured = merge(&original, linkers, environment, owned)?;
    if configured != original {
        fs::write(&path, configured)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        return Ok(Some(path));
    }
    Ok(None)
}

/// Points the project's Cargo config at the native media tree that setup just
/// prepared, so published bindings find it without a build-script hook.
pub(crate) fn configure_media(project: &Path, report: &Value) -> Result<Option<PathBuf>> {
    let owned = media_environment(report)?;
    if owned.is_empty() {
        return Ok(None);
    }
    apply(project, &BTreeMap::new(), &BTreeMap::new(), &owned)
}

fn media_environment(report: &Value) -> Result<BTreeMap<String, String>> {
    let prepared = report["native"]
        .as_object()
        .into_iter()
        .flat_map(|native| native.values())
        .filter(|entry| entry["environment"].is_object())
        .collect::<Vec<_>>();
    if prepared.len() > 1 {
        return Err(
            "the media environment is one value per project; prepare one target at a time with --target <triple>".into(),
        );
    }
    let mut owned = BTreeMap::new();
    if let Some(entry) = prepared.first() {
        for (key, value) in entry["environment"].as_object().expect("environment") {
            owned.insert(key.clone(), value.as_str().unwrap_or_default().to_string());
        }
    }
    Ok(owned)
}

/// Reports whether the project config points at a usable native media tree.
/// A stale value is silent otherwise: Cargo would keep linking the old prefix.
pub(crate) fn media_state(project: &Path, report: &Value) -> Value {
    let mut state = json!({"configured": false, "state": "not-configured"});
    let path = project.join(".cargo/config.toml");
    let text = fs::read_to_string(&path).unwrap_or_default();
    let Ok(document) = toml::from_str::<toml::Table>(&text) else {
        state["state"] = json!("unreadable-config");
        return state;
    };
    let configured = document
        .get("env")
        .and_then(toml::Value::as_table)
        .and_then(|env| env.get("PKG_CONFIG_LIBDIR"))
        .and_then(|entry| match entry {
            toml::Value::String(value) => Some(value.clone()),
            toml::Value::Table(table) => table
                .get("value")
                .and_then(toml::Value::as_str)
                .map(str::to_owned),
            _ => None,
        });
    let Some(configured) = configured else {
        return state;
    };
    let directory = Path::new(&configured);
    let complete = directory.join("gstreamer-full-1.0.pc").is_file();
    let expected = report["native"]
        .as_object()
        .and_then(|native| native.values().next())
        .and_then(|entry| entry["prefix"].as_str())
        .and_then(|prefix| Path::new(prefix).parent().map(|out| out.join("pc")))
        .filter(|pc| directory.starts_with(pc))
        .is_some();
    state["configured"] = json!(configured);
    state["state"] = json!(match (complete, expected) {
        (true, true) => "current",
        (true, false) => "stale-or-other-target",
        (false, _) => "missing-tree",
    });
    state
}

fn merge(
    original: &str,
    linkers: &BTreeMap<String, String>,
    environment: &BTreeMap<String, String>,
    owned: &BTreeMap<String, String>,
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
    if environment.is_empty() && owned.is_empty() {
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
    // Setup owns these keys: the native tree moves when its build manifest
    // changes, so a previous value must not keep pointing at a stale prefix.
    for (key, value) in owned {
        let mut entry = toml::Table::new();
        entry.insert("value".into(), toml::Value::String(value.clone()));
        entry.insert("force".into(), toml::Value::Boolean(true));
        values.insert(key.clone(), toml::Value::Table(entry));
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
        let configured =
            merge(original, &linkers, &environment, &BTreeMap::new()).expect("configure");
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
            merge(&configured, &linkers, &environment, &BTreeMap::new()).expect("second setup"),
            configured
        );
    }
    #[test]
    fn malformed_configuration_is_rejected_before_writing() {
        assert!(
            merge("[env", &BTreeMap::new(), &BTreeMap::new(), &BTreeMap::new()).is_err(),
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
        let changed =
            apply(&project, &linkers, &environment, &BTreeMap::new()).expect("first setup");
        let config = project.join(".cargo/config.toml");
        assert_eq!(changed, Some(config.clone()));
        assert!(
            fs::read_to_string(&config)
                .expect("config")
                .contains("aarch64-linux-android")
        );
        assert_eq!(
            apply(&project, &linkers, &environment, &BTreeMap::new()).expect("second setup"),
            None,
            "an up-to-date config must not be reported as changed"
        );
    }

    fn prepared(prefix: &Path) -> Value {
        json!({"native": {
            "aarch64-apple-darwin": {
                "ready": true,
                "prefix": prefix.join("out/install"),
                "environment": {
                    "PKG_CONFIG_LIBDIR": prefix.join("out/pc/abc"),
                    "PKG_CONFIG_PATH": "",
                    "PKG_CONFIG_ALL_STATIC": "1",
                }
            }
        }})
    }

    #[test]
    fn media_environment_is_written_once_and_repointed_on_the_next_setup() {
        let (project, _fixture) = project();
        let first = prepared(Path::new("/cache/tree-one"));
        let changed = configure_media(&project, &first)
            .expect("first media setup")
            .expect("config written");
        let text = fs::read_to_string(&changed).expect("config");
        assert!(text.contains("/cache/tree-one/out/pc/abc"), "{text}");
        assert!(text.contains("force = true"), "{text}");
        // The native prefix moves with its build manifest; the old value must
        // not keep the project linked against a stale tree.
        configure_media(&project, &prepared(Path::new("/cache/tree-two")))
            .expect("second media setup")
            .expect("config repointed");
        let text = fs::read_to_string(&changed).expect("config");
        assert!(text.contains("/cache/tree-two/out/pc/abc"), "{text}");
        assert!(!text.contains("tree-one"), "{text}");
    }

    #[test]
    fn one_project_config_cannot_hold_two_media_targets() {
        let (project, _fixture) = project();
        let report = json!({"native": {
            "aarch64-apple-darwin": {"environment": {"PKG_CONFIG_LIBDIR": "/a"}},
            "aarch64-linux-android": {"environment": {"PKG_CONFIG_LIBDIR": "/b"}}
        }});
        assert!(
            configure_media(&project, &report)
                .expect_err("two targets")
                .contains("one target at a time")
        );
        assert!(
            !project.join(".cargo/config.toml").exists(),
            "nothing written"
        );
    }

    #[test]
    fn media_state_names_the_configured_tree_and_its_staleness() {
        let (project, _fixture) = project();
        let out = project.join("native/out");
        fs::create_dir_all(out.join("pc/abc")).expect("pc fixture");
        let report = prepared(&project.join("native"));
        configure_media(&project, &report).expect("configured");
        // Without the .pc file the tree is unusable, not merely stale.
        assert_eq!(media_state(&project, &report)["state"], "missing-tree");
        fs::write(out.join("pc/abc/gstreamer-full-1.0.pc"), "Name: gst\n").expect("pc fixture");
        assert_eq!(media_state(&project, &report)["state"], "current");
        let other = prepared(Path::new("/somewhere/else"));
        assert_eq!(
            media_state(&project, &other)["state"],
            "stale-or-other-target",
            "a config from another tree must be visible"
        );
    }
}
