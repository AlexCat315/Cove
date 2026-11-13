/* Boot */
import { renderBaselinesUi, renderGuidelinesUi, renderRepaintHighlightUi, renderSlowAnimationsUi } from "../core/debug-paint.js";
import { THEME_KEY, dom, state } from "../core/globals.js";
import { applyTheme, readToken, refreshSession, showSetup, showWorkspace } from "../core/session.js";
import { renderScreenshot, updateModeUi } from "../inspector/capture.js";
import { setDetailTab } from "../inspector/editors.js";
import { renderInputUi } from "../inspector/input.js";
import { renderDetails, renderTreeArea } from "../inspector/tree.js";
import { renderAppSize } from "../panels/app-size/diff.js";
import { renderDebugger, renderDebuggerConsole } from "../panels/debugger/view.js";
import { renderDeepLinks } from "../panels/deep-links/view.js";
import { renderLogs } from "../panels/logs.js";
import { renderMemory } from "../panels/memory/view.js";
import { renderNetwork } from "../panels/network.js";
import { renderPerformance } from "../panels/performance/overview.js";
import { bindEvents } from "./events.js";


function init() {
  try {
    const storedTheme = localStorage.getItem(THEME_KEY);
    state.theme = ["auto", "light", "dark"].includes(storedTheme) ? storedTheme : "auto";
  } catch {
    state.theme = "auto";
  }
  applyTheme();
  bindEvents();
  document.title = "Cove DevTools · Inspector";
  setDetailTab(state.activeDetailTab);
  renderScreenshot();
  renderInputUi();
  renderTreeArea();
  renderDetails();
  renderRepaintHighlightUi();
  renderGuidelinesUi();
  renderBaselinesUi();
  renderSlowAnimationsUi();
  renderNetwork();
  renderLogs();
  renderPerformance();
  renderMemory();
  renderAppSize();
  dom.deepLinksRoot.value = state.deepLinks.root;
  renderDeepLinks();
  renderDebugger();
  renderDebuggerConsole();
  updateModeUi();
  const token = readToken();
  if (token) {
    state.token = token;
    showWorkspace();
    refreshSession();
  } else {
    showSetup();
  }
}

init();
