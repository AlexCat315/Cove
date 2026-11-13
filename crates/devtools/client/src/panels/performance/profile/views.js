import { clear, el, fmtMs, fmtNumber, numberValue } from "../../../core/format.js";
import { dom, MAX_HOTSPOTS, MAX_PROFILE_BOTTOM_CALLERS, MAX_PROFILE_BOTTOM_GROUPS, MAX_PROFILE_FLAME_BARS, MAX_PROFILE_STACK_FRAMES, MAX_PROFILE_TREE_DEPTH, MAX_PROFILE_TREE_ROWS, PROFILE_FLAME_MIN_PERCENT, PROFILE_METHOD_SORT_KEYS, PROFILE_VIEW_LABELS, state } from "../../../core/globals.js";
import { errorMessage, targetById } from "../../../core/session.js";
import { emptyInline, errorInline, showState } from "../../../core/ui-helpers.js";
import { snapshotBanner } from "../frames.js";
import { renderPerformance } from "../overview.js";
import { returnToLive } from "../snapshots.js";
import { PROFILE_VIEWS } from "../../../core/globals.js";
import { loadHotspots, renderProfileControls, renderProfileStats, startProfile } from "./control.js";
import { fmtSeconds, formatCount, formatPercent, loadProfile, profilePlatformName, profilePlatformUnsupported } from "./model.js";
import { buildProfileModel, collapseProfileAll, expandProfileAll, listProfileNotes, profileCallerRows, profileEmptyMessage, profileFilterNeedle, profileFrameIdentity, profileFrameKeyFromHotspot, profileFrameLabel, profileFrameLocation, profileModelNotes, profileNodeMatches, profileSelect, profileSelectionNode, profileSharePercent, profileTextMatches, profileViewMeta, profileViewTabKeydown, refreshProfileView, renderProfileCallTree, renderProfileTreeHead, setProfileView, toggleProfileNode } from "./trees.js";
function renderProfileCallerRow(profile, entry, needle) {
  const node = entry.node;
  const expandable = Boolean(node.callersArray.length);
  const selected = profile.selection?.key === node.identity;
  const matched = Boolean(needle) && profileNodeMatches(node, needle);
  const row = el("div", {
    class: `profile-tree-row is-caller${selected ? " is-selected" : ""}${matched ? " is-match" : ""}`,
    attrs: {
      role: "treeitem",
      "aria-level": String(entry.depth + 2),
      "aria-selected": selected ? "true" : "false",
      "aria-expanded": expandable ? String(entry.expanded) : null,
      tabindex: "0",
      style: `--profile-indent:${(entry.depth + 1) * 14}px`,
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
        attrs: { "aria-label": `${entry.expanded ? "Collapse" : "Expand"} caller ${node.label}`, "aria-expanded": String(entry.expanded) },
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
    el("span", { class: "num", text: "—" }),
    el("span", { class: "num", text: formatCount(node.samples) }),
    el("span", { class: "num", text: "—" }),
    el("span", { class: "num", text: node.percent === null ? "—" : `${node.percent.toFixed(1)}%` }),
  );
  return row;
}

function expandableBottomGroup(group) {
  return Boolean(group.callersArray.length);
}

function renderProfileBottomUp(profile, model) {
  const bottom = model.bottomUp;
  if (!model.rows.length) return [emptyInline(profileEmptyMessage(profile, "bottom-up"))];
  const needle = profileFilterNeedle(profile);
  const groups = bottom.groups.filter(group => {
    if (!needle) return true;
    if (profileNodeMatches(group, needle)) return true;
    return profileCallerRows(group, profile.expanded, needle).rows.some(row => profileNodeMatches(row.node, needle));
  });
  const content = [];
  if (!groups.length) {
    content.push(emptyInline(needle ? `No function matches “${String(profile.filter).trim()}”.` : profileEmptyMessage(profile, "bottom-up")));
    content.push(...listProfileNotes(profileModelNotes(profile, model)));
    return content;
  }
  content.push(renderProfileTreeHead());
  let truncatedRows = false;
  const list = el("div", { class: "profile-tree", attrs: { role: "tree", "aria-label": "CPU profile bottom-up tree" } });
  for (const group of groups) {
    const expanded = needle ? true : profile.expanded.has(group.key);
    const selected = profile.selection?.key === group.identity;
    const groupRow = el("div", {
      class: `profile-tree-row is-group${selected ? " is-selected" : ""}${needle && profileNodeMatches(group, needle) ? " is-match" : ""}`,
      attrs: {
        role: "treeitem",
        "aria-level": "1",
        "aria-selected": selected ? "true" : "false",
        "aria-expanded": group.callersArray.length ? String(expanded) : null,
        tabindex: "0",
      },
      on: {
        click: () => profileSelect(profile, { key: group.identity, label: group.label, location: group.location }),
        keydown: event => {
          if (event.target !== groupRow) return;
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            profileSelect(profile, { key: group.identity, label: group.label, location: group.location });
          } else if (event.key === "ArrowRight" && expandableBottomGroup(group) && !expanded) {
            event.preventDefault();
            profile.expanded.add(group.key);
            refreshProfileView(profile);
          } else if (event.key === "ArrowLeft" && expandableBottomGroup(group) && expanded) {
            event.preventDefault();
            profile.expanded.delete(group.key);
            refreshProfileView(profile);
          }
        },
      },
    });
    groupRow.append(
      group.callersArray.length
        ? el("button", {
          class: "profile-twisty",
          props: { type: "button" },
          attrs: { "aria-label": `${expanded ? "Collapse" : "Expand"} callers of ${group.label}`, "aria-expanded": String(expanded) },
          text: expanded ? "▾" : "▸",
          on: {
            click: event => {
              event.stopPropagation();
              toggleProfileNode(profile, group);
            },
          },
        })
        : el("span", { class: "profile-twisty is-empty", attrs: { "aria-hidden": "true" } }),
      el("span", { class: "profile-fn", text: group.label, attrs: { title: group.label } }),
      el("span", { class: "profile-location", text: group.location || "—", attrs: { title: group.location || "" } }),
      el("span", { class: "num", text: formatCount(group.samples) }),
      el("span", { class: "num", text: formatCount(group.samples) }),
      el("span", { class: "num", text: group.percent === null ? "—" : `${group.percent.toFixed(1)}%` }),
      el("span", { class: "num", text: group.percent === null ? "—" : `${group.percent.toFixed(1)}%` }),
    );
    list.append(groupRow);
    if (group.callersArray.length && expanded) {
      const callers = profileCallerRows(group, profile.expanded, needle);
      if (callers.truncated) truncatedRows = true;
      for (const entry of callers.rows) list.append(renderProfileCallerRow(profile, entry, needle));
    }
  }
  content.push(list);
  const notes = profileModelNotes(profile, model);
  notes.push("Each leaf row counts samples whose innermost frame is that function; expanding it walks outward through its callers.");
  if (bottom.unattributedSamples) {
    notes.push(`${formatCount(bottom.unattributedSamples)} sample${bottom.unattributedSamples === 1 ? "" : "s"} from ${formatCount(bottom.unattributedStacks)} stack${bottom.unattributedStacks === 1 ? "" : "s"} have no captured frames and are not attributed to a leaf function.`);
  }
  if (bottom.cappedSamples) {
    notes.push(`${formatCount(bottom.cappedSamples)} sample${bottom.cappedSamples === 1 ? "" : "s"} exceed${bottom.cappedSamples === 1 ? "s" : ""} the ${MAX_PROFILE_BOTTOM_CALLERS}-caller or ${MAX_PROFILE_TREE_DEPTH}-frame bottom-up caps and their deeper callers are not shown.`);
  }
  if (bottom.omittedGroups) {
    notes.push(`The ${MAX_PROFILE_BOTTOM_GROUPS} leaf functions with the most samples are shown; ${formatCount(bottom.omittedGroups)} more are omitted by the client cap.`);
  }
  if (truncatedRows) notes.push(`Caller rows stop at ${MAX_PROFILE_TREE_ROWS} visible rows; collapse a group or filter to reach the rest.`);
  if (needle) notes.push("Filtering hides rows only; sample counts still cover every retained sample.");
  content.push(...listProfileNotes(notes));
  return content;
}

function renderProfileFlameChart(profile, model) {
  const flame = model.flame;
  if (!model.rows.length || !model.shareBase) return [emptyInline(profileEmptyMessage(profile, "flame-chart"))];
  const needle = profileFilterNeedle(profile);
  let matches = 0;
  const plot = el("div", { class: "profile-flame-plot" });
  for (let depth = 0; depth < flame.rows.length; depth += 1) {
    const track = el("div", { class: "profile-flame-track" });
    for (const bar of flame.rows[depth] || []) {
      const node = bar.node;
      const matched = Boolean(needle) && profileNodeMatches(node, needle);
      if (matched) matches += 1;
      const selected = profile.selection?.key === node.identity;
      const width = Math.min(100, Math.max(0, bar.width));
      const left = Math.min(100, Math.max(0, bar.start));
      const label = node.root ? "All samples" : node.label;
      const title = `${label}${node.location ? ` · ${node.location}` : ""} · ${formatCount(node.samples)} samples · ${profileSharePercent(node.samples, model.shareBase)}${node.selfSamples ? ` · self ${formatCount(node.selfSamples)} (${profileSharePercent(node.selfSamples, model.shareBase)})` : ""}`;
      const classes = ["profile-flame-bar"];
      if (selected) classes.push("is-selected");
      if (matched) classes.push("is-match");
      if (needle && !matched) classes.push("is-dimmed");
      const button = el("button", {
        class: classes.join(" "),
        props: { type: "button" },
        attrs: {
          style: `left:${left}%;width:${width}%`,
          title,
          "aria-label": title,
          "data-profile-frame": node.identity,
        },
        on: {
          click: () => profileSelect(profile, { key: node.identity, label, location: node.location }),
        },
      });
      if (width >= 12) {
        button.append(el("span", { class: "profile-flame-label", text: label }));
        if (width >= 26 && node.location) button.append(el("span", { class: "profile-flame-location", text: node.location }));
      }
      track.append(button);
    }
    plot.append(el("div", { class: "profile-flame-row" },
      el("span", { class: "profile-flame-depth", text: depth === 0 ? "root" : `d${depth}` }),
      track,
    ));
  }
  const chart = el("div", {
    class: "profile-flame-scroll",
    attrs: { role: "group", "aria-label": `CPU flame chart: ${flame.bars} bars across ${flame.depth} depth rows. Width is the share of retained samples.` },
  }, plot);
  const notes = profileModelNotes(profile, model);
  notes.push("The flame chart is inverted: the outermost call is the full-width top bar, children sit directly below their caller, and width is the share of retained samples.");
  if (flame.depthCapped) notes.push(`Branches deeper than ${flame.maxDepth} levels are not drawn.`);
  if (flame.barCapped) notes.push(`Only the first ${MAX_PROFILE_FLAME_BARS} bars are drawn; branches beyond the cap are not shown.`);
  if (flame.thinSkipped) notes.push(`${formatCount(flame.thinSkipped)} branches below ${PROFILE_FLAME_MIN_PERCENT}% share are not drawn; their samples remain counted in the parent bar.`);
  if (needle) {
    notes.push(matches
      ? `${formatCount(matches)} bar${matches === 1 ? "" : "s"} ${matches === 1 ? "matches" : "match"} “${String(profile.filter).trim()}”.`
      : `No bar matches “${String(profile.filter).trim()}”; every bar is dimmed.`);
  }
  return [chart, ...listProfileNotes(notes)];
}

function profileStacksShareBase(profile) {
  return profile.retainedSamples || profile.stackTotalSamples || profile.stacks.reduce((sum, row) => sum + (numberValue(row.count) ?? 0), 0);
}

function renderProfileStacksView(profile, model) {
  const rows = profile.stacks;
  if (!rows.length) return [emptyInline(profileEmptyMessage(profile, "stacks"))];
  const needle = profileFilterNeedle(profile);
  const base = model.shareBase || profileStacksShareBase(profile);
  const visible = needle
    ? rows.filter(row => row.frames.some(frame => profileTextMatches(needle, frame.function, frame.label, frame.file)))
    : rows;
  const content = [];
  if (!visible.length) {
    content.push(emptyInline(`No stack contains “${String(profile.filter).trim()}”.`));
    content.push(...listProfileNotes(profileModelNotes(profile, model)));
    return content;
  }
  const table = el("table", { class: "data-table profile-stacks-table" },
    el("thead", {}, el("tr", {},
      el("th", { class: "num", text: "#" }),
      el("th", { class: "num", text: "Samples" }),
      el("th", { class: "num", text: "Share" }),
      el("th", { text: "Stack (innermost first)" }),
    )),
    el("tbody", {}, ...visible.map((row, index) => {
      const share = base ? row.count / base : null;
      const selected = profile.selection && row.frames.some(frame => profileFrameIdentity(frame) === profile.selection.key);
      const cell = el("td", { class: "profile-stack-cell" });
      const list = el("div", { class: "profile-stack-frames", attrs: { title: row.frames.map(frame => frame.label).join("\n") } });
      if (!row.frames.length) {
        list.append(el("div", { class: "profile-stack-more", text: "No frames captured for these samples" }));
      }
      for (const [frameIndex, frame] of row.frames.slice(0, MAX_PROFILE_STACK_FRAMES).entries()) {
        const frameSelected = profile.selection?.key === profileFrameIdentity(frame);
        list.append(el("div", {
          class: `profile-stack-frame ${frameIndex === 0 ? "is-leaf" : "is-caller"}${frameSelected ? " is-selected" : ""}`,
          attrs: { title: profileFrameLocation(frame) || frame.label },
          text: frame.label,
          on: {
            click: event => {
              event.stopPropagation();
              profileSelect(profile, { key: profileFrameIdentity(frame), label: profileFrameLabel(frame), location: profileFrameLocation(frame) });
            },
          },
        }));
      }
      if (row.frames.length > MAX_PROFILE_STACK_FRAMES) {
        list.append(el("div", { class: "profile-stack-more", text: `+${row.frames.length - MAX_PROFILE_STACK_FRAMES} deeper frames` }));
      }
      cell.append(list);
      return el("tr", {
        class: selected ? "is-selected" : "",
        on: {
          click: () => {
            const leaf = row.frames[0];
            if (leaf) profileSelect(profile, { key: profileFrameIdentity(leaf), label: profileFrameLabel(leaf), location: profileFrameLocation(leaf) });
          },
        },
      },
      el("td", { class: "num mono-cell", text: String(index + 1) }),
      el("td", { class: "num", text: formatCount(row.count) }),
      el("td", { class: "num", text: share === null ? "—" : formatPercent(share) }),
      cell,
      );
    })),
  );
  content.push(el("div", { class: "table-wrap" }, table));
  const notes = profileModelNotes(profile, model);
  notes.push("Stacks are grouped by identical frame sequence. The first frame is the innermost (leaf); deeper callers follow.");
  content.push(...listProfileNotes(notes));
  return content;
}

function sortedHotspotEntries(entries, sort) {
  const key = PROFILE_METHOD_SORT_KEYS.includes(sort.key) ? sort.key : "selfSamples";
  const dir = sort.dir === 1 ? 1 : -1;
  const read = entry => {
    switch (key) {
      case "label": return entry.function || entry.label || "";
      case "location": return entry.file ? `${entry.file}:${entry.line ?? ""}` : "";
      case "selfSamples": return entry.selfSamples;
      case "inclusiveSamples": return entry.inclusiveSamples;
      case "selfMs": return entry.selfMs;
      case "inclusiveMs": return entry.inclusiveMs;
      case "selfPercent": return entry.selfPercent;
      case "inclusivePercent": return entry.inclusivePercent;
      default: return entry.selfSamples;
    }
  };
  return [...entries].sort((left, right) => {
    const a = read(left);
    const b = read(right);
    if (typeof a === "string" || typeof b === "string") {
      return String(a ?? "").localeCompare(String(b ?? "")) * dir || String(left.label).localeCompare(String(right.label));
    }
    const leftValue = numberValue(a);
    const rightValue = numberValue(b);
    if (leftValue === null && rightValue === null) return String(left.label).localeCompare(String(right.label));
    if (leftValue === null) return 1;
    if (rightValue === null) return -1;
    return (leftValue - rightValue) * dir || String(left.label).localeCompare(String(right.label));
  });
}

function profileSortHeader(profile, label, key, className = "") {
  const active = profile.methodSort.key === key;
  const direction = active ? profile.methodSort.dir : -1;
  return el("th", {
    class: className,
    attrs: { "aria-sort": active ? (direction === 1 ? "ascending" : "descending") : "none" },
  }, el("button", {
    class: `profile-sort${active ? " is-active" : ""}`,
    props: { type: "button" },
    attrs: { title: `Sort methods by ${label.toLowerCase()}` },
    text: `${label}${active ? (direction === 1 ? " ↑" : " ↓") : ""}`,
    on: {
      click: () => {
        if (profile.methodSort.key === key) profile.methodSort.dir = direction === 1 ? -1 : 1;
        else profile.methodSort = { key, dir: key === "label" || key === "location" ? 1 : -1 };
        renderPerformance();
      },
    },
  }));
}

function renderMethodsView(profile, model) {
  const hotspot = profile.hotspots;
  const content = [];
  if (hotspot.loading) {
    content.push(el("div", { class: "inline-spinner" }, el("span", { class: "spinner" }), "Reading hot spots…"));
    return content;
  }
  if (hotspot.error) {
    content.push(errorInline(errorMessage(hotspot.error)));
    content.push(el("button", {
      class: "tool-button",
      props: { type: "button" },
      text: "Retry",
      on: { click: () => loadHotspots({ quiet: true }) },
    }));
    return content;
  }
  if (!hotspot.data) {
    content.push(emptyInline(["sampling", "starting"].includes(profile.status)
      ? "Stop the profile to rank hot functions."
      : profile.status === "idle"
        ? "No hot spots yet. Start a profile to rank self and inclusive CPU time."
        : "No hot-spot data. Refresh to read query_ui_hotspots."));
    return content;
  }
  if (hotspot.data.status === "unsupported" || hotspot.data.status === "unavailable") {
    content.push(emptyInline(hotspot.data.reason || "The runtime reported no hot-spot data."));
    return content;
  }
  if (!hotspot.data.entries.length) {
    content.push(emptyInline(profile.status === "complete"
      ? "No hot functions were computed from the retained samples."
      : "No hot functions yet."));
    return content;
  }
  const needle = profileFilterNeedle(profile);
  const all = hotspot.data.entries;
  const filtered = needle
    ? all.filter(entry => profileTextMatches(needle, entry.function, entry.label, entry.file, entry.line))
    : all;
  if (!filtered.length) {
    content.push(emptyInline(`No function matches “${String(profile.filter).trim()}”.`));
    content.push(el("div", { class: "perf-note", text: `${formatCount(all.length)} methods are in the table; the filter hides all of them.` }));
    return content;
  }
  const sorted = sortedHotspotEntries(filtered, profile.methodSort);
  const table = el("table", { class: "data-table profile-methods-table" },
    el("thead", {}, el("tr", {},
      profileSortHeader(profile, "Function", "label"),
      profileSortHeader(profile, "Location", "location"),
      profileSortHeader(profile, "Self time", "selfMs", "num"),
      profileSortHeader(profile, "Inclusive time", "inclusiveMs", "num"),
      profileSortHeader(profile, "Self %", "selfPercent", "num"),
      profileSortHeader(profile, "Inclusive %", "inclusivePercent", "num"),
      profileSortHeader(profile, "Self samples", "selfSamples", "num"),
      profileSortHeader(profile, "Inclusive samples", "inclusiveSamples", "num"),
    )),
    el("tbody", {}, ...sorted.map(entry => {
      const location = entry.file ? `${entry.file}${entry.line === null ? "" : `:${Math.trunc(entry.line)}`}` : "—";
      const key = profileFrameKeyFromHotspot(entry);
      const selected = profile.selection?.key === key;
      return el("tr", {
        class: selected ? "is-selected" : "",
        on: {
          click: () => profileSelect(profile, { key, label: entry.function || entry.label, location: entry.file ? location : "" }),
        },
      },
      el("td", { attrs: { title: entry.label } }, el("span", { text: entry.function })),
      el("td", { class: "profile-location", text: location, attrs: { title: location } }),
      el("td", { class: "num", text: entry.selfMs === null ? "—" : fmtMs(entry.selfMs, 2) }),
      el("td", { class: "num", text: entry.inclusiveMs === null ? "—" : fmtMs(entry.inclusiveMs, 2) }),
      el("td", { class: "num", text: entry.selfPercent === null ? "—" : `${entry.selfPercent.toFixed(1)}%` }),
      el("td", { class: "num", text: entry.inclusivePercent === null ? "—" : `${entry.inclusivePercent.toFixed(1)}%` }),
      el("td", { class: "num mono-cell", text: entry.selfSamples === null ? "—" : formatCount(entry.selfSamples) }),
      el("td", { class: "num mono-cell", text: entry.inclusiveSamples === null ? "—" : formatCount(entry.inclusiveSamples) }),
      );
    })),
  );
  content.push(el("div", { class: "table-wrap" }, table));
  const notes = [];
  notes.push("Self counts only samples whose innermost frame is this function; inclusive also counts samples where it appears as a caller.");
  if (hotspot.data.totalSamples !== null) notes.push(`Computed from ${formatCount(hotspot.data.totalSamples)} retained samples over ${fmtSeconds(hotspot.data.durationMs)}.`);
  if (needle) notes.push(`${formatCount(filtered.length)} of ${formatCount(all.length)} methods match “${String(profile.filter).trim()}”.`);
  if (hotspot.data.omitted) notes.push(`The runtime returned the ${MAX_HOTSPOTS} hottest functions; ${formatCount(hotspot.data.omitted)} more were omitted by the client cap.`);
  else if (hotspot.data.truncated) notes.push(`The runtime's bounded answer contains the ${formatCount(all.length)} hottest functions; more were omitted by the runtime.`);
  if (hotspot.data.reason) notes.push(hotspot.data.reason);
  content.push(...listProfileNotes(notes));
  return content;
}

function renderProfileViewTabs(profile) {
  return el("div", { class: "profile-view-tabs", attrs: { role: "tablist", "aria-label": "CPU profile views" } },
    ...PROFILE_VIEWS.map(view => {
      const selected = profile.view === view;
      return el("button", {
        class: "profile-view-tab",
        props: { type: "button", id: `profile-view-tab-${view}` },
        attrs: {
          role: "tab",
          "aria-selected": String(selected),
          "aria-controls": "profile-view-body",
          tabindex: selected ? "0" : "-1",
        },
        text: PROFILE_VIEW_LABELS[view],
        on: {
          click: () => setProfileView(profile, view),
          keydown: event => profileViewTabKeydown(profile, view, event),
        },
      });
    }),
  );
}

function renderProfileViewToolbar(profile) {
  const treeView = profile.view === "call-tree" || profile.view === "bottom-up";
  const filter = el("input", {
    class: "profile-filter",
    props: { id: "profile-filter", type: "search", value: profile.filter, placeholder: "Filter functions", autocomplete: "off", spellcheck: false },
    attrs: { "aria-label": "Filter profile functions" },
    on: {
      input: () => {
        profile.filter = filter.value;
        refreshProfileView(profile);
      },
    },
  });
  return el("div", { class: "profile-view-toolbar" },
    filter,
    treeView
      ? el("button", { class: "tool-button small", props: { type: "button", id: "profile-expand-all" }, text: "Expand all", on: { click: () => expandProfileAll(profile) } })
      : null,
    treeView
      ? el("button", { class: "tool-button small", props: { type: "button", id: "profile-collapse-all" }, text: "Collapse all", on: { click: () => collapseProfileAll(profile) } })
      : null,
    el("span", { id: "profile-view-meta", class: "pane-meta", text: profileViewMeta(profile) }),
  );
}

export function renderProfileViewBody(profile, model) {
  switch (profile.view) {
    case "bottom-up": return renderProfileBottomUp(profile, model);
    case "flame-chart": return renderProfileFlameChart(profile, model);
    case "methods": return renderMethodsView(profile, model);
    case "stacks": return renderProfileStacksView(profile, model);
    default: return renderProfileCallTree(profile, model);
  }
}

function renderProfileViews(profile) {
  const model = buildProfileModel(profile);
  const body = el("div", { class: "profile-view-body", props: { id: "profile-view-body" }, attrs: { role: "tabpanel", "aria-labelledby": `profile-view-tab-${profile.view}` } });
  body.append(...renderProfileViewBody(profile, model));
  return el("div", { class: "panel-block profile-views" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Profile views" }),
      el("span", { class: "pane-meta", text: `${formatCount(profile.samplesTaken)} samples · ${formatCount(profile.retainedSamples || profile.stackTotalSamples)} retained` }),
    ),
    el("div", { class: "panel-block-body" },
      renderProfileViewTabs(profile),
      renderProfileViewToolbar(profile),
      profileSelectionNode(profile, model),
      body,
    ),
  );
}

export function renderProfilePanel() {
  const host = dom.performanceProfile;
  clear(host);
  const perf = state.performance;
  const profile = perf.profile;
  const target = targetById();
  if (perf.snapshot) {
    host.append(snapshotBanner());
    const block = el("div");
    showState(block, {
      icon: "timer",
      title: "CPU profiles are not in this snapshot",
      message: "Imported performance snapshots contain frame records and trace spans only. Return to live data to sample a CPU profile.",
      action: "Return to live",
      onAction: returnToLive,
    });
    host.append(block);
    return;
  }
  if (!target) {
    showState(host, { icon: "device", title: "No target", message: "Select a connected Cove runtime to sample CPU stacks." });
    return;
  }
  if (!state.caps.runtime) {
    showState(host, { icon: "timer", title: "CPU profiling is unavailable", message: "CPU sampling comes from a Cove runtime. Select a runtime target." });
    return;
  }
  const unsupported = profilePlatformUnsupported(target);
  if (unsupported) {
    showState(host, {
      icon: "timer",
      title: `CPU sampling is unavailable on ${profilePlatformName(target) === "windows" ? "Windows" : profilePlatformName(target) === "linux" ? "Linux" : profilePlatformName(target)}`,
      message: unsupported,
    });
    return;
  }
  if (!state.caps.profile) {
    const block = el("div");
    showState(block, {
      icon: "timer",
      title: "CPU profiling is unavailable",
      message: "This runtime does not advertise cpu_profile. If the controller was built with profiling, check support directly; no samples are synthesized when it is absent.",
      action: "Check support",
      onAction: () => loadProfile({ quiet: true, force: true }),
    });
    host.append(block);
    if (profile.error || profile.reason) host.append(errorInline(profile.reason || errorMessage(profile.error)));
    return;
  }
  if (["unsupported", "unavailable"].includes(profile.status) && !profile.loading) {
    const block = el("div");
    showState(block, {
      icon: "timer",
      title: profile.status === "unsupported" ? "CPU sampling is unsupported" : "CPU profile is unavailable",
      message: profile.reason || "The runtime reported that CPU profiling cannot run here. No samples are synthesized.",
      action: "Check support",
      onAction: () => loadProfile({ quiet: true, force: true }),
    });
    host.append(block);
    return;
  }
  host.append(renderProfileControls(profile));
  host.append(renderProfileStats(profile));
  const intervalMs = profile.intervalMs ?? (profile.hz > 0 ? 1000 / profile.hz : null);
  host.append(el("div", { class: "profile-note" },
    `Real stack samples from ${profile.thread || "the UI thread"} at ${fmtNumber(intervalMs, 2)} ms (${fmtNumber(profile.hz, 0)} Hz). macOS code and symbols are resolved from the process image; Linux and Windows report unsupported and never synthesize samples.`));
  if (profile.reason && ["unsupported", "unavailable"].includes(profile.status)) {
    host.append(emptyInline(profile.reason));
  }
  if (profile.loading && ["starting", "stopping"].includes(profile.status)) {
    host.append(el("div", { class: "inline-spinner" }, el("span", { class: "spinner" }), profile.status === "stopping" ? "Stopping and reading the profile…" : "Starting the sampler…"));
  }
  if (profile.error) {
    host.append(errorInline(errorMessage(profile.error)));
    if (profile.status !== "sampling") {
      host.append(el("button", {
        class: "tool-button",
        props: { type: "button" },
        text: "Retry",
        on: { click: () => loadProfile({ quiet: true }) },
      }));
    }
  }
  if (profile.status === "idle" && !profile.loading) {
    const block = el("div");
    showState(block, {
      icon: "timer",
      title: "No CPU profile",
      message: "Start a bounded sample of the UI thread, perform the operation, then stop. Samples are recorded by the in-process sampler, not inferred from frames.",
      action: "Start profile",
      onAction: startProfile,
    });
    host.append(block);
  }
  const hasProfileData = profile.samples.length > 0 || profile.stacks.length > 0;
  if (hasProfileData || ["sampling", "starting", "stopping", "complete"].includes(profile.status)) {
    host.append(renderProfileViews(profile));
  }
  host.append(el("div", { class: "perf-note", text: "CPU profiles are sampled from the live process. They do not require Screen Recording and are distinct from trace spans: a trace records named spans, a profile samples raw stacks." }));
}

