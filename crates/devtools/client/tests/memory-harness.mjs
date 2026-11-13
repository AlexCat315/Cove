#!/usr/bin/env node
// Fake-controller harness for the Memory view's allocator section. It serves the
// real index.html/inspector.js/inspector.css, answers /rpc with the controller's
// memory wire shape (DiagnosticMemorySnapshot plus optional allocator counters),
// and drives the page through Chrome DevTools Protocol. No Rust process starts
// and no allocation data is invented outside the fake controller's responses.
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import assert from "node:assert/strict";
import { serveClientAsset } from "./harness-server.mjs";

const chromePath = process.env.CHROME_PATH
  || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

function cacheCounters() {
  return {
    processResidentBytes: 104857600,
    processVirtualBytes: 536870912,
    shapingEntries: 12,
    shapingBytes: 4096,
    layoutEntries: 8,
    layoutBytes: 2048,
    imageEntries: 3,
    imageBytes: 8192,
    skippedParentUpdates: 5,
  };
}

function windows() {
  return {
    "window-1": {
      targetBytes: 1048576,
      msaaLogicalBytes: 524288,
      msaaAllocatedBytes: 262144,
      samples: 2,
      suspended: 0,
      hasSwapchain: 1,
    },
  };
}

// Eight allocation counters plus the flat size-class byte counters the real
// cove_alloc tracker publishes. The values are internally consistent:
// total - deallocated = current for both bytes and blocks.
function allocatorSnapshot() {
  return {
    counters: {
      ...cacheCounters(),
      allocCurrentBytes: 16384,
      allocPeakBytes: 32768,
      allocTotalBytes: 65536,
      allocLiveBlocks: 96,
      allocPeakLiveBlocks: 120,
      allocTotalBlocks: 240,
      allocDeallocatedBytes: 49152,
      allocDeallocatedBlocks: 144,
      allocSizeClass64Bytes: 1012874,
      allocSizeClass256Bytes: 1012209,
      allocSizeClass1KiBBytes: 4514738,
      allocSizeClass4KiBBytes: 4134607,
      allocSizeClass16KiBBytes: 812042,
      allocSizeClass64KiBBytes: 553080,
      allocSizeClass1MiBBytes: 913680,
      allocSizeClassOver1MiBBytes: 21702288,
    },
    windows: windows(),
  };
}

// A host with a tracking allocator that only reports current values, and only
// one measured size class.
function partialSnapshot() {
  return {
    counters: {
      ...cacheCounters(),
      processResidentBytes: 52428800,
      allocCurrentBytes: 2048,
      allocLiveBlocks: 12,
      allocSizeClass64Bytes: 2048,
    },
    windows: windows(),
  };
}

function cacheOnlySnapshot() {
  return {
    counters: cacheCounters(),
    windows: windows(),
  };
}

// A provider whose readings move between polls, so the overview chart has a
// real series to draw. Read index 2 reports a subscribed low-pressure signal.
function seriesSnapshot(read) {
  return {
    counters: {
      ...cacheCounters(),
      processResidentBytes: (100 + read * 20) * 1048576,
      processVirtualBytes: 536870912,
      allocCurrentBytes: (16 + read * 4) * 1048576,
      allocLiveBlocks: 96 + read,
    },
    windows: windows(),
    pressure: { subscribed: true, low: read === 2 },
  };
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
  let seriesReads = 0;

  function session() {
    return {
      platform: "macos",
      permissions: { screen_capture: false, accessibility: false },
      targets: [
        {
          id: "runtime-1",
          title: "Memory Target",
          platform: "macos",
          pid: 4242,
          kind: "runtime",
          capabilities: ["tree", "diagnostics"],
        },
      ],
    };
  }

  function memoryResponse() {
    if (mode === "null") return { ok: true, data: null };
    if (mode === "series") return { ok: true, data: seriesSnapshot(seriesReads++) };
    if (mode === "allocator") return { ok: true, data: allocatorSnapshot() };
    if (mode === "partial") return { ok: true, data: partialSnapshot() };
    return { ok: true, data: cacheOnlySnapshot() };
  }

  function handle(method) {
    calls.push(method);
    if (method === "get_ui_session") return session();
    if (method === "query_ui_memory") return memoryResponse();
    throw rpcError("unknown_command", `Unknown command ${method}.`);
  }

  return { calls, handle };
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
    `--window-size=${process.env.MEMORY_WINDOW || "1440,900"}`,
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

async function waitUntil(description, predicate, timeout = 8000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await new Promise(resolve => setTimeout(resolve, 40));
  }
  throw new Error(`Timed out waiting for ${description}.`);
}

function close(a, b, tolerance, message) {
  assert.ok(Math.abs(a - b) <= tolerance, `${message}: ${a} vs ${b} (tolerance ${tolerance})`);
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

async function openMemory(cdp) {
  await waitFor(cdp, "controller connection", `document.getElementById("connection-text")?.textContent === "Connected"`);
  await click(cdp, "#tab-memory");
  await waitFor(cdp, "memory view", `document.getElementById("view-memory")?.hidden === false`);
  await waitFor(cdp, "memory response", `document.querySelector("#memory-content .memory-scope, #memory-content .state strong") !== null`);
}

async function screenshot(cdp, path) {
  if (!path) return;
  const shot = await cdp.send("Page.captureScreenshot", { format: "png" });
  await writeFile(path, Buffer.from(shot.data, "base64"));
}

async function page(cdp) {
  return cdp.evaluate(`(() => {
    const host = document.getElementById("memory-content");
    const q = selector => host?.querySelector(selector);
    const qa = selector => [...(host?.querySelectorAll(selector) || [])];
    const stat = label => {
      const node = qa(".stat").find(row => row.querySelector(".stat-label")?.textContent === label);
      return node?.querySelector(".stat-value")?.textContent.trim() ?? "";
    };
    const block = title => qa(".panel-block").find(item => item.querySelector("h2")?.textContent === title);
    const rows = table => [...(table?.querySelectorAll("tbody tr") || [])]
      .map(row => [...row.children].map(cell => cell.textContent.trim()));
    const allocator = q(".memory-allocation");
    const histogram = q(".memory-size-classes");
    return {
      meta: document.getElementById("memory-meta")?.textContent.trim() ?? "",
      scope: q(".memory-scope")?.textContent.trim() ?? "",
      stateTitle: q(".state strong")?.textContent.trim() ?? "",
      stateMessage: q(".state p")?.textContent.trim() ?? "",
      stats: {
        rss: stat("Process RSS"),
        virtual: stat("Process virtual"),
        caches: stat("Logical caches"),
        imageAssets: stat("Image assets"),
        targets: stat("Render targets"),
        windows: stat("Windows"),
      },
      allocator: allocator ? {
        title: allocator.querySelector("h2")?.textContent.trim() ?? "",
        meta: allocator.querySelector(".pane-meta")?.textContent.trim() ?? "",
        note: allocator.querySelector(".memory-note")?.textContent.trim() ?? "",
        rows: rows(allocator.querySelector("table")),
      } : null,
      histogram: histogram ? {
        title: histogram.querySelector("h2")?.textContent.trim() ?? "",
        meta: histogram.querySelector(".pane-meta")?.textContent.trim() ?? "",
        headers: [...histogram.querySelectorAll("thead th")].map(cell => cell.textContent.trim()),
        rows: rows(histogram.querySelector("table")),
      } : null,
      cacheRows: rows(block("Cache and render counters")?.querySelector("table")),
      windowRows: rows(block("Per-window render targets")?.querySelector("table")),
    };
  })()`);
}

async function auditMemoryDom(cdp) {
  const report = await cdp.evaluate(`(() => {
    const failures = [];
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
    const tables = [...document.querySelectorAll("#view-memory .data-table")];
    for (const table of tables) {
      if (!table.querySelector("thead th")) failures.push("table without a header row");
      if (!table.querySelector("tbody tr")) failures.push("table without body rows");
    }
    const allocation = document.querySelector("#view-memory .memory-allocation");
    if (!allocation) failures.push("allocation panel is missing");
    return failures;
  })()`);
  assert.deepEqual(report, [], `Memory DOM audit failed:\n${report.join("\n")}`);
  console.log("PASS  DOM audit: unique ids, live memory view references, icon symbols, named controls with type, tables with headers and rows");
}

let audited = false;

async function runScenario(mode, body) {
  const server = await startServer(mode);
  const profile = await mkdtemp(join(tmpdir(), "cove-memory-harness-"));
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

async function scenarioAllocator() {
  await runScenario("allocator", async ({ cdp, fake }) => {
    await waitFor(cdp, "allocation rows", `document.querySelectorAll("#memory-content .memory-allocation tbody tr").length === 4`);
    if (!audited) {
      await auditMemoryDom(cdp);
      audited = true;
    }
    const view = await page(cdp);
    assert.equal(view.allocator.title, "Allocator tracking");
    assert.equal(view.allocator.meta, "8 of 8 reported");
    assert.deepEqual(view.allocator.rows, [
      ["Current", "16.0 KB", "96"],
      ["Peak", "32.0 KB", "120"],
      ["Total", "64.0 KB", "240"],
      ["Deallocated", "48.0 KB", "144"],
    ]);
    assert.match(view.scope, /Allocator tracking, not a heap object graph/);
    assert.match(view.scope, /Rust has no GC/);
    assert.match(view.scope, /retaining paths/);
    assert.match(view.scope, /object types are not tracked/);
    assert.match(view.scope, /drawable pools/);
    assert.match(view.meta, /16\.0 KB allocated/);
    assert.match(view.meta, /RSS and allocator counters are measured; caches are estimates/);
    assert.equal(view.stats.rss, "100.0 MB");
    assert.equal(view.stats.virtual, "512.0 MB");
    assert.equal(view.histogram.title, "Allocator size classes");
    assert.equal(view.histogram.meta, "8 classes");
    assert.deepEqual(view.histogram.headers, ["Size class", "Live bytes"]);
    assert.deepEqual(view.histogram.rows, [
      ["64 B", "989.1 KB"],
      ["256 B", "988.5 KB"],
      ["1 KiB", "4.3 MB"],
      ["4 KiB", "3.9 MB"],
      ["16 KiB", "793.0 KB"],
      ["64 KiB", "540.1 KB"],
      ["1 MiB", "892.3 KB"],
      ["Over 1 MiB", "20.7 MB"],
    ]);
    assert.deepEqual(view.cacheRows.map(row => row[0]), ["Shaping cache", "Text layout cache", "Image cache"]);
    assert.equal(view.cacheRows[0][2], "4.0 KB");
    assert.equal(view.windowRows.length, 1);
    assert.equal(fake.calls.filter(name => name === "query_ui_memory").length >= 1, true);
    await screenshot(cdp, process.env.MEMORY_SHOT);
  });
}

async function scenarioPartial() {
  await runScenario("partial", async ({ cdp }) => {
    await waitFor(cdp, "partial allocation rows", `document.querySelectorAll("#memory-content .memory-allocation tbody tr").length === 4`);
    const view = await page(cdp);
    assert.equal(view.allocator.meta, "2 of 8 reported");
    assert.deepEqual(view.allocator.rows, [
      ["Current", "2.0 KB", "12"],
      ["Peak", "Not reported", "Not reported"],
      ["Total", "Not reported", "Not reported"],
      ["Deallocated", "Not reported", "Not reported"],
    ]);
    assert.equal(view.histogram.meta, "1 class");
    assert.deepEqual(view.histogram.headers, ["Size class", "Live bytes"]);
    assert.deepEqual(view.histogram.rows, [["64 B", "2.0 KB"]]);
    assert.equal(view.stats.rss, "50.0 MB");
    assert.match(view.meta, /2\.0 KB allocated/);
    assert.equal(view.cacheRows.length, 3);
    const cacheText = view.cacheRows.flat().join(" ");
    assert.doesNotMatch(cacheText, /allocCurrentBytes|allocLiveBlocks|allocSizeClass64Bytes/);
    assert.doesNotMatch(view.allocator.rows.flat().join(" "), /(^|\\s)0 B(\\s|$)/);
  });
}

async function scenarioCacheOnly() {
  await runScenario("cache-only", async ({ cdp }) => {
    await waitFor(cdp, "allocator section", `document.querySelector("#memory-content .memory-allocation") !== null`);
    const view = await page(cdp);
    assert.equal(view.allocator.title, "Allocator tracking");
    assert.equal(view.allocator.meta, "Not reported");
    assert.equal(view.allocator.rows.length, 0);
    assert.match(view.allocator.note, /no tracking allocator/);
    assert.match(view.allocator.note, /not reused as allocation totals/);
    assert.equal(view.histogram, null);
    assert.equal(view.stats.rss, "100.0 MB");
    assert.equal(view.stats.caches, "14.0 KB estimated");
    assert.equal(view.cacheRows.length, 3);
    assert.equal(view.windowRows.length, 1);
    assert.match(view.meta, /no allocator counters are reported/);
    assert.match(view.scope, /Rust has no GC/);
    assert.equal(view.stateTitle, "");
  });
}

async function scenarioNull() {
  await runScenario("null", async ({ cdp }) => {
    await waitFor(cdp, "provider state", `document.querySelector("#memory-content .state strong")?.textContent === "This host has no memory provider"`);
    const view = await page(cdp);
    assert.equal(view.allocator, null);
    assert.equal(view.histogram, null);
    assert.match(view.stateMessage, /nothing is substituted/);
    assert.match(view.meta, /No process or cache readings are available/);
  });
}

function chartScript() {
  return `(() => {
    const block = [...document.querySelectorAll("#memory-content .panel-block")]
      .find(node => node.querySelector("h2")?.textContent === "Memory overview");
    if (!block) return null;
    const plot = block.querySelector(".memory-chart-plot");
    const clip = node => node ? node.style.clipPath : "";
    return {
      meta: block.querySelector(".pane-meta")?.textContent || "",
      empty: block.querySelector(".empty-inline")?.textContent || "",
      hasCanvas: block.querySelector("canvas") !== null,
      lines: [...block.querySelectorAll(".memory-line")].map(node => ({
        key: [...node.classList].find(name => name.startsWith("is-")).slice(3),
        clip: clip(node),
      })),
      areas: [...block.querySelectorAll(".memory-area")].map(node => [...node.classList].find(name => name.startsWith("is-")).slice(3)),
      dots: [...block.querySelectorAll(".memory-dot")].map(node => [...node.classList].find(name => name.startsWith("is-")).slice(3)),
      pressure: block.querySelectorAll(".memory-pressure").length,
      pressureTitles: [...block.querySelectorAll(".memory-pressure")].map(node => node.getAttribute("title")),
      yaxis: [...block.querySelectorAll(".memory-yaxis span")].map(node => node.textContent),
      xaxis: [...block.querySelectorAll(".memory-xaxis span")].map(node => node.textContent),
      readout: block.querySelector(".memory-chart-readout")?.textContent || "",
      cursorHidden: block.querySelector(".memory-cursor")?.hidden ?? true,
      legend: [...block.querySelectorAll(".memory-legend-toggle")].map(node => ({
        key: [...node.classList].find(name => name.startsWith("is-")).slice(3),
        on: node.classList.contains("is-on"),
        pressed: node.getAttribute("aria-pressed"),
        text: node.textContent,
      })),
      notes: [...block.querySelectorAll(".perf-note")].map(node => node.textContent),
      plotRect: plot ? (() => { const r = plot.getBoundingClientRect(); return { left: r.left, top: r.top, width: r.width, height: r.height }; })() : null,
      pressureStat: (() => {
        const node = [...document.querySelectorAll("#memory-content .stat")]
          .find(row => row.querySelector(".stat-label")?.textContent === "Memory pressure");
        return node?.querySelector(".stat-value")?.textContent.trim() ?? "";
      })(),
    };
  })()`;
}

function niceByteScale(max) {
  if (max <= 0) return 1024;
  const base = 10 ** Math.floor(Math.log10(max));
  for (const step of [1, 2, 5, 10]) {
    if (step * base >= max) return step * base;
  }
  return 10 * base;
}

function fmtBytes(value) {
  if (value < 1024) return `${value} B`;
  if (value < 1048576) return `${(value / 1024).toFixed(1)} KB`;
  if (value < 1073741824) return `${(value / 1048576).toFixed(1)} MB`;
  return `${(value / 1073741824).toFixed(2)} GB`;
}

async function refreshMemory(cdp, fake) {
  const before = fake.calls.filter(name => name === "query_ui_memory").length;
  await click(cdp, "#memory-refresh");
  await waitFor(cdp, "memory refresh", `(() => {
    const button = document.getElementById("memory-refresh");
    return button.disabled === false;
  })()`);
  await waitUntil("refresh read", () => fake.calls.filter(name => name === "query_ui_memory").length > before);
  await new Promise(resolve => setTimeout(resolve, 120));
}

async function scenarioOverviewChart() {
  await runScenario("series", async ({ cdp, fake }) => {
    // One reading: every series is a single point, never an invented trend.
    await waitFor(cdp, "overview chart", `document.querySelector("#memory-content .memory-chart-plot") !== null`);
    let chart = await cdp.evaluate(chartScript());
    assert.equal(chart.hasCanvas, false, "the overview chart stays DOM/CSS");
    assert.match(chart.meta, /1 sample ·/);
    assert.deepEqual(chart.dots.sort(), ["allocated", "caches", "rss", "targets"]);
    assert.equal(chart.lines.length, 0);
    assert.ok(chart.notes.some(note => /a trend needs at least two readings/.test(note)));
    assert.equal(chart.pressure, 0, "a normal pressure sample draws no mark");
    assert.equal(chart.pressureStat, "Normal");

    // Three more readings produce real lines whose geometry matches the
    // provider's bytes, not the row order.
    await refreshMemory(cdp, fake);
    await refreshMemory(cdp, fake);
    await refreshMemory(cdp, fake);
    chart = await cdp.evaluate(chartScript());
    assert.match(chart.meta, /4 samples ·/);
    assert.equal(chart.dots.length, 0);
    assert.deepEqual(chart.lines.map(line => line.key).sort(), ["allocated", "caches", "rss", "targets"]);
    assert.deepEqual(chart.areas.sort(), ["allocated", "caches", "rss", "targets"]);
    const scale = niceByteScale(160 * 1048576);
    assert.equal(chart.yaxis[4], fmtBytes(scale), "the top tick labels the real byte scale");
    assert.equal(chart.yaxis[0], "0 B");
    const rss = chart.lines.find(line => line.key === "rss");
    const points = rss.clip.replace(/^polygon\(/, "").replace(/\)$/, "").split(", ").map(pair => pair.trim().split(" "));
    // Forward points then the reversed line band: the first four are the series.
    const forward = points.slice(0, 4);
    close(parseFloat(forward[0][0]), 0, 0.001, "the first sample sits at the axis origin");
    close(parseFloat(forward[3][0]), 100, 0.001, "the last sample sits at the axis end");
    close(parseFloat(forward[0][1]), 100 - ((100 * 1048576) / scale) * 100, 0.01, "the first RSS point matches 100 MiB");
    close(parseFloat(forward[3][1]), 100 - ((160 * 1048576) / scale) * 100, 0.01, "the last RSS point matches 160 MiB");
    close(parseFloat(forward[1][1]), 100 - ((120 * 1048576) / scale) * 100, 0.01, "the second RSS point matches 120 MiB");
    assert.ok(points.slice(4).every(pair => pair[1].startsWith("calc(")), "the line band keeps a constant pixel thickness");
    assert.equal(chart.pressure, 1, "only the subscribed low sample draws a mark");
    assert.match(chart.pressureTitles[0], /Memory-pressure signal at/);
    assert.ok(chart.notes.some(note => /Vertical marks are the host's subscribed memory-pressure signal/.test(note)));
    assert.equal(chart.pressureStat, "Normal", "the latest sample is normal again");
    assert.match(chart.readout, /Process RSS 160\.0 MB/);
    assert.match(chart.readout, /Allocator current 28\.0 MB/);
    assert.equal(chart.cursorHidden, true);
    assert.equal(chart.xaxis.length, 3);
    await screenshot(cdp, process.env.MEMORY_CHART_SHOT);

    // Hovering reads the nearest real sample; leaving returns to the latest.
    // The extremes are spacing-independent: the first and last readings.
    let rect = (await cdp.evaluate(chartScript())).plotRect;
    await cdp.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: rect.left + 2, y: rect.top + rect.height / 2 });
    await waitFor(cdp, "cursor readout", `(() => {
      const block = [...document.querySelectorAll("#memory-content .panel-block")].find(node => node.querySelector("h2")?.textContent === "Memory overview");
      return block.querySelector(".memory-cursor").hidden === false;
    })()`);
    chart = await cdp.evaluate(chartScript());
    assert.match(chart.readout, /Process RSS 100\.0 MB/, "the cursor reads the nearest sample, not an interpolation");
    rect = chart.plotRect;
    await cdp.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: rect.left + rect.width - 2, y: rect.top + rect.height / 2 });
    await waitFor(cdp, "cursor at last sample", `(() => {
      const block = [...document.querySelectorAll("#memory-content .panel-block")].find(node => node.querySelector("h2")?.textContent === "Memory overview");
      return /Process RSS 160\\.0 MB/.test(block.querySelector(".memory-chart-readout").textContent);
    })()`);
    await cdp.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: rect.left - 40, y: rect.top - 40 });
    await waitFor(cdp, "cursor hidden", `(() => {
      const block = [...document.querySelectorAll("#memory-content .panel-block")].find(node => node.querySelector("h2")?.textContent === "Memory overview");
      return block.querySelector(".memory-cursor").hidden === true;
    })()`);
    chart = await cdp.evaluate(chartScript());
    assert.match(chart.readout, /Process RSS 160\.0 MB/);

    // Legend toggles rescale the axis to the remaining series.
    await click(cdp, '.memory-legend-toggle.is-rss');
    await waitFor(cdp, "rss hidden", `document.querySelector("#memory-content .memory-line.is-rss") === null`);
    chart = await cdp.evaluate(chartScript());
    assert.equal(chart.legend.find(item => item.key === "rss").on, false);
    assert.equal(chart.legend.find(item => item.key === "rss").pressed, "false");
    assert.equal(chart.yaxis[4], fmtBytes(niceByteScale(28 * 1048576)), "hiding RSS rescales to the allocator series");
    assert.doesNotMatch(chart.readout, /Process RSS/);
    await click(cdp, '.memory-legend-toggle.is-virtual');
    await waitFor(cdp, "virtual shown", `document.querySelector("#memory-content .memory-line.is-virtual") !== null`);
    chart = await cdp.evaluate(chartScript());
    assert.equal(chart.yaxis[4], fmtBytes(niceByteScale(536870912)), "virtual dominates the scale when shown");
    await click(cdp, '.memory-legend-toggle.is-virtual');
    await click(cdp, '.memory-legend-toggle.is-rss');
    await waitFor(cdp, "rss restored", `document.querySelector("#memory-content .memory-line.is-rss") !== null`);

    // Follow appends real readings on the chosen interval.
    await cdp.evaluate(`(() => {
      const select = document.getElementById("memory-interval");
      select.value = "1000";
      select.dispatchEvent(new Event("change", { bubbles: true }));
    })()`);
    await click(cdp, "#memory-follow");
    const readsBeforeFollow = fake.calls.filter(name => name === "query_ui_memory").length;
    await waitUntil("follow polls", () => fake.calls.filter(name => name === "query_ui_memory").length >= readsBeforeFollow + 2, 9000);
    chart = await cdp.evaluate(chartScript());
    const followedSamples = parseInt(chart.meta, 10);
    assert.ok(followedSamples >= 6, `follow appended real readings: ${chart.meta}`);
    await click(cdp, "#memory-follow");
    const readsAfterStop = fake.calls.filter(name => name === "query_ui_memory").length;
    await new Promise(resolve => setTimeout(resolve, 1600));
    assert.equal(fake.calls.filter(name => name === "query_ui_memory").length, readsAfterStop, "unfollowing stops the poll");

    // Reset discards the series without touching the provider snapshot.
    await click(cdp, '#memory-content .panel-block-head button');
    await waitFor(cdp, "series reset", `document.querySelector("#memory-content .memory-chart-plot") === null`);
    chart = await cdp.evaluate(chartScript());
    assert.match(chart.empty, /No samples recorded yet/);
    assert.equal(chart.pressureStat, "Normal", "resetting the series keeps the current snapshot");
    await refreshMemory(cdp, fake);
    chart = await cdp.evaluate(chartScript());
    assert.match(chart.meta, /1 sample ·/);
  });
}

console.log("Memory allocation harness: fake controller + headless Chrome");
await scenarioAllocator();
console.log("PASS  allocator counters and flat size-class histogram");
await scenarioPartial();
console.log("PASS  partial allocator counters keep Not reported cells and flat size classes");
await scenarioCacheOnly();
console.log("PASS  no allocator reports Not reported while RSS/cache/render stay");
await scenarioNull();
console.log("PASS  null provider keeps the unavailable state with no allocation panel");
await scenarioOverviewChart();
console.log("PASS  overview chart: real sample series, byte-accurate geometry, pressure marks, hover readout, legend rescale, follow polling, reset");
console.log("RESULT PASS");
