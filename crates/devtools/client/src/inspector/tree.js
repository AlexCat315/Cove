import { appendChildren, basename, clear, el, fmtMs, fmtNumber, icon, shortId } from "../core/format.js";
import { RpcError, dom, state } from "../core/globals.js";
import { errorMessage, retryTree, rpc, targetById, toast, unwrapData } from "../core/session.js";
import { emptyInline, errorInline, hideState, kvList, metric, rawDetails, sectionBlock, showState } from "../core/ui-helpers.js";
import { releaseCapture, renderScreenshot } from "./capture.js";
import { renderDetailTab, renderLayout, renderRender, renderSource, resetFlexEditorState, updateNodeButtons } from "./editors.js";

function nodeChildren(node) {
  return Array.isArray(node?.children) ? node.children : [];
}

function nodeIsAnchor(node, isRoot = false) {
  if (isRoot) return true;
  return node?.ownerScopeID === node?.id || Boolean(node?.recomposition);
}

export function shortType(viewTag) {
  const value = String(viewTag ?? "");
  if (!value) return "";
  const angle = value.indexOf("<");
  const base = angle >= 0 ? value.slice(0, angle) : value;
  const path = base.split("::");
  return path[path.length - 1];
}

/**
 * A wrapper is the node the composition layer inserts for a single modifier: it has
 * no authored name, no source, no recomposition scope and exactly one child. Folding
 * those runs is what turns the modifier stack into the node it wraps, so the tree
 * reads as content instead of as a chain of one-child hops.
 */
function nodeIsWrapper(node, isRoot = false) {
  if (!node || isRoot) return false;
  if (node.debugName || node.semanticRole || node.source || node.recomposition) return false;
  if (node.ownerScopeID === node.id) return false;
  return nodeChildren(node).length === 1;
}

function foldWrapperChains(root) {
  let folded = 0;
  const fold = (node) => {
    const kept = [];
    for (const child of nodeChildren(node)) {
      let current = child;
      const chain = [];
      while (nodeIsWrapper(current)) {
        const next = nodeChildren(current)[0];
        if (!next) break;
        chain.push(nodeLabel(current));
        current = next;
      }
      if (chain.length > 0) {
        folded += chain.length;
        current.foldedWrappers = [...(current.foldedWrappers ?? []), ...chain];
      }
      kept.push(fold(current));
    }
    node.children = kept;
    return node;
  };
  if (!root) return { root: null, folded: 0 };
  return { root: fold(root), folded };
}

export function nodeLabel(node) {
  return node?.debugName || node?.semanticRole || shortType(node?.viewTag)
    || (node?.id === state.rootId ? "Root" : shortId(node?.id)) || "Node";
}

function nodeRole(node) {
  return node?.semanticRole || node?.layoutRole || "";
}

function nodeSourceText(node) {
  const source = node?.source;
  if (!source) return "";
  return `${basename(source.filePath || source.fileID)}:${source.line ?? "?"}`;
}

function nodeMatches(node, query) {
  if (!query) return true;
  const source = node?.source;
  const haystack = [
    node?.debugName,
    node?.viewTag,
    node?.semanticRole,
    node?.layoutRole,
    node?.id,
    node?.ownerScopeID,
    source?.filePath,
    source?.fileID,
    source?.line,
  ].filter(Boolean).join(" ").toLowerCase();
  return haystack.includes(query);
}

function buildIndex(root) {
  state.entries = new Map();
  state.rootId = root?.id || null;
  if (!root) return;
  let order = 0;
  let anchors = 0;
  const stack = [{ node: root, parent: null, depth: 0, isRoot: true }];
  while (stack.length) {
    const item = stack.pop();
    const anchor = nodeIsAnchor(item.node, item.isRoot);
    if (anchor) anchors += 1;
    state.entries.set(item.node.id, {
      node: item.node,
      parentId: item.parent,
      depth: item.depth,
      order: order++,
      root: item.isRoot,
      anchor,
    });
    const children = nodeChildren(item.node);
    for (let index = children.length - 1; index >= 0; index -= 1) {
      stack.push({ node: children[index], parent: item.node.id, depth: item.depth + 1, isRoot: false });
    }
  }
  state.anchorFallback = false;
  if (anchors <= 1 && state.entries.size > 1) {
    anchors = 0;
    for (const entry of state.entries.values()) {
      entry.anchor = entry.root || Boolean(entry.node.debugName);
      if (entry.anchor) anchors += 1;
    }
    state.anchorFallback = true;
  }
  state.anchorCount = anchors;
}

export async function loadTree(options = {}) {
  if (!state.caps.tree || !state.targetId) return;
  const targetId = state.targetId;
  const epoch = state.targetEpoch;
  const request = ++state.treeRequest;
  state.treeLoading = true;
  state.treeError = null;
  renderTreeArea();
  dom.treeRefresh.disabled = true;
  try {
    const snapshot = await rpc("inspect_ui_tree", { target: targetId });
    if (epoch !== state.targetEpoch || request !== state.treeRequest || targetId !== state.targetId) return;
    state.tree = snapshot && typeof snapshot === "object" ? snapshot : { root: null };
    state.foldedWrappers = 0;
    if (state.tree.root && state.filters.chains) {
      const folded = foldWrapperChains(state.tree.root);
      state.tree.root = folded.root;
      state.foldedWrappers = folded.folded;
    }
    state.treeLoaded = true;
    state.treeError = null;
    buildIndex(state.tree.root || null);
    if (state.rootId) {
      if (!options.preserveExpansion) state.expanded.clear();
      state.expanded.add(state.rootId);
    }
    if (state.selectedId && !state.entries.has(state.selectedId)) {
      state.selectedId = null;
      state.detail = null;
      state.detailError = null;
      state.detailRequest += 1;
      toast("The selected component is no longer in the scene.", "info");
    }
    if (state.capture?.kind === "node" && state.capture.nodeId && !state.entries.has(state.capture.nodeId)) {
      releaseCapture();
    }
    if (state.selectedId && state.entries.has(state.selectedId)) {
      let parent = state.entries.get(state.selectedId)?.parentId;
      while (parent) {
        state.expanded.add(parent);
        parent = state.entries.get(parent)?.parentId;
      }
    }
    renderTreeArea();
    renderDetails();
    updateTreeButtons();
    updateNodeButtons();
    if (state.selectedId && state.entries.has(state.selectedId)) loadNodeDetail(state.selectedId);
  } catch (error) {
    if (epoch !== state.targetEpoch || targetId !== state.targetId || request !== state.treeRequest) return;
    state.treeError = error;
    state.treeLoaded = false;
    renderTreeArea();
    updateTreeButtons();
  } finally {
    if (epoch === state.targetEpoch && targetId === state.targetId && request === state.treeRequest) {
      state.treeLoading = false;
      renderTreeArea();
      updateTreeButtons();
    }
  }
}

function updateTreeButtons() {
  dom.treeRefresh.disabled = !state.caps.tree || state.treeLoading;
  dom.treeCollapse.disabled = !state.caps.tree || !state.treeLoaded;
}

function computeVisibleModel() {
  const root = state.tree?.root || null;
  const model = {
    included: new Set(),
    context: new Set(),
    matched: new Set(),
    visibleChildren: new Map(),
    total: state.entries.size,
    query: state.search.trim().toLowerCase(),
  };
  if (!root) return model;
  const query = model.query;
  const anchors = state.filters.anchors;
  const primitives = state.filters.primitives;
  if (!anchors && !primitives) return model;
  const stack = [{ node: root, phase: 0 }];
  while (stack.length) {
    const frame = stack.pop();
    const node = frame.node;
    if (frame.phase === 0) {
      stack.push({ node, phase: 1 });
      const children = nodeChildren(node);
      for (let index = children.length - 1; index >= 0; index -= 1) {
        stack.push({ node: children[index], phase: 0 });
      }
      continue;
    }
    const entry = state.entries.get(node.id);
    if (!entry) continue;
    const typeSelected = entry.anchor ? anchors : primitives;
    const queryMatch = nodeMatches(node, query);
    const own = typeSelected && queryMatch;
    let childIncluded = false;
    for (const child of nodeChildren(node)) {
      if (model.included.has(child.id)) {
        childIncluded = true;
        break;
      }
    }
    if (own) model.included.add(node.id);
    else if (query && childIncluded) {
      model.included.add(node.id);
      model.context.add(node.id);
    }
    if (query && queryMatch) model.matched.add(node.id);
  }
  return model;
}

function visibleChildrenFor(node, model) {
  if (model.visibleChildren.has(node.id)) return model.visibleChildren.get(node.id);
  const result = [];
  const stack = [...nodeChildren(node)].reverse();
  while (stack.length) {
    const child = stack.pop();
    if (model.included.has(child.id)) {
      result.push(child);
    } else {
      const grandchildren = nodeChildren(child);
      for (let index = grandchildren.length - 1; index >= 0; index -= 1) stack.push(grandchildren[index]);
    }
  }
  model.visibleChildren.set(node.id, result);
  return result;
}

function appendHighlighted(container, text, query) {
  const value = String(text ?? "");
  if (!query) {
    container.textContent = value;
    return;
  }
  const lower = value.toLowerCase();
  let from = 0;
  let index = lower.indexOf(query);
  while (index !== -1) {
    if (index > from) container.append(document.createTextNode(value.slice(from, index)));
    const mark = document.createElement("mark");
    mark.textContent = value.slice(index, index + query.length);
    container.append(mark);
    from = index + query.length;
    index = lower.indexOf(query, from);
  }
  if (from < value.length) container.append(document.createTextNode(value.slice(from)));
}

export function renderTreeArea() {
  const target = targetById();
  const caps = state.caps;
  const scrollTop = dom.tree.scrollTop;
  const hadFocus = dom.tree.contains(document.activeElement);
  const focusedId = hadFocus ? document.activeElement?.closest?.(".tree-row")?.dataset.id : null;
  clear(dom.tree);
  state.visibleRows = [];
  state.visibleModel = null;

  if (!target) {
    showState(dom.treeState, { icon: "device", title: "No target", message: "Start a Cove app with COVE_DEVTOOLS=1 and refresh." });
    dom.tree.hidden = true;
    dom.treeMeta.textContent = "";
    dom.treeFooter.hidden = true;
    return;
  }
  if (!caps.tree) {
    showState(dom.treeState, {
      icon: "hierarchy",
      title: "Component tree unavailable",
      message: caps.runtime ? "This runtime did not advertise a component tree." : "Select a Cove runtime to inspect components.",
    });
    dom.tree.hidden = true;
    dom.treeMeta.textContent = "No component tree";
    dom.treeFooter.hidden = true;
    return;
  }
  if (state.treeLoading && !state.treeLoaded) {
    showState(dom.treeState, { kind: "loading", title: "Reading components", message: "Waiting for the runtime snapshot." });
    dom.tree.hidden = true;
    dom.treeMeta.textContent = "Loading…";
    dom.treeFooter.hidden = true;
    return;
  }
  if (state.treeError) {
    showState(dom.treeState, {
      kind: "error",
      title: "Component tree failed",
      message: errorMessage(state.treeError),
      action: "Retry",
      onAction: retryTree,
    });
    dom.tree.hidden = true;
    dom.treeMeta.textContent = "Error";
    dom.treeFooter.hidden = true;
    return;
  }
  if (!state.treeLoaded) {
    showState(dom.treeState, { icon: "hierarchy", title: "No tree loaded", message: "Refresh the component tree.", action: "Load", onAction: () => loadTree() });
    dom.tree.hidden = true;
    dom.treeFooter.hidden = true;
    return;
  }
  if (!state.tree?.root) {
    showState(dom.treeState, { icon: "empty", title: "No root node", message: "The runtime published an empty scene." });
    dom.tree.hidden = true;
    dom.treeMeta.textContent = "0 nodes";
    dom.treeFooter.hidden = true;
    return;
  }
  if (!state.filters.anchors && !state.filters.primitives) {
    showState(dom.treeState, { icon: "empty", title: "Filters hide every node", message: "Enable Components or Primitives to show the tree." });
    dom.tree.hidden = true;
    dom.treeMeta.textContent = `${state.entries.size} nodes`;
    dom.treeFooter.hidden = true;
    return;
  }

  const model = computeVisibleModel();
  state.visibleModel = model;
  const query = model.query;
  const rows = [];
  const limit = 500;
  const walk = (node, depth, parentId) => {
    if (rows.length >= limit) return;
    const entry = state.entries.get(node.id);
    if (!entry) return;
    const children = visibleChildrenFor(node, model);
    const expanded = query ? true : state.expanded.has(node.id);
    rows.push({
      id: node.id,
      node,
      entry,
      depth,
      parentId,
      hasChildren: children.length > 0,
      expanded,
      context: model.context.has(node.id),
      matched: model.matched.has(node.id),
    });
    if (!expanded) return;
    for (const child of children) walk(child, depth + 1, node.id);
  };
  if (model.included.has(state.rootId)) {
    walk(state.tree.root, 0, null);
  } else {
    for (const child of visibleChildrenFor(state.tree.root, model)) walk(child, 0, null);
  }

  if (!rows.length) {
    if (query) {
      showState(dom.treeState, { icon: "search", title: "No matching components", message: `No component matches “${state.search.trim()}” under the current filters.` });
    } else {
      showState(dom.treeState, { icon: "empty", title: "No components to show", message: "The current type filters hide every published node." });
    }
    dom.tree.hidden = true;
    dom.treeFooter.hidden = true;
    dom.treeMeta.textContent = query ? "0 matches" : "0 rows";
    return;
  }

  hideState(dom.treeState);
  dom.tree.hidden = false;
  const fragment = document.createDocumentFragment();
  const selectedFocusable = rows.some(row => row.id === state.selectedId) || rows.some(row => row.id === focusedId);
  for (const row of rows) {
    const isSelected = row.id === state.selectedId;
    const tabIndex = isSelected ? 0 : (!state.selectedId && row === rows[0]) || (!selectedFocusable && row === rows[0]) ? 0 : -1;
    const rowNode = el("div", {
      class: `tree-row${row.context ? " is-context" : ""}`,
      attrs: {
        role: "treeitem",
        "aria-level": row.depth + 1,
        "aria-selected": String(isSelected),
        "aria-expanded": row.hasChildren ? String(row.expanded) : null,
        tabindex: tabIndex,
      },
      dataset: { id: row.id },
    });
    const twisty = el("button", {
      class: `tree-twisty${row.hasChildren ? "" : " empty"}`,
      props: { type: "button", tabIndex: -1, disabled: !row.hasChildren },
      attrs: { "aria-label": row.expanded ? "Collapse" : "Expand", "aria-expanded": String(row.expanded) },
    }, icon("chevron"));
    const label = el("span", { class: "tree-label" });
    appendHighlighted(label, nodeLabel(row.node), query);
    const role = nodeRole(row.node);
    const folded = row.node.foldedWrappers ?? [];
    const foldedChip = folded.length > 0
      ? el("span", { class: "tree-fold", text: `+${folded.length}`, attrs: { title: folded.join(" → ") } })
      : null;
    const title = [row.node.debugName, row.node.viewTag, role, row.node.id, folded.length > 0 ? `${folded.length} folded: ${folded.join(" → ")}` : null].filter(Boolean).join(" · ");
    appendChildren(rowNode, [
      el("span", { class: "tree-indent", attrs: { "aria-hidden": "true", style: `width:${row.depth * 14}px;flex:0 0 ${row.depth * 14}px` } }),
      twisty,
      el("span", { class: `node-glyph ${row.entry.anchor ? "anchor" : "primitive"}`, attrs: { "aria-hidden": "true" } }),
      label,
      foldedChip,
      role && role !== label.textContent ? el("span", { class: "tree-role", text: role }) : null,
    ]);
    rowNode.title = title;
    fragment.append(rowNode);
  }
  dom.tree.append(fragment);
  state.visibleRows = rows;
  const matches = query ? rows.filter(row => row.matched).length : rows.length;
  dom.treeMeta.textContent = query
    ? `${model.matched.size} match${model.matched.size === 1 ? "" : "es"}`
    : `${state.entries.size} nodes · ${state.anchorCount ?? 0} components`;
  dom.treeFooter.hidden = rows.length < limit && !query;
  if (!dom.treeFooter.hidden) {
    clear(dom.treeFooter);
    dom.treeFooter.append(document.createTextNode(rows.length >= limit ? `Showing the first ${limit} rows.` : `${matches} visible row${matches === 1 ? "" : "s"}.`));
  } else if (rows.length >= limit) {
    dom.treeFooter.hidden = false;
    clear(dom.treeFooter);
    dom.treeFooter.append(document.createTextNode(`Showing the first ${limit} matching rows.`));
  }
  if (hadFocus && focusedId) {
    const node = [...dom.tree.children].find(child => child.dataset.id === focusedId);
    if (node) node.focus();
  } else if (dom.tree.contains(document.activeElement) === false) {
    dom.tree.scrollTop = scrollTop;
  }
  if (state.scrollSelection && state.selectedId) {
    state.scrollSelection = false;
    const selected = [...dom.tree.children].find(child => child.dataset.id === state.selectedId);
    selected?.scrollIntoView({ block: "nearest" });
  }
}

export function selectNode(id, options = {}) {
  const entry = state.entries.get(id);
  if (!entry) {
    toast("The node is no longer in the scene.", "error");
    return;
  }
  if (state.selectedId === id && options.force !== true) {
    renderDetails();
    return;
  }
  if (state.capture?.kind === "node" && state.capture.nodeId !== id) releaseCapture();
  state.selectedId = id;
  state.detail = null;
  state.detailError = null;
  state.detailLoading = true;
  state.valueRequested = false;
  state.detailRequest += 1;
  resetFlexEditorState();
  clearTimeout(state.detailTimer);
  let parent = entry.parentId;
  while (parent) {
    state.expanded.add(parent);
    parent = state.entries.get(parent)?.parentId;
  }
  state.scrollSelection = options.reveal !== false;
  renderTreeArea();
  renderDetails();
  updateNodeButtons();
  state.detailTimer = setTimeout(() => {
    if (state.selectedId === id) loadNodeDetail(id);
  }, 70);
  highlightNode(id);
}

export async function loadNodeDetail(id) {
  if (!state.caps.runtime || !state.targetId) return;
  const targetId = state.targetId;
  const epoch = state.targetEpoch;
  const request = ++state.detailRequest;
  state.detailLoading = true;
  state.detailError = null;
  renderDetails();
  try {
    const result = await rpc("query_ui_nodes", { target: targetId, node_id: id });
    if (epoch !== state.targetEpoch || request !== state.detailRequest || state.selectedId !== id) return;
    const page = unwrapData(result);
    const item = Array.isArray(page?.items) ? page.items[0] : null;
    if (!item) throw new RpcError("The runtime returned no record for this node.", "empty_result");
    const entry = state.entries.get(id);
    if (entry && item.node) {
      entry.node = { ...entry.node, ...item.node, children: entry.node.children };
    }
    state.detail = item;
    state.detailError = null;
  } catch (error) {
    if (epoch !== state.targetEpoch || request !== state.detailRequest || state.selectedId !== id) return;
    state.detailError = error;
  } finally {
    if (epoch === state.targetEpoch && request === state.detailRequest && state.selectedId === id) {
      state.detailLoading = false;
      renderDetails();
      updateNodeButtons();
    }
  }
}

export async function readLiveValue() {
  const id = state.selectedId;
  if (!id) return;
  const epoch = state.targetEpoch;
  const targetId = state.targetId;
  dom.readValueButton.disabled = true;
  try {
    const result = await rpc("query_ui_nodes", { target: targetId, node_id: id, include_value: true });
    if (epoch !== state.targetEpoch || id !== state.selectedId) return;
    const page = unwrapData(result);
    const item = Array.isArray(page?.items) ? page.items[0] : null;
    if (item) {
      state.detail = item;
      state.valueRequested = true;
      renderDetails();
      toast("Live value read from the runtime.", "success", 4000);
    }
  } catch (error) {
    toast(errorMessage(error), "error", 8000);
  } finally {
    dom.readValueButton.disabled = false;
  }
}

export function highlightNode(id) {
  const targetId = state.targetId;
  const epoch = state.targetEpoch;
  clearTimeout(state.highlightTimer);
  state.highlightTimer = setTimeout(() => {
    rpc("select_ui_node", { target: targetId, node_id: id }).catch(error => {
      if (epoch === state.targetEpoch && targetId === state.targetId) toast(`Highlight failed: ${errorMessage(error)}`, "error", 7000);
    });
  }, 90);
}

export function clearHighlight() {
  clearTimeout(state.highlightTimer);
  const targetId = state.targetId;
  rpc("select_ui_node", { target: targetId }).catch(error => toast(errorMessage(error), "error", 7000));
}

function selectedEntry() {
  return state.selectedId ? state.entries.get(state.selectedId) : null;
}

export function currentNode() {
  const entry = selectedEntry();
  if (!entry) return null;
  const detailNode = state.detail?.node;
  if (!detailNode) return entry.node;
  return { ...entry.node, ...detailNode, children: entry.node.children };
}

export function renderDetails() {
  const target = targetById();
  if (!target) {
    showState(dom.detailsState, { icon: "device", title: "No target", message: "Select a connected target." });
    dom.detailsContent.hidden = true;
    return;
  }
  if (!state.caps.runtime) {
    showState(dom.detailsState, {
      icon: "hierarchy",
      title: "Component details unavailable",
      message: "This is a platform target. Select a Cove runtime to inspect nodes, properties and source.",
    });
    dom.detailsContent.hidden = true;
    return;
  }
  if (!state.caps.tree) {
    showState(dom.detailsState, { icon: "hierarchy", title: "No component tree", message: "This runtime did not advertise a component tree." });
    dom.detailsContent.hidden = true;
    return;
  }
  if (!state.selectedId) {
    showState(dom.detailsState, {
      icon: "select",
      title: "Select a component",
      message: state.mode === "pick" ? "Click a point in the screenshot to select the component there." : "Choose a row in the component tree, or use Pick to select at a point.",
    });
    dom.detailsContent.hidden = true;
    return;
  }
  const entry = selectedEntry();
  if (!entry) {
    showState(dom.detailsState, {
      kind: "error",
      title: "Component is stale",
      message: "The selected node is no longer in the current scene.",
      action: "Refresh tree",
      onAction: retryTree,
    });
    dom.detailsContent.hidden = true;
    return;
  }
  hideState(dom.detailsState);
  dom.detailsContent.hidden = false;
  const node = currentNode();
  const detail = state.detail;
  dom.detailsHeading.textContent = nodeLabel(node);
  dom.nodeId.textContent = node.id;
  dom.nodeKind.textContent = entry.anchor ? "Component" : "Primitive";
  dom.nodeKind.className = `badge ${entry.anchor ? "accent" : ""}`;
  renderNodeFlags(node, detail?.presentation);
  dom.readValueButton.hidden = !detail?.semantics;
  renderDetailTab();
  renderProperties(node, detail, entry);
  renderRender(node, detail, entry);
  renderLayout(node, detail, entry);
  renderSource(node, entry);
  renderScreenshot();
}

function renderNodeFlags(node, presentation) {
  clear(dom.nodeFlags);
  const flags = node?.flags || {};
  const items = [
    ["hit", flags.hitTestable],
    ["focus", flags.focusable],
    ["clip", flags.clipsToBounds],
    ["bg", flags.hasBackground],
    ["border", flags.hasBorder],
  ];
  for (const [label, value] of items) {
    dom.nodeFlags.append(el("span", { class: `flag ${value ? "on" : ""}`, attrs: { title: `${label}: ${value ? "yes" : "no"}` } }, label));
  }
  if (presentation) {
    dom.nodeFlags.append(el("span", { class: `flag ${presentation.isVisible ? "on" : ""}` }, presentation.isVisible ? "visible" : "hidden"));
    dom.nodeFlags.append(el("span", { class: `flag ${presentation.inputEnabled ? "on" : ""}` }, presentation.inputEnabled ? "input" : "blocked"));
  }
}

function renderProperties(node, detail, entry) {
  const panel = dom.panels.properties;
  clear(panel);
  const semantics = detail?.semantics;
  const presentation = detail?.presentation;

  panel.append(sectionBlock("Node",
    kvList([
      ["ID", el("code", { text: node.id })],
      ["Tree class", entry.anchor ? "Component anchor" : "Primitive node"],
      ["Debug name", node.debugName],
      ["View tag", node.viewTag],
      ["Semantic role", node.semanticRole],
      ["Layout role", node.layoutRole],
      ["Owner scope", node.ownerScopeID ? el("code", { text: node.ownerScopeID }) : "—"],
      ["Parent", entry.parentId ? el("code", { text: shortId(entry.parentId), attrs: { title: entry.parentId } }) : "—"],
      ["Children", String(nodeChildren(node).length)],
    ]),
  ));

  const semanticsChildren = [];
  if (state.detailLoading && !detail) {
    semanticsChildren.push(el("div", { class: "inline-spinner" }, el("span", { class: "spinner" }), "Reading accessibility data…"));
  } else if (!semantics) {
    semanticsChildren.push(emptyInline("This node does not publish accessibility semantics."));
  } else {
    const stateWords = [];
    const semanticState = semantics.state || {};
    for (const [key, value] of Object.entries(semanticState)) {
      const name = key.replace(/^is_?/, "").replace(/[A-Z]/g, letter => ` ${letter.toLowerCase()}`).replace(/_/g, " ").trim();
      if (value === true) stateWords.push(name);
      else if (value === false && (key === "isEnabled" || key === "is_enabled")) stateWords.push("disabled");
    }
    semanticsChildren.push(kvList([
      ["Role", semantics.role],
      ["Label", semantics.label],
      ["Value", semantics.value ? el("span", { class: "value-emphasis", text: semantics.value }) : "—"],
      ["Help", semantics.help],
      ["Identifier", semantics.identifier ? el("code", { text: semantics.identifier }) : "—"],
      ["Described by", semantics.described_by ? el("code", { text: semantics.described_by }) : "—"],
      ["Hidden", semantics.is_hidden ? "yes" : "no"],
      ["Combines children", semantics.combines_children ? "yes" : "no"],
      ["State", stateWords.length ? stateWords.join(", ") : "—"],
    ]));
    if (detail?.valueTruncated) semanticsChildren.push(el("div", { class: "truncation", text: "The live value is truncated to 4096 characters." }));
    if (semanticState.isSecure) semanticsChildren.push(el("div", { class: "warn-inline", text: "Secure fields report a redacted value. The provider is not invoked for search." }));
    if (!state.valueRequested) semanticsChildren.push(el("div", { class: "muted", text: "Input values are omitted until Read live value is used." }));
  }
  panel.append(sectionBlock("Semantics", ...semanticsChildren));

  const presentationChildren = [];
  if (state.detailLoading && !presentation) {
    presentationChildren.push(el("div", { class: "inline-spinner" }, el("span", { class: "spinner" }), "Reading presentation…"));
  } else if (!presentation) {
    presentationChildren.push(emptyInline("Presentation bounds were not published for this node."));
  } else {
    presentationChildren.push(kvList([
      ["Visible", presentation.isVisible ? "yes" : "no"],
      ["Input enabled", presentation.inputEnabled ? "yes" : "no"],
      ["Visible frame", presentation.visibleFrame
        ? `${fmtNumber(presentation.visibleFrame.x)} × ${fmtNumber(presentation.visibleFrame.y)} · ${fmtNumber(presentation.visibleFrame.w)} × ${fmtNumber(presentation.visibleFrame.h)}`
        : "—"],
    ]));
  }
  panel.append(sectionBlock("Presentation", ...presentationChildren));

  const recomposition = node.recomposition;
  if (recomposition) {
    panel.append(sectionBlock("Recomposition",
      el("div", { class: "metrics" },
        metric("Count", recomposition.count ?? "—"),
        metric("Last", fmtMs(recomposition.last_ms), ""),
        metric("Total", fmtMs(recomposition.total_ms), ""),
        metric("Max", fmtMs(recomposition.max_ms), ""),
      ),
      recomposition.reasons?.length
        ? el("div", { class: "section" }, el("h3", { text: "Reasons" }), kvList(recomposition.reasons.map(reason => [
          reason.kind || "reason",
          [reason.detail, reason.originScopeID ? `origin ${shortId(reason.originScopeID)}` : ""].filter(Boolean).join(" · ") || "—",
        ])))
        : null,
    ));
  } else {
    panel.append(sectionBlock("Recomposition", emptyInline("This node does not own a recomposition scope.")));
  }

  if (state.detailError) panel.append(errorInline(errorMessage(state.detailError)));

  const rawNode = { ...node };
  delete rawNode.children;
  panel.append(rawDetails("Raw node record", { node: rawNode, semantics: semantics ?? null, presentation: presentation ?? null, parentID: detail?.parentID ?? entry.parentId }));
}

