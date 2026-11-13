/* Static event wiring */
import { toggleBaselines, toggleGuidelines, toggleOversizedImages, toggleRepaintHighlight, toggleSlowAnimations } from "../core/debug-paint.js";
import { $, BUDGET_RATES, DETAIL_TABS, MEMORY_INTERVALS, dom, state } from "../core/globals.js";
import { applyTheme, chooseTarget, errorMessage, refreshSession, setThemePreference, showWorkspace, storeToken, toast } from "../core/session.js";
import { ensureCurrentView } from "../core/ui-helpers.js";
import { captureNode, captureWindow, clearScreenshot, enterPickMode, exitPickMode, renderScreenshot, saveCapture, toggleScreenshotZoom, updateModeUi } from "../inspector/capture.js";
import { copyText, setDetailTab } from "../inspector/editors.js";
import { sendKey, sendText, toggleInputDrawer, updateInputButtons } from "../inspector/input.js";
import { clearHighlight, highlightNode, loadTree, readLiveValue, renderDetails, renderTreeArea, selectNode } from "../inspector/tree.js";
import { loadAppSize } from "../panels/app-size/diff.js";
import { selectedDebuggerFrame } from "../panels/debugger/model.js";
import { attachDebugger, detachDebugger, loadDebuggerScopes, refreshDebugger, runDebuggerControl } from "../panels/debugger/session.js";
import { clearDebuggerConsole, debuggerUnloadDetach, evaluateDebuggerExpression, openDebuggerSource } from "../panels/debugger/view.js";
import { deepLinksRootError } from "../panels/deep-links/model.js";
import { renderDeepLinks, showDeepLinksRootError, submitDeepLinks } from "../panels/deep-links/view.js";
import { loadExtensions } from "../panels/extensions.js";
import { clearLogs, loadLogs, renderLogs } from "../panels/logs.js";
import { stopAllocationPolling, syncAllocationPolling } from "../panels/memory/sampling.js";
import { loadMemory, refreshMemoryView } from "../panels/memory/view.js";
import { clearNetwork, loadNetwork, renderNetwork, setNetworkRecording, stopAllFollowTimers, syncFollowTimers } from "../panels/network.js";
import { loadFrames } from "../panels/performance/frames.js";
import { renderPerformance, setPerformanceTab, syncBudgetOptions, writeBudgetPreference, writeDeepLinksRootPreference, writeMemoryIntervalPreference } from "../panels/performance/overview.js";
import { pollProfile } from "../panels/performance/profile/control.js";
import { clearProfileTimers, loadProfile } from "../panels/performance/profile/model.js";
import { exportPerformance, importPerformance } from "../panels/performance/snapshots.js";
import { startTrace, stopTrace } from "../panels/performance/timeline.js";
import { loadRegisteredState } from "../panels/state.js";
import { refreshCurrent, switchView } from "./views.js";


export function bindEvents() {
  const railTabs = [...document.querySelectorAll(".rail-tab")];
  for (const tab of railTabs) {
    tab.addEventListener("click", () => switchView(tab.dataset.view));
    tab.addEventListener("keydown", event => {
      const index = railTabs.indexOf(tab);
      let next = null;
      if (event.key === "ArrowDown" || event.key === "ArrowRight") next = railTabs[(index + 1) % railTabs.length];
      else if (event.key === "ArrowUp" || event.key === "ArrowLeft") next = railTabs[(index - 1 + railTabs.length) % railTabs.length];
      else if (event.key === "Home") next = railTabs[0];
      else if (event.key === "End") next = railTabs[railTabs.length - 1];
      else if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        switchView(tab.dataset.view, { focusTab: true });
        return;
      }
      if (next) {
        event.preventDefault();
        switchView(next.dataset.view, { focusTab: true });
      }
    });
  }

  dom.targets.addEventListener("change", () => {
    chooseTarget(dom.targets.value).catch(error => toast(errorMessage(error), "error", 8000));
  });
  dom.refresh.addEventListener("click", () => refreshCurrent());
  dom.treeRefresh.addEventListener("click", () => loadTree({ preserveExpansion: true }));
  dom.treeCollapse.addEventListener("click", () => {
    state.search = "";
    dom.treeSearch.value = "";
    state.expanded.clear();
    if (state.rootId) state.expanded.add(state.rootId);
    state.scrollSelection = false;
    renderTreeArea();
  });

  let searchTimer = null;
  dom.treeSearch.addEventListener("input", () => {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => {
      state.search = dom.treeSearch.value;
      state.scrollSelection = false;
      renderTreeArea();
    }, 100);
  });
  dom.filterAnchors.addEventListener("change", () => {
    state.filters.anchors = dom.filterAnchors.checked;
    state.scrollSelection = false;
    renderTreeArea();
  });
  dom.filterPrimitives.addEventListener("change", () => {
    state.filters.primitives = dom.filterPrimitives.checked;
    state.scrollSelection = false;
    renderTreeArea();
  });

  dom.tree.addEventListener("click", event => {
    const twisty = event.target.closest(".tree-twisty");
    const row = event.target.closest(".tree-row");
    if (!row) return;
    const id = row.dataset.id;
    if (twisty && !twisty.disabled) {
      event.preventDefault();
      if (state.expanded.has(id)) state.expanded.delete(id);
      else state.expanded.add(id);
      renderTreeArea();
      const current = [...dom.tree.children].find(child => child.dataset.id === id);
      current?.focus();
      return;
    }
    selectNode(id, { reveal: false });
    row.focus();
  });
  dom.tree.addEventListener("keydown", event => {
    const row = event.target.closest(".tree-row");
    if (!row || !state.visibleRows.length) return;
    const index = state.visibleRows.findIndex(item => item.id === row.dataset.id);
    if (index < 0) return;
    const current = state.visibleRows[index];
    let next = null;
    if (event.key === "ArrowDown") {
      next = state.visibleRows[Math.min(state.visibleRows.length - 1, index + 1)];
    } else if (event.key === "ArrowUp") {
      next = state.visibleRows[Math.max(0, index - 1)];
    } else if (event.key === "Home") {
      next = state.visibleRows[0];
    } else if (event.key === "End") {
      next = state.visibleRows[state.visibleRows.length - 1];
    } else if (event.key === "ArrowRight") {
      if (current.hasChildren && !current.expanded) {
        state.expanded.add(current.id);
        renderTreeArea();
      } else if (current.hasChildren) {
        const child = state.visibleRows[index + 1];
        if (child) next = child;
      }
    } else if (event.key === "ArrowLeft") {
      if (current.expanded && current.hasChildren) {
        state.expanded.delete(current.id);
        renderTreeArea();
      } else if (current.parentId) {
        next = state.visibleRows.find(item => item.id === current.parentId) || null;
      }
    } else if (event.key === "Enter" || event.key === " ") {
      selectNode(current.id, { reveal: false });
      event.preventDefault();
      return;
    } else {
      return;
    }
    event.preventDefault();
    if (next) {
      selectNode(next.id, { reveal: false });
      const nextRow = [...dom.tree.children].find(child => child.dataset.id === next.id);
      nextRow?.focus();
    } else {
      const currentRow = [...dom.tree.children].find(child => child.dataset.id === current.id);
      currentRow?.focus();
    }
  });

  dom.modeSelect.addEventListener("click", () => {
    state.mode = "select";
    updateModeUi();
    renderScreenshot();
    renderDetails();
  });
  dom.modePick.addEventListener("click", () => {
    if (state.mode === "pick") exitPickMode();
    else enterPickMode();
  });
  dom.modeInteract.addEventListener("click", () => {
    if (dom.modeInteract.disabled) return;
    state.mode = "interact";
    updateModeUi();
    if (state.capture?.kind === "window") renderScreenshot();
    else captureWindow();
  });
  dom.pickExit.addEventListener("click", () => exitPickMode());
  dom.captureButton.addEventListener("click", () => captureWindow());
  dom.nodeShotButton.addEventListener("click", () => captureNode());
  dom.saveButton.addEventListener("click", saveCapture);
  dom.screenshotZoom.addEventListener("click", toggleScreenshotZoom);
  dom.screenshotClear.addEventListener("click", clearScreenshot);
  dom.highlightButton.addEventListener("click", () => {
    if (state.selectedId) highlightNode(state.selectedId);
  });
  dom.clearHighlight.addEventListener("click", clearHighlight);
  dom.readValueButton.addEventListener("click", readLiveValue);
  dom.repaintHighlightButton.addEventListener("click", () => { toggleRepaintHighlight(); });
  dom.guidelinesButton.addEventListener("click", () => { toggleGuidelines(); });
  dom.baselinesButton.addEventListener("click", () => { toggleBaselines(); });
  dom.slowAnimationsButton.addEventListener("click", () => { toggleSlowAnimations(); });
  dom.oversizedImagesButton.addEventListener("click", () => { toggleOversizedImages(); });
  dom.copyNodeId.addEventListener("click", () => copyText(state.selectedId, "Node ID"));

  for (const tab of DETAIL_TABS) {
    const button = $(`tab-${tab}`);
    button.addEventListener("click", () => setDetailTab(tab));
    button.addEventListener("keydown", event => {
      const index = DETAIL_TABS.indexOf(tab);
      let next = null;
      if (event.key === "ArrowRight") next = DETAIL_TABS[(index + 1) % DETAIL_TABS.length];
      else if (event.key === "ArrowLeft") next = DETAIL_TABS[(index - 1 + DETAIL_TABS.length) % DETAIL_TABS.length];
      else if (event.key === "Home") next = DETAIL_TABS[0];
      else if (event.key === "End") next = DETAIL_TABS[DETAIL_TABS.length - 1];
      if (next) {
        event.preventDefault();
        setDetailTab(next, true);
      }
    });
  }

  dom.inputToggle.addEventListener("click", toggleInputDrawer);
  dom.keyForm.addEventListener("submit", sendKey);
  dom.textForm.addEventListener("submit", sendText);
  dom.keyValue.addEventListener("input", updateInputButtons);
  dom.textValue.addEventListener("input", updateInputButtons);

  dom.networkRecordingToggle.addEventListener("click", () => setNetworkRecording(state.network.recording === false));
  dom.networkClear.addEventListener("click", () => clearNetwork());
  dom.networkRefresh.addEventListener("click", () => loadNetwork());
  dom.networkFollow.addEventListener("change", () => {
    state.network.follow = dom.networkFollow.checked;
    syncFollowTimers();
    if (state.network.follow) loadNetwork({ quiet: true });
  });
  dom.networkSearch.addEventListener("input", renderNetwork);
  dom.networkStateFilter.addEventListener("change", renderNetwork);
  dom.networkWaterfall.addEventListener("change", () => {
    state.network.waterfall = dom.networkWaterfall.checked;
    renderNetwork();
  });
  dom.networkDialogClose.addEventListener("click", () => dom.networkDialog.close());
  dom.networkDialog.addEventListener("click", event => {
    if (event.target === dom.networkDialog) dom.networkDialog.close();
  });

  dom.logsClear.addEventListener("click", () => clearLogs());
  dom.logsRefresh.addEventListener("click", () => loadLogs({ reset: true }));
  dom.logsFollow.addEventListener("change", () => {
    state.logs.follow = dom.logsFollow.checked;
    syncFollowTimers();
    if (state.logs.follow) loadLogs({ quiet: true });
  });
  dom.logsSearch.addEventListener("input", renderLogs);
  dom.logsLevel.addEventListener("change", renderLogs);
  dom.logsLabel.addEventListener("change", renderLogs);
  dom.stateRefresh.addEventListener("click", () => loadRegisteredState(state.registeredState.offset));
  dom.extensionsRefresh.addEventListener("click", () => loadExtensions({ reset: true }));
  dom.stateScope.addEventListener("change", () => {
    state.registeredState.scope = dom.stateScope.value;
    loadRegisteredState(0, { quiet: true });
  });
  dom.stateFollow.addEventListener("change", () => {
    state.registeredState.follow = dom.stateFollow.checked;
    syncFollowTimers();
    if (state.registeredState.follow) loadRegisteredState(state.registeredState.offset, { quiet: true });
  });

  dom.performanceRefresh.addEventListener("click", () => {
    const perf = state.performance;
    if (perf.tab === "profile") {
      if (perf.profile.status === "sampling") pollProfile();
      else loadProfile({ quiet: true });
      return;
    }
    loadFrames({ reset: true });
  });
  dom.memoryRefresh.addEventListener("click", () => refreshMemoryView());
  dom.memoryFollow.addEventListener("change", () => {
    state.memory.follow = dom.memoryFollow.checked;
    syncFollowTimers();
    if (state.memory.follow) loadMemory({ quiet: true });
  });
  dom.memoryInterval.addEventListener("change", () => {
    const interval = Number(dom.memoryInterval.value);
    if (!MEMORY_INTERVALS.includes(interval)) return;
    state.memory.intervalMs = interval;
    writeMemoryIntervalPreference(interval);
    syncFollowTimers();
  });
  dom.appSizeRefresh.addEventListener("click", () => loadAppSize({ reset: false }));
  dom.deepLinksRootForm.addEventListener("submit", submitDeepLinks);
  dom.deepLinksRoot.addEventListener("input", () => {
    if (!dom.deepLinksRootError.hidden) {
      showDeepLinksRootError("");
      state.deepLinks.rootError = "";
      if (state.view === "deep-links") renderDeepLinks();
    }
  });
  dom.deepLinksRoot.addEventListener("change", () => {
    const root = dom.deepLinksRoot.value.trim();
    if (!root || !deepLinksRootError(root)) writeDeepLinksRootPreference(root);
  });
  dom.performanceFollow.addEventListener("change", () => {
    state.performance.follow = dom.performanceFollow.checked;
    syncFollowTimers();
    if (state.performance.follow) loadFrames({ quiet: true });
  });
  dom.performanceBudget.addEventListener("change", () => {
    const rate = Number(dom.performanceBudget.value);
    const perf = state.performance;
    if (!Number.isFinite(rate) || rate <= 0) return;
    if (!BUDGET_RATES.includes(rate) && rate !== perf.refreshHz) return;
    perf.budgetHz = rate;
    // Picking the measured rate by hand returns the budget to the runtime;
    // any other choice is an explicit override of it.
    perf.budgetManual = perf.refreshHz !== null && rate !== perf.refreshHz;
    writeBudgetPreference(rate);
    syncBudgetOptions(perf);
    renderPerformance();
  });
  dom.performanceStart.addEventListener("click", startTrace);
  dom.performanceStop.addEventListener("click", () => stopTrace());
  dom.performanceExport.addEventListener("click", exportPerformance);
  dom.performanceImport.addEventListener("click", () => dom.performanceFile.click());
  dom.performanceFile.addEventListener("change", () => importPerformance(dom.performanceFile.files?.[0]));

  const perfTabs = [...document.querySelectorAll(".perf-tab")];
  for (const tab of perfTabs) {
    tab.addEventListener("click", () => setPerformanceTab(tab.dataset.perfTab));
    tab.addEventListener("keydown", event => {
      const index = perfTabs.indexOf(tab);
      let next = null;
      if (event.key === "ArrowRight") next = perfTabs[(index + 1) % perfTabs.length];
      else if (event.key === "ArrowLeft") next = perfTabs[(index - 1 + perfTabs.length) % perfTabs.length];
      else if (event.key === "Home") next = perfTabs[0];
      else if (event.key === "End") next = perfTabs[perfTabs.length - 1];
      else if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        setPerformanceTab(tab.dataset.perfTab, { focusTab: true });
        return;
      }
      if (next) {
        event.preventDefault();
        setPerformanceTab(next.dataset.perfTab, { focusTab: true });
      }
    });
  }
  dom.debuggerRefresh.addEventListener("click", () => refreshDebugger());
  dom.debuggerAttach.addEventListener("click", attachDebugger);
  dom.debuggerDetach.addEventListener("click", detachDebugger);
  dom.debuggerPause.addEventListener("click", () => runDebuggerControl("pause", "Pause"));
  dom.debuggerContinue.addEventListener("click", () => runDebuggerControl("continue", "Continue"));
  dom.debuggerStepOver.addEventListener("click", () => runDebuggerControl("step_over", "Step over"));
  dom.debuggerStepInto.addEventListener("click", () => runDebuggerControl("step_into", "Step in"));
  dom.debuggerStepOut.addEventListener("click", () => runDebuggerControl("step_out", "Step out"));
  dom.debuggerVariablesRefresh.addEventListener("click", () => {
    const frame = selectedDebuggerFrame();
    if (frame) loadDebuggerScopes(frame, { quiet: false });
  });
  dom.debuggerOpenForm.addEventListener("submit", openDebuggerSource);
  dom.debuggerEvaluateForm.addEventListener("submit", evaluateDebuggerExpression);
  dom.debuggerConsoleClear.addEventListener("click", clearDebuggerConsole);

  dom.themeButton.addEventListener("click", () => {
    const order = ["auto", "light", "dark"];
    const next = order[(order.indexOf(state.theme) + 1) % order.length];
    setThemePreference(next);
  });
  const colorScheme = matchMedia("(prefers-color-scheme: dark)");
  const onColorScheme = () => {
    if (state.theme === "auto") applyTheme();
  };
  if (colorScheme.addEventListener) colorScheme.addEventListener("change", onColorScheme);
  else colorScheme.addListener?.(onColorScheme);

  dom.shortcutsButton.addEventListener("click", openShortcuts);
  dom.shortcutsClose.addEventListener("click", () => dom.shortcutsDialog.close());
  dom.shortcutsDialog.addEventListener("click", event => {
    if (event.target === dom.shortcutsDialog) dom.shortcutsDialog.close();
  });

  dom.toastClose.addEventListener("click", () => { dom.toast.hidden = true; });

  dom.tokenForm.addEventListener("submit", event => {
    event.preventDefault();
    const token = dom.tokenInput.value.trim();
    if (!/^[A-Za-z0-9_-]{1,256}$/.test(token)) {
      dom.setupError.hidden = false;
      dom.setupError.textContent = "Enter the token from the Inspector link. It contains letters, digits, hyphens and underscores.";
      return;
    }
    storeToken(token);
    showWorkspace();
    refreshSession();
  });
  dom.setupError.hidden = true;

  document.addEventListener("keydown", handleGlobalKeydown);
  document.addEventListener("visibilitychange", () => {
    syncFollowTimers();
    syncAllocationPolling();
    if (!document.hidden && state.connected) ensureCurrentView();
  });
  window.addEventListener("beforeunload", () => {
    stopAllFollowTimers();
    stopAllocationPolling();
    if (state.performance.recordTimer) clearInterval(state.performance.recordTimer);
    clearProfileTimers(state.performance.profile);
    debuggerUnloadDetach();
    if (state.capture?.url) URL.revokeObjectURL(state.capture.url);
  });
}

function openShortcuts() {
  if (!dom.shortcutsDialog.open) dom.shortcutsDialog.showModal();
  dom.shortcutsClose.focus();
}

function isTextEntry(target) {
  return Boolean(target?.closest?.("input, textarea, select, [contenteditable='true']"));
}

function handleGlobalKeydown(event) {
  const typing = isTextEntry(event.target);
  const dialogOpen = Boolean(document.querySelector("dialog[open]"));
  if (event.key === "Escape") {
    if (dialogOpen || typing) return;
    if (state.mode === "pick") {
      event.preventDefault();
      exitPickMode();
      return;
    }
    if (dom.treeSearch.value) {
      dom.treeSearch.value = "";
      state.search = "";
      renderTreeArea();
    }
    return;
  }
  if (typing || dialogOpen) return;
  if (event.altKey && !event.ctrlKey && !event.metaKey) {
    const index = Number(event.key);
    if (index >= 1 && index <= 9) {
      event.preventDefault();
      switchView(["inspector", "network", "logs", "performance", "memory", "app-size", "deep-links", "debugger", "state", "extensions"][index - 1], { focusTab: true });
      return;
    }
  }
  if (state.view === "debugger") {
    let action = null;
    let label = "";
    if (event.key === "F5" && !dom.debuggerContinue.disabled) {
      action = "continue";
      label = "Continue";
    } else if (event.key === "F10" && !dom.debuggerStepOver.disabled) {
      action = "step_over";
      label = "Step over";
    } else if (event.key === "F11" && !dom.debuggerStepOut.disabled && event.shiftKey) {
      action = "step_out";
      label = "Step out";
    } else if (event.key === "F11" && !dom.debuggerStepInto.disabled) {
      action = "step_into";
      label = "Step in";
    }
    if (action) {
      event.preventDefault();
      runDebuggerControl(action, label);
      return;
    }
  }
  if (event.ctrlKey || event.metaKey) return;
  if (event.key === "?") {
    event.preventDefault();
    openShortcuts();
    return;
  }
  if (event.key === "/") {
    event.preventDefault();
    if (state.view !== "inspector") switchView("inspector");
    dom.treeSearch.focus();
    dom.treeSearch.select();
    return;
  }
  if (event.key.toLowerCase() === "p" && !event.altKey) {
    event.preventDefault();
    if (state.view !== "inspector") switchView("inspector");
    if (state.mode === "pick") exitPickMode();
    else enterPickMode();
    return;
  }
  if (event.key.toLowerCase() === "r" && !event.altKey) {
    event.preventDefault();
    refreshCurrent();
  }
}

