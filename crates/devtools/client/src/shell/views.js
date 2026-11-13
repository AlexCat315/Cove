/* Views and refresh */
import { $, state } from "../core/globals.js";
import { refreshSession, showSetup, updateTargetUi } from "../core/session.js";
import { ensureCurrentView } from "../core/ui-helpers.js";
import { loadTree } from "../inspector/tree.js";
import { loadAppSize } from "../panels/app-size/diff.js";
import { refreshDebugger } from "../panels/debugger/session.js";
import { renderDebugger } from "../panels/debugger/view.js";
import { loadDeepLinks, renderDeepLinks } from "../panels/deep-links/view.js";
import { loadExtensions, renderExtensions, renderRegisteredState } from "../panels/extensions.js";
import { loadLogs, renderLogs } from "../panels/logs.js";
import { syncAllocationPolling } from "../panels/memory/sampling.js";
import { loadMemory, renderMemory } from "../panels/memory/view.js";
import { loadNetwork, renderNetwork, syncFollowTimers } from "../panels/network.js";
import { loadFrames } from "../panels/performance/frames.js";
import { activeCaptureRecord, renderPerformance } from "../panels/performance/overview.js";
import { pollProfile } from "../panels/performance/profile/control.js";
import { loadProfile } from "../panels/performance/profile/model.js";
import { loadTraceEvents } from "../panels/performance/timeline.js";
import { loadRegisteredState } from "../panels/state.js";
import { loadSceneDiagnostics } from "../panels/scene-diagnostics.js";


export function switchView(view, options = {}) {
  if (!["inspector", "network", "logs", "performance", "memory", "app-size", "deep-links", "debugger", "state", "extensions", "semantics"].includes(view)) return;
  if (state.view === view && !options.force) return;
  state.view = view;
  const titles = { inspector: "Inspector", network: "Network", logs: "Logs", performance: "Performance", memory: "Memory", "app-size": "App Size", "deep-links": "Deep Links", debugger: "Debugger", state: "State", extensions: "Extensions", semantics: "Semantics" };
  document.title = `Cove DevTools · ${titles[view]}`;
  const tabs = [...document.querySelectorAll(".rail-tab")];
  for (const tab of tabs) {
    const selected = tab.dataset.view === view;
    tab.setAttribute("aria-selected", String(selected));
    tab.tabIndex = selected ? 0 : -1;
    const panel = $(`view-${tab.dataset.view}`);
    panel.hidden = !selected;
  }
  syncFollowTimers();
  syncAllocationPolling();
  ensureCurrentView();
  if (options.focusTab) {
    const tab = tabs.find(item => item.dataset.view === view);
    tab?.focus();
  }
}

export async function refreshCurrent() {
  if (!state.token) {
    showSetup();
    return;
  }
  await refreshSession({ quiet: true });
  if (state.view === "inspector") {
    if (state.caps.tree) await loadTree({ preserveExpansion: true });
    updateTargetUi();
  } else if (state.view === "network") {
    if (state.caps.network) await loadNetwork();
    else renderNetwork();
  } else if (state.view === "logs") {
    if (state.caps.logs) await loadLogs({ reset: true });
    else renderLogs();
  } else if (state.view === "state") {
    if (state.caps.state) await loadRegisteredState(0);
    else renderRegisteredState();
  } else if (state.view === "extensions") {
    if (state.caps.extensions) await loadExtensions({ reset: true });
    else renderExtensions();
  } else if (state.view === "semantics") {
    await loadSceneDiagnostics("semantics");
  } else if (state.view === "performance") {
    if (state.performance.snapshot) {
      renderPerformance();
    } else if (state.performance.tab === "profile") {
      if (state.performance.profile.status === "sampling") await pollProfile();
      else if (state.caps.profile) await loadProfile({ quiet: true });
      else renderPerformance();
    } else if (state.performance.tab === "components") {
      await loadSceneDiagnostics("components");
    } else {
      if (state.caps.timing) await loadFrames({ reset: true });
      else renderPerformance();
      const record = activeCaptureRecord();
      if (record && !record.imported && !record.complete) await loadTraceEvents({ captureId: record.summary?.id, quiet: true });
    }
  } else if (state.view === "memory") {
    if (state.caps.runtime) await loadMemory({ quiet: true });
    else renderMemory();
  } else if (state.view === "app-size") {
    await loadAppSize({ quiet: true });
  } else if (state.view === "deep-links") {
    if (state.deepLinks.root) await loadDeepLinks({ root: state.deepLinks.root, quiet: true });
    else renderDeepLinks();
  } else if (state.view === "debugger") {
    if (state.caps.runtime) await refreshDebugger({ quiet: true });
    else renderDebugger();
  }
}

