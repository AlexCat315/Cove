/* Performance snapshots */
import { numberValue, slug } from "../../core/format.js";
import { BUDGET_RATES, MAX_FRAME_RECORDS, MAX_SNAPSHOT_EVENTS, PROFILE_DEFAULT_DURATION, PROFILE_DEFAULT_HZ, PROFILE_DURATIONS, PROFILE_HZ_RATES, SNAPSHOT_SCHEMA, SNAPSHOT_VERSION, dom, state } from "../../core/globals.js";
import { errorMessage, targetById, toast } from "../../core/session.js";
import { stopTimer, syncFollowTimers } from "../network.js";
import { activeCaptureRecord, emptyPerformance, emptyProfile, ensurePerformance, renderPerformance, writeBudgetPreference } from "./overview.js";
import { clearProfileTimers } from "./profile/model.js";
import { fetchTracePage } from "./timeline.js";


export function downloadJson(filename, value) {
  const blob = new Blob([JSON.stringify(value, null, 2)], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  document.body.append(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

function snapshotFilename() {
  const target = targetById();
  const name = state.performance.snapshot?.target?.title || target?.title || "snapshot";
  const stamp = new Date().toISOString().replace(/[:.]/g, "-");
  return `cove-performance-${slug(name)}-${stamp}.json`;
}

function performanceSnapshotPayload() {
  const perf = state.performance;
  const target = targetById();
  const snapshotTarget = perf.snapshot?.target || (target
    ? { id: target.id, title: target.title, platform: target.platform, capabilities: Array.isArray(target.capabilities) ? target.capabilities : [] }
    : null);
  return {
    schema: SNAPSHOT_SCHEMA,
    version: SNAPSHOT_VERSION,
    exportedAt: new Date().toISOString(),
    budgetHz: perf.budgetHz,
    target: snapshotTarget,
    frames: perf.frames,
    captures: perf.captures.map(record => ({
      capture: record.summary,
      events: record.events,
      eventsComplete: record.complete === true,
    })),
    notes: [
      "TimingFrame records carry no wall-clock timestamp; frame-to-span alignment uses each frame's timeline sequence window, not wall-clock time.",
      "Only the active capture is paged to completion on export; other captures keep their summaries and already-loaded spans.",
    ],
  };
}

export async function exportPerformance() {
  const perf = state.performance;
  if (perf.loading) {
    toast("Wait for the current frame read to finish before exporting.", "info");
    return;
  }
  if (perf.recording) {
    toast("Stop the trace before exporting.", "info");
    return;
  }
  if (!perf.frames.length && !perf.captures.length) {
    toast("Record frame timings or a trace before exporting.", "info");
    return;
  }
  const exportLabel = dom.performanceExport.querySelector("span");
  const exportText = exportLabel?.textContent || "Export";
  dom.performanceExport.disabled = true;
  dom.performanceExport.setAttribute("aria-busy", "true");
  if (exportLabel) exportLabel.textContent = "Exporting…";
  try {
    const active = activeCaptureRecord();
    if (active && !active.imported && !active.complete) {
      while (!active.complete && !active.imported && active.events.length < MAX_SNAPSHOT_EVENTS) {
        const before = active.events.length;
        const loaded = await fetchTracePage(active, { quiet: true, silent: true });
        if (!loaded) {
          throw new Error(`Could not read every span for ${active.summary?.id || "the active capture"}; export stopped. ${perf.eventError ? errorMessage(perf.eventError) : ""}`.trim());
        }
        if (active.events.length === before) break;
      }
    }
    const payload = performanceSnapshotPayload();
    downloadJson(snapshotFilename(), payload);
    toast(`Exported ${payload.frames.length} frames and ${payload.captures.length} capture${payload.captures.length === 1 ? "" : "s"}.`, "success");
  } catch (error) {
    toast(errorMessage(error), "error", 9000);
  } finally {
    dom.performanceExport.removeAttribute("aria-busy");
    if (exportLabel) exportLabel.textContent = exportText;
    dom.performanceExport.disabled = state.performance.loading || state.performance.recording || state.performance.traceLoading || state.performance.eventLoading || !(state.performance.frames.length || state.performance.captures.length);
    renderPerformance();
  }
}

function snapshotNumber(value, label, options = {}) {
  if (!Number.isFinite(value)) throw new Error(`${label} is not a finite number.`);
  if (value < 0) throw new Error(`${label} is negative.`);
  if (options.integer && !Number.isInteger(value)) throw new Error(`${label} is not an integer.`);
  return value;
}

function snapshotOptionalNumber(value, label) {
  if (value === null || value === undefined) return null;
  return snapshotNumber(value, label);
}

function validateGpuBreakdown(value, label) {
  if (value == null) return undefined;
  const { timingSource, passes, layers, passCount, layerCount } = value;
  if (typeof timingSource !== "string" || !Array.isArray(passes) || !Array.isArray(layers)
      || !passes.every(pass => pass && typeof pass === "object")
      || !layers.every(layer => layer && typeof layer === "object")) {
    throw new Error(`${label} has an invalid GPU breakdown.`);
  }
  return { timingSource, passes, layers, passCount, layerCount };
}

function validateSnapshotFrame(frame, index) {
  const label = `Frame ${index + 1}`;
  if (!frame || typeof frame !== "object" || Array.isArray(frame)) throw new Error(`${label} is not an object.`);
  const number = snapshotNumber(frame.frame, `${label} sequence`, { integer: true });
  if (typeof frame.submitted !== "boolean") throw new Error(`${label} has no submitted flag.`);
  if (frame.presented !== null && frame.presented !== undefined && typeof frame.presented !== "boolean") {
    throw new Error(`${label} has an invalid presented flag.`);
  }
  return {
    frame: number,
    recompositionMs: snapshotOptionalNumber(frame.recompositionMs, `${label} recompositionMs`),
    layoutMs: snapshotNumber(frame.layoutMs, `${label} layoutMs`),
    drawMs: snapshotNumber(frame.drawMs, `${label} drawMs`),
    renderThreadMs: snapshotOptionalNumber(frame.renderThreadMs, `${label} renderThreadMs`),
    gpuMs: snapshotOptionalNumber(frame.gpuMs, `${label} gpuMs`),
    gpuBreakdown: validateGpuBreakdown(frame.gpuBreakdown, label),
    pipelineCreation: frame.pipelineCreation == null ? null : {
      count: snapshotNumber(frame.pipelineCreation.count, `${label} pipeline creations`, { integer: true }),
      cpuMs: snapshotNumber(frame.pipelineCreation.cpuMs, `${label} pipeline creation CPU ms`),
    },
    submitted: frame.submitted,
    presented: frame.presented === true ? true : frame.presented === false ? false : null,
    totalMs: snapshotNumber(frame.totalMs, `${label} totalMs`),
    nodeCount: snapshotNumber(frame.nodeCount, `${label} nodeCount`, { integer: true }),
    batchCount: snapshotNumber(frame.batchCount, `${label} batchCount`, { integer: true }),
    timelineStart: frame.timelineStart === undefined ? 0 : snapshotNumber(frame.timelineStart, `${label} timelineStart`, { integer: true }),
    timelineEnd: frame.timelineEnd === undefined ? 0 : snapshotNumber(frame.timelineEnd, `${label} timelineEnd`, { integer: true }),
  };
}

function validateSnapshotEvent(event, label) {
  if (!event || typeof event !== "object" || Array.isArray(event)) throw new Error(`${label} is not an object.`);
  const sequence = numberValue(event.sequence);
  if (sequence === null) throw new Error(`${label} has no finite sequence.`);
  const phase = typeof event.phase === "string" ? event.phase : "";
  if (!phase) throw new Error(`${label} has no phase.`);
  const start = numberValue(event.start_ms ?? event.startMs);
  const duration = numberValue(event.duration_ms ?? event.durationMs);
  if (start === null) throw new Error(`${label} has no finite start_ms.`);
  if (duration === null) throw new Error(`${label} has no finite duration_ms.`);
  const scope = event.scope_id ?? event.scopeID ?? null;
  if (scope !== null && typeof scope !== "string") throw new Error(`${label} has an invalid scope_id.`);
  return {
    sequence,
    phase,
    name: typeof event.name === "string" ? event.name : "",
    scope_id: scope,
    start_ms: start,
    duration_ms: duration,
    reasons: Array.isArray(event.reasons) ? event.reasons : [],
  };
}

function validateSnapshotCapture(entry, index) {
  const label = `Capture ${index + 1}`;
  if (!entry || typeof entry !== "object" || Array.isArray(entry)) throw new Error(`${label} is not an object.`);
  const summary = entry.capture;
  if (!summary || typeof summary !== "object" || Array.isArray(summary)) throw new Error(`${label} has no capture summary.`);
  if (typeof summary.id !== "string" || !summary.id) throw new Error(`${label} has no capture id.`);
  const durationMs = snapshotNumber(numberValue(summary.durationMs), `${label} durationMs`);
  const eventCount = snapshotNumber(numberValue(summary.eventCount), `${label} eventCount`, { integer: true });
  const dropped = summary.dropped === undefined ? 0 : snapshotNumber(numberValue(summary.dropped), `${label} dropped`, { integer: true });
  const phases = [];
  if (summary.phases !== undefined && !Array.isArray(summary.phases)) throw new Error(`${label} has an invalid phases list.`);
  for (const [phaseIndex, phase] of (summary.phases || []).entries()) {
    if (!phase || typeof phase !== "object" || typeof phase.phase !== "string") throw new Error(`${label} phase ${phaseIndex + 1} has no name.`);
    phases.push({
      phase: phase.phase,
      count: snapshotNumber(numberValue(phase.count), `${label} phase ${phase.phase} count`, { integer: true }),
      averageMs: snapshotNumber(numberValue(phase.averageMs ?? phase.average_ms), `${label} phase ${phase.phase} averageMs`),
      p95Ms: snapshotNumber(numberValue(phase.p95Ms ?? phase.p95_ms), `${label} phase ${phase.phase} p95Ms`),
      maximumMs: snapshotNumber(numberValue(phase.maximumMs ?? phase.maximum_ms), `${label} phase ${phase.phase} maximumMs`),
    });
  }
  if (entry.events !== undefined && !Array.isArray(entry.events)) throw new Error(`${label} has an invalid events list.`);
  if ((entry.events || []).length > MAX_SNAPSHOT_EVENTS) {
    throw new Error(`${label} contains ${entry.events.length} spans; this client reads at most ${MAX_SNAPSHOT_EVENTS}. Export fewer spans.`);
  }
  const normalized = (entry.events || []).map((event, eventIndex) => validateSnapshotEvent(event, `${label} event ${eventIndex + 1}`));
  const eventsComplete = entry.eventsComplete === true;
  if (eventsComplete && normalized.length < eventCount) {
    throw new Error(`${label} declares complete events but contains ${normalized.length} of ${eventCount} spans.`);
  }
  return {
    summary: { id: summary.id, durationMs, eventCount, dropped, phases },
    events: normalized,
    total: eventCount,
    nextOffset: null,
    complete: eventsComplete && normalized.length >= eventCount,
    imported: true,
    loading: false,
  };
}

function parsePerformanceSnapshot(text) {
  let data;
  try {
    data = JSON.parse(text);
  } catch {
    throw new Error("The file is not valid JSON.");
  }
  if (!data || typeof data !== "object" || Array.isArray(data)) throw new Error("The snapshot must be a JSON object.");
  if (data.schema !== SNAPSHOT_SCHEMA) throw new Error(`Not a Cove performance snapshot (schema ${JSON.stringify(data.schema ?? null)}).`);
  if (data.version !== SNAPSHOT_VERSION) throw new Error(`Unsupported snapshot version ${JSON.stringify(data.version)}; this client reads version ${SNAPSHOT_VERSION}.`);
  if (!Array.isArray(data.frames)) throw new Error("The snapshot has no frames array.");
  if (!Array.isArray(data.captures)) throw new Error("The snapshot has no captures array.");
  if (data.frames.length > MAX_FRAME_RECORDS) {
    throw new Error(`The snapshot contains ${data.frames.length} frames; this client reads at most ${MAX_FRAME_RECORDS}.`);
  }
  const frames = data.frames.map(validateSnapshotFrame);
  if (new Set(frames.map(frame => frame.frame)).size !== frames.length) throw new Error("The snapshot contains duplicate frame sequences.");
  const captures = data.captures.map(validateSnapshotCapture);
  if (new Set(captures.map(record => record.summary.id)).size !== captures.length) throw new Error("The snapshot contains duplicate capture ids.");
  if (!frames.length && !captures.length) throw new Error("The snapshot contains no frames or trace captures.");
  return {
    frames,
    captures,
    budgetHz: BUDGET_RATES.includes(Number(data.budgetHz)) ? Number(data.budgetHz) : null,
    target: data.target && typeof data.target === "object" ? data.target : null,
    exportedAt: typeof data.exportedAt === "string" ? data.exportedAt : "",
  };
}

export async function importPerformance(file) {
  if (!file) return;
  const perf = state.performance;
  if (perf.recording) {
    toast("Stop the active trace before importing a snapshot.", "error");
    return;
  }
  if (perf.profile.status === "sampling" || perf.profile.status === "starting" || perf.profile.status === "stopping") {
    toast("Stop the active CPU profile before importing a snapshot.", "error");
    return;
  }
  if (perf.loading) {
    toast("Wait for the current frame read to finish before importing.", "info");
    return;
  }
  if (perf.traceLoading || perf.eventLoading || perf.captures.some(record => record.loading)) {
    toast("Wait for the current trace operation to finish before importing.", "info");
    return;
  }
  const importLabel = dom.performanceImport.querySelector("span");
  const importText = importLabel?.textContent || "Import";
  dom.performanceImport.disabled = true;
  dom.performanceImport.setAttribute("aria-busy", "true");
  if (importLabel) importLabel.textContent = "Reading…";
  perf.importError = null;
  try {
    toast(`Reading ${file.name || "snapshot"}…`, "info", 2000);
    const snapshot = parsePerformanceSnapshot(await file.text());
    perf.frames = snapshot.frames;
    perf.dropped = 0;
    perf.loaded = true;
    perf.loading = false;
    perf.error = null;
    perf.selectedFrame = null;
    perf.analysis = { frame: null, loading: false, error: null, data: null, request: perf.analysis.request + 1 };
    perf.analysisCache.clear();
    perf.captures = snapshot.captures;
    perf.activeCaptureId = snapshot.captures[0]?.summary.id || "";
    perf.baseline = snapshot.captures[0]?.summary.id || "";
    perf.candidate = snapshot.captures[snapshot.captures.length - 1]?.summary.id || "";
    perf.comparison = null;
    perf.selectedSpan = null;
    perf.zoom = 1;
    perf.eventError = null;
    perf.eventLoading = false;
    perf.traceError = null;
    perf.traceLoading = false;
    perf.recording = false;
    clearProfileTimers(perf.profile);
    const profileHz = perf.profile.hz;
    const profileDuration = perf.profile.durationMs;
    perf.profile = emptyProfile();
    perf.profile.hz = profileHz;
    perf.profile.durationMs = profileDuration;
    if (snapshot.budgetHz) {
      perf.budgetHz = snapshot.budgetHz;
      writeBudgetPreference(snapshot.budgetHz);
    }
    perf.snapshot = {
      name: file.name || "snapshot.json",
      exportedAt: snapshot.exportedAt,
      target: snapshot.target,
    };
    perf.tab = "frames";
    stopTimer("performance");
    renderPerformance();
    dom.performanceContent.scrollTop = 0;
    toast(`Imported ${perf.frames.length} frame${perf.frames.length === 1 ? "" : "s"} and ${perf.captures.length} capture${perf.captures.length === 1 ? "" : "s"}.`, "success");
  } catch (error) {
    perf.importError = error;
    toast(`Import failed: ${error.message || error}`, "error", 10000);
    renderPerformance();
  } finally {
    dom.performanceImport.removeAttribute("aria-busy");
    dom.performanceImport.disabled = false;
    if (importLabel) importLabel.textContent = importText;
    dom.performanceFile.value = "";
  }
}

export function returnToLive() {
  const perf = state.performance;
  const budget = perf.budgetHz;
  const tab = perf.tab;
  const hz = PROFILE_HZ_RATES.includes(perf.profile.hz) ? perf.profile.hz : PROFILE_DEFAULT_HZ;
  const durationMs = PROFILE_DURATIONS.includes(perf.profile.durationMs) ? perf.profile.durationMs : PROFILE_DEFAULT_DURATION;
  clearProfileTimers(perf.profile);
  stopTimer("performance");
  state.performance = emptyPerformance();
  state.performance.budgetHz = budget;
  state.performance.tab = tab;
  state.performance.profile.hz = hz;
  state.performance.profile.durationMs = durationMs;
  state.performance.follow = dom.performanceFollow.checked === true;
  renderPerformance();
  ensurePerformance();
  syncFollowTimers();
}


