#!/usr/bin/env node
// Fake-controller harness for the Inspector "Guidelines" debug-paint toggle.
// It serves the real index.html/inspector.js/inspector.css, answers /rpc with
// the controller's session/tree/detail shapes plus `set_ui_debug_paint`, and
// drives the page through Chrome DevTools Protocol. No Rust process starts and
// no debug-paint state is invented outside the fake controller's responses.
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import assert from "node:assert/strict";
import { serveClientAsset } from "./harness-server.mjs";

const chromePath = process.env.CHROME_PATH
  || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

function target(id, title, capabilities, pid = 4242) {
  return { id, title, platform: "macos", pid, kind: "runtime", capabilities };
}

const NODES = {
  root: { debugName: "AppRoot", viewTag: "gallery::App", layoutRole: "stack" },
  "node-1": { debugName: "Header", viewTag: "gallery::Header", layoutRole: "stack" },
  "node-2": { debugName: "ButtonPrimary", viewTag: "gallery::Button", semanticRole: "button" },
};

function treePayload() {
  return {
    root: {
      id: "root",
      ownerScopeID: "root",
      ...NODES.root,
      children: [
        { id: "node-1", ownerScopeID: "node-1", ...NODES["node-1"], children: [] },
        { id: "node-2", ownerScopeID: "node-2", ...NODES["node-2"], children: [] },
      ],
    },
  };
}

function detailFor(nodeId) {
  const node = NODES[nodeId];
  if (!node) return { ok: true, data: { items: [] } };
  return { ok: true, data: { items: [{ node: { id: nodeId, ownerScopeID: nodeId, ...node } }] } };
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
  const state = { contradicted: false, on: new Map() };

  function targets() {
    if (mode === "no-capability") {
      return [target("runtime-1", "Plain Target", ["tree", "diagnostics"])];
    }
    if (mode === "switch") {
      return [
        target("runtime-1", "Guidelines One", ["tree", "diagnostics", "debug_paint"], 4242),
        target("runtime-2", "Guidelines Two", ["tree", "diagnostics", "debug_paint"], 4243),
        target("runtime-3", "No Debug Paint", ["tree", "diagnostics"], 4244),
        { id: "window:4242:0", title: "Guidelines One Window", platform: "macos", pid: 4242, kind: "window", capabilities: ["capture"] },
      ];
    }
    return [target("runtime-1", "Guidelines Target", ["tree", "diagnostics", "debug_paint"])];
  }

  function session() {
    return {
      platform: "macos",
      permissions: { screen_capture: false, accessibility: false },
      targets: targets(),
    };
  }

  function handle(method, params = {}) {
    calls.push({ method, params });
    if (method === "get_ui_session") return session();
    if (method === "inspect_ui_tree") return treePayload();
    if (method === "query_ui_nodes") return detailFor(params.node_id);
    if (method === "select_ui_node") return { ok: true };
    if (method === "set_ui_debug_paint") {
      if (mode === "error") {
        throw rpcError("unsupported", "The runtime has no debug paint state.");
      }
      // The runtime answers with the state it applied, not the requested one.
      if (mode === "contradict" && params.guidelines === true && !state.contradicted) {
        state.contradicted = true;
        state.on.set(params.target, false);
        return { guidelines: false };
      }
      state.on.set(params.target, params.guidelines === true);
      // The controller returns the applied DebugPaint state directly.
      return { guidelines: params.guidelines === true };
    }
    throw rpcError("unknown_command", `Unknown command ${method}.`);
  }

  return { calls, state, handle };
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
    if (request.method === "GET" && await serveClientAsset(response, url.pathname)) return;
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
    `--window-size=${process.env.GUIDELINES_WINDOW || "1440,900"}`,
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

async function click(cdp, selector) {
  const clicked = await cdp.evaluate(`(() => {
    const node = document.querySelector(${JSON.stringify(selector)});
    if (!node) return false;
    node.click();
    return true;
  })()`);
  assert.equal(clicked, true, `click ${selector}`);
}

async function openInspector(cdp) {
  await waitFor(cdp, "controller connection", `document.getElementById("connection-text")?.textContent === "Connected"`);
  await waitFor(cdp, "component tree", `document.querySelectorAll("#tree .tree-row").length === 3`);
  await waitFor(cdp, "guidelines button", `document.getElementById("guidelines-button") !== null`);
}

async function guidelines(cdp) {
  return cdp.evaluate(`(() => {
    const button = document.getElementById("guidelines-button");
    return {
      disabled: button.disabled,
      pressed: button.getAttribute("aria-pressed"),
      active: button.classList.contains("is-active"),
      title: button.title,
      label: button.getAttribute("aria-label"),
      text: button.textContent.trim(),
    };
  })()`);
}

async function toggle(cdp) {
  await click(cdp, "#guidelines-button");
}

async function waitIdle(cdp) {
  await waitFor(cdp, "guidelines request settled", `document.getElementById("guidelines-button").disabled === false`);
}

async function auditDom(cdp) {
  const report = await cdp.evaluate(`(() => {
    const failures = [];
    const ids = [...document.querySelectorAll("[id]")].map(node => node.id);
    const duplicates = ids.filter((id, index) => ids.indexOf(id) !== index);
    if (duplicates.length) failures.push("duplicate ids: " + [...new Set(duplicates)].join(", "));
    const brokenRefs = [];
    for (const attribute of ["aria-controls", "aria-labelledby", "aria-describedby"]) {
      for (const node of document.querySelectorAll("#view-inspector [" + attribute + "]")) {
        for (const id of (node.getAttribute(attribute) || "").split(/\\s+/).filter(Boolean)) {
          if (!document.getElementById(id)) brokenRefs.push(attribute + "=" + id);
        }
      }
    }
    if (brokenRefs.length) failures.push("broken references: " + [...new Set(brokenRefs)].join(", "));
    const symbols = new Set([...document.querySelectorAll("symbol[id]")].map(node => node.id));
    const brokenIcons = [...document.querySelectorAll("#view-inspector use[href^='#']")]
      .map(node => node.getAttribute("href").slice(1))
      .filter(id => !symbols.has(id));
    if (brokenIcons.length) failures.push("broken icon references: " + [...new Set(brokenIcons)].join(", "));
    const unlabeled = [];
    for (const node of document.querySelectorAll("#view-inspector button, #view-inspector input, #view-inspector select")) {
      const name = (node.getAttribute("aria-label") || node.getAttribute("title") || "").trim()
        || (node.id && document.querySelector('label[for="' + node.id + '"]')?.textContent.trim())
        || (node.closest("label")?.textContent.trim())
        || (node.tagName === "BUTTON" ? node.textContent.trim() : "");
      if (!name) unlabeled.push(node.id || node.className || node.tagName);
    }
    if (unlabeled.length) failures.push("unlabeled controls: " + unlabeled.join(", "));
    const untyped = [...document.querySelectorAll("#view-inspector button")].filter(button => !button.hasAttribute("type")).map(button => button.id || button.className || button.textContent.trim());
    if (untyped.length) failures.push("buttons without type: " + untyped.join(", "));
    const guidelines = document.getElementById("guidelines-button");
    if (!guidelines) failures.push("guidelines button is missing");
    else {
      if (!guidelines.hasAttribute("aria-pressed")) failures.push("guidelines button has no aria-pressed");
      if (!guidelines.title) failures.push("guidelines button has no title");
      if (guidelines.previousElementSibling?.classList.contains("filter-group") === true) failures.push("guidelines button is misplaced");
    }
    return failures;
  })()`);
  assert.deepEqual(report, [], `Inspector DOM audit failed:\n${report.join("\n")}`);
  console.log("PASS  DOM audit: unique ids, live inspector references, icon symbols, named typed controls, guidelines toggle attributes");
}

let audited = false;

async function runScenario(mode, body) {
  const server = await startServer(mode);
  const profile = await mkdtemp(join(tmpdir(), "cove-guidelines-harness-"));
  let chrome = null;
  let cdp = null;
  try {
    chrome = await startChrome(profile, server.url);
    cdp = await connect(chrome.wsUrl);
    await openInspector(cdp);
    await body({ cdp, fake: server.fake, server });
    assert.deepEqual(cdp.exceptions, [], `page exceptions: ${cdp.exceptions.join("\n")}`);
  } finally {
    cdp?.close();
    await chrome?.close();
    await server.close();
    await rm(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
  }
}

async function screenshot(cdp, path) {
  if (!path) return;
  const shot = await cdp.send("Page.captureScreenshot", { format: "png" });
  await writeFile(path, Buffer.from(shot.data, "base64"));
}

function debugPaintCalls(fake) {
  return fake.calls.filter(call => call.method === "set_ui_debug_paint");
}

async function scenarioNoCapability() {
  await runScenario("no-capability", async ({ cdp, fake }) => {
    if (!audited) {
      await auditDom(cdp);
      audited = true;
    }
    const view = await guidelines(cdp);
    assert.equal(view.label, "Layout guidelines");
    assert.equal(view.text, "Guidelines");
    assert.equal(view.disabled, true, "guidelines must be disabled without debug_paint");
    assert.equal(view.pressed, "false");
    assert.equal(view.active, false);
    assert.equal(view.title, "This runtime does not advertise debug_paint");
    await toggle(cdp);
    await new Promise(resolve => setTimeout(resolve, 250));
    assert.equal(debugPaintCalls(fake).length, 0, "a disabled toggle must not send set_ui_debug_paint");
    assert.equal((await guidelines(cdp)).pressed, "false");
    assert.equal(await cdp.evaluate(`document.querySelectorAll("#tree .tree-row").length`), 3);
  });
}

async function scenarioEnableDisable() {
  await runScenario("capable", async ({ cdp, fake }) => {
    const initial = await guidelines(cdp);
    assert.equal(initial.disabled, false);
    assert.equal(initial.pressed, "false");
    assert.equal(initial.active, false);
    assert.equal(initial.title, "Show layout guidelines on the target");

    await toggle(cdp);
    await waitFor(cdp, "guidelines enabled", `document.getElementById("guidelines-button").getAttribute("aria-pressed") === "true"`);
    await waitIdle(cdp);
    assert.deepEqual(debugPaintCalls(fake).map(call => call.params), [{ target: "runtime-1", guidelines: true }]);
    assert.deepEqual(fake.state.on.get("runtime-1"), true);
    const on = await guidelines(cdp);
    assert.equal(on.active, true);
    assert.equal(on.title, "Hide layout guidelines on the target");
    await screenshot(cdp, process.env.GUIDELINES_SHOT);

    // The existing tree/details/selection path keeps working while the toggle is on.
    await click(cdp, `#tree .tree-row[data-id="node-2"]`);
    await waitFor(cdp, "selected node", `document.getElementById("node-id")?.textContent === "node-2"`);
    await waitFor(cdp, "node details", `document.getElementById("details-heading")?.textContent === "ButtonPrimary"`);
    await waitForCall(fake, "select_ui_node for node-2", calls => calls.some(call => call.method === "select_ui_node" && call.params.node_id === "node-2"));
    assert.equal((await guidelines(cdp)).pressed, "true", "selection must not change the guidelines state");

    await toggle(cdp);
    await waitFor(cdp, "guidelines disabled", `document.getElementById("guidelines-button").getAttribute("aria-pressed") === "false"`);
    await waitIdle(cdp);
    assert.deepEqual(debugPaintCalls(fake).map(call => call.params), [
      { target: "runtime-1", guidelines: true },
      { target: "runtime-1", guidelines: false },
    ]);
    assert.equal(fake.state.on.get("runtime-1"), false);
    const off = await guidelines(cdp);
    assert.equal(off.active, false);
    assert.equal(off.title, "Show layout guidelines on the target");
  });
}

async function scenarioRuntimeContradicts() {
  await runScenario("contradict", async ({ cdp, fake }) => {
    await toggle(cdp);
    await waitForCall(fake, "first debug paint request", calls => calls.filter(call => call.method === "set_ui_debug_paint").length === 1);
    await waitIdle(cdp);
    const afterFirst = await guidelines(cdp);
    assert.equal(afterFirst.pressed, "false", "the toggle must follow the runtime response, not the request");
    assert.equal(afterFirst.active, false);

    await toggle(cdp);
    await waitFor(cdp, "guidelines enabled", `document.getElementById("guidelines-button").getAttribute("aria-pressed") === "true"`);
    await waitIdle(cdp);
    assert.deepEqual(debugPaintCalls(fake).map(call => call.params), [
      { target: "runtime-1", guidelines: true },
      { target: "runtime-1", guidelines: true },
    ]);
  });
}

async function scenarioError() {
  await runScenario("error", async ({ cdp, fake }) => {
    await toggle(cdp);
    await waitFor(cdp, "guidelines error toast", `document.getElementById("toast").classList.contains("is-error") && document.getElementById("toast").hidden === false`);
    await waitIdle(cdp);
    const text = await cdp.evaluate(`document.getElementById("toast-text").textContent`);
    assert.match(text, /Guidelines failed: The runtime has no debug paint state\. \(unsupported\)/);
    const view = await guidelines(cdp);
    assert.equal(view.pressed, "false", "a failed response must leave the toggle off");
    assert.equal(view.active, false);
    assert.equal(view.title, "Show layout guidelines on the target");
    assert.deepEqual(debugPaintCalls(fake).map(call => call.params), [{ target: "runtime-1", guidelines: true }]);
    assert.equal(fake.state.on.get("runtime-1"), undefined);
    assert.equal(await cdp.evaluate(`document.querySelectorAll("#tree .tree-row").length`), 3);
  });
}

async function selectTarget(cdp, id, ready = `document.querySelectorAll("#tree .tree-row").length === 3`) {
  const value = await cdp.evaluate(`(() => {
    const select = document.getElementById("targets");
    select.value = ${JSON.stringify(id)};
    select.dispatchEvent(new Event("change", { bubbles: true }));
    return select.value;
  })()`);
  assert.equal(value, id, `select target ${id}`);
  await waitFor(cdp, `target ${id}`, `(document.getElementById("targets").value === ${JSON.stringify(id)}) && (${ready})`);
}

async function scenarioTargetSwitch() {
  await runScenario("switch", async ({ cdp, fake }) => {
    await toggle(cdp);
    await waitFor(cdp, "guidelines enabled on runtime-1", `document.getElementById("guidelines-button").getAttribute("aria-pressed") === "true"`);
    await waitIdle(cdp);

    await selectTarget(cdp, "runtime-2");
    await waitIdle(cdp);
    await waitForCall(fake, "release on runtime-1", calls => calls.some(call =>
      call.method === "set_ui_debug_paint" && call.params.target === "runtime-1" && call.params.guidelines === false));
    const switched = await guidelines(cdp);
    assert.equal(switched.pressed, "false", "target switch must reset guidelines to off");
    assert.equal(switched.active, false);
    assert.equal(switched.disabled, false, "runtime-2 advertises debug_paint");
    assert.equal(fake.state.on.get("runtime-1"), false);

    await toggle(cdp);
    await waitFor(cdp, "guidelines enabled on runtime-2", `document.getElementById("guidelines-button").getAttribute("aria-pressed") === "true"`);
    await waitIdle(cdp);
    assert.deepEqual(debugPaintCalls(fake).map(call => call.params), [
      { target: "runtime-1", guidelines: true },
      { target: "runtime-1", guidelines: false },
      { target: "runtime-2", guidelines: true },
    ]);

    await selectTarget(cdp, "runtime-3");
    await waitForCall(fake, "release on runtime-2", calls => calls.some(call =>
      call.method === "set_ui_debug_paint" && call.params.target === "runtime-2" && call.params.guidelines === false));
    const plain = await guidelines(cdp);
    assert.equal(plain.pressed, "false");
    assert.equal(plain.active, false);
    assert.equal(plain.disabled, true, "runtime-3 does not advertise debug_paint");
    assert.equal(plain.title, "This runtime does not advertise debug_paint");
    assert.equal(fake.state.on.get("runtime-2"), false);

    await selectTarget(cdp, "window:4242:0", `document.getElementById("tree").hidden === true`);
    const platform = await guidelines(cdp);
    assert.equal(platform.disabled, true, "a platform window target cannot toggle guidelines");
    assert.equal(platform.pressed, "false");
    assert.equal(platform.title, "Layout guidelines need a Cove runtime target");
  });
}

async function scenarioDisconnect() {
  await runScenario("capable", async ({ cdp, fake, server }) => {
    await toggle(cdp);
    await waitFor(cdp, "guidelines enabled", `document.getElementById("guidelines-button").getAttribute("aria-pressed") === "true"`);
    await waitIdle(cdp);
    assert.equal(fake.state.on.get("runtime-1"), true);

    // The controller is gone, so the release attempt can only be observed in the
    // page: wrap fetch and record every JSON-RPC body before the server closes.
    await cdp.evaluate(`(() => {
      window.__guidelineRpcAttempts = [];
      const original = window.fetch;
      window.fetch = (input, init) => {
        try {
          const body = JSON.parse(init?.body || "null");
          if (body?.method) window.__guidelineRpcAttempts.push({ method: body.method, params: body.params });
        } catch {}
        return original(input, init);
      };
      return true;
    })()`);
    await server.close();
    await click(cdp, "#refresh");
    await waitFor(cdp, "offline connection", `document.getElementById("connection-text")?.textContent === "Offline"`);
    const view = await guidelines(cdp);
    assert.equal(view.pressed, "false", "disconnect must reset the UI to off");
    assert.equal(view.active, false);
    assert.equal(view.disabled, true);
    assert.equal(view.title, "Reconnect to the controller to toggle layout guidelines");
    const attempts = await cdp.evaluate(`window.__guidelineRpcAttempts.filter(call => call.method === "set_ui_debug_paint")`);
    assert.deepEqual(attempts.at(-1)?.params, { target: "runtime-1", guidelines: false },
      "disconnect must attempt to release the previous target");
  });
}

console.log("Inspector guidelines harness: fake controller + headless Chrome");
await scenarioNoCapability();
console.log("PASS  capability absent: toggle disabled, no RPC, no state change");
await scenarioEnableDisable();
console.log("PASS  enable/disable: set_ui_debug_paint params exact, active state follows the response, tree/details/selection intact");
await scenarioRuntimeContradicts();
console.log("PASS  response-driven: a runtime that answers guidelines=false keeps the toggle off");
await scenarioError();
console.log("PASS  error response: toggle stays off, error toast shown, no active state");
await scenarioTargetSwitch();
console.log("PASS  target switch: old target released with guidelines=false, new target off; capability loss disables");
await scenarioDisconnect();
console.log("PASS  disconnect: UI reset off and release sent for the previous target");
console.log("RESULT PASS");
