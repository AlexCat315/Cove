/* Debugger
 *
 * A debugger session is controller-level and keyed by process. Status comes from
 * get_ui_debugger, controls from attach_ui_debugger, detach_ui_debugger and
 * control_ui_debugger, stopped state from get_ui_debug_stack, variables from
 * read_ui_debug_variables, expression results from evaluate_ui_debug and
 * breakpoints from set_ui_breakpoint and clear_ui_breakpoint. The controller
 * has no source-text tool yet; the source pane asks for a debugger source tool
 * and shows the captured reason instead of substituting lines when none exists.
 * Every paused state, frame, scope, variable, breakpoint and source line shown
 * here comes from a response.
 */
import { basename } from "../../core/format.js";
import { $, MAX_DEBUGGER_SOURCE_LINES, state } from "../../core/globals.js";
import { debuggerBreakpointKey, debuggerVisibleBreakpoints } from "./session.js";
import { firstStringValue, isObjectValue } from "../performance/profile/model.js";


export function emptyDebugger() {
  return {
    loaded: false,
    status: "idle",
    phase: "",
    reason: "",
    reasonCode: "",
    unavailable: false,
    adapterAvailable: true,
    warning: "",
    developerMode: null,
    error: null,
    adapter: null,
    pid: null,
    sessionId: "",
    attached: false,
    stop: null,
    statusLoading: false,
    request: 0,
    lastRefresh: 0,
    threads: [],
    threadsError: null,
    threadId: null,
    frames: [],
    stackLoaded: false,
    stackLoading: false,
    stackError: null,
    frameIndex: null,
    frameId: null,
    framesTotal: null,
    framesTruncated: false,
    stackRequest: 0,
    scopes: [],
    scopesLoaded: false,
    scopesLoading: false,
    scopesError: null,
    scopesTruncated: false,
    scopesRequest: 0,
    variableChildren: new Map(),
    variableLoading: new Set(),
    variableErrors: new Map(),
    variableRequests: new Map(),
    variableTruncated: new Set(),
    variableEpoch: 0,
    expandedScopes: new Set(),
    expandedVariables: new Set(),
    source: null,
    sourceLoaded: false,
    sourceLoading: false,
    sourceError: null,
    sourceRequest: 0,
    breakpoints: [],
    capabilities: null,
    requiresForward: false,
    exceptionFilters: [],
    outputAfter: 0,
    disabledBreakpoints: new Map(),
    busy: "",
    evaluateBusy: false,
    evaluateRequest: 0,
    console: [],
    timer: null,
    renderSig: {},
  };
}

export function debuggerRoot(data) {
  if (!isObjectValue(data)) return {};
  return isObjectValue(data.debugger) ? data.debugger : data;
}

function debuggerSessionObject(root) {
  return isObjectValue(root.session) ? root.session : {};
}

function debuggerText(value) {
  if (value === null || value === undefined) return "";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return "";
}

function debuggerId(value) {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && value.trim()) {
    const text = value.trim();
    return /^-?\d+$/.test(text) ? Number(text) : text;
  }
  return null;
}

function debuggerVariablesReference(value) {
  const id = debuggerId(value);
  return id === 0 || id === "0" ? null : id;
}

export function debuggerNumber(value) {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && /^-?\d+(\.\d+)?$/.test(value.trim())) return Number(value);
  return null;
}

function debuggerArray(...values) {
  for (const value of values) {
    if (Array.isArray(value)) return value;
  }
  return null;
}

export function debuggerPathKey(value) {
  let path = String(value ?? "").trim();
  if (!path) return "";
  path = path.replace(/^file:\/\//i, "");
  try { path = decodeURIComponent(path); } catch {}
  path = path.replace(/\\/g, "/").replace(/\/+$/, "");
  return path;
}

export function debuggerPathsMatch(left, right) {
  const a = debuggerPathKey(left);
  const b = debuggerPathKey(right);
  if (!a || !b) return false;
  const fold = /^[a-z]:\//i.test(a) || /^[a-z]:\//i.test(b) || String(state.platform).toLowerCase().includes("win");
  const x = fold ? a.toLowerCase() : a;
  const y = fold ? b.toLowerCase() : b;
  return x === y || x.endsWith(`/${y}`) || y.endsWith(`/${x}`);
}

function debuggerRawStatus(root) {
  const session = debuggerSessionObject(root);
  return firstStringValue(root.status, root.state, root.phase, session.status, session.state, session.phase)
    .trim()
    .toLowerCase()
    .replace(/[\s-]+/g, "_");
}

export function debuggerUnavailableInfo(root) {
  const session = debuggerSessionObject(root);
  const raw = debuggerRawStatus(root);
  const marker = root.unavailable ?? root.debugger_unavailable ?? session.unavailable;
  const flagged = root.available === false
    || marker === true
    || (isObjectValue(marker) && marker.available !== true);
  const named = ["unavailable", "unsupported", "not_supported", "not_available", "denied", "disabled"].includes(raw);
  if (!flagged && !named) return null;
  const markerReason = typeof marker === "string"
    ? marker
    : isObjectValue(marker)
      ? firstStringValue(marker.reason, marker.message, marker.detail)
      : "";
  const error = root.error;
  const reason = firstStringValue(
    markerReason,
    root.reason,
    root.detail,
    typeof root.message === "string" && root.message !== root.status ? root.message : "",
    isObjectValue(error) ? firstStringValue(error.reason, error.message, error.detail) : debuggerText(error),
    session.reason,
    session.detail,
    root.warning,
  );
  const code = firstStringValue(
    root.reason_code,
    root.error_code,
    isObjectValue(error) ? error.code : "",
    isObjectValue(marker) ? marker.code : "",
    session.reason_code,
  );
  return { reason, code };
}

function debuggerStatusErrorInfo(root) {
  const session = debuggerSessionObject(root);
  const error = root.error;
  const reason = firstStringValue(
    isObjectValue(error) ? firstStringValue(error.message, error.reason, error.detail) : debuggerText(error),
    root.error_message,
    root.message,
    root.reason,
    session.error,
    session.message,
    isObjectValue(session.last_error) ? firstStringValue(session.last_error.message, session.last_error.reason) : "",
    isObjectValue(root.last_error) ? firstStringValue(root.last_error.message, root.last_error.reason) : "",
  );
  const code = firstStringValue(
    root.error_code,
    root.reason_code,
    isObjectValue(error) ? error.code : "",
    session.error_code,
    isObjectValue(session.last_error) ? session.last_error.code : "",
    isObjectValue(root.last_error) ? root.last_error.code : "",
  );
  return { reason, code };
}

function normalizeDebuggerStatus(data) {
  const root = debuggerRoot(data);
  const session = debuggerSessionObject(root);
  const raw = debuggerRawStatus(root);
  const unavailable = debuggerUnavailableInfo(root);
  if (unavailable) return { status: "unavailable", phase: raw, reason: unavailable.reason, code: unavailable.code };
  if (["failed", "error"].includes(raw)) {
    const info = debuggerStatusErrorInfo(root);
    const code = info.code;
    const refused = ["unavailable", "not_allowed", "attach_denied", "permission"].includes(code);
    return {
      status: refused ? "unavailable" : "error",
      phase: raw,
      reason: info.reason,
      code,
    };
  }
  const sessionPresent = Object.prototype.hasOwnProperty.call(root, "session") && isObjectValue(root.session);
  const sessionAbsent = Object.prototype.hasOwnProperty.call(root, "session") && root.session === null;
  const paused = root.paused === true
    || session.paused === true
    || ["paused", "suspended", "break", "stopped", "stopped_on_breakpoint"].includes(raw);
  const running = root.running === true
    || session.running === true
    || ["running", "resumed", "continued"].includes(raw);
  const attaching = root.attaching === true
    || ["attaching", "initializing", "launching", "starting", "connecting", "detaching"].includes(raw);
  const attachedFlag = root.attached === true
    || session.attached === true
    || ["attached", "running", "stopped"].includes(raw);
  const detachedFlag = root.detached === true
    || session.detached === true
    || root.attached === false
    || session.attached === false;
  const failed = ["error", "failed"].includes(raw) || (root.error !== undefined && root.error !== null);
  let status = "unknown";
  if (attaching) status = "attaching";
  else if (paused) status = "paused";
  else if (running) status = "running";
  else if (detachedFlag) status = "detached";
  else if (attachedFlag) status = "attached";
  else if (["detached", "not_attached", "not-attached", "disconnected", "inactive", "idle", "none"].includes(raw)) status = "detached";
  else if (["terminated", "exited", "ended", "gone", "closed"].includes(raw)) status = "terminated";
  else if (sessionAbsent && !attachedFlag) status = "detached";
  else if (sessionPresent && !raw && !attachedFlag) status = "detached";
  else if (failed) status = "error";
  if (status === "error") {
    const info = debuggerStatusErrorInfo(root);
    return { status, phase: raw, reason: info.reason, code: info.code };
  }
  return { status, phase: raw, reason: "", code: "" };
}

function normalizeDebuggerStop(raw) {
  if (!isObjectValue(raw)) return null;
  const hits = Array.isArray(raw.hit_breakpoint_ids)
    ? raw.hit_breakpoint_ids.map(debuggerId).filter(id => id !== null)
    : Array.isArray(raw.hitBreakpointIds)
      ? raw.hitBreakpointIds.map(debuggerId).filter(id => id !== null)
      : [];
  return {
    reason: firstStringValue(raw.reason),
    description: firstStringValue(raw.description),
    text: firstStringValue(raw.text),
    threadId: debuggerId(raw.thread_id ?? raw.threadId),
    allThreadsStopped: raw.all_threads_stopped === true || raw.allThreadsStopped === true,
    hitBreakpointIds: hits,
  };
}

export function debuggerStatusIsExplicit(data) {
  const root = debuggerRoot(data);
  return ["status", "state", "phase", "session", "available", "unavailable", "attached", "detached", "paused", "running", "attaching", "terminated", "sessions", "warning", "developer_mode", "last_error"].some(
    key => Object.prototype.hasOwnProperty.call(root, key),
  );
}

export function debuggerHasData(data) {
  const root = debuggerRoot(data);
  return ["status", "state", "phase", "session", "available", "unavailable", "attached", "detached", "paused", "running", "attaching", "terminated", "sessions", "warning", "developer_mode", "last_error", "pid", "adapter", "debug_adapter", "dap", "threads", "frames", "stack", "call_stack", "scopes", "source", "breakpoints", "thread_id", "frame_id", "stop", "total_frames", "truncated", "threads_error", "scopes_error"].some(
    key => Object.prototype.hasOwnProperty.call(root, key),
  );
}

function debuggerAdapterFrom(data, fallback = null) {
  const root = debuggerRoot(data);
  const session = debuggerSessionObject(root);
  const raw = root.adapter ?? root.debug_adapter ?? root.dap ?? session.adapter ?? session.debug_adapter;
  const name = firstStringValue(root.adapter_name, session.adapter_name);
  const version = firstStringValue(root.adapter_version, session.adapter_version);
  if (raw === undefined || raw === null) {
    if (!name && !version) return fallback;
    return { name: name || fallback?.name || "", version: version || fallback?.version || "", path: fallback?.path || "", protocol: fallback?.protocol || "" };
  }
  if (typeof raw === "string") return { name: raw, version: version || fallback?.version || "", path: fallback?.path || "", protocol: fallback?.protocol || "" };
  if (!isObjectValue(raw)) return fallback;
  const path = firstStringValue(raw.path, raw.executable, raw.binary, raw.command);
  const adapter = {
    name: firstStringValue(raw.name, raw.id, raw.adapter, name) || (path ? basename(path) : ""),
    version: firstStringValue(raw.version, raw.build, raw.llvm_version, version),
    path,
    protocol: firstStringValue(raw.protocol, raw.transport, raw.kind) || "dap",
  };
  if (!adapter.name && !adapter.version && !adapter.path && !adapter.protocol) return fallback;
  return adapter;
}

export function debuggerAdapterLabel(adapter) {
  if (!adapter) return "";
  const clean = value => String(value || "").replace(/\s+/g, " ").trim();
  const name = clean(adapter.name);
  const version = clean(adapter.version);
  const path = clean(adapter.path);
  const llvm = /LLVM version ([0-9.]+)/i.exec(version);
  const lldb = /liblldb:\s*(lldb-[0-9.]+)/i.exec(version);
  const shortVersion = llvm
    ? [`LLVM ${llvm[1]}`, lldb?.[1]].filter(Boolean).join(" · ")
    : version;
  const parts = [name || path, shortVersion];
  const label = parts.filter(Boolean).join(" · ");
  if (path && path !== name) return `${label} · ${path}`;
  return label;
}

function debuggerFirstNumber(...values) {
  for (const value of values) {
    const number = debuggerNumber(value);
    if (number !== null) return number;
  }
  return null;
}

function debuggerPidFrom(data, fallback) {
  const root = debuggerRoot(data);
  const session = debuggerSessionObject(root);
  const pid = debuggerFirstNumber(root.pid, root.process_id, session.pid, session.process_id, root.process?.pid);
  return pid ?? fallback;
}

function debuggerSessionIdFrom(data, fallback) {
  const root = debuggerRoot(data);
  const session = debuggerSessionObject(root);
  return firstStringValue(root.session_id, root.sessionId, session.id, session.session_id, fallback);
}

function normalizeDebuggerThread(raw, index) {
  if (!isObjectValue(raw)) return null;
  const id = debuggerId(raw.id ?? raw.thread_id ?? raw.threadId);
  const name = firstStringValue(raw.name, raw.label, raw.title) || (id !== null ? `Thread ${id}` : `Thread ${index + 1}`);
  return {
    id,
    name,
    state: firstStringValue(raw.state, raw.status),
    current: raw.current === true || raw.selected === true || raw.stopped === true,
  };
}

function normalizeDebuggerThreads(raw) {
  if (!Array.isArray(raw)) return null;
  return raw.map(normalizeDebuggerThread).filter(Boolean);
}

function normalizeDebuggerSourceRef(raw, fallback = {}) {
  if (typeof raw === "string") return { path: raw, name: raw, sourceReference: fallback.sourceReference ?? null };
  if (!isObjectValue(raw)) return fallback;
  return {
    path: firstStringValue(raw.path, raw.file, raw.file_path, fallback.path),
    name: firstStringValue(raw.name, raw.path, raw.file, fallback.name),
    sourceReference: debuggerId(raw.sourceReference ?? raw.source_reference ?? raw.reference) ?? fallback.sourceReference ?? null,
  };
}

export function normalizeDebuggerSource(raw, fallback = {}) {
  if (raw === null || raw === undefined) return null;
  if (typeof raw === "string") {
    return {
      path: raw,
      name: raw,
      sourceReference: fallback.sourceReference ?? null,
      lines: [],
      reason: "",
      unavailable: false,
      language: "",
      truncated: false,
      frameId: fallback.frameId ?? null,
    };
  }
  if (!isObjectValue(raw)) return null;
  const nested = isObjectValue(raw.source) ? raw.source : null;
  const path = firstStringValue(raw.path, raw.file, raw.file_path, nested?.path, nested?.file, fallback.path);
  const sourceReference = debuggerId(raw.sourceReference ?? raw.source_reference ?? raw.reference ?? nested?.sourceReference ?? nested?.source_reference) ?? fallback.sourceReference ?? null;
  const content = [
    typeof raw.content === "string" ? raw.content : "",
    typeof raw.text === "string" ? raw.text : "",
    typeof raw.code === "string" ? raw.code : "",
    typeof raw.source === "string" ? raw.source : "",
    typeof nested?.content === "string" ? nested.content : "",
    typeof nested?.text === "string" ? nested.text : "",
  ].find(value => value !== "") ?? "";
  const rawLines = debuggerArray(raw.lines, raw.code_lines, nested?.lines, Array.isArray(raw.content) ? raw.content : null);
  const sourceLines = rawLines ?? (content ? content.split(/\r?\n/) : []);
  const startLine = debuggerNumber(raw.start_line ?? raw.startLine ?? raw.line_start) ?? 1;
  const lines = sourceLines.map((line, index) => {
    if (isObjectValue(line)) {
      const number = debuggerNumber(line.number ?? line.line ?? line.line_number) ?? startLine + index;
      const text = typeof line.text === "string"
        ? line.text
        : typeof line.content === "string"
          ? line.content
          : typeof line.code === "string"
            ? line.code
            : "";
      return { number, text };
    }
    return { number: startLine + index, text: line === null || line === undefined ? "" : String(line) };
  });
  const truncated = lines.length > MAX_DEBUGGER_SOURCE_LINES;
  const currentLine = debuggerNumber(raw.currentLine ?? raw.current_line ?? raw.line ?? raw.lineNumber) ?? fallback.line ?? null;
  const currentColumn = debuggerNumber(raw.currentColumn ?? raw.current_column ?? raw.column ?? raw.columnNumber) ?? fallback.column ?? null;
  const unavailableMarker = raw.unavailable ?? raw.available;
  const reason = firstStringValue(
    raw.reason,
    raw.detail,
    isObjectValue(raw.unavailable) ? firstStringValue(raw.unavailable.reason, raw.unavailable.message) : "",
    unavailableMarker === false ? raw.message : "",
  );
  return {
    path,
    name: firstStringValue(raw.name, path),
    sourceReference,
    lines: truncated ? lines.slice(0, MAX_DEBUGGER_SOURCE_LINES) : lines,
    truncated,
    reason,
    unavailable: raw.available === false || raw.unavailable === true || isObjectValue(raw.unavailable),
    language: firstStringValue(raw.language, raw.language_id, nested?.language),
    frameId: fallback.frameId ?? null,
    currentLine,
    currentColumn,
  };
}

function normalizeDebuggerFrame(raw, index) {
  if (!isObjectValue(raw)) return null;
  const source = normalizeDebuggerSourceRef(
    isObjectValue(raw.source) ? raw.source : raw.location,
    { path: firstStringValue(raw.path, raw.file), name: firstStringValue(raw.file, raw.path) },
  );
  const line = debuggerNumber(raw.line ?? raw.lineNumber ?? raw.location?.line);
  const column = debuggerNumber(raw.column ?? raw.columnNumber ?? raw.location?.column);
  const id = debuggerId(raw.id ?? raw.frame_id ?? raw.frameId);
  const name = firstStringValue(raw.name, raw.function, raw.function_name, raw.method, raw.label);
  return {
    id,
    name,
    index,
    source,
    line,
    column,
    module: firstStringValue(raw.module, raw.module_id, raw.moduleId),
  };
}

function normalizeDebuggerFrames(raw) {
  if (!Array.isArray(raw)) return null;
  return raw.map(normalizeDebuggerFrame).filter(Boolean);
}

export function debuggerScalarText(value) {
  if (value === null) return "null";
  if (value === undefined) return "";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean" || typeof value === "bigint") return String(value);
  try {
    return JSON.stringify(value);
  } catch {
    return "";
  }
}

function normalizeDebuggerVariable(raw, index) {
  if (!isObjectValue(raw)) return null;
  const name = firstStringValue(raw.name, raw.key, raw.label) || `[${index}]`;
  const reference = debuggerVariablesReference(raw.variablesReference ?? raw.variables_reference ?? raw.variables_ref);
  const childrenRaw = debuggerArray(raw.variables, raw.children, isObjectValue(raw.value) ? raw.value.variables : null);
  const children = childrenRaw ? normalizeDebuggerVariables(childrenRaw) : [];
  return {
    name,
    value: raw.value === undefined ? "" : debuggerScalarText(raw.value),
    valueTruncated: raw.value_truncated === true,
    type: firstStringValue(raw.type, raw.value_type, raw.class),
    reference,
    children,
    namedVariables: debuggerNumber(raw.namedVariables ?? raw.named_variables),
    indexedVariables: debuggerNumber(raw.indexedVariables ?? raw.indexed_variables),
    evaluateName: firstStringValue(raw.evaluateName, raw.evaluate_name),
  };
}

function normalizeDebuggerVariables(raw) {
  if (!Array.isArray(raw)) return [];
  return raw.map(normalizeDebuggerVariable).filter(Boolean);
}

function normalizeDebuggerScope(raw, index) {
  if (!isObjectValue(raw)) return null;
  return {
    name: firstStringValue(raw.name, raw.label) || `Scope ${index + 1}`,
    reference: debuggerVariablesReference(raw.variablesReference ?? raw.variables_reference ?? raw.variables_ref),
    expensive: raw.expensive === true,
    variables: normalizeDebuggerVariables(debuggerArray(raw.variables, raw.children) ?? []),
    truncated: raw.truncated === true,
    namedVariables: debuggerNumber(raw.namedVariables ?? raw.named_variables),
    indexedVariables: debuggerNumber(raw.indexedVariables ?? raw.indexed_variables),
  };
}

function normalizeDebuggerScopes(raw) {
  if (!Array.isArray(raw)) return null;
  return raw.map(normalizeDebuggerScope).filter(Boolean);
}

function normalizeDebuggerBreakpoint(raw, index, fallbackPath = "") {
  if (!isObjectValue(raw)) return null;
  const location = isObjectValue(raw.location) ? raw.location : {};
  const source = isObjectValue(raw.source) ? raw.source : {};
  const path = firstStringValue(raw.path, raw.file, raw.file_path, raw.source_path, source.path, source.file, location.path, fallbackPath);
  const line = debuggerNumber(raw.line ?? raw.line_number ?? location.line ?? location.line_number);
  const column = debuggerNumber(raw.column ?? raw.column_number ?? location.column);
  const id = debuggerId(raw.id ?? raw.breakpoint_id ?? raw.breakpointId);
  return {
    id,
    path,
    line,
    column,
    resolvedLine: debuggerNumber(raw.resolved_line ?? raw.resolvedLine),
    label: firstStringValue(raw.label),
    message: firstStringValue(raw.message),
    enabled: raw.enabled === false || raw.disabled === true ? false : true,
    verified: raw.verified === true ? true : raw.verified === false ? false : null,
    condition: firstStringValue(raw.condition),
    hitCondition: firstStringValue(raw.hitCondition, raw.hit_condition),
    logMessage: firstStringValue(raw.logMessage, raw.log_message),
  };
}

function normalizeDebuggerBreakpoints(raw, fallbackPath = "") {
  if (!Array.isArray(raw)) return null;
  return raw.map((breakpoint, index) => normalizeDebuggerBreakpoint(breakpoint, index, fallbackPath)).filter(Boolean);
}

export function debuggerBreakpointList(data) {
  const root = debuggerRoot(data);
  const session = debuggerSessionObject(root);
  const source = isObjectValue(root.source) ? root.source : isObjectValue(root.selected_source) ? root.selected_source : null;
  return normalizeDebuggerBreakpoints(
    debuggerArray(root.breakpoints, root.breakpoint_list, session.breakpoints, source?.breakpoints),
    firstStringValue(source?.path),
  );
}

export function sortDebuggerBreakpoints(breakpoints) {
  return [...breakpoints].sort((left, right) => {
    const path = debuggerPathKey(left.path).localeCompare(debuggerPathKey(right.path));
    if (path !== 0) return path;
    return (left.line ?? 0) - (right.line ?? 0);
  });
}

export function debuggerBreakpointsAt(path, line) {
  return debuggerVisibleBreakpoints().filter(breakpoint => breakpoint.line === line && debuggerPathsMatch(breakpoint.path, path));
}

export function debuggerBreakpointLabel(breakpoint) {
  const path = breakpoint.path || "unknown source";
  return breakpoint.line === null ? path : `${path}:${breakpoint.line}`;
}

export function debuggerThreadList(data) {
  const root = debuggerRoot(data);
  const session = debuggerSessionObject(root);
  return normalizeDebuggerThreads(debuggerArray(root.threads, root.thread_list, session.threads));
}

export function debuggerFrameList(data) {
  const root = debuggerRoot(data);
  const session = debuggerSessionObject(root);
  return normalizeDebuggerFrames(debuggerArray(root.frames, root.stack, root.call_stack, root.callStack, session.frames, session.stack));
}

export function debuggerScopeList(data) {
  const root = debuggerRoot(data);
  const session = debuggerSessionObject(root);
  const frame = isObjectValue(root.frame) ? root.frame : null;
  return normalizeDebuggerScopes(debuggerArray(root.scopes, root.scope_list, session.scopes, frame?.scopes));
}

export function debuggerVariablesFrom(data) {
  if (Array.isArray(data)) return normalizeDebuggerVariables(data);
  const root = debuggerRoot(data);
  const result = isObjectValue(root.result) ? root.result : null;
  const raw = debuggerArray(root.variables, root.children, result?.variables, root.variable?.variables);
  return raw ? normalizeDebuggerVariables(raw) : null;
}

export function selectedDebuggerFrame() {
  const dbg = state.debugger;
  if (dbg.frameIndex === null) return null;
  return dbg.frames[dbg.frameIndex] || null;
}

export function debuggerCurrentLocation() {
  const dbg = state.debugger;
  if (dbg.status !== "paused") return null;
  const frame = selectedDebuggerFrame();
  if (!frame || frame.line === null) return null;
  if (!dbg.source) return null;
  const sourcePath = dbg.source.path;
  const framePath = frame.source?.path;
  const sameFrame = dbg.source.frameId !== null && frame.id !== null && dbg.source.frameId === frame.id
    && (!sourcePath || !framePath || debuggerPathsMatch(sourcePath, framePath));
  if (!sameFrame && !(sourcePath && framePath && debuggerPathsMatch(sourcePath, framePath))) return null;
  return { path: sourcePath || framePath, line: frame.line, column: frame.column };
}

export function debuggerFrameLabel(frame) {
  return frame.name || `Frame ${frame.index + 1}`;
}

export function debuggerFrameLocation(frame) {
  const path = frame.source?.path || frame.source?.name || "";
  const file = path ? basename(path) : "";
  const line = frame.line !== null ? String(frame.line) : "";
  return [file, line].filter(Boolean).join(":");
}

export function resetDebuggerVariables() {
  const dbg = state.debugger;
  dbg.variableEpoch += 1;
  dbg.variableChildren.clear();
  dbg.variableLoading.clear();
  dbg.variableErrors.clear();
  dbg.variableRequests.clear();
  dbg.variableTruncated.clear();
  dbg.expandedScopes.clear();
  dbg.expandedVariables.clear();
  dbg.renderSig.variables = "";
}

export function clearDebuggerSessionData(options = {}) {
  const dbg = state.debugger;
  dbg.threads = [];
  dbg.threadsError = null;
  dbg.threadId = null;
  dbg.frames = [];
  dbg.stackLoaded = false;
  dbg.stackError = null;
  dbg.frameIndex = null;
  dbg.frameId = null;
  dbg.framesTotal = null;
  dbg.framesTruncated = false;
  dbg.stop = null;
  dbg.scopes = [];
  dbg.scopesLoaded = false;
  dbg.scopesError = null;
  dbg.scopesTruncated = false;
  resetDebuggerVariables();
  if (!options.keepSource) {
    dbg.source = null;
    dbg.sourceLoaded = false;
    dbg.sourceError = null;
  }
  if (!options.keepBreakpoints) {
    dbg.breakpoints = [];
    dbg.capabilities = null;
    dbg.exceptionFilters = [];
    dbg.outputAfter = 0;
  }
  dbg.renderSig = {};
}

export function applyDebuggerSnapshot(data) {
  const dbg = state.debugger;
  const root = debuggerRoot(data);
  const session = debuggerSessionObject(root);
  const explicit = debuggerStatusIsExplicit(data);
  const previousStatus = dbg.status;
  let nextStatus = previousStatus;
  let nextPhase = dbg.phase;
  let nextReason = dbg.reason;
  let nextCode = dbg.reasonCode;
  let nextUnavailable = dbg.unavailable;
  let nextError = dbg.error;

  if (explicit) {
    const status = normalizeDebuggerStatus(data);
    nextStatus = status.status;
    nextPhase = status.phase || "";
    nextReason = status.reason || "";
    nextCode = status.code || "";
    nextUnavailable = status.status === "unavailable";
    nextError = status.status === "error" ? (status.reason || "The debugger reported an error without a message.") : null;
  }

  if (root.available !== undefined) dbg.adapterAvailable = root.available !== false;
  if (root.requires_forward !== undefined) dbg.requiresForward = root.requires_forward === true;
  if (session.capabilities !== undefined) dbg.capabilities = session.capabilities;
  if (Array.isArray(session.exception_filters)) dbg.exceptionFilters = session.exception_filters;
  if (root.warning !== undefined) dbg.warning = firstStringValue(root.warning);
  if (root.developer_mode !== undefined) {
    dbg.developerMode = root.developer_mode === true ? true : root.developer_mode === false ? false : null;
  }
  const adapter = debuggerAdapterFrom(data, dbg.adapter);
  if (adapter) dbg.adapter = adapter;
  const pid = debuggerPidFrom(data, dbg.pid);
  if (pid !== null) dbg.pid = pid;
  const sessionId = debuggerSessionIdFrom(data, dbg.sessionId);
  if (sessionId) dbg.sessionId = sessionId;
  dbg.loaded = true;

  const threads = debuggerThreadList(data);
  const frames = debuggerFrameList(data);
  const scopes = debuggerScopeList(data);
  const breakpoints = debuggerBreakpointList(data);
  const stop = normalizeDebuggerStop(root.stop !== undefined ? root.stop : session.stop);
  const sourceRaw = root.source !== undefined ? root.source : root.selected_source !== undefined ? root.selected_source : undefined;
  const ended = ["detached", "terminated", "unavailable", "error"].includes(nextStatus);
  const paused = nextStatus === "paused";

  if (explicit) {
    dbg.status = nextStatus;
    dbg.phase = nextPhase;
    dbg.reason = nextReason;
    dbg.reasonCode = nextCode;
    dbg.unavailable = nextUnavailable;
    dbg.error = nextError;
    dbg.attached = ["attached", "running", "paused"].includes(nextStatus);
    dbg.stop = paused ? (stop || dbg.stop) : null;
    if (ended) {
      clearDebuggerSessionData({ keepSource: nextStatus !== "unavailable" && nextStatus !== "error", keepBreakpoints: false });
      if (["unavailable", "error", "detached", "terminated"].includes(nextStatus) && !isObjectValue(root.session)) {
        dbg.pid = null;
        dbg.sessionId = "";
      }
    } else if (!paused) {
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
      dbg.threads = [];
      dbg.threadsError = null;
      dbg.threadId = null;
      resetDebuggerVariables();
      dbg.sourceLoaded = false;
    } else if (previousStatus !== "paused") {
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
      dbg.sourceLoaded = false;
    }
  }

  if (threads) {
    dbg.threads = threads;
    dbg.threadsError = null;
    const currentThread = debuggerId(root.thread_id ?? root.current_thread_id ?? root.threadId ?? session.thread_id ?? session.current_thread_id);
    if (currentThread !== null) dbg.threadId = currentThread;
    if (dbg.threadId !== null && !threads.some(thread => thread.id === dbg.threadId)) dbg.threadId = null;
    if (dbg.threadId === null) {
      const current = threads.find(thread => thread.current);
      if (current) dbg.threadId = current.id;
      else if (threads.length === 1) dbg.threadId = threads[0].id;
    }
  }
  const stopThread = debuggerId(root.thread_id ?? stop?.threadId);
  if (stopThread !== null) dbg.threadId = stopThread;

  if (frames) {
    dbg.frames = frames;
    dbg.stackLoaded = true;
    dbg.stackError = null;
    dbg.framesTotal = debuggerNumber(root.total_frames) ?? dbg.framesTotal;
    dbg.framesTruncated = root.truncated === true;
    const currentFrame = debuggerId(root.frame_id ?? root.current_frame_id ?? root.frameId ?? session.frame_id ?? session.current_frame_id);
    let index = currentFrame !== null ? frames.findIndex(frame => frame.id === currentFrame) : -1;
    if (index < 0 && frames.length) index = 0;
    dbg.frameIndex = index >= 0 ? index : null;
    dbg.frameId = index >= 0 ? frames[index].id : null;
  }

  if (scopes) {
    const signature = scopes.map(scope => `${scope.name}:${scope.reference ?? ""}`).join("|");
    const previousSignature = dbg.scopes.map(scope => `${scope.name}:${scope.reference ?? ""}`).join("|");
    if (signature !== previousSignature) resetDebuggerVariables();
    dbg.scopes = scopes;
    dbg.scopesLoaded = true;
    dbg.scopesError = null;
    dbg.scopesTruncated = root.scopes_truncated === true;
  }

  if (sourceRaw !== undefined) {
    if (sourceRaw === null) {
      dbg.source = null;
      dbg.sourceLoaded = true;
      dbg.sourceError = null;
    } else {
      const source = normalizeDebuggerSource(sourceRaw);
      if (source) {
        dbg.source = source;
        dbg.sourceLoaded = true;
        dbg.sourceError = null;
      }
    }
  }

  if (breakpoints) {
    dbg.breakpoints = sortDebuggerBreakpoints(breakpoints);
    for (const breakpoint of dbg.breakpoints) {
      dbg.disabledBreakpoints.delete(debuggerBreakpointKey(breakpoint));
    }
  }
}

