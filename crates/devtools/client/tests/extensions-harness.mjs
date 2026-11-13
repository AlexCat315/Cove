#!/usr/bin/env node
// Fake-controller harness for the DevTools Extensions panel. It serves the real
// index.html/inspector.js/inspector.css, answers /rpc with the controller's
// session/tree plus `list_extensions` and `set_extension_enabled`, and drives
// the page through Chrome DevTools Protocol. The page can only show what the
// runtime publishes, so an extension the registry never lists must stay
// invisible, and a disabled extension must not open a panel.
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import assert from "node:assert/strict";
import { serveClientAsset } from "./harness-server.mjs";

const chromePath = process.env.CHROME_PATH
  || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

const DATA_URL = "data:text/html,%3Cbody%3EBuilt-in%20panel%3C%2Fbody%3E";

function target(capabilities) {
  return {
    id: "runtime-1", title: "Extensions Target", platform: "macos",
    pid: 4242, kind: "runtime", capabilities,
  };
}

function rpcError(code, message) {
  return {
    jsonrpc: "2.0", id: null,
    error: { code: -32000, message, data: { code } },
  };
}

function createFake(mode) {
  const calls = [];
  let listCount = 0;
  const entries = mode === "empty" ? [] : [
    {
      id: "cove.inspector", name: "Cove Inspector", version: "1.0.0",
      description: "Built-in DevTools extension.", homepage: null, iconPath: null,
      entryPath: mode === "unsafe" ? "javascript:alert(1)" : DATA_URL,
      enabled: true,
    },
    {
      id: "demo.theme", name: "Theme Explorer", version: "0.2.0",
      description: "Inspect live theme tokens.", homepage: "https://example.com/theme",
      iconPath: null, entryPath: "/extensions/theme/index.html", enabled: false,
    },
  ];

  function session() {
    const capabilities = mode === "no-capability"
      ? ["tree"]
      : ["tree", "extensions"];
    return {
      platform: "macos",
      permissions: { screen_capture: false, accessibility: false },
      targets: [target(capabilities)],
    };
  }

  function treePayload() {
    return {
      root: {
        id: "root", ownerScopeID: "root", debugName: "AppRoot",
        viewTag: "gallery::App", layoutRole: "stack", children: [],
      },
    };
  }

  async function handle(method, params = {}) {
    calls.push({ method, params });
    if (method === "get_ui_session") return session();
    if (method === "inspect_ui_tree") return treePayload();
    if (method === "list_extensions") {
      listCount += 1;
      if (mode === "error" && listCount === 1) {
        throw rpcError("unavailable", "DevTools extensions are unavailable in this host.");
      }
      if (mode === "empty") return { ok: true, data: { extensions: [] } };
      return { ok: true, data: { extensions: entries.map(entry => ({ ...entry })) } };
    }
    if (method === "set_extension_enabled") {
      const entry = entries.find(item => item.id === params.id);
      if (!entry) return { ok: false };
      entry.enabled = params.enabled === true;
      return { ok: true };
    }
    throw rpcError("unknown_command", `Unknown command ${method}.`);
  }

  return { calls, handle, entries, get listCount() { return listCount; } };
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
    `--window-size=${process.env.EXTENSIONS_WINDOW || "1440,900"}`,
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
        this.exceptions.push(message.params.exceptionDetails?.text || "exception");
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
      expression, awaitPromise: true, returnByValue: true,
    });
    if (result.exceptionDetails) {
      throw new Error(result.exceptionDetails.exception?.description || result.exceptionDetails.text);
    }
    return result.result.value;
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

async function waitFor(cdp, label, expression, timeout = 9000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (await cdp.evaluate(`Boolean(${expression})`)) return;
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  throw new Error(`Timed out waiting for ${label}: ${expression}\nPage: ${JSON.stringify(await snapshot(cdp))}`);
}

async function click(cdp, selector) {
  return cdp.evaluate(`(() => { const n = document.querySelector(${JSON.stringify(selector)}); if (!n) return false; n.click(); return true; })()`);
}

async function clickButton(cdp, text) {
  return cdp.evaluate(`(() => { const b = [...document.querySelectorAll("#extensions-content button")].find(x => x.textContent.trim() === ${JSON.stringify(text)}); if (!b) return false; b.click(); return true; })()`);
}

async function clickCheckbox(cdp, index) {
  return cdp.evaluate(`(() => { const boxes = [...document.querySelectorAll("#extensions-content input[type=checkbox]")]; if (!boxes[${index}]) return false; boxes[${index}].click(); return true; })()`);
}

async function snapshot(cdp) {
  return cdp.evaluate(`(() => {
    const content = document.getElementById("extensions-content");
    return {
      meta: document.getElementById("extensions-meta").textContent,
      cards: [...content.querySelectorAll(".section:not(.extension-frame-section)")].map(c => c.textContent),
      boxes: [...content.querySelectorAll("input[type=checkbox]")].map(b => b.checked),
      buttons: [...content.querySelectorAll("button")].map(b => b.textContent.trim()),
      frames: [...content.querySelectorAll("iframe")].map(f => ({ src: f.getAttribute("src"), sandbox: f.getAttribute("sandbox") })),
      stateTitle: content.querySelector(".state strong")?.textContent || null,
      stateMessage: content.querySelector(".state p")?.textContent || null,
      errorInline: content.querySelector(".error-inline")?.textContent || null,
      emptyText: content.querySelector(".empty-inline")?.textContent || null,
      refreshDisabled: document.getElementById("extensions-refresh").disabled,
    };
  })()`);
}

async function openExtensions(cdp) {
  await waitFor(cdp, "the Extensions rail tab", `document.getElementById("tab-extensions") && document.getElementById("extensions-content")`);
  assert.equal(await click(cdp, "#tab-extensions"), true, "the Extensions tab is clickable");
  await waitFor(cdp, "the Extensions view", `!document.getElementById("view-extensions").hidden`);
}

async function auditDom(cdp) {
  const report = await cdp.evaluate(`(() => {
    const problems = [];
    const ids = [...document.querySelectorAll("[id]")].map(node => node.id);
    for (const id of ["tab-extensions", "view-extensions", "extensions-content", "extensions-meta", "extensions-refresh", "i-extension"]) {
      if (ids.filter(value => value === id).length !== 1) problems.push("missing or duplicated id: " + id);
    }
    const tab = document.getElementById("tab-extensions");
    if (tab.dataset.view !== "extensions") problems.push("the tab points at the wrong view");
    if (tab.getAttribute("aria-controls") !== "view-extensions") problems.push("the tab does not control the Extensions panel");
    const panel = document.getElementById("view-extensions");
    if (panel.getAttribute("aria-labelledby") !== "tab-extensions") problems.push("the panel is not labelled by its tab");
    if (!document.querySelector('symbol#i-extension use, symbol#i-extension path')) problems.push("the Extensions icon symbol is empty");
    return problems;
  })()`);
  assert.deepEqual(report, [], `Extensions DOM audit failed:\n${report.join("\n")}`);
}

async function runScenario(mode, body) {
  const server = await startServer(mode);
  const profile = await mkdtemp(join(tmpdir(), "cove-extensions-harness-"));
  let chrome = null;
  let cdp = null;
  try {
    chrome = await startChrome(profile, server.url);
    cdp = await connect(chrome.wsUrl);
    await waitFor(cdp, "the Inspector to boot", `document.getElementById("connection-text")?.textContent.trim() !== "Disconnected"`);
    await waitFor(cdp, "the Extensions view to exist", `document.getElementById("extensions-content") !== null`);
    await body({ cdp, fake: server.fake });
    assert.deepEqual(cdp.exceptions, [], "no uncaught page exceptions");
  } finally {
    cdp?.socket.close();
    if (chrome) await chrome.close();
    await server.close();
    await rm(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
  }
}

async function scenarioCapable() {
  await runScenario("capable", async ({ cdp, fake }) => {
    await auditDom(cdp);
    await openExtensions(cdp);
    await waitFor(cdp, "two published extension cards", `document.querySelectorAll("#extensions-content .section:not(.extension-frame-section)").length === 2`);

    const listCalls = fake.calls.filter(call => call.method === "list_extensions");
    assert.equal(listCalls.length, 1, "switching to Extensions reads the registry once");
    assert.equal(listCalls[0].params.target, "runtime-1", "the read targets the selected runtime");

    const initial = await snapshot(cdp);
    assert.match(initial.cards[0], /Cove Inspector/);
    assert.match(initial.cards[0], /1\.0\.0/);
    assert.match(initial.cards[1], /Theme Explorer/);
    assert.deepEqual(initial.boxes, [true, false], "enabled state comes from the runtime, not the client");
    assert.equal(initial.meta, "2 registered · 1 enabled");
    assert.deepEqual(initial.buttons, ["Open panel"], "only an enabled extension with a loadable entry point can open a panel");
    assert.equal(initial.refreshDisabled, false);

    assert.equal(await clickCheckbox(cdp, 1), true, "the second toggle is clickable");
    await waitFor(cdp, "the refreshed registry", `document.getElementById("extensions-meta").textContent === "2 registered · 2 enabled"`);
    const toggles = fake.calls.filter(call => call.method === "set_extension_enabled");
    assert.equal(toggles.length, 1, "one enable command was sent");
    assert.deepEqual(toggles[0].params, { target: "runtime-1", id: "demo.theme", enabled: true });
    const after = await snapshot(cdp);
    assert.deepEqual(after.boxes, [true, true], "the toggle reflects the registry's new state");
    assert.deepEqual(after.buttons, ["Open panel", "Open panel"], "the newly enabled panel becomes openable");

    assert.equal(await clickButton(cdp, "Open panel"), true, "Open panel is clickable");
    await waitFor(cdp, "the sandboxed frame", `document.querySelectorAll("#extensions-content iframe").length === 1`);
    const framed = await snapshot(cdp);
    assert.equal(framed.frames.length, 1);
    assert.equal(framed.frames[0].src, DATA_URL, "the frame loads the published entry point");
    assert.ok(!framed.frames[0].sandbox.includes("allow-same-origin"), "the frame cannot escape its sandbox");

    assert.equal(await clickButton(cdp, "Close panel"), true);
    await waitFor(cdp, "the frame to close", `document.querySelectorAll("#extensions-content iframe").length === 0`);
  });
  console.log("PASS  capable: registry read, published cards, enable command, refreshed state, sandboxed panel");
}

async function scenarioNoCapability() {
  await runScenario("no-capability", async ({ cdp, fake }) => {
    await openExtensions(cdp);
    await waitFor(cdp, "the unavailable state", `document.querySelector("#extensions-content .state strong")?.textContent === "Extensions are unavailable"`);
    const state = await snapshot(cdp);
    assert.equal(state.refreshDisabled, true, "refresh is disabled without the capability");
    assert.deepEqual(state.buttons, [], "no controls are offered without the capability");
    assert.equal(fake.calls.filter(call => call.method === "list_extensions").length, 0, "a runtime without the capability receives no extension RPC");
  });
  console.log("PASS  no capability: explicit unavailable state, disabled controls, no RPC");
}

async function scenarioEmpty() {
  await runScenario("empty", async ({ cdp }) => {
    await openExtensions(cdp);
    await waitFor(cdp, "the empty registry", `document.querySelector("#extensions-content .empty-inline") !== null`);
    const state = await snapshot(cdp);
    assert.match(state.emptyText, /no DevTools extensions/);
    assert.equal(state.meta, "0 registered · 0 enabled");
    assert.deepEqual(state.cards, [], "no card is invented for an empty registry");
  });
  console.log("PASS  empty: an empty registry is reported instead of fabricating entries");
}

async function scenarioUnsafe() {
  await runScenario("unsafe", async ({ cdp }) => {
    await openExtensions(cdp);
    await waitFor(cdp, "the published cards", `document.querySelectorAll("#extensions-content .section:not(.extension-frame-section)").length === 2`);
    const state = await snapshot(cdp);
    assert.match(state.cards[0], /no entry point/, "an unsafe entry point is not advertised as a panel");
    assert.deepEqual(state.buttons, [], "neither a rejected scheme nor a disabled extension can open a panel");
    assert.deepEqual(state.frames, [], "no frame is created before the user opens one");

    assert.equal(await clickCheckbox(cdp, 1), true, "the extension with a safe entry point can be enabled");
    await waitFor(cdp, "the safe panel becomes openable", `document.querySelector("#extensions-content button")?.textContent.trim() === "Open panel"`);
    const enabled = await snapshot(cdp);
    assert.deepEqual(enabled.buttons, ["Open panel"], "only the extension with a safe entry point can open a panel");
    assert.deepEqual(enabled.frames, [], "enabling alone does not create a frame");

    assert.equal(await clickButton(cdp, "Open panel"), true);
    await waitFor(cdp, "the safe frame", `document.querySelectorAll("#extensions-content iframe").length === 1`);
    const framed = await snapshot(cdp);
    assert.equal(framed.frames[0].src, "/extensions/theme/index.html");
  });
  console.log("PASS  unsafe scheme: javascript: entry is rejected, a valid relative path still loads");
}

async function scenarioError() {
  await runScenario("error", async ({ cdp, fake }) => {
    await openExtensions(cdp);
    await waitFor(cdp, "the read error", `document.querySelector("#extensions-content .state.is-error strong")?.textContent === "Extension read failed"`);
    const failed = await snapshot(cdp);
    assert.match(failed.stateMessage, /unavailable/);
    assert.deepEqual(failed.buttons, ["Retry"]);
    assert.deepEqual(failed.cards, [], "no card is fabricated from a failed read");

    assert.equal(await clickButton(cdp, "Retry"), true);
    await waitFor(cdp, "the retried read", `document.querySelectorAll("#extensions-content .section:not(.extension-frame-section)").length === 2`);
    assert.equal(fake.listCount, 2, "Retry issues exactly one more read");
    const retried = await snapshot(cdp);
    assert.equal(retried.errorInline, null, "the error clears after a successful retry");
  });
  console.log("PASS  error: explicit failure, no fabricated data, Retry re-reads and clears the error");
}

async function main() {
  await scenarioCapable();
  await scenarioNoCapability();
  await scenarioEmpty();
  await scenarioUnsafe();
  await scenarioError();
  console.log("\nAll Extensions harness scenarios passed.");
}

main().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
