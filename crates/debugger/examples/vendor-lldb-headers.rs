//! Refreshes the vendored LLDB SB API headers under `native/include`.
//!
//! ```text
//! cargo run -p cove-debugger --example vendor-lldb-headers -- llvmorg-23.1.5
//! ```
//!
//! Without an argument the tag recorded in `native/include/VERSION` is fetched
//! again. The tool needs `git` and Python 3 on `PATH`; it:
//!
//! 1. sparse-clones llvm-project at the tag,
//! 2. copies `lldb/include/lldb/API/*.h` and every `lldb/...` header they
//!    include,
//! 3. runs LLDB's own `generate-sbapi-dwarf-enum.py` over `Dwarf.def` to
//!    produce `SBLanguages.h`, exactly as LLDB's build does,
//! 4. copies LLDB's licence and records the tag and commit in `VERSION`.
use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const REPOSITORY: &str = "https://github.com/llvm/llvm-project";
const GENERATED: &str = "lldb/API/SBLanguages.h";

type Outcome<T> = Result<T, Box<dyn Error>>;

fn main() -> Outcome<()> {
    let include = Path::new(env!("CARGO_MANIFEST_DIR")).join("native/include");
    let tag = match std::env::args().nth(1) {
        Some(tag) => tag,
        None => recorded_tag(&include)?,
    };
    let checkout = std::env::temp_dir().join(format!("cove-lldb-headers-{}", std::process::id()));
    let result = vendor(&tag, &checkout, &include);
    let _ = fs::remove_dir_all(&checkout);
    result
}

fn recorded_tag(include: &Path) -> Outcome<String> {
    let version = fs::read_to_string(include.join("VERSION"))
        .map_err(|error| format!("pass a tag such as llvmorg-23.1.5 (no VERSION: {error})"))?;
    version
        .lines()
        .find_map(|line| line.strip_prefix("tag: "))
        .map(|tag| tag.trim().to_owned())
        .ok_or_else(|| "VERSION has no `tag:` line; pass a tag".into())
}

fn run(command: &mut Command) -> Outcome<String> {
    let output = command
        .output()
        .map_err(|error| format!("cannot run {command:?}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{command:?} failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn vendor(tag: &str, checkout: &Path, include: &Path) -> Outcome<()> {
    run(Command::new("git")
        .args([
            "clone",
            "--quiet",
            "--depth",
            "1",
            "--filter=blob:none",
            "--sparse",
        ])
        .args(["--branch", tag, REPOSITORY])
        .arg(checkout))?;
    run(Command::new("git")
        .arg("-C")
        .arg(checkout)
        .args(["sparse-checkout", "set"])
        .args([
            "lldb/include/lldb",
            "lldb/scripts",
            "llvm/include/llvm/BinaryFormat",
        ]))?;
    let commit = run(Command::new("git")
        .arg("-C")
        .arg(checkout)
        .args(["rev-parse", "HEAD"]))?;

    let upstream = checkout.join("lldb/include");
    let headers = closure(&upstream)?;

    // Build the new tree beside the old one so a failure leaves it untouched.
    let staging = include.with_extension("staging");
    let _ = fs::remove_dir_all(&staging);
    for header in &headers {
        let target = staging.join(header);
        fs::create_dir_all(target.parent().ok_or("header without a parent")?)?;
        fs::copy(upstream.join(header), target)?;
    }
    generate_languages(checkout, &staging.join(GENERATED))?;
    fs::copy(
        checkout.join("lldb/LICENSE.TXT"),
        staging.join("LICENSE.TXT"),
    )?;
    fs::write(
        staging.join("VERSION"),
        format!("repository: {REPOSITORY}\ntag: {tag}\ncommit: {commit}\n"),
    )?;
    let readme = include.join("README.md");
    if readme.is_file() {
        fs::copy(&readme, staging.join("README.md"))?;
    }
    let _ = fs::remove_dir_all(include);
    fs::rename(&staging, include)?;
    println!(
        "vendored {} headers from {tag} ({commit}) into {}",
        headers.len() + 1,
        include.display()
    );
    Ok(())
}

/// Every header in `lldb/API` plus the `lldb/...` headers they include,
/// as paths relative to the include root.
fn closure(upstream: &Path) -> Outcome<BTreeSet<PathBuf>> {
    let mut pending: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(upstream.join("lldb/API"))? {
        let path = entry?.path();
        if path.extension().is_some_and(|extension| extension == "h") {
            pending.push(Path::new("lldb/API").join(path.file_name().ok_or("unnamed header")?));
        }
    }
    let mut headers = BTreeSet::new();
    while let Some(header) = pending.pop() {
        if !headers.insert(header.clone()) {
            continue;
        }
        let text = fs::read_to_string(upstream.join(&header))?;
        for included in text.lines().filter_map(included_header) {
            if included == GENERATED {
                continue;
            }
            if !upstream.join(included).is_file() {
                return Err(format!(
                    "{} includes {included}, which is not in the source tree",
                    header.display()
                )
                .into());
            }
            pending.push(PathBuf::from(included));
        }
    }
    Ok(headers)
}

/// The path of an `#include "lldb/..."` line.
fn included_header(line: &str) -> Option<&str> {
    let rest = line.trim_start().strip_prefix("#include")?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let path = &rest[..rest.find('"')?];
    path.starts_with("lldb/").then_some(path)
}

fn generate_languages(checkout: &Path, output: &Path) -> Outcome<()> {
    let script = checkout.join("lldb/scripts/generate-sbapi-dwarf-enum.py");
    let definitions = checkout.join("llvm/include/llvm/BinaryFormat/Dwarf.def");
    let mut failure = String::new();
    for python in ["python3", "python"] {
        let attempt = run(Command::new(python)
            .arg(&script)
            .arg(&definitions)
            .arg("-o")
            .arg(output));
        match attempt {
            Ok(_) if output.is_file() => return Ok(()),
            Ok(_) => failure = format!("{python} wrote no {}", output.display()),
            Err(error) => failure = error.to_string(),
        }
    }
    Err(format!("cannot generate SBLanguages.h with Python 3: {failure}").into())
}
