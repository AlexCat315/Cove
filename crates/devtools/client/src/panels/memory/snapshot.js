/* Memory */

const MEMORY_BYTE_LABELS = {
  shapingBytes: ["Shaping cache", "Estimated bytes held by the text shaping cache."],
  layoutBytes: ["Text layout cache", "Estimated bytes held by the text layout cache."],
  imageBytes: ["Image cache", "Bytes held by decoded image assets."],
  targetBytes: ["Render target", "Estimated bytes for owned MSAA, backdrop and effect scratch."],
  msaaLogicalBytes: ["MSAA logical", "Logical MSAA target bytes reported by the host."],
  msaaAllocatedBytes: ["MSAA allocated", "Backend allocation reported for the current MSAA target."],
  processResidentBytes: ["Process RSS", "Resident set size of the host process, read from the operating system."],
  processVirtualBytes: ["Process virtual", "Virtual address space reserved by the host process, read from the operating system."],
};

const MEMORY_PROCESS_COUNTERS = [
  ["processResidentBytes", "Process RSS"],
  ["processVirtualBytes", "Process virtual"],
];

const MEMORY_ENTRY_LABELS = {
  shapingEntries: "Shaping entries",
  layoutEntries: "Text layout entries",
  imageEntries: "Image assets",
  skippedParentUpdates: "Skipped parent updates",
  samples: "Samples",
  hasSwapchain: "Swapchain",
  suspended: "Suspended",
  msaaMemoryless: "Memoryless MSAA",
};

const MEMORY_ALLOCATION_ROWS = [
  {
    label: "Current",
    description: "Bytes held by the host's tracking allocator and not yet freed.",
    bytes: ["allocCurrentBytes", "allocatorCurrentBytes", "currentAllocatedBytes", "allocatorAllocatedBytes", "liveAllocatedBytes", "allocatorLiveBytes"],
    blocks: ["allocLiveBlocks", "allocatorLiveBlocks", "liveBlocks", "allocatorCurrentBlocks", "currentBlocks", "liveAllocations"],
  },
  {
    label: "Peak",
    description: "Highest concurrent allocation the tracking allocator observed.",
    bytes: ["allocPeakBytes", "allocatorPeakBytes", "peakAllocatedBytes", "allocatorMaxBytes"],
    blocks: ["allocPeakLiveBlocks", "allocatorPeakBlocks", "peakBlocks", "peakAllocations"],
  },
  {
    label: "Total",
    description: "Cumulative allocations since tracking started, including blocks that were freed.",
    bytes: ["allocTotalBytes", "allocatorTotalBytes", "totalAllocatedBytes", "allocatedTotalBytes"],
    blocks: ["allocTotalBlocks", "allocatorTotalBlocks", "totalBlocks", "totalAllocations"],
  },
  {
    label: "Deallocated",
    description: "Cumulative bytes and blocks freed since tracking started.",
    bytes: ["allocDeallocatedBytes", "allocatorDeallocatedBytes", "deallocatedBytes", "allocatorFreedBytes", "freedBytes"],
    blocks: ["allocDeallocatedBlocks", "allocatorDeallocatedBlocks", "deallocatedBlocks", "allocatorFreedBlocks", "freedBlocks"],
  },
];

function normalizeMemoryCounterName(name) {
  return String(name).toLowerCase().replace(/[^a-z0-9]/g, "");
}

function memoryAllocationStats(counters) {
  const byName = new Map();
  for (const [name, value] of Object.entries(counters)) {
    const number = numberValue(value);
    if (number !== null) byName.set(normalizeMemoryCounterName(name), { name, value: number });
  }
  const consumed = new Set();
  const resolve = aliases => {
    for (const alias of aliases) {
      const hit = byName.get(normalizeMemoryCounterName(alias));
      if (!hit) continue;
      consumed.add(hit.name);
      return hit.value;
    }
    return null;
  };
  const rows = MEMORY_ALLOCATION_ROWS.map(row => ({
    label: row.label,
    description: row.description,
    bytes: resolve(row.bytes),
    blocks: resolve(row.blocks),
  }));
  const reported = rows.reduce((total, row) => total + Number(row.bytes !== null) + Number(row.blocks !== null), 0);
  return { rows, reported, consumed };
}

function memorySizeClassSourceNumber(source, keys) {
  for (const key of keys) {
    const number = memoryWireNumber(source[key]);
    if (number !== null) return number;
  }
  return null;
}

function memorySizeClassList(source) {
  const entries = [];
  const add = (fallback, item) => {
    const size = memorySizeClassSourceNumber(item, ["size", "class", "blockSize", "sizeBytes", "block"]);
    const bytes = memorySizeClassSourceNumber(item, ["bytes", "liveBytes", "allocatedBytes"]);
    const blocks = memorySizeClassSourceNumber(item, ["blocks", "liveBlocks", "count", "allocations"]);
    if (bytes === null && blocks === null) return;
    const named = typeof fallback === "string" && fallback.trim() !== "" ? fallback.trim() : null;
    entries.push({ label: size === null ? (named || "class") : String(size), bytes, blocks });
  };
  if (Array.isArray(source)) {
    for (const item of source) {
      if (!isObjectValue(item)) continue;
      add(firstStringValue(item.size, item.class, item.blockSize, item.name, item.id), item);
    }
  } else if (isObjectValue(source)) {
    for (const [key, item] of Object.entries(source)) {
      if (isObjectValue(item)) add(key, item);
    }
  }
  return entries;
}

function memorySizeClassCounters(counters, consumed) {
  const classes = new Map();
  for (const [name, value] of Object.entries(counters)) {
    const match = /^(?:allocator|alloc)?sizeclass(?:es)?([a-z0-9]+?)(bytes|blocks)$/.exec(normalizeMemoryCounterName(name));
    if (!match) continue;
    const number = numberValue(value);
    if (number === null) continue;
    consumed.add(name);
    const entry = classes.get(match[1]) || { label: match[1], bytes: null, blocks: null };
    entry[match[2]] = (entry[match[2]] ?? 0) + number;
    classes.set(match[1], entry);
  }
  return [...classes.values()];
}

function memorySizeClasses(root, counters, consumed) {
  const rawCounters = isObjectValue(root.counters) ? root.counters : null;
  const sources = [
    root.sizeClasses,
    root.allocatorSizeClasses,
    isObjectValue(root.allocator) ? root.allocator.sizeClasses : null,
    rawCounters ? rawCounters.sizeClasses : null,
    rawCounters ? rawCounters.allocatorSizeClasses : null,
  ];
  for (const source of sources) {
    const entries = memorySizeClassList(source);
    if (entries.length) return entries;
  }
  return memorySizeClassCounters(counters, consumed);
}

function memorySizeClassLabel(label) {
  const text = String(label).trim();
  const number = Number(text);
  if (text !== "" && Number.isFinite(number)) return `${formatCount(number)} B`;
  const unit = suffix => (suffix === "kib" ? "KiB" : suffix === "mib" ? "MiB" : "GiB");
  const sized = /^(\d+)(kib|mib|gib)$/i.exec(text);
  if (sized) return `${sized[1]} ${unit(sized[2].toLowerCase())}`;
  const over = /^over(\d+)(kib|mib|gib)$/i.exec(text);
  if (over) return `Over ${over[1]} ${unit(over[2].toLowerCase())}`;
  return text;
}

function memorySizeClassSortValue(label) {
  const text = String(label).trim().toLowerCase();
  const number = Number(text);
  if (text !== "" && Number.isFinite(number)) return number;
  const sized = /^(\d+)(kib|mib|gib)$/.exec(text);
  if (sized) {
    const scale = sized[2] === "kib" ? 1024 : sized[2] === "mib" ? 1024 * 1024 : 1024 * 1024 * 1024;
    return Number(sized[1]) * scale;
  }
  return Number.POSITIVE_INFINITY;
}

function memoryWireNumber(value) {
  const direct = numberValue(value);
  if (direct !== null) return direct;
  if (typeof value === "string" && value.trim() !== "") return numberValue(Number(value));
  if (isObjectValue(value)) {
    return firstNumberValue(value.bytes, value.value, value.count, value.amount);
  }
  return null;
}

function readMemoryCounterMap(source) {
  const counters = {};
  if (!isObjectValue(source)) return counters;
  for (const [key, value] of Object.entries(source)) {
    const number = memoryWireNumber(value);
    if (number !== null) counters[key] = number;
  }
  return counters;
}

function normalizeMemorySnapshot(data) {
  const root = isObjectValue(data) && isObjectValue(data.data) ? data.data : data;
  if (!isObjectValue(root)) return null;
  const counters = readMemoryCounterMap(root.counters);
  const windows = new Map();
  const addWindow = (id, key, value) => {
    const number = memoryWireNumber(value);
    if (number === null) return;
    if (!windows.has(id)) windows.set(id, {});
    windows.get(id)[key] = number;
  };
  const rawWindows = root.windows;
  if (Array.isArray(rawWindows)) {
    for (const entry of rawWindows) {
      if (!isObjectValue(entry)) continue;
      const id = firstStringValue(entry.id, entry.window, entry.windowID, entry.window_id, entry.name, "window");
      const source = isObjectValue(entry.counters) ? entry.counters : entry;
      for (const [key, value] of Object.entries(source)) {
        if (["id", "window", "windowID", "window_id", "name", "counters"].includes(key)) continue;
        addWindow(id, key, value);
      }
    }
  } else if (isObjectValue(rawWindows)) {
    for (const [key, value] of Object.entries(rawWindows)) {
      const dot = key.lastIndexOf(".");
      if (dot > 0) {
        const encoded = memoryWireNumber(value);
        if (encoded !== null) {
          addWindow(key.slice(0, dot), key.slice(dot + 1), encoded);
          continue;
        }
      }
      if (isObjectValue(value)) {
        for (const [counter, count] of Object.entries(value)) addWindow(key, counter, count);
        continue;
      }
      const number = memoryWireNumber(value);
      if (number !== null) addWindow(key, "value", number);
    }
  }
  const allocation = memoryAllocationStats(counters);
  const sizeClasses = memorySizeClasses(root, counters, allocation.consumed);
  return { counters, windows, allocation, sizeClasses, pressure: readMemoryPressure(root.pressure) };
}

// An unsubscribed signal is not a measured "normal": the chart must draw no
// pressure mark for it, and the stats must say it was never observed.
function readMemoryPressure(value) {
  if (!isObjectValue(value)) return null;
  const subscribed = booleanOf(value.subscribed);
  const low = booleanOf(value.low);
  if (subscribed === null && low === null) return null;
  return { subscribed: subscribed === true, low: low === true };
}

function recordMemorySample(data) {
  const snapshot = normalizeMemorySnapshot(data);
  if (!snapshot) return;
  const memory = state.memory;
  const counters = snapshot.counters;
  memory.history.push({
    at: Date.now(),
    rss: memoryCounterValue(counters, "processResidentBytes"),
    virtual: memoryCounterValue(counters, "processVirtualBytes"),
    allocated: snapshot.allocation.rows.find(row => row.label === "Current")?.bytes ?? null,
    caches: sumMemoryCounters(counters, ["shapingBytes", "layoutBytes", "imageBytes"]),
    targets: memoryWindowTotals(snapshot.windows, "targetBytes"),
    pressure: snapshot.pressure,
  });
  if (memory.history.length > MEMORY_SAMPLE_LIMIT) {
    memory.history.splice(0, memory.history.length - MEMORY_SAMPLE_LIMIT);
  }
}

function memoryCounterValue(counters, name) {
  return numberValue(counters[name]);
}

function sumMemoryCounters(counters, names) {
  let total = null;
  for (const name of names) {
    const value = numberValue(counters[name]);
    if (value !== null) total = (total ?? 0) + value;
  }
  return total;
}

function memoryWindowTotals(windows, name) {
  let total = null;
  for (const counters of windows.values()) {
    const value = numberValue(counters[name]);
    if (value !== null) total = (total ?? 0) + value;
  }
  return total;
}

function formatWallTime(milliseconds) {
  const number = numberValue(milliseconds);
  if (number === null) return "—";
  const date = new Date(number);
  if (Number.isNaN(date.getTime())) return "—";
  try {
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
  } catch {
    return date.toLocaleTimeString([], { hour12: false });
  }
}

function allocationSiteErrorKind(error) {
  if (!error) return "error";
  const code = String(error.code || "").toLowerCase();
  const message = String(error.message || "");
  if (["unsupported", "not_supported", "not-supported"].includes(code)
    || /not available on|not supported|unsupported|cannot capture|no stack capture/i.test(message)) return "unsupported";
  if (["unavailable", "unknown_command", "not_implemented"].includes(code)) return "unavailable";
  return "error";
}

function allocationSiteRateValue(value) {
  const number = numberValue(value);
  if (number === null || !Number.isInteger(number) || number < 1 || number > ALLOCATION_RATE_LIMIT) return null;
  return number;
}

function allocationSiteArray(...values) {
  for (const value of values) {
    if (Array.isArray(value) && value.length > 0) return value;
  }
  for (const value of values) {
    if (Array.isArray(value)) return value;
  }
  return [];
}

function allocationSiteFirstDefined(...values) {
  for (const value of values) {
    if (value !== undefined && value !== null && value !== "") return value;
  }
  return undefined;
}

function allocationSiteAddress(value) {
  if (value === undefined || value === null) return "";
  if (typeof value === "number") {
    return Number.isFinite(value) && value >= 0 ? `0x${Math.trunc(value).toString(16)}` : "";
  }
  const text = String(value).trim();
  if (!text) return "";
  if (/^0x[0-9a-f]+$/i.test(text)) return text.toLowerCase();
  if (/^\d+$/.test(text)) {
    try {
      return `0x${BigInt(text).toString(16)}`;
    } catch {
      return text;
    }
  }
  return text;
}

function normalizeAllocationSiteFrame(frame) {
  if (typeof frame === "string" || typeof frame === "number") {
    const text = String(frame).trim();
    const address = allocationSiteAddress(frame);
    return {
      label: text,
      function: "",
      file: "",
      line: null,
      address: /^0x[0-9a-f]+$/i.test(text) ? address : "",
      module: "",
    };
  }
  if (!isObjectValue(frame)) {
    return { label: "", function: "", file: "", line: null, address: "", module: "" };
  }
  const location = isObjectValue(frame.location) ? frame.location : {};
  const source = isObjectValue(frame.source) ? frame.source : {};
  const functionName = firstStringValue(
    frame.function,
    frame.functionName,
    frame.function_name,
    frame.name,
    frame.symbol,
    frame.symbolName,
    frame.symbol_name,
  );
  const file = firstStringValue(
    frame.file,
    frame.filePath,
    frame.file_path,
    location.file,
    location.filePath,
    location.file_path,
    source.file,
    source.filePath,
    source.file_path,
  );
  const line = firstNumberValue(frame.line, frame.lineNumber, frame.line_number, location.line, location.lineNumber);
  const address = allocationSiteAddress(allocationSiteFirstDefined(
    frame.address,
    frame.addressHex,
    frame.address_hex,
    frame.pc,
    frame.ip,
    frame.instruction,
    frame.instructionPointer,
    frame.instruction_pointer,
    frame.returnAddress,
    frame.return_address,
    frame.offset,
  ));
  const module = firstStringValue(frame.module, frame.moduleName, frame.module_name, frame.image, frame.binary);
  const locationLabel = file ? `${file}${line === null ? "" : `:${Math.trunc(line)}`}` : "";
  const label = firstStringValue(
    frame.label,
    functionName && locationLabel ? `${functionName} ${locationLabel}` : "",
    functionName,
    locationLabel,
    address,
  );
  return { label, function: functionName, file, line, address, module };
}

function allocationSiteStackIdentity(frames) {
  return frames
    .map(frame => `${frame.address}\u0000${frame.function}\u0000${frame.file}\u0000${frame.line ?? ""}`)
    .join("\u0001");
}

function normalizeAllocationSite(site, index) {
  if (!isObjectValue(site)) return null;
  const rawFrames = allocationSiteArray(
    site.frames,
    site.stack,
    site.callFrames,
    site.call_frames,
    site.addresses,
    site.returnAddresses,
    site.return_addresses,
  );
  const frames = rawFrames
    .slice(0, MAX_ALLOCATION_SITE_FRAMES)
    .map(normalizeAllocationSiteFrame);
  const reportedOmitted = firstNumberValue(
    site.framesOmitted,
    site.frames_omitted,
    site.omittedFrames,
    site.omitted_frames,
  );
  const framesOmitted = Math.max(0, rawFrames.length - frames.length) + Math.max(0, reportedOmitted ?? 0);
  const framesTruncated = [site.framesTruncated, site.frames_truncated, site.stackTruncated, site.stack_truncated]
    .find(value => typeof value === "boolean") === true;
  const bytes = firstNumberValue(
    site.sampledBytes,
    site.sampled_bytes,
    site.bytes,
    site.totalBytes,
    site.total_bytes,
    site.allocatedBytes,
    site.allocated_bytes,
  );
  const blocks = firstNumberValue(
    site.sampledBlocks,
    site.sampled_blocks,
    site.blocks,
    site.allocations,
    site.allocationCount,
    site.allocation_count,
    site.count,
  );
  const stackKey = allocationSiteStackIdentity(frames);
  const key = stackKey || `site-${index}`;
  return { key, stackKey, frames, framesOmitted, framesTruncated, bytes, blocks, raw: site };
}

function normalizeAllocationSitePayload(data) {
  const empty = {
    status: "",
    rawStatus: "",
    supported: null,
    reason: "",
    symbolizationReason: "",
    rate: null,
    sites: [],
    sampledBytes: null,
    sampledBlocks: null,
    samplesAttempted: null,
    samplesRetained: null,
    samplesDropped: null,
    captureFailures: null,
    totalSites: null,
    sitesOmitted: 0,
    framesOmitted: 0,
    sitesTruncated: false,
    framesTruncated: false,
    hasReport: false,
  };
  if (data === null || data === undefined) return empty;
  const outer = isObjectValue(data) ? data : {};
  const nested = [
    outer.report,
    outer.allocationSites,
    outer.allocation_sites,
    outer.sitesReport,
    outer.sites_report,
    outer.allocation,
    outer.result,
  ].find(isObjectValue);
  const root = nested || outer;
  const siteSource = allocationSiteArray(
    root.sites,
    root.entries,
    root.items,
    root.stacks,
    root.rows,
    root.allocationSites,
    root.allocation_sites,
    outer.sites,
    outer.entries,
    Array.isArray(data) ? data : [],
  );
  const sites = siteSource
    .map(normalizeAllocationSite)
    .filter(Boolean);
  const statusRaw = firstStringValue(
    outer.status,
    outer.state,
    root.status,
    root.state,
    outer.phase,
    root.phase,
  ).trim().toLowerCase();
  const runningFlag = [outer.running, root.running, outer.sampling, root.sampling, outer.active, root.active]
    .find(value => typeof value === "boolean");
  const supportedValue = [root.supported, outer.supported, root.available, outer.available]
    .find(value => typeof value === "boolean");
  const supportText = firstStringValue(
    root.support,
    outer.support,
    root.samplingSupport,
    outer.samplingSupport,
    root.sampling_support,
    outer.sampling_support,
  ).trim().toLowerCase();
  let supported = supportedValue ?? null;
  if (supported === null) {
    if (["unsupported", "not_supported", "not-supported"].includes(supportText)) supported = false;
    else if (["supported", "available", "enabled"].includes(supportText)) supported = true;
  }
  const truncation = isObjectValue(root.truncation) ? root.truncation : {};
  const sitesOmitted = firstNumberValue(
    root.omittedSites,
    root.omitted_sites,
    outer.omittedSites,
    outer.omitted_sites,
    root.sitesOmitted,
    root.sites_omitted,
    truncation.omittedSites,
    truncation.omitted_sites,
    root.omitted,
  );
  const framesOmitted = firstNumberValue(
    root.framesOmitted,
    root.frames_omitted,
    outer.framesOmitted,
    outer.frames_omitted,
    root.omittedFrames,
    root.omitted_frames,
    truncation.omittedFrames,
    truncation.omitted_frames,
  );
  const sampledBytes = firstNumberValue(
    root.sampledBytes,
    root.sampled_bytes,
    outer.sampledBytes,
    outer.sampled_bytes,
    root.totalSampledBytes,
    root.total_sampled_bytes,
    root.allocatedBytes,
    root.allocated_bytes,
  );
  const sampledBlocks = firstNumberValue(
    root.sampledBlocks,
    root.sampled_blocks,
    outer.sampledBlocks,
    outer.sampled_blocks,
    root.totalSampledBlocks,
    root.total_sampled_blocks,
    root.blockCount,
    root.block_count,
  );
  const samplesAttempted = firstNumberValue(
    root.samplesAttempted,
    root.samples_attempted,
    outer.samplesAttempted,
    outer.samples_attempted,
    root.attempted,
    root.attempts,
    outer.attempted,
    outer.attempts,
  );
  const samplesRetained = firstNumberValue(
    root.samplesRetained,
    root.samples_retained,
    outer.samplesRetained,
    outer.samples_retained,
    root.retained,
    root.retainedSamples,
    root.retained_samples,
    outer.retained,
  );
  const samplesDropped = firstNumberValue(
    root.samplesDropped,
    root.samples_dropped,
    outer.samplesDropped,
    outer.samples_dropped,
    root.dropped,
    root.droppedSamples,
    root.dropped_samples,
    root.overwritten,
    outer.dropped,
  );
  const captureFailures = firstNumberValue(
    root.captureFailures,
    root.capture_failures,
    outer.captureFailures,
    outer.capture_failures,
    root.failedCaptures,
    root.failed_captures,
    root.captureFailureCount,
    root.capture_failure_count,
  );
  const totalSites = firstNumberValue(
    root.totalSites,
    root.total_sites,
    outer.totalSites,
    outer.total_sites,
    root.siteCount,
    root.site_count,
    root.total,
  );
  const sitesTruncatedFlag = [root.sitesTruncated, root.sites_truncated, root.truncated, outer.sitesTruncated]
    .find(value => typeof value === "boolean");
  const framesTruncatedFlag = [
    root.framesTruncated,
    root.frames_truncated,
    root.stacksTruncated,
    root.stacks_truncated,
    root.stackTruncated,
    outer.framesTruncated,
    truncation.frames,
  ].find(value => typeof value === "boolean");
  const reason = firstStringValue(
    root.reason,
    root.message,
    outer.reason,
    outer.message,
    isObjectValue(root.error) ? root.error.message : "",
    isObjectValue(outer.error) ? outer.error.message : "",
  );
  const symbolizationReason = firstStringValue(
    root.symbolizationReason,
    root.symbolization_reason,
    outer.symbolizationReason,
    outer.symbolization_reason,
  );
  let status = statusRaw;
  if (["sampling", "running", "active", "started", "recording"].includes(statusRaw)) status = "running";
  else if (["complete", "completed", "stopped", "finished", "done", "ready"].includes(statusRaw)) status = "stopped";
  else if (["unsupported", "not_supported", "not-supported"].includes(statusRaw)) status = "unsupported";
  else if (["unavailable", "disabled", "not_implemented"].includes(statusRaw)) status = "unavailable";
  else if (["error", "failed", "failure"].includes(statusRaw)) status = "error";
  else if (["idle", "inactive", "not_started", "not-started", "none"].includes(statusRaw)) status = "idle";
  else status = "";
  if (runningFlag === true && status !== "unsupported" && status !== "unavailable") status = "running";
  if (supported === false && status !== "running") status = "unsupported";
  const hasReport = status !== "idle"
    && (Boolean(nested)
      || sites.length > 0
      || sampledBytes !== null
      || sampledBlocks !== null
      || samplesAttempted !== null
      || samplesRetained !== null
      || samplesDropped !== null
      || captureFailures !== null
      || status === "stopped");
  const siteFramesTruncated = sites.some(site => site.framesTruncated);
  return {
    status,
    rawStatus: statusRaw,
    supported,
    reason,
    symbolizationReason,
    rate: firstNumberValue(
      root.rate,
      outer.rate,
      root.samplingRate,
      root.sampling_rate,
      root.sampleRate,
      root.sample_rate,
      outer.samplingRate,
      outer.sampling_rate,
    ),
    sites,
    sampledBytes,
    sampledBlocks,
    samplesAttempted,
    samplesRetained,
    samplesDropped,
    captureFailures,
    totalSites,
    sitesOmitted: Math.max(0, sitesOmitted ?? 0),
    framesOmitted: Math.max(0, framesOmitted ?? 0),
    sitesTruncated: sitesTruncatedFlag === true || Math.max(0, sitesOmitted ?? 0) > 0,
    framesTruncated: framesTruncatedFlag === true || Math.max(0, framesOmitted ?? 0) > 0 || siteFramesTruncated,
    hasReport,
  };
}

function allocationSitePayloadRate(payload, fallback) {
  return Number.isInteger(payload.rate) && payload.rate > 0 ? payload.rate : fallback;
}

