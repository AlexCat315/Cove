import { resetBaselines, resetDebugPaintState, resetOversizedImages, resetRepaintHighlight, resetSlowAnimations } from "./debug-paint.js";
import { clear, el, icon, textOf } from "./format.js";
import { dom, state } from "./globals.js";
import { releaseCapture } from "../inspector/capture.js";
import { resetFlexEditorState } from "../inspector/editors.js";
import { ensureAppSize, resetAppSize } from "../panels/app-size/diff.js";
import { ensureDebugger } from "../panels/debugger/session.js";
import { resetDebugger } from "../panels/debugger/view.js";
import { ensureDeepLinks } from "../panels/deep-links/view.js";
import { ensureExtensions, resetExtensions } from "../panels/extensions.js";
import { ensureLogs, resetLogs } from "../panels/logs.js";
import { ensureMemory, resetMemory } from "../panels/memory/view.js";
import { ensureNetwork, resetNetwork, stopAllFollowTimers } from "../panels/network.js";
import { ensurePerformance, resetPerformance } from "../panels/performance/overview.js";
import { ensureRegisteredState, resetRegisteredState } from "../panels/state.js";
import { ensureSceneDiagnostics, resetSceneDiagnostics } from "../panels/scene-diagnostics.js";

export function resetTargetData(previousTarget = state.targetId) {
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
  resetSceneDiagnostics();
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

export function ensureCurrentView() {
  if (state.view === "network") ensureNetwork();
  else if (state.view === "logs") ensureLogs();
  else if (state.view === "state") ensureRegisteredState();
  else if (state.view === "semantics") ensureSceneDiagnostics("semantics");
  else if (state.view === "extensions") ensureExtensions();
  else if (state.view === "performance") ensurePerformance();
  else if (state.view === "memory") ensureMemory();
  else if (state.view === "app-size") ensureAppSize();
  else if (state.view === "deep-links") ensureDeepLinks();
  else if (state.view === "debugger") ensureDebugger();
}

export function showState(host, options = {}) {
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

export function hideState(host) {
  clear(host);
  host.hidden = true;
}

export function sectionBlock(title, ...children) {
  return el("div", { class: "section" }, el("h3", { text: title }), ...children);
}

export function kvList(pairs) {
  const dl = el("dl", { class: "kv" });
  for (const [label, value] of pairs) {
    if (value === undefined) continue;
    dl.append(el("dt", { text: label }), el("dd", {}, value instanceof Node ? value : textOf(value)));
  }
  return dl;
}

export function emptyInline(message) {
  return el("div", { class: "empty-inline", text: message });
}

export function errorInline(message) {
  return el("div", { class: "error-inline", text: message });
}

export function metric(label, value, suffix = "") {
  return el("div", { class: "metric" },
    el("span", { class: "metric-label", text: label }),
    el("span", { class: "metric-value" }, String(value), suffix ? el("small", { text: ` ${suffix}` }) : null));
}

export function rawDetails(label, value) {
  return el("details", { class: "raw" },
    el("summary", { text: label }),
    el("pre", { text: JSON.stringify(value, null, 2) }));
}

