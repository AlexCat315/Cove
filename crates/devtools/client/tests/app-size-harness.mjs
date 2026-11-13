#!/usr/bin/env node
// Fake-controller harness for the App Size view. It serves the real
// index.html/inspector.js/inspector.css, answers /rpc with the controller's
// native-executable wire shape, and drives the page through Chrome DevTools
// Protocol. It covers the existing tables and states, the proportional treemap
// with click-to-focus row selection, and the baseline snapshot diff including
// export, import, malformed snapshots and truncated lists. No Rust process
// starts and no size is invented outside the fake controller's responses.
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import assert from "node:assert/strict";
import { serveClientAsset } from "./harness-server.mjs";

const chromePath = process.env.CHROME_PATH
  || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

const executablePath = "/tmp/cove-app-size-harness/gallery";

function successPayload() {
  return {
    available: true,
    measurement: "native_executable",
    label: "Native executable of the selected runtime",
    attribution: "This measurement covers the selected runtime's own executable file. Flutter App Size's Dart AOT, package and asset attribution does not apply to Cove.",
    pid: 4242,
    executable: executablePath,
    format: "MachO",
    kind: "Executable",
    architecture: "Aarch64",
    file_bytes: 2097152,
    section_bytes: 1572864,
    section_count: 5,
    sections_returned: 5,
    sections_truncated: false,
    sections: [
      { name: "__text", segment: "__TEXT", kind: "Text", size: 1048576, address: 4294967296 },
      { name: "__data", segment: "__DATA", kind: "Data", size: 262144, address: 4294971392 },
      { name: "__cstring", segment: "__TEXT", kind: "ReadOnlyString", size: 65536, address: 4294968000 },
      { name: "__unwind_info", segment: "__TEXT", kind: "ReadOnlyData", size: 32768, address: 4294969000 },
      { name: "__bss", segment: "__DATA", kind: "UninitializedData", size: 4096, address: 4294980000 },
    ],
    symbol_bytes: 900000,
    symbol_count: 5,
    symbols_returned: 5,
    symbols_truncated: false,
    symbols: [
      { name: "cove_ui::Frame::paint", raw: "_ZN8cove_ui5Frame5paint17h1234567890abcdefE", kind: "Text", size: 524288, address: 4294967500 },
      { name: "core::fmt::write", raw: "_ZN4core3fmt5write17h1234567890abcdefE", kind: "Text", size: 262144, address: 4294970000 },
      { name: "cove_compose::layout::measure", raw: "_ZN12cove_compose6layout7measure17h1234567890abcdefE", kind: "Text", size: 98304, address: 4294971000 },
      { name: "main", raw: "main", kind: "Text", size: 16384, address: 4294967400 },
      { name: "memcpy", raw: "memcpy", kind: "Unknown", size: 4096, address: 4294972000 },
    ],
    group_count: 4,
    groups_returned: 4,
    groups_truncated: false,
    groups: [
      { name: "cove_ui", bytes: 600000, symbols: 2 },
      { name: "core", bytes: 200000, symbols: 1 },
      { name: "cove_compose", bytes: 150000, symbols: 1 },
      { name: "main", bytes: 16384, symbols: 1 },
    ],
    target: "runtime-1",
  };
}

function truncatedPayload() {
  const symbols = [];
  for (let index = 0; index < 205; index += 1) {
    symbols.push({
      name: `cove_core::generated::symbol_${index}`,
      raw: `_ZN4cove4core9generated8symbol_${index}E`,
      kind: "Text",
      size: 100000 - index * 100,
      address: 4295000000 + index * 16,
    });
  }
  return {
    available: true,
    measurement: "native_executable",
    label: "Native executable of the selected runtime",
    attribution: "This measurement covers the selected runtime's own executable file. Flutter App Size's Dart AOT, package and asset attribution does not apply to Cove.",
    pid: 4242,
    executable: executablePath,
    format: "MachO",
    kind: "Executable",
    architecture: "Aarch64",
    file_bytes: 4194304,
    section_bytes: 3000000,
    section_count: 50,
    sections_returned: 5,
    sections_truncated: true,
    sections: successPayload().sections,
    symbol_bytes: 4000000,
    symbol_count: 250,
    symbols_returned: 205,
    symbols_truncated: true,
    symbols,
    group_count: 120,
    groups_returned: 2,
    groups_truncated: true,
    groups: [
      { name: "cove_core", bytes: 2500000, symbols: 200 },
      { name: "cove_ui", bytes: 500000, symbols: 5 },
    ],
    target: "runtime-1",
  };
}

function diffPayload() {
  const payload = successPayload();
  payload.file_bytes = 2130000;
  payload.sections = [
    { name: "__text", segment: "__TEXT", kind: "Text", size: 1100000, address: 4294967296 },
    { name: "__data", segment: "__DATA", kind: "Data", size: 240000, address: 4294971392 },
    { name: "__cstring", segment: "__TEXT", kind: "ReadOnlyString", size: 65536, address: 4294968000 },
    { name: "__unwind_info", segment: "__TEXT", kind: "ReadOnlyData", size: 32768, address: 4294969000 },
    { name: "__const", segment: "__TEXT", kind: "ReadOnlyData", size: 20000, address: 4294981000 },
  ];
  payload.section_count = 5;
  payload.sections_returned = 5;
  payload.symbols = [
    { name: "cove_ui::Frame::paint", raw: "_ZN8cove_ui5Frame5paint17h1234567890abcdefE", kind: "Text", size: 600000, address: 4294967500 },
    { name: "core::fmt::write", raw: "_ZN4core3fmt5write17h1234567890abcdefE", kind: "Text", size: 262144, address: 4294970000 },
    { name: "cove_compose::layout::measure", raw: "_ZN12cove_compose6layout7measure17h1234567890abcdefE", kind: "Text", size: 98304, address: 4294971000 },
    { name: "cove_ui::Window::draw", raw: "_ZN8cove_ui6Window4draw17h1234567890abcdefE", kind: "Text", size: 50000, address: 4294973000 },
    { name: "memcpy", raw: "memcpy", kind: "Unknown", size: 4096, address: 4294972000 },
  ];
  payload.symbol_count = 5;
  payload.symbols_returned = 5;
  payload.groups = [
    { name: "cove_ui", bytes: 700000, symbols: 2 },
    { name: "core", bytes: 150000, symbols: 1 },
    { name: "cove_compose", bytes: 150000, symbols: 1 },
    { name: "cc", bytes: 50000, symbols: 1 },
  ];
  payload.group_count = 4;
  payload.groups_returned = 4;
  return payload;
}

function unmeasuredPayload() {
  const payload = successPayload();
  payload.group_count = 0;
  payload.groups_returned = 0;
  payload.groups_truncated = false;
  payload.groups = [];
  payload.symbol_count = 0;
  payload.symbols_returned = 0;
  payload.symbols_truncated = false;
  payload.symbols = [];
  return payload;
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
  let appSizeCount = 0;

  function session() {
    const remote = mode === "remote";
    return {
      platform: "macos",
      permissions: { screen_capture: false, accessibility: false },
      targets: [
        {
          id: "runtime-1",
          title: "App Size Target",
          platform: remote ? "android" : "macos",
          pid: 4242,
          kind: "runtime",
          capabilities: ["tree", "diagnostics"],
        },
      ],
    };
  }

  function appSizeResponse() {
    if (mode === "unavailable") {
      throw rpcError("unavailable", "The selected runtime's executable is not a readable object file: invalid magic.");
    }
    if (mode === "error") {
      throw rpcError("internal", "The object file table is corrupt.");
    }
    if (mode === "empty") return null;
    if (mode === "unmeasured") return unmeasuredPayload();
    if (mode === "malformed") return { available: true, executable: null };
    if (mode === "truncated") return truncatedPayload();
    if (mode === "diff") {
      appSizeCount += 1;
      return appSizeCount === 1 ? successPayload() : diffPayload();
    }
    if (mode === "diff-truncated") {
      appSizeCount += 1;
      return appSizeCount === 1 ? successPayload() : truncatedPayload();
    }
    return successPayload();
  }

  function handle(method, params) {
    calls.push({ method, params });
    if (method === "get_ui_session") return session();
    if (method === "get_ui_app_size") return appSizeResponse();
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
  await new Promise((resolvePromise, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolvePromise);
  });
  const { port } = server.address();
  return {
    fake,
    url: `http://127.0.0.1:${port}/#test-token`,
    close: () => new Promise(resolvePromise => {
      server.closeAllConnections?.();
      server.close(resolvePromise);
    }),
  };
}

async function startChrome(profile, url) {
  const probe = createServer();
  await new Promise((resolvePromise, reject) => {
    probe.once("error", reject);
    probe.listen(0, "127.0.0.1", resolvePromise);
  });
  const port = probe.address().port;
  await new Promise(resolvePromise => probe.close(resolvePromise));
  const child = spawn(chromePath, [
    "--headless=new",
    `--remote-debugging-port=${port}`,
    `--user-data-dir=${profile}`,
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-gpu",
    "--hide-scrollbars",
    `--window-size=${process.env.APP_SIZE_WINDOW || "1440,900"}`,
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
    await new Promise(resolvePromise => setTimeout(resolvePromise, 100));
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
      await new Promise(resolvePromise => {
        const timer = setTimeout(() => { child.kill("SIGKILL"); resolvePromise(); }, 3000);
        child.once("exit", () => { clearTimeout(timer); resolvePromise(); });
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
    return new Promise((resolvePromise, reject) => this.pending.set(id, { resolve: resolvePromise, reject }));
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
  await new Promise((resolvePromise, reject) => {
    socket.addEventListener("open", resolvePromise, { once: true });
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
    await new Promise(resolvePromise => setTimeout(resolvePromise, 50));
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

async function openAppSize(cdp, title) {
  await waitFor(cdp, "controller connection", `document.getElementById("connection-text")?.textContent === "Connected"`);
  await click(cdp, "#tab-app-size");
  await waitFor(cdp, "app size view", `document.getElementById("view-app-size")?.hidden === false`);
  await waitFor(cdp, title, `document.querySelector("#app-size-content .app-size-scope, #app-size-content .state strong") !== null`);
}

async function screenshot(cdp, path) {
  if (!path) return;
  const shot = await cdp.send("Page.captureScreenshot", { format: "png" });
  await writeFile(path, Buffer.from(shot.data, "base64"));
}

async function screenshotAfterScroll(cdp, selector, path) {
  if (!path) return;
  await cdp.evaluate(`document.querySelector(${JSON.stringify(selector)})?.scrollIntoView({ block: "start" })`);
  await new Promise(resolvePromise => setTimeout(resolvePromise, 100));
  await screenshot(cdp, path);
}

async function page(cdp) {
  return cdp.evaluate(`(() => {
    const host = document.getElementById("app-size-content");
    const q = selector => host?.querySelector(selector);
    const qa = selector => [...(host?.querySelectorAll(selector) || [])];
    const stat = label => {
      const node = qa(".stat").find(row => row.querySelector(".stat-label")?.textContent === label);
      return {
        value: node?.querySelector(".stat-value")?.childNodes[0]?.textContent.trim() ?? "",
        detail: node?.querySelector(".stat-value small")?.textContent.trim() ?? "",
      };
    };
    const block = title => qa(".panel-block").find(item => item.querySelector("h2")?.textContent === title);
    const rows = table => [...(table?.querySelectorAll("tbody tr") || [])]
      .map(row => [...row.children].map(cell => cell.textContent.trim()));
    const table = title => {
      const panel = block(title);
      if (!panel) return null;
      return {
        meta: panel.querySelector(".pane-meta")?.textContent.trim() ?? "",
        headers: [...panel.querySelectorAll("thead th")].map(cell => cell.textContent.trim()),
        rows: rows(panel.querySelector("table")),
        selected: [...panel.querySelectorAll("tbody tr.is-selected")].map(row => row.dataset.appSizeRow || ""),
        notices: [...panel.querySelectorAll(".app-size-note")].map(cell => cell.textContent.trim()),
      };
    };
    const treemapPanel = block("Size treemap");
    const treemap = treemapPanel ? {
      meta: treemapPanel.querySelector(".pane-meta")?.textContent.trim() ?? "",
      mode: [...treemapPanel.querySelectorAll("[data-app-size-treemap-mode]")].find(node => node.getAttribute("aria-pressed") === "true")?.dataset.appSizeTreemapMode ?? "",
      cells: [...treemapPanel.querySelectorAll(".app-size-treemap-cell")].map(cell => ({
        name: cell.dataset.appSizeTreemapName || "",
        title: cell.getAttribute("title") || "",
        ariaLabel: cell.getAttribute("aria-label") || "",
        focused: cell.classList.contains("is-focused"),
        pressed: cell.getAttribute("aria-pressed") || "",
        label: cell.querySelector(".app-size-treemap-label")?.textContent ?? "",
        size: cell.querySelector(".app-size-treemap-size")?.textContent ?? "",
        left: parseFloat(cell.style.left || "0"),
        top: parseFloat(cell.style.top || "0"),
        width: parseFloat(cell.style.width || "0"),
        height: parseFloat(cell.style.height || "0"),
      })),
      legend: [...treemapPanel.querySelectorAll(".app-size-treemap-legend-item")].map(item => ({
        name: item.querySelector("span:last-child")?.textContent ?? "",
        pressed: item.getAttribute("aria-pressed") || "",
        focused: item.classList.contains("is-focused"),
      })),
      notes: [...treemapPanel.querySelectorAll(".app-size-note")].map(cell => cell.textContent.trim()),
    } : null;
    const diffPanel = block("Size diff");
    const diff = diffPanel ? {
      meta: diffPanel.querySelector(".pane-meta")?.textContent.trim() ?? "",
      empty: diffPanel.querySelector(".app-size-diff-empty")?.textContent.trim() ?? "",
      error: diffPanel.querySelector(".error-inline")?.textContent.trim() ?? "",
      baseline: diffPanel.querySelector(".app-size-diff-baseline")?.textContent.trim() ?? "",
      current: diffPanel.querySelector(".app-size-diff-current")?.textContent.trim() ?? "",
      summary: diffPanel.querySelector(".app-size-diff-summary")?.textContent.trim() ?? "",
      total: diffPanel.querySelector(".app-size-diff-total")?.textContent.trim() ?? "",
      notes: [...diffPanel.querySelectorAll(".app-size-note")].map(cell => cell.textContent.trim()),
      rows: [...diffPanel.querySelectorAll(".app-size-diff-row")].map(row => ({
        status: row.dataset.appSizeDiffStatus || "",
        cells: [...row.children].map(cell => cell.textContent.trim()),
      })),
      table: Boolean(diffPanel.querySelector(".data-table")),
      buttons: {
        snapshotDisabled: document.getElementById("app-size-diff-snapshot")?.disabled ?? null,
        exportDisabled: document.getElementById("app-size-diff-export")?.disabled ?? null,
        clearDisabled: document.getElementById("app-size-diff-clear")?.disabled ?? null,
      },
    } : null;
    return {
      meta: document.getElementById("app-size-meta")?.textContent.trim() ?? "",
      scopeLabel: q(".app-size-scope strong")?.textContent.trim() ?? "",
      scopeText: q(".app-size-scope span")?.textContent.trim() ?? "",
      stateTitle: q(".state strong")?.textContent.trim() ?? "",
      stateMessage: q(".state p")?.textContent.trim() ?? "",
      stateError: Boolean(q(".state.is-error")),
      stats: {
        total: stat("Total executable"),
        sections: stat("Sections"),
        symbols: stat("Symbols"),
        groups: stat("Crate groups"),
      },
      sourcePath: q(".app-size-source-path")?.textContent.trim() ?? "",
      sourceMeta: q(".app-size-source-meta")?.textContent.trim() ?? "",
      sections: table("Sections"),
      symbols: table("Top symbols"),
      groups: table("Crate groups"),
      treemap,
      diff,
      symbolTitles: [...(block("Top symbols")?.querySelectorAll("tbody tr td:first-child") || [])].map(cell => cell.getAttribute("title") || ""),
      notes: [...host.querySelectorAll(":scope > .app-size-note")].map(cell => cell.textContent.trim()),
      inlineError: q(".error-inline")?.textContent.trim() ?? "",
      refreshDisabled: document.getElementById("app-size-refresh")?.disabled ?? null,
    };
  })()`);
}

async function auditAppSizeDom(cdp) {
  const report = await cdp.evaluate(`(() => {
    const failures = [];
    const ids = [...document.querySelectorAll("[id]")].map(node => node.id);
    const duplicates = ids.filter((id, index) => ids.indexOf(id) !== index);
    if (duplicates.length) failures.push("duplicate ids: " + [...new Set(duplicates)].join(", "));
    const brokenRefs = [];
    for (const attribute of ["aria-controls", "aria-labelledby", "aria-describedby"]) {
      for (const node of document.querySelectorAll("#view-app-size [" + attribute + "]")) {
        for (const id of (node.getAttribute(attribute) || "").split(/\\s+/).filter(Boolean)) {
          if (!document.getElementById(id)) brokenRefs.push(attribute + "=" + id);
        }
      }
    }
    if (brokenRefs.length) failures.push("broken references: " + [...new Set(brokenRefs)].join(", "));
    const symbols = new Set([...document.querySelectorAll("symbol[id]")].map(node => node.id));
    const brokenIcons = [...document.querySelectorAll("#view-app-size use[href^='#']")]
      .map(node => node.getAttribute("href").slice(1))
      .filter(id => !symbols.has(id));
    if (brokenIcons.length) failures.push("broken icon references: " + [...new Set(brokenIcons)].join(", "));
    const tabs = [...document.querySelectorAll(".rail-tab")];
    const panels = [...document.querySelectorAll(".view[role='tabpanel']")];
    if (panels.length !== tabs.length) failures.push("rail tabs (" + tabs.length + ") do not match views (" + panels.length + ")");
    for (const panel of panels) {
      if (!tabs.some(tab => tab.getAttribute("aria-controls") === panel.id)) failures.push(panel.id + " has no rail tab");
    }
    const selected = document.getElementById("tab-app-size");
    if (!selected) failures.push("app size rail tab is missing");
    else {
      if (selected.getAttribute("role") !== "tab") failures.push("app size tab is not role=tab");
      if (selected.getAttribute("aria-controls") !== "view-app-size") failures.push("app size tab controls the wrong view");
      if (selected.getAttribute("aria-selected") !== "true") failures.push("app size tab is not selected");
    }
    const unlabeled = [];
    for (const node of document.querySelectorAll("#view-app-size button, #view-app-size input, #view-app-size select")) {
      const name = (node.getAttribute("aria-label") || node.getAttribute("title") || "").trim()
        || (node.id && document.querySelector('label[for="' + node.id + '"]')?.textContent.trim())
        || (node.closest("label")?.textContent.trim())
        || (node.tagName === "BUTTON" ? node.textContent.trim() : "");
      if (!name) unlabeled.push(node.id || node.className || node.tagName);
    }
    if (unlabeled.length) failures.push("unlabeled controls: " + unlabeled.join(", "));
    const untyped = [...document.querySelectorAll("#view-app-size button")].filter(button => !button.hasAttribute("type")).map(button => button.id || button.className || button.textContent.trim());
    if (untyped.length) failures.push("buttons without type: " + untyped.join(", "));
    const tables = [...document.querySelectorAll("#view-app-size .data-table")];
    for (const table of tables) {
      if (!table.querySelector("thead th")) failures.push("table without a header row");
      if (!table.querySelector("tbody tr")) failures.push("table without body rows");
    }
    if (!document.querySelector("#view-app-size .app-size-scope")) failures.push("native-executable scope label is missing");
    const panel = title => [...document.querySelectorAll("#view-app-size .panel-block")].find(block => block.querySelector("h2")?.textContent === title);
    const treemapPanel = panel("Size treemap");
    if (!treemapPanel) {
      failures.push("size treemap panel is missing");
    } else {
      const cells = [...treemapPanel.querySelectorAll(".app-size-treemap-cell")];
      if (!cells.length) failures.push("treemap has no cells");
      if (!treemapPanel.querySelector(".app-size-treemap")?.getAttribute("aria-label")) failures.push("treemap canvas has no aria-label");
      for (const cell of cells) {
        if (cell.tagName !== "BUTTON") failures.push("treemap cell is not a button");
        if (!cell.getAttribute("title")) failures.push("treemap cell has no tooltip");
        if (!cell.getAttribute("aria-label")) failures.push("treemap cell has no aria-label");
        if (cell.getAttribute("type") !== "button") failures.push("treemap cell has no type=button");
        const left = parseFloat(cell.style.left);
        const top = parseFloat(cell.style.top);
        const width = parseFloat(cell.style.width);
        const height = parseFloat(cell.style.height);
        if (![left, top, width, height].every(Number.isFinite)) failures.push("treemap cell has no percentage geometry");
        else if (left < 0 || top < 0 || left + width > 100.05 || top + height > 100.05) failures.push("treemap cell leaves the canvas");
      }
      const legend = [...treemapPanel.querySelectorAll(".app-size-treemap-legend-item")];
      if (legend.length !== cells.length) failures.push("treemap legend entries do not match the drawn cells");
      for (const item of legend) if (!item.textContent.trim()) failures.push("treemap legend entry has no name");
    }
    const diffPanel = panel("Size diff");
    if (!diffPanel) failures.push("size diff panel is missing");
    else {
      if (!diffPanel.querySelector(".app-size-diff-empty") && !diffPanel.querySelector(".app-size-diff-baseline")) failures.push("size diff has neither a baseline nor an explicit no-baseline state");
      if (!diffPanel.querySelector("#app-size-diff-import")) failures.push("snapshot import input is missing");
      if (!diffPanel.querySelector("#app-size-diff-snapshot")) failures.push("snapshot capture button is missing");
    }
    return failures;
  })()`);
  assert.deepEqual(report, [], `App Size DOM audit failed:\n${report.join("\n")}`);
  console.log("PASS  DOM audit: unique ids, live references, icon symbols, rail tabs matching tabpanels, named controls, table headers, scope label");
}

let audited = false;

async function runScenario(mode, title, body) {
  const server = await startServer(mode);
  const profile = await mkdtemp(join(tmpdir(), "cove-app-size-harness-"));
  let chrome = null;
  let cdp = null;
  try {
    chrome = await startChrome(profile, server.url);
    cdp = await connect(chrome.wsUrl);
    await openAppSize(cdp, title);
    await body({ cdp, fake: server.fake, mode });
    assert.deepEqual(cdp.exceptions, [], `page exceptions: ${cdp.exceptions.join("\n")}`);
  } finally {
    cdp?.close();
    await chrome?.close();
    await server.close();
    await rm(profile, { recursive: true, force: true });
  }
}

function appSizeCalls(fake) {
  return fake.calls.filter(call => call.method === "get_ui_app_size");
}

function diffRow(view, status, name) {
  return view.diff.rows.find(row => row.status === status && row.cells[2] === name);
}

async function dispatchSnapshotFile(cdp, text, name) {
  await cdp.evaluate(`(() => {
    const input = document.getElementById("app-size-diff-import");
    const transfer = new DataTransfer();
    transfer.items.add(new File([${JSON.stringify(text)}], ${JSON.stringify(name)}, { type: "application/json" }));
    input.files = transfer.files;
    input.dispatchEvent(new Event("change", { bubbles: true }));
  })()`);
}

async function scenarioUnmeasured() {
  await runScenario("unmeasured", `document.querySelector("#app-size-content .app-size-scope") !== null`, async ({ cdp }) => {
    await waitFor(cdp, "empty treemap note", `[...document.querySelectorAll("#app-size-content .app-size-note")].some(note => note.textContent.includes("no crate groups with bytes"))`);
    const view = await page(cdp);
    assert.equal(view.treemap.meta, "0 B · 0 crate groups");
    assert.equal(view.treemap.cells.length, 0);
    assert.equal(view.treemap.legend.length, 0);
    assert.match(view.treemap.notes[0], /The analyzer reported no crate groups with bytes\. No treemap is substituted\./);
    assert.equal(view.groups.rows.length, 0);
    assert.match(view.groups.notices[0], /reported no crate groups/);
    assert.equal(view.diff.meta, "No baseline");
    assert.match(view.diff.empty, /No baseline snapshot/);
  });
}

async function scenarioSuccess() {
  await runScenario("success", `document.querySelector("#app-size-content .app-size-scope") !== null`, async ({ cdp, fake }) => {
    await waitFor(cdp, "section table", `document.querySelectorAll("#app-size-content .panel-block").length >= 4`);
    const calls = appSizeCalls(fake);
    assert.equal(calls.length, 1);
    assert.deepEqual(calls[0].params, { target: "runtime-1" });
    const view = await page(cdp);
    assert.equal(view.scopeLabel, "Native executable of the selected runtime");
    assert.match(view.scopeText, /object file of pid 4242/);
    assert.match(view.scopeText, /\/tmp\/cove-app-size-harness\/gallery/);
    assert.match(view.scopeText, /Flutter App Size's Dart AOT, package and asset attribution does not apply to Cove/);
    assert.match(view.meta, /2\.0 MB native executable/);
    assert.match(view.meta, /5 sections/);
    assert.match(view.meta, /5 symbols/);
    assert.match(view.meta, /4 crate groups/);
    assert.match(view.meta, /updated \d{2}:\d{2}:\d{2}/);
    assert.equal(view.stats.total.value, "2.0 MB");
    assert.match(view.stats.total.detail, /^2[,.]?097[,.]?152 bytes$/);
    assert.equal(view.stats.sections.value, "5");
    assert.equal(view.stats.sections.detail, "");
    assert.equal(view.stats.symbols.value, "5");
    assert.equal(view.stats.groups.value, "4");
    assert.equal(view.sourcePath, executablePath);
    assert.equal(view.sourceMeta, "PID 4242 · Executable · MachO · Aarch64 · macos");
    assert.equal(view.sections.meta, "5 reported");
    assert.deepEqual(view.sections.headers, ["Section", "Kind", "Bytes", "Address"]);
    assert.deepEqual(view.sections.rows[0], ["__TEXT,__text", "Text", "1.0 MB", "0x100000000"]);
    assert.deepEqual(view.sections.rows[1], ["__DATA,__data", "Data", "256.0 KB", "0x100001000"]);
    assert.equal(view.sections.rows.length, 5);
    assert.deepEqual(view.sections.notices, []);
    assert.equal(view.symbols.meta, "5 reported");
    assert.deepEqual(view.symbols.headers, ["Symbol", "Bytes"]);
    assert.deepEqual(view.symbols.rows[0], ["cove_ui::Frame::paint", "512.0 KB"]);
    assert.deepEqual(view.symbols.rows[3], ["main", "16.0 KB"]);
    assert.equal(view.symbols.rows.length, 5);
    assert.equal(view.symbolTitles[0], "Mangled: _ZN8cove_ui5Frame5paint17h1234567890abcdefE");
    assert.equal(view.symbolTitles[3], "");
    assert.equal(view.groups.meta, "4 reported");
    assert.deepEqual(view.groups.headers, ["Group", "Symbols", "Bytes"]);
    assert.deepEqual(view.groups.rows[0], ["cove_ui", "2", "585.9 KB"]);
    assert.deepEqual(view.groups.rows[1], ["core", "1", "195.3 KB"]);
    assert.equal(view.groups.rows.length, 4);
    assert.equal(view.treemap.meta, "943.7 KB · 4 crate groups");
    assert.equal(view.treemap.mode, "groups");
    assert.deepEqual(view.treemap.legend.map(item => item.name), ["cove_ui", "core", "cove_compose", "main"]);
    assert.equal(view.treemap.cells.length, 4);
    const biggest = view.treemap.cells.find(cell => cell.name === "cove_ui");
    const second = view.treemap.cells.find(cell => cell.name === "core");
    assert.ok(biggest.width > second.width);
    assert.ok(biggest.width > 40);
    assert.match(biggest.title, /^cove_ui · 585\.9 KB · 62\.09% of shown crate groups bytes · 2 symbols$/);
    assert.match(biggest.ariaLabel, /Selects the matching group row\.$/);
    assert.equal(biggest.label, "cove_ui");
    assert.match(biggest.size, /^585\.9 KB · 62\.1%$/);
    const smallest = view.treemap.cells.find(cell => cell.name === "main");
    assert.equal(smallest.label, "main");
    assert.equal(smallest.size, "");
    assert.match(smallest.title, /^main · 16\.0 KB · 1\.70% of shown crate groups bytes · 1 symbol$/);
    assert.ok(view.treemap.cells.every(cell => cell.left >= 0 && cell.top >= 0 && cell.left + cell.width <= 100.05 && cell.top + cell.height <= 100.05));
    assert.equal(view.diff.meta, "No baseline");
    assert.match(view.diff.empty, /No baseline snapshot\. Snapshot the current report or import a snapshot JSON/);
    assert.equal(view.diff.table, false);
    assert.deepEqual(view.diff.rows, []);
    assert.equal(view.diff.buttons.snapshotDisabled, false);
    assert.equal(view.diff.buttons.exportDisabled, true);
    assert.equal(view.diff.buttons.clearDisabled, true);
    assert.equal(view.notes.length, 1);
    assert.equal(view.notes[0], "Sections are object-file sections, not loaded memory mappings. Symbol sizes and crate groups are reported by the native analyzer; bytes outside those groups stay unassigned.");
    assert.equal(view.inlineError, "");
    assert.equal(view.refreshDisabled, false);

    await click(cdp, '[data-app-size-treemap-name="cove_ui"]');
    let focused = await page(cdp);
    assert.equal(focused.treemap.cells.find(cell => cell.name === "cove_ui").focused, true);
    assert.equal(focused.treemap.cells.find(cell => cell.name === "cove_ui").pressed, "true");
    assert.deepEqual(focused.groups.selected, ["cove_ui"]);
    assert.equal(focused.treemap.legend.find(item => item.name === "cove_ui").focused, true);
    await click(cdp, '[data-app-size-treemap-name="cove_ui"]');
    focused = await page(cdp);
    assert.equal(focused.treemap.cells.find(cell => cell.name === "cove_ui").focused, false);
    assert.deepEqual(focused.groups.selected, []);

    await click(cdp, '[data-app-size-treemap-mode="sections"]');
    await waitFor(cdp, "section treemap", `document.querySelector('[data-app-size-treemap-mode="sections"]')?.getAttribute("aria-pressed") === "true"`);
    const sectionsView = await page(cdp);
    assert.equal(sectionsView.treemap.mode, "sections");
    assert.equal(sectionsView.treemap.cells.length, 5);
    assert.deepEqual(sectionsView.treemap.legend.map(item => item.name), ["__TEXT,__text", "__DATA,__data", "__TEXT,__cstring", "__TEXT,__unwind_info", "__DATA,__bss"]);
    assert.match(sectionsView.treemap.meta, /1\.3 MB · 5 sections/);
    await click(cdp, '[data-app-size-treemap-name="__TEXT,__text"]');
    const sectionFocus = await page(cdp);
    assert.deepEqual(sectionFocus.sections.selected, ["__TEXT,__text"]);
    assert.deepEqual(sectionFocus.groups.selected, []);
    assert.equal(sectionFocus.treemap.cells.find(cell => cell.name === "__TEXT,__text").focused, true);
    await click(cdp, '[data-app-size-treemap-mode="groups"]');
    await waitFor(cdp, "group treemap", `document.querySelector('[data-app-size-treemap-mode="groups"]')?.getAttribute("aria-pressed") === "true"`);
    if (!audited) {
      await auditAppSizeDom(cdp);
      audited = true;
    }
    await screenshotAfterScroll(cdp, "#app-size-treemap-panel", process.env.APP_SIZE_TREEMAP_SHOT);

    await click(cdp, "#app-size-refresh");
    const deadline = Date.now() + 5000;
    while (Date.now() < deadline && appSizeCalls(fake).length < 2) {
      await new Promise(resolvePromise => setTimeout(resolvePromise, 50));
    }
    assert.equal(appSizeCalls(fake).length, 2);
    assert.deepEqual(appSizeCalls(fake)[1].params, { target: "runtime-1" });
    await waitFor(cdp, "refreshed view", `document.querySelector("#app-size-content .app-size-scope") !== null && !document.querySelector("#app-size-content .inline-spinner")`);
    await screenshot(cdp, process.env.APP_SIZE_SHOT);
  });
}

async function scenarioUnavailable() {
  await runScenario("unavailable", `document.querySelector("#app-size-content .state strong")?.textContent === "App Size unavailable"`, async ({ cdp, fake }) => {
    const view = await page(cdp);
    assert.equal(view.stateTitle, "App Size unavailable");
    assert.match(view.stateMessage, /not a readable object file/);
    assert.match(view.stateMessage, /unavailable/);
    assert.match(view.meta, /App Size unavailable/);
    assert.equal(view.sections, null);
    assert.equal(view.symbols, null);
    assert.equal(view.groups, null);
    assert.equal(view.refreshDisabled, false);
    assert.equal(appSizeCalls(fake).length, 1);
    await click(cdp, "#app-size-content .state .tool-button");
    const deadline = Date.now() + 5000;
    while (Date.now() < deadline && appSizeCalls(fake).length < 2) {
      await new Promise(resolvePromise => setTimeout(resolvePromise, 50));
    }
    assert.equal(appSizeCalls(fake).length, 2);
  });
}

async function scenarioError() {
  await runScenario("error", `document.querySelector("#app-size-content .state strong")?.textContent === "Executable read failed"`, async ({ cdp, fake }) => {
    const view = await page(cdp);
    assert.equal(view.stateTitle, "Executable read failed");
    assert.equal(view.stateError, true);
    assert.match(view.stateMessage, /object file table is corrupt/);
    assert.match(view.stateMessage, /internal/);
    assert.match(view.meta, /Executable read failed/);
    assert.equal(view.refreshDisabled, false);
    assert.equal(appSizeCalls(fake).length, 1);
  });
}

async function scenarioTruncated() {
  await runScenario("truncated", `document.querySelector("#app-size-content .app-size-scope") !== null`, async ({ cdp }) => {
    await waitFor(cdp, "truncated symbol rows", `[...document.querySelectorAll("#app-size-content .panel-block")].some(panel => panel.querySelector("h2")?.textContent === "Top symbols" && panel.querySelectorAll("tbody tr").length === 200)`);
    const view = await page(cdp);
    assert.equal(view.stats.sections.value, "5");
    assert.equal(view.stats.sections.detail, "of 50");
    assert.equal(view.stats.symbols.value, "205");
    assert.equal(view.stats.symbols.detail, "of 250");
    assert.equal(view.stats.groups.value, "2");
    assert.equal(view.stats.groups.detail, "of 120");
    assert.equal(view.sections.rows.length, 5);
    assert.deepEqual(view.sections.notices, ["Showing 5 of 50 sections."]);
    assert.equal(view.symbols.rows.length, 200);
    assert.deepEqual(view.symbols.notices, ["Showing 200 of 250 symbols."]);
    assert.equal(view.groups.rows.length, 2);
    assert.deepEqual(view.groups.notices, ["Showing 2 of 120 groups."]);
    assert.equal(view.treemap.meta, "2.9 MB · 2 crate groups");
    assert.deepEqual(view.treemap.notes, ["Showing 2 of 120 crate groups. Entries outside that list are not drawn."]);
    assert.match(view.meta, /4\.0 MB native executable/);
    assert.match(view.meta, /205 symbols/);
    assert.match(view.meta, /2 crate groups/);
  });
}

async function scenarioMalformed() {
  await runScenario("malformed", `document.querySelector("#app-size-content .state strong")?.textContent === "Unrecognized app size response"`, async ({ cdp }) => {
    const view = await page(cdp);
    assert.equal(view.stateTitle, "Unrecognized app size response");
    assert.equal(view.stateError, true);
    assert.match(view.stateMessage, /no total size, sections, symbols, groups or executable path/);
    assert.match(view.meta, /Unrecognized app size response/);
    assert.equal(view.scopeLabel, "");
    assert.equal(view.sections, null);
  });
}

async function scenarioEmpty() {
  await runScenario("empty", `document.querySelector("#app-size-content .state strong")?.textContent === "No executable analysis"`, async ({ cdp }) => {
    const view = await page(cdp);
    assert.equal(view.stateTitle, "No executable analysis");
    assert.match(view.stateMessage, /Nothing is substituted/);
    assert.match(view.meta, /No executable analysis returned/);
    assert.equal(view.scopeLabel, "");
  });
}

async function scenarioRemote() {
  await runScenario("remote", `document.querySelector("#app-size-content .state strong")?.textContent === "App Size unavailable"`, async ({ cdp, fake }) => {
    const view = await page(cdp);
    assert.equal(view.stateTitle, "App Size unavailable");
    assert.match(view.stateMessage, /Android targets run remotely or on a mobile device/);
    assert.match(view.meta, /App Size is unavailable for this target/);
    assert.equal(view.refreshDisabled, true);
    assert.equal(appSizeCalls(fake).length, 0);
    assert.equal(fake.calls.some(call => call.method === "get_ui_app_size"), false);
  });
}

async function scenarioDiff() {
  await runScenario("diff", `document.querySelector("#app-size-content .app-size-scope") !== null`, async ({ cdp, fake }) => {
    await waitFor(cdp, "diff panel", `document.querySelectorAll("#app-size-content .panel-block").length >= 6`);
    let view = await page(cdp);
    assert.equal(view.diff.meta, "No baseline");
    assert.match(view.diff.empty, /No baseline snapshot/);
    assert.equal(view.diff.rows.length, 0);

    await click(cdp, "#app-size-diff-snapshot");
    await waitFor(cdp, "captured baseline", `document.getElementById("app-size-diff-export")?.disabled === false`);
    view = await page(cdp);
    assert.match(view.diff.meta, /^Baseline \d{2}:\d{2}:\d{2}$/);
    assert.match(view.diff.baseline, /^session snapshot · \d{2}:\d{2}:\d{2} · 2\.0 MB · \/tmp\/cove-app-size-harness\/gallery$/);
    assert.match(view.diff.current, /^Current report · \d{2}:\d{2}:\d{2} · 2\.0 MB · \/tmp\/cove-app-size-harness\/gallery$/);
    assert.equal(view.diff.summary, "0 added · 0 removed · 0 changed · 14 unchanged");
    assert.match(view.diff.notes.join(" "), /No added, removed or changed entries between the baseline and the current report\./);
    assert.equal(view.diff.table, false);
    assert.equal(view.diff.buttons.snapshotDisabled, false);
    assert.equal(view.diff.buttons.exportDisabled, false);
    assert.equal(view.diff.buttons.clearDisabled, false);

    await cdp.evaluate(`(() => {
      window.__coveExport = null;
      window.__coveExportName = "";
      URL.createObjectURL = blob => {
        blob.text().then(text => { window.__coveExport = text; });
        return "blob:cove-app-size-harness";
      };
      HTMLAnchorElement.prototype.click = function () { window.__coveExportName = this.download; };
    })()`);
    await click(cdp, "#app-size-diff-export");
    await waitFor(cdp, "exported snapshot", `typeof window.__coveExport === "string" && window.__coveExportName !== ""`);
    const exported = await cdp.evaluate("JSON.parse(window.__coveExport)");
    const exportName = await cdp.evaluate("window.__coveExportName");
    assert.match(exportName, /^cove-app-size-\d{8}-\d{6}\.json$/);
    assert.equal(exported.schema, "cove.devtools.app-size");
    assert.equal(exported.version, 1);
    assert.equal(typeof exported.savedAt, "number");
    assert.equal(exported.path, executablePath);
    assert.deepEqual(exported.groups.map(group => [group.name, group.sizeBytes]), [
      ["cove_ui", 600000],
      ["core", 200000],
      ["cove_compose", 150000],
      ["main", 16384],
    ]);
    assert.equal(exported.sections.length, 5);
    assert.equal(exported.symbols.length, 5);
    assert.equal(exported.totalBytes, 2097152);

    await click(cdp, "#app-size-refresh");
    await waitFor(cdp, "diff rows after refresh", `document.querySelectorAll("#app-size-content .app-size-diff-row").length === 11`);
    view = await page(cdp);
    assert.equal(view.diff.summary, "3 added · 3 removed · 5 changed · 6 unchanged");
    assert.match(view.diff.total, /^Executable total 2\.0 MB → 2\.0 MB · \+32\.1 KB · \+1\.6%$/);
    assert.deepEqual(diffRow(view, "changed", "cove_ui").cells, ["Changed", "Group", "cove_ui", "585.9 KB", "683.6 KB", "+97.7 KB", "+16.7%"]);
    assert.deepEqual(diffRow(view, "changed", "core").cells, ["Changed", "Group", "core", "195.3 KB", "146.5 KB", "−48.8 KB", "−25.0%"]);
    assert.deepEqual(diffRow(view, "removed", "main").cells, ["Removed", "Group", "main", "16.0 KB", "—", "−16.0 KB", "−100.0%"]);
    assert.deepEqual(diffRow(view, "added", "cc").cells, ["Added", "Group", "cc", "—", "48.8 KB", "+48.8 KB", "—"]);
    assert.deepEqual(diffRow(view, "changed", "__TEXT,__text").cells, ["Changed", "Section", "__TEXT,__text", "1.0 MB", "1.0 MB", "+50.2 KB", "+4.9%"]);
    assert.deepEqual(diffRow(view, "changed", "cove_ui::Frame::paint").cells, ["Changed", "Symbol", "cove_ui::Frame::paint", "512.0 KB", "585.9 KB", "+73.9 KB", "+14.4%"]);
    assert.equal(view.diff.notes.some(note => note.startsWith("Truncated lists:")), false);
    assert.match(view.diff.notes.join(" "), /A dash means the entry was absent from that report, not zero bytes\./);
    assert.equal(view.diff.table, true);
    assert.equal(appSizeCalls(fake).length, 2);
    await screenshotAfterScroll(cdp, "#app-size-diff-panel", process.env.APP_SIZE_DIFF_SHOT);

    await dispatchSnapshotFile(cdp, "not json", "malformed.json");
    await waitFor(cdp, "rejected snapshot", `document.querySelector("#app-size-content .app-size-diff-toolbar") && document.querySelector("#app-size-content .error-inline")?.textContent.includes("Snapshot rejected")`);
    view = await page(cdp);
    assert.match(view.diff.error, /^Snapshot rejected: the file is not valid JSON$/);
    assert.equal(view.diff.buttons.exportDisabled, false);
    assert.equal(view.diff.table, true);
    assert.equal(view.diff.rows.length, 11);

    const imported = JSON.parse(await cdp.evaluate("window.__coveExport"));
    imported.savedAt = 1700000000000;
    imported.groups = imported.groups.map(group => group.name === "cove_ui" ? { ...group, sizeBytes: 500000 } : group);
    await dispatchSnapshotFile(cdp, JSON.stringify(imported), "baseline.json");
    await waitFor(cdp, "imported baseline", `!document.querySelector("#app-size-content .error-inline") && document.querySelector("#app-size-content .app-size-diff-baseline")?.textContent.startsWith("imported snapshot")`);
    view = await page(cdp);
    assert.equal(view.diff.error, "");
    assert.match(view.diff.baseline, /^imported snapshot · \d{2}:\d{2}:\d{2} · 2\.0 MB · \/tmp\/cove-app-size-harness\/gallery$/);
    assert.deepEqual(diffRow(view, "changed", "cove_ui").cells, ["Changed", "Group", "cove_ui", "488.3 KB", "683.6 KB", "+195.3 KB", "+40.0%"]);
    assert.equal(view.diff.rows.length, 11);

    await click(cdp, "#app-size-diff-clear");
    await waitFor(cdp, "cleared baseline", `document.querySelector("#app-size-content .app-size-diff-empty") !== null`);
    view = await page(cdp);
    assert.equal(view.diff.meta, "No baseline");
    assert.equal(view.diff.table, false);
    assert.equal(view.diff.buttons.exportDisabled, true);
    assert.equal(view.diff.buttons.clearDisabled, true);

    await dispatchSnapshotFile(cdp, JSON.stringify(successPayload()), "payload.json");
    await waitFor(cdp, "imported controller payload", `document.querySelector("#app-size-content .app-size-diff-baseline")?.textContent.startsWith("imported controller payload")`);
    view = await page(cdp);
    assert.match(view.diff.baseline, /^imported controller payload · timestamp not reported · 2\.0 MB · \/tmp\/cove-app-size-harness\/gallery$/);
    assert.equal(view.diff.rows.length, 11);
  });
}

async function scenarioDiffTruncated() {
  await runScenario("diff-truncated", `document.querySelector("#app-size-content .app-size-scope") !== null`, async ({ cdp }) => {
    await click(cdp, "#app-size-diff-snapshot");
    await waitFor(cdp, "captured baseline", `document.getElementById("app-size-diff-export")?.disabled === false`);
    await click(cdp, "#app-size-refresh");
    await waitFor(cdp, "truncated diff rows", `document.querySelectorAll("#app-size-content .app-size-diff-row").length > 0`);
    const view = await page(cdp);
    assert.equal(
      view.diff.notes.find(note => note.startsWith("Truncated lists:")),
      "Truncated lists: groups (current 2 of 120); sections (current 5 of 50); symbols (current 205 of 250). The diff compares only returned entries, so unlisted entries cannot be matched.",
    );
    assert.match(view.diff.total, /^Executable total 2\.0 MB → 4\.0 MB · \+2\.0 MB · \+100\.0%$/);
    assert.ok(view.diff.rows.length > 200);
    assert.equal(view.diff.rows.some(row => row.status === "removed" && row.cells[2] === "main"), true);
    assert.equal(view.diff.rows.some(row => row.status === "added" && row.cells[2] === "cove_core::generated::symbol_0"), true);
  });
}

console.log("App Size harness: fake controller + headless Chrome");
await scenarioSuccess();
console.log("PASS  success renders the native-executable label, totals, treemap, sections, symbols and groups with row focus and refresh");
await scenarioUnavailable();
console.log("PASS  unavailable error keeps the explicit unavailable state and retries");
await scenarioError();
console.log("PASS  parse/read error keeps the red error state with a retry");
await scenarioTruncated();
console.log("PASS  reported and local caps show truncation notices without extra rows");
await scenarioMalformed();
console.log("PASS  malformed payload is an explicit parse failure, not a fabricated table");
await scenarioEmpty();
console.log("PASS  null payload is an explicit empty state");
await scenarioUnmeasured();
console.log("PASS  groups without bytes draw no treemap and keep an explicit empty message");
await scenarioRemote();
console.log("PASS  remote target is unavailable without calling get_ui_app_size");
await scenarioDiff();
console.log("PASS  diff snapshots, exports, imports, rejects malformed JSON and shows signed added/removed/changed bytes");
await scenarioDiffTruncated();
console.log("PASS  diff reports truncated baseline and current lists without inventing unlisted entries");
console.log("RESULT PASS");
