//! Read-only deep-link configuration inspection for a Cove project.
//!
//! Flutter DevTools validates the platform files that turn a URL into an app
//! launch: Android intent filters and their website association, and Apple URL
//! types and associated domains. Cove projects keep the same files under
//! `Platforms/`. This module reads only those bounded, known paths beneath the
//! supplied project root and reports what is actually declared. An empty
//! project is a valid answer, not an error.
use cove_devtools_protocol::control::Fault;
use plist::Value as Plist;
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Cursor, Read as _};
use std::path::{Component, Path, PathBuf};

/// One configuration file cannot exceed this size. These are manifests and
/// entitlement lists, not application data.
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_SCAN_DEPTH: usize = 4;
const MAX_SCAN_ENTRIES: usize = 256;

const ANDROID_MANIFEST: &str = "Platforms/Android/app/src/main/AndroidManifest.xml";
const MACOS_INFO: &str = "Platforms/macOS/Info.plist";
const IOS_DIR: &str = "Platforms/iOS";
const MACOS_DIR: &str = "Platforms/macOS";

const ANDROID_ASSOCIATIONS: [&str; 4] = [
    "Platforms/Android/app/src/main/assets/.well-known/assetlinks.json",
    "Platforms/Android/.well-known/assetlinks.json",
    "Platforms/Android/assetlinks.json",
    ".well-known/assetlinks.json",
];
const APPLE_ASSOCIATIONS: [&str; 5] = [
    "Platforms/iOS/.well-known/apple-app-site-association",
    "Platforms/iOS/apple-app-site-association",
    "Platforms/macOS/.well-known/apple-app-site-association",
    "Platforms/macOS/apple-app-site-association",
    ".well-known/apple-app-site-association",
];

fn invalid(message: impl Into<String>) -> Fault {
    Fault::new("invalid_request", message)
}

/// Resolves the project root without allowing a caller to address files outside
/// it. The root itself may be a symlink; its canonical target is what confines
/// every later read.
fn canonical_root(input: &str) -> Result<PathBuf, Fault> {
    if input.is_empty() {
        return Err(invalid("root must be a canonical absolute project path."));
    }
    let path = Path::new(input);
    if !path.is_absolute() {
        return Err(invalid(format!(
            "root must be an absolute path, got {input:?}."
        )));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(invalid(format!(
            "root must not contain '..' components: {input:?}."
        )));
    }
    let canonical = fs::canonicalize(path)
        .map_err(|error| invalid(format!("root {input:?} is not readable: {error}.")))?;
    let metadata = fs::metadata(&canonical)
        .map_err(|error| invalid(format!("root {input:?} is not readable: {error}.")))?;
    if !metadata.is_dir() {
        return Err(invalid(format!("root {input:?} is not a directory.")));
    }
    Ok(canonical)
}

/// Checks that a relative candidate has only normal components. Callers build
/// candidates from constants; this keeps that invariant explicit.
fn relative(candidate: &str) -> PathBuf {
    let path = Path::new(candidate);
    debug_assert!(
        path.components()
            .all(|component| matches!(component, Component::Normal(_))),
        "deep-link candidates use only normal path components"
    );
    path.to_path_buf()
}

enum Read {
    Missing,
    Content(Vec<u8>),
    Invalid { code: &'static str, message: String },
}

/// Reads one allowlisted path and proves it resolves inside `root`.
///
/// A symlink that leaves the root is a request fault, never a read. A path that
/// exists but cannot be read is reported as an issue because it is project
/// configuration, not a malformed request.
fn read_file(root: &Path, candidate: &str) -> Result<Read, Fault> {
    let path = root.join(relative(candidate));
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Read::Missing),
        Err(error) => {
            return Ok(Read::Invalid {
                code: "unreadable_file",
                message: format!("{candidate} could not be inspected: {error}."),
            });
        }
    };
    if metadata.file_type().is_symlink() && !path.exists() {
        return Ok(Read::Missing);
    }
    let resolved = fs::canonicalize(&path).map_err(|error| {
        invalid(format!(
            "refusing to read {candidate}: it cannot be resolved inside the project root ({error})."
        ))
    })?;
    if !resolved.starts_with(root) {
        return Err(invalid(format!(
            "refusing to read {candidate}: it resolves outside {}.",
            root.display()
        )));
    }
    let metadata = fs::metadata(&resolved).map_err(|error| {
        invalid(format!(
            "refusing to read {candidate}: its target is not readable ({error})."
        ))
    })?;
    if !metadata.is_file() {
        return Ok(Read::Invalid {
            code: "unreadable_file",
            message: format!("{candidate} is not a regular file."),
        });
    }
    if metadata.len() > MAX_FILE_BYTES {
        return Ok(Read::Invalid {
            code: "file_too_large",
            message: format!(
                "{candidate} is {} bytes; the limit is {MAX_FILE_BYTES}.",
                metadata.len()
            ),
        });
    }
    let file = match fs::File::open(&resolved) {
        Ok(file) => file,
        Err(error) => {
            return Ok(Read::Invalid {
                code: "unreadable_file",
                message: format!("{candidate} could not be read: {error}."),
            });
        }
    };
    let mut bytes = Vec::new();
    match file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes) {
        Ok(_) if bytes.len() as u64 > MAX_FILE_BYTES => Ok(Read::Invalid {
            code: "file_too_large",
            message: format!("{candidate} exceeds the {MAX_FILE_BYTES}-byte limit."),
        }),
        Ok(_) => Ok(Read::Content(bytes)),
        Err(error) => Ok(Read::Invalid {
            code: "unreadable_file",
            message: format!("{candidate} could not be read: {error}."),
        }),
    }
}

fn line_at(bytes: &[u8], position: u64) -> usize {
    let end = usize::try_from(position)
        .unwrap_or(bytes.len())
        .min(bytes.len());
    bytes[..end].iter().filter(|byte| **byte == b'\n').count() + 1
}

fn issue(
    platform: &str,
    severity: &str,
    code: &str,
    message: impl Into<String>,
    file: &str,
    line: Option<usize>,
    fix: &str,
) -> Value {
    let location = match line {
        Some(line) => format!("{file}:{line}"),
        None => file.to_owned(),
    };
    let mut value = json!({
        "platform": platform,
        "severity": severity,
        "code": code,
        "message": message.into(),
        "file": file,
        "location": location,
        "fix": fix,
    });
    if let Some(line) = line {
        value["line"] = json!(line);
    }
    value
}

#[derive(Default)]
struct Issues {
    values: Vec<Value>,
}

impl Issues {
    fn push(&mut self, value: Value) {
        self.values.push(value);
    }

    fn errors(&self) -> usize {
        self.values
            .iter()
            .filter(|issue| issue["severity"] == "error")
            .count()
    }

    fn warnings(&self) -> usize {
        self.values
            .iter()
            .filter(|issue| issue["severity"] == "warning")
            .count()
    }

    fn for_platform(&self, platform: &str) -> Vec<Value> {
        self.values
            .iter()
            .filter(|issue| issue["platform"] == platform)
            .cloned()
            .collect()
    }
}

/// One Android `<intent-filter>` that declares at least one data scheme.
struct AndroidFilter {
    activity: String,
    schemes: Vec<String>,
    hosts: Vec<String>,
    paths: Vec<String>,
    auto_verify: bool,
    view_action: bool,
    browsable: bool,
    line: usize,
}

fn attribute(reader: &Reader<&[u8]>, element: &BytesStart<'_>, name: &str) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.local_name().as_ref() == name.as_bytes())
        .and_then(|attribute| {
            attribute
                .decoded_and_normalized_value(quick_xml::XmlVersion::Implicit1_0, reader.decoder())
                .ok()
                .map(|value| value.into_owned())
        })
}

fn element_name(name: &[u8]) -> String {
    String::from_utf8_lossy(name).into_owned()
}

/// Parses the manifest with a real XML reader. Returns the deep-link filters
/// and whether the document was well formed.
fn parse_manifest(contents: &[u8], file: &str, issues: &mut Issues) -> (Vec<AndroidFilter>, bool) {
    let text = match std::str::from_utf8(contents) {
        Ok(text) => text,
        Err(error) => {
            issues.push(issue(
                "android",
                "error",
                "malformed_manifest",
                format!("AndroidManifest.xml is not UTF-8: {error}."),
                file,
                None,
                "Save AndroidManifest.xml as UTF-8.",
            ));
            return (Vec::new(), false);
        }
    };
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut filters = Vec::new();
    let mut current_activity: Option<String> = None;
    let mut current: Option<AndroidFilter> = None;
    let mut saw_manifest = false;
    let mut malformed = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) | Ok(Event::Empty(element)) => {
                match element_name(element.local_name().as_ref()).as_str() {
                    "manifest" => saw_manifest = true,
                    "activity" | "activity-alias" => {
                        current_activity = attribute(&reader, &element, "name");
                    }
                    "intent-filter" => {
                        if current.is_none() {
                            current = Some(AndroidFilter {
                                activity: current_activity.clone().unwrap_or_default(),
                                schemes: Vec::new(),
                                hosts: Vec::new(),
                                paths: Vec::new(),
                                auto_verify: attribute(&reader, &element, "autoVerify")
                                    .is_some_and(|value| value.eq_ignore_ascii_case("true")),
                                view_action: false,
                                browsable: false,
                                line: line_at(contents, reader.buffer_position()),
                            });
                        }
                    }
                    "action" => {
                        if let Some(filter) = &mut current
                            && attribute(&reader, &element, "name").as_deref()
                                == Some("android.intent.action.VIEW")
                        {
                            filter.view_action = true;
                        }
                    }
                    "category" => {
                        if let Some(filter) = &mut current
                            && attribute(&reader, &element, "name").as_deref()
                                == Some("android.intent.category.BROWSABLE")
                        {
                            filter.browsable = true;
                        }
                    }
                    "data" => {
                        if let Some(filter) = &mut current {
                            if let Some(scheme) = attribute(&reader, &element, "scheme") {
                                filter.schemes.push(scheme.to_ascii_lowercase());
                            }
                            if let Some(host) = attribute(&reader, &element, "host") {
                                filter.hosts.push(host.to_ascii_lowercase());
                            }
                            for name in ["path", "pathPrefix", "pathPattern"] {
                                if let Some(path) = attribute(&reader, &element, name) {
                                    filter.paths.push(path);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(element)) => match element_name(element.local_name().as_ref()).as_str() {
                "intent-filter" => {
                    if let Some(mut filter) = current.take()
                        && !filter.schemes.is_empty()
                    {
                        filter.schemes.sort();
                        filter.schemes.dedup();
                        filter.hosts.sort();
                        filter.hosts.dedup();
                        filter.paths.sort();
                        filter.paths.dedup();
                        filters.push(filter);
                    }
                }
                "activity" | "activity-alias" => current_activity = None,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => {
                malformed = true;
                let line = line_at(contents, reader.buffer_position());
                issues.push(issue(
                    "android",
                    "error",
                    "malformed_manifest",
                    format!("AndroidManifest.xml is not valid XML: {error}."),
                    file,
                    Some(line),
                    "Fix the XML around this line and rebuild.",
                ));
                break;
            }
        }
    }
    if !malformed && !saw_manifest {
        malformed = true;
        issues.push(issue(
            "android",
            "error",
            "malformed_manifest",
            "AndroidManifest.xml has no <manifest> root element.",
            file,
            None,
            "Restore the <manifest> root element.",
        ));
    }
    (filters, !malformed)
}

fn valid_android_scheme(scheme: &str) -> bool {
    let mut characters = scheme.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && characters.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        })
}

fn valid_android_host(host: &str) -> bool {
    !host.is_empty()
        && !host.starts_with('.')
        && !host.ends_with('.')
        && !host.contains('/')
        && !host.contains(':')
        && !host.contains('*')
        && !host.chars().any(char::is_whitespace)
}

/// Associated-domain hosts may be wildcards but never carry a scheme or path.
fn valid_domain(domain: &str) -> bool {
    let domain = domain.strip_prefix("*.").unwrap_or(domain);
    !domain.is_empty()
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains('/')
        && !domain.contains(':')
        && !domain.chars().any(char::is_whitespace)
}

fn file_entry(root: &Path, candidate: &str, platform: &str, kind: &str) -> Value {
    json!({
        "path": root.join(relative(candidate)).display().to_string(),
        "relative": candidate,
        "platform": platform,
        "kind": kind,
    })
}

struct AndroidReport {
    configured: bool,
    schemes: Vec<String>,
    hosts: Vec<String>,
    links: Value,
    web_hosts: BTreeMap<String, usize>,
}

fn collect_android(
    root: &Path,
    issues: &mut Issues,
    files: &mut Vec<Value>,
) -> Result<AndroidReport, Fault> {
    let mut report = AndroidReport {
        configured: false,
        schemes: Vec::new(),
        hosts: Vec::new(),
        links: json!([]),
        web_hosts: BTreeMap::new(),
    };
    match read_file(root, ANDROID_MANIFEST)? {
        Read::Missing => {}
        Read::Invalid { code, message } => issues.push(issue(
            "android",
            "error",
            code,
            message,
            ANDROID_MANIFEST,
            None,
            "Restore AndroidManifest.xml as a readable regular file.",
        )),
        Read::Content(contents) => {
            files.push(file_entry(root, ANDROID_MANIFEST, "android", "manifest"));
            let (filters, well_formed) = parse_manifest(&contents, ANDROID_MANIFEST, issues);
            if well_formed {
                let mut schemes = BTreeSet::new();
                let mut hosts = BTreeSet::new();
                let mut seen = BTreeMap::new();
                let mut links = Vec::new();
                for filter in &filters {
                    let web = filter
                        .schemes
                        .iter()
                        .any(|scheme| scheme == "http" || scheme == "https");
                    for scheme in &filter.schemes {
                        schemes.insert(scheme.clone());
                        if !valid_android_scheme(scheme) {
                            issues.push(issue(
                                "android",
                                "error",
                                "invalid_scheme",
                                format!("{scheme:?} is not a valid URI scheme."),
                                ANDROID_MANIFEST,
                                Some(filter.line),
                                "Use lowercase letters, digits, '+', '-' or '.'.",
                            ));
                        }
                        if filter.hosts.is_empty() {
                            let severity = if scheme == "http" || scheme == "https" {
                                "error"
                            } else {
                                "warning"
                            };
                            issues.push(issue(
                                "android",
                                severity,
                                "scheme_without_host",
                                format!(
                                    "Scheme {scheme:?} has no android:host, so every authority matches."
                                ),
                                ANDROID_MANIFEST,
                                Some(filter.line),
                                &format!(
                                    "Add <data android:scheme=\"{scheme}\" android:host=\"your.host\" />."
                                ),
                            ));
                        }
                    }
                    for host in &filter.hosts {
                        hosts.insert(host.clone());
                        if web {
                            report.web_hosts.entry(host.clone()).or_insert(filter.line);
                        }
                        if !valid_android_host(host) {
                            issues.push(issue(
                                "android",
                                "error",
                                "invalid_host",
                                format!("{host:?} is not a valid android:host."),
                                ANDROID_MANIFEST,
                                Some(filter.line),
                                "Use a bare host such as example.com.",
                            ));
                        }
                    }
                    if !filter.view_action {
                        issues.push(issue(
                            "android",
                            "error",
                            "missing_view_action",
                            "The intent filter has no android.intent.action.VIEW action.",
                            ANDROID_MANIFEST,
                            Some(filter.line),
                            "Add <action android:name=\"android.intent.action.VIEW\" />.",
                        ));
                    }
                    if !filter.browsable {
                        issues.push(issue(
                            "android",
                            "error",
                            "missing_browsable",
                            "The intent filter has no android.intent.category.BROWSABLE category, so browsers cannot open it.",
                            ANDROID_MANIFEST,
                            Some(filter.line),
                            "Add <category android:name=\"android.intent.category.BROWSABLE\" />.",
                        ));
                    }
                    if web && !filter.auto_verify {
                        issues.push(issue(
                            "android",
                            "warning",
                            "missing_auto_verify",
                            "This filter matches a web host but does not set android:autoVerify=\"true\".",
                            ANDROID_MANIFEST,
                            Some(filter.line),
                            "Set android:autoVerify=\"true\" on the intent-filter.",
                        ));
                    }
                    let key = json!({
                        "schemes": filter.schemes,
                        "hosts": filter.hosts,
                        "paths": filter.paths,
                    });
                    if let Some(previous) = seen.insert(key.to_string(), filter.line) {
                        issues.push(issue(
                            "android",
                            "warning",
                            "duplicate_link",
                            format!("This intent filter duplicates the one at line {previous}."),
                            ANDROID_MANIFEST,
                            Some(filter.line),
                            "Remove the duplicate filter or give it a distinct scheme, host or path.",
                        ));
                    }
                    links.push(json!({
                        "activity": filter.activity,
                        "schemes": filter.schemes,
                        "hosts": filter.hosts,
                        "paths": filter.paths,
                        "autoVerify": filter.auto_verify,
                        "viewAction": filter.view_action,
                        "browsable": filter.browsable,
                        "file": ANDROID_MANIFEST,
                        "line": filter.line,
                    }));
                }
                report.configured = !filters.is_empty();
                report.schemes = schemes.into_iter().collect();
                report.hosts = hosts.into_iter().collect();
                report.links = json!(links);
            }
        }
    }

    let mut association_present = false;
    for candidate in ANDROID_ASSOCIATIONS {
        match read_file(root, candidate)? {
            Read::Missing => {}
            Read::Content(bytes) => {
                association_present = true;
                files.push(file_entry(root, candidate, "android", "assetlinks"));
                validate_assetlinks(&bytes, candidate, issues);
            }
            Read::Invalid { code, message } => issues.push(issue(
                "android",
                "error",
                code,
                message,
                candidate,
                None,
                "Replace the association file with readable JSON.",
            )),
        }
    }
    if !association_present {
        for (host, line) in &report.web_hosts {
            issues.push(issue(
                "android",
                "warning",
                "missing_association_file",
                format!("https://{host}/.well-known/assetlinks.json was not found in the project."),
                ANDROID_MANIFEST,
                Some(*line),
                &format!(
                    "Publish https://{host}/.well-known/assetlinks.json and keep a copy at {}.",
                    ANDROID_ASSOCIATIONS[0]
                ),
            ));
        }
    }
    Ok(report)
}

fn validate_assetlinks(contents: &[u8], file: &str, issues: &mut Issues) {
    let root: Value = match serde_json::from_slice(contents) {
        Ok(value) => value,
        Err(error) => {
            issues.push(issue(
                "android",
                "error",
                "malformed_association",
                format!("assetlinks.json is not valid JSON: {error}."),
                file,
                None,
                "Serve a JSON array of Digital Asset Links statements.",
            ));
            return;
        }
    };
    let Some(statements) = root.as_array() else {
        issues.push(issue(
            "android",
            "error",
            "invalid_association",
            "assetlinks.json must be a JSON array.",
            file,
            None,
            "Wrap the statements in an array.",
        ));
        return;
    };
    if statements.is_empty() {
        issues.push(issue(
            "android",
            "warning",
            "invalid_association",
            "assetlinks.json has no statements.",
            file,
            None,
            "Add at least one delegate_permission/common.handle_all_urls statement.",
        ));
        return;
    }
    for statement in statements {
        let handles_urls = statement["relation"].as_array().is_some_and(|relations| {
            relations.iter().any(|relation| {
                relation.as_str() == Some("delegate_permission/common.handle_all_urls")
            })
        });
        let target = &statement["target"];
        let complete = target["namespace"].as_str() == Some("android_app")
            && target["package_name"]
                .as_str()
                .is_some_and(|name| !name.is_empty())
            && target["sha256_cert_fingerprints"]
                .as_array()
                .is_some_and(|fingerprints| !fingerprints.is_empty());
        if !handles_urls || !complete {
            issues.push(issue(
                "android",
                "error",
                "invalid_association",
                "An assetlinks statement is missing handle_all_urls, an android_app target, package_name or a SHA-256 fingerprint.",
                file,
                None,
                "Use relation delegate_permission/common.handle_all_urls and a complete android_app target.",
            ));
        }
    }
}

#[derive(Default)]
struct AppleLinks {
    schemes: BTreeSet<String>,
    schemes_by_file: BTreeSet<(String, String)>,
    domains: BTreeSet<String>,
    associated_domains: BTreeSet<String>,
    entitlements: BTreeSet<(String, String, String)>,
    association_domains: BTreeMap<String, String>,
}

fn parse_plist(contents: &[u8], file: &str, issues: &mut Issues) -> Option<Plist> {
    match Plist::from_reader(Cursor::new(contents)) {
        Ok(value) => Some(value),
        Err(error) => {
            let message = error.to_string();
            let offset = message
                .split("offset ")
                .nth(1)
                .and_then(|rest| {
                    rest.chars()
                        .take_while(char::is_ascii_digit)
                        .collect::<String>()
                        .parse::<u64>()
                        .ok()
                })
                .unwrap_or(0);
            let line = (offset > 0).then(|| line_at(contents, offset));
            issues.push(issue(
                "apple",
                "error",
                "malformed_plist",
                format!("{file} is not a valid property list: {error}."),
                file,
                line,
                "Fix the plist syntax or regenerate it from Xcode.",
            ));
            None
        }
    }
}

fn collect_plist(
    root: &Path,
    candidate: &str,
    platform: &str,
    links: &mut AppleLinks,
    issues: &mut Issues,
    files: &mut Vec<Value>,
) -> Result<(), Fault> {
    match read_file(root, candidate)? {
        Read::Missing => {}
        Read::Invalid { code, message } => issues.push(issue(
            "apple",
            "error",
            code,
            message,
            candidate,
            None,
            "Restore the property list as a readable regular file.",
        )),
        Read::Content(contents) => {
            files.push(file_entry(root, candidate, platform, "info_plist"));
            let Some(plist) = parse_plist(&contents, candidate, issues) else {
                return Ok(());
            };
            let Some(dictionary) = plist.as_dictionary() else {
                issues.push(issue(
                    "apple",
                    "error",
                    "invalid_plist",
                    format!("{candidate} does not contain a dictionary."),
                    candidate,
                    None,
                    "Make the plist root a <dict>.",
                ));
                return Ok(());
            };
            let Some(types) = dictionary.get("CFBundleURLTypes") else {
                return Ok(());
            };
            let Some(types) = types.as_array() else {
                issues.push(issue(
                    "apple",
                    "error",
                    "invalid_url_types",
                    format!("{candidate} has a CFBundleURLTypes value that is not an array."),
                    candidate,
                    None,
                    "Make CFBundleURLTypes an array of dictionaries.",
                ));
                return Ok(());
            };
            for url_type in types {
                let Some(schemes) = url_type
                    .as_dictionary()
                    .and_then(|dictionary| dictionary.get("CFBundleURLSchemes"))
                else {
                    continue;
                };
                let Some(schemes) = schemes.as_array() else {
                    issues.push(issue(
                        "apple",
                        "error",
                        "invalid_url_types",
                        format!("A CFBundleURLSchemes value in {candidate} is not an array."),
                        candidate,
                        None,
                        "Make CFBundleURLSchemes an array of strings.",
                    ));
                    continue;
                };
                for scheme in schemes {
                    let Some(scheme) = scheme.as_string() else {
                        continue;
                    };
                    if scheme.is_empty()
                        || scheme.contains(':')
                        || scheme.contains('/')
                        || scheme.chars().any(char::is_whitespace)
                    {
                        issues.push(issue(
                            "apple",
                            "error",
                            "invalid_scheme",
                            format!("{scheme:?} is not a valid CFBundleURLSchemes entry."),
                            candidate,
                            None,
                            "Use a bare scheme such as myapp.",
                        ));
                        continue;
                    }
                    let normalized = scheme.to_ascii_lowercase();
                    if !links
                        .schemes_by_file
                        .insert((candidate.to_owned(), normalized.clone()))
                    {
                        issues.push(issue(
                            "apple",
                            "warning",
                            "duplicate_scheme",
                            format!(
                                "URL scheme {scheme:?} is declared more than once in {candidate}."
                            ),
                            candidate,
                            None,
                            "Declare each scheme once per Info.plist.",
                        ));
                    }
                    links.schemes.insert(normalized);
                }
            }
        }
    }
    Ok(())
}

fn collect_entitlements(
    root: &Path,
    candidate: &str,
    platform: &str,
    links: &mut AppleLinks,
    issues: &mut Issues,
    files: &mut Vec<Value>,
) -> Result<(), Fault> {
    match read_file(root, candidate)? {
        Read::Missing => {}
        Read::Invalid { code, message } => issues.push(issue(
            "apple",
            "error",
            code,
            message,
            candidate,
            None,
            "Restore the entitlements as a readable property list.",
        )),
        Read::Content(contents) => {
            files.push(file_entry(root, candidate, platform, "entitlements"));
            let Some(plist) = parse_plist(&contents, candidate, issues) else {
                return Ok(());
            };
            let Some(dictionary) = plist.as_dictionary() else {
                issues.push(issue(
                    "apple",
                    "error",
                    "invalid_plist",
                    format!("{candidate} does not contain a dictionary."),
                    candidate,
                    None,
                    "Make the entitlements root a <dict>.",
                ));
                return Ok(());
            };
            let Some(domains) = dictionary.get("com.apple.developer.associated-domains") else {
                return Ok(());
            };
            let Some(domains) = domains.as_array() else {
                issues.push(issue(
                    "apple",
                    "error",
                    "invalid_domain",
                    format!(
                        "{candidate} has a com.apple.developer.associated-domains value that is not an array."
                    ),
                    candidate,
                    None,
                    "Make the associated-domains entitlement an array of strings.",
                ));
                return Ok(());
            };
            for entitlement in domains {
                let Some(entitlement) = entitlement.as_string() else {
                    issues.push(issue(
                        "apple",
                        "error",
                        "invalid_domain",
                        "An associated domain entry is not a string.",
                        candidate,
                        None,
                        "Use applinks:example.com.",
                    ));
                    continue;
                };
                let Some((service, domain)) = entitlement.split_once(':') else {
                    issues.push(issue(
                        "apple",
                        "error",
                        "invalid_domain",
                        format!("{entitlement:?} has no service prefix."),
                        candidate,
                        None,
                        "Use applinks:example.com, webcredentials:example.com or appclips:example.com.",
                    ));
                    continue;
                };
                if !matches!(
                    service,
                    "applinks" | "webcredentials" | "appclips" | "activitycontinuation"
                ) || !valid_domain(domain)
                {
                    issues.push(issue(
                        "apple",
                        "error",
                        "invalid_domain",
                        format!("{entitlement:?} is not a valid associated domain."),
                        candidate,
                        None,
                        "Use applinks:example.com with a bare host.",
                    ));
                    continue;
                }
                let domain = domain.to_ascii_lowercase();
                let key = (candidate.to_owned(), service.to_owned(), domain.clone());
                if !links.entitlements.insert(key.clone()) {
                    issues.push(issue(
                        "apple",
                        "warning",
                        "duplicate_domain",
                        format!(
                            "Associated domain {:?} is declared more than once in {candidate}.",
                            format!("{service}:{domain}")
                        ),
                        candidate,
                        None,
                        "Declare each associated-domain entitlement once per file.",
                    ));
                }
                links.associated_domains.insert(entitlement.to_owned());
                links
                    .association_domains
                    .entry(domain.clone())
                    .or_insert_with(|| candidate.to_owned());
                links.domains.insert(domain);
            }
        }
    }
    Ok(())
}

fn validate_aasa(contents: &[u8], file: &str, issues: &mut Issues) {
    let root: Value = match serde_json::from_slice(contents) {
        Ok(value) => value,
        Err(error) => {
            issues.push(issue(
                "apple",
                "error",
                "malformed_association",
                format!("apple-app-site-association is not valid JSON: {error}."),
                file,
                None,
                "Serve the association as JSON.",
            ));
            return;
        }
    };
    let Some(object) = root.as_object() else {
        issues.push(issue(
            "apple",
            "error",
            "invalid_association",
            "apple-app-site-association must be a JSON object.",
            file,
            None,
            "Serve a JSON object with applinks or webcredentials.",
        ));
        return;
    };
    let known = [
        "applinks",
        "webcredentials",
        "activitycontinuation",
        "appclips",
    ];
    if !known
        .iter()
        .any(|key| object.get(*key).is_some_and(|value| !value.is_null()))
    {
        issues.push(issue(
            "apple",
            "error",
            "invalid_association",
            "apple-app-site-association has no applinks, webcredentials, activitycontinuation or appclips section.",
            file,
            None,
            "Add an applinks object with details or a legacy appID entry.",
        ));
        return;
    }
    let Some(applinks) = object.get("applinks") else {
        return;
    };
    let Some(applinks) = applinks.as_object() else {
        issues.push(issue(
            "apple",
            "error",
            "invalid_association",
            "apple-app-site-association has an applinks value that is not an object.",
            file,
            None,
            "Make applinks an object with details or a legacy appID entry.",
        ));
        return;
    };
    if let Some(details) = applinks.get("details") {
        let valid = details.as_array().is_some_and(|details| {
            !details.is_empty()
                && details.iter().all(|detail| {
                    detail["appID"].as_str().is_some_and(|id| !id.is_empty())
                        || detail["appIDs"]
                            .as_array()
                            .is_some_and(|ids| !ids.is_empty())
                })
        });
        if !valid {
            issues.push(issue(
                "apple",
                "error",
                "invalid_association",
                "applinks.details must contain entries with a non-empty appID or appIDs.",
                file,
                None,
                "Add one detail entry per signing team and bundle identifier.",
            ));
        }
    } else if applinks.is_empty() {
        issues.push(issue(
            "apple",
            "error",
            "invalid_association",
            "apple-app-site-association applinks object is empty.",
            file,
            None,
            "Add appID entries for the app.",
        ));
    }
}

/// Bounded scan of a platform directory for the two file names Apple reads.
/// The scan never descends into symlinked directories and never leaves `root`;
/// a matching symlinked file is returned so `read_file` can reject an escape.
fn discover_apple_files(root: &Path, base: &str, want_info: bool) -> Vec<String> {
    let mut found = Vec::new();
    let directory = root.join(relative(base));
    let Ok(metadata) = fs::symlink_metadata(&directory) else {
        return found;
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return found;
    }
    let Ok(canonical) = fs::canonicalize(&directory) else {
        return found;
    };
    if !canonical.starts_with(root) {
        return found;
    }
    let mut visited = 0usize;
    let mut stack = vec![(canonical, 0usize)];
    while let Some((directory, depth)) = stack.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            visited += 1;
            if visited > MAX_SCAN_ENTRIES {
                found.sort();
                found.dedup();
                return found;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let path = entry.path();
            let is_entitlements = !want_info && name.ends_with(".entitlements");
            let is_info = want_info && name == "Info.plist";
            if metadata.file_type().is_symlink() {
                if (is_entitlements || is_info)
                    && let Ok(relative) = path.strip_prefix(root)
                {
                    found.push(relative.to_string_lossy().replace('\\', "/"));
                }
                continue;
            }
            if metadata.is_dir() {
                if depth < MAX_SCAN_DEPTH
                    && !matches!(
                        name.as_str(),
                        "build" | "Pods" | "DerivedData" | "node_modules" | "target"
                    )
                {
                    stack.push((path, depth + 1));
                }
                continue;
            }
            if (is_entitlements || is_info)
                && let Ok(relative) = path.strip_prefix(root)
            {
                found.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

struct AppleReport {
    configured: bool,
    schemes: Vec<String>,
    domains: Vec<String>,
    associated_domains: Vec<String>,
}

fn collect_apple(
    root: &Path,
    issues: &mut Issues,
    files: &mut Vec<Value>,
) -> Result<AppleReport, Fault> {
    let mut links = AppleLinks::default();
    collect_plist(root, MACOS_INFO, "macos", &mut links, issues, files)?;
    for candidate in discover_apple_files(root, IOS_DIR, true) {
        collect_plist(root, &candidate, "ios", &mut links, issues, files)?;
    }
    for candidate in discover_apple_files(root, MACOS_DIR, false) {
        if candidate != MACOS_INFO {
            collect_entitlements(root, &candidate, "macos", &mut links, issues, files)?;
        }
    }
    for candidate in discover_apple_files(root, IOS_DIR, false) {
        collect_entitlements(root, &candidate, "ios", &mut links, issues, files)?;
    }

    let mut association_present = false;
    for candidate in APPLE_ASSOCIATIONS {
        match read_file(root, candidate)? {
            Read::Missing => {}
            Read::Content(bytes) => {
                association_present = true;
                files.push(file_entry(
                    root,
                    candidate,
                    "apple",
                    "apple_app_site_association",
                ));
                validate_aasa(&bytes, candidate, issues);
            }
            Read::Invalid { code, message } => issues.push(issue(
                "apple",
                "error",
                code,
                message,
                candidate,
                None,
                "Replace the association file with readable JSON.",
            )),
        }
    }
    if !association_present {
        for (domain, source) in &links.association_domains {
            issues.push(issue(
                "apple",
                "warning",
                "missing_association_file",
                format!(
                    "https://{domain}/.well-known/apple-app-site-association was not found in the project."
                ),
                source,
                None,
                &format!(
                    "Publish https://{domain}/.well-known/apple-app-site-association and keep a copy at {}.",
                    APPLE_ASSOCIATIONS[0]
                ),
            ));
        }
    }

    Ok(AppleReport {
        configured: !links.schemes.is_empty() || !links.domains.is_empty(),
        schemes: links.schemes.into_iter().collect(),
        domains: links.domains.into_iter().collect(),
        associated_domains: links.associated_domains.into_iter().collect(),
    })
}

fn summary_text(
    configured: bool,
    android_schemes: usize,
    android_hosts: usize,
    apple_schemes: usize,
    apple_domains: usize,
    errors: usize,
    warnings: usize,
) -> String {
    if !configured {
        let mut text = "No deep links configured.".to_owned();
        if errors + warnings > 0 {
            text.push_str(&format!(
                " {errors} error(s), {warnings} warning(s) in discovered configuration files."
            ));
        }
        return text;
    }
    let mut parts = Vec::new();
    if android_schemes > 0 || android_hosts > 0 {
        parts.push(format!(
            "Android: {android_schemes} scheme(s), {android_hosts} host(s)"
        ));
    }
    if apple_schemes > 0 || apple_domains > 0 {
        parts.push(format!(
            "Apple: {apple_schemes} scheme(s), {apple_domains} domain(s)"
        ));
    }
    format!(
        "{}; {errors} error(s), {warnings} warning(s).",
        parts.join("; ")
    )
}

/// Inspects a project's platform deep-link configuration.
pub(crate) fn inspect(root: &str) -> Result<Value, Fault> {
    let root = canonical_root(root)?;
    let mut issues = Issues::default();
    let mut files = Vec::new();

    let android = collect_android(&root, &mut issues, &mut files)?;
    let apple = collect_apple(&root, &mut issues, &mut files)?;

    let errors = issues.errors();
    let warnings = issues.warnings();
    files.sort_by(|left, right| left["path"].as_str().cmp(&right["path"].as_str()));
    files.dedup_by(|left, right| left["path"] == right["path"]);

    let configured = android.configured || apple.configured;
    let summary = json!({
        "configured": configured,
        "android": android.configured,
        "apple": apple.configured,
        "schemes": android.schemes.len() + apple.schemes.len(),
        "hosts": android.hosts.len(),
        "domains": apple.domains.len(),
        "issues": errors + warnings,
        "errors": errors,
        "warnings": warnings,
        "text": summary_text(
            configured,
            android.schemes.len(),
            android.hosts.len(),
            apple.schemes.len(),
            apple.domains.len(),
            errors,
            warnings,
        ),
    });
    Ok(json!({
        "root": root.display().to_string(),
        "files": files,
        "android": {
            "configured": android.configured,
            "schemes": android.schemes,
            "hosts": android.hosts,
            "links": android.links,
            "issues": issues.for_platform("android"),
        },
        "apple": {
            "configured": apple.configured,
            "schemes": apple.schemes,
            "domains": apple.domains,
            "associatedDomains": apple.associated_domains,
            "issues": issues.for_platform("apple"),
        },
        "summary": summary,
    }))
}
#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>
<manifest xmlns:android=\"http://schemas.android.com/apk/res/android\">
    <application>
        <activity android:name=\".MainActivity\" android:exported=\"true\">
            <intent-filter android:autoVerify=\"true\">
                <action android:name=\"android.intent.action.VIEW\" />
                <category android:name=\"android.intent.category.DEFAULT\" />
                <category android:name=\"android.intent.category.BROWSABLE\" />
                <data android:scheme=\"https\" android:host=\"links.example.com\" android:pathPrefix=\"/pets\" />
            </intent-filter>
        </activity>
    </application>
</manifest>
";

    const IOS_INFO: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">
<plist version=\"1.0\"><dict>
    <key>CFBundleURLTypes</key>
    <array><dict>
        <key>CFBundleURLName</key><string>com.example.gallery</string>
        <key>CFBundleURLSchemes</key><array><string>coveapp</string></array>
    </dict></array>
</dict></plist>
";

    const ENTITLEMENTS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">
<plist version=\"1.0\"><dict>
    <key>com.apple.developer.associated-domains</key>
    <array>
        <string>applinks:links.example.com</string>
        <string>webcredentials:links.example.com</string>
    </array>
</dict></plist>
";

    const ASSETLINKS: &str = r#"[{
        "relation": ["delegate_permission/common.handle_all_urls"],
        "target": {
            "namespace": "android_app",
            "package_name": "com.example.gallery",
            "sha256_cert_fingerprints": ["AA:BB:CC"]
        }
    }]"#;

    const AASA: &str = r#"{"applinks": {"details": [{"appID": "TEAM.com.example.gallery", "paths": ["/pets/*"]}]}}"#;

    fn write(root: &Path, relative: &str, contents: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn valid_project() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        write(root.path(), ANDROID_MANIFEST, MANIFEST);
        write(root.path(), ANDROID_ASSOCIATIONS[0], ASSETLINKS);
        write(root.path(), "Platforms/iOS/Runner/Info.plist", IOS_INFO);
        write(
            root.path(),
            "Platforms/iOS/Runner/Runner.entitlements",
            ENTITLEMENTS,
        );
        write(
            root.path(),
            "Platforms/iOS/.well-known/apple-app-site-association",
            AASA,
        );
        root
    }

    fn issue_codes(report: &Value, platform: &str) -> Vec<String> {
        report[platform]["issues"]
            .as_array()
            .unwrap()
            .iter()
            .map(|issue| issue["code"].as_str().unwrap().to_owned())
            .collect()
    }

    #[test]
    fn valid_android_and_apple_links_are_reported() {
        let project = valid_project();
        let report = inspect(project.path().to_str().unwrap()).unwrap();
        assert_eq!(
            report["root"],
            project.path().canonicalize().unwrap().display().to_string()
        );
        assert_eq!(report["android"]["configured"], true);
        assert_eq!(report["android"]["schemes"], json!(["https"]));
        assert_eq!(report["android"]["hosts"], json!(["links.example.com"]));
        assert_eq!(report["android"]["links"][0]["autoVerify"], true);
        assert_eq!(report["android"]["links"][0]["viewAction"], true);
        assert_eq!(report["android"]["links"][0]["browsable"], true);
        assert_eq!(report["android"]["links"][0]["line"], 5);
        assert_eq!(report["apple"]["configured"], true);
        assert_eq!(report["apple"]["schemes"], json!(["coveapp"]));
        assert_eq!(report["apple"]["domains"], json!(["links.example.com"]));
        assert!(
            report["apple"]["associatedDomains"]
                .as_array()
                .unwrap()
                .iter()
                .any(|domain| domain == "applinks:links.example.com")
        );
        assert_eq!(report["summary"]["configured"], true);
        assert_eq!(report["summary"]["errors"], 0);
        assert_eq!(report["summary"]["warnings"], 0, "{report}");
        assert!(
            report["summary"]["text"]
                .as_str()
                .unwrap()
                .contains("Android:")
        );
        assert!(
            report["summary"]["text"]
                .as_str()
                .unwrap()
                .contains("Apple:")
        );
        let files = report["files"].as_array().unwrap();
        assert!(files.iter().any(|file| {
            file["kind"] == "manifest"
                && file["path"]
                    .as_str()
                    .unwrap()
                    .ends_with("AndroidManifest.xml")
        }));
        assert!(files.iter().any(|file| file["kind"] == "info_plist"));
        assert!(files.iter().any(|file| file["kind"] == "entitlements"));
        assert!(files.iter().any(|file| file["kind"] == "assetlinks"));
        assert!(
            files
                .iter()
                .any(|file| file["kind"] == "apple_app_site_association")
        );
    }

    #[test]
    fn missing_browsable_view_and_auto_verify_are_reported_with_locations() {
        let project = tempfile::tempdir().unwrap();
        write(
            project.path(),
            ANDROID_MANIFEST,
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>
<manifest xmlns:android=\"http://schemas.android.com/apk/res/android\">
    <application>
        <activity android:name=\".MainActivity\">
            <intent-filter>
                <category android:name=\"android.intent.category.DEFAULT\" />
                <data android:scheme=\"https\" android:host=\"links.example.com\" />
            </intent-filter>
        </activity>
    </application>
</manifest>
",
        );
        let report = inspect(project.path().to_str().unwrap()).unwrap();
        let codes = issue_codes(&report, "android");
        assert!(codes.contains(&"missing_browsable".to_owned()), "{codes:?}");
        assert!(
            codes.contains(&"missing_view_action".to_owned()),
            "{codes:?}"
        );
        assert!(
            codes.contains(&"missing_auto_verify".to_owned()),
            "{codes:?}"
        );
        assert!(
            codes.contains(&"missing_association_file".to_owned()),
            "{codes:?}"
        );
        let issue = &report["android"]["issues"]
            .as_array()
            .unwrap()
            .iter()
            .find(|issue| issue["code"] == "missing_browsable")
            .unwrap();
        assert_eq!(issue["severity"], "error");
        assert_eq!(issue["line"], 5);
        assert!(
            issue["location"]
                .as_str()
                .unwrap()
                .ends_with("AndroidManifest.xml:5")
        );
        assert!(issue["fix"].as_str().unwrap().contains("BROWSABLE"));
    }

    #[test]
    fn scheme_without_host_is_reported() {
        let project = tempfile::tempdir().unwrap();
        write(
            project.path(),
            ANDROID_MANIFEST,
            "<manifest xmlns:android=\"http://schemas.android.com/apk/res/android\">
    <application><activity android:name=\".Main\">
        <intent-filter>
            <action android:name=\"android.intent.action.VIEW\" />
            <category android:name=\"android.intent.category.BROWSABLE\" />
            <data android:scheme=\"myapp\" />
        </intent-filter>
    </activity></application>
</manifest>",
        );
        let report = inspect(project.path().to_str().unwrap()).unwrap();
        assert_eq!(report["android"]["schemes"], json!(["myapp"]));
        let issue = report["android"]["issues"]
            .as_array()
            .unwrap()
            .iter()
            .find(|issue| issue["code"] == "scheme_without_host")
            .expect("scheme without host is reported");
        assert_eq!(issue["severity"], "warning");
    }

    #[test]
    fn duplicate_links_and_schemes_are_reported() {
        let project = tempfile::tempdir().unwrap();
        let filter = "<intent-filter android:autoVerify=\"true\">
            <action android:name=\"android.intent.action.VIEW\" />
            <category android:name=\"android.intent.category.BROWSABLE\" />
            <data android:scheme=\"https\" android:host=\"links.example.com\" />
        </intent-filter>";
        write(
            project.path(),
            ANDROID_MANIFEST,
            &format!(
                "<manifest xmlns:android=\"http://schemas.android.com/apk/res/android\">
    <application><activity android:name=\".Main\">{filter}{filter}</activity></application>
</manifest>"
            ),
        );
        write(project.path(), MACOS_INFO, IOS_INFO);
        let duplicate_schemes =
            IOS_INFO.replacen("</array>", "<string>coveapp</string></array>", 1);
        write(
            project.path(),
            "Platforms/iOS/Runner/Info.plist",
            &duplicate_schemes,
        );
        write(
            project.path(),
            "Platforms/iOS/Runner/Runner.entitlements",
            "<plist version=\"1.0\"><dict>
                <key>com.apple.developer.associated-domains</key>
                <array>
                    <string>applinks:links.example.com</string>
                    <string>applinks:links.example.com</string>
                </array>
            </dict></plist>",
        );
        write(project.path(), ANDROID_ASSOCIATIONS[0], ASSETLINKS);
        write(
            project.path(),
            "Platforms/iOS/.well-known/apple-app-site-association",
            AASA,
        );
        let report = inspect(project.path().to_str().unwrap()).unwrap();
        let android = issue_codes(&report, "android");
        assert!(
            android.contains(&"duplicate_link".to_owned()),
            "{android:?}"
        );
        let apple = issue_codes(&report, "apple");
        assert!(apple.contains(&"duplicate_scheme".to_owned()), "{apple:?}");
        assert!(apple.contains(&"duplicate_domain".to_owned()), "{apple:?}");
    }

    #[test]
    fn malformed_manifest_and_plist_are_issues_not_errors() {
        let project = tempfile::tempdir().unwrap();
        write(
            project.path(),
            ANDROID_MANIFEST,
            "<manifest><application><activity><intent-filter></application>",
        );
        write(
            project.path(),
            MACOS_INFO,
            "<plist version=\"1.0\"><dict><key>CFBundleURLTypes</key></plist>",
        );
        let report = inspect(project.path().to_str().unwrap()).unwrap();
        let android = issue_codes(&report, "android");
        assert_eq!(android, vec!["malformed_manifest"]);
        let manifest = &report["android"]["issues"][0];
        assert_eq!(manifest["severity"], "error");
        assert!(manifest["line"].as_u64().unwrap() >= 1);
        assert_eq!(report["android"]["configured"], false);
        let apple = issue_codes(&report, "apple");
        assert_eq!(apple, vec!["malformed_plist"]);
        assert_eq!(report["apple"]["issues"][0]["severity"], "error");
        assert_eq!(report["apple"]["configured"], false);
        assert_eq!(report["summary"]["configured"], false);
    }

    #[test]
    fn malformed_association_files_are_reported() {
        let project = tempfile::tempdir().unwrap();
        write(project.path(), ANDROID_MANIFEST, MANIFEST);
        write(project.path(), ANDROID_ASSOCIATIONS[0], "{not json");
        write(
            project.path(),
            "Platforms/iOS/.well-known/apple-app-site-association",
            "[]",
        );
        let report = inspect(project.path().to_str().unwrap()).unwrap();
        let android = issue_codes(&report, "android");
        assert!(
            android.contains(&"malformed_association".to_owned()),
            "{android:?}"
        );
        let apple = issue_codes(&report, "apple");
        assert!(
            apple.contains(&"invalid_association".to_owned()),
            "{apple:?}"
        );
    }

    #[test]
    fn missing_platform_files_are_not_configured() {
        let project = tempfile::tempdir().unwrap();
        let report = inspect(project.path().to_str().unwrap()).unwrap();
        assert_eq!(report["android"]["configured"], false);
        assert_eq!(report["apple"]["configured"], false);
        assert!(report["files"].as_array().unwrap().is_empty());
        assert!(report["android"]["issues"].as_array().unwrap().is_empty());
        assert!(report["apple"]["issues"].as_array().unwrap().is_empty());
        assert_eq!(report["summary"]["configured"], false);
        assert_eq!(report["summary"]["issues"], 0);
        assert!(
            report["summary"]["text"]
                .as_str()
                .unwrap()
                .contains("No deep links configured")
        );
    }

    #[test]
    fn ios_layout_is_discovered_without_an_exact_candidate() {
        let project = tempfile::tempdir().unwrap();
        write(project.path(), "Platforms/iOS/Runner/Info.plist", IOS_INFO);
        write(
            project.path(),
            "Platforms/iOS/Runner/Runner.entitlements",
            ENTITLEMENTS,
        );
        write(
            project.path(),
            "Platforms/iOS/.well-known/apple-app-site-association",
            AASA,
        );
        let report = inspect(project.path().to_str().unwrap()).unwrap();
        assert_eq!(report["apple"]["schemes"], json!(["coveapp"]));
        assert_eq!(report["apple"]["domains"], json!(["links.example.com"]));
        assert_eq!(report["apple"]["configured"], true);
    }

    #[test]
    fn non_absolute_and_parent_roots_are_rejected() {
        let error = inspect("examples/gallery").unwrap_err();
        assert_eq!(error.code, "invalid_request");
        assert!(error.message.contains("absolute"), "{}", error.message);
        let project = tempfile::tempdir().unwrap();
        let parent = format!("{}/..", project.path().display());
        let error = inspect(&parent).unwrap_err();
        assert_eq!(error.code, "invalid_request");
        assert!(error.message.contains("'..'"), "{}", error.message);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escape_is_rejected_for_known_files() {
        use std::os::unix::fs::symlink;

        let project = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        write(outside.path(), "AndroidManifest.xml", MANIFEST);
        let manifest = project.path().join(ANDROID_MANIFEST);
        fs::create_dir_all(manifest.parent().unwrap()).unwrap();
        symlink(outside.path().join("AndroidManifest.xml"), &manifest).unwrap();
        let error = inspect(project.path().to_str().unwrap()).unwrap_err();
        assert_eq!(error.code, "invalid_request");
        assert!(error.message.contains("outside"), "{}", error.message);

        let project = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        write(outside.path(), "assetlinks.json", ASSETLINKS);
        write(project.path(), ANDROID_MANIFEST, MANIFEST);
        let association = project.path().join(ANDROID_ASSOCIATIONS[0]);
        fs::create_dir_all(association.parent().unwrap()).unwrap();
        symlink(outside.path().join("assetlinks.json"), &association).unwrap();
        let error = inspect(project.path().to_str().unwrap()).unwrap_err();
        assert_eq!(error.code, "invalid_request");
        assert!(error.message.contains("outside"), "{}", error.message);

        let project = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        write(outside.path(), "Info.plist", IOS_INFO);
        let info = project.path().join("Platforms/iOS/Runner/Info.plist");
        fs::create_dir_all(info.parent().unwrap()).unwrap();
        symlink(outside.path().join("Info.plist"), &info).unwrap();
        let error = inspect(project.path().to_str().unwrap()).unwrap_err();
        assert_eq!(error.code, "invalid_request");
        assert!(error.message.contains("outside"), "{}", error.message);
    }

    #[test]
    fn the_same_scheme_and_domain_across_macos_and_ios_are_not_duplicates() {
        let project = tempfile::tempdir().unwrap();
        write(project.path(), MACOS_INFO, IOS_INFO);
        write(project.path(), "Platforms/iOS/Runner/Info.plist", IOS_INFO);
        write(
            project.path(),
            "Platforms/macOS/Entitlements.plist",
            ENTITLEMENTS,
        );
        write(
            project.path(),
            "Platforms/iOS/Runner/Runner.entitlements",
            ENTITLEMENTS,
        );
        write(
            project.path(),
            "Platforms/iOS/.well-known/apple-app-site-association",
            AASA,
        );
        let report = inspect(project.path().to_str().unwrap()).unwrap();
        let apple = issue_codes(&report, "apple");
        assert!(!apple.contains(&"duplicate_scheme".to_owned()), "{apple:?}");
        assert!(!apple.contains(&"duplicate_domain".to_owned()), "{apple:?}");
        assert_eq!(report["apple"]["schemes"], json!(["coveapp"]));
        assert_eq!(report["apple"]["domains"], json!(["links.example.com"]));
    }

    #[test]
    fn webcredentials_without_an_association_file_is_reported() {
        let project = tempfile::tempdir().unwrap();
        write(
            project.path(),
            "Platforms/iOS/Runner/Runner.entitlements",
            "<plist version=\"1.0\"><dict>
                <key>com.apple.developer.associated-domains</key>
                <array><string>webcredentials:login.example.com</string></array>
            </dict></plist>",
        );
        let report = inspect(project.path().to_str().unwrap()).unwrap();
        assert_eq!(report["apple"]["domains"], json!(["login.example.com"]));
        assert!(
            issue_codes(&report, "apple").contains(&"missing_association_file".to_owned()),
            "{report}"
        );
    }

    #[test]
    fn a_root_that_is_a_file_is_rejected() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let error = inspect(file.path().to_str().unwrap()).unwrap_err();
        assert_eq!(error.code, "invalid_request");
        assert!(error.message.contains("directory"), "{}", error.message);
    }
}
