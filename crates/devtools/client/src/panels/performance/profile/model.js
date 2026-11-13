/* CPU profile */

function isObjectValue(value) {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function firstStringValue(...values) {
  for (const value of values) {
    if (typeof value === "string" && value.trim()) return value;
  }
  return "";
}

function firstNumberValue(...values) {
  for (const value of values) {
    const number = numberValue(value);
    if (number !== null) return number;
  }
  return null;
}

function firstArrayValue(...values) {
  for (const value of values) {
    if (Array.isArray(value)) return value;
  }
  return [];
}

function formatCount(value) {
  const number = numberValue(value);
  return number === null ? "—" : Math.round(number).toLocaleString();
}

function fmtSeconds(value) {
  const number = numberValue(value);
  if (number === null) return "—";
  if (number < 1000) return `${number.toFixed(0)} ms`;
  return `${(number / 1000).toFixed(number < 10000 ? 2 : 1)} s`;
}

function formatPercent(value, digits = 1) {
  const number = numberValue(value);
  if (number === null) return "—";
  const percent = number > 1.5 ? number : number * 100;
  return `${percent.toFixed(digits)}%`;
}

function formatAddress(value) {
  if (typeof value === "string" && value.trim()) return value.trim();
  const number = numberValue(value);
  return number === null ? "" : `0x${Math.max(0, Math.trunc(number)).toString(16)}`;
}

function profilePlatformName(target = targetById()) {
  const raw = String(state.performance?.profile?.platform || target?.platform || state.platform || "").toLowerCase();
  if (raw.includes("mac") || raw === "darwin" || raw === "osx") return "macos";
  if (raw.includes("windows") || raw === "win32" || raw === "win64") return "windows";
  if (raw.includes("linux")) return "linux";
  return raw;
}

function profilePlatformUnsupported(target = targetById()) {
  const platform = profilePlatformName(target);
  if (platform === "linux") {
    return "CPU sampling is not implemented on Linux in this build. The sampler reports Unsupported and synthesizes no samples; use a macOS host or an external system profiler.";
  }
  if (platform === "windows") {
    return "CPU sampling is not implemented on Windows in this build. The sampler reports Unsupported and synthesizes no samples; use a macOS host or an external system profiler.";
  }
  return "";
}

function normalizeProfileStatus(value, hasSamples) {
  const status = String(value ?? "").trim().toLowerCase();
  if (["sampling", "running", "recording", "active", "started"].includes(status)) return "sampling";
  if (["stopping", "finalizing"].includes(status)) return "stopping";
  if (["complete", "completed", "stopped", "finished", "done", "ready"].includes(status)) return "complete";
  if (status === "ok") return hasSamples ? "complete" : "idle";
  if (["idle", "ready", "not_started", "not-started", "none"].includes(status)) return "idle";
  if (["unsupported", "not_supported", "not-supported"].includes(status)) return "unsupported";
  if (["unavailable", "disabled", "no_profile"].includes(status)) return "unavailable";
  if (["error", "failed", "failure"].includes(status)) return "error";
  if (hasSamples) return "complete";
  return status ? status : "idle";
}

function profilePayloadRoot(data) {
  const outer = isObjectValue(data) ? data : {};
  if (isObjectValue(outer.profile)) return outer.profile;
  if (isObjectValue(outer.cpuProfile)) return outer.cpuProfile;
  if (isObjectValue(outer.cpu_profile)) return outer.cpu_profile;
  if (isObjectValue(outer.result)) return outer.result;
  if (isObjectValue(outer.data)) return outer.data;
  return outer;
}

function normalizeProfileFrame(frame) {
  if (typeof frame === "string") {
    return { label: frame, function: frame, file: "", line: null, address: "" };
  }
  if (!isObjectValue(frame)) {
    return { label: "unknown frame", function: "", file: "", line: null, address: "" };
  }
  const location = isObjectValue(frame.location) ? frame.location : {};
  const source = isObjectValue(frame.source) ? frame.source : {};
  const functionName = firstStringValue(
    frame.function,
    frame.functionName,
    frame.function_name,
    frame.name,
    frame.symbol,
    frame.label,
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
  );
  const line = firstNumberValue(frame.line, frame.lineNumber, frame.line_number, location.line, location.lineNumber);
  const address = formatAddress(frame.address ?? frame.pc ?? frame.instruction);
  const label = firstStringValue(
    frame.label,
    functionName && file ? `${functionName} ${file}${line === null ? "" : `:${Math.trunc(line)}`}` : "",
    functionName,
    file ? `${file}${line === null ? "" : `:${Math.trunc(line)}`}` : "",
    address,
    "unknown frame",
  );
  return { label, function: functionName, file, line, address };
}

function normalizeProfileSample(sample, index) {
  const source = isObjectValue(sample) ? sample : {};
  const rawFrames = firstArrayValue(source.frames, source.stack, source.callFrames, source.call_frames);
  const frames = rawFrames.slice(0, MAX_PROFILE_FRAMES).map(normalizeProfileFrame);
  const offsetMs = firstNumberValue(
    source.offsetMs,
    source.offset_ms,
    source.timeMs,
    source.time_ms,
    source.offsetUs === undefined ? null : numberValue(source.offsetUs) === null ? null : numberValue(source.offsetUs) / 1000,
    source.offset_us === undefined ? null : numberValue(source.offset_us) === null ? null : numberValue(source.offset_us) / 1000,
    source.offsetSeconds === undefined ? null : numberValue(source.offsetSeconds) === null ? null : numberValue(source.offsetSeconds) * 1000,
    source.offset_seconds === undefined ? null : numberValue(source.offset_seconds) === null ? null : numberValue(source.offset_seconds) * 1000,
  );
  return {
    index,
    offsetMs,
    frames,
    count: Math.max(1, firstNumberValue(source.count, source.samples, source.sampleCount) ?? 1),
  };
}

function normalizeProfileSamples(rawSamples) {
  const list = Array.isArray(rawSamples) ? rawSamples : [];
  const slice = list.length > MAX_PROFILE_SAMPLES ? list.slice(-MAX_PROFILE_SAMPLES) : list;
  const samples = slice.map(normalizeProfileSample);
  return {
    samples,
    truncated: list.length > slice.length,
    omitted: Math.max(0, list.length - slice.length),
  };
}

function normalizeProfileStacks(rawStacks) {
  const list = Array.isArray(rawStacks) ? rawStacks : [];
  const rows = [];
  let omitted = 0;
  for (const entry of list) {
    if (!isObjectValue(entry)) continue;
    const rawFrames = firstArrayValue(entry.frames, entry.stack, entry.callFrames);
    const frames = rawFrames.slice(0, MAX_PROFILE_FRAMES).map(normalizeProfileFrame);
    const count = Math.max(1, firstNumberValue(entry.count, entry.samples, entry.sampleCount, entry.selfSamples) ?? 1);
    const offsetMs = firstNumberValue(entry.firstOffsetMs, entry.first_offset_ms, entry.offsetMs, entry.offset_ms);
    rows.push({ frames, count, firstOffsetMs: offsetMs });
    if (rows.length >= MAX_PROFILE_STACKS + 1) {
      omitted += 1;
    }
  }
  const total = rows.reduce((sum, row) => sum + row.count, 0);
  return {
    stacks: rows.slice(0, MAX_PROFILE_STACKS),
    truncated: rows.length > MAX_PROFILE_STACKS,
    omitted,
    total,
  };
}

function rankProfileStacks(samples) {
  const groups = new Map();
  let total = 0;
  for (const sample of samples) {
    const labels = sample.frames.map(frame => frame.label);
    const key = labels.join("\u0000") || "\u0000(no stack captured)";
    let group = groups.get(key);
    if (!group) {
      group = {
        frames: sample.frames.length ? sample.frames : [{ label: "No stack captured", function: "", file: "", line: null, address: "" }],
        count: 0,
        firstOffsetMs: sample.offsetMs,
      };
      groups.set(key, group);
    }
    group.count += sample.count;
    if (group.firstOffsetMs === null && sample.offsetMs !== null) group.firstOffsetMs = sample.offsetMs;
    total += sample.count;
  }
  const rows = [...groups.values()].sort((left, right) => right.count - left.count || (left.firstOffsetMs ?? 0) - (right.firstOffsetMs ?? 0));
  return {
    stacks: rows.slice(0, MAX_PROFILE_STACKS),
    truncated: rows.length > MAX_PROFILE_STACKS,
    omitted: Math.max(0, rows.length - MAX_PROFILE_STACKS),
    total,
  };
}

function normalizeProfilePayload(data, options = {}) {
  const outer = isObjectValue(data) ? data : {};
  const root = profilePayloadRoot(data);
  const rawSamples = firstArrayValue(root.samples, outer.samples);
  const rawStacks = firstArrayValue(root.stacks, root.hotStacks, root.hot_stacks, outer.stacks);
  const hasSamples = rawSamples.length > 0 || rawStacks.length > 0;
  const status = normalizeProfileStatus(
    firstStringValue(root.status, outer.status, root.state, outer.state),
    hasSamples,
  );
  const supportedValue = root.supported ?? outer.supported;
  const availableValue = root.available ?? outer.available;
  const reason = firstStringValue(
    root.reason,
    root.message,
    outer.reason,
    outer.message,
    isObjectValue(root.error) ? root.error.message : "",
    isObjectValue(outer.error) ? outer.error.message : "",
  );
  const intervalMs = firstNumberValue(
    root.intervalMs,
    root.interval_ms,
    root.sampleIntervalMs,
    root.sample_interval_ms,
    outer.intervalMs,
    outer.interval_ms,
    outer.sampleIntervalMs,
    outer.sample_interval_ms,
    root.intervalUs === undefined ? null : numberValue(root.intervalUs) === null ? null : numberValue(root.intervalUs) / 1000,
    outer.intervalUs === undefined ? null : numberValue(outer.intervalUs) === null ? null : numberValue(outer.intervalUs) / 1000,
  );
  const hz = firstNumberValue(
    root.hz,
    root.rateHz,
    root.rate_hz,
    root.sampleHz,
    root.sample_hz,
    outer.hz,
    outer.rateHz,
    intervalMs === null || intervalMs <= 0 ? null : 1000 / intervalMs,
  );
  const elapsedMs = firstNumberValue(
    root.elapsedMs,
    root.elapsed_ms,
    root.elapsed,
    outer.elapsedMs,
    outer.elapsed_ms,
    root.durationMs,
    root.duration_ms,
    outer.durationMs,
    outer.duration_ms,
  );
  const durationMs = firstNumberValue(
    root.durationMs,
    root.duration_ms,
    root.samplingDurationMs,
    outer.durationMs,
    outer.duration_ms,
  );
  const totalSamples = firstNumberValue(
    root.samplesTaken,
    root.samples_taken,
    root.totalSamples,
    root.total_samples,
    root.sampleCount,
    root.sample_count,
    outer.samplesTaken,
    outer.samples_taken,
    outer.totalSamples,
  );
  const dropped = firstNumberValue(
    root.samplesDropped,
    root.samples_dropped,
    root.dropped,
    outer.samplesDropped,
    outer.samples_dropped,
  );
  const rate = firstNumberValue(
    root.samplesPerSecond,
    root.samples_per_second,
    outer.samplesPerSecond,
    outer.samples_per_second,
  );
  const retainedSamples = firstNumberValue(
    root.retainedSamples,
    root.retained_samples,
    outer.retainedSamples,
    outer.retained_samples,
  );
  const stacksTruncated = root.stacksTruncated === true
    || root.stacks_truncated === true
    || outer.stacksTruncated === true
    || outer.stacks_truncated === true
    || root.truncated === true
    || outer.truncated === true;
  return {
    status,
    supported: supportedValue === undefined ? null : supportedValue === true,
    available: availableValue === undefined ? null : availableValue === true,
    reason,
    thread: firstStringValue(root.thread, root.threadName, root.thread_name, outer.thread, outer.threadName),
    platform: firstStringValue(root.platform, outer.platform),
    intervalMs,
    hz,
    elapsedMs,
    durationMs,
    totalSamples,
    dropped,
    rate,
    samples: rawSamples,
    stacks: rawStacks,
    retainedSamples,
    stacksTruncated,
    truncated: root.truncated === true || outer.truncated === true,
    id: firstStringValue(root.id, root.profileID, root.profile_id, outer.id, outer.profileID),
  };
}

function applyProfilePayload(data, options = {}) {
  const profile = state.performance.profile;
  const normalized = normalizeProfilePayload(data, options);
  if (normalized.supported === false && normalized.status !== "complete") {
    profile.status = "unsupported";
  } else if (normalized.available === false && normalized.status !== "complete") {
    profile.status = "unavailable";
  } else if (normalized.status) {
    profile.status = normalized.status;
  }
  if (profile.status === "idle" && (profile.samplesTaken > 0 || profile.stacks.length > 0 || profile.samples.length > 0)) {
    profile.status = "complete";
  }
  if (normalized.reason) profile.reason = normalized.reason;
  if (normalized.thread) profile.thread = normalized.thread;
  if (normalized.platform) profile.platform = normalized.platform;
  // An idle answer describes no run: its hz/interval are runtime defaults, not
  // the rate the user selected, so they must not overwrite the controls.
  const describesRun = ["sampling", "stopping", "complete"].includes(profile.status) || (normalized.totalSamples ?? 0) > 0;
  if (describesRun) {
    if (normalized.intervalMs !== null) profile.intervalMs = normalized.intervalMs;
    if (normalized.hz !== null && normalized.hz > 0) profile.hz = normalized.hz;
  }
  if (profile.intervalMs === null && profile.hz > 0) profile.intervalMs = 1000 / profile.hz;
  if (normalized.elapsedMs !== null) profile.elapsedMs = normalized.elapsedMs;
  if (normalized.durationMs !== null) profile.reportedDurationMs = normalized.durationMs;
  if (normalized.totalSamples !== null) profile.samplesTaken = normalized.totalSamples;
  if (normalized.dropped !== null) profile.samplesDropped = normalized.dropped;
  if (normalized.rate !== null) profile.samplesPerSecond = normalized.rate;
  if (normalized.id) profile.id = normalized.id;
  if (options.includeSamples !== false) {
    if (normalized.samples.length) {
      const normalizedSamples = normalizeProfileSamples(normalized.samples);
      profile.samples = normalizedSamples.samples;
      profile.samplesTruncated = normalizedSamples.truncated;
      profile.samplesOmitted = normalizedSamples.omitted;
      const ranked = rankProfileStacks(profile.samples);
      profile.stacks = ranked.stacks;
      profile.stackTotalSamples = ranked.total;
      profile.stacksTruncated = ranked.truncated;
      profile.stacksOmitted = ranked.omitted;
    } else if (normalized.stacks.length) {
      const normalizedStacks = normalizeProfileStacks(normalized.stacks);
      profile.samples = [];
      profile.stacks = normalizedStacks.stacks;
      profile.stackTotalSamples = normalizedStacks.total;
      profile.stacksTruncated = normalizedStacks.truncated || normalized.stacksTruncated;
      profile.stacksOmitted = normalizedStacks.omitted;
      profile.samplesTruncated = false;
      profile.samplesOmitted = 0;
    }
  }
  profile.retainedSamples = normalized.retainedSamples ?? (profile.samples.length || profile.stackTotalSamples || 0);
  if (profile.status === "complete") {
    profile.loaded = true;
    profile.completedAt = profile.completedAt || Date.now();
    if (profile.elapsedMs <= 0 && profile.reportedDurationMs !== null) profile.elapsedMs = profile.reportedDurationMs;
  }
  if (profile.samplesTaken < profile.stackTotalSamples) profile.samplesTaken = profile.stackTotalSamples;
  return normalized;
}

function profileElapsedMs(profile = state.performance.profile) {
  if (profile.status === "sampling" && profile.startedAt) {
    return Math.max(profile.elapsedMs, Date.now() - profile.startedAt);
  }
  return profile.elapsedMs;
}

function clearProfileTimers(profile = state.performance?.profile) {
  if (!profile) return;
  if (profile.pollTimer) clearInterval(profile.pollTimer);
  if (profile.stopTimer) clearTimeout(profile.stopTimer);
  profile.pollTimer = null;
  profile.stopTimer = null;
  profile.polling = false;
}

function profileStatusLabel(profile) {
  switch (profile.status) {
    case "sampling": return "Running";
    case "starting": return "Starting";
    case "stopping": return "Stopping";
    case "complete": return "Ready";
    case "unsupported": return "Unsupported";
    case "unavailable": return "Unavailable";
    case "error": return "Error";
    default: return profile.loading ? "Reading" : "Idle";
  }
}

function profileStatusTone(profile) {
  switch (profile.status) {
    case "sampling":
    case "starting": return "bad";
    case "complete": return "good";
    case "unsupported":
    case "unavailable":
    case "error": return "bad";
    case "stopping": return "accent";
    default: return "muted";
  }
}

function startProfilePolling(profile) {
  clearProfileTimers(profile);
  if (profile.status !== "sampling" || !state.targetId) return;
  profile.polling = true;
  profile.pollTimer = setInterval(() => { pollProfile().catch(() => {}); }, PROFILE_POLL_MS);
  if (profile.durationMs > 0) {
    profile.stopTimer = setTimeout(() => {
      if (state.performance.profile === profile && profile.status === "sampling") {
        stopProfile({ auto: true }).catch(() => {});
      }
    }, profile.durationMs + 100);
  }
}

async function ensureProfile() {
  const perf = state.performance;
  const profile = perf.profile;
  if (!state.token || perf.snapshot || !state.targetId) return;
  if (!state.caps.runtime || !state.caps.profile) return;
  if (profilePlatformUnsupported()) return;
  if (profile.loading || profile.status === "sampling" || profile.status === "starting" || profile.status === "stopping") return;
  if (profile.loaded || profile.status === "complete" || profile.status === "unsupported" || profile.status === "unavailable") {
    renderPerformance();
    return;
  }
  await loadProfile({ quiet: true });
}

async function loadProfile(options = {}) {
  const perf = state.performance;
  const profile = perf.profile;
  const targetId = state.targetId;
  if (!state.token || !targetId || perf.snapshot) return;
  if (!state.caps.profile && options.force !== true) return;
  if (profilePlatformUnsupported()) return;
  if (profile.loading) return;
  const request = profile.loadRequest + 1;
  profile.loadRequest = request;
  profile.loading = true;
  profile.error = null;
  renderPerformance();
  try {
    const data = await rpc("get_ui_profile", { target: targetId });
    if (request !== profile.loadRequest || targetId !== state.targetId || state.performance.profile !== profile) return;
    applyProfilePayload(data, { includeSamples: true });
    if (profile.status === "sampling" || profile.status === "starting") {
      profile.status = "sampling";
      if (!profile.startedAt) profile.startedAt = Date.now() - Math.max(0, profile.elapsedMs || 0);
      startProfilePolling(profile);
    } else if (profile.status === "complete") {
      profile.loaded = true;
      await loadHotspots({ quiet: options.quiet === true });
    }
  } catch (error) {
    if (request !== profile.loadRequest || targetId !== state.targetId || state.performance.profile !== profile) return;
    if (profileErrorIsUnsupported(error)) {
      profile.status = "unsupported";
      profile.reason = errorMessage(error);
    } else if (profileErrorIsUnavailable(error) || error?.code === "unknown_command") {
      profile.status = "unavailable";
      profile.reason = error?.code === "unknown_command" ? "This controller does not expose get_ui_profile." : errorMessage(error);
      profile.error = error;
    } else {
      profile.status = profile.status === "complete" ? "complete" : "error";
      profile.error = error;
      if (!options.quiet) toast(errorMessage(error), "error", 8000);
    }
  } finally {
    if (request === profile.loadRequest && targetId === state.targetId && state.performance.profile === profile) {
      profile.loading = false;
      renderPerformance();
    }
  }
}

