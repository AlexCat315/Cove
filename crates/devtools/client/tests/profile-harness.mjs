#!/usr/bin/env node
// Fake-controller harness for the CPU Profile views. It serves the real
// index.html/inspector.js/inspector.css, answers /rpc with the controller's
// profile wire shapes (UiProfile / UiHotspots), and drives the page through
// Chrome DevTools Protocol. No Rust process is started and no samples are
// invented outside the fake controller's declared responses.
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import assert from "node:assert/strict";
import { serveClientAsset } from "./harness-server.mjs";

const chromePath = process.env.CHROME_PATH
  || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

function frame(functionName, filePath, line, label = null) {
  return {
    label: label || (functionName
      ? `${functionName}${filePath ? ` ${filePath}${line === null ? "" : `:${line}`}` : ""}`
      : `0x${line.toString(16)}`),
    function: functionName,
    filePath,
    line,
  };
}

const unresolved = { label: "0x104a2b3c4", function: null, filePath: null, line: null };
const mainFrame = frame("main", "/src/main.rs", 5);
const runFrame = frame("run", "/src/main.rs", 20);
const fibonacciFrame = frame("fibonacci", "/src/fib.rs", 12);
const layoutFrame = frame("layout_widget", "/src/ui.rs", 88);
const measureFrame = frame("measure_text", "/src/text.rs", 44);

function readyProfile() {
  return {
    status: "ready",
    thread: "cove-ui",
    hz: 500,
    durationMs: 220,
    sampleIntervalMs: 2,
    samplesTaken: 114,
    samplesDropped: 4,
    retainedSamples: 110,
    samplesPerSecond: 518.2,
    stacksTruncated: false,
    stacks: [
      { samples: 50, percent: 45.45, frames: [fibonacciFrame, runFrame, mainFrame] },
      { samples: 25, percent: 22.73, frames: [layoutFrame, runFrame, mainFrame] },
      { samples: 10, percent: 9.09, frames: [measureFrame, layoutFrame, runFrame, mainFrame] },
      { samples: 20, percent: 18.18, frames: [unresolved, runFrame, mainFrame] },
      { samples: 5, percent: 4.55, frames: [] },
    ],
  };
}

function runningProfile(live) {
  return {
    status: "running",
    thread: "cove-ui",
    hz: 500,
    durationMs: 16,
    sampleIntervalMs: 2,
    samplesTaken: live,
    samplesDropped: 0,
    retainedSamples: 0,
    samplesPerSecond: 437.5,
    stacksTruncated: false,
    stacks: [],
  };
}

function hotspotsPayload(profile = readyProfile()) {
  return {
    status: "ready",
    thread: profile.thread,
    hz: profile.hz,
    durationMs: profile.durationMs,
    sampleIntervalMs: profile.sampleIntervalMs,
    samplesTaken: profile.samplesTaken,
    samplesDropped: profile.samplesDropped,
    retainedSamples: profile.retainedSamples,
    truncated: false,
    hotspots: [
      {
        label: "fibonacci /src/fib.rs:12",
        function: "fibonacci",
        filePath: "/src/fib.rs",
        line: 12,
        selfSamples: 50,
        inclusiveSamples: 50,
        selfMs: 100,
        inclusiveMs: 100,
        selfPercent: 45.45,
        inclusivePercent: 45.45,
      },
      {
        label: "layout_widget /src/ui.rs:88",
        function: "layout_widget",
        filePath: "/src/ui.rs",
        line: 88,
        selfSamples: 25,
        inclusiveSamples: 35,
        selfMs: 50,
        inclusiveMs: 70,
        selfPercent: 22.73,
        inclusivePercent: 31.82,
      },
      {
        label: "run /src/main.rs:20",
        function: "run",
        filePath: "/src/main.rs",
        line: 20,
        selfSamples: 0,
        inclusiveSamples: 105,
        selfMs: 0,
        inclusiveMs: 210,
        selfPercent: 0,
        inclusivePercent: 95.45,
      },
    ],
  };
}

function truncatedProfile() {
  const stacks = [];
  for (let index = 0; index < 259; index += 1) {
    const leaf = frame(`leaf_${index}`, "/src/leaf.rs", 10 + index);
    const dispatch = frame(`dispatch_${index % 90}`, "/src/dispatch.rs", 30 + (index % 90));
    stacks.push({
      samples: 1,
      percent: 100 / 260,
      frames: [leaf, dispatch, runFrame, mainFrame],
    });
  }
  const deep = [];
  for (let index = 0; index < 60; index += 1) {
    deep.push(frame(`deep_${index}`, "/src/deep.rs", index + 1));
  }
  stacks.unshift({ samples: 1, percent: 100 / 260, frames: deep });
  return {
    status: "ready",
    thread: "cove-ui",
    hz: 500,
    durationMs: 400,
    sampleIntervalMs: 2,
    samplesTaken: 260,
    samplesDropped: 0,
    retainedSamples: 260,
    samplesPerSecond: 650,
    stacksTruncated: true,
    stacks,
  };
}

function emptyProfile() {
  return {
    status: "ready",
    thread: "cove-ui",
    hz: 500,
    durationMs: 200,
    sampleIntervalMs: 2,
    samplesTaken: 0,
    samplesDropped: 0,
    retainedSamples: 0,
    samplesPerSecond: 0,
    stacksTruncated: false,
    stacks: [],
  };
}

// A WebAssembly answer: measured phase durations, no sampling rate.
function instrumentedProfile() {
  const frame = { label: "frame", function: "Cove frame" };
  return {
    status: "ready",
    source: "instrumented",
    thread: "web",
    hz: 0,
    durationMs: 400,
    sampleIntervalMs: 0,
    samplesTaken: 24,
    samplesDropped: 0,
    // One unit is one measured millisecond: 86 + 170 + 144 = 400 ms.
    retainedSamples: 400,
    samplesPerSecond: 60,
    stacksTruncated: false,
    stacks: [
      { samples: 86, percent: 21.5, frames: [frame] },
      { samples: 170, percent: 42.5, frames: [frame, { label: "layout", function: "layout" }] },
      { samples: 144, percent: 36.0, frames: [frame, { label: "encode", function: "encode" }] },
    ],
  };
}

function instrumentedHotspots(profile = instrumentedProfile()) {
  return {
    status: "ready",
    source: "instrumented",
    thread: profile.thread,
    hz: 0,
    durationMs: profile.durationMs,
    sampleIntervalMs: 0,
    samplesTaken: profile.samplesTaken,
    samplesDropped: 0,
    retainedSamples: profile.retainedSamples,
    truncated: false,
    hotspots: [
      { label: "frame", function: "Cove frame", selfSamples: 24, inclusiveSamples: 24, selfMs: 86, inclusiveMs: 400, selfPercent: 21.5, inclusivePercent: 100 },
      { label: "layout", function: "layout", selfSamples: 24, inclusiveSamples: 24, selfMs: 170, inclusiveMs: 170, selfPercent: 42.5, inclusivePercent: 42.5 },
    ],
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
  const flow = { live: 7, stopped: false };

  function session() {
    return {
      platform: "macos",
      permissions: { screen_capture: false, accessibility: false },
      targets: [
        {
          id: "runtime-1",
          title: "Profile Target",
          platform: "macos",
          pid: 4242,
          kind: "runtime",
          capabilities: ["tree", "timing", "trace", "cpu_profile"],
        },
      ],
    };
  }

  function profileResponse() {
    if (mode === "unsupported") {
      throw rpcError("unsupported", "CPU sampling is not available on this platform. The sampler reports unsupported and synthesizes no samples.");
    }
    if (mode === "empty") return emptyProfile();
    if (mode === "instrumented") return instrumentedProfile();
    if (mode === "truncated") return truncatedProfile();
    if (mode === "idle") {
      return {
        status: "idle",
        thread: "cove-ui",
        hz: 1,
        durationMs: 0,
        sampleIntervalMs: 1000,
        samplesTaken: 0,
        samplesDropped: 0,
        retainedSamples: 0,
        samplesPerSecond: null,
        stacksTruncated: false,
        stacks: [],
      };
    }
    if (mode === "running") {
      if (!flow.stopped) {
        flow.live += 9;
        return runningProfile(flow.live);
      }
      return readyProfile();
    }
    return readyProfile();
  }

  function handle(method, params) {
    calls.push({ method, params });
    if (method === "get_ui_session") return session();
    if (method === "record_ui_timings") return { frames: [], dropped: 0, next_after: 0 };
    if (method === "get_ui_profile") return profileResponse();
    if (method === "start_ui_profile") {
      if (mode === "unsupported") return profileResponse();
      if (mode === "running") return runningProfile(flow.live);
      if (mode === "instrumented") return instrumentedProfile();
      return readyProfile();
    }
    if (method === "stop_ui_profile") {
      if (mode === "running") flow.stopped = true;
      if (mode === "instrumented") return instrumentedProfile();
      return mode === "empty" ? emptyProfile() : mode === "truncated" ? truncatedProfile() : readyProfile();
    }
    if (method === "query_ui_hotspots") {
      if (mode === "unsupported") throw rpcError("unsupported", "CPU sampling is not available on this platform.");
      if (mode === "instrumented") return instrumentedHotspots();
      if (mode === "empty") return { ...emptyProfile(), hotspots: [] };
      if (mode === "truncated") return { ...truncatedProfile(), hotspots: [] };
      return hotspotsPayload();
    }
    throw rpcError("unknown_command", `Unknown command ${method}.`);
  }

  return { calls, flow, handle };
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
    `--window-size=${process.env.PROFILE_WINDOW || "1440,900"}`,
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

async function openProfile(cdp) {
  await waitFor(cdp, "controller connection", `document.getElementById("connection-text")?.textContent === "Connected"`);
  await click(cdp, "#tab-performance");
  await waitFor(cdp, "performance view", `document.getElementById("view-performance")?.hidden === false`);
  await click(cdp, "#tab-performance-profile");
  await waitFor(cdp, "profile panel", `document.getElementById("performance-profile")?.hidden === false`);
}

async function typeFilter(cdp, value) {
  const ok = await cdp.evaluate(`(() => {
    const input = document.getElementById("profile-filter");
    if (!input) return false;
    input.value = ${JSON.stringify(value)};
    input.dispatchEvent(new Event("input", { bubbles: true }));
    return true;
  })()`);
  assert.equal(ok, true, "profile filter present");
  await new Promise(resolve => setTimeout(resolve, 60));
}

async function expandByLabel(cdp, label) {
  const clicked = await cdp.evaluate(`(() => {
    const button = [...document.querySelectorAll("#profile-view-body .profile-twisty")]
      .find(node => node.getAttribute("aria-label") === ${JSON.stringify(label)});
    if (!button) return false;
    button.click();
    return true;
  })()`);
  assert.equal(clicked, true, `twisty ${label}`);
  await new Promise(resolve => setTimeout(resolve, 60));
}

async function screenshot(cdp, path) {
  if (!path) return;
  const shot = await cdp.send("Page.captureScreenshot", { format: "png" });
  await writeFile(path, Buffer.from(shot.data, "base64"));
}

async function page(cdp) {
  return cdp.evaluate(`(() => {
    const panel = document.getElementById("performance-profile");
    const q = selector => panel?.querySelector(selector);
    const qa = selector => [...(panel?.querySelectorAll(selector) || [])];
    const stat = label => {
      const node = qa(".stat").find(row => row.querySelector(".stat-label")?.textContent === label);
      return node?.querySelector(".stat-value")?.textContent.trim() ?? "";
    };
    return {
      status: q(".perf-chip")?.textContent.trim() ?? "",
      stats: {
        elapsed: stat("Elapsed"),
        taken: stat("Samples") || stat("Frames"),
        retained: stat("Retained"),
        dropped: stat("Dropped"),
        rate: stat("Observed rate"),
        interval: stat("Interval"),
        thread: stat("Thread"),
      },
      view: q(".profile-view-tab[aria-selected='true']")?.textContent ?? "",
      treeRows: qa(".profile-tree-row").map(row => ({
        label: row.querySelector(".profile-fn")?.textContent ?? "",
        location: row.querySelector(".profile-location")?.textContent ?? "",
        cells: [...row.querySelectorAll(".num")].map(node => node.textContent),
        level: row.getAttribute("aria-level"),
        selected: row.classList.contains("is-selected"),
        expanded: row.getAttribute("aria-expanded"),
      })),
      root: q(".profile-tree-root")?.textContent ?? "",
      notes: qa(".perf-note").map(node => node.textContent),
      empty: qa(".empty-inline").map(node => node.textContent),
      flameBars: qa(".profile-flame-bar").map(bar => ({
        label: bar.textContent,
        title: bar.getAttribute("title"),
        width: bar.style.width,
        left: bar.style.left,
        selected: bar.classList.contains("is-selected"),
        dimmed: bar.classList.contains("is-dimmed"),
      })),
      flameRows: qa(".profile-flame-row").length,
      methods: qa(".profile-methods-table tbody tr").map(row => ({
        cells: [...row.children].map(cell => cell.textContent.trim()),
        selected: row.classList.contains("is-selected"),
      })),
      sort: qa(".profile-methods-table th").find(th => th.getAttribute("aria-sort") && th.getAttribute("aria-sort") !== "none")?.getAttribute("aria-sort") ?? "none",
      stacks: qa(".profile-stacks-table tbody tr").map(row => ({
        samples: row.children[1]?.textContent ?? "",
        share: row.children[2]?.textContent ?? "",
        frames: [...row.querySelectorAll(".profile-stack-frame")].map(frame => ({
          text: frame.textContent,
          leaf: frame.classList.contains("is-leaf"),
          selected: frame.classList.contains("is-selected"),
        })),
        selected: row.classList.contains("is-selected"),
      })),
      selection: document.getElementById("profile-selection")?.hidden === false
        ? document.getElementById("profile-selection")?.textContent ?? ""
        : null,
      stateTitle: q(".state strong")?.textContent ?? "",
      stateMessage: q(".state p")?.textContent ?? "",
    };
  })()`);
}

async function auditProfileDom(cdp) {
  const report = await cdp.evaluate(`(() => {
    const failures = [];
    const ids = [...document.querySelectorAll("[id]")].map(node => node.id);
    const duplicates = ids.filter((id, index) => ids.indexOf(id) !== index);
    if (duplicates.length) failures.push("duplicate ids: " + [...new Set(duplicates)].join(", "));
    const brokenRefs = [];
    for (const attribute of ["aria-controls", "aria-labelledby", "aria-describedby"]) {
      for (const node of document.querySelectorAll("#view-performance [" + attribute + "]")) {
        for (const id of (node.getAttribute(attribute) || "").split(/\\s+/).filter(Boolean)) {
          if (!document.getElementById(id)) brokenRefs.push(attribute + "=" + id);
        }
      }
    }
    if (brokenRefs.length) failures.push("broken references: " + [...new Set(brokenRefs)].join(", "));
    const symbols = new Set([...document.querySelectorAll("symbol[id]")].map(node => node.id));
    const brokenIcons = [...document.querySelectorAll("#view-performance use[href^='#']")]
      .map(node => node.getAttribute("href").slice(1))
      .filter(id => !symbols.has(id));
    if (brokenIcons.length) failures.push("broken icon references: " + [...new Set(brokenIcons)].join(", "));
    const tabs = [...document.querySelectorAll("#view-performance .profile-view-tab")];
    for (const tab of tabs) {
      if (tab.getAttribute("role") !== "tab") failures.push((tab.id || "tab") + " is not role=tab");
      if (!document.getElementById(tab.getAttribute("aria-controls"))) failures.push((tab.id || "tab") + " controls a missing panel");
      if (tab.tagName === "BUTTON" && !tab.hasAttribute("type")) failures.push((tab.id || "tab") + " has no type");
    }
    const unlabeled = [];
    for (const node of document.querySelectorAll("#view-performance button, #view-performance input, #view-performance select")) {
      const name = (node.getAttribute("aria-label") || node.getAttribute("title") || "").trim()
        || (node.id && document.querySelector('label[for="' + node.id + '"]')?.textContent.trim())
        || (node.closest("label")?.textContent.trim())
        || (node.tagName === "BUTTON" ? node.textContent.trim() : "");
      if (!name) unlabeled.push(node.id || node.className || node.tagName);
    }
    if (unlabeled.length) failures.push("unlabeled controls: " + unlabeled.join(", "));
    const untyped = [...document.querySelectorAll("#view-performance button")].filter(button => !button.hasAttribute("type")).map(button => button.id || button.className || button.textContent.trim());
    if (untyped.length) failures.push("buttons without type: " + untyped.join(", "));
    return failures;
  })()`);
  assert.deepEqual(report, [], `Profile DOM audit failed:\n${report.join("\n")}`);
  console.log("PASS  DOM audit: unique ids, live profile tab references, named controls with type");
}

let audited = false;

async function runScenario(mode, body) {
  const server = await startServer(mode);
  const profile = await mkdtemp(join(tmpdir(), "cove-profile-harness-"));
  let chrome = null;
  let cdp = null;
  try {
    chrome = await startChrome(profile, server.url);
    cdp = await connect(chrome.wsUrl);
    await openProfile(cdp);
    await body({ cdp, fake: server.fake, mode });
    assert.deepEqual(cdp.exceptions, [], `page exceptions: ${cdp.exceptions.join("\n")}`);
  } finally {
    cdp?.close();
    await chrome?.close();
    await server.close();
    await rm(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
  }
}

async function scenarioReady() {
  await runScenario("ready", async ({ cdp }) => {
    await waitFor(cdp, "ready profile", `document.querySelector("#performance-profile .perf-chip")?.textContent.trim() === "Ready"`);
    await waitFor(cdp, "hot spots", `document.querySelectorAll("#performance-profile .profile-tree-row").length > 0`);
    if (!audited) {
      await auditProfileDom(cdp);
      audited = true;
    }
    let view = await page(cdp);
    assert.equal(view.view, "Call tree");
    assert.equal(view.status, "Ready");
    assert.equal(view.stats.taken, "114 taken");
    assert.equal(view.stats.retained, "110 samples");
    assert.equal(view.stats.dropped, "4");
    assert.match(view.stats.rate, /518/);
    assert.match(view.stats.interval, /2\.00 ms/);
    assert.equal(view.stats.thread, "cove-ui");
    assert.equal(view.treeRows.length, 1, "only the outermost frame is expanded by default");
    assert.equal(view.treeRows[0].label, "main");
    assert.deepEqual(view.treeRows[0].cells, ["0", "105", "0.0%", "95.5%"]);
    assert.match(view.root, /All samples/);
    assert.match(view.root, /105 samples/);

    // Keyboard: ArrowRight expands the focused row, ArrowLeft collapses it.
    await cdp.evaluate(`(() => {
      const row = document.querySelector("#profile-view-body .profile-tree-row");
      row.focus();
      row.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));
    })()`);
    await waitFor(cdp, "keyboard expand", `document.querySelectorAll("#profile-view-body .profile-tree-row").length === 2`);
    await cdp.evaluate(`(() => {
      const row = document.querySelector("#profile-view-body .profile-tree-row");
      row.focus();
      row.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowLeft", bubbles: true }));
    })()`);
    await waitFor(cdp, "keyboard collapse", `document.querySelectorAll("#profile-view-body .profile-tree-row").length === 1`);

    await expandByLabel(cdp, "Expand main");
    view = await page(cdp);
    assert.deepEqual(view.treeRows.map(row => row.label), ["main", "run"]);
    assert.deepEqual(view.treeRows[1].cells, ["0", "105", "0.0%", "95.5%"]);
    await expandByLabel(cdp, "Expand run");
    view = await page(cdp);
    assert.deepEqual(view.treeRows.map(row => row.label), ["main", "run", "fibonacci", "layout_widget", "0x104a2b3c4"]);
    const fibonacciRow = view.treeRows.find(row => row.label === "fibonacci");
    assert.deepEqual(fibonacciRow.cells, ["50", "50", "45.5%", "45.5%"]);
    const layoutRow = view.treeRows.find(row => row.label === "layout_widget");
    assert.deepEqual(layoutRow.cells, ["25", "35", "22.7%", "31.8%"]);
    assert.equal(fibonacciRow.location, "/src/fib.rs:12");
    assert.match(view.notes.join(" "), /5 samples from 1 stack have no captured frames/);
    if (process.env.PROFILE_SHOT) {
      const shot = await cdp.send("Page.captureScreenshot", { format: "png" });
      await writeFile(process.env.PROFILE_SHOT, Buffer.from(shot.data, "base64"));
    }
    assert.match(view.notes.join(" "), /45\.5%|retained samples/);

    // Collapse via the twisty.
    await expandByLabel(cdp, "Collapse run");
    view = await page(cdp);
    assert.deepEqual(view.treeRows.map(row => row.label), ["main", "run"]);

    // Selection sync: select fibonacci in the call tree.
    await expandByLabel(cdp, "Expand run");
    await cdp.evaluate(`(() => {
      const row = [...document.querySelectorAll("#profile-view-body .profile-tree-row")]
        .find(node => node.querySelector(".profile-fn")?.textContent === "fibonacci");
      row?.click();
    })()`);
    await waitFor(cdp, "selection summary", `document.getElementById("profile-selection")?.hidden === false`);
    view = await page(cdp);
    assert.match(view.selection, /fibonacci/);
    assert.match(view.selection, /self 50 \(45\.5%\)/);
    assert.match(view.selection, /inclusive 50 \(45\.5%\)/);
    assert.equal(view.treeRows.find(row => row.label === "fibonacci").selected, true);

    // Filter the call tree; matches are revealed, ancestors kept, and unmatched branches pruned.
    await typeFilter(cdp, "fib");
    view = await page(cdp);
    assert.deepEqual(view.treeRows.map(row => row.label), ["main", "run", "fibonacci"]);
    assert.equal(view.treeRows.at(-1).cells[0], "50");
    await typeFilter(cdp, "no-such-function");
    view = await page(cdp);
    assert.equal(view.treeRows.length, 0);
    assert.match(view.empty.join(" "), /No function matches/);
    await typeFilter(cdp, "");

    // Bottom-up.
    await click(cdp, "#profile-view-tab-bottom-up");
    await waitFor(cdp, "bottom-up view", `document.querySelector("#profile-view-tab-bottom-up")?.getAttribute("aria-selected") === "true"`);
    view = await page(cdp);
    assert.equal(view.view, "Bottom-up");
    assert.equal(view.treeRows[0].label, "fibonacci");
    assert.deepEqual(view.treeRows[0].cells, ["50", "50", "45.5%", "45.5%"]);
    await expandByLabel(cdp, "Expand callers of fibonacci");
    await expandByLabel(cdp, "Expand caller run");
    view = await page(cdp);
    assert.deepEqual(view.treeRows.map(row => row.label), ["fibonacci", "run", "main", "layout_widget", "0x104a2b3c4", "measure_text"]);
    assert.equal(view.treeRows[1].selected, false);
    await screenshot(cdp, process.env.PROFILE_SHOT_BOTTOM_UP);
    await cdp.evaluate(`(() => {
      const row = [...document.querySelectorAll("#profile-view-body .profile-tree-row")]
        .find(node => node.querySelector(".profile-fn")?.textContent === "run");
      row?.click();
    })()`);
    view = await page(cdp);
    assert.match(view.selection, /self 0 \(0\.0%\)/);
    assert.match(view.selection, /inclusive 105 \(95\.5%\)/);

    // Flame chart.
    await click(cdp, "#profile-view-tab-flame-chart");
    await waitFor(cdp, "flame view", `document.querySelector("#profile-view-tab-flame-chart")?.getAttribute("aria-selected") === "true"`);
    view = await page(cdp);
    assert.ok(view.flameBars.length >= 6, `flame bars ${view.flameBars.length}`);
    assert.equal(view.flameBars[0].label, "All samples");
    assert.ok(Math.abs(parseFloat(view.flameBars[0].width) - 95.45) < 0.05, `root width ${view.flameBars[0].width}`);
    const fibonacciBar = view.flameBars.find(bar => bar.label.startsWith("fibonacci"));
    assert.ok(fibonacciBar, "fibonacci bar");
    assert.match(fibonacciBar.title, /\/src\/fib\.rs:12/);
    assert.match(fibonacciBar.title, /50 samples · 45\.5%/);
    assert.equal(view.flameBars.find(bar => bar.label.startsWith("run"))?.selected, true, "selection carries across views");
    await cdp.evaluate(`(() => {
      const bar = [...document.querySelectorAll("#profile-view-body .profile-flame-bar")]
        .find(node => node.textContent.startsWith("fibonacci"));
      bar?.click();
    })()`);
    view = await page(cdp);
    assert.match(view.selection, /fibonacci/);
    assert.equal(view.flameBars.find(bar => bar.label.startsWith("fibonacci")).selected, true);
    await cdp.evaluate(`(() => {
      const bar = [...document.querySelectorAll("#profile-view-body .profile-flame-bar")]
        .find(node => node.textContent.startsWith("layout_widget"));
      bar?.click();
    })()`);
    view = await page(cdp);
    assert.match(view.selection, /layout_widget/);
    assert.match(view.selection, /self 25 \(22\.7%\)/);
    assert.equal(view.flameBars.find(bar => bar.label.startsWith("layout_widget")).selected, true);
    await typeFilter(cdp, "fib");
    view = await page(cdp);
    assert.match(view.notes.join(" "), /1 bar matches/);
    assert.ok(view.flameBars.some(bar => bar.dimmed));
    await typeFilter(cdp, "");
    await screenshot(cdp, process.env.PROFILE_SHOT_FLAME);

    // Methods table: sorting and selection.
    await click(cdp, "#profile-view-tab-methods");
    await waitFor(cdp, "methods view", `document.querySelectorAll(".profile-methods-table tbody tr").length === 3`);
    view = await page(cdp);
    assert.deepEqual(view.methods.map(row => row.cells[0]), ["fibonacci", "layout_widget", "run"]);
    assert.equal(view.sort, "descending");
    await cdp.evaluate(`(() => {
      const button = [...document.querySelectorAll(".profile-methods-table th button")]
        .find(node => node.textContent.startsWith("Self samples"));
      button?.click();
    })()`);
    await waitFor(cdp, "ascending methods", `document.querySelector(".profile-methods-table th[aria-sort='ascending']") !== null`);
    view = await page(cdp);
    assert.deepEqual(view.methods.map(row => row.cells[0]), ["run", "layout_widget", "fibonacci"]);
    await typeFilter(cdp, "layout");
    view = await page(cdp);
    assert.deepEqual(view.methods.map(row => row.cells[0]), ["layout_widget"]);
    assert.match(view.notes.join(" "), /1 of 3 methods match/);
    await typeFilter(cdp, "");
    await cdp.evaluate(`(() => {
      const row = [...document.querySelectorAll(".profile-methods-table tbody tr")]
        .find(node => node.children[0]?.textContent === "fibonacci");
      row?.click();
    })()`);
    view = await page(cdp);
    assert.equal(view.methods.find(row => row.cells[0] === "fibonacci").selected, true);
    assert.match(view.selection, /fibonacci/);
    await screenshot(cdp, process.env.PROFILE_SHOT_METHODS);

    // Stacks view keeps the ranked stacks with correct leaf labels.
    await click(cdp, "#profile-view-tab-stacks");
    await waitFor(cdp, "stacks view", `document.querySelectorAll(".profile-stacks-table tbody tr").length === 5`);
    view = await page(cdp);
    assert.equal(view.stacks[0].samples, "50");
    assert.equal(view.stacks[0].share, "45.5%");
    assert.equal(view.stacks[0].frames[0].text, "fibonacci /src/fib.rs:12");
    assert.equal(view.stacks[0].frames[0].leaf, true);
    assert.equal(view.stacks[0].frames[1].leaf, false);
    assert.equal(view.stacks[0].selected, true, "selection from Methods highlights the matching stack");
    assert.equal(view.stacks[0].frames[0].selected, true);
    await typeFilter(cdp, "fibonacci");
    view = await page(cdp);
    assert.equal(view.stacks.length, 1);
    await typeFilter(cdp, "");

    // Narrow window: the profile panel scrolls, the tree keeps its columns and
    // the flame chart stays inside its own scroll container.
    await cdp.evaluate(`document.getElementById("profile-view-tab-call-tree").click()`);
    await cdp.send("Emulation.setDeviceMetricsOverride", { width: 760, height: 470, deviceScaleFactor: 1, mobile: false });
    await new Promise(resolve => setTimeout(resolve, 150));
    const narrow = await cdp.evaluate(`(() => {
      const content = document.getElementById("performance-content");
      const tree = document.querySelector(".profile-tree");
      const panel = document.getElementById("performance-profile");
      return {
        overflow: getComputedStyle(content).overflowY,
        contentScroll: content.scrollHeight > content.clientHeight,
        treePresent: Boolean(tree),
        treeWidth: Math.round(tree?.getBoundingClientRect().width || 0),
        panelWidth: Math.round(panel.getBoundingClientRect().width || 0),
        bodyOverflow: document.body.scrollWidth - window.innerWidth,
      };
    })()`);
    assert.equal(narrow.overflow, "auto");
    assert.equal(narrow.treePresent, true);
    assert.equal(narrow.contentScroll, true, "narrow content scrolls");
    assert.ok(narrow.treeWidth > 0 && narrow.treeWidth <= narrow.panelWidth, `tree ${narrow.treeWidth} within panel ${narrow.panelWidth}`);
    assert.ok(narrow.bodyOverflow <= 1, `body overflow ${narrow.bodyOverflow}`);
    await cdp.send("Emulation.clearDeviceMetricsOverride");

    // Inspector/Network/Logs/Performance/Memory/Debugger still render.
    for (const name of ["network", "logs", "performance", "memory", "inspector", "debugger"]) {
      await click(cdp, `#tab-${name}`);
      await waitFor(cdp, `${name} view`, `document.getElementById("view-${name}")?.hidden === false`);
    }
    console.log("PASS  ready profile: call tree, bottom-up, flame chart, methods sorting, stacks, filter and selection sync");
  });
}

async function scenarioIdleRate() {
  await runScenario("idle", async ({ cdp, fake }) => {
    await waitFor(cdp, "profile idle", `document.querySelector("#performance-profile .perf-chip")?.textContent.trim() === "Idle"`);
    const rate = await cdp.evaluate(`document.getElementById("profile-hz")?.value`);
    assert.equal(rate, "1000", "an idle runtime answer must not overwrite the selected sample rate");
    const interval = await text(cdp, "#performance-profile .stat:nth-child(7) .stat-value");
    assert.match(interval, /1\.00 ms/);
    await click(cdp, "#profile-start");
    await waitFor(cdp, "ready after idle start", `document.querySelector("#performance-profile .perf-chip")?.textContent.trim() === "Ready"`);
    const start = fake.calls.find(call => call.method === "start_ui_profile");
    assert.equal(start?.params.hz, 1000, "the start request carries the selected rate");
    console.log("PASS  idle runtime defaults do not overwrite the selected sample rate or interval");
  });
}

async function scenarioRunning() {
  await runScenario("running", async ({ cdp }) => {
    await waitFor(cdp, "profile controls", `document.getElementById("profile-start") !== null`);
    await click(cdp, "#profile-start");
    await waitFor(cdp, "running status", `document.querySelector("#performance-profile .perf-chip")?.textContent.trim() === "Running"`);
    let view = await page(cdp);
    assert.equal(view.status, "Running");
    assert.equal(view.treeRows.length, 0);
    assert.equal(view.flameBars.length, 0);
    assert.match(view.empty.join(" "), /Sampling is running\. Stacks are ranked when the sampler stops/);
    assert.match(view.stats.taken, /\d+ taken/);
    await waitFor(cdp, "live samples", `/(\\d[\\d,]*)\\s+taken/.test(document.querySelectorAll("#performance-profile .stat")[2]?.textContent || "") && Number((document.querySelectorAll("#performance-profile .stat")[2]?.textContent || "").replace(/[^\\d]/g, "")) >= 16`);
    view = await page(cdp);
    assert.equal(view.status, "Running");
    assert.ok(Number(view.stats.taken.replace(/\D+/g, "")) >= 16, `live sample count ${view.stats.taken}`);

    await click(cdp, "#profile-stop");
    await waitFor(cdp, "ready after stop", `document.querySelector("#performance-profile .perf-chip")?.textContent.trim() === "Ready"`);
    await waitFor(cdp, "tree after stop", `document.querySelectorAll("#profile-view-body .profile-tree-row").length === 1`);
    view = await page(cdp);
    assert.equal(view.view, "Call tree");
    assert.equal(view.treeRows[0].label, "main");
    assert.equal(view.status, "Ready");
    console.log("PASS  running profile: explicit running state with live counts, no synthesized stacks, ready after stop");
  });
}

async function scenarioEmpty() {
  await runScenario("empty", async ({ cdp }) => {
    await waitFor(cdp, "empty profile", `document.querySelector("#performance-profile .perf-chip")?.textContent.trim() === "Ready"`);
    await waitFor(cdp, "empty call tree", `document.querySelector("#profile-view-body .empty-inline") !== null`);
    let view = await page(cdp);
    assert.match(view.empty.join(" "), /no stack samples/i);
    assert.equal(view.treeRows.length, 0);
    await click(cdp, "#profile-view-tab-bottom-up");
    view = await page(cdp);
    assert.match(view.empty.join(" "), /no stack samples/i);
    await click(cdp, "#profile-view-tab-flame-chart");
    view = await page(cdp);
    assert.match(view.empty.join(" "), /no stack samples to plot/i);
    assert.equal(view.flameBars.length, 0);
    await click(cdp, "#profile-view-tab-stacks");
    view = await page(cdp);
    assert.match(view.empty.join(" "), /no stack samples/i);
    assert.equal(view.stacks.length, 0);
    await click(cdp, "#profile-view-tab-methods");
    view = await page(cdp);
    assert.match(view.empty.join(" "), /No hot functions were computed from the retained samples/);
    assert.equal(view.methods.length, 0);
    console.log("PASS  empty profile: explicit empty state in every view, no invented nodes or bars");
  });
}

async function scenarioUnsupported() {
  await runScenario("unsupported", async ({ cdp }) => {
    await waitFor(cdp, "unsupported state", `document.querySelector("#performance-profile .state strong")?.textContent.includes("CPU sampling is unsupported")`);
    const view = await page(cdp);
    assert.match(view.stateTitle, /CPU sampling is unsupported/);
    assert.match(view.stateMessage, /CPU sampling is not available on this platform/);
    assert.equal(view.treeRows.length, 0);
    assert.equal(view.flameBars.length, 0);
    assert.equal(await cdp.evaluate(`Boolean(document.querySelector("#profile-view-body"))`), false);
    console.log("PASS  unsupported: exact runtime reason, no profile views and no synthesized data");
  });
}

async function scenarioTruncated() {
  await runScenario("truncated", async ({ cdp }) => {
    await waitFor(cdp, "truncated profile", `document.querySelector("#performance-profile .perf-chip")?.textContent.trim() === "Ready"`);
    await waitFor(cdp, "tree rows", `document.querySelectorAll("#profile-view-body .profile-tree-row").length > 0`);
    const view = await page(cdp);
    assert.ok(view.treeRows.length <= 1200, `bounded rows ${view.treeRows.length}`);
    const notes = view.notes.join(" ");
    assert.match(notes, /runtime returned only its 200 hottest stack groups/);
    assert.match(notes, /client read the 200 hottest of 260 distinct stacks/);
    assert.match(notes, /48-frame tree cap/);
    assert.match(notes, /64-child or 3000-node cap/);
    await click(cdp, "#profile-view-tab-flame-chart");
    const flame = await page(cdp);
    assert.ok(flame.flameBars.length > 0);
    assert.ok(flame.flameBars.length <= 320, `bounded flame bars ${flame.flameBars.length}`);
    assert.match(flame.notes.join(" "), /deeper than 40 levels|first 320 bars|runtime returned only/);
    await click(cdp, "#profile-view-tab-bottom-up");
    const bottom = await page(cdp);
    assert.ok(bottom.treeRows.length <= 1200);
    const bottomNotes = bottom.notes.join(" ");
    assert.match(bottomNotes, /branch|cap|hottest stack groups/i);
    console.log("PASS  truncated profile: node/row/bar caps are applied and labeled, wire truncation is stated");
  });
}

async function scenarioInstrumented() {
  await runScenario("instrumented", async ({ cdp }) => {
    await waitFor(cdp, "instrumented profile", `document.querySelector("#performance-profile .stat-row .perf-chip")?.textContent.trim() === "Ready"`);
    const view = await page(cdp);
    // A sample-rate control would claim a rate the browser cannot honour.
    assert.equal(await cdp.evaluate(`document.getElementById("profile-hz") === null`), true, "no sample-rate select");
    assert.match(await cdp.evaluate(`document.querySelector("#performance-profile .profile-controls")?.textContent || ""`), /Phase timing/);
    assert.match(await cdp.evaluate(`document.getElementById("profile-start")?.title || ""`), /frame phase/);
    assert.equal(view.stats.taken, "24 recorded", "the frame count is named, not a sample count");
    assert.equal(view.stats.retained, "400 ms measured", "the retained unit is measured milliseconds");
    assert.match(view.stats.rate, /60 frames\/s/);
    assert.equal(await cdp.evaluate(`[...document.querySelectorAll("#performance-profile .stat-label")]
      .some(label => label.textContent === "Interval")`), false, "no interval is reported without a rate");
    assert.deepEqual(
      view.treeRows.map(row => row.label).sort(),
      ["Cove frame", "encode", "layout"],
      "the measured frame expands into its phases",
    );
    const layoutRow = view.treeRows.find(row => row.label === "layout");
    // The phase leaf carries its 170 measured milliseconds as an inclusive total
    // and no self time of its own.
    assert.deepEqual(layoutRow.cells, ["0", "170", "0.0%", "42.5%"], "phase units are measured milliseconds");
    assert.match(view.root, /All frames/, "the shared root of the measured stacks is named for frames");
    assert.match(view.root, /400 ms/, "the units add up to the measured frame time");
    console.log("PASS  instrumented profile: phase timing replaces the sample rate and reports frames");
  });
}

const scenarios = [
  ["ready", scenarioReady],
  ["instrumented", scenarioInstrumented],
  ["idle", scenarioIdleRate],
  ["running", scenarioRunning],
  ["empty", scenarioEmpty],
  ["unsupported", scenarioUnsupported],
  ["truncated", scenarioTruncated],
];

async function main() {
  const selected = process.argv.slice(2);
  for (const [name, run] of scenarios) {
    if (selected.length && !selected.includes(name)) continue;
    console.log(`--- ${name}`);
    await run();
  }
}

main().catch(error => {
  console.error(error.stack || error.message);
  process.exit(1);
});
