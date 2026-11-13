/* Logs */

function resetLogs() {
  stopTimer("logs");
  state.logs = {
    loaded: false, loading: false, error: null, records: [], dropped: 0, firstAvailable: null,
    follow: dom.logsFollow.checked, timer: null, clearing: false, clearError: null, generation: 0,
    cleared: false,
  };
  dom.logsMeta.textContent = "Recent runtime log records.";
  dom.logsLabel.value = "";
  dom.logsBadge.hidden = true;
  if (state.view === "logs") renderLogs();
}

function ensureLogs() {
  if (!state.token || !state.targetId || !state.caps.logs) {
    renderLogs();
    return;
  }
  if (!state.logs.loaded && !state.logs.loading) loadLogs({ reset: true });
  else renderLogs();
}

async function loadLogs(options = {}) {
  const quiet = options.quiet === true;
  const logs = state.logs;
  if (!state.caps.logs || !state.targetId || logs.loading || logs.clearing) return;
  const targetId = state.targetId;
  const epoch = state.targetEpoch;
  const generation = logs.generation;
  if (options.reset) {
    logs.records = [];
    logs.dropped = 0;
    logs.firstAvailable = null;
    logs.loaded = false;
    logs.cleared = false;
    logs.clearError = null;
  }
  logs.loading = true;
  logs.error = null;
  renderLogs();
  try {
    const after = logs.records.reduce((maximum, record) => Math.max(maximum, numberValue(record.sequence) ?? 0), 0);
    const params = { target: targetId };
    if (after > 0) params.after = after;
    const data = unwrapData(await rpc("query_ui_logs", params));
    if (epoch !== state.targetEpoch || targetId !== state.targetId || generation !== logs.generation) return;
    const incoming = Array.isArray(data?.records) ? data.records : [];
    const merged = new Map(logs.records.map(record => [record.sequence, record]));
    for (const record of incoming) merged.set(record.sequence, record);
    let records = [...merged.values()].sort((left, right) => (numberValue(left.sequence) ?? 0) - (numberValue(right.sequence) ?? 0));
    const capped = records.length > 500;
    if (capped) records = records.slice(-500);
    logs.records = records;
    logs.capped = capped;
    logs.dropped = numberValue(data?.dropped) ?? logs.dropped;
    logs.firstAvailable = data?.firstAvailableSequence ?? logs.firstAvailable;
    logs.loaded = true;
    logs.error = null;
    if (records.length) logs.cleared = false;
    dom.logsBadge.hidden = records.length === 0;
    dom.logsBadge.textContent = String(records.length);
  } catch (error) {
    if (epoch !== state.targetEpoch || targetId !== state.targetId || generation !== logs.generation) return;
    logs.error = error;
    if (!quiet) toast(errorMessage(error), "error", 8000);
  } finally {
    if (epoch === state.targetEpoch && targetId === state.targetId && generation === logs.generation) {
      logs.loading = false;
      renderLogs();
    }
  }
}

async function clearLogs() {
  const logs = state.logs;
  if (!state.caps.logs || !state.targetId || !state.token || !state.connected || logs.clearing) return;
  const targetId = state.targetId;
  const epoch = state.targetEpoch;
  // Responses from reads that started before the clear describe the old replay
  // window, so they must not merge records back into the cleared table.
  logs.generation += 1;
  logs.clearing = true;
  logs.loading = false;
  logs.clearError = null;
  logs.error = null;
  renderLogs();
  let cleared = false;
  try {
    const data = unwrapData(await rpc("clear_ui_logs", { target: targetId }));
    if (epoch !== state.targetEpoch || targetId !== state.targetId || state.logs !== logs) return;
    const count = numberValue(data?.cleared);
    logs.records = [];
    logs.capped = false;
    logs.dropped = numberValue(data?.dropped) ?? 0;
    logs.firstAvailable = data?.firstAvailableSequence ?? null;
    logs.loaded = true;
    logs.error = null;
    logs.clearError = null;
    logs.cleared = true;
    dom.logsBadge.hidden = true;
    dom.logsBadge.textContent = "0";
    cleared = true;
    toast(count !== null && count > 0
      ? `${count} log record${count === 1 ? "" : "s"} cleared.`
      : "Log history cleared.", "success", 4000);
  } catch (error) {
    if (epoch !== state.targetEpoch || targetId !== state.targetId || state.logs !== logs) return;
    logs.clearError = error;
    logs.loaded = true;
    toast(`Clear failed: ${errorMessage(error)}`, "error", 9000);
  } finally {
    if (epoch === state.targetEpoch && targetId === state.targetId && state.logs === logs) {
      logs.clearing = false;
      renderLogs();
      syncFollowTimers();
      if (cleared && logs.follow) loadLogs({ quiet: true });
    }
  }
}

function setLogsChrome() {
  const logs = state.logs;
  const target = targetById();
  const offline = !state.token || !state.connected;
  const unavailable = !target || !state.caps.logs;
  dom.logsClear.disabled = offline || unavailable || logs.clearing;
  dom.logsRefresh.disabled = logs.clearing;
  dom.logsClear.setAttribute("aria-busy", String(logs.clearing));
  dom.logsContent.setAttribute("aria-busy", String(logs.clearing));
  let title = "Clear the runtime's replay log history";
  if (offline) title = "Reconnect to the controller to clear logs";
  else if (!target) title = "Select a connected target to clear logs";
  else if (!state.caps.logs) title = state.caps.runtime
    ? "This runtime does not advertise diagnostics"
    : "Logs come from a Cove runtime. Select a runtime target";
  else if (logs.clearing) title = "Clearing the runtime's replay log history";
  dom.logsClear.title = title;
  const label = dom.logsClear.querySelector("span");
  if (label) label.textContent = logs.clearing ? "Clearing…" : "Clear";
}

function logMatches(record) {
  if (dom.logsLabel.value && record.label !== dom.logsLabel.value) return false;
  const minimum = LEVELS.indexOf(dom.logsLevel.value);
  const levelIndex = LEVELS.indexOf(record.level);
  if (minimum >= 0 && levelIndex >= 0 && levelIndex < minimum) return false;
  const query = dom.logsSearch.value.trim().toLowerCase();
  if (!query) return true;
  const location = record.location || {};
  const process = record.process || {};
  return [record.level, record.label, record.message, location.file, location.function, location.source, process.name, process.runID]
    .filter(Boolean).join(" ").toLowerCase().includes(query);
}

function renderLogs() {
  const host = dom.logsContent;
  clear(host);
  setLogsChrome();
  const selectedLabel = dom.logsLabel.value;
  const labels = [...new Set(state.logs.records.map(record => record.label).filter(Boolean))].sort();
  if (selectedLabel && !labels.includes(selectedLabel)) labels.push(selectedLabel);
  dom.logsLabel.replaceChildren(el("option", { text: "All labels", attrs: { value: "" } }),
    ...labels.map(label => el("option", { text: label, attrs: { value: label } })));
  dom.logsLabel.value = selectedLabel;
  dom.logsLabel.disabled = !state.caps.logs || !labels.length;
  const target = targetById();
  if (!target) {
    dom.logsMeta.textContent = "No target selected.";
    showState(host, { icon: "device", title: "No target", message: "Select a connected target to read logs." });
    return;
  }
  if (!state.caps.logs) {
    dom.logsMeta.textContent = "Unavailable for this target.";
    showState(host, {
      icon: "logs",
      title: "Logs are unavailable",
      message: state.caps.runtime ? "This runtime does not advertise diagnostics." : "Logs come from a Cove runtime. Select a runtime target.",
    });
    return;
  }
  if (state.logs.clearing && !state.logs.records.length) {
    dom.logsMeta.textContent = "Clearing…";
    showState(host, { kind: "loading", title: "Clearing log history" });
    return;
  }
  if (state.logs.loading && !state.logs.loaded) {
    dom.logsMeta.textContent = "Loading…";
    showState(host, { kind: "loading", title: "Reading logs" });
    return;
  }
  if (state.logs.error && !state.logs.records.length) {
    dom.logsMeta.textContent = "Error";
    showState(host, {
      kind: "error",
      title: "Log read failed",
      message: errorMessage(state.logs.error),
      action: "Retry",
      onAction: () => isDisconnectedError(state.logs.error) ? refreshCurrent() : loadLogs({ reset: true }),
    });
    return;
  }
  if (state.logs.clearError && !state.logs.records.length) {
    dom.logsMeta.textContent = "Clear failed";
    showState(host, {
      kind: "error",
      title: "Clear failed",
      message: errorMessage(state.logs.clearError),
      action: "Retry",
      onAction: () => clearLogs(),
    });
    return;
  }
  if (!state.logs.records.length) {
    const wasCleared = state.logs.cleared;
    dom.logsMeta.textContent = wasCleared ? "Cleared" : "0 records";
    showState(host, {
      icon: "logs",
      title: wasCleared ? "Log history cleared" : "No log records",
      message: wasCleared
        ? (state.logs.follow ? "Waiting for records logged after the clear." : "Records logged after the clear will appear here.")
        : "The runtime has not published log records yet.",
      action: "Refresh",
      onAction: () => loadLogs({ reset: true }),
    });
    return;
  }
  const filtered = state.logs.records.filter(logMatches).slice().reverse();
  const parts = [];
  if (state.logs.clearing) parts.push("Clearing…");
  parts.push(`${state.logs.records.length} record${state.logs.records.length === 1 ? "" : "s"}`);
  if (filtered.length !== state.logs.records.length) parts.push(`${filtered.length} shown`);
  if (state.logs.dropped) parts.push(`${state.logs.dropped} dropped`);
  if (state.logs.capped) parts.push("retaining last 500");
  if (state.logs.firstAvailable) parts.push(`first available #${state.logs.firstAvailable}`);
  dom.logsMeta.textContent = parts.join(" · ");
  if (state.logs.clearError) host.append(errorInline(`Clear failed: ${errorMessage(state.logs.clearError)}`));
  if (state.logs.error) host.append(errorInline(errorMessage(state.logs.error)));
  if (!filtered.length) {
    showState(host, { icon: "search", title: "No matching logs", message: "Change the level or text filter." });
    return;
  }
  const list = el("div", { class: "log-list" });
  for (const record of filtered) {
    const level = LEVELS.includes(record.level) ? record.level : "info";
    const location = record.location || {};
    const source = location.file ? `${basename(location.file)}:${location.line ?? "?"}` : (location.source || location.function || "");
    const summary = el("summary", {},
      el("span", { class: "log-time", text: fmtClock(record.timestamp), attrs: { title: fmtDate(record.timestamp) } }),
      el("span", { class: `log-level ${level}`, text: level }),
      el("span", { class: "log-label", text: textOf(record.label, "—"), attrs: { title: record.label || "" } }),
      el("span", { class: "log-message", text: textOf(record.message, "") }),
      el("span", { class: "log-source", text: source, attrs: { title: [location.file, location.function, location.line].filter(Boolean).join(":") } }),
    );
    const detail = el("div", { class: "log-detail" },
      kvList([
        ["Sequence", String(record.sequence ?? "—")],
        ["Timestamp", fmtDate(record.timestamp)],
        ["Label", record.label || "—"],
        ["Location", [location.file, location.function, location.line, location.column].filter(value => value !== undefined && value !== null && value !== "").join(":") || "—"],
        ["Process", record.process ? `${record.process.name || "?"} · pid ${record.process.pid ?? "?"}` : "—"],
        ["Run", record.process?.runID || "—"],
      ]),
    );
    if (record.metadata && Object.keys(record.metadata).length) {
      detail.append(el("div", { class: "section" }, el("h3", { text: "Metadata" }), el("pre", { text: JSON.stringify(record.metadata, null, 2) })));
    }
    const entry = el("details", { class: "log-entry" }, summary, detail);
    list.append(entry);
  }
  host.append(list);
}

