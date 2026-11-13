function updateDebuggerButtons() {
  const dbg = state.debugger;
  const targetOk = Boolean(state.connected && state.token && state.targetId && state.caps.runtime);
  const busy = Boolean(dbg.busy);
  const attaching = dbg.status === "attaching";
  const attached = dbg.attached;
  const paused = dbg.status === "paused";
  dom.debuggerAttach.disabled = !targetOk || dbg.adapterAvailable === false || attached || attaching || busy;
  dom.debuggerDetach.disabled = !targetOk || (!attached && !attaching) || busy;
  dom.debuggerPause.disabled = !targetOk || !attached || paused || busy;
  dom.debuggerContinue.disabled = !targetOk || !paused || busy;
  dom.debuggerStepOver.disabled = !targetOk || !paused || busy;
  dom.debuggerStepInto.disabled = !targetOk || !paused || busy;
  dom.debuggerStepOut.disabled = !targetOk || !paused || busy;
  dom.debuggerOpen.disabled = !targetOk || !attached || busy;
  dom.debuggerOpenPath.disabled = !targetOk || !attached || busy;
  dom.debuggerVariablesRefresh.disabled = !targetOk || !paused || dbg.frameId === null || dbg.scopesLoading || busy;
  dom.debuggerEvaluate.disabled = !targetOk || !paused || dbg.evaluateBusy || busy;
  dom.debuggerExpression.disabled = !targetOk || !paused || dbg.evaluateBusy || busy;
  dom.debuggerConsoleClear.disabled = dbg.console.length === 0;
  dom.debuggerRefresh.disabled = !targetOk || dbg.statusLoading;
  dom.debuggerAttach.textContent = dbg.status === "error" || dbg.status === "unavailable" ? "Retry attach" : "Attach";
  if (!targetOk) {
    dom.debuggerExpression.placeholder = "Select a runtime target";
  } else if (!paused) {
    dom.debuggerExpression.placeholder = "Pause the process to evaluate";
  } else {
    dom.debuggerExpression.placeholder = "Evaluate in the selected frame";
  }
}

function renderDebuggerChrome() {
  const dbg = state.debugger;
  const label = debuggerStatusLabel(dbg.status);
  dom.debuggerStatus.textContent = label;
  dom.debuggerStatus.className = `state-badge ${debuggerStatusTone(dbg.status)}`.trim();
  const stopTitle = dbg.stop?.reason ? ` · ${dbg.stop.reason}` : "";
  dom.debuggerStatus.title = `${dbg.phase ? `Adapter phase: ${dbg.phase}` : ""}${stopTitle}`.trim();
  dom.debuggerBadge.hidden = dbg.status !== "paused";
  dom.debuggerBadge.title = dbg.status === "paused" ? "Process paused" : "";

  const failed = dbg.status === "unavailable" || dbg.status === "error";
  const warning = !failed && !dbg.attached && Boolean(dbg.warning);
  const showBanner = failed || warning;
  dom.debuggerUnavailable.hidden = !showBanner;
  dom.debuggerUnavailable.classList.toggle("is-warning", warning);
  if (showBanner) {
    dom.debuggerUnavailableTitle.textContent = failed
      ? (dbg.status === "unavailable" ? "Debugger unavailable" : "Debugger error")
      : "Attach may be denied";
    const missing = dbg.status === "unavailable"
      ? "The controller reported the debugger as unavailable without a reason."
      : "The debugger request failed without a message.";
    let reason = failed ? (dbg.reason || missing) : dbg.warning;
    if (failed && dbg.reasonCode && !reason.includes(dbg.reasonCode)) reason = `${reason} (${dbg.reasonCode})`;
    if (dom.debuggerUnavailableReason.textContent !== reason) dom.debuggerUnavailableReason.textContent = reason;
    const adapter = failed ? debuggerAdapterLabel(dbg.adapter) : "";
    dom.debuggerAdapter.hidden = !adapter;
    const adapterLine = adapter ? `Adapter: ${adapter}` : "";
    if (dom.debuggerAdapter.textContent !== adapterLine) dom.debuggerAdapter.textContent = adapterLine;
    dom.debuggerAdapter.title = dbg.adapter?.version || "";
  }

  const adapter = debuggerAdapterLabel(dbg.adapter);
  dom.debuggerMeta.textContent = adapter
    ? `Adapter ${dbg.adapter?.name || adapter}. Sessions attach to one process.`
    : "Debug adapter sessions attach to one process at a time.";
  dom.debuggerMeta.title = adapter;
  const session = [];
  if (dbg.attached) {
    if (dbg.pid !== null) session.push(`PID ${dbg.pid}`);
    if (dbg.adapter?.name) session.push(dbg.adapter.name);
    if (dbg.sessionId) session.push(`session ${dbg.sessionId}`);
  }
  dom.debuggerSession.textContent = dbg.attached
    ? session.length ? session.join(" · ") : "Attached session"
    : "No session";
  dom.debuggerSession.title = dom.debuggerSession.textContent;
  dom.debuggerBusy.hidden = !dbg.busy;
  dom.debuggerBusyText.textContent = dbg.busy || "Working";
  updateDebuggerButtons();
}

function debuggerContextMessage(what) {
  if (!state.token) return { text: "Connect to a controller." };
  const target = targetById();
  if (!target) return { text: "Select a target." };
  if (!state.caps.runtime) return { text: "Select a Cove runtime; debugger sessions attach to a process." };
  const dbg = state.debugger;
  if (dbg.status === "unavailable") {
    return {
      text: dbg.adapterAvailable === false
        ? "The debugger is unavailable for this target. The reason is shown above."
        : "The last attach was refused. The adapter's reason is shown above.",
    };
  }
  if (dbg.status === "error") return { text: "The debugger reported an error. See the message above." };
  if (dbg.status === "attaching") return { text: "Attaching…", loading: true };
  if (!dbg.attached) return { text: `Attach to read ${what}.` };
  return null;
}

function appendDebuggerPaneMessage(host, message, options = {}) {
  const node = el("div", { class: `dbg-empty ${options.error ? "is-error" : ""}`.trim() });
  if (options.loading) {
    node.append(el("span", { class: "spinner", attrs: { role: "progressbar", "aria-label": options.loadingLabel || "Loading" } }), " ", message);
  } else {
    node.append(message);
  }
  host.append(node);
  if (options.retry) {
    host.append(el("div", { class: "dbg-empty" },
      el("button", {
        class: "text-button",
        props: { type: "button" },
        text: options.retryLabel || "Retry",
        on: { click: options.retry },
      })));
  }
}

function renderDebuggerThreads() {
  const dbg = state.debugger;
  const signature = JSON.stringify({
    threads: dbg.threads,
    threadId: dbg.threadId,
    status: dbg.status,
    loading: dbg.stackLoading,
    error: dbg.threadsError ? errorMessage(dbg.threadsError) : "",
  });
  if (signature === dbg.renderSig.threads) {
    updateDebuggerButtons();
    return;
  }
  dbg.renderSig.threads = signature;
  const host = dom.debuggerThreads;
  clear(host);
  dom.debuggerThreadsMeta.textContent = dbg.threads.length ? String(dbg.threads.length) : "";
  const context = debuggerContextMessage("threads");
  if (context && !dbg.threads.length) {
    appendDebuggerPaneMessage(host, context.text, { loading: context.loading, retry: (dbg.status === "detached" || dbg.status === "unknown") && debuggerCanAct(false) ? () => attachDebugger() : null, retryLabel: "Attach" });
    return;
  }
  if (dbg.stackLoading && dbg.status === "paused" && !dbg.threads.length) {
    appendDebuggerPaneMessage(host, "Reading threads…", { loading: true, loadingLabel: "Reading threads" });
    return;
  }
  if (dbg.threadsError && !dbg.threads.length) {
    appendDebuggerPaneMessage(host, errorMessage(dbg.threadsError), { error: true, retry: dbg.attached ? () => loadDebuggerStack(dbg.threadId) : null, retryLabel: "Retry" });
    return;
  }
  if (!dbg.threads.length) {
    host.append(el("div", { class: "dbg-empty", text: dbg.status === "paused" ? "No threads were reported." : "Pause the process to read threads." }));
    return;
  }
  for (const thread of dbg.threads) {
    const selected = thread.id !== null && thread.id === dbg.threadId;
    const row = el("button", {
      class: `dbg-row ${selected ? "is-selected" : ""} ${thread.current ? "is-current" : ""}`.trim(),
      attrs: {
        type: "button",
        "aria-pressed": String(selected),
        title: thread.id === null ? "The adapter returned no thread ID." : thread.name,
      },
      props: { disabled: thread.id === null },
    },
    el("span", { class: "dbg-row-mark", attrs: { "aria-hidden": "true" } }),
    el("span", { class: "dbg-row-main" },
      el("span", { class: "dbg-row-name", text: thread.name }),
      thread.state ? el("span", { class: "dbg-row-meta", text: thread.state }) : null));
    row.addEventListener("click", () => selectDebuggerThread(thread));
    host.append(row);
  }
}

function renderDebuggerStack() {
  const dbg = state.debugger;
  const signature = JSON.stringify({
    frames: dbg.frames,
    frameIndex: dbg.frameIndex,
    status: dbg.status,
    loading: dbg.stackLoading,
    error: dbg.stackError ? errorMessage(dbg.stackError) : "",
    threadId: dbg.threadId,
    stop: dbg.stop,
    total: dbg.framesTotal,
    truncated: dbg.framesTruncated,
  });
  if (signature === dbg.renderSig.stack) {
    updateDebuggerButtons();
    return;
  }
  dbg.renderSig.stack = signature;
  const host = dom.debuggerStack;
  clear(host);
  const stackMeta = [];
  if (dbg.frames.length) stackMeta.push(`${dbg.frames.length} frame${dbg.frames.length === 1 ? "" : "s"}`);
  if (dbg.framesTruncated) stackMeta.push(dbg.framesTotal ? `of ${dbg.framesTotal}` : "truncated");
  if (dbg.stop?.reason) stackMeta.push(dbg.stop.reason);
  dom.debuggerStackMeta.textContent = stackMeta.join(" · ");
  dom.debuggerStackMeta.title = dbg.stop?.description || dbg.stop?.text || "";
  if (dbg.status === "paused" && dbg.threadId === null && dbg.threads.length > 1) {
    appendDebuggerPaneMessage(host, "Select a thread.");
    return;
  }
  const context = debuggerContextMessage("the call stack");
  if (context && !dbg.frames.length) {
    appendDebuggerPaneMessage(host, context.text, { loading: context.loading });
    return;
  }
  if (dbg.status !== "paused" && !dbg.frames.length) {
    appendDebuggerPaneMessage(host, "Pause the process to read the call stack.");
    return;
  }
  if (dbg.stackLoading && !dbg.frames.length) {
    appendDebuggerPaneMessage(host, "Reading the call stack…", { loading: true, loadingLabel: "Reading the call stack" });
    return;
  }
  if (dbg.stackError && !dbg.frames.length) {
    appendDebuggerPaneMessage(host, errorMessage(dbg.stackError), { error: true, retry: dbg.attached ? () => loadDebuggerStack(dbg.threadId) : null, retryLabel: "Retry" });
    return;
  }
  if (!dbg.frames.length) {
    appendDebuggerPaneMessage(host, "The adapter returned no frames.");
    return;
  }
  dbg.frames.forEach((frame, index) => {
    const selected = index === dbg.frameIndex;
    const location = debuggerFrameLocation(frame);
    const row = el("button", {
      class: `dbg-row ${selected ? "is-selected is-current" : ""}`.trim(),
      attrs: {
        type: "button",
        "aria-pressed": String(selected),
        title: frame.source?.path ? `${frame.source.path}${frame.line !== null ? `:${frame.line}` : ""}` : debuggerFrameLabel(frame),
      },
    },
    el("span", { class: "dbg-row-mark", attrs: { "aria-hidden": "true" } }),
    el("span", { class: "dbg-row-main" },
      el("span", { class: "dbg-row-name", text: debuggerFrameLabel(frame) }),
      location || frame.module ? el("span", { class: "dbg-row-meta", text: [location, frame.module].filter(Boolean).join(" · ") }) : null));
    row.addEventListener("click", () => selectDebuggerFrame(frame, index, { quiet: true }));
    host.append(row);
  });
  if (dbg.framesTruncated) {
    host.append(el("div", { class: "dbg-empty", text: dbg.framesTotal ? `The adapter returned ${dbg.frames.length} of ${dbg.framesTotal} frames.` : "The adapter truncated this stack." }));
  }
}

function renderDebuggerBreakpoints(force = false) {
  const dbg = state.debugger;
  const visible = debuggerVisibleBreakpoints();
  const hitIds = new Set(dbg.stop?.hitBreakpointIds || []);
  const disabledCount = visible.filter(breakpoint => !breakpoint.enabled).length;
  const signature = JSON.stringify({
    breakpoints: visible,
    status: dbg.status,
    attached: dbg.attached,
    busy: dbg.busy,
    hit: [...hitIds],
  });
  if (!force && signature === dbg.renderSig.breakpoints) return;
  dbg.renderSig.breakpoints = signature;
  const host = dom.debuggerBreakpoints;
  clear(host);
  dom.debuggerBreakpointsMeta.textContent = visible.length
    ? `${visible.length}${disabledCount ? ` · ${disabledCount} disabled` : ""}`
    : "";
  const context = debuggerContextMessage("breakpoints");
  if (context && !visible.length) {
    appendDebuggerPaneMessage(host, context.text, { loading: context.loading });
    return;
  }
  if (!visible.length) {
    host.append(el("div", { class: "dbg-empty", text: dbg.status === "paused" ? "No breakpoints. Select a source line gutter to set one." : "No breakpoints." }));
    return;
  }
  for (const breakpoint of visible) {
    const label = debuggerBreakpointLabel(breakpoint);
    const canToggle = dbg.attached && !dbg.busy && Boolean(breakpoint.path) && breakpoint.line !== null;
    const hit = breakpoint.id !== null && hitIds.has(breakpoint.id);
    const meta = [];
    if (!breakpoint.enabled) meta.push("disabled");
    if (breakpoint.verified === false) meta.push("unverified");
    if (breakpoint.message) meta.push(breakpoint.message);
    if (breakpoint.resolvedLine !== null && breakpoint.resolvedLine !== undefined && breakpoint.resolvedLine !== breakpoint.line) {
      meta.push(`resolved ${breakpoint.resolvedLine}`);
    }
    if (breakpoint.condition) meta.push(`if ${breakpoint.condition}`);
    if (hit) meta.push("hit");
    const row = el("label", {
      class: `dbg-bp-row ${breakpoint.enabled ? "" : "is-disabled"} ${hit ? "is-hit" : ""}`.trim(),
      attrs: breakpoint.enabled ? null : { title: "Disabled breakpoints are cleared in lldb-dap and restored when enabled." },
    },
    el("input", {
      attrs: {
        type: "checkbox",
        "aria-label": `${breakpoint.enabled ? "Disable" : "Enable"} breakpoint at ${label}`,
      },
      props: { checked: breakpoint.enabled, disabled: !canToggle },
    }),
    el("span", { class: "dbg-bp-main" },
      el("span", { class: "dbg-bp-label", text: label }),
      meta.length ? el("span", { class: "dbg-bp-meta", text: meta.join(" · ") }) : null));
    const input = row.querySelector("input");
    input.addEventListener("change", () => toggleDebuggerBreakpoint(breakpoint, input.checked));
    host.append(row);
  }
}

function debuggerSourceSignature() {
  const dbg = state.debugger;
  const frame = selectedDebuggerFrame();
  return JSON.stringify({
    source: dbg.source,
    status: dbg.status,
    frameIndex: dbg.frameIndex,
    frameId: dbg.frameId,
    line: frame?.line ?? null,
    column: frame?.column ?? null,
    framePath: frame?.source?.path || "",
    loading: dbg.sourceLoading,
    error: dbg.sourceError ? errorMessage(dbg.sourceError) : "",
    breakpoints: debuggerVisibleBreakpoints(),
    attached: dbg.attached,
    busy: dbg.busy,
  });
}

function updateDebuggerSourceHead() {
  const dbg = state.debugger;
  const source = dbg.source;
  const current = debuggerCurrentLocation();
  const path = source?.path || source?.name || "No file";
  dom.debuggerSourcePath.textContent = path;
  dom.debuggerSourcePath.title = path;
  const meta = [];
  if (source?.lines?.length) meta.push(`${source.lines.length} lines`);
  if (source?.truncated) meta.push("truncated");
  if (source?.language) meta.push(source.language);
  if (current) meta.push(`${current.line}${current.column !== null ? `:${current.column}` : ""}`);
  if (dbg.sourceLoading) meta.push("loading");
  if (dbg.sourceError) meta.push("error");
  dom.debuggerSourceMeta.textContent = meta.join(" · ");
}

function renderDebuggerSource(force = false) {
  const dbg = state.debugger;
  const signature = debuggerSourceSignature();
  if (!force && signature === dbg.renderSig.source) {
    updateDebuggerSourceHead();
    return;
  }
  dbg.renderSig.source = signature;
  const host = dom.debuggerSource;
  const active = document.activeElement;
  const focusedLine = active && host.contains(active) && active.classList?.contains("dbg-gutter") ? active.dataset.line : null;
  const scrollTop = host.scrollTop;
  const scrollLeft = host.scrollLeft;
  clear(host);
  updateDebuggerSourceHead();
  const source = dbg.source;
  if (dbg.sourceError) host.append(el("div", { class: "dbg-empty is-error", text: errorMessage(dbg.sourceError) }));
  if (!source) {
    const context = debuggerContextMessage("source");
    showState(host, {
      compact: true,
      icon: "logs",
      title: dbg.attached ? "No source selected" : "Source",
      message: context?.text || "Select a frame or open a source path.",
    });
    return;
  }
  if (!source.lines.length) {
    const reason = source.reason
      || (source.unavailable ? "The adapter reported that this source is unavailable without a reason." : "")
      || (dbg.sourceLoading ? "Waiting for the adapter's source response." : "The adapter returned no source text for this location.");
    showState(host, {
      compact: true,
      kind: source.unavailable ? "error" : dbg.sourceLoading ? "loading" : "empty",
      icon: "logs",
      title: source.unavailable ? "Source unavailable" : "Source text unavailable",
      message: reason,
    });
    return;
  }
  if (source.truncated) {
    host.append(el("div", { class: "dbg-empty", text: `Showing the first ${MAX_DEBUGGER_SOURCE_LINES} lines returned by the adapter.` }));
  }
  const current = debuggerCurrentLocation();
  const canToggle = dbg.attached && !dbg.busy && Boolean(source.path);
  let currentRow = null;
  for (const line of source.lines) {
    const breakpoints = debuggerBreakpointsAt(source.path, line.number);
    const enabledBreakpoint = breakpoints.some(breakpoint => breakpoint.enabled);
    const isCurrent = current !== null && current.line === line.number;
    const row = el("div", {
      class: `dbg-line ${isCurrent ? "is-current" : ""} ${breakpoints.length ? "has-breakpoint" : ""}`.trim(),
      attrs: isCurrent ? { "aria-current": "location" } : null,
      dataset: { line: String(line.number) },
    });
    const action = breakpoints.length
      ? enabledBreakpoint ? "Disable" : "Enable"
      : "Set";
    const gutter = el("button", {
      class: "dbg-gutter",
      attrs: {
        type: "button",
        "aria-pressed": String(enabledBreakpoint),
        "aria-label": `${action} breakpoint at line ${line.number}`,
        title: `${action} breakpoint at line ${line.number}`,
        "data-line": String(line.number),
      },
      props: { disabled: !canToggle },
    });
    gutter.addEventListener("click", () => toggleDebuggerBreakpointAtLine(source.path, line.number));
    row.append(
      gutter,
      el("span", { class: "dbg-line-number", text: String(line.number) }),
      el("span", { class: "dbg-line-code", text: line.text }),
    );
    if (isCurrent) currentRow = row;
    host.append(row);
  }
  if (currentRow) {
    host.scrollTop = Math.max(0, currentRow.offsetTop - host.offsetTop - host.clientHeight / 2);
  } else {
    host.scrollTop = scrollTop;
  }
  host.scrollLeft = scrollLeft;
  if (focusedLine) {
    host.querySelector(`.dbg-gutter[data-line="${CSS.escape(focusedLine)}"]`)?.focus();
  }
}

function debuggerScopeKey(index) {
  return `scope:${index}`;
}

function debuggerVariableKey(parentKey, index) {
  return `${parentKey}/${index}`;
}

function debuggerVariablesSignature() {
  const dbg = state.debugger;
  return JSON.stringify({
    status: dbg.status,
    frameIndex: dbg.frameIndex,
    frameId: dbg.frameId,
    scopes: dbg.scopes,
    loaded: dbg.scopesLoaded,
    loading: dbg.scopesLoading,
    error: dbg.scopesError ? errorMessage(dbg.scopesError) : "",
    expandedScopes: [...dbg.expandedScopes],
    expandedVariables: [...dbg.expandedVariables],
    variableLoading: [...dbg.variableLoading],
    variableErrors: [...dbg.variableErrors].map(([key, error]) => [key, errorMessage(error)]),
    children: [...dbg.variableChildren],
  });
}

function renderDebuggerVariables(force = false) {
  const dbg = state.debugger;
  const signature = debuggerVariablesSignature();
  if (!force && signature === dbg.renderSig.variables) {
    updateDebuggerButtons();
    return;
  }
  dbg.renderSig.variables = signature;
  const host = dom.debuggerVariables;
  clear(host);
  dom.debuggerScopesMeta.textContent = dbg.scopes.length
    ? `${dbg.scopes.length}${dbg.scopesTruncated ? " · truncated" : ""}`
    : "";
  if (dbg.scopesError) host.append(el("div", { class: "dbg-empty is-error", text: errorMessage(dbg.scopesError) }));
  if (dbg.status !== "paused") {
    const context = debuggerContextMessage("scopes");
    appendDebuggerPaneMessage(host, context?.text || "Pause the process to read scopes.");
    return;
  }
  if (dbg.frameIndex === null) {
    appendDebuggerPaneMessage(host, "Select a frame to read scopes.");
    return;
  }
  if (dbg.frameId === null) {
    appendDebuggerPaneMessage(host, "The selected frame has no ID; scopes cannot be requested.");
    return;
  }
  if (dbg.scopesLoading && !dbg.scopes.length) {
    appendDebuggerPaneMessage(host, "Reading scopes…", { loading: true, loadingLabel: "Reading scopes" });
    return;
  }
  if (!dbg.scopes.length) {
    if (dbg.scopesLoaded) appendDebuggerPaneMessage(host, "The adapter returned no scopes for this frame.");
    else appendDebuggerPaneMessage(host, "No scopes loaded.", { loading: false });
    return;
  }
  dbg.scopes.forEach((scope, index) => {
    const key = debuggerScopeKey(index);
    const expanded = dbg.expandedScopes.has(key);
    const canExpand = scope.variables.length > 0 || scope.reference !== null;
    const counts = [];
    if (scope.namedVariables !== null) counts.push(`${scope.namedVariables} named`);
    if (scope.indexedVariables !== null) counts.push(`${scope.indexedVariables} indexed`);
    if (scope.expensive) counts.push("expensive");
    const row = el("div", { class: "dbg-var-row is-scope" },
      canExpand
        ? el("button", {
          class: "dbg-var-twisty",
          attrs: {
            type: "button",
            "aria-expanded": String(expanded),
            "aria-label": `${expanded ? "Collapse" : "Expand"} scope ${scope.name}`,
          },
          on: { click: () => toggleDebuggerScope(index) },
        }, icon("chevron"))
        : el("span", { class: "dbg-var-twisty empty", attrs: { "aria-hidden": "true" } }, icon("chevron")),
      el("span", { class: "dbg-var-body" },
        el("span", { class: "dbg-var-name", text: scope.name }),
        counts.length ? el("span", { class: "dbg-var-type", text: counts.join(" · ") }) : null));
    host.append(row);
    if (expanded) {
      const children = scope.variables.length ? scope.variables : dbg.variableChildren.get(key) || [];
      const box = el("div", { class: "dbg-var-children" });
      if (children.length) {
        renderDebuggerVariableRows(box, children, key, index);
      } else if (dbg.variableLoading.has(key)) {
        box.append(el("div", { class: "dbg-var-loading", text: "Reading variables…" }));
      } else if (dbg.variableErrors.has(key)) {
        box.append(el("div", { class: "dbg-empty is-error", text: errorMessage(dbg.variableErrors.get(key)) }));
      } else if (scope.reference === null) {
        box.append(el("div", { class: "dbg-var-loading", text: "No variables were returned for this scope." }));
      } else {
        box.append(el("div", { class: "dbg-var-loading", text: "No variables returned." }));
      }
      if (dbg.variableTruncated.has(key) || (scope.truncated && children.length)) {
        box.append(el("div", { class: "dbg-var-loading", text: "The adapter returned more variables than this page." }));
      }
      host.append(box);
    }
  });
}

function renderDebuggerVariableRows(host, variables, parentKey, scopeIndex) {
  const dbg = state.debugger;
  variables.forEach((variable, index) => {
    const key = debuggerVariableKey(parentKey, index);
    const expanded = dbg.expandedVariables.has(key);
    const children = variable.children.length ? variable.children : dbg.variableChildren.get(key) || [];
    const canExpand = children.length > 0 || variable.reference !== null;
    const meta = [];
    if (variable.type) meta.push(variable.type);
    if (variable.namedVariables !== null) meta.push(`${variable.namedVariables} named`);
    if (variable.indexedVariables !== null) meta.push(`${variable.indexedVariables} indexed`);
    const row = el("div", { class: "dbg-var-row" },
      canExpand
        ? el("button", {
          class: "dbg-var-twisty",
          attrs: {
            type: "button",
            "aria-expanded": String(expanded),
            "aria-label": `${expanded ? "Collapse" : "Expand"} ${variable.name}`,
          },
          on: { click: () => toggleDebuggerVariable(key, variable, scopeIndex) },
        }, icon("chevron"))
        : el("span", { class: "dbg-var-twisty empty", attrs: { "aria-hidden": "true" } }, icon("chevron")),
      el("span", { class: "dbg-var-body" },
        el("span", { class: "dbg-var-name", text: variable.name }),
        el("span", {
          class: "dbg-var-value",
          text: variable.value !== ""
            ? `${variable.value}${variable.valueTruncated ? "…" : ""}`
            : variable.reference !== null
              ? "…"
              : "—",
        }),
        meta.length ? el("span", { class: "dbg-var-type", text: meta.join(" · ") }) : null));
    host.append(row);
    if (expanded) {
      const box = el("div", { class: "dbg-var-children" });
      if (children.length) {
        renderDebuggerVariableRows(box, children, key, scopeIndex);
      } else if (dbg.variableLoading.has(key)) {
        box.append(el("div", { class: "dbg-var-loading", text: "Reading variables…" }));
      } else if (dbg.variableErrors.has(key)) {
        box.append(el("div", { class: "dbg-empty is-error", text: errorMessage(dbg.variableErrors.get(key)) }));
      } else if (variable.reference === null) {
        box.append(el("div", { class: "dbg-var-loading", text: "No variables returned." }));
      } else {
        box.append(el("div", { class: "dbg-var-loading", text: "No variables returned." }));
      }
      if (dbg.variableTruncated.has(key)) {
        box.append(el("div", { class: "dbg-var-loading", text: "The adapter returned more variables than this page." }));
      }
      host.append(box);
    }
  });
}

function toggleDebuggerScope(index) {
  const dbg = state.debugger;
  const key = debuggerScopeKey(index);
  if (dbg.expandedScopes.has(key)) {
    dbg.expandedScopes.delete(key);
  } else {
    dbg.expandedScopes.add(key);
    const scope = dbg.scopes[index];
    if (scope && !scope.variables.length && scope.reference !== null && !dbg.variableChildren.has(key)) {
      loadDebuggerVariableChildren(key, scope.reference, { quiet: true });
    }
  }
  dbg.renderSig.variables = "";
  renderDebuggerVariables();
}

function toggleDebuggerVariable(key, variable, scopeIndex) {
  const dbg = state.debugger;
  if (dbg.expandedVariables.has(key)) {
    dbg.expandedVariables.delete(key);
  } else {
    dbg.expandedVariables.add(key);
    if (!variable.children.length && variable.reference !== null && !dbg.variableChildren.has(key)) {
      loadDebuggerVariableChildren(key, variable.reference, { quiet: true });
    }
  }
  dbg.renderSig.variables = "";
  renderDebuggerVariables();
}

function debuggerConsoleEntryElement(entry) {
  return el("div", { class: `dbg-console-entry is-${entry.kind}` },
    el("span", { class: "dbg-console-kind", text: { command: "Eval", result: "Result", error: "Error", event: "Event" }[entry.kind] || "Info" }),
    el("span", { class: "dbg-console-text" },
      entry.text,
      entry.type ? el("span", { class: "dbg-console-type", text: ` ${entry.type}` }) : null));
}

function appendDebuggerConsole(kind, text, options = {}) {
  const dbg = state.debugger;
  const entry = { kind, text: String(text ?? ""), type: options.type || "" };
  dbg.console.push(entry);
  if (dbg.console.length > DEBUGGER_CONSOLE_LIMIT) dbg.console.splice(0, dbg.console.length - DEBUGGER_CONSOLE_LIMIT);
  const host = dom.debuggerConsole;
  host.querySelector(".dbg-empty")?.remove();
  const nearBottom = host.scrollHeight - host.scrollTop - host.clientHeight < 24;
  host.append(debuggerConsoleEntryElement(entry));
  while (host.children.length > DEBUGGER_CONSOLE_LIMIT) host.firstElementChild.remove();
  if (nearBottom) host.scrollTop = host.scrollHeight;
  dom.debuggerConsoleMeta.textContent = dbg.console.length ? `${dbg.console.length} entries` : "";
  updateDebuggerButtons();
}

function renderDebuggerConsole() {
  const dbg = state.debugger;
  const host = dom.debuggerConsole;
  clear(host);
  if (!dbg.console.length) {
    host.append(el("div", { class: "dbg-empty", text: "Evaluate an expression while the process is paused." }));
  } else {
    for (const entry of dbg.console) host.append(debuggerConsoleEntryElement(entry));
    host.scrollTop = host.scrollHeight;
  }
  dom.debuggerConsoleMeta.textContent = dbg.console.length ? `${dbg.console.length} entries` : "";
  updateDebuggerButtons();
}

function clearDebuggerConsole() {
  state.debugger.console = [];
  renderDebuggerConsole();
}

function debuggerEvaluation(data) {
  if (data === null || data === undefined) return { text: "(no value returned)", type: "" };
  if (!isObjectValue(data) && !Array.isArray(data)) return { text: debuggerScalarText(data), type: "" };
  const root = isObjectValue(data.result) ? data.result : data;
  if (isObjectValue(root)) {
    const value = root.value !== undefined
      ? root.value
      : root.result !== undefined
        ? root.result
        : root.display !== undefined
          ? root.display
          : root.text;
    const truncated = root.result_truncated === true || data.result_truncated === true;
    let type = firstStringValue(root.type, root.value_type, data.type);
    if (truncated) type = type ? `${type} · truncated` : "truncated";
    if (value !== undefined) {
      const text = typeof value === "string" ? (value === "" ? '""' : value) : debuggerScalarText(value);
      if (text !== "") return { text, type };
    }
  }
  return { text: debuggerScalarText(data), type: "" };
}

async function evaluateDebuggerExpression(event) {
  event.preventDefault();
  const dbg = state.debugger;
  if (!dbg.attached || dbg.status !== "paused" || dbg.evaluateBusy) return;
  const expression = dom.debuggerExpression.value.trim();
  if (!expression) return;
  const targetId = state.targetId;
  const frameId = dbg.frameId;
  const request = dbg.evaluateRequest + 1;
  dbg.evaluateRequest = request;
  dbg.evaluateBusy = true;
  appendDebuggerConsole("command", expression);
  dom.debuggerExpression.value = "";
  updateDebuggerButtons();
  try {
    const params = { expression, context: "repl" };
    if (frameId !== null) params.frame_id = frameId;
    const data = await debuggerCall("evaluate_ui_debug", params, { target: targetId });
    if (request !== dbg.evaluateRequest || targetId !== state.targetId) return;
    if (debuggerStatusIsExplicit(data)) {
      applyDebuggerSnapshot(data);
      renderDebugger();
    }
    const result = debuggerEvaluation(data);
    appendDebuggerConsole("result", result.text, { type: result.type });
  } catch (error) {
    if (request !== dbg.evaluateRequest || targetId !== state.targetId) return;
    appendDebuggerConsole("error", errorMessage(error));
  } finally {
    if (request === dbg.evaluateRequest && targetId === state.targetId) {
      dbg.evaluateBusy = false;
      updateDebuggerButtons();
      renderDebuggerChrome();
      dom.debuggerExpression.focus();
    }
  }
}

function openDebuggerSource(event) {
  event.preventDefault();
  const path = dom.debuggerOpenPath.value.trim();
  if (!path) return;
  loadDebuggerSource({ path }, { quiet: false });
}

function renderDebugger() {
  renderDebuggerChrome();
  renderDebuggerThreads();
  renderDebuggerStack();
  renderDebuggerBreakpoints();
  renderDebuggerSource();
  renderDebuggerVariables();
}

function resetDebugger(previousTarget = state.targetId) {
  const previous = state.debugger;
  stopTimer("debugger");
  if (previous?.attached && previousTarget && state.token) {
    debuggerCall("detach_ui_debugger", {}, { target: previousTarget }).catch(() => {});
  }
  const request = (previous?.request || 0) + 1;
  state.debugger = emptyDebugger();
  state.debugger.request = request;
  if (state.view === "debugger") {
    renderDebugger();
    renderDebuggerConsole();
  }
}

function debuggerUnloadDetach() {
  const dbg = state.debugger;
  if (!state.token || !state.targetId || !dbg.attached) return;
  try {
    fetch("/rpc", {
      method: "POST",
      keepalive: true,
      headers: { ...authHeaders(), "Content-Type": "application/json" },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: ++rpcSequence,
        method: "get_ui_debugger",
        params: { target: state.targetId, action: "detach" },
      }),
    }).catch(() => {});
  } catch {}
}

