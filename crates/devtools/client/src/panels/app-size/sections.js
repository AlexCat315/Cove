/* App Size containment
 *
 * Flutter's App Size dominator tree and call graph describe Dart AOT ownership
 * and call edges. Cove has neither, so this hierarchy contains only what the
 * object file reports: segments, their sections, and the crate groups of the
 * symbols that explicitly name a section. It never infers an edge, an owner or
 * an AOT attribution. Bytes that no returned segment or section accounts for
 * stay in an explicit bucket instead of being distributed.
 */

function appSizeSegmentEntry(entry) {
  if (!isObjectValue(entry)) return null;
  const name = appSizeFirstText(entry.name, entry.segment, entry.segmentName, entry.segment_name, entry.id);
  if (!name) return null;
  const fileBytes = appSizeFirstNumber(
    entry.fileBytes,
    entry.file_bytes,
    entry.fileSize,
    entry.file_size,
    entry.filesize,
  );
  const vmBytes = appSizeFirstNumber(
    entry.vmBytes,
    entry.vm_bytes,
    entry.vmSize,
    entry.vm_size,
    entry.virtualSize,
    entry.virtual_size,
    entry.memsize,
  );
  return {
    name,
    fileBytes,
    vmBytes,
    reportedBytes: appSizeFirstNumber(entry.sizeBytes, entry.size_bytes, entry.bytes, entry.size),
    explicitFile: fileBytes !== null,
    explicitVm: vmBytes !== null,
    address: appSizeAddress(entry.address ?? entry.addr ?? entry.vmAddress ?? entry.vm_address ?? entry.vmaddr),
    fileOffset: appSizeAddress(entry.fileOffset ?? entry.file_offset ?? entry.fileoff),
    sectionCount: Array.isArray(entry.sections)
      ? entry.sections.length
      : appSizeFirstNumber(entry.sectionCount, entry.section_count, entry.sections),
    aliases: appSizeAliasCount(entry),
    zeroFillBytes: appSizeFirstNumber(entry.zeroFillBytes, entry.zero_fill_bytes),
    sections: [],
    unassigned: [],
    gapFile: null,
    gapVm: null,
    inconsistent: false,
    derived: false,
    synthetic: false,
  };
}

function appSizeListCoverage(shown, returned, total, truncated) {
  const complete = truncated !== true && (total === null ? shown >= returned : shown >= total);
  return { shown, returned, total, truncated: truncated === true, complete };
}

function appSizeCoverageLabel(info) {
  if (!info) return "Not reported";
  if (info.total !== null) return `${formatCount(info.shown)} of ${formatCount(info.total)}`;
  return `${formatCount(info.shown)} of ${formatCount(info.returned)} returned`;
}

function appSizeMatchSymbolSection(symbol, sections) {
  if (symbol.sectionIndex !== null) {
    const byIndex = sections.find(section => section.index === symbol.sectionIndex);
    if (byIndex) return byIndex;
  }
  const needle = String(symbol.section || "").trim().toLowerCase();
  if (!needle) return null;
  const full = sections.find(section => section.name.toLowerCase() === needle);
  if (full) return full;
  const bare = sections.filter(section => section.section && section.section.toLowerCase() === needle);
  return bare.length === 1 ? bare[0] : null;
}

function appSizeGroupSymbols(symbols) {
  const map = new Map();
  for (const symbol of symbols) {
    const name = symbol.crateName || appSizeSymbolGroupName(symbol.name);
    const group = map.get(name) || {
      name,
      bytes: 0,
      symbols: 0,
      unsized: 0,
      inferred: 0,
      aliases: 0,
      aliasesKnown: false,
      largest: [],
    };
    group.symbols += 1;
    if (symbol.sizeBytes === null) group.unsized += 1;
    else group.bytes += symbol.sizeBytes;
    if (symbol.aliases !== null) {
      group.aliases += symbol.aliases;
      group.aliasesKnown = true;
    }
    if (symbol.inferred) group.inferred += 1;
    if (symbol.name && group.largest.length < 3) group.largest.push(symbol);
    map.set(name, group);
  }
  return [...map.values()].sort((left, right) => right.bytes - left.bytes
    || right.symbols - left.symbols
    || (left.name < right.name ? -1 : left.name > right.name ? 1 : 0));
}

function appSizeBuildContainment({
  root,
  sections,
  symbols,
  rawSegments,
  fileBytes,
  symbolBytes,
  segmentInfo,
  sectionInfo,
  symbolInfo,
}) {
  let skippedSegments = 0;
  const segments = [];
  for (const entry of rawSegments) {
    const segment = appSizeSegmentEntry(entry);
    if (segment) segments.push(segment);
    else skippedSegments += 1;
  }
  const explicitSegments = segments.length > 0;
  const segmentByName = new Map();
  for (const segment of segments) {
    const key = segment.name.toLowerCase();
    if (!segmentByName.has(key)) segmentByName.set(key, segment);
  }

  const makeDerived = (name, synthetic) => ({
    name: name || "No segment reported",
    fileBytes: null,
    vmBytes: null,
    reportedBytes: null,
    explicitFile: false,
    explicitVm: false,
    address: "",
    fileOffset: "",
    sectionCount: null,
    aliases: null,
    zeroFillBytes: null,
    sections: [],
    unassigned: [],
    gapFile: null,
    gapVm: null,
    inconsistent: false,
    derived: true,
    synthetic,
  });

  let orphans = null;
  let missingSegments = null;
  for (const section of sections) {
    const reported = String(section.segment || "").trim();
    let segment = reported ? segmentByName.get(reported.toLowerCase()) : null;
    if (!segment) {
      if (!explicitSegments) {
        const key = reported || "";
        let group = key
          ? segments.find(item => item.name.toLowerCase() === key.toLowerCase())
          : missingSegments;
        if (!group) {
          group = makeDerived(reported, !reported);
          if (!reported) missingSegments = group;
          segments.push(group);
        }
        group.sections.push(section);
        continue;
      }
      if (!reported) {
        if (!missingSegments) {
          missingSegments = makeDerived("", true);
          segments.push(missingSegments);
        }
        missingSegments.sections.push(section);
        continue;
      }
      if (!orphans) {
        orphans = makeDerived("Sections outside returned segments", true);
        segments.push(orphans);
      }
      orphans.sections.push(section);
    } else {
      segment.sections.push(section);
    }
  }

  for (const segment of segments) {
    segment.derived = segment.derived || !explicitSegments;
    const fileValues = segment.sections.map(section => section.fileBytes);
    const vmValues = segment.sections.map(section => section.vmBytes);
    const reportedValues = segment.sections.map(section => section.sizeBytes);
    const fileKnown = fileValues.every(value => value !== null);
    const vmKnown = vmValues.every(value => value !== null);
    const reportedKnown = reportedValues.every(value => value !== null);
    const fileSum = fileKnown ? fileValues.reduce((sum, value) => sum + value, 0) : null;
    const vmSum = vmKnown ? vmValues.reduce((sum, value) => sum + value, 0) : null;
    const reportedSum = reportedKnown ? reportedValues.reduce((sum, value) => sum + value, 0) : null;
    if (!segment.explicitFile && fileSum !== null) segment.fileBytes = fileSum;
    if (!segment.explicitVm && vmSum !== null) segment.vmBytes = vmSum;
    if (segment.reportedBytes === null && segment.fileBytes === null && segment.vmBytes === null) {
      segment.reportedBytes = reportedSum;
    }
    if (segment.explicitFile && fileSum !== null) {
      const gap = segment.fileBytes - fileSum;
      if (gap < 0) segment.inconsistent = true;
      else segment.gapFile = gap;
    } else if (segment.fileBytes !== null) {
      segment.gapFile = 0;
    }
    if (segment.explicitVm && vmSum !== null) {
      const gap = segment.vmBytes - vmSum;
      segment.gapVm = gap < 0 ? null : gap;
      if (gap < 0) segment.inconsistent = true;
    } else if (segment.vmBytes !== null) {
      segment.gapVm = 0;
    }
    const aliasValues = segment.sections.map(section => section.aliases);
    if (segment.aliases === null && aliasValues.some(value => value !== null)) {
      segment.aliases = aliasValues.reduce((sum, value) => sum + (value ?? 0), 0);
    }
  }

  let unassignedGlobal = [];
  for (const symbol of symbols) {
    const section = appSizeMatchSymbolSection(symbol, sections);
    if (section) {
      section.symbols = section.symbols || [];
      section.symbols.push(symbol);
      continue;
    }
    const reportedSegment = String(symbol.segment || "").trim().toLowerCase();
    const segment = reportedSegment ? segmentByName.get(reportedSegment) : null;
    if (segment) {
      segment.unassigned.push(symbol);
    } else {
      unassignedGlobal.push(symbol);
    }
  }

  for (const section of sections) {
    section.symbols = section.symbols || [];
    const matched = section.symbols;
    section.groups = appSizeGroupSymbols(matched);
    section.groupSymbolBytes = matched.reduce((sum, symbol) => sum + (symbol.sizeBytes ?? 0), 0);
    section.symbolBytesKnown = matched.some(symbol => symbol.sizeBytes !== null);
    if (section.aliases === null && matched.some(symbol => symbol.aliases !== null)) {
      section.aliases = matched.reduce((sum, symbol) => sum + (symbol.aliases ?? 0), 0);
    }
    if (section.symbolCount === null && matched.length) section.symbolCount = matched.length;
  }
  for (const segment of segments) {
    segment.unassignedGroups = appSizeGroupSymbols(segment.unassigned);
    if (segment.unassigned.some(symbol => symbol.aliases !== null)) {
      segment.aliases = (segment.aliases ?? 0) + segment.unassigned.reduce((sum, symbol) => sum + (symbol.aliases ?? 0), 0);
    }
  }

  const allSegmentsShown = segments.length <= MAX_APP_SIZE_SEGMENTS;
  const segmentComplete = segmentInfo.complete && allSegmentsShown;
  const sectionComplete = sectionInfo.complete;

  const segmentFileComplete = segments.every(segment => segment.fileBytes !== null);
  const segmentFileSum = segmentFileComplete
    ? segments.reduce((sum, segment) => sum + segment.fileBytes, 0)
    : null;
  const sectionFileComplete = sections.every(section => section.fileBytes !== null);
  const sectionFileSum = sectionFileComplete
    ? sections.reduce((sum, section) => sum + section.fileBytes, 0)
    : null;

  const reportedUnattributed = appSizeFirstNumber(
    root.unattributedBytes,
    root.unattributed_bytes,
    root.unattributedFileBytes,
    root.unattributed_file_bytes,
    root.otherFileBytes,
    root.other_file_bytes,
    root.paddingBytes,
    root.padding_bytes,
  );
  let outside = null;
  let outsideReported = false;
  let outsideBasis = "";
  if (fileBytes !== null && segmentFileSum !== null && segmentComplete) {
    outside = fileBytes - segmentFileSum;
    outsideBasis = "segments";
  } else if (!explicitSegments && fileBytes !== null && sectionFileSum !== null && sectionComplete) {
    outside = fileBytes - sectionFileSum;
    outsideBasis = "sections";
  } else if (reportedUnattributed !== null) {
    outside = reportedUnattributed;
    outsideBasis = "reported";
    outsideReported = true;
  }
  let outsideInconsistent = false;
  if (outside !== null && outside < 0) {
    outsideInconsistent = true;
    outside = null;
    outsideBasis = "";
  }
  if (outside === null && reportedUnattributed !== null && outsideBasis !== "reported") {
    outside = reportedUnattributed;
    outsideBasis = "reported";
    outsideReported = true;
  }

  const gapsKnown = segments.every(segment => segment.gapFile !== null);
  const gapFile = gapsKnown
    ? segments.reduce((sum, segment) => sum + segment.gapFile, 0)
    : null;
  const gapSegments = segments
    .filter(segment => segment.gapFile !== null && segment.gapFile > 0)
    .sort((left, right) => right.gapFile - left.gapFile);
  const bucketTotal = outside !== null && gapFile !== null
    ? outside + gapFile
    : reportedUnattributed;
  const bucketReported = outsideReported || (bucketTotal !== null && outside === null && gapFile === null);

  const explicitZeroFill = appSizeFirstNumber(
    root.zeroFillBytes,
    root.zero_fill_bytes,
    root.zeroFill,
    root.zero_fill,
  );
  const zeroFillSections = sections.filter(section => section.zeroFill);
  const zeroFillKnown = zeroFillSections.every(section => section.vmBytes !== null);
  const zeroFillComputed = explicitZeroFill !== null
    ? explicitZeroFill
    : zeroFillKnown
      ? zeroFillSections.reduce((sum, section) => sum + Math.max(0, section.vmBytes - (section.fileBytes ?? 0)), 0)
      : sectionComplete && !zeroFillSections.length
        ? 0
        : null;

  const vmKnown = segments.every(segment => segment.vmBytes !== null);
  const vmSum = vmKnown ? segments.reduce((sum, segment) => sum + segment.vmBytes, 0) : null;
  const reportedVm = appSizeFirstNumber(root.vmBytes, root.vm_bytes, root.virtualBytes, root.virtual_bytes);
  const totalVmBytes = reportedVm ?? vmSum;

  const mode = segments.some(segment => segment.explicitFile || segment.explicitVm)
    || sections.some(section => section.fileBytes !== null || section.vmBytes !== null)
    || reportedVm !== null
    ? "file-vm"
    : "reported";

  const shownSegments = segments.slice(0, MAX_APP_SIZE_SEGMENTS);
  const shownSections = sections.slice(0, MAX_APP_SIZE_SECTIONS);
  const sectionBytesInFile = sectionFileSum;
  const accountedFile = fileBytes !== null && bucketTotal !== null
    ? Math.max(0, fileBytes - bucketTotal)
    : null;

  return {
    mode,
    segments,
    shownSegments,
    segmentsTotal: segmentInfo.total,
    segmentCoverage: {
      ...segmentInfo,
      shown: shownSegments.length,
      returned: Math.max(segmentInfo.returned, shownSegments.length),
      derived: !explicitSegments,
      skipped: skippedSegments,
      complete: segmentComplete,
    },
    sectionCoverage: {
      ...sectionInfo,
      shown: shownSections.length,
      drawn: shownSections.length,
      complete: sectionComplete,
    },
    symbolCoverage: symbolInfo,
    fileBytes,
    totalVmBytes,
    vmReported: reportedVm !== null,
    vmComplete: segmentComplete && vmKnown,
    zeroFillBytes: zeroFillComputed,
    zeroFillKnown: zeroFillComputed !== null,
    bucket: {
      total: bucketTotal,
      outside,
      outsideBasis,
      outsideReported,
      outsideInconsistent,
      gapFile,
      gapSegments,
      reported: bucketReported,
      reportedTotal: reportedUnattributed,
    },
    sections: shownSections,
    sectionBytesInFile,
    sectionBytesKnown: sectionFileComplete,
    accountedFile,
    symbolBytes,
    unassignedSymbols: unassignedGlobal,
    unassignedGroups: appSizeGroupSymbols(unassignedGlobal),
    notes: [],
    skippedSegments,
  };
}

function appSizePanel(title, meta, ...children) {
  return el("div", { class: "panel-block" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: title }),
      meta ? el("span", { class: "pane-meta", text: meta }) : null,
    ),
    ...children,
  );
}

function appSizeTable(headers, rows) {
  return el("div", { class: "table-wrap" }, el("table", { class: "data-table" },
    el("thead", {}, el("tr", {}, ...headers.map(header => el("th", { class: header.num ? "num" : "", text: header.label })))),
    el("tbody", {}, ...rows),
  ));
}

function appSizeEmptyBody(message) {
  return el("div", { class: "panel-block-body" }, el("p", { class: "app-size-note", text: message }));
}

function renderAppSizeSections(executable) {
  const info = executable.truncation.sections;
  const shown = executable.sections.slice(0, MAX_APP_SIZE_SECTIONS);
  const block = appSizePanel("Sections", `${formatCount(executable.sections.length)} reported`);
  if (!shown.length) {
    block.append(appSizeEmptyBody("The analyzer reported no sections. No section table is substituted."));
  } else {
    block.append(appSizeTable(
      [
        { label: "Section" },
        { label: "Kind" },
        { label: "Bytes", num: true },
        { label: "Address", num: true },
      ],
      shown.map(section => el("tr", {
        class: appSizeRowSelected("sections", section.name) ? "is-selected" : "",
        attrs: {
          "data-app-size-kind": "sections",
          "data-app-size-row": section.name,
          "aria-selected": appSizeRowSelected("sections", section.name) ? "true" : "false",
        },
      },
        el("td", { class: "app-size-name", text: section.name, attrs: section.name ? { title: section.name } : {} }),
        el("td", { text: section.kind || "—" }),
        el("td", { class: "num app-size-bytes", text: section.sizeBytes === null ? "Not reported" : fmtBytes(section.sizeBytes), attrs: section.sizeBytes === null ? {} : { title: `${formatCount(section.sizeBytes)} bytes` } }),
        el("td", { class: "num app-size-address", text: section.address || "—" }),
      )),
    ));
  }
  const notice = appSizeTruncationNotice(info, "sections");
  if (notice) block.append(notice);
  return block;
}

function renderAppSizeSymbols(executable) {
  const info = executable.truncation.symbols;
  const shown = executable.symbols.slice(0, MAX_APP_SIZE_SYMBOLS);
  const showCrate = shown.some(symbol => symbol.crateName);
  const block = appSizePanel("Top symbols", `${formatCount(executable.symbols.length)} reported`);
  if (!shown.length) {
    block.append(appSizeEmptyBody("The analyzer reported no sized symbols. The executable may be stripped; no symbol list is substituted."));
  } else {
    const headers = [{ label: "Symbol" }];
    if (showCrate) headers.push({ label: "Crate" });
    headers.push({ label: "Bytes", num: true });
    block.append(appSizeTable(headers, shown.map(symbol => el("tr", {},
      el("td", {
        class: "app-size-name",
        text: symbol.name,
        attrs: symbol.rawName ? { title: `Mangled: ${symbol.rawName}` } : {},
      }),
      showCrate ? el("td", { class: "app-size-group", text: symbol.crateName || "—" }) : null,
      el("td", { class: "num app-size-bytes", text: symbol.sizeBytes === null ? "Not reported" : fmtBytes(symbol.sizeBytes), attrs: symbol.sizeBytes === null ? {} : { title: `${formatCount(symbol.sizeBytes)} bytes` } }),
    ))));
  }
  const notice = appSizeTruncationNotice(info, "symbols");
  if (notice) block.append(notice);
  return block;
}

function renderAppSizeGroups(executable) {
  const info = executable.truncation.groups;
  const shown = executable.groups.slice(0, MAX_APP_SIZE_GROUPS);
  const showSymbols = shown.some(group => group.symbolCount !== null);
  const block = appSizePanel("Crate groups", `${formatCount(executable.groups.length)} reported`);
  if (!shown.length) {
    block.append(appSizeEmptyBody("The analyzer reported no crate groups. Symbol bytes are not regrouped here."));
  } else {
    const headers = [{ label: "Group" }];
    if (showSymbols) headers.push({ label: "Symbols", num: true });
    headers.push({ label: "Bytes", num: true });
    block.append(appSizeTable(headers, shown.map(group => el("tr", {
      class: appSizeRowSelected("groups", group.name) ? "is-selected" : "",
      attrs: {
        "data-app-size-kind": "groups",
        "data-app-size-row": group.name,
        "aria-selected": appSizeRowSelected("groups", group.name) ? "true" : "false",
      },
    },
      el("td", { class: "app-size-group", text: group.name, attrs: { title: group.name } }),
      showSymbols ? el("td", { class: "num", text: group.symbolCount === null ? "—" : formatCount(group.symbolCount) }) : null,
      el("td", { class: "num app-size-bytes", text: group.sizeBytes === null ? "Not reported" : fmtBytes(group.sizeBytes), attrs: group.sizeBytes === null ? {} : { title: `${formatCount(group.sizeBytes)} bytes` } }),
    ))));
  }
  const notice = appSizeTruncationNotice(info, "groups");
  if (notice) block.append(notice);
  return block;
}

function appSizeStat(label, value, detail) {
  return el("div", { class: "stat" },
    el("div", { class: "stat-label", text: label }),
    el("div", { class: "stat-value" }, value, detail ? el("small", { text: ` ${detail}` }) : null));
}

function renderAppSizeStats(executable) {
  const total = executable.totalBytes;
  const countDetail = (info, count) => (info.total !== null && info.total > count ? `of ${formatCount(info.total)}` : null);
  return el("div", { class: "stat-row" },
    appSizeStat("Total executable", total === null ? "Not reported" : fmtBytes(total), total === null ? null : `${formatCount(total)} bytes`),
    appSizeStat("Sections", formatCount(executable.sections.length), countDetail(executable.truncation.sections, executable.sections.length)),
    appSizeStat("Symbols", formatCount(executable.symbols.length), countDetail(executable.truncation.symbols, executable.symbols.length)),
    appSizeStat("Crate groups", formatCount(executable.groups.length), countDetail(executable.truncation.groups, executable.groups.length)),
  );
}

function appSizeSource(executable, target) {
  const pid = executable.pid ?? appSizeFirstNumber(target?.pid, target?.processId, target?.process_id);
  const meta = [
    pid === null || pid === undefined ? "" : `PID ${pid}`,
    executable.kind,
    executable.format,
    executable.arch,
    executable.platform || target?.platform,
  ].filter(Boolean).join(" · ");
  return appSizePanel("Executable",
    null,
    el("div", { class: "panel-block-body" },
      el("div", { class: "app-size-source" },
        el("div", { class: "app-size-source-path", text: executable.path || "Executable path not reported", attrs: executable.path ? { title: executable.path } : {} }),
        el("div", { class: "app-size-source-meta", text: meta || "Process metadata not reported" }),
      ),
    ),
  );
}

function appSizeScope(executable, target) {
  const pid = executable.pid ?? appSizeFirstNumber(target?.pid, target?.processId, target?.process_id);
  const path = executable.path ? ` (${executable.path})` : "";
  const label = executable.label || "Running native executable, not Flutter/Dart AOT.";
  const attribution = executable.attribution
    || "Cove has no Dart AOT output, package or class attribution, so no bytes are assigned to Dart libraries, packages or classes.";
  return el("div", { class: "app-size-scope" },
    icon("info"),
    el("span", {},
      el("strong", { text: label }),
      ` Sizes are read from the object file of pid ${pid ?? "unknown"}${path}. ${attribution}`,
    ),
  );
}

function appSizeRowSelected(kind, name) {
  const focus = state.appSize.treemapFocus;
  return Boolean(focus && focus.mode === kind && focus.name === name);
}

function appSizeCssEscape(value) {
  const text = String(value);
  if (typeof CSS !== "undefined" && typeof CSS.escape === "function") return CSS.escape(text);
  return text.replace(/["\\]/g, "\\$&");
}

function appSizeTreemapTone(name) {
  let hash = 2166136261;
  const text = String(name);
  for (let index = 0; index < text.length; index += 1) {
    hash ^= text.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0) % APP_SIZE_TREEMAP_TONES;
}

function appSizeWorstRatio(areas, side, area) {
  if (!areas.length || !(side > 0) || !(area > 0)) return Infinity;
  let max = 0;
  let min = Infinity;
  for (const value of areas) {
    if (value > max) max = value;
    if (value < min) min = value;
  }
  const sideSquared = side * side;
  const areaSquared = area * area;
  return Math.max((sideSquared * max) / areaSquared, areaSquared / (sideSquared * min));
}

// Squarified layout over a fixed grid. Sorting by bytes, then name, keeps the
// output identical for identical payloads; positions are converted to
// percentages so the rendered treemap stays responsive.
function appSizeSquarify(entries, width, height) {
  const total = entries.reduce((sum, entry) => sum + entry.value, 0);
  if (!(total > 0) || !(width > 0) || !(height > 0)) return [];
  const scale = (width * height) / total;
  const nodes = entries
    .map(entry => ({ entry, area: entry.value * scale }))
    .filter(node => node.area > 0);
  const rects = [];
  let free = { x: 0, y: 0, w: width, h: height };
  let index = 0;
  while (index < nodes.length && free.w > 0 && free.h > 0) {
    const side = Math.min(free.w, free.h);
    const row = [];
    const areas = [];
    let rowArea = 0;
    let worst = Infinity;
    while (index < nodes.length) {
      const candidate = nodes[index];
      const nextArea = rowArea + candidate.area;
      const nextWorst = appSizeWorstRatio([...areas, candidate.area], side, nextArea);
      if (row.length && nextWorst > worst) break;
      row.push(candidate);
      areas.push(candidate.area);
      rowArea = nextArea;
      worst = nextWorst;
      index += 1;
    }
    if (free.w >= free.h) {
      const rowWidth = rowArea / free.h;
      let y = free.y;
      row.forEach((node, position) => {
        const cellHeight = position === row.length - 1 ? free.y + free.h - y : node.area / rowWidth;
        rects.push({ entry: node.entry, x: free.x, y, w: rowWidth, h: cellHeight });
        y += cellHeight;
      });
      free = { x: free.x + rowWidth, y: free.y, w: free.w - rowWidth, h: free.h };
    } else {
      const rowHeight = rowArea / free.w;
      let x = free.x;
      row.forEach((node, position) => {
        const cellWidth = position === row.length - 1 ? free.x + free.w - x : node.area / rowHeight;
        rects.push({ entry: node.entry, x, y: free.y, w: cellWidth, h: rowHeight });
        x += cellWidth;
      });
      free = { x: free.x, y: free.y + rowHeight, w: free.w, h: free.h - rowHeight };
    }
  }
  return rects.map(rect => {
    const x = Math.max(0, Math.min(rect.x, width));
    const y = Math.max(0, Math.min(rect.y, height));
    return {
      entry: rect.entry,
      x,
      y,
      w: Math.max(0, Math.min(rect.w, width - x)),
      h: Math.max(0, Math.min(rect.h, height - y)),
    };
  }).filter(rect => rect.w > 0 && rect.h > 0);
}

function appSizeTreemapSource(executable) {
  const mode = state.appSize.treemapMode === "sections" ? "sections" : "groups";
  const all = mode === "sections" ? executable.sections : executable.groups;
  const limit = mode === "sections" ? MAX_APP_SIZE_SECTIONS : MAX_APP_SIZE_GROUPS;
  const shown = all.slice(0, limit);
  const drawn = shown
    .map((entry, index) => ({ name: entry.name, value: entry.sizeBytes, entry, order: index }))
    .filter(item => typeof item.value === "number" && Number.isFinite(item.value) && item.value > 0)
    .sort((left, right) => right.value - left.value
      || (left.name < right.name ? -1 : left.name > right.name ? 1 : 0)
      || left.order - right.order);
  return {
    mode,
    label: mode === "sections" ? "sections" : "crate groups",
    all,
    shown,
    drawn,
    unmeasured: shown.length - drawn.length,
    info: executable.truncation?.[mode] || null,
    sum: drawn.reduce((sum, item) => sum + item.value, 0),
  };
}

function appSizeTreemapTruncation(source) {
  const info = source.info;
  if (!info) return "";
  const shown = source.shown.length;
  const total = info.total;
  const returned = info.returned;
  const cut = info.truncated === true || (total !== null && total > shown) || (returned !== null && returned > shown);
  if (!cut) return "";
  const parts = [];
  if (total !== null && total > shown) parts.push(`Showing ${formatCount(shown)} of ${formatCount(total)} ${source.label}.`);
  else if (returned !== null && returned > shown) parts.push(`Showing ${formatCount(shown)} of ${formatCount(returned)} returned ${source.label}.`);
  else parts.push(`The controller capped the ${source.label} list at ${formatCount(shown)} entries.`);
  parts.push("Entries outside that list are not drawn.");
  return parts.join(" ");
}

function appSizeTreemapCellTitle(source, item) {
  const percent = source.sum > 0 ? (item.value / source.sum) * 100 : 0;
  const parts = [item.name, fmtBytes(item.value), `${percent.toFixed(2)}% of shown ${source.label} bytes`];
  const symbols = item.entry.symbolCount;
  if (Number.isFinite(symbols)) parts.push(`${formatCount(symbols)} ${symbols === 1 ? "symbol" : "symbols"}`);
  return parts.join(" · ");
}

function appSizeFocusTreemapRow(mode, name) {
  const current = state.appSize.treemapFocus;
  const next = current && current.mode === mode && current.name === name ? null : { mode, name };
  state.appSize.treemapFocus = next;
  for (const node of dom.appSizeContent.querySelectorAll("[data-app-size-treemap-name]")) {
    const active = Boolean(next)
      && node.dataset.appSizeTreemap === mode
      && node.dataset.appSizeTreemapName === name;
    node.classList.toggle("is-focused", active);
    node.setAttribute("aria-pressed", active ? "true" : "false");
  }
  for (const node of dom.appSizeContent.querySelectorAll("[data-app-size-treemap-legend-name]")) {
    const active = Boolean(next)
      && node.dataset.appSizeTreemapLegendMode === mode
      && node.dataset.appSizeTreemapLegendName === name;
    node.classList.toggle("is-focused", active);
    node.setAttribute("aria-pressed", active ? "true" : "false");
  }
  for (const row of dom.appSizeContent.querySelectorAll("[data-app-size-row]")) {
    const active = Boolean(next)
      && row.dataset.appSizeKind === mode
      && row.dataset.appSizeRow === name;
    row.classList.toggle("is-selected", active);
    row.setAttribute("aria-selected", active ? "true" : "false");
  }
  if (!next) return;
  const row = dom.appSizeContent.querySelector(`[data-app-size-kind="${mode}"][data-app-size-row="${appSizeCssEscape(name)}"]`);
  if (row) row.scrollIntoView({ block: "nearest" });
}

