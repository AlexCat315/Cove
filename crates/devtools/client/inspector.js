const $ = id => document.getElementById(id);
const targets = $("targets");
const image = $("capture");
const details = $("details");
const nodes = $("nodes");
const status = $("status");
const bind = $("platform-target");
const sessionKey = "cove.devtools.token";
const token = location.hash.slice(1) || sessionStorage.getItem(sessionKey);
if (token) {
  sessionStorage.setItem(sessionKey, token);
  history.replaceState(null, "", location.pathname);
}
const authorization = { Authorization: `Bearer ${token ?? ""}` };
let sequence = 0;
let frame;
let picture;
let busy = false;
let currentTargets = [];

async function call(method, params = {}) {
  const response = await fetch("/rpc", {
    method: "POST",
    headers: { ...authorization, "Content-Type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: ++sequence, method, params }),
  });
  const reply = await response.json();
  if (reply.error || !response.ok) throw new Error(reply.error?.message ?? reply.message);
  return reply.result;
}

function clearCapture() {
  frame = undefined;
  image.hidden = true;
  image.removeAttribute("src");
  if (picture) URL.revokeObjectURL(picture);
  picture = undefined;
  $("save").hidden = true;
  $("save").removeAttribute("href");
}

function updateTargetControls() {
  const target = currentTargets.find(item => item.id === targets.value);
  const caps = target?.capabilities ?? [];
  $("tree").disabled = !caps.includes("tree");
  $("native").disabled = !caps.includes("native_elements");
  $("logs").disabled = !caps.includes("diagnostics");
  $("timing").disabled = !caps.includes("timing");
  $("trace-start").disabled = !caps.includes("trace");
  $("trace-stop").disabled = !caps.includes("trace");
  $("snap").disabled = !caps.includes("capture");
  $("press").disabled = !caps.includes("input") || target.kind === "display";
  $("type").disabled = $("press").disabled;
  $("binding").hidden = target?.kind !== "runtime";
  updateBindings();
}

async function refresh() {
  const selected = targets.value;
  const { targets: list } = await call("get_ui_session");
  currentTargets = list;
  targets.replaceChildren(new Option("Select a target", ""));
  for (const target of list) {
    targets.append(new Option(`${target.title} · ${target.kind}`, target.id));
  }
  if (list.some(target => target.id === selected)) targets.value = selected;
  else clearCapture();
  updateTargetControls();
  status.textContent = `${list.length} targets`;
}

function updateBindings() {
  const target = currentTargets.find(item => item.id === targets.value);
  bind.replaceChildren(new Option("Select a platform target", ""));
  if (target?.kind !== "runtime") return;
  for (const candidate of currentTargets) {
    if (target.platform !== candidate.platform) continue;
    if (candidate.kind === "webdriver" || candidate.kind === "portal" ||
        (candidate.kind === "window" && candidate.id.startsWith(`window:${target.pid}:`) &&
         (!target.platform_target || target.platform_target === candidate.id))) {
      bind.append(new Option(candidate.title, candidate.id));
    }
  }
  if (target.platform_target) bind.value = target.platform_target;
}

async function run(action) {
  if (busy) return;
  busy = true;
  document.body.setAttribute("aria-busy", "true");
  try { await action(); }
  catch (error) { status.textContent = error.message; }
  finally {
    busy = false;
    document.body.removeAttribute("aria-busy");
  }
}

function inspect(method) {
  return run(async () => {
    const target = targets.value;
    const data = await call(method, { target });
    if (target !== targets.value) return;
    nodes.hidden = true;
    nodes.replaceChildren();
    if (method === "inspect_ui_tree" && data.root) {
      const root = document.createElement("ul");
      root.append(component(data.root, true));
      nodes.append(root);
      nodes.hidden = false;
      details.textContent = "";
    } else if (method === "query_native_ui" && data.elements) {
      nativeElements(data.elements);
      details.textContent = data.truncated ? "Native tree truncated." : "";
    } else {
      details.textContent = JSON.stringify(data, null, 2);
    }
    status.textContent = "Updated";
  });
}

function component(node, expanded = false) {
  const item = document.createElement("li");
  const { children, ...properties } = node;
  const title = node.debugName ?? node.semanticRole ?? node.viewTag ?? node.id;
  const show = () => {
    details.textContent = JSON.stringify(properties, null, 2);
    run(async () => { await call("select_ui_node", { target: targets.value, node_id: node.id }); });
  };
  if (!children.length) {
    const button = document.createElement("button");
    button.textContent = title;
    button.onclick = show;
    item.append(button);
    return item;
  }
  const disclosure = document.createElement("details");
  disclosure.open = expanded;
  const label = document.createElement("summary");
  label.textContent = title;
  label.onclick = show;
  const list = document.createElement("ul");
  const populate = () => {
    if (disclosure.open && !list.hasChildNodes()) list.append(...children.map(child => component(child)));
  };
  disclosure.ontoggle = populate;
  populate();
  disclosure.append(label, list);
  item.append(disclosure);
  return item;
}

function nativeElements(elements) {
  const lists = new Map();
  const root = document.createElement("ul");
  for (const node of elements) {
    const item = document.createElement("li");
    const button = document.createElement("button");
    button.textContent = [node.name, node.role].filter(Boolean).join(" · ") || node.id;
    button.onclick = () => {
      details.textContent = JSON.stringify(node, null, 2);
      const activate = document.createElement("button");
      activate.textContent = "Activate";
      activate.onclick = () => perform({ kind: "activate", element: node.id });
      details.append("\n", activate);
    };
    const children = document.createElement("ul");
    lists.set(node.id, children);
    item.append(button, children);
    (lists.get(node.parent) ?? root).append(item);
  }
  nodes.append(root);
  nodes.hidden = false;
}

async function capture(target) {
  const next = await call("capture_ui", { target, scope: $("scope").value });
  const response = await fetch(next.resource, { headers: authorization });
  if (!response.ok) throw new Error("Capture resource expired.");
  const blob = await response.blob();
  if (target !== targets.value) return;
  clearCapture();
  picture = URL.createObjectURL(blob);
  image.src = picture;
  image.hidden = false;
  $("save").href = picture;
  $("save").hidden = false;
  frame = next;
  status.textContent = next.source.replaceAll("_", " ");
}

function perform(action) {
  const target = targets.value;
  const capture_id = ["click", "drag", "scroll"].includes(action.kind) ? frame?.capture_id : undefined;
  return run(async () => {
    const result = await call("input_ui", { target, capture_id, action });
    if (target !== targets.value) return;
    details.textContent = JSON.stringify(result, null, 2);
    status.textContent = "Input delivered";
  });
}

function coordinate(event) {
  if (!frame || image.hidden) return null;
  const rect = image.getBoundingClientRect();
  const { x, y, width, height } = frame.coordinates;
  return {
    x: x + Math.floor((event.clientX - rect.left) / rect.width * width),
    y: y + Math.floor((event.clientY - rect.top) / rect.height * height),
  };
}

let pressed;
image.addEventListener("pointerdown", event => {
  if (event.button !== 0) return;
  pressed = coordinate(event);
  image.setPointerCapture(event.pointerId);
});
image.addEventListener("pointerup", event => {
  const start = pressed;
  pressed = undefined;
  const end = coordinate(event);
  if (!start || !end) return;
  if (Math.abs(end.x - start.x) + Math.abs(end.y - start.y) > 4) {
    perform({ kind: "drag", ...start, to_x: end.x, to_y: end.y });
  } else {
    perform({ kind: "click", ...end });
  }
});
image.addEventListener("pointercancel", () => { pressed = undefined; });
image.addEventListener("contextmenu", event => {
  event.preventDefault();
  const point = coordinate(event);
  if (point) perform({ kind: "click", ...point, button: 2 });
});
image.addEventListener("wheel", event => {
  if (event.ctrlKey) return;
  const point = coordinate(event);
  if (!point) return;
  event.preventDefault();
  perform({ kind: "scroll", ...point, dx: Math.round(event.deltaX), dy: Math.round(event.deltaY) });
}, { passive: false });

$("refresh").onclick = () => run(refresh);
targets.onchange = () => {
  clearCapture();
  nodes.hidden = true;
  nodes.replaceChildren();
  details.textContent = "";
  updateTargetControls();
};
$("bind").onclick = () => run(async () => {
  await call("bind_ui_target", { target: targets.value, platform_target: bind.value });
  await refresh();
});
$("snap").onclick = () => run(() => capture(targets.value));
$("tree").onclick = () => inspect("inspect_ui_tree");
$("native").onclick = () => inspect("query_native_ui");
$("logs").onclick = () => inspect("query_ui_logs");
$("timing").onclick = () => inspect("record_ui_timings");
$("trace-start").onclick = () => inspect("start_ui_trace");
$("trace-stop").onclick = () => inspect("stop_ui_trace");
$("press").onclick = () => perform({ kind: "key", key: $("key").value });
$("type").onclick = () => perform({ kind: "text", text: $("text").value });
$("scope").onchange = clearCapture;
updateTargetControls();
if (token) run(refresh);
else status.textContent = "Open the Inspector link printed by cove-devtools serve.";
