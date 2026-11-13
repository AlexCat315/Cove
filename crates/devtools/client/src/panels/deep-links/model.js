/* Deep Links
 *
 * The controller validates a project's platform configuration on disk; the
 * client never reads files or infers links. `get_ui_deep_links {root}` returns
 * the discovered project files, Android schemes/hosts/issues, Apple
 * schemes/domains/issues and a summary. Only the absolute root is sent, so the
 * controller can reject traversal, symlink escapes and unknown files.
 */

function deepLinksAbsoluteRoot(root) {
  return root.startsWith("/") || /^[A-Za-z]:[\\/]/.test(root) || root.startsWith("\\\\");
}

function deepLinksRootError(root) {
  if (!root) return "Enter an absolute project root.";
  if (!deepLinksAbsoluteRoot(root)) return "The project root must be an absolute path. Relative paths and traversal are not sent to the controller.";
  if (root.split(/[\\/]+/).includes("..")) return "The project root must not contain .. path segments.";
  return "";
}

function deepLinksFirstArray(...values) {
  for (const value of values) {
    if (Array.isArray(value)) return value;
  }
  return [];
}

function deepLinksObject(...values) {
  for (const value of values) {
    if (isObjectValue(value)) return value;
  }
  return null;
}

function deepLinksFirstNumber(...values) {
  for (const value of values) {
    const number = numberValue(value);
    if (number !== null) return number;
  }
  return null;
}

function deepLinksRoot(data) {
  if (!isObjectValue(data)) return null;
  return isObjectValue(data.data) ? data.data : data;
}

function deepLinksUnavailableReason(root) {
  const unavailable = root.available === false
    || root.supported === false
    || root.unavailable === true
    || root.ok === false
    || root.status === "unavailable";
  if (!unavailable) return "";
  return firstStringValue(
    root.reason,
    root.unavailable_reason,
    root.unavailableReason,
    root.message,
    root.detail,
    typeof root.error === "string" ? root.error : "",
    root.error?.message,
    root.error?.reason,
  ) || "The controller reported no deep links validation for this root.";
}

function deepLinksPlatform(value) {
  const platform = String(value || "").toLowerCase();
  if (platform === "android") return "android";
  if (["ios", "ipados", "macos", "apple", "tvos", "watchos"].includes(platform)) return "apple";
  return platform;
}

function deepLinksPlatformLabel(platform) {
  if (platform === "android") return "Android";
  if (platform === "apple") return "Apple";
  return platform || "Project";
}

function deepLinksKindLabel(kind) {
  return {
    manifest: "Android manifest",
    android_manifest: "Android manifest",
    info_plist: "Info.plist",
    plist: "Info.plist",
    entitlements: "Entitlements",
    assetlinks: "Asset links",
    asset_links: "Asset links",
    assetlinks_json: "Asset links",
    associated_domains: "Associated domains",
    apple_app_site_association: "App site association",
    aasa: "App site association",
  }[String(kind || "").toLowerCase()] || kind || "";
}

function deepLinksFileStatus(status) {
  const raw = String(status || "").toLowerCase();
  if (["missing", "absent", "not_configured", "not-configured", "notconfigured", "none"].includes(raw)) {
    return { label: "Not configured", tone: "muted" };
  }
  if (["found", "present", "exists", "ok", "valid"].includes(raw)) return { label: "Found", tone: "good" };
  if (["invalid", "unreadable", "unparseable", "parse_error", "error"].includes(raw)) {
    return { label: raw === "unreadable" ? "Unreadable" : "Invalid", tone: "bad" };
  }
  if (raw) return { label: status, tone: "" };
  return { label: "—", tone: "" };
}

function deepLinksFileName(entry) {
  if (typeof entry === "string") return { path: entry, platform: "", kind: "", status: "", detail: "" };
  if (!isObjectValue(entry)) return null;
  const location = isObjectValue(entry.location) ? entry.location : {};
  const path = firstStringValue(entry.path, entry.file, entry.filePath, entry.file_path, entry.filename, entry.name, typeof entry.location === "string" ? entry.location : "", location.path, location.file);
  const platform = deepLinksPlatform(firstStringValue(entry.platform, entry.os, entry.target));
  const kind = firstStringValue(entry.kind, entry.type, entry.fileKind, entry.file_kind, entry.role);
  const status = firstStringValue(entry.status, entry.state, entry.presence, entry.result);
  const detail = firstStringValue(entry.detail, entry.message, entry.reason, entry.error, typeof entry.error === "string" ? entry.error : "");
  if (!path && !kind) return null;
  return { path, platform, kind, status, detail };
}

function deepLinksFiles(root, android, apple) {
  const entries = [
    ...deepLinksFirstArray(root.files, root.discoveredFiles, root.discovered_files, root.sources, root.checkedFiles, root.checked_files),
    ...deepLinksFirstArray(android?.files, android?.discoveredFiles, android?.discovered_files, android?.sources),
    ...deepLinksFirstArray(apple?.files, apple?.discoveredFiles, apple?.discovered_files, apple?.sources),
  ];
  const files = entries.map(deepLinksFileName).filter(Boolean);
  const nestedPaths = [
    ["android", "manifest", firstStringValue(android?.manifest, android?.manifestPath, android?.manifest_path)]
      .concat(typeof android?.manifest === "object" ? [android.manifest.path] : []),
    ["apple", "info_plist", firstStringValue(apple?.infoPlist, apple?.info_plist, apple?.plist, apple?.plistPath, apple?.plist_path)]
      .concat(typeof apple?.infoPlist === "object" ? [apple.infoPlist.path] : []),
    ["apple", "entitlements", firstStringValue(apple?.entitlements, apple?.entitlementsPath, apple?.entitlements_path)]
      .concat(typeof apple?.entitlements === "object" ? [apple.entitlements.path] : []),
  ];
  for (const [platform, kind, path] of nestedPaths) {
    if (!path) continue;
    if (files.some(file => file.path === path)) continue;
    files.push({ path, platform, kind, status: "", detail: "" });
  }
  const seen = new Set();
  return files.filter(file => {
    const key = `${file.path}\u0000${file.kind}\u0000${file.platform}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function deepLinksStringEntries(value, ...keys) {
  if (!Array.isArray(value)) return [];
  const entries = [];
  for (const entry of value) {
    if (typeof entry === "string" && entry.trim()) {
      entries.push({ name: entry.trim(), detail: "" });
      continue;
    }
    if (!isObjectValue(entry)) continue;
    const name = firstStringValue(...keys.map(key => entry[key]), entry.name, entry.value, entry.id);
    if (!name) continue;
    const detail = firstStringValue(entry.scheme, entry.schemeName, entry.scheme_name, entry.path, entry.pathPrefix, entry.path_prefix);
    entries.push({ name, detail: detail && detail !== name ? detail : "" });
  }
  return entries;
}

function deepLinksSchemes(source, ...names) {
  for (const name of names) {
    if (Array.isArray(source?.[name])) {
      return deepLinksStringEntries(source[name], "scheme", "scheme_name", "name");
    }
  }
  return [];
}

function deepLinksDomains(source, ...names) {
  for (const name of names) {
    if (Array.isArray(source?.[name])) {
      return deepLinksStringEntries(source[name], "domain", "host", "domain_name", "name");
    }
  }
  return [];
}

function deepLinksSeverity(value) {
  const raw = String(value || "").toLowerCase();
  if (["error", "fatal", "severe"].includes(raw)) return "error";
  if (["warning", "warn"].includes(raw)) return "warning";
  if (["note", "info", "information", "notice"].includes(raw)) return "note";
  return raw;
}

function deepLinksAssociatedDomains(entries) {
  return entries.map(entry => {
    const value = String(entry?.name ?? entry ?? "");
    const separator = value.indexOf(":");
    if (separator <= 0) return { name: value, detail: "—" };
    return { name: value.slice(separator + 1), detail: value.slice(0, separator) };
  }).filter(entry => entry.name);
}

function deepLinksIntentFilter(entry) {
  if (!isObjectValue(entry)) return null;
  const stringList = value => Array.isArray(value)
    ? value.map(item => typeof item === "string" ? item : firstStringValue(item?.name, item?.value)).filter(Boolean)
    : [];
  const activity = firstStringValue(entry.activity, entry.activityName, entry.activity_name, entry.name);
  const schemes = stringList(entry.schemes);
  const hosts = stringList(entry.hosts);
  const paths = stringList(entry.paths);
  const autoVerify = typeof entry.autoVerify === "boolean" ? entry.autoVerify : typeof entry.auto_verify === "boolean" ? entry.auto_verify : null;
  const file = firstStringValue(entry.file, entry.path);
  const line = deepLinksFirstNumber(entry.line, entry.lineNumber, entry.line_number);
  if (!activity && !schemes.length && !hosts.length && !paths.length) return null;
  return { activity, schemes, hosts, paths, autoVerify, file, line };
}

function deepLinksIssue(entry, platform) {
  if (typeof entry === "string") {
    const message = entry.trim();
    if (!message) return null;
    return { severity: "", message, path: "", line: null, column: null, code: "", platform };
  }
  if (!isObjectValue(entry)) return null;
  const location = isObjectValue(entry.location) ? entry.location : isObjectValue(entry.position) ? entry.position : {};
  const range = isObjectValue(entry.range) ? entry.range : {};
  const start = isObjectValue(range.start) ? range.start : {};
  const message = firstStringValue(entry.message, entry.detail, entry.description, entry.title, entry.reason, entry.text);
  const code = firstStringValue(entry.code, entry.rule, entry.id, entry.kind);
  if (!message && !code) return null;
  return {
    severity: deepLinksSeverity(firstStringValue(entry.severity, entry.level, entry.priority)),
    message: message || code,
    path: firstStringValue(entry.path, entry.file, entry.filePath, entry.file_path, entry.source, location.path, location.file),
    location: firstStringValue(typeof entry.location === "string" ? entry.location : "", location.label),
    line: deepLinksFirstNumber(entry.line, entry.lineNumber, entry.line_number, location.line, location.lineNumber, start.line),
    column: deepLinksFirstNumber(entry.column, entry.col, location.column, start.column),
    code: message ? code : "",
    fix: firstStringValue(entry.fix, entry.suggestion, entry.remediation, entry.hint),
    platform,
  };
}

function deepLinksIssues(source, platform) {
  if (!source) return [];
  return [
    ...deepLinksFirstArray(source.issues, source.errors, source.warnings, source.problems, source.diagnostics),
  ].map(entry => deepLinksIssue(entry, platform)).filter(Boolean);
}

function deepLinksSummary(root, files, android, apple, issues) {
  const summary = deepLinksObject(root.summary, root.totals, root.counts) || {};
  const androidSummary = deepLinksObject(summary.android, summary.android_links, summary.androidLinks) || {};
  const appleSummary = deepLinksObject(summary.apple, summary.apple_links, summary.appleLinks, summary.ios) || {};
  const errors = issues.filter(issue => issue.severity === "error").length;
  const warnings = issues.filter(issue => issue.severity === "warning").length;
  return {
    files: deepLinksFirstNumber(summary.files, summary.fileCount, summary.file_count, root.fileCount, root.file_count) ?? files.length,
    androidSchemes: deepLinksFirstNumber(summary.androidSchemes, summary.android_schemes, androidSummary.schemes, android.schemesCount, android.schemes_count) ?? android.schemes.length,
    androidHosts: deepLinksFirstNumber(summary.androidHosts, summary.android_hosts, androidSummary.hosts, android.hostsCount, android.hosts_count) ?? android.hosts.length,
    appleSchemes: deepLinksFirstNumber(summary.appleSchemes, summary.apple_schemes, summary.iosSchemes, summary.ios_schemes, appleSummary.schemes, apple.schemesCount, apple.schemes_count) ?? apple.schemes.length,
    appleDomains: deepLinksFirstNumber(summary.appleDomains, summary.apple_domains, summary.iosDomains, summary.ios_domains, appleSummary.domains, apple.domainsCount, apple.domains_count) ?? apple.domains.length,
    issues: deepLinksFirstNumber(summary.issues, summary.issueCount, summary.issue_count) ?? issues.length,
    errors: deepLinksFirstNumber(summary.errors, summary.errorCount, summary.error_count, androidSummary.errors, appleSummary.errors) ?? errors,
    warnings: deepLinksFirstNumber(summary.warnings, summary.warningCount, summary.warning_count, androidSummary.warnings, appleSummary.warnings) ?? warnings,
    text: firstStringValue(summary.text, summary.summary, summary.detail),
  };
}

function deepLinksConfigured(root, android, apple) {
  const summary = deepLinksObject(root.summary, root.totals, root.counts);
  for (const value of [root.configured, root.deepLinksConfigured, root.deep_links_configured, root.hasDeepLinks, root.has_deep_links, summary?.configured, summary?.deep_links_configured]) {
    if (typeof value === "boolean") return value;
  }
  const status = String(firstStringValue(root.status, root.state, root.result)).toLowerCase();
  if (["no_deep_links", "no-deep-links", "none", "not_configured", "not-configured", "unconfigured", "empty"].includes(status)) return false;
  if (["configured", "deep_links_configured", "deep-links-configured"].includes(status)) return true;
  return android.schemes.length > 0 || android.hosts.length > 0 || apple.schemes.length > 0 || apple.domains.length > 0;
}

function normalizeDeepLinks(data) {
  const root = deepLinksRoot(data);
  if (!root) return null;
  const reason = deepLinksUnavailableReason(root);
  if (reason) return { unavailable: true, reason };
  const projectRoot = firstStringValue(root.root, root.projectRoot, root.project_root, root.project, root.path);
  const androidSource = deepLinksObject(root.android, root.android_project, root.androidProject);
  const appleSource = deepLinksObject(root.apple, root.ios, root.apple_project, root.appleProject);
  const issuesSource = deepLinksFirstArray(root.issues, root.problems, root.diagnostics, root.errors, root.warnings);
  const hasShape = Boolean(androidSource || appleSource || Array.isArray(root.files) || Array.isArray(root.discovered_files)
    || Array.isArray(root.discoveredFiles) || isObjectValue(root.summary) || issuesSource.length
    || typeof root.configured === "boolean" || root.status !== undefined);
  if (!hasShape) return null;
  const android = {
    manifest: firstStringValue(
      typeof androidSource?.manifest === "string" ? androidSource.manifest : "",
      androidSource?.manifestPath,
      androidSource?.manifest_path,
      isObjectValue(androidSource?.manifest) ? androidSource.manifest.path : "",
    ),
    schemes: deepLinksSchemes(androidSource, "schemes", "urlSchemes", "url_schemes", "url_schemes"),
    hosts: deepLinksDomains(androidSource, "hosts", "domains", "deepLinkHosts", "deep_link_hosts"),
    links: deepLinksFirstArray(androidSource?.links, androidSource?.intentFilters, androidSource?.intent_filters)
      .map(deepLinksIntentFilter).filter(Boolean),
    configured: typeof androidSource?.configured === "boolean" ? androidSource.configured : null,
    applicationId: firstStringValue(androidSource?.applicationId, androidSource?.application_id, androidSource?.package),
  };
  const apple = {
    infoPlist: firstStringValue(
      typeof appleSource?.infoPlist === "string" ? appleSource.infoPlist : "",
      typeof appleSource?.info_plist === "string" ? appleSource.info_plist : "",
      typeof appleSource?.plist === "string" ? appleSource.plist : "",
      isObjectValue(appleSource?.infoPlist) ? appleSource.infoPlist.path : "",
      isObjectValue(appleSource?.info_plist) ? appleSource.info_plist.path : "",
    ),
    entitlements: firstStringValue(
      typeof appleSource?.entitlements === "string" ? appleSource.entitlements : "",
      appleSource?.entitlementsPath,
      appleSource?.entitlements_path,
      isObjectValue(appleSource?.entitlements) ? appleSource.entitlements.path : "",
    ),
    schemes: deepLinksSchemes(appleSource, "schemes", "urlSchemes", "url_schemes", "url_types", "urlTypes", "cfBundleURLSchemes"),
    domains: deepLinksDomains(appleSource, "domains", "associatedDomains", "associated_domains", "associatedDomainsList", "associated_domains_list"),
    associatedDomains: deepLinksAssociatedDomains(deepLinksStringEntries(
      deepLinksFirstArray(appleSource?.associatedDomains, appleSource?.associated_domains),
      "domain",
      "name",
    )),
    configured: typeof appleSource?.configured === "boolean" ? appleSource.configured : null,
    bundleId: firstStringValue(appleSource?.bundleId, appleSource?.bundle_id, appleSource?.bundleIdentifier, appleSource?.bundle_identifier),
  };
  const files = deepLinksFiles(root, androidSource, appleSource);
  const discoveredPath = (platform, kind) => files.find(file => (!platform || file.platform === platform) && file.kind === kind)?.path || "";
  if (!android.manifest) android.manifest = discoveredPath("android", "manifest");
  if (!apple.infoPlist) apple.infoPlist = discoveredPath("apple", "info_plist");
  if (!apple.entitlements) apple.entitlements = discoveredPath("apple", "entitlements");
  const androidIssues = deepLinksIssues(androidSource, "android");
  const appleIssues = deepLinksIssues(appleSource, "apple");
  const globalIssues = issuesSource.map(entry => deepLinksIssue(entry, "")).filter(Boolean);
  const seenIssues = new Set();
  const issues = [...androidIssues, ...appleIssues, ...globalIssues].filter(issue => {
    const key = [issue.platform, issue.path, issue.line, issue.column, issue.severity, issue.code, issue.message].join("\u0000");
    if (seenIssues.has(key)) return false;
    seenIssues.add(key);
    return true;
  });
  const summary = deepLinksSummary(root, files, android, apple, issues);
  const configured = deepLinksConfigured(root, android, apple);
  const displayPath = candidate => candidate && projectRoot && !deepLinksAbsoluteRoot(candidate)
    ? `${projectRoot.replace(/[\\/]+$/, "")}/${candidate.replace(/^[\\/]+/, "")}`
    : candidate;
  for (const issue of issues) issue.path = displayPath(issue.path);
  for (const link of android.links) link.file = displayPath(link.file);
  return {
    root: projectRoot,
    status: firstStringValue(root.status, root.state, root.result),
    statusMessage: firstStringValue(root.message, root.detail, root.reason, summary.text),
    configured,
    files,
    android: { ...android, issues: androidIssues },
    apple: { ...apple, issues: appleIssues },
    otherIssues: globalIssues,
    summary,
    warnings: deepLinksFirstArray(root.warnings).map(value => typeof value === "string" ? value : firstStringValue(value?.message, value?.detail)).filter(Boolean),
  };
}

function deepLinksPlural(count, singular, plural = `${singular}s`) {
  return `${formatCount(count)} ${count === 1 ? singular : plural}`;
}

function deepLinksPanel(title, meta, ...children) {
  return el("div", { class: "panel-block" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: title }),
      meta ? el("span", { class: "pane-meta", text: meta }) : null,
    ),
    ...children,
  );
}

function deepLinksTable(headers, rows) {
  return el("div", { class: "table-wrap" }, el("table", { class: "data-table" },
    el("thead", {}, el("tr", {}, ...headers.map(header => el("th", { class: header.num ? "num" : "", text: header.label })))),
    el("tbody", {}, ...rows),
  ));
}

function deepLinksEmptyBody(message) {
  return el("div", { class: "panel-block-body" }, el("p", { class: "dl-note", text: message }));
}

function deepLinksStat(label, value, detail = "") {
  return el("div", { class: "stat" },
    el("div", { class: "stat-label", text: label }),
    el("div", { class: "stat-value" }, formatCount(value), detail ? el("small", { text: ` ${detail}` }) : null),
  );
}

function deepLinksLocation(issue) {
  const path = issue.path || issue.file;
  if (path) {
    let location = path;
    if (issue.line !== undefined && issue.line !== null) location += `:${issue.line}`;
    if (issue.column !== undefined && issue.column !== null) location += `:${issue.column}`;
    return location;
  }
  if (issue.location) return issue.location;
  if (issue.line !== undefined && issue.line !== null) return `line ${issue.line}${issue.column !== undefined && issue.column !== null ? `:${issue.column}` : ""}`;
  return "—";
}

