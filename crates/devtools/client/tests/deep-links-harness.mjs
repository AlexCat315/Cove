#!/usr/bin/env node
// Fake-controller harness for the Deep Links view. It serves the real
// index.html/inspector.js/inspector.css, answers /rpc with a deep-links wire
// payload, and drives the page through Chrome DevTools Protocol. It covers
// declared Android/Apple links with issue locations, a project with no deep
// links, malformed payloads and roots, unavailable/error/unsupported states,
// null results, the DOM audit, and remembering the last validated root across
// a reload. No Rust process starts and no link or issue is invented outside the
// fake controller's responses.
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { access } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import assert from "node:assert/strict";
import { serveClientAsset } from "./harness-server.mjs";

const chromePath = process.env.CHROME_PATH
  || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

const validRoot = "/tmp/cove-deep-links/valid";
const secondRoot = "/tmp/cove-deep-links/second";
const androidManifest = `${validRoot}/Platforms/Android/app/src/main/AndroidManifest.xml`;
const assetLinks = `${validRoot}/Platforms/Android/app/src/main/assets/assetlinks.json`;
const infoPlist = `${validRoot}/Platforms/iOS/Runner/Info.plist`;
const siteAssociation = `${validRoot}/Platforms/iOS/Runner/apple-app-site-association`;

function validPayload(root = validRoot) {
  const absolute = candidate => `${root}/${candidate}`;
  return {
    root,
    files: [
      { path: absolute("Platforms/Android/app/src/main/AndroidManifest.xml"), relative: "Platforms/Android/app/src/main/AndroidManifest.xml", platform: "android", kind: "manifest" },
      { path: absolute("Platforms/Android/app/src/main/assets/assetlinks.json"), relative: "Platforms/Android/app/src/main/assets/assetlinks.json", platform: "android", kind: "assetlinks" },
      { path: absolute("Platforms/iOS/Runner/Info.plist"), relative: "Platforms/iOS/Runner/Info.plist", platform: "ios", kind: "info_plist" },
      { path: absolute("Platforms/iOS/Runner/apple-app-site-association"), relative: "Platforms/iOS/Runner/apple-app-site-association", platform: "ios", kind: "apple_app_site_association" },
    ],
    android: {
      configured: true,
      schemes: ["https", "cove"],
      hosts: ["gallery.cove.dev"],
      links: [
        {
          activity: ".MainActivity",
          schemes: ["https"],
          hosts: ["gallery.cove.dev"],
          paths: ["/pets"],
          autoVerify: false,
          viewAction: true,
          browsable: true,
          file: "Platforms/Android/app/src/main/AndroidManifest.xml",
          line: 14,
        },
      ],
      issues: [
        {
          platform: "android",
          severity: "warning",
          code: "missing_auto_verify",
          message: "This filter matches a web host but does not set android:autoVerify=\"true\".",
          file: "Platforms/Android/app/src/main/AndroidManifest.xml",
          location: "Platforms/Android/app/src/main/AndroidManifest.xml:14",
          line: 14,
          fix: "Set android:autoVerify=\"true\" on the intent-filter.",
        },
      ],
    },
    apple: {
      configured: true,
      schemes: ["cove"],
      domains: ["gallery.cove.dev"],
      associatedDomains: ["applinks:gallery.cove.dev"],
      issues: [
        {
          platform: "apple",
          severity: "error",
          code: "missing_association_file",
          message: "https://gallery.cove.dev/.well-known/apple-app-site-association was not found in the project.",
          file: "Platforms/iOS/Runner/Info.plist",
          location: "Platforms/iOS/Runner/Info.plist:22",
          line: 22,
          fix: "Publish https://gallery.cove.dev/.well-known/apple-app-site-association and keep a copy at Platforms/iOS/.well-known/apple-app-site-association.",
        },
      ],
    },
    summary: {
      configured: true,
      android: true,
      apple: true,
      schemes: 3,
      hosts: 1,
      domains: 1,
      issues: 2,
      errors: 1,
      warnings: 1,
      text: "Android: 2 scheme(s), 1 host(s); Apple: 1 scheme(s), 1 domain(s); 1 error(s), 1 warning(s).",
    },
  };
}

function emptyPayload() {
  return {
    root: validRoot,
    files: [],
    android: { configured: false, schemes: [], hosts: [], links: [], issues: [] },
    apple: { configured: false, schemes: [], domains: [], associatedDomains: [], issues: [] },
    summary: {
      configured: false,
      android: false,
      apple: false,
      schemes: 0,
      hosts: 0,
      domains: 0,
      issues: 0,
      errors: 0,
      warnings: 0,
      text: "No deep links configured.",
    },
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

  function session() {
    return {
      platform: "macos",
      permissions: { screen_capture: false, accessibility: false },
      targets: [
        { id: "runtime-1", title: "Deep Links Target", platform: "macos", pid: 4242, kind: "runtime", capabilities: ["tree", "diagnostics"] },
      ],
    };
  }

  function deepLinksResponse(params) {
    if (mode === "unavailable") {
      throw rpcError("unavailable", "The project root is not readable or is outside the allowed project.");
    }
    if (mode === "error") {
      throw rpcError("internal", "The Android manifest parser failed.");
    }
    if (mode === "unknown") {
      throw rpcError("unknown_command", "Unknown command get_ui_deep_links.");
    }
    if (mode === "null") return null;
    if (mode === "empty") return emptyPayload();
    if (mode === "malformed") return { available: true, android: 5, apple: "no", summary: 7 };
    return validPayload(params.root || validRoot);
  }

  function handle(method, params) {
    calls.push({ method, params });
    if (method === "get_ui_session") return session();
    if (method === "get_ui_deep_links") return deepLinksResponse(params || {});
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
    `--window-size=${process.env.DEEP_LINKS_WINDOW || "1440,900"}`,
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
    if (!node || node.disabled) return false;
    node.click();
    return true;
  })()`);
  assert.equal(clicked, true, `click ${selector}`);
}

async function openDeepLinks(cdp, title = `document.getElementById("deep-links-content")?.querySelector(".state strong")?.textContent === "No project validated"`) {
  await waitFor(cdp, "controller connection", `document.getElementById("connection-text")?.textContent === "Connected"`);
  await click(cdp, "#tab-deep-links");
  await waitFor(cdp, "deep links view", `document.getElementById("view-deep-links")?.hidden === false`);
  await waitFor(cdp, "deep links initial state", title);
}

async function setRoot(cdp, root) {
  await cdp.evaluate(`(() => {
    const input = document.getElementById("deep-links-root");
    input.value = ${JSON.stringify(root)};
    input.dispatchEvent(new Event("input", { bubbles: true }));
  })()`);
}

async function validateRoot(cdp, root) {
  await setRoot(cdp, root);
  await click(cdp, "#deep-links-validate");
}

function deepLinksCalls(fake) {
  return fake.calls.filter(call => call.method === "get_ui_deep_links");
}

async function waitForDeepLinksCalls(fake, count) {
  const deadline = Date.now() + 5000;
  while (Date.now() < deadline && deepLinksCalls(fake).length < count) {
    await new Promise(resolvePromise => setTimeout(resolvePromise, 50));
  }
  assert.equal(deepLinksCalls(fake).length, count);
}

async function page(cdp) {
  return cdp.evaluate(`(() => {
    const host = document.getElementById("deep-links-content");
    const q = selector => host?.querySelector(selector);
    const qa = selector => [...(host?.querySelectorAll(selector) || [])];
    const stat = label => {
      const node = qa(".stat").find(row => row.querySelector(".stat-label")?.textContent === label);
      return {
        value: node?.querySelector(".stat-value")?.childNodes[0]?.textContent.trim() ?? "",
        detail: node?.querySelector(".stat-value small")?.textContent.trim() ?? "",
      };
    };
    const block = title => qa(".panel-block").find(item => item.querySelector(".panel-block-head h2")?.textContent === title);
    const table = node => node ? {
      headers: [...node.querySelectorAll("thead th")].map(cell => cell.textContent.trim()),
      rows: [...node.querySelectorAll("tbody tr")].map(row => [...row.children].map(cell => cell.textContent.trim())),
    } : null;
    const tables = title => {
      const panel = block(title);
      if (!panel) return null;
      return [...panel.querySelectorAll(".data-table")].map(table);
    };
    const panel = title => {
      const node = block(title);
      if (!node) return null;
      return {
        meta: node.querySelector(".pane-meta")?.textContent.trim() ?? "",
        source: [...node.querySelectorAll(".dl-source")].map(line => [...line.children].map(cell => cell.textContent.trim()).join(" ")),
        notes: [...node.querySelectorAll(".dl-note")].map(cell => cell.textContent.trim()),
        tables: tables(title),
      };
    };
    return {
      meta: document.getElementById("deep-links-meta")?.textContent.trim() ?? "",
      rootValue: document.getElementById("deep-links-root")?.value ?? "",
      rootError: document.getElementById("deep-links-root-error")?.textContent.trim() ?? "",
      rootErrorHidden: document.getElementById("deep-links-root-error")?.hidden ?? null,
      rootInvalid: document.getElementById("deep-links-root")?.getAttribute("aria-invalid") ?? null,
      stateTitle: q(".state strong")?.textContent.trim() ?? "",
      stateMessage: q(".state p")?.textContent.trim() ?? "",
      stateError: Boolean(q(".state.is-error")),
      scopeRoot: q(".dl-scope code")?.textContent.trim() ?? "",
      scopeStatus: q(".dl-scope-meta")?.textContent.trim() ?? "",
      statusTitle: q(".dl-status strong")?.textContent.trim() ?? "",
      statusText: q(".dl-status span")?.textContent.trim() ?? "",
      stats: {
        androidSchemes: stat("Android schemes"),
        androidHosts: stat("Android hosts"),
        appleSchemes: stat("Apple schemes"),
        appleDomains: stat("Apple domains"),
        files: stat("Files"),
        issues: stat("Issues"),
      },
      files: panel("Discovered files"),
      android: panel("Android"),
      apple: panel("Apple"),
      otherIssues: panel("Other issues"),
      notes: [...host.querySelectorAll(":scope > .dl-note")].map(cell => cell.textContent.trim()),
      inlineError: q(".error-inline")?.textContent.trim() ?? "",
      validateDisabled: document.getElementById("deep-links-validate")?.disabled ?? null,
    };
  })()`);
}

async function auditDeepLinksDom(cdp) {
  const report = await cdp.evaluate(`(() => {
    const failures = [];
    const ids = [...document.querySelectorAll("[id]")].map(node => node.id);
    const duplicates = ids.filter((id, index) => ids.indexOf(id) !== index);
    if (duplicates.length) failures.push("duplicate ids: " + [...new Set(duplicates)].join(", "));
    const brokenRefs = [];
    for (const attribute of ["aria-controls", "aria-labelledby", "aria-describedby"]) {
      for (const node of document.querySelectorAll("#view-deep-links [" + attribute + "]")) {
        for (const id of (node.getAttribute(attribute) || "").split(/\\s+/).filter(Boolean)) {
          if (!document.getElementById(id)) brokenRefs.push(attribute + "=" + id);
        }
      }
    }
    for (const label of document.querySelectorAll("#view-deep-links label[for]")) {
      if (!document.getElementById(label.getAttribute("for"))) brokenRefs.push("for=" + label.getAttribute("for"));
    }
    if (brokenRefs.length) failures.push("broken references: " + [...new Set(brokenRefs)].join(", "));
    const symbols = new Set([...document.querySelectorAll("symbol[id]")].map(node => node.id));
    const brokenIcons = [...document.querySelectorAll("#view-deep-links use[href^='#']")]
      .map(node => node.getAttribute("href").slice(1))
      .filter(id => !symbols.has(id));
    if (brokenIcons.length) failures.push("broken icon references: " + [...new Set(brokenIcons)].join(", "));
    const tabs = [...document.querySelectorAll(".rail-tab")];
    const panels = [...document.querySelectorAll(".view[role='tabpanel']")];
    if (panels.length !== tabs.length) failures.push("rail tabs (" + tabs.length + ") do not match views (" + panels.length + ")");
    for (const panel of panels) {
      if (!tabs.some(tab => tab.getAttribute("aria-controls") === panel.id)) failures.push(panel.id + " has no rail tab");
    }
    const views = new Set(tabs.map(tab => tab.dataset.view));
    if (views.size !== tabs.length) failures.push("duplicate rail views");
    const selected = document.getElementById("tab-deep-links");
    if (!selected) failures.push("deep links rail tab is missing");
    else {
      if (selected.getAttribute("role") !== "tab") failures.push("deep links tab is not role=tab");
      if (selected.getAttribute("aria-controls") !== "view-deep-links") failures.push("deep links tab controls the wrong view");
      if (selected.getAttribute("aria-selected") !== "true") failures.push("deep links tab is not selected");
      if (selected.tabIndex !== 0) failures.push("deep links tab is not keyboard-reachable");
    }
    const unlabeled = [];
    for (const node of document.querySelectorAll("#view-deep-links button, #view-deep-links input, #view-deep-links select")) {
      const name = (node.getAttribute("aria-label") || node.getAttribute("title") || "").trim()
        || (node.id && document.querySelector('label[for="' + node.id + '"]')?.textContent.trim())
        || (node.closest("label")?.textContent.trim())
        || (node.tagName === "BUTTON" ? node.textContent.trim() : "");
      if (!name) unlabeled.push(node.id || node.className || node.tagName);
    }
    if (unlabeled.length) failures.push("unlabeled controls: " + unlabeled.join(", "));
    const untyped = [...document.querySelectorAll("#view-deep-links button")].filter(button => !button.hasAttribute("type")).map(button => button.id || button.textContent.trim());
    if (untyped.length) failures.push("buttons without type: " + untyped.join(", "));
    const rootInput = document.getElementById("deep-links-root");
    if (rootInput?.type !== "text") failures.push("project root input is not type=text");
    if (!rootInput?.getAttribute("placeholder")?.startsWith("/")) failures.push("project root input does not show an absolute placeholder");
    if (rootInput?.getAttribute("aria-describedby") !== "deep-links-root-error") failures.push("project root input does not describe its error node");
    if (document.getElementById("deep-links-root-error")?.getAttribute("role") !== "alert") failures.push("project root error is not a live alert");
    const tables = [...document.querySelectorAll("#view-deep-links .data-table")];
    if (tables.length < 8) failures.push("expected at least 8 deep link tables, found " + tables.length);
    for (const table of tables) {
      if (!table.querySelector("thead th")) failures.push("table without a header row");
      if (!table.querySelector("tbody tr")) failures.push("table without body rows");
    }
    for (const node of document.querySelectorAll("#view-deep-links .state, #view-deep-links .dl-status")) {
      if (!node.textContent.trim()) failures.push("empty state region");
    }
    return failures;
  })()`);
  assert.deepEqual(report, [], `Deep Links DOM audit failed:\n${report.join("\n")}`);
  console.log("PASS  DOM audit: unique ids, live references, icon symbols, rail tabs matching tabpanels, named controls, absolute-root input, issue tables");
}

let audited = false;

async function runScenario(mode, title, body, options = {}) {
  const server = await startServer(mode);
  const profile = await mkdtemp(join(tmpdir(), "cove-deep-links-harness-"));
  let chrome = null;
  let cdp = null;
  try {
    chrome = await startChrome(profile, server.url);
    cdp = await connect(chrome.wsUrl);
    await openDeepLinks(cdp, title);
    await body({ cdp, fake: server.fake, mode, profile });
    assert.deepEqual(cdp.exceptions, [], `page exceptions: ${cdp.exceptions.join("\n")}`);
  } finally {
    cdp?.close();
    await chrome?.close();
    await server.close();
    await rm(profile, { recursive: true, force: true });
  }
}

async function scenarioValid() {
  await runScenario("valid", undefined, async ({ cdp, fake }) => {
    assert.equal(deepLinksCalls(fake).length, 0);
    await validateRoot(cdp, validRoot);
    await waitFor(cdp, "validated scope", `document.querySelector("#deep-links-content .dl-scope code")?.textContent === ${JSON.stringify(validRoot)}`);
    await waitFor(cdp, "issue tables", `document.querySelectorAll("#deep-links-content .data-table").length === 8`);
    assert.equal(deepLinksCalls(fake).length, 1);
    assert.deepEqual(deepLinksCalls(fake)[0].params, { root: validRoot });

    const view = await page(cdp);
    assert.equal(view.rootValue, validRoot);
    assert.equal(view.rootErrorHidden, true);
    assert.equal(view.rootInvalid, "false");
    assert.equal(view.stateTitle, "");
    assert.equal(view.scopeRoot, validRoot);
    assert.equal(view.scopeStatus, "");
    assert.match(view.meta, /^Deep links configured · 4 files · 2 Android schemes · 1 Apple domain · 2 issues · validated \d{2}:\d{2}:\d{2}$/);
    assert.equal(view.stats.androidSchemes.value, "2");
    assert.equal(view.stats.androidHosts.value, "1");
    assert.equal(view.stats.appleSchemes.value, "1");
    assert.equal(view.stats.appleDomains.value, "1");
    assert.equal(view.stats.files.value, "4");
    assert.equal(view.stats.issues.value, "2");
    assert.equal(view.stats.issues.detail, "1 error · 1 warning");

    assert.equal(view.files.meta, "4 reported");
    assert.deepEqual(view.files.tables[0].headers, ["Path", "Platform", "Kind", "Status", "Detail"]);
    assert.deepEqual(view.files.tables[0].rows[0], [androidManifest, "Android", "Android manifest", "Found", "—"]);
    assert.deepEqual(view.files.tables[0].rows[1], [assetLinks, "Android", "Asset links", "Found", "—"]);
    assert.deepEqual(view.files.tables[0].rows[2], [infoPlist, "Apple", "Info.plist", "Found", "—"]);
    assert.deepEqual(view.files.tables[0].rows[3], [siteAssociation, "Apple", "App site association", "Found", "—"]);

    assert.equal(view.android.meta, "2 schemes · 1 host");
    assert.equal(view.android.source[0], `Manifest ${androidManifest}`);
    assert.deepEqual(view.android.tables[0].headers, ["Scheme"]);
    assert.deepEqual(view.android.tables[0].rows, [["https"], ["cove"]]);
    assert.deepEqual(view.android.tables[1].headers, ["Host"]);
    assert.deepEqual(view.android.tables[1].rows, [["gallery.cove.dev"]]);
    assert.deepEqual(view.android.tables[2].headers, ["Activity", "Schemes", "Hosts", "Paths", "Auto verify", "Location"]);
    assert.deepEqual(view.android.tables[2].rows[0], [
      ".MainActivity",
      "https",
      "gallery.cove.dev",
      "/pets",
      "No",
      `${androidManifest}:14`,
    ]);
    assert.deepEqual(view.android.tables[3].headers, ["Severity", "Issue", "Location", "Code", "Fix"]);
    assert.deepEqual(view.android.tables[3].rows[0], [
      "warning",
      "This filter matches a web host but does not set android:autoVerify=\"true\".",
      `${androidManifest}:14`,
      "missing_auto_verify",
      "Set android:autoVerify=\"true\" on the intent-filter.",
    ]);

    assert.equal(view.apple.meta, "1 scheme · 1 domain");
    assert.equal(view.apple.source[0], `Info.plist ${infoPlist}`);
    assert.deepEqual(view.apple.tables[0].rows, [["cove"]]);
    assert.deepEqual(view.apple.tables[1].headers, ["Domain", "Service"]);
    assert.deepEqual(view.apple.tables[1].rows, [["gallery.cove.dev", "applinks"]]);
    assert.deepEqual(view.apple.tables[2].headers, ["Severity", "Issue", "Location", "Code", "Fix"]);
    assert.deepEqual(view.apple.tables[2].rows[0], [
      "error",
      "https://gallery.cove.dev/.well-known/apple-app-site-association was not found in the project.",
      `${infoPlist}:22`,
      "missing_association_file",
      "Publish https://gallery.cove.dev/.well-known/apple-app-site-association and keep a copy at Platforms/iOS/.well-known/apple-app-site-association.",
    ]);
    assert.equal(view.otherIssues, null);
    assert.deepEqual(view.notes, ["Validation reads known Android and Apple project files under the given root. Paths and locations come from the controller's report."]);
    assert.equal(view.inlineError, "");
    assert.equal(view.validateDisabled, false);

    if (!audited) {
      await auditDeepLinksDom(cdp);
      audited = true;
    }

    const narrow = await cdp.evaluate(`(() => {
      const form = document.getElementById("deep-links-root-form");
      const input = document.getElementById("deep-links-root");
      return { formWidth: Math.round(form.getBoundingClientRect().width), inputWidth: Math.round(input.getBoundingClientRect().width) };
    })()`);
    assert.ok(narrow.inputWidth > 180, `root input width ${narrow.inputWidth}`);
    await cdp.send("Emulation.setDeviceMetricsOverride", { width: 760, height: 520, deviceScaleFactor: 1, mobile: false });
    await new Promise(resolvePromise => setTimeout(resolvePromise, 80));
    const stacked = await cdp.evaluate(`(() => {
      const content = document.getElementById("deep-links-content");
      const input = document.getElementById("deep-links-root");
      return {
        overflow: getComputedStyle(content).overflowY,
        scroll: content.scrollHeight > content.clientHeight,
        inputWidth: Math.round(input.getBoundingClientRect().width),
      };
    })()`);
    assert.equal(stacked.overflow, "auto");
    assert.ok(stacked.inputWidth > 180, `narrow input width ${stacked.inputWidth}`);
    await cdp.send("Emulation.clearDeviceMetricsOverride");
    if (process.env.DEEP_LINKS_SHOT) {
      const shot = await cdp.send("Page.captureScreenshot", { format: "png" });
      await writeFile(process.env.DEEP_LINKS_SHOT, Buffer.from(shot.data, "base64"));
    }
  });
}

async function scenarioEmpty() {
  await runScenario("empty", undefined, async ({ cdp, fake }) => {
    await validateRoot(cdp, validRoot);
    await waitFor(cdp, "empty deep links result", `document.querySelector("#deep-links-content .dl-status strong")?.textContent === "No deep links configured"`);
    const view = await page(cdp);
    assert.equal(view.statusTitle, "No deep links configured");
    assert.match(view.statusText, /No Android intent filters or Apple URL types were reported/);
    assert.match(view.meta, /^No deep links configured · 0 files · 0 Android schemes · 0 Apple domains · 0 issues/);
    assert.equal(view.stats.androidSchemes.value, "0");
    assert.equal(view.stats.issues.value, "0");
    assert.equal(view.stateError, false);
    assert.equal(view.files.meta, "0 reported");
    assert.deepEqual(view.files.tables, []);
    assert.deepEqual(view.files.notes, ["No platform files were reported for this root."]);
    assert.deepEqual(view.android.tables, []);
    assert.equal(view.android.meta, "Not configured · 0 schemes · 0 hosts");
    assert.deepEqual(view.android.notes, ["No Android schemes reported.", "No Android hosts reported.", "No issues reported."]);
    assert.equal(view.apple.meta, "Not configured · 0 schemes · 0 domains");
    assert.deepEqual(view.apple.notes, ["No Apple URL schemes reported.", "No associated domains reported.", "No issues reported."]);
    assert.equal(view.otherIssues, null);
    assert.equal(deepLinksCalls(fake).length, 1);
    assert.deepEqual(deepLinksCalls(fake)[0].params, { root: validRoot });
  });
}

async function scenarioMalformedPayload() {
  await runScenario("malformed", undefined, async ({ cdp, fake }) => {
    await validateRoot(cdp, validRoot);
    await waitFor(cdp, "unrecognized state", `document.querySelector("#deep-links-content .state strong")?.textContent === "Unrecognized deep links response"`);
    const view = await page(cdp);
    assert.equal(view.stateTitle, "Unrecognized deep links response");
    assert.equal(view.stateError, true);
    assert.match(view.stateMessage, /no project files, Android or Apple configuration, issues or summary/);
    assert.match(view.meta, /Unrecognized deep links response/);
    assert.equal(view.files, null);
    assert.equal(view.android, null);
    assert.equal(deepLinksCalls(fake).length, 1);
  });
}

async function scenarioMalformedRoot() {
  await runScenario("valid", undefined, async ({ cdp, fake }) => {
    await setRoot(cdp, "relative/project");
    await click(cdp, "#deep-links-validate");
    await waitFor(cdp, "relative root rejected", `document.getElementById("deep-links-root-error")?.hidden === false`);
    let view = await page(cdp);
    assert.equal(view.rootError, "The project root must be an absolute path. Relative paths and traversal are not sent to the controller.");
    assert.equal(view.rootInvalid, "true");
    assert.equal(view.stateTitle, "Invalid project root");
    assert.match(view.stateMessage, /must be an absolute path/);
    assert.equal(view.meta, "Invalid project root.");
    assert.equal(deepLinksCalls(fake).length, 0);

    await setRoot(cdp, "/tmp/../etc");
    await click(cdp, "#deep-links-validate");
    await waitFor(cdp, "traversal rejected", `document.getElementById("deep-links-root-error")?.textContent.includes("..")`);
    view = await page(cdp);
    assert.equal(view.rootError, "The project root must not contain .. path segments.");
    assert.equal(view.stateTitle, "Invalid project root");
    assert.equal(deepLinksCalls(fake).length, 0);

    await validateRoot(cdp, validRoot);
    await waitFor(cdp, "valid root accepted", `document.querySelector("#deep-links-content .dl-scope code")?.textContent === ${JSON.stringify(validRoot)}`);
    view = await page(cdp);
    assert.equal(view.rootErrorHidden, true);
    assert.equal(view.rootInvalid, "false");
    assert.equal(deepLinksCalls(fake).length, 1);
  });
}

async function scenarioUnavailable() {
  await runScenario("unavailable", undefined, async ({ cdp, fake }) => {
    await validateRoot(cdp, validRoot);
    await waitFor(cdp, "unavailable state", `document.querySelector("#deep-links-content .state strong")?.textContent === "Deep Links unavailable"`);
    const view = await page(cdp);
    assert.equal(view.stateTitle, "Deep Links unavailable");
    assert.match(view.stateMessage, /not readable or is outside the allowed project/);
    assert.match(view.stateMessage, /unavailable/);
    assert.match(view.meta, /Deep Links unavailable/);
    assert.equal(view.files, null);
    assert.equal(view.validateDisabled, false);
    await click(cdp, "#deep-links-content .state .tool-button");
    await waitForDeepLinksCalls(fake, 2);
  });
}

async function scenarioError() {
  await runScenario("error", undefined, async ({ cdp, fake }) => {
    await validateRoot(cdp, validRoot);
    await waitFor(cdp, "error state", `document.querySelector("#deep-links-content .state strong")?.textContent === "Deep links validation failed"`);
    const view = await page(cdp);
    assert.equal(view.stateTitle, "Deep links validation failed");
    assert.equal(view.stateError, true);
    assert.match(view.stateMessage, /Android manifest parser failed/);
    assert.match(view.stateMessage, /internal/);
    assert.match(view.meta, /Validation failed/);
    assert.equal(view.validateDisabled, false);
    assert.equal(deepLinksCalls(fake).length, 1);
  });
}

async function scenarioUnknown() {
  await runScenario("unknown", undefined, async ({ cdp, fake }) => {
    await validateRoot(cdp, validRoot);
    await waitFor(cdp, "unsupported state", `document.querySelector("#deep-links-content .state strong")?.textContent === "Deep Links are not supported by this controller"`);
    const view = await page(cdp);
    assert.equal(view.stateTitle, "Deep Links are not supported by this controller");
    assert.match(view.stateMessage, /does not expose get_ui_deep_links/);
    assert.equal(view.validateDisabled, false);
    assert.equal(deepLinksCalls(fake).length, 1);
  });
}

async function scenarioNull() {
  await runScenario("null", undefined, async ({ cdp, fake }) => {
    await validateRoot(cdp, validRoot);
    await waitFor(cdp, "null result state", `document.querySelector("#deep-links-content .state strong")?.textContent === "No validation result"`);
    const view = await page(cdp);
    assert.equal(view.stateTitle, "No validation result");
    assert.match(view.stateMessage, /Nothing is substituted/);
    assert.match(view.meta, /No validation result returned/);
    assert.equal(view.scopeRoot, "");
    assert.equal(deepLinksCalls(fake).length, 1);
  });
}

async function scenarioPersistence() {
  await runScenario("valid", undefined, async ({ cdp, fake }) => {
    await validateRoot(cdp, validRoot);
    await waitFor(cdp, "first validation", `document.querySelector("#deep-links-content .dl-scope code")?.textContent === ${JSON.stringify(validRoot)}`);
    await validateRoot(cdp, secondRoot);
    await waitFor(cdp, "second validation", `document.querySelector("#deep-links-content .dl-scope code")?.textContent === ${JSON.stringify(secondRoot)}`);
    await waitForDeepLinksCalls(fake, 2);
    assert.equal(await cdp.evaluate(`localStorage.getItem("cove.devtools.deepLinksRoot")`), secondRoot);

    await cdp.send("Page.reload", { ignoreCache: true });
    await waitFor(cdp, "reconnected", `document.getElementById("connection-text")?.textContent === "Connected"`);
    await waitFor(cdp, "restored root", `document.getElementById("deep-links-root")?.value === ${JSON.stringify(secondRoot)}`);
    assert.equal(await cdp.evaluate(`sessionStorage.getItem("cove.devtools.token")`), "test-token");
    assert.equal(deepLinksCalls(fake).length, 2);
    const restored = await page(cdp);
    assert.equal(restored.rootValue, secondRoot);
    assert.equal(restored.stateTitle, "No project validated");

    await click(cdp, "#tab-deep-links");
    await waitFor(cdp, "deep links after reload", `document.getElementById("view-deep-links")?.hidden === false`);
    assert.equal(deepLinksCalls(fake).length, 2);
    await click(cdp, "#deep-links-validate");
    await waitForDeepLinksCalls(fake, 3);
    assert.deepEqual(deepLinksCalls(fake)[2].params, { root: secondRoot });
    await waitFor(cdp, "remembered root validated", `document.querySelector("#deep-links-content .dl-scope code")?.textContent === ${JSON.stringify(secondRoot)}`);
  });
}

async function main() {
  const nodeMajor = Number(process.versions.node.split(".")[0]);
  assert.ok(nodeMajor >= 22, `Node ${process.versions.node} has no global WebSocket/fetch`);
  try {
    await access(chromePath);
  } catch {
    console.error(`BLOCKED  Chrome not found at ${chromePath}. Set CHROME_PATH.`);
    process.exit(2);
  }

  console.log("Deep Links harness: fake controller + headless Chrome");
  await scenarioValid();
  console.log("PASS  declared Android/Apple links, paths, issue locations and summary render from the payload");
  await scenarioEmpty();
  console.log("PASS  no deep links configured keeps missing files muted as Not configured");
  await scenarioMalformedPayload();
  console.log("PASS  malformed payload reports an unrecognized response without links");
  await scenarioMalformedRoot();
  console.log("PASS  relative and traversal roots are rejected before any RPC");
  await scenarioUnavailable();
  console.log("PASS  unavailable error keeps its reason and retry calls again");
  await scenarioError();
  console.log("PASS  internal error reports the controller message");
  await scenarioUnknown();
  console.log("PASS  controller without get_ui_deep_links reports unsupported");
  await scenarioNull();
  console.log("PASS  null payload has an explicit no-result state");
  await scenarioPersistence();
  console.log("PASS  last validated root persists across reload and is revalidated only on request");
  console.log("RESULT PASS");
}

try {
  await main();
} catch (error) {
  console.error("RESULT FAIL");
  console.error(error?.stack || error);
  process.exitCode = 1;
}
