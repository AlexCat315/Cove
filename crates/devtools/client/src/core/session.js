import { renderBaselinesUi, renderGuidelinesUi, renderOversizedImagesUi, renderRepaintHighlightUi, renderSlowAnimationsUi, resetBaselines, resetDebugPaintState, resetOversizedImages, resetRepaintHighlight, resetSlowAnimations } from "./debug-paint.js";
import { clear, el } from "./format.js";
import { $, ALLOCATION_SITE_CAPABILITIES, FRAME_ANALYSIS_CAPABILITIES, PROFILE_CAPABILITIES, RpcError, THEME_KEY, TOKEN_KEY, dom, emptyCaps, nextRpcSequence, state } from "./globals.js";
import { ensureCurrentView, resetTargetData } from "./ui-helpers.js";
import { renderScreenshot, updateModeUi } from "../inspector/capture.js";
import { renderInputUi } from "../inspector/input.js";
import { loadTree, renderDetails, renderTreeArea } from "../inspector/tree.js";
import { ensureDeepLinks } from "../panels/deep-links/view.js";
import { renderLogs } from "../panels/logs.js";
import { abandonAllocationSites, ensureAllocationSites } from "../panels/memory/sampling.js";
import { renderMemory } from "../panels/memory/view.js";
import { renderNetwork, syncFollowTimers } from "../panels/network.js";
import { refreshCurrent } from "../shell/views.js";

function setConnection(kind, label) {
  const wasOnline = state.connected;
  dom.connection.classList.remove("is-online", "is-offline", "is-busy");
  dom.connection.classList.add(`is-${kind}`);
  dom.connectionText.textContent = label;
  state.connected = kind === "online";
  if (wasOnline && !state.connected) {
    resetRepaintHighlight();
    resetDebugPaintState();
    resetBaselines();
    resetSlowAnimations();
    resetOversizedImages();
    abandonAllocationSites();
    if (state.view === "logs") renderLogs();
    if (state.view === "network") renderNetwork();
    if (state.view === "memory") renderMemory();
  } else if (!wasOnline && state.connected && state.view === "memory") {
    ensureAllocationSites();
  }
}

function setConnectionBusy(busy) {
  if (busy) {
    dom.connection.classList.add("is-busy");
    dom.connectionText.textContent = "Connecting…";
  } else if (state.connected) {
    dom.connection.classList.remove("is-busy");
    dom.connectionText.textContent = "Connected";
  }
}

export function toast(message, kind = "info", duration = 6000) {
  clearTimeout(state.toastTimer);
  const text = String(message ?? "");
  if (!text) return;
  dom.toast.hidden = false;
  dom.toast.className = `toast is-${kind}`;
  dom.toastText.textContent = text;
  dom.toastIcon.setAttribute("href", kind === "error" ? "#i-alert" : kind === "success" ? "#i-check" : "#i-info");
  dom.liveRegion.textContent = text;
  state.toastTimer = setTimeout(() => { dom.toast.hidden = true; }, duration);
}

export function isDisconnectedError(error) {
  return error?.code === "target_gone" || error?.code === "target_required";
}

export function retryTree() {
  if (isDisconnectedError(state.treeError)) refreshCurrent();
  else loadTree({ preserveExpansion: true });
}

export function errorMessage(error) {
  if (error instanceof RpcError) {
    return error.code && error.code !== "error" ? `${error.message} (${error.code})` : error.message;
  }
  return error?.message ? String(error.message) : String(error);
}

export function authHeaders() {
  return { Authorization: `Bearer ${state.token}` };
}

export async function rpc(method, params = {}, options = {}) {
  if (!state.token) throw new RpcError("The controller token is missing.", "unauthorized");
  let response;
  try {
    response = await fetch("/rpc", {
      method: "POST",
      headers: { ...authHeaders(), "Content-Type": "application/json" },
      body: JSON.stringify({ jsonrpc: "2.0", id: nextRpcSequence(), method, params }),
      signal: options.signal,
    });
  } catch (error) {
    if (error?.name === "AbortError") throw error;
    setConnection("offline", "Offline");
    throw new RpcError("Could not reach the controller. Is cove-devtools serve still running?", "network_error");
  }
  const raw = await response.text();
  let reply = null;
  try {
    reply = raw ? JSON.parse(raw) : null;
  } catch {
    throw new RpcError("The controller returned an invalid response.", "invalid_response");
  }
  if (response.status === 401) {
    setConnection("offline", "Unauthorized");
    throw new RpcError("The controller rejected this session token.", "unauthorized");
  }
  if (!response.ok) {
    throw new RpcError(reply?.message || `The controller returned HTTP ${response.status}.`, "http_error");
  }
  if (!reply || typeof reply !== "object") {
    throw new RpcError("The controller returned an empty response.", "invalid_response");
  }
  if (reply.error) {
    throw new RpcError(reply.error.message || "The controller reported an error.", reply.error.data?.code || "error", reply.error.data ?? null);
  }
  setConnection("online", "Connected");
  return reply.result;
}

export function unwrapData(result) {
  if (result && result.ok === false) {
    throw new RpcError(result.error?.message || "The runtime reported an error.", result.error?.code || "runtime_error");
  }
  if (result && result.ok === true && Object.prototype.hasOwnProperty.call(result, "data")) return result.data;
  return result;
}

export function targetById(id = state.targetId) {
  return state.targets.find(target => target.id === id) || null;
}

function readTargetCaps(target) {
  const caps = emptyCaps();
  if (!target) return caps;
  const advertised = new Set(Array.isArray(target.capabilities) ? target.capabilities : []);
  caps.runtime = target.kind === "runtime";
  caps.tree = caps.runtime && advertised.has("tree");
  caps.capture = advertised.has("capture");
  caps.input = advertised.has("input");
  caps.logs = caps.runtime && advertised.has("diagnostics");
  caps.state = caps.runtime && advertised.has("state");
  caps.extensions = caps.runtime && advertised.has("extensions");
  caps.network = caps.runtime && advertised.has("network");
  caps.networkControl = caps.runtime && advertised.has("network_control");
  caps.timing = caps.runtime && advertised.has("timing");
  caps.trace = caps.runtime && advertised.has("trace");
  caps.frameAnalysis = caps.runtime && FRAME_ANALYSIS_CAPABILITIES.some(name => advertised.has(name));
  caps.componentActivity = caps.runtime && advertised.has("component_activity");
  caps.semanticsTree = caps.runtime && advertised.has("semantics_tree");
  caps.profile = caps.runtime && PROFILE_CAPABILITIES.some(name => advertised.has(name));
  caps.inspectNodes = caps.runtime && advertised.has("inspect_nodes");
  caps.repaintHighlight = caps.runtime && advertised.has("repaint_highlight");
  caps.debugPaint = caps.runtime && advertised.has("debug_paint");
  caps.baselines = caps.runtime && advertised.has("baselines");
  caps.animationSpeed = caps.runtime && advertised.has("animation_speed");
  caps.oversizedImages = caps.runtime && advertised.has("oversized_images");
  caps.allocationSites = caps.runtime && ALLOCATION_SITE_CAPABILITIES.some(name => advertised.has(name));
  caps.style = caps.runtime && advertised.has("style");
  return caps;
}

function targetLabel(target) {
  if (!target) return "No target";
  const kind = target.kind === "runtime" ? "runtime" : target.kind;
  return `${target.title} · ${kind}`;
}

function defaultTargetId() {
  const runtime = state.targets.find(target => target.kind === "runtime" && Array.isArray(target.capabilities) && target.capabilities.includes("tree"));
  if (runtime) return runtime.id;
  const anyRuntime = state.targets.find(target => target.kind === "runtime");
  if (anyRuntime) return anyRuntime.id;
  const window = state.targets.find(target => target.kind === "window");
  if (window) return window.id;
  return state.targets[0]?.id || "";
}

export function readToken() {
  const hash = location.hash.replace(/^#/, "");
  if (hash) {
    let token = hash;
    try { token = decodeURIComponent(hash); } catch { token = hash; }
    if (/^[A-Za-z0-9_-]{1,256}$/.test(token)) {
      try { sessionStorage.setItem(TOKEN_KEY, token); } catch {}
      history.replaceState(null, "", `${location.pathname}${location.search}`);
      return token;
    }
  }
  try {
    const stored = sessionStorage.getItem(TOKEN_KEY) || "";
    return /^[A-Za-z0-9_-]{1,256}$/.test(stored) ? stored : "";
  } catch {
    return "";
  }
}

export function storeToken(token) {
  state.token = token;
  try { sessionStorage.setItem(TOKEN_KEY, token); } catch {}
}

export function showSetup(error = "") {
  if (state.token) {
    state.token = "";
    try { sessionStorage.removeItem(TOKEN_KEY); } catch {}
  }
  dom.setup.hidden = false;
  dom.inspectorContent.hidden = true;
  dom.targets.disabled = true;
  dom.targets.replaceChildren(new Option("No target", ""));
  dom.setupError.hidden = !error;
  dom.setupError.textContent = error;
  dom.tokenInput.value = "";
  clear(dom.railMeta);
  setConnection("offline", "Disconnected");
}

export function showWorkspace() {
  dom.setup.hidden = true;
  dom.inspectorContent.hidden = false;
  dom.targets.disabled = state.targets.length === 0;
}

export function setThemePreference(preference) {
  state.theme = ["auto", "light", "dark"].includes(preference) ? preference : "auto";
  try { localStorage.setItem(THEME_KEY, state.theme); } catch {}
  applyTheme();
}

export function applyTheme() {
  const resolved = state.theme === "auto"
    ? (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light")
    : state.theme;
  document.documentElement.dataset.theme = resolved;
  const iconName = state.theme === "auto" ? "auto" : resolved === "dark" ? "moon" : "sun";
  dom.themeIcon.setAttribute("href", `#i-${iconName}`);
  const label = state.theme === "auto" ? "follow system" : state.theme;
  dom.themeButton.title = `Theme: ${label}`;
  dom.themeButton.setAttribute("aria-label", `Theme: ${label}`);
}

function renderTargets() {
  const previous = state.targetId;
  dom.targets.replaceChildren();
  if (!state.targets.length) {
    dom.targets.append(new Option("No targets connected", ""));
    dom.targets.disabled = true;
    return;
  }
  const runtimes = state.targets.filter(target => target.kind === "runtime");
  const platforms = state.targets.filter(target => target.kind !== "runtime");
  if (runtimes.length) {
    const group = document.createElement("optgroup");
    group.label = "Cove runtimes";
    for (const target of runtimes) group.append(new Option(targetLabel(target), target.id));
    dom.targets.append(group);
  }
  if (platforms.length) {
    const group = document.createElement("optgroup");
    group.label = "Windows, displays and drivers";
    for (const target of platforms) group.append(new Option(targetLabel(target), target.id));
    dom.targets.append(group);
  }
  if (state.targets.some(target => target.id === previous)) dom.targets.value = previous;
  else if (state.targets.some(target => target.id === state.targetId)) dom.targets.value = state.targetId;
  else dom.targets.value = defaultTargetId();
  dom.targets.disabled = false;
}

export async function refreshSession(options = {}) {
  const quiet = options.quiet === true;
  if (!state.token) {
    showSetup();
    return;
  }
  if (state.sessionLoading) return;
  state.sessionLoading = true;
  setConnectionBusy(true);
  const previousTarget = state.targetId;
  try {
    const session = await rpc("get_ui_session");
    state.platform = String(session?.platform || "");
    state.targets = Array.isArray(session?.targets) ? session.targets : [];
    let next = previousTarget;
    if (!state.targets.some(target => target.id === next)) next = defaultTargetId();
    if (next !== previousTarget) {
      await activateTarget(next, { reset: true });
      renderTargets();
    } else {
      state.targetId = next;
      renderTargets();
      updateTargetUi();
      if (state.caps.tree && !state.treeLoaded && !state.treeLoading) loadTree();
      ensureCurrentView();
    }
    showWorkspace();
    if (!quiet) {
      const count = state.targets.length;
      toast(count ? `${count} target${count === 1 ? "" : "s"} connected.` : "No targets are connected. Start the app with COVE_DEVTOOLS=1.", count ? "success" : "info");
    }
  } catch (error) {
    if (error?.code === "unauthorized") {
      showSetup("The controller rejected this session token. Open the Inspector link again or paste a current token.");
    } else {
      setConnection("offline", "Offline");
      if (!quiet) toast(errorMessage(error), "error", 9000);
      if (!state.targets.length) {
        showWorkspace();
        renderTreeArea();
        renderDetails();
      }
    }
  } finally {
    state.sessionLoading = false;
    setConnectionBusy(false);
    if (state.token) ensureDeepLinks();
  }
}

export async function chooseTarget(id) {
  if (!id || id === state.targetId) return;
  await activateTarget(id, { reset: true });
}

async function activateTarget(id, options = {}) {
  const previousTarget = state.targetId;
  if (options.reset) resetTargetData(previousTarget);
  state.targetId = id;
  state.targetEpoch += 1;
  state.caps = readTargetCaps(targetById(id));
  dom.targets.value = id;
  updateTargetUi();
  if (state.caps.tree) {
    await loadTree();
  } else {
    state.treeLoaded = true;
    renderTreeArea();
    renderDetails();
  }
  ensureCurrentView();
  syncFollowTimers();
}

export function updateTargetUi() {
  const target = targetById();
  state.caps = readTargetCaps(target);
  if ((state.repaintHighlight.active || state.repaintHighlight.busy) && !state.caps.repaintHighlight) resetRepaintHighlight(target?.id || "");
  if ((state.debugPaint.guidelines || state.debugPaint.busy) && !state.caps.debugPaint) resetDebugPaintState(target?.id || "");
  if ((state.baselines.active || state.baselines.busy) && !state.caps.baselines) resetBaselines(target?.id || "");
  if ((state.slowAnimations.active || state.slowAnimations.busy) && !state.caps.animationSpeed) resetSlowAnimations(target?.id || "");
  if ((state.oversizedImages.active || state.oversizedImages.busy) && !state.caps.oversizedImages) resetOversizedImages(target?.id || "");
  if (state.mode === "pick" && (!state.caps.capture || !state.caps.inspectNodes)) {
    state.mode = "select";
  }
  if (state.mode === "interact" && !(state.caps.capture && state.caps.input)) state.mode = "select";
  const caps = state.caps;
  dom.captureButton.disabled = !caps.capture || state.captureLoading;
  dom.nodeShotButton.disabled = !(caps.inspectNodes && caps.capture) || !state.selectedId || state.captureLoading;
  dom.saveButton.disabled = !state.capture;
  dom.modePick.disabled = !(caps.inspectNodes && caps.capture);
  dom.treeRefresh.disabled = !caps.tree || state.treeLoading;
  dom.treeCollapse.disabled = !caps.tree || !state.treeLoaded;
  updateModeUi();
  dom.tree.hidden = !caps.tree || !state.treeLoaded || state.treeError !== null;
  renderCapabilityNote();
  renderRepaintHighlightUi();
  renderGuidelinesUi();
  renderBaselinesUi();
  renderSlowAnimationsUi();
  renderOversizedImagesUi();
  renderInputUi();
  updateRailMeta();
  if (!state.capture && caps.capture) dom.screenshotMeta.textContent = "";
  renderScreenshot();
  if (state.treeLoaded) renderTreeArea();
  if (state.selectedId) renderDetails();
}

function updateRailMeta() {
  const target = targetById();
  clear(dom.railMeta);
  if (!target) return;
  dom.railMeta.append(
    el("div", { text: target.kind === "runtime" ? "Runtime" : target.kind }),
    el("div", { text: target.platform || "" }),
  );
}

function renderCapabilityNote() {
  const target = targetById();
  const caps = state.caps;
  const notes = [];
  if (!target) {
    notes.push("No target is selected.");
  } else if (!caps.runtime && !caps.capture) {
    notes.push("This target exposes neither a component tree nor capture.");
  } else {
    if (!caps.tree) notes.push("This runtime does not advertise a component tree.");
    if (!caps.inspectNodes) notes.push("This runtime does not advertise inspect_nodes: pick-at-point and node screenshots are unavailable.");
    if (!caps.capture) notes.push("This runtime does not advertise frame capture.");
  }
  dom.capabilityNote.hidden = notes.length === 0;
  dom.capabilityNote.textContent = notes.join(" ");
}

// Repaint highlighting is driven by the runtime's real paint passes: the
// controller reports which nodes painted, not how many frames elapsed. The
// client only mirrors the applied state that `set_ui_repaint_highlight`
// returns, so cached layer reuse never shows up as a repaint here.
