import { clear, el, icon } from "../../core/format.js";
import { MAX_DEEP_LINKS_ENTRIES, MAX_DEEP_LINKS_FILES, MAX_DEEP_LINKS_ISSUES, RpcError, dom, state } from "../../core/globals.js";
import { errorMessage, rpc, toast, unwrapData } from "../../core/session.js";
import { errorInline, showState } from "../../core/ui-helpers.js";
import { deepLinksEmptyBody, deepLinksFileStatus, deepLinksKindLabel, deepLinksLocation, deepLinksPanel, deepLinksPlatformLabel, deepLinksPlural, deepLinksRootError, deepLinksStat, deepLinksTable, normalizeDeepLinks } from "./model.js";
import { formatWallTime } from "../memory/snapshot.js";
import { writeDeepLinksRootPreference } from "../performance/overview.js";
import { formatCount } from "../performance/profile/model.js";

function deepLinksIssueRows(issues, limit) {
  return issues.slice(0, limit).map(issue => el("tr", { class: `dl-issue-row is-${issue.severity || "neutral"}` },
    el("td", {}, el("span", { class: `dl-severity is-${issue.severity || "neutral"}`, text: issue.severity || "issue" })),
    el("td", { class: "dl-issue-message", text: issue.message }),
    el("td", { class: "dl-location", text: deepLinksLocation(issue) }),
    el("td", { class: "dl-code", text: issue.code || "—" }),
    el("td", { class: "dl-fix", text: issue.fix || "—" }),
  ));
}

function deepLinksIssuesBody(issues, limit) {
  if (!issues.length) return deepLinksEmptyBody("No issues reported.");
  const body = deepLinksTable([
    { label: "Severity" },
    { label: "Issue" },
    { label: "Location" },
    { label: "Code" },
    { label: "Fix" },
  ], deepLinksIssueRows(issues, limit));
  if (issues.length <= limit) return body;
  return el("div", {},
    body,
    el("div", { class: "panel-block-body" }, el("p", { class: "dl-note", text: `Showing ${formatCount(limit)} of ${formatCount(issues.length)} reported issues.` })),
  );
}

function deepLinksEntryTable(headers, entries) {
  const rows = entries.slice(0, MAX_DEEP_LINKS_ENTRIES).map(entry => el("tr", {},
    el("td", { class: "dl-entry", text: entry.name }),
    headers.length > 1 ? el("td", { class: "dl-entry-detail", text: entry.detail || "—" }) : null,
  ));
  const body = deepLinksTable(headers, rows);
  if (entries.length <= MAX_DEEP_LINKS_ENTRIES) return body;
  return el("div", {},
    body,
    el("div", { class: "panel-block-body" }, el("p", { class: "dl-note", text: `Showing ${formatCount(MAX_DEEP_LINKS_ENTRIES)} of ${formatCount(entries.length)} reported entries.` })),
  );
}

function deepLinksSchemesBlock(label, entries) {
  if (!entries.length) return deepLinksEmptyBody(`No ${label} reported.`);
  return deepLinksEntryTable([{ label: "Scheme" }], entries);
}

function deepLinksDomainsBlock(label, entries) {
  if (!entries.length) return deepLinksEmptyBody(`No ${label} reported.`);
  return deepLinksEntryTable([{ label: "Domain" }], entries);
}

function deepLinksHostsBlock(label, entries) {
  if (!entries.length) return deepLinksEmptyBody(`No ${label} reported.`);
  return deepLinksEntryTable([{ label: "Host" }], entries);
}

function deepLinksIntentFiltersBody(links) {
  if (!links.length) return null;
  const rows = links.slice(0, MAX_DEEP_LINKS_ENTRIES).map(link => el("tr", {},
    el("td", { class: "dl-entry", text: link.activity || "—" }),
    el("td", { class: "dl-entry", text: link.schemes.join(", ") || "—" }),
    el("td", { class: "dl-entry", text: link.hosts.join(", ") || "—" }),
    el("td", { class: "dl-entry", text: link.paths.join(", ") || "—" }),
    el("td", { text: link.autoVerify === null ? "—" : link.autoVerify ? "Yes" : "No" }),
    el("td", { class: "dl-location", text: deepLinksLocation(link) }),
  ));
  const body = deepLinksTable([
    { label: "Activity" },
    { label: "Schemes" },
    { label: "Hosts" },
    { label: "Paths" },
    { label: "Auto verify" },
    { label: "Location" },
  ], rows);
  if (links.length <= MAX_DEEP_LINKS_ENTRIES) return body;
  return el("div", {},
    body,
    el("div", { class: "panel-block-body" }, el("p", { class: "dl-note", text: `Showing ${formatCount(MAX_DEEP_LINKS_ENTRIES)} of ${formatCount(links.length)} reported intent filters.` })),
  );
}

function deepLinksAssociatedBody(entries) {
  if (!entries.length) return deepLinksEmptyBody("No associated domains reported.");
  return deepLinksEntryTable([{ label: "Domain" }, { label: "Service" }], entries);
}

function deepLinksFilesPanel(files) {
  const block = deepLinksPanel("Discovered files", `${formatCount(files.length)} reported`);
  if (!files.length) {
    block.append(deepLinksEmptyBody("No platform files were reported for this root."));
    return block;
  }
  const rows = files.slice(0, MAX_DEEP_LINKS_FILES).map(file => {
    const status = deepLinksFileStatus(file.status || "found");
    return el("tr", { class: "dl-file-row" },
      el("td", { class: "dl-location", text: file.path || "—", attrs: file.path ? { title: file.path } : {} }),
      el("td", { text: deepLinksPlatformLabel(file.platform) }),
      el("td", { text: deepLinksKindLabel(file.kind) || "—" }),
      el("td", {}, el("span", { class: `dl-file-status is-${status.tone || "neutral"}`, text: status.label })),
      el("td", { class: "dl-file-detail", text: file.detail || "—" }),
    );
  });
  block.append(deepLinksTable([
    { label: "Path" },
    { label: "Platform" },
    { label: "Kind" },
    { label: "Status" },
    { label: "Detail" },
  ], rows));
  if (files.length > MAX_DEEP_LINKS_FILES) {
    block.append(el("div", { class: "panel-block-body" }, el("p", { class: "dl-note", text: `Showing ${formatCount(MAX_DEEP_LINKS_FILES)} of ${formatCount(files.length)} discovered files.` })));
  }
  return block;
}

function deepLinksAndroidPanel(android) {
  const configured = android.configured === false ? "Not configured · " : "";
  const block = deepLinksPanel("Android", `${configured}${deepLinksPlural(android.schemes.length, "scheme")} · ${deepLinksPlural(android.hosts.length, "host")}`);
  if (android.manifest || android.applicationId) {
    block.append(el("div", { class: "panel-block-body dl-source" },
      el("span", { text: "Manifest" }),
      el("code", { text: android.manifest || "not reported" }),
      android.applicationId ? el("span", { class: "dl-source-meta", text: `applicationId ${android.applicationId}` }) : null,
    ));
  }
  block.append(el("div", { class: "dl-subhead" }, el("h3", { text: "Schemes" })));
  block.append(deepLinksSchemesBlock("Android schemes", android.schemes));
  block.append(el("div", { class: "dl-subhead" }, el("h3", { text: "Hosts" })));
  block.append(deepLinksHostsBlock("Android hosts", android.hosts));
  if (android.links.length) {
    block.append(el("div", { class: "dl-subhead" }, el("h3", { text: "Intent filters" }), el("span", { class: "pane-meta", text: formatCount(android.links.length) })));
    block.append(deepLinksIntentFiltersBody(android.links));
  }
  block.append(el("div", { class: "dl-subhead" }, el("h3", { text: "Issues" }), el("span", { class: "pane-meta", text: formatCount(android.issues.length) })));
  block.append(deepLinksIssuesBody(android.issues, MAX_DEEP_LINKS_ISSUES));
  return block;
}

function deepLinksApplePanel(apple) {
  const configured = apple.configured === false ? "Not configured · " : "";
  const block = deepLinksPanel("Apple", `${configured}${deepLinksPlural(apple.schemes.length, "scheme")} · ${deepLinksPlural(apple.domains.length, "domain")}`);
  if (apple.infoPlist || apple.entitlements || apple.bundleId) {
    block.append(el("div", { class: "panel-block-body dl-source" },
      el("span", { text: "Info.plist" }),
      el("code", { text: apple.infoPlist || "not reported" }),
      apple.bundleId ? el("span", { class: "dl-source-meta", text: apple.bundleId }) : null,
    ));
    if (apple.entitlements) {
      block.append(el("div", { class: "panel-block-body dl-source" },
        el("span", { text: "Entitlements" }),
        el("code", { text: apple.entitlements }),
      ));
    }
  }
  block.append(el("div", { class: "dl-subhead" }, el("h3", { text: "Schemes" })));
  block.append(deepLinksSchemesBlock("Apple URL schemes", apple.schemes));
  block.append(el("div", { class: "dl-subhead" }, el("h3", { text: "Associated domains" })));
  block.append(apple.associatedDomains.length
    ? deepLinksAssociatedBody(apple.associatedDomains)
    : deepLinksDomainsBlock("associated domains", apple.domains));
  block.append(el("div", { class: "dl-subhead" }, el("h3", { text: "Issues" }), el("span", { class: "pane-meta", text: formatCount(apple.issues.length) })));
  block.append(deepLinksIssuesBody(apple.issues, MAX_DEEP_LINKS_ISSUES));
  return block;
}

function deepLinksScope(model, root) {
  return el("div", { class: "dl-scope" },
    icon("link"),
    el("span", {},
      el("strong", { text: "Project root" }),
      " ",
      el("code", { text: model.root || root || "not reported" }),
    ),
    model.status ? el("span", { class: "dl-scope-meta", text: model.status }) : null,
  );
}

export function renderDeepLinks() {
  const host = dom.deepLinksContent;
  clear(host);
  const app = state.deepLinks;
  setDeepLinksChrome();
  if (!state.token) {
    showState(host, { icon: "device", title: "Not connected", message: "Connect to a controller to validate a project's deep links." });
    return;
  }
  if (app.rootError) {
    showState(host, { kind: "error", icon: "link", title: "Invalid project root", message: app.rootError });
    return;
  }
  if (app.loading && !app.data) {
    showState(host, { kind: "loading", title: "Validating deep links", message: `Waiting for the controller's get_ui_deep_links response for ${app.root || "the project root"}.` });
    return;
  }
  if (app.error) {
    if (app.error.code === "unavailable") {
      showState(host, {
        icon: "link",
        title: "Deep Links unavailable",
        message: errorMessage(app.error),
        action: "Retry",
        onAction: () => loadDeepLinks({ root: app.root, quiet: false }),
      });
      return;
    }
    if (app.error.code === "unknown_command") {
      showState(host, {
        icon: "link",
        title: "Deep Links are not supported by this controller",
        message: errorMessage(app.error),
      });
      return;
    }
    showState(host, {
      kind: "error",
      title: "Deep links validation failed",
      message: errorMessage(app.error),
      action: "Retry",
      onAction: () => loadDeepLinks({ root: app.root, quiet: false }),
    });
    return;
  }
  if (app.data === null || app.data === undefined) {
    if (!app.validatedRoot) {
      showState(host, { icon: "link", title: "No project validated", message: "Enter an absolute project root and choose Validate." });
      return;
    }
    showState(host, {
      icon: "link",
      title: "No validation result",
      message: "The controller returned no deep links payload. Nothing is substituted.",
      action: "Retry",
      onAction: () => loadDeepLinks({ root: app.root, quiet: false }),
    });
    return;
  }
  const model = normalizeDeepLinks(app.data);
  if (!model) {
    showState(host, {
      kind: "error",
      title: "Unrecognized deep links response",
      message: "The response contained no project files, Android or Apple configuration, issues or summary. No links are shown.",
      action: "Retry",
      onAction: () => loadDeepLinks({ root: app.root, quiet: false }),
    });
    return;
  }
  if (model.unavailable) {
    showState(host, {
      icon: "link",
      title: "Deep Links unavailable",
      message: model.reason,
      action: "Retry",
      onAction: () => loadDeepLinks({ root: app.root, quiet: false }),
    });
    return;
  }
  if (app.loading) {
    host.append(el("div", { class: "inline-spinner" }, el("span", { class: "spinner", attrs: { role: "progressbar", "aria-label": "Validating deep links" } }), "Validating deep links…"));
  }
  host.append(deepLinksScope(model, app.root));
  if (!model.configured) {
    const reported = model.statusMessage && model.statusMessage !== "No deep links configured." && model.statusMessage !== "No deep links configured"
      ? model.statusMessage
      : "No Android intent filters or Apple URL types were reported for this project.";
    host.append(el("div", { class: "dl-status is-empty" },
      icon("link"),
      el("div", {},
        el("strong", { text: "No deep links configured" }),
        el("span", { text: reported }),
      ),
    ));
  }
  host.append(el("div", { class: "stat-row" },
    deepLinksStat("Android schemes", model.summary.androidSchemes),
    deepLinksStat("Android hosts", model.summary.androidHosts),
    deepLinksStat("Apple schemes", model.summary.appleSchemes),
    deepLinksStat("Apple domains", model.summary.appleDomains),
    deepLinksStat("Files", model.summary.files),
    deepLinksStat("Issues", model.summary.issues, model.summary.errors || model.summary.warnings ? `${deepLinksPlural(model.summary.errors, "error")} · ${deepLinksPlural(model.summary.warnings, "warning")}` : ""),
  ));
  host.append(deepLinksFilesPanel(model.files));
  host.append(deepLinksAndroidPanel(model.android));
  host.append(deepLinksApplePanel(model.apple));
  if (model.otherIssues.length) {
    const block = deepLinksPanel("Other issues", formatCount(model.otherIssues.length));
    block.append(deepLinksIssuesBody(model.otherIssues, MAX_DEEP_LINKS_ISSUES));
    host.append(block);
  }
  for (const warning of model.warnings) host.append(errorInline(warning));
  host.append(el("div", { class: "dl-note", text: "Validation reads known Android and Apple project files under the given root. Paths and locations come from the controller's report." }));
}

function setDeepLinksChrome() {
  const app = state.deepLinks;
  dom.deepLinksValidate.disabled = !state.token || app.loading;
  if (!state.token) {
    dom.deepLinksMeta.textContent = "Connect to a controller to validate deep links.";
    return;
  }
  if (app.rootError) {
    dom.deepLinksMeta.textContent = "Invalid project root.";
    return;
  }
  if (app.loading) {
    dom.deepLinksMeta.textContent = "Validating project configuration…";
    return;
  }
  if (app.error) {
    dom.deepLinksMeta.textContent = app.error.code === "unavailable" ? "Deep Links unavailable." : "Validation failed.";
    return;
  }
  if (app.data === null || app.data === undefined) {
    dom.deepLinksMeta.textContent = app.validatedRoot ? "No validation result returned." : "Validate Android intent filters and Apple URL types in a project root.";
    return;
  }
  const model = normalizeDeepLinks(app.data);
  if (!model) {
    dom.deepLinksMeta.textContent = "Unrecognized deep links response.";
    return;
  }
  if (model.unavailable) {
    dom.deepLinksMeta.textContent = "Deep Links unavailable.";
    return;
  }
  const stateText = model.configured ? "Deep links configured" : "No deep links configured";
  dom.deepLinksMeta.textContent = `${stateText} · ${deepLinksPlural(model.summary.files, "file")} · ${deepLinksPlural(model.summary.androidSchemes, "Android scheme")} · ${deepLinksPlural(model.summary.appleDomains, "Apple domain")} · ${deepLinksPlural(model.summary.issues, "issue")} · validated ${formatWallTime(app.refreshedAt)}`;
}

export function showDeepLinksRootError(message) {
  dom.deepLinksRootError.hidden = !message;
  dom.deepLinksRootError.textContent = message || "";
  dom.deepLinksRoot.setAttribute("aria-invalid", message ? "true" : "false");
}

export async function loadDeepLinks(options = {}) {
  const app = state.deepLinks;
  const root = String(options.root ?? app.root).trim();
  if (!state.token) {
    renderDeepLinks();
    return;
  }
  const invalid = deepLinksRootError(root);
  if (invalid) {
    app.root = root;
    app.rootError = invalid;
    app.loading = false;
    app.error = null;
    app.data = null;
    app.loaded = false;
    app.validatedRoot = "";
    showDeepLinksRootError(invalid);
    renderDeepLinks();
    return;
  }
  if (app.loading) return;
  app.root = root;
  app.rootError = "";
  writeDeepLinksRootPreference(root);
  showDeepLinksRootError("");
  const request = app.request + 1;
  app.request = request;
  app.loading = true;
  app.error = null;
  app.validatedRoot = root;
  if (options.reset !== false) {
    app.data = null;
    app.loaded = false;
  }
  renderDeepLinks();
  try {
    const data = unwrapData(await rpc("get_ui_deep_links", { root }));
    if (request !== app.request) return;
    app.loaded = true;
    app.data = data === null || data === undefined ? null : data;
    app.refreshedAt = Date.now();
  } catch (error) {
    if (request !== app.request) return;
    app.error = error;
    if (error?.code === "unknown_command") {
      app.error = new RpcError("This controller does not expose get_ui_deep_links.", "unknown_command");
    }
    app.data = null;
    app.loaded = false;
    if (options.quiet !== true && error?.code !== "unavailable") toast(errorMessage(error), "error", 8000);
  } finally {
    if (request === app.request) {
      app.loading = false;
      renderDeepLinks();
    }
  }
}

export function submitDeepLinks(event) {
  event?.preventDefault();
  const root = dom.deepLinksRoot.value.trim();
  dom.deepLinksRoot.value = root;
  state.deepLinks.root = root;
  loadDeepLinks({ root, quiet: false });
}

export function ensureDeepLinks() {
  renderDeepLinks();
}

