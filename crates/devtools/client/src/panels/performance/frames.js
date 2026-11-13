/* Frames */
import { clear, el, fmtMs, fmtNumber, icon, numberValue, shortId, textOf } from "../../core/format.js";
import { $, FRAME_TABLE_LIMIT, FRAME_WINDOW, MAX_FRAME_RECORDS, dom, state } from "../../core/globals.js";
import { errorMessage, isDisconnectedError, rpc, targetById, toast } from "../../core/session.js";
import { emptyInline, errorInline, kvList, metric, rawDetails, showState } from "../../core/ui-helpers.js";
import { applyRuntimeRefreshRate, renderPerformance, setPerformanceTab } from "./overview.js";
import { returnToLive } from "./snapshots.js";
import { refreshCurrent } from "../../shell/views.js";


function frameParts(frame) {
  return {
    recomposition: numberValue(frame?.recompositionMs),
    layout: numberValue(frame?.layoutMs),
    draw: numberValue(frame?.drawMs),
    encode: numberValue(frame?.renderThreadMs),
    // GPU execution overlaps the CPU phases and has its own budget comparison.
    gpu: numberValue(frame?.gpuMs),
    total: numberValue(frame?.totalMs),
  };
}

function frameUiMs(frame) {
  const parts = frameParts(frame);
  if (parts.recomposition === null && parts.layout === null && parts.draw === null) return null;
  return (parts.recomposition ?? 0) + (parts.layout ?? 0) + (parts.draw ?? 0);
}

export function frameJank(frame, budget) {
  return frameCost(frame) > budget;
}

function frameCost(frame) {
  const { encode, gpu } = frameParts(frame);
  return Math.max(frameUiMs(frame) ?? 0, encode ?? 0, gpu ?? 0);
}

export async function loadFrames(options = {}) {
  const quiet = options.quiet === true;
  const perf = state.performance;
  if (perf.snapshot || !state.caps.timing || !state.targetId || perf.loading) return;
  const targetId = state.targetId;
  const epoch = state.targetEpoch;
  if (options.reset) {
    perf.frames = [];
    perf.dropped = 0;
    perf.loaded = false;
  }
  perf.loading = true;
  perf.error = null;
  renderPerformance();
  try {
    const after = perf.frames.reduce((maximum, frame) => Math.max(maximum, numberValue(frame.frame) ?? 0), 0);
    const params = { target: targetId };
    if (after > 0) params.after = after;
    const data = await rpc("record_ui_timings", params);
    if (perf.snapshot || epoch !== state.targetEpoch || targetId !== state.targetId) return;
    const incoming = Array.isArray(data?.frames) ? data.frames : [];
    const merged = new Map(perf.frames.map(frame => [frame.frame, frame]));
    for (const frame of incoming) merged.set(frame.frame, frame);
    let frames = [...merged.values()].sort((left, right) => (numberValue(left.frame) ?? 0) - (numberValue(right.frame) ?? 0));
    if (frames.length > MAX_FRAME_RECORDS) frames = frames.slice(-MAX_FRAME_RECORDS);
    perf.frames = frames;
    perf.dropped = Math.max(perf.dropped, numberValue(data?.dropped) ?? 0);
    applyRuntimeRefreshRate(perf, numberValue(data?.refreshRate));
    perf.loaded = true;
    perf.error = null;
    if (perf.selectedFrame !== null && !frames.some(frame => frame.frame === perf.selectedFrame)) {
      perf.selectedFrame = null;
      perf.analysis = { frame: null, loading: false, error: null, data: null, request: perf.analysis.request + 1 };
    }
  } catch (error) {
    if (perf.snapshot || epoch !== state.targetEpoch || targetId !== state.targetId) return;
    perf.error = error;
    if (!quiet) toast(errorMessage(error), "error", 8000);
  } finally {
    if (epoch === state.targetEpoch && targetId === state.targetId) {
      perf.loading = false;
      renderPerformance();
    }
  }
}

export function renderFramesPanel() {
  const host = dom.performanceFrames;
  clear(host);
  const perf = state.performance;
  const target = targetById();
  if (perf.snapshot) {
    host.append(snapshotBanner());
    if (!perf.frames.length) {
      const block = el("div");
      showState(block, {
        icon: "timer",
        title: "No frames in this snapshot",
        message: "The imported file contains no TimingFrame records. Its trace captures are available on the Timeline tab.",
        action: "Open timeline",
        onAction: () => setPerformanceTab("timeline"),
      });
      host.append(block);
      return;
    }
  } else if (!target) {
    showState(host, { icon: "device", title: "No target", message: "Select a connected target to read frame timings." });
    return;
  } else if (!state.caps.runtime) {
    showState(host, {
      icon: "timer",
      title: "Frame timings are unavailable",
      message: "Frame timing comes from a Cove runtime. Select a runtime target.",
    });
    return;
  } else if (!state.caps.timing) {
    showState(host, {
      icon: "timer",
      title: "Frame timings are unavailable",
      message: "This runtime does not advertise frame timing. CPU spans may still be available on the Timeline tab.",
    });
    return;
  }
  if (perf.loading && !perf.loaded) {
    showState(host, { kind: "loading", title: "Reading frame timings", message: "Waiting for the runtime's TimingFrame records." });
    return;
  }
  if (perf.error && !perf.frames.length) {
    showState(host, {
      kind: "error",
      title: "Frame timing read failed",
      message: errorMessage(perf.error),
      action: "Retry",
      onAction: () => isDisconnectedError(perf.error) ? refreshCurrent() : loadFrames({ reset: true }),
    });
    return;
  }
  if (!perf.frames.length) {
    showState(host, {
      icon: "timer",
      title: "No frame samples",
      message: "The runtime has not submitted frames with timing data yet. Frame timings come from the runtime probe and do not require Screen Recording.",
      action: "Refresh",
      onAction: () => loadFrames({ reset: true }),
    });
    return;
  }
  if (perf.error) host.append(errorInline(errorMessage(perf.error)));
  if (perf.recording) {
    host.append(el("div", { class: "perf-banner", attrs: { role: "status" } },
      el("span", { class: "recording-label", text: "Live" }),
      el("span", { text: "Recording CPU spans. Stop the trace in the header when the operation is complete; the result opens on the Timeline tab." }),
    ));
  }
  const budget = 1000 / perf.budgetHz;
  host.append(renderFrameStats(perf.frames, budget));
  host.append(renderFrameChart(perf.frames, budget));
  host.append(renderFrameTable(perf.frames, budget));
  host.append(renderFrameAnalysisBlock());
  renderFrameAnalysis();
  if (perf.selectedFrame !== null && !perf.snapshot && state.caps.frameAnalysis && perf.analysis.frame !== perf.selectedFrame && !perf.analysis.loading) {
    loadFrameAnalysis(perf.selectedFrame);
  }
}

export function snapshotBanner() {
  const perf = state.performance;
  const snapshot = perf.snapshot;
  const source = snapshot?.target?.title || "";
  return el("div", { class: "snapshot-banner", attrs: { role: "status" } },
    icon("info"),
    el("span", {},
      el("strong", { text: snapshot.name }),
      ` · ${perf.frames.length} frames · ${perf.captures.length} capture${perf.captures.length === 1 ? "" : "s"}`,
      source ? ` from ${source}` : "",
      snapshot.exportedAt ? ` · exported ${snapshot.exportedAt}` : "",
    ),
    el("span", { class: "muted", text: "Imported files are read-only; frame records are shown exactly as exported." }),
    el("button", {
      class: "tool-button",
      props: { type: "button" },
      text: "Return to live",
      on: { click: returnToLive },
    }),
  );
}

function averageOf(values) {
  return values.length ? values.reduce((total, value) => total + value, 0) / values.length : null;
}

function percentileOf(values, fraction) {
  if (!values.length) return null;
  const sorted = [...values].sort((left, right) => left - right);
  const index = Math.min(sorted.length - 1, Math.max(0, Math.ceil(fraction * sorted.length) - 1));
  return sorted[index];
}

function renderFrameStats(frames, budget) {
  const recent = frames.slice(-60);
  const janks = recent.filter(frame => frameJank(frame, budget)).length;
  const uiValues = recent.map(frameUiMs).filter(value => value !== null);
  const totals = recent.map(frame => numberValue(frame.totalMs)).filter(value => value !== null);
  const latest = frames[frames.length - 1];
  const stats = [
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: `Jank · last ${recent.length}` }),
      el("div", { class: "stat-value" }, String(janks), el("small", { text: ` / ${recent.length} frames` }))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Average UI" }),
      el("div", { class: "stat-value" }, fmtNumber(averageOf(uiValues), 2), el("small", { text: " ms" }))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Average total" }),
      el("div", { class: "stat-value" }, fmtNumber(averageOf(totals), 2), el("small", { text: " ms" }))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "P95 total" }),
      el("div", { class: "stat-value" }, fmtNumber(percentileOf(totals, 0.95), 2), el("small", { text: " ms" }))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Latest" }),
      el("div", { class: "stat-value" }, fmtNumber(numberValue(latest.totalMs), 2), el("small", { text: " ms" }))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: `Budget · ${state.performance.budgetHz} Hz` }),
      el("div", { class: "stat-value" }, fmtNumber(budget, 1), el("small", { text: " ms" }))),
  ];
  return el("div", { class: "stat-row" }, ...stats);
}

function barPercent(value, scale) {
  return Math.max(1.2, Math.min(100, (value / scale) * 100));
}

function frameUiBar(parts, ui, scale) {
  const bar = el("div", { class: "frame-bar is-ui", attrs: { style: `height:${barPercent(ui, scale)}%` } });
  for (const [name, value] of [["recomposition", parts.recomposition], ["layout", parts.layout], ["draw", parts.draw]]) {
    if (value === null || value <= 0) continue;
    bar.append(el("span", { class: `frame-seg is-${name}`, attrs: { style: `flex:${value} 1 0` } }));
  }
  return bar;
}

function renderFrameChart(frames, budget) {
  const recent = frames.slice(-FRAME_WINDOW);
  const values = [];
  for (const frame of recent) {
    const parts = frameParts(frame);
    const ui = frameUiMs(frame);
    if (ui !== null) values.push(ui);
    if (parts.encode !== null) values.push(parts.encode);
    if (parts.gpu !== null) values.push(parts.gpu);
    if (parts.total !== null) values.push(parts.total);
  }
  const scale = Math.max(values.reduce((maximum, value) => Math.max(maximum, value), 0), budget * 1.25, 1);
  const threshold = Math.max(0, Math.min(100, (budget / scale) * 100));
  const selected = state.performance.selectedFrame;
  const columns = el("div", { class: "frame-columns" });
  let focusIndex = selected === null ? recent.length - 1 : recent.findIndex(frame => frame.frame === selected);
  if (focusIndex < 0) focusIndex = recent.length - 1;
  for (const [index, frame] of recent.entries()) {
    const parts = frameParts(frame);
    const ui = frameUiMs(frame);
    const jank = frameJank(frame, budget);
    const isSelected = frame.frame === selected;
    const describe = `Frame ${frame.frame}: total ${fmtMs(parts.total, 2)}, UI ${fmtMs(ui, 2)}, encode ${fmtMs(parts.encode, 2)}, GPU ${fmtMs(parts.gpu, 2)}${jank ? ", over budget" : ""}`;
    const column = el("div", {
      class: `frame-column${jank ? " is-jank" : ""}${isSelected ? " is-selected" : ""}`,
      attrs: {
        role: "option",
        "aria-selected": String(isSelected),
        tabindex: isSelected || index === focusIndex ? "0" : "-1",
        "aria-label": describe,
        title: describe,
        "data-frame": String(frame.frame),
      },
      on: { click: () => selectFrame(frame.frame) },
    }, el("div", { class: "frame-bars" }));
    const bars = column.firstElementChild;
    if (ui !== null) bars.append(frameUiBar(parts, ui, scale));
    if (parts.encode !== null) bars.append(el("div", { class: "frame-bar is-encode", attrs: { style: `height:${barPercent(parts.encode, scale)}%` } }));
    if (parts.gpu !== null) bars.append(el("div", { class: "frame-bar is-gpu", attrs: { style: `height:${barPercent(parts.gpu, scale)}%` } }));
    if (parts.total !== null) bars.append(el("div", { class: "frame-bar is-total", attrs: { style: `height:${barPercent(parts.total, scale)}%` } }));
    columns.append(column);
  }
  const plot = el("div", {
    class: "frame-plot",
    attrs: {
      role: "listbox",
      "aria-label": `Frame timings for the last ${recent.length} frames. Budget ${fmtNumber(budget, 1)} milliseconds.`,
    },
  }, columns);
  plot.addEventListener("keydown", event => {
    const current = event.target.closest?.(".frame-column");
    if (!current) return;
    const options = [...columns.children];
    const index = options.indexOf(current);
    let next = null;
    if (event.key === "ArrowRight") next = options[Math.min(options.length - 1, index + 1)];
    else if (event.key === "ArrowLeft") next = options[Math.max(0, index - 1)];
    else if (event.key === "Home") next = options[0];
    else if (event.key === "End") next = options[options.length - 1];
    else if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      selectFrame(Number(current.dataset.frame), { focus: true });
      return;
    }
    if (!next) return;
    event.preventDefault();
    selectFrame(Number(next.dataset.frame), { focus: true });
  });
  const missingRecomposition = recent.some(frame => frameParts(frame).recomposition === null);
  const missingEncode = recent.some(frame => frameParts(frame).encode === null);
  const notes = [];
  if (missingRecomposition) notes.push("Recomposition is not reported by this runtime; UI bars contain layout and draw only.");
  if (missingEncode) notes.push("Encode bars appear only for frames whose presentation the host observed.");
  const hasGpu = recent.some(frame => frameParts(frame).gpu !== null);
  return el("div", { class: "panel-block" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Frame chart" }),
      el("span", { class: "pane-meta", text: `last ${recent.length} of ${frames.length} · ${fmtNumber(scale, 1)} ms scale` }),
    ),
    el("div", { class: "panel-block-body" },
      el("div", { class: "frame-chart" },
        el("div", { class: "frame-chart-plot" },
          el("div", { class: "frame-threshold", attrs: { style: `bottom:${threshold}%` } }, el("span", { text: `${fmtNumber(budget, 1)} ms` })),
          plot,
        ),
      ),
      el("div", { class: "frame-axis" }, el("span", { text: "0 ms" }), el("span", { text: `${fmtNumber(scale, 1)} ms` })),
      el("div", { class: "perf-legend" },
        el("span", { class: "is-budget", text: `${fmtNumber(budget, 1)} ms budget` }),
        el("span", { class: "is-ui", text: "UI" }),
        el("span", { class: "is-encode", text: "Encode" }),
        hasGpu ? el("span", { class: "is-gpu", text: "GPU" }) : null,
        el("span", { class: "is-total", text: "Total" }),
        el("span", { class: "is-jank", text: "Over budget" }),
      ),
      ...notes.map(note => el("div", { class: "perf-note", text: note })),
    ),
  );
}

function selectFrame(frameNumber, options = {}) {
  const perf = state.performance;
  if (!Number.isFinite(frameNumber)) return;
  perf.selectedFrame = frameNumber;
  updateFrameSelection();
  if (options.focus) {
    dom.performanceFrames.querySelector(`.frame-column[data-frame="${frameNumber}"]`)?.focus();
  }
  renderFrameAnalysis();
  if (!perf.snapshot && state.caps.frameAnalysis) loadFrameAnalysis(frameNumber);
}

function updateFrameSelection() {
  const selected = state.performance.selectedFrame;
  for (const node of dom.performanceFrames.querySelectorAll("[data-frame]")) {
    const isSelected = Number(node.dataset.frame) === selected;
    node.classList.toggle("is-selected", isSelected);
    if (node.getAttribute("role") === "option") {
      node.setAttribute("aria-selected", String(isSelected));
      node.tabIndex = isSelected ? 0 : -1;
    }
  }
}

function renderFrameTable(frames, budget) {
  const performance = state.performance;
  const order = performance.frameOrder ?? "latest";
  const rows = (order === "worst"
    ? frames.toSorted((left, right) => frameCost(right) - frameCost(left) || right.frame - left.frame)
    : frames.slice().reverse()).slice(0, FRAME_TABLE_LIMIT);
  const selected = state.performance.selectedFrame;
  const header = el("tr", {},
    el("th", { class: "num", text: "Frame" }),
    el("th", { class: "num", text: "UI" }),
    el("th", { class: "num", text: "Encode" }),
    el("th", { class: "num", text: "GPU" }),
    el("th", { class: "num", text: "Total" }),
    el("th", { class: "num", text: "Recomp" }),
    el("th", { class: "num", text: "Layout" }),
    el("th", { class: "num", text: "Draw" }),
    el("th", { class: "num", text: "Nodes" }),
    el("th", { class: "num", text: "Batches" }),
    el("th", { text: "Submitted" }),
    el("th", { text: "Presented" }),
  );
  const body = el("tbody");
  for (const frame of rows) {
    const parts = frameParts(frame);
    const ui = frameUiMs(frame);
    const jank = frameJank(frame, budget);
    const isSelected = frame.frame === selected;
    body.append(el("tr", {
      class: `frame-row${isSelected ? " is-selected" : ""}${jank ? " is-jank" : ""}`,
      attrs: { "data-frame": String(frame.frame), title: `Frame ${frame.frame}` },
      on: { click: () => selectFrame(frame.frame) },
    },
      el("td", { class: "num mono-cell", text: `#${frame.frame}` }),
      el("td", { class: "num", text: ui === null ? "not reported" : fmtNumber(ui, 2) }),
      el("td", { class: "num", text: parts.encode === null ? "not reported" : fmtNumber(parts.encode, 2) }),
      el("td", { class: "num", text: parts.gpu === null ? "not reported" : fmtNumber(parts.gpu, 2) }),
      el("td", { class: "num", text: fmtNumber(parts.total, 2) }),
      el("td", { class: "num", text: parts.recomposition === null ? "not reported" : fmtNumber(parts.recomposition, 2) }),
      el("td", { class: "num", text: fmtNumber(parts.layout, 2) }),
      el("td", { class: "num", text: fmtNumber(parts.draw, 2) }),
      el("td", { class: "num", text: fmtNumber(numberValue(frame.nodeCount), 0) }),
      el("td", { class: "num", text: fmtNumber(numberValue(frame.batchCount), 0) }),
      el("td", { text: frame.submitted ? "yes" : "no" }),
      el("td", { text: frame.presented === null || frame.presented === undefined ? "not observed" : frame.presented ? "yes" : "no" }),
    ));
  }
  return el("div", { class: "panel-block" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Frames" }),
      el("span", { class: "pane-meta", text: `${rows.length} of ${frames.length}` }),
      el("select", {
        attrs: { "aria-label": "Frame order" },
        on: { change: event => {
          performance.frameOrder = event.target.value;
          renderPerformance();
          document.querySelector('[aria-label="Frame order"]')?.focus();
        } },
      }, [["latest", "Newest first"], ["worst", "Slowest stage first"]].map(([value, text]) =>
        el("option", { attrs: { value }, props: { selected: order === value }, text }))),
    ),
    el("div", { class: "table-wrap" }, el("table", { class: "data-table frame-table" }, el("thead", {}, header), body)),
  );
}

function renderFrameAnalysisBlock() {
  return el("div", { class: "panel-block" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Frame analysis" }),
      el("span", { class: "pane-meta", attrs: { id: "frame-analysis-meta" } }),
    ),
    el("div", { class: "panel-block-body", attrs: { id: "frame-analysis-body" } }),
  );
}

function perfChip(text, tone) {
  return el("span", { class: `perf-chip is-${tone}`, text });
}

function frameAnalysisAttribution() {
  const perf = state.performance;
  const wrap = el("div", { class: "perf-attribution" });
  if (perf.snapshot) {
    wrap.append(emptyInline("Imported snapshot: per-span attribution was not part of this file. The exported TimingFrame records are shown above."));
    return wrap;
  }
  if (!state.caps.frameAnalysis) {
    wrap.append(emptyInline("Per-span attribution is unavailable: this target does not advertise frame_analysis. The runtime publishes TimingFrame fields without a span breakdown, so this panel can show only measured frame totals."));
    return wrap;
  }
  const analysis = perf.analysis;
  if (analysis.frame !== perf.selectedFrame) {
    wrap.append(emptyInline("Waiting for the runtime's frame analysis."));
    return wrap;
  }
  if (analysis.loading) {
    wrap.append(el("div", { class: "inline-spinner" }, el("span", { class: "spinner" }), "Reading frame attribution…"));
    return wrap;
  }
  if (analysis.error) {
    wrap.append(errorInline(errorMessage(analysis.error)));
    wrap.append(el("button", {
      class: "tool-button",
      props: { type: "button" },
      text: "Retry",
      on: { click: () => loadFrameAnalysis(perf.selectedFrame) },
    }));
    return wrap;
  }
  if (!analysis.data || typeof analysis.data !== "object") {
    wrap.append(emptyInline("The runtime returned no frame analysis."));
    return wrap;
  }
  const data = analysis.data;
  const status = data.status === "unavailable" ? "unavailable" : data.status === "ok" ? "ok" : null;
  const reason = typeof data.reason === "string" ? data.reason : "";
  const phases = Array.isArray(data.phases) ? data.phases : [];
  const spans = Array.isArray(data.spans) ? data.spans : [];
  const window = data.window && typeof data.window === "object" ? data.window : null;
  const chips = el("div", { class: "perf-verdicts" },
    status === "unavailable" ? perfChip("Attribution unavailable", "bad") : status === "ok" ? perfChip("Attributed from retained spans", "good") : null,
    data.dominantPhase ? perfChip(`Dominant: ${data.dominantPhase}`, "accent") : null,
    window ? perfChip(`Sequences ${textOf(window.startSequence)}–${textOf(window.endSequence)}`, "muted") : null,
  );
  if (chips.childElementCount) wrap.append(chips);
  if (status === "unavailable" || reason) {
    wrap.append(emptyInline(reason || "The runtime reported that this frame cannot be attributed to retained spans."));
  }
  if (phases.length) {
    const table = el("table", { class: "data-table" },
      el("thead", {}, el("tr", {},
        el("th", { text: "Phase" }),
        el("th", { class: "num", text: "Spans" }),
        el("th", { class: "num", text: "CPU" }),
        el("th", { class: "num", text: "Maximum" }),
        el("th", { class: "num", text: "Share" }),
      )),
      el("tbody", {}, ...phases.map(phase => {
        const share = numberValue(phase.share);
        return el("tr", {},
          el("td", { class: "mono-cell", text: textOf(phase.phase) }),
          el("td", { class: "num", text: fmtNumber(numberValue(phase.spanCount), 0) }),
          el("td", { class: "num", text: fmtMs(numberValue(phase.cpuMs), 2) }),
          el("td", { class: "num", text: fmtMs(numberValue(phase.maximumMs), 2) }),
          el("td", { class: "num", text: share === null ? "—" : `${(share * 100).toFixed(1)}%` }),
        );
      })),
    );
    wrap.append(el("div", { class: "table-wrap" }, table));
    wrap.append(el("div", { class: "perf-note", text: "Phase CPU time is summed from the retained spans in this frame's window; the chart's Encode bar is the host's render-thread measurement." }));
  }
  if (spans.length) {
    const table = el("table", { class: "data-table", attrs: { style: "margin-top:8px" } },
      el("thead", {}, el("tr", {},
        el("th", { class: "num", text: "Seq" }),
        el("th", { text: "Phase" }),
        el("th", { text: "Name" }),
        el("th", { class: "num", text: "Start" }),
        el("th", { class: "num", text: "Duration" }),
        el("th", { text: "Scope" }),
      )),
      el("tbody", {}, ...spans.map(span => {
        const scope = span.scopeID ?? span.scope_id ?? "";
        return el("tr", {},
          el("td", { class: "num mono-cell", text: String(numberValue(span.sequence) ?? "—") }),
          el("td", { class: "mono-cell", text: textOf(span.phase) }),
          el("td", { text: textOf(span.name) }),
          el("td", { class: "num", text: fmtMs(numberValue(span.startMs ?? span.start_ms), 2) }),
          el("td", { class: "num", text: fmtMs(numberValue(span.durationMs ?? span.duration_ms), 2) }),
          el("td", { class: "mono-cell", text: scope ? shortId(scope) : "—", attrs: { title: scope } }),
        );
      })),
    );
    wrap.append(el("div", { class: "table-wrap" }, table));
  }
  const notes = [];
  const spanCount = numberValue(data.spanCount);
  if (data.truncated) notes.push(`Only the longest ${spans.length} of ${spanCount ?? "?"} retained spans in this frame's window are listed.`);
  const unclassifiedCount = numberValue(data.unclassifiedSpanCount);
  if (unclassifiedCount) notes.push(`Frame-level work outside the five attributed phases: ${unclassifiedCount} span${unclassifiedCount === 1 ? "" : "s"}, ${fmtNumber(numberValue(data.unclassifiedCpuMs), 2)} ms.`);
  if (!spans.length && !phases.length && status !== "unavailable" && !reason) notes.push("The runtime returned no retained spans for this frame window.");
  if (notes.length) wrap.append(el("ul", { class: "perf-notes" }, ...notes.map(note => el("li", { text: note }))));
  wrap.append(rawDetails("Runtime payload", data));
  return wrap;
}

function renderFrameAnalysis() {
  const body = $("frame-analysis-body");
  const meta = $("frame-analysis-meta");
  if (!body || !meta) return;
  clear(body);
  const perf = state.performance;
  const frame = perf.frames.find(item => item.frame === perf.selectedFrame) || null;
  if (!frame) {
    meta.textContent = "";
    body.append(emptyInline("Select a frame bar or row to inspect its TimingFrame record."));
    return;
  }
  meta.textContent = `frame #${frame.frame}`;
  const budget = 1000 / perf.budgetHz;
  const parts = frameParts(frame);
  const ui = frameUiMs(frame);
  const jank = frameJank(frame, budget);
  const verdicts = el("div", { class: "perf-verdicts" },
    perfChip(jank ? "Over budget" : "Within budget", jank ? "bad" : "good"),
    parts.recomposition === null ? perfChip("Recomposition not reported", "muted") : null,
    parts.encode === null ? perfChip("Encode not observed", "muted") : null,
    frame.pipelineCreation?.count > 0 ? perfChip(`${frame.pipelineCreation.count} shader or pipeline creations · ${fmtMs(frame.pipelineCreation.cpuMs, 2)} CPU`, "muted") : null,
  );
  body.append(verdicts);
  body.append(el("div", { class: "metrics" },
    metric("Total", fmtNumber(parts.total, 2), "ms"),
    metric("UI", ui === null ? "Not reported" : fmtNumber(ui, 2), ui === null ? "" : "ms"),
    metric("Recomposition", parts.recomposition === null ? "Not reported" : fmtNumber(parts.recomposition, 2), parts.recomposition === null ? "" : "ms"),
    metric("Layout", fmtNumber(parts.layout, 2), "ms"),
    metric("Draw", fmtNumber(parts.draw, 2), "ms"),
    metric("Encode", parts.encode === null ? "Not reported" : fmtNumber(parts.encode, 2), parts.encode === null ? "" : "ms"),
    metric("GPU", parts.gpu === null ? "Not reported" : fmtNumber(parts.gpu, 2), parts.gpu === null ? "" : "ms"),
    metric("Nodes", fmtNumber(numberValue(frame.nodeCount), 0)),
    metric("Batches", fmtNumber(numberValue(frame.batchCount), 0)),
  ));
  const windowStart = numberValue(frame.timelineStart);
  const windowEnd = numberValue(frame.timelineEnd);
  const timelineWindow = windowStart === null || windowEnd === null || (windowStart === 0 && windowEnd === 0)
    ? "not reported"
    : `${windowStart}–${windowEnd}`;
  body.append(kvList([
    ["Submitted", frame.submitted ? "yes" : "no"],
    ["Presented", frame.presented === null || frame.presented === undefined ? "not observed by the host" : frame.presented ? "yes" : "no"],
    ["Frame sequence", String(frame.frame)],
    ["Timeline window (seq)", timelineWindow],
  ]));
  const verdict = el("div", { class: "perf-note" });
  if (jank) {
    const reasons = [];
    if (ui !== null && ui > budget) reasons.push(`UI ${fmtNumber(ui, 2)} ms exceeds ${fmtNumber(budget, 1)} ms by ${fmtNumber(ui - budget, 2)} ms`);
    if (parts.encode !== null && parts.encode > budget) reasons.push(`encode ${fmtNumber(parts.encode, 2)} ms exceeds ${fmtNumber(budget, 1)} ms by ${fmtNumber(parts.encode - budget, 2)} ms`);
    verdict.textContent = `${reasons.join("; ")}.`;
  } else if (parts.encode === null) {
    verdict.textContent = `UI is within the ${fmtNumber(budget, 1)} ms budget. Encode time was not observed, so raster jank cannot be assessed.`;
  } else if (numberValue(parts.total) !== null && parts.total > budget) {
    verdict.textContent = `UI and encode are within the ${fmtNumber(budget, 1)} ms budget. Total is ${fmtNumber(parts.total, 2)} ms; the runtime measures it separately, so it also covers work outside the UI and encode sums.`;
  } else {
    verdict.textContent = `UI and encode are within the ${fmtNumber(budget, 1)} ms budget.`;
  }
  body.append(verdict);
  body.append(gpuBreakdownSection(frame.gpuBreakdown));
  body.append(el("div", { class: "section", attrs: { style: "margin-top:10px" } },
    el("h3", { text: "Attribution" }),
    frameAnalysisAttribution(),
  ));
  const hasWindow = windowStart !== null && windowEnd !== null && !(windowStart === 0 && windowEnd === 0);
  let correlation;
  if (perf.snapshot) {
    correlation = hasWindow
      ? `This snapshot reports timeline sequences ${windowStart}–${windowEnd}; imported frames have no live timeline to attribute.`
      : "The imported frame has no timeline window, so it cannot be attributed to spans.";
  } else if (!state.caps.frameAnalysis) {
    correlation = hasWindow
      ? `This frame reports timeline sequences ${windowStart}–${windowEnd}, but this target does not advertise frame_analysis.`
      : "This TimingFrame has no timeline window or wall-clock timestamp, so it cannot be aligned with trace spans.";
  } else {
    correlation = hasWindow
      ? `Attribution uses the retained spans in timeline sequences ${windowStart}–${windowEnd}. TimingFrame records carry no wall-clock timestamp, so they are not aligned to wall-clock time.`
      : "The runtime reports no timeline window for this frame, so retained spans cannot be attributed to it.";
  }
  body.append(el("div", { class: "perf-note", text: correlation }));
}

// The breakdown names how pass times were obtained, because the backends do not
// measure the same thing: Vulkan times each pass, Metal times each submission and
// leaves pass rows without a number. A missing number means "not measured".
const GPU_SOURCE_NOTES = {
  "vulkan-timestamps": "GPU time is one labelled sample per pass.",
  "metal-submission": "GPU time covers the whole submission; this backend does not time passes individually.",
  "webgpu": "GPU time covers the whole frame; the browser encoder does not report passes with times.",
  "cpu-only": "No GPU samples were attached, so pass rows carry geometry only.",
};

function gpuBreakdownSection(breakdown) {
  if (!breakdown || !Array.isArray(breakdown.passes) || breakdown.passes.length === 0) return null;
  const passes = breakdown.passes;
  const layers = Array.isArray(breakdown.layers) ? breakdown.layers : [];
  const note = GPU_SOURCE_NOTES[breakdown.timingSource] ?? `Timing source ${breakdown.timingSource ?? "unknown"}.`;
  const passRows = [["Pass", "Kind", "Draws", "Indices", "GPU", "Layers"]];
  for (const pass of passes) {
    const gpu = numberValue(pass.gpuMs);
    passRows.push([
      pass.label ?? "unnamed",
      pass.kind ?? "",
      fmtNumber(numberValue(pass.draws), 0),
      fmtNumber(numberValue(pass.indices), 0),
      gpu === null ? "not measured" : fmtMs(gpu, 3),
      Array.isArray(pass.layers) && pass.layers.length > 0 ? pass.layers.join(", ") : "none",
    ]);
  }
  const layerRows = [["Layer", "Indices", "State", "Passes"]];
  for (const [index, layer] of layers.entries()) {
    const state = layer.skipped
      ? "skipped by culling"
      : layer.recorded ? "recorded" : layer.translated ? "cached, moved" : "cached";
    const first = numberValue(layer.firstIndex) ?? 0;
    const count = numberValue(layer.indexCount) ?? 0;
    layerRows.push([
      `${index}: ${shortId(layer.nodeID) || "unnamed layer"}`,
      `${first}..${first + count}`,
      state,
      Array.isArray(layer.passes) && layer.passes.length > 0 ? layer.passes.join(", ") : "none",
    ]);
  }
  return el("div", { class: "section", attrs: { style: "margin-top:10px" } },
    el("h3", { text: "GPU passes" }),
    el("div", { class: "perf-note", text: note }),
    dataTable(passRows, "No pass was recorded for this frame."),
    layers.length === 0
      ? el("div", { class: "perf-note", text: "This host reports no compositing layers, so passes cannot be attributed to layers." })
      : dataTable(layerRows, "No layer survived culling."),
    el("div", { class: "perf-note", text: "Layer windows nest: a container that composites its children owns an outer window that contains theirs. A strip pass draws every visible layer, so its layer list is the whole visible set." }),
  );
}

function dataTable(rows, emptyText) {
  if (rows.length <= 1) return el("div", { class: "perf-note", text: emptyText });
  const head = el("tr", {}, ...rows[0].map(cell => el("th", { text: String(cell) })));
  const body = rows.slice(1).map(row => el("tr", {}, ...row.map(cell => el("td", { text: String(cell) }))));
  return el("div", { class: "table-wrap" },
    el("table", { class: "data-table" },
      el("thead", {}, head),
      el("tbody", {}, ...body),
    ),
  );
}

async function loadFrameAnalysis(frameNumber) {
  const perf = state.performance;
  if (perf.snapshot || !state.caps.frameAnalysis || !state.targetId) return;
  const frame = perf.frames.find(item => item.frame === frameNumber);
  if (!frame) return;
  const key = `${frameNumber}:${numberValue(frame.totalMs) ?? "?"}`;
  const cached = perf.analysisCache.get(key);
  if (cached) {
    perf.analysis = { frame: frameNumber, loading: false, error: null, data: cached, request: perf.analysis.request };
    renderFrameAnalysis();
    return;
  }
  const targetId = state.targetId;
  const request = perf.analysis.request + 1;
  perf.analysis = { frame: frameNumber, loading: true, error: null, data: null, request };
  renderFrameAnalysis();
  try {
    const data = await rpc("analyze_ui_frame", { target: targetId, frame: frameNumber });
    if (request !== state.performance.analysis.request || targetId !== state.targetId) return;
    state.performance.analysis = { frame: frameNumber, loading: false, error: null, data, request };
    state.performance.analysisCache.set(key, data);
  } catch (error) {
    if (request !== state.performance.analysis.request || targetId !== state.targetId) return;
    state.performance.analysis = { frame: frameNumber, loading: false, error, data: null, request };
  } finally {
    renderFrameAnalysis();
  }
}

