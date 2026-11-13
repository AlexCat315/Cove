#!/usr/bin/env node
// Fake-controller harness for the Network recording controls. It serves the
// real index.html/inspector.js/inspector.css, answers /rpc with the
// controller's session/network shapes, and drives the page through Chrome
// DevTools Protocol. The fake models the process-wide recorder: pausing stops
// it from retaining new requests, clearing empties only that target's retained
// window, and the enabled state survives a clear. It covers pause/resume,
// clear, a pre-paused runtime reported by the query, runtime errors, a rejected
// acknowledgement, an unsupported tool, target switches during in-flight
// control calls, disconnect, capability-absent targets, follow across a clear
// and the DOM audit. No Rust process starts and no record is invented outside
// the fake controller's responses.
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

const LIMIT = 256;
const RUNTIME_CAPS = ["tree", "network", "network_control"];
const NO_CONTROL_CAPS = ["tree", "network"];

function target(id, title, capabilities, pid = 4242, kind = "runtime") {
  return { id, title, platform: "macos", pid, kind, capabilities };
}

let nextId = 1;

function request(method, url, status, state = "complete", overrides = {}) {
  const id = nextId++;
  return {
    id,
    method,
    url,
    status,
    state,
    error: state === "failed" ? "connection reset" : null,
    startedAt: 1760000000000 + id * 1000,
    ttfbMs: 12.5 + id,
    durationMs: 40 + id * 2,
    requestBytes: 120 + id,
    responseBytes: 512 + id * 10,
    requestHeaders: [["accept", "*/*"]],
    responseHeaders: [["content-type", "text/plain"]],
    ...overrides,
  };
}

function initialRecords(id) {
  if (id === "runtime-2") {
    return [request("GET", "https://example.test/health", 200), request("GET", "https://example.test/ready", 204)];
  }
  return [
    request("GET", "https://example.test/items", 200),
    request("POST", "https://example.test/items", 201),
    request("GET", "https://example.test/missing", 500, "failed"),
  ];
}

function waterfallRecords() {
  // Wall-clock-relative starts so the pending request's lower bound is a
  // sensible span rather than a year of clock drift.
  const base = Date.now() - 4000;
  const make = overrides => ({ ...request("GET", "https://example.test/placeholder", 200), ...overrides });
  return [
    make({ id: 901, url: "https://example.test/first", startedAt: base, ttfbMs: 20, durationMs: 100 }),
    make({ id: 902, url: "https://example.test/unsplit", startedAt: base + 400, ttfbMs: null, durationMs: 80 }),
    make({ id: 903, url: "https://example.test/failed", startedAt: base + 900, ttfbMs: 15, durationMs: 60, status: 503, state: "failed" }),
    make({ id: 904, url: "https://example.test/pending", startedAt: base + 3000, ttfbMs: 10, durationMs: 0, status: null, state: "pending", error: null }),
  ];
}

function rpcError(code, message) {
  return {
    jsonrpc: "2.0",
    id: null,
    error: { code: -32000, message, data: { code } },
  };
}

function createFake(mode) {
  const calls = [];
  const recorders = new Map();
  for (const id of ["runtime-1", "runtime-2"]) {
    recorders.set(id, {
      records: initialRecords(id),
      // The recorder starts enabled in debug builds. The query reports it; the
      // "reports" mode starts the first runtime paused so the paused state can
      // only come from the query. Other modes omit the flag to prove the
      // controls still work against a controller that does not report it.
      enabled: !(mode === "reports" && id === "runtime-1"),
    });
  }
  let pendingToggle = null;
  let pendingClear = null;
  if (mode === "waterfall") {
    recorders.set("runtime-1", { records: waterfallRecords(), enabled: true });
  }

  function targets() {
    if (mode === "no-network") {
      return [target("runtime-1", "No Network", ["tree", "diagnostics"])];
    }
    if (mode === "no-control") {
      return [target("runtime-1", "Read Only Network", NO_CONTROL_CAPS)];
    }
    if (mode === "switch" || mode === "slow") {
      return [
        target("runtime-1", "Network One", RUNTIME_CAPS, 4242),
        target("runtime-2", "Network Two", RUNTIME_CAPS, 4243),
        target("runtime-3", "Plain Window", ["capture"], 4244, "window"),
      ];
    }
    return [target("runtime-1", "Network Target", RUNTIME_CAPS)];
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

  function recorder(id) {
    if (!recorders.has(id)) recorders.set(id, { records: [], enabled: true });
    return recorders.get(id);
  }

  async function handle(method, params = {}) {
    calls.push({ method, params });
    if (method === "get_ui_session") return session();
    if (method === "inspect_ui_tree") return treePayload();
    if (method === "query_ui_network") {
      const entry = recorder(params.target);
      const data = { records: entry.records.map(record => ({ ...record })), limit: LIMIT };
      if (mode === "reports") data.enabled = entry.enabled;
      return { ok: true, data };
    }
    if (method === "set_ui_network_recording") {
      if (mode === "unknown-tool") throw rpcError("unknown_command", "Unknown command set_ui_network_recording.");
      if (mode === "error") throw rpcError("probe_failed", "The recorder refused the pause.");
      if (mode === "reject") {
        return { ok: false, error: { code: "rejected", message: "The recorder refused the toggle." } };
      }
      if (mode === "slow") {
        await new Promise(resolve => { pendingToggle = resolve; });
        pendingToggle = null;
      }
      const entry = recorder(params.target);
      if (typeof params.enabled !== "boolean") {
        throw rpcError("invalid_request", "Network recording requires a boolean enabled flag.");
      }
      entry.enabled = params.enabled;
      return { ok: true, data: { enabled: entry.enabled } };
    }
    if (method === "clear_ui_network") {
      if (mode === "unknown-tool") throw rpcError("unknown_command", "Unknown command clear_ui_network.");
      if (mode === "error") throw rpcError("probe_failed", "The recorder refused the clear.");
      if (mode === "reject") {
        return { ok: false, error: { code: "rejected", message: "The recorder refused the clear." } };
      }
      if (mode === "slow") {
        await new Promise(resolve => { pendingClear = resolve; });
        pendingClear = null;
      }
      const entry = recorder(params.target);
      const count = entry.records.length;
      entry.records = [];
      return { ok: true, data: { cleared: count, limit: LIMIT } };
    }
    throw rpcError("unknown_command", `Unknown command ${method}.`);
  }

  function append(id, entry = request("GET", "https://example.test/new", 200)) {
    const recorderEntry = recorder(id);
    // The real recorder drops new requests while it is disabled; retained
    // records keep their order otherwise.
    if (!recorderEntry.enabled) return null;
    recorderEntry.records.push(entry);
    if (recorderEntry.records.length > LIMIT) recorderEntry.records.shift();
    return entry;
  }

  return {
    calls,
    recorders,
    handle,
    append,
    releaseToggle: () => pendingToggle?.(),
    releaseClear: () => pendingClear?.(),
    enabled: id => recorder(id).enabled,
    records: id => recorder(id).records.length,
    setCalls: () => calls.filter(call => call.method === "set_ui_network_recording"),
    clearCalls: () => calls.filter(call => call.method === "clear_ui_network"),
  };
}

async function startServer(mode) {
  const fake = createFake(mode);
  const server = createServer(async (req, res) => {
    const url = new URL(req.url, "http://127.0.0.1");
    if (req.method === "POST" && url.pathname === "/rpc") {
      const chunks = [];
      for await (const chunk of req) chunks.push(chunk);
      let message;
      try {
        message = JSON.parse(Buffer.concat(chunks).toString("utf8"));
      } catch {
        res.writeHead(400);
        res.end();
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
      res.writeHead(200, { "content-type": "application/json", "cache-control": "no-store" });
      res.end(JSON.stringify(reply));
      return;
    }
    if (req.method === "GET" && url.pathname === "/favicon.ico") {
      res.writeHead(204);
      res.end();
      return;
    }
    const entry = files[url.pathname];
    if (req.method === "GET" && entry) {
      const body = await readFile(join(clientDir, entry[0]));
      res.writeHead(200, { "content-type": entry[1], "cache-control": "no-store" });
      res.end(body);
      return;
    }
    res.writeHead(404);
    res.end();
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
    `--window-size=${process.env.NETWORK_WINDOW || "1440,900"}`,
    url,
  ], { stdio: ["ignore", "pipe", "pipe"] });
  let stderr = "";
  child.stderr.on("data", chunk => { stderr = (stderr + chunk).slice(-8000); });
  const deadline = Date.now() + 15000;
  let page = null;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json/list`);
      const targets = await response.json();
      page = targets.find(item => item.type === "page" && item.webSocketDebuggerUrl && item.url.startsWith("http://127.0.0.1"));
      if (page) break;
    } catch {}
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  if (!page) {
    child.kill("SIGKILL");
    throw new Error(`Chrome did not expose a page target.\n${stderr}`);
  }
  return {
    child,
    wsUrl: page.webSocketDebuggerUrl,
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
    const result = await this.send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
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

async function openNetwork(cdp, rows = 3) {
  await waitFor(cdp, "controller connection", `document.getElementById("connection-text")?.textContent === "Connected"`);
  await waitFor(cdp, "component tree", `document.querySelectorAll("#tree .tree-row").length === 3`);
  await click(cdp, "#tab-network");
  await waitFor(cdp, "network view", `document.getElementById("view-network").hidden === false`);
  await waitFor(cdp, `${rows} network rows`, `document.querySelectorAll("#network-content tbody tr").length === ${rows}`);
}

function controlScript() {
  return `(() => {
    const toggle = document.getElementById("network-recording-toggle");
    const clear = document.getElementById("network-clear");
    const content = document.getElementById("network-content");
    const badge = document.getElementById("network-badge");
    return {
      toggle: {
        disabled: toggle.disabled,
        text: toggle.textContent.trim(),
        title: toggle.title,
        busy: toggle.getAttribute("aria-busy"),
        icon: toggle.querySelector("use")?.getAttribute("href") || "",
      },
      clear: {
        disabled: clear.disabled,
        text: clear.textContent.trim(),
        title: clear.title,
        busy: clear.getAttribute("aria-busy"),
      },
      meta: document.getElementById("network-meta").textContent,
      rows: content.querySelectorAll("tbody tr").length,
      banner: document.getElementById("network-recording-status").hidden ? "" : document.getElementById("network-recording-status").textContent.trim(),
      bannerRole: document.getElementById("network-recording-status").getAttribute("role") || "",
      bannerHidden: document.getElementById("network-recording-status").hidden,
      note: content.querySelector(".network-note")?.textContent.trim() || "",
      stateTitle: content.querySelector(".state strong")?.textContent || "",
      stateMessage: content.querySelector(".state p")?.textContent || "",
      stateError: content.querySelector(".state.is-error") !== null,
      inlineError: [...content.querySelectorAll(".error-inline")].map(node => node.textContent).join(" | "),
      contentBusy: content.getAttribute("aria-busy"),
      badgeHidden: badge.hidden,
      badgeText: badge.textContent,
      follow: document.getElementById("network-follow").checked,
    };
  })()`;
}

async function controls(cdp) {
  return cdp.evaluate(controlScript());
}

async function waitToggleIdle(cdp) {
  await waitFor(cdp, "recording control settled", `(() => {
    const toggle = document.getElementById("network-recording-toggle");
    return toggle.disabled === false && !toggle.textContent.includes("…");
  })()`);
}

async function waitClearIdle(cdp) {
  await waitFor(cdp, "clear requested settled", `(() => {
    const button = document.getElementById("network-clear");
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
      for (const node of document.querySelectorAll("#view-network [" + attribute + "]")) {
        for (const id of (node.getAttribute(attribute) || "").split(/\\s+/).filter(Boolean)) {
          if (!document.getElementById(id)) brokenRefs.push(attribute + "=" + id);
        }
      }
    }
    if (brokenRefs.length) failures.push("broken references: " + [...new Set(brokenRefs)].join(", "));
    const symbols = new Set([...document.querySelectorAll("symbol[id]")].map(node => node.id));
    const brokenIcons = [...document.querySelectorAll("#view-network use[href^='#']")]
      .map(node => node.getAttribute("href").slice(1))
      .filter(id => !symbols.has(id));
    if (brokenIcons.length) failures.push("broken icon references: " + [...new Set(brokenIcons)].join(", "));
    const unlabeled = [];
    for (const node of document.querySelectorAll("#view-network button, #view-network input, #view-network select")) {
      const name = (node.getAttribute("aria-label") || node.getAttribute("title") || "").trim()
        || (node.id && document.querySelector('label[for="' + node.id + '"]')?.textContent.trim())
        || (node.closest("label")?.textContent.trim())
        || (node.tagName === "BUTTON" ? node.textContent.trim() : "");
      if (!name) unlabeled.push(node.id || node.className || node.tagName);
    }
    if (unlabeled.length) failures.push("unlabeled controls: " + unlabeled.join(", "));
    const untyped = [...document.querySelectorAll("#view-network button")].filter(button => !button.hasAttribute("type")).map(button => button.id || button.className || button.textContent.trim());
    if (untyped.length) failures.push("buttons without type: " + untyped.join(", "));
    const toggle = document.getElementById("network-recording-toggle");
    if (!toggle) failures.push("recording toggle is missing");
    else {
      if (toggle.getAttribute("type") !== "button") failures.push("recording toggle is not type=button");
      if (!toggle.title) failures.push("recording toggle has no title");
      if (!toggle.closest(".aux-actions")) failures.push("recording toggle is outside the network actions");
      if (toggle.closest("label")) failures.push("recording toggle is nested in a label");
      if (toggle.hasAttribute("aria-pressed")) failures.push("recording toggle fakes a pressed state");
    }
    const clear = document.getElementById("network-clear");
    if (!clear) failures.push("clear button is missing");
    else {
      if (clear.getAttribute("type") !== "button") failures.push("clear button is not type=button");
      if (!clear.title) failures.push("clear button has no title");
      if (!clear.closest(".aux-actions")) failures.push("clear button is outside the network actions");
      if (clear.closest("label")) failures.push("clear button is nested in a label");
      if (clear.hasAttribute("aria-pressed")) failures.push("clear button pretends to be a toggle");
    }
    const status = document.getElementById("network-recording-status");
    if (!status) failures.push("recording banner is missing");
    else {
      if (status.getAttribute("role") !== "status") failures.push("recording banner has no status role");
      if (!status.hidden) failures.push("recording banner is visible before a pause");
      if (!status.closest(".aux-view")) failures.push("recording banner is outside the network view");
      if (status.hasAttribute("aria-pressed")) failures.push("recording banner pretends to be a toggle");
    }
    const content = document.getElementById("network-content");
    if (!content) failures.push("network content is missing");
    else if (content.getAttribute("aria-busy") !== "false") failures.push("network content is not settled");
    if (!document.getElementById("network-follow")) failures.push("follow checkbox is missing");
    return failures;
  })()`);
  assert.deepEqual(report, [], `Network DOM audit failed:\n${report.join("\n")}`);
  console.log("PASS  DOM audit: unique ids, live network references, icon symbols, named typed controls, control attributes");
}

let audited = false;

async function runScenario(mode, body) {
  const server = await startServer(mode);
  const profile = await mkdtemp(join(tmpdir(), "cove-network-harness-"));
  let chrome = null;
  let cdp = null;
  try {
    chrome = await startChrome(profile, server.url);
    cdp = await connect(chrome.wsUrl);
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

function toast(cdp) {
  return cdp.evaluate(`({
    hidden: document.getElementById("toast").hidden,
    kind: document.getElementById("toast").className,
    text: document.getElementById("toast-text").textContent,
  })`);
}

async function scenarioPauseResumeClear() {
  await runScenario("success", async ({ cdp, fake }) => {
    await openNetwork(cdp);
    if (!audited) {
      await auditDom(cdp);
      audited = true;
    }
    const initial = await controls(cdp);
    assert.equal(initial.toggle.disabled, false);
    assert.equal(initial.toggle.text, "Pause");
    assert.equal(initial.toggle.title, "Pause recording new HTTP requests");
    assert.equal(initial.toggle.busy, "false");
    assert.equal(initial.toggle.icon, "#i-pause");
    assert.equal(initial.clear.disabled, false);
    assert.equal(initial.clear.text, "Clear");
    assert.equal(initial.clear.title, "Clear the runtime's retained network records");
    assert.equal(initial.banner, "", "no paused banner before a pause is confirmed");
    assert.equal(initial.bannerHidden, true);
    assert.equal(initial.rows, 3);
    assert.equal(initial.badgeHidden, false);
    assert.equal(initial.badgeText, "3");
    assert.match(initial.meta, /3 requests/);
    assert.match(initial.meta, /buffer 256/);

    // Pause. Existing records stay; the fake recorder stops retaining new ones.
    await click(cdp, "#network-recording-toggle");
    await waitFor(cdp, "paused banner", `document.getElementById("network-recording-status").hidden === false`);
    await waitToggleIdle(cdp);
    const paused = await controls(cdp);
    assert.equal(paused.toggle.text, "Resume");
    assert.equal(paused.toggle.title, "Resume recording new HTTP requests");
    assert.equal(paused.toggle.icon, "#i-play");
    assert.match(paused.banner, /Recording paused/);
    assert.match(paused.banner, /Existing requests are retained/);
    assert.equal(paused.bannerRole, "status");
    assert.equal(paused.bannerHidden, false);
    assert.equal(paused.rows, 3, "pausing keeps the retained table");
    assert.match(paused.meta, /3 requests/);
    assert.match(paused.meta, /recording paused/);
    assert.equal(paused.badgeText, "3");
    assert.deepEqual(fake.setCalls().map(call => call.params), [{ target: "runtime-1", enabled: false }]);
    assert.equal(fake.enabled("runtime-1"), false, "the fake recorder is paused");
    assert.equal(fake.append("runtime-1"), null, "a paused recorder retains no new request");
    await screenshot(cdp, process.env.NETWORK_PAUSED_SHOT);

    const queriesBeforePausedRefresh = fake.calls.filter(call => call.method === "query_ui_network").length;
    await click(cdp, "#network-refresh");
    await waitForCall(fake, "read while paused", calls => calls.filter(call => call.method === "query_ui_network").length > queriesBeforePausedRefresh);
    await new Promise(resolve => setTimeout(resolve, 200));
    const afterAppend = await controls(cdp);
    assert.equal(afterAppend.rows, 3, "requests issued while paused never reach the table");
    assert.match(afterAppend.banner, /Recording paused/);

    // Resume. The next request is retained and appears on refresh.
    await click(cdp, "#network-recording-toggle");
    await waitFor(cdp, "resumed toggle", `document.getElementById("network-recording-toggle").textContent.trim() === "Pause"`);
    await waitToggleIdle(cdp);
    const resumed = await controls(cdp);
    assert.equal(resumed.banner, "");
    assert.equal(resumed.bannerHidden, true);
    assert.equal(resumed.rows, 3);
    assert.equal(fake.enabled("runtime-1"), true);
    assert.deepEqual(fake.setCalls().map(call => call.params), [
      { target: "runtime-1", enabled: false },
      { target: "runtime-1", enabled: true },
    ]);
    const added = fake.append("runtime-1");
    assert.ok(added, "a resumed recorder retains new requests");
    const queriesBeforeResumedRefresh = fake.calls.filter(call => call.method === "query_ui_network").length;
    await click(cdp, "#network-refresh");
    await waitForCall(fake, "read after resume", calls => calls.filter(call => call.method === "query_ui_network").length > queriesBeforeResumedRefresh);
    await waitFor(cdp, "new recorded request", `document.querySelectorAll("#network-content tbody tr").length === 4`);
    const withNew = await controls(cdp);
    assert.equal(withNew.badgeText, "4");
    assert.match(withNew.meta, /4 requests/);

    // Clear empties the real retained window and the table only after the ack.
    await click(cdp, "#network-clear");
    await waitFor(cdp, "cleared empty state", `document.querySelector("#network-content .state strong")?.textContent === "Network history cleared"`);
    await waitClearIdle(cdp);
    const cleared = await controls(cdp);
    assert.equal(cleared.rows, 0);
    assert.equal(cleared.meta, "Cleared");
    assert.equal(cleared.stateMessage, "Requests issued after the clear will appear here.");
    assert.equal(cleared.stateError, false);
    assert.equal(cleared.badgeHidden, true);
    assert.equal(cleared.badgeText, "0");
    assert.equal(cleared.follow, false, "a clear preserves follow mode");
    assert.deepEqual(fake.clearCalls().map(call => call.params), [{ target: "runtime-1" }]);
    assert.equal(fake.records("runtime-1"), 0, "the fake recorder's retained window is cleared");
    assert.equal(fake.enabled("runtime-1"), true, "a clear does not change the enabled state");
    const clearedToast = await toast(cdp);
    assert.equal(clearedToast.hidden, false);
    assert.match(clearedToast.kind, /is-success/);
    assert.equal(clearedToast.text, "4 requests cleared.");
    await screenshot(cdp, process.env.NETWORK_SHOT);
  });
}

async function scenarioPrePaused() {
  await runScenario("reports", async ({ cdp, fake }) => {
    await openNetwork(cdp);
    // The query reports `enabled: false`; the client must show the paused state
    // without sending a control call of its own.
    await waitFor(cdp, "reported paused banner", `document.getElementById("network-recording-status").hidden === false`);
    const reported = await controls(cdp);
    assert.equal(reported.toggle.text, "Resume");
    assert.match(reported.banner, /Recording paused/);
    assert.equal(reported.rows, 3);
    assert.equal(fake.setCalls().length, 0, "the paused state came from the query, not a toggle");

    await click(cdp, "#network-clear");
    await waitFor(cdp, "cleared while paused", `document.querySelector("#network-content .state strong")?.textContent === "Recording paused"`);
    await waitClearIdle(cdp);
    const cleared = await controls(cdp);
    assert.equal(cleared.rows, 0);
    assert.equal(cleared.meta, "0 requests · recording paused");
    assert.match(cleared.banner, /Recording paused/);
    assert.equal(cleared.toggle.text, "Resume");
    assert.equal(fake.records("runtime-1"), 0);
    assert.equal(fake.enabled("runtime-1"), false, "clearing while paused leaves the recorder paused");
    const stateAction = await cdp.evaluate(`document.querySelector("#network-content .state button")?.textContent.trim() || ""`);
    assert.equal(stateAction, "Resume");

    await click(cdp, "#network-content .state button");
    await waitFor(cdp, "resume from paused empty state", `document.getElementById("network-recording-toggle").textContent.trim() === "Pause"`);
    await waitToggleIdle(cdp);
    const resumed = await controls(cdp);
    assert.equal(resumed.banner, "");
    assert.equal(resumed.bannerHidden, true);
    assert.equal(resumed.stateTitle, "Network history cleared");
    assert.equal(fake.enabled("runtime-1"), true);
    assert.deepEqual(fake.setCalls().map(call => call.params), [{ target: "runtime-1", enabled: true }]);

    fake.append("runtime-1");
    const queriesBeforeResumeRefresh = fake.calls.filter(call => call.method === "query_ui_network").length;
    await click(cdp, "#network-refresh");
    await waitForCall(fake, "read after resume", calls => calls.filter(call => call.method === "query_ui_network").length > queriesBeforeResumeRefresh);
    await waitFor(cdp, "post-resume record", `document.querySelectorAll("#network-content tbody tr").length === 1`);
    const after = await controls(cdp);
    assert.equal(after.badgeText, "1");
  });
}

async function scenarioRuntimeError() {
  await runScenario("error", async ({ cdp, fake }) => {
    await openNetwork(cdp);
    await click(cdp, "#network-recording-toggle");
    await waitFor(cdp, "pause error toast", `document.getElementById("toast").classList.contains("is-error") && document.getElementById("toast").hidden === false`);
    await waitToggleIdle(cdp);
    const pauseToast = await toast(cdp);
    assert.match(pauseToast.text, /Pause failed: The recorder refused the pause\./);
    const paused = await controls(cdp);
    assert.equal(paused.rows, 3, "a failed pause must keep the retained table");
    assert.equal(paused.banner, "", "a failed pause must not show a paused state");
    assert.equal(paused.toggle.text, "Pause", "a failed pause leaves the control state unchanged");
    assert.match(paused.inlineError, /Recording control failed: The recorder refused the pause\./);
    assert.equal(paused.toggle.disabled, false, "a transient failure leaves the control usable");
    assert.deepEqual(fake.setCalls().map(call => call.params), [{ target: "runtime-1", enabled: false }]);
    assert.equal(fake.enabled("runtime-1"), true, "the fake recorder stayed enabled");

    await click(cdp, "#network-clear");
    await waitFor(cdp, "clear error toast", `document.getElementById("toast").classList.contains("is-error")`);
    await waitClearIdle(cdp);
    const clearToast = await toast(cdp);
    assert.match(clearToast.text, /Clear failed: The recorder refused the clear\./);
    const cleared = await controls(cdp);
    assert.equal(cleared.rows, 3, "a failed clear must keep the retained table");
    assert.equal(cleared.meta === "Cleared", false);
    assert.match(cleared.inlineError, /Clear failed: The recorder refused the clear\./);
    assert.equal(cleared.badgeText, "3");
    assert.equal(fake.records("runtime-1"), 3);
  });
}

async function scenarioRejectedAcknowledgement() {
  await runScenario("reject", async ({ cdp, fake }) => {
    await openNetwork(cdp);
    await click(cdp, "#network-recording-toggle");
    await waitFor(cdp, "rejected toggle error", `document.querySelector("#network-content .error-inline")?.textContent.includes("The recorder refused the toggle.")`);
    await waitToggleIdle(cdp);
    const paused = await controls(cdp);
    assert.equal(paused.banner, "", "an ok=false acknowledgement must not show a paused state");
    assert.equal(paused.rows, 3);
    assert.equal(fake.enabled("runtime-1"), true);
    assert.equal(paused.toggle.disabled, false);

    await click(cdp, "#network-clear");
    await waitFor(cdp, "rejected clear error", `document.querySelector("#network-content .error-inline")?.textContent.includes("The recorder refused the clear.")`);
    await waitClearIdle(cdp);
    const cleared = await controls(cdp);
    assert.equal(cleared.rows, 3, "an ok=false acknowledgement must not clear the table");
    assert.equal(cleared.meta === "Cleared", false);
    assert.equal(fake.records("runtime-1"), 3);
  });
}

async function scenarioUnsupportedTool() {
  await runScenario("unknown-tool", async ({ cdp, fake }) => {
    await openNetwork(cdp);
    const before = await controls(cdp);
    assert.equal(before.toggle.disabled, false, "the capability is advertised, so the control starts usable");
    await click(cdp, "#network-recording-toggle");
    await waitFor(cdp, "unsupported control error", `document.querySelector("#network-content .error-inline")?.textContent.includes("Unknown command set_ui_network_recording.")`);
    await waitFor(cdp, "controls marked unavailable", `document.getElementById("network-recording-toggle").disabled === true`);
    const after = await controls(cdp);
    assert.equal(after.rows, 3);
    assert.equal(after.banner, "");
    assert.match(after.note, /does not expose network recording controls/);
    assert.equal(after.toggle.title, "This runtime does not expose network recording controls");
    assert.equal(after.clear.title, "This runtime does not expose network recording controls");
    assert.equal(after.clear.disabled, true);
    await click(cdp, "#network-recording-toggle");
    await click(cdp, "#network-clear");
    await new Promise(resolve => setTimeout(resolve, 250));
    assert.equal(fake.setCalls().length, 1, "an unavailable control sends no further RPC");
    assert.equal(fake.clearCalls().length, 0);
    assert.equal(fake.enabled("runtime-1"), true);
  });
}

async function scenarioTargetChangeDuringPause() {
  await runScenario("slow", async ({ cdp, fake }) => {
    await openNetwork(cdp);
    await click(cdp, "#network-recording-toggle");
    await waitForCall(fake, "set_ui_network_recording", calls => calls.some(call => call.method === "set_ui_network_recording"));
    await waitFor(cdp, "pause busy state", `(() => {
      const toggle = document.getElementById("network-recording-toggle");
      return toggle.disabled === true && toggle.textContent.trim() === "Pausing…";
    })()`);
    const busy = await controls(cdp);
    assert.equal(busy.toggle.busy, "true");
    assert.match(busy.meta, /3 requests/);

    await selectTarget(cdp, "runtime-2", `document.querySelectorAll("#network-content tbody tr").length === 2`);
    const switched = await controls(cdp);
    assert.equal(switched.rows, 2);
    assert.equal(switched.toggle.text, "Pause");
    assert.equal(switched.banner, "", "a target switch starts from an unknown recording state");
    assert.equal(switched.toggle.disabled, false);

    fake.releaseToggle();
    await waitUntil("the old toggle to reach the fake runtime", () => fake.enabled("runtime-1") === false);
    await new Promise(resolve => setTimeout(resolve, 350));
    const after = await controls(cdp);
    assert.equal(after.rows, 2, "an old pause acknowledgement must not touch the new target's table");
    assert.equal(after.banner, "", "an old pause acknowledgement must not pause the new target's view");
    assert.equal(after.toggle.text, "Pause");
    assert.equal(fake.enabled("runtime-2"), true, "the new target's recorder is untouched");
    assert.deepEqual(fake.setCalls().map(call => call.params), [{ target: "runtime-1", enabled: false }]);

    await selectTarget(cdp, "runtime-1", `document.querySelectorAll("#network-content tbody tr").length === 3`);
    const back = await controls(cdp);
    assert.equal(back.banner, "", "the client does not invent a paused state after switching back");
    assert.equal(back.toggle.text, "Pause");
    assert.equal(fake.enabled("runtime-1"), false);
  });
}

async function scenarioTargetChangeDuringClear() {
  await runScenario("slow", async ({ cdp, fake }) => {
    await openNetwork(cdp);
    await click(cdp, "#network-clear");
    await waitForCall(fake, "clear_ui_network", calls => calls.some(call => call.method === "clear_ui_network"));
    await waitFor(cdp, "clear busy state", `(() => {
      const button = document.getElementById("network-clear");
      return button.disabled === true && button.textContent.trim() === "Clearing…";
    })()`);
    const busy = await controls(cdp);
    assert.equal(busy.clear.busy, "true");
    assert.equal(busy.contentBusy, "true");
    assert.match(busy.meta, /Clearing…/);
    assert.equal(busy.rows, 3, "clearing keeps the table until the ack arrives");

    await selectTarget(cdp, "runtime-2", `document.querySelectorAll("#network-content tbody tr").length === 2`);
    const switched = await controls(cdp);
    assert.equal(switched.rows, 2);
    assert.equal(switched.clear.text, "Clear");
    assert.equal(switched.meta === "Cleared", false);

    fake.releaseClear();
    await waitUntil("the old clear to reach the fake runtime", () => fake.records("runtime-1") === 0);
    await new Promise(resolve => setTimeout(resolve, 350));
    const after = await controls(cdp);
    assert.equal(after.rows, 2, "an old clear acknowledgement must not clear the new target's table");
    assert.match(after.meta, /2 requests/);
    assert.equal(after.stateTitle, "");
    assert.equal(after.clear.text, "Clear");
    assert.deepEqual(fake.clearCalls().map(call => call.params), [{ target: "runtime-1" }]);
    assert.equal(fake.records("runtime-2"), 2);

    await selectTarget(cdp, "runtime-1", `document.querySelector("#network-content .state strong")?.textContent === "No HTTP activity"`);
    const back = await controls(cdp);
    assert.equal(back.rows, 0, "the first target's clear really removed its retained window");
    assert.equal(back.badgeHidden, true);
  });
}

async function scenarioDisconnect() {
  await runScenario("success", async ({ cdp, fake, server }) => {
    await openNetwork(cdp);
    await server.close();
    await click(cdp, "#network-recording-toggle");
    await waitFor(cdp, "offline connection", `document.getElementById("connection-text")?.textContent === "Offline"`);
    await waitFor(cdp, "control network error", `document.querySelector("#network-content .error-inline")?.textContent.includes("Could not reach the controller")`);
    const view = await controls(cdp);
    assert.equal(view.rows, 3, "a disconnected control must keep the existing table");
    assert.equal(view.banner, "");
    assert.equal(view.badgeText, "3");
    assert.equal(view.toggle.disabled, true, "recording control is unavailable while offline");
    assert.equal(view.clear.disabled, true);
    assert.equal(view.toggle.title, "Reconnect to the controller to control network recording");
    assert.equal(view.clear.title, "Reconnect to the controller to clear network records");
    assert.match(view.inlineError, /Recording control failed: Could not reach the controller/);
    const disconnected = await toast(cdp);
    assert.match(disconnected.kind, /is-error/);
    assert.match(disconnected.text, /Pause failed: Could not reach the controller/);
    assert.equal(fake.records("runtime-1"), 3);
  });
}

async function scenarioNoControlCapability() {
  await runScenario("no-control", async ({ cdp, fake }) => {
    await openNetwork(cdp);
    const view = await controls(cdp);
    assert.equal(view.rows, 3, "records still render without the control capability");
    assert.equal(view.toggle.disabled, true);
    assert.equal(view.clear.disabled, true);
    assert.equal(view.toggle.title, "This runtime does not expose network recording controls");
    assert.equal(view.clear.title, "This runtime does not expose network recording controls");
    assert.match(view.note, /does not advertise network recording controls/);
    assert.match(view.meta, /controls unavailable/);
    await click(cdp, "#network-recording-toggle");
    await click(cdp, "#network-clear");
    await new Promise(resolve => setTimeout(resolve, 250));
    assert.equal(fake.setCalls().length, 0);
    assert.equal(fake.clearCalls().length, 0);
  });
}

async function scenarioNoNetworkCapability() {
  await runScenario("no-network", async ({ cdp, fake }) => {
    await waitFor(cdp, "controller connection", `document.getElementById("connection-text")?.textContent === "Connected"`);
    await click(cdp, "#tab-network");
    await waitFor(cdp, "unavailable network", `document.querySelector("#network-content .state strong")?.textContent === "Network is unavailable"`);
    const view = await controls(cdp);
    assert.equal(view.rows, 0);
    assert.equal(view.toggle.disabled, true);
    assert.equal(view.clear.disabled, true);
    assert.equal(view.toggle.title, "Network records come from a Cove runtime. Select a runtime target");
    await click(cdp, "#network-recording-toggle");
    await new Promise(resolve => setTimeout(resolve, 200));
    assert.equal(fake.setCalls().length, 0);
  });
}

async function scenarioFollowAcrossClear() {
  await runScenario("success", async ({ cdp, fake }) => {
    await openNetwork(cdp);
    await click(cdp, "#network-follow");
    await waitFor(cdp, "follow enabled", `document.getElementById("network-follow").checked === true`);
    await click(cdp, "#network-clear");
    await waitFor(cdp, "cleared empty state", `document.querySelector("#network-content .state strong")?.textContent === "Network history cleared"`);
    await waitForCall(fake, "post-clear follow read", calls => {
      const index = calls.findIndex(call => call.method === "clear_ui_network");
      return index !== -1 && calls.slice(index + 1).some(call => call.method === "query_ui_network");
    });
    const cleared = await controls(cdp);
    assert.equal(cleared.rows, 0);
    assert.equal(cleared.follow, true, "clear must leave follow enabled");
    assert.equal(cleared.stateMessage, "Waiting for requests issued after the clear.");
    assert.equal(cleared.badgeHidden, true);

    fake.append("runtime-1");
    await waitFor(cdp, "followed request", `document.querySelectorAll("#network-content tbody tr").length === 1 && document.getElementById("network-content").textContent.includes("https://example.test/new")`, 9000);
    const followed = await controls(cdp);
    assert.equal(followed.follow, true);
    assert.match(followed.meta, /1 request/);
    assert.equal(followed.badgeText, "1");
    assert.equal(followed.rows, 1, "the cleared records must not come back");
  });
}

function waterfallScript() {
  return `(() => {
    const block = [...document.querySelectorAll("#network-content .panel-block")]
      .find(node => node.querySelector("h2")?.textContent === "Waterfall");
    if (!block) return null;
    const rows = [...block.querySelectorAll(".waterfall-row")].map(row => {
      const bar = row.querySelector(".waterfall-bar");
      return {
        id: row.dataset.request,
        url: row.querySelector(".waterfall-url")?.textContent || "",
        method: row.querySelector(".waterfall-method")?.textContent || "",
        duration: row.querySelector(".waterfall-duration")?.textContent || "",
        title: row.getAttribute("title") || "",
        aria: row.getAttribute("aria-label") || "",
        role: row.getAttribute("role") || "",
        tabindex: row.tabIndex,
        failed: row.classList.contains("is-failed"),
        barFailed: bar ? bar.classList.contains("is-failed") : false,
        barPending: bar ? bar.classList.contains("is-pending") : false,
        left: parseFloat(bar ? bar.style.left : "NaN"),
        width: parseFloat(bar ? bar.style.width : "NaN"),
        segments: [...(bar ? bar.querySelectorAll(".waterfall-seg") : [])].map(seg => ({
          kind: [...seg.classList].find(name => name.startsWith("is-")) || "",
          flex: parseFloat(seg.style.flex.split(" ")[0]),
          title: seg.getAttribute("title") || "",
        })),
      };
    });
    const track = block.querySelector(".waterfall-rows");
    return {
      meta: block.querySelector(".pane-meta")?.textContent || "",
      rows,
      ticks: [...block.querySelectorAll(".waterfall-tick")].map(node => ({ left: parseFloat(node.style.left), text: node.textContent })),
      legend: [...block.querySelectorAll(".perf-legend span")].map(node => node.textContent),
      notes: [...block.querySelectorAll(".perf-note")].map(node => node.textContent),
      role: track ? track.getAttribute("role") || "" : "",
      ariaLabel: track ? track.getAttribute("aria-label") || "" : "",
      tableRows: document.querySelectorAll("#network-content tbody tr").length,
      hasCanvas: block.querySelector("canvas") !== null,
    };
  })()`;
}

function close(a, b, tolerance, message) {
  assert.ok(Math.abs(a - b) <= tolerance, `${message}: ${a} vs ${b} (tolerance ${tolerance})`);
}

async function scenarioWaterfall() {
  await runScenario("waterfall", async ({ cdp }) => {
    await openNetwork(cdp, 4);
    const chart = await cdp.evaluate(waterfallScript());
    assert.ok(chart, "the waterfall panel is rendered above the table");
    assert.equal(chart.hasCanvas, false, "charts stay DOM/CSS, no canvas");
    assert.equal(chart.tableRows, 4, "the table keeps every record");
    assert.equal(chart.role, "listbox");
    assert.match(chart.ariaLabel, /4 requests/);
    assert.deepEqual(chart.rows.map(row => row.id), ["901", "902", "903", "904"], "rows run oldest to newest");
    assert.deepEqual(chart.rows.map(row => row.method), ["GET", "GET", "GET", "GET"]);
    assert.match(chart.meta, /4 requests/);
    assert.match(chart.meta, /span/);

    // The axis span is derived from two known start offsets, then every other
    // row's position is checked against it: the bars must be placed by the
    // recorder's timestamps, not by row index.
    close(chart.rows[0].left, 0, 0.001, "the first request starts at the axis origin");
    const span = (400 * 100) / chart.rows[1].left;
    assert.ok(span > 3000 && span < 20000, `derived span is plausible: ${span}`);
    close(chart.rows[2].left, (900 / span) * 100, 0.05, "the failed request is placed by its start time");
    close(chart.rows[3].left, (3000 / span) * 100, 0.05, "the pending request is placed by its start time");
    close(chart.rows[0].width, (100 / span) * 100, 0.05, "a completed bar spans its recorded duration");
    close(chart.rows[1].width, (80 / span) * 100, 0.05, "the unsplit bar spans its recorded duration");
    close(chart.rows[2].width, (60 / span) * 100, 0.05, "the failed bar spans its recorded duration");
    // The pending request is the right edge: it was still in flight at read time.
    close(chart.rows[3].left + chart.rows[3].width, 100, 0.05, "the in-flight bar ends at the read instant");
    assert.ok(chart.rows[3].width > (100 / span) * 100, "the in-flight bar is longer than its 10 ms head arrival");

    // Phase split comes from ttfbMs and durationMs only.
    const [firstWaiting, firstReceiving] = chart.rows[0].segments;
    assert.equal(firstWaiting.kind, "is-waiting");
    assert.equal(firstReceiving.kind, "is-receiving");
    close(firstWaiting.flex / firstReceiving.flex, 20 / 80, 0.01, "wait/receive split matches 20 ms of 100 ms");
    assert.match(firstWaiting.title, /20\.00 ms/);
    assert.match(firstReceiving.title, /80\.00 ms/);
    assert.equal(chart.rows[1].segments.length, 1, "no head arrival means one undivided bar");
    assert.equal(chart.rows[1].segments[0].kind, "is-unsplit");
    assert.match(chart.rows[1].segments[0].title, /did not capture a response-head arrival/);
    const [failedWaiting, failedReceiving] = chart.rows[2].segments;
    close(failedWaiting.flex / failedReceiving.flex, 15 / 45, 0.01, "the failed request keeps its real split");
    assert.equal(chart.rows[2].failed, true);
    assert.equal(chart.rows[2].barFailed, true);
    const [pendingWaiting, pendingInflight] = chart.rows[3].segments;
    assert.equal(pendingWaiting.kind, "is-waiting");
    assert.equal(pendingInflight.kind, "is-inflight");
    assert.equal(chart.rows[3].barPending, true);
    assert.match(chart.rows[3].duration, /^≥ /, "an in-flight duration is a lower bound, not a completion");
    assert.match(chart.rows[3].title, /in flight/);
    assert.match(chart.rows[3].aria, /head 10\.00 ms/);
    assert.match(chart.rows[0].duration, /^100\.0 ms$/);

    // Axis and honest provenance notes.
    assert.deepEqual(chart.ticks.map(tick => tick.left), [0, 25, 50, 75, 100]);
    close(parseFloat(chart.ticks[4].text), span, 1, "the last tick labels the real span");
    assert.equal(chart.ticks[0].text, "0 ms");
    assert.equal(chart.legend.length, 5);
    assert.ok(chart.notes.some(note => /no DNS, connect, TLS or queueing phases/.test(note)), "the missing phases are stated, not drawn");
    assert.ok(chart.notes.some(note => /lower bound on their duration/.test(note)), "the in-flight edge is explained");
    assert.ok(chart.notes.some(note => /wait\/receive split is not estimated/.test(note)), "the unsplit bar is explained");
    await screenshot(cdp, process.env.NETWORK_WATERFALL_ALL_SHOT);

    // A row opens the same detail dialog as the table.
    await click(cdp, '.waterfall-row[data-request="902"]');
    await waitFor(cdp, "waterfall detail dialog", `document.getElementById("network-dialog").open === true`);
    const dialogTitle = await cdp.evaluate(`document.getElementById("network-dialog-title").textContent`);
    assert.match(dialogTitle, /https:\/\/example\.test\/unsplit/);
    await click(cdp, "#network-dialog-close");
    await waitFor(cdp, "dialog closed", `document.getElementById("network-dialog").open === false`);

    // Keyboard traversal mirrors the frame chart.
    await cdp.evaluate(`document.querySelector('.waterfall-row[data-request="901"]').focus()`);
    await cdp.send("Input.dispatchKeyEvent", { type: "keyDown", key: "ArrowDown", code: "ArrowDown", windowsVirtualKeyCode: 40 });
    await cdp.send("Input.dispatchKeyEvent", { type: "keyUp", key: "ArrowDown", code: "ArrowDown", windowsVirtualKeyCode: 40 });
    const focused = await cdp.evaluate(`document.activeElement?.dataset?.request || ""`);
    assert.equal(focused, "902", "ArrowDown moves to the next request");
    const tabindexes = await cdp.evaluate(`[...document.querySelectorAll(".waterfall-row")].map(row => row.tabIndex)`);
    assert.deepEqual(tabindexes, [-1, 0, -1, -1], "only the focused row stays tabbable");

    // The toggle hides the chart without touching the table.
    await click(cdp, "#network-waterfall");
    await waitFor(cdp, "waterfall hidden", `document.querySelector("#network-content .waterfall") === null`);
    const hidden = await controls(cdp);
    assert.equal(hidden.rows, 4, "hiding the waterfall keeps the table");
    await click(cdp, "#network-waterfall");
    await waitFor(cdp, "waterfall restored", `document.querySelector("#network-content .waterfall") !== null`);

    // Filtering narrows the axis to the matching requests.
    await cdp.evaluate(`(() => {
      const select = document.getElementById("network-state-filter");
      select.value = "pending";
      select.dispatchEvent(new Event("change", { bubbles: true }));
    })()`);
    await waitFor(cdp, "filtered waterfall", `document.querySelectorAll("#network-content .waterfall-row").length === 1`);
    const filtered = await cdp.evaluate(waterfallScript());
    assert.equal(filtered.rows[0].id, "904");
    close(filtered.rows[0].left, 0, 0.001, "a single filtered request defines its own origin");
    close(filtered.rows[0].width, 100, 0.001, "a single in-flight request fills the axis");
    assert.equal(filtered.tableRows, 1);
    await screenshot(cdp, process.env.NETWORK_WATERFALL_SHOT);
  });
}

console.log("Network controls harness: fake controller + headless Chrome");
await scenarioPauseResumeClear();
console.log("PASS  pause/resume/clear: recorder toggled, paused table retained, new requests blocked live, clear empties the recorder and the table");
await scenarioPrePaused();
console.log("PASS  reported paused state: query's enabled flag drives the banner, clear keeps the recorder paused, resume records again");
await scenarioRuntimeError();
console.log("PASS  runtime error: table and recorder intact, inline and toast errors, control stays usable");
await scenarioRejectedAcknowledgement();
console.log("PASS  ok=false acknowledgement: neither table nor recorder state changes");
await scenarioUnsupportedTool();
console.log("PASS  unsupported tool: controls disabled with an explanation, no state invented, no further RPC");
await scenarioTargetChangeDuringPause();
console.log("PASS  target change during pause: old acknowledgement cannot pause the new target's view or recorder");
await scenarioTargetChangeDuringClear();
console.log("PASS  target change during clear: old acknowledgement cannot clear the new target's table");
await scenarioDisconnect();
console.log("PASS  disconnect: control fails visibly, table intact, controls disabled offline");
await scenarioNoControlCapability();
console.log("PASS  network_control absent: controls disabled with a note, no RPC sent");
await scenarioNoNetworkCapability();
console.log("PASS  network absent: view unavailable, controls disabled, no RPC sent");
await scenarioFollowAcrossClear();
console.log("PASS  follow: stays enabled across a clear, new requests appear, cleared rows stay gone");
await scenarioWaterfall();
console.log("PASS  waterfall: bars placed by recorded timestamps, real wait/receive split, in-flight lower bound, unsplit and failed rows, dialog, keyboard, toggle, filter");
console.log("RESULT PASS");
