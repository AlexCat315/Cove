/* App Size
 *
 * get_ui_app_size analyzes the object file behind the selected runtime's local
 * host process. Every size, section, symbol and group below comes from that
 * response. Cove has no Dart AOT output, so no package, library, class or
 * function attribution is invented; unattributed bytes stay unassigned.
 */

function appSizeWireNumber(value) {
  const direct = numberValue(value);
  if (direct !== null) return direct;
  if (typeof value === "string" && value.trim() !== "") {
    const number = Number(value.trim());
    if (Number.isFinite(number)) return number;
  }
  if (isObjectValue(value)) return firstNumberValue(value.bytes, value.value, value.size, value.amount);
  return null;
}

function appSizeFirstNumber(...values) {
  for (const value of values) {
    const number = appSizeWireNumber(value);
    if (number !== null) return number;
  }
  return null;
}

function appSizeText(value) {
  if (typeof value === "string" && value.trim()) return value.trim();
  if (numberValue(value) !== null) return String(value);
  return "";
}

function appSizeFirstText(...values) {
  for (const value of values) {
    const text = appSizeText(value);
    if (text) return text;
  }
  return "";
}

function appSizeAddress(value) {
  const text = appSizeText(value);
  if (text) {
    if (/^0x[0-9a-f]+$/i.test(text)) return text.toLowerCase();
    if (/^[0-9]+$/.test(text)) {
      try {
        return `0x${BigInt(text).toString(16)}`;
      } catch {
        return text;
      }
    }
    return text;
  }
  if (isObjectValue(value)) return appSizeFirstText(value.address, value.value, value.offset);
  return "";
}

function appSizePlatformLabel(platform) {
  return {
    macos: "macOS",
    windows: "Windows",
    linux: "Linux",
    android: "Android",
    ios: "iOS",
    web: "Web",
    wasm: "Web",
  }[platform] || platform;
}

function appSizeTargetUnavailable(target) {
  if (!target) return "Select a connected runtime.";
  if (target.kind !== "runtime") {
    return "App Size reads a running runtime's native executable. This target is a window, display or driver, so there is no process to analyze.";
  }
  const platform = String(target.platform || state.platform || "").toLowerCase();
  if (platform && !APP_SIZE_LOCAL_PLATFORMS.includes(platform)) {
    return `App Size reads the executable of a local host process. ${appSizePlatformLabel(platform)} targets run remotely or on a mobile device, so no local executable is available.`;
  }
  const pid = appSizeFirstNumber(target.pid, target.processId, target.process_id, target.process?.pid);
  if (pid === null) {
    return "The controller reported no local process ID for this runtime, so no executable can be read.";
  }
  return "";
}

function appSizeRoot(data) {
  if (!isObjectValue(data)) return null;
  return isObjectValue(data.data) ? data.data : data;
}

function appSizeUnavailableReason(root) {
  const unavailable = root.available === false
    || root.supported === false
    || root.unavailable === true
    || root.ok === false
    || root.status === "unavailable";
  if (!unavailable) return "";
  return appSizeFirstText(
    root.reason,
    root.unavailable_reason,
    root.unavailableReason,
    root.message,
    root.detail,
    typeof root.error === "string" ? root.error : "",
    root.error?.message,
    root.error?.reason,
  ) || "The controller reported no native executable analysis for this target.";
}

function appSizeTruncationFlag(root, name) {
  const truncated = root.truncated;
  if (Array.isArray(truncated) && truncated.includes(name)) return true;
  if (typeof truncated === "string" && truncated === name) return true;
  const values = [
    root[`${name}Truncated`],
    root[`${name}_truncated`],
    isObjectValue(truncated) ? truncated[name] : null,
    isObjectValue(truncated) ? truncated[`${name}_truncated`] : null,
    isObjectValue(root.limits) ? root.limits[name] : null,
    isObjectValue(root.limits) ? root.limits[`${name}_truncated`] : null,
  ];
  for (const value of values) {
    if (value === true) return true;
    if (value === false) return false;
  }
  return null;
}

function appSizeReportedTotal(root, name) {
  const singular = { sections: "section", symbols: "symbol", groups: "group", segments: "segment" }[name];
  const plural = { sections: "Sections", symbols: "Symbols", groups: "Groups", segments: "Segments" }[name];
  const singularCap = singular.charAt(0).toUpperCase() + singular.slice(1);
  const totals = isObjectValue(root.totals) ? root.totals : {};
  const counts = isObjectValue(root.counts) ? root.counts : {};
  return appSizeFirstNumber(
    root[`total${plural}`],
    root[`total_${name}`],
    root[`${name}Total`],
    root[`${name}_total`],
    root[`${name}Count`],
    root[`${name}_count`],
    root[`${singular}Count`],
    root[`${singular}_count`],
    totals[name],
    totals[`total_${name}`],
    totals[`${name}Count`],
    totals[`${singular}Count`],
    counts[name],
    counts[singular],
    root[`total${singularCap}`],
    root[`total_${singular}`],
  );
}

function appSizeList(root, executable, ...names) {
  for (const source of [root, executable]) {
    if (!isObjectValue(source)) continue;
    for (const name of names) {
      if (Array.isArray(source[name])) return source[name];
    }
  }
  return [];
}

function appSizeListFrom(sources, ...names) {
  for (const source of sources) {
    if (!isObjectValue(source)) continue;
    for (const name of names) {
      if (Array.isArray(source[name])) return source[name];
    }
  }
  return [];
}

function appSizeReportedTotalFrom(sources, name) {
  for (const source of sources) {
    if (!isObjectValue(source)) continue;
    const total = appSizeReportedTotal(source, name);
    if (total !== null) return total;
  }
  return null;
}

function appSizeTruncationFlagFrom(sources, name) {
  for (const source of sources) {
    if (!isObjectValue(source)) continue;
    const flag = appSizeTruncationFlag(source, name);
    if (flag !== null) return flag;
  }
  return null;
}

function appSizeSectionSegment(entry) {
  return appSizeFirstText(entry.segment, entry.segmentName, entry.segment_name);
}

function appSizeSectionBareName(entry) {
  return appSizeFirstText(entry.name, entry.section, entry.sectionName, entry.section_name, entry.id);
}

function appSizeAliasCount(entry) {
  if (Array.isArray(entry.aliases)) return entry.aliases.length;
  if (Array.isArray(entry.aliasNames)) return entry.aliasNames.length;
  if (Array.isArray(entry.alias_names)) return entry.alias_names.length;
  return appSizeFirstNumber(entry.aliasCount, entry.alias_count, entry.aliases, entry.aliased);
}

function appSizeZeroFill(entry) {
  const explicit = entry.zeroFill ?? entry.zero_fill ?? entry.zerofill
    ?? entry.isZeroFill ?? entry.is_zero_fill;
  if (explicit === true || explicit === false) return explicit;
  const kind = appSizeFirstText(entry.kind, entry.type, entry.sectionKind, entry.section_kind).toLowerCase();
  return ["uninitializeddata", "uninitialized_data", "uninitialized", "nobits", "bss",
    "zerofill", "zero_fill", "szero_fill", "threadlocalzerofill"].includes(kind);
}

function appSizeSymbolGroupName(name) {
  const text = String(name || "").replace(/^</, "");
  const head = (text.split("::")[0] || text).split(" as ")[0].trim();
  return (head || "(unknown)").slice(0, 160);
}

function appSizeSectionName(entry) {
  const segment = appSizeFirstText(entry.segment, entry.segmentName, entry.segment_name);
  const name = appSizeFirstText(entry.name, entry.section, entry.sectionName, entry.section_name, entry.id);
  if (segment && name && name !== segment && !name.startsWith(`${segment},`)) return `${segment},${name}`;
  return name || segment || "";
}

function appSizeSymbolName(entry) {
  return appSizeFirstText(
    entry.demangled,
    entry.demangledName,
    entry.demangled_name,
    entry.displayName,
    entry.display_name,
    entry.name,
    entry.symbol,
    entry.symbolName,
    entry.symbol_name,
  );
}

function appSizeSymbolRawName(entry) {
  return appSizeFirstText(
    entry.mangled,
    entry.mangledName,
    entry.mangled_name,
    entry.rawName,
    entry.raw_name,
    entry.raw,
    entry.symbol,
    entry.symbolName,
    entry.symbol_name,
    entry.name,
  );
}

function appSizeWarnings(root) {
  const warnings = [];
  const add = value => {
    if (typeof value === "string" && value.trim()) warnings.push(value.trim());
  };
  if (Array.isArray(root.warnings)) root.warnings.forEach(add);
  if (Array.isArray(root.errors)) root.errors.forEach(add);
  add(root.warning);
  add(root.warningsMessage);
  add(root.symbolError);
  add(root.symbol_error);
  add(root.sectionError);
  add(root.section_error);
  add(root.groupError);
  add(root.group_error);
  if (typeof root.error === "string") add(root.error);
  else if (isObjectValue(root.error) && root.available !== false) add(root.error.message);
  return [...new Set(warnings)];
}

function normalizeAppSize(data) {
  const root = appSizeRoot(data);
  if (!root) return null;
  const reason = appSizeUnavailableReason(root);
  if (reason) return { unavailable: true, reason };
  const executable = isObjectValue(root.executable)
    ? root.executable
    : isObjectValue(root.binary)
      ? root.binary
      : {};
  const rawSections = appSizeList(root, executable, "sections", "sectionTable", "section_table");
  const rawSymbols = appSizeList(root, executable, "symbols", "topSymbols", "top_symbols", "symbolTable", "symbol_table");
  const rawGroups = appSizeList(root, executable, "groups", "crates", "crateGroups", "crate_groups", "topGroups", "top_groups");
  const containmentRoot = isObjectValue(root.containment)
    ? root.containment
    : isObjectValue(executable.containment)
      ? executable.containment
      : null;
  const rawSegments = appSizeListFrom(
    [containmentRoot, root, executable],
    "segments",
    "segmentTable",
    "segment_table",
  );
  const sections = rawSections.map(entry => {
    if (!isObjectValue(entry)) return null;
    const name = appSizeSectionName(entry);
    const kind = appSizeFirstText(entry.kind, entry.type, entry.sectionKind, entry.section_kind, entry.flags);
    const fileBytes = appSizeFirstNumber(
      entry.fileBytes,
      entry.file_bytes,
      entry.fileSize,
      entry.file_size,
    );
    const vmBytes = appSizeFirstNumber(
      entry.vmBytes,
      entry.vm_bytes,
      entry.vmSize,
      entry.vm_size,
      entry.virtualSize,
      entry.virtual_size,
      entry.memorySize,
      entry.memory_size,
    );
    const sizeBytes = appSizeFirstNumber(
      entry.sizeBytes,
      entry.size_bytes,
      entry.bytes,
      entry.size,
      entry.sectionSize,
      entry.section_size,
    );
    const address = appSizeAddress(entry.address ?? entry.addr ?? entry.virtualAddress ?? entry.virtual_address ?? entry.vmaddr ?? entry.offset);
    if (!name && sizeBytes === null && fileBytes === null && vmBytes === null && !address) return null;
    return {
      name: name || "Unnamed section",
      section: appSizeSectionBareName(entry),
      segment: appSizeSectionSegment(entry),
      kind,
      sizeBytes: sizeBytes ?? fileBytes ?? vmBytes,
      fileBytes,
      vmBytes,
      zeroFill: appSizeZeroFill(entry),
      index: appSizeFirstNumber(entry.index, entry.sectionIndex, entry.section_index),
      aliases: appSizeAliasCount(entry),
      symbolCount: Array.isArray(entry.symbols)
        ? entry.symbols.length
        : appSizeFirstNumber(entry.symbolCount, entry.symbol_count, entry.symbols),
      address,
    };
  }).filter(Boolean);
  const symbols = rawSymbols.map(entry => {
    if (!isObjectValue(entry)) return null;
    const name = appSizeSymbolName(entry);
    if (!name) return null;
    const rawName = appSizeSymbolRawName(entry);
    const sizeBytes = appSizeFirstNumber(
      entry.sizeBytes,
      entry.size_bytes,
      entry.bytes,
      entry.size,
      entry.symbolSize,
      entry.symbol_size,
    );
    return {
      name,
      rawName: rawName && rawName !== name ? rawName : "",
      sizeBytes,
      crateName: appSizeFirstText(
        entry.crate,
        entry.crateName,
        entry.crate_name,
        entry.group,
        entry.groupName,
        entry.group_name,
        entry.packageName,
        entry.package_name,
        entry.module,
      ),
      section: appSizeFirstText(entry.section, entry.sectionName, entry.section_name),
      sectionIndex: appSizeFirstNumber(entry.sectionIndex, entry.section_index),
      segment: appSizeFirstText(entry.segment, entry.segmentName, entry.segment_name),
      address: appSizeAddress(entry.address ?? entry.addr),
      aliases: appSizeAliasCount(entry),
      inferred: entry.inferredSize === true || entry.inferred_size === true,
    };
  }).filter(Boolean);
  const groups = rawGroups.map(entry => {
    if (!isObjectValue(entry)) return null;
    const name = appSizeFirstText(entry.name, entry.crate, entry.crateName, entry.crate_name, entry.group, entry.package, entry.packageName, entry.id);
    if (!name) return null;
    const sizeBytes = appSizeFirstNumber(
      entry.sizeBytes,
      entry.size_bytes,
      entry.bytes,
      entry.size,
      entry.totalBytes,
      entry.total_bytes,
    );
    const symbolCount = Array.isArray(entry.symbols)
      ? entry.symbols.length
      : appSizeFirstNumber(entry.symbols, entry.symbolCount, entry.symbol_count, entry.count, entry.functions);
    return { name, sizeBytes, symbolCount };
  }).filter(Boolean);
  const totalBytes = appSizeFirstNumber(
    root.totalBytes,
    root.total_bytes,
    root.totalSizeBytes,
    root.total_size_bytes,
    root.executableBytes,
    root.executable_bytes,
    root.fileBytes,
    root.file_bytes,
    root.sizeBytes,
    root.size_bytes,
    root.fileSize,
    root.file_size,
    executable.totalBytes,
    executable.total_bytes,
    executable.sizeBytes,
    executable.size_bytes,
    executable.bytes,
    executable.size,
  );
  const path = appSizeFirstText(
    root.executablePath,
    root.executable_path,
    root.path,
    root.binaryPath,
    root.binary_path,
    typeof root.executable === "string" ? root.executable : "",
    executable.path,
    executable.file,
    executable.filePath,
    executable.file_path,
  );
  const sectionBytes = appSizeFirstNumber(root.sectionBytes, root.section_bytes, executable.sectionBytes, executable.section_bytes);
  const symbolBytes = appSizeFirstNumber(root.symbolBytes, root.symbol_bytes, executable.symbolBytes, executable.symbol_bytes);
  const segmentsTotal = appSizeReportedTotalFrom([containmentRoot, root, executable], "segments");
  const segmentsTruncated = appSizeTruncationFlagFrom([containmentRoot, root, executable], "segments");
  const sectionsInfo = appSizeListCoverage(
    Math.min(sections.length, MAX_APP_SIZE_SECTIONS),
    rawSections.length,
    appSizeReportedTotal(root, "sections"),
    appSizeTruncationFlag(root, "sections"),
  );
  const symbolsInfo = appSizeListCoverage(
    Math.min(symbols.length, MAX_APP_SIZE_SYMBOLS),
    rawSymbols.length,
    appSizeReportedTotal(root, "symbols"),
    appSizeTruncationFlag(root, "symbols"),
  );
  const segmentsInfo = appSizeListCoverage(
    Math.min(rawSegments.length, MAX_APP_SIZE_SEGMENTS),
    rawSegments.length,
    segmentsTotal,
    segmentsTruncated,
  );
  const containment = appSizeBuildContainment({
    root: containmentRoot || root,
    sections,
    symbols,
    rawSegments,
    fileBytes: totalBytes,
    symbolBytes,
    segmentInfo: segmentsInfo,
    sectionInfo: sectionsInfo,
    symbolInfo: symbolsInfo,
  });
  const executableMeta = {
    path,
    pid: appSizeFirstNumber(root.pid, root.processId, root.process_id, root.process?.pid, executable.pid),
    platform: appSizeFirstText(root.platform, executable.platform),
    format: appSizeFirstText(root.format, root.fileFormat, root.file_format, root.objectFormat, root.object_format, executable.format, executable.fileFormat, executable.file_format),
    arch: appSizeFirstText(root.arch, root.architecture, executable.arch, executable.architecture),
    kind: appSizeFirstText(root.kind, root.fileKind, root.file_kind, executable.kind),
    label: appSizeFirstText(root.label, root.measurementLabel, root.measurement_label),
    attribution: appSizeFirstText(root.attribution, root.measurementAttribution, root.measurement_attribution),
    measurement: appSizeFirstText(root.measurement, root.measurementKind, root.measurement_kind),
    sectionBytes,
    symbolBytes,
    totalBytes,
    sections,
    symbols,
    groups,
    containment,
    truncation: {
      sections: sectionsInfo,
      symbols: symbolsInfo,
      groups: {
        shown: Math.min(groups.length, MAX_APP_SIZE_GROUPS),
        returned: rawGroups.length,
        total: appSizeReportedTotal(root, "groups"),
        truncated: appSizeTruncationFlag(root, "groups") === true,
      },
      segments: segmentsInfo,
    },
    warnings: appSizeWarnings(root),
  };
  if (totalBytes === null && !sections.length && !symbols.length && !groups.length && !path) return null;
  return executableMeta;
}

function appSizeTruncationNotice(info, label) {
  if (!info) return null;
  let message = "";
  if (info.total !== null && info.total > info.shown) {
    message = `Showing ${formatCount(info.shown)} of ${formatCount(info.total)} ${label}.`;
  } else if (info.shown < info.returned) {
    message = `Showing ${formatCount(info.shown)} of ${formatCount(info.returned)} returned ${label}.`;
  } else if (info.truncated === true) {
    message = `The controller capped the ${label} list at ${formatCount(info.shown)} entries.`;
  }
  return message ? el("div", { class: "panel-block-body" }, el("p", { class: "app-size-note", text: message })) : null;
}

