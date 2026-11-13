import { clear, el, fmtBytes, icon, numberValue } from "../../core/format.js";
import { APP_SIZE_SNAPSHOT_SCHEMA, APP_SIZE_SNAPSHOT_VERSION, MAX_APP_SIZE_DIFF_ROWS, RpcError, dom, state } from "../../core/globals.js";
import { errorMessage, rpc, targetById, toast, unwrapData } from "../../core/session.js";
import { errorInline, showState } from "../../core/ui-helpers.js";
import { appSizeFirstNumber, appSizeTargetUnavailable, normalizeAppSize } from "./model.js";
import { appSizePanel, appSizeScope, appSizeSource, appSizeTable, renderAppSizeGroups, renderAppSizeSections, renderAppSizeStats, renderAppSizeSymbols } from "./sections.js";
import { renderAppSizeTreemap } from "./treemap.js";
import { formatWallTime } from "../memory/snapshot.js";
import { emptyAppSize } from "../performance/overview.js";
import { formatCount, isObjectValue } from "../performance/profile/model.js";

function appSizeSnapshotOptionalNumber(value, label) {
  if (value === undefined || value === null) return null;
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) {
    throw new Error(`${label} is not a non-negative number`);
  }
  return value;
}

function appSizeSnapshotText(value) {
  return typeof value === "string" ? value.trim() : "";
}

function appSizeSnapshotOf(executable, target) {
  const pid = executable.pid ?? appSizeFirstNumber(target?.pid, target?.processId, target?.process_id);
  const path = executable.path || "";
  return {
    schema: APP_SIZE_SNAPSHOT_SCHEMA,
    version: APP_SIZE_SNAPSHOT_VERSION,
    savedAt: Date.now(),
    source: "session snapshot",
    measurement: executable.measurement || "",
    label: executable.label || "",
    attribution: executable.attribution || "",
    target: {
      id: target?.id || "",
      pid: pid ?? null,
      platform: executable.platform || target?.platform || "",
    },
    pid: pid ?? null,
    path,
    format: executable.format || "",
    arch: executable.arch || "",
    totalBytes: executable.totalBytes,
    sections: executable.sections.map(section => ({
      name: section.name,
      kind: section.kind || "",
      sizeBytes: section.sizeBytes,
      address: section.address || "",
    })),
    symbols: executable.symbols.map(symbol => ({
      name: symbol.name,
      rawName: symbol.rawName || "",
      crateName: symbol.crateName || "",
      sizeBytes: symbol.sizeBytes,
    })),
    groups: executable.groups.map(group => ({
      name: group.name,
      symbolCount: group.symbolCount,
      sizeBytes: group.sizeBytes,
    })),
    truncation: executable.truncation,
    warnings: executable.warnings,
  };
}

function appSizeSnapshotEntries(value, kind) {
  if (value === undefined || value === null) return [];
  if (!Array.isArray(value)) throw new Error(`${kind} is not an array`);
  return value.map((entry, index) => {
    if (!isObjectValue(entry)) throw new Error(`${kind}[${index}] is not an object`);
    const name = appSizeSnapshotText(entry.name);
    if (!name) throw new Error(`${kind}[${index}] has no name`);
    const sizeBytes = appSizeSnapshotOptionalNumber(entry.sizeBytes, `${kind}[${index}].sizeBytes`);
    if (kind === "sections") {
      return { name, sizeBytes, kind: appSizeSnapshotText(entry.kind), address: appSizeSnapshotText(entry.address) };
    }
    if (kind === "symbols") {
      return {
        name,
        sizeBytes,
        rawName: appSizeSnapshotText(entry.rawName),
        crateName: appSizeSnapshotText(entry.crateName),
      };
    }
    return { name, sizeBytes, symbolCount: appSizeSnapshotOptionalNumber(entry.symbolCount, `${kind}[${index}].symbolCount`) };
  });
}

function appSizeSnapshotTruncation(value) {
  if (!isObjectValue(value)) return null;
  const result = {};
  for (const kind of ["groups", "sections", "symbols"]) {
    const info = value[kind];
    if (!isObjectValue(info)) continue;
    result[kind] = {
      shown: appSizeSnapshotOptionalNumber(info.shown, `truncation.${kind}.shown`),
      returned: appSizeSnapshotOptionalNumber(info.returned, `truncation.${kind}.returned`),
      total: appSizeSnapshotOptionalNumber(info.total, `truncation.${kind}.total`),
      truncated: info.truncated === true ? true : info.truncated === false ? false : null,
    };
  }
  return Object.keys(result).length ? result : null;
}

function appSizeBaselineFromSerialized(parsed, source) {
  const baseline = {
    schema: APP_SIZE_SNAPSHOT_SCHEMA,
    version: APP_SIZE_SNAPSHOT_VERSION,
    savedAt: appSizeSnapshotOptionalNumber(parsed.savedAt, "savedAt"),
    source,
    measurement: appSizeSnapshotText(parsed.measurement),
    label: appSizeSnapshotText(parsed.label),
    attribution: appSizeSnapshotText(parsed.attribution),
    target: isObjectValue(parsed.target) ? parsed.target : null,
    pid: appSizeSnapshotOptionalNumber(parsed.pid, "pid"),
    path: appSizeSnapshotText(parsed.path),
    format: appSizeSnapshotText(parsed.format),
    arch: appSizeSnapshotText(parsed.arch),
    totalBytes: appSizeSnapshotOptionalNumber(parsed.totalBytes, "totalBytes"),
    sections: appSizeSnapshotEntries(parsed.sections, "sections"),
    symbols: appSizeSnapshotEntries(parsed.symbols, "symbols"),
    groups: appSizeSnapshotEntries(parsed.groups, "groups"),
    truncation: appSizeSnapshotTruncation(parsed.truncation),
    warnings: Array.isArray(parsed.warnings) ? parsed.warnings.filter(warning => typeof warning === "string") : [],
  };
  if (!baseline.path && isObjectValue(baseline.target)) baseline.path = appSizeSnapshotText(baseline.target.path);
  if (!baseline.pid && isObjectValue(baseline.target)) baseline.pid = appSizeSnapshotOptionalNumber(baseline.target.pid, "target.pid");
  if (baseline.totalBytes === null && !baseline.sections.length && !baseline.symbols.length && !baseline.groups.length && !baseline.path) {
    throw new Error("the snapshot contains no total size, sections, symbols, groups or executable path");
  }
  return baseline;
}

function appSizeBaselineFromText(text) {
  let parsed;
  try {
    parsed = JSON.parse(text);
  } catch {
    throw new Error("the file is not valid JSON");
  }
  if (!isObjectValue(parsed)) throw new Error("the file does not contain a JSON object");
  if (parsed.schema === APP_SIZE_SNAPSHOT_SCHEMA) {
    if (parsed.version !== APP_SIZE_SNAPSHOT_VERSION) {
      throw new Error(`unsupported snapshot version ${JSON.stringify(parsed.version)}`);
    }
    return appSizeBaselineFromSerialized(parsed, "imported snapshot");
  }
  const executable = normalizeAppSize(parsed);
  if (!executable) throw new Error("the file has neither a Cove App Size snapshot nor a controller app-size payload");
  if (executable.unavailable) throw new Error(`the payload reports App Size unavailable: ${executable.reason}`);
  const baseline = appSizeSnapshotOf(executable, null);
  baseline.savedAt = appSizeFirstNumber(parsed.savedAt, parsed.saved_at, parsed.capturedAt, parsed.captured_at, parsed.timestamp);
  baseline.source = "imported controller payload";
  return baseline;
}

function appSizeDiffEntryKey(kind, entry) {
  return kind === "symbols" ? (entry.rawName || entry.name) : entry.name;
}

function appSizeDiffIndex(kind, entries) {
  const map = new Map();
  let duplicates = 0;
  for (const entry of entries || []) {
    const key = appSizeDiffEntryKey(kind, entry);
    const existing = map.get(key);
    if (existing) {
      duplicates += 1;
      if (existing.sizeBytes === null || entry.sizeBytes === null) existing.sizeBytes = null;
      else existing.sizeBytes += entry.sizeBytes;
      continue;
    }
    map.set(key, { key, name: entry.name, sizeBytes: entry.sizeBytes });
  }
  return { map, duplicates };
}

function appSizeDiff(baseline, current) {
  const kinds = [
    { key: "groups", label: "Group" },
    { key: "sections", label: "Section" },
    { key: "symbols", label: "Symbol" },
  ];
  const rows = [];
  let duplicates = 0;
  for (const kind of kinds) {
    const before = appSizeDiffIndex(kind.key, baseline?.[kind.key]);
    const after = appSizeDiffIndex(kind.key, current?.[kind.key]);
    duplicates += before.duplicates + after.duplicates;
    const keys = new Set([...before.map.keys(), ...after.map.keys()]);
    for (const key of keys) {
      const previous = before.map.get(key);
      const next = after.map.get(key);
      let status;
      let delta = null;
      if (!previous) {
        status = "added";
        delta = next.sizeBytes;
      } else if (!next) {
        status = "removed";
        delta = previous.sizeBytes === null ? null : -previous.sizeBytes;
      } else if (previous.sizeBytes === null || next.sizeBytes === null) {
        status = previous.sizeBytes === next.sizeBytes ? "unchanged" : "changed";
      } else {
        status = previous.sizeBytes === next.sizeBytes ? "unchanged" : "changed";
        delta = next.sizeBytes - previous.sizeBytes;
      }
      const denominator = previous?.sizeBytes ?? null;
      rows.push({
        kind: kind.key,
        kindLabel: kind.label,
        name: next?.name || previous?.name,
        status,
        beforeBytes: previous?.sizeBytes ?? null,
        afterBytes: next?.sizeBytes ?? null,
        beforePresent: Boolean(previous),
        afterPresent: Boolean(next),
        delta,
        percent: delta === null || denominator === null || denominator <= 0 ? null : (delta / denominator) * 100,
      });
    }
  }
  const counts = { added: 0, removed: 0, changed: 0, unchanged: 0 };
  for (const row of rows) counts[row.status] += 1;
  const order = { groups: 0, sections: 1, symbols: 2 };
  const changed = rows
    .filter(row => row.status !== "unchanged")
    .sort((left, right) => Math.abs(right.delta ?? 0) - Math.abs(left.delta ?? 0)
      || order[left.kind] - order[right.kind]
      || (left.name < right.name ? -1 : left.name > right.name ? 1 : 0));
  const totalBefore = appSizeSnapshotOptionalNumber(baseline?.totalBytes, "baseline total");
  const totalAfter = appSizeSnapshotOptionalNumber(current?.totalBytes, "current total");
  const totalDelta = totalBefore !== null && totalAfter !== null ? totalAfter - totalBefore : null;
  return {
    counts,
    rows,
    changed,
    duplicates,
    total: {
      before: totalBefore,
      after: totalAfter,
      delta: totalDelta,
      percent: totalDelta === null || totalBefore === null || totalBefore <= 0 ? null : (totalDelta / totalBefore) * 100,
    },
  };
}

function appSizeDeltaText(delta) {
  if (delta === null) return "Not reported";
  if (delta === 0) return "±0 B";
  return `${delta > 0 ? "+" : "−"}${fmtBytes(Math.abs(delta))}`;
}

function appSizePercentText(percent) {
  if (percent === null || !Number.isFinite(percent)) return "—";
  if (Math.abs(percent) < 0.05) return "±0.0%";
  return `${percent > 0 ? "+" : "−"}${Math.abs(percent).toFixed(1)}%`;
}

function appSizeDeltaClass(delta) {
  if (delta === null) return "";
  if (delta > 0) return "is-positive";
  if (delta < 0) return "is-negative";
  return "is-zero";
}

function appSizeDiffTruncationDetail(info) {
  if (!info) return "";
  const returned = info.returned;
  const total = info.total;
  if (total !== null && returned !== null && total > returned) return `${formatCount(returned)} of ${formatCount(total)}`;
  if (info.truncated === true) return returned !== null ? `${formatCount(returned)} returned, controller-capped` : "controller-capped";
  return "";
}

function appSizeDiffTruncation(baseline, current) {
  const labels = { groups: "groups", sections: "sections", symbols: "symbols" };
  const parts = [];
  for (const kind of ["groups", "sections", "symbols"]) {
    const before = appSizeDiffTruncationDetail(baseline?.truncation?.[kind]);
    const after = appSizeDiffTruncationDetail(current?.truncation?.[kind]);
    if (!before && !after) continue;
    const sides = [];
    if (before) sides.push(`baseline ${before}`);
    if (after) sides.push(`current ${after}`);
    parts.push(`${labels[kind]} (${sides.join(", ")})`);
  }
  return parts.length ? `Truncated lists: ${parts.join("; ")}. The diff compares only returned entries, so unlisted entries cannot be matched.` : "";
}

function appSizeDiffCellBytes(value, present) {
  if (!present) return el("span", { class: "app-size-diff-absent", text: "—" });
  if (value === null) return el("span", { text: "Not reported" });
  return el("span", { text: fmtBytes(value), attrs: { title: `${formatCount(value)} bytes` } });
}

function renderAppSizeDiff(executable) {
  const app = state.appSize;
  const baseline = app.baseline;
  const hasData = Boolean(executable) && !executable.unavailable;
  const meta = baseline
    ? `Baseline ${baseline.savedAt === null ? "not timestamped" : formatWallTime(baseline.savedAt)}`
    : "No baseline";
  const block = appSizePanel("Size diff", meta);
  block.id = "app-size-diff-panel";
  const body = el("div", { class: "panel-block-body" });
  const toolbar = el("div", { class: "app-size-diff-toolbar" });

  toolbar.append(el("button", {
    class: "tool-button small",
    props: { type: "button", disabled: !hasData || app.loading, id: "app-size-diff-snapshot" },
    attrs: { title: "Store the current report as the size baseline" },
    text: "Snapshot baseline",
    on: { click: () => appSizeCaptureBaseline() },
  }));
  toolbar.append(el("button", {
    class: "tool-button small",
    props: { type: "button", disabled: !baseline, id: "app-size-diff-export" },
    attrs: { title: "Download the baseline snapshot as JSON" },
    text: "Export JSON",
    on: { click: () => appSizeExportBaseline() },
  }));
  toolbar.append(el("label", {
    class: "tool-button small",
    attrs: { for: "app-size-diff-import", title: "Import an App Size snapshot JSON" },
  }, icon("upload"), "Import JSON"));
  const importInput = el("input", {
    class: "sr-only",
    attrs: {
      type: "file",
      id: "app-size-diff-import",
      accept: ".json,application/json",
      "aria-label": "Import an App Size snapshot JSON",
    },
  });
  importInput.addEventListener("change", () => appSizeImportSnapshot(importInput));
  toolbar.append(importInput);
  toolbar.append(el("button", {
    class: "tool-button small",
    props: { type: "button", disabled: !baseline, id: "app-size-diff-clear" },
    attrs: { title: "Discard the current baseline" },
    text: "Clear baseline",
    on: {
      click: () => {
        state.appSize.baseline = null;
        state.appSize.baselineError = "";
        renderAppSize();
      },
    },
  }));
  body.append(toolbar);
  if (app.baselineError) body.append(errorInline(app.baselineError));

  if (!baseline) {
    body.append(el("p", {
      class: "app-size-diff-empty",
      text: "No baseline snapshot. Snapshot the current report or import a snapshot JSON to compare later refreshes.",
    }));
    block.append(body);
    return block;
  }

  const stamp = baseline.savedAt === null ? "timestamp not reported" : formatWallTime(baseline.savedAt);
  body.append(el("p", {
    class: "app-size-diff-baseline",
    text: `${baseline.source || "snapshot"} · ${stamp} · ${baseline.totalBytes === null ? "total not reported" : fmtBytes(baseline.totalBytes)} · ${baseline.path || "path not reported"}`,
    attrs: baseline.path ? { title: baseline.path } : {},
  }));
  const currentStamp = app.refreshedAt ? formatWallTime(app.refreshedAt) : "timestamp not reported";
  body.append(el("p", {
    class: "app-size-diff-current",
    text: `Current report · ${currentStamp} · ${executable.totalBytes === null ? "total not reported" : fmtBytes(executable.totalBytes)} · ${executable.path || "path not reported"}`,
    attrs: executable.path ? { title: executable.path } : {},
  }));
  if (baseline.path && executable.path && baseline.path !== executable.path) {
    body.append(el("p", {
      class: "app-size-note",
      text: "The baseline comes from a different executable path. Entries are matched by name, so bytes may not be comparable.",
    }));
  }

  const diff = appSizeDiff(baseline, executable);
  const summary = el("div", { class: "app-size-diff-summary" });
  summary.append(
    el("span", { class: "is-added", text: `${formatCount(diff.counts.added)} added` }),
    el("span", { text: " · " }),
    el("span", { class: "is-removed", text: `${formatCount(diff.counts.removed)} removed` }),
    el("span", { text: " · " }),
    el("span", { class: "is-changed", text: `${formatCount(diff.counts.changed)} changed` }),
    el("span", { text: " · " }),
    el("span", { class: "is-unchanged", text: `${formatCount(diff.counts.unchanged)} unchanged` }),
  );
  body.append(summary);

  const total = diff.total;
  if (total.before !== null && total.after !== null) {
    body.append(el("p", {
      class: `app-size-diff-total ${appSizeDeltaClass(total.delta)}`,
      text: `Executable total ${fmtBytes(total.before)} → ${fmtBytes(total.after)} · ${appSizeDeltaText(total.delta)} · ${appSizePercentText(total.percent)}`,
    }));
  } else if (total.before === null && total.after === null) {
    body.append(el("p", { class: "app-size-note", text: "Executable total is not reported in either report; no total delta is computed." }));
  } else {
    body.append(el("p", {
      class: "app-size-note",
      text: `Executable total is only reported in the ${total.before === null ? "current" : "baseline"} report; no total delta is computed.`,
    }));
  }

  const truncation = appSizeDiffTruncation(baseline, executable);
  if (truncation) body.append(el("p", { class: "app-size-note", text: truncation }));
  if (diff.duplicates > 0) {
    body.append(el("p", {
      class: "app-size-note",
      text: `${formatCount(diff.duplicates)} duplicate names were combined by name for this diff.`,
    }));
  }

  if (!diff.changed.length) {
    body.append(el("p", {
      class: "app-size-note",
      text: "No added, removed or changed entries between the baseline and the current report.",
    }));
  } else {
    const shown = diff.changed.slice(0, MAX_APP_SIZE_DIFF_ROWS);
    body.append(appSizeTable(
      [
        { label: "Status" },
        { label: "Kind" },
        { label: "Name" },
        { label: "Baseline", num: true },
        { label: "Current", num: true },
        { label: "Δ bytes", num: true },
        { label: "Δ %", num: true },
      ],
      shown.map(row => el("tr", {
        class: `app-size-diff-row is-${row.status}`,
        attrs: { "data-app-size-diff-status": row.status, "data-app-size-diff-kind": row.kind },
      },
        el("td", { class: `app-size-diff-status is-${row.status}`, text: row.status === "added" ? "Added" : row.status === "removed" ? "Removed" : "Changed" }),
        el("td", { text: row.kindLabel }),
        el("td", { class: "app-size-name", text: row.name, attrs: { title: row.name } }),
        el("td", { class: "num app-size-bytes", attrs: { title: row.beforePresent ? "" : "Absent from the baseline" } }, appSizeDiffCellBytes(row.beforeBytes, row.beforePresent)),
        el("td", { class: "num app-size-bytes", attrs: { title: row.afterPresent ? "" : "Absent from the current report" } }, appSizeDiffCellBytes(row.afterBytes, row.afterPresent)),
        el("td", { class: `num app-size-delta ${appSizeDeltaClass(row.delta)}`, text: appSizeDeltaText(row.delta) }),
        el("td", { class: `num app-size-delta ${appSizeDeltaClass(row.delta)}`, text: appSizePercentText(row.percent) }),
      )),
    ));
    if (diff.changed.length > shown.length) {
      body.append(el("p", {
        class: "app-size-note",
        text: `Showing ${formatCount(shown.length)} of ${formatCount(diff.changed.length)} added, removed or changed entries.`,
      }));
    }
    if (diff.counts.added || diff.counts.removed) {
      body.append(el("p", {
        class: "app-size-note",
        text: "A dash means the entry was absent from that report, not zero bytes. Percentages are relative to the baseline entry; entries without baseline bytes have no percentage.",
      }));
    }
  }
  block.append(body);
  return block;
}

function appSizeCaptureBaseline() {
  const executable = normalizeAppSize(state.appSize.data);
  if (!executable || executable.unavailable) return;
  state.appSize.baseline = appSizeSnapshotOf(executable, targetById());
  state.appSize.baselineError = "";
  renderAppSize();
  toast("App Size baseline captured.", "success", 3000);
}

function appSizeFileStamp(milliseconds) {
  const date = new Date(numberValue(milliseconds) ?? Date.now());
  const pad = value => String(value).padStart(2, "0");
  return `${date.getFullYear()}${pad(date.getMonth() + 1)}${pad(date.getDate())}-${pad(date.getHours())}${pad(date.getMinutes())}${pad(date.getSeconds())}`;
}

function appSizeDownload(filename, text) {
  const blob = new Blob([text], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const anchor = el("a", { attrs: { href: url, download: filename, rel: "noopener" } });
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 0);
}

function appSizeExportBaseline() {
  const baseline = state.appSize.baseline;
  if (!baseline) return;
  try {
    const text = `${JSON.stringify(baseline, null, 2)}\n`;
    appSizeDownload(`cove-app-size-${appSizeFileStamp(baseline.savedAt)}.json`, text);
  } catch (error) {
    state.appSize.baselineError = `Export failed: ${error?.message || error}`;
    renderAppSize();
  }
}

async function appSizeImportSnapshot(input) {
  const file = input.files && input.files[0];
  input.value = "";
  if (!file) return;
  try {
    const text = await file.text();
    state.appSize.baseline = appSizeBaselineFromText(text);
    state.appSize.baselineError = "";
    toast("App Size baseline imported.", "success", 3000);
  } catch (error) {
    state.appSize.baselineError = `Snapshot rejected: ${error?.message || error}`;
  }
  renderAppSize();
}

function setAppSizeChrome() {
  const target = targetById();
  const app = state.appSize;
  const gate = target ? appSizeTargetUnavailable(target) : "";
  dom.appSizeRefresh.disabled = !state.token || !target || Boolean(gate) || app.loading;
  if (!state.token) {
    dom.appSizeMeta.textContent = "No controller connection.";
    return;
  }
  if (!target) {
    dom.appSizeMeta.textContent = "No target selected.";
    return;
  }
  if (gate) {
    dom.appSizeMeta.textContent = "App Size is unavailable for this target.";
    return;
  }
  if (app.loading && !app.loaded) {
    dom.appSizeMeta.textContent = "Reading the native executable…";
    return;
  }
  if (app.error && !app.loaded) {
    if (app.error.code === "unavailable") dom.appSizeMeta.textContent = "App Size unavailable.";
    else if (app.error.code === "unknown_command") dom.appSizeMeta.textContent = "This controller does not expose get_ui_app_size.";
    else dom.appSizeMeta.textContent = "Executable read failed.";
    return;
  }
  if (app.data === null) {
    dom.appSizeMeta.textContent = "No executable analysis returned.";
    return;
  }
  const executable = normalizeAppSize(app.data);
  if (!executable) {
    dom.appSizeMeta.textContent = "Unrecognized app size response.";
    return;
  }
  if (executable.unavailable) {
    dom.appSizeMeta.textContent = "App Size unavailable.";
    return;
  }
  const total = executable.totalBytes === null ? "total not reported" : `${fmtBytes(executable.totalBytes)} native executable`;
  dom.appSizeMeta.textContent = `${total} · ${formatCount(executable.sections.length)} sections · ${formatCount(executable.symbols.length)} symbols · ${formatCount(executable.groups.length)} crate groups · updated ${formatWallTime(app.refreshedAt)}`;
}

export function renderAppSize() {
  const host = dom.appSizeContent;
  clear(host);
  setAppSizeChrome();
  const app = state.appSize;
  const target = targetById();
  if (!state.token) {
    showState(host, { icon: "device", title: "Not connected", message: "Connect to a controller to read the selected runtime's native executable." });
    return;
  }
  if (!target) {
    showState(host, { icon: "device", title: "No target", message: "Select a connected runtime to read its native executable." });
    return;
  }
  const gate = appSizeTargetUnavailable(target);
  if (gate) {
    showState(host, { icon: "package", title: "App Size unavailable", message: gate });
    return;
  }
  if (app.loading && !app.loaded) {
    showState(host, { kind: "loading", title: "Reading the native executable", message: "Waiting for the controller's get_ui_app_size response." });
    return;
  }
  if (app.error && !app.loaded) {
    if (app.error.code === "unavailable") {
      showState(host, {
        icon: "package",
        title: "App Size unavailable",
        message: errorMessage(app.error),
        action: "Retry",
        onAction: () => loadAppSize({ reset: true }),
      });
      return;
    }
    if (app.error.code === "unknown_command") {
      showState(host, {
        icon: "package",
        title: "App Size is not supported by this controller",
        message: errorMessage(app.error),
      });
      return;
    }
    showState(host, {
      kind: "error",
      title: "Executable read failed",
      message: errorMessage(app.error),
      action: "Retry",
      onAction: () => loadAppSize({ reset: true }),
    });
    return;
  }
  if (app.data === null) {
    showState(host, {
      icon: "package",
      title: "No executable analysis",
      message: "The controller returned no app size payload. Nothing is substituted.",
      action: "Refresh",
      onAction: () => loadAppSize({ reset: true }),
    });
    return;
  }
  const executable = normalizeAppSize(app.data);
  if (!executable) {
    showState(host, {
      kind: "error",
      title: "Unrecognized app size response",
      message: "The response contained no total size, sections, symbols, groups or executable path. No sizes are shown.",
      action: "Retry",
      onAction: () => loadAppSize({ reset: true }),
    });
    return;
  }
  if (executable.unavailable) {
    showState(host, {
      icon: "package",
      title: "App Size unavailable",
      message: executable.reason,
      action: "Retry",
      onAction: () => loadAppSize({ reset: true }),
    });
    return;
  }
  if (app.loading) {
    host.append(el("div", { class: "inline-spinner" }, el("span", { class: "spinner", attrs: { role: "progressbar", "aria-label": "Reading the native executable" } }), "Reading the native executable…"));
  }
  if (app.error) host.append(errorInline(errorMessage(app.error)));
  host.append(appSizeScope(executable, target));
  host.append(renderAppSizeStats(executable));
  host.append(appSizeSource(executable, target));
  for (const warning of executable.warnings) host.append(errorInline(warning));
  host.append(renderAppSizeTreemap(executable));
  host.append(renderAppSizeSections(executable));
  host.append(renderAppSizeSymbols(executable));
  host.append(renderAppSizeGroups(executable));
  host.append(renderAppSizeDiff(executable));

  if (executable.totalBytes === null) {
    host.append(el("div", { class: "app-size-note", text: "The controller reported no total executable size. A sum of section sizes is not substituted for it." }));
  }
  host.append(el("div", { class: "app-size-note", text: "Sections are object-file sections, not loaded memory mappings. Symbol sizes and crate groups are reported by the native analyzer; bytes outside those groups stay unassigned." }));
}

export async function loadAppSize(options = {}) {
  const app = state.appSize;
  const target = targetById();
  if (!state.token || !target) {
    renderAppSize();
    return;
  }
  const gate = appSizeTargetUnavailable(target);
  if (gate) {
    app.loading = false;
    app.error = null;
    app.data = null;
    app.loaded = true;
    app.refreshedAt = Date.now();
    renderAppSize();
    return;
  }
  if (app.loading) return;
  const targetId = state.targetId;
  const request = app.request + 1;
  app.request = request;
  app.loading = true;
  app.error = null;
  if (options.reset) {
    app.data = null;
    app.loaded = false;
  }
  renderAppSize();
  try {
    const data = unwrapData(await rpc("get_ui_app_size", { target: targetId }));
    if (request !== app.request || targetId !== state.targetId) return;
    app.loaded = true;
    app.data = data === null || data === undefined ? null : data;
    app.refreshedAt = Date.now();
    if (options.quiet !== true) toast("App size refreshed.", "success", 3000);
  } catch (error) {
    if (request !== app.request || targetId !== state.targetId) return;
    app.error = error;
    if (error?.code === "unknown_command") {
      app.error = new RpcError("This controller does not expose get_ui_app_size.", "unknown_command");
    }
    if (options.quiet !== true && error?.code !== "unavailable") toast(errorMessage(error), "error", 8000);
  } finally {
    if (request === app.request && targetId === state.targetId) {
      app.loading = false;
      renderAppSize();
    }
  }
}

export function ensureAppSize() {
  const target = targetById();
  if (!state.token || !target || appSizeTargetUnavailable(target)) {
    renderAppSize();
    return;
  }
  if (!state.appSize.loaded && !state.appSize.loading) {
    loadAppSize({ quiet: true });
    return;
  }
  renderAppSize();
}

export function resetAppSize() {
  const request = state.appSize.request + 1;
  state.appSize = emptyAppSize();
  state.appSize.request = request;
  if (state.view === "app-size") renderAppSize();
}

