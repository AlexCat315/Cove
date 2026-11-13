import { el, fmtBytes, icon } from "../../core/format.js";
import { APP_SIZE_TREEMAP_HEIGHT, APP_SIZE_TREEMAP_LEGEND, APP_SIZE_TREEMAP_WIDTH, MAX_APP_SIZE_CONTAINMENT_GROUPS, state } from "../../core/globals.js";
import { formatCount } from "../performance/profile/model.js";
import { renderAppSize } from "./diff.js";
import { appSizeCoverageLabel, appSizeFocusTreemapRow, appSizePanel, appSizeSquarify, appSizeTreemapCellTitle, appSizeTreemapSource, appSizeTreemapTone, appSizeTreemapTruncation } from "./sections.js";
export function renderAppSizeTreemap(executable) {
  const source = appSizeTreemapSource(executable);
  const block = appSizePanel("Size treemap", `${fmtBytes(source.sum)} · ${formatCount(source.drawn.length)} ${source.label}`);
  block.id = "app-size-treemap-panel";
  const body = el("div", { class: "panel-block-body" });
  const toolbar = el("div", { class: "app-size-treemap-toolbar" });
  for (const option of [{ mode: "groups", label: "Crate groups" }, { mode: "sections", label: "Sections" }]) {
    const active = source.mode === option.mode;
    toolbar.append(el("button", {
      class: `tool-button small${active ? " is-active" : ""}`,
      attrs: {
        type: "button",
        "aria-pressed": active ? "true" : "false",
        "data-app-size-treemap-mode": option.mode,
        title: `Draw ${option.label.toLowerCase()} by reported bytes`,
      },
      text: option.label,
      on: {
        click: () => {
          if (state.appSize.treemapMode === option.mode) return;
          state.appSize.treemapMode = option.mode;
          state.appSize.treemapFocus = null;
          renderAppSize();
        },
      },
    }));
  }
  toolbar.append(el("span", { class: "pane-meta", text: "area = reported bytes" }));
  body.append(toolbar);

  if (!source.drawn.length) {
    const message = source.shown.length
      ? `${formatCount(source.shown.length)} ${source.label} were returned without positive reported bytes, so no treemap is drawn.`
      : `The analyzer reported no ${source.label} with bytes. No treemap is substituted.`;
    body.append(el("p", { class: "app-size-note", text: message }));
  } else {
    const canvas = el("div", {
      class: "app-size-treemap",
      attrs: {
        role: "group",
        "aria-label": `Proportional treemap of ${source.label} by reported bytes`,
      },
    });
    const rects = appSizeSquarify(source.drawn, APP_SIZE_TREEMAP_WIDTH, APP_SIZE_TREEMAP_HEIGHT);
    for (const rect of rects) {
      const item = rect.entry;
      const percent = (item.value / source.sum) * 100;
      const focused = state.appSize.treemapFocus;
      const active = Boolean(focused) && focused.mode === source.mode && focused.name === item.name;
      const title = appSizeTreemapCellTitle(source, item);
      const cell = el("button", {
        class: `app-size-treemap-cell${active ? " is-focused" : ""}`,
        attrs: {
          type: "button",
          style: [
            `left:${(rect.x / APP_SIZE_TREEMAP_WIDTH * 100).toFixed(4)}%`,
            `top:${(rect.y / APP_SIZE_TREEMAP_HEIGHT * 100).toFixed(4)}%`,
            `width:${(rect.w / APP_SIZE_TREEMAP_WIDTH * 100).toFixed(4)}%`,
            `height:${(rect.h / APP_SIZE_TREEMAP_HEIGHT * 100).toFixed(4)}%`,
          ].join(";"),
          "data-tone": String(appSizeTreemapTone(item.name)),
          "data-app-size-treemap": source.mode,
          "data-app-size-treemap-name": item.name,
          "aria-pressed": active ? "true" : "false",
          "aria-label": `${title}. Selects the matching ${source.mode === "sections" ? "section" : "group"} row.`,
          title,
        },
        on: { click: () => appSizeFocusTreemapRow(source.mode, item.name) },
      });
      if (rect.w >= 60 && rect.h >= 26) cell.append(el("span", { class: "app-size-treemap-label", text: item.name }));
      if (rect.w >= 90 && rect.h >= 44) cell.append(el("span", { class: "app-size-treemap-size", text: `${fmtBytes(item.value)} · ${percent.toFixed(1)}%` }));
      canvas.append(cell);
    }
    body.append(canvas);

    const legend = el("div", { class: "app-size-treemap-legend", attrs: { "aria-label": `Treemap legend for ${source.label}` } });
    for (const item of source.drawn.slice(0, APP_SIZE_TREEMAP_LEGEND)) {
      const focused = state.appSize.treemapFocus;
      const active = Boolean(focused) && focused.mode === source.mode && focused.name === item.name;
      legend.append(el("button", {
        class: `app-size-treemap-legend-item${active ? " is-focused" : ""}`,
        attrs: {
          type: "button",
          "aria-pressed": active ? "true" : "false",
          "data-app-size-treemap-legend-mode": source.mode,
          "data-app-size-treemap-legend-name": item.name,
          title: appSizeTreemapCellTitle(source, item),
        },
        on: { click: () => appSizeFocusTreemapRow(source.mode, item.name) },
      },
        el("span", { class: "app-size-treemap-swatch", attrs: { "data-tone": String(appSizeTreemapTone(item.name)) } }),
        el("span", { text: item.name }),
      ));
    }
    body.append(legend);
    if (source.drawn.length > APP_SIZE_TREEMAP_LEGEND) {
      body.append(el("p", { class: "app-size-note", text: `${formatCount(source.drawn.length - APP_SIZE_TREEMAP_LEGEND)} more cells reuse the palette; hover a cell for its name and bytes.` }));
    }
  }

  if (source.unmeasured > 0 && source.drawn.length) {
    body.append(el("p", { class: "app-size-note", text: `${formatCount(source.unmeasured)} shown ${source.label} have no positive reported bytes and are not drawn.` }));
  }
  const truncation = appSizeTreemapTruncation(source);
  if (truncation) body.append(el("p", { class: "app-size-note", text: truncation }));
  block.append(body);
  return block;
}

/* Containment rendering. Rows are native <details>/<summary> disclosures so
 * expansion works with the keyboard and screen readers without custom roving
 * focus. */

function appSizeTreeOpen(path, fallback) {
  const saved = state.appSize.containmentOpen[path];
  return saved === undefined ? fallback : saved === true;
}

function appSizeTreeBadge(text, className = "") {
  return el("span", {
    class: `app-size-tree-badge${className ? ` ${className}` : ""}`,
    text,
  });
}

function appSizeTreeCell(cell) {
  const text = cell.text !== undefined
    ? cell.text
    : cell.value === null || cell.value === undefined
      ? "—"
      : fmtBytes(cell.value);
  return el("span", {
    class: `app-size-tree-cell app-size-tree-${cell.kind}`,
    text,
    attrs: cell.title ? { title: cell.title } : {},
  });
}

function appSizeTreeRow(tag, spec, branch) {
  const attributes = {
    "data-app-size-tree-row": spec.kind,
    "data-app-size-tree-level": String(spec.level),
    "data-app-size-tree-kind": spec.kind,
    "data-app-size-tree-name": spec.name,
  };
  if (spec.title) attributes.title = spec.title;
  if (tag === "summary") attributes["aria-label"] = spec.aria || spec.title || spec.name;
  const row = el(tag, {
    class: `app-size-tree-row${spec.className ? ` ${spec.className}` : ""}`,
    attrs: attributes,
  });
  const name = el("span", { class: "app-size-tree-name" });
  name.append(el("span", {
    class: branch ? "app-size-tree-twisty" : "app-size-tree-spacer",
    attrs: { "aria-hidden": "true" },
  }));
  name.append(el("span", { class: "app-size-tree-label", text: spec.name }));
  for (const badge of spec.badges || []) name.append(appSizeTreeBadge(badge.text, badge.className));
  row.append(name);
  for (const cell of spec.cells || []) row.append(appSizeTreeCell(cell));
  return row;
}

function appSizeTreeNode(spec, children, fallbackOpen) {
  const visible = (children || []).filter(Boolean);
  if (!visible.length) {
    return el("div", {
      class: `app-size-tree-node is-leaf${spec.className ? ` ${spec.className}` : ""}`,
      attrs: {
        "data-app-size-tree-node": "leaf",
        "data-app-size-tree-level": String(spec.level),
        "data-app-size-tree-kind": spec.kind,
        "data-app-size-tree-name": spec.name,
      },
    }, appSizeTreeRow("div", spec, false));
  }
  const details = el("details", {
    class: `app-size-tree-node${spec.className ? ` ${spec.className}` : ""}`,
    props: { open: appSizeTreeOpen(spec.path, fallbackOpen) },
    attrs: {
      "data-app-size-tree-node": "branch",
      "data-app-size-tree-level": String(spec.level),
      "data-app-size-tree-kind": spec.kind,
      "data-app-size-tree-name": spec.name,
      "data-app-size-tree-path": spec.path,
    },
  });
  details.append(appSizeTreeRow("summary", spec, true));
  details.append(el("div", { class: "app-size-tree-children" }, ...visible));
  details.addEventListener("toggle", () => {
    state.appSize.containmentOpen[spec.path] = details.open;
  });
  return details;
}

function appSizeShareText(value, total) {
  if (value === null || value === undefined || total === null || total === undefined || !(total > 0)) return "—";
  return `${((value / total) * 100).toFixed(1)}%`;
}

function appSizeTreeCells(containment, row) {
  const mode = containment.mode;
  const cells = [];
  if (mode === "reported") {
    cells.push({
      kind: "file",
      value: row.reportedBytes,
      title: row.reportedBytes === null ? "No reported section size" : `${formatCount(row.reportedBytes)} reported section bytes`,
    });
  } else {
    cells.push({
      kind: "file",
      value: row.fileBytes,
      title: row.fileBytes === null ? "File bytes not reported" : `${formatCount(row.fileBytes)} file bytes`,
    });
    cells.push({
      kind: "vm",
      value: row.vmBytes,
      title: row.vmBytes === null ? "VM bytes not reported" : `${formatCount(row.vmBytes)} VM bytes`,
    });
  }
  cells.push({
    kind: "symbols",
    value: row.symbolBytes ?? null,
    text: row.symbolText,
    title: row.symbolTitle,
  });
  cells.push({
    kind: "share",
    text: row.shareText ?? appSizeShareText(row.shareValue, containment.fileBytes),
    title: row.shareTitle,
  });
  return cells;
}

function appSizeContainmentGroupSpec(containment, group, section, segment, path) {
  const bytesText = group.unsized ? `≥ ${fmtBytes(group.bytes)}` : fmtBytes(group.bytes);
  const badges = [{ text: `${formatCount(group.symbols)} ${group.symbols === 1 ? "symbol" : "symbols"}` }];
  if (group.aliasesKnown && group.aliases > 0) {
    badges.push({ text: `${formatCount(group.aliases)} ${group.aliases === 1 ? "alias" : "aliases"}` });
  }
  if (group.inferred > 0) badges.push({ text: `${formatCount(group.inferred)} inferred` });
  if (group.unsized > 0) badges.push({ text: `${formatCount(group.unsized)} unsized` });
  const largest = group.largest
    .map(symbol => `${symbol.name} (${fmtBytes(symbol.sizeBytes)})`)
    .join(", ");
  const title = [
    `${group.name} · crate group`,
    `${bytesText} across ${formatCount(group.symbols)} returned ${group.symbols === 1 ? "symbol" : "symbols"}`,
    `section ${section.name}`,
    segment ? `segment ${segment.name}` : "",
    group.unsized ? `${formatCount(group.unsized)} symbols have no reported size and are excluded from the sum` : "",
    group.aliasesKnown && group.aliases > 0 ? `${formatCount(group.aliases)} aliases` : "",
    largest ? `largest ${largest}` : "",
  ].filter(Boolean).join(" · ");
  return {
    path,
    level: 3,
    kind: "group",
    name: group.name,
    badges,
    cells: appSizeTreeCells(containment, {
      symbolBytes: group.bytes,
      symbolText: bytesText,
      symbolTitle: "Symbol bytes of the returned symbols in this crate group",
      shareValue: group.bytes,
      shareTitle: `Symbol bytes over the ${containment.fileBytes === null ? "returned symbol" : "executable file"} bytes`,
    }),
    title,
    aria: title,
  };
}

function appSizeContainmentGroupNodes(containment, groups, section, segment, path) {
  const nodes = [];
  const shown = groups.slice(0, MAX_APP_SIZE_CONTAINMENT_GROUPS);
  for (const group of shown) {
    nodes.push(appSizeTreeNode(
      appSizeContainmentGroupSpec(containment, group, section, segment, `${path}/group:${group.name}`),
      [],
    ));
  }
  if (groups.length > shown.length) {
    nodes.push(appSizeTreeNode({
      path: `${path}/groups-more`,
      level: 3,
      kind: "note",
      name: `${formatCount(groups.length - shown.length)} more groups not shown`,
      cells: appSizeTreeCells(containment, { symbolBytes: null, symbolText: "—" }),
      className: "is-note",
    }, []));
  }
  return nodes;
}

function appSizeContainmentSectionNode(containment, section, segment, path) {
  const badges = [];
  if (section.kind) badges.push({ text: section.kind });
  if (section.zeroFill) badges.push({ text: "zero-fill", className: "is-zero-fill" });
  if (section.groupSymbols > 0) {
    badges.push({ text: `${formatCount(section.symbols?.length || 0)} groups` });
  }
  if (section.aliases !== null && section.aliases > 0) {
    badges.push({ text: `${formatCount(section.aliases)} ${section.aliases === 1 ? "alias" : "aliases"}` });
  }
  if (section.symbolCount !== null && (section.symbols?.length || 0) < section.symbolCount) {
    badges.push({ text: `${formatCount(section.symbolCount - (section.symbols?.length || 0))} symbols not returned` });
  }
  const groupNames = section.groups.slice(0, 3).map(group => group.name).join(", ");
  const title = [
    `${section.name} · section`,
    section.kind ? section.kind : "",
    section.fileBytes !== null ? `file ${fmtBytes(section.fileBytes)}` : "",
    section.vmBytes !== null ? `VM ${fmtBytes(section.vmBytes)}` : "",
    section.zeroFill ? "zero-fill/uninitialized" : "",
    section.address ? `address ${section.address}` : "",
    segment ? `segment ${segment.name}` : "",
    section.symbols?.length
      ? `${formatCount(section.symbols.length)} returned symbols in ${formatCount(section.groups.length)} crate groups`
      : "no symbol is attributed to this section",
    groupNames ? `groups ${groupNames}${section.groups.length > 3 ? ", …" : ""}` : "",
    section.aliases !== null && section.aliases > 0 ? `${formatCount(section.aliases)} aliases` : "",
  ].filter(Boolean).join(" · ");
  const spec = {
    path,
    level: 2,
    kind: "section",
    name: section.name,
    badges,
    cells: appSizeTreeCells(containment, {
      fileBytes: section.fileBytes,
      vmBytes: section.vmBytes,
      reportedBytes: section.sizeBytes,
      symbolBytes: null,
      symbolText: "—",
      shareValue: section.fileBytes ?? (containment.mode === "reported" ? section.sizeBytes : null),
      shareTitle: "Section file bytes over the executable file",
    }),
    title,
    aria: title,
  };
  const children = section.groups.length
    ? appSizeContainmentGroupNodes(containment, section.groups, section, segment, path)
    : [];
  return appSizeTreeNode(spec, children, false);
}

function appSizeContainmentUnassignedNode(containment, symbols, groups, segment, path, level) {
  const bytes = symbols.reduce((sum, symbol) => sum + (symbol.sizeBytes ?? 0), 0);
  const unsized = symbols.filter(symbol => symbol.sizeBytes === null).length;
  const aliases = symbols.some(symbol => symbol.aliases !== null)
    ? symbols.reduce((sum, symbol) => sum + (symbol.aliases ?? 0), 0)
    : null;
  const title = [
    "Symbols without a section attribution",
    `${formatCount(symbols.length)} returned ${symbols.length === 1 ? "symbol" : "symbols"}`,
    unsized ? `${formatCount(unsized)} without a reported size` : "",
    segment ? `reported segment ${segment.name}` : "reported segment not available",
    "their crate groups are shown without a section or file/VM size",
  ].filter(Boolean).join(" · ");
  const badges = [{ text: `${formatCount(symbols.length)} ${symbols.length === 1 ? "symbol" : "symbols"}` }];
  if (aliases !== null && aliases > 0) badges.push({ text: `${formatCount(aliases)} aliases` });
  if (unsized > 0) badges.push({ text: `${formatCount(unsized)} unsized` });
  const spec = {
    path,
    level,
    kind: "unassigned",
    name: "Symbols without a section",
    badges,
    cells: appSizeTreeCells(containment, {
      symbolBytes: bytes,
      symbolText: unsized ? `≥ ${fmtBytes(bytes)}` : fmtBytes(bytes),
      symbolTitle: "Symbol bytes of returned symbols that name no returned section",
      shareValue: bytes,
      shareTitle: "Symbol bytes over the executable file",
    }),
    title,
    aria: title,
  };
  const section = { name: "no returned section", segment: segment?.name || "" };
  return appSizeTreeNode(spec, appSizeContainmentGroupNodes(containment, groups, section, segment, path), false);
}

function appSizeContainmentSegmentNode(containment, segment, index) {
  const path = `segment:${segment.name}:${index}`;
  const badges = [];
  if (segment.sections.length) {
    badges.push({ text: `${formatCount(segment.sections.length)} sections` });
  }
  if (segment.sectionCount !== null && segment.sectionCount > segment.sections.length) {
    badges.push({ text: `${formatCount(segment.sectionCount - segment.sections.length)} not returned` });
  }
  if (segment.aliases !== null && segment.aliases > 0) {
    badges.push({ text: `${formatCount(segment.aliases)} aliases` });
  }
  if (segment.derived && !segment.synthetic) badges.push({ text: "grouped from sections" });
  if (segment.inconsistent) badges.push({ text: "inconsistent", className: "is-warn" });
  const title = [
    `${segment.name} · segment`,
    segment.fileBytes !== null ? `file ${fmtBytes(segment.fileBytes)}` : "",
    segment.vmBytes !== null ? `VM ${fmtBytes(segment.vmBytes)}` : "",
    segment.reportedBytes !== null && segment.fileBytes === null && segment.vmBytes === null
      ? `${fmtBytes(segment.reportedBytes)} reported`
      : "",
    segment.address ? `address ${segment.address}` : "",
    segment.fileOffset ? `file offset ${segment.fileOffset}` : "",
    segment.sections.length ? `${formatCount(segment.sections.length)} returned sections` : "no returned sections",
    segment.gapFile !== null && segment.gapFile > 0
      ? `${fmtBytes(segment.gapFile)} of the segment's file bytes are not represented by returned section rows`
      : "",
    segment.gapVm !== null && segment.gapVm > 0
      ? `${fmtBytes(segment.gapVm)} of the segment's VM bytes are not represented by returned section rows`
      : "",
    segment.inconsistent ? "reported section bytes exceed the segment bytes, so no remainder is computed" : "",
    segment.derived ? "this row is grouped from the sections' own segment names, not a returned segment table" : "",
  ].filter(Boolean).join(" · ");
  const spec = {
    path,
    level: 1,
    kind: "segment",
    name: segment.name,
    badges,
    cells: appSizeTreeCells(containment, {
      fileBytes: segment.fileBytes,
      vmBytes: segment.vmBytes,
      reportedBytes: segment.reportedBytes,
      symbolBytes: null,
      symbolText: "—",
      shareValue: segment.fileBytes ?? (containment.mode === "reported" ? segment.reportedBytes : null),
      shareTitle: "Segment file bytes over the executable file",
    }),
    title,
    aria: title,
  };
  const children = [];
  for (const [sectionIndex, section] of segment.sections.entries()) {
    children.push(appSizeContainmentSectionNode(
      containment,
      section,
      segment,
      `${path}/section:${sectionIndex}`,
    ));
  }
  if (segment.unassigned.length) {
    children.push(appSizeContainmentUnassignedNode(
      containment,
      segment.unassigned,
      segment.unassignedGroups,
      segment,
      `${path}/unassigned`,
      2,
    ));
  }
  if (!children.length) {
    children.push(appSizeTreeNode({
      path: `${path}/empty`,
      level: 2,
      kind: "note",
      name: segment.sectionCount === 0 ? "No sections in this segment" : "No section rows returned for this segment",
      cells: appSizeTreeCells(containment, { symbolBytes: null, symbolText: "—" }),
      className: "is-note",
    }, []));
  }
  return appSizeTreeNode(spec, children, true);
}

function appSizeContainmentBucketNode(containment) {
  const bucket = containment.bucket;
  const badges = [];
  if (bucket.reported) badges.push({ text: "reported" });
  if (bucket.outsideInconsistent) badges.push({ text: "inconsistent", className: "is-warn" });
  const title = [
    "Unattributed file bytes",
    "file bytes no returned section row represents",
    "headers, symbol/string tables (LINKEDIT), section padding and unreturned sections may all be here; this bucket does not split them",
    bucket.outside !== null ? `${fmtBytes(bucket.outside)} outside returned segments` : "",
    bucket.gapFile !== null ? `${fmtBytes(bucket.gapFile)} inside returned segments but outside section rows` : "",
    bucket.reported ? "the analyzer reported this remainder directly" : "",
  ].filter(Boolean).join(" · ");
  const spec = {
    path: "bucket:unattributed",
    level: 1,
    kind: "bucket",
    name: "Unattributed bytes",
    badges,
    cells: appSizeTreeCells(containment, {
      fileBytes: bucket.total,
      symbolBytes: null,
      symbolText: "—",
      shareValue: bucket.total,
      shareTitle: "Unattributed bytes over the executable file",
    }),
    title,
    aria: title,
    className: "is-bucket",
  };
  const children = [];
  if (bucket.outside !== null) {
    children.push(appSizeTreeNode({
      path: "bucket:unattributed/outside",
      level: 2,
      kind: "bucket-part",
      name: "Outside returned segments",
      cells: appSizeTreeCells(containment, {
        fileBytes: bucket.outside,
        symbolBytes: null,
        symbolText: "—",
        shareValue: bucket.outside,
        shareTitle: "These bytes over the executable file",
      }),
      title: bucket.outsideReported
        ? `The analyzer reported ${formatCount(bucket.outside)} unattributed file bytes.`
        : `${formatCount(bucket.outside)} file bytes are left after the returned segment file bytes. Headers and LINKEDIT tables may be part of this remainder.`,
    }, []));
  }
  if (bucket.gapFile !== null) {
    const top = bucket.gapSegments.slice(0, 5)
      .map(segment => `${segment.name} ${fmtBytes(segment.gapFile)}`)
      .join(", ");
    children.push(appSizeTreeNode({
      path: "bucket:unattributed/gap",
      level: 2,
      kind: "bucket-part",
      name: "Inside segments, outside section rows",
      cells: appSizeTreeCells(containment, {
        fileBytes: bucket.gapFile,
        symbolBytes: null,
        symbolText: "—",
        shareValue: bucket.gapFile,
        shareTitle: "These bytes over the executable file",
      }),
      title: [
        `${formatCount(bucket.gapFile)} file bytes belong to returned segments but not to returned section rows.`,
        top ? `Largest remainders: ${top}.` : "",
        "This includes unreturned sections, LINKEDIT/symbol tables, headers and alignment padding; the analyzer does not split them.",
      ].filter(Boolean).join(" "),
    }, []));
  }
  if (!children.length) {
    children.push(appSizeTreeNode({
      path: "bucket:unattributed/unknown",
      level: 2,
      kind: "note",
      name: bucket.total === null ? "File remainder not reported" : "No file remainder is reported",
      cells: appSizeTreeCells(containment, { symbolBytes: null, symbolText: "—" }),
      title: bucket.total === null
        ? "Per-section file sizes or the executable file size were not reported, so no remainder is computed."
        : "The reported remainder is zero; no bytes are left outside the returned sections.",
      className: "is-note",
    }, []));
  }
  return appSizeTreeNode(spec, children, false);
}

function appSizeContainmentTotal(label, value, detail) {
  return el("div", { class: "app-size-containment-total" },
    el("span", { class: "app-size-containment-total-label", text: label }),
    el("span", { class: "app-size-containment-total-value", text: value }),
    detail ? el("span", { class: "app-size-containment-total-detail", text: detail }) : null,
  );
}

function appSizeContainmentCoverageText(containment) {
  const segments = appSizeCoverageLabel(containment.segmentCoverage);
  const sections = appSizeCoverageLabel(containment.sectionCoverage);
  const symbols = appSizeCoverageLabel(containment.symbolCoverage);
  const parts = [`Returned: segments ${segments}, sections ${sections}, symbols ${symbols}.`];
  if (containment.segmentCoverage.derived) {
    parts.push("No segment table was returned, so segment rows group sections by each section's own reported segment name.");
  }
  if (!containment.segmentCoverage.complete || !containment.sectionCoverage.complete) {
    parts.push("Segment and section rows are partial; bytes of unreturned entries stay in the unattributed bucket and are not drawn.");
  }
  if (!containment.symbolCoverage.complete) {
    parts.push("Group rows sum only the returned symbols.");
  }
  return parts.join(" ");
}

function appSizeContainmentRemainderText(containment) {
  const file = containment.fileBytes;
  const bucket = containment.bucket.total;
  if (file === null) {
    return "The executable file size was not reported, so the visible rows cannot be compared with the complete file.";
  }
  if (containment.mode === "reported") {
    return `The analyzer returned one size per section without a file/VM split. These are reported section sizes, not file bytes, and the ${fmtBytes(file)} file is not fully mapped.`;
  }
  if (bucket === null) {
    return `The rows are not the complete ${fmtBytes(file)} file: per-section file sizes or the segment file sizes were not reported, so no remainder is computed and no estimate is substituted.`;
  }
  if (bucket <= 0) {
    return `Every reported file byte is covered by returned segment and section rows (${fmtBytes(file)}). Symbol groups still cover only returned symbols.`;
  }
  const sectionBytes = containment.sectionBytesKnown ? containment.sectionBytesInFile : null;
  const lead = sectionBytes === null
    ? `The rows are not the complete ${fmtBytes(file)} file.`
    : `Section rows report ${fmtBytes(sectionBytes)} of the ${fmtBytes(file)} file.`;
  return `${lead} The unattributed bucket holds ${fmtBytes(bucket)}. Visible rows are not the complete file unless the returned counts above say they are complete.`;
}

function appSizeContainmentNotes(containment) {
  const notes = [];
  if (containment.skippedSegments > 0) {
    notes.push(`${formatCount(containment.skippedSegments)} segment entries had no name and were skipped; sections are grouped by their own segment names.`);
  }
  if (containment.segmentCoverage.derived) {
    notes.push("Containment is grouped from the returned sections because the analyzer returned no segment table. Segment file and VM bytes are subtotals of their returned sections.");
  }
  if (containment.bucket.outsideInconsistent) {
    notes.push("The reported segment file bytes exceed the executable file size. No negative remainder is shown; the payload is inconsistent.");
  }
  if (containment.bucket.gapSegments.length) {
    const top = containment.bucket.gapSegments.slice(0, 4)
      .map(segment => `${segment.name} ${fmtBytes(segment.gapFile)}`)
      .join(", ");
    notes.push(`Largest segment remainders outside section rows: ${top}${containment.bucket.gapSegments.length > 4 ? ", …" : ""}. These bytes are not assigned to a crate group.`);
  }
  if (containment.zeroFillKnown && containment.zeroFillBytes > 0) {
    notes.push(`${fmtBytes(containment.zeroFillBytes)} of zero-fill/uninitialized VM bytes occupy no file bytes, so the file and VM columns are not interchangeable.`);
  }
  if (containment.sections.some(section => section.inconsistent)) {
    notes.push("Some returned sections report more bytes than their segment. Those segments show no remainder instead of a negative value.");
  }
  notes.push("Group rows sum the returned symbols by crate prefix, the same grouping the Crate groups table uses. They are not Dart AOT ownership, package or class attribution, and there are no call edges.");
  return notes;
}

function renderAppSizeContainment(executable) {
  const containment = executable.containment;
  const block = appSizePanel(
    "Containment",
    `${formatCount(containment.segments.length)} segments · ${formatCount(containment.sections.length)} sections`,
  );
  block.id = "app-size-containment-panel";
  const body = el("div", { class: "panel-block-body" });

  body.append(el("div", { class: "app-size-containment-scope" },
    icon("info"),
    el("span", {},
      el("strong", { text: "Object-file containment, not Flutter's dominator tree or call graph." }),
      " Segments and sections come from the executable; group rows sum returned symbols by crate prefix. There are no call edges, no AOT ownership and no package/class attribution.",
    ),
  ));

  const totals = el("div", { class: "app-size-containment-totals" });
  totals.append(
    appSizeContainmentTotal(
      "File",
      containment.fileBytes === null ? "Not reported" : fmtBytes(containment.fileBytes),
      containment.fileBytes === null ? null : `${formatCount(containment.fileBytes)} bytes`,
    ),
    appSizeContainmentTotal(
      "Segments",
      appSizeCoverageLabel(containment.segmentCoverage),
      containment.segmentCoverage.derived ? "grouped from sections" : "returned",
    ),
    appSizeContainmentTotal(
      "Sections",
      appSizeCoverageLabel(containment.sectionCoverage),
      "returned",
    ),
    appSizeContainmentTotal(
      "VM",
      containment.totalVmBytes === null ? "Not reported" : fmtBytes(containment.totalVmBytes),
      containment.vmComplete ? "segment VM sum" : "returned segment VM sum, partial",
    ),
    appSizeContainmentTotal(
      "Zero-fill",
      containment.zeroFillKnown ? fmtBytes(containment.zeroFillBytes) : "Not reported",
      containment.zeroFillKnown ? "VM bytes without file bytes" : "no section VM split",
    ),
    appSizeContainmentTotal(
      "Unattributed",
      containment.bucket.total === null ? "Not reported" : fmtBytes(containment.bucket.total),
      containment.bucket.total === null ? "not computed" : "in the explicit bucket",
    ),
  );
  body.append(totals);
  body.append(el("p", { class: "app-size-note", text: appSizeContainmentCoverageText(containment) }));
  body.append(el("p", { class: "app-size-note app-size-containment-remainder", text: appSizeContainmentRemainderText(containment) }));

  const tree = el("div", { class: "app-size-tree", attrs: { role: "group", "aria-label": "Segment, section and crate group containment" } });
  const columns = el("div", { class: "app-size-tree-columns", attrs: { "aria-hidden": "true" } });
  columns.append(
    el("span", { text: "Segment / section / group" }),
    el("span", { class: "app-size-tree-cell app-size-tree-file", text: containment.mode === "reported" ? "Reported" : "File" }),
  );
  if (containment.mode !== "reported") {
    columns.append(el("span", { class: "app-size-tree-cell app-size-tree-vm", text: "VM" }));
  }
  columns.append(
    el("span", { class: "app-size-tree-cell app-size-tree-symbols", text: "Symbols" }),
    el("span", { class: "app-size-tree-cell app-size-tree-share", text: "Share" }),
  );
  tree.append(columns);

  for (const [index, segment] of containment.shownSegments.entries()) {
    tree.append(appSizeContainmentSegmentNode(containment, segment, index));
  }
  if (!containment.segments.length) {
    tree.append(appSizeTreeNode({
      path: "empty",
      level: 1,
      kind: "note",
      name: "The analyzer reported no segments or sections",
      cells: appSizeTreeCells(containment, { symbolBytes: null, symbolText: "—" }),
      className: "is-note",
    }, []));
  }
  if (containment.unassignedSymbols.length) {
    tree.append(appSizeContainmentUnassignedNode(
      containment,
      containment.unassignedSymbols,
      containment.unassignedGroups,
      null,
      "unassigned:global",
      1,
    ));
  }
  tree.append(appSizeContainmentBucketNode(containment));
  body.append(tree);

  for (const note of appSizeContainmentNotes(containment)) {
    body.append(el("p", { class: "app-size-note", text: note }));
  }
  if (containment.segments.length > containment.shownSegments.length) {
    body.append(el("p", {
      class: "app-size-note",
      text: `Showing ${formatCount(containment.shownSegments.length)} of ${formatCount(containment.segments.length)} segments. Unshown segments are still part of the totals and the unattributed remainder.`,
    }));
  }
  if (containment.sections.length < (containment.sectionCoverage.returned || 0)) {
    body.append(el("p", {
      class: "app-size-note",
      text: `Showing ${formatCount(containment.sections.length)} of ${formatCount(containment.sectionCoverage.returned)} returned sections. Unshown sections stay in the segment remainder.`,
    }));
  }
  block.append(body);
  return block;
}

