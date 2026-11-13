import { WASI, File, OpenFile, ConsoleStdout } from "./wasi/index.js";
import { createWebGpuBridge } from "./webgpu.js";

const surface = document.querySelector("#surface");
const textInput = document.createElement("textarea");
textInput.id = "textInput"; textInput.setAttribute("aria-label", "Cove text input");
textInput.style.cssText = "position:fixed;left:0;top:0;width:1px;height:1px;padding:0;border:0;font-size:16px;opacity:0;pointer-events:none";
textInput.tabIndex = -1; document.body.append(textInput);
const status = document.querySelector("#status");
const decoder = new TextDecoder();
const encoder = new TextEncoder();
let wasm, renderer, lastFrame, scheduled = false;
const webgpu = createWebGpuBridge(() => wasm, document.querySelector("#gpu"));
const forcedRenderer = new URLSearchParams(location.search).get("renderer");
const atlas = { width: 0, height: 0, pixels: null, revision: 0, update: null };

function updateAtlas(frame) {
  if (atlas.width !== frame.atlasWidth || atlas.height !== frame.atlasHeight) {
    atlas.width = frame.atlasWidth; atlas.height = frame.atlasHeight;
    atlas.pixels = new Uint8Array(atlas.width * atlas.height);
  }
  atlas.update = frame.atlasUpdate;
  if (!atlas.update) return;
  const [x, y, width, height] = atlas.update;
  const pixels = readBytes(wasm.cove_atlas(), wasm.cove_atlas_size());
  if (pixels.length !== width * height) throw new Error("Invalid font atlas update");
  for (let row = 0; row < height; row++) {
    atlas.pixels.set(pixels.subarray(row * width, (row + 1) * width), (y + row) * atlas.width + x);
  }
  atlas.revision++;
}

function readBytes(pointer, size) {
  // Copy before another Wasm call can grow memory or replace an export buffer.
  return new Uint8Array(wasm.memory.buffer, pointer >>> 0, size).slice();
}
function readJSON(pointer, size) { return JSON.parse(decoder.decode(readBytes(pointer, size))); }
function response() { return readJSON(wasm.cove_response(), wasm.cove_response_size()); }
function dispatch(envelope) {
  const bytes = encoder.encode(JSON.stringify(envelope));
  const pointer = wasm.cove_alloc(bytes.length);
  if (!pointer) throw new Error("DevTools message exceeds the 1 MiB limit");
  try {
    new Uint8Array(wasm.memory.buffer, pointer >>> 0, bytes.length).set(bytes);
    wasm.cove_dispatch(pointer, bytes.length);
    return response();
  } finally { wasm.cove_free(pointer, bytes.length); }
}


function syncTextInput() {
  const active = document.activeElement;
  if (active !== surface && active !== textInput) return;
  const pointer = wasm.cove_text_input_area();
  if (!pointer) {
    if (active === textInput) surface.focus({preventScroll:true});
    return;
  }
  const [x, y, height] = new Float32Array(wasm.memory.buffer, pointer >>> 0, 3);
  const rect = surface.getBoundingClientRect();
  const clamp = (value, max) => Math.max(0, Math.min(value, max));
  textInput.style.left = `${rect.left + surface.clientLeft + clamp(x, surface.clientWidth - 1)}px`;
  textInput.style.top = `${rect.top + surface.clientTop + clamp(y, surface.clientHeight - height)}px`;
  textInput.style.height = `${Math.max(1, height)}px`;
  if (active === surface) textInput.focus({preventScroll:true});
}

function schedule() {
  if (scheduled) return;
  scheduled = true;
  requestAnimationFrame(() => {
    scheduled = false;
    const result = renderFrame();
    if (result === 1 || (result === 0 && wasm.cove_needs_frame())) schedule();
  });
}

function renderFrame({ syncNativeFocus = true } = {}) {
  const width = surface.clientWidth, height = surface.clientHeight;
  const rect = surface.getBoundingClientRect();
  wasm.cove_set_content_origin(rect.left + surface.clientLeft, rect.top + surface.clientTop);
  wasm.cove_set_scale(window.devicePixelRatio || 1);
  const outcome = wasm.cove_render(width, height);
  if (outcome === 1) return 1;
  if (outcome < 0) {
    if (forcedRenderer === "webgpu") {
      status.textContent = "WebGPU RHI frame failed";
      return -1;
    }
    wasm.cove_webgpu_disable();
    renderer = makeCanvasRenderer();
    wasm.cove_render(width, height);
  }
  lastFrame = readJSON(wasm.cove_snapshot(), wasm.cove_snapshot_size());
  updateAtlas(lastFrame);
  if (renderer.backend === "canvas2d") {
    const vertices = readBytes(wasm.cove_vertices(), wasm.cove_vertex_bytes());
    const indices = new Uint32Array(readBytes(wasm.cove_indices(), wasm.cove_index_count() * 4).buffer);
    renderer.draw(lastFrame, vertices, indices);
  }
  if (syncNativeFocus) syncTextInput();
  document.querySelector("#stats").textContent = `count=${lastFrame.count} · ${lastFrame.vertices} vertices · ${lastFrame.batches.length} batches · ${lastFrame.fontCount} fonts · FreeType/HarfBuzz`;
  return 0;
}

async function makeRenderer() {
  if (forcedRenderer === "canvas2d") return makeCanvasRenderer();
  try {
    const format = await webgpu.initialize();
    if (!wasm.cove_webgpu_enable(format === "bgra8unorm")) throw new Error("WebGPU RHI initialization failed");
    status.textContent = "Rust Wasm · WebGPU RHI";
    return { backend: "webgpu", adapter: webgpu.adapter, get lost() { return webgpu.lost; } };
  } catch (error) {
    if (forcedRenderer === "webgpu") throw error;
    console.info("Using Canvas2D geometry adapter:", error.message);
    return makeCanvasRenderer();
  }
}

function makeCanvasRenderer() {
  document.querySelector("#gpu").hidden = true;
  const canvas = document.querySelector("#fallback"); canvas.hidden = false;
  status.textContent = "Rust Wasm · Canvas2D fallback";
  const alphaCanvas = document.createElement("canvas");
  const tinted = new Map();
  const rasterAssets = new Map();
  let atlasRevision = -1;

  function updateRasterAssets(descriptors) {
    const visible = new Set(descriptors.map(asset => asset.id));
    for (const id of rasterAssets.keys()) if (!visible.has(id)) rasterAssets.delete(id);
    for (const { id, width, height } of descriptors) {
      if (rasterAssets.has(id)) continue;
      const pointer = wasm.cove_raster_asset(id);
      if (!pointer) continue;
      const source = document.createElement("canvas");
      source.width = width; source.height = height;
      const rgba = new Uint8ClampedArray(readBytes(pointer, width * height * 4).buffer);
      source.getContext("2d").putImageData(new ImageData(rgba, width, height), 0, 0);
      rasterAssets.set(id, source);
    }
  }

  function drawImageBatch(context, source, batch, data, indices) {
    let path, transform, alpha;
    function flush() {
      if (!path) return;
      context.save();
      context.clip(path);
      context.transform(...transform);
      context.globalAlpha = alpha;
      context.drawImage(source, 0, 0);
      context.restore();
    }
    function vertex(index) {
      const offset = indices[index] * 20;
      const u = data.getFloat32(offset + 8, true);
      return [data.getFloat32(offset, true), data.getFloat32(offset + 4, true),
        (u - (u >= 20 ? 20 : 10)) * source.width, data.getFloat32(offset + 12, true) * source.height,
        (data.getUint32(offset + 16, true) >>> 24) / 255];
    }
    for (let i = batch.offset; i < batch.offset + batch.count; i += 3) {
      const [x, y, u, v, opacity] = vertex(i);
      const [x1, y1, u1, v1] = vertex(i + 1);
      const [x2, y2, u2, v2] = vertex(i + 2);
      const [du1, dv1, du2, dv2] = [u1 - u, v1 - v, u2 - u, v2 - v];
      const det = du1 * dv2 - du2 * dv1;
      if (Math.abs(det) < 1e-7) continue;
      const a = ((x1 - x) * dv2 - (x2 - x) * dv1) / det;
      const b = ((y1 - y) * dv2 - (y2 - y) * dv1) / det;
      const c = ((x2 - x) * du1 - (x1 - x) * du2) / det;
      const d = ((y2 - y) * du1 - (y1 - y) * du2) / det;
      const matrix = [a, b, c, d, x - a * u - c * v, y - b * u - d * v];
      // Clip adjacent triangles as one image so their shared edges have no seam.
      if (!transform || opacity !== alpha || matrix.some((value, j) => Math.abs(value - transform[j]) > 0.002)) {
        flush(); path = new Path2D(); transform = matrix; alpha = opacity;
      }
      path.moveTo(x, y); path.lineTo(x1, y1); path.lineTo(x2, y2); path.closePath();
    }
    flush();
  }
    function fontSource(color) {
      if (!tinted.has(color)) {
        // Bound the cache; these are alpha textures tinted by vertex RGBA.
        if (tinted.size >= 8) tinted.delete(tinted.keys().next().value);
        const source = document.createElement("canvas");
        source.width = atlas.width; source.height = atlas.height;
        const context = source.getContext("2d");
        context.fillStyle = `rgb(${color & 255},${(color >>> 8) & 255},${(color >>> 16) & 255})`;
        context.fillRect(0, 0, source.width, source.height);
        context.globalCompositeOperation = "destination-in";
        context.drawImage(alphaCanvas, 0, 0);
        tinted.set(color, source);
      }
      return tinted.get(color);
    }
    return { backend: "canvas2d", draw(frame, vertices, indices) {
      updateRasterAssets(frame.rasterAssets);
      if (atlasRevision !== atlas.revision) {
        const update = atlasRevision < 0 ? [0, 0, atlas.width, atlas.height] : atlas.update;
        if (atlasRevision < 0) { alphaCanvas.width = atlas.width; alphaCanvas.height = atlas.height; }
        if (update) {
          const [x, y, width, height] = update;
          const data = new ImageData(width, height);
          for (let row = 0; row < height; row++) for (let col = 0; col < width; col++) {
            const offset = (row * width + col) * 4;
            data.data[offset] = data.data[offset + 1] = data.data[offset + 2] = 255;
            data.data[offset + 3] = Math.round(255 * Math.pow(atlas.pixels[(y + row) * atlas.width + x + col] / 255, 0.75));
          }
          alphaCanvas.getContext("2d").putImageData(data, x, y);
        }
        tinted.clear(); atlasRevision = atlas.revision;
      }
      const ratio = window.devicePixelRatio || 1;
      canvas.width = Math.round(surface.clientWidth * ratio); canvas.height = Math.round(surface.clientHeight * ratio);
      const context = canvas.getContext("2d"); context.setTransform(ratio, 0, 0, ratio, 0, 0);
      const data = new DataView(vertices.buffer);
      for (const batch of frame.batches) {
        context.save();
        if (batch.clip) { context.beginPath(); context.rect(...batch.clip); context.clip(); }
        if (batch.textureID === 1) {
          // DrawList.addGlyphQuad emits two triangles per rectangular glyph.
          // Consume their positions and atlas UVs without invoking Canvas fonts.
          for (let i = batch.offset; i < batch.offset + batch.count; i += 6) {
            const a = indices[i] * 20, b = indices[i + 2] * 20;
            const x = data.getFloat32(a, true), y = data.getFloat32(a + 4, true);
            const width = data.getFloat32(b, true) - x, height = data.getFloat32(b + 4, true) - y;
            const u = data.getFloat32(a + 8, true) * atlas.width, v = data.getFloat32(a + 12, true) * atlas.height;
            const sourceWidth = data.getFloat32(b + 8, true) * atlas.width - u;
            const sourceHeight = data.getFloat32(b + 12, true) * atlas.height - v;
            const color = data.getUint32(a + 16, true);
            context.globalAlpha = (color >>> 24) / 255;
            context.drawImage(fontSource(color), u, v, sourceWidth, sourceHeight, x, y, width, height);
          }
          context.restore(); continue;
        }
        const image = rasterAssets.get(batch.textureID);
        if (image) {
          drawImageBatch(context, image, batch, data, indices);
          context.restore(); continue;
        }
        let pathColor;
        for (let i = batch.offset; i < batch.offset + batch.count; i += 3) {
          const start = indices[i] * 20;
          const color = data.getUint32(start + 16, true);
          // Merge opaque triangles so shared internal edges have no AA seams.
          // Translucent triangles keep their original source-over ordering.
          if (pathColor !== color || (color >>> 24) !== 255) {
            if (pathColor !== undefined) context.fill();
            context.beginPath();
            context.fillStyle = `rgba(${color & 255},${(color >>> 8) & 255},${(color >>> 16) & 255},${(color >>> 24) / 255})`;
            pathColor = color;
          }
          for (let j = 0; j < 3; j++) {
            const p = indices[i + j] * 20, x = data.getFloat32(p, true), y = data.getFloat32(p + 4, true);
            if (j === 0) context.moveTo(x, y); else context.lineTo(x, y);
          }
          context.closePath();
        }
        if (pathColor !== undefined) context.fill();
        context.restore();
      }
    } };
}

try {
  const component = new URLSearchParams(location.search).get("component");
  const args = component ? ["gallery", "--component", component] : ["counter"];
  const wasi = new WASI(args, [], [new OpenFile(new File([])), ConsoleStdout.lineBuffered(console.log), ConsoleStdout.lineBuffered(console.error)]);
  const module = await WebAssembly.compile(await (await fetch("./cove.wasm")).arrayBuffer());
  const instance = await WebAssembly.instantiate(module, {
    wasi_snapshot_preview1: wasi.wasiImport,
    cove_webgpu: webgpu.imports,
  });
  wasm = instance.exports;
  wasi.initialize(instance);
  for (const font of ["NotoSans.ttf", "NotoSansCJKsc.otf", "NotoSansArabic.ttf", "NotoSansDevanagari.ttf", "NotoEmoji.ttf"]) {
    const result = await fetch(`./fonts/${font}`);
    if (!result.ok) throw new Error(`Cannot load font ${font}: HTTP ${result.status}`);
    const bytes = new Uint8Array(await result.arrayBuffer());
    const pointer = wasm.cove_font_alloc(bytes.length);
    if (!pointer) throw new Error(`Font exceeds the 32 MiB limit: ${font}`);
    try {
      new Uint8Array(wasm.memory.buffer, pointer >>> 0, bytes.length).set(bytes);
      if (!wasm.cove_font_load(pointer, bytes.length)) throw new Error(`Invalid font: ${font}`);
    } finally { wasm.cove_free(pointer, bytes.length); }
  }
  renderer = await makeRenderer();
  if (component) document.querySelector(".keyboard").hidden = true;
  else ["increment", "reset", "theme"].forEach((id, action) => {
    document.getElementById(id).addEventListener("click", () => {
      wasm.cove_counter_action(action);
      schedule();
    });
  });
  surface.tabIndex = 0;
  let composing = false, skipCompositionInput = false;
  const point = event => {
    const rect = surface.getBoundingClientRect();
    return [event.clientX - rect.left - surface.clientLeft, event.clientY - rect.top - surface.clientTop];
  };
  surface.addEventListener("wheel", event => {
    if (event.ctrlKey) return; // Browser pinch-to-zoom uses Ctrl+wheel.
    // Read the mode first: some browsers choose delta units based on this access.
    const mode = event.deltaMode;
    wasm.cove_wheel(mode, event.deltaX, event.deltaY, ...point(event), event.timeStamp);
    event.preventDefault();
    schedule();
  }, {passive:false});
  surface.addEventListener("pointermove", event => { wasm.cove_pointer(0, ...point(event)); schedule(); });
  surface.addEventListener("pointerleave", () => { wasm.cove_pointer(0, -1, -1); schedule(); });
  surface.addEventListener("pointerdown", event => {
    if (event.button !== 0) return;
    event.preventDefault();
    // End any live preedit while its original Cove editor still has focus.
    if (document.activeElement !== textInput || composing) surface.focus({preventScroll:true});
    surface.setPointerCapture(event.pointerId);
    wasm.cove_pointer(1, ...point(event));
    // Keep the focus change inside the user gesture for mobile keyboards.
    syncTextInput();
    schedule();
  });
  surface.addEventListener("pointerup", event => {
    if (event.button !== 0) return;
    wasm.cove_pointer(2, ...point(event));
    if (surface.hasPointerCapture(event.pointerId)) surface.releasePointerCapture(event.pointerId);
    syncTextInput();
    schedule();
  });
  surface.addEventListener("pointercancel", () => { wasm.cove_pointer(2, -1, -1); schedule(); });
  function sendText(string, editing = false) {
    const bytes = encoder.encode(string);
    const pointer = wasm.cove_alloc(Math.max(1, bytes.length));
    if (!pointer) return;
    try {
      new Uint8Array(wasm.memory.buffer, pointer >>> 0, bytes.length).set(bytes);
      wasm.cove_text(pointer, bytes.length, editing ? 1 : 0); schedule();
    } finally { wasm.cove_free(pointer, Math.max(1, bytes.length)); }
  }
  textInput.addEventListener("compositionstart", () => { composing = true; });
  textInput.addEventListener("compositionupdate", event => sendText(event.data, true));
  textInput.addEventListener("compositionend", event => {
    composing = false; sendText(event.data, !event.data); textInput.value = "";
    skipCompositionInput = true; setTimeout(() => { skipCompositionInput = false; }, 0);
  });
  const editKeys = {deleteContentBackward:42, deleteContentForward:76, insertLineBreak:40, insertParagraph:40};
  textInput.addEventListener("beforeinput", event => {
    if (composing || event.isComposing) return;
    // Soft keyboards can report these edits without a usable KeyboardEvent.code.
    const code = editKeys[event.inputType];
    if (!code) return;
    event.preventDefault();
    wasm.cove_key(code, 0, 1, 0);
    wasm.cove_key(code, 0, 0, 0);
    schedule();
  });
  textInput.addEventListener("input", event => {
    if (composing || event.isComposing || skipCompositionInput) return;
    sendText(event.data ?? textInput.value); textInput.value = "";
  });
  textInput.addEventListener("blur", () => { if (composing) sendText("", true); composing = false; textInput.value = ""; });
  const codes = { Enter:40, Escape:41, Backspace:42, Tab:43, Space:44, Home:74, PageUp:75, Delete:76, End:77, PageDown:78, ArrowRight:79, ArrowLeft:80, ArrowDown:81, ArrowUp:82 };
  for (const target of [surface, textInput]) for (const name of ["keydown", "keyup"]) target.addEventListener(name, event => {
    if (composing || event.isComposing) return;
    const code = codes[event.code];
    if (!code || (target === textInput && code === 44)) return;
    event.preventDefault();
    const modifiers = (event.shiftKey ? 1 : 0) | (event.ctrlKey ? 4 : 0) | (event.altKey ? 16 : 0) | (event.metaKey ? 64 : 0);
    wasm.cove_key(code, modifiers, name === "keydown" ? 1 : 0, event.repeat ? 1 : 0);
    if (name === "keydown") syncTextInput();
    schedule();
  });
  new ResizeObserver(schedule).observe(surface);
  window.addEventListener("scroll", syncTextInput, {passive:true});
  window.coveDebug = {
    get snapshot() { return lastFrame; }, get backend() { return renderer.backend; },
    get gpuInfo() {
      const info = renderer.adapter?.info;
      return info ? { vendor: info.vendor, architecture: info.architecture, device: info.device,
        description: info.description, isFallbackAdapter: info.isFallbackAdapter, lost: renderer.lost } : null;
    },
  };
  schedule();
  if (location.pathname.startsWith("/app/")) connectProbe().catch(error => console.warn("DevTools:", error.message));
} catch (error) { status.textContent = "Prototype failed to load"; console.error(error); document.querySelector("#stats").textContent = String(error); }

async function renderProbeFrame() {
  const deadline = performance.now() + 2500;
  let result = renderFrame({ syncNativeFocus: false });
  while (result === 1 && performance.now() < deadline) {
    await new Promise(resolve => setTimeout(resolve, 16));
    result = renderFrame({ syncNativeFocus: false });
  }
  return result;
}

async function captureFrame() {
  try {
    if (await renderProbeFrame() !== 0) throw new Error("The canvas could not render a frame.");
    // The canvas still owns the current WebGPU texture in this task.
    const current = document.querySelector(renderer.backend === "webgpu" ? "#gpu" : "#fallback");
    const canvas = document.createElement("canvas");
    canvas.width = current.width;
    canvas.height = current.height;
    canvas.getContext("2d").drawImage(current, 0, 0);
    const viewport = { width: lastFrame.width, height: lastFrame.height };
    const maxPng = 1_500_000;
    let source = canvas;
    while (true) {
      const blob = await new Promise(resolve => source.toBlob(resolve, "image/png"));
      if (!blob) throw new Error("The canvas could not be exported.");
      if (blob.size <= maxPng) {
        const png = await new Promise((resolve, reject) => {
          const reader = new FileReader();
          reader.onload = () => resolve(reader.result.split(",")[1]);
          reader.onerror = () => reject(reader.error);
          reader.readAsDataURL(blob);
        });
        return { Ok: { viewport, png } };
      }
      if (source.width <= 1 && source.height <= 1) throw new Error("The image exceeds the capture budget.");
      const scaled = document.createElement("canvas");
      scaled.width = Math.max(1, Math.floor(source.width / 2));
      scaled.height = Math.max(1, Math.floor(source.height / 2));
      scaled.getContext("2d").drawImage(source, 0, 0, scaled.width, scaled.height);
      source = scaled;
    }
  } catch (error) {
    return { Err: { code: "capture_failed", message: error.message } };
  }
}

async function connectProbe() {
  const storageKey = "cove.devtools.token";
  const token = location.hash.slice(1) || sessionStorage.getItem(storageKey);
  if (!token) return;
  sessionStorage.setItem(storageKey, token);
  history.replaceState(null, "", location.pathname + location.search);
  if (!lastFrame) await renderProbeFrame();

  const abort = new AbortController();
  const post = async (path, payload, authorization) => {
    const response = await fetch(path, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...(authorization ? { Authorization: "Bearer " + token } : {}),
      },
      body: JSON.stringify(payload),
      signal: abort.signal,
    });
    if (response.status === 401) return Promise.reject(new DOMException("Open the current controller link.", "SecurityError"));
    if (!response.ok) throw new Error("Controller request failed.");
    return response.json();
  };
  let lease;
  const release = () => {
    if (lease) navigator.sendBeacon("/probe/close", JSON.stringify(lease));
    lease = undefined;
  };
  const close = () => { abort.abort(); release(); };
  window.addEventListener("pagehide", close, { once: true });
  try {
    while (!abort.signal.aborted) {
      let completion = null;
      try {
        wasm.cove_probe_register();
        lease = await post("/probe/register", response(), true);
        while (!abort.signal.aborted) {
          const work = await post("/probe/poll", { lease, completion });
          if (!work) {
            completion = null;
            continue;
          }
          const method = work.command.method;
          const result = method === "capture" ? await captureFrame() : dispatch(work.command);
          completion = { sequence: work.sequence, result };
          if (method === "input" && result.Ok?.delivered) {
            await renderProbeFrame();
          } else if (method === "edit" || method === "select") {
            schedule();
          }
        }
      } catch (error) {
        if (error.name === "SecurityError") throw error;
      } finally {
        release();
        wasm.cove_probe_disconnect();
        if (!abort.signal.aborted) schedule();
      }
      if (!abort.signal.aborted) await new Promise(resolve => setTimeout(resolve, 400));
    }
  } finally {
    close();
    window.removeEventListener("pagehide", close);
  }
}
