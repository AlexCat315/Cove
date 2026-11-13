#!/usr/bin/env node
// Fake-controller harness for the Inspector component tree.
// Two behaviours are asserted against the real served client:
//   1. a row keeps its label: deep rows must not squeeze the label to zero width;
//   2. a modifier chain reads as content: single-child wrapper nodes fold into the
//      node they wrap, and unfolding them makes the pane scroll sideways instead.
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import assert from "node:assert/strict";
import { serveClientAsset } from "./harness-server.mjs";

const chromePath = process.env.CHROME_PATH
  || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

// One wrapper per modifier in a real composition chain. The generated tail keeps
// the chain deep enough that its indent alone is wider than the pane, which is
// what the screenshot showed: past a certain depth the label has no room left.
const WRAPPERS = [
  "cove_ui_compose::modifiers::layout::FrameModifier<cove_ui_compose::modifiers::layout::Flex>",
  "cove_composition::composition_local_provider::ProvideCompositionLocalModifier<cove_composition::CompositionLocal>",
  "cove_ui_compose::modifiers::layout::PaddingModifier<cove_ui_compose::modifiers::layout::Inset>",
  "cove_ui_compose::modifiers::style::BackgroundModifier<cove_ui_compose::primitives::background::Fill>",
  "cove_ui_compose::modifiers::interaction::GestureModifier<cove_ui_compose::modifiers::interaction::Tap>",
  "cove_ui_compose::modifiers::layout::ClipModifier<cove_ui_compose::modifiers::layout::Rounded>",
  "cove_ui_compose::modifiers::animation::TransitionModifier<cove_ui_compose::modifiers::animation::Fade>",
  "cove_ui_compose::modifiers::layout::SafeAreaModifier<cove_ui_compose::modifiers::layout::Insets>",
  ...Array.from({ length: 32 }, (_, index) =>
    `cove_ui_compose::modifiers::layout::ChainModifier${index}<cove_ui_compose::modifiers::layout::Chain${index}>`),
];

function wrapperChain(prefix, leaf) {
  let node = leaf;
  for (let index = WRAPPERS.length - 1; index >= 0; index -= 1) {
    node = {
      id: `${prefix}-wrap-${index}`,
      viewTag: WRAPPERS[index],
      children: [node],
    };
  }
  return node;
}

function treePayload() {
  return {
    root: {
      id: "root",
      ownerScopeID: "root",
      debugName: "AppRoot",
      viewTag: "gallery::App",
      children: [
        wrapperChain("gallery", {
          id: "gallery",
          ownerScopeID: "gallery",
          debugName: "GalleryContent",
          viewTag: "gallery::GalleryContent",
          children: [
            { id: "leaf-a", viewTag: "gallery::Card", children: [] },
            { id: "leaf-b", viewTag: "gallery::StatusLine", children: [] },
          ],
        }),
        wrapperChain("footer", {
          id: "footer",
          ownerScopeID: "footer",
          debugName: "StatusBar",
          viewTag: "gallery::StatusBar",
          children: [],
        }),
      ],
    },
  };
}

function nodeRecord(id) {
  return {
    id,
    ownerScopeID: id,
    frame: { x: 0, y: 0, w: 100, h: 40 },
    absoluteFrame: { x: 0, y: 0, w: 100, h: 40 },
    layout: null,
    style: null,
    flags: { hitTestable: true, focusable: true, clipsToBounds: true, hasBackground: false, hasBorder: false },
    presentation: { isVisible: true, inputEnabled: true, visibleFrame: { x: 0, y: 0, w: 100, h: 40 } },
  };
}

function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

function createFake() {
  const calls = [];
  async function handle(method, params = {}) {
    calls.push({ method, params });
    if (method === "get_ui_session") {
      return {
        platform: "macos",
        permissions: { screen_capture: false, accessibility: false },
        targets: [{
          id: "runtime-1",
          title: "Tree Target",
          platform: "macos",
          pid: 4242,
          kind: "runtime",
          capabilities: ["tree"],
        }],
      };
    }
    if (method === "inspect_ui_tree") return treePayload();
    if (method === "query_ui_nodes") return { ok: true, data: { items: [{ node: nodeRecord(params.node_id) }] } };
    if (method === "select_ui_node") return { ok: true };
    return { ok: true };
  }
  return { calls, handle };
}

async function startServer() {
  const fake = createFake();
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
      const result = await fake.handle(message.method, message.params || {});
      response.writeHead(200, { "content-type": "application/json", "cache-control": "no-store" });
      response.end(JSON.stringify({ jsonrpc: "2.0", id: message.id, result }));
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
    `--window-size=${process.env.TREE_WINDOW || "480,820"}`,
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
        this.exceptions.push(message.params.exceptionDetails?.exception?.description || message.params.exceptionDetails?.text || "exception");
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
    await sleep(50);
  }
  throw new Error(`Timed out waiting for ${description}. Last value: ${JSON.stringify(last)}`);
}

const ROWS = `[...document.querySelectorAll("#tree .tree-row")].map(row => ({
  id: row.dataset.id,
  depth: Number(row.getAttribute("aria-level") || 0),
  label: row.querySelector(".tree-label")?.textContent || "",
  labelWidth: row.querySelector(".tree-label")?.getBoundingClientRect().width || 0,
  folded: row.querySelector(".tree-fold")?.textContent || "",
}))`;

async function treeState(cdp) {
  return cdp.evaluate(`(() => {
    const tree = document.getElementById("tree");
    return {
      rows: ${ROWS},
      scrollWidth: tree?.scrollWidth || 0,
      clientWidth: tree?.clientWidth || 0,
      meta: document.getElementById("tree-meta")?.textContent || "",
    };
  })()`);
}

// Folding off exposes every wrapper as its own row, but the tree only draws
// children of expanded nodes, so the harness opens the chain one level at a time
// from the deepest collapsed row until nothing is left to open.
async function expandEveryRow(cdp, expected) {
  const count = await cdp.evaluate(`(async () => {
    for (let step = 0; step < 400; step += 1) {
      const collapsed = [...document.querySelectorAll("#tree .tree-row")].reverse().find(row => {
        const twisty = row.querySelector(".tree-twisty");
        return twisty && !twisty.disabled && row.getAttribute("aria-expanded") !== "true";
      });
      if (!collapsed) break;
      collapsed.querySelector(".tree-twisty").click();
      await new Promise(resolve => setTimeout(resolve, 25));
    }
    return document.querySelectorAll("#tree .tree-row").length;
  })()`);
  assert.equal(count, expected, `expanded tree row count (${count} of ${expected})`);
}

async function main() {
  const profile = await mkdtemp(join(tmpdir(), "cove-tree-harness-"));
  const server = await startServer();
  const chrome = await startChrome(profile, server.url);
  const cdp = await connect(chrome.wsUrl);
  try {
    await waitFor(cdp, "controller connection", `document.getElementById("connection-text")?.textContent === "Connected"`);
    await waitFor(cdp, "folded tree", `document.querySelectorAll("#tree .tree-row").length > 0`);

    // 1. Folded: the chain of eight wrappers collapses into the node it wraps.
    const folded = await treeState(cdp);
    assert.equal(folded.rows.length, 3, `folded rows: ${JSON.stringify(folded.rows)}`);
    const galleryRow = folded.rows.find(row => row.id === "gallery");
    assert.ok(galleryRow, "the wrapped node keeps its own id");
    assert.equal(galleryRow.label, "GalleryContent", "the wrapped node keeps its authored name");
    assert.equal(galleryRow.folded, `+${WRAPPERS.length}`, "the row reports how many wrappers it folded");
    assert.equal(galleryRow.depth, 2, "folding removes the chain depth, not just the rows");
    const statusRow = folded.rows.find(row => row.id === "footer");
    assert.equal(statusRow.folded, `+${WRAPPERS.length}`, "a chain ending in a leaf folds too");

    // 2. Every row keeps its label readable instead of collapsing to an ellipsis.
    for (const row of folded.rows) {
      assert.ok(row.labelWidth > 0, `row ${row.id} keeps a visible label (was ${row.labelWidth}px)`);
    }

    // 3. Unfolded: the chains come back and the pane scrolls sideways.
    await cdp.evaluate(`(() => {
      const input = document.getElementById("filter-chains");
      input.checked = false;
      input.dispatchEvent(new Event("change", { bubbles: true }));
      return true;
    })()`);
    // root + (chain → GalleryContent + its two leaves) + (chain → StatusBar)
    const unfoldedRows = 1 + (WRAPPERS.length + 3) + (WRAPPERS.length + 1);
    await expandEveryRow(cdp, unfoldedRows);
    const unfolded = await treeState(cdp);
    assert.equal(unfolded.rows.length, unfoldedRows, "every wrapper is its own row again");
    assert.ok(unfolded.rows.every(row => row.folded === ""), "unfolded rows carry no fold chip");
    assert.ok(
      unfolded.scrollWidth > unfolded.clientWidth,
      `the tree pane must scroll horizontally when a row is wider than it (${unfolded.scrollWidth}/${unfolded.clientWidth}; widths ${JSON.stringify(unfolded.rows.map(row => [row.id, row.depth, row.labelWidth]))})`,
    );
    for (const row of unfolded.rows) {
      assert.ok(row.labelWidth > 0, `unfolded row ${row.id} keeps a visible label`);
    }

    // 4. Folding back restores the compact view.
    await cdp.evaluate(`(() => {
      const input = document.getElementById("filter-chains");
      input.checked = true;
      input.dispatchEvent(new Event("change", { bubbles: true }));
      return true;
    })()`);
    // Refolding keeps the expansion this harness opened: GalleryContent still
    // shows its two leaves, so the compact view is root + chain tails + leaves.
    await waitFor(cdp, "refolded tree", `document.querySelectorAll("#tree .tree-row").length === 5`);

    assert.deepEqual(cdp.exceptions, [], "the page raised no exception");
    console.log(`tree harness: ${folded.rows.length} folded rows over ${folded.rows.filter(row => row.folded).length} chains; `
      + `${unfolded.rows.length} unfolded rows scroll ${unfolded.scrollWidth}px inside ${unfolded.clientWidth}px; `
      + "every label kept its width.");
  } finally {
    cdp.close();
    await chrome.close();
    await server.close();
    await rm(profile, { recursive: true, force: true });
  }
}

main().catch(async error => {
  console.error(error);
  process.exitCode = 1;
});
