/* Allocation-site baselines
 *
 * A baseline is the exact sampled report the client received, plus the target,
 * rate, session and timestamps that make the comparison interpretable. Sites
 * are matched by their full symbolized stack identity, never by array order.
 * Cumulative sampled counters from different sessions or rates are not
 * heap growth and are labelled as such instead of being converted.
 */

function allocationSnapshotRequireKeys(value, keys, label) {
  if (!isObjectValue(value)) throw new Error(`${label} is not an object`);
  const missing = keys.filter(key => !Object.prototype.hasOwnProperty.call(value, key));
  if (missing.length) throw new Error(`${label} is missing ${missing.join(", ")}`);
  const unknown = Object.keys(value).filter(key => !keys.includes(key));
  if (unknown.length) throw new Error(`${label} has an unknown field: ${unknown.join(", ")}`);
  return value;
}

function allocationSnapshotInteger(value, label, options = {}) {
  const { nullable = false, min = 0, max = Number.MAX_SAFE_INTEGER } = options;
  if (value === null && nullable) return null;
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < min || value > max) {
    throw new Error(`${label} is not ${nullable ? "null or " : ""}an integer from ${formatCount(min)} to ${formatCount(max)}`);
  }
  return value;
}

function allocationSnapshotString(value, label, max = 2048) {
  if (typeof value !== "string") throw new Error(`${label} is not a string`);
  if (value.length > max) throw new Error(`${label} is longer than ${formatCount(max)} characters`);
  return value;
}

function allocationSnapshotFrame(value, label) {
  allocationSnapshotRequireKeys(value, ALLOCATION_SNAPSHOT_FRAME_KEYS, label);
  const address = allocationSnapshotString(value.address, `${label}.address`, 64);
  if (address && !/^0x[0-9a-f]+$/.test(address)) {
    throw new Error(`${label}.address is not a lowercase hexadecimal address`);
  }
  const functionName = allocationSnapshotString(value.function, `${label}.function`);
  const file = allocationSnapshotString(value.file, `${label}.file`);
  const module = allocationSnapshotString(value.module, `${label}.module`);
  const line = value.line === null
    ? null
    : allocationSnapshotInteger(value.line, `${label}.line`, { min: 0, max: 0xffffffff });
  if (!address && !functionName && !file && line === null) {
    throw new Error(`${label} has no function, file, line or address`);
  }
  return { address, function: functionName, file, line, module };
}

function allocationSnapshotSite(value, label, index) {
  allocationSnapshotRequireKeys(value, ALLOCATION_SNAPSHOT_SITE_KEYS, label);
  if (!Array.isArray(value.frames)) throw new Error(`${label}.frames is not an array`);
  if (value.frames.length > MAX_ALLOCATION_SITE_FRAMES) {
    throw new Error(`${label}.frames has more than ${formatCount(MAX_ALLOCATION_SITE_FRAMES)} frames`);
  }
  const frames = value.frames.map((frame, frameIndex) => allocationSnapshotFrame(frame, `${label}.frames[${frameIndex}]`));
  const bytes = allocationSnapshotInteger(value.bytes, `${label}.bytes`, { nullable: true });
  const blocks = allocationSnapshotInteger(value.blocks, `${label}.blocks`, { nullable: true });
  const framesOmitted = allocationSnapshotInteger(value.framesOmitted, `${label}.framesOmitted`);
  if (typeof value.framesTruncated !== "boolean") throw new Error(`${label}.framesTruncated is not a boolean`);
  const stackKey = allocationSiteStackIdentity(frames);
  return {
    key: stackKey || `site-${index}`,
    stackKey,
    frames,
    bytes,
    blocks,
    framesOmitted,
    framesTruncated: value.framesTruncated,
  };
}

function allocationSnapshotReport(value) {
  allocationSnapshotRequireKeys(value, ALLOCATION_SNAPSHOT_REPORT_KEYS, "report");
  if (!ALLOCATION_SNAPSHOT_STATUSES.includes(value.status)) {
    throw new Error(`report.status is not one of ${ALLOCATION_SNAPSHOT_STATUSES.join(", ")}`);
  }
  if (typeof value.sitesTruncated !== "boolean") throw new Error("report.sitesTruncated is not a boolean");
  if (typeof value.framesTruncated !== "boolean") throw new Error("report.framesTruncated is not a boolean");
  if (!Array.isArray(value.sites)) throw new Error("report.sites is not an array");
  if (value.sites.length > MAX_ALLOCATION_SITES) {
    throw new Error(`report.sites has more than ${formatCount(MAX_ALLOCATION_SITES)} sites`);
  }
  const sites = value.sites.map((site, index) => allocationSnapshotSite(site, `report.sites[${index}]`, index));
  const seen = new Set();
  let knownBytes = 0;
  let knownBlocks = 0;
  for (const site of sites) {
    if (site.stackKey) {
      if (seen.has(site.stackKey)) throw new Error("report.sites repeats a stack identity; sites cannot be matched unambiguously");
      seen.add(site.stackKey);
    }
    if (site.bytes !== null) knownBytes += site.bytes;
    if (site.blocks !== null) knownBlocks += site.blocks;
  }
  const sampledBytes = allocationSnapshotInteger(value.sampledBytes, "report.sampledBytes", { nullable: true });
  const sampledBlocks = allocationSnapshotInteger(value.sampledBlocks, "report.sampledBlocks", { nullable: true });
  if (sampledBytes !== null && knownBytes > sampledBytes) {
    throw new Error("report.sampledBytes is smaller than the sum of its sites");
  }
  if (sampledBlocks !== null && knownBlocks > sampledBlocks) {
    throw new Error("report.sampledBlocks is smaller than the sum of its sites");
  }
  const totalSites = allocationSnapshotInteger(value.totalSites, "report.totalSites", { nullable: true });
  if (totalSites !== null && totalSites < sites.length) {
    throw new Error("report.totalSites is smaller than the number of listed sites");
  }
  return {
    status: value.status,
    sampledBytes,
    sampledBlocks,
    samplesAttempted: allocationSnapshotInteger(value.samplesAttempted, "report.samplesAttempted", { nullable: true }),
    samplesRetained: allocationSnapshotInteger(value.samplesRetained, "report.samplesRetained", { nullable: true }),
    samplesDropped: allocationSnapshotInteger(value.samplesDropped, "report.samplesDropped", { nullable: true }),
    captureFailures: allocationSnapshotInteger(value.captureFailures, "report.captureFailures", { nullable: true }),
    totalSites,
    sitesOmitted: allocationSnapshotInteger(value.sitesOmitted, "report.sitesOmitted"),
    framesOmitted: allocationSnapshotInteger(value.framesOmitted, "report.framesOmitted"),
    sitesTruncated: value.sitesTruncated,
    framesTruncated: value.framesTruncated,
    symbolizationReason: allocationSnapshotString(value.symbolizationReason, "report.symbolizationReason"),
    sites,
  };
}

function allocationSnapshotFromText(text, expectedTargetId) {
  let parsed;
  try {
    parsed = JSON.parse(text);
  } catch {
    throw new Error("the file is not valid JSON");
  }
  allocationSnapshotRequireKeys(parsed, ALLOCATION_SNAPSHOT_KEYS, "the snapshot");
  if (parsed.schema !== ALLOCATION_SNAPSHOT_SCHEMA) {
    throw new Error("the file is not a Cove allocation-site snapshot");
  }
  if (parsed.version !== ALLOCATION_SNAPSHOT_VERSION) {
    throw new Error(`unsupported snapshot version ${JSON.stringify(parsed.version)}`);
  }
  const capturedAt = allocationSnapshotInteger(parsed.capturedAt, "capturedAt", { min: 1 });
  const reportUpdatedAt = allocationSnapshotInteger(parsed.reportUpdatedAt, "reportUpdatedAt", { nullable: true, min: 1 });
  allocationSnapshotRequireKeys(parsed.target, ["id", "title"], "target");
  const targetId = allocationSnapshotString(parsed.target.id, "target.id", 256).trim();
  if (!targetId) throw new Error("target.id is empty");
  const title = allocationSnapshotString(parsed.target.title, "target.title", 512);
  if (expectedTargetId && targetId !== expectedTargetId) {
    throw new Error(`the snapshot was captured on target ${JSON.stringify(targetId)}, not the selected target`);
  }
  if (parsed.session !== null && (typeof parsed.session !== "string" || !/^[A-Za-z0-9:._-]{1,128}$/.test(parsed.session))) {
    throw new Error("session is not null or a session identifier");
  }
  const rate = allocationSnapshotInteger(parsed.rate, "rate", { min: 1, max: ALLOCATION_RATE_LIMIT });
  return {
    schema: ALLOCATION_SNAPSHOT_SCHEMA,
    version: ALLOCATION_SNAPSHOT_VERSION,
    capturedAt,
    reportUpdatedAt,
    target: { id: targetId, title },
    session: parsed.session,
    rate,
    report: allocationSnapshotReport(parsed.report),
  };
}

function allocationSessionId(allocation) {
  return allocation.sessionSeq > 0 ? `${ALLOCATION_SESSION_NONCE}:${allocation.sessionSeq}` : null;
}

function allocationSnapshotOfReport(report, allocation) {
  const target = targetById();
  const rate = allocationSiteRateValue(report.rate)
    ?? allocationSiteRateValue(allocation.rate)
    ?? ALLOCATION_DEFAULT_RATE;
  return {
    schema: ALLOCATION_SNAPSHOT_SCHEMA,
    version: ALLOCATION_SNAPSHOT_VERSION,
    capturedAt: Date.now(),
    reportUpdatedAt: allocation.queriedAt || report.refreshedAt || null,
    target: { id: state.targetId, title: target?.title || "" },
    session: allocationSessionId(allocation),
    rate,
    report: {
      status: allocation.status === "running" || report.status === "running" ? "running" : "stopped",
      sampledBytes: report.sampledBytes,
      sampledBlocks: report.sampledBlocks,
      samplesAttempted: report.samplesAttempted,
      samplesRetained: report.samplesRetained,
      samplesDropped: report.samplesDropped,
      captureFailures: report.captureFailures,
      totalSites: report.totalSites,
      sitesOmitted: report.sitesOmitted ?? 0,
      framesOmitted: report.framesOmitted ?? 0,
      sitesTruncated: Boolean(report.sitesTruncated),
      framesTruncated: Boolean(report.framesTruncated),
      symbolizationReason: report.symbolizationReason || "",
      sites: report.sites.slice(0, MAX_ALLOCATION_SITES).map(site => ({
        frames: site.frames.slice(0, MAX_ALLOCATION_SITE_FRAMES).map(frame => ({
          address: /^0x[0-9a-f]+$/.test(frame.address || "") ? frame.address : "",
          function: frame.function || "",
          file: frame.file || "",
          line: Number.isInteger(frame.line) && frame.line >= 0 && frame.line <= 0xffffffff ? frame.line : null,
          module: frame.module || "",
        })),
        bytes: site.bytes,
        blocks: site.blocks,
        framesOmitted: site.framesOmitted,
        framesTruncated: site.framesTruncated,
      })),
    },
  };
}

function allocationSnapshotSerializable(snapshot) {
  return {
    schema: snapshot.schema,
    version: snapshot.version,
    capturedAt: snapshot.capturedAt,
    reportUpdatedAt: snapshot.reportUpdatedAt,
    target: { id: snapshot.target.id, title: snapshot.target.title },
    session: snapshot.session,
    rate: snapshot.rate,
    report: {
      status: snapshot.report.status,
      sampledBytes: snapshot.report.sampledBytes,
      sampledBlocks: snapshot.report.sampledBlocks,
      samplesAttempted: snapshot.report.samplesAttempted,
      samplesRetained: snapshot.report.samplesRetained,
      samplesDropped: snapshot.report.samplesDropped,
      captureFailures: snapshot.report.captureFailures,
      totalSites: snapshot.report.totalSites,
      sitesOmitted: snapshot.report.sitesOmitted,
      framesOmitted: snapshot.report.framesOmitted,
      sitesTruncated: snapshot.report.sitesTruncated,
      framesTruncated: snapshot.report.framesTruncated,
      symbolizationReason: snapshot.report.symbolizationReason,
      sites: snapshot.report.sites.map(site => ({
        frames: site.frames.map(frame => ({
          address: frame.address,
          function: frame.function,
          file: frame.file,
          line: frame.line,
          module: frame.module,
        })),
        bytes: site.bytes,
        blocks: site.blocks,
        framesOmitted: site.framesOmitted,
        framesTruncated: site.framesTruncated,
      })),
    },
  };
}

function allocationCaptureBaseline() {
  const allocation = state.memory.allocation;
  const report = allocation.report;
  if (!report || !state.targetId) return;
  allocation.baseline = allocationSnapshotOfReport(report, allocation);
  allocation.baselineSource = "session snapshot";
  allocation.baselineError = "";
  allocation.diffExpanded.clear();
  renderAllocationSitesView();
  toast("Allocation-site baseline captured.", "success", 3000);
}

function allocationSnapshotFilename(snapshot) {
  const date = new Date(snapshot.capturedAt);
  const pad = value => String(value).padStart(2, "0");
  const stamp = `${date.getFullYear()}${pad(date.getMonth() + 1)}${pad(date.getDate())}-${pad(date.getHours())}${pad(date.getMinutes())}${pad(date.getSeconds())}`;
  return `cove-allocation-sites-${slug(snapshot.target.title || snapshot.target.id)}-${stamp}.json`;
}

function allocationExportBaseline() {
  const allocation = state.memory.allocation;
  const baseline = allocation.baseline;
  if (!baseline) return;
  try {
    downloadJson(allocationSnapshotFilename(baseline), allocationSnapshotSerializable(baseline));
  } catch (error) {
    allocation.baselineError = `Export failed: ${error?.message || error}`;
    renderAllocationSitesView();
  }
}

function allocationClearBaseline() {
  const allocation = state.memory.allocation;
  if (!allocation.baseline && !allocation.baselineError) return;
  const hadBaseline = Boolean(allocation.baseline);
  allocation.baseline = null;
  allocation.baselineSource = "";
  allocation.baselineError = "";
  allocation.diffExpanded.clear();
  renderAllocationSitesView();
  if (hadBaseline) toast("Allocation-site baseline cleared.", "info", 3000);
}

async function allocationImportBaseline(input) {
  const file = input.files && input.files[0];
  input.value = "";
  if (!file) return;
  const allocation = state.memory.allocation;
  const targetId = state.targetId;
  try {
    if (file.size > MAX_ALLOCATION_SNAPSHOT_BYTES) {
      throw new Error(`the file is larger than ${formatCount(MAX_ALLOCATION_SNAPSHOT_BYTES)} bytes`);
    }
    const text = await file.text();
    if (targetId !== state.targetId) return;
    allocation.baseline = allocationSnapshotFromText(text, targetId);
    allocation.baselineSource = "imported snapshot";
    allocation.baselineError = "";
    allocation.diffExpanded.clear();
    renderAllocationSitesView();
    toast("Allocation-site baseline imported.", "success", 3000);
  } catch (error) {
    if (targetId !== state.targetId) return;
    allocation.baselineError = `Snapshot rejected: ${error?.message || error}`;
    renderAllocationSitesView();
  }
}

function allocationSiteDiffIndex(sites) {
  const map = new Map();
  let duplicates = 0;
  let unmatchable = 0;
  for (const site of sites) {
    const key = site.stackKey ?? allocationSiteStackIdentity(site.frames || []);
    if (!key) {
      unmatchable += 1;
      continue;
    }
    const existing = map.get(key);
    if (existing) {
      duplicates += 1;
      existing.bytes = existing.bytes === null || site.bytes === null ? null : existing.bytes + site.bytes;
      existing.blocks = existing.blocks === null || site.blocks === null ? null : existing.blocks + site.blocks;
      existing.framesTruncated = existing.framesTruncated || site.framesTruncated;
      existing.framesOmitted = Math.max(existing.framesOmitted, site.framesOmitted);
      continue;
    }
    map.set(key, { ...site, key });
  }
  return { map, duplicates, unmatchable };
}

function allocationDiffPercent(before, after) {
  if (before === null || after === null || before <= 0) return null;
  return ((after - before) / before) * 100;
}

function allocationSiteDiff(baselineReport, currentReport) {
  const before = allocationSiteDiffIndex((baselineReport?.sites || []).slice(0, MAX_ALLOCATION_SITES));
  const after = allocationSiteDiffIndex((currentReport?.sites || []).slice(0, MAX_ALLOCATION_SITES));
  const keys = new Set([...before.map.keys(), ...after.map.keys()]);
  const rows = [];
  for (const key of keys) {
    const previous = before.map.get(key) || null;
    const next = after.map.get(key) || null;
    let status = "unchanged";
    if (!previous) status = "added";
    else if (!next) status = "removed";
    else if (previous.bytes !== next.bytes || previous.blocks !== next.blocks) status = "changed";
    let deltaBytes = null;
    let deltaBlocks = null;
    if (previous && next && previous.bytes !== null && next.bytes !== null) deltaBytes = next.bytes - previous.bytes;
    else if (!previous && next) deltaBytes = next.bytes;
    else if (previous && !next) deltaBytes = previous.bytes === null ? null : -previous.bytes;
    if (previous && next && previous.blocks !== null && next.blocks !== null) deltaBlocks = next.blocks - previous.blocks;
    else if (!previous && next) deltaBlocks = next.blocks;
    else if (previous && !next) deltaBlocks = previous.blocks === null ? null : -previous.blocks;
    const percentBytes = previous && next
      ? allocationDiffPercent(previous.bytes, next.bytes)
      : previous && previous.bytes !== null && previous.bytes > 0 ? -100 : null;
    const percentBlocks = previous && next
      ? allocationDiffPercent(previous.blocks, next.blocks)
      : previous && previous.blocks !== null && previous.blocks > 0 ? -100 : null;
    rows.push({
      key,
      status,
      before: previous,
      after: next,
      site: next || previous,
      deltaBytes,
      deltaBlocks,
      percentBytes,
      percentBlocks,
    });
  }
  const counts = { added: 0, removed: 0, changed: 0, unchanged: 0 };
  for (const row of rows) counts[row.status] += 1;
  const changed = rows
    .filter(row => row.status !== "unchanged")
    .sort((left, right) => Math.abs(right.deltaBytes ?? 0) - Math.abs(left.deltaBytes ?? 0)
      || Math.abs(right.deltaBlocks ?? 0) - Math.abs(left.deltaBlocks ?? 0)
      || left.key.localeCompare(right.key));
  const totalBytes = {
    before: baselineReport?.sampledBytes ?? null,
    after: currentReport?.sampledBytes ?? null,
  };
  totalBytes.delta = totalBytes.before !== null && totalBytes.after !== null
    ? totalBytes.after - totalBytes.before
    : null;
  totalBytes.percent = allocationDiffPercent(totalBytes.before, totalBytes.after);
  const totalBlocks = {
    before: baselineReport?.sampledBlocks ?? null,
    after: currentReport?.sampledBlocks ?? null,
  };
  totalBlocks.delta = totalBlocks.before !== null && totalBlocks.after !== null
    ? totalBlocks.after - totalBlocks.before
    : null;
  totalBlocks.percent = allocationDiffPercent(totalBlocks.before, totalBlocks.after);
  return {
    counts,
    changed,
    duplicates: before.duplicates + after.duplicates,
    unmatchableBefore: before.unmatchable,
    unmatchableAfter: after.unmatchable,
    totalBytes,
    totalBlocks,
  };
}

function allocationDeltaBytesText(delta) {
  if (delta === null) return "—";
  if (delta === 0) return "±0 B";
  return `${delta > 0 ? "+" : "−"}${fmtBytes(Math.abs(delta))}`;
}

function allocationDeltaCountText(delta) {
  if (delta === null) return "—";
  if (delta === 0) return "±0";
  return `${delta > 0 ? "+" : "−"}${formatCount(Math.abs(delta))}`;
}

function allocationDeltaPercentText(percent) {
  if (percent === null || !Number.isFinite(percent)) return "—";
  if (Math.abs(percent) < 0.05) return "±0.0%";
  return `${percent > 0 ? "+" : "−"}${Math.abs(percent).toFixed(1)}%`;
}

function allocationDeltaClass(delta) {
  if (delta === null) return "";
  if (delta > 0) return "is-positive";
  if (delta < 0) return "is-negative";
  return "is-zero";
}

function allocationDiffSampleCell(value, present, format) {
  if (!present) {
    return el("span", {
      class: "memory-diff-absent",
      text: "—",
      attrs: { title: "Absent from this report" },
    });
  }
  if (value === null) return el("span", { text: "Not reported" });
  return el("span", {
    text: format === "bytes" ? fmtBytes(value) : formatCount(value),
    attrs: { title: format === "bytes" ? `${formatCount(value)} bytes` : `${formatCount(value)} blocks` },
  });
}

function allocationDiffStatusLabel(status) {
  return status === "added" ? "Added" : status === "removed" ? "Removed" : "Changed";
}

function allocationBaselineMeta(baseline, source) {
  const report = baseline.report;
  const sites = formatCount(report.sites.length);
  const sampled = report.sampledBytes === null ? "bytes not reported" : `${fmtBytes(report.sampledBytes)} sampled`;
  return `${source || "baseline snapshot"} · captured ${formatWallTime(baseline.capturedAt)} · 1-in-${formatCount(baseline.rate)} · ${sites} site${report.sites.length === 1 ? "" : "s"} · ${sampled}`;
}

function allocationCurrentMeta(report, allocation) {
  if (!report) return "Current report · no allocation-site report";
  const rate = allocationSiteRateValue(report.rate) ?? allocationSiteRateValue(allocation.rate);
  const sites = formatCount(report.sites.length);
  const sampled = report.sampledBytes === null ? "bytes not reported" : `${fmtBytes(report.sampledBytes)} sampled`;
  const updated = report.refreshedAt ? formatWallTime(report.refreshedAt) : "update time not reported";
  return `Current report · updated ${updated} · 1-in-${formatCount(rate ?? ALLOCATION_DEFAULT_RATE)} · ${sites} site${report.sites.length === 1 ? "" : "s"} · ${sampled}`;
}

function allocationBaselineFullTime(milliseconds) {
  try {
    return new Date(milliseconds).toLocaleString();
  } catch {
    return "timestamp not reported";
  }
}

function renderAllocationBaseline(allocation, report) {
  const baseline = allocation.baseline;
  const block = el("section", {
    class: "memory-baseline",
    attrs: { id: "memory-allocation-baseline", "aria-labelledby": "alloc-baseline-title" },
  });
  block.append(el("div", { class: "memory-baseline-head" },
    el("h3", { attrs: { id: "alloc-baseline-title" }, text: "Baseline diff" }),
    el("span", {
      class: "pane-meta",
      attrs: { title: "Baseline capture time and sampling rate" },
      text: baseline ? `Baseline ${formatWallTime(baseline.capturedAt)}` : "No baseline",
    }),
  ));
  const actions = el("div", { class: "memory-baseline-actions" });
  actions.append(el("button", {
    class: "tool-button small",
    props: { type: "button", id: "alloc-baseline-capture", disabled: !report || allocation.loading },
    attrs: { title: report ? "Store the current allocation-site report as the baseline" : "Start sampling or query a report first" },
    text: "Capture baseline",
    on: { click: () => allocationCaptureBaseline() },
  }));
  actions.append(el("button", {
    class: "tool-button small",
    props: { type: "button", id: "alloc-baseline-export", disabled: !baseline },
    attrs: { title: "Download the baseline snapshot as JSON" },
    text: "Export JSON",
    on: { click: () => allocationExportBaseline() },
  }));
  actions.append(el("label", {
    class: "tool-button small",
    attrs: { for: "alloc-baseline-import", title: "Import an allocation-site baseline snapshot JSON" },
  }, icon("upload"), "Import JSON"));
  const importInput = el("input", {
    class: "sr-only",
    attrs: {
      type: "file",
      id: "alloc-baseline-import",
      accept: ".json,application/json",
      "aria-label": "Import an allocation-site baseline snapshot JSON",
    },
  });
  importInput.addEventListener("change", () => allocationImportBaseline(importInput));
  actions.append(importInput);
  actions.append(el("button", {
    class: "tool-button small",
    props: { type: "button", id: "alloc-baseline-clear", disabled: !baseline && !allocation.baselineError },
    attrs: { title: "Discard the allocation-site baseline" },
    text: "Clear baseline",
    on: { click: () => allocationClearBaseline() },
  }));
  block.append(actions);

  if (allocation.baselineError) block.append(errorInline(allocation.baselineError));
  block.append(el("p", {
    class: "memory-note memory-baseline-scope",
    text: "Baselines compare cumulative sampled allocations at the configured 1-in-N rate, not live heap, object counts or leaks.",
  }));

  if (!baseline) {
    block.append(el("p", {
      class: "memory-baseline-empty",
      text: report
        ? "No baseline snapshot. Capture the current report or import a snapshot JSON to compare sampled sites. No site is inferred."
        : "No baseline snapshot and no current report. Start sampling to capture a baseline, or import a snapshot JSON. No site is inferred.",
    }));
    return block;
  }

  block.append(el("p", {
    class: "memory-diff-baseline",
    text: allocationBaselineMeta(baseline, allocation.baselineSource),
    attrs: { title: allocationBaselineFullTime(baseline.capturedAt) },
  }));
  block.append(el("p", {
    class: "memory-diff-current",
    text: allocationCurrentMeta(report, allocation),
    attrs: report?.refreshedAt ? { title: allocationBaselineFullTime(report.refreshedAt) } : {},
  }));

  const notices = allocationSiteTruncationNotices(baseline.report).map(text => `Baseline: ${text}`);
  if (report) {
    notices.push(...allocationSiteTruncationNotices(report).map(text => `Current: ${text}`));
  }
  const eitherTruncated = baseline.report.sitesTruncated || baseline.report.sitesOmitted > 0
    || Boolean(report && (report.sitesTruncated || report.sitesOmitted > 0));
  if (eitherTruncated) {
    notices.push("Sites omitted from either report cannot be matched; an added or removed row can reflect a truncated list rather than an allocation change.");
  }
  if (notices.length) {
    block.append(el("div", { class: "warn-inline memory-site-truncation memory-diff-truncation" },
      notices.map(text => el("p", { class: "memory-note", text }))));
  }

  if (baseline.target.id !== state.targetId) {
    block.append(allocationSiteState("error", "Baseline belongs to another target",
      `The baseline was captured on ${baseline.target.id || "an unknown target"}; this view is comparing ${state.targetId || "no target"}. No sites are compared across targets.`));
    return block;
  }

  if (!report) {
    block.append(allocationSiteState("memory", "No current report",
      "The baseline is retained, but the runtime has not returned an allocation-site report for this target. Start or query sampling to compute a diff."));
    return block;
  }

  const currentRate = allocationSiteRateValue(report.rate) ?? allocationSiteRateValue(allocation.rate);
  if (currentRate !== null && currentRate !== baseline.rate) {
    block.append(allocationSiteState("memory", "Sampling rate changed",
      `The baseline sampled every ${formatCount(baseline.rate)} allocation${baseline.rate === 1 ? "" : "s"}; the current report samples every ${formatCount(currentRate)}. Cumulative sampled bytes and blocks at different rates are not comparable, so no site diff is computed. Capture a baseline at the current rate.`,
      { action: "Capture baseline", onAction: () => allocationCaptureBaseline() }));
    return block;
  }

  const sameSession = baseline.session !== null && baseline.session === allocationSessionId(allocation);
  if (!sameSession) {
    block.append(el("div", { class: "warn-inline memory-diff-session" },
      el("p", { class: "memory-note", text: "Different sampling session. The runtime restarts its cumulative sampled counters at each Start, so a negative delta is not freed memory and a positive delta is not live growth. Deltas describe cumulative sampled allocations at the configured rate." })));
  }

  const diff = allocationSiteDiff(baseline.report, report);
  block.append(el("div", { class: "memory-diff-summary" },
    el("span", { class: "is-added", text: `${formatCount(diff.counts.added)} added` }),
    el("span", { text: " · " }),
    el("span", { class: "is-removed", text: `${formatCount(diff.counts.removed)} removed` }),
    el("span", { text: " · " }),
    el("span", { class: "is-changed", text: `${formatCount(diff.counts.changed)} changed` }),
    el("span", { text: " · " }),
    el("span", { class: "is-unchanged", text: `${formatCount(diff.counts.unchanged)} unchanged` }),
  ));

  const totalLine = (label, total, format) => {
    if (total.before === null && total.after === null) {
      return el("p", { class: "memory-note", text: `${label} is not reported in either report; no total delta is computed.` });
    }
    if (total.before === null || total.after === null) {
      return el("p", { class: "memory-note", text: `${label} is only reported in the ${total.before === null ? "current" : "baseline"} report; no total delta is computed.` });
    }
    return el("p", { class: `memory-diff-total ${allocationDeltaClass(total.delta)}` },
      `${label} ${format(total.before)} → ${format(total.after)} · ${format === fmtBytes ? allocationDeltaBytesText(total.delta) : allocationDeltaCountText(total.delta)} · ${allocationDeltaPercentText(total.percent)}`);
  };
  block.append(totalLine("Sampled bytes", diff.totalBytes, fmtBytes));
  block.append(totalLine("Sampled blocks", diff.totalBlocks, formatCount));
  if (sameSession && ((diff.totalBytes.delta ?? 0) < 0 || (diff.totalBlocks.delta ?? 0) < 0)) {
    block.append(el("p", { class: "memory-note", text: "A decrease can come from the sampler's fixed ring dropping older samples; it is not freed memory or live heap." }));
  }

  if (diff.unmatchableBefore || diff.unmatchableAfter) {
    const sides = [];
    if (diff.unmatchableBefore) sides.push(`${formatCount(diff.unmatchableBefore)} in the baseline`);
    if (diff.unmatchableAfter) sides.push(`${formatCount(diff.unmatchableAfter)} in the current report`);
    block.append(el("p", { class: "memory-note", text: `${sides.join(" and ")} have no captured stack and cannot be matched; they are excluded from this diff.` }));
  }
  if (diff.duplicates > 0) {
    block.append(el("p", { class: "memory-note", text: `${formatCount(diff.duplicates)} duplicate stack identities were combined for this diff.` }));
  }
  if (sameSession && diff.counts.removed > 0) {
    block.append(el("p", { class: "memory-note", text: "Within one sampling session a removed row means the site is absent from this report's list (for example dropped from the retained ring), not that its allocations were freed." }));
  }

  if (!diff.changed.length) {
    block.append(el("p", { class: "memory-note", text: "No added, removed or changed sampled sites between the baseline and the current report." }));
  } else {
    const tableBody = el("tbody");
    diff.changed.forEach((row, index) => {
      const rank = index + 1;
      const detailsId = `alloc-diff-stack-${rank}`;
      const expanded = allocation.diffExpanded.has(row.key);
      const top = allocationSiteTopFrame(row.site);
      const toggle = el("button", {
        class: "link-button memory-site-toggle",
        props: { type: "button" },
        attrs: {
          "aria-expanded": String(expanded),
          "aria-controls": detailsId,
          "aria-label": `${expanded ? "Hide" : "Show"} stack for ${row.status} site ${rank}`,
        },
      },
        el("span", { class: "memory-site-top-name", text: top.name }),
        top.location ? el("span", { class: "memory-site-top-location", text: top.location }) : null,
        top.address && top.name !== top.address ? el("code", { class: "memory-site-top-address", text: top.address }) : null,
      );
      const details = el("tr", { class: "memory-site-stack-row memory-diff-stack-row", attrs: { id: detailsId } },
        el("td", { attrs: { colspan: "8" } }, renderAllocationSiteStack(row.site)),
      );
      details.hidden = !expanded;
      toggle.addEventListener("click", () => {
        const show = details.hidden;
        details.hidden = !show;
        toggle.setAttribute("aria-expanded", String(show));
        toggle.setAttribute("aria-label", `${show ? "Hide" : "Show"} stack for ${row.status} site ${rank}`);
        if (show) allocation.diffExpanded.add(row.key);
        else allocation.diffExpanded.delete(row.key);
      });
      tableBody.append(el("tr", {
        class: `memory-diff-row is-${row.status}`,
        attrs: { "data-allocation-diff-status": row.status },
      },
        el("td", {}, el("span", { class: `memory-diff-status is-${row.status}`, text: allocationDiffStatusLabel(row.status) })),
        el("td", { class: "num memory-bytes" },
          allocationDiffSampleCell(row.before?.bytes ?? null, Boolean(row.before), "bytes"),
          " → ",
          allocationDiffSampleCell(row.after?.bytes ?? null, Boolean(row.after), "bytes"),
        ),
        el("td", { class: `num memory-diff-delta ${allocationDeltaClass(row.deltaBytes)}`, text: allocationDeltaBytesText(row.deltaBytes) }),
        el("td", { class: `num memory-diff-delta ${allocationDeltaClass(row.deltaBytes)}`, text: allocationDeltaPercentText(row.percentBytes) }),
        el("td", { class: "num memory-bytes" },
          allocationDiffSampleCell(row.before?.blocks ?? null, Boolean(row.before), "blocks"),
          " → ",
          allocationDiffSampleCell(row.after?.blocks ?? null, Boolean(row.after), "blocks"),
        ),
        el("td", { class: `num memory-diff-delta ${allocationDeltaClass(row.deltaBlocks)}`, text: allocationDeltaCountText(row.deltaBlocks) }),
        el("td", { class: `num memory-diff-delta ${allocationDeltaClass(row.deltaBlocks)}`, text: allocationDeltaPercentText(row.percentBlocks) }),
        el("td", { class: "memory-site-stack-cell" }, toggle),
      ));
      tableBody.append(details);
    });
    block.append(el("div", { class: "table-wrap memory-site-table-wrap memory-diff-table-wrap" },
      el("table", { class: "data-table memory-site-table memory-diff-table" },
        el("thead", {},
          el("tr", {},
            el("th", { attrs: { scope: "col" }, text: "Change" }),
            el("th", { class: "num", attrs: { scope: "col" }, text: "Sampled bytes" }),
            el("th", { class: "num", attrs: { scope: "col" }, text: "Δ bytes" }),
            el("th", { class: "num", attrs: { scope: "col" }, text: "Δ %" }),
            el("th", { class: "num", attrs: { scope: "col" }, text: "Sampled blocks" }),
            el("th", { class: "num", attrs: { scope: "col" }, text: "Δ blocks" }),
            el("th", { class: "num", attrs: { scope: "col" }, text: "Δ %" }),
            el("th", { attrs: { scope: "col" }, text: "Sampling stack" }),
          ),
        ),
        tableBody,
      )));
    if (diff.counts.added || diff.counts.removed) {
      block.append(el("p", {
        class: "memory-note",
        text: "A dash means the site was absent from that report, not zero sampled bytes. Percentages are relative to the baseline sample; absent sites have no percentage.",
      }));
    }
  }
  return block;
}

