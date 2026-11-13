async function pollProfile() {
  const perf = state.performance;
  const profile = perf.profile;
  const targetId = state.targetId;
  if (!state.token || !targetId || perf.snapshot || profile.status !== "sampling" || profile.loading) return;
  const request = profile.loadRequest + 1;
  profile.loadRequest = request;
  try {
    const data = await rpc("get_ui_profile", { target: targetId });
    if (request !== profile.loadRequest || targetId !== state.targetId || state.performance.profile !== profile) return;
    applyProfilePayload(data, { includeSamples: profile.samples.length === 0 });
    if (profile.status === "complete") {
      profile.loaded = true;
      clearProfileTimers(profile);
      await loadHotspots({ quiet: true });
    } else if (profile.status !== "sampling" && profile.status !== "starting") {
      clearProfileTimers(profile);
    }
    profile.error = null;
  } catch (error) {
    if (request !== profile.loadRequest || targetId !== state.targetId || state.performance.profile !== profile) return;
    if (profileErrorIsUnsupported(error)) {
      profile.status = "unsupported";
      profile.reason = errorMessage(error);
      clearProfileTimers(profile);
    } else if (profileErrorIsUnavailable(error) || error?.code === "unknown_command") {
      profile.status = "unavailable";
      profile.reason = error?.code === "unknown_command" ? "This controller does not expose get_ui_profile." : errorMessage(error);
      profile.error = error;
      clearProfileTimers(profile);
    } else {
      profile.error = error;
    }
  } finally {
    if (request === profile.loadRequest && targetId === state.targetId && state.performance.profile === profile) {
      renderPerformance();
    }
  }
}

function profileErrorIsUnsupported(error) {
  if (!error) return false;
  if (["unsupported", "not_supported"].includes(error.code)) return true;
  return /not available on|unsupported|no sampler/i.test(error.message || "");
}

function profileErrorIsUnavailable(error) {
  if (!error) return false;
  return error.code === "unavailable" || /unavailable/i.test(error.message || "");
}

async function startProfile() {
  const perf = state.performance;
  const profile = perf.profile;
  if (perf.snapshot || !state.targetId) return;
  if (profilePlatformUnsupported()) {
    renderPerformance();
    return;
  }
  if (!state.caps.profile) {
    toast("This runtime does not advertise CPU profiling.", "info");
    return;
  }
  if (profile.loading || ["sampling", "starting", "stopping"].includes(profile.status)) return;
  const targetId = state.targetId;
  const request = profile.request + 1;
  profile.request = request;
  profile.loadRequest += 1;
  clearProfileTimers(profile);
  profile.loading = true;
  profile.loaded = false;
  profile.status = "starting";
  profile.error = null;
  profile.reason = "";
  profile.supported = null;
  profile.startedAt = Date.now();
  profile.completedAt = 0;
  profile.elapsedMs = 0;
  profile.samplesTaken = 0;
  profile.samplesDropped = 0;
  profile.samplesPerSecond = null;
  profile.reportedDurationMs = null;
  profile.samples = [];
  profile.samplesTruncated = false;
  profile.samplesOmitted = 0;
  profile.retainedSamples = 0;
  profile.stacks = [];
  profile.stackTotalSamples = 0;
  profile.stacksTruncated = false;
  profile.stacksOmitted = 0;
  profile.selection = null;
  profile.expanded = new Set();
  profile.hotspots = { loading: false, error: null, data: null };
  renderPerformance();
  try {
    const data = await rpc("start_ui_profile", {
      target: targetId,
      hz: profile.hz,
      duration_ms: profile.durationMs,
    });
    if (request !== profile.request || targetId !== state.targetId || state.performance.profile !== profile) return;
    applyProfilePayload(data, { includeSamples: false });
    if (profile.status === "unsupported" || profile.status === "unavailable") {
      clearProfileTimers(profile);
    } else if (profile.status === "complete") {
      profile.loaded = true;
      await loadHotspots({ quiet: true });
    } else {
      profile.status = "sampling";
      if (!profile.startedAt) profile.startedAt = Date.now();
      startProfilePolling(profile);
      toast(`Sampling ${profile.thread || "the UI thread"} at ${profile.hz} Hz. Stop when the operation is complete.`, "success", 5000);
    }
  } catch (error) {
    if (request !== profile.request || targetId !== state.targetId || state.performance.profile !== profile) return;
    if (profileErrorIsUnsupported(error)) {
      profile.status = "unsupported";
      profile.reason = errorMessage(error);
    } else if (profileErrorIsUnavailable(error) || error?.code === "unknown_command") {
      profile.status = "unavailable";
      profile.reason = error?.code === "unknown_command" ? "This controller does not expose start_ui_profile." : errorMessage(error);
      profile.error = error;
    } else {
      profile.status = "error";
      profile.error = error;
      toast(errorMessage(error), "error", 9000);
    }
  } finally {
    if (request === profile.request && targetId === state.targetId && state.performance.profile === profile) {
      profile.loading = false;
      renderPerformance();
    }
  }
}

async function stopProfile(options = {}) {
  const perf = state.performance;
  const profile = perf.profile;
  const targetId = state.targetId;
  if (!targetId || perf.snapshot) return;
  if (!["sampling", "starting", "stopping"].includes(profile.status)) {
    if (!options.quiet) toast("No CPU profile is sampling.", "info");
    return;
  }
  if (profile.loading && profile.status === "stopping") return;
  clearProfileTimers(profile);
  profile.loadRequest += 1;
  profile.loading = true;
  profile.status = "stopping";
  profile.error = null;
  renderPerformance();
  let stopped = false;
  try {
    const data = await rpc("stop_ui_profile", { target: targetId });
    if (targetId !== state.targetId || state.performance.profile !== profile) return;
    applyProfilePayload(data, { includeSamples: true });
    if (profile.status !== "unsupported" && profile.status !== "unavailable") {
      profile.status = "complete";
      profile.loaded = true;
      profile.completedAt = Date.now();
      if (profile.elapsedMs <= 0) profile.elapsedMs = Math.max(0, profile.completedAt - profile.startedAt);
      profile.samplesTaken = Math.max(profile.samplesTaken, profile.stackTotalSamples);
      stopped = true;
    }
  } catch (error) {
    if (targetId !== state.targetId || state.performance.profile !== profile) return;
    if (profileErrorIsUnsupported(error)) {
      profile.status = "unsupported";
      profile.reason = errorMessage(error);
    } else if (profileErrorIsUnavailable(error)) {
      profile.status = "unavailable";
      profile.reason = errorMessage(error);
      profile.error = error;
    } else {
      profile.error = error;
      if (!options.quiet) toast(errorMessage(error), "error", 9000);
    }
  } finally {
    if (targetId === state.targetId && state.performance.profile === profile) {
      profile.loading = false;
      renderPerformance();
    }
  }
  if (!stopped || targetId !== state.targetId || state.performance.profile !== profile) return;
  await loadProfile({ quiet: true });
  if (profile.status === "complete") {
    profile.loaded = true;
    if (profile.samplesTaken === 0 && profile.stackTotalSamples > 0) profile.samplesTaken = profile.stackTotalSamples;
    if (!profile.hotspots.data && !profile.hotspots.loading) await loadHotspots({ quiet: options.quiet === true });
  }
  if (!options.quiet) {
    toast(
      profile.samplesTaken
        ? `CPU profile stopped with ${formatCount(profile.samplesTaken)} samples.`
        : "CPU profile stopped with no samples.",
      profile.samplesTaken ? "success" : "info",
      6000,
    );
  }
  renderPerformance();
}

async function loadHotspots(options = {}) {
  const perf = state.performance;
  const profile = perf.profile;
  const targetId = state.targetId;
  if (!state.token || !targetId || perf.snapshot || !state.caps.profile) return;
  if (profilePlatformUnsupported()) return;
  if (profile.hotspots.loading) return;
  profile.hotspots.loading = true;
  profile.hotspots.error = null;
  renderPerformance();
  try {
    const data = await rpc("query_ui_hotspots", { target: targetId, limit: MAX_HOTSPOTS });
    if (targetId !== state.targetId || state.performance.profile !== profile) return;
    profile.hotspots.data = normalizeHotspots(data, profile);
  } catch (error) {
    if (targetId !== state.targetId || state.performance.profile !== profile) return;
    profile.hotspots.error = error;
    if (error?.code === "unknown_command") {
      profile.hotspots.error = new RpcError(
        "This controller does not expose query_ui_hotspots.",
        "unknown_command",
      );
    }
    if (!options.quiet) toast(errorMessage(error), "error", 8000);
  } finally {
    if (targetId === state.targetId && state.performance.profile === profile) {
      profile.hotspots.loading = false;
      renderPerformance();
    }
  }
}

function normalizeHotspotPercent(value) {
  const number = numberValue(value);
  if (number === null) return null;
  return number > 1.5 ? number : number * 100;
}

function normalizeHotspots(data, profile) {
  if (data === null || data === undefined) {
    return {
      status: "unavailable",
      reason: "The runtime returned no hot-spot data.",
      entries: [],
      total: 0,
      omitted: 0,
      totalSamples: null,
      durationMs: null,
      truncated: false,
    };
  }
  const root = profilePayloadRoot(data);
  const rawEntries = firstArrayValue(
    root.entries,
    root.hotspots,
    isObjectValue(root.hotspots) ? root.hotspots.entries : null,
    root.functions,
    root.items,
    root.rows,
    root.hotFunctions,
    root.hot_functions,
    Array.isArray(data) ? data : [],
    root.top,
  );
  const totalSamples = firstNumberValue(
    root.retainedSamples,
    root.retained_samples,
    root.totalSamples,
    root.total_samples,
    root.samplesTaken,
    root.samples_taken,
    root.sampleCount,
    profile?.retainedSamples,
    profile?.samplesTaken,
  );
  const durationMs = firstNumberValue(root.durationMs, root.duration_ms, profile?.elapsedMs);
  const intervalMs = firstNumberValue(
    root.intervalMs,
    root.interval_ms,
    root.sampleIntervalMs,
    root.sample_interval_ms,
    profile?.intervalMs,
  );
  const status = normalizeProfileStatus(firstStringValue(root.status, root.state), rawEntries.length > 0);
  const reason = firstStringValue(
    root.reason,
    root.message,
    isObjectValue(root.error) ? root.error.message : "",
  );
  const entries = [];
  let omitted = 0;
  for (const entry of rawEntries) {
    if (!isObjectValue(entry)) continue;
    const frame = isObjectValue(entry.frame) ? entry.frame : entry;
    const location = isObjectValue(entry.location) ? entry.location : isObjectValue(frame.location) ? frame.location : {};
    const functionName = firstStringValue(
      entry.function,
      entry.functionName,
      entry.function_name,
      entry.name,
      entry.symbol,
      frame.function,
      frame.name,
      frame.symbol,
    );
    const file = firstStringValue(
      entry.file,
      entry.filePath,
      entry.file_path,
      location.file,
      location.filePath,
      frame.file,
      frame.filePath,
    );
    const line = firstNumberValue(entry.line, entry.lineNumber, location.line, frame.line, frame.lineNumber);
    const selfSamples = firstNumberValue(
      entry.selfSamples,
      entry.self_samples,
      entry.self,
      entry.selfCount,
      entry.self_count,
      entry.samples,
    );
    const inclusiveSamples = firstNumberValue(
      entry.inclusiveSamples,
      entry.inclusive_samples,
      entry.inclusive,
      entry.inclusiveCount,
      entry.inclusive_count,
      entry.totalSamples,
      entry.total_samples,
    );
    const selfMs = firstNumberValue(
      entry.selfMs,
      entry.self_ms,
      entry.selfTimeMs,
      entry.self_time_ms,
      entry.selfUs === undefined ? null : numberValue(entry.selfUs) === null ? null : numberValue(entry.selfUs) / 1000,
      entry.self_us === undefined ? null : numberValue(entry.self_us) === null ? null : numberValue(entry.self_us) / 1000,
      selfSamples !== null && intervalMs !== null ? selfSamples * intervalMs : null,
    );
    const inclusiveMs = firstNumberValue(
      entry.inclusiveMs,
      entry.inclusive_ms,
      entry.inclusiveTimeMs,
      entry.inclusive_time_ms,
      entry.inclusiveUs === undefined ? null : numberValue(entry.inclusiveUs) === null ? null : numberValue(entry.inclusiveUs) / 1000,
      entry.inclusive_us === undefined ? null : numberValue(entry.inclusive_us) === null ? null : numberValue(entry.inclusive_us) / 1000,
      inclusiveSamples !== null && intervalMs !== null ? inclusiveSamples * intervalMs : null,
    );
    const selfPercentValue = firstNumberValue(entry.selfPercent, entry.self_percent);
    const inclusivePercentValue = firstNumberValue(entry.inclusivePercent, entry.inclusive_percent);
    const selfPercent = (selfPercentValue !== null ? selfPercentValue : normalizeHotspotPercent(firstNumberValue(
      entry.selfShare,
      entry.self_share,
      entry.selfRatio,
    ))) ?? (selfSamples !== null && totalSamples ? (selfSamples / totalSamples) * 100 : null);
    const inclusivePercent = (inclusivePercentValue !== null ? inclusivePercentValue : normalizeHotspotPercent(firstNumberValue(
      entry.inclusiveShare,
      entry.inclusive_share,
      entry.inclusiveRatio,
    ))) ?? (inclusiveSamples !== null && totalSamples ? (inclusiveSamples / totalSamples) * 100 : null);
    const label = firstStringValue(
      entry.label,
      functionName && file ? `${functionName} ${file}${line === null ? "" : `:${Math.trunc(line)}`}` : "",
      functionName,
      file,
      formatAddress(entry.address ?? frame.address),
      "unknown frame",
    );
    entries.push({
      function: functionName || label,
      file,
      line,
      label,
      selfSamples,
      inclusiveSamples,
      selfMs,
      inclusiveMs,
      selfPercent,
      inclusivePercent,
    });
    if (entries.length > MAX_HOTSPOTS) omitted += 1;
  }
  const limited = entries.slice(0, MAX_HOTSPOTS);
  return {
    status,
    reason,
    entries: limited,
    total: entries.length,
    omitted,
    totalSamples,
    durationMs,
    truncated: root.truncated === true || entries.length > MAX_HOTSPOTS,
  };
}

function renderProfileControls(profile, options = {}) {
  const disabled = options.disabled === true;
  const sampling = ["sampling", "starting", "stopping"].includes(profile.status);
  const hzRates = PROFILE_HZ_RATES.includes(profile.hz)
    ? PROFILE_HZ_RATES
    : [...PROFILE_HZ_RATES, profile.hz].filter(rate => rate > 0).sort((left, right) => left - right);
  const hz = el("select", {
    props: { id: "profile-hz" },
    attrs: { "aria-label": "CPU profile sample rate" },
    on: {
      change: () => {
        const value = Number(hz.value);
        if (hzRates.includes(value)) profile.hz = value;
      },
    },
  }, ...hzRates.map(rate => new Option(`${rate} Hz`, String(rate))));
  hz.value = String(profile.hz);
  const duration = el("select", {
    props: { id: "profile-duration" },
    attrs: { "aria-label": "CPU profile duration" },
    on: {
      change: () => {
        const value = Number(duration.value);
        if (PROFILE_DURATIONS.includes(value)) profile.durationMs = value;
      },
    },
  }, ...PROFILE_DURATIONS.map(ms => new Option(ms >= 1000 ? `${(ms / 1000).toFixed(0)} s` : `${ms} ms`, String(ms))));
  duration.value = String(profile.durationMs);
  const start = el("button", {
    class: "tool-button is-primary",
    props: { id: "profile-start", type: "button", disabled: disabled || sampling || profile.loading },
    attrs: { title: "Start a real CPU sample of the UI thread" },
    on: { click: () => startProfile() },
  }, icon("record"), el("span", { text: "Start profile" }));
  const stop = el("button", {
    class: "tool-button",
    props: { id: "profile-stop", type: "button", disabled: disabled || !sampling || profile.loading },
    attrs: { title: "Stop sampling and read the profile" },
    on: { click: () => stopProfile() },
  }, icon("stop"), el("span", { text: "Stop" }));
  const refresh = el("button", {
    class: "tool-button",
    props: { id: "profile-refresh", type: "button", disabled: disabled || profile.loading },
    attrs: { title: profile.status === "sampling" ? "Read the current sample count" : "Read the profile and hot spots" },
    on: {
      click: () => {
        if (profile.status === "sampling") pollProfile();
        else loadProfile({ quiet: true });
      },
    },
  }, icon("refresh"), el("span", { text: "Refresh" }));
  return el("div", { class: "profile-controls" },
    el("label", { class: "profile-field" }, "Sample rate", hz),
    el("label", { class: "profile-field" }, "Duration", duration),
    el("div", { class: "profile-actions" },
      profile.status === "sampling"
        ? el("span", { class: "profile-live", text: `${fmtSeconds(profileElapsedMs(profile))} · ${formatCount(profile.samplesTaken)} samples` })
        : null,
      start,
      stop,
      refresh,
    ),
  );
}

function renderProfileStats(profile) {
  const elapsed = profileElapsedMs(profile);
  const rate = profile.samplesPerSecond ?? (elapsed > 0 ? (profile.samplesTaken / (elapsed / 1000)) : null);
  const interval = profile.intervalMs ?? (profile.hz > 0 ? 1000 / profile.hz : null);
  return el("div", { class: "stat-row" },
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Status" }),
      el("div", { class: "stat-value" }, el("span", { class: `perf-chip is-${profileStatusTone(profile)}`, text: profileStatusLabel(profile) }))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Elapsed" }),
      el("div", { class: "stat-value" }, fmtSeconds(elapsed))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Samples" }),
      el("div", { class: "stat-value" }, formatCount(profile.samplesTaken), el("small", { text: " taken" }))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Retained" }),
      el("div", { class: "stat-value" }, formatCount(profile.retainedSamples || profile.samples.length || profile.stackTotalSamples), el("small", { text: " samples" }))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Dropped" }),
      el("div", { class: "stat-value" }, formatCount(profile.samplesDropped))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Observed rate" }),
      el("div", { class: "stat-value" }, rate === null ? "—" : fmtNumber(rate, 0), el("small", { text: " samples/s" }))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Interval" }),
      el("div", { class: "stat-value" }, interval === null ? `${fmtNumber(profile.hz, 0)}` : fmtNumber(interval, 2), el("small", { text: interval === null ? " Hz" : " ms" }))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Thread" }),
      el("div", { class: "stat-value", attrs: { title: profile.thread || "" } }, profile.thread || "—")),
  );
}

