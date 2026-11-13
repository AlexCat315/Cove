#!/usr/bin/env node
// Fake-controller harness for the Memory view's Allocation sites section. It
// serves the real index.html/inspector.js/inspector.css, answers /rpc with the
// controller's allocation-sampling wire shapes, and drives the page through
// Chrome DevTools Protocol. No Rust process starts and no allocation site is
// invented outside the fake controller's declared responses.
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

// Addresses are the u64 values the protocol serializes; the client renders
// them as hex without inventing a symbol.
const SITE_ONE = {
  bytes: 262144,
  blocks: 4,
  framesTruncated: false,
  frames: [
    {
      address: 0x104a2b3c4,
      function: "gallery::render",
      filePath: "/Users/dev/cove/examples/gallery/src/main.rs",
      line: 120,
    },
    {
      address: 0x1049f1188,
      function: "cove_core::scene::paint_node",
      filePath: "/Users/dev/cove/crates/core/src/scene/node.rs",
      line: 904,
    },
    {
      address: 0x10477e020,
      function: "cove_core::scene::layer::paint_layers",
      filePath: "/Users/dev/cove/crates/core/src/scene/layer.rs",
      line: 318,
    },
  ],
};

const SITE_TWO = {
  bytes: 131072,
  blocks: 32,
  framesTruncated: false,
  frames: [
    {
      address: 0x104a00000,
      function: null,
      filePath: null,
      line: null,
    },
    {
      address: 0x1049f2010,
      function: "cove_core::scene::layout_node",
      filePath: "/Users/dev/cove/crates/core/src/scene/node.rs",
      line: 760,
    },
  ],
};

const SITE_THREE = {
  bytes: 65536,
  blocks: 512,
  framesTruncated: false,
  frames: [
    {
      address: 0x104b12040,
      function: "<cove_core::scene::Node as core::clone::Clone>::clone",
      filePath: "/Users/dev/cove/crates/core/src/scene/node.rs",
      line: 88,
    },
  ],
};

function supportedTarget(id, title, platform = "macos", capabilities = ["tree", "diagnostics", "allocation_sites"]) {
  return {
    id,
    title,
    platform,
    pid: id === "runtime-2" ? 4343 : 4242,
    kind: "runtime",
    capabilities,
  };
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

const UNRESOLVED_SITE = {
  bytes: 4096,
  blocks: 1,
  framesTruncated: false,
  frames: [
    {
      address: 0x104c0ffee,
      function: null,
      filePath: null,
      line: null,
    },
  ],
};

function reportSites(mode) {
  if (mode === "empty") return [];
  if (mode === "unsymbolized") return [UNRESOLVED_SITE];
  if (mode === "truncated") {
    const deepFrames = [];
    // The probe bounds each captured stack at 32 frames and marks the cut.
    for (let index = 0; index < 32; index += 1) {
      deepFrames.push({
        address: 0x104a2b3c4 + index * 16,
        function: index === 0 ? "gallery::deep_allocation" : `gallery::caller_${index}`,
        filePath: "/Users/dev/cove/examples/gallery/src/main.rs",
        line: 300 + index,
      });
    }
    return [
      {
        bytes: 1048576,
        blocks: 64,
        framesTruncated: true,
        frames: deepFrames,
      },
      {
        bytes: 524288,
        blocks: 8,
        framesTruncated: false,
        frames: [SITE_TWO.frames[1], SITE_ONE.frames[1]],
      },
    ];
  }
  return [SITE_ONE, SITE_TWO, SITE_THREE];
}

// The flat `UiAllocationSites` shape the probe and controller publish.
function reportFor(mode, status, rate, counters, limit = 64) {
  const allSites = reportSites(mode);
  const sites = allSites.slice(0, limit);
  const sampledBytes = allSites.reduce((sum, site) => sum + site.bytes, 0);
  const sampledBlocks = allSites.reduce((sum, site) => sum + site.blocks, 0);
  const siteCount = mode === "truncated" ? 57 : allSites.length;
  return {
    status,
    rate,
    sampledBytes,
    sampledBlocks,
    samplesAttempted: counters.attempts,
    samplesRetained: counters.retained,
    samplesDropped: counters.dropped,
    captureFailures: counters.captureFailures,
    sitesTruncated: siteCount > sites.length,
    siteCount,
    symbolizationReason: mode === "unsymbolized"
      ? "cove_profiling has no symbolizer on this platform"
      : null,
    sites,
  };
}

function rpcError(code, message) {
  return {
    jsonrpc: "2.0",
    id: null,
    error: { code: -32000, message, data: { code } },
  };
}

function fakeRpcError(code, message) {
  return { rpc: { code, message } };
}

function createFake(mode) {
  const calls = [];
  const sessions = new Map();
  const targetIds = mode === "targets" ? ["runtime-1", "runtime-2"] : ["runtime-1"];
  for (const id of targetIds) {
    sessions.set(id, {
      supported: mode !== "unsupported-initially",
      running: false,
      rate: 1024,
      attempts: 0,
      retained: 0,
      dropped: 0,
      captureFailures: 0,
      queries: 0,
      last: null,
    });
  }
  const state = {
    unauthorized: false,
    failQuery: false,
    failStart: false,
  };

  function session(targetId) {
    const found = sessions.get(targetId);
    if (!found) throw fakeRpcError("target_gone", `Unknown target ${targetId}.`);
    return found;
  }

  function targets() {
    return targetIds.map(id => supportedTarget(
      id,
      id === "runtime-1" ? "Gallery" : "Second Gallery",
      "macos",
      mode === "no-capability" ? ["tree", "diagnostics"] : ["tree", "diagnostics", "allocation_sites"],
    ));
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
    current.queries += 1;
    if (state.failQuery) {
      throw fakeRpcError("internal", "Allocation report failed: sampler lock unavailable.");
    }
    if (mode === "no-capability") {
      throw fakeRpcError("unavailable", "The runtime does not advertise allocation_sites.");
    }
    if (!current.supported) {
      throw fakeRpcError(
        "unavailable",
        "Allocation-site sampling is not available on this target: libc::backtrace is unavailable.",
      );
    }
    if (!current.running) {
      return current.last
        ? { ...current.last, status: "ready", sites: current.last.sites.slice(0, limit), sitesTruncated: current.last.siteCount > limit }
        : idleSites();
    }
    current.attempts += 4;
    current.retained += mode === "empty" ? 0 : 3;
    current.dropped += 1;
    if (!current.captureFailures && mode === "empty") current.captureFailures = 3;
    return reportFor(mode, "running", current.rate, current, limit);
  }

  function startSites(targetId, rate) {
    const current = session(targetId);
    if (state.failStart) {
      throw fakeRpcError("internal", "The runtime rejected the sampling session.");
    }
    if (!current.supported || mode === "unsupported") {
      current.supported = false;
      throw fakeRpcError(
        "unavailable",
        "Allocation-site sampling is not available on android: libc::backtrace is unavailable.",
      );
    }
    if (!Number.isInteger(rate) || rate < 1) {
      throw fakeRpcError("invalid_request", "rate must be an integer from 1 to 1000000.");
    }
    current.running = true;
    current.rate = rate;
    current.attempts = 0;
    current.retained = 0;
    current.dropped = 0;
    current.captureFailures = 0;
    current.queries = 0;
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
      throw fakeRpcError("no_allocation_profile", "Start allocation-site sampling before stopping it.");
    }
    current.running = false;
    current.last = reportFor(mode, "ready", current.rate, current, 64);
    return current.last;
  }

  function handle(method, params) {
    calls.push({ method, params });
    if (method === "get_ui_session") return sessionPayload();
    if (method === "query_ui_memory") {
      return { ok: true, data: { counters: cacheCounters(), windows: {} } };
    }
    const targetId = String(params.target || targetIds[0]);
    // The real controller names these start_ui_allocation_profile and
    // stop_ui_allocation_profile. `alias` mode proves the client falls back
    // when an older controller uses the `_sites` names instead.
    const startName = mode === "alias" ? "start_ui_allocation_sites" : "start_ui_allocation_profile";
    const stopName = mode === "alias" ? "stop_ui_allocation_sites" : "stop_ui_allocation_profile";
    if (method === "query_ui_allocation_sites") return querySites(targetId, Number(params.limit) || 64);
    if (method === startName) return startSites(targetId, params.rate);
    if (method === stopName) return stopSites(targetId);
    throw fakeRpcError("unknown_command", `Unknown command ${method}.`);
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
            error: {
              code: -32000,
              message: error.rpc.message,
              data: { code: error.rpc.code },
            },
          };
        } else {
          reply = { ...rpcError("internal", String(error?.message || error)), id: message.id };
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

async function waitUntil(description, predicate, timeout = 5000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await new Promise(resolve => setTimeout(resolve, 50));
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

async function clickAllocationControl(cdp, label) {
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

async function setAllocationRate(cdp, value) {
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
  await waitFor(cdp, "allocation idle or unavailable", `["Idle", "Unsupported", "Unavailable"].includes(document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim())`);
}

async function screenshot(cdp, path) {
  if (!path) return;
  const shot = await cdp.send("Page.captureScreenshot", { format: "png" });
  await writeFile(path, Buffer.from(shot.data, "base64"));
}

async function allocationPage(cdp) {
  return cdp.evaluate(`(() => {
    const host = document.getElementById("memory-content");
    const section = document.getElementById("memory-allocation-sites");
    const q = selector => section?.querySelector(selector);
    const qa = selector => [...(section?.querySelectorAll(selector) || [])];
    const metric = label => {
      const node = qa(".memory-site-metrics .metric").find(item => item.querySelector(".metric-label")?.textContent.trim() === label);
      return node?.querySelector(".metric-value")?.textContent.trim() ?? "";
    };
    const control = label => {
      const node = qa(".profile-actions button").find(item => item.textContent.trim() === label);
      return { found: Boolean(node), disabled: node?.disabled ?? null, text: node?.textContent.trim() ?? "" };
    };
    const rows = [...(section?.querySelectorAll(".memory-site-table tbody tr.memory-site-row") || [])]
      .map(row => [...row.children].map(cell => cell.textContent.trim()));
    const frames = [...(section?.querySelectorAll(".memory-site-stack-row:not([hidden]) .memory-site-frame") || [])]
      .map(row => ({
        index: row.querySelector(".memory-site-frame-index")?.textContent.trim() ?? "",
        name: row.querySelector(".memory-site-frame-name")?.textContent.trim() ?? "",
        location: row.querySelector(".memory-site-frame-location")?.textContent.trim() ?? "",
        address: row.querySelector(".memory-site-frame-address")?.textContent.trim() ?? "",
      }));
    return {
      present: Boolean(section),
      badge: q(".panel-block-head .state-badge")?.textContent.trim() ?? "",
      badgeClass: q(".panel-block-head .state-badge")?.className ?? "",
      meta: q(".panel-block-head .pane-meta")?.textContent.trim() ?? "",
      scope: q(".memory-scope")?.textContent.trim() ?? "",
      footnote: [...(section?.querySelectorAll(".panel-block-body > .memory-note") || [])].map(node => node.textContent.trim()).join(" "),
      stateTitle: q(".memory-site-state strong")?.textContent.trim() ?? "",
      stateMessage: q(".memory-site-state p")?.textContent.trim() ?? "",
      rate: document.getElementById("alloc-site-rate")?.value ?? "",
      rateDisabled: document.getElementById("alloc-site-rate")?.disabled ?? null,
      start: control("Start"),
      stop: control("Stop"),
      query: control("Query"),
      clear: control("Clear"),
      metrics: {
        bytes: metric("Sampled bytes"),
        blocks: metric("Sampled blocks"),
        attempted: metric("Attempted"),
        retained: metric("Retained"),
        dropped: metric("Dropped"),
        failures: metric("Capture failures"),
      },
      headers: [...(q(".memory-site-table thead tr")?.children || [])].map(cell => cell.textContent.trim()),
      rows,
      notices: qa(".memory-site-truncation .memory-note").map(node => node.textContent.trim()),
      detailsVisible: qa(".memory-site-stack-row:not([hidden])").length,
      frames,
      omitted: q(".memory-site-stack-omitted")?.textContent.trim() ?? "",
      toggleCount: qa(".memory-site-toggle").length,
      toggleExpanded: qa(".memory-site-toggle")[0]?.getAttribute("aria-expanded") ?? "",
      tablePresent: Boolean(q(".memory-site-table")),
      statePresent: Boolean(q(".memory-site-state")),
    };
  })()`);
}

async function auditAllocationDom(cdp) {
  const report = await cdp.evaluate(`(() => {
    const failures = [];
    const view = document.getElementById("view-memory");
    const section = document.getElementById("memory-allocation-sites");
    if (!section) failures.push("allocation sites section is missing");
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
    for (const table of document.querySelectorAll("#view-memory .data-table")) {
      if (!table.querySelector("thead th")) failures.push("table without a header row");
      if (!table.querySelector("tbody tr")) failures.push("table without body rows");
    }
    const rate = document.getElementById("alloc-site-rate");
    if (!rate) failures.push("rate input is missing");
    if (rate && (rate.getAttribute("min") !== "1" || !rate.getAttribute("max"))) failures.push("rate input lacks bounds");
    if (rate && !rate.closest("label") && !document.querySelector('label[for="' + rate.id + '"]')) failures.push("rate input is unlabeled");
    const buttons = [...section.querySelectorAll(".profile-actions button")].map(button => button.textContent.trim());
    for (const label of ["Start", "Stop", "Query", "Clear"]) {
      if (!buttons.includes(label)) failures.push("missing control: " + label);
    }
    if (section.querySelectorAll(".memory-scope").length !== 1) failures.push("allocation scope note is not singular");
    const rawText = section.textContent;
    if (!/Cumulative sampled allocations, not live heap/.test(rawText)) failures.push("cumulative-not-live label is missing");
    if (!/not live heap, object counts, object types, retaining paths or leaks/.test(rawText)) failures.push("scope exclusions are missing");
    return failures;
  })()`);
  assert.deepEqual(report, [], `Allocation sites DOM audit failed:\n${report.join("\n")}`);
  console.log("PASS  DOM audit: unique ids, named/typed controls, bounded rate input, table headers, scoped labels");
}

async function runScenario(mode, body) {
  const server = await startServer(mode);
  const profile = await mkdtemp(join(tmpdir(), "cove-allocation-sites-"));
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

async function scenarioLifecycle() {
  await runScenario("live", async ({ cdp, fake }) => {
    await auditAllocationDom(cdp);
    let view = await allocationPage(cdp);
    assert.equal(view.badge, "Idle");
    assert.equal(view.start.disabled, false);
    assert.equal(view.stop.disabled, true);
    assert.equal(view.query.disabled, false);
    assert.equal(view.clear.disabled, true);
    assert.equal(view.rate, "1024");
    assert.match(view.scope, /Cumulative sampled allocations, not live heap/);
    assert.match(view.scope, /Fixed ring|raw PC stack/);
    assert.match(view.scope, /retaining paths or leaks/);
    assert.equal(view.stateTitle, "Not sampling");
    assert.equal(view.tablePresent, false);

    await setAllocationRate(cdp, 8);
    await clickAllocationControl(cdp, "Start");
    await waitFor(cdp, "running state", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Running"`);
    await waitFor(cdp, "three sites", `document.querySelectorAll("#memory-allocation-sites .memory-site-table tbody tr.memory-site-row").length === 3`);
    view = await allocationPage(cdp);
    assert.equal(view.start.disabled, true);
    assert.equal(view.stop.disabled, false);
    assert.equal(view.rateDisabled, true);
    assert.equal(view.rate, "8");
    assert.match(view.meta, /1-in-8/);
    assert.match(view.meta, /retained/);
    assert.deepEqual(view.headers, ["#", "Sampled bytes", "Blocks", "Share", "Sampling stack"]);
    assert.deepEqual(view.rows.map(row => [row[0], row[1], row[2], row[3]]), [
      ["1", "256.0 KB", "4", "57.1%"],
      ["2", "128.0 KB", "32", "28.6%"],
      ["3", "64.0 KB", "512", "14.3%"],
    ]);
    assert.match(view.rows[0][4], /gallery::render/);
    assert.match(view.rows[0][4], /main\.rs:120/);
    assert.match(view.rows[0][4], /0x104a2b3c4/);
    assert.equal(view.metrics.bytes, "448.0 KB");
    assert.equal(view.metrics.blocks, "548");
    assert.equal(view.metrics.attempted, "4");
    assert.equal(view.metrics.retained, "3");
    assert.equal(view.metrics.dropped, "1");
    assert.equal(view.metrics.failures, "0");

    const beforePoll = Number(view.metrics.attempted);
    await new Promise(resolve => setTimeout(resolve, 1250));
    view = await allocationPage(cdp);
    assert.ok(Number(view.metrics.attempted) > beforePoll, "live polling must advance the attempted counter");

    await clickAllocationControl(cdp, "Query");
    const queriedAttempts = Number((await allocationPage(cdp)).metrics.attempted);
    assert.ok(queriedAttempts >= 8, "manual query must read fresh live counters");
    await waitFor(cdp, "manual query settles", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Running"`);

    await click(cdp, "#memory-allocation-sites .memory-site-toggle");
    await waitFor(cdp, "expanded stack", `document.querySelectorAll("#memory-allocation-sites .memory-site-stack-row:not([hidden]) .memory-site-frame").length === 3`);
    view = await allocationPage(cdp);
    assert.equal(view.toggleExpanded, "true");
    assert.equal(view.detailsVisible, 1);
    assert.deepEqual(view.frames.map(frame => frame.index), ["0", "1", "2"]);
    assert.equal(view.frames[0].name, "gallery::render");
    assert.equal(view.frames[0].location, "/Users/dev/cove/examples/gallery/src/main.rs:120");
    assert.equal(view.frames[0].address, "0x104a2b3c4");
    assert.equal(view.frames[2].name, "cove_core::scene::layer::paint_layers");
    assert.match(view.frames[2].location, /layer\.rs:318/);
    assert.equal(fake.calls.filter(call => call.method === "query_ui_allocation_sites").length >= 2, true);
    await screenshot(cdp, process.env.ALLOCATION_SITES_SHOT);

    await clickAllocationControl(cdp, "Stop");
    await waitFor(cdp, "stopped state", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Stopped"`);
    await waitFor(cdp, "stopped sites remain", `document.querySelectorAll("#memory-allocation-sites .memory-site-table tbody tr.memory-site-row").length === 3`);
    view = await allocationPage(cdp);
    assert.equal(view.start.disabled, false);
    assert.equal(view.stop.disabled, true);
    assert.equal(view.clear.disabled, false);
    assert.equal(fake.sessions.get("runtime-1").running, false);
    assert.match(view.meta, /stopped/);
    const stoppedAttempts = view.metrics.attempted;

    await clickAllocationControl(cdp, "Query");
    await waitFor(cdp, "stopped query settles", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Stopped"`);
    view = await allocationPage(cdp);
    assert.equal(view.tablePresent, true, "a stopped query must keep the last real report");
    assert.equal(view.metrics.attempted, stoppedAttempts);

    await clickAllocationControl(cdp, "Clear");
    await waitFor(cdp, "stopped report cleared", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Idle"`);
    view = await allocationPage(cdp);
    assert.equal(view.tablePresent, false);
    assert.equal(view.clear.disabled, true);
    assert.equal(view.stateTitle, "Not sampling");
  });
  console.log("PASS  idle → start → live counters → drill-down → stop → query → clear");
}

async function scenarioEmpty() {
  await runScenario("empty", async ({ cdp }) => {
    await setAllocationRate(cdp, 8);
    await clickAllocationControl(cdp, "Start");
    await waitFor(cdp, "empty running", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Running"`);
    await waitFor(cdp, "empty state", `document.querySelector("#memory-allocation-sites .memory-site-state strong")?.textContent === "No sampled allocations yet"`);
    let view = await allocationPage(cdp);
    assert.equal(view.tablePresent, false);
    assert.equal(view.rows.length, 0);
    assert.equal(view.metrics.bytes, "0 B");
    assert.equal(view.metrics.blocks, "0");
    assert.equal(view.metrics.attempted, "4");
    assert.equal(view.metrics.retained, "0");
    assert.equal(view.metrics.failures, "3");
    assert.match(view.stateMessage, /no site is inferred/);

    await clickAllocationControl(cdp, "Stop");
    await waitFor(cdp, "empty stopped", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Stopped"`);
    view = await allocationPage(cdp);
    assert.equal(view.stateTitle, "No retained allocation sites");
    assert.equal(view.tablePresent, false);
    assert.match(view.stateMessage, /attempts, drops and capture failures/);
  });
  console.log("PASS  empty running and stopped states keep zero real counters and no invented site");
}

async function scenarioNoCapability() {
  await runScenario("no-capability", async ({ cdp, fake }) => {
    const view = await allocationPage(cdp);
    assert.equal(view.badge, "Unavailable");
    assert.equal(view.start.disabled, true);
    assert.equal(view.stop.disabled, true);
    assert.equal(view.rateDisabled, true);
    assert.equal(view.tablePresent, false);
    assert.match(view.stateMessage, /does not advertise allocation_sites/);
    assert.equal(
      fake.calls.filter(call => call.method === "query_ui_allocation_sites").length,
      0,
      "a missing capability must not be probed",
    );
    await clickAllocationControl(cdp, "Query");
    await waitFor(cdp, "capability query refused", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Unavailable"`);
    const after = await allocationPage(cdp);
    assert.match(after.stateMessage, /does not advertise allocation_sites/);
    assert.equal(after.start.disabled, true);
  });
  console.log("PASS  an unadvertised capability shows unavailable immediately with no sampler probe");
}

async function scenarioUnsupported() {
  await runScenario("unsupported", async ({ cdp }) => {
    await setAllocationRate(cdp, 8);
    await clickAllocationControl(cdp, "Start");
    await waitFor(cdp, "unsupported state", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Unsupported"`);
    const view = await allocationPage(cdp);
    assert.equal(view.stateTitle, "Allocation sampling is unavailable");
    assert.match(view.stateMessage, /not available on android/);
    assert.equal(view.start.disabled, true);
    assert.equal(view.stop.disabled, true);
    assert.equal(view.rateDisabled, true);
    assert.equal(view.tablePresent, false);
    assert.equal(view.rows.length, 0);
    assert.match(view.meta, /not available/);
  });
  console.log("PASS  unsupported target is explicit, disables Start/rate and invents no sites");
}

async function scenarioAlias() {
  await runScenario("alias", async ({ cdp, fake }) => {
    await setAllocationRate(cdp, 8);
    await clickAllocationControl(cdp, "Start");
    await waitFor(cdp, "alias running", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Running"`);
    await waitFor(cdp, "alias sites", `document.querySelectorAll("#memory-allocation-sites .memory-site-table tbody tr.memory-site-row").length === 3`);
    const startCalls = fake.calls.filter(call => call.method.startsWith("start_ui_"));
    assert.deepEqual(startCalls.map(call => call.method), [
      "start_ui_allocation_profile",
      "start_ui_allocation_sites",
    ]);
    await clickAllocationControl(cdp, "Stop");
    await waitFor(cdp, "alias stopped", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Stopped"`);
    const stopCalls = fake.calls.filter(call => call.method.startsWith("stop_ui_"));
    assert.deepEqual(stopCalls.map(call => call.method), [
      "stop_ui_allocation_profile",
      "stop_ui_allocation_sites",
    ]);
  });
  console.log("PASS  unknown start/stop command falls back to the controller's sampling name without fabricating state");
}

async function scenarioTruncation() {
  await runScenario("truncated", async ({ cdp }) => {
    await setAllocationRate(cdp, 16);
    await clickAllocationControl(cdp, "Start");
    await waitFor(cdp, "truncated running", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Running"`);
    await waitFor(cdp, "truncation notices", `document.querySelectorAll("#memory-allocation-sites .memory-site-truncation .memory-note").length >= 2`);
    let view = await allocationPage(cdp);
    assert.equal(view.rows.length, 2);
    assert.ok(view.notices.some(text => /Showing 2 of 57 sampled sites/.test(text)));
    assert.ok(view.notices.some(text => /Stacks are bounded at 32 frames/.test(text)));
    assert.match(view.meta, /1-in-16/);
    await click(cdp, "#memory-allocation-sites .memory-site-toggle");
    await waitFor(cdp, "capped stack", `document.querySelectorAll("#memory-allocation-sites .memory-site-stack-row:not([hidden]) .memory-site-frame").length === 32`);
    view = await allocationPage(cdp);
    assert.equal(view.frames.length, 32);
    assert.match(view.omitted, /Capture reached the frame limit; deeper callers were not recorded/);
    assert.equal(view.frames[0].name, "gallery::deep_allocation");
    await clickAllocationControl(cdp, "Stop");
    await waitFor(cdp, "truncated stopped", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Stopped"`);
  });
  console.log("PASS  site and frame truncation are reported; deep stacks are capped with an omitted count");
}

async function scenarioUnsymbolized() {
  await runScenario("unsymbolized", async ({ cdp }) => {
    await setAllocationRate(cdp, 8);
    await clickAllocationControl(cdp, "Start");
    await waitFor(cdp, "unsymbolized running", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Running"`);
    await waitFor(cdp, "unsymbolized notice", `[...document.querySelectorAll("#memory-allocation-sites .memory-site-truncation .memory-note")].some(node => /could not be symbolized/.test(node.textContent))`);
    const view = await allocationPage(cdp);
    assert.equal(view.rows.length, 1);
    assert.ok(view.notices.some(text => /could not be symbolized: cove_profiling has no symbolizer on this platform/.test(text)));
    await click(cdp, "#memory-allocation-sites .memory-site-toggle");
    await waitFor(cdp, "raw address frame", `document.querySelectorAll("#memory-allocation-sites .memory-site-stack-row:not([hidden]) .memory-site-frame").length === 1`);
    const expanded = await allocationPage(cdp);
    assert.equal(expanded.frames[0].name, "0x104c0ffee");
    assert.equal(expanded.frames[0].location, "");
    await clickAllocationControl(cdp, "Stop");
    await waitFor(cdp, "unsymbolized stopped", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Stopped"`);
  });
  console.log("PASS  unsymbolized frames keep their real address and state the controller's reason");
}

async function scenarioError() {
  await runScenario("live", async ({ cdp, fake }) => {
    fake.state.failQuery = true;
    await clickAllocationControl(cdp, "Query");
    await waitFor(cdp, "query error", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Error"`);
    let view = await allocationPage(cdp);
    assert.equal(view.stateTitle, "Allocation-site read failed");
    assert.match(view.stateMessage, /sampler lock unavailable/);
    assert.equal(view.start.disabled, false);
    assert.equal(view.tablePresent, false);
    assert.equal(await cdp.evaluate(`document.querySelectorAll("#memory-allocation-sites .memory-site-state .tool-button").length`), 1);

    fake.state.failQuery = false;
    const retry = await cdp.evaluate(`(() => {
      const button = [...document.querySelectorAll("#memory-allocation-sites .memory-site-state .tool-button")]
        .find(item => item.textContent.trim() === "Retry");
      if (!button) return false;
      button.click();
      return true;
    })()`);
    assert.equal(retry, true, "retry control exists");
    await waitFor(cdp, "query recovered", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Idle"`);
    view = await allocationPage(cdp);
    assert.equal(view.tablePresent, false);

    fake.state.failStart = true;
    await setAllocationRate(cdp, 8);
    await clickAllocationControl(cdp, "Start");
    await waitFor(cdp, "start error", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Error"`);
    view = await allocationPage(cdp);
    assert.match(view.stateMessage, /rejected the sampling session/);
    assert.equal(view.rows.length, 0);
    assert.equal(view.start.disabled, false);
  });
  console.log("PASS  query and start errors are actionable, recover on retry and fabricate no sites");
}

async function scenarioTargetSwitch() {
  await runScenario("targets", async ({ cdp, fake }) => {
    await setAllocationRate(cdp, 8);
    await clickAllocationControl(cdp, "Start");
    await waitFor(cdp, "first target running", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Running"`);
    await waitFor(cdp, "first target sites", `document.querySelectorAll("#memory-allocation-sites .memory-site-table tbody tr.memory-site-row").length === 3`);
    assert.equal(fake.sessions.get("runtime-1").running, true);

    const switched = await cdp.evaluate(`(() => {
      const select = document.getElementById("targets");
      select.value = "runtime-2";
      select.dispatchEvent(new Event("change", { bubbles: true }));
      return select.value;
    })()`);
    assert.equal(switched, "runtime-2");
    await waitFor(cdp, "second target idle", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Idle" && document.getElementById("targets")?.value === "runtime-2"`, 10000);
    await waitUntil("previous sampler stopped", () => fake.calls.some(call => call.method === "stop_ui_allocation_profile" && call.params.target === "runtime-1"));
    const stopCalls = fake.calls.filter(call => call.method === "stop_ui_allocation_profile");
    assert.ok(stopCalls.length >= 1, "target switch must stop the previous target's sampler");
    assert.equal(stopCalls[0].params.target, "runtime-1");
    assert.equal(fake.sessions.get("runtime-1").running, false);
    assert.equal(fake.sessions.get("runtime-2").running, false);
    const view = await allocationPage(cdp);
    assert.equal(view.badge, "Idle");
    assert.equal(view.tablePresent, false);
    assert.equal(view.rate, "1024");
    assert.match(view.meta, /not sampling/);
  });
  console.log("PASS  target switch stops the previous target's real sampler and resets the view");
}

async function scenarioDisconnect() {
  await runScenario("live", async ({ cdp, fake }) => {
    await setAllocationRate(cdp, 8);
    await clickAllocationControl(cdp, "Start");
    await waitFor(cdp, "disconnect target running", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Running"`);
    await waitFor(cdp, "disconnect sites", `document.querySelectorAll("#memory-allocation-sites .memory-site-table tbody tr.memory-site-row").length === 3`);
    fake.state.unauthorized = true;
    await clickAllocationControl(cdp, "Query");
    await waitFor(cdp, "offline state", `document.querySelector("#memory-allocation-sites .state-badge")?.textContent.trim() === "Disconnected"`, 10000);
    const view = await allocationPage(cdp);
    assert.equal(view.stateTitle, "Disconnected");
    assert.match(view.stateMessage, /connection dropped/i);
    assert.equal(view.start.disabled, true);
    assert.equal(view.stop.disabled, true);
    assert.equal(view.query.disabled, true);
    assert.equal(view.tablePresent, false);
    const queries = fake.calls.filter(call => call.method === "query_ui_allocation_sites").length;
    await new Promise(resolve => setTimeout(resolve, 1300));
    assert.equal(
      fake.calls.filter(call => call.method === "query_ui_allocation_sites").length,
      queries,
      "polling must stop after the controller disconnects",
    );
  });
  console.log("PASS  disconnect clears polling, disables controls and reports unknown sampling state");
}

console.log("Allocation sites harness: fake controller + headless Chrome");
await scenarioLifecycle();
await scenarioEmpty();
await scenarioNoCapability();
await scenarioUnsupported();
await scenarioAlias();
await scenarioTruncation();
await scenarioUnsymbolized();
await scenarioError();
await scenarioTargetSwitch();
await scenarioDisconnect();
console.log("RESULT PASS");
