//! License materials for the source-built media profile. This inventory includes
//! build dependencies; it is not a claim about an application's final link closure.
use crate::gstreamer::Build;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Component, Path, PathBuf},
    process::Command,
};

/// Generate notices without rebuilding the native prefix. `workspace` must be
/// the Cove checkout whose locked media dependencies were used for the build.
/// The output is replaced only after all license inputs have been validated.
pub fn generate(build: &Build, workspace: &Path, destination: &Path) -> io::Result<()> {
    let destination = absolute(destination)?;
    let workspace = workspace.canonicalize()?;
    let source = build.source.canonicalize()?;
    if destination.starts_with(&source) || source.starts_with(&destination) {
        return Err(io::Error::other(
            "license output must be separate from upstream sources",
        ));
    }
    let manifest = fs::read_to_string(build.out_dir.join("build-manifest.txt"))?;
    if !manifest
        .lines()
        .any(|line| line == format!("target={}", build.target))
    {
        return Err(io::Error::other(
            "license target differs from native build manifest",
        ));
    }
    let directory = build.out_dir.join("build");
    let project = meson(&directory, "--projectinfo")?;
    let dependencies = meson(&directory, "--dependencies")?;
    let targets = meson(&directory, "--targets")?;
    let metadata = command_json(
        Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .current_dir(&workspace)
            .args([
                "metadata",
                "--locked",
                "--format-version",
                "1",
                "--filter-platform",
            ])
            .arg(&build.target)
            .args(["--features", "cove-media-rhi/gst"]),
    )?;
    let catalog = read_json(&workspace.join("LICENSES/media/native.json"))?;
    let supplements = read_json(&workspace.join("LICENSES/media/rust-supplements.json"))?;
    let mut copies = BTreeMap::new();
    let native = native_packages(&source, &project, &catalog, &mut copies)?;
    copies.insert(
        "native/source-copyrights.txt".into(),
        source_notices(&source, &targets)?.into_bytes(),
    );
    let rust = rust_packages(&workspace, &metadata, &supplements, &mut copies)?;
    // Include exact wrap hashes, local patches and nested wrap redirects.
    collect_wraps(&source.join("subprojects"), &source, &mut copies)?;
    add_file(
        &mut copies,
        "Cove-LICENSE".into(),
        &workspace.join("LICENSE"),
    )?;
    add_file(
        &mut copies,
        "Cargo.lock".into(),
        &workspace.join("Cargo.lock"),
    )?;
    add_file(
        &mut copies,
        "DISTRIBUTION.md".into(),
        &workspace.join("Docs/media-licensing.md"),
    )?;
    add_file(
        &mut copies,
        "build-manifest.txt".into(),
        &build.out_dir.join("build-manifest.txt"),
    )?;
    copies.insert("meson-project.json".into(), json_bytes(&project)?);
    copies.insert("meson-dependencies.json".into(), json_bytes(&dependencies)?);
    copies.insert("meson-targets.json".into(), json_bytes(&targets)?);
    let source_revision = git(&source, &["rev-parse", "HEAD"])?;
    let changes = git(&source, &["diff", "--binary", "HEAD"])?;
    copies.insert(
        "gstreamer-changes.patch".into(),
        changes.as_bytes().to_vec(),
    );
    let untracked = git(&source, &["ls-files", "--others", "--exclude-standard"])?;
    let inventory = json!({
        "schema": 1, "target": build.target, "gstreamer_revision": source_revision.trim(),
        "tracked_source_changes": !changes.is_empty(), "untracked_source_files": untracked.lines().collect::<Vec<_>>(),
        "scope": "GStreamer enabled Meson subprojects and Cove media/media-rhi Cargo normal + build dependency closure; not the final application closure",
        "native": native, "rust": rust,
    });
    copies.insert("inventory.json".into(), json_bytes(&inventory)?);
    copies.insert(
        "THIRD_PARTY_NOTICES.md".into(),
        notice_text(&inventory)?.into_bytes(),
    );
    write_bundle(&destination, &copies)
}

fn native_packages(
    source: &Path,
    project: &Value,
    catalog: &Value,
    copies: &mut BTreeMap<PathBuf, Vec<u8>>,
) -> io::Result<Vec<Value>> {
    let definitions = array(catalog)?;
    let mut packages = Vec::new();
    for subproject in array(&project["subprojects"])? {
        let name = string(subproject, "name")?;
        let definition = definitions
            .iter()
            .find(|entry| entry["name"] == name)
            .ok_or_else(|| {
                io::Error::other(format!(
                    "unreviewed native license: {name}; update LICENSES/media/native.json"
                ))
            })?;
        let root = source.join(relative(string(definition, "directory")?)?);
        let output = PathBuf::from("native").join(name);
        let mut files = Vec::new();
        for file in array(&definition["files"])? {
            let path = relative(value_string(file)?)?;
            collect_files(&root.join(&path), &output.join(&path), copies, &mut files)?;
        }
        if let Some(headers) = definition.get("headers") {
            for header in array(headers)? {
                let header = relative(value_string(header)?)?;
                let text = fs::read_to_string(root.join(&header))?;
                let path = output.join(header).with_extension("notice.txt");
                copies.insert(path.clone(), header_notice(&text)?.as_bytes().to_vec());
                files.push(path);
            }
        }
        packages.push(json!({"name":name, "version":string(subproject, "version")?,
            "license":string(definition, "license")?, "source_directory":definition["directory"], "files":files}));
    }
    packages.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    Ok(packages)
}

/// Follow normal and build edges only; dev-only dependencies are not shipped.
fn closure(metadata: &Value) -> io::Result<BTreeSet<String>> {
    let packages = array(&metadata["packages"])?;
    let roots: Vec<_> = packages
        .iter()
        .filter(|p| {
            ["cove-media", "cove-media-rhi"].contains(&p["name"].as_str().unwrap_or_default())
        })
        .collect();
    if roots.len() != 2 {
        return Err(io::Error::other(
            "Cargo metadata must contain both Cove media roots",
        ));
    }
    let nodes: BTreeMap<_, _> = array(&metadata["resolve"]["nodes"])?
        .iter()
        .map(|node| Ok((string(node, "id")?, node)))
        .collect::<io::Result<_>>()?;
    let mut pending: Vec<_> = roots
        .iter()
        .map(|p| string(p, "id").map(str::to_owned))
        .collect::<io::Result<_>>()?;
    let mut selected = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !selected.insert(id.clone()) {
            continue;
        }
        let node = nodes
            .get(id.as_str())
            .ok_or_else(|| io::Error::other(format!("missing Cargo resolve node: {id}")))?;
        for dep in array(&node["deps"])? {
            if array(&dep["dep_kinds"])?
                .iter()
                .any(|kind| kind["kind"].is_null() || kind["kind"] == "build")
            {
                pending.push(string(dep, "pkg")?.to_owned());
            }
        }
    }
    Ok(selected)
}

fn rust_packages(
    workspace: &Path,
    metadata: &Value,
    supplements: &Value,
    copies: &mut BTreeMap<PathBuf, Vec<u8>>,
) -> io::Result<Vec<Value>> {
    let selected = closure(metadata)?;
    let mut packages = Vec::new();
    for package in array(&metadata["packages"])? {
        if !selected.contains(string(package, "id")?) || package["source"].is_null() {
            continue;
        }
        let name = string(package, "name")?;
        let version = string(package, "version")?;
        let root = Path::new(string(package, "manifest_path")?)
            .parent()
            .ok_or_else(|| io::Error::other("crate manifest has no parent"))?;
        let output = PathBuf::from("rust").join(format!("{name}-{version}"));
        let mut files = Vec::new();
        for entry in sorted_entries(root)? {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if is_notice(&file_name) {
                collect_files(&entry.path(), &output.join(file_name), copies, &mut files)?;
            }
        }
        if let Some(file) = package["license_file"].as_str() {
            let file = relative(file)?;
            collect_files(&root.join(&file), &output.join(file), copies, &mut files)?;
        }
        let supplement = array(supplements)?
            .iter()
            .find(|s| s["name"] == name && s["version"] == version);
        if let Some(supplement) = supplement {
            let vcs = read_json(&root.join(".cargo_vcs_info.json"))?;
            if vcs["git"]["sha1"] != supplement["revision"]
                || package["license"] != supplement["license"]
            {
                return Err(io::Error::other(format!(
                    "stale upstream license supplement: {name}-{version}"
                )));
            }
            for file in array(&supplement["files"])? {
                let file = relative(value_string(file)?)?;
                collect_files(
                    &workspace.join("LICENSES/media").join(&file),
                    &output.join(
                        file.file_name()
                            .ok_or_else(|| io::Error::other("supplement filename"))?,
                    ),
                    copies,
                    &mut files,
                )?;
            }
            copies.insert(output.join("upstream.json"), json_bytes(supplement)?);
        }
        if files.is_empty() {
            return Err(io::Error::other(format!(
                "no license text for {name}-{version}; add a versioned upstream supplement"
            )));
        }
        files.sort();
        files.dedup();
        let mut record = json!({"name":name,"version":version,"license":package["license"],"source":package["source"],"repository":package["repository"],"authors":package["authors"],"files":files});
        if let Some(supplement) = supplement {
            record["selected_license"] = supplement["selected_license"].clone();
        }
        if root.join(".cargo-checksum.json").is_file() {
            record["checksum"] = read_json(&root.join(".cargo-checksum.json"))?["package"].clone();
        }
        packages.push(record);
    }
    packages.sort_by(|a, b| {
        (a["name"].as_str(), a["version"].as_str())
            .cmp(&(b["name"].as_str(), b["version"].as_str()))
    });
    Ok(packages)
}

fn notice_text(inventory: &Value) -> io::Result<String> {
    let mut text = format!(
        "# Third-party notices — Cove media\n\nTarget: `{}`. Cove's own code is Apache-2.0 (see [Cove-LICENSE](Cove-LICENSE)). Third-party code retains its upstream terms.\n\nThis directory contains license texts and a build inventory, not the corresponding source or application relink materials. See [DISTRIBUTION.md](DISTRIBUTION.md) for those separate obligations. Build-time dependencies and upstream supplementary license texts may be included even when not linked. Other Cove native libraries and application dependencies require their own inventory.\n\n",
        string(inventory, "target")?
    );
    text.push_str("Upstream copyright and permission comments from the configured native targets are retained in [native/source-copyrights.txt](native/source-copyrights.txt). These include build tools and configured support libraries, not only the final executable.\n\n");
    for (key, title) in [
        ("native", "Native source projects"),
        ("rust", "Rust normal and build dependencies"),
    ] {
        text.push_str(&format!("## {title}\n\n"));
        for package in array(&inventory[key])? {
            text.push_str(&format!(
                "### {} {}\n\nDeclared license: `{}`.\n\n",
                string(package, "name")?,
                string(package, "version")?,
                package["license"].as_str().unwrap_or("See license file")
            ));
            for file in array(&package["files"])? {
                let file = value_string(file)?.replace('\\', "/");
                text.push_str(&format!("- [{file}](<{file}>)\n"));
            }
            text.push('\n');
        }
    }
    Ok(text)
}

fn source_notices(source: &Path, targets: &Value) -> io::Result<String> {
    let mut paths = BTreeSet::new();
    for target in array(targets)? {
        for group in array(&target["target_sources"])? {
            // Meson custom targets have no source list in some versions.
            if let Some(sources) = group.get("sources") {
                for path in array(sources)? {
                    let path = Path::new(value_string(path)?);
                    if path.is_file() {
                        let path = path.canonicalize()?;
                        if path.starts_with(source) {
                            paths.insert(path);
                        }
                    }
                }
            }
        }
    }
    let mut text = String::from(
        "Copyright and permission comments from configured native source targets.\nSee the separate full license texts and corresponding source for per-file terms.\n\n",
    );
    for path in paths {
        // Binary inputs and generated build scripts do not carry C comments.
        if !path.extension().is_some_and(|ext| {
            ["c", "cc", "cpp", "h", "m", "mm", "S"]
                .iter()
                .any(|name| ext == *name)
        }) {
            continue;
        }
        let data = fs::read_to_string(&path)?;
        if let Ok(notice) = header_notice(&data) {
            let relative = path.strip_prefix(source).map_err(io::Error::other)?;
            text.push_str(&format!("===== {} =====\n{notice}\n\n", relative.display()));
        }
    }
    Ok(text)
}

fn header_notice(text: &str) -> io::Result<&str> {
    let copyright = text
        .to_ascii_lowercase()
        .find("copyright")
        .ok_or_else(|| io::Error::other("missing upstream copyright notice"))?;
    let start = text[..copyright]
        .rfind("/*")
        .ok_or_else(|| io::Error::other("copyright is not in a C comment"))?;
    let end = text[copyright..]
        .find("*/")
        .ok_or_else(|| io::Error::other("unterminated license comment"))?
        + copyright
        + 2;
    Ok(&text[start..end])
}

fn collect_wraps(
    directory: &Path,
    source: &Path,
    copies: &mut BTreeMap<PathBuf, Vec<u8>>,
) -> io::Result<()> {
    for entry in sorted_entries(directory)? {
        let path = entry.path();
        let name = entry.file_name();
        if name == ".git" || name == "packagecache" {
            continue;
        }
        if entry.file_type()?.is_dir() {
            collect_wraps(&path, source, copies)?;
        } else if path
            .extension()
            .is_some_and(|ext| ext == "wrap" || ext == "patch")
        {
            let relative = path.strip_prefix(source).map_err(io::Error::other)?;
            add_file(
                copies,
                PathBuf::from("source-records").join(relative),
                &path,
            )?;
        }
    }
    Ok(())
}
fn collect_files(
    source: &Path,
    output: &Path,
    copies: &mut BTreeMap<PathBuf, Vec<u8>>,
    files: &mut Vec<PathBuf>,
) -> io::Result<()> {
    if source.is_dir() {
        for entry in sorted_entries(source)? {
            collect_files(
                &entry.path(),
                &output.join(entry.file_name()),
                copies,
                files,
            )?;
        }
    } else {
        add_file(copies, output.to_owned(), source)?;
        files.push(output.to_owned());
    }
    Ok(())
}
fn add_file(
    copies: &mut BTreeMap<PathBuf, Vec<u8>>,
    output: PathBuf,
    source: &Path,
) -> io::Result<()> {
    let data = fs::read(source)
        .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", source.display())))?;
    if data.is_empty() {
        return Err(io::Error::other(format!(
            "empty license input: {}",
            source.display()
        )));
    }
    copies.insert(output, data);
    Ok(())
}
fn write_bundle(destination: &Path, copies: &BTreeMap<PathBuf, Vec<u8>>) -> io::Result<()> {
    if destination.exists() && !destination.join(".cove-media-notices").is_file() {
        return Err(io::Error::other(
            "refusing to replace a directory not owned by the media notice generator",
        ));
    }
    fs::create_dir_all(destination)?;
    // Keep unrelated files safe and remove stale licenses when the dependency set shrinks.
    if destination.join(".cove-media-notices").exists() {
        fs::remove_dir_all(destination)?;
    }
    fs::create_dir_all(destination)?;
    fs::write(destination.join(".cove-media-notices"), "1\n")?;
    for (path, data) in copies {
        let path = destination.join(path);
        fs::create_dir_all(
            path.parent()
                .ok_or_else(|| io::Error::other("license output parent"))?,
        )?;
        fs::write(path, data)?;
    }
    Ok(())
}
fn absolute(path: &Path) -> io::Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}
fn relative(value: &str) -> io::Result<PathBuf> {
    let path = PathBuf::from(value);
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|p| !matches!(p, Component::Normal(_)))
    {
        return Err(io::Error::other(format!(
            "expected safe relative license path: {value}"
        )));
    }
    Ok(path)
}
fn is_notice(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "license",
        "licence",
        "copying",
        "copyright",
        "notice",
        "unlicense",
    ]
    .iter()
    .any(|prefix| name.starts_with(prefix))
}
fn sorted_entries(path: &Path) -> io::Result<Vec<fs::DirEntry>> {
    let mut entries = fs::read_dir(path)?.collect::<io::Result<Vec<_>>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    Ok(entries)
}
fn string<'a>(value: &'a Value, key: &str) -> io::Result<&'a str> {
    value_string(&value[key])
}
fn value_string(value: &Value) -> io::Result<&str> {
    value
        .as_str()
        .ok_or_else(|| io::Error::other("expected a license metadata string"))
}
fn array(value: &Value) -> io::Result<&Vec<Value>> {
    value
        .as_array()
        .ok_or_else(|| io::Error::other("expected a license metadata array"))
}
fn read_json(path: &Path) -> io::Result<Value> {
    serde_json::from_slice(&fs::read(path)?).map_err(io::Error::other)
}
fn json_bytes(value: &Value) -> io::Result<Vec<u8>> {
    serde_json::to_vec_pretty(value).map_err(io::Error::other)
}
fn command_json(command: &mut Command) -> io::Result<Value> {
    let output = command.output()?;
    if !output.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(io::Error::other)
}
fn meson(directory: &Path, argument: &str) -> io::Result<Value> {
    command_json(
        Command::new("meson")
            .args(["introspect", argument])
            .arg(directory),
    )
}
fn git(source: &Path, arguments: &[&str]) -> io::Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(source)
        .args(arguments)
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    String::from_utf8(output.stdout).map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "cove-license-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).expect("temporary license directory");
            Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove temporary license directory");
        }
    }

    #[test]
    fn follows_build_edges_and_mixed_edges_but_not_dev_only_edges() {
        let metadata = json!({"packages":[{"name":"cove-media","id":"media"},{"name":"cove-media-rhi","id":"rhi"}],"resolve":{"nodes":[
            {"id":"media","deps":[{"pkg":"build","dep_kinds":[{"kind":"build"}]},{"pkg":"dev","dep_kinds":[{"kind":"dev"}]}]},
            {"id":"rhi","deps":[{"pkg":"mixed","dep_kinds":[{"kind":"dev"},{"kind":null}]}]},
            {"id":"build","deps":[]},{"id":"mixed","deps":[]}
        ]}});
        assert_eq!(
            closure(&metadata).expect("closure"),
            ["media", "rhi", "build", "mixed"].map(str::to_owned).into()
        );
    }
    #[test]
    fn extracts_the_complete_copyright_permission_and_warranty_comment_verbatim() {
        let text = "#ifndef HEADER\n/* Copyright holder\nPermission granted\nNO WARRANTY\n*/\nint unrelated;";
        assert_eq!(
            header_notice(text).expect("notice"),
            "/* Copyright holder\nPermission granted\nNO WARRANTY\n*/"
        );
        assert!(header_notice("// no license").is_err());
    }
    #[test]
    fn unknown_native_projects_require_a_reviewed_license_record() {
        let project = json!({"subprojects":[{"name":"new-codec","version":"1"}]});
        assert!(
            native_packages(Path::new("."), &project, &json!([]), &mut BTreeMap::new())
                .unwrap_err()
                .to_string()
                .contains("new-codec")
        );
    }
    #[test]
    fn source_paths_cannot_escape_the_license_bundle() {
        for path in ["../LICENSE", "/LICENSE", "a/../../LICENSE", ""] {
            assert!(relative(path).is_err());
        }
        assert_eq!(
            relative("LICENSES/Unicode.txt").expect("safe path"),
            PathBuf::from("LICENSES/Unicode.txt")
        );
    }

    #[test]
    fn copies_all_alternative_and_additional_license_texts_byte_for_byte() {
        let directory = Directory::new();
        let license = "(MIT OR Apache-2.0) AND Unicode-3.0";
        let texts = [
            ("LICENSE-MIT", b"MIT\r\nCopyright A\r\n".as_slice()),
            ("LICENSE-APACHE", b"Apache\n".as_slice()),
            ("LICENSE-UNICODE", b"Unicode\n".as_slice()),
        ];
        for (name, bytes) in texts {
            fs::write(directory.0.join(name), bytes).expect("license fixture");
        }
        let metadata = json!({"packages":[{"name":"cove-media","id":"media"},{"name":"cove-media-rhi","id":"rhi"},
            {"name":"unicode-fixture","version":"1","id":"fixture","source":"registry+test","license":license,"manifest_path":directory.0.join("Cargo.toml")}],
            "resolve":{"nodes":[{"id":"media","deps":[{"pkg":"fixture","dep_kinds":[{"kind":null}]}]}, {"id":"rhi","deps":[]},{"id":"fixture","deps":[]}]}});
        let mut copies = BTreeMap::new();
        let records =
            rust_packages(&directory.0, &metadata, &json!([]), &mut copies).expect("Rust notices");
        assert_eq!(records[0]["license"], license);
        assert_eq!(records[0]["files"].as_array().expect("files").len(), 3);
        for (name, bytes) in texts {
            assert_eq!(
                copies[&PathBuf::from("rust/unicode-fixture-1").join(name)],
                bytes
            );
        }
        fs::remove_file(directory.0.join("LICENSE-MIT")).expect("remove MIT");
        fs::remove_file(directory.0.join("LICENSE-APACHE")).expect("remove Apache");
        fs::remove_file(directory.0.join("LICENSE-UNICODE")).expect("remove Unicode");
        assert!(rust_packages(&directory.0, &metadata, &json!([]), &mut BTreeMap::new()).is_err());
    }

    #[test]
    fn only_replaces_owned_bundles_and_removes_stale_dependencies() {
        let directory = Directory::new();
        let output = directory.0.join("bundle");
        fs::create_dir(&output).expect("unowned directory");
        fs::write(output.join("keep"), "user data").expect("unrelated file");
        let first = BTreeMap::from([(PathBuf::from("old/LICENSE"), b"old\r\n".to_vec())]);
        assert!(write_bundle(&output, &first).is_err());
        assert_eq!(
            fs::read_to_string(output.join("keep")).expect("preserved file"),
            "user data"
        );
        fs::remove_dir_all(&output).expect("fixture cleanup");
        write_bundle(&output, &first).expect("first bundle");
        assert_eq!(
            fs::read(output.join("old/LICENSE")).expect("copied file"),
            b"old\r\n"
        );
        write_bundle(
            &output,
            &BTreeMap::from([(PathBuf::from("new/LICENSE"), b"new".to_vec())]),
        )
        .expect("replacement");
        assert!(!output.join("old").exists());
        assert_eq!(
            fs::read(output.join("new/LICENSE")).expect("new license"),
            b"new"
        );
    }
}
