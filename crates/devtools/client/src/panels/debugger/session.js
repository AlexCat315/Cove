import { $, DEBUGGER_RPC_TIMEOUT_MS, RpcError, state } from "../../core/globals.js";
import { errorMessage, rpc, targetById, toast, unwrapData } from "../../core/session.js";
import { applyDebuggerSnapshot, clearDebuggerSessionData, debuggerBreakpointList, debuggerBreakpointsAt, debuggerFrameList, debuggerHasData, debuggerNumber, debuggerPathKey, debuggerPathsMatch, debuggerRoot, debuggerScopeList, debuggerThreadList, debuggerUnavailableInfo, debuggerVariablesFrom, normalizeDebuggerSource, resetDebuggerVariables, selectedDebuggerFrame, sortDebuggerBreakpoints } from "./model.js";
import { appendDebuggerConsole, debuggerScopeKey, renderDebugger, renderDebuggerBreakpoints, renderDebuggerChrome, renderDebuggerSource, renderDebuggerStack, renderDebuggerThreads, renderDebuggerVariables, updateDebuggerButtons, updateDebuggerSourceHead } from "./view.js";
import { syncDebuggerTimer } from "../network.js";
import { firstStringValue, isObjectValue } from "../performance/profile/model.js";

function debuggerRequestParams(params = {}) {
  const body = {};
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== null) body[key] = value;
  }
  return body;
}

export async function debuggerCall(method, params = {}, options = {}) {
  const target = options.target ?? state.targetId;
  if (!target) throw new RpcError("Select a runtime target first.", "no_target");
  const timeoutMs = options.timeout ?? DEBUGGER_RPC_TIMEOUT_MS;
  const externalSignal = options.rpcOptions?.signal;
  const controller = externalSignal ? null : new AbortController();
  const timer = controller ? setTimeout(() => controller.abort(), timeoutMs) : null;
  try {
    return await unwrapData(await rpc(method, { target, ...debuggerRequestParams(params) }, {
      ...options.rpcOptions,
      signal: externalSignal || controller.signal,
    }));
  } catch (error) {
    if (error?.name === "AbortError") {
      if (externalSignal?.aborted) throw error;
      throw new RpcError(`The debugger request timed out after ${Math.round(timeoutMs / 1000)} s.`, "timeout");
    }
    throw error;
  } finally {
    if (timer) clearTimeout(timer);
  }
}

async function debuggerSourceCall(params, options = {}) {
  const missing = [];
  for (const method of ["get_ui_debug_source", "read_ui_debug_source"]) {
    try {
      return { method, data: await debuggerCall(method, params, options) };
    } catch (error) {
      if (error?.code !== "unknown_command") throw error;
      missing.push(`${method} (${errorMessage(error)})`);
    }
  }
  throw new RpcError(`The controller exposes no debugger source tool. Tried ${missing.join(" and ")}.`, "unknown_command");
}

export async function refreshDebugger(options = {}) {
  const dbg = state.debugger;
  if (!state.token) {
    renderDebugger();
    return;
  }
  const target = targetById();
  if (!target || !state.caps.runtime) {
    renderDebugger();
    return;
  }
  if (dbg.statusLoading) return;
  if (options.poll && (dbg.busy || !dbg.attached || dbg.unavailable || dbg.status === "attaching")) return;
  const targetId = state.targetId;
  const request = dbg.request + 1;
  dbg.request = request;
  dbg.statusLoading = true;
  if (!options.quiet) renderDebuggerChrome();
  try {
    const data = await debuggerCall("get_ui_debugger", {}, { target: targetId });
    if (request !== dbg.request || targetId !== state.targetId) return;
    if (!isObjectValue(data)) throw new RpcError("The controller returned an empty debugger status.", "invalid_response");
    applyDebuggerSnapshot(data);
    dbg.loaded = true;
    dbg.lastRefresh = Date.now();
    if (dbg.attached) {
      await ensureDebuggerSessionData({ quiet: true });
      if (dbg.capabilities?.supports_log_points) {
        const output = await debuggerCall("get_ui_debugger_output", { after: dbg.outputAfter }, { target: targetId }).catch(() => null);
        if (request !== dbg.request || targetId !== state.targetId) return;
        for (const entry of output?.events ?? []) {
          appendDebuggerConsole(entry.category === "stderr" ? "error" : "event", entry.text);
          dbg.outputAfter = entry.sequence;
        }
      }
    }
  } catch (error) {
    if (request !== dbg.request || targetId !== state.targetId) return;
    const message = errorMessage(error);
    dbg.phase = "";
    dbg.attached = false;
    dbg.reason = message;
    dbg.reasonCode = error.code || "";
    dbg.error = message;
    dbg.unavailable = ["unavailable", "unsupported", "not_supported", "unknown_command", "not_available"].includes(error?.code);
    dbg.status = dbg.unavailable ? "unavailable" : "error";
    if (["unavailable", "unknown_command", "unsupported", "not_supported", "not_available"].includes(error?.code)) dbg.adapterAvailable = false;
    dbg.pid = null;
    dbg.sessionId = "";
    clearDebuggerSessionData({ keepSource: false, keepBreakpoints: false });
    if (!options.quiet) toast(message, "error", 9000);
  } finally {
    if (request === dbg.request && targetId === state.targetId) {
      dbg.statusLoading = false;
      renderDebugger();
      syncDebuggerTimer();
    }
  }
}

async function ensureDebuggerSessionData(options = {}) {
  const dbg = state.debugger;
  if (!dbg.attached || dbg.unavailable || dbg.status !== "paused") return;
  if (!dbg.stackLoaded && !dbg.stackLoading) await loadDebuggerStack(dbg.threadId, options);
  if (dbg.status !== "paused") return;
  const frame = selectedDebuggerFrame();
  if (!frame) return;
  if (!dbg.sourceLoaded && !dbg.sourceLoading) await loadDebuggerSourceForFrame(frame, options);
}

export function ensureDebugger() {
  if (!state.token || !state.targetId || !state.caps.runtime) {
    renderDebugger();
    return;
  }
  if (!state.debugger.loaded && !state.debugger.statusLoading) {
    refreshDebugger({ quiet: true });
    return;
  }
  renderDebugger();
}

export async function loadDebuggerStack(threadId, options = {}) {
  const dbg = state.debugger;
  if (!state.token || !state.targetId || !state.caps.runtime || !dbg.attached) return;
  if (dbg.stackLoading) return;
  const targetId = state.targetId;
  const requested = threadId !== null && threadId !== undefined ? threadId : null;
  const request = dbg.stackRequest + 1;
  dbg.stackRequest = request;
  dbg.stackLoading = true;
  dbg.stackError = null;
  dbg.threadsError = null;
  dbg.renderSig.stack = "";
  dbg.renderSig.threads = "";
  renderDebuggerStack();
  renderDebuggerThreads();
  try {
    const data = await debuggerCall("get_ui_debug_stack", { thread_id: requested, start_frame: 0, levels: 100 }, { target: targetId });
    if (request !== dbg.stackRequest || targetId !== state.targetId) return;
    if (requested !== null && dbg.threadId !== null && dbg.threadId !== requested && !debuggerThreadList(data)?.some(thread => thread.id === requested)) return;
    if (debuggerHasData(data)) applyDebuggerSnapshot(data);
    const root = debuggerRoot(data);
    const frames = debuggerFrameList(data);
    if (frames) {
      dbg.frames = frames;
      dbg.stackLoaded = true;
      dbg.stackError = null;
      dbg.framesTotal = debuggerNumber(root.total_frames) ?? null;
      dbg.framesTruncated = root.truncated === true;
    } else if (!dbg.stackLoaded) {
      dbg.stackError = new RpcError("The stack response contained no frames.", "invalid_response");
    }
    if (isObjectValue(root.threads_error)) {
      dbg.threadsError = new RpcError(
        firstStringValue(root.threads_error.message) || "The adapter could not read threads.",
        firstStringValue(root.threads_error.code) || "adapter_error",
      );
    }
    if (isObjectValue(root.scopes_error)) {
      dbg.scopesError = new RpcError(
        firstStringValue(root.scopes_error.message) || "The adapter could not read scopes.",
        firstStringValue(root.scopes_error.code) || "adapter_error",
      );
      dbg.scopesLoaded = true;
    }
    if (root.scopes_truncated === true) dbg.scopesTruncated = true;
    resetDebuggerVariables();
    if (dbg.frameIndex === null && dbg.frames.length) {
      dbg.frameIndex = 0;
      dbg.frameId = dbg.frames[0].id;
    }
    const frame = selectedDebuggerFrame();
    if (frame) {
      await selectDebuggerFrame(frame, dbg.frameIndex, {
        skipStackRender: true,
        skipScopes: true,
        quiet: options.quiet !== false,
      });
      await openDebuggerFirstScope({ quiet: options.quiet !== false });
    } else {
      dbg.scopes = [];
      dbg.scopesLoaded = true;
      dbg.scopesError = null;
    }
  } catch (error) {
    if (request !== dbg.stackRequest || targetId !== state.targetId) return;
    dbg.stackError = error;
    appendDebuggerConsole("error", `Call stack: ${errorMessage(error)}`);
  } finally {
    if (request === dbg.stackRequest && targetId === state.targetId) {
      dbg.stackLoading = false;
      dbg.renderSig.stack = "";
      dbg.renderSig.threads = "";
      renderDebuggerStack();
      renderDebuggerThreads();
      renderDebuggerVariables();
      updateDebuggerButtons();
    }
  }
}

function debuggerFrameSourceMatches(frame) {
  const source = state.debugger.source;
  if (!source || !frame) return false;
  const frameSource = frame.source || {};
  if (source.frameId !== null && frame.id !== null && source.frameId === frame.id
    && (!source.path || !frameSource.path || debuggerPathsMatch(frameSource.path, source.path))) return true;
  if (frameSource.path && source.path && debuggerPathsMatch(frameSource.path, source.path)) return true;
  if (frameSource.sourceReference !== null && frameSource.sourceReference !== undefined
    && source.sourceReference !== null && source.sourceReference !== undefined
    && String(frameSource.sourceReference) === String(source.sourceReference)) return true;
  return false;
}

export async function selectDebuggerFrame(frame, index, options = {}) {
  const dbg = state.debugger;
  if (!frame) return;
  const sameFrame = dbg.frameIndex === index;
  dbg.frameIndex = index;
  dbg.frameId = frame.id;
  if (!sameFrame && !options.skipScopes) {
    dbg.scopes = [];
    dbg.scopesLoaded = false;
    dbg.scopesError = null;
    dbg.scopesTruncated = false;
    resetDebuggerVariables();
  }
  dbg.renderSig.stack = "";
  dbg.renderSig.variables = "";
  if (!options.skipStackRender) renderDebuggerStack();
  updateDebuggerButtons();
  const tasks = [];
  const sourceMatches = debuggerFrameSourceMatches(frame);
  if (sourceMatches) {
    if (dbg.source) dbg.source.frameId = frame.id;
    dbg.sourceLoaded = true;
    dbg.renderSig.source = "";
    renderDebuggerSource(true);
  } else {
    tasks.push(loadDebuggerSourceForFrame(frame, { quiet: options.quiet !== false }));
  }
  if (options.skipScopes) {
    // get_ui_debug_stack already returned the top frame's scopes.
  } else if (!dbg.scopesLoaded && !dbg.scopesLoading && frame.id !== null) {
    tasks.push(loadDebuggerScopes(frame, { quiet: options.quiet !== false }));
  } else if (frame.id === null) {
    dbg.scopes = [];
    dbg.scopesLoaded = true;
    dbg.scopesError = null;
  }
  await Promise.all(tasks);
}

export async function selectDebuggerThread(thread) {
  if (!thread || thread.id === null) return;
  const dbg = state.debugger;
  if (dbg.threadId === thread.id) return;
  dbg.threadId = thread.id;
  dbg.frames = [];
  dbg.stackLoaded = false;
  dbg.stackError = null;
  dbg.frameIndex = null;
  dbg.frameId = null;
  dbg.framesTotal = null;
  dbg.framesTruncated = false;
  dbg.scopes = [];
  dbg.scopesLoaded = false;
  dbg.scopesError = null;
  dbg.scopesTruncated = false;
  resetDebuggerVariables();
  dbg.renderSig = { ...dbg.renderSig, threads: "", stack: "", variables: "", source: "" };
  renderDebuggerThreads();
  renderDebuggerStack();
  renderDebuggerVariables();
  if (dbg.status === "paused") await loadDebuggerStack(thread.id, { quiet: true });
}

async function openDebuggerFirstScope(options = {}) {
  const dbg = state.debugger;
  if (!dbg.scopes.length || dbg.expandedScopes.size) return;
  const first = dbg.scopes[0];
  const key = debuggerScopeKey(0);
  if (!first.variables.length && first.reference === null) return;
  dbg.expandedScopes.add(key);
  dbg.renderSig.variables = "";
  if (!first.variables.length && first.reference !== null) {
    await loadDebuggerVariableChildren(key, first.reference, { quiet: options.quiet !== false });
  }
}

export async function loadDebuggerScopes(frame, options = {}) {
  const dbg = state.debugger;
  if (!state.token || !state.targetId || !state.caps.runtime || !frame || frame.id === null || !dbg.attached) return;
  const targetId = state.targetId;
  const frameIndex = dbg.frameIndex;
  const request = dbg.scopesRequest + 1;
  dbg.scopesRequest = request;
  dbg.scopesLoading = true;
  dbg.scopesError = null;
  dbg.renderSig.variables = "";
  renderDebuggerVariables();
  try {
    const data = await debuggerCall("read_ui_debug_variables", { frame_id: frame.id }, { target: targetId });
    if (request !== dbg.scopesRequest || targetId !== state.targetId || dbg.frameIndex !== frameIndex) return;
    if (debuggerHasData(data)) applyDebuggerSnapshot(data);
    const scopes = debuggerScopeList(data);
    if (scopes) {
      dbg.scopes = scopes;
      dbg.scopesLoaded = true;
      dbg.scopesError = null;
    } else if (!dbg.scopesLoaded) {
      dbg.scopesError = new RpcError("The scopes response contained no scopes.", "invalid_response");
    }
    resetDebuggerVariables();
    await openDebuggerFirstScope({ quiet: options.quiet !== false });
  } catch (error) {
    if (request !== dbg.scopesRequest || targetId !== state.targetId || dbg.frameIndex !== frameIndex) return;
    dbg.scopesError = error;
    appendDebuggerConsole("error", `Scopes: ${errorMessage(error)}`);
  } finally {
    if (request === dbg.scopesRequest && targetId === state.targetId) {
      dbg.scopesLoading = false;
      dbg.renderSig.variables = "";
      renderDebuggerVariables();
      updateDebuggerButtons();
    }
  }
}

export async function loadDebuggerVariableChildren(key, reference, options = {}) {
  const dbg = state.debugger;
  if (!state.token || !state.targetId || !state.caps.runtime || reference === null || !dbg.attached) return;
  if (dbg.variableLoading.has(key)) return;
  const targetId = state.targetId;
  const epoch = dbg.variableEpoch;
  const request = (dbg.variableRequests.get(key) || 0) + 1;
  dbg.variableRequests.set(key, request);
  dbg.variableLoading.add(key);
  dbg.variableErrors.delete(key);
  dbg.renderSig.variables = "";
  renderDebuggerVariables();
  try {
    const data = await debuggerCall("read_ui_debug_variables", { variables_reference: reference, count: 100 }, { target: targetId });
    if (request !== dbg.variableRequests.get(key) || targetId !== state.targetId || epoch !== dbg.variableEpoch) return;
    const variables = debuggerVariablesFrom(data);
    if (!variables) throw new RpcError("The variables response contained no variables list.", "invalid_response");
    dbg.variableChildren.set(key, variables);
    if (debuggerRoot(data).truncated === true) dbg.variableTruncated.add(key);
    else dbg.variableTruncated.delete(key);
  } catch (error) {
    if (request !== dbg.variableRequests.get(key) || targetId !== state.targetId || epoch !== dbg.variableEpoch) return;
    dbg.variableErrors.set(key, error);
    appendDebuggerConsole("error", `Variables: ${errorMessage(error)}`);
  } finally {
    if (request === dbg.variableRequests.get(key) && targetId === state.targetId && epoch === dbg.variableEpoch) {
      dbg.variableLoading.delete(key);
      dbg.renderSig.variables = "";
      renderDebuggerVariables();
    }
  }
}

async function loadDebuggerSourceForFrame(frame, options = {}) {
  const dbg = state.debugger;
  if (!frame) return;
  const frameSource = frame.source || {};
  const frameIndex = dbg.frameIndex;
  if (!frameSource.path && frameSource.sourceReference === null) {
    dbg.source = {
      path: "",
      name: "",
      sourceReference: null,
      lines: [],
      truncated: false,
      reason: "The adapter returned no source location for this frame.",
      unavailable: false,
      language: "",
      frameId: frame.id,
    };
    dbg.sourceLoaded = true;
    dbg.sourceError = null;
    dbg.renderSig.source = "";
    renderDebuggerSource(true);
    return;
  }
  const params = frameSource.path ? { path: frameSource.path } : { source_reference: frameSource.sourceReference };
  if (frame.line !== null) params.line = frame.line;
  await loadDebuggerSource(params, {
    frameId: frame.id,
    frameIndex,
    line: frame.line,
    column: frame.column,
    quiet: options.quiet !== false,
  });
}

export async function loadDebuggerSource(params = {}, options = {}) {
  const dbg = state.debugger;
  if (!state.token || !state.targetId || !state.caps.runtime || !dbg.attached) return;
  const targetId = state.targetId;
  const request = dbg.sourceRequest + 1;
  dbg.sourceRequest = request;
  const frameIndex = options.frameIndex ?? null;
  dbg.sourceLoading = true;
  dbg.sourceError = null;
  dbg.renderSig.source = "";
  updateDebuggerSourceHead();
  renderDebuggerSource(true);
  try {
    const response = await debuggerSourceCall(params, { target: targetId });
    const data = response.data;
    if (request !== dbg.sourceRequest || targetId !== state.targetId) return;
    if (frameIndex !== null && dbg.frameIndex !== frameIndex) return;
    const root = debuggerRoot(data);
    if (root.breakpoints !== undefined || root.source?.breakpoints !== undefined) applyDebuggerSnapshot(data);
    const raw = root.source !== undefined ? root.source : data;
    const unavailable = root.available === false || root.unavailable === true || isObjectValue(root.unavailable);
    if (unavailable) {
      const info = debuggerUnavailableInfo(root) || { reason: "", code: "" };
      dbg.source = {
        path: firstStringValue(params.path, raw?.path),
        name: firstStringValue(params.path, raw?.name),
        sourceReference: params.source_reference ?? null,
        lines: [],
        truncated: false,
        reason: info.reason || "The adapter reported that this source is unavailable without a reason.",
        unavailable: true,
        language: "",
        frameId: options.frameId ?? null,
      };
    } else {
      const source = normalizeDebuggerSource(raw, {
        path: params.path,
        sourceReference: params.source_reference ?? null,
        line: options.line,
        column: options.column,
        frameId: options.frameId ?? null,
      });
      if (!source) throw new RpcError("The source response contained no source.", "invalid_response");
      if (!source.path && params.path) source.path = params.path;
      source.frameId = options.frameId ?? null;
      dbg.source = source;
    }
    dbg.sourceLoaded = true;
    dbg.sourceError = null;
  } catch (error) {
    if (request !== dbg.sourceRequest || targetId !== state.targetId) return;
    if (error?.code === "unknown_command") {
      const path = params.path || "";
      const line = options.line !== null && options.line !== undefined ? `:${options.line}` : "";
      dbg.source = {
        path,
        name: path,
        sourceReference: params.source_reference ?? null,
        lines: [],
        truncated: false,
        reason: `${errorMessage(error)}${path ? ` The selected frame is at ${path}${line}.` : ""} No source text is substituted.`,
        unavailable: false,
        language: "",
        frameId: options.frameId ?? null,
      };
      dbg.sourceLoaded = true;
      dbg.sourceError = null;
      appendDebuggerConsole("error", `Source: ${errorMessage(error)}`);
    } else {
      dbg.sourceError = error;
      appendDebuggerConsole("error", `Source: ${errorMessage(error)}`);
      if (options.quiet !== true) toast(errorMessage(error), "error", 9000);
    }
  } finally {
    if (request === dbg.sourceRequest && targetId === state.targetId) {
      dbg.sourceLoading = false;
      dbg.renderSig.source = "";
      renderDebuggerSource(true);
    }
  }
}

export function debuggerBreakpointKey(breakpoint) {
  return `${debuggerPathKey(breakpoint.path)}:${breakpoint.line ?? ""}`;
}

export function debuggerVisibleBreakpoints() {
  const dbg = state.debugger;
  const active = dbg.breakpoints.map(breakpoint => ({ ...breakpoint, enabled: true }));
  const disabled = [...dbg.disabledBreakpoints.values()].map(breakpoint => ({ ...breakpoint, enabled: false }));
  return sortDebuggerBreakpoints([...active, ...disabled]);
}

function applyDebuggerBreakpointList(data, path) {
  const dbg = state.debugger;
  const list = debuggerBreakpointList(data);
  if (!list) return false;
  const others = dbg.breakpoints.filter(breakpoint => !debuggerPathsMatch(breakpoint.path, path));
  dbg.breakpoints = sortDebuggerBreakpoints([...others, ...list]);
  for (const breakpoint of dbg.breakpoints) {
    dbg.disabledBreakpoints.delete(debuggerBreakpointKey(breakpoint));
  }
  return true;
}

export async function setDebuggerBreakpoint(path, line, enabled, options = {}) {
  const dbg = state.debugger;
  if (!debuggerCanAct(true) || !path || line === null) return;
  const targetId = state.targetId;
  const location = `${path}:${line}`;
  const visible = debuggerBreakpointsAt(path, line);
  const existing = visible[0] || null;
  const condition = firstStringValue(options.condition, existing?.condition);
  const logMessage = options.logMessage ?? existing?.logMessage;
  const key = debuggerBreakpointKey({ path, line });
  invalidateDebuggerStatusRequest();
  dbg.busy = enabled ? "Setting breakpoint" : "Clearing breakpoint";
  renderDebuggerChrome();
  try {
    let data;
    if (enabled) {
      data = await debuggerCall("set_ui_breakpoint", {
        path,
        line,
        condition: condition || undefined,
        log_message: logMessage || undefined,
      }, { target: targetId });
    } else {
      data = await debuggerCall("clear_ui_breakpoint", { path, line }, { target: targetId });
    }
    if (targetId !== state.targetId) return;
    if (!applyDebuggerBreakpointList(data, path)) {
      throw new RpcError("The controller returned no breakpoint list.", "invalid_response");
    }
    if (enabled) {
      dbg.disabledBreakpoints.delete(key);
    } else if (existing) {
      dbg.disabledBreakpoints.set(key, {
        path: existing.path || path,
        line,
        column: existing.column,
        resolvedLine: null,
        condition: existing.condition,
        logMessage: existing.logMessage,
        label: "",
        enabled: false,
        verified: null,
        id: null,
        resolvedLine: null,
        message: "",
      });
    }
    appendDebuggerConsole("event", `${enabled ? "Set" : "Cleared"} breakpoint at ${location}.`);
  } catch (error) {
    if (targetId !== state.targetId) return;
    appendDebuggerConsole("error", `Breakpoint: ${errorMessage(error)}`);
    toast(errorMessage(error), "error", 9000);
  } finally {
    if (targetId === state.targetId) {
      dbg.busy = "";
      dbg.renderSig.breakpoints = "";
      dbg.renderSig.source = "";
      renderDebuggerChrome();
      renderDebuggerBreakpoints(true);
      renderDebuggerSource(true);
    }
  }
}

export async function setDebuggerExceptionFilters(filters) {
  const dbg = state.debugger;
  if (!debuggerCanAct(true)) return;
  const targetId = state.targetId;
  dbg.busy = "Setting exception breakpoints";
  renderDebuggerBreakpoints(true);
  try {
    const result = await debuggerCall("set_ui_exception_breakpoints", { filters }, { target: targetId });
    if (dbg === state.debugger && targetId === state.targetId) dbg.exceptionFilters = result.exception_filters;
  } catch (error) {
    if (dbg === state.debugger) toast(errorMessage(error), "error", 8000);
  } finally {
    if (dbg === state.debugger) {
      dbg.busy = "";
      renderDebuggerBreakpoints(true);
    }
  }
}

export function toggleDebuggerBreakpointAtLine(path, line) {
  const existing = debuggerBreakpointsAt(path, line);
  const active = existing.find(breakpoint => breakpoint.enabled);
  const disabled = existing.find(breakpoint => !breakpoint.enabled);
  if (active) setDebuggerBreakpoint(path, line, false, { condition: active.condition });
  else setDebuggerBreakpoint(path, line, true, { condition: disabled?.condition });
}

export function toggleDebuggerBreakpoint(breakpoint, enabled) {
  if (!breakpoint.path || breakpoint.line === null) {
    toast("The adapter returned this breakpoint without a source location.", "error", 8000);
    return;
  }
  setDebuggerBreakpoint(breakpoint.path, breakpoint.line, enabled, { condition: breakpoint.condition });
}

function invalidateDebuggerStatusRequest() {
  state.debugger.request += 1;
  state.debugger.statusLoading = false;
}

export async function attachDebugger() {
  const dbg = state.debugger;
  if (!debuggerCanAct(false) || dbg.attached) return;
  const args = { timeout_ms: 8000 };
  if (dbg.requiresForward) {
    const port = $("debugger-forward-port");
    if (!port.value || !port.checkValidity()) {
      port.focus();
      toast("Enter the local port forwarded to the device debugger.", "error", 5000);
      return;
    }
    args.remote_port = port.valueAsNumber;
    const program = $("debugger-symbols").value.trim();
    if (program) args.program = program;
  }
  const targetId = state.targetId;
  invalidateDebuggerStatusRequest();
  dbg.busy = "Attaching";
  dbg.error = null;
  appendDebuggerConsole("event", "Attach requested.");
  renderDebuggerChrome();
  try {
    const data = await debuggerCall("attach_ui_debugger", args, { target: targetId, timeout: 15000 });
    if (targetId !== state.targetId) return;
    invalidateDebuggerStatusRequest();
    if (debuggerHasData(data)) applyDebuggerSnapshot(data);
    appendDebuggerConsole("event", "Attach completed.");
    if (dbg.attached) {
      await ensureDebuggerSessionData({ quiet: true });
    } else {
      await refreshDebugger({ quiet: true });
    }
  } catch (error) {
    if (targetId !== state.targetId) return;
    const message = errorMessage(error);
    dbg.phase = "";
    dbg.attached = false;
    dbg.reason = message;
    dbg.reasonCode = error.code || "";
    dbg.error = message;
    dbg.unavailable = ["unavailable", "unsupported", "not_supported", "not_available"].includes(error?.code);
    dbg.status = dbg.unavailable ? "unavailable" : "error";
    if (error?.code === "unknown_command") dbg.adapterAvailable = false;
    dbg.pid = null;
    dbg.sessionId = "";
    clearDebuggerSessionData({ keepSource: false, keepBreakpoints: false });
    appendDebuggerConsole("error", message);
    toast(message, "error", 10000);
  } finally {
    if (targetId === state.targetId) {
      dbg.busy = "";
      renderDebugger();
      syncDebuggerTimer();
    }
  }
}

export async function detachDebugger() {
  const dbg = state.debugger;
  if (!debuggerCanAct(false) || dbg.busy) return;
  if (!dbg.attached && dbg.status !== "attaching") return;
  const targetId = state.targetId;
  invalidateDebuggerStatusRequest();
  dbg.busy = "Detaching";
  renderDebuggerChrome();
  try {
    const data = await debuggerCall("detach_ui_debugger", {}, { target: targetId });
    if (targetId !== state.targetId) return;
    invalidateDebuggerStatusRequest();
    if (debuggerHasData(data)) applyDebuggerSnapshot(data);
    appendDebuggerConsole("event", "Detach completed.");
    await refreshDebugger({ quiet: true });
  } catch (error) {
    if (targetId !== state.targetId) return;
    appendDebuggerConsole("error", `Detach: ${errorMessage(error)}`);
    toast(errorMessage(error), "error", 10000);
  } finally {
    if (targetId === state.targetId) {
      dbg.busy = "";
      renderDebugger();
      syncDebuggerTimer();
    }
  }
}

export async function runDebuggerControl(action, label) {
  const dbg = state.debugger;
  if (!dbg.attached || dbg.busy) return;
  if (action !== "pause" && dbg.status !== "paused") return;
  if (action === "pause" && dbg.status === "paused") return;
  const targetId = state.targetId;
  invalidateDebuggerStatusRequest();
  dbg.busy = label;
  renderDebuggerChrome();
  try {
    const wireAction = { continue: "resume", step_into: "step_in" }[action] || action;
    const data = await debuggerCall("control_ui_debugger", { action: wireAction }, { target: targetId });
    if (targetId !== state.targetId) return;
    invalidateDebuggerStatusRequest();
    if (debuggerHasData(data)) applyDebuggerSnapshot(data);
    if (!debuggerFrameList(data)) {
      dbg.frames = [];
      dbg.stackLoaded = false;
      dbg.stackError = null;
      dbg.frameIndex = null;
      dbg.frameId = null;
      dbg.scopes = [];
      dbg.scopesLoaded = false;
      dbg.scopesError = null;
      resetDebuggerVariables();
      dbg.sourceLoaded = false;
    }
    appendDebuggerConsole("event", `${label} completed.`);
    await refreshDebugger({ quiet: true });
    if (state.debugger.status === "paused") await ensureDebuggerSessionData({ quiet: true });
  } catch (error) {
    if (targetId !== state.targetId) return;
    appendDebuggerConsole("error", `${label}: ${errorMessage(error)}`);
    toast(errorMessage(error), "error", 10000);
  } finally {
    if (targetId === state.targetId) {
      dbg.busy = "";
      renderDebugger();
      updateDebuggerButtons();
      syncDebuggerTimer();
    }
  }
}

export function debuggerCanAct(requireAttached) {
  if (!state.connected || !state.token || !state.targetId || !state.caps.runtime) return false;
  const dbg = state.debugger;
  if (requireAttached) return dbg.attached && !dbg.busy;
  return dbg.adapterAvailable !== false;
}

export function debuggerStatusLabel(status) {
  return {
    idle: "Not attached",
    unknown: "Unknown",
    unavailable: "Unavailable",
    error: "Error",
    detached: "Detached",
    attaching: "Attaching",
    attached: "Attached",
    running: "Running",
    paused: "Paused",
    terminated: "Terminated",
  }[status] || "Unknown";
}

export function debuggerStatusTone(status) {
  if (status === "paused") return "paused";
  if (status === "attached" || status === "running") return "complete";
  if (status === "attaching") return "pending";
  if (status === "unavailable" || status === "error") return "failed";
  if (status === "detached" || status === "terminated" || status === "idle") return "cancelled";
  return "";
}

