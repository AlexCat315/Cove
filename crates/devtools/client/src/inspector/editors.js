import { clear, el, fmtNumber, numberValue } from "../core/format.js";
import { $, DETAIL_TABS, FLEX_ALIGN_VALUES, FLEX_FACTOR_LIMIT, FLEX_JUSTIFY_VALUES, FLEX_PROPERTY_NAMES, FLEX_RECORD_ATTEMPTS, FLEX_RECORD_DELAY_MS, RpcError, dom, state } from "../core/globals.js";
import { errorMessage, rpc, targetById, toast, unwrapData } from "../core/session.js";
import { emptyInline, errorInline, kvList, metric, rawDetails, sectionBlock } from "../core/ui-helpers.js";
import { currentNode, renderDetails, selectNode } from "./tree.js";

export function renderRender(node, detail, entry) {
  const panel = dom.panels.render;
  clear(panel);
  const flags = node.flags || {};
  const frame = node.frame;
  const absolute = node.absoluteFrame;
  panel.append(sectionBlock("Paint and input flags",
    el("div", { class: "metrics" },
      metric("Hit testable", flags.hitTestable ? "yes" : "no"),
      metric("Focusable", flags.focusable ? "yes" : "no"),
      metric("Clips", flags.clipsToBounds ? "yes" : "no"),
      metric("Background", flags.hasBackground ? "yes" : "no"),
      metric("Border", flags.hasBorder ? "yes" : "no"),
    ),
  ));
  panel.append(sectionBlock("Render state",
    kvList([
      ["Frame", frame ? `${fmtNumber(frame.x)} × ${fmtNumber(frame.y)} · ${fmtNumber(frame.w)} × ${fmtNumber(frame.h)}` : "—"],
      ["Absolute frame", absolute ? `${fmtNumber(absolute.x)} × ${fmtNumber(absolute.y)} · ${fmtNumber(absolute.w)} × ${fmtNumber(absolute.h)}` : "—"],
      ["Z-index", node.zIndex ?? "—"],
      ["Opacity", node.opacity === undefined || node.opacity === null ? "—" : fmtNumber(node.opacity, 2)],
      ["Content offset", node.contentOffset ? `${fmtNumber(node.contentOffset.x)} × ${fmtNumber(node.contentOffset.y)}` : "—"],
    ]),
  ));

  const style = node.style;
  if (style) {
    panel.append(sectionBlock("Style",
      kvList([
        ["Background", style.backgroundColor || "—"],
        ["Foreground", style.foregroundColor || "—"],
        ["Overrides", Array.isArray(style.overrides) && style.overrides.length ? style.overrides.join(", ") : "—"],
      ]),
    ));
  }

  const inventory = state.tree?.renderInventory;
  const object = detail?.renderObject;
  if (object) {
    const bounds = object.paintBounds;
    panel.append(sectionBlock("Render object", kvList([
      ["Parent", object.parentID && state.entries.has(object.parentID)
        ? el("button", { class: "tool-button", text: `#${object.parentID}`, on: { click: () => selectNode(object.parentID) } }) : "—"],
      ["Children", object.childCount],
      ["Layer root", object.layerRoot ? "Yes" : "No"],
      ["Layer cache", object.cachedBatches == null ? "Empty" : object.cacheInvalid ? "Invalidated" : `${object.cachedBatches} batches`],
      ["Paint bounds", bounds ? `${fmtNumber(bounds.x)}, ${fmtNumber(bounds.y)} · ${fmtNumber(bounds.w)} × ${fmtNumber(bounds.h)}` : "—"],
      ["Clip stack", object.clipCount],
      ["Paint count", object.paintCount],
    ])));
  }
  if (inventory) {
    const isLayerRoot = Array.isArray(inventory.layerRoots) && inventory.layerRoots.includes(node.id);
    panel.append(sectionBlock("Render inventory",
      kvList([
        ["Objects", inventory.objectCount ?? "—"],
        ["Layer roots", Array.isArray(inventory.layerRoots) ? inventory.layerRoots.length : "—"],
        ["This node is a layer root", isLayerRoot ? "yes" : "no"],
      ]),
    ));
  } else {
    panel.append(sectionBlock("Render inventory", emptyInline("The runtime did not publish a render tree inventory.")));
  }

  const inputInventory = state.tree?.inputInventory;
  if (inputInventory) {
    panel.append(sectionBlock("Input inventory",
      kvList([
        ["Nodes", inputInventory.nodeCount ?? "—"],
        ["Focusable", Array.isArray(inputInventory.focusables) ? inputInventory.focusables.length : "—"],
        ["Hit testable", Array.isArray(inputInventory.hitTestables) ? inputInventory.hitTestables.length : "—"],
        ["This node focusable", Array.isArray(inputInventory.focusables) && inputInventory.focusables.includes(node.id) ? "yes" : "no"],
        ["This node hit testable", Array.isArray(inputInventory.hitTestables) && inputInventory.hitTestables.includes(node.id) ? "yes" : "no"],
      ]),
    ));
  }

  const invalidations = Array.isArray(state.tree?.invalidations) ? state.tree.invalidations : null;
  if (invalidations) {
    const mine = invalidations.filter(record => record.target === node.id).slice(-12).reverse();
    const children = [];
    if (!mine.length) {
      children.push(emptyInline("No recorded invalidations target this node in the snapshot window."));
    } else {
      const rows = el("table", { class: "data-table" },
        el("thead", {}, el("tr", {},
          el("th", { text: "Phase" }), el("th", { text: "Source" }), el("th", { text: "Time" }),
        )),
        el("tbody", {}, ...mine.map(record => el("tr", {},
          el("td", { text: record.phase || "—" }),
          el("td", { class: "mono-cell", text: record.source || "—" }),
          el("td", { class: "mono-cell", text: fmtNumber(record.timestamp, 3) }),
        ))),
      );
      children.push(el("div", { class: "table-wrap", attrs: { style: "max-height:220px" } }, rows));
    }
    panel.append(sectionBlock("Recent invalidations", ...children));
  }
  if (state.detailError) panel.append(errorInline(errorMessage(state.detailError)));
}

function insetText(inset) {
  if (!inset) return "—";
  return `${fmtNumber(inset.top)} / ${fmtNumber(inset.right)} / ${fmtNumber(inset.bottom)} / ${fmtNumber(inset.left)}`;
}

const FLEX_FIELD_IDS = {
  flexGrow: "flex-grow",
  flexShrink: "flex-shrink",
  justifyContent: "flex-justify",
  alignItems: "flex-align",
};

function isFlexFactor(name) {
  return name === "flexGrow" || name === "flexShrink";
}

function flexEditorKey(nodeId) {
  return `${state.targetId}\u0000${nodeId}`;
}

function flexRecordValue(layout, name) {
  if (!layout) return null;
  if (isFlexFactor(name)) return numberValue(layout[name]);
  const value = layout[name];
  return typeof value === "string" && value ? value : null;
}

function flexNumberText(value) {
  const number = numberValue(value);
  if (number === null) return "";
  return String(Number(number.toFixed(3)));
}

function parseFlexFactor(raw) {
  if (typeof raw !== "string" || raw.trim() === "") return null;
  const parsed = Number(raw);
  if (!Number.isFinite(parsed) || parsed < 0 || parsed > FLEX_FACTOR_LIMIT) return null;
  return parsed;
}

function flexFieldText(name, layout, draft) {
  const pending = draft?.values?.[name];
  if (pending !== undefined) return pending;
  const value = flexRecordValue(layout, name);
  return isFlexFactor(name) ? flexNumberText(value) : (value ?? "");
}

function flexSameValue(name, left, right) {
  if (isFlexFactor(name)) {
    return typeof left === "number" && typeof right === "number" && Math.abs(left - right) <= 0.001;
  }
  return left === right;
}

function flexValuesMatch(layout, properties) {
  return FLEX_PROPERTY_NAMES.every(name =>
    !(name in properties) || flexSameValue(name, properties[name], flexRecordValue(layout, name)));
}

function flexDraftFieldInvalid(name, raw) {
  if (raw === undefined) return false;
  if (isFlexFactor(name)) return parseFlexFactor(raw) === null;
  return !raw;
}

function pruneFlexDraft(layout) {
  const values = state.flexEditor.draft?.values;
  if (!values) return;
  for (const name of FLEX_PROPERTY_NAMES) {
    if (!(name in values)) continue;
    const parsed = isFlexFactor(name) ? parseFlexFactor(values[name]) : values[name];
    if (parsed === null || parsed === "") continue;
    if (flexSameValue(name, parsed, flexRecordValue(layout, name))) delete values[name];
  }
}

function flexDraftProperties(layout) {
  const values = state.flexEditor.draft?.values || {};
  const properties = {};
  let invalid = false;
  for (const name of FLEX_PROPERTY_NAMES) {
    if (!(name in values)) continue;
    if (isFlexFactor(name)) {
      const parsed = parseFlexFactor(values[name]);
      if (parsed === null) {
        invalid = true;
        continue;
      }
      const record = flexRecordValue(layout, name);
      if (record === null || Math.abs(parsed - record) > 0.001) properties[name] = parsed;
    } else {
      const allowed = name === "justifyContent" ? FLEX_JUSTIFY_VALUES : FLEX_ALIGN_VALUES;
      if (!allowed.includes(values[name])) {
        invalid = true;
        continue;
      }
      if (values[name] !== flexRecordValue(layout, name)) properties[name] = values[name];
    }
  }
  return { properties, invalid, dirty: Object.keys(properties).length > 0 };
}

function flexEditorFor(node, layout) {
  const editor = state.flexEditor;
  const key = flexEditorKey(node.id);
  if (editor.key !== key) {
    editor.key = key;
    editor.draft = null;
    editor.error = null;
    editor.notice = null;
    editor.stale = false;
  }
  if (!editor.draft) editor.draft = { values: {} };
  pruneFlexDraft(layout);
  return editor;
}

function setFlexDraft(name, value) {
  const editor = state.flexEditor;
  if (!editor.draft) editor.draft = { values: {} };
  editor.draft.values[name] = value;
}

function flexAppliedRecord(targetId, nodeId) {
  return state.flexEditor.applied.get(targetId)?.get(nodeId) || null;
}

function markFlexOverride(targetId, nodeId, properties) {
  let map = state.flexEditor.applied.get(targetId);
  if (!map) {
    map = new Map();
    state.flexEditor.applied.set(targetId, map);
  }
  map.set(nodeId, { ...(map.get(nodeId) || {}), ...properties });
  while (map.size > 512) map.delete(map.keys().next().value);
}

function clearFlexOverride(targetId, nodeId) {
  state.flexEditor.applied.get(targetId)?.delete(nodeId);
}

function hasFlexOverride(node) {
  if (!node) return false;
  if (state.flexEditor.applied.get(state.targetId)?.has(node.id)) return true;
  const overrides = Array.isArray(node.style?.overrides) ? node.style.overrides : [];
  return overrides.some(name => FLEX_PROPERTY_NAMES.includes(name));
}

function flexStale(node) {
  const editor = state.flexEditor;
  if (editor.stale && editor.key === flexEditorKey(node.id)) return true;
  return state.detailError?.code === "stale_node";
}

function flexEditorBlocked(node, layout) {
  return !layout
    || !state.connected
    || !state.caps.runtime
    || !state.caps.style
    || flexStale(node || {})
    || state.flexEditor.busy;
}

function flexStatusFor(node, layout, invalid, dirty) {
  const editor = state.flexEditor;
  if (!layout) return { text: "This node has no layout box.", tone: "" };
  if (!state.caps.style) return { text: "This runtime does not advertise style overrides.", tone: "" };
  if (flexStale(node || {})) {
    const staleError = editor.error || (state.detailError ? errorMessage(state.detailError) : "");
    return { text: staleError || "This node is no longer in the scene. Refresh the tree.", tone: "error" };
  }
  if (editor.busy) return { text: editor.mode === "clear" ? "Resetting…" : "Applying…", tone: "busy" };
  if (editor.error) return { text: editor.error, tone: "error" };
  if (editor.notice) return { text: editor.notice, tone: "warn" };
  if (invalid) return { text: `Flex factors must be between 0 and ${FLEX_FACTOR_LIMIT}.`, tone: "error" };
  if (dirty) return { text: "Unapplied changes", tone: "" };
  return { text: "", tone: "" };
}

function fillFlexStatus(status, node, layout, invalid, dirty) {
  const view = flexStatusFor(node, layout, invalid, dirty);
  status.className = `flex-status${view.tone ? ` is-${view.tone}` : ""}`;
  clear(status);
  if (view.tone === "busy") status.append(el("span", { class: "spinner" }), view.text);
  else status.textContent = view.text;
}

function syncFlexEditor() {
  const form = document.getElementById("flex-editor");
  if (!form) return;
  const node = currentNode();
  const layout = node?.layout ?? null;
  const editor = state.flexEditor;
  const { properties, invalid, dirty } = flexDraftProperties(layout);
  const blocked = flexEditorBlocked(node, layout);
  const apply = document.getElementById("flex-apply");
  const reset = document.getElementById("flex-reset");
  if (apply) apply.disabled = blocked || invalid || !dirty;
  if (reset) reset.disabled = blocked || !hasFlexOverride(node);
  for (const name of FLEX_PROPERTY_NAMES) {
    const field = form.querySelector(`[data-flex-field="${name}"]`);
    if (!field) continue;
    field.classList.toggle("is-dirty", Boolean(properties[name]));
    const input = field.querySelector("input");
    if (input) input.setAttribute("aria-invalid", String(flexDraftFieldInvalid(name, editor.draft?.values?.[name])));
  }
  const status = document.getElementById("flex-status");
  if (status) fillFlexStatus(status, node, layout, invalid, dirty);
}

function flexNumberInput(name, label, layout, draft, disabled) {
  const id = FLEX_FIELD_IDS[name];
  const input = el("input", {
    class: "compact-input",
    attrs: {
      id,
      name,
      type: "number",
      min: "0",
      max: String(FLEX_FACTOR_LIMIT),
      step: "0.1",
      inputmode: "decimal",
      "aria-label": label,
      "aria-invalid": String(flexDraftFieldInvalid(name, draft?.values?.[name])),
    },
    props: { disabled, value: flexFieldText(name, layout, draft) },
    on: {
      input: () => {
        setFlexDraft(name, input.value);
        syncFlexEditor();
      },
    },
  });
  return input;
}

function flexSelect(name, label, options, layout, draft, disabled) {
  const id = FLEX_FIELD_IDS[name];
  const current = flexFieldText(name, layout, draft);
  const values = current && !options.includes(current) ? [...options, current] : options;
  const select = el("select", {
    class: "compact-input",
    attrs: { id, name, "aria-label": label },
    props: { disabled },
    on: {
      change: () => {
        setFlexDraft(name, select.value);
        syncFlexEditor();
      },
    },
  });
  if (!current) select.append(el("option", { props: { value: "" }, text: "—" }));
  for (const value of values) select.append(el("option", { props: { value }, text: value }));
  select.value = current;
  return select;
}

function renderFlexSection(node, layout, contentWidth, contentHeight) {
  const editor = flexEditorFor(node, layout);
  const draft = editor.draft;
  const disabled = flexEditorBlocked(node, layout);
  const { properties, invalid, dirty } = flexDraftProperties(layout);
  const overridden = Boolean(layout) && hasFlexOverride(node);
  const form = el("form", {
    class: "flex-editor",
    props: { id: "flex-editor", noValidate: true },
    on: {
      submit: event => {
        event.preventDefault();
        submitFlexEditor("set");
      },
    },
  });
  const fields = el("div", { class: "flex-fields" },
    el("label", { class: `flex-field${properties.flexGrow !== undefined ? " is-dirty" : ""}`, attrs: { for: FLEX_FIELD_IDS.flexGrow }, dataset: { flexField: "flexGrow" } },
      el("span", { text: "Grow" }),
      flexNumberInput("flexGrow", "Flex grow", layout, draft, disabled)),
    el("label", { class: `flex-field${properties.flexShrink !== undefined ? " is-dirty" : ""}`, attrs: { for: FLEX_FIELD_IDS.flexShrink }, dataset: { flexField: "flexShrink" } },
      el("span", { text: "Shrink" }),
      flexNumberInput("flexShrink", "Flex shrink", layout, draft, disabled)),
    el("label", { class: `flex-field${properties.justifyContent !== undefined ? " is-dirty" : ""}`, attrs: { for: FLEX_FIELD_IDS.justifyContent }, dataset: { flexField: "justifyContent" } },
      el("span", { text: "Justify" }),
      flexSelect("justifyContent", "Justify content", FLEX_JUSTIFY_VALUES, layout, draft, disabled)),
    el("label", { class: `flex-field${properties.alignItems !== undefined ? " is-dirty" : ""}`, attrs: { for: FLEX_FIELD_IDS.alignItems }, dataset: { flexField: "alignItems" } },
      el("span", { text: "Align" }),
      flexSelect("alignItems", "Align items", FLEX_ALIGN_VALUES, layout, draft, disabled)),
  );
  const apply = el("button", {
    class: "tool-button is-primary",
    props: { id: "flex-apply", type: "submit", disabled: disabled || invalid || !dirty },
    text: "Apply",
  });
  const reset = el("button", {
    class: "tool-button",
    props: { id: "flex-reset", type: "button", disabled: disabled || !overridden },
    text: "Reset",
    on: { click: () => submitFlexEditor("clear") },
  });
  const status = el("span", { class: "flex-status", props: { id: "flex-status" }, attrs: { role: "status" } });
  fillFlexStatus(status, node, layout, invalid, dirty);
  form.append(fields, el("div", { class: "flex-actions" }, apply, reset, status));
  return sectionBlock("Flex", form,
    layout ? kvList([
      ["Direction", layout.direction],
      ["Flex direction", layout.flexDirection],
      ["Content size", `${fmtNumber(contentWidth)} × ${fmtNumber(contentHeight)}`],
    ]) : emptyInline("This node did not publish layout metrics, so flex overrides are unavailable."),
  );
}

function flexEditorCurrent(epoch, targetId, request, nodeId) {
  const editor = state.flexEditor;
  return epoch === state.targetEpoch
    && targetId === state.targetId
    && request === editor.request
    && nodeId === state.selectedId
    && editor.key === flexEditorKey(nodeId);
}

export function resetFlexEditorState() {
  const editor = state.flexEditor;
  editor.request += 1;
  editor.key = "";
  editor.draft = null;
  editor.busy = false;
  editor.mode = null;
  editor.error = null;
  editor.notice = null;
  editor.stale = false;
}

function sendFlexStyle(targetId, command, args) {
  return rpc("edit_ui_style", { target: targetId, command, arguments: args });
}

async function fetchFlexRecord(targetId, nodeId) {
  const page = unwrapData(await rpc("query_ui_nodes", { target: targetId, node_id: nodeId }));
  const item = Array.isArray(page?.items) ? page.items[0] : null;
  if (!item?.node) throw new RpcError("The runtime returned no record for this node.", "stale_node");
  return item;
}

function adoptFlexRecord(nodeId, item) {
  const entry = state.entries.get(nodeId);
  if (entry && item.node) entry.node = { ...entry.node, ...item.node, children: entry.node.children };
  if (state.selectedId === nodeId) state.detail = item;
  if (state.selectedId === nodeId) state.detailError = null;
}

function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

// Read the node record back until it reflects the request, so the panel shows
// the runtime's values rather than the values the client asked for.
async function readFlexRecord(targetId, nodeId, mode, properties, epoch, request) {
  const editor = state.flexEditor;
  let item = null;
  let mismatch = false;
  for (let attempt = 0; attempt < FLEX_RECORD_ATTEMPTS; attempt += 1) {
    if (attempt > 0) await sleep(FLEX_RECORD_DELAY_MS);
    if (!flexEditorCurrent(epoch, targetId, request, nodeId)) return null;
    try {
      item = await fetchFlexRecord(targetId, nodeId);
    } catch (error) {
      if (!flexEditorCurrent(epoch, targetId, request, nodeId)) return null;
      if (error?.code === "stale_node") {
        editor.stale = true;
        editor.error = errorMessage(error);
        return null;
      }
      throw error;
    }
    if (!flexEditorCurrent(epoch, targetId, request, nodeId)) return null;
    const layout = item.node?.layout ?? null;
    if (!layout) {
      mismatch = true;
      continue;
    }
    if (mode === "set") {
      mismatch = !flexValuesMatch(layout, properties || {});
      if (!mismatch) return { item, mismatch };
    } else {
      const remaining = Array.isArray(item.node?.style?.overrides)
        ? item.node.style.overrides.filter(name => FLEX_PROPERTY_NAMES.includes(name))
        : [];
      const stillApplied = properties ? flexValuesMatch(layout, properties) : false;
      mismatch = remaining.length > 0 || stillApplied;
      if (!mismatch) return { item, mismatch };
    }
  }
  return { item, mismatch };
}

async function submitFlexEditor(mode) {
  const target = targetById();
  const nodeId = state.selectedId;
  const editor = state.flexEditor;
  if (!state.connected || !target || !state.caps.runtime || !state.caps.tree || !nodeId || editor.busy) return;
  const node = currentNode();
  const layout = node?.layout ?? null;
  if (!layout || flexStale(node || {})) return;
  let properties = null;
  if (mode === "set") {
    const draft = flexDraftProperties(layout);
    if (draft.invalid || !draft.dirty) return;
    properties = draft.properties;
  }
  const targetId = target.id;
  const epoch = state.targetEpoch;
  const request = ++editor.request;
  const appliedBefore = mode === "clear" ? flexAppliedRecord(targetId, nodeId) : null;
  editor.busy = true;
  editor.mode = mode;
  editor.error = null;
  editor.notice = null;
  renderDetails();
  try {
    const args = mode === "set" ? { id: nodeId, properties } : { id: nodeId };
    unwrapData(await sendFlexStyle(targetId, mode === "set" ? "inspect.style.set" : "inspect.style.clear", args));
    // The override exists in the runtime now, even if the selection moved on.
    if (mode === "set") markFlexOverride(targetId, nodeId, properties);
    else clearFlexOverride(targetId, nodeId);
    if (!flexEditorCurrent(epoch, targetId, request, nodeId)) return;
    const outcome = await readFlexRecord(targetId, nodeId, mode, mode === "set" ? properties : appliedBefore, epoch, request);
    if (!outcome) return;
    if (outcome.item) {
      adoptFlexRecord(nodeId, outcome.item);
      editor.draft = null;
    }
    if (outcome.mismatch) {
      editor.notice = mode === "set"
        ? "The layout record has not picked up the override yet."
        : "The layout record still reports the override.";
    }
  } catch (error) {
    if (!flexEditorCurrent(epoch, targetId, request, nodeId)) return;
    editor.error = errorMessage(error);
    editor.stale = error?.code === "stale_node";
  } finally {
    if (flexEditorCurrent(epoch, targetId, request, nodeId)) {
      editor.busy = false;
      editor.mode = null;
      renderDetails();
    }
  }
}

export function renderLayout(node, detail, entry) {
  const panel = dom.panels.layout;
  clear(panel);
  const layout = node.layout;
  const frame = node.frame;
  const absolute = node.absoluteFrame;
  panel.append(sectionBlock("Frame",
    el("div", { class: "metrics" },
      metric("X", fmtNumber(frame?.x)),
      metric("Y", fmtNumber(frame?.y)),
      metric("Width", fmtNumber(frame?.w)),
      metric("Height", fmtNumber(frame?.h)),
    ),
    kvList([
      ["Absolute frame", absolute ? `${fmtNumber(absolute.x)} × ${fmtNumber(absolute.y)} · ${fmtNumber(absolute.w)} × ${fmtNumber(absolute.h)}` : "—"],
      ["Content offset", node.contentOffset ? `${fmtNumber(node.contentOffset.x)} × ${fmtNumber(node.contentOffset.y)}` : "—"],
      ["Visible frame", detail?.presentation?.visibleFrame
        ? `${fmtNumber(detail.presentation.visibleFrame.x)} × ${fmtNumber(detail.presentation.visibleFrame.y)} · ${fmtNumber(detail.presentation.visibleFrame.w)} × ${fmtNumber(detail.presentation.visibleFrame.h)}`
        : "—"],
      ["Z-index", node.zIndex ?? "—"],
    ]),
  ));
  if (detail?.layoutChain) panel.append(renderLayoutChain(detail.layoutChain));
  if (!layout) {
    if (state.detailLoading && !state.detail) {
      panel.append(sectionBlock("Box model", el("div", { class: "inline-spinner" }, el("span", { class: "spinner" }), "Reading layout…")));
      return;
    }
    panel.append(sectionBlock("Box model", emptyInline("This node did not publish layout metrics.")));
    panel.append(renderFlexSection(node, null, null, null));
    if (state.detailError) panel.append(errorInline(errorMessage(state.detailError)));
    return;
  }
  const contentWidth = layout.contentWidth ?? Math.max(0, (frame?.w ?? 0) - (layout.padding?.left ?? 0) - (layout.padding?.right ?? 0) - (layout.border?.left ?? 0) - (layout.border?.right ?? 0));
  const contentHeight = layout.contentHeight ?? Math.max(0, (frame?.h ?? 0) - (layout.padding?.top ?? 0) - (layout.padding?.bottom ?? 0) - (layout.border?.top ?? 0) - (layout.border?.bottom ?? 0));
  panel.append(sectionBlock("Box model",
    el("div", { class: "box-model" },
      el("div", { class: "box-cell box-margin", text: `margin · ${insetText(layout.margin)}` }),
      el("div", { class: "box-cell box-border", text: `border · ${insetText(layout.border)}` }),
      el("div", { class: "box-cell box-padding", text: `padding · ${insetText(layout.padding)}` }),
      el("div", { class: "box-cell box-content", text: `content · ${fmtNumber(contentWidth)} × ${fmtNumber(contentHeight)}` }),
    ),
    el("div", { class: "legend" },
      el("span", { class: "padding", text: "padding" }),
      el("span", { class: "border", text: "border" }),
      el("span", { class: "content", text: "content" }),
    ),
    el("table", { class: "data-table", attrs: { style: "margin-top:8px" } },
      el("thead", {}, el("tr", {},
        el("th", { text: "Inset" }), el("th", { class: "num", text: "Top" }), el("th", { class: "num", text: "Right" }),
        el("th", { class: "num", text: "Bottom" }), el("th", { class: "num", text: "Left" }),
      )),
      el("tbody", {},
        ["margin", "border", "padding"].map(kind => el("tr", {},
          el("td", { text: kind }),
          el("td", { class: "num", text: fmtNumber(layout[kind]?.top) }),
          el("td", { class: "num", text: fmtNumber(layout[kind]?.right) }),
          el("td", { class: "num", text: fmtNumber(layout[kind]?.bottom) }),
          el("td", { class: "num", text: fmtNumber(layout[kind]?.left) }),
        )),
      ),
    ),
  ));
  const constraints = layout.constraints || {};
  const bound = value => value == null ? "—" : fmtNumber(value);
  panel.append(sectionBlock("Authored bounds",
    el("div", { class: "metrics" },
      metric("Min W", bound(constraints.minWidth)),
      metric("Max W", bound(constraints.maxWidth)),
      metric("Min H", bound(constraints.minHeight)),
      metric("Max H", bound(constraints.maxHeight)),
    ),
  ));
  if (layout.overflowDetected && !detail?.layoutChain) {
    panel.append(el("div", { class: "warn-inline", text: "A child extends beyond this node’s bounds." }));
  }
  panel.append(renderFlexSection(node, layout, contentWidth, contentHeight));
  if (state.detailError) panel.append(errorInline(errorMessage(state.detailError)));
}

function renderLayoutChain(chain) {
  const axis = input => input
    ? [input.known == null ? "Auto" : `${fmtNumber(input.known)}${input.definite === false ? " (indefinite)" : ""}`,
      input.available == null ? input.availableKind : fmtNumber(input.available),
      input.parent == null ? "—" : fmtNumber(input.parent)].join(" / ")
    : "—";
  const size = frame => frame ? `${fmtNumber(frame.w)} × ${fmtNumber(frame.h)}` : "—";
  const links = [chain.parent && ["Parent", chain.parent], ["Selected", chain.node],
    ...chain.children.map(child => ["Child", child])].filter(Boolean);
  const rows = links.map(([kind, item]) => {
    const name = item.name || (item.id ? `#${item.id}` : "Layout root");
    const linked = item.id !== state.selectedId && state.entries.has(item.id);
    return el("tr", {},
      el("td", {}, linked
        ? el("button", { class: "tool-button", text: `${kind} · ${name}`, on: { click: () => selectNode(item.id) } })
        : `${kind} · ${name}`),
      el("td", { text: axis(item.incoming?.width) }),
      el("td", { text: axis(item.incoming?.height) }),
      el("td", { text: size(item.solved) }),
      el("td", { text: item.visible === false ? "Hidden" : size(item.resolved) }),
    );
  });
  const overflow = chain.textOverflow;
  const edges = ["top", "right", "bottom", "left"].filter(edge => chain.overflow?.[edge] > 0.5)
    .map(edge => `${edge} ${fmtNumber(chain.overflow[edge])} pt`);
  return sectionBlock("Layout chain",
    el("div", { class: "table-wrap" }, el("table", { class: "data-table" },
      el("thead", {}, el("tr", {}, ["Node", "Width input", "Height input", "Solved", "Resolved"].map(text => el("th", { text })))),
      el("tbody", {}, rows),
    )),
    el("p", { class: "muted", text: "Input order: known / available / percentage basis. Units are logical points." }),
    chain.childCount > chain.children.length ? el("p", { class: "muted", text: `${chain.children.length} of ${chain.childCount} layout children` }) : null,
    kvList([["Flex basis", chain.flexBasis], ["Align self", chain.alignSelf]]),
    chain.textTruncated ? el("div", { class: "warn-inline", text: "Text truncated" }) : null,
    edges.length ? el("div", { class: "warn-inline", text: `Child overflow · ${edges.join(" · ")}` }) : null,
    (overflow?.width > 0.5 || overflow?.height > 0.5)
      ? el("div", { class: "warn-inline", text: `Text exceeds its content box by ${fmtNumber(overflow.width)} × ${fmtNumber(overflow.height)} pt` }) : null,
  );
}

export function renderSource(node, entry) {
  const panel = dom.panels.source;
  clear(panel);
  const source = node.source;
  if (!source) {
    panel.append(sectionBlock("Source", emptyInline("This node did not publish a source location. Rebuild the app with source locations enabled.")));
    return;
  }
  const path = source.filePath || source.fileID || "";
  const location = `${path}:${source.line ?? "?"}:${source.column ?? "?"}`;
  panel.append(sectionBlock("Source",
    el("div", { class: "source-card" },
      el("div", { class: "source-path", text: path || "(no file path)" }),
      el("div", { class: "source-location" },
        el("span", { class: "pill", text: `line ${source.line ?? "?"}` }),
        el("span", { class: "pill", text: `column ${source.column ?? "?"}` }),
      ),
      el("div", { class: "source-actions" },
        el("button", { class: "tool-button", props: { type: "button" }, text: "Copy path", on: { click: () => copyText(location, "Source location") } }),
        el("button", { class: "tool-button", props: { type: "button" }, text: "Copy file ID", on: { click: () => copyText(source.fileID, "File ID") } }),
      ),
    ),
    kvList([
      ["File ID", el("code", { text: source.fileID || "—" })],
      ["Line", String(source.line ?? "—")],
      ["Column", String(source.column ?? "—")],
    ]),
    rawDetails("Raw source record", source),
  ));
}

export function setDetailTab(name, focus = false) {
  if (!DETAIL_TABS.includes(name)) return;
  state.activeDetailTab = name;
  for (const tab of DETAIL_TABS) {
    const button = $(`tab-${tab}`);
    const panel = dom.panels[tab];
    const selected = tab === name;
    button.setAttribute("aria-selected", String(selected));
    button.tabIndex = selected ? 0 : -1;
    panel.hidden = !selected;
    if (selected && focus) button.focus();
  }
}

export function renderDetailTab() {
  setDetailTab(state.activeDetailTab);
}

export function updateNodeButtons() {
  dom.nodeShotButton.disabled = !(state.caps.inspectNodes && state.caps.capture) || !state.selectedId || state.captureLoading;
  dom.highlightButton.disabled = !state.selectedId || !state.caps.runtime;
  dom.clearHighlight.disabled = !state.caps.runtime;
  dom.readValueButton.disabled = !state.selectedId || state.detailLoading;
}

export async function copyText(value, label = "Value") {
  const text = String(value ?? "");
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
    } else {
      const area = document.createElement("textarea");
      area.value = text;
      area.style.position = "fixed";
      area.style.opacity = "0";
      document.body.append(area);
      area.select();
      document.execCommand("copy");
      area.remove();
    }
    toast(`${label} copied.`, "success", 3000);
  } catch {
    toast(`Could not copy ${label.toLowerCase()}.`, "error");
  }
}

