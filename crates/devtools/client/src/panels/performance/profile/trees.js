/* CPU profile views: call tree, bottom-up, flame chart, methods and stacks */

function profileFrameIdentity(frame) {
  const name = typeof frame?.function === "string" ? frame.function.trim() : "";
  if (name) return `fn:${name}`;
  const file = typeof frame?.file === "string" ? frame.file.trim() : "";
  if (file) return `loc:${file}:${frame.line === null || frame.line === undefined ? "" : Math.trunc(frame.line)}`;
  const address = typeof frame?.address === "string" ? frame.address.trim() : "";
  if (address) return `addr:${address}`;
  return `label:${String(frame?.label || "unknown frame")}`;
}

function profileFrameLabel(frame) {
  const name = typeof frame?.function === "string" ? frame.function.trim() : "";
  if (name) return name;
  const label = String(frame?.label || "").trim();
  return label || "unknown frame";
}

function profileFrameLocation(frame) {
  if (!frame?.file) return "";
  return `${frame.file}${frame.line === null || frame.line === undefined ? "" : `:${Math.trunc(frame.line)}`}`;
}

function profileFrameKeyFromHotspot(entry) {
  return profileFrameIdentity({
    function: entry.function,
    file: entry.file,
    line: entry.line,
    address: "",
    label: entry.label,
  });
}

function profileSharePercent(samples, base, digits = 1) {
  if (samples === null || samples === undefined || !base) return "—";
  return `${((samples / base) * 100).toFixed(digits)}%`;
}

function newProfileNode({ key, identity, label, location, depth, frame = null, root = false }) {
  return {
    key,
    identity,
    label,
    location,
    depth,
    frame,
    root,
    samples: 0,
    selfSamples: 0,
    children: new Map(),
    childrenArray: [],
    percent: null,
    selfPercent: null,
  };
}

function finalizeProfileNode(node, shareBase) {
  node.childrenArray = [...node.children.values()]
    .sort((left, right) => right.samples - left.samples || left.label.localeCompare(right.label));
  node.percent = shareBase ? (node.samples / shareBase) * 100 : null;
  node.selfPercent = shareBase ? (node.selfSamples / shareBase) * 100 : null;
  for (const child of node.childrenArray) finalizeProfileNode(child, shareBase);
}

function buildProfileCallTree(rows, shareBase) {
  const root = newProfileNode({ key: "root", identity: "root", label: "All samples", location: "", depth: 0, root: true });
  let nodes = 1;
  let depthCappedSamples = 0;
  let branchCappedSamples = 0;
  let unattributedSamples = 0;
  let unattributedStacks = 0;
  for (const row of rows) {
    if (!row.frames.length) {
      unattributedSamples += row.count;
      unattributedStacks += 1;
      continue;
    }
    root.samples += row.count;
    const frames = row.frames;
    const depthLimit = Math.min(frames.length, MAX_PROFILE_TREE_DEPTH);
    let node = root;
    let capped = false;
    for (let depth = 1; depth <= depthLimit; depth += 1) {
      const frame = frames[frames.length - depth];
      const identity = profileFrameIdentity(frame);
      let child = node.children.get(identity);
      if (!child) {
        if (node.children.size >= MAX_PROFILE_TREE_CHILDREN || nodes >= MAX_PROFILE_TREE_NODES) {
          branchCappedSamples += row.count;
          capped = true;
          break;
        }
        child = newProfileNode({
          key: `${node.key}/${identity}`,
          identity,
          label: profileFrameLabel(frame),
          location: profileFrameLocation(frame),
          depth,
          frame,
        });
        node.children.set(identity, child);
        nodes += 1;
      }
      child.samples += row.count;
      node = child;
    }
    if (capped) continue;
    if (depthLimit < frames.length) {
      depthCappedSamples += row.count;
      continue;
    }
    node.selfSamples += row.count;
  }
  finalizeProfileNode(root, shareBase);
  return {
    root,
    nodes,
    depthCappedSamples,
    branchCappedSamples,
    unattributedSamples,
    unattributedStacks,
    cappedDepth: MAX_PROFILE_TREE_DEPTH,
  };
}

function finalizeProfileCallers(node, shareBase) {
  node.callersArray = [...node.callers.values()]
    .sort((left, right) => right.samples - left.samples || left.label.localeCompare(right.label));
  node.percent = shareBase ? (node.samples / shareBase) * 100 : null;
  for (const caller of node.callersArray) finalizeProfileCallers(caller, shareBase);
}

function buildProfileBottomUp(rows, shareBase) {
  const groups = new Map();
  let unattributedSamples = 0;
  let unattributedStacks = 0;
  let cappedSamples = 0;
  for (const row of rows) {
    if (!row.frames.length) {
      unattributedSamples += row.count;
      unattributedStacks += 1;
      continue;
    }
    const leaf = row.frames[0];
    const identity = profileFrameIdentity(leaf);
    let group = groups.get(identity);
    if (!group) {
      group = {
        key: identity,
        identity,
        label: profileFrameLabel(leaf),
        location: profileFrameLocation(leaf),
        frame: leaf,
        samples: 0,
        callers: new Map(),
        callersArray: [],
        percent: null,
      };
      groups.set(identity, group);
    }
    group.samples += row.count;
    let node = group;
    const limit = Math.min(row.frames.length, MAX_PROFILE_TREE_DEPTH + 1);
    for (let depth = 1; depth < limit; depth += 1) {
      const frame = row.frames[depth];
      const callerIdentity = profileFrameIdentity(frame);
      let caller = node.callers.get(callerIdentity);
      if (!caller) {
        if (node.callers.size >= MAX_PROFILE_BOTTOM_CALLERS) {
          cappedSamples += row.count;
          break;
        }
        caller = {
          key: `${node.key}>${callerIdentity}`,
          identity: callerIdentity,
          label: profileFrameLabel(frame),
          location: profileFrameLocation(frame),
          frame,
          depth,
          samples: 0,
          callers: new Map(),
          callersArray: [],
          percent: null,
        };
        node.callers.set(callerIdentity, caller);
      }
      caller.samples += row.count;
      node = caller;
    }
    if (row.frames.length > MAX_PROFILE_TREE_DEPTH + 1) cappedSamples += row.count;
  }
  const all = [...groups.values()]
    .sort((left, right) => right.samples - left.samples || left.label.localeCompare(right.label));
  const shown = all.slice(0, MAX_PROFILE_BOTTOM_GROUPS);
  for (const group of shown) finalizeProfileCallers(group, shareBase);
  return {
    groups: shown,
    totalGroups: all.length,
    omittedGroups: Math.max(0, all.length - shown.length),
    unattributedSamples,
    unattributedStacks,
    cappedSamples,
  };
}

function layoutProfileFlame(root, shareBase) {
  const rows = [];
  const state = { bars: 0, depthCapped: false, barCapped: false, thinSkipped: 0 };
  const visit = (node, start, depth) => {
    if (!shareBase) return;
    const width = (node.samples / shareBase) * 100;
    if (depth > MAX_PROFILE_FLAME_DEPTH) {
      state.depthCapped = true;
      return;
    }
    if (state.bars >= MAX_PROFILE_FLAME_BARS) {
      state.barCapped = true;
      return;
    }
    if (depth > 0 && width < PROFILE_FLAME_MIN_PERCENT) {
      state.thinSkipped += 1;
      return;
    }
    if (!rows[depth]) rows[depth] = [];
    rows[depth].push({ node, start, width });
    state.bars += 1;
    let cursor = start;
    for (const child of node.childrenArray) {
      visit(child, cursor, depth + 1);
      cursor += (child.samples / shareBase) * 100;
    }
  };
  visit(root, 0, 0);
  return {
    rows,
    bars: state.bars,
    depthCapped: state.depthCapped,
    barCapped: state.barCapped,
    thinSkipped: state.thinSkipped,
    depth: rows.length,
    maxDepth: MAX_PROFILE_FLAME_DEPTH,
  };
}

function buildProfileModel(profile) {
  const rows = [];
  let total = 0;
  for (const stack of profile.stacks) {
    if (!stack || !Array.isArray(stack.frames)) continue;
    const frames = stack.frames.map(normalizeProfileFrame);
    const count = numberValue(stack.count);
    if (count === null || count <= 0) continue;
    rows.push({ frames, count });
    total += count;
  }
  const retained = numberValue(profile.retainedSamples) ?? 0;
  const stackTotal = numberValue(profile.stackTotalSamples) ?? 0;
  const shareBase = Math.max(retained, stackTotal, total);
  const functions = new Map();
  for (const row of rows) {
    const seen = new Set();
    for (const frame of row.frames) {
      const identity = profileFrameIdentity(frame);
      let stat = functions.get(identity);
      if (!stat) {
        stat = {
          key: identity,
          label: profileFrameLabel(frame),
          location: profileFrameLocation(frame),
          selfSamples: 0,
          inclusiveSamples: 0,
        };
        functions.set(identity, stat);
      }
      if (!seen.has(identity)) {
        stat.inclusiveSamples += row.count;
        seen.add(identity);
      }
    }
    if (row.frames.length) {
      const leaf = functions.get(profileFrameIdentity(row.frames[0]));
      if (leaf) leaf.selfSamples += row.count;
    }
  }
  const tree = buildProfileCallTree(rows, shareBase);
  const bottomUp = buildProfileBottomUp(rows, shareBase);
  const flame = layoutProfileFlame(tree.root, shareBase);
  return { rows, total, shareBase, functions, tree, bottomUp, flame };
}

function profileModelNotes(profile, model) {
  const notes = [];
  if (!model.shareBase) return notes;
  notes.push(`Shares are relative to ${formatCount(model.shareBase)} retained samples, not the requested rate.`);
  if (profile.stacksTruncated) {
    notes.push(`The runtime returned only its ${formatCount(profile.stacks.length)} hottest stack groups; samples outside them are not represented.`);
  }
  if (profile.stacksOmitted) {
    notes.push(`The client read the ${MAX_PROFILE_STACKS} hottest of ${formatCount(profile.stacks.length + profile.stacksOmitted)} distinct stacks returned by the runtime.`);
  }
  if (!profile.stacksTruncated && !profile.stacksOmitted && model.shareBase > model.total) {
    notes.push(`${formatCount(model.shareBase - model.total)} retained samples are not covered by the returned stacks.`);
  }
  if (profile.samplesTruncated) {
    notes.push(`Only the newest ${MAX_PROFILE_SAMPLES} samples were read; ${formatCount(profile.samplesOmitted)} older samples in this payload were skipped.`);
  }
  if (profile.samplesDropped) {
    notes.push(`${formatCount(profile.samplesDropped)} samples were overwritten by the sampler ring before the profile stopped.`);
  }
  return notes;
}

function profileFilterNeedle(profile) {
  return String(profile.filter || "").trim().toLowerCase();
}

function profileTextMatches(needle, ...values) {
  if (!needle) return true;
  return values.some(value => String(value ?? "").toLowerCase().includes(needle));
}

function profileNodeMatches(node, needle) {
  return profileTextMatches(needle, node.label, node.location, node.frame?.function, node.frame?.file);
}

function profileEmptyMessage(profile, view) {
  if (["sampling", "starting"].includes(profile.status)) {
    return "Sampling is running. Stacks are ranked when the sampler stops; no stack is synthesized while it runs.";
  }
  if (profile.status === "stopping") return "Stopping the sampler. The profile views fill in when it returns.";
  if (["unsupported", "unavailable"].includes(profile.status)) {
    return profile.reason || "The runtime reported that CPU profiling cannot run here.";
  }
  if (profile.status === "complete") {
    if (view === "flame-chart") return "The retained profile has no stack samples to plot.";
    return "The retained profile has no stack samples.";
  }
  return "Start a CPU profile to build this view from real samples.";
}

function profileVisibleTreeRows(root, expanded, needle) {
  const rows = [];
  let truncated = false;
  const visible = new Map();
  const hasVisible = node => {
    if (visible.has(node.key)) return visible.get(node.key);
    let result = profileNodeMatches(node, needle);
    if (!result) {
      for (const child of node.childrenArray) {
        if (hasVisible(child)) {
          result = true;
          break;
        }
      }
    }
    visible.set(node.key, result);
    return result;
  };
  const walk = (node, depth) => {
    for (const child of node.childrenArray) {
      if (!hasVisible(child)) continue;
      if (rows.length >= MAX_PROFILE_TREE_ROWS) {
        truncated = true;
        continue;
      }
      const isExpanded = needle ? true : expanded.has(child.key);
      rows.push({ node: child, depth, expanded: isExpanded });
      if (child.childrenArray.length && isExpanded) walk(child, depth + 1);
    }
  };
  walk(root, 0);
  return { rows, truncated };
}

function profileCallerRows(group, expanded, needle) {
  const rows = [];
  let truncated = false;
  const visible = new Map();
  const hasVisible = node => {
    if (visible.has(node.key)) return visible.get(node.key);
    let result = profileNodeMatches(node, needle);
    if (!result) {
      for (const caller of node.callersArray) {
        if (hasVisible(caller)) {
          result = true;
          break;
        }
      }
    }
    visible.set(node.key, result);
    return result;
  };
  const walk = (node, depth) => {
    for (const caller of node.callersArray) {
      if (!hasVisible(caller)) continue;
      if (rows.length >= MAX_PROFILE_TREE_ROWS) {
        truncated = true;
        continue;
      }
      const isExpanded = needle ? true : expanded.has(caller.key);
      rows.push({ node: caller, depth, expanded: isExpanded });
      if (caller.callersArray.length && isExpanded) walk(caller, depth + 1);
    }
  };
  walk(group, 0);
  return { rows, truncated };
}

function profileViewMeta(profile) {
  const parts = [
    `${formatCount(profile.stacks.length)} stacks`,
    `${formatCount(profile.stackTotalSamples || profile.retainedSamples)} samples`,
  ];
  const needle = profileFilterNeedle(profile);
  if (needle) parts.push(`filter “${String(profile.filter).trim()}”`);
  return parts.join(" · ");
}

function listProfileNotes(notes) {
  return notes.filter(Boolean).map(note => el("div", { class: "perf-note", text: note }));
}

function profileSelect(profile, selection) {
  if (!selection) return;
  if (profile.selection && profile.selection.key === selection.key) profile.selection = null;
  else profile.selection = { key: selection.key, label: selection.label, location: selection.location || "" };
  refreshProfileView(profile);
}

function toggleProfileNode(profile, node) {
  const expandable = Boolean(node.childrenArray?.length || node.callersArray?.length);
  if (!expandable) return;
  if (profile.expanded.has(node.key)) profile.expanded.delete(node.key);
  else profile.expanded.add(node.key);
  refreshProfileView(profile);
}

function expandProfileAll(profile) {
  const model = buildProfileModel(profile);
  const expanded = new Set();
  const visit = (node, key) => {
    const children = node[key] || [];
    if (children.length) expanded.add(node.key);
    for (const child of children) {
      if (expanded.size >= MAX_PROFILE_TREE_NODES) return;
      visit(child, key);
    }
  };
  if (profile.view === "bottom-up") {
    for (const group of model.bottomUp.groups) visit(group, "callersArray");
  } else {
    visit(model.tree.root, "childrenArray");
  }
  profile.expanded = expanded;
  refreshProfileView(profile);
}

function collapseProfileAll(profile) {
  profile.expanded = new Set();
  refreshProfileView(profile);
}

function setProfileView(profile, view) {
  if (!PROFILE_VIEWS.includes(view) || profile.view === view) return;
  profile.view = view;
  renderPerformance();
}

function profileViewTabKeydown(profile, view, event) {
  const index = PROFILE_VIEWS.indexOf(view);
  let next = null;
  if (event.key === "ArrowRight") next = PROFILE_VIEWS[(index + 1) % PROFILE_VIEWS.length];
  else if (event.key === "ArrowLeft") next = PROFILE_VIEWS[(index - 1 + PROFILE_VIEWS.length) % PROFILE_VIEWS.length];
  else if (event.key === "Home") next = PROFILE_VIEWS[0];
  else if (event.key === "End") next = PROFILE_VIEWS[PROFILE_VIEWS.length - 1];
  if (!next) return;
  event.preventDefault();
  profile.view = next;
  renderPerformance();
  $(`profile-view-tab-${next}`)?.focus();
}

function profileSelectionNode(profile, model) {
  const host = el("div", { class: "profile-selection", props: { id: "profile-selection" }, attrs: { "aria-live": "polite" } });
  const selection = profile.selection;
  host.hidden = !selection;
  if (!selection) return host;
  const stat = model.functions.get(selection.key);
  const label = stat?.label || selection.label || "unknown frame";
  const location = stat?.location || selection.location || "";
  const detail = stat
    ? `self ${formatCount(stat.selfSamples)} (${profileSharePercent(stat.selfSamples, model.shareBase)}) · inclusive ${formatCount(stat.inclusiveSamples)} (${profileSharePercent(stat.inclusiveSamples, model.shareBase)})`
    : "not present in the returned stacks";
  host.append(
    el("span", { class: "profile-selection-label", text: label, attrs: { title: label } }),
    location ? el("span", { class: "profile-selection-location", text: location, attrs: { title: location } }) : null,
    el("span", { class: "profile-selection-stats", text: detail }),
    el("button", {
      class: "tool-button small",
      props: { type: "button", id: "profile-selection-clear" },
      text: "Clear",
      on: {
        click: () => {
          profile.selection = null;
          refreshProfileView(profile);
        },
      },
    }),
  );
  return host;
}

function updateProfileSelection(profile, model) {
  const current = $("profile-selection");
  if (!current) return;
  current.replaceWith(profileSelectionNode(profile, model));
}

function refreshProfileView(profile = state.performance.profile) {
  const model = buildProfileModel(profile);
  const body = $("profile-view-body");
  if (body) {
    clear(body);
    body.append(...renderProfileViewBody(profile, model));
  }
  updateProfileSelection(profile, model);
  const meta = $("profile-view-meta");
  if (meta) meta.textContent = profileViewMeta(profile);
}

function renderProfileTreeHead() {
  return el("div", { class: "profile-tree-head", attrs: { "aria-hidden": "true" } },
    el("span", { text: "" }),
    el("span", { text: "Function" }),
    el("span", { text: "Location" }),
    el("span", { class: "num", text: "Self" }),
    el("span", { class: "num", text: "Samples" }),
    el("span", { class: "num", text: "Self %" }),
    el("span", { class: "num", text: "%" }),
  );
}

function renderProfileTreeRow(profile, entry, needle) {
  const node = entry.node;
  const expandable = Boolean(node.childrenArray.length);
  const selected = profile.selection?.key === node.identity;
  const matched = Boolean(needle) && profileNodeMatches(node, needle);
  const row = el("div", {
    class: `profile-tree-row${selected ? " is-selected" : ""}${matched ? " is-match" : ""}`,
    attrs: {
      role: "treeitem",
      "aria-level": String(entry.depth + 1),
      "aria-selected": selected ? "true" : "false",
      "aria-expanded": expandable ? String(entry.expanded) : null,
      tabindex: "0",
      style: `--profile-indent:${entry.depth * 14}px`,
    },
    on: {
      click: () => profileSelect(profile, { key: node.identity, label: node.label, location: node.location }),
      keydown: event => {
        if (event.target !== row) return;
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          profileSelect(profile, { key: node.identity, label: node.label, location: node.location });
        } else if (event.key === "ArrowRight" && expandable && !entry.expanded) {
          event.preventDefault();
          profile.expanded.add(node.key);
          refreshProfileView(profile);
        } else if (event.key === "ArrowLeft" && expandable && entry.expanded) {
          event.preventDefault();
          profile.expanded.delete(node.key);
          refreshProfileView(profile);
        }
      },
    },
  });
  row.append(
    expandable
      ? el("button", {
        class: "profile-twisty",
        props: { type: "button" },
        attrs: { "aria-label": `${entry.expanded ? "Collapse" : "Expand"} ${node.label}`, "aria-expanded": String(entry.expanded) },
        text: entry.expanded ? "▾" : "▸",
        on: {
          click: event => {
            event.stopPropagation();
            toggleProfileNode(profile, node);
          },
        },
      })
      : el("span", { class: "profile-twisty is-empty", attrs: { "aria-hidden": "true" } }),
    el("span", { class: "profile-fn", text: node.label, attrs: { title: node.label } }),
    el("span", { class: "profile-location", text: node.location || "—", attrs: { title: node.location || "" } }),
    el("span", { class: "num", text: formatCount(node.selfSamples) }),
    el("span", { class: "num", text: formatCount(node.samples) }),
    el("span", { class: "num", text: node.selfPercent === null ? "—" : `${node.selfPercent.toFixed(1)}%` }),
    el("span", { class: "num", text: node.percent === null ? "—" : `${node.percent.toFixed(1)}%` }),
  );
  return row;
}

function renderProfileCallTree(profile, model) {
  const tree = model.tree;
  if (!model.rows.length) return [emptyInline(profileEmptyMessage(profile, "call-tree"))];
  const needle = profileFilterNeedle(profile);
  const { rows, truncated } = profileVisibleTreeRows(tree.root, profile.expanded, needle);
  const content = [renderProfileTreeHead()];
  content.push(el("div", { class: "profile-tree-root" },
    el("span", { class: "profile-fn", text: tree.root.label }),
    el("span", { class: "num", text: `${formatCount(tree.root.samples)} samples` }),
    el("span", { class: "num", text: profileSharePercent(tree.root.samples, model.shareBase) }),
  ));
  if (!rows.length) {
    content.push(emptyInline(needle ? `No function matches “${String(profile.filter).trim()}”.` : profileEmptyMessage(profile, "call-tree")));
  } else {
    const treeHost = el("div", { class: "profile-tree", attrs: { role: "tree", "aria-label": "CPU profile call tree" } });
    for (const entry of rows) treeHost.append(renderProfileTreeRow(profile, entry, needle));
    content.push(treeHost);
  }
  const notes = profileModelNotes(profile, model);
  if (tree.unattributedSamples) {
    notes.push(`${formatCount(tree.unattributedSamples)} sample${tree.unattributedSamples === 1 ? "" : "s"} from ${formatCount(tree.unattributedStacks)} stack${tree.unattributedStacks === 1 ? "" : "s"} have no captured frames and are not attributed to a function.`);
  }
  if (tree.depthCappedSamples) {
    notes.push(`${formatCount(tree.depthCappedSamples)} sample${tree.depthCappedSamples === 1 ? "" : "s"} continue${tree.depthCappedSamples === 1 ? "s" : ""} deeper than the ${tree.cappedDepth}-frame tree cap; those deeper frames are not shown.`);
  }
  if (tree.branchCappedSamples) {
    notes.push(`${formatCount(tree.branchCappedSamples)} sample${tree.branchCappedSamples === 1 ? "" : "s"} reached a branch beyond the ${MAX_PROFILE_TREE_CHILDREN}-child or ${MAX_PROFILE_TREE_NODES}-node cap; those branches stay under their parent without expanded children.`);
  }
  if (truncated) {
    notes.push(`Only the first ${MAX_PROFILE_TREE_ROWS} visible rows are drawn; collapse a branch or filter to reach the rest.`);
  }
  if (needle) notes.push("Filtering hides rows only; each node still counts every retained sample in its path.");
  content.push(...listProfileNotes(notes));
  return content;
}

