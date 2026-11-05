//! The pinned native media tree is prepared by `cargo cove setup --media`.
use crate::{
    gstreamer::{Build, REVISION},
    setup,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    env, fs, io,
    path::{Path, PathBuf},
};

/// The cached native tree for `target`, keyed by the build identity so a
/// changed toolchain, SDK or profile starts a new tree instead of reusing it.
pub fn cached_build(target: &str) -> io::Result<Build> {
    let host = setup::host_triple()?;
    crate::gstreamer::platform(target)?;
    Ok(native_tree(target, &host)?.1)
}

/// Build or reuse the pinned native tree for `target`, refresh its license
/// bundle and create the pkg-config search directory. Called by
/// `cargo cove setup --media`; nothing here depends on a build script, so the
/// published bindings only need the environment this tree implies.
pub fn prepare(target: &str) -> io::Result<(Build, BTreeMap<String, String>)> {
    let host = setup::host_triple()?;
    crate::gstreamer::platform(target)?;
    let (identity, build, key) = native_tree(target, &host)?;
    let root = build.out_dir.parent().expect("native tree root");
    let locks = setup::cache_dir()?.join("native/locks");
    fs::create_dir_all(&locks)?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(locks.join(format!("{key}.lock")))?;
    lock.lock()?;
    let ready = root.join("ready.json");
    let previous = fs::read(&ready)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    let notice_key = notice_key()?;
    if previous.as_ref().map(|value| &value["identity"]) != Some(&identity)
        || !prefix_complete(&build.prefix())
    {
        prepare_source(&build.source)?;
        build.build()?;
    } else if previous
        .as_ref()
        .and_then(|value| value["notices"].as_str())
        != Some(notice_key.as_str())
    {
        build.write_notices()?;
    }
    fs::write(
        &ready,
        serde_json::to_vec_pretty(&json!({"identity":identity,"notices":notice_key}))
            .map_err(io::Error::other)?,
    )?;
    lock.unlock()?;
    let environment = build.cargo_environment()?;
    Ok((build, environment))
}

fn native_tree(target: &str, host: &str) -> io::Result<(Value, Build, String)> {
    let identity = identity(target, host)?;
    let key = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&identity).map_err(io::Error::other)?)
    );
    let root = setup::cache_dir()?.join("native").join(&key);
    let build = Build {
        source: root.join("source"),
        out_dir: root.join("out"),
        target: target.into(),
        host: host.into(),
        cross_file: setup::setting(&format!("COVE_CROSS_FILE_{}", target.replace('-', "_")))?
            .map(PathBuf::from),
        jobs: env::var("NUM_JOBS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(4)
            .clamp(1, 8),
    };
    Ok((identity, build, key))
}

fn prefix_complete(prefix: &Path) -> bool {
    prefix.join("lib/pkgconfig/gstreamer-full-1.0.pc").is_file()
        && [
            "libgstreamer-full-1.0.a",
            "gstreamer-full-1.0.lib",
            "libgstreamer-full-1.0.lib",
        ]
        .iter()
        .any(|name| prefix.join("lib").join(name).is_file())
}
fn notice_key() -> io::Result<String> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut hash = Sha256::new();
    hash.update(include_bytes!("media_notices.rs"));
    for path in [
        "LICENSE",
        "Docs/media-licensing.md",
        "LICENSES/media/native.json",
    ] {
        hash.update(fs::read(workspace.join(path))?);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn identity(target: &str, host: &str) -> io::Result<Value> {
    let mut tools = serde_json::Map::new();
    for name in ["meson", "ninja", "pkg-config", "bison", "flex"] {
        let mut command = setup::command(name)?;
        let output = command
            .arg("--version")
            .output()
            .map_err(|e| io::Error::other(format!("{name}: {e}; run cargo cove setup --media")))?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "{name} is not usable; run cargo cove setup --media"
            )));
        }
        tools.insert(name.into(),json!({"program":command.get_program().to_string_lossy(),"version":String::from_utf8_lossy(&output.stdout).lines().next().unwrap_or_default()}));
    }
    let mut environment = serde_json::Map::new();
    for name in [
        "CC",
        "CXX",
        "CFLAGS",
        "CXXFLAGS",
        "SDKROOT",
        "MACOSX_DEPLOYMENT_TARGET",
        "ANDROID_NDK_HOME",
        "ANDROID_NDK_ROOT",
        "CARGO_CFG_TARGET_FEATURE",
    ] {
        let value = setup::setting(name)?;
        environment.insert(
            name.into(),
            json!(match (name, value) {
                ("CARGO_CFG_TARGET_FEATURE", None) => Some(target_features(target)?),
                (_, value) => value,
            }),
        );
    }
    let mut sdk = String::new();
    if host.contains("apple") {
        let name = if target.contains("ios") {
            if target.ends_with("-sim") || target.starts_with("x86_64") {
                "iphonesimulator"
            } else {
                "iphoneos"
            }
        } else {
            "macosx"
        };
        sdk = setup::checked_output(setup::command("xcrun")?.args([
            "--sdk",
            name,
            "--show-sdk-path",
        ]))?;
        sdk.push_str(&setup::checked_output(setup::command("xcrun")?.args([
            "--sdk",
            name,
            "--show-sdk-version",
        ]))?);
    }
    let mut compiler = setup::command(if host.contains("windows") { "cl" } else { "cc" })?;
    let result = compiler
        .arg(if host.contains("windows") {
            "/Bv"
        } else {
            "--version"
        })
        .output()?;
    let compiler_version = format!(
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let plan = Build {
        source: PathBuf::new(),
        out_dir: PathBuf::new(),
        target: target.into(),
        host: host.into(),
        cross_file: None,
        jobs: 1,
    };
    let cross = setup::setting(&format!("COVE_CROSS_FILE_{}", target.replace('-', "_")))?;
    let cross_contents = cross.as_ref().map(fs::read_to_string).transpose()?;
    Ok(
        json!({"schema":2,"revision":REVISION,"target":target,"host":host,"options":plan.options()?,"tools":tools,"environment":environment,"sdk":sdk,"compiler":compiler_version,"cross_file":cross,"cross_contents":cross_contents}),
    )
}

/// Cargo exposes target features only to build scripts; outside one, derive the
/// same list from rustc so a cached tree stays findable by the `cargo cove` CLI.
fn target_features(target: &str) -> io::Result<String> {
    let output = setup::checked_output(
        setup::command("rustc")?.args(["--print", "cfg", "--target", target]),
    )?;
    Ok(output
        .lines()
        .filter_map(|line| line.strip_prefix("target_feature="))
        .map(|feature| feature.trim_matches('"'))
        .collect::<Vec<_>>()
        .join(","))
}

fn prepare_source(destination: &Path) -> io::Result<()> {
    if destination.join(".git/cove-source-ready").is_file() {
        return Ok(());
    }
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let local = workspace.join("third-party/gstreamer");
    fs::create_dir_all(
        destination
            .parent()
            .ok_or_else(|| io::Error::other("native source parent"))?,
    )?;
    if destination.exists() {
        fs::remove_dir_all(destination)?;
    }
    if local.join(".git").exists() {
        setup::checked(
            setup::command("git")?
                .args(["clone", "--no-checkout", "--no-hardlinks"])
                .arg(&local)
                .arg(destination),
        )?;
    } else {
        setup::checked(setup::command("git")?.arg("init").arg(destination))?;
        setup::checked(setup::command("git")?.arg("-C").arg(destination).args([
            "remote",
            "add",
            "origin",
            "https://github.com/GStreamer/gstreamer.git",
        ]))?;
    }
    let has_revision = setup::command("git")?
        .arg("-C")
        .arg(destination)
        .args(["cat-file", "-e", REVISION])
        .status()?
        .success();
    if !has_revision {
        setup::checked(
            setup::command("git")?
                .arg("-C")
                .arg(destination)
                .args(["fetch", "--depth", "1", "origin", REVISION]),
        )?;
    }
    setup::checked(
        setup::command("git")?
            .arg("-C")
            .arg(destination)
            .args(["checkout", "--detach", REVISION]),
    )?;
    // Copy compressed archives only. Meson rechecks their upstream hashes and
    // extracts pristine sources; a developer's modified extracted wrap is ignored.
    let archives = local.join("subprojects/packagecache");
    if archives.is_dir() {
        copy_tree(&archives, &destination.join("subprojects/packagecache"))?;
    }
    fs::write(destination.join(".git/cove-source-ready"), REVISION)?;
    Ok(())
}
fn copy_tree(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        if entry.file_name() == ".git" {
            continue;
        }
        let target = destination.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
