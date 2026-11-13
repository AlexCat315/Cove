#!/usr/bin/env node
// Fake-controller harness for the Inspector Layout panel's Flex Explorer.
// It serves the real index.html/inspector.js/inspector.css, answers /rpc with
// the controller's session/tree/detail shapes plus `edit_ui_style`, and owns
// the node records. The page can only show what `query_ui_nodes` returns, so a
// requested override that the record never adopts must stay invisible.
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import assert from "node:assert/strict";
import { serveClientAsset } from "./harness-server.mjs";

const chromePath = process.env.CHROME_PATH
  || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

// The names the runtime's layout record publishes: the LayoutNode Justify and
// Align variants. The harness asserts the client offers exactly these.
const JUSTIFY = ["FlexStart", "Center", "FlexEnd", "SpaceBetween", "SpaceAround", "SpaceEvenly"];
const ALIGN = ["Auto", "FlexStart", "Center", "FlexEnd", "Stretch", "Baseline", "SpaceBetween", "SpaceAround", "SpaceEvenly"];
const FLEX_PROPERTIES = ["flexGrow", "flexShrink", "justifyContent", "alignItems"];
const ENUMS = { justifyContent: JUSTIFY, alignItems: ALIGN };
const EDIT_DELAY_MS = 200;

const META = {
  root: { debugName: "AppRoot", viewTag: "gallery::App", layoutRole: "stack" },
  "node-1": { debugName: "Header", viewTag: "gallery::Header", layoutRole: "stack" },
  "node-2": { debugName: "ButtonPrimary", viewTag: "gallery::Button", semanticRole: "button" },
};

const ORIGINAL = {
  "node-1": { flexGrow: 1, flexShrink: 1, justifyContent: "Center", alignItems: "FlexEnd" },
  "node-2": { flexGrow: 0, flexShrink: 1, justifyContent: "FlexStart", alignItems: "Stretch" },
};

const FRAME = { x: 12, y: 24, w: 160, h: 48 };

function target(id, title, capabilities, pid = 4242) {
  return { id, title, platform: "macos", pid, kind: "runtime", capabilities };
}

function treePayload() {
  return {
    root: {
      id: "root",
      ownerScopeID: "root",
      ...META.root,
      children: [
        { id: "node-1", ownerScopeID: "node-1", ...META["node-1"], children: [] },
        { id: "node-2", ownerScopeID: "node-2", ...META["node-2"], children: [] },
      ],
    },
  };
}

function layoutFor(values) {
  return {
    padding: { top: 4, right: 4, bottom: 4, left: 4 },
    margin: { top: 0, right: 0, bottom: 0, left: 0 },
    border: { top: 0, right: 0, bottom: 0, left: 0 },
    contentWidth: 120,
    contentHeight: 40,
    direction: "Ltr",
    flexDirection: "Column",
    justifyContent: values.justifyContent,
    alignItems: values.alignItems,
    flexGrow: values.flexGrow,
    flexShrink: values.flexShrink,
  };
}

function nodeRecord(nodeId, entry) {
  return {
    id: nodeId,
    ownerScopeID: nodeId,
    ...META[nodeId],
    frame: { ...FRAME },
    absoluteFrame: { x: 40, y: 60, w: FRAME.w, h: FRAME.h },
    layout: entry ? layoutFor(entry.values) : null,
    style: {
      backgroundColor: null,
      foregroundColor: null,
      overrides: entry ? [...entry.overrides] : [],
    },
    flags: { hitTestable: true, focusable: nodeId !== "root", clipsToBounds: true, hasBackground: false, hasBorder: false },
    presentation: { isVisible: true, inputEnabled: true, visibleFrame: { ...FRAME } },
  };
}

function detailFor(nodeId, nodes) {
  const entry = nodes.get(nodeId);
  if (!entry) return { ok: true, data: { items: [] } };
  return { ok: true, data: { items: [{ node: nodeRecord(nodeId, entry) }] } };
}

function inspectionState(nodeId, nodes) {
  return {
    selectedID: nodeId,
    canUndo: true,
    canRedo: false,
    overrideCount: [...nodes.values()].filter(entry => entry.overrides.length).length,
  };
}

function rpcError(code, message) {
  return {
    jsonrpc: "2.0",
    id: null,
    error: { code: -32000, message, data: { code } },
  };
}

function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

function createFake(mode) {
  const calls = [];
  const nodes = new Map();
  for (const [nodeId, values] of Object.entries(ORIGINAL)) {
    nodes.set(nodeId, { values: { ...values }, overrides: [] });
  }

  function session() {
    const capabilities = mode === "no-style" ? ["tree", "diagnostics"] : ["tree", "diagnostics", "style"];
    return {
      platform: "macos",
      permissions: { screen_capture: false, accessibility: false },
      targets: [target("runtime-1", "Flex Target", capabilities)],
    };
  }

  async function edit(command, args) {
    // Mirror the protocol checks so a malformed client request fails loudly.
    if (!args || typeof args !== "object" || Array.isArray(args)) {
      throw rpcError("bad_request", "Expected node id and supported properties");
    }
    if (command === "inspect.style.set") {
      if (Object.keys(args).length !== 2 || !("id" in args) || !("properties" in args)) {
        throw rpcError("bad_request", "Expected node id and supported properties");
      }
      const entry = nodes.get(args.id);
      if (!entry) throw rpcError("stale_node", "Selected node is no longer in the scene");
      const properties = args.properties;
      if (!properties || typeof properties !== "object" || Array.isArray(properties) || Object.keys(properties).length === 0) {
        throw rpcError("bad_request", "Expected node id and supported properties");
      }
      for (const [name, value] of Object.entries(properties)) {
        if (!FLEX_PROPERTIES.includes(name)) throw rpcError("bad_request", "Unsupported style property");
        if (name === "flexGrow" || name === "flexShrink") {
          if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > 1000) {
            throw rpcError("bad_request", "Flex factors require a finite number in 0..1000");
          }
        } else if (!ENUMS[name].includes(value)) {
          throw rpcError("bad_request", `${name} requires a LayoutNode enum name`);
        }
      }
      await sleep(EDIT_DELAY_MS);
      if (mode === "error") throw rpcError("unsupported", "The runtime rejected the flex override.");
      if (mode === "stale") throw rpcError("stale_node", "Selected node is no longer in the scene");
      // The runtime invalidates layout; the record follows on its next pass.
      // "lag" keeps the old record to prove the client does not echo its request.
      if (mode !== "lag") Object.assign(entry.values, properties);
      for (const name of Object.keys(properties)) {
        if (!entry.overrides.includes(name)) entry.overrides.push(name);
      }
      return inspectionState(args.id, nodes);
    }
    if (command === "inspect.style.clear") {
      if (Object.keys(args).length !== 1 || !("id" in args)) {
        throw rpcError("bad_request", "Expected node id");
      }
      const entry = nodes.get(args.id);
      if (!entry) throw rpcError("stale_node", "Selected node is no longer in the scene");
      await sleep(EDIT_DELAY_MS);
      if (mode === "error") throw rpcError("unsupported", "The runtime rejected the flex override.");
      if (mode === "stale") throw rpcError("stale_node", "Selected node is no longer in the scene");
      entry.values = { ...ORIGINAL[args.id] };
      entry.overrides = [];
      return inspectionState(args.id, nodes);
    }
    throw rpcError("unknown_command", "Unknown inspection command.");
  }

  async function handle(method, params = {}) {
    calls.push({ method, params });
    if (method === "get_ui_session") return session();
    if (method === "inspect_ui_tree") return treePayload();
    if (method === "query_ui_nodes") return detailFor(params.node_id, nodes);
    if (method === "select_ui_node") return { ok: true };
    if (method === "edit_ui_style") return edit(params.command, params.arguments);
    throw rpcError("unknown_command", `Unknown command ${method}.`);
  }

  return { calls, nodes, handle };
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
    `--window-size=${process.env.FLEX_WINDOW || "1440,900"}`,
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
    await sleep(100);
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
    await sleep(50);
  }
  throw new Error(`Timed out waiting for ${description}. Last value: ${JSON.stringify(last)}`);
}

async function waitForCall(fake, description, predicate, timeout = 8000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (predicate(fake.calls)) return fake.calls;
    await sleep(40);
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

const FIELD_IDS = {
  flexGrow: "flex-grow",
  flexShrink: "flex-shrink",
  justifyContent: "flex-justify",
  alignItems: "flex-align",
};

async function openInspector(cdp) {
  await waitFor(cdp, "controller connection", `document.getElementById("connection-text")?.textContent === "Connected"`);
  await waitFor(cdp, "component tree", `document.querySelectorAll("#tree .tree-row").length === 3`);
  await click(cdp, "#tab-layout");
  await waitFor(cdp, "layout tab", `document.getElementById("panel-layout").hidden === false`);
}

async function selectNode(cdp, id) {
  await click(cdp, `#tree .tree-row[data-id="${id}"]`);
  await waitFor(cdp, "selected node", `document.getElementById("node-id")?.textContent === ${JSON.stringify(id)}`);
}

async function waitForFlexFields(cdp) {
  await waitFor(cdp, "flex editor", `document.getElementById("flex-editor") !== null`);
}

async function flexForm(cdp) {
  return cdp.evaluate(`(() => {
    const read = id => {
      const node = document.getElementById(id);
      if (!node) return null;
      return {
        value: node.value,
        disabled: node.disabled,
        invalid: node.getAttribute("aria-invalid"),
        options: node.tagName === "SELECT" ? [...node.options].map(option => option.value) : null,
      };
    };
    const apply = document.getElementById("flex-apply");
    const reset = document.getElementById("flex-reset");
    const status = document.getElementById("flex-status");
    return {
      grow: read("flex-grow"),
      shrink: read("flex-shrink"),
      justify: read("flex-justify"),
      align: read("flex-align"),
      apply: apply ? { disabled: apply.disabled, type: apply.type } : null,
      reset: reset ? { disabled: reset.disabled, type: reset.type } : null,
      status: status?.textContent || "",
      statusRole: status?.getAttribute("role") || "",
      statusClass: status?.className || "",
      dirty: [...document.querySelectorAll("#panel-layout .flex-field.is-dirty")].map(node => node.dataset.flexField),
    };
  })()`);
}

async function setField(cdp, name, value) {
  const id = FIELD_IDS[name];
  const applied = await cdp.evaluate(`(() => {
    const node = document.getElementById(${JSON.stringify(id)});
    if (!node) return null;
    node.value = ${JSON.stringify(value)};
    node.dispatchEvent(new Event("input", { bubbles: true }));
    node.dispatchEvent(new Event("change", { bubbles: true }));
    return node.value;
  })()`);
  assert.equal(applied, value, `${id} accepted ${JSON.stringify(value)}`);
}

function editCalls(fake) {
  return fake.calls.filter(call => call.method === "edit_ui_style");
}

function assertRecord(fake, nodeId, expected) {
  const entry = fake.nodes.get(nodeId);
  assert.ok(entry, `${nodeId} exists in the fake runtime`);
  for (const [name, value] of Object.entries(expected)) {
    assert.equal(entry.values[name], value, `${nodeId} ${name} in the runtime record`);
  }
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

    const editor = document.getElementById("flex-editor");
    if (!editor) failures.push("flex editor is missing");
    else if (editor.closest("#panel-layout") === null) failures.push("flex editor is outside the layout panel");
    for (const name of ["flexGrow", "flexShrink", "justifyContent", "alignItems"]) {
      const id = { flexGrow: "flex-grow", flexShrink: "flex-shrink", justifyContent: "flex-justify", alignItems: "flex-align" }[name];
      const control = document.getElementById(id);
      if (!control) { failures.push("missing control " + id); continue; }
      if (control.closest("label[for='" + id + "']") === null) failures.push(id + " is not associated with its label");
      if (!control.getAttribute("aria-label")) failures.push(id + " has no aria-label");
    }
    const grow = document.getElementById("flex-grow");
    if (grow && grow.type !== "number") failures.push("flex grow is not a number input");
    if (grow && (grow.getAttribute("min") !== "0" || grow.getAttribute("max") !== "1000")) failures.push("flex grow limits are not 0..1000");
    const justify = document.getElementById("flex-justify");
    const align = document.getElementById("flex-align");
    const options = node => [...node.options].map(option => option.value);
    const expectedJustify = ["FlexStart", "Center", "FlexEnd", "SpaceBetween", "SpaceAround", "SpaceEvenly"];
    const expectedAlign = ["Auto", "FlexStart", "Center", "FlexEnd", "Stretch", "Baseline", "SpaceBetween", "SpaceAround", "SpaceEvenly"];
    if (justify && JSON.stringify(options(justify)) !== JSON.stringify(expectedJustify)) failures.push("justify options are not the LayoutNode enum names");
    if (align && JSON.stringify(options(align)) !== JSON.stringify(expectedAlign)) failures.push("align options are not the LayoutNode enum names");
    if (document.getElementById("flex-status")?.getAttribute("role") !== "status") failures.push("flex status is not a status region");
    const apply = document.getElementById("flex-apply");
    const reset = document.getElementById("flex-reset");
    if (apply && apply.type !== "submit") failures.push("apply is not a submit button");
    if (reset && reset.type !== "button") failures.push("reset is not a plain button");
    return failures;
  })()`);
  assert.deepEqual(report, [], `Flex Explorer DOM audit failed:\n${report.join("\n")}`);
  console.log("PASS  DOM audit: unique ids, live inspector references, icon symbols, named typed controls, label/control pairs, exact enum options, status region");
}

let audited = false;

async function runScenario(mode, body) {
  const server = await startServer(mode);
  const profile = await mkdtemp(join(tmpdir(), "cove-flex-harness-"));
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

async function scenarioSet() {
  await runScenario("capable", async ({ cdp, fake }) => {
    await selectNode(cdp, "node-2");
    await waitForFlexFields(cdp);
    await waitFor(cdp, "node-2 record", `document.getElementById("flex-grow")?.value === "0"`);

    const initial = await flexForm(cdp);
    assert.equal(initial.grow.value, "0");
    assert.equal(initial.shrink.value, "1");
    assert.equal(initial.justify.value, "FlexStart");
    assert.equal(initial.align.value, "Stretch");
    assert.deepEqual(initial.justify.options, JUSTIFY);
    assert.deepEqual(initial.align.options, ALIGN);
    assert.deepEqual(initial.dirty, [], "a fresh record has no pending fields");
    assert.equal(initial.apply.disabled, true, "Apply is disabled with no changes");
    assert.equal(initial.reset.disabled, true, "Reset is disabled without an override");
    assert.equal(initial.status, "");
    assert.equal(editCalls(fake).length, 0);

    if (!audited) {
      await auditDom(cdp);
      audited = true;
    }

    // The factor limit is enforced before the request leaves the page.
    await setField(cdp, "flexGrow", "1500");
    const overLimit = await flexForm(cdp);
    assert.equal(overLimit.apply.disabled, true);
    assert.match(overLimit.status, /Flex factors must be between 0 and 1000/);
    assert.equal(overLimit.grow.invalid, "true");
    await setField(cdp, "flexGrow", "0");
    await waitFor(cdp, "limit cleared", `document.getElementById("flex-grow").getAttribute("aria-invalid") === "false"`);

    await setField(cdp, "flexGrow", "2");
    await setField(cdp, "flexShrink", "0.5");
    await setField(cdp, "justifyContent", "SpaceBetween");
    await setField(cdp, "alignItems", "Center");

    const dirty = await flexForm(cdp);
    assert.equal(dirty.apply.disabled, false);
    assert.deepEqual(dirty.dirty.sort(), ["alignItems", "flexGrow", "flexShrink", "justifyContent"]);
    assert.equal(dirty.status, "Unapplied changes");
    assert.equal(dirty.reset.disabled, true);

    await click(cdp, "#flex-apply");
    await waitFor(cdp, "apply loading state", `document.getElementById("flex-status").textContent.startsWith("Applying")`);
    const busy = await flexForm(cdp);
    assert.equal(busy.grow.disabled, true, "fields are disabled while the override is in flight");
    assert.equal(busy.justify.disabled, true);
    assert.equal(busy.apply.disabled, true);
    assert.equal(busy.reset.disabled, true);

    await waitFor(cdp, "override settled", `document.getElementById("flex-status").textContent === "" && document.getElementById("flex-grow").value === "2"`);
    const after = await flexForm(cdp);
    assert.equal(after.grow.value, "2");
    assert.equal(after.shrink.value, "0.5");
    assert.equal(after.justify.value, "SpaceBetween");
    assert.equal(after.align.value, "Center");
    assert.deepEqual(after.dirty, []);
    assert.equal(after.reset.disabled, false);
    assert.equal(after.apply.disabled, true);
    assert.equal(after.status, "");

    assert.deepEqual(editCalls(fake).map(call => call.params), [{
      target: "runtime-1",
      command: "inspect.style.set",
      arguments: {
        id: "node-2",
        properties: { flexGrow: 2, flexShrink: 0.5, justifyContent: "SpaceBetween", alignItems: "Center" },
      },
    }]);
    assertRecord(fake, "node-2", { flexGrow: 2, flexShrink: 0.5, justifyContent: "SpaceBetween", alignItems: "Center" });
    assert.deepEqual([...fake.nodes.get("node-2").overrides].sort(), ["alignItems", "flexGrow", "flexShrink", "justifyContent"]);
    await cdp.evaluate(`document.getElementById("flex-editor").scrollIntoView({ block: "center" })`);
    await screenshot(cdp, process.env.FLEX_SHOT);
  });
  console.log("PASS  set: record values seed the fields, exact edit_ui_style params, loading state, record-backed result, Apply/Reset gating");
}

async function scenarioPartial() {
  await runScenario("capable", async ({ cdp, fake }) => {
    await selectNode(cdp, "node-2");
    await waitFor(cdp, "node-2 record", `document.getElementById("flex-grow")?.value === "0"`);

    await setField(cdp, "justifyContent", "SpaceEvenly");
    await click(cdp, "#flex-apply");
    await waitFor(cdp, "partial override settled", `document.getElementById("flex-status").textContent === "" && document.getElementById("flex-justify").value === "SpaceEvenly"`);

    assert.deepEqual(editCalls(fake).map(call => call.params), [{
      target: "runtime-1",
      command: "inspect.style.set",
      arguments: { id: "node-2", properties: { justifyContent: "SpaceEvenly" } },
    }], "only the changed property is sent");
    const form = await flexForm(cdp);
    assert.equal(form.grow.value, "0", "untouched factors keep their record value");
    assert.equal(form.shrink.value, "1");
    assert.equal(form.align.value, "Stretch");
    assertRecord(fake, "node-2", { flexGrow: 0, flexShrink: 1, alignItems: "Stretch", justifyContent: "SpaceEvenly" });
    assert.deepEqual(fake.nodes.get("node-2").overrides, ["justifyContent"]);
  });
  console.log("PASS  partial set: only changed properties are sent, untouched record values stay intact");
}

async function scenarioClear() {
  await runScenario("capable", async ({ cdp, fake }) => {
    await selectNode(cdp, "node-2");
    await waitFor(cdp, "node-2 record", `document.getElementById("flex-grow")?.value === "0"`);

    await setField(cdp, "flexGrow", "2");
    await click(cdp, "#flex-apply");
    await waitFor(cdp, "override settled", `document.getElementById("flex-grow").value === "2" && document.getElementById("flex-reset").disabled === false`);

    await click(cdp, "#flex-reset");
    await waitFor(cdp, "reset loading state", `document.getElementById("flex-status").textContent.startsWith("Resetting")`);
    await waitFor(cdp, "reset settled", `document.getElementById("flex-status").textContent === "" && document.getElementById("flex-grow").value === "0"`);

    const form = await flexForm(cdp);
    assert.equal(form.grow.value, "0", "Reset restores the runtime's original value");
    assert.equal(form.reset.disabled, true, "no override remains after Reset");
    assert.equal(form.apply.disabled, true);
    assert.deepEqual(form.dirty, []);
    assert.deepEqual(editCalls(fake).map(call => call.params), [
      {
        target: "runtime-1",
        command: "inspect.style.set",
        arguments: { id: "node-2", properties: { flexGrow: 2 } },
      },
      { target: "runtime-1", command: "inspect.style.clear", arguments: { id: "node-2" } },
    ], "Reset clears the node, not every override");
    assertRecord(fake, "node-2", ORIGINAL["node-2"]);
    assert.deepEqual(fake.nodes.get("node-2").overrides, []);
  });
  console.log("PASS  clear: Reset sends inspect.style.clear for the node, record returns to the original values");
}

async function scenarioError() {
  await runScenario("error", async ({ cdp, fake }) => {
    await selectNode(cdp, "node-2");
    await waitFor(cdp, "node-2 record", `document.getElementById("flex-grow")?.value === "0"`);

    await setField(cdp, "flexGrow", "1.5");
    await click(cdp, "#flex-apply");
    await waitFor(cdp, "error state", `document.getElementById("flex-status").textContent.includes("(unsupported)")`);

    const form = await flexForm(cdp);
    assert.match(form.status, /The runtime rejected the flex override\. \(unsupported\)/);
    assert.match(form.statusClass, /is-error/);
    assert.equal(form.grow.disabled, false, "the form returns to an editable state");
    assert.equal(form.apply.disabled, false, "the draft can be retried");
    assert.equal(form.reset.disabled, true, "a rejected override never enables Reset");
    assert.equal(form.grow.value, "1.5", "the unapplied draft is kept");
    assertRecord(fake, "node-2", { flexGrow: 0 });
    assert.deepEqual(fake.nodes.get("node-2").overrides, []);
  });
  console.log("PASS  error: inline error, editable retry, no override state and no fabricated value");
}

async function scenarioStale() {
  await runScenario("stale", async ({ cdp, fake }) => {
    await selectNode(cdp, "node-2");
    await waitFor(cdp, "node-2 record", `document.getElementById("flex-grow")?.value === "0"`);

    await setField(cdp, "flexGrow", "1.5");
    await click(cdp, "#flex-apply");
    await waitFor(cdp, "stale state", `document.getElementById("flex-status").textContent.includes("no longer in the scene")`);

    const stale = await flexForm(cdp);
    assert.match(stale.status, /stale_node/);
    assert.match(stale.statusClass, /is-error/);
    assert.equal(stale.grow.disabled, true, "a stale node disables editing");
    assert.equal(stale.shrink.disabled, true);
    assert.equal(stale.justify.disabled, true);
    assert.equal(stale.apply.disabled, true);
    assert.equal(stale.reset.disabled, true);
    assertRecord(fake, "node-2", { flexGrow: 0 });

    // Selecting a live node clears the stale state.
    await selectNode(cdp, "node-1");
    await waitFor(cdp, "live node record", `document.getElementById("flex-grow")?.value === "1" && document.getElementById("flex-grow").disabled === false`);
    const live = await flexForm(cdp);
    assert.equal(live.status, "");
    assert.equal(live.justify.value, "Center");
    assert.equal(live.align.value, "FlexEnd");
    assert.equal(live.apply.disabled, true);
  });
  console.log("PASS  stale node: explicit stale state, disabled controls, state clears on a live selection");
}

async function scenarioNoStyle() {
  await runScenario("no-style", async ({ cdp, fake }) => {
    await selectNode(cdp, "node-2");
    await waitForFlexFields(cdp);
    await waitFor(cdp, "style capability status", `document.getElementById("flex-status").textContent === "This runtime does not advertise style overrides."`);

    const form = await flexForm(cdp);
    assert.equal(form.grow.value, "0", "the layout record is still readable");
    assert.equal(form.grow.disabled, true);
    assert.equal(form.justify.disabled, true);
    assert.equal(form.apply.disabled, true);
    assert.equal(form.reset.disabled, true);
    await click(cdp, "#flex-apply");
    await sleep(250);
    assert.equal(editCalls(fake).length, 0, "a runtime without the style capability cannot receive an override");
  });
  console.log("PASS  no style capability: record still shown, controls disabled, no RPC");
}

async function scenarioNoLayout() {
  await runScenario("capable", async ({ cdp, fake }) => {
    await selectNode(cdp, "root");
    await waitForFlexFields(cdp);
    await waitFor(cdp, "no-layout status", `document.getElementById("flex-status").textContent === "This node has no layout box."`);

    const form = await flexForm(cdp);
    assert.equal(form.grow.value, "");
    assert.equal(form.justify.value, "");
    assert.equal(form.grow.disabled, true);
    assert.equal(form.justify.disabled, true);
    assert.equal(form.apply.disabled, true);
    assert.equal(form.reset.disabled, true);
    await click(cdp, "#flex-apply");
    await sleep(250);
    assert.equal(editCalls(fake).length, 0, "a node without a layout cannot send an override");
  });
  console.log("PASS  no layout: fields disabled, empty values, no RPC");
}

async function scenarioRecordLag() {
  await runScenario("lag", async ({ cdp, fake }) => {
    await selectNode(cdp, "node-2");
    await waitFor(cdp, "node-2 record", `document.getElementById("flex-grow")?.value === "0"`);

    await setField(cdp, "flexGrow", "3");
    await click(cdp, "#flex-apply");
    await waitFor(cdp, "record lag notice", `document.getElementById("flex-status").textContent.includes("has not picked up the override")`);

    const form = await flexForm(cdp);
    assert.equal(form.grow.value, "0", "the field shows the layout record, not the requested value");
    assert.match(form.statusClass, /is-warn/);
    assert.equal(form.reset.disabled, false, "the runtime accepted the override, so Reset stays available");
    assert.match(form.status, /has not picked up the override yet/);
    assert.deepEqual(editCalls(fake).map(call => call.params), [{
      target: "runtime-1",
      command: "inspect.style.set",
      arguments: { id: "node-2", properties: { flexGrow: 3 } },
    }]);
  });
  console.log("PASS  record lag: the client shows the unchanged record value with a warning instead of echoing the request");
}

console.log("Inspector Flex Explorer harness: fake controller + headless Chrome");
await scenarioSet();
await scenarioPartial();
await scenarioClear();
await scenarioError();
await scenarioStale();
await scenarioNoStyle();
await scenarioNoLayout();
await scenarioRecordLag();
console.log("RESULT PASS");
