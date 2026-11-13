#!/usr/bin/env node
// Fake-controller harness for the capture pane.
// It serves a 2560×1440 frame, so the pane has to work with a capture far wider
// than itself: the frame must fill the pane instead of a fixed height cap, the
// zoom steps must change the drawn size, dragging must pan it, and picking must
// still map a screen point back to the right image pixel at every zoom.
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import assert from "node:assert/strict";
import { serveClientAsset } from "./harness-server.mjs";

const chromePath = process.env.CHROME_PATH
  || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

const FRAME_WIDTH = 2560;
const FRAME_HEIGHT = 1440;

function frameSvg() {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${FRAME_WIDTH}" height="${FRAME_HEIGHT}" viewBox="0 0 ${FRAME_WIDTH} ${FRAME_HEIGHT}">
  <rect width="100%" height="100%" fill="#101014"/>
  <rect x="40" y="40" width="600" height="200" fill="#2a2a33"/>
  <rect x="700" y="40" width="300" height="200" fill="#3a3a46"/>
</svg>`;
}

function treePayload() {
  return {
    root: {
      id: "root",
      ownerScopeID: "root",
      debugName: "AppRoot",
      viewTag: "gallery::App",
      children: [
        { id: "hit-node", ownerScopeID: "hit-node", debugName: "HitTarget", viewTag: "gallery::Card", children: [] },
      ],
    },
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
          title: "Capture Target",
          platform: "macos",
          pid: 4242,
          kind: "runtime",
          capabilities: ["tree", "capture", "inspect_nodes", "input"],
        }],
      };
    }
    if (method === "inspect_ui_tree") return treePayload();
    if (method === "query_ui_nodes") {
      return { ok: true, data: { items: [{ node: { id: params.node_id, ownerScopeID: params.node_id } }] } };
    }
    if (method === "select_ui_node") return { ok: true };
    if (method === "capture_ui") {
      return {
        capture_id: "cap-1",
        width: FRAME_WIDTH,
        height: FRAME_HEIGHT,
        source: "window",
        resource: "/capture/cap-1.svg",
      };
    }
    if (method === "select_ui_at") return { capture_id: "cap-1", hit: { nodeID: "hit-node", path: ["root", "hit-node"] } };
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
    if (request.method === "GET" && url.pathname === "/capture/cap-1.svg") {
      response.writeHead(200, { "content-type": "image/svg+xml", "cache-control": "no-store" });
      response.end(frameSvg());
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
    `--window-size=${process.env.CAPTURE_WINDOW || "1280,900"}`,
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

  async mouse(type, x, y, extra = {}) {
    await this.send("Input.dispatchMouseEvent", { type, x, y, button: "left", clickCount: 1, ...extra });
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

async function click(cdp, selector) {
  const clicked = await cdp.evaluate(`(() => {
    const node = document.querySelector(${JSON.stringify(selector)});
    if (!node) return false;
    node.click();
    return true;
  })()`);
  assert.equal(clicked, true, `click ${selector}`);
}

function paneState() {
  return `(() => {
    const body = document.getElementById("screenshot-body");
    const image = body?.querySelector("img");
    const rect = image?.getBoundingClientRect() || null;
    return {
      viewportHeight: window.innerHeight,
      paneHeight: body?.getBoundingClientRect().height || 0,
      imageWidth: rect?.width || 0,
      imageHeight: rect?.height || 0,
      imageLeft: rect?.left || 0,
      imageTop: rect?.top || 0,
      styleWidth: image?.style.width || "",
      scrollWidth: body?.scrollWidth || 0,
      scrollLeft: body?.scrollLeft || 0,
      clientWidth: body?.clientWidth || 0,
      meta: document.getElementById("screenshot-meta")?.textContent || "",
      zoomLabel: document.getElementById("screenshot-zoom")?.textContent || "",
    };
  })()`;
}

async function main() {
  const profile = await mkdtemp(join(tmpdir(), "cove-capture-harness-"));
  const server = await startServer();
  const chrome = await startChrome(profile, server.url);
  const cdp = await connect(chrome.wsUrl);
  try {
    await waitFor(cdp, "controller connection", `document.getElementById("connection-text")?.textContent === "Connected"`);
    await waitFor(cdp, "tree", `document.querySelectorAll("#tree .tree-row").length > 0`);

    await click(cdp, "#capture-button");
    await waitFor(cdp, "capture image", `document.getElementById("screenshot-body")?.querySelector("img") !== null`);
    await sleep(200);

    // 1. Fit fills the pane: the old fixed 34vh cap left the frame smaller than that.
    const fit = await cdp.evaluate(paneState());
    assert.equal(fit.zoomLabel, "100%", "a fresh capture offers the 1:1 step");
    assert.ok(
      fit.imageHeight > fit.viewportHeight * 0.34,
      `fit must use more than the old 34vh cap (${fit.imageHeight} vs ${fit.viewportHeight * 0.34})`,
    );
    assert.ok(fit.meta.includes("%"), `the meta reports the drawn scale: ${fit.meta}`);
    const fitScale = Math.round((fit.imageWidth / FRAME_WIDTH) * 100);
    assert.ok(fit.meta.includes(`${fitScale}%`), `meta ${fit.meta} reports the real scale ${fitScale}%`);

    // 1a. The splitter gives the frame whatever share the reader drags to, and
    //     that share is remembered across reloads.
    await cdp.evaluate(`localStorage.removeItem("cove.devtools.frameShare")`);
    const splitRect = await cdp.evaluate(`document.getElementById("details-split").getBoundingClientRect().toJSON()`);
    assert.ok(splitRect.height > 0 && !splitRect.hidden, "the splitter is visible under the frame");
    const splitX = Math.round(splitRect.left + splitRect.width / 2);
    const splitY = Math.round(splitRect.top + splitRect.height / 2);
    await cdp.mouse("mousePressed", splitX, splitY);
    await cdp.mouse("mouseMoved", splitX, splitY - 120);
    await cdp.mouse("mouseReleased", splitX, splitY - 120);
    const shrunk = await cdp.evaluate(paneState());
    assert.ok(
      Math.abs((fit.paneHeight - shrunk.paneHeight) - 120) <= 3,
      `dragging the splitter 120px up shrank the frame by that much (${Math.round(fit.paneHeight - shrunk.paneHeight)})`,
    );
    const storedShare = Number(await cdp.evaluate(`localStorage.getItem("cove.devtools.frameShare")`));
    assert.ok(storedShare > 0 && storedShare < 1, `the dragged share is stored (${storedShare})`);
    // A reload reads it back instead of the built-in default.
    await cdp.send("Page.reload");
    await waitFor(cdp, "reloaded tree", `document.querySelectorAll("#tree .tree-row").length > 0`);
    for (let attempt = 0; attempt < 6; attempt++) {
      if (await cdp.evaluate(`document.getElementById("screenshot-body")?.querySelector("img") !== null`)) break;
      await click(cdp, "#capture-button");
      await sleep(300);
    }
    await waitFor(cdp, "capture after reload", `document.getElementById("screenshot-body")?.querySelector("img") !== null`);
    await sleep(200);
    const restored = await cdp.evaluate(paneState());
    assert.ok(
      Math.abs(restored.paneHeight - shrunk.paneHeight) <= 3,
      `the stored share survives a reload (${Math.round(restored.paneHeight)} vs ${Math.round(shrunk.paneHeight)})`,
    );
    // Double-click returns to the default divide.
    await cdp.evaluate(`(() => {
      const handle = document.getElementById("details-split");
      handle.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
      return true;
    })()`);
    await sleep(100);
    const reset = await cdp.evaluate(paneState());
    assert.ok(reset.paneHeight > shrunk.paneHeight, `double-click restores a taller frame (${Math.round(reset.paneHeight)})`);
    // Keyboard: focus the handle and shrink it with ArrowDown.
    await cdp.evaluate(`document.getElementById("details-split").focus()`);
    await cdp.send("Input.dispatchKeyEvent", { type: "keyDown", key: "ArrowDown", code: "ArrowDown", windowsVirtualKeyCode: 40 });
    await cdp.send("Input.dispatchKeyEvent", { type: "keyUp", key: "ArrowDown", code: "ArrowDown", windowsVirtualKeyCode: 40 });
    await sleep(100);
    const keyed = await cdp.evaluate(paneState());
    assert.ok(Math.abs(keyed.paneHeight - reset.paneHeight - 24) <= 3, `ArrowDown grows the frame by a step (${Math.round(keyed.paneHeight - reset.paneHeight)})`);
    // Back to the default share for the rest of the scenario.
    await cdp.evaluate(`document.getElementById("details-split").dispatchEvent(new MouseEvent("dblclick", { bubbles: true }))`);
    await sleep(100);

    // 1b. Expanding hands the whole details column to the frame, which is the
    //     one-click answer to "the frame is too small to pick in".
    const column = await cdp.evaluate(`document.querySelector(".details-pane").getBoundingClientRect().height`);
    await click(cdp, "#screenshot-expand");
    await waitFor(cdp, "expanded frame", `document.querySelector(".screenshot-panel").classList.contains("is-expanded")`);
    const expanded = await cdp.evaluate(paneState());
    assert.equal(expanded.zoomLabel, "100%", "expanding keeps the zoom step");
    assert.ok(
      expanded.paneHeight > fit.paneHeight * 1.3,
      `the expanded frame is far taller than the docked one (${Math.round(expanded.paneHeight)} vs ${Math.round(fit.paneHeight)})`,
    );
    assert.ok(
      expanded.paneHeight >= column * 0.85,
      `the expanded frame fills the details column (${Math.round(expanded.paneHeight)} of ${Math.round(column)})`,
    );
    assert.equal(
      await cdp.evaluate(`document.getElementById("screenshot-expand").textContent`),
      "Collapse",
      "the toggle offers the way back",
    );
    await click(cdp, "#screenshot-expand");
    await waitFor(cdp, "collapsed frame", `!document.querySelector(".screenshot-panel").classList.contains("is-expanded")`);
    const collapsed = await cdp.evaluate(paneState());
    assert.ok(
      Math.abs(collapsed.paneHeight - fit.paneHeight) <= 2,
      `collapsing restores the docked height (${Math.round(collapsed.paneHeight)} vs ${Math.round(fit.paneHeight)})`,
    );

    // 2. 1:1 draws one image pixel per screen pixel and lets the pane scroll.
    await click(cdp, "#screenshot-zoom");
    await waitFor(cdp, "1:1 zoom", `document.getElementById("screenshot-body")?.querySelector("img")?.style.width === "${FRAME_WIDTH}px"`);
    const actual = await cdp.evaluate(paneState());
    assert.equal(actual.zoomLabel, "200%", "1:1 offers the doubled step next");
    assert.ok(actual.meta.includes("100%"), `meta reports 1:1: ${actual.meta}`);
    assert.ok(
      actual.scrollWidth > actual.clientWidth,
      `a 1:1 frame scrolls inside the pane (${actual.scrollWidth}/${actual.clientWidth})`,
    );

    // 3. Dragging pans a zoomed frame instead of hunting for a scrollbar.
    const paneRect = await cdp.evaluate(`document.getElementById("screenshot-body").getBoundingClientRect().toJSON()`);
    const centreX = Math.round(paneRect.left + paneRect.width / 2);
    const centreY = Math.round(paneRect.top + 60);
    await cdp.mouse("mousePressed", centreX, centreY);
    await cdp.mouse("mouseMoved", centreX - 220, centreY);
    await cdp.mouse("mouseReleased", centreX - 220, centreY);
    await waitFor(cdp, "panned frame", `document.getElementById("screenshot-body").scrollLeft > 0`);
    const panned = await cdp.evaluate(paneState());
    assert.ok(panned.scrollLeft > 0, `dragging moved the frame (${panned.scrollLeft})`);

    // 4. 2× keeps the mapping: a click lands on the image pixel under the cursor.
    await click(cdp, "#screenshot-zoom");
    await waitFor(cdp, "2x zoom", `document.getElementById("screenshot-body")?.querySelector("img")?.style.width === "${FRAME_WIDTH * 2}px"`);
    await cdp.evaluate(`(() => { document.getElementById("screenshot-body").scrollLeft = 0; return true; })()`);
    await click(cdp, "#mode-pick");
    await waitFor(cdp, "pick mode", `document.getElementById("screenshot-body").classList.contains("is-pick")`);
    const doubled = await cdp.evaluate(paneState());
    const clickX = Math.round(doubled.imageLeft + 100);
    const clickY = Math.round(doubled.imageTop + 50);
    assert.ok(clickX >= 0 && clickY >= 0, `a zoomed frame is anchored where the pane can reach it (${clickX},${clickY})`);
    await cdp.mouse("mousePressed", clickX, clickY);
    await cdp.mouse("mouseReleased", clickX, clickY);
    await waitFor(cdp, "pick call", `document.getElementById("node-id")?.textContent === "hit-node"`);
    const picks = server.fake.calls.filter(call => call.method === "select_ui_at");
    assert.equal(picks.length, 1, "one pick reached the controller");
    // The pane sits at a fractional offset, so a one-pixel difference is layout
    // rounding, not a mapping error.
    // At 2× a screen offset of one pixel is half an image pixel, so the image
    // coordinates are simply the offsets divided by the scale.
    const expectedX = 100 / 2;
    const expectedY = 50 / 2;
    assert.ok(
      Math.abs(picks[0].params.x - expectedX) <= 1 && Math.abs(picks[0].params.y - expectedY) <= 1,
      `a doubled frame maps screen offsets back to image pixels: got ${picks[0].params.x},${picks[0].params.y}, expected about ${Math.round(expectedX)},${Math.round(expectedY)}`,
    );

    // 5. Back to fit restores the pane-sized frame.
    await cdp.evaluate(`(() => { document.getElementById("screenshot-body").scrollLeft = 0; return true; })()`);
    await click(cdp, "#screenshot-zoom");
    await waitFor(cdp, "fit again", `document.getElementById("screenshot-body")?.querySelector("img")?.style.width === ""`);
    const refit = await cdp.evaluate(paneState());
    assert.equal(refit.zoomLabel, "100%", "fit cycles back to the 1:1 step");
    assert.ok(refit.scrollWidth <= refit.clientWidth + 1, `a fitted frame needs no horizontal scroll (${refit.scrollWidth}/${refit.clientWidth})`);

    assert.deepEqual(cdp.exceptions, [], "the page raised no exception");
    console.log(`capture harness: splitter drag ${Math.round(fit.paneHeight - shrunk.paneHeight)}px (share ${storedShare.toFixed(3)}, restored after reload), `
      + `expanded ${Math.round(expanded.paneHeight)}px of a ${Math.round(column)}px column (docked ${Math.round(fit.paneHeight)}), `
      + `fit ${fitScale}% (${Math.round(fit.imageWidth)}×${Math.round(fit.imageHeight)} in a `
      + `${Math.round(fit.paneHeight)}px pane), 1:1 scrolls ${actual.scrollWidth}/${actual.clientWidth}, drag panned ${panned.scrollLeft}px, `
      + `2× pick mapped to ${picks[0].params.x},${picks[0].params.y}.`);
  } finally {
    cdp.close();
    await chrome.close();
    await server.close();
    await rm(profile, { recursive: true, force: true });
  }
}

main().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
