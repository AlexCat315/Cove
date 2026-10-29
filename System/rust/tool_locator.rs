//! Translation of `System/Sources/CoveProcess/ToolLocator.swift`.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub enum ProcessEnvironment {}

impl ProcessEnvironment {
    pub fn setting(
        value: &str,
        for name: &str,
        environment: &BTreeMap<String, String>,
    ) -> BTreeMap<String, String> {
        let mut result = environment.clone();
        #[cfg(windows)]
        {
            let existing: Vec<String> = result
                .keys()
                .filter(|key| key.eq_ignore_ascii_case(name))
                .cloned()
                .collect();
            for key in existing {
                result.remove(&key);
            }
        }
        result.insert(name.to_owned(), value.to_owned());
        result
    }

    pub fn value(name: &str, environment: &BTreeMap<String, String>) -> Option<String> {
        if let Some(value) = environment.get(name) {
            return Some(value.clone());
        }
        #[cfg(windows)]
        {
            return environment
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .map(|(_, value)| value.clone());
        }
        #[cfg(not(windows))]
        {
            None
        }
    }
}

pub enum ToolLocator {}

impl ToolLocator {
    pub fn executable(
        name: &str,
        environment: &BTreeMap<String, String>,
    ) -> Option<PathBuf> {
        if name.is_empty() || name.contains('\0') {
            return None;
        }
        if name.contains('/') || name.contains('\\') {
            let path = PathBuf::from(name);
            return is_runnable(&path).then_some(path);
        }
        #[cfg(windows)]
        let (separator, extensions) = (
            ';',
            std::iter::once(String::new())
                .chain(
                    ProcessEnvironment::value("PATHEXT", environment)
                        .unwrap_or_else(|| ".EXE;.CMD;.BAT".into())
                        .split(';')
                        .map(str::to_owned),
                )
                .collect::<Vec<_>>(),
        );
        #[cfg(not(windows))]
        let (separator, extensions) = (':', vec![String::new()]);
        let path = ProcessEnvironment::value("PATH", environment).unwrap_or_default();
        for directory in path.split(separator) {
            for suffix in &extensions {
                let file = Path::new(directory).join(format!("{name}{suffix}"));
                if is_runnable(&file) {
                    return Some(file);
                }
            }
        }
        None
    }
}

fn is_runnable(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}
