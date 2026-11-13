#!/usr/bin/env node
// Fake-controller harness for the Debugger client. It serves the real
// index.html/inspector.js/inspector.css, answers /rpc with the controller's
// debugger response shapes, and drives the page through Chrome DevTools
// Protocol. No Rust process is started and no real process is attached.
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { access, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
import { serveClientAsset } from "./harness-server.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const clientDir = resolve(here, "..");
const chromePath = process.env.CHROME_PATH
  || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

const adapter = {
  path: "/Library/Developer/CommandLineTools/usr/bin/lldb-dap",
  version: "lldb-dap version 21.0.0 (LLVM 21.0.0)",
};

const warning = "Developer mode is disabled, so macOS can deny task_for_pid attach. Enable it with `sudo DevToolsSecurity -enable` or run a target signed with get-task-allow.";

function sessionSummary(flow, state = flow.state) {
  return {
    pid: 4242,
    target: "runtime-1",
    state,
    stop: state === "stopped" ? flow.stop() : null,
    last_error: null,
    adapter_version: "lldb-dap version 21.0.0",
    breakpoints: flow.breakpoints,
    events_dropped: 0,
    since_ms: 1234,
  };
}

function rpcError(id, code, message) {
  return {
    jsonrpc: "2.0",
    id,
    error: { code: -32000, message, data: { code } },
  };
}

function createFake(mode) {
  const calls = [];
  const flow = {
    state: "detached",
    stepLine: 12,
    breakpoints: mode === "flow" || mode === "nosource"
      ? [{ path: "/src/main.rs", line: 12, resolved_line: 12, condition: null, id: 1, verified: true, message: null }]
      : [],
    stop() {
      return {
        reason: this.state === "stopped" ? "breakpoint" : "pause",
        description: "breakpoint 1",
        thread_id: 1,
        all_threads_stopped: true,
        text: null,
        hit_breakpoint_ids: this.state === "stopped" ? [1] : [],
      };
    },
  };

  function failedSummary() {
    return {
      pid: 4242,
      target: "runtime-1",
      state: "failed",
      stop: null,
      last_error: {
        code: "unavailable",
        message: "Not allowed to attach to process. Developer mode is disabled, so macOS can deny task_for_pid attach. Enable it with `sudo DevToolsSecurity -enable` or sign the target with get-task-allow. The target has no DWARF or dSYM for this frame.",
      },
      adapter_version: "lldb-dap version 21.0.0",
      breakpoints: [],
      events_dropped: 0,
      since_ms: 900,
    };
  }

  function statusPayload() {
    const failed = mode === "failedstatus";
    const summary = failed ? failedSummary() : sessionSummary(flow);
    return {
      adapter: mode === "unavailable" ? null : adapter,
      developer_mode: mode === "unavailable" ? true : false,
      platform: "macos",
      reason: mode === "unavailable"
        ? "lldb-dap was not found. Install LLVM or the Xcode Command Line Tools, or set COVE_LLDB_DAP."
        : null,
      available: mode !== "unavailable",
      warning: mode === "unavailable" ? null : warning,
      sessions: failed ? [summary] : flow.state === "detached" ? [] : [summary],
      session: failed ? summary : flow.state === "detached" ? null : summary,
    };
  }

  function stackPayload() {
    const line = flow.stepLine;
    return {
      target: "runtime-1",
      pid: 4242,
      state: "stopped",
      stop: flow.stop(),
      thread_id: 1,
      threads: [{ id: 1, name: "main" }],
      threads_error: null,
      frames: [
        { id: 10, name: "main", source: { path: "/src/main.rs", name: "main.rs", source_reference: null }, line, column: 5, instruction_pointer: "0x1000" },
        { id: 11, name: "helper", source: { path: "/src/main.rs", name: "main.rs", source_reference: null }, line: 5, column: 3, instruction_pointer: "0x2000" },
      ],
      total_frames: 2,
      start_frame: 0,
      truncated: false,
      scopes: [{ name: "Locals", variables_reference: 100, expensive: false, presentation_hint: null, source: null, line: null }],
      scopes_error: null,
    };
  }

  function variablesFor(reference) {
    if (reference === 100) {
      return {
        pid: 4242,
        state: "stopped",
        variables_reference: 100,
        start: 0,
        count: 100,
        total: null,
        truncated: false,
        variables: [
          { name: "count", value: "3", value_truncated: false, type: "i32", variables_reference: 0, evaluate_name: "count", named_variables: null, indexed_variables: null, memory_reference: null },
          { name: "widget", value: "Widget { width: 800.0 }", value_truncated: false, type: "Widget", variables_reference: 101, evaluate_name: "widget", named_variables: 1, indexed_variables: null, memory_reference: null },
        ],
      };
    }
    if (reference === 101) {
      return {
        pid: 4242,
        state: "stopped",
        variables_reference: 101,
        start: 0,
        count: 100,
        total: null,
        truncated: false,
        variables: [
          { name: "width", value: "800.0", value_truncated: false, type: "f64", variables_reference: 0, evaluate_name: "widget.width", named_variables: null, indexed_variables: null, memory_reference: null },
        ],
      };
    }
    if (reference === 200) {
      return {
        pid: 4242,
        state: "stopped",
        variables_reference: 200,
        start: 0,
        count: 100,
        total: null,
        truncated: false,
        variables: [
          { name: "helper_arg", value: "7", value_truncated: false, type: "u32", variables_reference: 0, evaluate_name: "helper_arg", named_variables: null, indexed_variables: null, memory_reference: null },
        ],
      };
    }
    return {
      pid: 4242,
      state: "stopped",
      variables_reference: reference,
      start: 0,
      count: 100,
      total: null,
      truncated: false,
      variables: [],
    };
  }

  function scopesForFrame(frameId) {
    if (frameId === 11) {
      return {
        pid: 4242,
        state: "stopped",
        frame_id: 11,
        scopes_truncated: false,
        scopes: [
          {
            name: "Arguments",
            variables_reference: 200,
            expensive: false,
            presentation_hint: null,
            source: null,
            line: null,
            variables: [
              { name: "helper_arg", value: "7", value_truncated: false, type: "u32", variables_reference: 0, evaluate_name: "helper_arg", named_variables: null, indexed_variables: null, memory_reference: null },
            ],
            total: null,
            truncated: false,
          },
        ],
      };
    }
    return {
      pid: 4242,
      state: "stopped",
      frame_id: 10,
      scopes_truncated: false,
      scopes: [
        {
          name: "Locals",
          variables_reference: 100,
          expensive: false,
          presentation_hint: null,
          source: null,
          line: null,
          variables: variablesFor(100).variables,
          total: null,
          truncated: false,
        },
      ],
    };
  }

  function sourcePayload() {
    return {
      path: "/src/main.rs",
      name: "main.rs",
      source_reference: null,
      language: "rust",
      lines: [
        { number: 10, text: "fn main() {" },
        { number: 11, text: "    let count = 3;" },
        { number: 12, text: "    run(count);" },
        { number: 13, text: "    finish();" },
        { number: 14, text: "}" },
        { number: 4, text: "fn helper() {" },
        { number: 5, text: "    helper_body();" },
        { number: 6, text: "}" },
      ],
    };
  }

  function control(state, stopReason = null) {
    flow.state = state;
    const stop = stopReason ? {
      reason: stopReason,
      description: null,
      thread_id: 1,
      all_threads_stopped: true,
      text: null,
      hit_breakpoint_ids: [],
    } : null;
    return {
      pid: 4242,
      state,
      stop,
      session: {
        state,
        stop,
        pid: 4242,
        process_name: "gallery",
        adapter_version: "lldb-dap version 21.0.0",
        exit_code: null,
        events_dropped: 0,
        breakpoints: flow.breakpoints,
      },
    };
  }

  function setBreakpoints(path, line) {
    const existing = flow.breakpoints.find(breakpoint => breakpoint.line === line && breakpoint.path === path);
    if (!existing) {
      flow.breakpoints = [...flow.breakpoints, {
        path,
        line,
        resolved_line: line,
        condition: null,
        id: 1 + flow.breakpoints.length,
        verified: true,
        message: null,
      }].sort((left, right) => left.line - right.line);
    }
    return {
      pid: 4242,
      path,
      line,
      condition: null,
      verified: true,
      breakpoints: flow.breakpoints.filter(breakpoint => breakpoint.path === path),
    };
  }

  function clearBreakpoint(path, line) {
    flow.breakpoints = flow.breakpoints.filter(breakpoint => !(breakpoint.path === path && breakpoint.line === line));
    return {
      pid: 4242,
      path,
      line,
      breakpoints: flow.breakpoints.filter(breakpoint => breakpoint.path === path),
    };
  }

  function handle(method, params) {
    calls.push({ method, params });
    if (method === "get_ui_session") {
      return {
        platform: "macos",
        permissions: { screen_capture: true, accessibility: true },
        targets: [
          {
            id: "runtime-1",
            title: "Debug Target",
            platform: "macos",
            pid: 4242,
            kind: "runtime",
            capabilities: [],
            platform_target: "window-1",
          },
          {
            id: "window-1",
            title: "Gallery",
            platform: "macos",
            kind: "window",
            capabilities: [],
          },
        ],
      };
    }
    if (method === "get_ui_debugger") return statusPayload();
    if (method === "attach_ui_debugger") {
      if (mode === "refused") throw rpcError(-1, "unavailable", "Not allowed to attach to process. Developer mode is disabled, so macOS can deny task_for_pid attach. Enable it with `sudo DevToolsSecurity -enable` or sign the target with get-task-allow.");
      if (mode === "timeout") throw rpcError(-1, "timeout", "lldb-dap did not finish attaching to PID 4242 within 8000 ms. An authorization prompt may be waiting. Developer mode is disabled, so macOS can deny task_for_pid attach.");
      flow.state = "stopped";
      return { ...statusPayload(), attached: true, pid: 4242 };
    }
    if (method === "detach_ui_debugger") {
      flow.state = "detached";
      return { pid: 4242, detached: true, outcome: "detached" };
    }
    if (method === "get_ui_debug_stack") return stackPayload();
    if (method === "read_ui_debug_variables") {
      if (params.variables_reference !== undefined) return variablesFor(params.variables_reference);
      return scopesForFrame(params.frame_id ?? 10);
    }
    if (method === "get_ui_debug_source" || method === "read_ui_debug_source") {
      if (mode === "nosource") throw rpcError(-1, "unknown_command", `Unknown command ${method}.`);
      return sourcePayload();
    }
    if (method === "evaluate_ui_debug") {
      return {
        pid: 4242,
        state: "stopped",
        expression: params.expression,
        result: "4",
        result_truncated: false,
        type: "i32",
        variables_reference: 0,
        named_variables: null,
        indexed_variables: null,
        memory_reference: null,
      };
    }
    if (method === "control_ui_debugger") {
      if (params.action === "resume") return control("running");
      if (params.action === "pause") {
        flow.stepLine = 12;
        return control("stopped", "pause");
      }
      if (params.action === "step_over") {
        flow.stepLine = 13;
        return control("stopped", "step");
      }
      if (params.action === "step_in" || params.action === "step_out") {
        flow.stepLine = 13;
        return control("stopped", params.action);
      }
    }
    if (method === "set_ui_breakpoint") return setBreakpoints(params.path, params.line);
    if (method === "clear_ui_breakpoint") return clearBreakpoint(params.path, params.line);
    throw rpcError(-1, "unknown_command", `Unknown command ${method}.`);
  }

  return { calls, handle, flow };
}

async function startServer(mode) {
  const fake = createFake(mode);
  const server = createServer(async (request, response) => {
    const url = new URL(request.url, "http://127.0.0.1");
    if (request.method === "POST" && url.pathname === "/rpc") {
      const chunks = [];
      for await (const chunk of request) chunks.push(chunk);
      let message;
      try {
        message = JSON.parse(Buffer.concat(chunks).toString("utf8"));
      } catch {
        response.writeHead(400);
        response.end();
        return;
      }
      let reply;
      try {
        const result = fake.handle(message.method, message.params || {});
        reply = { jsonrpc: "2.0", id: message.id, result };
      } catch (error) {
        if (error && error.jsonrpc) reply = { ...error, id: message.id };
        else reply = rpcError(message.id, "internal", String(error?.message || error));
      }
      response.writeHead(200, { "content-type": "application/json", "cache-control": "no-store" });
      response.end(JSON.stringify(reply));
      return;
    }
    if (request.method === "GET" && url.pathname === "/favicon.ico") {
      response.writeHead(204);
      response.end();
      return;
    }
    if (request.method === "GET" && await serveClientAsset(response, url.pathname)) return;
    response.writeHead(404);
    response.end();
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const { port } = server.address();
  return {
    fake,
    server,
    url: `http://127.0.0.1:${port}/#test-token`,
    close: () => new Promise(resolve => {
      server.closeAllConnections?.();
      server.close(resolve);
    }),
  };
}

async function startChrome(profile, url) {
  const probe = createServer();
  await new Promise((resolve, reject) => {
    probe.once("error", reject);
    probe.listen(0, "127.0.0.1", resolve);
  });
  const port = probe.address().port;
  await new Promise(resolve => probe.close(resolve));
  const child = spawn(chromePath, [
    "--headless=new",
    `--remote-debugging-port=${port}`,
    `--user-data-dir=${profile}`,
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-gpu",
    "--hide-scrollbars",
    `--window-size=${process.env.DEBUGGER_WINDOW || "1280,800"}`,
    url,
  ], { stdio: ["ignore", "pipe", "pipe"] });
  let stderr = "";
  child.stderr.on("data", chunk => { stderr = (stderr + chunk).slice(-8000); });
  const deadline = Date.now() + 15000;
  let target = null;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json/list`);
      const targets = await response.json();
      target = targets.find(item => item.type === "page" && item.webSocketDebuggerUrl && item.url.startsWith("http://127.0.0.1"));
      if (target) break;
    } catch {}
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  if (!target) {
    child.kill("SIGKILL");
    throw new Error(`Chrome did not expose a page target.\n${stderr}`);
  }
  return {
    child,
    wsUrl: target.webSocketDebuggerUrl,
    close: async () => {
      child.kill("SIGTERM");
      await new Promise(resolve => {
        const timer = setTimeout(() => { child.kill("SIGKILL"); resolve(); }, 3000);
        child.once("exit", () => { clearTimeout(timer); resolve(); });
      });
    },
  };
}

class Cdp {
  constructor(socket) {
    this.socket = socket;
    this.nextId = 1;
    this.pending = new Map();
    this.exceptions = [];
    socket.addEventListener("message", event => {
      const message = JSON.parse(String(event.data));
      if (message.id) {
        const pending = this.pending.get(message.id);
        if (!pending) return;
        this.pending.delete(message.id);
        if (message.error) pending.reject(new Error(message.error.message));
        else pending.resolve(message.result);
      } else if (message.method === "Runtime.exceptionThrown") {
        this.exceptions.push(message.params.exceptionDetails?.text || message.params.exceptionDetails?.exception?.description || "exception");
      }
    });
  }

  send(method, params = {}) {
    const id = this.nextId++;
    this.socket.send(JSON.stringify({ id, method, params }));
    return new Promise((resolve, reject) => this.pending.set(id, { resolve, reject }));
  }

  async evaluate(expression) {
    const result = await this.send("Runtime.evaluate", {
      expression,
      awaitPromise: true,
      returnByValue: true,
    });
    if (result.exceptionDetails) {
      throw new Error(result.exceptionDetails.exception?.description || result.exceptionDetails.text);
    }
    return result.result.value;
  }

  close() {
    try { this.socket.close(); } catch {}
  }
}

async function connect(wsUrl) {
  const socket = new WebSocket(wsUrl);
  await new Promise((resolve, reject) => {
    socket.addEventListener("open", resolve, { once: true });
    socket.addEventListener("error", () => reject(new Error("CDP websocket failed.")), { once: true });
  });
  const cdp = new Cdp(socket);
  await cdp.send("Runtime.enable");
  await cdp.send("Page.enable");
  return cdp;
}

async function waitFor(cdp, description, expression, timeout = 8000) {
  const deadline = Date.now() + timeout;
  let last = null;
  while (Date.now() < deadline) {
    try {
      last = await cdp.evaluate(expression);
      if (last) return last;
    } catch (error) {
      last = error.message;
    }
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  throw new Error(`Timed out waiting for ${description}. Last value: ${JSON.stringify(last)}`);
}

async function click(cdp, selector) {
  const clicked = await cdp.evaluate(`(() => {
    const node = document.querySelector(${JSON.stringify(selector)});
    if (!node) return false;
    node.click();
    return true;
  })()`);
  assert.equal(clicked, true, `click ${selector}`);
}

async function text(cdp, selector) {
  return cdp.evaluate(`(() => document.querySelector(${JSON.stringify(selector)})?.textContent ?? "")()`);
}

async function disabled(cdp, selector) {
  return cdp.evaluate(`(() => Boolean(document.querySelector(${JSON.stringify(selector)})?.disabled))()`);
}

async function page(cdp) {
  return cdp.evaluate(`(() => {
    const view = document.getElementById("view-debugger");
    const q = selector => document.querySelector(selector);
    const text = selector => q(selector)?.textContent ?? "";
    const lines = [...document.querySelectorAll("#debugger-source .dbg-line")].map(row => ({
      line: row.dataset.line,
      code: row.querySelector(".dbg-line-code")?.textContent ?? "",
      current: row.classList.contains("is-current"),
    }));
    return {
      hidden: view?.hidden,
      status: text("#debugger-status").trim(),
      meta: text("#debugger-meta"),
      bannerHidden: document.getElementById("debugger-unavailable").hidden,
      bannerWarning: document.getElementById("debugger-unavailable").classList.contains("is-warning"),
      reason: text("#debugger-unavailable-reason"),
      adapter: text("#debugger-adapter"),
      session: text("#debugger-session"),
      attachDisabled: document.getElementById("debugger-attach").disabled,
      attachLabel: text("#debugger-attach").trim(),
      detachDisabled: document.getElementById("debugger-detach").disabled,
      pauseDisabled: document.getElementById("debugger-pause").disabled,
      continueDisabled: document.getElementById("debugger-continue").disabled,
      stepOverDisabled: document.getElementById("debugger-step-over").disabled,
      stepIntoDisabled: document.getElementById("debugger-step-into").disabled,
      stepOutDisabled: document.getElementById("debugger-step-out").disabled,
      evaluateDisabled: document.getElementById("debugger-evaluate").disabled,
      pausedDotHidden: document.getElementById("debugger-badge").hidden,
      threads: [...document.querySelectorAll("#debugger-threads .dbg-row .dbg-row-name")].map(node => node.textContent),
      frames: [...document.querySelectorAll("#debugger-stack .dbg-row .dbg-row-name")].map(node => node.textContent),
      frameTexts: [...document.querySelectorAll("#debugger-stack .dbg-row")].map(node => node.textContent),
      currentFrame: [...document.querySelectorAll("#debugger-stack .dbg-row")].findIndex(node => node.classList.contains("is-selected")),
      sourcePath: text("#debugger-source-path"),
      sourceMeta: text("#debugger-source-meta"),
      lines,
      sourceBody: text("#debugger-source"),
      breakpoints: [...document.querySelectorAll("#debugger-breakpoints .dbg-bp-row")].map(row => ({
        label: row.querySelector(".dbg-bp-label")?.textContent ?? "",
        checked: row.querySelector("input")?.checked,
        disabled: row.querySelector("input")?.disabled,
        rowDisabled: row.classList.contains("is-disabled"),
        hit: row.classList.contains("is-hit"),
      })),
      scopes: [...document.querySelectorAll("#debugger-variables .dbg-var-row.is-scope .dbg-var-name")].map(node => node.textContent),
      variables: [...document.querySelectorAll("#debugger-variables .dbg-var-row:not(.is-scope)")].map(row => ({
        name: row.querySelector(".dbg-var-name")?.textContent ?? "",
        value: row.querySelector(".dbg-var-value")?.textContent ?? "",
      })),
      console: [...document.querySelectorAll("#debugger-console .dbg-console-entry")].map(node => node.textContent),
      threadsText: text("#debugger-threads"),
      stackText: text("#debugger-stack"),
      variablesText: text("#debugger-variables"),
      breakpointsText: text("#debugger-breakpoints"),
    };
  })()`);
}

async function openDebugger(cdp) {
  await waitFor(cdp, "controller connection", `document.getElementById("connection-text")?.textContent === "Connected"`);
  await click(cdp, "#tab-debugger");
  await waitFor(cdp, "debugger view", `document.getElementById("view-debugger")?.hidden === false`);
  await waitFor(cdp, "debugger status", `document.getElementById("debugger-status")?.textContent.trim() !== ""`);
}

async function auditDom(cdp) {
  const report = await cdp.evaluate(`(() => {
    const failures = [];
    const ids = [...document.querySelectorAll("[id]")].map(node => node.id);
    const duplicates = ids.filter((id, index) => ids.indexOf(id) !== index);
    if (duplicates.length) failures.push("duplicate ids: " + [...new Set(duplicates)].join(", "));
    const brokenRefs = [];
    for (const attribute of ["aria-controls", "aria-labelledby", "aria-describedby"]) {
      for (const node of document.querySelectorAll("[" + attribute + "]")) {
        for (const id of (node.getAttribute(attribute) || "").split(/\\s+/).filter(Boolean)) {
          if (!document.getElementById(id)) brokenRefs.push(attribute + "=" + id);
        }
      }
    }
    for (const label of document.querySelectorAll("label[for]")) {
      if (!document.getElementById(label.getAttribute("for"))) brokenRefs.push("for=" + label.getAttribute("for"));
    }
    if (brokenRefs.length) failures.push("broken references: " + [...new Set(brokenRefs)].join(", "));

    const symbols = new Set([...document.querySelectorAll("symbol[id]")].map(node => node.id));
    const brokenIcons = [...document.querySelectorAll("use[href^='#']")]
      .map(node => node.getAttribute("href").slice(1))
      .filter(id => !symbols.has(id));
    if (brokenIcons.length) failures.push("broken icon references: " + [...new Set(brokenIcons)].join(", "));

    const tabs = [...document.querySelectorAll(".rail-tab")];
    const panels = [...document.querySelectorAll(".view[role='tabpanel']")];
    if (panels.length !== tabs.length) failures.push("rail tabs (" + tabs.length + ") do not match views (" + panels.length + ")");
    for (const panel of panels) {
      if (!tabs.some(tab => tab.getAttribute("aria-controls") === panel.id)) failures.push(panel.id + " has no rail tab");
    }
    const views = new Set(tabs.map(tab => tab.dataset.view));
    if (views.size !== tabs.length) failures.push("duplicate rail views");
    for (const tab of tabs) {
      if (tab.getAttribute("role") !== "tab") failures.push(tab.id + " is not role=tab");
      if (!document.getElementById(tab.getAttribute("aria-controls"))) failures.push(tab.id + " controls a missing view");
    }

    const unlabeled = [];
    for (const node of document.querySelectorAll("#view-debugger button, #view-debugger input, #view-debugger select")) {
      const name = (node.getAttribute("aria-label") || node.getAttribute("title") || "").trim()
        || (node.id && document.querySelector('label[for="' + node.id + '"]')?.textContent.trim())
        || (node.closest("label")?.textContent.trim())
        || (node.tagName === "BUTTON" ? node.textContent.trim() : "");
      if (!name) unlabeled.push(node.id || node.tagName);
    }
    if (unlabeled.length) failures.push("unlabeled controls: " + unlabeled.join(", "));

    const untyped = [...document.querySelectorAll("#view-debugger button")].filter(button => !button.hasAttribute("type")).map(button => button.id || button.textContent.trim());
    if (untyped.length) failures.push("buttons without type: " + untyped.join(", "));

    if (document.querySelectorAll("#debugger-source .dbg-line").length) failures.push("source lines rendered before any response");
    return failures;
  })()`);
  assert.deepEqual(report, [], `DOM audit failed:\n${report.join("\n")}`);
  console.log("PASS  DOM audit: unique ids, live aria/label references, icon symbols, rail tabs matching tabpanels, named controls");
}

async function readCalls(fake) {
  return fake.calls.map(call => ({ method: call.method, params: call.params }));
}

let audited = false;

async function runScenario(mode, body) {
  const server = await startServer(mode);
  const profile = await mkdtemp(join(tmpdir(), "cove-debugger-harness-"));
  let chrome = null;
  let cdp = null;
  try {
    chrome = await startChrome(profile, server.url);
    cdp = await connect(chrome.wsUrl);
    await openDebugger(cdp);
    if (!audited) {
      await auditDom(cdp);
      audited = true;
    }
    await body({ cdp, server, fake: server.fake });
    assert.deepEqual(cdp.exceptions, [], `page exceptions: ${cdp.exceptions.join("\n")}`);
  } finally {
    cdp?.close();
    await chrome?.close();
    await server.close();
    await rm(profile, { recursive: true, force: true });
  }
}

async function scenarioUnavailable() {
  await runScenario("unavailable", async ({ cdp, fake }) => {
    const view = await page(cdp);
    assert.equal(view.status, "Unavailable");
    assert.equal(view.bannerHidden, false);
    assert.equal(view.bannerWarning, false);
    assert.match(view.reason, /lldb-dap was not found/);
    assert.equal(view.adapter, "");
    assert.equal(view.attachDisabled, true);
    assert.equal(view.pauseDisabled, true);
    assert.equal(view.continueDisabled, true);
    assert.equal(view.stepOverDisabled, true);
    assert.equal(view.stepIntoDisabled, true);
    assert.equal(view.stepOutDisabled, true);
    assert.equal(view.evaluateDisabled, true);
    assert.equal(view.pausedDotHidden, true);
    assert.equal(view.lines.length, 0);
    assert.equal(view.frames.length, 0);
    assert.equal(await cdp.evaluate(`document.getElementById("view-debugger").getAttribute("aria-labelledby")`), "tab-debugger");
    assert.ok((await readCalls(fake)).some(call => call.method === "get_ui_debugger"));
    console.log("PASS  unavailable adapter: exact reason shown, process controls disabled");
  });
}

async function scenarioDeveloperModeWarning() {
  await runScenario("warning", async ({ cdp }) => {
    const view = await page(cdp);
    assert.equal(view.status, "Detached");
    assert.equal(view.bannerHidden, false);
    assert.equal(view.bannerWarning, true);
    assert.match(view.reason, /Developer mode is disabled/);
    assert.match(view.meta, /Adapter lldb-dap/);
    assert.equal(view.adapter, "");
    assert.equal(view.attachDisabled, false);
    assert.equal(view.pauseDisabled, true);
    assert.equal(view.stepOverDisabled, true);
    assert.equal(view.lines.length, 0);
    assert.equal(view.frames.length, 0);
    console.log("PASS  developer-mode warning: exact hint shown, attach remains available, no paused state");
  });
}

async function scenarioRefusedAttach() {
  await runScenario("refused", async ({ cdp, fake }) => {
    await click(cdp, "#debugger-attach");
    await waitFor(cdp, "refused attach", `document.getElementById("debugger-status")?.textContent.trim() === "Unavailable"`);
    const view = await page(cdp);
    assert.equal(view.bannerHidden, false);
    assert.equal(view.bannerWarning, false);
    assert.match(view.reason, /Not allowed to attach to process/);
    assert.match(view.reason, /Developer mode is disabled/);
    assert.equal(view.attachDisabled, false);
    assert.equal(view.attachLabel, "Retry attach");
    assert.equal(view.pauseDisabled, true);
    assert.equal(view.continueDisabled, true);
    assert.equal(view.stepOverDisabled, true);
    assert.equal(view.pausedDotHidden, true);
    assert.equal(view.lines.length, 0);
    assert.equal(view.frames.length, 0);
    const calls = await readCalls(fake);
    assert.equal(calls.filter(call => call.method === "attach_ui_debugger").length, 1);
    console.log("PASS  refused attach: adapter reason shown, retry available, controls disabled");
  });
}

async function scenarioAttachTimeout() {
  await runScenario("timeout", async ({ cdp, fake }) => {
    await click(cdp, "#debugger-attach");
    await waitFor(cdp, "attach timeout", `document.getElementById("debugger-status")?.textContent.trim() === "Error"`);
    const view = await page(cdp);
    assert.equal(view.bannerHidden, false);
    assert.equal(view.bannerWarning, false);
    assert.match(view.reason, /did not finish attaching to PID 4242 within 8000 ms/);
    assert.match(view.reason, /authorization prompt/);
    assert.equal(view.attachDisabled, false);
    assert.equal(view.pauseDisabled, true);
    assert.equal(view.continueDisabled, true);
    assert.equal(view.stepOverDisabled, true);
    assert.equal(view.pausedDotHidden, true);
    assert.equal(view.lines.length, 0);
    const calls = await readCalls(fake);
    assert.equal(calls.filter(call => call.method === "attach_ui_debugger").length, 1);
    console.log("PASS  attach timeout: controller timeout reason shown, retry available, no paused state");
  });
}

async function scenarioFailedStatus() {
  await runScenario("failedstatus", async ({ cdp }) => {
    await waitFor(cdp, "failed status", `document.getElementById("debugger-status")?.textContent.trim() === "Unavailable"`);
    const view = await page(cdp);
    assert.equal(view.bannerHidden, false);
    assert.equal(view.bannerWarning, false);
    assert.match(view.reason, /Not allowed to attach to process/);
    assert.match(view.reason, /Developer mode is disabled/);
    assert.match(view.reason, /no DWARF or dSYM/);
    assert.match(view.meta, /Adapter lldb-dap/);
    assert.equal(view.session, "No session");
    assert.equal(view.attachDisabled, false, "a refused attach can be retried after permissions change");
    assert.equal(view.attachLabel, "Retry attach");
    assert.equal(view.pauseDisabled, true);
    assert.equal(view.continueDisabled, true);
    assert.equal(view.stepOverDisabled, true);
    assert.equal(view.pausedDotHidden, true);
    assert.equal(view.lines.length, 0);
    assert.equal(view.frames.length, 0);
    console.log("PASS  get_ui_debugger failed-session reason (task_for_pid/developer mode) shown; no paused state");
  });
}

async function scenarioMissingSource() {
  await runScenario("nosource", async ({ cdp }) => {
    await click(cdp, "#debugger-attach");
    await waitFor(cdp, "paused after attach", `document.getElementById("debugger-status")?.textContent.trim() === "Paused"`);
    await waitFor(cdp, "missing source reason", `document.getElementById("debugger-source")?.textContent.includes("no debugger source tool")`);
    const view = await page(cdp);
    assert.equal(view.sourcePath, "/src/main.rs");
    assert.match(view.sourceBody, /Unknown command get_ui_debug_source/);
    assert.match(view.sourceBody, /Unknown command read_ui_debug_source/);
    assert.match(view.sourceBody, /main\.rs:12/);
    assert.equal(view.lines.length, 0, "no source lines may be invented");
    assert.equal(view.frames.length, 2);
    assert.deepEqual(view.scopes, ["Locals"]);
    assert.ok(view.variables.some(variable => variable.name === "count"));
    assert.equal(view.breakpoints.length, 1);
    console.log("PASS  missing source tool: exact reason and frame location shown, no fabricated lines");
  });
}

async function scenarioFlow() {
  await runScenario("flow", async ({ cdp, fake }) => {
    let view = await page(cdp);
    assert.equal(view.status, "Detached");
    assert.equal(view.bannerWarning, true);
    assert.equal(view.attachDisabled, false);
    assert.equal(view.detachDisabled, true);
    assert.equal(view.pauseDisabled, true);
    assert.equal(view.lines.length, 0, "no source before attach");
    assert.equal(view.frames.length, 0, "no frames before attach");

    await click(cdp, "#debugger-attach");
    await waitFor(cdp, "paused after attach", `document.getElementById("debugger-status")?.textContent.trim() === "Paused"`);
    await waitFor(cdp, "source lines", `document.querySelectorAll("#debugger-source .dbg-line").length === 8`);
    await waitFor(cdp, "first scope variables", `[...document.querySelectorAll("#debugger-variables .dbg-var-name")].some(node => node.textContent === "count")`);
    if (process.env.DEBUGGER_LAYOUT) {
      console.log("LAYOUT", JSON.stringify(await cdp.evaluate(`(() => {
        const pick = selector => {
          const node = document.querySelector(selector);
          if (!node) return null;
          const rect = node.getBoundingClientRect();
          const style = getComputedStyle(node);
          return { x: Math.round(rect.x), y: Math.round(rect.y), w: Math.round(rect.width), h: Math.round(rect.height), display: style.display, flex: style.flex, minHeight: style.minHeight, overflow: style.overflow };
        };
        return {
          viewport: [innerWidth, innerHeight],
          appBody: pick(".app-body"),
          view: pick("#view-debugger"),
          header: pick("#view-debugger .aux-header"),
          banner: pick("#debugger-unavailable"),
          controls: pick(".dbg-controls"),
          grid: pick(".dbg-grid"),
          side: pick(".dbg-side"),
          sourcePane: pick(".dbg-source-pane"),
          varsPane: pick(".dbg-vars-pane"),
          console: pick(".dbg-console"),
        };
      })()`), null, 2));
    }
    if (process.env.DEBUGGER_SHOT) {
      const shot = await cdp.send("Page.captureScreenshot", { format: "png" });
      await writeFile(process.env.DEBUGGER_SHOT, Buffer.from(shot.data, "base64"));
    }

    view = await page(cdp);
    assert.equal(view.bannerWarning, false);
    assert.equal(view.bannerHidden, true);
    assert.equal(view.attachDisabled, true);
    assert.equal(view.detachDisabled, false);
    assert.equal(view.pauseDisabled, true);
    assert.equal(view.continueDisabled, false);
    assert.equal(view.stepOverDisabled, false);
    assert.equal(view.stepIntoDisabled, false);
    assert.equal(view.stepOutDisabled, false);
    assert.equal(view.evaluateDisabled, false);
    assert.equal(view.pausedDotHidden, false);
    assert.deepEqual(view.threads, ["main"]);
    assert.deepEqual(view.frames, ["main", "helper"]);
    assert.equal(view.currentFrame, 0);
    assert.equal(view.sourcePath, "/src/main.rs");
    assert.equal(view.lines.length, 8);
    assert.equal(view.lines.find(line => line.line === "12")?.code, "    run(count);");
    assert.equal(view.lines.find(line => line.line === "12")?.current, true);
    assert.equal(view.sourceMeta.includes("12"), true, "current line in source meta");
    assert.deepEqual(view.scopes, ["Locals"]);
    assert.deepEqual(view.variables.find(variable => variable.name === "count"), { name: "count", value: "3" });
    assert.deepEqual(view.breakpoints.map(breakpoint => breakpoint.label), ["/src/main.rs:12"]);
    assert.equal(view.breakpoints[0].checked, true);
    assert.equal(view.breakpoints[0].hit, true);
    assert.match(view.session, /PID 4242/);
    assert.match(view.meta, /Adapter lldb-dap/);

    const gutter = await cdp.evaluate(`(() => {
      const node = document.querySelector('#debugger-source .dbg-gutter[data-line="12"]');
      return { present: Boolean(node), pressed: node?.getAttribute("aria-pressed"), disabled: node?.disabled };
    })()`);
    assert.deepEqual(gutter, { present: true, pressed: "true", disabled: false });

    // Nested variables: widget -> width.
    const expanded = await cdp.evaluate(`(() => {
      const button = [...document.querySelectorAll("#debugger-variables .dbg-var-twisty")]
        .find(node => node.getAttribute("aria-label") === "Expand widget");
      if (!button) return false;
      button.click();
      return true;
    })()`);
    assert.equal(expanded, true, "widget twisty");
    await waitFor(cdp, "nested width", `[...document.querySelectorAll("#debugger-variables .dbg-var-value")].some(node => node.textContent === "800.0")`);
    view = await page(cdp);
    assert.deepEqual(view.variables.find(variable => variable.name === "width"), { name: "width", value: "800.0" });

    // Frame selection: helper's scopes and line.
    await cdp.evaluate(`(() => {
      const rows = [...document.querySelectorAll("#debugger-stack .dbg-row")];
      rows[1]?.click();
    })()`);
    await waitFor(cdp, "helper frame selected", `(() => {
      const rows = [...document.querySelectorAll("#debugger-stack .dbg-row")];
      return rows[1]?.classList.contains("is-selected");
    })()`);
    await waitFor(cdp, "helper scope", `[...document.querySelectorAll("#debugger-variables .dbg-var-name")].some(node => node.textContent === "Arguments")`);
    view = await page(cdp);
    assert.equal(view.currentFrame, 1);
    assert.equal(view.lines.find(line => line.line === "5")?.current, true);
    assert.equal(view.scopes.includes("Arguments"), true);
    assert.deepEqual(view.variables.find(variable => variable.name === "helper_arg"), { name: "helper_arg", value: "7" });
    const frameScopes = (await readCalls(fake)).filter(call => call.method === "read_ui_debug_variables" && call.params.frame_id === 11);
    assert.ok(frameScopes.length >= 1, "scopes requested for the selected frame");

    // Return to the top frame.
    await cdp.evaluate(`(() => {
      const rows = [...document.querySelectorAll("#debugger-stack .dbg-row")];
      rows[0]?.click();
    })()`);
    await waitFor(cdp, "top frame selected", `(() => {
      const rows = [...document.querySelectorAll("#debugger-stack .dbg-row")];
      return rows[0]?.classList.contains("is-selected");
    })()`);
    await waitFor(cdp, "top scope", `[...document.querySelectorAll("#debugger-variables .dbg-var-name")].some(node => node.textContent === "count")`);

    // Evaluate.
    await cdp.evaluate(`(() => {
      document.getElementById("debugger-expression").value = "count + 1";
      document.getElementById("debugger-evaluate-form").requestSubmit();
    })()`);
    await waitFor(cdp, "evaluate result", `[...document.querySelectorAll("#debugger-console .dbg-console-entry.is-result")].some(node => node.textContent.includes("4"))`);
    view = await page(cdp);
    assert.ok(view.console.some(line => line.includes("count + 1")), "evaluate command logged");
    assert.ok(view.console.some(line => line.includes("4")), "evaluate result logged");

    // Breakpoints: set line 14, disable it, then enable it again.
    await click(cdp, '#debugger-source .dbg-gutter[data-line="14"]');
    await waitFor(cdp, "breakpoint at 14", `document.querySelectorAll("#debugger-breakpoints .dbg-bp-row").length === 2`);
    view = await page(cdp);
    assert.deepEqual(view.breakpoints.map(breakpoint => breakpoint.label), ["/src/main.rs:12", "/src/main.rs:14"]);
    let setCalls = (await readCalls(fake)).filter(call => call.method === "set_ui_breakpoint");
    assert.deepEqual(setCalls.at(-1)?.params, { target: "runtime-1", path: "/src/main.rs", line: 14 });

    await cdp.evaluate(`(() => {
      const row = [...document.querySelectorAll("#debugger-breakpoints .dbg-bp-row")]
        .find(node => node.textContent.includes(":14"));
      const input = row?.querySelector("input");
      if (input) {
        input.checked = false;
        input.dispatchEvent(new Event("change", { bubbles: true }));
      }
    })()`);
    await waitFor(cdp, "breakpoint disabled", `(() => {
      const row = [...document.querySelectorAll("#debugger-breakpoints .dbg-bp-row")]
        .find(node => node.textContent.includes(":14"));
      return row?.classList.contains("is-disabled");
    })()`);
    view = await page(cdp);
    const disabled = view.breakpoints.find(breakpoint => breakpoint.label === "/src/main.rs:14");
    assert.equal(disabled.checked, false);
    assert.equal(disabled.rowDisabled, true);
    const clearCalls = (await readCalls(fake)).filter(call => call.method === "clear_ui_breakpoint");
    assert.deepEqual(clearCalls.at(-1)?.params, { target: "runtime-1", path: "/src/main.rs", line: 14 });
    const gutter14 = await cdp.evaluate(`document.querySelector('#debugger-source .dbg-gutter[data-line="14"]')?.getAttribute("aria-pressed")`);
    assert.equal(gutter14, "false");

    await cdp.evaluate(`(() => {
      const row = [...document.querySelectorAll("#debugger-breakpoints .dbg-bp-row")]
        .find(node => node.textContent.includes(":14"));
      const input = row?.querySelector("input");
      if (input) {
        input.checked = true;
        input.dispatchEvent(new Event("change", { bubbles: true }));
      }
    })()`);
    await waitFor(cdp, "breakpoint enabled again", `(() => {
      const row = [...document.querySelectorAll("#debugger-breakpoints .dbg-bp-row")]
        .find(node => node.textContent.includes(":14"));
      return row && !row.classList.contains("is-disabled") && row.querySelector("input")?.checked;
    })()`);
    setCalls = (await readCalls(fake)).filter(call => call.method === "set_ui_breakpoint");
    assert.equal(setCalls.length, 2);

    // Step over: fake moves the stop to line 13.
    await click(cdp, "#debugger-step-over");
    await waitFor(cdp, "stepped line", `document.querySelector('#debugger-source .dbg-line.is-current')?.dataset.line === "13"`);
    view = await page(cdp);
    assert.equal(view.status, "Paused");
    assert.equal(view.lines.find(line => line.line === "13")?.current, true);
    assert.equal(view.frames.length, 2);
    const stackCalls = (await readCalls(fake)).filter(call => call.method === "get_ui_debug_stack");
    assert.ok(stackCalls.length >= 2, "stack reloaded after the step");

    // Continue: no current line and no stale frames while running.
    await click(cdp, "#debugger-continue");
    await waitFor(cdp, "running", `document.getElementById("debugger-status")?.textContent.trim() === "Running"`);
    view = await page(cdp);
    assert.equal(view.pauseDisabled, false);
    assert.equal(view.continueDisabled, true);
    assert.equal(view.stepOverDisabled, true);
    assert.equal(view.evaluateDisabled, true);
    assert.equal(view.lines.filter(line => line.current).length, 0);
    assert.equal(view.frames.length, 0);
    assert.match(view.threadsText, /Pause the process to read threads/);
    assert.match(view.stackText, /Pause the process to read the call stack/);
    assert.equal(view.pausedDotHidden, true);

    // Pause: a real stopped response is needed before the stack appears.
    await click(cdp, "#debugger-pause");
    await waitFor(cdp, "paused again", `document.getElementById("debugger-status")?.textContent.trim() === "Paused"`);
    await waitFor(cdp, "stack after pause", `document.querySelectorAll("#debugger-stack .dbg-row").length === 2`);

    // Detach: no paused state, no current line, session cleared.
    await click(cdp, "#debugger-detach");
    await waitFor(cdp, "detached", `document.getElementById("debugger-status")?.textContent.trim() === "Detached"`);
    view = await page(cdp);
    assert.equal(view.attachDisabled, false);
    assert.equal(view.detachDisabled, true);
    assert.equal(view.frames.length, 0);
    assert.equal(view.lines.filter(line => line.current).length, 0);
    assert.equal(view.pausedDotHidden, true);
    assert.match(view.session, /No session/);

    // Narrow window: the panes keep a usable height and the view scrolls.
    await cdp.send("Emulation.setDeviceMetricsOverride", { width: 760, height: 470, deviceScaleFactor: 1, mobile: false });
    await new Promise(resolve => setTimeout(resolve, 80));
    const narrow = await cdp.evaluate(`(() => {
      const view = document.getElementById("view-debugger");
      const grid = document.querySelector(".dbg-grid");
      const side = document.querySelector(".dbg-side");
      return {
        viewScroll: view.scrollHeight > view.clientHeight,
        overflow: getComputedStyle(view).overflowY,
        gridHeight: Math.round(grid.getBoundingClientRect().height),
        sideScroll: side.scrollHeight >= side.clientHeight,
        breakpointsPresent: Boolean(document.getElementById("debugger-breakpoints")),
      };
    })()`);
    assert.equal(narrow.overflow, "auto");
    assert.equal(narrow.viewScroll, true);
    assert.ok(narrow.gridHeight >= 200, `narrow grid height ${narrow.gridHeight}`);
    assert.equal(narrow.breakpointsPresent, true);
    await cdp.send("Emulation.clearDeviceMetricsOverride");
    console.log("PASS  narrow window keeps a 220px debugger grid and scrolls the view");

    // Existing views must keep rendering after the debugger is added.
    for (const name of ["network", "logs", "performance", "memory", "app-size", "deep-links", "inspector", "debugger"]) {
      await click(cdp, `#tab-${name}`);
      await waitFor(cdp, `${name} view`, `document.getElementById("view-${name}")?.hidden === false`);
    }
    console.log("PASS  attach/pause/step/continue/threads/stack/scopes/variables/evaluate/breakpoints/detach flow");
    console.log("PASS  all rail views render without a page exception");
  });
}

async function main() {
  const nodeMajor = Number(process.versions.node.split(".")[0]);
  assert.ok(nodeMajor >= 22, `Node ${process.versions.node} has no global WebSocket/fetch`);
  try {
    await access(chromePath);
  } catch {
    throw new Error(`Chrome was not found at ${chromePath}; set CHROME_PATH to a Chrome or Chromium binary.`);
  }
  console.log(`Debugger harness: ${process.version}, client dir ${clientDir}`);
  await scenarioUnavailable();
  await scenarioDeveloperModeWarning();
  await scenarioRefusedAttach();
  await scenarioAttachTimeout();
  await scenarioFailedStatus();
  await scenarioMissingSource();
  await scenarioFlow();
  console.log("RESULT PASS  Debugger client fake-controller harness");
}

main().catch(error => {
  console.error(`RESULT FAIL  ${error.stack || error.message}`);
  process.exitCode = 1;
});
