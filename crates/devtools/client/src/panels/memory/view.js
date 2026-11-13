import { clear, el, fmtBytes, fmtEpochMs, icon } from "../../core/format.js";
import { MAX_MEMORY_COUNTERS, RpcError, dom, state } from "../../core/globals.js";
import { errorMessage, rpc, targetById, toast, unwrapData } from "../../core/session.js";
import { emptyInline, errorInline, showState } from "../../core/ui-helpers.js";
import { renderAllocationBaseline } from "./baselines.js";
import { allocationSiteFailureKind, allocationSiteMeta, allocationSiteMetric, allocationSiteRateControl, allocationSiteScopeNote, allocationSiteState, allocationSiteStatusClass, allocationSiteStatusLabel, allocationSiteTruncationNotices, ensureAllocationSites, queryAllocationSites, releaseAllocationSampling, renderAllocationSiteTable, resetAllocationSites, startAllocationSites, stopAllocationPolling, stopAllocationSites } from "./sampling.js";
import { MEMORY_ALLOCATION_ROWS, MEMORY_BYTE_LABELS, MEMORY_ENTRY_LABELS, MEMORY_PROCESS_COUNTERS, formatWallTime, memoryCounterValue, memorySizeClassLabel, memorySizeClassSortValue, memoryWindowTotals, normalizeMemorySnapshot, recordMemorySample, sumMemoryCounters } from "./snapshot.js";
import { stopTimer } from "../network.js";
import { emptyMemory } from "../performance/overview.js";
import { fmtSeconds, formatCount } from "../performance/profile/model.js";

function renderAllocationSitesBody(allocation, report) {
  const failure = allocationSiteFailureKind(allocation);
  const body = el("div", { class: "memory-site-body" });
  if (failure === "offline") {
    body.append(allocationSiteState("error", "Disconnected", allocation.reason || "The controller connection dropped. Sampling state is unknown until it reconnects."));
  } else if (failure === "unsupported") {
    body.append(allocationSiteState("memory", "Allocation sampling is unavailable", allocation.reason || "The runtime reports that stack capture is not available on this target. No sites are substituted."));
  } else if (failure === "unavailable") {
    body.append(allocationSiteState("memory", "Allocation sites are unavailable", allocation.reason || "This controller does not expose allocation-site sampling. No sites are substituted."));
  } else if (failure === "error") {
    body.append(allocationSiteState("error", "Allocation-site read failed", allocation.reason || errorMessage(allocation.error), {
      action: "Retry",
      onAction: () => queryAllocationSites({ force: true }),
    }));
  } else if (allocation.status === "starting") {
    body.append(allocationSiteState("loading", "Starting allocation sampling", "Waiting for the runtime to enable cove_alloc's sampler. No sites are shown until the runtime reports them."));
  } else if (allocation.status === "stopping") {
    body.append(allocationSiteState("loading", "Stopping allocation sampling", "Waiting for the runtime to aggregate the retained stacks."));
  } else if (allocation.status === "idle") {
    body.append(allocationSiteState("memory", "Not sampling", "Start enables the runtime's opt-in sampler. Sites appear only after allocations are sampled; nothing is inferred while it is off."));
  } else if (allocation.status === "running" && !report) {
    body.append(allocationSiteState("memory", "Sampling is running", "No counters have arrived yet. Query to read the sites collected so far; allocations made before Start are not sampled."));
  } else if (allocation.status === "running" && report && report.sites.length === 0) {
    body.append(allocationSiteState("memory", "No sampled allocations yet", "Sampling is running. Allocations made after Start appear here once the sampler captures a matching stack; no site is inferred."));
  } else if (allocation.status === "stopped" && !report) {
    body.append(allocationSiteState("memory", "No allocation-site report", allocation.reason || "Sampling stopped without returning a retained report. No sites are substituted."));
  } else if (allocation.status === "stopped" && report && report.sites.length === 0) {
    body.append(allocationSiteState("memory", "No retained allocation sites", "Sampling stopped without retaining a matching site. The counters below are the sampler's own attempts, drops and capture failures."));
  } else if (report && report.sites.length > 0) {
    body.append(renderAllocationSiteTable(report, allocation));
  }
  if (report) {
    const notices = allocationSiteTruncationNotices(report);
    if (notices.length) {
      body.prepend(el("div", { class: "warn-inline memory-site-truncation" },
        notices.map(text => el("p", { class: "memory-note", text }))));
    }
    if (failure || allocation.status === "running" || allocation.status === "stopped") {
      body.append(el("div", { class: "metrics memory-site-metrics" },
        allocationSiteMetric("Sampled bytes", report.sampledBytes, fmtBytes, "Cumulative bytes on sampled stacks for this session at the configured rate; not live heap."),
        allocationSiteMetric("Sampled blocks", report.sampledBlocks, formatCount, "Cumulative blocks on sampled stacks for this session at the configured rate; not live heap."),
        allocationSiteMetric("Attempted", report.samplesAttempted, formatCount, "Sampling triggers: every Nth successful fresh allocation."),
        allocationSiteMetric("Retained", report.samplesRetained, formatCount, "Samples retained in the sampler's fixed ring."),
        allocationSiteMetric("Dropped", report.samplesDropped, formatCount, "Samples overwritten, lost to a slot race or rejected as incomplete."),
        allocationSiteMetric("Capture failures", report.captureFailures, formatCount, "Sampling triggers where stack capture produced no caller frame."),
      ));
    }
  }
  return body;
}

function renderMemoryAllocationSites() {
  const allocation = state.memory.allocation;
  const report = allocation.report;
  const target = targetById();
  const offline = Boolean(state.token && target && state.caps.runtime && !state.connected);
  const busy = allocation.loading && !allocation.polling;
  const running = ["running", "starting", "stopping"].includes(allocation.status);
  const failure = allocationSiteFailureKind(allocation);
  const controlsBlocked = offline || busy || failure === "unsupported" || failure === "unavailable";
  const start = el("button", {
    class: "tool-button is-primary",
    props: { type: "button" },
    text: "Start",
    attrs: { title: "Enable cove_alloc's sampler in the runtime" },
    on: { click: () => startAllocationSites() },
  });
  start.disabled = controlsBlocked || running;
  const stopButton = el("button", {
    class: "tool-button",
    props: { type: "button" },
    text: "Stop",
    attrs: { title: "Stop sampling and collect the retained sites" },
    on: { click: () => stopAllocationSites() },
  });
  stopButton.disabled = offline || busy || !["running", "starting"].includes(allocation.status);
  const query = el("button", {
    class: "tool-button",
    props: { type: "button" },
    text: "Query",
    attrs: { title: "Read the sites collected so far without ending the session" },
    on: { click: () => queryAllocationSites({ force: true }) },
  });
  query.disabled = offline || busy;
  const reset = el("button", {
    class: "tool-button",
    props: { type: "button" },
    text: "Clear",
    attrs: { title: "Discard the stopped report from this view; while sampling, ask the controller to clear its retained sites" },
    on: { click: () => resetAllocationSites() },
  });
  reset.disabled = offline || busy || (!report && !running);
  const showBaseline = !["unsupported", "unavailable"].includes(allocation.status) || Boolean(allocation.baseline);
  return el("div", { class: "panel-block memory-allocation-sites", attrs: { id: "memory-allocation-sites" } },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Allocation sites" }),
      el("span", { class: `state-badge ${allocationSiteStatusClass(allocation)}`, text: allocationSiteStatusLabel(allocation) }),
      el("span", { class: "pane-meta", attrs: { title: "Sampling rate and the runtime's retained sample count" }, text: allocationSiteMeta(allocation) }),
    ),
    el("div", { class: "panel-block-body" },
      allocationSiteScopeNote(),
      el("div", { class: "memory-site-toolbar" },
        allocationSiteRateControl(allocation, controlsBlocked || running),
        el("div", { class: "profile-actions" }, start, stopButton, query, reset),
      ),
      allocation.error && failure === "error" ? errorInline(errorMessage(allocation.error)) : null,
      renderAllocationSitesBody(allocation, report),
      showBaseline ? renderAllocationBaseline(allocation, report) : null,
      el("p", { class: "memory-note", text: "Sampled bytes and blocks are cumulative for this session at the configured 1-in-N rate. They are not live heap, object counts, object types, retaining paths or leaks. The sampler counts fresh allocations only; reallocations are not sampled." }),
    ),
  );
}

export function renderAllocationSitesView(options = {}) {
  if (state.view !== "memory") return;
  const host = dom.memoryContent;
  const current = host.querySelector("#memory-allocation-sites");
  if (!current) {
    renderMemory();
    return;
  }
  if (options.preserveFocus && current.contains(document.activeElement)) return;
  const scrollTop = host.scrollTop;
  current.replaceWith(renderMemoryAllocationSites());
  host.scrollTop = scrollTop;
}

export function refreshMemoryView() {
  loadMemory({ reset: false });
  queryAllocationSites({ quiet: true });
}

function emptyMemoryState(message) {
  return el("div", { class: "state" },
    el("span", { class: "state-icon" }, icon("memory")),
    el("strong", { text: "No memory counters" }),
    el("p", { text: message }),
  );
}

function memoryScopeNote() {
  return el("div", { class: "memory-scope" },
    icon("info"),
    el("span", {},
      el("strong", { text: "Allocator tracking, not a heap object graph." }),
      " Process RSS and process virtual are real readings of the host process. Allocator counters, when the host installs a tracking allocator, are real totals from that allocator and cover allocated bytes and block counts only. Rust has no GC, so no GC generations, retaining paths or leak tracking are reported, and object types are not tracked. Cache and render-target values are host estimates; drawable pools, driver memory and external scene textures are excluded.",
    ),
  );
}

function renderMemoryAllocator(snapshot) {
  const allocation = snapshot.allocation;
  const block = el("div", { class: "panel-block memory-allocation" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Allocator tracking" }),
      el("span", {
        class: "pane-meta",
        text: allocation.reported ? `${allocation.reported} of ${MEMORY_ALLOCATION_ROWS.length * 2} reported` : "Not reported",
      }),
    ),
  );
  if (!allocation.reported) {
    block.append(el("div", { class: "panel-block-body" },
      el("p", { class: "memory-note", text: "The host installs no tracking allocator, so allocated bytes, block counts and size classes are not reported. RSS, cache and render values below are separate host readings and are not reused as allocation totals." }),
    ));
    return block;
  }
  const rows = allocation.rows.map(row => el("tr", {},
    el("td", { text: row.label, attrs: { title: row.description } }),
    el("td", { class: "num memory-bytes", text: row.bytes === null ? "Not reported" : fmtBytes(row.bytes) }),
    el("td", { class: "num memory-bytes", text: row.blocks === null ? "Not reported" : formatCount(row.blocks) }),
  ));
  block.append(el("div", { class: "table-wrap" }, el("table", { class: "data-table" },
    el("thead", {}, el("tr", {},
      el("th", { text: "Measure" }),
      el("th", { class: "num", text: "Bytes" }),
      el("th", { class: "num", text: "Blocks" }),
    )),
    el("tbody", {}, ...rows),
  )));
  return block;
}

function renderMemorySizeClasses(snapshot) {
  const entries = snapshot.sizeClasses;
  if (!entries.length) return null;
  const sorted = entries.slice().sort((left, right) => {
    const leftSize = memorySizeClassSortValue(left.label);
    const rightSize = memorySizeClassSortValue(right.label);
    if (leftSize !== rightSize) return leftSize < rightSize ? -1 : 1;
    return left.label.localeCompare(right.label);
  });
  const limit = 16;
  const shown = sorted.slice(0, limit);
  const showBlocks = shown.some(entry => entry.blocks !== null);
  const showBytes = shown.some(entry => entry.bytes !== null);
  return el("div", { class: "panel-block memory-size-classes" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Allocator size classes" }),
      el("span", { class: "pane-meta", text: `${sorted.length} class${sorted.length === 1 ? "" : "es"}` }),
    ),
    el("div", { class: "table-wrap" }, el("table", { class: "data-table" },
      el("thead", {}, el("tr", {},
        el("th", { text: "Size class" }),
        showBlocks ? el("th", { class: "num", text: "Live blocks" }) : null,
        showBytes ? el("th", { class: "num", text: "Live bytes" }) : null,
      )),
      el("tbody", {}, ...shown.map(entry => el("tr", {},
        el("td", { class: "memory-size-class", text: memorySizeClassLabel(entry.label) }),
        showBlocks ? el("td", { class: "num memory-bytes", text: entry.blocks === null ? "Not reported" : formatCount(entry.blocks) }) : null,
        showBytes ? el("td", { class: "num memory-bytes", text: entry.bytes === null ? "Not reported" : fmtBytes(entry.bytes) }) : null,
      ))),
    )),
    sorted.length > limit
      ? el("div", { class: "panel-block-body" },
        el("p", { class: "memory-note", text: `Showing ${limit} of ${sorted.length} size classes.` }))
      : null,
  );
}

function renderMemoryStats(snapshot) {
  const counters = snapshot.counters;
  const windowCount = snapshot.windows.size;
  const cacheBytes = sumMemoryCounters(counters, ["shapingBytes", "layoutBytes", "imageBytes"]);
  const targetBytes = memoryWindowTotals(snapshot.windows, "targetBytes");
  const msaaAllocated = memoryWindowTotals(snapshot.windows, "msaaAllocatedBytes");
  const msaaLogical = memoryWindowTotals(snapshot.windows, "msaaLogicalBytes");
  const rss = memoryCounterValue(counters, "processResidentBytes");
  const virtualBytes = memoryCounterValue(counters, "processVirtualBytes");
  return el("div", { class: "stat-row" },
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Process RSS" }),
      el("div", { class: "stat-value" }, rss === null ? "Not reported" : fmtBytes(rss))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Process virtual" }),
      el("div", { class: "stat-value" }, virtualBytes === null ? "Not reported" : fmtBytes(virtualBytes))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Logical caches" }),
      el("div", { class: "stat-value" }, fmtBytes(cacheBytes), el("small", { text: " estimated" }))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Image assets" }),
      el("div", { class: "stat-value" }, formatCount(memoryCounterValue(counters, "imageEntries")))) ,
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Render targets" }),
      el("div", { class: "stat-value" }, fmtBytes(targetBytes), el("small", { text: " estimated" }))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "MSAA allocated" }),
      el("div", { class: "stat-value" }, msaaAllocated === null ? "Not reported" : fmtBytes(msaaAllocated))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "MSAA logical" }),
      el("div", { class: "stat-value" }, fmtBytes(msaaLogical))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Windows" }),
      el("div", { class: "stat-value" }, String(windowCount))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Skipped parents" }),
      el("div", { class: "stat-value" }, formatCount(memoryCounterValue(counters, "skippedParentUpdates")))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Memory pressure" }),
      el("div", {
        class: "stat-value",
        attrs: {
          title: snapshot.pressure === null
            ? "The host reported no memory-pressure field."
            : snapshot.pressure.subscribed
              ? "The host subscribes to the platform memory-pressure signal."
              : "The host never wired the platform memory-pressure signal; this is not a measured normal.",
        },
      }, snapshot.pressure === null ? "Not reported" : snapshot.pressure.subscribed ? (snapshot.pressure.low ? "Low" : "Normal") : "Not observed")),
  );
}

function renderMemoryProcess(snapshot) {
  const counters = snapshot.counters;
  let reported = 0;
  const rows = MEMORY_PROCESS_COUNTERS.map(([key, label]) => {
    const bytes = memoryCounterValue(counters, key);
    if (bytes !== null) reported += 1;
    return el("tr", {},
      el("td", { text: label, attrs: { title: MEMORY_BYTE_LABELS[key][1] } }),
      el("td", { class: "num memory-bytes", text: bytes === null ? "Not reported" : fmtBytes(bytes) }),
    );
  });
  return el("div", { class: "panel-block" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Host process" }),
      el("span", { class: "pane-meta", text: `${reported} of ${MEMORY_PROCESS_COUNTERS.length} reported` }),
    ),
    el("div", { class: "table-wrap" }, el("table", { class: "data-table" },
      el("thead", {}, el("tr", {},
        el("th", { text: "Measure" }),
        el("th", { class: "num", text: "Bytes" }),
      )),
      el("tbody", {}, ...rows),
    )),
  );
}

function renderMemoryCounters(snapshot) {
  const counters = snapshot.counters;
  const rows = [];
  const addRow = (label, entries, bytes, description) => {
    rows.push(el("tr", {},
      el("td", { text: label, attrs: { title: description || "" } }),
      el("td", { class: "num", text: entries === null ? "—" : formatCount(entries) }),
      el("td", { class: "num memory-bytes", text: bytes === null ? "—" : fmtBytes(bytes) }),
    ));
  };
  addRow("Shaping cache", memoryCounterValue(counters, "shapingEntries"), memoryCounterValue(counters, "shapingBytes"), MEMORY_BYTE_LABELS.shapingBytes[1]);
  addRow("Text layout cache", memoryCounterValue(counters, "layoutEntries"), memoryCounterValue(counters, "layoutBytes"), MEMORY_BYTE_LABELS.layoutBytes[1]);
  addRow("Image cache", memoryCounterValue(counters, "imageEntries"), memoryCounterValue(counters, "imageBytes"), MEMORY_BYTE_LABELS.imageBytes[1]);
  const known = new Set(["shapingEntries", "shapingBytes", "layoutEntries", "layoutBytes", "imageEntries", "imageBytes", "skippedParentUpdates", "processResidentBytes", "processVirtualBytes"]);
  for (const name of snapshot.allocation.consumed) known.add(name);
  const other = Object.entries(counters).filter(([key]) => !known.has(key));
  if (other.length) {
    for (const [key, value] of other.slice(0, MAX_MEMORY_COUNTERS)) {
      const label = MEMORY_ENTRY_LABELS[key] || key;
      addRow(label, value, key.toLowerCase().includes("bytes") ? value : null, `Reported counter ${key}.`);
    }
  }
  return el("div", { class: "panel-block" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Cache and render counters" }),
      el("span", { class: "pane-meta", text: `${rows.length} counters` }),
    ),
    el("div", { class: "table-wrap" }, el("table", { class: "data-table" },
      el("thead", {}, el("tr", {},
        el("th", { text: "Counter" }),
        el("th", { class: "num", text: "Entries" }),
        el("th", { class: "num", text: "Bytes" }),
      )),
      el("tbody", {}, ...rows),
    )),
  );
}

function renderMemoryWindows(snapshot) {
  const ids = [...snapshot.windows.keys()];
  if (!ids.length) return null;
  return el("div", { class: "panel-block" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Per-window render targets" }),
      el("span", { class: "pane-meta", text: `${ids.length} window${ids.length === 1 ? "" : "s"}` }),
    ),
    el("div", { class: "table-wrap" }, el("table", { class: "data-table" },
      el("thead", {}, el("tr", {},
        el("th", { text: "Window" }),
        el("th", { class: "num", text: "Target estimate" }),
        el("th", { class: "num", text: "MSAA logical" }),
        el("th", { class: "num", text: "MSAA allocated" }),
        el("th", { class: "num", text: "Samples" }),
        el("th", { text: "State" }),
      )),
      el("tbody", {}, ...ids.map(id => {
        const counters = snapshot.windows.get(id);
        const suspended = memoryCounterValue(counters, "suspended");
        const swapchain = memoryCounterValue(counters, "hasSwapchain");
        const state = suspended === 1 ? "Suspended" : swapchain === 0 ? "No swapchain" : "Active";
        return el("tr", {},
          el("td", { class: "memory-window-id", text: id, attrs: { title: id } }),
          el("td", { class: "num memory-bytes", text: fmtBytes(memoryCounterValue(counters, "targetBytes")) }),
          el("td", { class: "num memory-bytes", text: fmtBytes(memoryCounterValue(counters, "msaaLogicalBytes")) }),
          el("td", { class: "num memory-bytes", text: fmtBytes(memoryCounterValue(counters, "msaaAllocatedBytes")) }),
          el("td", { class: "num", text: formatCount(memoryCounterValue(counters, "samples")) }),
          el("td", { text: state }),
        );
      })),
    )),
  );
}

function setMemoryChrome() {
  const target = targetById();
  const memory = state.memory;
  dom.memoryRefresh.disabled = !state.token || !state.caps.runtime || memory.loading;
  dom.memoryFollow.disabled = !state.token || !state.connected || !state.caps.runtime;
  dom.memoryInterval.disabled = dom.memoryFollow.disabled;
  dom.memoryInterval.value = String(memory.intervalMs);
  if (!state.token || !target) {
    dom.memoryMeta.textContent = "No target selected.";
    return;
  }
  if (!state.caps.runtime) {
    dom.memoryMeta.textContent = "Memory counters come from a Cove runtime. Select a runtime target.";
    return;
  }
  if (memory.loading) {
    dom.memoryMeta.textContent = "Reading memory counters…";
    return;
  }
  if (memory.error && !memory.loaded) {
    dom.memoryMeta.textContent = "Memory counter read failed.";
    return;
  }
  if (memory.data === null) {
    dom.memoryMeta.textContent = "This host has no memory provider. No process or cache readings are available.";
    return;
  }
  const snapshot = normalizeMemorySnapshot(memory.data);
  if (!snapshot) {
    dom.memoryMeta.textContent = "The host returned no counter object: no process or cache readings.";
    return;
  }
  const cacheBytes = sumMemoryCounters(snapshot.counters, ["shapingBytes", "layoutBytes", "imageBytes"]);
  const rss = memoryCounterValue(snapshot.counters, "processResidentBytes");
  const parts = [];
  if (rss !== null) parts.push(`${fmtBytes(rss)} RSS`);
  const allocation = snapshot.allocation;
  const currentBytes = allocation.rows.find(row => row.label === "Current")?.bytes ?? null;
  if (currentBytes !== null) parts.push(`${fmtBytes(currentBytes)} allocated`);
  parts.push(`${fmtBytes(cacheBytes)} in logical caches`);
  parts.push(`${snapshot.windows.size} window${snapshot.windows.size === 1 ? "" : "s"}`);
  parts.push(`updated ${formatWallTime(memory.refreshedAt)}`);
  dom.memoryMeta.textContent = allocation.reported
    ? `${parts.join(" · ")}. RSS and allocator counters are measured; caches are estimates.`
    : `${parts.join(" · ")}. RSS is measured; no allocator counters are reported; caches are estimates.`;
}

const MEMORY_SERIES = [
  { key: "rss", label: "Process RSS" },
  { key: "allocated", label: "Allocator current" },
  { key: "caches", label: "Logical caches" },
  { key: "targets", label: "Render targets" },
  { key: "virtual", label: "Process virtual" },
];

function memoryChartScale(values) {
  const max = values.reduce((maximum, value) => Math.max(maximum, value), 0);
  if (max <= 0) return 1024;
  const base = 10 ** Math.floor(Math.log10(max));
  for (const step of [1, 2, 5, 10]) {
    if (step * base >= max) return step * base;
  }
  return 10 * base;
}

function memorySeriesPoints(history, key, scale, origin, span) {
  const points = [];
  for (const sample of history) {
    const value = sample[key];
    if (value === null) continue;
    points.push({
      x: span > 0 ? ((sample.at - origin) / span) * 100 : 0,
      y: Math.max(0, Math.min(100, (value / scale) * 100)),
      sample,
    });
  }
  return points;
}

function renderMemoryChart(memory) {
  const history = memory.history;
  const reset = el("button", {
    class: "tool-button",
    attrs: { type: "button", title: "Discard the recorded samples and start a new series" },
    text: "Reset",
    on: {
      click: () => {
        memory.history = [];
        renderMemory();
      },
    },
  });
  const head = el("div", { class: "panel-block-head" }, el("h2", { text: "Memory overview" }));
  if (!history.length) {
    head.append(reset);
    return el("div", { class: "panel-block" }, head,
      el("div", { class: "panel-block-body" },
        emptyInline("No samples recorded yet. Refresh or enable Follow to append provider readings to the series."),
      ),
    );
  }
  const origin = history[0].at;
  const latest = history[history.length - 1].at;
  const span = latest - origin;
  const visible = MEMORY_SERIES.filter(series => memory.series[series.key]);
  const values = [];
  for (const series of visible) {
    for (const sample of history) {
      if (sample[series.key] !== null) values.push(sample[series.key]);
    }
  }
  const scale = memoryChartScale(values);
  head.append(
    el("span", { class: "pane-meta", text: `${history.length} sample${history.length === 1 ? "" : "s"} · ${fmtSeconds(span)} · ${fmtBytes(scale)} scale` }),
    reset,
  );
  const plot = el("div", {
    class: "memory-chart-plot",
    attrs: {
      role: "img",
      "aria-label": `Memory overview: ${history.length} samples over ${fmtSeconds(span)}, scale ${fmtBytes(scale)}.`,
    },
  });
  for (const fraction of [0.25, 0.5, 0.75, 1]) {
    plot.append(el("div", { class: "memory-grid", attrs: { style: `bottom:${fraction * 100}%` } }));
  }
  for (const series of visible) {
    const points = memorySeriesPoints(history, series.key, scale, origin, span);
    if (!points.length) continue;
    if (points.length === 1) {
      plot.append(el("div", {
        class: `memory-dot is-${series.key}`,
        attrs: {
          style: `left:${points[0].x}%;bottom:${points[0].y}%`,
          title: `${series.label} ${fmtBytes(points[0].sample[series.key])} at ${fmtEpochMs(points[0].sample.at)}`,
        },
      }));
      continue;
    }
    const forward = points.map(point => `${point.x}% ${100 - point.y}%`).join(", ");
    plot.append(el("div", {
      class: `memory-area is-${series.key}`,
      attrs: { style: `clip-path:polygon(${forward}, ${points[points.length - 1].x}% 100%, ${points[0].x}% 100%)` },
    }));
    const backward = points.slice().reverse().map(point => `${point.x}% calc(${100 - point.y}% - 2px)`).join(", ");
    plot.append(el("div", {
      class: `memory-line is-${series.key}`,
      attrs: { style: `clip-path:polygon(${forward}, ${backward})` },
    }));
  }
  let pressureMarks = 0;
  for (const sample of history) {
    if (!sample.pressure?.subscribed || !sample.pressure.low) continue;
    pressureMarks += 1;
    const x = span > 0 ? ((sample.at - origin) / span) * 100 : 0;
    plot.append(el("div", {
      class: "memory-pressure",
      attrs: { style: `left:${x}%`, title: `Memory-pressure signal at ${fmtEpochMs(sample.at)}` },
    }));
  }
  const cursor = el("div", { class: "memory-cursor", props: { hidden: true } });
  plot.append(cursor);
  const readout = el("div", { class: "memory-chart-readout", attrs: { role: "status" } });
  const describe = sample => `${fmtEpochMs(sample.at)} · ` + visible
    .map(series => `${series.label} ${sample[series.key] === null ? "not reported" : fmtBytes(sample[series.key])}`)
    .join(" · ");
  readout.textContent = describe(history[history.length - 1]);
  plot.addEventListener("pointermove", event => {
    const rect = plot.getBoundingClientRect();
    if (rect.width <= 0) return;
    const ratio = Math.max(0, Math.min(1, (event.clientX - rect.left) / rect.width));
    const at = origin + ratio * span;
    let nearest = history[0];
    for (const sample of history) {
      if (Math.abs(sample.at - at) < Math.abs(nearest.at - at)) nearest = sample;
    }
    cursor.hidden = false;
    cursor.style.left = `${span > 0 ? ((nearest.at - origin) / span) * 100 : 0}%`;
    readout.textContent = describe(nearest);
  });
  plot.addEventListener("pointerleave", () => {
    cursor.hidden = true;
    readout.textContent = describe(history[history.length - 1]);
  });
  const yaxis = el("div", { class: "memory-yaxis" },
    ...[0, 0.25, 0.5, 0.75, 1].map(fraction => el("span", {
      attrs: { style: `bottom:${fraction * 100}%` },
      text: fmtBytes(scale * fraction),
    })),
  );
  const legend = el("div", { class: "perf-legend memory-legend" },
    ...MEMORY_SERIES.map(series => el("button", {
      class: `memory-legend-toggle is-${series.key}${memory.series[series.key] ? " is-on" : ""}`,
      attrs: { type: "button", "aria-pressed": String(memory.series[series.key]), title: `Show or hide ${series.label} on the overview chart` },
      text: series.label,
      on: {
        click: () => {
          memory.series[series.key] = !memory.series[series.key];
          renderMemory();
        },
      },
    })),
    el("span", { class: "is-pressure", text: "Pressure signal" }),
  );
  const notes = [
    "Each point is one query_ui_memory reading. Lines connect reported readings; a gap is never filled with an invented sample.",
    "RSS covers the whole process. Allocator current, logical caches and render targets are subsets or estimates, drawn as separate series and never stacked into a total.",
  ];
  if (visible.some(series => history.some(sample => sample[series.key] === null))) {
    notes.push("A series with unreported readings plots only the readings the host returned.");
  }
  notes.push(pressureMarks
    ? "Vertical marks are the host's subscribed memory-pressure signal at the sample that observed it."
    : "No subscribed memory-pressure signal was observed in these samples, so no pressure marks are drawn.");
  if (history.length === 1) {
    notes.push("One sample: a trend needs at least two readings, so each series is drawn as a single point.");
  }
  return el("div", { class: "panel-block" }, head,
    el("div", { class: "panel-block-body" },
      el("div", { class: "memory-chart" },
        yaxis,
        plot,
        el("div", { class: "memory-xaxis" },
          el("span", { text: fmtEpochMs(origin) }),
          el("span", { text: fmtSeconds(span) }),
          el("span", { text: fmtEpochMs(latest) }),
        ),
      ),
      readout,
      legend,
      ...notes.map(note => el("div", { class: "perf-note", text: note })),
    ),
  );
}

export function renderMemory() {
  const host = dom.memoryContent;
  clear(host);
  setMemoryChrome();
  const memory = state.memory;
  const target = targetById();
  if (!state.token) {
    showState(host, { icon: "device", title: "Not connected", message: "Connect to a controller to read memory counters." });
    return;
  }
  if (!target) {
    showState(host, { icon: "device", title: "No target", message: "Select a connected Cove runtime to read memory counters." });
    return;
  }
  if (!state.caps.runtime) {
    showState(host, { icon: "memory", title: "Memory counters are unavailable", message: "Memory counters come from a Cove runtime. Select a runtime target." });
    return;
  }
  if (memory.loading && !memory.loaded) {
    showState(host, { kind: "loading", title: "Reading memory counters", message: "Waiting for the runtime's query_ui_memory response." });
    return;
  }
  if (memory.error && !memory.loaded) {
    showState(host, {
      kind: "error",
      title: "Memory counter read failed",
      message: errorMessage(memory.error),
      action: "Retry",
      onAction: () => loadMemory({ reset: true }),
    });
    return;
  }
  if (memory.data === null) {
    showState(host, {
      icon: "memory",
      title: "This host has no memory provider",
      message: "The runtime returned null for query_ui_memory. Process RSS, cache counters and render-target estimates are all unavailable; nothing is substituted.",
      action: "Refresh",
      onAction: () => loadMemory({ reset: true }),
    });
    return;
  }
  const snapshot = normalizeMemorySnapshot(memory.data);
  if (!snapshot) {
    showState(host, {
      icon: "memory",
      title: "No memory counters",
      message: "The host returned a response without a counters object. No process or cache readings are shown.",
      action: "Refresh",
      onAction: () => loadMemory({ reset: true }),
    });
    return;
  }
  host.append(memoryScopeNote());
  if (memory.error) host.append(errorInline(errorMessage(memory.error)));
  host.append(renderMemoryChart(memory));
  host.append(renderMemoryStats(snapshot));
  host.append(renderMemoryAllocator(snapshot));
  host.append(renderMemoryAllocationSites());
  const sizeClasses = renderMemorySizeClasses(snapshot);
  if (sizeClasses) host.append(sizeClasses);
  host.append(renderMemoryProcess(snapshot));
  const counterCount = Object.keys(snapshot.counters).length;
  if (counterCount) host.append(renderMemoryCounters(snapshot));
  const windows = renderMemoryWindows(snapshot);
  if (windows) host.append(windows);
  if (!counterCount && !snapshot.windows.size) {
    const block = el("div");
    block.append(emptyMemoryState("The host returned empty counters and no windows. No process reading is substituted."));
    host.append(block);
  } else {
    host.append(el("div", { class: "perf-note", text: "Process RSS and process virtual are read from the operating system for the host process. They cover the whole process, not the UI scene, and an unreported reading is shown as Not reported rather than as zero." }));
    host.append(el("div", { class: "perf-note", text: "targetBytes is the host's own estimate for owned MSAA, backdrop and effect scratch. msaaAllocatedBytes is reported only when the backend exposes the allocation; absent values are shown as not reported, never estimated here." }));
    host.append(el("div", { class: "perf-note", text: "Refresh reads a fresh provider snapshot. Allocator counters are point-in-time allocation totals; RSS and virtual are process readings. None of these is a heap object graph, and Rust reports no GC generations or retaining paths." }));
  }
}

export async function loadMemory(options = {}) {
  const memory = state.memory;
  if (!state.token || !state.targetId || !state.caps.runtime) {
    renderMemory();
    return;
  }
  if (memory.loading) return;
  const targetId = state.targetId;
  const request = memory.request + 1;
  memory.request = request;
  memory.loading = true;
  memory.error = null;
  if (options.reset) {
    memory.data = null;
    memory.loaded = false;
  }
  renderMemory();
  try {
    const data = unwrapData(await rpc("query_ui_memory", { target: targetId }));
    if (request !== memory.request || targetId !== state.targetId) return;
    state.memory.loaded = true;
    state.memory.data = data === null ? null : data;
    state.memory.refreshedAt = Date.now();
    recordMemorySample(data);
    if (options.quiet !== true) toast("Memory counters refreshed.", "success", 3000);
  } catch (error) {
    if (request !== memory.request || targetId !== state.targetId) return;
    state.memory.error = error;
    if (error?.code === "unknown_command") {
      state.memory.error = new RpcError("This controller does not expose query_ui_memory.", "unknown_command");
    }
    if (options.quiet !== true) toast(errorMessage(error), "error", 8000);
  } finally {
    if (request === memory.request && targetId === state.targetId) {
      state.memory.loading = false;
      renderMemory();
    }
  }
}

export function ensureMemory() {
  if (!state.token || !state.targetId || !state.caps.runtime) {
    renderMemory();
    return;
  }
  if (!state.memory.loaded && !state.memory.loading) {
    loadMemory({ quiet: true });
  } else {
    renderMemory();
  }
  ensureAllocationSites();
}

export function resetMemory(previousTarget = state.targetId) {
  const previousAllocation = state.memory?.allocation;
  stopAllocationPolling();
  stopTimer("memory");
  if (previousTarget && previousAllocation && ["running", "starting", "stopping"].includes(previousAllocation.status)) {
    releaseAllocationSampling(previousTarget).catch(() => {});
  }
  const request = state.memory.request + 1;
  state.memory = emptyMemory();
  state.memory.request = request;
  if (state.view === "memory") renderMemory();
}

