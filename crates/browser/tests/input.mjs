import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { WASI, File, OpenFile, ConsoleStdout } from "../dist/wasi/index.js";
import { createWebGpuBridge } from "../dist/webgpu.js";

const read = path => readFileSync(new URL(path, import.meta.url));
const module = await WebAssembly.compile(read("../dist/cove.wasm"));
const font = read("../dist/fonts/NotoSans.ttf");
const decoder = new TextDecoder();

async function launch(component) {
  const args = component ? ["gallery", "--component", component] : ["counter"];
  const wasi = new WASI(args, [], [
    new OpenFile(new File([])),
    ConsoleStdout.lineBuffered(console.log),
    ConsoleStdout.lineBuffered(console.error),
  ]);
  const bridge = createWebGpuBridge(() => instance.exports, null);
  const instance = await WebAssembly.instantiate(module, {
    wasi_snapshot_preview1: wasi.wasiImport,
    cove_webgpu: bridge.imports,
  });
  wasi.initialize(instance);
  const wasm = instance.exports;
  const pointer = wasm.cove_font_alloc(font.length);
  assert.ok(pointer);
  new Uint8Array(wasm.memory.buffer, pointer >>> 0, font.length).set(font);
  assert.ok(wasm.cove_font_load(pointer, font.length));
  wasm.cove_free(pointer, font.length);
  return wasm;
}

function render(wasm) {
  assert.equal(wasm.cove_render(638, 438), 0);
  const pointer = wasm.cove_snapshot();
  const size = wasm.cove_snapshot_size();
  return JSON.parse(decoder.decode(new Uint8Array(wasm.memory.buffer, pointer >>> 0, size)));
}

function find(node, name) {
  if (node.debugName === name) return node;
  for (const child of node.children) {
    const found = find(child, name);
    if (found) return found;
  }
}

function center({ absoluteFrame: { x, y, w, h } }) {
  return [x + w / 2, y + h / 2];
}

function anchor(wasm) {
  const pointer = wasm.cove_text_input_area();
  return pointer ? Array.from(new Float32Array(wasm.memory.buffer, pointer >>> 0, 3)) : null;
}

function text(wasm, value, editing = false) {
  const bytes = Buffer.from(value);
  const size = Math.max(1, bytes.length);
  const pointer = wasm.cove_alloc(size);
  assert.ok(pointer);
  new Uint8Array(wasm.memory.buffer, pointer >>> 0, bytes.length).set(bytes);
  wasm.cove_text(pointer, bytes.length, Number(editing));
  wasm.cove_free(pointer, size);
}

{
  const wasm = await launch("scrollView");
  const scroll = () => find(render(wasm).tree.root, "scroll-view-control");
  // In the default browser viewport the surrounding story clips the lower rows.
  const wheelPoint = ({ absoluteFrame: { x, y, w } }) => [x + w / 2, y + 8];
  const start = scroll();
  assert.equal(start.contentOffset.y, 0);
  wasm.cove_wheel(0, 0, 70, ...wheelPoint(start), 10);
  const afterPixels = scroll();
  assert.equal(afterPixels.contentOffset.y, 70);
  wasm.cove_wheel(1, 0, 1, ...wheelPoint(afterPixels), 20);
  const afterLines = scroll();
  assert.ok(afterLines.contentOffset.y > 71, "line delta uses the scroll view's step");
  wasm.cove_wheel(2, 0, 1, ...wheelPoint(afterLines), 30);
  const pageDelta = scroll().contentOffset.y - afterLines.contentOffset.y;
  assert.ok(pageDelta > 1 && pageDelta < afterLines.frame.h, "page delta uses the clipped viewport");
}

{
  const wasm = await launch("textFields");
  let root = render(wasm).tree.root;
  assert.equal(anchor(wasm), null);
  const search = find(root, "gallery-search");
  wasm.cove_pointer(3, ...center(search));
  const before = anchor(wasm);
  assert.ok(before?.every(Number.isFinite), "Gallery search exposes its caret before a render");
  text(wasm, "Calendar", true);
  root = render(wasm).tree.root;
  assert.ok(anchor(wasm)[0] > before[0], "preedit advances the visual caret");
  assert.ok(find(root, "sidebar-item-textFields"), "preedit does not filter the catalog");
  text(wasm, "", true);
  render(wasm);
  assert.equal(anchor(wasm)[0], before[0], "empty preedit clears the composition");
  text(wasm, "Calendar");
  root = render(wasm).tree.root;
  assert.ok(find(root, "sidebar-item-calendar"));
  assert.equal(find(root, "sidebar-item-textFields"), undefined, "committed text filters the catalog");
  wasm.cove_pointer(3, 630, 20);
  assert.equal(anchor(wasm), null, "clicking outside the editor releases browser input");
}

{
  const wasm = await launch();
  const root = render(wasm).tree.root;
  wasm.cove_pointer(3, ...center(find(root, "counter.increment")));
  wasm.cove_pointer(3, ...center(find(root, "counter.note")));
  assert.ok(anchor(wasm), "Counter note retains its input bridge");
  text(wasm, "你好");
  const frame = render(wasm);
  assert.equal(frame.count, 1);
  assert.equal(frame.note, "你好");
}

console.log("Wasm input smoke test passed");
