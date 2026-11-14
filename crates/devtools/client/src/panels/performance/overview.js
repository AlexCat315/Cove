/* Performance */
import { clear, el, fmtMs, fmtNumber, numberValue } from "../../core/format.js";
import { $, ALLOCATION_DEFAULT_RATE, BUDGET_KEY, BUDGET_RATES, DEEP_LINKS_ROOT_KEY, MEMORY_INTERVALS, MEMORY_INTERVAL_KEY, PERFORMANCE_TABS, PROFILE_DEFAULT_DURATION, PROFILE_DEFAULT_HZ, PROFILE_DURATIONS, PROFILE_HZ_RATES, dom, state } from "../../core/globals.js";
import { rpc, targetById } from "../../core/session.js";
import { stopTimer, syncFollowTimers } from "../network.js";
import { frameJank, loadFrames, renderFramesPanel } from "./frames.js";
import { clearProfileTimers, ensureProfile, fmtSeconds, formatCount, profileElapsedMs } from "./profile/model.js";
import { renderProfilePanel } from "./profile/views.js";
import { renderTimelinePanel, updateTraceElapsed } from "./timeline.js";
import { ensureSceneDiagnostics } from "../scene-diagnostics.js";


function readBudgetPreference() {
  try {
    const stored = Number(localStorage.getItem(BUDGET_KEY));
    return BUDGET_RATES.includes(stored) ? stored : 60;
  } catch {
    return 60;
  }
}

export function writeBudgetPreference(rate) {
  try { localStorage.setItem(BUDGET_KEY, String(rate)); } catch {}
}

/// Adopts the display rate the runtime measured. An absent or non-positive
/// rate leaves the user's preference in charge rather than assuming 60 Hz.
export function applyRuntimeRefreshRate(perf, rate) {
  const hz = rate !== null && Number.isFinite(rate) && rate > 0 ? rate : null;
  perf.refreshHz = hz;
  if (hz !== null && !perf.budgetManual) perf.budgetHz = hz;
  syncBudgetOptions(perf);
}

/// Keeps the budget selector honest about where its rate comes from: the
/// runtime's measured display rate, or a rate the user chose by hand.
export function syncBudgetOptions(perf) {
  const select = dom.performanceBudget;
  if (!select) return;
  const rates = [...BUDGET_RATES];
  if (perf.refreshHz !== null && !rates.includes(perf.refreshHz)) rates.push(perf.refreshHz);
  rates.sort((left, right) => left - right);
  const existing = [...select.options].map(option => Number(option.value));
  if (existing.length !== rates.length || existing.some((rate, index) => rate !== rates[index])) {
    clear(select);
    for (const rate of rates) {
      select.append(el("option", {
        attrs: { value: String(rate) },
        text: `${fmtNumber(rate, rate % 1 === 0 ? 0 : 1)} Hz · ${fmtNumber(1000 / rate, 1)} ms`,
      }));
    }
  }
  select.value = String(perf.budgetHz);
  const measured = perf.refreshHz !== null && !perf.budgetManual;
  select.title = measured
    ? `The runtime measured this display's refresh rate: ${fmtNumber(perf.refreshHz, 1)} Hz. Choose another rate to override it.`
    : perf.refreshHz === null
      ? "The runtime does not publish its display refresh rate. The budget is the rate you select here."
      : `Overriding the runtime's measured ${fmtNumber(perf.refreshHz, 1)} Hz display rate.`;
}

function emptyAllocationSites() {
  return {
    loaded: false,
    loading: false,
    polling: false,
    action: null,
    error: null,
    status: "idle",
    supported: null,
    reason: "",
    rate: ALLOCATION_DEFAULT_RATE,
    report: null,
    queriedAt: 0,
    startedAt: 0,
    stoppedAt: 0,
    request: 0,
    timer: null,
    expanded: new Set(),
    sessionSeq: 0,
    baseline: null,
    baselineSource: "",
    baselineError: "",
    diffExpanded: new Set(),
  };
}

function readMemoryIntervalPreference() {
  try {
    const value = Number(localStorage.getItem(MEMORY_INTERVAL_KEY));
    if (MEMORY_INTERVALS.includes(value)) return value;
  } catch {}
  return 2000;
}

export function writeMemoryIntervalPreference(interval) {
  try {
    localStorage.setItem(MEMORY_INTERVAL_KEY, String(interval));
  } catch {}
}

export function emptyMemory() {
  return {
    loaded: false,
    loading: false,
    error: null,
    data: null,
    refreshedAt: 0,
    request: 0,
    history: [],
    follow: dom.memoryFollow?.checked === true,
    timer: null,
    intervalMs: readMemoryIntervalPreference(),
    series: { rss: true, allocated: true, caches: true, targets: true, virtual: false },
    allocation: emptyAllocationSites(),
  };
}

export function emptyAppSize() {
  return {
    loaded: false,
    loading: false,
    error: null,
    data: null,
    refreshedAt: 0,
    request: 0,
    treemapMode: "groups",
    treemapFocus: null,
    containmentOpen: {},
    baseline: null,
    baselineError: "",
  };
}

export function emptyDeepLinks() {
  return {
    root: readDeepLinksRootPreference(),
    validatedRoot: "",
    loaded: false,
    loading: false,
    error: null,
    data: null,
    refreshedAt: 0,
    request: 0,
  };
}

function readDeepLinksRootPreference() {
  try {
    return localStorage.getItem(DEEP_LINKS_ROOT_KEY) || "";
  } catch {
    return "";
  }
}

export function writeDeepLinksRootPreference(root) {
  try {
    if (root) localStorage.setItem(DEEP_LINKS_ROOT_KEY, root);
    else localStorage.removeItem(DEEP_LINKS_ROOT_KEY);
  } catch {}
}

export function emptyProfile() {
  return {
    status: "idle",
    supported: null,
    reason: "",
    error: null,
    loaded: false,
    loading: false,
    polling: false,
    request: 0,
    loadRequest: 0,
    hz: PROFILE_DEFAULT_HZ,
    durationMs: PROFILE_DEFAULT_DURATION,
    startedAt: 0,
    completedAt: 0,
    elapsedMs: 0,
    thread: "",
    // "sampler" for native stack sampling, "instrumented" for the WebAssembly
    // runtime's measured frame phases.
    source: "sampler",
    intervalMs: null,
    samplesTaken: 0,
    samplesDropped: 0,
    reportedDurationMs: null,
    samplesPerSecond: null,
    samples: [],
    samplesTruncated: false,
    retainedSamples: 0,
    stacks: [],
    stackTotalSamples: 0,
    stacksTruncated: false,
    hotspots: { loading: false, error: null, data: null },
    view: "call-tree",
    filter: "",
    selection: null,
    expanded: new Set(),
    methodSort: { key: "selfSamples", dir: -1 },
    pollTimer: null,
    stopTimer: null,
  };
}

export function emptyPerformance() {
  return {
    tab: "frames",
    budgetHz: readBudgetPreference(),
    /// The display rate the runtime published, or null when it cannot observe
    /// one. A published rate drives the budget until the user overrides it.
    refreshHz: null,
    budgetManual: false,
    frames: [],
    dropped: 0,
    loaded: false,
    loading: false,
    error: null,
    follow: dom.performanceFollow?.checked === true,
    timer: null,
    selectedFrame: null,
    analysis: { frame: null, loading: false, error: null, data: null, request: 0 },
    analysisCache: new Map(),
    recording: false,
    startedAt: 0,
    recordTimer: null,
    traceLoading: false,
    traceError: null,
    captures: [],
    activeCaptureId: "",
    eventLoading: false,
    eventError: null,
    baseline: "",
    candidate: "",
    comparison: null,
    comparing: false,
    selectedSpan: null,
    zoom: 1,
    snapshot: null,
    importError: null,
    profile: emptyProfile(),
  };
}

export function resetPerformance(previousTarget = state.targetId) {
  const previous = state.performance;
  if (previous?.recordTimer) clearInterval(previous.recordTimer);
  if (previous?.recording && previousTarget) {
    rpc("stop_ui_trace", { target: previousTarget }).catch(() => {});
  }
  if (previous?.profile?.status === "sampling" && previousTarget) {
    rpc("stop_ui_profile", { target: previousTarget }).catch(() => {});
  }
  clearProfileTimers(previous?.profile);
  stopTimer("performance");
  const budget = previous?.budgetHz ?? readBudgetPreference();
  const tab = previous?.tab ?? "frames";
  const hz = PROFILE_HZ_RATES.includes(previous?.profile?.hz) ? previous.profile.hz : PROFILE_DEFAULT_HZ;
  const durationMs = PROFILE_DURATIONS.includes(previous?.profile?.durationMs) ? previous.profile.durationMs : PROFILE_DEFAULT_DURATION;
  state.performance = emptyPerformance();
  state.performance.budgetHz = BUDGET_RATES.includes(budget) ? budget : 60;
  state.performance.tab = PERFORMANCE_TABS.includes(tab) ? tab : "frames";
  state.performance.profile.hz = hz;
  state.performance.profile.durationMs = durationMs;
  if (dom.performanceFollow) state.performance.follow = dom.performanceFollow.checked === true;
  dom.performanceMeta.textContent = "Frame timings, CPU spans and sampled CPU profiles published by the runtime.";
  if (state.view === "performance") renderPerformance();
}

export function ensurePerformance() {
  const perf = state.performance;
  if (!state.token || perf.snapshot || !state.targetId) {
    renderPerformance();
    return;
  }
  if (perf.tab === "frames" && state.caps.timing && !perf.loaded && !perf.loading && !perf.error) {
    loadFrames();
    return;
  }
  renderPerformance();
  if (perf.tab === "profile") ensureProfile();
}

export function activeCaptureRecord() {
  const perf = state.performance;
  return perf.captures.find(record => record.summary?.id === perf.activeCaptureId) || null;
}

export function captureRecordById(id) {
  return state.performance.captures.find(record => record.summary?.id === id) || null;
}

export function setPerformanceTab(tab, options = {}) {
  if (!PERFORMANCE_TABS.includes(tab)) return;
  if (state.performance.tab === tab && options.force !== true) return;
  state.performance.tab = tab;
  syncFollowTimers();
  renderPerformance();
  dom.performanceContent.scrollTop = 0;
  if (tab === "profile") ensureProfile();
  if (options.focusTab) $(`tab-performance-${tab}`)?.focus();
}

function setPerformanceChrome() {
  const perf = state.performance;
  const caps = state.caps;
  const snapshot = Boolean(perf.snapshot);
  const framesTab = perf.tab === "frames";
  const profileTab = perf.tab === "profile";
  dom.performanceBudget.value = String(perf.budgetHz);
  dom.performanceBudget.disabled = snapshot || !framesTab;
  dom.performanceFollow.checked = perf.follow;
  dom.performanceFollow.disabled = snapshot || !caps.timing || !framesTab;
  dom.performanceRefresh.disabled = snapshot || (perf.tab === "components"
    ? !caps.componentActivity : profileTab ? !caps.profile || perf.profile.loading : !caps.timing || perf.loading);
  dom.performanceRefresh.title = perf.tab === "components" ? "Refresh component activity"
    : profileTab ? "Refresh CPU profile and hot spots" : "Refresh frame timings";
  dom.performanceStart.disabled = snapshot || !caps.trace || perf.recording || perf.traceLoading;
  dom.performanceStop.disabled = snapshot || !perf.recording || perf.traceLoading;
  dom.performanceStart.title = snapshot
    ? "Return to live data before recording a trace."
    : caps.trace
      ? "Record CPU spans around a real operation"
      : "This target does not advertise trace capture.";
  dom.performanceStop.title = perf.recording ? "Stop the CPU trace" : "No trace is recording.";
  dom.performanceExport.disabled = perf.loading || perf.recording || perf.traceLoading || perf.eventLoading || !(perf.frames.length || perf.captures.length);
  dom.performanceElapsed.hidden = !perf.recording;
  dom.performanceRecordingDot.hidden = !perf.recording;
  dom.performanceSnapshotBadge.hidden = !snapshot;
  if (snapshot) dom.performanceSnapshotBadge.textContent = "Imported snapshot";
  if (perf.recording) updateTraceElapsed();
}

function updatePerformanceMeta() {
  const perf = state.performance;
  const target = targetById();
  if (perf.snapshot) {
    const spans = perf.captures.reduce((total, record) => total + (numberValue(record.summary?.eventCount) ?? record.events.length), 0);
    const parts = [`Imported ${perf.snapshot.name}`, `${perf.frames.length} frames`, `${spans} spans`];
    if (perf.snapshot.exportedAt) parts.push(`exported ${perf.snapshot.exportedAt}`);
    dom.performanceMeta.textContent = parts.join(" · ");
    return;
  }
  if (!target) {
    dom.performanceMeta.textContent = "No target selected.";
    return;
  }
  if (perf.tab === "components") {
    dom.performanceMeta.textContent = "Mounted components";
    return;
  }
  if (!state.caps.runtime) {
    dom.performanceMeta.textContent = "Performance data comes from a Cove runtime. Select a runtime target.";
    return;
  }
  if (perf.tab === "frames") {
    if (!state.caps.timing) {
      dom.performanceMeta.textContent = "This runtime does not advertise frame timing.";
      return;
    }
    if (perf.loading && !perf.loaded) {
      dom.performanceMeta.textContent = "Reading frame timings…";
      return;
    }
    if (perf.error && !perf.frames.length) {
      dom.performanceMeta.textContent = "Frame timing read failed.";
      return;
    }
    if (!perf.frames.length) {
      dom.performanceMeta.textContent = "0 frames";
      return;
    }
    const recent = perf.frames.slice(-60);
    const budget = 1000 / perf.budgetHz;
    const janks = recent.filter(frame => frameJank(frame, budget)).length;
    const parts = [`${perf.frames.length} frames`, `${janks} jank in last ${recent.length}`, `budget ${fmtNumber(budget, 1)} ms`];
    if (perf.dropped) parts.push(`${perf.dropped} dropped`);
    if (perf.loading) parts.push("updating…");
    dom.performanceMeta.textContent = parts.join(" · ");
    return;
  }
  if (perf.tab === "profile") {
    const profile = perf.profile;
    if (["unsupported", "unavailable"].includes(profile.status)) {
      dom.performanceMeta.textContent = "CPU profiling is unavailable.";
      return;
    }
    if (!state.caps.profile) {
      dom.performanceMeta.textContent = "This runtime does not advertise CPU profiling.";
      return;
    }
    const instrumented = profile.source === "instrumented";
    const count = `${formatCount(profile.samplesTaken)} ${instrumented ? "frames" : "samples"}`;
    if (profile.status === "sampling" || profile.status === "starting") {
      const parts = [
        instrumented ? "Recording frame phases" : `Sampling ${profile.thread || "the UI thread"}`,
        fmtSeconds(profileElapsedMs(profile)),
        count,
      ];
      if (profile.samplesDropped) parts.push(`${formatCount(profile.samplesDropped)} dropped`);
      dom.performanceMeta.textContent = parts.join(" · ");
      return;
    }
    if (profile.loading && !profile.loaded) {
      dom.performanceMeta.textContent = "Reading CPU profile…";
      return;
    }
    if (profile.error && !profile.samplesTaken && !profile.samples.length) {
      dom.performanceMeta.textContent = "CPU profile read failed.";
      return;
    }
    if (profile.status === "complete" || profile.samples.length || profile.samplesTaken) {
      const parts = [
        count,
        fmtSeconds(profile.elapsedMs || profile.reportedDurationMs),
        profile.thread || "unknown thread",
        instrumented ? `${formatCount(profile.retainedSamples)} ms measured` : `${formatCount(profile.stacks.length)} hot stacks`,
      ];
      if (profile.samplesDropped) parts.push(`${formatCount(profile.samplesDropped)} dropped`);
      dom.performanceMeta.textContent = parts.join(" · ");
      return;
    }
    dom.performanceMeta.textContent = "No CPU profile.";
    return;
  }
  if (!state.caps.trace) {
    dom.performanceMeta.textContent = "This runtime does not advertise trace capture.";
    return;
  }
  if (perf.recording) {
    dom.performanceMeta.textContent = perf.traceLoading ? "Stopping trace…" : "Recording CPU spans…";
    return;
  }
  if (perf.traceLoading) {
    dom.performanceMeta.textContent = "Starting trace…";
    return;
  }
  const record = activeCaptureRecord();
  if (!record) {
    dom.performanceMeta.textContent = "No trace capture.";
    return;
  }
  const summary = record.summary || {};
  const parts = [String(summary.id || "capture"), `${numberValue(summary.eventCount) ?? record.events.length} spans`, fmtMs(summary.durationMs, 1)];
  if (record.imported) parts.push("imported");
  else if (!record.complete) parts.push(`${record.events.length} loaded`);
  dom.performanceMeta.textContent = parts.join(" · ");
}

export function renderPerformance() {
  const scrollTop = dom.performanceContent.scrollTop;
  const spansScroll = state.performance.tab === "timeline"
    ? dom.performanceTimeline.querySelector(".spans-table-wrap")?.scrollTop ?? 0
    : 0;
  setPerformanceChrome();
  const perf = state.performance;
  for (const name of PERFORMANCE_TABS) {
    const button = $(`tab-performance-${name}`);
    const panel = $(`performance-${name}`);
    if (!button || !panel) continue;
    const selected = name === perf.tab;
    button.setAttribute("aria-selected", String(selected));
    button.tabIndex = selected ? 0 : -1;
    panel.hidden = !selected;
  }
  if (perf.tab === "frames") renderFramesPanel();
  else if (perf.tab === "timeline") renderTimelinePanel();
  else if (perf.tab === "profile") renderProfilePanel();
  else ensureSceneDiagnostics("components");
  updatePerformanceMeta();
  dom.performanceContent.scrollTop = scrollTop;
  const spans = dom.performanceTimeline.querySelector(".spans-table-wrap");
  if (spans) spans.scrollTop = spansScroll;
}

