#!/usr/bin/env node
// Fake-controller harness for the Logs "Clear" control. It serves the real
// index.html/inspector.js/inspector.css, answers /rpc with the controller's
// session/log/clear shapes, and drives the page through Chrome DevTools
// Protocol. It covers a successful clear, a rejected acknowledgement, a
// runtime error, an in-flight clear followed by a target switch, disconnect,
// follow mode across a clear, and the DOM audit. No Rust process starts and no
// log record is invented outside the fake controller's responses.
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";

const here = dirname(fileURLToPath(import.meta.url));
const clientDir = resolve(here, "..");
const chromePath = process.env.CHROME_PATH
  || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

const files = {
  "/": ["index.html", "text/html; charset=utf-8"],
  "/index.html": ["index.html", "text/html; charset=utf-8"],
  "/inspector.js": ["inspector.js", "text/javascript; charset=utf-8"],
  "/inspector.css": ["inspector.css", "text/css; charset=utf-8"],
};

function target(id, title, capabilities, pid = 4242, kind = "runtime") {
  return { id, title, platform: "macos", pid, kind, capabilities };
}

function record(sequence, level, message, label = "gallery") {
  return {
    sequence,
    timestamp: 1760000000000 + sequence * 1000,
    level,
    label,
    message,
    location: {
      file: "/Users/dev/cove/examples/gallery/src/main.rs",
      function: "gallery::run",
      line: 40 + sequence,
      column: 5,
      source: "gallery",
    },
    process: { name: "gallery", pid: 4242, runID: "run-logs-harness" },
    metadata: {},
  };
}

const INITIAL = {
  "runtime-1": [record(41, "info", "Gallery started"), record(42, "warning", "Slow frame"), record(43, "error", "Draw failed")],
  "runtime-2": [record(71, "info", "Second target ready"), record(72, "notice", "Configuration loaded")],
};

function rpcError(code, message) {
  return {
    jsonrpc: "2.0",
    id: null,
    error: { code: -32000, message, data: { code } },
  };
}

function createFake(mode) {
  const calls = [];
  const history = new Map();
  const nextSequence = new Map();
  for (const [id, records] of Object.entries(INITIAL)) {
    history.set(id, { records: records.map(entry => ({ ...entry })), dropped: id === "runtime-1" ? 2 : 0 });
    nextSequence.set(id, records.at(-1).sequence + 1);
  }
  const cleared = [];
  let pendingClear = null;
  let pendingRead = null;
  let readCount = 0;

  function targets() {
    if (mode === "no-capability") {
      return [target("runtime-1", "No Diagnostics", ["tree"])];
    }
    if (mode === "switch" || mode === "slow") {
      return [
        target("runtime-1", "Logs One", ["tree", "diagnostics"], 4242),
        target("runtime-2", "Logs Two", ["tree", "diagnostics"], 4243),
        target("runtime-3", "Plain Window", ["capture"], 4244, "window"),
      ];
    }
    return [target("runtime-1", "Logs Target", ["tree", "diagnostics"])];
  }

  function session() {
    return {
      platform: "macos",
      permissions: { screen_capture: false, accessibility: false },
      targets: targets(),
    };
  }

  function treePayload() {
    return {
      root: {
        id: "root",
        ownerScopeID: "root",
        debugName: "AppRoot",
        viewTag: "gallery::App",
        layoutRole: "stack",
        children: [
          { id: "node-1", ownerScopeID: "node-1", debugName: "Header", viewTag: "gallery::Header", layoutRole: "stack", children: [] },
          { id: "node-2", ownerScopeID: "node-2", debugName: "ButtonPrimary", viewTag: "gallery::Button", semanticRole: "button", children: [] },
        ],
      },
    };
  }

  function snapshot(id, after) {
    const entry = history.get(id) || { records: [], dropped: 0 };
    let records = entry.records.filter(entry => after === undefined || entry.sequence > after);
    let dropped = entry.dropped;
    if (records.length > 100) {
      const excess = records.length - 100;
      records = records.slice(excess);
      dropped += excess;
    }
    return {
      records,
      dropped,
      firstAvailableSequence: records[0]?.sequence ?? entry.records[0]?.sequence ?? null,
    };
  }

  async function handle(method, params = {}) {
    calls.push({ method, params });
    if (method === "get_ui_session") return session();
    if (method === "inspect_ui_tree") return treePayload();
    if (method === "query_ui_logs") {
      readCount += 1;
      const result = snapshot(params.target, params.after);
      if (mode === "slow-read" && readCount > 1) {
        await new Promise(resolve => { pendingRead = resolve; });
        pendingRead = null;
      }
      return result;
    }
    if (method === "clear_ui_logs") {
      if (mode === "error") {
        throw rpcError("unsupported", "The runtime cannot clear its log history.");
      }
      if (mode === "reject") {
        return { ok: false, error: { code: "unsupported", message: "The runtime cannot clear its log history." } };
      }
      if (mode === "slow" && !pendingClear) {
        await new Promise(resolve => { pendingClear = resolve; });
        pendingClear = null;
      }
      const entry = history.get(params.target) || { records: [], dropped: 0 };
      const count = entry.records.length;
      // Clearing removes the replay window and the drops counted for it. The
      // sequence counter itself stays where it was, so later records remain
      // monotonically newer than the cleared window.
      entry.records = [];
      entry.dropped = 0;
      history.set(params.target, entry);
      cleared.push({ target: params.target, count });
      return { cleared: count, dropped: 0, firstAvailableSequence: null };
    }
    throw rpcError("unknown_command", `Unknown command ${method}.`);
  }

  function append(id, level, message) {
    const entry = history.get(id) || { records: [], dropped: 0 };
    const sequence = nextSequence.get(id) || 1;
    nextSequence.set(id, sequence + 1);
    const added = record(sequence, level, message);
    entry.records.push(added);
    history.set(id, entry);
    return added;
  }

  return {
    calls,
    history,
    cleared,
    handle,
    append,
    releaseClear: () => pendingClear?.(),
    releaseRead: () => pendingRead?.(),
    records: id => history.get(id)?.records.length ?? 0,
  };
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
        const result = await fake.handle(message.method, message.params || {});
        reply = { jsonrpc: "2.0", id: message.id, result };
      } catch (error) {
        if (error && error.jsonrpc) reply = { ...error, id: message.id };
        else reply = { ...rpcError("internal", String(error?.message || error)), id: message.id };
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
    const entry = files[url.pathname];
    if (request.method === "GET" && entry) {
      const body = await readFile(join(clientDir, entry[0]));
      response.writeHead(200, { "content-type": entry[1], "cache-control": "no-store" });
      response.end(body);
      return;
    }
    response.writeHead(404);
    response.end();
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const { port } = server.address();
  let closed = false;
  return {
    fake,
    url: `http://127.0.0.1:${port}/#test-token`,
    close: () => {
      if (closed) return Promise.resolve();
      closed = true;
      return new Promise(resolve => {
        server.closeAllConnections?.();
        server.close(() => resolve());
      });
    },
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
    `--window-size=${process.env.LOGS_WINDOW || "1440,900"}`,
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

async function waitForCall(fake, description, predicate, timeout = 8000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (predicate(fake.calls)) return fake.calls;
    await new Promise(resolve => setTimeout(resolve, 40));
  }
  throw new Error(`Timed out waiting for ${description}. Calls: ${JSON.stringify(fake.calls)}`);
}

async function waitUntil(description, predicate, timeout = 8000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await new Promise(resolve => setTimeout(resolve, 40));
  }
  throw new Error(`Timed out waiting for ${description}.`);
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

async function openLogs(cdp) {
  await waitFor(cdp, "controller connection", `document.getElementById("connection-text")?.textContent === "Connected"`);
  await waitFor(cdp, "component tree", `document.querySelectorAll("#tree .tree-row").length === 3`);
  await click(cdp, "#tab-logs");
  await waitFor(cdp, "logs view", `document.getElementById("view-logs").hidden === false`);
}

async function waitRows(cdp, count) {
  await waitFor(cdp, `${count} log rows`, `document.querySelectorAll("#logs-content .log-entry").length === ${count}`);
}

function clearControlScript() {
  return `(() => {
    const button = document.getElementById("logs-clear");
    const content = document.getElementById("logs-content");
    const badge = document.getElementById("logs-badge");
    return {
      disabled: button.disabled,
      text: button.textContent.trim(),
      label: button.getAttribute("aria-label") || button.textContent.trim(),
      title: button.title,
      busy: button.getAttribute("aria-busy"),
      type: button.getAttribute("type"),
      meta: document.getElementById("logs-meta").textContent,
      rows: content.querySelectorAll(".log-entry").length,
      stateTitle: content.querySelector(".state strong")?.textContent || "",
      stateMessage: content.querySelector(".state p")?.textContent || "",
      stateError: content.querySelector(".state.is-error") !== null,
      inlineError: content.querySelector(".error-inline")?.textContent || "",
      contentBusy: content.getAttribute("aria-busy"),
      badgeHidden: badge.hidden,
      badgeText: badge.textContent,
      follow: document.getElementById("logs-follow").checked,
    };
  })()`;
}

async function clearControl(cdp) {
  return cdp.evaluate(clearControlScript());
}

async function waitClearIdle(cdp) {
  await waitFor(cdp, "clear request settled", `(() => {
    const button = document.getElementById("logs-clear");
    return button.disabled === false && button.textContent.trim() === "Clear";
  })()`);
}

async function auditDom(cdp) {
  const report = await cdp.evaluate(`(() => {
    const failures = [];
    const ids = [...document.querySelectorAll("[id]")].map(node => node.id);
    const duplicates = ids.filter((id, index) => ids.indexOf(id) !== index);
    if (duplicates.length) failures.push("duplicate ids: " + [...new Set(duplicates)].join(", "));
    const brokenRefs = [];
    for (const attribute of ["aria-controls", "aria-labelledby", "aria-describedby", "for"]) {
      for (const node of document.querySelectorAll("#view-logs [" + attribute + "]")) {
        for (const id of (node.getAttribute(attribute) || "").split(/\\s+/).filter(Boolean)) {
          if (!document.getElementById(id)) brokenRefs.push(attribute + "=" + id);
        }
      }
    }
    if (brokenRefs.length) failures.push("broken references: " + [...new Set(brokenRefs)].join(", "));
    const symbols = new Set([...document.querySelectorAll("symbol[id]")].map(node => node.id));
    const brokenIcons = [...document.querySelectorAll("#view-logs use[href^='#']")]
      .map(node => node.getAttribute("href").slice(1))
      .filter(id => !symbols.has(id));
    if (brokenIcons.length) failures.push("broken icon references: " + [...new Set(brokenIcons)].join(", "));
    const unlabeled = [];
    for (const node of document.querySelectorAll("#view-logs button, #view-logs input, #view-logs select")) {
      const name = (node.getAttribute("aria-label") || node.getAttribute("title") || "").trim()
        || (node.id && document.querySelector('label[for="' + node.id + '"]')?.textContent.trim())
        || (node.closest("label")?.textContent.trim())
        || (node.tagName === "BUTTON" ? node.textContent.trim() : "");
      if (!name) unlabeled.push(node.id || node.className || node.tagName);
    }
    if (unlabeled.length) failures.push("unlabeled controls: " + unlabeled.join(", "));
    const untyped = [...document.querySelectorAll("#view-logs button")].filter(button => !button.hasAttribute("type")).map(button => button.id || button.className || button.textContent.trim());
    if (untyped.length) failures.push("buttons without type: " + untyped.join(", "));
    const clear = document.getElementById("logs-clear");
    if (!clear) failures.push("clear button is missing");
    else {
      if (clear.getAttribute("type") !== "button") failures.push("clear button is not type=button");
      if (clear.textContent.trim() !== "Clear") failures.push("clear button is not labelled Clear");
      if (!clear.title) failures.push("clear button has no title");
      if (!clear.closest(".aux-actions")) failures.push("clear button is outside the logs actions");
      if (clear.closest("label")) failures.push("clear button is nested in a label");
      if (clear.hasAttribute("aria-pressed")) failures.push("clear button pretends to be a toggle");
    }
    const content = document.getElementById("logs-content");
    if (!content) failures.push("logs content is missing");
    else if (content.getAttribute("aria-busy") !== "false") failures.push("logs content is not settled");
    const badge = document.getElementById("logs-badge");
    if (!badge) failures.push("logs badge is missing");
    return failures;
  })()`);
  assert.deepEqual(report, [], `Logs DOM audit failed:\n${report.join("\n")}`);
  console.log("PASS  DOM audit: unique ids, live logs references, icon symbols, named typed controls, clear control attributes");
}

let audited = false;

async function runScenario(mode, body) {
  const server = await startServer(mode);
  const profile = await mkdtemp(join(tmpdir(), "cove-logs-harness-"));
  let chrome = null;
  let cdp = null;
  try {
    chrome = await startChrome(profile, server.url);
    cdp = await connect(chrome.wsUrl);
    await openLogs(cdp);
    await body({ cdp, fake: server.fake, server });
    assert.deepEqual(cdp.exceptions, [], `page exceptions: ${cdp.exceptions.join("\n")}`);
  } finally {
    cdp?.close();
    await chrome?.close();
    await server.close();
    await rm(profile, { recursive: true, force: true });
  }
}

async function screenshot(cdp, path) {
  if (!path) return;
  const shot = await cdp.send("Page.captureScreenshot", { format: "png" });
  await writeFile(path, Buffer.from(shot.data, "base64"));
}

function clearCalls(fake) {
  return fake.calls.filter(call => call.method === "clear_ui_logs");
}

async function scenarioSuccess() {
  await runScenario("success", async ({ cdp, fake }) => {
    await waitRows(cdp, 3);
    if (!audited) {
      await auditDom(cdp);
      audited = true;
    }
    const initial = await clearControl(cdp);
    assert.equal(initial.disabled, false);
    assert.equal(initial.text, "Clear");
    assert.equal(initial.title, "Clear the runtime's replay log history");
    assert.equal(initial.busy, "false");
    assert.equal(initial.badgeHidden, false);
    assert.equal(initial.badgeText, "3");
    assert.match(initial.meta, /3 records/);
    assert.match(initial.meta, /2 dropped/);
    assert.match(initial.meta, /first available #41/);

    await click(cdp, "#logs-clear");
    await waitFor(cdp, "cleared empty state", `document.querySelector("#logs-content .state strong")?.textContent === "Log history cleared"`);
    await waitClearIdle(cdp);
    const after = await clearControl(cdp);
    assert.equal(after.rows, 0);
    assert.equal(after.meta, "Cleared");
    assert.equal(after.stateTitle, "Log history cleared");
    assert.equal(after.stateMessage, "Records logged after the clear will appear here.");
    assert.equal(after.stateError, false);
    assert.equal(after.badgeHidden, true);
    assert.equal(after.badgeText, "0");
    assert.equal(after.follow, false, "a clear must preserve follow mode");
    assert.equal(after.disabled, false);
    assert.deepEqual(clearCalls(fake).map(call => call.params), [{ target: "runtime-1" }]);
    assert.equal(fake.records("runtime-1"), 0, "the fake runtime's replay history is cleared");
    assert.deepEqual(fake.history.get("runtime-1").dropped, 0, "the cleared window's dropped count is reset");
    assert.deepEqual(fake.cleared, [{ target: "runtime-1", count: 3 }]);
    const toast = await cdp.evaluate(`({ hidden: document.getElementById("toast").hidden, kind: document.getElementById("toast").className, text: document.getElementById("toast-text").textContent })`);
    assert.equal(toast.hidden, false);
    assert.match(toast.kind, /is-success/);
    assert.equal(toast.text, "3 log records cleared.");
    await screenshot(cdp, process.env.LOGS_SHOT);
  });
}

async function scenarioRuntimeError() {
  await runScenario("error", async ({ cdp, fake }) => {
    await waitRows(cdp, 3);
    await click(cdp, "#logs-clear");
    await waitFor(cdp, "clear error toast", `document.getElementById("toast").classList.contains("is-error") && document.getElementById("toast").hidden === false`);
    await waitClearIdle(cdp);
    const toast = await cdp.evaluate(`document.getElementById("toast-text").textContent`);
    assert.match(toast, /Clear failed: The runtime cannot clear its log history\./);
    const view = await clearControl(cdp);
    assert.equal(view.rows, 3, "a failed clear must keep the existing table");
    assert.equal(view.badgeHidden, false);
    assert.equal(view.badgeText, "3");
    assert.match(view.meta, /3 records/);
    assert.match(view.inlineError, /Clear failed: The runtime cannot clear its log history\./);
    assert.equal(view.stateTitle, "");
    assert.equal(view.meta === "Cleared", false);
    assert.equal(view.disabled, false);
    assert.deepEqual(clearCalls(fake).map(call => call.params), [{ target: "runtime-1" }]);
    assert.equal(fake.records("runtime-1"), 3, "the runtime history must survive a failed clear");
  });
}

async function scenarioRejectedAcknowledgement() {
  await runScenario("reject", async ({ cdp, fake }) => {
    await waitRows(cdp, 3);
    await click(cdp, "#logs-clear");
    await waitFor(cdp, "acknowledgement rejection rejected", `document.querySelector("#logs-content .error-inline")?.textContent.includes("The runtime cannot clear its log history.")`);
    await waitClearIdle(cdp);
    const view = await clearControl(cdp);
    assert.equal(view.rows, 3, "an ok=false acknowledgement must not clear the table");
    assert.equal(view.meta === "Cleared", false);
    assert.match(view.meta, /3 records/);
    assert.deepEqual(clearCalls(fake).map(call => call.params), [{ target: "runtime-1" }]);
    assert.equal(fake.records("runtime-1"), 3);
  });
}

async function scenarioFollow() {
  await runScenario("success", async ({ cdp, fake }) => {
    await waitRows(cdp, 3);
    await click(cdp, "#logs-follow");
    await waitFor(cdp, "follow enabled", `document.getElementById("logs-follow").checked === true`);
    await click(cdp, "#logs-clear");
    await waitFor(cdp, "cleared empty state", `document.querySelector("#logs-content .state strong")?.textContent === "Log history cleared"`);
    await waitForCall(fake, "post-clear follow read", calls => {
      const clearIndex = calls.findIndex(call => call.method === "clear_ui_logs");
      return clearIndex !== -1 && calls.slice(clearIndex + 1)
        .some(call => call.method === "query_ui_logs" && call.params.target === "runtime-1");
    });
    const calls = fake.calls;
    const clearIndex = calls.findIndex(call => call.method === "clear_ui_logs");
    assert.notEqual(clearIndex, -1, "clear_ui_logs was sent");
    const followQuery = calls.slice(clearIndex + 1).find(call => call.method === "query_ui_logs" && call.params.target === "runtime-1");
    assert.ok(followQuery, "follow mode reads a fresh window after the clear");
    assert.equal(followQuery.params.after, undefined, "the fresh window starts without an after cursor");
    const cleared = await clearControl(cdp);
    assert.equal(cleared.follow, true, "clear must leave follow enabled");
    assert.equal(cleared.rows, 0);
    assert.equal(cleared.stateMessage, "Waiting for records logged after the clear.");

    const added = fake.append("runtime-1", "info", "After clear record");
    assert.equal(added.sequence, 44, "sequences continue after the cleared window");
    await waitFor(cdp, "followed record", `document.querySelectorAll("#logs-content .log-entry").length === 1 && document.getElementById("logs-content").textContent.includes("After clear record")`, 9000);
    const followed = await clearControl(cdp);
    assert.equal(followed.follow, true);
    assert.match(followed.meta, /1 record/);
    assert.equal(followed.badgeHidden, false);
    assert.equal(followed.badgeText, "1");
    assert.equal(followed.rows, 1, "the cleared records must not come back");
  });
}

async function scenarioStaleReadDuringClear() {
  await runScenario("slow-read", async ({ cdp, fake }) => {
    await waitRows(cdp, 3);
    // Turning follow on starts a read; turning it back off stops the timer but
    // leaves the request in flight.
    await click(cdp, "#logs-follow");
    await click(cdp, "#logs-follow");
    await waitFor(cdp, "follow disabled", `document.getElementById("logs-follow").checked === false`);
    await waitForCall(fake, "in-flight read", calls => calls.filter(call => call.method === "query_ui_logs").length >= 2);

    await click(cdp, "#logs-clear");
    await waitFor(cdp, "cleared empty state", `document.querySelector("#logs-content .state strong")?.textContent === "Log history cleared"`);
    await waitClearIdle(cdp);
    assert.equal(fake.records("runtime-1"), 0);

    // The read started before the clear answers with the old window. It must
    // not merge those records back into the cleared table.
    fake.releaseRead();
    await new Promise(resolve => setTimeout(resolve, 350));
    const after = await clearControl(cdp);
    assert.equal(after.rows, 0);
    assert.equal(after.meta, "Cleared");
    assert.equal(after.stateTitle, "Log history cleared");
    assert.equal(after.badgeHidden, true);
    assert.deepEqual(fake.cleared, [{ target: "runtime-1", count: 3 }]);
  });
}

async function selectTarget(cdp, id, ready) {
  const value = await cdp.evaluate(`(() => {
    const select = document.getElementById("targets");
    select.value = ${JSON.stringify(id)};
    select.dispatchEvent(new Event("change", { bubbles: true }));
    return select.value;
  })()`);
  assert.equal(value, id, `select target ${id}`);
  await waitFor(cdp, `target ${id}`, `(document.getElementById("targets").value === ${JSON.stringify(id)}) && (${ready})`);
}

async function scenarioTargetChangeDuringClear() {
  await runScenario("slow", async ({ cdp, fake }) => {
    await waitRows(cdp, 3);
    await click(cdp, "#logs-clear");
    await waitForCall(fake, "clear_ui_logs", calls => calls.some(call => call.method === "clear_ui_logs"));
    await waitFor(cdp, "clear busy state", `(() => {
      const button = document.getElementById("logs-clear");
      return button.disabled === true && button.textContent.trim() === "Clearing…";
    })()`);
    const busy = await clearControl(cdp);
    assert.equal(busy.busy, "true");
    assert.equal(busy.contentBusy, "true");
    assert.match(busy.meta, /Clearing…/);

    await selectTarget(cdp, "runtime-2", `document.querySelectorAll("#logs-content .log-entry").length === 2`);
    const switched = await clearControl(cdp);
    assert.equal(switched.rows, 2);
    assert.match(switched.meta, /2 records/);
    assert.equal(switched.badgeText, "2");
    assert.equal(switched.meta === "Cleared", false);

    fake.releaseClear();
    await waitUntil("the old clear to reach the fake runtime", () => fake.cleared.length === 1);
    await new Promise(resolve => setTimeout(resolve, 350));
    const after = await clearControl(cdp);
    assert.equal(after.rows, 2, "an old clear acknowledgement must not touch the new target's table");
    assert.match(after.meta, /2 records/);
    assert.equal(after.stateTitle, "");
    assert.equal(after.disabled, false);
    assert.equal(after.text, "Clear");
    assert.deepEqual(clearCalls(fake).map(call => call.params), [{ target: "runtime-1" }]);
    assert.equal(fake.records("runtime-2"), 2);

    await selectTarget(cdp, "runtime-1", `document.querySelector("#logs-content .state strong")?.textContent === "No log records"`);
    const back = await clearControl(cdp);
    assert.equal(back.rows, 0, "the first target's clear really removed its replay history");
    assert.equal(back.badgeHidden, true);
  });
}

async function scenarioDisconnect() {
  await runScenario("success", async ({ cdp, fake, server }) => {
    await waitRows(cdp, 3);
    await server.close();
    await click(cdp, "#logs-clear");
    await waitFor(cdp, "offline connection", `document.getElementById("connection-text")?.textContent === "Offline"`);
    await waitFor(cdp, "clear network error", `document.querySelector("#logs-content .error-inline")?.textContent.includes("Could not reach the controller")`);
    const view = await clearControl(cdp);
    assert.equal(view.rows, 3, "a disconnected clear must keep the existing table");
    assert.match(view.meta, /3 records/);
    assert.equal(view.badgeText, "3");
    assert.equal(view.disabled, true, "clear is unavailable while offline");
    assert.equal(view.title, "Reconnect to the controller to clear logs");
    assert.equal(view.text, "Clear");
    assert.match(view.inlineError, /Clear failed: Could not reach the controller/);
    const toast = await cdp.evaluate(`({ kind: document.getElementById("toast").className, text: document.getElementById("toast-text").textContent })`);
    assert.match(toast.kind, /is-error/);
    assert.match(toast.text, /Clear failed: Could not reach the controller/);
    assert.equal(fake.records("runtime-1"), 3);
  });
}

async function scenarioNoCapability() {
  await runScenario("no-capability", async ({ cdp, fake }) => {
    await waitFor(cdp, "unavailable logs", `document.querySelector("#logs-content .state strong")?.textContent === "Logs are unavailable"`);
    const view = await clearControl(cdp);
    assert.equal(view.disabled, true);
    assert.equal(view.title, "This runtime does not advertise diagnostics");
    assert.equal(view.rows, 0);
    await click(cdp, "#logs-clear");
    await new Promise(resolve => setTimeout(resolve, 300));
    assert.equal(clearCalls(fake).length, 0, "a disabled clear must not send clear_ui_logs");
  });
}

console.log("Logs clear harness: fake controller + headless Chrome");
await scenarioSuccess();
console.log("PASS  success: clear_ui_logs acknowledged, rows/empty state/badge/dropped reset, follow preserved");
await scenarioRuntimeError();
console.log("PASS  runtime error: table and runtime history intact, inline and toast error, control usable");
await scenarioRejectedAcknowledgement();
console.log("PASS  ok=false acknowledgement: no local clear");
await scenarioFollow();
console.log("PASS  follow: stays enabled, fresh after-less window, new record appears, cleared rows stay gone");
await scenarioStaleReadDuringClear();
console.log("PASS  stale read: a pre-clear response cannot repopulate the cleared table");
await scenarioTargetChangeDuringClear();
console.log("PASS  target change during clear: old acknowledgement cannot clear the new target's table");
await scenarioDisconnect();
console.log("PASS  disconnect: clear fails visibly, table intact, control disabled offline");
await scenarioNoCapability();
console.log("PASS  capability absent: clear disabled with an explanatory title and no RPC");
console.log("RESULT PASS");
