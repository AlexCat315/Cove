/* Screenshot and pick-at-point */

function releaseCapture() {
  if (state.capture?.url) URL.revokeObjectURL(state.capture.url);
  state.capture = null;
  state.captureError = null;
  state.captureLoading = false;
  state.pickBusy = false;
}

async function captureWindow() {
  if (state.captureLoading || state.pickBusy) return;
  if (!state.caps.capture || !state.targetId) {
    toast("This target does not advertise capture.", "error");
    return;
  }
  const targetId = state.targetId;
  const epoch = state.targetEpoch;
  state.captureAttempt = "window";
  state.captureLoading = true;
  state.captureError = null;
  updateNodeButtons();
  renderScreenshot();
  try {
    const result = await rpc("capture_ui", { target: targetId });
    if (epoch !== state.targetEpoch || targetId !== state.targetId) return;
    await applyCaptureResult(result, "window", null, targetId, epoch);
  } catch (error) {
    if (epoch !== state.targetEpoch || targetId !== state.targetId) return;
    state.captureError = error;
    toast(errorMessage(error), "error", 9000);
  } finally {
    if (epoch === state.targetEpoch && targetId === state.targetId) {
      state.captureLoading = false;
      updateNodeButtons();
      renderScreenshot();
    }
  }
}

async function captureNode() {
  if (state.captureLoading || state.pickBusy) return;
  if (!state.caps.inspectNodes) {
    toast("This runtime does not advertise inspect_nodes.", "error", 8000);
    return;
  }
  if (!state.caps.capture) {
    toast("This runtime does not advertise frame capture.", "error", 8000);
    return;
  }
  if (!state.selectedId) {
    toast("Select a component before taking a node screenshot.", "info");
    return;
  }
  const targetId = state.targetId;
  const epoch = state.targetEpoch;
  const nodeId = state.selectedId;
  state.captureAttempt = "node";
  state.captureLoading = true;
  state.captureError = null;
  updateNodeButtons();
  renderScreenshot();
  try {
    const result = await rpc("capture_ui_node", { target: targetId, node_id: nodeId });
    if (epoch !== state.targetEpoch || targetId !== state.targetId) return;
    await applyCaptureResult(result, "node", result?.node_id || result?.nodeID || nodeId, targetId, epoch);
  } catch (error) {
    if (epoch !== state.targetEpoch || targetId !== state.targetId) return;
    state.captureError = error;
    toast(errorMessage(error), "error", 9000);
  } finally {
    if (epoch === state.targetEpoch && targetId === state.targetId) {
      state.captureLoading = false;
      updateNodeButtons();
      renderScreenshot();
    }
  }
}

async function applyCaptureResult(result, kind, nodeId, targetId, epoch) {
  if (!result || typeof result !== "object") throw new RpcError("The controller returned no capture.", "capture_response");
  const resource = typeof result.resource === "string" ? result.resource : "";
  if (!resource) throw new RpcError("The controller returned no capture resource.", "capture_response");
  let response;
  try {
    response = await fetch(resource, { headers: authHeaders() });
  } catch {
    throw new RpcError("Could not load the capture image from the controller.", "capture_resource");
  }
  if (!response.ok) throw new RpcError(`The capture resource request failed (HTTP ${response.status}).`, "capture_resource");
  const blob = await response.blob();
  if (!blob.size) throw new RpcError("The controller returned an empty capture image.", "capture_resource");
  if (epoch !== state.targetEpoch || targetId !== state.targetId) return false;
  if (state.capture?.url) URL.revokeObjectURL(state.capture.url);
  state.capture = {
    ...result,
    url: URL.createObjectURL(blob),
    kind,
    nodeId: nodeId || result.node_id || result.nodeID || null,
    contentFrame: result.contentFrame || result.content_frame || null,
    capturedAt: Date.now(),
    zoom: "fit",
  };
  state.captureError = null;
  state.captureLoading = false;
  updateNodeButtons();
  renderScreenshot();
  return true;
}

function captureDimensions(capture) {
  const width = numberValue(capture?.width) ?? numberValue(capture?.naturalWidth) ?? 0;
  const height = numberValue(capture?.height) ?? numberValue(capture?.naturalHeight) ?? 0;
  return { width, height };
}

function renderScreenshot() {
  const body = dom.screenshotBody;
  body.replaceChildren();
  body.classList.toggle("is-pick", state.mode === "pick" && state.capture?.kind === "window");
  body.classList.toggle("is-interact", state.mode === "interact" && state.capture?.kind === "window");
  body.classList.toggle("is-zoom", state.capture?.zoom === "actual");
  body.classList.toggle("is-busy", state.captureLoading || state.pickBusy);
  dom.screenshotZoom.hidden = !state.capture;
  dom.screenshotClear.hidden = !state.capture;
  dom.saveButton.disabled = !state.capture;
  dom.captureButton.disabled = !state.caps.capture || state.captureLoading;
  dom.nodeShotButton.disabled = !(state.caps.inspectNodes && state.caps.capture) || !state.selectedId || state.captureLoading;
  dom.screenshotZoom.textContent = state.capture?.zoom === "actual" ? "Fit" : "100%";

  if (state.captureLoading && !state.capture) {
    dom.screenshotMeta.textContent = state.captureAttempt === "node" ? "Capturing node…" : "Capturing window…";
    showState(body, { kind: "loading", title: "Capturing", compact: true });
    return;
  }
  if (state.captureError) {
    showState(body, {
      kind: "error",
      title: "Capture failed",
      message: errorMessage(state.captureError),
      compact: true,
      action: "Retry",
      onAction: retryCapture,
    });
    if (!state.capture) dom.screenshotMeta.textContent = "Error";
    return;
  }
  if (!state.capture) {
    const caps = state.caps;
    let message = "Capture the window to inspect it visually and pick components.";
    if (!caps.capture) message = "Select a runtime with frame capture.";
    showState(body, {
      icon: "camera",
      title: "No screenshot",
      message,
      compact: true,
      action: caps.capture ? "Capture" : "",
      onAction: caps.capture ? captureWindow : null,
    });
    dom.screenshotMeta.textContent = "";
    return;
  }

  const capture = state.capture;
  const image = document.createElement("img");
  image.src = capture.url;
  image.draggable = false;
  image.alt = capture.kind === "node"
    ? `Screenshot of ${nodeLabel(currentNode())}`
    : `Screenshot of ${targetById()?.title || "target"}`;
  image.addEventListener("load", () => {
    const width = numberValue(capture.width) ?? image.naturalWidth;
    const height = numberValue(capture.height) ?? image.naturalHeight;
    if (capture.width !== width || capture.height !== height) {
      capture.width = width;
      capture.height = height;
      updateScreenshotMeta();
    }
  });
  if (state.mode === "pick" && capture.kind === "window") {
    image.addEventListener("click", event => handlePickClick(event, image));
    image.title = "Click to select the component at this point";
  }
  if (state.mode === "interact" && capture.kind === "window") bindFrameInput(image, capture);
  body.append(image);
  updateScreenshotMeta();
}

function updateScreenshotMeta() {
  const capture = state.capture;
  if (!capture) {
    dom.screenshotMeta.textContent = "";
    return;
  }
  const { width, height } = captureDimensions(capture);
  const pieces = [];
  pieces.push(capture.kind === "node" ? "Node" : "Window");
  if (width && height) pieces.push(`${width}×${height}`);
  if (capture.source) pieces.push(String(capture.source));
  if (capture.kind === "node" && capture.nodeId) pieces.push(`node ${shortId(capture.nodeId)}`);
  if (capture.capture_id || capture.captureId) pieces.push(shortId(capture.capture_id || capture.captureId));
  dom.screenshotMeta.textContent = pieces.join(" · ");
  dom.screenshotMeta.title = pieces.join(" · ");
}

function handlePickClick(event, image) {
  if (state.mode !== "pick" || state.pickBusy || !state.capture) return;
  const capture = state.capture;
  if (capture.kind !== "window") return;
  const point = imagePoint(event, image, capture);
  if (point && capture.capture_id) pickAt(point.x, point.y, capture.capture_id);
}

function imagePoint(event, image, capture) {
  const rect = image.getBoundingClientRect();
  const { width, height } = captureDimensions(capture);
  if (!rect.width || !rect.height || !width || !height) return null;
  const x = (event.clientX - rect.left) / rect.width;
  const y = (event.clientY - rect.top) / rect.height;
  if (x < 0 || x >= 1 || y < 0 || y >= 1) return null;
  return { x: Math.floor(x * width), y: Math.floor(y * height) };
}

function bindFrameInput(image, capture) {
  image.draggable = false;
  let press = null;
  image.addEventListener("pointerdown", event => {
    if (state.inputBusy || event.button > 2 || !event.isPrimary) return;
    const point = imagePoint(event, image, capture);
    if (!point) return;
    event.preventDefault();
    image.setPointerCapture(event.pointerId);
    press = { point, button: event.button, id: event.pointerId };
  });
  image.addEventListener("pointercancel", () => { press = null; });
  image.addEventListener("pointerup", event => {
    const start = press;
    press = null;
    if (!start || start.id !== event.pointerId) return;
    const end = imagePoint(event, image, capture);
    if (!end) return;
    const moved = Math.hypot(end.x - start.point.x, end.y - start.point.y) > 4;
    if (moved && start.button === 0) {
      sendFrameInput(capture, { kind: "drag", ...start.point, to_x: end.x, to_y: end.y });
    } else if (!moved) {
      sendFrameInput(capture, { kind: "click", ...end, button: start.button });
    }
  });
  image.addEventListener("contextmenu", event => event.preventDefault());
  let scroll;
  let timer;
  image.addEventListener("wheel", event => {
    const point = imagePoint(event, image, capture);
    if (!point || state.inputBusy) return;
    event.preventDefault();
    const divisor = event.deltaMode === 0 ? 40 : event.deltaMode === 2 ? 0.1 : 1;
    scroll ??= { kind: "scroll", ...point, dx: 0, dy: 0 };
    scroll.dx += event.deltaX / divisor;
    scroll.dy += event.deltaY / divisor;
    clearTimeout(timer);
    timer = setTimeout(() => {
      const action = scroll;
      scroll = null;
      const delta = value => Math.max(-10000, Math.min(10000, Math.round(value) || Math.sign(value)));
      sendFrameInput(capture, { ...action, dx: delta(action.dx), dy: delta(action.dy) });
    }, 100);
  }, { passive: false });
}

async function sendFrameInput(capture, action) {
  if (state.mode !== "interact" || state.capture !== capture || state.inputBusy) return;
  const target = state.targetId;
  const epoch = state.targetEpoch;
  setInputBusy(true);
  try {
    await rpc("input_ui", { target, capture_id: capture.capture_id, action });
    if (epoch === state.targetEpoch) await captureWindow();
  } catch (error) {
    if (epoch === state.targetEpoch) toast(errorMessage(error), "error");
  } finally {
    if (epoch === state.targetEpoch) setInputBusy(false);
  }
}

async function pickAt(x, y, captureId) {
  const targetId = state.targetId;
  const epoch = state.targetEpoch;
  state.pickBusy = true;
  renderScreenshot();
  try {
    const result = await rpc("select_ui_at", { target: targetId, capture_id: captureId, x, y });
    if (epoch !== state.targetEpoch || targetId !== state.targetId) return;
    if (result?.capture_id && state.capture) state.capture.capture_id = result.capture_id;
    const hit = result?.hit || null;
    const nodeId = hit?.nodeID || hit?.nodeId || hit?.node_id || null;
    if (!nodeId) {
      state.selectedId = null;
      state.detail = null;
      state.detailError = null;
      state.detailRequest += 1;
      clearTimeout(state.detailTimer);
      if (state.capture?.kind === "node") releaseCapture();
      renderTreeArea();
      renderDetails();
      updateNodeButtons();
      renderScreenshot();
      toast("No component was hit at that point.", "info");
      return;
    }
    if (!state.entries.has(nodeId)) {
      await loadTree({ preserveExpansion: true });
      if (epoch !== state.targetEpoch || targetId !== state.targetId) return;
    }
    if (!state.entries.has(nodeId)) {
      toast("The runtime hit a node that is not in the current tree. Refresh and try again.", "error", 8000);
      return;
    }
    for (const pathId of Array.isArray(hit.path) ? hit.path : []) {
      if (state.entries.has(pathId)) state.expanded.add(pathId);
    }
    state.mode = "pick";
    updateTargetUi();
    if (state.selectedId === nodeId) {
      highlightNode(nodeId);
      loadNodeDetail(nodeId);
      renderDetails();
    } else {
      selectNode(nodeId, { reveal: true, reason: "pick" });
    }
    const contentPoint = result.contentPoint || result.content_point;
    const pointText = contentPoint && numberValue(contentPoint.x) !== null
      ? ` at content ${fmtNumber(contentPoint.x)}×${fmtNumber(contentPoint.y)}`
      : "";
    toast(`Picked ${nodeLabel(currentNode())}${pointText}.`, "success", 4000);
  } catch (error) {
    if (epoch !== state.targetEpoch || targetId !== state.targetId) return;
    if (error?.code === "stale_capture") {
      releaseCapture();
      renderScreenshot();
    }
    toast(`Pick failed: ${errorMessage(error)}`, "error", 9000);
  } finally {
    if (epoch === state.targetEpoch && targetId === state.targetId) {
      state.pickBusy = false;
      renderScreenshot();
    }
  }
}

function saveCapture() {
  const capture = state.capture;
  if (!capture?.url) return;
  const target = targetById();
  const node = currentNode();
  const kind = capture.kind === "node" && node ? `node-${slug(nodeLabel(node))}` : "window";
  const stamp = new Date().toISOString().replace(/[:.]/g, "-");
  const name = `cove-${slug(target?.title || "target")}-${kind}-${stamp}.png`;
  const link = document.createElement("a");
  link.href = capture.url;
  link.download = name;
  document.body.append(link);
  link.click();
  link.remove();
  toast(`Saved ${name}.`, "success", 4000);
}

function retryCapture() {
  if (isDisconnectedError(state.captureError)) refreshCurrent();
  else if (state.captureAttempt === "node") captureNode();
  else captureWindow();
}

function toggleScreenshotZoom() {
  if (!state.capture) return;
  state.capture.zoom = state.capture.zoom === "actual" ? "fit" : "actual";
  renderScreenshot();
}

function clearScreenshot() {
  releaseCapture();
  updateTargetUi();
  renderScreenshot();
}

function enterPickMode() {
  if (!state.caps.inspectNodes) {
    state.mode = "select";
    updateModeUi();
    toast("This runtime does not advertise inspect_nodes.", "error", 8000);
    return;
  }
  if (!state.caps.capture) {
    state.mode = "select";
    updateModeUi();
    toast("This target does not advertise capture; pick-at-point needs a screenshot.", "error", 8000);
    return;
  }
  state.mode = "pick";
  updateModeUi();
  if (!state.capture || state.capture.kind !== "window") {
    captureWindow();
  } else {
    renderScreenshot();
  }
  dom.pickHintText.textContent = "Click a point in the screenshot. The runtime resolves it to a component through a real hit test.";
}

function exitPickMode() {
  state.mode = "select";
  updateModeUi();
  renderScreenshot();
  renderDetails();
}

function updateModeUi() {
  dom.modeSelect.setAttribute("aria-pressed", String(state.mode === "select"));
  dom.modePick.setAttribute("aria-pressed", String(state.mode === "pick"));
  dom.modePick.disabled = !(state.caps.inspectNodes && state.caps.capture);
  dom.modeInteract.disabled = !(state.caps.input && state.caps.capture);
  dom.modeInteract.setAttribute("aria-pressed", String(state.mode === "interact"));
  dom.pickHint.hidden = state.mode !== "pick";
  dom.screenshotBody.classList.toggle("is-pick", state.mode === "pick" && state.capture?.kind === "window");
  if (!state.selectedId && state.mode === "pick") renderDetails();
}

