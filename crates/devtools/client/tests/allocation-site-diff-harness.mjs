#!/usr/bin/env node
// Fake-controller harness for the Memory allocation-sites baseline diff. It
// serves the real index.html/inspector.js/inspector.css, answers /rpc with the
// controller's allocation-sampling wire shapes, and drives the page through
// Chrome DevTools Protocol. The fake controller owns every stack and counter;
// the client cannot invent a site, a byte or a delta. No Rust process starts.
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

function frame(address, functionName, filePath, line) {
  return { address, function: functionName, filePath, line };
}

// Sites A and A2 share the innermost frame and differ only in the caller. The
// diff must keep them apart by the full stack, not by row order or the top
// frame alone.
const F_RENDER = frame(0x104a2b3c4, "gallery::render", "/Users/dev/cove/examples/gallery/src/main.rs", 120);
const F_PAINT_NODE = frame(0x1049f1188, "cove_core::scene::paint_node", "/Users/dev/cove/crates/core/src/scene/node.rs", 904);
const F_PAINT_LAYERS = frame(0x10477e020, "cove_core::scene::layer::paint_layers", "/Users/dev/cove/crates/core/src/scene/layer.rs", 318);
const F_PAINT_TEXT = frame(0x1048e00a4, "cove_core::scene::paint_text", "/Users/dev/cove/crates/core/src/text/paint.rs", 77);
const F_OLD = frame(0x104b00000, "gallery::old_path", "/Users/dev/cove/examples/gallery/src/old.rs", 44);
const F_OLD_CALLER = frame(0x1049f0010, "gallery::run", "/Users/dev/cove/examples/gallery/src/main.rs", 18);
const F_LAYOUT = frame(0x1049f2010, "cove_core::scene::layout_node", "/Users/dev/cove/crates/core/src/scene/node.rs", 760);
const F_NEW = frame(0x104c10040, "gallery::new_path", "/Users/dev/cove/examples/gallery/src/new.rs", 91);

function site(sites, bytes, blocks) {
  return { bytes, blocks, framesTruncated: false, frames: sites };
}

const SITE_A = site([F_RENDER, F_PAINT_NODE, F_PAINT_LAYERS], 262144, 4);
const SITE_A_GROWN = site([F_RENDER, F_PAINT_NODE, F_PAINT_LAYERS], 393216, 6);
const SITE_A2 = site([F_RENDER, F_PAINT_TEXT, F_PAINT_LAYERS], 524288, 8);
const SITE_A2_SHRUNK = site([F_RENDER, F_PAINT_TEXT, F_PAINT_LAYERS], 262144, 4);
const SITE_B = site([F_OLD, F_OLD_CALLER], 131072, 32);
const SITE_C = site([F_LAYOUT], 65536, 512);
const SITE_D = site([F_NEW], 196608, 16);

const PHASE_ONE = [SITE_A, SITE_A2, SITE_B, SITE_C];
// The second report reorders every surviving site and drops B. Matching by
// array index would pair A with D; matching by stack keeps A and A2 distinct.
const PHASE_TWO = [SITE_D, SITE_C, SITE_A2_SHRUNK, SITE_A_GROWN];
const TRUNCATED_ONE = [SITE_A, SITE_C];
const TRUNCATED_TWO = [SITE_A_GROWN, SITE_D];

function target(id, title, pid) {
  return { id, title, platform: "macos", pid, kind: "runtime", capabilities: ["tree", "diagnostics", "allocation_sites"] };
}

function cacheCounters() {
  return {
    processResidentBytes: 104857600,
    processVirtualBytes: 536870912,
    allocCurrentBytes: 16384,
    allocPeakBytes: 32768,
    allocTotalBytes: 65536,
    allocLiveBlocks: 96,
    allocPeakLiveBlocks: 120,
    allocTotalBlocks: 240,
    allocDeallocatedBytes: 49152,
    allocDeallocatedBlocks: 144,
    shapingEntries: 3,
    shapingBytes: 2048,
  };
}

function sitesFor(mode, phase) {
  if (mode === "empty") return [];
  if (mode === "truncated") return phase === 1 ? TRUNCATED_ONE : TRUNCATED_TWO;
  return phase === 1 ? PHASE_ONE : PHASE_TWO;
}

function reportFor(mode, phase, rate, counters, limit = 64) {
  const allSites = sitesFor(mode, phase);
  const sites = allSites.slice(0, limit);
  const sampledBytes = allSites.reduce((sum, item) => sum + item.bytes, 0);
  const sampledBlocks = allSites.reduce((sum, item) => sum + item.blocks, 0);
  const siteCount = mode === "truncated" ? 57 : allSites.length;
  return {
    status: "running",
    rate,
    sampledBytes,
    sampledBlocks,
    samplesAttempted: counters.attempts,
    samplesRetained: counters.retained,
    samplesDropped: counters.dropped,
    captureFailures: counters.captureFailures,
    sitesTruncated: mode === "truncated" && siteCount > sites.length,
    siteCount,
    symbolizationReason: null,
    sites,
  };
}

function rpcError(code, message) {
  return { rpc: { code, message } };
}

function createFake(mode) {
  const calls = [];
  const sessions = new Map();
  const targetIds = mode === "targets" ? ["runtime-1", "runtime-2"] : ["runtime-1"];
  for (const id of targetIds) {
    sessions.set(id, {
      supported: true,
      running: false,
      rate: 1024,
      attempts: 0,
      retained: 0,
      dropped: 0,
      captureFailures: 0,
      last: null,
    });
  }
  const state = { phase: 1, unauthorized: false, failQuery: false };

  function session(targetId) {
    const found = sessions.get(targetId);
    if (!found) throw rpcError("target_gone", `Unknown target ${targetId}.`);
    return found;
  }

  function targets() {
    return targetIds.map(id => target(id, id === "runtime-1" ? "Gallery" : "Second Gallery", id === "runtime-1" ? 4242 : 4343));
  }

  function sessionPayload() {
    return {
      platform: "macos",
      permissions: { screen_capture: false, accessibility: false },
      targets: targets(),
    };
  }

  function idleSites() {
    return {
      status: "idle",
      rate: 0,
      sampledBytes: 0,
      sampledBlocks: 0,
      samplesAttempted: 0,
      samplesRetained: 0,
      samplesDropped: 0,
      captureFailures: 0,
      sitesTruncated: false,
      siteCount: 0,
      symbolizationReason: null,
      sites: [],
    };
  }

  function querySites(targetId, limit) {
    const current = session(targetId);
    if (state.failQuery) throw rpcError("internal", "Allocation report failed: sampler lock unavailable.");
    if (!current.supported) {
      throw rpcError("unavailable", "Allocation-site sampling is not available on this target: libc::backtrace is unavailable.");
    }
    if (!current.running) {
      if (!current.last) return idleSites();
      return {
        ...current.last,
        status: "ready",
        sites: current.last.sites.slice(0, limit),
        sitesTruncated: current.last.siteCount > limit,
      };
    }
    current.attempts += 4;
    current.retained += sitesFor(mode, state.phase).length;
    current.dropped += 1;
    if (mode === "empty") current.captureFailures = 3;
    return reportFor(mode, state.phase, current.rate, current, limit);
  }

  function startSites(targetId, rate) {
    const current = session(targetId);
    if (mode === "unsupported") {
      current.supported = false;
      throw rpcError("unavailable", "Allocation-site sampling is not available on android: libc::backtrace is unavailable.");
    }
    if (!Number.isInteger(rate) || rate < 1) {
      throw rpcError("invalid_request", "rate must be an integer from 1 to 1000000.");
    }
    current.running = true;
    current.rate = rate;
    current.attempts = 0;
    current.retained = 0;
    current.dropped = 0;
    current.captureFailures = 0;
    current.last = null;
    return {
      status: "running",
      rate,
      sampledBytes: 0,
      sampledBlocks: 0,
      samplesAttempted: 0,
      samplesRetained: 0,
      samplesDropped: 0,
      captureFailures: 0,
      sitesTruncated: false,
      siteCount: 0,
      symbolizationReason: null,
      sites: [],
    };
  }

  function stopSites(targetId) {
    const current = session(targetId);
    if (!current.running) {
      throw rpcError("no_allocation_profile", "Start allocation-site sampling before stopping it.");
    }
    current.running = false;
    current.last = { ...reportFor(mode, state.phase, current.rate, current, 64), status: "ready" };
    return current.last;
  }

  function handle(method, params) {
    calls.push({ method, params });
    if (method === "get_ui_session") return sessionPayload();
    if (method === "query_ui_memory") {
      return { ok: true, data: { counters: cacheCounters(), windows: {} } };
    }
    const targetId = String(params.target || targetIds[0]);
    if (method === "query_ui_allocation_sites") return querySites(targetId, Number(params.limit) || 64);
    if (method === "start_ui_allocation_profile") return startSites(targetId, params.rate);
    if (method === "stop_ui_allocation_profile") return stopSites(targetId);
    throw rpcError("unknown_command", `Unknown command ${method}.`);
  }

  return { calls, handle, sessions, state };
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
      if (fake.state.unauthorized) {
        response.writeHead(401, { "content-type": "application/json" });
        response.end(JSON.stringify({ message: "Unauthorized" }));
        return;
      }
      let reply;
      try {
        const result = fake.handle(message.method, message.params || {});
        reply = { jsonrpc: "2.0", id: message.id, result };
      } catch (error) {
        if (error && error.rpc) {
          reply = {
            jsonrpc: "2.0",
            id: message.id,
            error: { code: -32000, message: error.rpc.message, data: { code: error.rpc.code } },
          };
        } else {
          reply = {
            jsonrpc: "2.0",
            id: message.id,
            error: { code: -32000, message: String(error?.message || error), data: { code: "internal" } },
          };
        }
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
  return {
    fake,
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
    "--window-size=1440,1000",
    url,
  ], { stdio: ["ignore", "pipe", "pipe"] });
  let stderr = "";
  child.stderr.on("data", chunk => { stderr = (stderr + chunk).slice(-8000); });
  const deadline = Date.now() + 15000;
  let targetInfo = null;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json/list`);
      const targets = await response.json();
      targetInfo = targets.find(item => item.type === "page" && item.webSocketDebuggerUrl && item.url.startsWith("http://127.0.0.1"));
      if (targetInfo) break;
    } catch {}
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  if (!targetInfo) {
    child.kill("SIGKILL");
    throw new Error(`Chrome did not expose a page target.\n${stderr}`);
  }
  return {
    child,
    wsUrl: targetInfo.webSocketDebuggerUrl,
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
      } else if (message.method === "Runtime.consoleAPICalled" && message.params.type === "error") {
        this.exceptions.push(message.params.args?.map(arg => arg.value ?? arg.description).join(" ") || "console error");
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

async function click(cdp, selector) {
  const clicked = await cdp.evaluate(`(() => {
    const node = document.querySelector(${JSON.stringify(selector)});
    if (!node) return false;
    node.click();
    return true;
  })()`);
  assert.equal(clicked, true, `click ${selector}`);
}

async function clickControl(cdp, label) {
  const clicked = await cdp.evaluate(`(() => {
    const section = document.getElementById("memory-allocation-sites");
    const button = [...(section?.querySelectorAll(".profile-actions button") || [])]
      .find(item => item.textContent.trim() === ${JSON.stringify(label)});
    if (!button) return false;
    button.click();
    return true;
  })()`);
  assert.equal(clicked, true, `click allocation control ${label}`);
}

async function setRate(cdp, value) {
  const applied = await cdp.evaluate(`(() => {
    const input = document.getElementById("alloc-site-rate");
    if (!input) return false;
    input.value = ${JSON.stringify(String(value))};
    input.dispatchEvent(new Event("input", { bubbles: true }));
    input.dispatchEvent(new Event("change", { bubbles: true }));
    return true;
  })()`);
  assert.equal(applied, true, "set allocation rate");
}

async function openMemory(cdp) {
  await waitFor(cdp, "controller connection", `document.getElementById("connection-text")?.textContent === "Connected"`);
  await click(cdp, "#tab-memory");
  await waitFor(cdp, "memory view", `document.getElementById("view-memory")?.hidden === false`);
  await waitFor(cdp, "allocation section", `document.getElementById("memory-allocation-sites") !== null`);
  await waitFor(cdp, "allocation idle", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Idle"`);
  await waitFor(cdp, "baseline section", `document.getElementById("memory-allocation-baseline") !== null`);
}

async function startSampling(cdp, rate) {
  await setRate(cdp, rate);
  await clickControl(cdp, "Start");
  await waitFor(cdp, "sampling running", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Running"`);
}

async function waitForRows(cdp, count) {
  await waitFor(cdp, `${count} site rows`, `document.querySelectorAll("#memory-allocation-sites .memory-site-table tbody tr.memory-site-row").length === ${count}`);
}

async function waitForDiffRows(cdp, count) {
  await waitFor(cdp, `${count} diff rows`, `document.querySelectorAll("#memory-allocation-baseline .memory-diff-row").length === ${count}`);
}

async function captureBaseline(cdp) {
  await waitFor(cdp, "baseline capture enabled", `document.getElementById("alloc-baseline-capture")?.disabled === false`);
  await click(cdp, "#alloc-baseline-capture");
  await waitFor(cdp, "baseline export enabled", `document.getElementById("alloc-baseline-export")?.disabled === false`);
}

async function installDownloadCapture(cdp) {
  await cdp.evaluate(`(() => {
    window.__allocExport = null;
    window.__allocExportName = "";
    URL.createObjectURL = blob => {
      blob.text().then(text => { window.__allocExport = text; });
      return "blob:cove-allocation-diff-harness";
    };
    HTMLAnchorElement.prototype.click = function () { window.__allocExportName = this.download; };
  })()`);
}

async function exportBaseline(cdp) {
  await installDownloadCapture(cdp);
  await click(cdp, "#alloc-baseline-export");
  await waitFor(cdp, "exported baseline", `typeof window.__allocExport === "string" && window.__allocExportName !== ""`);
  return {
    text: await cdp.evaluate("window.__allocExport"),
    name: await cdp.evaluate("window.__allocExportName"),
  };
}

async function dispatchSnapshot(cdp, text, name) {
  const dispatched = await cdp.evaluate(`(() => {
    const input = document.getElementById("alloc-baseline-import");
    if (!input) return false;
    const transfer = new DataTransfer();
    transfer.items.add(new File([${JSON.stringify(text)}], ${JSON.stringify(name)}, { type: "application/json" }));
    input.files = transfer.files;
    input.dispatchEvent(new Event("change", { bubbles: true }));
    return true;
  })()`);
  assert.equal(dispatched, true, `dispatch snapshot ${name}`);
}

async function mainPage(cdp) {
  return cdp.evaluate(`(() => {
    const section = document.getElementById("memory-allocation-sites");
    const q = selector => section?.querySelector(selector);
    const qa = selector => [...(section?.querySelectorAll(selector) || [])];
    return {
      badge: q(".panel-block-head .state-badge")?.textContent.trim() ?? "",
      meta: q(".panel-block-head .pane-meta")?.textContent.trim() ?? "",
      stateTitle: q(".memory-site-body .memory-site-state strong")?.textContent.trim() ?? "",
      stateMessage: q(".memory-site-body .memory-site-state p")?.textContent.trim() ?? "",
      rows: qa(".memory-site-table tbody tr.memory-site-row").map(row => [...row.children].map(cell => cell.textContent.trim())),
      scope: q(".memory-scope")?.textContent.trim() ?? "",
    };
  })()`);
}

async function baselinePage(cdp) {
  return cdp.evaluate(`(() => {
    const section = document.getElementById("memory-allocation-baseline");
    if (!section) return { present: false };
    const q = selector => section.querySelector(selector);
    const qa = selector => [...section.querySelectorAll(selector)];
    return {
      present: true,
      title: q("#alloc-baseline-title")?.textContent.trim() ?? "",
      labelledBy: section.getAttribute("aria-labelledby") || "",
      meta: q(".memory-baseline-head .pane-meta")?.textContent.trim() ?? "",
      empty: q(".memory-baseline-empty")?.textContent.trim() ?? "",
      error: q(".error-inline")?.textContent.trim() ?? "",
      baseline: q(".memory-diff-baseline")?.textContent.trim() ?? "",
      current: q(".memory-diff-current")?.textContent.trim() ?? "",
      summary: q(".memory-diff-summary")?.textContent.trim() ?? "",
      totals: qa(".memory-diff-total").map(node => node.textContent.trim()),
      notes: qa(".memory-note").map(node => node.textContent.trim()),
      notices: qa(".memory-diff-truncation .memory-note").map(node => node.textContent.trim()),
      session: q(".memory-diff-session .memory-note")?.textContent.trim() ?? "",
      stateTitle: q(".memory-site-state strong")?.textContent.trim() ?? "",
      stateMessage: q(".memory-site-state p")?.textContent.trim() ?? "",
      stateAction: q(".memory-site-state .tool-button")?.textContent.trim() ?? "",
      rows: qa(".memory-diff-row").map(row => ({
        status: row.getAttribute("data-allocation-diff-status") || "",
        cells: [...row.children].map(cell => cell.textContent.trim()),
      })),
      headers: [...(q(".memory-diff-table thead tr")?.children || [])].map(cell => cell.textContent.trim()),
      table: Boolean(q(".memory-diff-table")),
      detailsVisible: qa(".memory-diff-stack-row:not([hidden]) .memory-site-frame").map(row => row.querySelector(".memory-site-frame-name")?.textContent.trim() ?? ""),
      actions: {
        capture: document.getElementById("alloc-baseline-capture")?.disabled ?? null,
        export: document.getElementById("alloc-baseline-export")?.disabled ?? null,
        clear: document.getElementById("alloc-baseline-clear")?.disabled ?? null,
      },
      importInput: (() => {
        const input = document.getElementById("alloc-baseline-import");
        return input ? { type: input.type, accept: input.accept, label: input.getAttribute("aria-label") } : null;
      })(),
    };
  })()`);
}

async function auditDom(cdp, options = {}) {
  const failures = await cdp.evaluate(`(() => {
    const failures = [];
    const section = document.getElementById("memory-allocation-baseline");
    if (!section) failures.push("baseline section is missing");
    if (section) {
      const label = section.getAttribute("aria-labelledby");
      if (!label || !document.getElementById(label)) failures.push("baseline section heading reference is broken");
      for (const id of ["alloc-baseline-capture", "alloc-baseline-export", "alloc-baseline-import", "alloc-baseline-clear"]) {
        if (!document.getElementById(id)) failures.push("missing baseline control " + id);
      }
      const input = document.getElementById("alloc-baseline-import");
      if (!input || input.type !== "file") failures.push("baseline import is not a file input");
      if (input && (!input.accept.includes("json") || !input.getAttribute("aria-label"))) failures.push("baseline import input is not labeled for JSON");
      const text = section.textContent;
      if (!/cumulative sampled allocations/.test(text)) failures.push("baseline section does not label cumulative sampled allocations");
      if (!/not live heap/.test(text)) failures.push("baseline section does not exclude live heap");
    }
    const ids = [...document.querySelectorAll("[id]")].map(node => node.id);
    const duplicates = ids.filter((id, index) => ids.indexOf(id) !== index);
    if (duplicates.length) failures.push("duplicate ids: " + [...new Set(duplicates)].join(", "));
    const brokenRefs = [];
    for (const attribute of ["aria-controls", "aria-labelledby", "aria-describedby"]) {
      for (const node of document.querySelectorAll("#view-memory [" + attribute + "]")) {
        for (const id of (node.getAttribute(attribute) || "").split(/\\s+/).filter(Boolean)) {
          if (!document.getElementById(id)) brokenRefs.push(attribute + "=" + id);
        }
      }
    }
    if (brokenRefs.length) failures.push("broken references: " + [...new Set(brokenRefs)].join(", "));
    const symbols = new Set([...document.querySelectorAll("symbol[id]")].map(node => node.id));
    const brokenIcons = [...document.querySelectorAll("#view-memory use[href^='#']")]
      .map(node => node.getAttribute("href").slice(1))
      .filter(id => !symbols.has(id));
    if (brokenIcons.length) failures.push("broken icon references: " + [...new Set(brokenIcons)].join(", "));
    const unlabeled = [];
    for (const node of document.querySelectorAll("#view-memory button, #view-memory input, #view-memory select")) {
      const name = (node.getAttribute("aria-label") || node.getAttribute("title") || "").trim()
        || (node.id && document.querySelector('label[for="' + node.id + '"]')?.textContent.trim())
        || (node.closest("label")?.textContent.trim())
        || (node.tagName === "BUTTON" ? node.textContent.trim() : "");
      if (!name) unlabeled.push(node.id || node.className || node.tagName);
    }
    if (unlabeled.length) failures.push("unlabeled controls: " + unlabeled.join(", "));
    const untyped = [...document.querySelectorAll("#view-memory button")].filter(button => !button.hasAttribute("type")).map(button => button.id || button.className || button.textContent.trim());
    if (untyped.length) failures.push("buttons without type: " + untyped.join(", "));
    if (${options.diff ? "true" : "false"}) {
      const table = section?.querySelector(".memory-diff-table");
      if (!table) failures.push("diff table is missing");
      if (table) {
        const headers = [...table.querySelectorAll("thead th")].map(cell => cell.textContent.trim());
        if (headers.length !== 8) failures.push("diff table headers: " + headers.join("|"));
        const rows = [...table.querySelectorAll("tbody tr.memory-diff-row")];
        if (!rows.length) failures.push("diff table has no rows");
        for (const row of rows) {
          if (!row.getAttribute("data-allocation-diff-status")) failures.push("diff row has no status");
          const toggle = row.querySelector(".memory-site-toggle");
          if (!toggle || !toggle.getAttribute("aria-controls") || !document.getElementById(toggle.getAttribute("aria-controls"))) {
            failures.push("diff stack toggle reference is broken");
          }
        }
      }
    } else if (section && !section.querySelector(".memory-baseline-empty")) {
      failures.push("no-baseline state is missing");
    }
    return failures;
  })()`);
  assert.deepEqual(failures, [], `Allocation baseline DOM audit failed:\n${failures.join("\n")}`);
}

async function runScenario(mode, body) {
  const server = await startServer(mode);
  const profile = await mkdtemp(join(tmpdir(), "cove-allocation-diff-"));
  let chrome = null;
  let cdp = null;
  try {
    chrome = await startChrome(profile, server.url);
    cdp = await connect(chrome.wsUrl);
    await openMemory(cdp);
    await body({ cdp, fake: server.fake, mode });
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

async function scenarioNoBaseline() {
  await runScenario("diff", async ({ cdp }) => {
    const view = await baselinePage(cdp);
    assert.equal(view.present, true);
    assert.equal(view.title, "Baseline diff");
    assert.equal(view.labelledBy, "alloc-baseline-title");
    assert.equal(view.meta, "No baseline");
    assert.match(view.empty, /No baseline snapshot and no current report/);
    assert.match(view.empty, /No site is inferred/);
    assert.equal(view.actions.capture, true, "capture is disabled until a report exists");
    assert.equal(view.actions.export, true);
    assert.equal(view.actions.clear, true);
    assert.deepEqual(view.rows, []);
    assert.equal(view.table, false);
    assert.equal(view.importInput.type, "file");
    assert.equal(view.importInput.accept, ".json,application/json");
    assert.match(view.importInput.label, /Import an allocation-site baseline snapshot JSON/);
    const main = await mainPage(cdp);
    assert.match(main.scope, /Cumulative sampled allocations, not live heap/);
    await auditDom(cdp, { diff: false });
  });
  console.log("PASS  explicit no-baseline state disables capture/export/clear and keeps the import path");
}

async function scenarioDiff() {
  await runScenario("diff", async ({ cdp, fake }) => {
    await startSampling(cdp, 8);
    await waitForRows(cdp, 4);
    await captureBaseline(cdp);

    let view = await baselinePage(cdp);
    assert.equal(view.meta.match(/^Baseline \d{2}:\d{2}:\d{2}(?: [AP]M)?$/)?.[0], view.meta);
    assert.match(view.baseline, /^session snapshot · captured \d{2}:\d{2}:\d{2}(?: [AP]M)? · 1-in-8 · 4 sites · 960\.0 KB sampled$/);
    assert.match(view.current, /^Current report · updated \d{2}:\d{2}:\d{2}(?: [AP]M)? · 1-in-8 · 4 sites · 960\.0 KB sampled$/);
    assert.equal(view.summary, "0 added · 0 removed · 0 changed · 4 unchanged");
    assert.deepEqual(view.totals, [
      "Sampled bytes 960.0 KB → 960.0 KB · ±0 B · ±0.0%",
      "Sampled blocks 556 → 556 · ±0 · ±0.0%",
    ]);
    assert.equal(view.table, false);
    assert.ok(view.notes.some(note => /No added, removed or changed sampled sites/.test(note)));
    assert.equal(view.session, "");
    assert.equal(view.actions.export, false);
    assert.equal(view.actions.clear, false);

    const exported = await exportBaseline(cdp);
    assert.match(exported.name, /^cove-allocation-sites-Gallery-\d{8}-\d{6}\.json$/);
    const snapshot = JSON.parse(exported.text);
    assert.equal(snapshot.schema, "cove.devtools.allocation-sites");
    assert.equal(snapshot.version, 1);
    assert.equal(typeof snapshot.capturedAt, "number");
    assert.equal(typeof snapshot.reportUpdatedAt, "number");
    assert.deepEqual(snapshot.target, { id: "runtime-1", title: "Gallery" });
    assert.equal(snapshot.rate, 8);
    assert.match(snapshot.session, /^[a-z0-9]+-[a-z0-9]+:1$/);
    assert.equal(snapshot.report.status, "running");
    assert.equal(snapshot.report.sampledBytes, 983040);
    assert.equal(snapshot.report.sampledBlocks, 556);
    assert.equal(snapshot.report.samplesAttempted, 4);
    assert.equal(snapshot.report.sitesTruncated, false);
    assert.equal(snapshot.report.sites.length, 4);
    assert.deepEqual(Object.keys(snapshot.report.sites[0]).sort(), ["blocks", "bytes", "frames", "framesOmitted", "framesTruncated"]);
    assert.deepEqual(Object.keys(snapshot.report.sites[0].frames[0]).sort(), ["address", "file", "function", "line", "module"]);
    assert.equal(snapshot.report.sites[0].frames[0].address, "0x104a2b3c4");
    assert.equal(snapshot.report.sites[0].frames[0].function, "gallery::render");
    assert.equal(snapshot.report.sites[0].frames[0].file, "/Users/dev/cove/examples/gallery/src/main.rs");
    assert.equal(snapshot.report.sites[0].frames[0].line, 120);

    fake.state.phase = 2;
    await clickControl(cdp, "Query");
    await waitForDiffRows(cdp, 4);
    view = await baselinePage(cdp);
    assert.equal(view.summary, "1 added · 1 removed · 2 changed · 1 unchanged");
    assert.deepEqual(view.totals, [
      "Sampled bytes 960.0 KB → 896.0 KB · −64.0 KB · −6.7%",
      "Sampled blocks 556 → 538 · −18 · −3.2%",
    ]);
    assert.deepEqual(view.headers, ["Change", "Sampled bytes", "Δ bytes", "Δ %", "Sampled blocks", "Δ blocks", "Δ %", "Sampling stack"]);
    assert.equal(view.rows.length, 4);
    const removed = view.rows.find(row => row.status === "removed");
    const added = view.rows.find(row => row.status === "added");
    const shrunk = view.rows.find(row => row.status === "changed" && row.cells[2] === "−256.0 KB");
    const grown = view.rows.find(row => row.status === "changed" && row.cells[2] === "+128.0 KB");
    assert.deepEqual(removed.cells.slice(0, 7), ["Removed", "128.0 KB → —", "−128.0 KB", "−100.0%", "32 → —", "−32", "−100.0%"]);
    assert.deepEqual(added.cells.slice(0, 7), ["Added", "— → 192.0 KB", "+192.0 KB", "—", "— → 16", "+16", "—"]);
    assert.deepEqual(shrunk.cells.slice(0, 7), ["Changed", "512.0 KB → 256.0 KB", "−256.0 KB", "−50.0%", "8 → 4", "−4", "−50.0%"]);
    assert.deepEqual(grown.cells.slice(0, 7), ["Changed", "256.0 KB → 384.0 KB", "+128.0 KB", "+50.0%", "4 → 6", "+2", "+50.0%"]);
    assert.ok(view.notes.some(note => /absent from that report, not zero sampled bytes/.test(note)));
    assert.ok(view.notes.some(note => /cumulative sampled allocations/.test(note)));
    assert.ok(view.notes.some(note => /dropped from the retained ring/.test(note)));
    assert.ok(view.notes.some(note => /fixed ring dropping older samples/.test(note)));
    assert.equal(view.session, "", "a baseline and a later report in the same session are comparable");
    assert.equal(view.rows.some(row => /old_path/.test(row.cells[7])), true);

    const expanded = await cdp.evaluate(`(() => {
      const row = [...document.querySelectorAll("#memory-allocation-baseline .memory-diff-row")]
        .find(item => item.getAttribute("data-allocation-diff-status") === "removed");
      const button = row?.querySelector(".memory-site-toggle");
      if (!button) return false;
      button.click();
      return true;
    })()`);
    assert.equal(expanded, true, "expand the removed site stack");
    await waitFor(cdp, "removed stack frames", `document.querySelectorAll("#memory-allocation-baseline .memory-diff-stack-row:not([hidden]) .memory-site-frame").length === 2`);
    view = await baselinePage(cdp);
    assert.deepEqual(view.detailsVisible, ["gallery::old_path", "gallery::run"]);
    await cdp.evaluate(`document.getElementById("memory-allocation-baseline")?.scrollIntoView({ block: "start" })`);
    await screenshot(cdp, process.env.ALLOCATION_DIFF_SHOT);

    const malformed = [
      { name: "malformed.json", text: "not json", needle: "not valid JSON" },
      { name: "rate.json", text: JSON.stringify({ ...snapshot, rate: 0 }), needle: "rate is not" },
      { name: "unknown.json", text: JSON.stringify({ ...snapshot, extra: true }), needle: "unknown field: extra" },
      {
        name: "target.json",
        text: JSON.stringify({ ...snapshot, target: { id: "runtime-2", title: "Second Gallery" } }),
        needle: "captured on target",
      },
      {
        name: "negative.json",
        text: (() => {
          const copy = JSON.parse(exported.text);
          copy.report.sites[0].bytes = -1;
          return JSON.stringify(copy);
        })(),
        needle: "report.sites[0].bytes",
      },
      {
        name: "duplicate.json",
        text: (() => {
          const copy = JSON.parse(exported.text);
          copy.report.sites.push(copy.report.sites[0]);
          return JSON.stringify(copy);
        })(),
        needle: "repeats a stack identity",
      },
      {
        name: "totals.json",
        text: (() => {
          const copy = JSON.parse(exported.text);
          copy.report.sampledBytes = 1;
          return JSON.stringify(copy);
        })(),
        needle: "smaller than the sum",
      },
    ];
    for (const item of malformed) {
      await dispatchSnapshot(cdp, item.text, item.name);
      await waitFor(cdp, `rejected ${item.name}`, `document.querySelector("#memory-allocation-baseline .error-inline")?.textContent.includes(${JSON.stringify(item.needle)})`);
      view = await baselinePage(cdp);
      assert.match(view.error, /^Snapshot rejected: /);
      assert.match(view.error, new RegExp(item.needle.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
      assert.equal(view.table, true, `a rejected snapshot must keep the previous diff (${item.name})`);
      assert.equal(view.rows.length, 4, `a rejected snapshot must keep the previous rows (${item.name})`);
      assert.match(view.baseline, /^session snapshot/);
    }

    const edited = JSON.parse(exported.text);
    edited.report.sites[0].bytes = 131072;
    edited.report.sites[0].blocks = 2;
    await dispatchSnapshot(cdp, JSON.stringify(edited), "edited-baseline.json");
    await waitFor(
      cdp,
      "imported baseline",
      `!document.querySelector("#memory-allocation-baseline .error-inline") && document.querySelector("#memory-allocation-baseline .memory-diff-baseline")?.textContent.startsWith("imported snapshot")`,
    );
    view = await baselinePage(cdp);
    assert.equal(view.error, "");
    assert.match(view.baseline, /^imported snapshot · captured \d{2}:\d{2}:\d{2}(?: [AP]M)? · 1-in-8 · 4 sites · 960\.0 KB sampled$/);
    assert.equal(view.summary, "1 added · 1 removed · 2 changed · 1 unchanged");
    const importedGrown = view.rows.find(row => row.status === "changed" && row.cells[2] === "+256.0 KB");
    assert.deepEqual(importedGrown.cells.slice(0, 7), ["Changed", "128.0 KB → 384.0 KB", "+256.0 KB", "+200.0%", "2 → 6", "+4", "+200.0%"]);
    await auditDom(cdp, { diff: true });

    const reexported = await exportBaseline(cdp);
    const roundtrip = JSON.parse(reexported.text);
    assert.deepEqual(Object.keys(roundtrip).sort(), ["capturedAt", "rate", "report", "reportUpdatedAt", "schema", "session", "target", "version"]);
    assert.deepEqual(Object.keys(roundtrip.report).sort(), [
      "captureFailures", "framesOmitted", "framesTruncated", "sampledBlocks", "sampledBytes",
      "samplesAttempted", "samplesDropped", "samplesRetained", "sites", "sitesOmitted",
      "sitesTruncated", "status", "symbolizationReason", "totalSites",
    ]);
    assert.deepEqual(Object.keys(roundtrip.report.sites[0]).sort(), ["blocks", "bytes", "frames", "framesOmitted", "framesTruncated"]);
    roundtrip.report.sites[0].bytes = 100000;
    roundtrip.report.sites[0].blocks = 3;
    await dispatchSnapshot(cdp, JSON.stringify(roundtrip), "roundtrip.json");
    await waitFor(
      cdp,
      "round-trip baseline imported",
      `!document.querySelector("#memory-allocation-baseline .error-inline") && [...document.querySelectorAll("#memory-allocation-baseline .memory-diff-row")].some(row => row.textContent.includes("286.3 KB") && row.textContent.includes("+293.2%"))`,
    );

    await click(cdp, "#alloc-baseline-clear");
    await waitFor(cdp, "cleared baseline", `document.getElementById("memory-allocation-baseline")?.querySelector(".memory-baseline-empty") !== null`);
    view = await baselinePage(cdp);
    assert.equal(view.meta, "No baseline");
    assert.equal(view.table, false);
    assert.equal(view.actions.export, true);
    assert.equal(view.actions.clear, true);
    await auditDom(cdp, { diff: false });
  });
  console.log("PASS  capture → export → reordered later report diff → rejected malformed variants → import → clear");
}

async function scenarioEmpty() {
  await runScenario("empty", async ({ cdp }) => {
    await startSampling(cdp, 8);
    await waitFor(cdp, "empty site state", `document.querySelector("#memory-allocation-sites .memory-site-state strong")?.textContent === "No sampled allocations yet"`);
    await captureBaseline(cdp);
    let view = await baselinePage(cdp);
    assert.equal(view.summary, "0 added · 0 removed · 0 changed · 0 unchanged");
    assert.deepEqual(view.totals, [
      "Sampled bytes 0 B → 0 B · ±0 B · —",
      "Sampled blocks 0 → 0 · ±0 · —",
    ]);
    assert.equal(view.table, false);
    assert.equal(view.rows.length, 0);
    assert.ok(view.notes.some(note => /No added, removed or changed sampled sites/.test(note)));

    await clickControl(cdp, "Stop");
    await waitFor(cdp, "empty stopped", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Stopped"`);
    await waitFor(cdp, "empty stopped state", `document.querySelector("#memory-allocation-sites .memory-site-state strong")?.textContent === "No retained allocation sites"`);
    view = await baselinePage(cdp);
    assert.equal(view.table, false);
    assert.equal(view.rows.length, 0);
    assert.equal(view.summary, "0 added · 0 removed · 0 changed · 0 unchanged");
  });
  console.log("PASS  an empty baseline stays empty across running and stopped reports with no invented sites");
}

async function scenarioTruncated() {
  await runScenario("truncated", async ({ cdp, fake }) => {
    await startSampling(cdp, 16);
    await waitFor(cdp, "truncation notices", `document.querySelectorAll("#memory-allocation-sites .memory-site-truncation .memory-note").length >= 1`);
    await captureBaseline(cdp);
    let view = await baselinePage(cdp);
    assert.ok(view.notices.some(note => /^Baseline: Showing 2 of 57 sampled sites/.test(note)));

    fake.state.phase = 2;
    await clickControl(cdp, "Query");
    await waitForDiffRows(cdp, 3);
    view = await baselinePage(cdp);
    assert.equal(view.summary, "1 added · 1 removed · 1 changed · 0 unchanged");
    assert.ok(view.notices.some(note => /^Baseline: Showing 2 of 57 sampled sites/.test(note)));
    assert.ok(view.notices.some(note => /^Current: Showing 2 of 57 sampled sites/.test(note)));
    assert.ok(view.notices.some(note => /Sites omitted from either report cannot be matched/.test(note)));
    assert.deepEqual(view.totals, [
      "Sampled bytes 320.0 KB → 576.0 KB · +256.0 KB · +80.0%",
      "Sampled blocks 516 → 22 · −494 · −95.7%",
    ]);
    const removed = view.rows.find(row => row.status === "removed");
    const added = view.rows.find(row => row.status === "added");
    const changed = view.rows.find(row => row.status === "changed");
    assert.deepEqual(removed.cells.slice(0, 7), ["Removed", "64.0 KB → —", "−64.0 KB", "−100.0%", "512 → —", "−512", "−100.0%"]);
    assert.deepEqual(added.cells.slice(0, 7), ["Added", "— → 192.0 KB", "+192.0 KB", "—", "— → 16", "+16", "—"]);
    assert.deepEqual(changed.cells.slice(0, 7), ["Changed", "256.0 KB → 384.0 KB", "+128.0 KB", "+50.0%", "4 → 6", "+2", "+50.0%"]);
    assert.equal(view.rows.length, 3);
  });
  console.log("PASS  truncated baseline and current lists are labelled on both sides and no omitted site is invented");
}

async function scenarioRateMismatch() {
  await runScenario("diff", async ({ cdp, fake }) => {
    await startSampling(cdp, 8);
    await waitForRows(cdp, 4);
    await captureBaseline(cdp);
    await clickControl(cdp, "Stop");
    await waitFor(cdp, "stopped", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Stopped"`);

    fake.state.phase = 1;
    await startSampling(cdp, 64);
    await waitForRows(cdp, 4);
    await waitFor(cdp, "rate mismatch state", `document.querySelector("#memory-allocation-baseline .memory-site-state strong")?.textContent === "Sampling rate changed"`);
    let view = await baselinePage(cdp);
    assert.match(view.baseline, /1-in-8/);
    assert.match(view.current, /1-in-64/);
    assert.match(view.stateMessage, /sampled every 8 allocations/);
    assert.match(view.stateMessage, /samples every 64/);
    assert.match(view.stateMessage, /not comparable/);
    assert.equal(view.stateAction, "Capture baseline");
    assert.equal(view.table, false);
    assert.equal(view.rows.length, 0);
    assert.deepEqual(view.totals, [], "no totals are shown across different sampling rates");
    assert.equal(view.actions.capture, false);
    assert.equal(view.actions.export, false);
    assert.equal(view.actions.clear, false);

    const recaptured = await cdp.evaluate(`(() => {
      const button = document.querySelector("#memory-allocation-baseline .memory-site-state .tool-button");
      if (!button) return false;
      button.click();
      return true;
    })()`);
    assert.equal(recaptured, true, "rate mismatch offers capture");
    await waitFor(cdp, "rate matched baseline", `!document.querySelector("#memory-allocation-baseline .memory-site-state") && document.querySelector("#memory-allocation-baseline .memory-diff-summary") !== null`);
    view = await baselinePage(cdp);
    assert.match(view.baseline, /1-in-64/);
    assert.equal(view.summary, "0 added · 0 removed · 0 changed · 4 unchanged");
    assert.equal(view.table, false);
  });
  console.log("PASS  a changed sampling rate blocks deltas, states both rates and recaptures at the current rate");
}

async function scenarioCrossSession() {
  await runScenario("diff", async ({ cdp, fake }) => {
    await startSampling(cdp, 8);
    await waitForRows(cdp, 4);
    await captureBaseline(cdp);
    await clickControl(cdp, "Stop");
    await waitFor(cdp, "first session stopped", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Stopped"`);
    const stopped = await baselinePage(cdp);
    assert.equal(stopped.summary, "0 added · 0 removed · 0 changed · 4 unchanged");
    assert.equal(stopped.table, false, "a stopped report with no changes keeps the explicit no-change state");

    fake.state.phase = 1;
    await startSampling(cdp, 8);
    await waitForRows(cdp, 4);
    await waitFor(cdp, "session warning", `document.querySelector("#memory-allocation-baseline .memory-diff-session") !== null`);
    const view = await baselinePage(cdp);
    assert.match(view.session, /Different sampling session/);
    assert.match(view.session, /negative delta is not freed memory/);
    assert.match(view.session, /not live growth/);
    assert.match(view.session, /cumulative sampled allocations/);
    assert.equal(view.summary, "0 added · 0 removed · 0 changed · 4 unchanged");
    assert.equal(view.rows.length, 0);
  });
  console.log("PASS  a later session is compared but labelled as cumulative counters, not freed or live heap");
}

async function scenarioTargetChange() {
  await runScenario("targets", async ({ cdp, fake }) => {
    await startSampling(cdp, 8);
    await waitForRows(cdp, 4);
    await captureBaseline(cdp);
    const exported = await exportBaseline(cdp);

    const switched = await cdp.evaluate(`(() => {
      const select = document.getElementById("targets");
      select.value = "runtime-2";
      select.dispatchEvent(new Event("change", { bubbles: true }));
      return select.value;
    })()`);
    assert.equal(switched, "runtime-2");
    await waitFor(cdp, "second target idle", `document.getElementById("targets")?.value === "runtime-2" && document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Idle"`, 10000);
    await waitFor(cdp, "second target baseline cleared", `document.getElementById("memory-allocation-baseline")?.querySelector(".memory-baseline-empty") !== null`);
    const view = await baselinePage(cdp);
    assert.equal(view.meta, "No baseline");
    assert.equal(view.table, false);
    assert.equal(view.actions.capture, true);
    assert.equal(view.actions.export, true);
    assert.equal(view.actions.clear, true);
    assert.ok(fake.calls.some(call => call.method === "stop_ui_allocation_profile" && call.params.target === "runtime-1"));
    assert.equal(fake.sessions.get("runtime-1").running, false);

    await dispatchSnapshot(cdp, exported.text, "runtime-one-baseline.json");
    await waitFor(cdp, "target mismatch rejected", `document.querySelector("#memory-allocation-baseline .error-inline")?.textContent.includes("captured on target")`);
    const rejected = await baselinePage(cdp);
    assert.match(rejected.error, /^Snapshot rejected: the snapshot was captured on target "runtime-1", not the selected target$/);
    assert.equal(rejected.table, false);
    assert.equal(rejected.actions.export, true);

    await startSampling(cdp, 8);
    await waitForRows(cdp, 4);
    await captureBaseline(cdp);
    const second = await baselinePage(cdp);
    assert.match(second.baseline, /^session snapshot/);
    assert.equal(second.summary, "0 added · 0 removed · 0 changed · 4 unchanged");
    const secondExport = await exportBaseline(cdp);
    assert.match(secondExport.name, /^cove-allocation-sites-Second-Gallery-\d{8}-\d{6}\.json$/);
    assert.deepEqual(JSON.parse(secondExport.text).target, { id: "runtime-2", title: "Second Gallery" });
  });
  console.log("PASS  target change stops the old sampler, clears the baseline and rejects cross-target snapshots");
}

async function scenarioError() {
  await runScenario("diff", async ({ cdp, fake }) => {
    await startSampling(cdp, 8);
    await waitForRows(cdp, 4);
    await captureBaseline(cdp);
    fake.state.phase = 2;
    await clickControl(cdp, "Query");
    await waitForDiffRows(cdp, 4);

    fake.state.failQuery = true;
    await clickControl(cdp, "Query");
    await waitFor(cdp, "query error", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Error"`);
    const main = await mainPage(cdp);
    assert.equal(main.stateTitle, "Allocation-site read failed");
    assert.match(main.stateMessage, /sampler lock unavailable/);
    const view = await baselinePage(cdp);
    assert.equal(view.table, true, "the last real report still supports the baseline diff");
    assert.equal(view.rows.length, 4);
    assert.equal(view.summary, "1 added · 1 removed · 2 changed · 1 unchanged");
    assert.equal(view.error, "");

    fake.state.failQuery = false;
    const retried = await cdp.evaluate(`(() => {
      const button = [...document.querySelectorAll("#memory-allocation-sites .memory-site-body .memory-site-state .tool-button")]
        .find(item => item.textContent.trim() === "Retry");
      if (!button) return false;
      button.click();
      return true;
    })()`);
    assert.equal(retried, true, "retry is offered");
    await waitFor(cdp, "query recovered", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Running"`);
    await waitForDiffRows(cdp, 4);

    fake.state.unauthorized = true;
    await clickControl(cdp, "Query");
    await waitFor(cdp, "disconnected", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Disconnected"`, 10000);
    const offline = await baselinePage(cdp);
    assert.equal(offline.table, true, "the last real report still supports the baseline diff while disconnected");
    assert.equal(offline.rows.length, 4);
    assert.equal(offline.actions.export, false);
  });
  console.log("PASS  a failed read keeps the last real report and baseline diff, then recovers; disconnect keeps the last diff");
}

async function scenarioUnsupported() {
  await runScenario("unsupported", async ({ cdp }) => {
    await setRate(cdp, 8);
    await clickControl(cdp, "Start");
    await waitFor(cdp, "unsupported", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Unsupported"`);
    const state = await cdp.evaluate(`({
      baseline: document.getElementById("memory-allocation-baseline") !== null,
      title: document.querySelector("#memory-allocation-sites .memory-site-state strong")?.textContent.trim() ?? "",
    })`);
    assert.equal(state.title, "Allocation sampling is unavailable");
    assert.equal(state.baseline, false, "no dead baseline controls are rendered without sampling support");
  });
  console.log("PASS  unsupported sampling hides baseline controls and invents no report");
}

console.log("Allocation-site diff harness: fake controller + headless Chrome");
await scenarioNoBaseline();
await scenarioDiff();
await scenarioEmpty();
await scenarioTruncated();
await scenarioRateMismatch();
await scenarioCrossSession();
await scenarioTargetChange();
await scenarioError();
await scenarioUnsupported();
console.log("RESULT PASS");
