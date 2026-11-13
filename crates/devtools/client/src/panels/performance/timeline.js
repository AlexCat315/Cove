/* Timeline */

function updateTraceElapsed() {
  const perf = state.performance;
  if (!perf.recording) {
    dom.performanceElapsed.hidden = true;
    return;
  }
  const seconds = Math.max(0, (Date.now() - perf.startedAt) / 1000);
  dom.performanceElapsed.textContent = `Recording ${seconds.toFixed(1)} s`;
  dom.performanceElapsed.hidden = false;
}

async function startTrace() {
  const perf = state.performance;
  if (perf.snapshot) {
    toast("Return to live data before recording a trace.", "info");
    return;
  }
  if (!state.caps.trace) {
    toast("This runtime does not advertise trace capture.", "error");
    return;
  }
  if (perf.recording || perf.traceLoading) return;
  const targetId = state.targetId;
  perf.traceLoading = true;
  perf.traceError = null;
  renderPerformance();
  try {
    await rpc("start_ui_trace", { target: targetId });
    if (targetId !== state.targetId) return;
    perf.recording = true;
    perf.startedAt = Date.now();
    perf.selectedSpan = null;
    perf.analysisCache.clear();
    perf.analysis = { frame: null, loading: false, error: null, data: null, request: perf.analysis.request + 1 };
    perf.recordTimer = setInterval(updateTraceElapsed, 250);
    toast("Recording CPU spans. Stop when the operation is complete.", "success", 5000);
  } catch (error) {
    if (targetId !== state.targetId) return;
    perf.traceError = error;
    toast(errorMessage(error), "error", 9000);
  } finally {
    if (targetId === state.targetId) {
      perf.traceLoading = false;
      renderPerformance();
    }
  }
}

async function stopTrace(options = {}) {
  const quiet = options.quiet === true;
  const perf = state.performance;
  if (!perf.recording) {
    if (!quiet) toast("No trace is recording.", "info");
    return;
  }
  const targetId = state.targetId;
  perf.traceLoading = true;
  renderPerformance();
  try {
    const summary = await rpc("stop_ui_trace", { target: targetId });
    if (targetId !== state.targetId) return;
    perf.recording = false;
    if (perf.recordTimer) clearInterval(perf.recordTimer);
    perf.recordTimer = null;
    if (!summary || typeof summary !== "object" || !summary.id) {
      throw new RpcError("The runtime returned no capture summary.", "invalid_response");
    }
    perf.traceError = null;
    perf.comparison = null;
    perf.tab = "timeline";
    perf.analysisCache.clear();
    perf.analysis = { frame: null, loading: false, error: null, data: null, request: perf.analysis.request + 1 };
    addTraceCapture(summary);
    const count = numberValue(summary.eventCount) ?? 0;
    if (!quiet) {
      toast(count ? `Trace ${summary.id} stopped with ${count} spans.` : `Trace ${summary.id} stopped with no spans.`, count ? "success" : "info", 6000);
    }
    await loadTraceEvents({ captureId: summary.id, quiet: true });
  } catch (error) {
    if (targetId !== state.targetId) return;
    perf.traceError = error;
    if (!quiet) toast(errorMessage(error), "error", 9000);
  } finally {
    if (targetId === state.targetId) {
      perf.traceLoading = false;
      renderPerformance();
    }
  }
}

function addTraceCapture(summary) {
  if (!summary?.id) return;
  const perf = state.performance;
  let record = captureRecordById(summary.id);
  if (!record) {
    record = {
      summary,
      events: [],
      total: numberValue(summary.eventCount) ?? 0,
      nextOffset: null,
      complete: false,
      imported: false,
      loading: false,
    };
    perf.captures.push(record);
  } else {
    record.summary = summary;
  }
  if (perf.captures.length > 8) {
    const index = perf.captures.findIndex(item => item !== record);
    if (index >= 0) perf.captures.splice(index, 1);
  }
  perf.activeCaptureId = summary.id;
  const ids = perf.captures.map(item => item.summary?.id).filter(Boolean);
  if (!perf.baseline || !ids.includes(perf.baseline)) perf.baseline = ids[0] || "";
  perf.candidate = summary.id;
}

function updateCaptureSummary(summary) {
  if (!summary?.id) return;
  const record = captureRecordById(summary.id);
  if (record) record.summary = summary;
}

async function fetchTracePage(record, options = {}) {
  const perf = state.performance;
  if (!record || !record.summary?.id || record.imported || record.loading || record.complete || !state.targetId) return false;
  const targetId = state.targetId;
  const offset = numberValue(record.nextOffset) ?? 0;
  record.loading = true;
  perf.eventLoading = true;
  perf.eventError = null;
  if (options.silent !== true) renderPerformance();
  try {
    const data = await rpc("get_ui_trace", { target: targetId, capture_id: record.summary?.id, offset, limit: 100 });
    if (targetId !== state.targetId) return false;
    if (data?.capture) updateCaptureSummary(data.capture);
    const page = data?.events || {};
    const items = Array.isArray(page.items) ? page.items : [];
    record.events = offset === 0 ? items : [...record.events, ...items];
    record.total = numberValue(page.total) ?? record.events.length;
    record.nextOffset = numberValue(page.nextOffset);
    record.complete = record.nextOffset === null || record.events.length >= record.total || record.events.length >= MAX_SNAPSHOT_EVENTS;
    return true;
  } catch (error) {
    if (targetId !== state.targetId) return false;
    perf.eventError = error;
    if (!options.quiet) toast(errorMessage(error), "error", 8000);
    return false;
  } finally {
    record.loading = false;
    if (targetId === state.targetId) {
      perf.eventLoading = false;
      if (options.silent !== true) renderPerformance();
    }
  }
}

async function loadTraceEvents(options = {}) {
  const record = options.captureId ? captureRecordById(options.captureId) : activeCaptureRecord();
  if (!record) return;
  if (record.loading || record.complete || record.imported) {
    renderPerformance();
    return;
  }
  await fetchTracePage(record, options);
}

function selectCapture(id) {
  const perf = state.performance;
  const record = captureRecordById(id);
  if (!record) return;
  if (perf.activeCaptureId !== id) {
    perf.activeCaptureId = id;
    perf.selectedSpan = null;
    perf.comparison = null;
    perf.zoom = 1;
    perf.eventError = null;
  }
  renderPerformance();
  if (!record.imported && !record.complete && !record.events.length) loadTraceEvents({ captureId: id, quiet: true });
}

function tracePhaseValue(phase, key) {
  return numberValue(phase?.[key]) ?? numberValue(phase?.[key.replace(/Ms$/, "_ms")]);
}

function localTraceComparison(baseline, candidate) {
  const phases = new Map();
  for (const phase of Array.isArray(baseline?.phases) ? baseline.phases : []) {
    if (phase?.phase) phases.set(phase.phase, { phase: phase.phase, baseline: phase, candidate: null });
  }
  for (const phase of Array.isArray(candidate?.phases) ? candidate.phases : []) {
    if (!phase?.phase) continue;
    const entry = phases.get(phase.phase) || { phase: phase.phase, baseline: null, candidate: null };
    entry.candidate = phase;
    phases.set(phase.phase, entry);
  }
  const changes = [...phases.values()].sort((left, right) => left.phase.localeCompare(right.phase)).map(entry => {
    const baselineAverage = tracePhaseValue(entry.baseline, "averageMs");
    const candidateAverage = tracePhaseValue(entry.candidate, "averageMs");
    const baselineP95 = tracePhaseValue(entry.baseline, "p95Ms");
    const candidateP95 = tracePhaseValue(entry.candidate, "p95Ms");
    return {
      phase: entry.phase,
      baseline: entry.baseline,
      candidate: entry.candidate,
      averageDeltaMs: baselineAverage === null || candidateAverage === null ? null : candidateAverage - baselineAverage,
      p95DeltaMs: baselineP95 === null || candidateP95 === null ? null : candidateP95 - baselineP95,
    };
  });
  return { baseline, candidate, changes, source: "summaries" };
}

async function compareTraces() {
  const perf = state.performance;
  if (perf.comparing) return;
  const baselineId = $("trace-baseline")?.value || perf.baseline;
  const candidateId = $("trace-candidate")?.value || perf.candidate;
  if (!baselineId || !candidateId) {
    toast("Record two traces to compare them.", "info");
    return;
  }
  if (baselineId === candidateId) {
    toast("Choose two different traces.", "info");
    return;
  }
  const baseline = captureRecordById(baselineId);
  const candidate = captureRecordById(candidateId);
  if (!baseline || !candidate) return;
  perf.baseline = baselineId;
  perf.candidate = candidateId;
  if (!baseline.imported && !candidate.imported && state.targetId) {
    const targetId = state.targetId;
    perf.comparing = true;
    perf.comparison = null;
    perf.traceError = null;
    renderPerformance();
    try {
      const data = await rpc("compare_ui_traces", { target: targetId, baseline_id: baselineId, candidate_id: candidateId });
      if (targetId !== state.targetId) return;
      perf.comparison = data && typeof data === "object" ? { ...data, source: "runtime" } : null;
    } catch (error) {
      if (targetId !== state.targetId) return;
      perf.traceError = error;
      toast(errorMessage(error), "error", 9000);
    } finally {
      if (targetId === state.targetId) {
        perf.comparing = false;
        renderPerformance();
      }
    }
    return;
  }
  perf.comparison = localTraceComparison(baseline.summary, candidate.summary);
  renderPerformance();
}

function renderTimelinePanel() {
  const host = dom.performanceTimeline;
  clear(host);
  const perf = state.performance;
  const target = targetById();
  if (perf.snapshot) {
    host.append(snapshotBanner());
    if (!perf.captures.length) {
      const block = el("div");
      showState(block, {
        icon: "trace",
        title: "No trace captures in this snapshot",
        message: "The imported file contains no trace captures. Frame records are available on the Frames tab.",
        action: "Open frames",
        onAction: () => setPerformanceTab("frames"),
      });
      host.append(block);
      return;
    }
  } else if (!target) {
    showState(host, { icon: "device", title: "No target", message: "Select a connected runtime to record CPU spans." });
    return;
  } else if (!state.caps.runtime) {
    showState(host, {
      icon: "trace",
      title: "Trace is unavailable",
      message: "Trace capture comes from a Cove runtime. Select a runtime target.",
    });
    return;
  } else if (!state.caps.trace) {
    showState(host, {
      icon: "trace",
      title: "Trace is unavailable",
      message: "This runtime does not advertise trace capture.",
    });
    return;
  }
  if (perf.recording) {
    host.append(el("div", { class: "panel-block" },
      el("div", { class: "panel-block-head" },
        el("h2", { text: "Recording" }),
        el("span", { class: "recording-label", text: perf.traceLoading ? "Stopping…" : "Live" }),
      ),
      el("div", { class: "panel-block-body" },
        el("p", { text: "Perform the real operation in the app, then stop the trace to read its spans. Spans come from the runtime probe and do not require Screen Recording." }),
      ),
    ));
    if (perf.traceError) host.append(errorInline(errorMessage(perf.traceError)));
    return;
  }
  if (perf.traceLoading && !perf.captures.length) {
    showState(host, { kind: "loading", title: "Reading trace", message: "Waiting for the runtime to finish the capture." });
    return;
  }
  const record = activeCaptureRecord();
  if (!record) {
    const block = el("div");
    showState(block, {
      icon: "trace",
      title: "No trace capture",
      message: "Start a trace, perform a real operation, then stop it to read CPU spans. Spans come from the runtime probe and do not require Screen Recording.",
      action: "Start trace",
      onAction: startTrace,
    });
    host.append(block);
    if (perf.traceError) host.append(errorInline(errorMessage(perf.traceError)));
    return;
  }
  if (perf.traceError) host.append(errorInline(errorMessage(perf.traceError)));
  host.append(renderCaptureToolbar(record));
  host.append(el("div", { class: "stat-row" },
    el("div", { class: "stat" }, el("div", { class: "stat-label", text: "Duration" }), el("div", { class: "stat-value" }, fmtNumber(numberValue(record.summary?.durationMs), 1), el("small", { text: " ms" }))),
    el("div", { class: "stat" }, el("div", { class: "stat-label", text: "Spans" }), el("div", { class: "stat-value" }, String(numberValue(record.summary?.eventCount) ?? record.events.length))),
    el("div", { class: "stat" }, el("div", { class: "stat-label", text: "Loaded" }), el("div", { class: "stat-value" }, String(record.events.length))),
    el("div", { class: "stat" }, el("div", { class: "stat-label", text: "Dropped" }), el("div", { class: "stat-value" }, String(numberValue(record.summary?.dropped) ?? 0))),
    el("div", { class: "stat" }, el("div", { class: "stat-label", text: "Phases" }), el("div", { class: "stat-value" }, String(Array.isArray(record.summary?.phases) ? record.summary.phases.length : 0))),
  ));
  host.append(el("div", { class: "panel-block" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Phases" }),
      el("span", { class: "pane-meta", text: record.summary?.id || "" }),
    ),
    el("div", { class: "panel-block-body" }, renderTracePhases(record.summary)),
  ));
  host.append(renderFlameBlock(record));
  host.append(renderTraceEventsBlock(record));
  host.append(renderTraceCompareBlock());
  renderSpanDetail();
}

function renderCaptureToolbar(record) {
  const perf = state.performance;
  const select = el("select", {
    props: { id: "performance-capture" },
    attrs: { "aria-label": "Trace capture" },
    on: { change: () => selectCapture(select.value) },
  }, ...perf.captures.map(item => new Option(
    `${item.summary?.id || "capture"} · ${numberValue(item.summary?.eventCount) ?? item.events.length} spans${item.imported ? " · imported" : ""}`,
    item.summary?.id || "",
  )));
  select.value = perf.activeCaptureId;
  const toolbar = el("div", { class: "perf-toolbar" },
    el("label", { class: "inline-field", attrs: { for: "performance-capture" }, text: "Capture" }),
    select,
    el("span", { class: "pane-meta", text: record.imported ? "Imported snapshot capture" : record.complete ? "All spans loaded" : `${record.events.length} of ${record.total || "?"} spans loaded` }),
    record.imported ? null : el("button", {
      class: "tool-button",
      props: { type: "button", disabled: record.loading },
      text: "Reload capture",
      on: { click: () => reloadCapture(record) },
    }),
  );
  return toolbar;
}

function reloadCapture(record) {
  if (!record || record.imported || record.loading) return;
  record.events = [];
  record.nextOffset = null;
  record.complete = false;
  record.total = numberValue(record.summary?.eventCount) ?? 0;
  renderPerformance();
  loadTraceEvents({ captureId: record.summary?.id, quiet: true });
}

function renderTracePhases(summary) {
  const phases = Array.isArray(summary?.phases) ? summary.phases : [];
  if (!phases.length) return emptyInline("No phase summaries were published for this capture.");
  return el("div", { class: "phases" },
    el("div", { class: "phase-row is-head" },
      el("span", { text: "Phase" }), el("span", { text: "Count" }), el("span", { text: "Average" }),
      el("span", { text: "P95" }), el("span", { text: "Maximum" }),
    ),
    ...phases.map(phase => el("div", { class: "phase-row" },
      el("span", { class: "phase-name", text: phase.phase || "—" }),
      el("span", { text: String(phase.count ?? "—") }),
      el("span", { text: fmtMs(tracePhaseValue(phase, "averageMs"), 3) }),
      el("span", { text: fmtMs(tracePhaseValue(phase, "p95Ms"), 3) }),
      el("span", { text: fmtMs(tracePhaseValue(phase, "maximumMs"), 3) }),
    )),
  );
}

function emptySpanMessage(record) {
  if (record?.imported) return "The imported file contains no spans for this capture.";
  return record?.summary?.eventCount ? "No events were returned for this page." : "The capture contains no CPU spans. The runtime may have been idle.";
}

function buildFlameModel(events) {
  const parsed = [];
  for (const event of events) {
    const start = numberValue(event.start_ms ?? event.startMs);
    if (start === null) continue;
    const duration = Math.max(0, numberValue(event.duration_ms ?? event.durationMs) ?? 0);
    parsed.push({
      sequence: numberValue(event.sequence) ?? 0,
      phase: String(event.phase || "unphased"),
      name: String(event.name || ""),
      scope: event.scope_id ?? event.scopeID ?? "",
      start,
      duration,
      end: start + duration,
    });
  }
  if (!parsed.length) return null;
  const start = parsed.reduce((minimum, item) => Math.min(minimum, item.start), parsed[0].start);
  const end = parsed.reduce((maximum, item) => Math.max(maximum, item.end), start + 0.001);
  const span = Math.max(end - start, 0.001);
  const tracks = new Map();
  const ordered = [...parsed].sort((left, right) => left.start - right.start || right.duration - left.duration || left.sequence - right.sequence);
  for (const item of ordered) {
    let track = tracks.get(item.phase);
    if (!track) {
      track = { phase: item.phase, lanes: [], items: [] };
      tracks.set(item.phase, track);
    }
    track.items.push(item);
    let lane = track.lanes.find(candidate => candidate.end <= item.start + 0.0005);
    if (!lane) {
      lane = { end: item.end, items: [] };
      track.lanes.push(lane);
    }
    lane.end = Math.max(lane.end, item.end);
    lane.items.push(item);
  }
  return { start, end, span, tracks: [...tracks.values()], count: parsed.length };
}

function renderFlameBlock(record) {
  const perf = state.performance;
  const events = record.events;
  const head = el("div", { class: "panel-block-head" },
    el("h2", { text: "Timeline flame chart" }),
    el("span", { class: "pane-meta", text: `${events.length} spans · ${record.complete ? "complete" : "partial"}` }),
  );
  if (perf.eventLoading && !events.length) {
    return el("div", { class: "panel-block" }, head,
      el("div", { class: "panel-block-body" }, el("div", { class: "inline-spinner" }, el("span", { class: "spinner" }), "Reading trace events…")));
  }
  if (perf.eventError && !events.length) {
    return el("div", { class: "panel-block" }, head,
      el("div", { class: "panel-block-body" },
        errorInline(errorMessage(perf.eventError)),
        el("button", { class: "tool-button", props: { type: "button" }, text: "Retry", on: { click: () => loadTraceEvents({ captureId: record.summary?.id }) } })));
  }
  if (!events.length) {
    return el("div", { class: "panel-block" }, head,
      el("div", { class: "panel-block-body" }, emptyInline(emptySpanMessage(record))));
  }
  const model = buildFlameModel(events);
  if (!model) {
    return el("div", { class: "panel-block" }, head,
      el("div", { class: "panel-block-body" }, emptyInline("The returned spans have no finite start times to plot.")));
  }
  const zoom = perf.zoom;
  const controls = el("div", { class: "flame-controls" },
    el("span", { class: "inline-field", text: "Zoom" }),
    ...[1, 2, 4, 8].map(level => el("button", {
      class: `tool-button small${zoom === level ? " is-active" : ""}`,
      props: { type: "button", "aria-pressed": zoom === level },
      text: `${level}×`,
      on: { click: () => { perf.zoom = level; renderPerformance(); } },
    })),
    el("span", { class: "pane-meta", text: `${fmtNumber(model.span, 1)} ms window · ${model.tracks.length} phases` }),
  );
  const plot = el("div", { class: "flame-plot", attrs: { style: `width:${zoom * 100}%` } });
  const axisRow = el("div", { class: "flame-row is-axis" },
    el("div", { class: "flame-track-label", text: "Time" }),
    el("div", { class: "flame-track-lanes" }),
  );
  const axisLanes = axisRow.lastElementChild;
  for (let index = 0; index <= 4; index += 1) {
    const fraction = index / 4;
    const label = index === 0 ? "0 ms" : index === 4 ? `${fmtNumber(model.span, 1)} ms` : fmtNumber(model.span * fraction, 1);
    axisLanes.append(el("span", { class: "flame-tick", attrs: { style: `left:${fraction * 100}%` }, text: label }));
  }
  plot.append(axisRow);
  let rendered = 0;
  let hidden = 0;
  let hiddenLanes = 0;
  for (const track of model.tracks) {
    const lanesToDraw = track.lanes.slice(0, 16);
    hiddenLanes += Math.max(0, track.lanes.length - lanesToDraw.length);
    const laneHeight = 16;
    const row = el("div", { class: "flame-row" },
      el("div", { class: "flame-track-label" },
        el("span", { class: "flame-phase", text: track.phase }),
        el("span", { class: "flame-count", text: `${track.items.length} span${track.items.length === 1 ? "" : "s"}` }),
      ),
      el("div", { class: "flame-track-lanes", attrs: { style: `height:${Math.max(1, lanesToDraw.length) * laneHeight}px` } }),
    );
    const laneHost = row.lastElementChild;
    for (const [laneIndex, lane] of lanesToDraw.entries()) {
      const laneNode = el("div", { class: "flame-lane", attrs: { style: `top:${laneIndex * laneHeight}px;height:${laneHeight}px` } });
      for (const item of lane.items) {
        if (rendered >= MAX_FLAME_BARS) {
          hidden += 1;
          continue;
        }
        rendered += 1;
        const left = ((item.start - model.start) / model.span) * 100;
        const width = Math.max(0.15, (item.duration / model.span) * 100);
        const isSelected = perf.selectedSpan === item.sequence;
        const describe = `${item.phase}${item.name ? ` · ${item.name}` : ""} · ${fmtNumber(item.duration, 2)} ms · starts +${fmtNumber(item.start - model.start, 2)} ms${item.scope ? ` · scope ${item.scope}` : ""}`;
        laneNode.append(el("div", {
          class: `flame-bar${isSelected ? " is-selected" : ""}`,
          attrs: {
            style: `left:${left}%;width:${width}%`,
            title: describe,
            "data-sequence": String(item.sequence),
          },
          on: { click: () => selectSpan(item.sequence) },
        }));
      }
      laneHost.append(laneNode);
    }
    plot.append(row);
  }
  const notes = [];
  if (hiddenLanes) notes.push(`${hiddenLanes} lane${hiddenLanes === 1 ? " is" : "s are"} not drawn because a track has more than 16 overlapping spans.`);
  if (hidden) notes.push(`${hidden} more span${hidden === 1 ? "" : "s"} are not drawn; the spans table below still lists every loaded span.`);
  if (!record.complete) {
    notes.push(record.imported
      ? `The imported file contains ${events.length} of ${record.total || "?"} spans for this capture; the rest were not exported.`
      : `The timeline plots the ${events.length} loaded spans of ${record.total || "?"} in this capture. Load more spans to extend it.`);
  }
  return el("div", { class: "panel-block" },
    head,
    el("div", { class: "panel-block-body" },
      controls,
      el("div", {
        class: "flame-scroll",
        attrs: {
          role: "img",
          "aria-label": `Timeline flame chart with ${events.length} spans across ${model.tracks.length} phases. The spans table below lists every loaded span.`,
        },
      }, plot),
      notes.length ? el("div", { class: "perf-note" }, notes.join(" ")) : null,
      el("div", { class: "perf-note", text: "Bars are pointer-selectable; the spans table provides keyboard selection. The chart window starts at the earliest loaded span; the table's Start column is relative to the capture's timeline origin." }),
      el("div", { class: "span-detail-host", attrs: { id: "span-detail" } }),
    ),
  );
}

function renderTraceEventsBlock(record) {
  const perf = state.performance;
  const events = record.events;
  const body = el("div", { class: "panel-block-body" });
  const selected = perf.selectedSpan;
  if (perf.eventLoading && !events.length) {
    body.append(el("div", { class: "inline-spinner" }, el("span", { class: "spinner" }), "Reading trace events…"));
  } else if (perf.eventError && !events.length) {
    body.append(errorInline(errorMessage(perf.eventError)));
  } else if (!events.length) {
    body.append(emptyInline(emptySpanMessage(record)));
  } else {
    const rows = events.slice(0, SPAN_TABLE_LIMIT).map((event, index) => {
      const sequence = numberValue(event.sequence) ?? 0;
      const scope = event.scope_id ?? event.scopeID ?? "";
      const isSelected = sequence === selected;
      return el("tr", {
        class: isSelected ? "is-selected" : "",
        attrs: { "data-sequence": String(sequence), tabindex: isSelected || (selected === null && index === 0) ? "0" : "-1" },
        on: { click: () => selectSpan(sequence) },
      },
        el("td", { class: "num mono-cell", text: String(sequence) }),
        el("td", { class: "mono-cell", text: event.phase || "—" }),
        el("td", { text: event.name || "—" }),
        el("td", { class: "num", text: fmtMs(event.start_ms ?? event.startMs, 2) }),
        el("td", { class: "num", text: fmtMs(event.duration_ms ?? event.durationMs, 2) }),
        el("td", { class: "mono-cell", text: scope ? shortId(scope) : "—", attrs: { title: scope } }),
      );
    });
    const tbody = el("tbody", {}, ...rows);
    tbody.addEventListener("keydown", event => {
      const row = event.target.closest("tr[data-sequence]");
      if (!row) return;
      const list = [...tbody.querySelectorAll("tr[data-sequence]")];
      const index = list.indexOf(row);
      let next = null;
      if (event.key === "ArrowDown") next = list[Math.min(list.length - 1, index + 1)];
      else if (event.key === "ArrowUp") next = list[Math.max(0, index - 1)];
      else if (event.key === "Home") next = list[0];
      else if (event.key === "End") next = list[list.length - 1];
      else if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        selectSpan(Number(row.dataset.sequence), { focus: true });
        return;
      }
      if (!next) return;
      event.preventDefault();
      selectSpan(Number(next.dataset.sequence), { focus: true });
    });
    body.append(el("div", { class: "table-wrap spans-table-wrap" }, el("table", { class: "data-table" },
      el("thead", {}, el("tr", {},
        el("th", { class: "num", text: "Seq" }),
        el("th", { text: "Phase" }),
        el("th", { text: "Name" }),
        el("th", { class: "num", text: "Start" }),
        el("th", { class: "num", text: "Duration" }),
        el("th", { text: "Scope" }),
      )),
      tbody,
    )));
  }
  const pager = el("div", { class: "trace-pager" },
    el("span", { text: `${events.length} of ${record.total || events.length} spans` }),
  );
  if (events.length > SPAN_TABLE_LIMIT) pager.append(el("span", { class: "muted", text: `table shows the first ${SPAN_TABLE_LIMIT} of ${events.length} loaded spans` }));
  if (perf.eventLoading) pager.append(el("span", { class: "inline-spinner" }, el("span", { class: "spinner" }), "Loading…"));
  if (!record.complete && !record.imported && !perf.eventLoading) {
    pager.append(el("button", {
      class: "tool-button",
      props: { type: "button" },
      text: "Load more",
      on: { click: () => loadTraceEvents({ captureId: record.summary?.id, quiet: true }) },
    }));
  } else if (record.imported && !record.complete) {
    pager.append(el("span", { class: "muted", text: "The imported file contains only the loaded spans." }));
  } else if (record.complete && events.length > 0) {
    pager.append(el("span", { text: "End of capture" }));
  }
  if (perf.eventError) pager.append(el("span", { class: "muted", text: errorMessage(perf.eventError) }));
  body.append(pager);
  const selectedEvent = events.find(event => (numberValue(event.sequence) ?? 0) === perf.selectedSpan) || null;
  const meta = selectedEvent
    ? `#${selectedEvent.sequence} · ${selectedEvent.phase || "span"}${selectedEvent.name ? ` · ${selectedEvent.name}` : ""}`
    : record.summary?.id || "";
  return el("div", { class: "panel-block" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Spans" }),
      el("span", { class: "pane-meta", text: meta }),
    ),
    body,
  );
}

function selectSpan(sequence, options = {}) {
  const perf = state.performance;
  if (!Number.isFinite(sequence)) return;
  perf.selectedSpan = sequence;
  updateSpanSelection();
  renderSpanDetail();
  if (options.focus) {
    dom.performanceTimeline.querySelector(`tr[data-sequence="${sequence}"]`)?.focus();
  }
  const bar = dom.performanceTimeline.querySelector(`.flame-bar[data-sequence="${perf.selectedSpan}"]`);
  const scroller = bar?.closest(".flame-scroll");
  if (bar && scroller) {
    const barRect = bar.getBoundingClientRect();
    const scrollRect = scroller.getBoundingClientRect();
    if (barRect.left < scrollRect.left) scroller.scrollLeft += barRect.left - scrollRect.left - 12;
    else if (barRect.right > scrollRect.right) scroller.scrollLeft += barRect.right - scrollRect.right + 12;
    if (barRect.top < scrollRect.top) scroller.scrollTop += barRect.top - scrollRect.top - 12;
    else if (barRect.bottom > scrollRect.bottom) scroller.scrollTop += barRect.bottom - scrollRect.bottom + 12;
  }
}

function updateSpanSelection() {
  const selected = state.performance.selectedSpan;
  for (const node of dom.performanceTimeline.querySelectorAll("[data-sequence]")) {
    const isSelected = Number(node.dataset.sequence) === selected;
    node.classList.toggle("is-selected", isSelected);
    if (node.tagName === "TR") node.tabIndex = isSelected ? 0 : -1;
  }
}

function renderSpanDetail() {
  const host = $("span-detail");
  if (!host) return;
  clear(host);
  const perf = state.performance;
  const record = activeCaptureRecord();
  const event = record?.events.find(item => (numberValue(item.sequence) ?? 0) === perf.selectedSpan) || null;
  if (!event) return;
  const scope = event.scope_id ?? event.scopeID ?? "";
  host.append(el("div", { class: "section", attrs: { style: "margin:10px 0 0" } },
    el("h3", { text: `Span #${event.sequence}` }),
    kvList([
      ["Phase", event.phase || "—"],
      ["Name", event.name || "—"],
      ["Start", fmtMs(event.start_ms ?? event.startMs, 3)],
      ["Duration", fmtMs(event.duration_ms ?? event.durationMs, 3)],
      ["Scope", scope || "—"],
    ]),
    Array.isArray(event.reasons) && event.reasons.length ? rawDetails("Reasons", event.reasons) : null,
  ));
}

function renderTraceCompareBlock() {
  const perf = state.performance;
  const captures = perf.captures;
  const block = el("div", { class: "panel-block" },
    el("div", { class: "panel-block-head" }, el("h2", { text: "Compare captures" })),
    el("div", { class: "panel-block-body" }),
  );
  const body = block.lastElementChild;
  if (captures.length < 2) {
    body.append(emptyInline("Record at least two traces in this session to compare phases."));
    return block;
  }
  const baselineSelect = el("select", { props: { id: "trace-baseline" }, attrs: { "aria-label": "Baseline trace" } },
    ...captures.map(record => new Option(`${record.summary?.id || "capture"} · ${numberValue(record.summary?.eventCount) ?? record.events.length} spans${record.imported ? " · imported" : ""}`, record.summary?.id || "")));
  const candidateSelect = el("select", { props: { id: "trace-candidate" }, attrs: { "aria-label": "Candidate trace" } },
    ...captures.map(record => new Option(`${record.summary?.id || "capture"} · ${numberValue(record.summary?.eventCount) ?? record.events.length} spans${record.imported ? " · imported" : ""}`, record.summary?.id || "")));
  baselineSelect.value = captures.some(record => record.summary?.id === perf.baseline) ? perf.baseline : captures[0].summary?.id || "";
  candidateSelect.value = captures.some(record => record.summary?.id === perf.candidate) ? perf.candidate : captures[captures.length - 1].summary?.id || "";
  const reselect = () => {
    perf.baseline = baselineSelect.value;
    perf.candidate = candidateSelect.value;
    perf.comparison = null;
    renderPerformance();
  };
  baselineSelect.addEventListener("change", reselect);
  candidateSelect.addEventListener("change", reselect);
  body.append(el("div", { class: "trace-compare-controls" },
    el("label", { class: "inline-field", attrs: { for: "trace-baseline" }, text: "Baseline" }),
    baselineSelect,
    el("label", { class: "inline-field", attrs: { for: "trace-candidate" }, text: "Candidate" }),
    candidateSelect,
    el("button", {
      class: "tool-button",
      props: { type: "button" },
      text: perf.comparing ? "Comparing…" : "Compare",
      on: { click: compareTraces },
    }),
  ));
  const comparison = perf.comparison;
  if (comparison && Array.isArray(comparison.changes)) {
    if (!comparison.changes.length) {
      body.append(emptyInline("Both captures reported no phase summaries to compare."));
    } else {
      body.append(el("div", { class: "table-wrap", attrs: { style: "margin-top:8px" } }, el("table", { class: "data-table" },
        el("thead", {}, el("tr", {},
          el("th", { text: "Phase" }),
          el("th", { class: "num", text: "Baseline avg" }),
          el("th", { class: "num", text: "Candidate avg" }),
          el("th", { class: "num", text: "Δ avg" }),
          el("th", { class: "num", text: "Δ p95" }),
        )),
        el("tbody", {}, ...comparison.changes.map(change => {
          const baselineAverage = tracePhaseValue(change.baseline, "averageMs");
          const candidateAverage = tracePhaseValue(change.candidate, "averageMs");
          const averageDelta = numberValue(change.averageDeltaMs ?? change.average_delta_ms);
          const p95Delta = numberValue(change.p95DeltaMs ?? change.p95_delta_ms);
          return el("tr", {},
            el("td", { class: "mono-cell", text: change.phase || "—" }),
            el("td", { class: "num", text: fmtMs(baselineAverage, 3) }),
            el("td", { class: "num", text: fmtMs(candidateAverage, 3) }),
            el("td", { class: `num ${averageDelta === null ? "" : averageDelta > 0 ? "change-positive" : averageDelta < 0 ? "change-negative" : ""}`, text: averageDelta === null ? "—" : `${averageDelta > 0 ? "+" : ""}${averageDelta.toFixed(3)} ms` }),
            el("td", { class: `num ${p95Delta === null ? "" : p95Delta > 0 ? "change-positive" : p95Delta < 0 ? "change-negative" : ""}`, text: p95Delta === null ? "—" : `${p95Delta > 0 ? "+" : ""}${p95Delta.toFixed(3)} ms` }),
          );
        })),
      )));
      body.append(el("div", { class: "perf-note" },
        comparison.baseline?.id || comparison.candidate?.id
          ? `${comparison.baseline?.id || "baseline"} → ${comparison.candidate?.id || "candidate"}`
          : "",
        comparison.source === "summaries" ? " · computed from the captures' published phase summaries" : " · reported by the runtime"));
    }
  }
  return block;
}

