function resetTargetData(previousTarget = state.targetId) {
  state.tree = null;
  state.entries = new Map();
  state.rootId = null;
  state.treeLoaded = false;
  state.treeLoading = false;
  state.treeError = null;
  state.treeRequest += 1;
  state.selectedId = null;
  state.detail = null;
  state.detailLoading = false;
  state.detailError = null;
  state.detailRequest += 1;
  state.valueRequested = false;
  clearTimeout(state.detailTimer);
  state.detailTimer = null;
  clearTimeout(state.highlightTimer);
  state.highlightTimer = null;
  state.expanded.clear();
  state.search = "";
  dom.treeSearch.value = "";
  state.visibleModel = null;
  state.visibleRows = [];
  state.mode = "select";
  releaseCapture();
  state.captureLoading = false;
  state.captureError = null;
  state.captureAttempt = "window";
  resetPerformance(previousTarget);
  resetNetwork();
  resetLogs();
  resetRegisteredState();
  resetExtensions();
  resetMemory(previousTarget);
  resetAppSize();
  resetDebugger(previousTarget);
  resetRepaintHighlight(previousTarget);
  resetDebugPaintState(previousTarget);
  resetBaselines(previousTarget);
  resetSlowAnimations(previousTarget);
  resetOversizedImages(previousTarget);
  resetFlexEditorState();
  stopAllFollowTimers();
  clear(dom.tree);
  clear(dom.detailsState);
  dom.detailsContent.hidden = true;
  dom.treeState.hidden = false;
  dom.tree.hidden = true;
  dom.treeFooter.hidden = true;
  dom.treeMeta.textContent = "";
  dom.screenshotMeta.textContent = "";
  dom.networkBadge.hidden = true;
  dom.logsBadge.hidden = true;
  if (dom.networkDialog.open) dom.networkDialog.close();
}

function ensureCurrentView() {
  if (state.view === "network") ensureNetwork();
  else if (state.view === "logs") ensureLogs();
  else if (state.view === "state") ensureRegisteredState();
  else if (state.view === "extensions") ensureExtensions();
  else if (state.view === "performance") ensurePerformance();
  else if (state.view === "memory") ensureMemory();
  else if (state.view === "app-size") ensureAppSize();
  else if (state.view === "deep-links") ensureDeepLinks();
  else if (state.view === "debugger") ensureDebugger();
}

function showState(host, options = {}) {
  clear(host);
  host.hidden = false;
  const kind = options.kind || "empty";
  const stateNode = el("div", { class: `state ${kind === "loading" ? "is-loading" : kind === "error" ? "is-error" : ""} ${options.compact ? "compact" : ""}` });
  if (kind === "loading") {
    stateNode.append(el("span", { class: "state-icon" }, el("span", { class: "spinner", attrs: { role: "progressbar", "aria-label": options.title || "Loading" } })));
  } else {
    stateNode.append(el("span", { class: "state-icon" }, icon(options.icon || (kind === "error" ? "alert" : "empty"))));
  }
  if (options.title) stateNode.append(el("strong", { text: options.title }));
  if (options.message) stateNode.append(el("p", { text: options.message }));
  if (options.action) {
    stateNode.append(el("button", {
      class: `tool-button ${options.primary ? "is-primary" : ""}`,
      props: { type: "button" },
      text: options.action,
      on: { click: options.onAction || (() => {}) },
    }));
  }
  host.append(stateNode);
  return host;
}

function hideState(host) {
  clear(host);
  host.hidden = true;
}

function sectionBlock(title, ...children) {
  return el("div", { class: "section" }, el("h3", { text: title }), ...children);
}

function kvList(pairs) {
  const dl = el("dl", { class: "kv" });
  for (const [label, value] of pairs) {
    if (value === undefined) continue;
    dl.append(el("dt", { text: label }), el("dd", {}, value instanceof Node ? value : textOf(value)));
  }
  return dl;
}

function emptyInline(message) {
  return el("div", { class: "empty-inline", text: message });
}

function errorInline(message) {
  return el("div", { class: "error-inline", text: message });
}

function metric(label, value, suffix = "") {
  return el("div", { class: "metric" },
    el("span", { class: "metric-label", text: label }),
    el("span", { class: "metric-value" }, String(value), suffix ? el("small", { text: ` ${suffix}` }) : null));
}

function rawDetails(label, value) {
  return el("details", { class: "raw" },
    el("summary", { text: label }),
    el("pre", { text: JSON.stringify(value, null, 2) }));
}

