import { el, fmtBytes, icon } from "../../core/format.js";
import { $, ALLOCATION_POLL_MS, ALLOCATION_RATE_LIMIT, ALLOCATION_SITE_COMMANDS, MAX_ALLOCATION_SITES, MAX_ALLOCATION_SITE_FRAMES, RpcError, state } from "../../core/globals.js";
import { errorMessage, rpc, toast, unwrapData } from "../../core/session.js";
import { allocationSiteErrorKind, allocationSitePayloadRate, allocationSiteRateValue, formatWallTime, normalizeAllocationSitePayload } from "./snapshot.js";
import { renderAllocationSitesView } from "./view.js";
import { formatCount, isObjectValue } from "../performance/profile/model.js";

async function allocationSiteRpc(operation, params) {
  const names = ALLOCATION_SITE_COMMANDS[operation];
  let last = null;
  for (const name of names) {
    try {
      const result = unwrapData(await rpc(name, params));
      return { command: name, result };
    } catch (error) {
      last = error;
      if (error?.code !== "unknown_command") throw error;
    }
  }
  throw last || new RpcError("The controller does not expose allocation-site sampling.", "unknown_command");
}

export function releaseAllocationSampling(targetId) {
  if (!targetId || !state.token) return Promise.resolve();
  return (async () => {
    for (const name of ALLOCATION_SITE_COMMANDS.stop) {
      try {
        await rpc(name, { target: targetId });
        return;
      } catch (error) {
        if (error?.code !== "unknown_command") return;
      }
    }
  })();
}

export function stopAllocationPolling() {
  const timer = state.memory?.allocation?.timer;
  if (timer) clearInterval(timer);
  if (state.memory?.allocation) state.memory.allocation.timer = null;
}

export function syncAllocationPolling() {
  const allocation = state.memory?.allocation;
  if (!allocation) return;
  const shouldPoll = allocation.status === "running"
    && !allocation.loading
    && state.connected
    && Boolean(state.token)
    && Boolean(state.targetId)
    && state.view === "memory"
    && !document.hidden;
  if (!shouldPoll) {
    stopAllocationPolling();
    return;
  }
  if (allocation.timer) return;
  const targetId = state.targetId;
  allocation.timer = setInterval(() => {
    if (state.targetId !== targetId || state.memory.allocation !== allocation) {
      stopAllocationPolling();
      return;
    }
    queryAllocationSites({ quiet: true, poll: true }).catch(() => {});
  }, ALLOCATION_POLL_MS);
}

function applyAllocationSiteFailure(allocation, error, options = {}) {
  const kind = allocationSiteErrorKind(error);
  allocation.loading = false;
  allocation.polling = false;
  allocation.action = null;
  allocation.error = kind === "error" ? error : null;
  allocation.loaded = true;
  allocation.status = kind;
  allocation.reason = errorMessage(error);
  if (kind === "unavailable") {
    allocation.reason = error?.code === "unknown_command"
      ? (options.operation === "query"
        ? "This controller does not expose allocation-site sampling. No sites are shown."
        : "This controller does not expose allocation-site sampling.")
      : errorMessage(error);
  }
  if (!options.quiet) toast(allocation.reason, "error", 9000);
  stopAllocationPolling();
}

export async function startAllocationSites() {
  const allocation = state.memory.allocation;
  const targetId = state.targetId;
  if (!state.token || !targetId || !state.caps.runtime) return;
  if (!state.connected) {
    toast("The controller is offline. Allocation sampling cannot start.", "error", 8000);
    return;
  }
  const rate = allocationSiteRateValue(allocation.rate);
  if (rate === null) {
    allocation.error = new RpcError(`The sampling rate must be an integer from 1 to ${formatCount(ALLOCATION_RATE_LIMIT)}.`, "invalid_rate");
    allocation.status = allocation.report ? "stopped" : "idle";
    renderAllocationSitesView();
    toast(errorMessage(allocation.error), "error", 8000);
    return;
  }
  if ((allocation.loading && !allocation.polling) || ["running", "starting", "stopping"].includes(allocation.status)) return;
  const request = allocation.request + 1;
  allocation.request = request;
  allocation.loading = true;
  allocation.polling = false;
  allocation.action = "start";
  allocation.error = null;
  allocation.reason = "";
  allocation.status = "starting";
  allocation.rate = rate;
  allocation.report = null;
  allocation.expanded.clear();
  allocation.queriedAt = 0;
  allocation.startedAt = Date.now();
  allocation.stoppedAt = 0;
  stopAllocationPolling();
  renderAllocationSitesView();
  try {
    const { result } = await allocationSiteRpc("start", { target: targetId, rate });
    if (request !== allocation.request || targetId !== state.targetId || state.memory.allocation !== allocation) return;    const payload = normalizeAllocationSitePayload(result);
    const failure = payload.status === "unsupported"
      ? "unsupported"
      : payload.status === "unavailable"
        ? "unavailable"
        : payload.status === "error"
          ? "error"
          : "";
    if (failure) {
      allocation.status = failure;
      allocation.supported = payload.supported;
      allocation.reason = payload.reason || (failure === "unsupported"
        ? "The runtime reports that allocation-site sampling is not available on this target."
        : "The runtime did not start allocation-site sampling.");
      allocation.error = failure === "error" ? new RpcError(allocation.reason, "runtime_error") : null;
      stopAllocationPolling();
      if (failure === "error") toast(allocation.reason, "error", 9000);
      return;
    }
    allocation.status = "running";
    allocation.supported = payload.supported ?? true;
    allocation.sessionSeq += 1;
    allocation.rate = allocationSitePayloadRate(payload, rate);
    allocation.report = payload.hasReport ? { ...payload, rate: allocation.rate, refreshedAt: Date.now() } : null;
    allocation.loaded = true;
    allocation.queriedAt = Date.now();
    if (!allocation.startedAt) allocation.startedAt = Date.now();
    await queryAllocationSites({ quiet: true, force: true });
    if (allocation.status === "running") {
      toast(`Sampling every ${formatCount(allocation.rate)} allocation${allocation.rate === 1 ? "" : "s"}. Stop to collect sites.`, "success", 5000);
    }
  } catch (error) {
    if (request !== allocation.request || targetId !== state.targetId || state.memory.allocation !== allocation) return;
    const kind = allocationSiteErrorKind(error);
    if (kind === "error" && /already (sampling|running)/i.test(String(error?.message || ""))) {
      allocation.status = "running";
      allocation.startedAt = Date.now();
      await queryAllocationSites({ quiet: true });
    } else {
      applyAllocationSiteFailure(allocation, error, { operation: "start" });
    }
  } finally {
    if (request === allocation.request && targetId === state.targetId && state.memory.allocation === allocation) {
      allocation.loading = false;
      allocation.action = null;
      renderAllocationSitesView();
      syncAllocationPolling();
    }
  }
}

export async function queryAllocationSites(options = {}) {
  const allocation = state.memory.allocation;
  const targetId = state.targetId;
  if (!state.token || !targetId || !state.caps.runtime) return;
  if (allocation.loading && !options.force) return;
  const request = allocation.request + 1;
  allocation.request = request;
  allocation.loading = true;
  allocation.polling = options.poll === true;
  allocation.action = "query";
  if (!options.quiet) allocation.error = null;
  if (!options.poll) renderAllocationSitesView();
  try {
    const { result } = await allocationSiteRpc("query", { target: targetId, limit: MAX_ALLOCATION_SITES });
    if (request !== allocation.request || targetId !== state.targetId || state.memory.allocation !== allocation) return;
    allocation.loaded = true;
    allocation.queriedAt = Date.now();
    if (result === null || result === undefined) {
      if (allocation.status === "running") {
        allocation.status = "stopped";
        allocation.stoppedAt = Date.now();
        allocation.reason = "The runtime no longer reports an active sampling session. The last retained report is shown.";
      } else if (!allocation.report) {
        allocation.status = "idle";
        allocation.reason = "";
      }
      allocation.error = null;
      if (!options.quiet && !options.poll) toast("No allocation sampling session is active.", "info", 4000);
      return;
    }
    const payload = normalizeAllocationSitePayload(result);
    const failure = payload.status === "unsupported"
      ? "unsupported"
      : payload.status === "unavailable"
        ? "unavailable"
        : payload.status === "error"
          ? "error"
          : "";
    if (failure) {
      allocation.status = failure;
      allocation.supported = payload.supported;
      allocation.reason = payload.reason || (failure === "unsupported"
        ? "The runtime reports that allocation-site sampling is not available on this target."
        : "The runtime did not return allocation sites.");
      allocation.error = failure === "error" ? new RpcError(allocation.reason, "runtime_error") : null;
      stopAllocationPolling();
      return;
    }
    allocation.supported = payload.supported ?? allocation.supported;
    allocation.reason = payload.reason;
    allocation.error = null;
    const rate = allocationSitePayloadRate(payload, allocation.rate);
    allocation.rate = rate;
    if (payload.hasReport) allocation.report = { ...payload, rate, refreshedAt: Date.now() };
    if (payload.status === "running") {
      allocation.status = "running";
      if (!allocation.startedAt) allocation.startedAt = Date.now();
    } else if (payload.status === "stopped") {
      allocation.status = "stopped";
      allocation.stoppedAt = allocation.stoppedAt || Date.now();
    } else if (payload.status === "idle") {
      if (allocation.status === "running") {
        allocation.status = "stopped";
        allocation.stoppedAt = Date.now();
        allocation.reason = payload.reason || "The runtime no longer reports an active sampling session.";
      } else if (!allocation.report) {
        allocation.status = "idle";
      }
    } else if (payload.hasReport) {
      allocation.status = allocation.status === "stopped" ? "stopped" : "running";
      if (allocation.status === "running" && !allocation.startedAt) allocation.startedAt = Date.now();
    } else {
      allocation.status = "idle";
    }
    if (!options.quiet && !options.poll) {
      toast(payload.sites.length
        ? `${formatCount(payload.sites.length)} allocation site${payload.sites.length === 1 ? "" : "s"} returned.`
        : "Allocation sites queried.", "success", 4000);
    }
  } catch (error) {
    if (request !== allocation.request || targetId !== state.targetId || state.memory.allocation !== allocation) return;
    if (!state.connected) {
      allocation.status = "offline";
      allocation.reason = "The controller connection dropped. Sampling state is unknown until it reconnects.";
      allocation.loading = false;
      allocation.action = null;
      stopAllocationPolling();
      return;
    }
    applyAllocationSiteFailure(allocation, error, { operation: "query", quiet: options.quiet === true || options.poll === true });
  } finally {
    if (request === allocation.request && targetId === state.targetId && state.memory.allocation === allocation) {
      allocation.loading = false;
      allocation.polling = false;
      allocation.action = null;
      renderAllocationSitesView({ preserveFocus: options.poll === true });
      syncAllocationPolling();
    }
  }
}

export async function stopAllocationSites(options = {}) {
  const allocation = state.memory.allocation;
  const targetId = state.targetId;
  if (!state.token || !targetId) return;
  if (!state.connected) {
    toast("The controller is offline. The sampler cannot be stopped from this view.", "error", 8000);
    return;
  }
  if (!["running", "starting"].includes(allocation.status)) {
    if (!options.quiet) toast("No allocation sampling session is running.", "info", 4000);
    return;
  }
  if (allocation.loading && !allocation.polling) return;
  const request = allocation.request + 1;
  allocation.request = request;
  allocation.loading = true;
  allocation.polling = false;
  allocation.action = "stop";
  allocation.status = "stopping";
  allocation.error = null;
  stopAllocationPolling();
  renderAllocationSitesView();
  try {
    const { result } = await allocationSiteRpc("stop", { target: targetId });
    if (request !== allocation.request || targetId !== state.targetId || state.memory.allocation !== allocation) return;
    const payload = normalizeAllocationSitePayload(result);
    const failure = payload.status === "unsupported"
      ? "unsupported"
      : payload.status === "unavailable"
        ? "unavailable"
        : payload.status === "error"
          ? "error"
          : "";
    if (failure) {
      allocation.status = failure;
      allocation.reason = payload.reason || "The runtime did not return the sampled sites.";
      allocation.error = failure === "error" ? new RpcError(allocation.reason, "runtime_error") : null;
      return;
    }
    allocation.status = "stopped";
    allocation.stoppedAt = Date.now();
    allocation.loaded = true;
    allocation.queriedAt = Date.now();
    allocation.rate = allocationSitePayloadRate(payload, allocation.rate);
    allocation.supported = payload.supported ?? true;
    allocation.reason = payload.reason;
    allocation.report = payload.hasReport ? { ...payload, rate: allocation.rate, refreshedAt: Date.now() } : null;
    if (!options.quiet) {
      const sites = allocation.report?.sites.length ?? 0;
      const samples = allocation.report?.samplesRetained ?? null;
      toast(
        sites
          ? `Sampling stopped: ${formatCount(sites)} site${sites === 1 ? "" : "s"} from ${samples === null ? "the retained sample ring" : `${formatCount(samples)} retained sample${samples === 1 ? "" : "s"}`}.`
          : "Sampling stopped with no retained sites.",
        sites ? "success" : "info",
        6000,
      );
    }
  } catch (error) {
    if (request !== allocation.request || targetId !== state.targetId || state.memory.allocation !== allocation) return;
    if (error?.code === "no_allocation_profile") {
      allocation.status = allocation.report ? "stopped" : "idle";
      allocation.reason = errorMessage(error);
      allocation.loading = false;
      allocation.action = null;
      stopAllocationPolling();
      if (!options.quiet) toast("No allocation sampling session is running.", "info", 5000);
      return;
    }
    if (!state.connected) {
      allocation.status = "offline";
      allocation.reason = "The controller connection dropped while stopping the sampler.";
      allocation.loading = false;
      allocation.action = null;
      stopAllocationPolling();
      return;
    }
    applyAllocationSiteFailure(allocation, error, { operation: "stop" });
  } finally {
    if (request === allocation.request && targetId === state.targetId && state.memory.allocation === allocation) {
      allocation.loading = false;
      allocation.action = null;
      renderAllocationSitesView();
      syncAllocationPolling();
    }
  }
}

export async function resetAllocationSites() {
  const allocation = state.memory.allocation;
  const targetId = state.targetId;
  if (!state.token || !targetId || !state.connected) return;
  if (allocation.loading && !allocation.polling) return;
  if (!allocation.report && allocation.status !== "running") return;
  const request = allocation.request + 1;
  allocation.request = request;
  allocation.loading = true;
  allocation.polling = false;
  allocation.action = "reset";
  allocation.error = null;
  renderAllocationSitesView();
  try {
    const { result } = await allocationSiteRpc("reset", { target: targetId });
    if (request !== allocation.request || targetId !== state.targetId || state.memory.allocation !== allocation) return;
    const cleared = result === true
      || (isObjectValue(result) && (result.reset === true || result.cleared === true || result.ok === true));
    const runtimeCleared = cleared || allocation.status === "running";
    allocation.report = null;
    allocation.expanded.clear();
    allocation.queriedAt = 0;
    if (allocation.status !== "running") {
      allocation.status = "idle";
      allocation.stoppedAt = 0;
      allocation.reason = "";
    }
    if (!runtimeCleared) {
      toast("The runtime has no active sampling session. The stopped report was cleared from this view.", "info", 5000);
    } else {
      toast("Sampled allocation sites cleared. Sampling state is unchanged.", "success", 4000);
      await queryAllocationSites({ quiet: true, force: true });
    }
  } catch (error) {
    if (request !== allocation.request || targetId !== state.targetId || state.memory.allocation !== allocation) return;
    if (error?.code === "unknown_command") {
      allocation.loading = false;
      allocation.action = null;
      if (allocation.status !== "running") {
        allocation.report = null;
        allocation.expanded.clear();
        allocation.status = "idle";
        allocation.stoppedAt = 0;
        allocation.reason = "";
        toast("The controller has no clear command. The stopped report was cleared from this view only.", "info", 6000);
      } else {
        toast("The controller cannot clear the retained ring. Stop and start again to reset sampling.", "info", 6000);
      }
      renderAllocationSitesView();
      return;
    }
    applyAllocationSiteFailure(allocation, error, { operation: "reset" });
  } finally {
    if (request === allocation.request && targetId === state.targetId && state.memory.allocation === allocation) {
      allocation.loading = false;
      allocation.action = null;
      renderAllocationSitesView();
      syncAllocationPolling();
    }
  }
}

export function ensureAllocationSites() {
  const allocation = state.memory.allocation;
  if (!state.token || !state.targetId || !state.caps.runtime) return;
  if (!state.caps.allocationSites) {
    if (!allocation.loaded && !allocation.loading && !allocation.report) {
      allocation.loaded = true;
      allocation.supported = false;
      allocation.status = "unavailable";
      allocation.reason = "This runtime does not advertise allocation_sites. Allocation stacks are captured only by native Apple and GNU Linux runtimes; no site is substituted.";
      renderAllocationSitesView();
    }
    return;
  }
  if (allocation.status === "running") {
    syncAllocationPolling();
    return;
  }
  if (allocation.status === "offline" && state.connected) {
    queryAllocationSites({ quiet: true });
    return;
  }
  if (!allocation.loaded && !allocation.loading && !allocation.error
    && !["unsupported", "unavailable"].includes(allocation.status)) {
    queryAllocationSites({ quiet: true });
  }
}

export function abandonAllocationSites() {
  const allocation = state.memory?.allocation;
  if (!allocation) return;
  stopAllocationPolling();
  if (allocation.status === "running" || allocation.status === "starting" || allocation.status === "stopping") {
    allocation.request += 1;
    allocation.loading = false;
    allocation.polling = false;
    allocation.action = null;
    allocation.status = "offline";
    allocation.reason = "The controller connection dropped. Sampling state is unknown; the last report is retained.";
  }
}

export function allocationSiteFailureKind(allocation) {
  if (!state.connected) return "offline";
  if (["unsupported", "unavailable", "error", "offline"].includes(allocation.status)) return allocation.status;
  return "";
}

export function allocationSiteStatusLabel(allocation) {
  if (!state.connected) return "Disconnected";
  return {
    idle: "Idle",
    starting: "Starting",
    running: "Running",
    stopping: "Stopping",
    stopped: "Stopped",
    unsupported: "Unsupported",
    unavailable: "Unavailable",
    error: "Error",
    offline: "Disconnected",
  }[allocation.status] || "Idle";
}

export function allocationSiteStatusClass(allocation) {
  if (!state.connected) return "failed";
  if (allocation.status === "running" || allocation.status === "starting") return "pending";
  if (allocation.status === "stopped") return "complete";
  if (["unsupported", "unavailable", "error", "offline"].includes(allocation.status)) return "failed";
  return "cancelled";
}

export function allocationSiteMeta(allocation) {
  const report = allocation.report;
  const rate = report?.rate ?? allocation.rate;
  const parts = [`1-in-${formatCount(rate)}`];
  if (allocation.status === "running") {
    if (report?.samplesRetained !== null && report?.samplesRetained !== undefined) {
      parts.push(`${formatCount(report.samplesRetained)} retained`);
    }
    if (allocation.queriedAt) parts.push(`updated ${formatWallTime(allocation.queriedAt)}`);
  } else if (allocation.status === "stopped") {
    const totalSites = report ? Math.max(report.totalSites ?? 0, report.sites.length) : 0;
    parts.push(report ? `${formatCount(totalSites)} sites` : "no report");
    if (allocation.stoppedAt) parts.push(`stopped ${formatWallTime(allocation.stoppedAt)}`);
  } else if (allocation.status === "unsupported") {
    parts.push("not available");
  } else if (allocation.status === "unavailable") {
    parts.push("controller unavailable");
  } else if (allocation.status === "offline") {
    parts.push("connection lost");
  } else {
    parts.push("not sampling");
  }
  return parts.join(" · ");
}

export function allocationSiteScopeNote() {
  return el("div", { class: "memory-scope" },
    icon("info"),
    el("span", {},
      el("strong", { text: "Cumulative sampled allocations, not live heap." }),
      " Start enables cove_alloc's opt-in sampler in the runtime: every Nth fresh allocation records a raw PC stack into a fixed ring. Sampled bytes and blocks are cumulative for the current session only; they are not live heap, object counts, object types, retaining paths or leaks. Frames are symbolized allocator return addresses; the runtime removes allocator-internal frames where it can identify them, and unresolved frames keep their raw address.",
    ),
  );
}

export function allocationSiteState(kind, title, message, options = {}) {
  const node = el("div", { class: `memory-site-state${kind === "error" ? " is-error" : ""}` },
    el("span", { class: "memory-site-state-icon" }, icon(options.icon || "memory")),
    el("div", { class: "memory-site-state-text" },
      el("strong", { text: title }),
      el("p", { text: message }),
    ),
  );
  if (options.action) {
    node.append(el("button", {
      class: "tool-button small",
      props: { type: "button" },
      text: options.action,
      on: { click: options.onAction },
    }));
  }
  return node;
}

export function allocationSiteRateControl(allocation, disabled) {
  const input = el("input", {
    class: "compact-input",
    attrs: {
      id: "alloc-site-rate",
      type: "number",
      min: "1",
      max: String(ALLOCATION_RATE_LIMIT),
      step: "1",
      inputmode: "numeric",
      "aria-label": "Allocation sampling rate: sample every Nth allocation",
    },
  });
  input.value = String(allocation.rate);
  input.disabled = disabled;
  input.addEventListener("input", () => {
    const value = allocationSiteRateValue(input.value === "" ? null : Number(input.value));
    input.setAttribute("aria-invalid", String(value === null));
    if (value !== null) allocation.rate = value;
  });
  input.addEventListener("change", () => {
    const value = allocationSiteRateValue(input.value === "" ? null : Number(input.value));
    if (value === null) {
      input.value = String(allocation.rate);
      input.removeAttribute("aria-invalid");
    } else {
      allocation.rate = value;
      input.value = String(value);
    }
  });
  input.addEventListener("keydown", event => {
    if (event.key === "Enter") {
      event.preventDefault();
      startAllocationSites();
    }
  });
  return el("label", { class: "profile-field memory-site-rate", attrs: { for: "alloc-site-rate" } },
    el("span", { text: "Sampling rate (1 in N)" }),
    input,
  );
}

export function allocationSiteMetric(label, value, format, title) {
  return el("div", { class: "metric", attrs: { title } },
    el("span", { class: "metric-label", text: label }),
    el("span", { class: "metric-value", text: value === null ? "—" : format(value) }),
  );
}

export function allocationSiteTopFrame(site) {
  const frame = site.frames[0] || null;
  if (!frame) {
    return { name: "No stack captured", location: "", address: "" };
  }
  return {
    name: frame.function || frame.address || "Unresolved frame",
    location: frame.file ? `${frame.file}${frame.line === null ? "" : `:${Math.trunc(frame.line)}`}` : "",
    address: frame.address,
  };
}

export function renderAllocationSiteStack(site) {
  const list = el("ol", { class: "memory-site-stack" });
  if (!site.frames.length) {
    list.append(el("li", { class: "memory-site-frame memory-site-frame-empty", text: "No stack was captured for this site." }));
  } else {
    site.frames.forEach((frame, index) => {
      const name = frame.function || frame.address || "Unresolved frame";
      const location = frame.file ? `${frame.file}${frame.line === null ? "" : `:${Math.trunc(frame.line)}`}` : "";
      list.append(el("li", { class: "memory-site-frame" },
        el("span", { class: "memory-site-frame-index", text: String(index) }),
        el("span", { class: "memory-site-frame-body" },
          el("code", { class: "memory-site-frame-name", text: name }),
          location ? el("span", { class: "memory-site-frame-location", text: location }) : null,
          frame.module ? el("span", { class: "memory-site-frame-module", text: frame.module }) : null,
        ),
        frame.address && frame.function ? el("code", { class: "memory-site-frame-address", text: frame.address }) : null,
      ));
    });
  }
  const omitted = site.framesOmitted > 0
    ? `${formatCount(site.framesOmitted)} frame${site.framesOmitted === 1 ? "" : "s"} omitted from this view.`
    : "";
  const captureLimit = site.framesTruncated
    ? "Capture reached the frame limit; deeper callers were not recorded."
    : "";
  const headNote = [captureLimit, omitted].filter(Boolean).join(" ");
  return el("div", { class: "memory-site-stack-body" },
    el("div", { class: "memory-site-stack-head" },
      el("span", { text: `Innermost caller first · ${formatCount(site.frames.length)} frame${site.frames.length === 1 ? "" : "s"} shown` }),
      headNote ? el("span", { class: "memory-site-stack-omitted", text: headNote }) : null,
    ),
    list,
    el("p", { class: "memory-note", text: "Return addresses are symbolized by the runtime. Allocator-internal frames are omitted when the runtime can identify them; unresolved frames keep their raw address and no symbol is invented." }),
  );
}

export function renderAllocationSiteTable(report, allocation) {
  const ranked = [...report.sites]
    .sort((left, right) => (right.bytes ?? -1) - (left.bytes ?? -1)
      || (right.blocks ?? -1) - (left.blocks ?? -1)
      || left.key.localeCompare(right.key))
    .slice(0, MAX_ALLOCATION_SITES);
  const denominator = report.sampledBytes && report.sampledBytes > 0
    ? report.sampledBytes
    : ranked.reduce((sum, site) => sum + (site.bytes ?? 0), 0);
  const body = el("tbody");
  ranked.forEach((site, index) => {
    const rank = index + 1;
    const detailsId = `alloc-site-stack-${rank}`;
    const expanded = allocation.expanded.has(site.key);
    const top = allocationSiteTopFrame(site);
    const share = denominator > 0 && site.bytes !== null ? (site.bytes / denominator) * 100 : null;
    const toggle = el("button", {
      class: "link-button memory-site-toggle",
      props: { type: "button" },
      attrs: {
        "aria-expanded": String(expanded),
        "aria-controls": detailsId,
        "aria-label": `${expanded ? "Hide" : "Show"} stack for site ${rank}`,
      },
    },
      el("span", { class: "memory-site-top-name", text: top.name }),
      top.location ? el("span", { class: "memory-site-top-location", text: top.location }) : null,
      top.address && top.name !== top.address ? el("code", { class: "memory-site-top-address", text: top.address }) : null,
    );
    const details = el("tr", { class: "memory-site-stack-row", attrs: { id: detailsId } },
      el("td", { attrs: { colspan: "5" } }, renderAllocationSiteStack(site)),
    );
    details.hidden = !expanded;
    toggle.addEventListener("click", () => {
      const show = details.hidden;
      details.hidden = !show;
      toggle.setAttribute("aria-expanded", String(show));
      toggle.setAttribute("aria-label", `${show ? "Hide" : "Show"} stack for site ${rank}`);
      if (show) allocation.expanded.add(site.key);
      else allocation.expanded.delete(site.key);
    });
    body.append(el("tr", { class: "memory-site-row" },
      el("td", { class: "num memory-site-rank", text: String(rank) }),
      el("td", { class: "num memory-bytes", text: site.bytes === null ? "—" : fmtBytes(site.bytes) }),
      el("td", { class: "num memory-bytes", text: site.blocks === null ? "—" : formatCount(site.blocks) }),
      el("td", { class: "num memory-site-share", text: share === null ? "—" : `${share.toFixed(1)}%` }),
      el("td", { class: "memory-site-stack-cell" }, toggle),
    ));
    body.append(details);
  });
  const table = el("table", { class: "data-table memory-site-table" },
    el("thead", {},
      el("tr", {},
        el("th", { attrs: { scope: "col" }, text: "#" }),
        el("th", { class: "num", attrs: { scope: "col" }, text: "Sampled bytes" }),
        el("th", { class: "num", attrs: { scope: "col" }, text: "Blocks" }),
        el("th", { class: "num", attrs: { scope: "col" }, text: "Share" }),
        el("th", { attrs: { scope: "col" }, text: "Sampling stack" }),
      ),
    ),
    body,
  );
  return el("div", { class: "table-wrap memory-site-table-wrap" }, table);
}

export function allocationSiteTruncationNotices(report) {
  const notices = [];
  if (report.symbolizationReason) {
    const reason = report.symbolizationReason.trim().replace(/[.!]$/, "");
    notices.push(`Addresses could not be symbolized: ${reason}. Only sampled return addresses are shown.`);
  }
  const shown = Math.min(report.sites.length, MAX_ALLOCATION_SITES);
  const total = Math.max(report.totalSites ?? 0, report.sites.length + (report.sitesOmitted ?? 0));
  if (total > shown) {
    notices.push(`Showing ${formatCount(shown)} of ${formatCount(total)} sampled sites, ranked by sampled bytes.`);
  }
  if (report.sitesTruncated && total <= shown) {
    notices.push("The runtime marked the site list as truncated.");
  }
  const omittedFrames = report.framesOmitted + report.sites.reduce((sum, site) => sum + site.framesOmitted, 0);
  if (report.framesTruncated || omittedFrames > 0) {
    notices.push(`Stacks are bounded at ${formatCount(MAX_ALLOCATION_SITE_FRAMES)} frames per site; deeper callers were not recorded or are omitted by this view.`);
  }
  return notices;
}

