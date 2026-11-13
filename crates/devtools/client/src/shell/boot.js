/* Boot */

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
