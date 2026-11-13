import { NORMAL_ANIMATION_SCALE, SLOW_ANIMATION_SCALE, dom, state } from "./globals.js";
import { errorMessage, rpc, targetById, toast, unwrapData } from "./session.js";

export function renderRepaintHighlightUi() {
  const target = targetById();
  const available = Boolean(state.connected && target && state.caps.runtime && state.caps.repaintHighlight);
  const on = available && state.repaintHighlight.active;
  let title = on ? "Hide repaint highlighting on the target" : "Highlight repainted nodes on the target";
  if (!state.connected) title = "Reconnect to the controller to toggle repaint highlighting";
  else if (!target || !state.caps.runtime) title = "Repaint highlighting needs a Cove runtime target";
  else if (!state.caps.repaintHighlight) title = "This runtime does not advertise repaint_highlight";
  dom.repaintHighlightButton.disabled = !available || state.repaintHighlight.busy;
  dom.repaintHighlightButton.setAttribute("aria-pressed", String(on));
  dom.repaintHighlightButton.classList.toggle("is-active", on);
  dom.repaintHighlightButton.title = title;
}

function sendRepaintHighlight(targetId, enabled) {
  const run = state.repaintHighlight.queue.then(() => rpc("set_ui_repaint_highlight", { target: targetId, enabled }));
  state.repaintHighlight.queue = run.then(() => {}, () => {});
  return run;
}

function repaintHighlightFrom(result, requested) {
  const data = unwrapData(result);
  if (typeof data === "boolean") return data;
  const record = data && typeof data === "object" ? data : null;
  for (const value of [record?.repaintHighlight, record?.repaint_highlight, record?.highlightRepaints, record?.highlight_repaints, record?.enabled]) {
    if (typeof value === "boolean") return value;
  }
  return requested;
}

export function resetRepaintHighlight(previousTarget = state.targetId) {
  const wasOn = state.repaintHighlight.active;
  const inFlight = state.repaintHighlight.busy;
  state.repaintHighlight.request += 1;
  state.repaintHighlight.active = false;
  state.repaintHighlight.busy = false;
  renderRepaintHighlightUi();
  if (!previousTarget || (!wasOn && !inFlight)) return;
  sendRepaintHighlight(previousTarget, false).catch(() => {});
}

export async function toggleRepaintHighlight() {
  const target = targetById();
  if (!state.connected || !target || !state.caps.runtime || !state.caps.repaintHighlight || state.repaintHighlight.busy) return;
  const targetId = target.id;
  const epoch = state.targetEpoch;
  const request = ++state.repaintHighlight.request;
  const next = !state.repaintHighlight.active;
  state.repaintHighlight.busy = true;
  renderRepaintHighlightUi();
  try {
    const enabled = repaintHighlightFrom(await sendRepaintHighlight(targetId, next), next);
    if (request !== state.repaintHighlight.request || epoch !== state.targetEpoch || targetId !== state.targetId) return;
    state.repaintHighlight.active = enabled;
  } catch (error) {
    if (request === state.repaintHighlight.request && epoch === state.targetEpoch && targetId === state.targetId) {
      toast(`Highlight repaints failed: ${errorMessage(error)}`, "error", 8000);
    }
  } finally {
    if (request === state.repaintHighlight.request) {
      state.repaintHighlight.busy = false;
      renderRepaintHighlightUi();
    }
  }
}

export function renderGuidelinesUi() {
  const target = targetById();
  const available = Boolean(state.connected && target && state.caps.runtime && state.caps.debugPaint);
  const on = available && state.debugPaint.guidelines;
  let title = on ? "Hide layout guidelines on the target" : "Show layout guidelines on the target";
  if (!state.connected) title = "Reconnect to the controller to toggle layout guidelines";
  else if (!target || !state.caps.runtime) title = "Layout guidelines need a Cove runtime target";
  else if (!state.caps.debugPaint) title = "This runtime does not advertise debug_paint";
  dom.guidelinesButton.disabled = !available || state.debugPaint.busy;
  dom.guidelinesButton.setAttribute("aria-pressed", String(on));
  dom.guidelinesButton.classList.toggle("is-active", on);
  dom.guidelinesButton.title = title;
}

function sendDebugPaint(targetId, guidelines) {
  const run = state.debugPaint.queue.then(() => rpc("set_ui_debug_paint", { target: targetId, guidelines }));
  state.debugPaint.queue = run.then(() => {}, () => {});
  return run;
}

function debugPaintFrom(result, requested) {
  const data = unwrapData(result);
  if (typeof data === "boolean") return data;
  const record = data && typeof data === "object" ? data : null;
  for (const value of [record?.guidelines, record?.debugPaint, record?.debug_paint, record?.enabled]) {
    if (typeof value === "boolean") return value;
  }
  return requested;
}

export function resetDebugPaintState(previousTarget = state.targetId) {
  const wasOn = state.debugPaint.guidelines;
  const inFlight = state.debugPaint.busy;
  state.debugPaint.request += 1;
  state.debugPaint.guidelines = false;
  state.debugPaint.busy = false;
  renderGuidelinesUi();
  if (!previousTarget || (!wasOn && !inFlight)) return;
  sendDebugPaint(previousTarget, false).catch(() => {});
}

export async function toggleGuidelines() {
  const target = targetById();
  if (!state.connected || !target || !state.caps.runtime || !state.caps.debugPaint || state.debugPaint.busy) return;
  const targetId = target.id;
  const epoch = state.targetEpoch;
  const request = ++state.debugPaint.request;
  const next = !state.debugPaint.guidelines;
  state.debugPaint.busy = true;
  renderGuidelinesUi();
  try {
    const enabled = debugPaintFrom(await sendDebugPaint(targetId, next), next);
    if (request !== state.debugPaint.request || epoch !== state.targetEpoch || targetId !== state.targetId) return;
    state.debugPaint.guidelines = enabled;
  } catch (error) {
    if (request === state.debugPaint.request && epoch === state.targetEpoch && targetId === state.targetId) {
      toast(`Guidelines failed: ${errorMessage(error)}`, "error", 8000);
    }
  } finally {
    if (request === state.debugPaint.request) {
      state.debugPaint.busy = false;
      renderGuidelinesUi();
    }
  }
}

export function renderBaselinesUi() {
  const target = targetById();
  const available = Boolean(state.connected && target && state.caps.runtime && state.caps.baselines);
  const on = available && state.baselines.active;
  let title = on ? "Hide text baselines on the target" : "Show text baselines on the target";
  if (!state.connected) title = "Reconnect to the controller to toggle text baselines";
  else if (!target || !state.caps.runtime) title = "Text baselines need a Cove runtime target";
  else if (!state.caps.baselines) title = "This runtime does not advertise baselines";
  dom.baselinesButton.disabled = !available || state.baselines.busy;
  dom.baselinesButton.setAttribute("aria-pressed", String(on));
  dom.baselinesButton.classList.toggle("is-active", on);
  dom.baselinesButton.title = title;
}

function sendBaselines(targetId, enabled) {
  const run = state.baselines.queue.then(() => rpc("set_ui_baselines", { target: targetId, enabled }));
  state.baselines.queue = run.then(() => {}, () => {});
  return run;
}

function baselinesFrom(result, requested) {
  const data = unwrapData(result);
  if (typeof data === "boolean") return data;
  const record = data && typeof data === "object" ? data : null;
  for (const value of [record?.baselines, record?.baselinesEnabled, record?.baselines_enabled, record?.enabled]) {
    if (typeof value === "boolean") return value;
  }
  return requested;
}

export function resetBaselines(previousTarget = state.targetId) {
  const wasOn = state.baselines.active;
  const inFlight = state.baselines.busy;
  state.baselines.request += 1;
  state.baselines.active = false;
  state.baselines.busy = false;
  renderBaselinesUi();
  if (!previousTarget || (!wasOn && !inFlight)) return;
  sendBaselines(previousTarget, false).catch(() => {});
}

export async function toggleBaselines() {
  const target = targetById();
  if (!state.connected || !target || !state.caps.runtime || !state.caps.baselines || state.baselines.busy) return;
  const targetId = target.id;
  const epoch = state.targetEpoch;
  const request = ++state.baselines.request;
  const next = !state.baselines.active;
  state.baselines.busy = true;
  renderBaselinesUi();
  try {
    const enabled = baselinesFrom(await sendBaselines(targetId, next), next);
    if (request !== state.baselines.request || epoch !== state.targetEpoch || targetId !== state.targetId) return;
    state.baselines.active = enabled;
  } catch (error) {
    if (request === state.baselines.request && epoch === state.targetEpoch && targetId === state.targetId) {
      toast(`Baselines failed: ${errorMessage(error)}`, "error", 8000);
    }
  } finally {
    if (request === state.baselines.request) {
      state.baselines.busy = false;
      renderBaselinesUi();
    }
  }
}

function sendAnimationSpeed(targetId, scale) {
  const run = state.slowAnimations.queue.then(() => rpc("set_ui_animation_speed", { target: targetId, scale }));
  state.slowAnimations.queue = run.then(() => {}, () => {});
  return run;
}

function animationScaleFrom(result, requested) {
  const data = unwrapData(result);
  if (typeof data === "number" && Number.isFinite(data)) return data;
  const record = data && typeof data === "object" ? data : null;
  for (const value of [record?.scale, record?.animationSpeed, record?.animation_speed, record?.speed, record?.timeDilation, record?.time_dilation]) {
    if (typeof value === "number" && Number.isFinite(value)) return value;
  }
  return requested;
}

function isSlowAnimationScale(scale) {
  return Number.isFinite(scale) && scale > 0 && scale < 0.999;
}

export function renderSlowAnimationsUi() {
  const target = targetById();
  const available = Boolean(state.connected && target && state.caps.runtime && state.caps.animationSpeed);
  const on = available && state.slowAnimations.active;
  let title = on ? "Resume normal animation speed" : "Slow animations to 0.2× the runtime time scale";
  if (!state.connected) title = "Reconnect to the controller to change the animation speed";
  else if (!target || !state.caps.runtime) title = "Slow animations need a Cove runtime target";
  else if (!state.caps.animationSpeed) title = "This runtime does not advertise animation_speed";
  dom.slowAnimationsButton.disabled = !available || state.slowAnimations.busy;
  dom.slowAnimationsButton.setAttribute("aria-pressed", String(on));
  dom.slowAnimationsButton.classList.toggle("is-active", on);
  dom.slowAnimationsButton.title = title;
}

export function resetSlowAnimations(previousTarget = state.targetId) {
  const wasSlowed = state.slowAnimations.active;
  const inFlight = state.slowAnimations.busy;
  state.slowAnimations.request += 1;
  state.slowAnimations.active = false;
  state.slowAnimations.scale = NORMAL_ANIMATION_SCALE;
  state.slowAnimations.busy = false;
  renderSlowAnimationsUi();
  if (!previousTarget || (!wasSlowed && !inFlight)) return;
  sendAnimationSpeed(previousTarget, NORMAL_ANIMATION_SCALE).catch(() => {});
}

export async function toggleSlowAnimations() {
  const target = targetById();
  if (!state.connected || !target || !state.caps.runtime || !state.caps.animationSpeed || state.slowAnimations.busy) return;
  const targetId = target.id;
  const epoch = state.targetEpoch;
  const request = ++state.slowAnimations.request;
  const next = state.slowAnimations.active ? NORMAL_ANIMATION_SCALE : SLOW_ANIMATION_SCALE;
  state.slowAnimations.busy = true;
  renderSlowAnimationsUi();
  try {
    const scale = animationScaleFrom(await sendAnimationSpeed(targetId, next), next);
    if (request !== state.slowAnimations.request || epoch !== state.targetEpoch || targetId !== state.targetId) return;
    state.slowAnimations.active = isSlowAnimationScale(scale);
    state.slowAnimations.scale = scale;
  } catch (error) {
    if (request === state.slowAnimations.request && epoch === state.targetEpoch && targetId === state.targetId) {
      toast(`Slow animations failed: ${errorMessage(error)}`, "error", 8000);
    }
  } finally {
    if (request === state.slowAnimations.request) {
      state.slowAnimations.busy = false;
      renderSlowAnimationsUi();
    }
  }
}

export function renderOversizedImagesUi() {
  const target = targetById();
  const available = Boolean(state.connected && target && state.caps.runtime && state.caps.oversizedImages);
  const on = available && state.oversizedImages.active;
  let title = on ? "Hide oversized image highlighting on the target" : "Highlight images decoded larger than their display rect";
  if (!state.connected) title = "Reconnect to the controller to toggle oversized image highlighting";
  else if (!target || !state.caps.runtime) title = "Oversized image highlighting needs a Cove runtime target";
  else if (!state.caps.oversizedImages) title = "This runtime does not advertise oversized_images";
  dom.oversizedImagesButton.disabled = !available || state.oversizedImages.busy;
  dom.oversizedImagesButton.setAttribute("aria-pressed", String(on));
  dom.oversizedImagesButton.classList.toggle("is-active", on);
  dom.oversizedImagesButton.title = title;
}

function sendOversizedImages(targetId, enabled) {
  const run = state.oversizedImages.queue.then(() => rpc("set_ui_oversized_images", { target: targetId, enabled }));
  state.oversizedImages.queue = run.then(() => {}, () => {});
  return run;
}

function oversizedImagesFrom(result, requested) {
  const data = unwrapData(result);
  if (typeof data === "boolean") return data;
  const record = data && typeof data === "object" ? data : null;
  for (const value of [record?.oversizedImages, record?.oversized_images, record?.enabled]) {
    if (typeof value === "boolean") return value;
  }
  return requested;
}

export function resetOversizedImages(previousTarget = state.targetId) {
  const wasOn = state.oversizedImages.active;
  const inFlight = state.oversizedImages.busy;
  state.oversizedImages.request += 1;
  state.oversizedImages.active = false;
  state.oversizedImages.busy = false;
  renderOversizedImagesUi();
  if (!previousTarget || (!wasOn && !inFlight)) return;
  sendOversizedImages(previousTarget, false).catch(() => {});
}

export async function toggleOversizedImages() {
  const target = targetById();
  if (!state.connected || !target || !state.caps.runtime || !state.caps.oversizedImages || state.oversizedImages.busy) return;
  const targetId = target.id;
  const epoch = state.targetEpoch;
  const request = ++state.oversizedImages.request;
  const next = !state.oversizedImages.active;
  state.oversizedImages.busy = true;
  renderOversizedImagesUi();
  try {
    const enabled = oversizedImagesFrom(await sendOversizedImages(targetId, next), next);
    if (request !== state.oversizedImages.request || epoch !== state.targetEpoch || targetId !== state.targetId) return;
    state.oversizedImages.active = enabled;
  } catch (error) {
    if (request === state.oversizedImages.request && epoch === state.targetEpoch && targetId === state.targetId) {
      toast(`Oversized images failed: ${errorMessage(error)}`, "error", 8000);
    }
  } finally {
    if (request === state.oversizedImages.request) {
      state.oversizedImages.busy = false;
      renderOversizedImagesUi();
    }
  }
}

