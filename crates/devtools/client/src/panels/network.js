/* Network */
import { clear, el, fmtBytes, fmtEpochMs, fmtMs, numberValue, textOf } from "../core/format.js";
import { DEBUGGER_POLL_MS, dom, state } from "../core/globals.js";
import { errorMessage, isDisconnectedError, rpc, targetById, toast, unwrapData } from "../core/session.js";
import { emptyInline, errorInline, kvList, sectionBlock, showState } from "../core/ui-helpers.js";
import { copyText } from "../inspector/editors.js";
import { refreshDebugger } from "./debugger/session.js";
import { loadLogs } from "./logs.js";
import { loadMemory } from "./memory/view.js";
import { loadFrames } from "./performance/frames.js";
import { loadRegisteredState } from "./state.js";
import { refreshCurrent } from "../shell/views.js";


export function stopTimer(key) {
  const timer = state[key]?.timer;
  if (timer) clearInterval(timer);
  if (state[key]) state[key].timer = null;
}

export function stopAllFollowTimers() {
  stopTimer("network");
  stopTimer("logs");
  stopTimer("performance");
  stopTimer("debugger");
  stopTimer("memory");
  stopTimer("registeredState");
}

export function syncFollowTimers() {
  stopAllFollowTimers();
  if (document.hidden || !state.connected || !state.targetId) return;
  if (state.view === "network" && state.network.follow && state.caps.network) {
    state.network.timer = setInterval(() => loadNetwork({ quiet: true }), 2000);
  }
  if (state.view === "logs" && state.logs.follow && state.caps.logs) {
    state.logs.timer = setInterval(() => loadLogs({ quiet: true }), 2000);
  }
  if (state.view === "performance" && state.performance.tab === "frames" && state.performance.follow && state.caps.timing && !state.performance.snapshot) {
    state.performance.timer = setInterval(() => loadFrames({ quiet: true }), 1000);
  }
  if (state.view === "memory" && state.memory.follow && state.caps.runtime) {
    state.memory.timer = setInterval(() => loadMemory({ quiet: true }), state.memory.intervalMs);
  }
  if (state.view === "state" && state.registeredState.follow && state.caps.state) {
    state.registeredState.timer = setInterval(() => loadRegisteredState(state.registeredState.offset, { quiet: true }), 2000);
  }
  syncDebuggerTimer();
}

export function syncDebuggerTimer() {
  stopTimer("debugger");
  if (document.hidden || !state.connected || !state.token || !state.targetId || state.view !== "debugger") return;
  const dbg = state.debugger;
  if (!dbg.attached || dbg.unavailable || !state.caps.runtime) return;
  dbg.timer = setInterval(() => refreshDebugger({ quiet: true, poll: true }), DEBUGGER_POLL_MS);
}

export function resetNetwork() {
  stopTimer("network");
  state.network = {
    loaded: false, loading: false, error: null, records: [], limit: 0, follow: dom.networkFollow.checked, timer: null,
    recording: null, recordingPending: false, recordingRequest: null, clearPending: false,
    controlError: null, controlUnavailable: false, cleared: false, generation: 0, recordingEpoch: 0,
    readAt: 0, waterfall: true,
  };
  dom.networkMeta.textContent = "Recorded in-process HTTP activity.";
  dom.networkBadge.hidden = true;
  if (state.view === "network") renderNetwork();
}

export function ensureNetwork() {
  if (!state.token || !state.targetId) {
    renderNetwork();
    return;
  }
  if (!state.caps.network) {
    renderNetwork();
    return;
  }
  if (!state.network.loaded && !state.network.loading) loadNetwork();
  else renderNetwork();
}

export async function loadNetwork(options = {}) {
  const quiet = options.quiet === true;
  const net = state.network;
  if (!state.caps.network || !state.targetId || net.loading || net.clearPending) return;
  const targetId = state.targetId;
  const epoch = state.targetEpoch;
  const generation = net.generation;
  const recordingEpoch = net.recordingEpoch;
  net.loading = true;
  net.error = null;
  renderNetwork();
  try {
    const result = await rpc("query_ui_network", { target: targetId });
    if (epoch !== state.targetEpoch || targetId !== state.targetId || generation !== net.generation) return;
    const data = unwrapData(result);
    const records = Array.isArray(data?.records) ? data.records : [];
    net.records = records;
    net.limit = numberValue(data?.limit) ?? 0;
    net.loaded = true;
    net.error = null;
    // The wall clock at which the runtime's snapshot was read. A pending
    // request has no recorded end, so this is the only honest right edge for
    // its bar: the request was still in flight at this instant.
    net.readAt = Date.now();
    if (records.length) net.cleared = false;
    // A read that started before a control call carries the pre-call state; it
    // may merge records but must not overwrite the confirmed recording state.
    if (!net.recordingPending && recordingEpoch === net.recordingEpoch) {
      const recording = booleanOf(data?.recording) ?? booleanOf(data?.enabled);
      if (recording !== null) net.recording = recording;
    }
    dom.networkBadge.hidden = records.length === 0;
    dom.networkBadge.textContent = String(records.length);
  } catch (error) {
    if (epoch !== state.targetEpoch || targetId !== state.targetId || generation !== net.generation) return;
    net.error = error;
    if (!quiet) toast(errorMessage(error), "error", 8000);
  } finally {
    if (epoch === state.targetEpoch && targetId === state.targetId && generation === net.generation) {
      net.loading = false;
      renderNetwork();
    }
  }
}

export function booleanOf(value) {
  return typeof value === "boolean" ? value : null;
}

function networkControlUnavailable(error) {
  if (!error) return false;
  if (["unknown_command", "unknown_tool", "method_not_found", "unsupported", "not_supported", "not-supported"].includes(error.code)) return true;
  return /does not (?:advertise|expose|support)|not supported|unsupported/i.test(error.message || "");
}

function networkControlsAvailable() {
  return Boolean(state.targetId && state.caps.network && state.caps.networkControl && !state.network.controlUnavailable);
}

export async function setNetworkRecording(enabled) {
  const net = state.network;
  if (!networkControlsAvailable() || !state.token || !state.connected) return;
  if (net.recordingPending || net.clearPending) return;
  if (net.recording === enabled) return;
  const targetId = state.targetId;
  const epoch = state.targetEpoch;
  net.recordingPending = true;
  net.recordingRequest = enabled;
  net.controlError = null;
  renderNetwork();
  try {
    const data = unwrapData(await rpc("set_ui_network_recording", { target: targetId, enabled }));
    if (epoch !== state.targetEpoch || targetId !== state.targetId || state.network !== net) return;
    // The reply carries the state the recorder actually holds; never assume the
    // requested value won.
    const confirmed = booleanOf(data?.enabled);
    net.recording = confirmed === null ? enabled : confirmed;
    net.recordingEpoch += 1;
    net.controlError = null;
    toast(net.recording ? "Network recording resumed." : "Network recording paused. Existing requests are retained.", "success", 4000);
  } catch (error) {
    if (epoch !== state.targetEpoch || targetId !== state.targetId || state.network !== net) return;
    net.controlError = error;
    if (networkControlUnavailable(error)) net.controlUnavailable = true;
    toast(`${enabled ? "Resume" : "Pause"} failed: ${errorMessage(error)}`, "error", 9000);
  } finally {
    if (epoch === state.targetEpoch && targetId === state.targetId && state.network === net) {
      net.recordingPending = false;
      net.recordingRequest = null;
      renderNetwork();
    }
  }
}

export async function clearNetwork() {
  const net = state.network;
  if (!networkControlsAvailable() || !state.token || !state.connected) return;
  if (net.clearPending || net.recordingPending) return;
  const targetId = state.targetId;
  const epoch = state.targetEpoch;
  // Reads that started before the clear describe the retired window, so they
  // must not merge records back into the cleared table.
  net.generation += 1;
  net.clearPending = true;
  net.loading = false;
  net.clearError = null;
  net.error = null;
  net.controlError = null;
  renderNetwork();
  let cleared = false;
  try {
    const data = unwrapData(await rpc("clear_ui_network", { target: targetId }));
    if (epoch !== state.targetEpoch || targetId !== state.targetId || state.network !== net) return;
    const count = numberValue(data?.cleared);
    net.records = [];
    net.limit = numberValue(data?.limit) ?? net.limit;
    net.loaded = true;
    net.error = null;
    net.clearError = null;
    net.cleared = true;
    dom.networkBadge.hidden = true;
    dom.networkBadge.textContent = "0";
    cleared = true;
    toast(count !== null && count > 0
      ? `${count} request${count === 1 ? "" : "s"} cleared.`
      : "Network history cleared.", "success", 4000);
  } catch (error) {
    if (epoch !== state.targetEpoch || targetId !== state.targetId || state.network !== net) return;
    net.clearError = error;
    if (networkControlUnavailable(error)) net.controlUnavailable = true;
    net.loaded = true;
    toast(`Clear failed: ${errorMessage(error)}`, "error", 9000);
  } finally {
    if (epoch === state.targetEpoch && targetId === state.targetId && state.network === net) {
      net.clearPending = false;
      renderNetwork();
      if (cleared && net.follow) loadNetwork({ quiet: true });
    }
  }
}

function setNetworkChrome() {
  const net = state.network;
  const target = targetById();
  const offline = !state.token || !state.connected;
  const unavailable = !target || !state.caps.network || !state.caps.networkControl || net.controlUnavailable;
  const pending = net.recordingPending || net.clearPending;
  dom.networkRecordingToggle.disabled = offline || unavailable || pending;
  dom.networkClear.disabled = offline || unavailable || pending;
  dom.networkRefresh.disabled = pending;
  dom.networkRecordingToggle.setAttribute("aria-busy", String(net.recordingPending));
  dom.networkClear.setAttribute("aria-busy", String(net.clearPending));
  dom.networkContent.setAttribute("aria-busy", String(net.clearPending));
  let recordingTitle;
  if (offline) recordingTitle = "Reconnect to the controller to control network recording";
  else if (!target) recordingTitle = "Select a connected target to control network recording";
  else if (!state.caps.network) recordingTitle = "Network records come from a Cove runtime. Select a runtime target";
  else if (!state.caps.networkControl || net.controlUnavailable) recordingTitle = "This runtime does not expose network recording controls";
  else if (net.recordingPending) recordingTitle = net.recordingRequest ? "Resuming network recording" : "Pausing network recording";
  else recordingTitle = net.recording === false ? "Resume recording new HTTP requests" : "Pause recording new HTTP requests";
  dom.networkRecordingToggle.title = recordingTitle;
  const paused = net.recording === false;
  const recordingLabel = net.recordingPending
    ? (net.recordingRequest ? "Resuming…" : "Pausing…")
    : (paused ? "Resume" : "Pause");
  const label = dom.networkRecordingToggle.querySelector("span");
  if (label) label.textContent = recordingLabel;
  const use = dom.networkRecordingToggle.querySelector("use");
  if (use) use.setAttribute("href", paused ? "#i-play" : "#i-pause");
  let clearTitle;
  if (offline) clearTitle = "Reconnect to the controller to clear network records";
  else if (!target) clearTitle = "Select a connected target to clear network records";
  else if (!state.caps.network) clearTitle = "Network records come from a Cove runtime. Select a runtime target";
  else if (!state.caps.networkControl || net.controlUnavailable) clearTitle = "This runtime does not expose network recording controls";
  else if (net.clearPending) clearTitle = "Clearing the runtime's retained network records";
  else clearTitle = "Clear the runtime's retained network records";
  dom.networkClear.title = clearTitle;
  const clearLabel = dom.networkClear.querySelector("span");
  if (clearLabel) clearLabel.textContent = net.clearPending ? "Clearing…" : "Clear";
}

function networkMatches(record) {
  const query = dom.networkSearch.value.trim().toLowerCase();
  const stateFilter = dom.networkStateFilter.value;
  if (stateFilter !== "all" && record.state !== stateFilter) return false;
  if (!query) return true;
  return [record.method, record.url, record.status, record.state, record.error]
    .filter(value => value !== undefined && value !== null)
    .join(" ").toLowerCase().includes(query);
}

function networkWaterfallRows(records, readAt) {
  const rows = [];
  let unplaced = 0;
  for (const record of records) {
    const start = numberValue(record.startedAt);
    if (start === null) {
      unplaced += 1;
      continue;
    }
    const ttfb = numberValue(record.ttfbMs);
    const duration = numberValue(record.durationMs);
    const pending = record.state === "pending";
    // A pending request has no recorded end. The read instant is the only
    // honest right edge: it was still in flight then, so its duration is at
    // least this much, never an invented completion.
    const elapsed = pending
      ? Math.max(0, readAt - start, ttfb ?? 0)
      : Math.max(0, duration ?? 0, ttfb ?? 0);
    rows.push({ record, start, ttfb, duration, pending, elapsed, end: start + elapsed });
  }
  rows.sort((left, right) => left.start - right.start || Number(left.record.id ?? 0) - Number(right.record.id ?? 0));
  return { rows, unplaced };
}

function renderNetworkWaterfall(records) {
  const net = state.network;
  const readAt = net.readAt || Date.now();
  const { rows, unplaced } = networkWaterfallRows(records, readAt);
  if (!rows.length) {
    return el("div", { class: "panel-block" },
      el("div", { class: "panel-block-head" }, el("h2", { text: "Waterfall" })),
      el("div", { class: "panel-block-body" },
        emptyInline("No request carries a start timestamp, so nothing can be placed on a time axis."),
      ),
    );
  }
  const origin = rows[0].start;
  const latest = rows.reduce((maximum, row) => Math.max(maximum, row.end), origin);
  const span = Math.max(latest - origin, 1);
  const percent = value => Math.max(0, Math.min(100, (value / span) * 100));
  const list = el("div", {
    class: "waterfall-rows",
    attrs: {
      role: "listbox",
      "aria-label": `Request waterfall for ${rows.length} requests over ${fmtMs(span, 1)}.`,
    },
  });
  for (const [index, row] of rows.entries()) {
    const { record, start, ttfb, pending, elapsed } = row;
    const offset = start - origin;
    const status = numberValue(record.status);
    const failed = record.state === "failed" || (status !== null && status >= 400);
    const segments = [];
    const head = ttfb === null ? null : Math.min(ttfb, elapsed);
    if (head !== null && head > 0) {
      segments.push({ kind: "waiting", from: 0, to: head, label: `Waiting for response head ${fmtMs(head)}` });
    }
    if (pending) {
      segments.push({
        kind: "inflight",
        from: head ?? 0,
        to: elapsed,
        label: head === null
          ? `In flight ${fmtMs(elapsed)}; the response head had not arrived when the runtime was read`
          : `Receiving body, still in flight after ${fmtMs(elapsed)}`,
      });
    } else if (elapsed > (head ?? 0)) {
      segments.push({
        kind: head === null ? "unsplit" : "receiving",
        from: head ?? 0,
        to: elapsed,
        label: head === null
          ? `Total ${fmtMs(elapsed)}; the recorder did not capture a response-head arrival, so no wait/receive split is drawn`
          : `Receiving body ${fmtMs(elapsed - head)}`,
      });
    }
    if (!segments.length) {
      segments.push({ kind: failed ? "failed" : "receiving", from: 0, to: elapsed, label: `Total ${fmtMs(elapsed)}` });
    }
    const describe = `${textOf(record.method, "?")} ${textOf(record.url, "")}: started ${fmtEpochMs(start)}, `
      + `${pending ? "in flight" : textOf(record.state, "unknown")}, `
      + `${ttfb === null ? "head arrival not recorded" : `head ${fmtMs(ttfb)}`}, `
      + `${pending ? `at least ${fmtMs(elapsed)}` : `total ${fmtMs(elapsed)}`}`
      + `${failed ? ", failed" : ""}`;
    const bar = el("div", {
      class: `waterfall-bar${failed ? " is-failed" : ""}${pending ? " is-pending" : ""}`,
      attrs: { style: `left:${percent(offset)}%;width:${percent(elapsed)}%` },
    });
    for (const segment of segments) {
      const width = segment.to - segment.from;
      bar.append(el("span", {
        class: `waterfall-seg is-${segment.kind}`,
        attrs: { style: `flex:${Math.max(width, 0.001)} 1 0`, title: segment.label },
      }));
    }
    const option = el("div", {
      class: `waterfall-row${failed ? " is-failed" : ""}`,
      attrs: {
        role: "option",
        "aria-selected": "false",
        tabindex: index === 0 ? "0" : "-1",
        "aria-label": describe,
        title: describe,
        "data-request": String(record.id ?? index),
      },
      on: { click: () => showNetworkRecord(record) },
    },
      el("span", { class: "waterfall-label" },
        el("span", { class: "waterfall-method", text: textOf(record.method, "?") }),
        el("span", { class: "waterfall-url", text: textOf(record.url, "(no URL)") }),
      ),
      el("span", { class: "waterfall-track" }, bar),
      el("span", { class: "waterfall-duration", text: pending ? `≥ ${fmtMs(elapsed, 1)}` : fmtMs(elapsed, 1) }),
    );
    list.append(option);
  }
  list.addEventListener("keydown", event => {
    const current = event.target.closest?.(".waterfall-row");
    if (!current) return;
    const options = [...list.children];
    const index = options.indexOf(current);
    let next = null;
    if (event.key === "ArrowDown") next = options[Math.min(options.length - 1, index + 1)];
    else if (event.key === "ArrowUp") next = options[Math.max(0, index - 1)];
    else if (event.key === "Home") next = options[0];
    else if (event.key === "End") next = options[options.length - 1];
    else if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      current.click();
      return;
    }
    if (!next) return;
    event.preventDefault();
    for (const option of options) option.tabIndex = -1;
    next.tabIndex = 0;
    next.focus();
  });
  const ticks = [0, 0.25, 0.5, 0.75, 1].map(fraction => el("span", {
    class: "waterfall-tick",
    attrs: { style: `left:${fraction * 100}%` },
    text: fmtMs(span * fraction, span < 10 ? 2 : 0),
  }));
  const notes = [
    "Cove's recorder timestamps the request start, the response head and the body end. It captures no DNS, connect, TLS or queueing phases, so none are drawn here.",
  ];
  if (rows.some(row => row.pending)) {
    notes.push(`In-flight requests end at the read instant ${fmtEpochMs(readAt)}. That is a lower bound on their duration, not a recorded completion.`);
  }
  if (rows.some(row => row.ttfb === null && !row.pending)) {
    notes.push("Requests without a recorded head arrival show one undivided bar; the wait/receive split is not estimated.");
  }
  if (unplaced) {
    notes.push(`${unplaced} request${unplaced === 1 ? "" : "s"} carried no start timestamp and ${unplaced === 1 ? "is" : "are"} omitted from the axis.`);
  }
  return el("div", { class: "panel-block" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Waterfall" }),
      el("span", { class: "pane-meta", text: `${rows.length} request${rows.length === 1 ? "" : "s"} · ${fmtMs(span, 1)} span · from ${fmtEpochMs(origin)}` }),
    ),
    el("div", { class: "panel-block-body" },
      el("div", { class: "waterfall" },
        el("div", { class: "waterfall-axis" },
          el("span", { class: "waterfall-axis-spacer" }),
          el("span", { class: "waterfall-axis-track" }, ...ticks),
          el("span", { class: "waterfall-axis-spacer is-duration" }),
        ),
        list,
      ),
      el("div", { class: "perf-legend" },
        el("span", { class: "is-waiting", text: "Waiting for head" }),
        el("span", { class: "is-receiving", text: "Receiving body" }),
        el("span", { class: "is-inflight", text: "In flight" }),
        el("span", { class: "is-unsplit", text: "Head not recorded" }),
        el("span", { class: "is-failed", text: "Failed" }),
      ),
      ...notes.map(note => el("div", { class: "perf-note", text: note })),
    ),
  );
}

export function renderNetwork() {
  const host = dom.networkContent;
  clear(host);
  setNetworkChrome();
  const net = state.network;
  const target = targetById();
  const paused = net.recording === false;
  dom.networkRecordingStatus.hidden = !paused;
  if (!target) {
    dom.networkMeta.textContent = "No target selected.";
    showState(host, { icon: "device", title: "No target", message: "Select a connected target to read network activity." });
    return;
  }
  if (!state.caps.network) {
    dom.networkMeta.textContent = "Unavailable for this target.";
    showState(host, {
      icon: "network",
      title: "Network is unavailable",
      message: state.caps.runtime ? "This runtime does not advertise network recording." : "Network records come from a Cove runtime. Select a runtime target.",
    });
    return;
  }
  const controlsMissing = !state.caps.networkControl || net.controlUnavailable;
  const chrome = () => {
    const nodes = [];
    if (controlsMissing) {
      nodes.push(el("div", {
        class: "warn-inline network-note",
        attrs: { role: "note" },
        text: state.caps.networkControl
          ? "This runtime does not expose network recording controls."
          : "This runtime does not advertise network recording controls.",
      }));
    }
    return nodes;
  };
  if (net.clearPending && !net.records.length) {
    dom.networkMeta.textContent = paused ? "Clearing… · recording paused" : "Clearing…";
    showState(host, { kind: "loading", title: "Clearing network history" });
    host.prepend(...chrome());
    return;
  }
  if (net.loading && !net.loaded) {
    dom.networkMeta.textContent = "Loading…";
    showState(host, { kind: "loading", title: "Reading network activity" });
    host.prepend(...chrome());
    return;
  }
  if (net.error && !net.records.length) {
    dom.networkMeta.textContent = "Error";
    showState(host, {
      kind: "error",
      title: "Network read failed",
      message: errorMessage(net.error),
      action: "Retry",
      onAction: () => isDisconnectedError(net.error) ? refreshCurrent() : loadNetwork(),
    });
    host.prepend(...chrome());
    return;
  }
  if (net.clearError && !net.records.length) {
    dom.networkMeta.textContent = "Clear failed";
    showState(host, {
      kind: "error",
      title: "Clear failed",
      message: errorMessage(net.clearError),
      action: "Retry",
      onAction: () => clearNetwork(),
    });
    host.prepend(...chrome());
    return;
  }
  if (!net.records.length) {
    if (paused) {
      dom.networkMeta.textContent = "0 requests · recording paused";
      showState(host, {
        icon: "pause",
        title: "Recording paused",
        message: "No requests are being recorded. Existing requests stay in the table; resume to capture new ones.",
        action: networkControlsAvailable() && state.connected ? "Resume" : null,
        onAction: () => setNetworkRecording(true),
      });
      host.prepend(...chrome());
      return;
    }
    dom.networkMeta.textContent = net.cleared ? "Cleared" : "0 requests";
    showState(host, {
      icon: "network",
      title: net.cleared ? "Network history cleared" : "No HTTP activity",
      message: net.cleared
        ? (net.follow ? "Waiting for requests issued after the clear." : "Requests issued after the clear will appear here.")
        : "Requests issued by this runtime appear here. Recording may be disabled in this build.",
      action: "Refresh",
      onAction: () => loadNetwork(),
    });
    host.prepend(...chrome());
    return;
  }
  const filtered = net.records.filter(networkMatches);
  const parts = [];
  if (net.clearPending) parts.push("Clearing…");
  parts.push(`${net.records.length} request${net.records.length === 1 ? "" : "s"}`);
  if (filtered.length !== net.records.length) parts.push(`${filtered.length} shown`);
  if (paused) parts.push("recording paused");
  if (controlsMissing) parts.push("controls unavailable");
  if (net.limit) parts.push(`buffer ${net.limit}`);
  dom.networkMeta.textContent = parts.join(" · ");
  if (!filtered.length) {
    showState(host, { icon: "search", title: "No matching requests", message: "Change the URL filter or request state." });
    host.prepend(...chrome());
    return;
  }
  host.append(...chrome());
  if (net.clearError) host.append(errorInline(`Clear failed: ${errorMessage(net.clearError)}`));
  if (net.controlError) host.append(errorInline(`Recording control failed: ${errorMessage(net.controlError)}`));
  if (net.error) host.append(errorInline(errorMessage(net.error)));
  if (net.waterfall) host.append(renderNetworkWaterfall(filtered));
  const table = el("table", { class: "data-table" },
    el("thead", {}, el("tr", {},
      el("th", { text: "Time" }),
      el("th", { text: "Method" }),
      el("th", { text: "Status" }),
      el("th", { text: "URL" }),
      el("th", { text: "State" }),
      el("th", { class: "num", text: "TTFB" }),
      el("th", { class: "num", text: "Duration" }),
      el("th", { class: "num", text: "Bytes" }),
    )),
    el("tbody", {}, ...filtered.map(record => {
      const status = numberValue(record.status);
      const statusClass = status === null ? "" : status >= 500 ? "bad" : status >= 400 ? "warn" : "ok";
      return el("tr", {},
        el("td", { class: "mono-cell", text: fmtEpochMs(record.startedAt), attrs: { title: record.startedAt ? new Date(record.startedAt).toLocaleString() : "" } }),
        el("td", { class: "method", text: textOf(record.method, "?") }),
        el("td", {}, el("span", { class: `status-code ${statusClass}`, text: status === null ? "—" : String(status) })),
        el("td", { class: "url-cell" }, el("button", {
          class: "link-button",
          props: { type: "button", title: record.url || "" },
          text: textOf(record.url, "(no URL)"),
          on: { click: () => showNetworkRecord(record) },
        })),
        el("td", {}, el("span", { class: `state-badge ${record.state || ""}`, text: textOf(record.state, "unknown") })),
        el("td", { class: "num", text: fmtMs(record.ttfbMs) }),
        el("td", { class: "num", text: fmtMs(record.durationMs) }),
        el("td", { class: "num", text: fmtBytes(record.responseBytes), attrs: { title: `request ${fmtBytes(record.requestBytes)} · response ${fmtBytes(record.responseBytes)}` } }),
      );
    })),
  );
  host.append(el("div", { class: "table-wrap" }, table));
}

function headerPairs(headers) {
  if (!headers) return [];
  if (Array.isArray(headers)) {
    return headers.map(entry => Array.isArray(entry) ? [String(entry[0]), String(entry[1])] : [String(entry?.name ?? ""), String(entry?.value ?? "")]);
  }
  if (typeof headers === "object") return Object.entries(headers).map(([name, value]) => [name, String(value)]);
  return [];
}

function renderHeaderPairs(pairs) {
  if (!pairs.length) return emptyInline("No headers were captured.");
  return el("div", { class: "header-pairs" }, ...pairs.flatMap(([name, value]) => [
    el("span", { class: "name", text: name }),
    el("span", { class: "value", text: value }),
  ]));
}

function showNetworkRecord(record) {
  dom.networkDialogTitle.textContent = `${textOf(record.method, "?")} ${textOf(record.url, "")}`;
  const body = dom.networkDialogBody;
  clear(body);
  body.append(
    sectionBlock("Request",
      kvList([
        ["ID", String(record.id ?? "—")],
        ["URL", el("span", { class: "mono", text: textOf(record.url, "—") })],
        ["Status", record.status ?? "—"],
        ["State", record.state || "—"],
        ["Error", record.error || "—"],
        ["Started", record.startedAt ? `${fmtEpochMs(record.startedAt)} · ${new Date(record.startedAt).toLocaleString()}` : "—"],
        ["TTFB", fmtMs(record.ttfbMs)],
        ["Duration", fmtMs(record.durationMs)],
        ["Request bytes", fmtBytes(record.requestBytes)],
        ["Response bytes", fmtBytes(record.responseBytes)],
      ]),
      el("div", { class: "source-actions" },
        el("button", { class: "tool-button", props: { type: "button" }, text: "Copy URL", on: { click: () => copyText(record.url, "URL") } }),
      ),
    ),
    sectionBlock("Request headers", renderHeaderPairs(headerPairs(record.requestHeaders))),
    sectionBlock("Response headers", renderHeaderPairs(headerPairs(record.responseHeaders))),
    sectionBlock("Request body",
      record.requestBody ? el("pre", { class: "body-block", text: record.requestBody }) : emptyInline("No request body was captured."),
      record.requestBodyTruncated ? el("div", { class: "truncation", text: "The request body was truncated at the recorder's byte limit." }) : null,
    ),
    sectionBlock("Response body",
      record.responseBody ? el("pre", { class: "body-block", text: record.responseBody }) : emptyInline("No response body was captured."),
      record.responseBodyTruncated ? el("div", { class: "truncation", text: "The response body was truncated at the recorder's byte limit." }) : null,
    ),
  );
  dom.networkDialog.showModal();
  dom.networkDialogClose.focus();
}

