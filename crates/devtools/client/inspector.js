const $ = id => document.getElementById(id);

const TOKEN_KEY = "cove.devtools.token";
const THEME_KEY = "cove.devtools.theme";
const DETAIL_TABS = ["properties", "render", "layout", "source"];
// The flex editor sends the LayoutNode variant names the layout record
// publishes (`Justify` and `Align`), so the client never invents a value the
// runtime cannot parse.
const FLEX_JUSTIFY_VALUES = ["FlexStart", "Center", "FlexEnd", "SpaceBetween", "SpaceAround", "SpaceEvenly"];
const FLEX_ALIGN_VALUES = ["Auto", "FlexStart", "Center", "FlexEnd", "Stretch", "Baseline", "SpaceBetween", "SpaceAround", "SpaceEvenly"];
const FLEX_PROPERTY_NAMES = ["flexGrow", "flexShrink", "justifyContent", "alignItems"];
const FLEX_FACTOR_LIMIT = 1000;
// A temporary override can reach the layout record one or two frames later.
// The editor reads the record back instead of echoing what it sent.
const FLEX_RECORD_ATTEMPTS = 5;
const FLEX_RECORD_DELAY_MS = 100;
const LEVELS = ["trace", "debug", "info", "notice", "warning", "error", "critical"];
const PERFORMANCE_TABS = ["frames", "timeline", "profile"];
const BUDGET_RATES = [60, 90, 120];
const BUDGET_KEY = "cove.devtools.budgetHz";
const PROFILE_CAPABILITIES = ["cpu_profile", "profiling", "cpu_sampling", "profile"];
const PROFILE_HZ_RATES = [100, 250, 500, 1000];
const PROFILE_DURATIONS = [1000, 3000, 5000, 10000];
const PROFILE_DEFAULT_HZ = 1000;
const PROFILE_DEFAULT_DURATION = 5000;
const PROFILE_POLL_MS = 500;
const MAX_PROFILE_SAMPLES = 20000;
const MAX_PROFILE_STACKS = 200;
const MAX_PROFILE_FRAMES = 64;
const MAX_PROFILE_STACK_FRAMES = 16;
const MAX_HOTSPOTS = 200;
const PROFILE_VIEWS = ["call-tree", "bottom-up", "flame-chart", "methods", "stacks"];
const PROFILE_VIEW_LABELS = {
  "call-tree": "Call tree",
  "bottom-up": "Bottom-up",
  "flame-chart": "Flame chart",
  methods: "Methods",
  stacks: "Stacks",
};
const MAX_PROFILE_TREE_NODES = 3000;
const MAX_PROFILE_TREE_DEPTH = 48;
const MAX_PROFILE_TREE_CHILDREN = 64;
const MAX_PROFILE_TREE_ROWS = 1200;
const MAX_PROFILE_BOTTOM_GROUPS = 200;
const MAX_PROFILE_BOTTOM_CALLERS = 32;
const MAX_PROFILE_FLAME_BARS = 320;
const MAX_PROFILE_FLAME_DEPTH = 40;
const PROFILE_FLAME_MIN_PERCENT = 0.15;
const PROFILE_METHOD_SORT_KEYS = ["label", "location", "selfMs", "inclusiveMs", "selfPercent", "inclusivePercent", "selfSamples", "inclusiveSamples"];
const MAX_MEMORY_COUNTERS = 256;
// The overview chart keeps a bounded ring of provider readings. Each point is
// one real query_ui_memory response; nothing is interpolated into a sample.
const MEMORY_SAMPLE_LIMIT = 720;
const MEMORY_INTERVALS = [1000, 2000, 5000];
const MEMORY_INTERVAL_KEY = "cove.devtools.memoryIntervalMs";
// Allocation sites are opt-in sampled stacks from cove_alloc, not heap objects.
// The controller owns the real ring, symbolization and bounds; this view only
// caps what it draws and says when something was cut.
const ALLOCATION_SITE_CAPABILITIES = [
  "allocation_sites",
  "allocation_sampling",
  "alloc_sites",
  "alloc_sampling",
];
const ALLOCATION_DEFAULT_RATE = 1024;
const ALLOCATION_RATE_LIMIT = 1_000_000;
const ALLOCATION_POLL_MS = 1000;
const MAX_ALLOCATION_SITES = 64;
const MAX_ALLOCATION_SITE_FRAMES = 32;
// Baseline snapshots are this view's own JSON files. Import is strict: the
// schema, every field's type and the site stack identities are checked before
// a snapshot can replace the baseline.
const ALLOCATION_SNAPSHOT_SCHEMA = "cove.devtools.allocation-sites";
const ALLOCATION_SNAPSHOT_VERSION = 1;
const MAX_ALLOCATION_SNAPSHOT_BYTES = 2 * 1024 * 1024;
const ALLOCATION_SESSION_NONCE = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
const ALLOCATION_SNAPSHOT_STATUSES = ["running", "stopped"];
const ALLOCATION_SNAPSHOT_KEYS = [
  "schema",
  "version",
  "capturedAt",
  "reportUpdatedAt",
  "target",
  "session",
  "rate",
  "report",
];
const ALLOCATION_SNAPSHOT_REPORT_KEYS = [
  "status",
  "sampledBytes",
  "sampledBlocks",
  "samplesAttempted",
  "samplesRetained",
  "samplesDropped",
  "captureFailures",
  "totalSites",
  "sitesOmitted",
  "framesOmitted",
  "sitesTruncated",
  "framesTruncated",
  "symbolizationReason",
  "sites",
];
const ALLOCATION_SNAPSHOT_SITE_KEYS = ["frames", "bytes", "blocks", "framesOmitted", "framesTruncated"];
const ALLOCATION_SNAPSHOT_FRAME_KEYS = ["address", "function", "file", "line", "module"];
// The controller may call these tools under a different allocation-sampling
// name; only an unknown-command answer moves to the next alias.
const ALLOCATION_SITE_COMMANDS = {
  start: [
    "start_ui_allocation_profile",
    "start_ui_allocation_sites",
    "start_ui_allocation_sampling",
    "start_ui_alloc_sites",
    "start_ui_alloc_sampling",
  ],
  stop: [
    "stop_ui_allocation_profile",
    "stop_ui_allocation_sites",
    "stop_ui_allocation_sampling",
    "stop_ui_alloc_sites",
    "stop_ui_alloc_sampling",
  ],
  query: [
    "query_ui_allocation_sites",
    "query_ui_allocation_sampling",
    "query_ui_alloc_sites",
    "get_ui_allocation_sites",
  ],
  reset: [
    "reset_ui_allocation_sites",
    "reset_ui_allocation_sampling",
    "reset_ui_alloc_sites",
  ],
};
const MAX_APP_SIZE_SECTIONS = 200;
const MAX_APP_SIZE_SYMBOLS = 200;
const MAX_APP_SIZE_GROUPS = 100;
const MAX_APP_SIZE_SEGMENTS = 100;
const MAX_APP_SIZE_CONTAINMENT_GROUPS = 25;
const APP_SIZE_LOCAL_PLATFORMS = ["macos", "windows", "linux"];
// The treemap draws only entries with reported bytes and a deterministic
// squarified layout over a fixed 1000x300 grid; percentages keep it responsive.
const APP_SIZE_TREEMAP_WIDTH = 1000;
const APP_SIZE_TREEMAP_HEIGHT = 300;
const APP_SIZE_TREEMAP_LEGEND = 16;
const APP_SIZE_TREEMAP_TONES = 12;
const APP_SIZE_SNAPSHOT_SCHEMA = "cove.devtools.app-size";
const APP_SIZE_SNAPSHOT_VERSION = 1;
const MAX_APP_SIZE_DIFF_ROWS = 400;
// Deep Links validates project files, not a runtime. The root is the only
// input, kept verbatim so the controller can enforce its own canonical path.
const DEEP_LINKS_ROOT_KEY = "cove.devtools.deepLinksRoot";
const MAX_DEEP_LINKS_FILES = 200;
const MAX_DEEP_LINKS_ENTRIES = 200;
const MAX_DEEP_LINKS_ISSUES = 400;
const DEBUGGER_POLL_MS = 1500;
const DEBUGGER_CONSOLE_LIMIT = 200;
const MAX_DEBUGGER_SOURCE_LINES = 20000;
const DEBUGGER_RPC_TIMEOUT_MS = 10000;
// Slow animations are a real runtime time scale: the host multiplies animation
// deltas by the scale without touching frame timings, input or diagnostics.
const SLOW_ANIMATION_SCALE = 0.2;
const NORMAL_ANIMATION_SCALE = 1;
// Per-frame span attribution is optional: hosts advertise it before the client
// calls `analyze_ui_frame`. Without it, only TimingFrame records are available.
const FRAME_ANALYSIS_CAPABILITIES = ["frame_analysis", "analyze_frame"];
const SNAPSHOT_SCHEMA = "cove.devtools.performance";
const SNAPSHOT_VERSION = 1;
const FRAME_WINDOW = 120;
const FRAME_TABLE_LIMIT = 100;
const MAX_FRAME_RECORDS = 240;
const MAX_FLAME_BARS = 1200;
const SPAN_TABLE_LIMIT = 1000;
const MAX_SNAPSHOT_EVENTS = 5000;

const dom = {
  targets: $("targets"),
  refresh: $("refresh"),
  connection: $("connection"),
  connectionText: $("connection-text"),
  themeButton: $("theme-button"),
  themeIcon: $("theme-icon"),
  shortcutsButton: $("shortcuts-button"),
  shortcutsDialog: $("shortcuts-dialog"),
  shortcutsClose: $("shortcuts-close"),
  railMeta: $("rail-meta"),
  setup: $("setup-state"),
  setupError: $("setup-error"),
  tokenForm: $("token-form"),
  tokenInput: $("token-input"),
  inspectorContent: $("inspector-content"),
  modeSelect: $("mode-select"),
  modePick: $("mode-pick"),
  modeInteract: $("mode-interact"),
  repaintHighlightButton: $("repaint-highlight-button"),
  guidelinesButton: $("guidelines-button"),
  baselinesButton: $("baselines-button"),
  slowAnimationsButton: $("slow-animations-button"),
  oversizedImagesButton: $("oversized-images-button"),
  treeSearch: $("tree-search"),
  filterAnchors: $("filter-anchors"),
  filterPrimitives: $("filter-primitives"),
  captureButton: $("capture-button"),
  nodeShotButton: $("node-shot-button"),
  saveButton: $("save-button"),
  pickHint: $("pick-hint"),
  pickHintText: $("pick-hint-text"),
  pickExit: $("pick-exit"),
  capabilityNote: $("capability-note"),
  treeState: $("tree-state"),
  tree: $("tree"),
  treeMeta: $("tree-meta"),
  treeFooter: $("tree-footer"),
  treeRefresh: $("tree-refresh"),
  treeCollapse: $("tree-collapse"),
  detailsState: $("details-state"),
  detailsContent: $("details-content"),
  detailsHeading: $("details-heading"),
  nodeKind: $("node-kind"),
  nodeId: $("node-id"),
  copyNodeId: $("copy-node-id"),
  nodeFlags: $("node-flags"),
  highlightButton: $("highlight-button"),
  clearHighlight: $("clear-highlight"),
  readValueButton: $("read-value-button"),
  screenshotMeta: $("screenshot-meta"),
  screenshotBody: $("screenshot-body"),
  screenshotZoom: $("screenshot-zoom"),
  screenshotClear: $("screenshot-clear"),
  panels: {
    properties: $("panel-properties"),
    render: $("panel-render"),
    layout: $("panel-layout"),
    source: $("panel-source"),
  },
  inputMeta: $("input-meta"),
  inputToggle: $("input-toggle"),
  inputBody: $("input-body"),
  keyForm: $("key-form"),
  keyValue: $("key-value"),
  textForm: $("text-form"),
  textValue: $("text-value"),
  sendKey: $("send-key"),
  sendText: $("send-text"),
  networkMeta: $("network-meta"),
  networkSearch: $("network-search"),
  networkStateFilter: $("network-state-filter"),
  networkFollow: $("network-follow"),
  networkWaterfall: $("network-waterfall"),
  networkRecordingToggle: $("network-recording-toggle"),
  networkRecordingStatus: $("network-recording-status"),
  networkClear: $("network-clear"),
  networkRefresh: $("network-refresh"),
  networkContent: $("network-content"),
  networkDialog: $("network-dialog"),
  networkDialogTitle: $("network-dialog-title"),
  networkDialogBody: $("network-dialog-body"),
  networkDialogClose: $("network-dialog-close"),
  logsMeta: $("logs-meta"),
  logsSearch: $("logs-search"),
  logsLevel: $("logs-level"),
  logsLabel: $("logs-label"),
  logsFollow: $("logs-follow"),
  logsClear: $("logs-clear"),
  logsRefresh: $("logs-refresh"),
  logsContent: $("logs-content"),
  stateMeta: $("state-meta"),
  stateRefresh: $("state-refresh"),
  stateContent: $("state-content"),
  stateScope: $("state-scope"),
  stateFollow: $("state-follow"),
  performanceMeta: $("performance-meta"),
  performanceBudget: $("performance-budget"),
  performanceFollow: $("performance-follow"),
  performanceElapsed: $("performance-elapsed"),
  performanceStart: $("performance-start"),
  performanceStop: $("performance-stop"),
  performanceRefresh: $("performance-refresh"),
  performanceExport: $("performance-export"),
  performanceImport: $("performance-import"),
  performanceFile: $("performance-file"),
  performanceSnapshotBadge: $("performance-snapshot-badge"),
  performanceFrames: $("performance-frames"),
  performanceTimeline: $("performance-timeline"),
  performanceProfile: $("performance-profile"),
  performanceContent: $("performance-content"),
  performanceRecordingDot: $("performance-recording-dot"),
  memoryMeta: $("memory-meta"),
  memoryRefresh: $("memory-refresh"),
  memoryFollow: $("memory-follow"),
  memoryInterval: $("memory-interval"),
  memoryContent: $("memory-content"),
  appSizeMeta: $("app-size-meta"),
  appSizeRefresh: $("app-size-refresh"),
  appSizeContent: $("app-size-content"),
  deepLinksMeta: $("deep-links-meta"),
  deepLinksRootForm: $("deep-links-root-form"),
  deepLinksRoot: $("deep-links-root"),
  deepLinksRootError: $("deep-links-root-error"),
  deepLinksValidate: $("deep-links-validate"),
  deepLinksContent: $("deep-links-content"),
  debuggerMeta: $("debugger-meta"),
  debuggerStatus: $("debugger-status"),
  debuggerRefresh: $("debugger-refresh"),
  debuggerAttach: $("debugger-attach"),
  debuggerDetach: $("debugger-detach"),
  debuggerUnavailable: $("debugger-unavailable"),
  debuggerUnavailableTitle: $("debugger-unavailable-title"),
  debuggerUnavailableReason: $("debugger-unavailable-reason"),
  debuggerAdapter: $("debugger-adapter"),
  debuggerSession: $("debugger-session"),
  debuggerBusy: $("debugger-busy"),
  debuggerBusyText: $("debugger-busy-text"),
  debuggerPause: $("debugger-pause"),
  debuggerContinue: $("debugger-continue"),
  debuggerStepOver: $("debugger-step-over"),
  debuggerStepInto: $("debugger-step-into"),
  debuggerStepOut: $("debugger-step-out"),
  debuggerThreads: $("debugger-threads"),
  debuggerThreadsMeta: $("debugger-threads-meta"),
  debuggerStack: $("debugger-stack"),
  debuggerStackMeta: $("debugger-stack-meta"),
  debuggerBreakpoints: $("debugger-breakpoints"),
  debuggerBreakpointsMeta: $("debugger-breakpoints-meta"),
  debuggerSourcePath: $("debugger-source-path"),
  debuggerSourceMeta: $("debugger-source-meta"),
  debuggerSource: $("debugger-source"),
  debuggerOpenForm: $("debugger-open-form"),
  debuggerOpenPath: $("debugger-open-path"),
  debuggerOpen: $("debugger-open"),
  debuggerVariables: $("debugger-variables"),
  debuggerScopesMeta: $("debugger-scopes-meta"),
  debuggerVariablesRefresh: $("debugger-variables-refresh"),
  debuggerConsole: $("debugger-console"),
  debuggerConsoleMeta: $("debugger-console-meta"),
  debuggerConsoleClear: $("debugger-console-clear"),
  debuggerEvaluateForm: $("debugger-evaluate-form"),
  debuggerExpression: $("debugger-expression"),
  debuggerEvaluate: $("debugger-evaluate"),
  debuggerBadge: $("debugger-badge"),
  networkBadge: $("network-badge"),
  logsBadge: $("logs-badge"),
  toast: $("toast"),
  toastText: $("toast-text"),
  toastClose: $("toast-close"),
  toastIcon: $("toast").querySelector(".toast-icon use"),
  liveRegion: $("live-region"),
};

const state = {
  token: "",
  connected: false,
  sessionLoading: false,
  platform: "",
  targets: [],
  targetId: "",
  targetEpoch: 0,
  caps: emptyCaps(),
  view: "inspector",
  mode: "select",
  theme: "auto",
  tree: null,
  entries: new Map(),
  rootId: null,
  treeLoading: false,
  treeError: null,
  treeLoaded: false,
  treeRequest: 0,
  selectedId: null,
  detail: null,
  detailLoading: false,
  detailError: null,
  detailRequest: 0,
  valueRequested: false,
  expanded: new Set(),
  search: "",
  filters: { anchors: true, primitives: true },
  visibleModel: null,
  visibleRows: [],
  activeDetailTab: "properties",
  capture: null,
  captureLoading: false,
  captureError: null,
  captureAttempt: "window",
  pickBusy: false,
  inputBusy: false,
  network: {
    loaded: false, loading: false, error: null, records: [], limit: 0, follow: false, timer: null,
    recording: null, recordingPending: false, recordingRequest: null, clearPending: false,
    controlError: null, controlUnavailable: false, cleared: false, generation: 0, recordingEpoch: 0,
    readAt: 0, waterfall: true,
  },
  logs: {
    loaded: false, loading: false, error: null, records: [], dropped: 0, firstAvailable: null,
    follow: false, timer: null, clearing: false, clearError: null, generation: 0, cleared: false,
  },
  performance: emptyPerformance(),
  registeredState: emptyRegisteredState(),
  memory: emptyMemory(),
  appSize: emptyAppSize(),
  deepLinks: emptyDeepLinks(),
  debugger: emptyDebugger(),
  repaintHighlight: emptyRepaintHighlight(),
  debugPaint: emptyDebugPaint(),
  baselines: emptyBaselines(),
  slowAnimations: emptySlowAnimations(),
  oversizedImages: emptyOversizedImages(),
  flexEditor: emptyFlexEditor(),
  toastTimer: null,
  highlightTimer: null,
  detailTimer: null,
};

let rpcSequence = 0;

function emptyCaps() {
  return {
    runtime: false,
    tree: false,
    capture: false,
    input: false,
    logs: false,
    state: false,
    network: false,
    networkControl: false,
    timing: false,
    trace: false,
    frameAnalysis: false,
    profile: false,
    inspectNodes: false,
    repaintHighlight: false,
    debugPaint: false,
    baselines: false,
    animationSpeed: false,
    oversizedImages: false,
    allocationSites: false,
    style: false,
  };
}

function emptyRepaintHighlight() {
  return { active: false, busy: false, request: 0, queue: Promise.resolve() };
}

function emptyDebugPaint() {
  return { guidelines: false, busy: false, request: 0, queue: Promise.resolve() };
}

function emptyBaselines() {
  return { active: false, busy: false, request: 0, queue: Promise.resolve() };
}

function emptySlowAnimations() {
  return {
    active: false,
    scale: NORMAL_ANIMATION_SCALE,
    busy: false,
    request: 0,
    queue: Promise.resolve(),
  };
}

function emptyOversizedImages() {
  return { active: false, busy: false, request: 0, queue: Promise.resolve() };
}

// Flex overrides live in the runtime's scene editor. The client tracks which
// node received one so Reset stays available even when the runtime does not
// list flex property names in its style record, but never treats a requested
// value as the applied one: every value shown comes from the layout record.
function emptyFlexEditor() {
  return {
    key: "",
    draft: null,
    busy: false,
    mode: null,
    error: null,
    notice: null,
    stale: false,
    request: 0,
    applied: new Map(),
  };
}

class RpcError extends Error {
  constructor(message, code = "error", data = null) {
    super(message);
    this.name = "RpcError";
    this.code = code;
    this.data = data;
  }
}

function el(tag, options = {}, ...children) {
  const node = document.createElement(tag);
  if (options.class) node.className = options.class;
  if (options.text !== undefined && options.text !== null) node.textContent = String(options.text);
  if (options.attrs) {
    for (const [name, value] of Object.entries(options.attrs)) {
      if (value !== undefined && value !== null) node.setAttribute(name, String(value));
    }
  }
  if (options.props) {
    for (const [name, value] of Object.entries(options.props)) {
      if (value !== undefined) node[name] = value;
    }
  }
  if (options.dataset) {
    for (const [name, value] of Object.entries(options.dataset)) {
      if (value !== undefined) node.dataset[name] = String(value);
    }
  }
  if (options.on) {
    for (const [name, handler] of Object.entries(options.on)) node.addEventListener(name, handler);
  }
  appendChildren(node, children);
  return node;
}

function appendChildren(node, children) {
  for (const child of children.flat(Infinity)) {
    if (child === undefined || child === null || child === false) continue;
    node.append(child instanceof Node ? child : document.createTextNode(String(child)));
  }
}

function clearNode(node) {
  node.replaceChildren();
}

function icon(name, className = "icon") {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("class", className);
  svg.setAttribute("aria-hidden", "true");
  const use = document.createElementNS("http://www.w3.org/2000/svg", "use");
  use.setAttribute("href", `#i-${name}`);
  svg.append(use);
  return svg;
}

function clear(element) {
  element.replaceChildren();
}

function textOf(value, fallback = "—") {
  if (value === undefined || value === null || value === "") return fallback;
  return String(value);
}

function numberValue(value) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function fmtNumber(value, digits = 1) {
  const number = numberValue(value);
  return number === null ? "—" : number.toFixed(digits);
}

function fmtMs(value, digits = 2) {
  const number = numberValue(value);
  return number === null ? "—" : `${number.toFixed(digits)} ms`;
}

function fmtBytes(value) {
  const number = numberValue(value);
  if (number === null) return "—";
  if (number < 1024) return `${number} B`;
  if (number < 1024 * 1024) return `${(number / 1024).toFixed(1)} KB`;
  if (number < 1024 * 1024 * 1024) return `${(number / (1024 * 1024)).toFixed(1)} MB`;
  return `${(number / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function fmtClock(seconds) {
  const number = numberValue(seconds);
  if (number === null) return "—";
  const date = new Date(number * 1000);
  if (Number.isNaN(date.getTime())) return "—";
  try {
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit", fractionalSecondDigits: 3 });
  } catch {
    return date.toLocaleTimeString([], { hour12: false });
  }
}

function fmtDate(seconds) {
  const number = numberValue(seconds);
  if (number === null) return "—";
  const date = new Date(number * 1000);
  return Number.isNaN(date.getTime()) ? "—" : date.toLocaleString();
}

function fmtEpochMs(milliseconds) {
  const number = numberValue(milliseconds);
  if (number === null) return "—";
  const date = new Date(number);
  if (Number.isNaN(date.getTime())) return "—";
  try {
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit", fractionalSecondDigits: 3 });
  } catch {
    return date.toLocaleTimeString([], { hour12: false });
  }
}

function shortId(id) {
  const value = String(id ?? "");
  return value.length > 12 ? `${value.slice(0, 4)}…${value.slice(-5)}` : value;
}

function basename(path) {
  const value = String(path ?? "");
  return value.split(/[\\/]/).pop() || value;
}

function slug(value) {
  return String(value ?? "capture").trim().replace(/[^A-Za-z0-9._-]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 64) || "capture";
}

function setConnection(kind, label) {
  const wasOnline = state.connected;
  dom.connection.classList.remove("is-online", "is-offline", "is-busy");
  dom.connection.classList.add(`is-${kind}`);
  dom.connectionText.textContent = label;
  state.connected = kind === "online";
  if (wasOnline && !state.connected) {
    resetRepaintHighlight();
    resetDebugPaintState();
    resetBaselines();
    resetSlowAnimations();
    resetOversizedImages();
    abandonAllocationSites();
    if (state.view === "logs") renderLogs();
    if (state.view === "network") renderNetwork();
    if (state.view === "memory") renderMemory();
  } else if (!wasOnline && state.connected && state.view === "memory") {
    ensureAllocationSites();
  }
}

function setConnectionBusy(busy) {
  if (busy) {
    dom.connection.classList.add("is-busy");
    dom.connectionText.textContent = "Connecting…";
  } else if (state.connected) {
    dom.connection.classList.remove("is-busy");
    dom.connectionText.textContent = "Connected";
  }
}

function toast(message, kind = "info", duration = 6000) {
  clearTimeout(state.toastTimer);
  const text = String(message ?? "");
  if (!text) return;
  dom.toast.hidden = false;
  dom.toast.className = `toast is-${kind}`;
  dom.toastText.textContent = text;
  dom.toastIcon.setAttribute("href", kind === "error" ? "#i-alert" : kind === "success" ? "#i-check" : "#i-info");
  dom.liveRegion.textContent = text;
  state.toastTimer = setTimeout(() => { dom.toast.hidden = true; }, duration);
}

function isDisconnectedError(error) {
  return error?.code === "target_gone" || error?.code === "target_required";
}

function retryTree() {
  if (isDisconnectedError(state.treeError)) refreshCurrent();
  else loadTree({ preserveExpansion: true });
}

function errorMessage(error) {
  if (error instanceof RpcError) {
    return error.code && error.code !== "error" ? `${error.message} (${error.code})` : error.message;
  }
  return error?.message ? String(error.message) : String(error);
}

function authHeaders() {
  return { Authorization: `Bearer ${state.token}` };
}

async function rpc(method, params = {}, options = {}) {
  if (!state.token) throw new RpcError("The controller token is missing.", "unauthorized");
  let response;
  try {
    response = await fetch("/rpc", {
      method: "POST",
      headers: { ...authHeaders(), "Content-Type": "application/json" },
      body: JSON.stringify({ jsonrpc: "2.0", id: ++rpcSequence, method, params }),
      signal: options.signal,
    });
  } catch (error) {
    if (error?.name === "AbortError") throw error;
    setConnection("offline", "Offline");
    throw new RpcError("Could not reach the controller. Is cove-devtools serve still running?", "network_error");
  }
  const raw = await response.text();
  let reply = null;
  try {
    reply = raw ? JSON.parse(raw) : null;
  } catch {
    throw new RpcError("The controller returned an invalid response.", "invalid_response");
  }
  if (response.status === 401) {
    setConnection("offline", "Unauthorized");
    throw new RpcError("The controller rejected this session token.", "unauthorized");
  }
  if (!response.ok) {
    throw new RpcError(reply?.message || `The controller returned HTTP ${response.status}.`, "http_error");
  }
  if (!reply || typeof reply !== "object") {
    throw new RpcError("The controller returned an empty response.", "invalid_response");
  }
  if (reply.error) {
    throw new RpcError(reply.error.message || "The controller reported an error.", reply.error.data?.code || "error", reply.error.data ?? null);
  }
  setConnection("online", "Connected");
  return reply.result;
}

function unwrapData(result) {
  if (result && result.ok === false) {
    throw new RpcError(result.error?.message || "The runtime reported an error.", result.error?.code || "runtime_error");
  }
  if (result && result.ok === true && Object.prototype.hasOwnProperty.call(result, "data")) return result.data;
  return result;
}

function targetById(id = state.targetId) {
  return state.targets.find(target => target.id === id) || null;
}

function readTargetCaps(target) {
  const caps = emptyCaps();
  if (!target) return caps;
  const advertised = new Set(Array.isArray(target.capabilities) ? target.capabilities : []);
  caps.runtime = target.kind === "runtime";
  caps.tree = caps.runtime && advertised.has("tree");
  caps.capture = advertised.has("capture");
  caps.input = advertised.has("input");
  caps.logs = caps.runtime && advertised.has("diagnostics");
  caps.state = caps.runtime && advertised.has("state");
  caps.network = caps.runtime && advertised.has("network");
  caps.networkControl = caps.runtime && advertised.has("network_control");
  caps.timing = caps.runtime && advertised.has("timing");
  caps.trace = caps.runtime && advertised.has("trace");
  caps.frameAnalysis = caps.runtime && FRAME_ANALYSIS_CAPABILITIES.some(name => advertised.has(name));
  caps.profile = caps.runtime && PROFILE_CAPABILITIES.some(name => advertised.has(name));
  caps.inspectNodes = caps.runtime && advertised.has("inspect_nodes");
  caps.repaintHighlight = caps.runtime && advertised.has("repaint_highlight");
  caps.debugPaint = caps.runtime && advertised.has("debug_paint");
  caps.baselines = caps.runtime && advertised.has("baselines");
  caps.animationSpeed = caps.runtime && advertised.has("animation_speed");
  caps.oversizedImages = caps.runtime && advertised.has("oversized_images");
  caps.allocationSites = caps.runtime && ALLOCATION_SITE_CAPABILITIES.some(name => advertised.has(name));
  caps.style = caps.runtime && advertised.has("style");
  return caps;
}

function targetLabel(target) {
  if (!target) return "No target";
  const kind = target.kind === "runtime" ? "runtime" : target.kind;
  return `${target.title} · ${kind}`;
}

function defaultTargetId() {
  const runtime = state.targets.find(target => target.kind === "runtime" && Array.isArray(target.capabilities) && target.capabilities.includes("tree"));
  if (runtime) return runtime.id;
  const anyRuntime = state.targets.find(target => target.kind === "runtime");
  if (anyRuntime) return anyRuntime.id;
  const window = state.targets.find(target => target.kind === "window");
  if (window) return window.id;
  return state.targets[0]?.id || "";
}

function readToken() {
  const hash = location.hash.replace(/^#/, "");
  if (hash) {
    let token = hash;
    try { token = decodeURIComponent(hash); } catch { token = hash; }
    if (/^[A-Za-z0-9_-]{1,256}$/.test(token)) {
      try { sessionStorage.setItem(TOKEN_KEY, token); } catch {}
      history.replaceState(null, "", `${location.pathname}${location.search}`);
      return token;
    }
  }
  try {
    const stored = sessionStorage.getItem(TOKEN_KEY) || "";
    return /^[A-Za-z0-9_-]{1,256}$/.test(stored) ? stored : "";
  } catch {
    return "";
  }
}

function storeToken(token) {
  state.token = token;
  try { sessionStorage.setItem(TOKEN_KEY, token); } catch {}
}

function showSetup(error = "") {
  if (state.token) {
    state.token = "";
    try { sessionStorage.removeItem(TOKEN_KEY); } catch {}
  }
  dom.setup.hidden = false;
  dom.inspectorContent.hidden = true;
  dom.targets.disabled = true;
  dom.targets.replaceChildren(new Option("No target", ""));
  dom.setupError.hidden = !error;
  dom.setupError.textContent = error;
  dom.tokenInput.value = "";
  clear(dom.railMeta);
  setConnection("offline", "Disconnected");
}

function showWorkspace() {
  dom.setup.hidden = true;
  dom.inspectorContent.hidden = false;
  dom.targets.disabled = state.targets.length === 0;
}

function setThemePreference(preference) {
  state.theme = ["auto", "light", "dark"].includes(preference) ? preference : "auto";
  try { localStorage.setItem(THEME_KEY, state.theme); } catch {}
  applyTheme();
}

function applyTheme() {
  const resolved = state.theme === "auto"
    ? (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light")
    : state.theme;
  document.documentElement.dataset.theme = resolved;
  const iconName = state.theme === "auto" ? "auto" : resolved === "dark" ? "moon" : "sun";
  dom.themeIcon.setAttribute("href", `#i-${iconName}`);
  const label = state.theme === "auto" ? "follow system" : state.theme;
  dom.themeButton.title = `Theme: ${label}`;
  dom.themeButton.setAttribute("aria-label", `Theme: ${label}`);
}

function renderTargets() {
  const previous = state.targetId;
  dom.targets.replaceChildren();
  if (!state.targets.length) {
    dom.targets.append(new Option("No targets connected", ""));
    dom.targets.disabled = true;
    return;
  }
  const runtimes = state.targets.filter(target => target.kind === "runtime");
  const platforms = state.targets.filter(target => target.kind !== "runtime");
  if (runtimes.length) {
    const group = document.createElement("optgroup");
    group.label = "Cove runtimes";
    for (const target of runtimes) group.append(new Option(targetLabel(target), target.id));
    dom.targets.append(group);
  }
  if (platforms.length) {
    const group = document.createElement("optgroup");
    group.label = "Windows, displays and drivers";
    for (const target of platforms) group.append(new Option(targetLabel(target), target.id));
    dom.targets.append(group);
  }
  if (state.targets.some(target => target.id === previous)) dom.targets.value = previous;
  else if (state.targets.some(target => target.id === state.targetId)) dom.targets.value = state.targetId;
  else dom.targets.value = defaultTargetId();
  dom.targets.disabled = false;
}

async function refreshSession(options = {}) {
  const quiet = options.quiet === true;
  if (!state.token) {
    showSetup();
    return;
  }
  if (state.sessionLoading) return;
  state.sessionLoading = true;
  setConnectionBusy(true);
  const previousTarget = state.targetId;
  try {
    const session = await rpc("get_ui_session");
    state.platform = String(session?.platform || "");
    state.targets = Array.isArray(session?.targets) ? session.targets : [];
    let next = previousTarget;
    if (!state.targets.some(target => target.id === next)) next = defaultTargetId();
    if (next !== previousTarget) {
      await activateTarget(next, { reset: true });
      renderTargets();
    } else {
      state.targetId = next;
      renderTargets();
      updateTargetUi();
      if (state.caps.tree && !state.treeLoaded && !state.treeLoading) loadTree();
      ensureCurrentView();
    }
    showWorkspace();
    if (!quiet) {
      const count = state.targets.length;
      toast(count ? `${count} target${count === 1 ? "" : "s"} connected.` : "No targets are connected. Start the app with COVE_DEVTOOLS=1.", count ? "success" : "info");
    }
  } catch (error) {
    if (error?.code === "unauthorized") {
      showSetup("The controller rejected this session token. Open the Inspector link again or paste a current token.");
    } else {
      setConnection("offline", "Offline");
      if (!quiet) toast(errorMessage(error), "error", 9000);
      if (!state.targets.length) {
        showWorkspace();
        renderTreeArea();
        renderDetails();
      }
    }
  } finally {
    state.sessionLoading = false;
    setConnectionBusy(false);
    if (state.token) ensureDeepLinks();
  }
}

async function chooseTarget(id) {
  if (!id || id === state.targetId) return;
  await activateTarget(id, { reset: true });
}

async function activateTarget(id, options = {}) {
  const previousTarget = state.targetId;
  if (options.reset) resetTargetData(previousTarget);
  state.targetId = id;
  state.targetEpoch += 1;
  state.caps = readTargetCaps(targetById(id));
  dom.targets.value = id;
  updateTargetUi();
  if (state.caps.tree) {
    await loadTree();
  } else {
    state.treeLoaded = true;
    renderTreeArea();
    renderDetails();
  }
  ensureCurrentView();
  syncFollowTimers();
}

function updateTargetUi() {
  const target = targetById();
  state.caps = readTargetCaps(target);
  if ((state.repaintHighlight.active || state.repaintHighlight.busy) && !state.caps.repaintHighlight) resetRepaintHighlight(target?.id || "");
  if ((state.debugPaint.guidelines || state.debugPaint.busy) && !state.caps.debugPaint) resetDebugPaintState(target?.id || "");
  if ((state.baselines.active || state.baselines.busy) && !state.caps.baselines) resetBaselines(target?.id || "");
  if ((state.slowAnimations.active || state.slowAnimations.busy) && !state.caps.animationSpeed) resetSlowAnimations(target?.id || "");
  if ((state.oversizedImages.active || state.oversizedImages.busy) && !state.caps.oversizedImages) resetOversizedImages(target?.id || "");
  if (state.mode === "pick" && (!state.caps.capture || !state.caps.inspectNodes)) {
    state.mode = "select";
  }
  if (state.mode === "interact" && !(state.caps.capture && state.caps.input)) state.mode = "select";
  const caps = state.caps;
  dom.captureButton.disabled = !caps.capture || state.captureLoading;
  dom.nodeShotButton.disabled = !(caps.inspectNodes && caps.capture) || !state.selectedId || state.captureLoading;
  dom.saveButton.disabled = !state.capture;
  dom.modePick.disabled = !(caps.inspectNodes && caps.capture);
  dom.treeRefresh.disabled = !caps.tree || state.treeLoading;
  dom.treeCollapse.disabled = !caps.tree || !state.treeLoaded;
  updateModeUi();
  dom.tree.hidden = !caps.tree || !state.treeLoaded || state.treeError !== null;
  renderCapabilityNote();
  renderRepaintHighlightUi();
  renderGuidelinesUi();
  renderBaselinesUi();
  renderSlowAnimationsUi();
  renderOversizedImagesUi();
  renderInputUi();
  updateRailMeta();
  if (!state.capture && caps.capture) dom.screenshotMeta.textContent = "";
  renderScreenshot();
  if (state.treeLoaded) renderTreeArea();
  if (state.selectedId) renderDetails();
}

function updateRailMeta() {
  const target = targetById();
  clear(dom.railMeta);
  if (!target) return;
  dom.railMeta.append(
    el("div", { text: target.kind === "runtime" ? "Runtime" : target.kind }),
    el("div", { text: target.platform || "" }),
  );
}

function renderCapabilityNote() {
  const target = targetById();
  const caps = state.caps;
  const notes = [];
  if (!target) {
    notes.push("No target is selected.");
  } else if (!caps.runtime && !caps.capture) {
    notes.push("This target exposes neither a component tree nor capture.");
  } else {
    if (!caps.tree) notes.push("This runtime does not advertise a component tree.");
    if (!caps.inspectNodes) notes.push("This runtime does not advertise inspect_nodes: pick-at-point and node screenshots are unavailable.");
    if (!caps.capture) notes.push("This runtime does not advertise frame capture.");
  }
  dom.capabilityNote.hidden = notes.length === 0;
  dom.capabilityNote.textContent = notes.join(" ");
}

// Repaint highlighting is driven by the runtime's real paint passes: the
// controller reports which nodes painted, not how many frames elapsed. The
// client only mirrors the applied state that `set_ui_repaint_highlight`
// returns, so cached layer reuse never shows up as a repaint here.
function renderRepaintHighlightUi() {
  const target = targetById();
  const available = Boolean(state.connected && target && state.caps.runtime && state.caps.repaintHighlight);
  const on = available && state.repaintHighlight.active;
  let title = on ? "Hide repaint highlighting on the target" : "Highlight repainted nodes on the target";
  if (!state.connected) title = "Reconnect to the controller to toggle repaint highlighting";
  else if (!target || !state.caps.runtime) title = "Repaint highlighting needs a Cove runtime target";
  else if (!state.caps.repaintHighlight) title = "This runtime does not advertise repaint_highlight";
  dom.repaintHighlightButton.disabled = !available || state.repaintHighlight.busy;
  dom.repaintHighlightButton.setAttribute("aria-pressed", String(on));
  dom.repaintHighlightButton.classList.toggle("is-active", on);
  dom.repaintHighlightButton.title = title;
}

function sendRepaintHighlight(targetId, enabled) {
  const run = state.repaintHighlight.queue.then(() => rpc("set_ui_repaint_highlight", { target: targetId, enabled }));
  state.repaintHighlight.queue = run.then(() => {}, () => {});
  return run;
}

function repaintHighlightFrom(result, requested) {
  const data = unwrapData(result);
  if (typeof data === "boolean") return data;
  const record = data && typeof data === "object" ? data : null;
  for (const value of [record?.repaintHighlight, record?.repaint_highlight, record?.highlightRepaints, record?.highlight_repaints, record?.enabled]) {
    if (typeof value === "boolean") return value;
  }
  return requested;
}

function resetRepaintHighlight(previousTarget = state.targetId) {
  const wasOn = state.repaintHighlight.active;
  const inFlight = state.repaintHighlight.busy;
  state.repaintHighlight.request += 1;
  state.repaintHighlight.active = false;
  state.repaintHighlight.busy = false;
  renderRepaintHighlightUi();
  if (!previousTarget || (!wasOn && !inFlight)) return;
  sendRepaintHighlight(previousTarget, false).catch(() => {});
}

async function toggleRepaintHighlight() {
  const target = targetById();
  if (!state.connected || !target || !state.caps.runtime || !state.caps.repaintHighlight || state.repaintHighlight.busy) return;
  const targetId = target.id;
  const epoch = state.targetEpoch;
  const request = ++state.repaintHighlight.request;
  const next = !state.repaintHighlight.active;
  state.repaintHighlight.busy = true;
  renderRepaintHighlightUi();
  try {
    const enabled = repaintHighlightFrom(await sendRepaintHighlight(targetId, next), next);
    if (request !== state.repaintHighlight.request || epoch !== state.targetEpoch || targetId !== state.targetId) return;
    state.repaintHighlight.active = enabled;
  } catch (error) {
    if (request === state.repaintHighlight.request && epoch === state.targetEpoch && targetId === state.targetId) {
      toast(`Highlight repaints failed: ${errorMessage(error)}`, "error", 8000);
    }
  } finally {
    if (request === state.repaintHighlight.request) {
      state.repaintHighlight.busy = false;
      renderRepaintHighlightUi();
    }
  }
}

function renderGuidelinesUi() {
  const target = targetById();
  const available = Boolean(state.connected && target && state.caps.runtime && state.caps.debugPaint);
  const on = available && state.debugPaint.guidelines;
  let title = on ? "Hide layout guidelines on the target" : "Show layout guidelines on the target";
  if (!state.connected) title = "Reconnect to the controller to toggle layout guidelines";
  else if (!target || !state.caps.runtime) title = "Layout guidelines need a Cove runtime target";
  else if (!state.caps.debugPaint) title = "This runtime does not advertise debug_paint";
  dom.guidelinesButton.disabled = !available || state.debugPaint.busy;
  dom.guidelinesButton.setAttribute("aria-pressed", String(on));
  dom.guidelinesButton.classList.toggle("is-active", on);
  dom.guidelinesButton.title = title;
}

function sendDebugPaint(targetId, guidelines) {
  const run = state.debugPaint.queue.then(() => rpc("set_ui_debug_paint", { target: targetId, guidelines }));
  state.debugPaint.queue = run.then(() => {}, () => {});
  return run;
}

function debugPaintFrom(result, requested) {
  const data = unwrapData(result);
  if (typeof data === "boolean") return data;
  const record = data && typeof data === "object" ? data : null;
  for (const value of [record?.guidelines, record?.debugPaint, record?.debug_paint, record?.enabled]) {
    if (typeof value === "boolean") return value;
  }
  return requested;
}

function resetDebugPaintState(previousTarget = state.targetId) {
  const wasOn = state.debugPaint.guidelines;
  const inFlight = state.debugPaint.busy;
  state.debugPaint.request += 1;
  state.debugPaint.guidelines = false;
  state.debugPaint.busy = false;
  renderGuidelinesUi();
  if (!previousTarget || (!wasOn && !inFlight)) return;
  sendDebugPaint(previousTarget, false).catch(() => {});
}

async function toggleGuidelines() {
  const target = targetById();
  if (!state.connected || !target || !state.caps.runtime || !state.caps.debugPaint || state.debugPaint.busy) return;
  const targetId = target.id;
  const epoch = state.targetEpoch;
  const request = ++state.debugPaint.request;
  const next = !state.debugPaint.guidelines;
  state.debugPaint.busy = true;
  renderGuidelinesUi();
  try {
    const enabled = debugPaintFrom(await sendDebugPaint(targetId, next), next);
    if (request !== state.debugPaint.request || epoch !== state.targetEpoch || targetId !== state.targetId) return;
    state.debugPaint.guidelines = enabled;
  } catch (error) {
    if (request === state.debugPaint.request && epoch === state.targetEpoch && targetId === state.targetId) {
      toast(`Guidelines failed: ${errorMessage(error)}`, "error", 8000);
    }
  } finally {
    if (request === state.debugPaint.request) {
      state.debugPaint.busy = false;
      renderGuidelinesUi();
    }
  }
}

function renderBaselinesUi() {
  const target = targetById();
  const available = Boolean(state.connected && target && state.caps.runtime && state.caps.baselines);
  const on = available && state.baselines.active;
  let title = on ? "Hide text baselines on the target" : "Show text baselines on the target";
  if (!state.connected) title = "Reconnect to the controller to toggle text baselines";
  else if (!target || !state.caps.runtime) title = "Text baselines need a Cove runtime target";
  else if (!state.caps.baselines) title = "This runtime does not advertise baselines";
  dom.baselinesButton.disabled = !available || state.baselines.busy;
  dom.baselinesButton.setAttribute("aria-pressed", String(on));
  dom.baselinesButton.classList.toggle("is-active", on);
  dom.baselinesButton.title = title;
}

function sendBaselines(targetId, enabled) {
  const run = state.baselines.queue.then(() => rpc("set_ui_baselines", { target: targetId, enabled }));
  state.baselines.queue = run.then(() => {}, () => {});
  return run;
}

function baselinesFrom(result, requested) {
  const data = unwrapData(result);
  if (typeof data === "boolean") return data;
  const record = data && typeof data === "object" ? data : null;
  for (const value of [record?.baselines, record?.baselinesEnabled, record?.baselines_enabled, record?.enabled]) {
    if (typeof value === "boolean") return value;
  }
  return requested;
}

function resetBaselines(previousTarget = state.targetId) {
  const wasOn = state.baselines.active;
  const inFlight = state.baselines.busy;
  state.baselines.request += 1;
  state.baselines.active = false;
  state.baselines.busy = false;
  renderBaselinesUi();
  if (!previousTarget || (!wasOn && !inFlight)) return;
  sendBaselines(previousTarget, false).catch(() => {});
}

async function toggleBaselines() {
  const target = targetById();
  if (!state.connected || !target || !state.caps.runtime || !state.caps.baselines || state.baselines.busy) return;
  const targetId = target.id;
  const epoch = state.targetEpoch;
  const request = ++state.baselines.request;
  const next = !state.baselines.active;
  state.baselines.busy = true;
  renderBaselinesUi();
  try {
    const enabled = baselinesFrom(await sendBaselines(targetId, next), next);
    if (request !== state.baselines.request || epoch !== state.targetEpoch || targetId !== state.targetId) return;
    state.baselines.active = enabled;
  } catch (error) {
    if (request === state.baselines.request && epoch === state.targetEpoch && targetId === state.targetId) {
      toast(`Baselines failed: ${errorMessage(error)}`, "error", 8000);
    }
  } finally {
    if (request === state.baselines.request) {
      state.baselines.busy = false;
      renderBaselinesUi();
    }
  }
}

function sendAnimationSpeed(targetId, scale) {
  const run = state.slowAnimations.queue.then(() => rpc("set_ui_animation_speed", { target: targetId, scale }));
  state.slowAnimations.queue = run.then(() => {}, () => {});
  return run;
}

function animationScaleFrom(result, requested) {
  const data = unwrapData(result);
  if (typeof data === "number" && Number.isFinite(data)) return data;
  const record = data && typeof data === "object" ? data : null;
  for (const value of [record?.scale, record?.animationSpeed, record?.animation_speed, record?.speed, record?.timeDilation, record?.time_dilation]) {
    if (typeof value === "number" && Number.isFinite(value)) return value;
  }
  return requested;
}

function isSlowAnimationScale(scale) {
  return Number.isFinite(scale) && scale > 0 && scale < 0.999;
}

function renderSlowAnimationsUi() {
  const target = targetById();
  const available = Boolean(state.connected && target && state.caps.runtime && state.caps.animationSpeed);
  const on = available && state.slowAnimations.active;
  let title = on ? "Resume normal animation speed" : "Slow animations to 0.2× the runtime time scale";
  if (!state.connected) title = "Reconnect to the controller to change the animation speed";
  else if (!target || !state.caps.runtime) title = "Slow animations need a Cove runtime target";
  else if (!state.caps.animationSpeed) title = "This runtime does not advertise animation_speed";
  dom.slowAnimationsButton.disabled = !available || state.slowAnimations.busy;
  dom.slowAnimationsButton.setAttribute("aria-pressed", String(on));
  dom.slowAnimationsButton.classList.toggle("is-active", on);
  dom.slowAnimationsButton.title = title;
}

function resetSlowAnimations(previousTarget = state.targetId) {
  const wasSlowed = state.slowAnimations.active;
  const inFlight = state.slowAnimations.busy;
  state.slowAnimations.request += 1;
  state.slowAnimations.active = false;
  state.slowAnimations.scale = NORMAL_ANIMATION_SCALE;
  state.slowAnimations.busy = false;
  renderSlowAnimationsUi();
  if (!previousTarget || (!wasSlowed && !inFlight)) return;
  sendAnimationSpeed(previousTarget, NORMAL_ANIMATION_SCALE).catch(() => {});
}

async function toggleSlowAnimations() {
  const target = targetById();
  if (!state.connected || !target || !state.caps.runtime || !state.caps.animationSpeed || state.slowAnimations.busy) return;
  const targetId = target.id;
  const epoch = state.targetEpoch;
  const request = ++state.slowAnimations.request;
  const next = state.slowAnimations.active ? NORMAL_ANIMATION_SCALE : SLOW_ANIMATION_SCALE;
  state.slowAnimations.busy = true;
  renderSlowAnimationsUi();
  try {
    const scale = animationScaleFrom(await sendAnimationSpeed(targetId, next), next);
    if (request !== state.slowAnimations.request || epoch !== state.targetEpoch || targetId !== state.targetId) return;
    state.slowAnimations.active = isSlowAnimationScale(scale);
    state.slowAnimations.scale = scale;
  } catch (error) {
    if (request === state.slowAnimations.request && epoch === state.targetEpoch && targetId === state.targetId) {
      toast(`Slow animations failed: ${errorMessage(error)}`, "error", 8000);
    }
  } finally {
    if (request === state.slowAnimations.request) {
      state.slowAnimations.busy = false;
      renderSlowAnimationsUi();
    }
  }
}

function renderOversizedImagesUi() {
  const target = targetById();
  const available = Boolean(state.connected && target && state.caps.runtime && state.caps.oversizedImages);
  const on = available && state.oversizedImages.active;
  let title = on ? "Hide oversized image highlighting on the target" : "Highlight images decoded larger than their display rect";
  if (!state.connected) title = "Reconnect to the controller to toggle oversized image highlighting";
  else if (!target || !state.caps.runtime) title = "Oversized image highlighting needs a Cove runtime target";
  else if (!state.caps.oversizedImages) title = "This runtime does not advertise oversized_images";
  dom.oversizedImagesButton.disabled = !available || state.oversizedImages.busy;
  dom.oversizedImagesButton.setAttribute("aria-pressed", String(on));
  dom.oversizedImagesButton.classList.toggle("is-active", on);
  dom.oversizedImagesButton.title = title;
}

function sendOversizedImages(targetId, enabled) {
  const run = state.oversizedImages.queue.then(() => rpc("set_ui_oversized_images", { target: targetId, enabled }));
  state.oversizedImages.queue = run.then(() => {}, () => {});
  return run;
}

function oversizedImagesFrom(result, requested) {
  const data = unwrapData(result);
  if (typeof data === "boolean") return data;
  const record = data && typeof data === "object" ? data : null;
  for (const value of [record?.oversizedImages, record?.oversized_images, record?.enabled]) {
    if (typeof value === "boolean") return value;
  }
  return requested;
}

function resetOversizedImages(previousTarget = state.targetId) {
  const wasOn = state.oversizedImages.active;
  const inFlight = state.oversizedImages.busy;
  state.oversizedImages.request += 1;
  state.oversizedImages.active = false;
  state.oversizedImages.busy = false;
  renderOversizedImagesUi();
  if (!previousTarget || (!wasOn && !inFlight)) return;
  sendOversizedImages(previousTarget, false).catch(() => {});
}

async function toggleOversizedImages() {
  const target = targetById();
  if (!state.connected || !target || !state.caps.runtime || !state.caps.oversizedImages || state.oversizedImages.busy) return;
  const targetId = target.id;
  const epoch = state.targetEpoch;
  const request = ++state.oversizedImages.request;
  const next = !state.oversizedImages.active;
  state.oversizedImages.busy = true;
  renderOversizedImagesUi();
  try {
    const enabled = oversizedImagesFrom(await sendOversizedImages(targetId, next), next);
    if (request !== state.oversizedImages.request || epoch !== state.targetEpoch || targetId !== state.targetId) return;
    state.oversizedImages.active = enabled;
  } catch (error) {
    if (request === state.oversizedImages.request && epoch === state.targetEpoch && targetId === state.targetId) {
      toast(`Oversized images failed: ${errorMessage(error)}`, "error", 8000);
    }
  } finally {
    if (request === state.oversizedImages.request) {
      state.oversizedImages.busy = false;
      renderOversizedImagesUi();
    }
  }
}

function resetTargetData(previousTarget = state.targetId) {
  state.tree = null;
  state.entries = new Map();
  state.rootId = null;
  state.treeLoaded = false;
  state.treeLoading = false;
  state.treeError = null;
  state.treeRequest += 1;
  state.selectedId = null;
  state.detail = null;
  state.detailLoading = false;
  state.detailError = null;
  state.detailRequest += 1;
  state.valueRequested = false;
  clearTimeout(state.detailTimer);
  state.detailTimer = null;
  clearTimeout(state.highlightTimer);
  state.highlightTimer = null;
  state.expanded.clear();
  state.search = "";
  dom.treeSearch.value = "";
  state.visibleModel = null;
  state.visibleRows = [];
  state.mode = "select";
  releaseCapture();
  state.captureLoading = false;
  state.captureError = null;
  state.captureAttempt = "window";
  resetPerformance(previousTarget);
  resetNetwork();
  resetLogs();
  resetRegisteredState();
  resetMemory(previousTarget);
  resetAppSize();
  resetDebugger(previousTarget);
  resetRepaintHighlight(previousTarget);
  resetDebugPaintState(previousTarget);
  resetBaselines(previousTarget);
  resetSlowAnimations(previousTarget);
  resetOversizedImages(previousTarget);
  resetFlexEditorState();
  stopAllFollowTimers();
  clear(dom.tree);
  clear(dom.detailsState);
  dom.detailsContent.hidden = true;
  dom.treeState.hidden = false;
  dom.tree.hidden = true;
  dom.treeFooter.hidden = true;
  dom.treeMeta.textContent = "";
  dom.screenshotMeta.textContent = "";
  dom.networkBadge.hidden = true;
  dom.logsBadge.hidden = true;
  if (dom.networkDialog.open) dom.networkDialog.close();
}

function ensureCurrentView() {
  if (state.view === "network") ensureNetwork();
  else if (state.view === "logs") ensureLogs();
  else if (state.view === "state") ensureRegisteredState();
  else if (state.view === "performance") ensurePerformance();
  else if (state.view === "memory") ensureMemory();
  else if (state.view === "app-size") ensureAppSize();
  else if (state.view === "deep-links") ensureDeepLinks();
  else if (state.view === "debugger") ensureDebugger();
}

function showState(host, options = {}) {
  clear(host);
  host.hidden = false;
  const kind = options.kind || "empty";
  const stateNode = el("div", { class: `state ${kind === "loading" ? "is-loading" : kind === "error" ? "is-error" : ""} ${options.compact ? "compact" : ""}` });
  if (kind === "loading") {
    stateNode.append(el("span", { class: "state-icon" }, el("span", { class: "spinner", attrs: { role: "progressbar", "aria-label": options.title || "Loading" } })));
  } else {
    stateNode.append(el("span", { class: "state-icon" }, icon(options.icon || (kind === "error" ? "alert" : "empty"))));
  }
  if (options.title) stateNode.append(el("strong", { text: options.title }));
  if (options.message) stateNode.append(el("p", { text: options.message }));
  if (options.action) {
    stateNode.append(el("button", {
      class: `tool-button ${options.primary ? "is-primary" : ""}`,
      props: { type: "button" },
      text: options.action,
      on: { click: options.onAction || (() => {}) },
    }));
  }
  host.append(stateNode);
  return host;
}

function hideState(host) {
  clear(host);
  host.hidden = true;
}

function sectionBlock(title, ...children) {
  return el("div", { class: "section" }, el("h3", { text: title }), ...children);
}

function kvList(pairs) {
  const dl = el("dl", { class: "kv" });
  for (const [label, value] of pairs) {
    if (value === undefined) continue;
    dl.append(el("dt", { text: label }), el("dd", {}, value instanceof Node ? value : textOf(value)));
  }
  return dl;
}

function emptyInline(message) {
  return el("div", { class: "empty-inline", text: message });
}

function errorInline(message) {
  return el("div", { class: "error-inline", text: message });
}

function metric(label, value, suffix = "") {
  return el("div", { class: "metric" },
    el("span", { class: "metric-label", text: label }),
    el("span", { class: "metric-value" }, String(value), suffix ? el("small", { text: ` ${suffix}` }) : null));
}

function rawDetails(label, value) {
  return el("details", { class: "raw" },
    el("summary", { text: label }),
    el("pre", { text: JSON.stringify(value, null, 2) }));
}

function nodeChildren(node) {
  return Array.isArray(node?.children) ? node.children : [];
}

function nodeIsAnchor(node, isRoot = false) {
  if (isRoot) return true;
  return node?.ownerScopeID === node?.id || Boolean(node?.recomposition);
}

function shortType(viewTag) {
  const value = String(viewTag ?? "");
  if (!value) return "";
  const path = value.split("::");
  return path[path.length - 1];
}

function nodeLabel(node) {
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

async function loadTree(options = {}) {
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

function renderTreeArea() {
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
    const title = [row.node.debugName, row.node.viewTag, role, row.node.id].filter(Boolean).join(" · ");
    appendChildren(rowNode, [
      el("span", { class: "tree-indent", attrs: { "aria-hidden": "true", style: `width:${row.depth * 14}px;flex:0 0 ${row.depth * 14}px` } }),
      twisty,
      el("span", { class: `node-glyph ${row.entry.anchor ? "anchor" : "primitive"}`, attrs: { "aria-hidden": "true" } }),
      label,
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

function selectNode(id, options = {}) {
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

async function loadNodeDetail(id) {
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

async function readLiveValue() {
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

function highlightNode(id) {
  const targetId = state.targetId;
  const epoch = state.targetEpoch;
  clearTimeout(state.highlightTimer);
  state.highlightTimer = setTimeout(() => {
    rpc("select_ui_node", { target: targetId, node_id: id }).catch(error => {
      if (epoch === state.targetEpoch && targetId === state.targetId) toast(`Highlight failed: ${errorMessage(error)}`, "error", 7000);
    });
  }, 90);
}

function clearHighlight() {
  clearTimeout(state.highlightTimer);
  const targetId = state.targetId;
  rpc("select_ui_node", { target: targetId }).catch(error => toast(errorMessage(error), "error", 7000));
}

function selectedEntry() {
  return state.selectedId ? state.entries.get(state.selectedId) : null;
}

function currentNode() {
  const entry = selectedEntry();
  if (!entry) return null;
  const detailNode = state.detail?.node;
  if (!detailNode) return entry.node;
  return { ...entry.node, ...detailNode, children: entry.node.children };
}

function renderDetails() {
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

function renderRender(node, detail, entry) {
  const panel = dom.panels.render;
  clear(panel);
  const flags = node.flags || {};
  const frame = node.frame;
  const absolute = node.absoluteFrame;
  panel.append(sectionBlock("Paint and input flags",
    el("div", { class: "metrics" },
      metric("Hit testable", flags.hitTestable ? "yes" : "no"),
      metric("Focusable", flags.focusable ? "yes" : "no"),
      metric("Clips", flags.clipsToBounds ? "yes" : "no"),
      metric("Background", flags.hasBackground ? "yes" : "no"),
      metric("Border", flags.hasBorder ? "yes" : "no"),
    ),
  ));
  panel.append(sectionBlock("Render state",
    kvList([
      ["Frame", frame ? `${fmtNumber(frame.x)} × ${fmtNumber(frame.y)} · ${fmtNumber(frame.w)} × ${fmtNumber(frame.h)}` : "—"],
      ["Absolute frame", absolute ? `${fmtNumber(absolute.x)} × ${fmtNumber(absolute.y)} · ${fmtNumber(absolute.w)} × ${fmtNumber(absolute.h)}` : "—"],
      ["Z-index", node.zIndex ?? "—"],
      ["Opacity", node.opacity === undefined || node.opacity === null ? "—" : fmtNumber(node.opacity, 2)],
      ["Content offset", node.contentOffset ? `${fmtNumber(node.contentOffset.x)} × ${fmtNumber(node.contentOffset.y)}` : "—"],
    ]),
  ));

  const style = node.style;
  if (style) {
    panel.append(sectionBlock("Style",
      kvList([
        ["Background", style.backgroundColor || "—"],
        ["Foreground", style.foregroundColor || "—"],
        ["Overrides", Array.isArray(style.overrides) && style.overrides.length ? style.overrides.join(", ") : "—"],
      ]),
    ));
  }

  const inventory = state.tree?.renderInventory;
  if (inventory) {
    const isLayerRoot = Array.isArray(inventory.layerRoots) && inventory.layerRoots.includes(node.id);
    panel.append(sectionBlock("Render inventory",
      kvList([
        ["Objects", inventory.objectCount ?? "—"],
        ["Layer roots", Array.isArray(inventory.layerRoots) ? inventory.layerRoots.length : "—"],
        ["This node is a layer root", isLayerRoot ? "yes" : "no"],
      ]),
    ));
  } else {
    panel.append(sectionBlock("Render inventory", emptyInline("The runtime did not publish a render tree inventory.")));
  }

  const inputInventory = state.tree?.inputInventory;
  if (inputInventory) {
    panel.append(sectionBlock("Input inventory",
      kvList([
        ["Nodes", inputInventory.nodeCount ?? "—"],
        ["Focusable", Array.isArray(inputInventory.focusables) ? inputInventory.focusables.length : "—"],
        ["Hit testable", Array.isArray(inputInventory.hitTestables) ? inputInventory.hitTestables.length : "—"],
        ["This node focusable", Array.isArray(inputInventory.focusables) && inputInventory.focusables.includes(node.id) ? "yes" : "no"],
        ["This node hit testable", Array.isArray(inputInventory.hitTestables) && inputInventory.hitTestables.includes(node.id) ? "yes" : "no"],
      ]),
    ));
  }

  const invalidations = Array.isArray(state.tree?.invalidations) ? state.tree.invalidations : null;
  if (invalidations) {
    const mine = invalidations.filter(record => record.target === node.id).slice(-12).reverse();
    const children = [];
    if (!mine.length) {
      children.push(emptyInline("No recorded invalidations target this node in the snapshot window."));
    } else {
      const rows = el("table", { class: "data-table" },
        el("thead", {}, el("tr", {},
          el("th", { text: "Phase" }), el("th", { text: "Source" }), el("th", { text: "Time" }),
        )),
        el("tbody", {}, ...mine.map(record => el("tr", {},
          el("td", { text: record.phase || "—" }),
          el("td", { class: "mono-cell", text: record.source || "—" }),
          el("td", { class: "mono-cell", text: fmtNumber(record.timestamp, 3) }),
        ))),
      );
      children.push(el("div", { class: "table-wrap", attrs: { style: "max-height:220px" } }, rows));
    }
    panel.append(sectionBlock("Recent invalidations", ...children));
  }
  if (state.detailError) panel.append(errorInline(errorMessage(state.detailError)));
}

function insetText(inset) {
  if (!inset) return "—";
  return `${fmtNumber(inset.top)} / ${fmtNumber(inset.right)} / ${fmtNumber(inset.bottom)} / ${fmtNumber(inset.left)}`;
}

const FLEX_FIELD_IDS = {
  flexGrow: "flex-grow",
  flexShrink: "flex-shrink",
  justifyContent: "flex-justify",
  alignItems: "flex-align",
};

function isFlexFactor(name) {
  return name === "flexGrow" || name === "flexShrink";
}

function flexEditorKey(nodeId) {
  return `${state.targetId}\u0000${nodeId}`;
}

function flexRecordValue(layout, name) {
  if (!layout) return null;
  if (isFlexFactor(name)) return numberValue(layout[name]);
  const value = layout[name];
  return typeof value === "string" && value ? value : null;
}

function flexNumberText(value) {
  const number = numberValue(value);
  if (number === null) return "";
  return String(Number(number.toFixed(3)));
}

function parseFlexFactor(raw) {
  if (typeof raw !== "string" || raw.trim() === "") return null;
  const parsed = Number(raw);
  if (!Number.isFinite(parsed) || parsed < 0 || parsed > FLEX_FACTOR_LIMIT) return null;
  return parsed;
}

function flexFieldText(name, layout, draft) {
  const pending = draft?.values?.[name];
  if (pending !== undefined) return pending;
  const value = flexRecordValue(layout, name);
  return isFlexFactor(name) ? flexNumberText(value) : (value ?? "");
}

function flexSameValue(name, left, right) {
  if (isFlexFactor(name)) {
    return typeof left === "number" && typeof right === "number" && Math.abs(left - right) <= 0.001;
  }
  return left === right;
}

function flexValuesMatch(layout, properties) {
  return FLEX_PROPERTY_NAMES.every(name =>
    !(name in properties) || flexSameValue(name, properties[name], flexRecordValue(layout, name)));
}

function flexDraftFieldInvalid(name, raw) {
  if (raw === undefined) return false;
  if (isFlexFactor(name)) return parseFlexFactor(raw) === null;
  return !raw;
}

function pruneFlexDraft(layout) {
  const values = state.flexEditor.draft?.values;
  if (!values) return;
  for (const name of FLEX_PROPERTY_NAMES) {
    if (!(name in values)) continue;
    const parsed = isFlexFactor(name) ? parseFlexFactor(values[name]) : values[name];
    if (parsed === null || parsed === "") continue;
    if (flexSameValue(name, parsed, flexRecordValue(layout, name))) delete values[name];
  }
}

function flexDraftProperties(layout) {
  const values = state.flexEditor.draft?.values || {};
  const properties = {};
  let invalid = false;
  for (const name of FLEX_PROPERTY_NAMES) {
    if (!(name in values)) continue;
    if (isFlexFactor(name)) {
      const parsed = parseFlexFactor(values[name]);
      if (parsed === null) {
        invalid = true;
        continue;
      }
      const record = flexRecordValue(layout, name);
      if (record === null || Math.abs(parsed - record) > 0.001) properties[name] = parsed;
    } else {
      const allowed = name === "justifyContent" ? FLEX_JUSTIFY_VALUES : FLEX_ALIGN_VALUES;
      if (!allowed.includes(values[name])) {
        invalid = true;
        continue;
      }
      if (values[name] !== flexRecordValue(layout, name)) properties[name] = values[name];
    }
  }
  return { properties, invalid, dirty: Object.keys(properties).length > 0 };
}

function flexEditorFor(node, layout) {
  const editor = state.flexEditor;
  const key = flexEditorKey(node.id);
  if (editor.key !== key) {
    editor.key = key;
    editor.draft = null;
    editor.error = null;
    editor.notice = null;
    editor.stale = false;
  }
  if (!editor.draft) editor.draft = { values: {} };
  pruneFlexDraft(layout);
  return editor;
}

function setFlexDraft(name, value) {
  const editor = state.flexEditor;
  if (!editor.draft) editor.draft = { values: {} };
  editor.draft.values[name] = value;
}

function flexAppliedRecord(targetId, nodeId) {
  return state.flexEditor.applied.get(targetId)?.get(nodeId) || null;
}

function markFlexOverride(targetId, nodeId, properties) {
  let map = state.flexEditor.applied.get(targetId);
  if (!map) {
    map = new Map();
    state.flexEditor.applied.set(targetId, map);
  }
  map.set(nodeId, { ...(map.get(nodeId) || {}), ...properties });
  while (map.size > 512) map.delete(map.keys().next().value);
}

function clearFlexOverride(targetId, nodeId) {
  state.flexEditor.applied.get(targetId)?.delete(nodeId);
}

function hasFlexOverride(node) {
  if (!node) return false;
  if (state.flexEditor.applied.get(state.targetId)?.has(node.id)) return true;
  const overrides = Array.isArray(node.style?.overrides) ? node.style.overrides : [];
  return overrides.some(name => FLEX_PROPERTY_NAMES.includes(name));
}

function flexStale(node) {
  const editor = state.flexEditor;
  if (editor.stale && editor.key === flexEditorKey(node.id)) return true;
  return state.detailError?.code === "stale_node";
}

function flexEditorBlocked(node, layout) {
  return !layout
    || !state.connected
    || !state.caps.runtime
    || !state.caps.style
    || flexStale(node || {})
    || state.flexEditor.busy;
}

function flexStatusFor(node, layout, invalid, dirty) {
  const editor = state.flexEditor;
  if (!layout) return { text: "This node has no layout box.", tone: "" };
  if (!state.caps.style) return { text: "This runtime does not advertise style overrides.", tone: "" };
  if (flexStale(node || {})) {
    const staleError = editor.error || (state.detailError ? errorMessage(state.detailError) : "");
    return { text: staleError || "This node is no longer in the scene. Refresh the tree.", tone: "error" };
  }
  if (editor.busy) return { text: editor.mode === "clear" ? "Resetting…" : "Applying…", tone: "busy" };
  if (editor.error) return { text: editor.error, tone: "error" };
  if (editor.notice) return { text: editor.notice, tone: "warn" };
  if (invalid) return { text: `Flex factors must be between 0 and ${FLEX_FACTOR_LIMIT}.`, tone: "error" };
  if (dirty) return { text: "Unapplied changes", tone: "" };
  return { text: "", tone: "" };
}

function fillFlexStatus(status, node, layout, invalid, dirty) {
  const view = flexStatusFor(node, layout, invalid, dirty);
  status.className = `flex-status${view.tone ? ` is-${view.tone}` : ""}`;
  clear(status);
  if (view.tone === "busy") status.append(el("span", { class: "spinner" }), view.text);
  else status.textContent = view.text;
}

function syncFlexEditor() {
  const form = document.getElementById("flex-editor");
  if (!form) return;
  const node = currentNode();
  const layout = node?.layout ?? null;
  const editor = state.flexEditor;
  const { properties, invalid, dirty } = flexDraftProperties(layout);
  const blocked = flexEditorBlocked(node, layout);
  const apply = document.getElementById("flex-apply");
  const reset = document.getElementById("flex-reset");
  if (apply) apply.disabled = blocked || invalid || !dirty;
  if (reset) reset.disabled = blocked || !hasFlexOverride(node);
  for (const name of FLEX_PROPERTY_NAMES) {
    const field = form.querySelector(`[data-flex-field="${name}"]`);
    if (!field) continue;
    field.classList.toggle("is-dirty", Boolean(properties[name]));
    const input = field.querySelector("input");
    if (input) input.setAttribute("aria-invalid", String(flexDraftFieldInvalid(name, editor.draft?.values?.[name])));
  }
  const status = document.getElementById("flex-status");
  if (status) fillFlexStatus(status, node, layout, invalid, dirty);
}

function flexNumberInput(name, label, layout, draft, disabled) {
  const id = FLEX_FIELD_IDS[name];
  const input = el("input", {
    class: "compact-input",
    attrs: {
      id,
      name,
      type: "number",
      min: "0",
      max: String(FLEX_FACTOR_LIMIT),
      step: "0.1",
      inputmode: "decimal",
      "aria-label": label,
      "aria-invalid": String(flexDraftFieldInvalid(name, draft?.values?.[name])),
    },
    props: { disabled, value: flexFieldText(name, layout, draft) },
    on: {
      input: () => {
        setFlexDraft(name, input.value);
        syncFlexEditor();
      },
    },
  });
  return input;
}

function flexSelect(name, label, options, layout, draft, disabled) {
  const id = FLEX_FIELD_IDS[name];
  const current = flexFieldText(name, layout, draft);
  const values = current && !options.includes(current) ? [...options, current] : options;
  const select = el("select", {
    class: "compact-input",
    attrs: { id, name, "aria-label": label },
    props: { disabled },
    on: {
      change: () => {
        setFlexDraft(name, select.value);
        syncFlexEditor();
      },
    },
  });
  if (!current) select.append(el("option", { props: { value: "" }, text: "—" }));
  for (const value of values) select.append(el("option", { props: { value }, text: value }));
  select.value = current;
  return select;
}

function renderFlexSection(node, layout, contentWidth, contentHeight) {
  const editor = flexEditorFor(node, layout);
  const draft = editor.draft;
  const disabled = flexEditorBlocked(node, layout);
  const { properties, invalid, dirty } = flexDraftProperties(layout);
  const overridden = Boolean(layout) && hasFlexOverride(node);
  const form = el("form", {
    class: "flex-editor",
    props: { id: "flex-editor", noValidate: true },
    on: {
      submit: event => {
        event.preventDefault();
        submitFlexEditor("set");
      },
    },
  });
  const fields = el("div", { class: "flex-fields" },
    el("label", { class: `flex-field${properties.flexGrow !== undefined ? " is-dirty" : ""}`, attrs: { for: FLEX_FIELD_IDS.flexGrow }, dataset: { flexField: "flexGrow" } },
      el("span", { text: "Grow" }),
      flexNumberInput("flexGrow", "Flex grow", layout, draft, disabled)),
    el("label", { class: `flex-field${properties.flexShrink !== undefined ? " is-dirty" : ""}`, attrs: { for: FLEX_FIELD_IDS.flexShrink }, dataset: { flexField: "flexShrink" } },
      el("span", { text: "Shrink" }),
      flexNumberInput("flexShrink", "Flex shrink", layout, draft, disabled)),
    el("label", { class: `flex-field${properties.justifyContent !== undefined ? " is-dirty" : ""}`, attrs: { for: FLEX_FIELD_IDS.justifyContent }, dataset: { flexField: "justifyContent" } },
      el("span", { text: "Justify" }),
      flexSelect("justifyContent", "Justify content", FLEX_JUSTIFY_VALUES, layout, draft, disabled)),
    el("label", { class: `flex-field${properties.alignItems !== undefined ? " is-dirty" : ""}`, attrs: { for: FLEX_FIELD_IDS.alignItems }, dataset: { flexField: "alignItems" } },
      el("span", { text: "Align" }),
      flexSelect("alignItems", "Align items", FLEX_ALIGN_VALUES, layout, draft, disabled)),
  );
  const apply = el("button", {
    class: "tool-button is-primary",
    props: { id: "flex-apply", type: "submit", disabled: disabled || invalid || !dirty },
    text: "Apply",
  });
  const reset = el("button", {
    class: "tool-button",
    props: { id: "flex-reset", type: "button", disabled: disabled || !overridden },
    text: "Reset",
    on: { click: () => submitFlexEditor("clear") },
  });
  const status = el("span", { class: "flex-status", props: { id: "flex-status" }, attrs: { role: "status" } });
  fillFlexStatus(status, node, layout, invalid, dirty);
  form.append(fields, el("div", { class: "flex-actions" }, apply, reset, status));
  return sectionBlock("Flex", form,
    layout ? kvList([
      ["Direction", layout.direction],
      ["Flex direction", layout.flexDirection],
      ["Content size", `${fmtNumber(contentWidth)} × ${fmtNumber(contentHeight)}`],
    ]) : emptyInline("This node did not publish layout metrics, so flex overrides are unavailable."),
  );
}

function flexEditorCurrent(epoch, targetId, request, nodeId) {
  const editor = state.flexEditor;
  return epoch === state.targetEpoch
    && targetId === state.targetId
    && request === editor.request
    && nodeId === state.selectedId
    && editor.key === flexEditorKey(nodeId);
}

function resetFlexEditorState() {
  const editor = state.flexEditor;
  editor.request += 1;
  editor.key = "";
  editor.draft = null;
  editor.busy = false;
  editor.mode = null;
  editor.error = null;
  editor.notice = null;
  editor.stale = false;
}

function sendFlexStyle(targetId, command, args) {
  return rpc("edit_ui_style", { target: targetId, command, arguments: args });
}

async function fetchFlexRecord(targetId, nodeId) {
  const page = unwrapData(await rpc("query_ui_nodes", { target: targetId, node_id: nodeId }));
  const item = Array.isArray(page?.items) ? page.items[0] : null;
  if (!item?.node) throw new RpcError("The runtime returned no record for this node.", "stale_node");
  return item;
}

function adoptFlexRecord(nodeId, item) {
  const entry = state.entries.get(nodeId);
  if (entry && item.node) entry.node = { ...entry.node, ...item.node, children: entry.node.children };
  if (state.selectedId === nodeId) state.detail = item;
  if (state.selectedId === nodeId) state.detailError = null;
}

function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

// Read the node record back until it reflects the request, so the panel shows
// the runtime's values rather than the values the client asked for.
async function readFlexRecord(targetId, nodeId, mode, properties, epoch, request) {
  const editor = state.flexEditor;
  let item = null;
  let mismatch = false;
  for (let attempt = 0; attempt < FLEX_RECORD_ATTEMPTS; attempt += 1) {
    if (attempt > 0) await sleep(FLEX_RECORD_DELAY_MS);
    if (!flexEditorCurrent(epoch, targetId, request, nodeId)) return null;
    try {
      item = await fetchFlexRecord(targetId, nodeId);
    } catch (error) {
      if (!flexEditorCurrent(epoch, targetId, request, nodeId)) return null;
      if (error?.code === "stale_node") {
        editor.stale = true;
        editor.error = errorMessage(error);
        return null;
      }
      throw error;
    }
    if (!flexEditorCurrent(epoch, targetId, request, nodeId)) return null;
    const layout = item.node?.layout ?? null;
    if (!layout) {
      mismatch = true;
      continue;
    }
    if (mode === "set") {
      mismatch = !flexValuesMatch(layout, properties || {});
      if (!mismatch) return { item, mismatch };
    } else {
      const remaining = Array.isArray(item.node?.style?.overrides)
        ? item.node.style.overrides.filter(name => FLEX_PROPERTY_NAMES.includes(name))
        : [];
      const stillApplied = properties ? flexValuesMatch(layout, properties) : false;
      mismatch = remaining.length > 0 || stillApplied;
      if (!mismatch) return { item, mismatch };
    }
  }
  return { item, mismatch };
}

async function submitFlexEditor(mode) {
  const target = targetById();
  const nodeId = state.selectedId;
  const editor = state.flexEditor;
  if (!state.connected || !target || !state.caps.runtime || !state.caps.tree || !nodeId || editor.busy) return;
  const node = currentNode();
  const layout = node?.layout ?? null;
  if (!layout || flexStale(node || {})) return;
  let properties = null;
  if (mode === "set") {
    const draft = flexDraftProperties(layout);
    if (draft.invalid || !draft.dirty) return;
    properties = draft.properties;
  }
  const targetId = target.id;
  const epoch = state.targetEpoch;
  const request = ++editor.request;
  const appliedBefore = mode === "clear" ? flexAppliedRecord(targetId, nodeId) : null;
  editor.busy = true;
  editor.mode = mode;
  editor.error = null;
  editor.notice = null;
  renderDetails();
  try {
    const args = mode === "set" ? { id: nodeId, properties } : { id: nodeId };
    unwrapData(await sendFlexStyle(targetId, mode === "set" ? "inspect.style.set" : "inspect.style.clear", args));
    // The override exists in the runtime now, even if the selection moved on.
    if (mode === "set") markFlexOverride(targetId, nodeId, properties);
    else clearFlexOverride(targetId, nodeId);
    if (!flexEditorCurrent(epoch, targetId, request, nodeId)) return;
    const outcome = await readFlexRecord(targetId, nodeId, mode, mode === "set" ? properties : appliedBefore, epoch, request);
    if (!outcome) return;
    if (outcome.item) {
      adoptFlexRecord(nodeId, outcome.item);
      editor.draft = null;
    }
    if (outcome.mismatch) {
      editor.notice = mode === "set"
        ? "The layout record has not picked up the override yet."
        : "The layout record still reports the override.";
    }
  } catch (error) {
    if (!flexEditorCurrent(epoch, targetId, request, nodeId)) return;
    editor.error = errorMessage(error);
    editor.stale = error?.code === "stale_node";
  } finally {
    if (flexEditorCurrent(epoch, targetId, request, nodeId)) {
      editor.busy = false;
      editor.mode = null;
      renderDetails();
    }
  }
}

function renderLayout(node, detail, entry) {
  const panel = dom.panels.layout;
  clear(panel);
  const layout = node.layout;
  const frame = node.frame;
  const absolute = node.absoluteFrame;
  panel.append(sectionBlock("Frame",
    el("div", { class: "metrics" },
      metric("X", fmtNumber(frame?.x)),
      metric("Y", fmtNumber(frame?.y)),
      metric("Width", fmtNumber(frame?.w)),
      metric("Height", fmtNumber(frame?.h)),
    ),
    kvList([
      ["Absolute frame", absolute ? `${fmtNumber(absolute.x)} × ${fmtNumber(absolute.y)} · ${fmtNumber(absolute.w)} × ${fmtNumber(absolute.h)}` : "—"],
      ["Content offset", node.contentOffset ? `${fmtNumber(node.contentOffset.x)} × ${fmtNumber(node.contentOffset.y)}` : "—"],
      ["Visible frame", detail?.presentation?.visibleFrame
        ? `${fmtNumber(detail.presentation.visibleFrame.x)} × ${fmtNumber(detail.presentation.visibleFrame.y)} · ${fmtNumber(detail.presentation.visibleFrame.w)} × ${fmtNumber(detail.presentation.visibleFrame.h)}`
        : "—"],
      ["Z-index", node.zIndex ?? "—"],
    ]),
  ));
  if (!layout) {
    if (state.detailLoading && !state.detail) {
      panel.append(sectionBlock("Box model", el("div", { class: "inline-spinner" }, el("span", { class: "spinner" }), "Reading layout…")));
      return;
    }
    panel.append(sectionBlock("Box model", emptyInline("This node did not publish layout metrics.")));
    panel.append(renderFlexSection(node, null, null, null));
    if (state.detailError) panel.append(errorInline(errorMessage(state.detailError)));
    return;
  }
  const contentWidth = layout.contentWidth ?? Math.max(0, (frame?.w ?? 0) - (layout.padding?.left ?? 0) - (layout.padding?.right ?? 0) - (layout.border?.left ?? 0) - (layout.border?.right ?? 0));
  const contentHeight = layout.contentHeight ?? Math.max(0, (frame?.h ?? 0) - (layout.padding?.top ?? 0) - (layout.padding?.bottom ?? 0) - (layout.border?.top ?? 0) - (layout.border?.bottom ?? 0));
  panel.append(sectionBlock("Box model",
    el("div", { class: "box-model" },
      el("div", { class: "box-cell box-margin", text: `margin · ${insetText(layout.margin)}` }),
      el("div", { class: "box-cell box-border", text: `border · ${insetText(layout.border)}` }),
      el("div", { class: "box-cell box-padding", text: `padding · ${insetText(layout.padding)}` }),
      el("div", { class: "box-cell box-content", text: `content · ${fmtNumber(contentWidth)} × ${fmtNumber(contentHeight)}` }),
    ),
    el("div", { class: "legend" },
      el("span", { class: "padding", text: "padding" }),
      el("span", { class: "border", text: "border" }),
      el("span", { class: "content", text: "content" }),
    ),
    el("table", { class: "data-table", attrs: { style: "margin-top:8px" } },
      el("thead", {}, el("tr", {},
        el("th", { text: "Inset" }), el("th", { class: "num", text: "Top" }), el("th", { class: "num", text: "Right" }),
        el("th", { class: "num", text: "Bottom" }), el("th", { class: "num", text: "Left" }),
      )),
      el("tbody", {},
        ["margin", "border", "padding"].map(kind => el("tr", {},
          el("td", { text: kind }),
          el("td", { class: "num", text: fmtNumber(layout[kind]?.top) }),
          el("td", { class: "num", text: fmtNumber(layout[kind]?.right) }),
          el("td", { class: "num", text: fmtNumber(layout[kind]?.bottom) }),
          el("td", { class: "num", text: fmtNumber(layout[kind]?.left) }),
        )),
      ),
    ),
  ));
  panel.append(renderFlexSection(node, layout, contentWidth, contentHeight));
  if (state.detailError) panel.append(errorInline(errorMessage(state.detailError)));
}

function renderSource(node, entry) {
  const panel = dom.panels.source;
  clear(panel);
  const source = node.source;
  if (!source) {
    panel.append(sectionBlock("Source", emptyInline("This node did not publish a source location. Rebuild the app with source locations enabled.")));
    return;
  }
  const path = source.filePath || source.fileID || "";
  const location = `${path}:${source.line ?? "?"}:${source.column ?? "?"}`;
  panel.append(sectionBlock("Source",
    el("div", { class: "source-card" },
      el("div", { class: "source-path", text: path || "(no file path)" }),
      el("div", { class: "source-location" },
        el("span", { class: "pill", text: `line ${source.line ?? "?"}` }),
        el("span", { class: "pill", text: `column ${source.column ?? "?"}` }),
      ),
      el("div", { class: "source-actions" },
        el("button", { class: "tool-button", props: { type: "button" }, text: "Copy path", on: { click: () => copyText(location, "Source location") } }),
        el("button", { class: "tool-button", props: { type: "button" }, text: "Copy file ID", on: { click: () => copyText(source.fileID, "File ID") } }),
      ),
    ),
    kvList([
      ["File ID", el("code", { text: source.fileID || "—" })],
      ["Line", String(source.line ?? "—")],
      ["Column", String(source.column ?? "—")],
    ]),
    rawDetails("Raw source record", source),
  ));
}

function setDetailTab(name, focus = false) {
  if (!DETAIL_TABS.includes(name)) return;
  state.activeDetailTab = name;
  for (const tab of DETAIL_TABS) {
    const button = $(`tab-${tab}`);
    const panel = dom.panels[tab];
    const selected = tab === name;
    button.setAttribute("aria-selected", String(selected));
    button.tabIndex = selected ? 0 : -1;
    panel.hidden = !selected;
    if (selected && focus) button.focus();
  }
}

function renderDetailTab() {
  setDetailTab(state.activeDetailTab);
}

function updateNodeButtons() {
  dom.nodeShotButton.disabled = !(state.caps.inspectNodes && state.caps.capture) || !state.selectedId || state.captureLoading;
  dom.highlightButton.disabled = !state.selectedId || !state.caps.runtime;
  dom.clearHighlight.disabled = !state.caps.runtime;
  dom.readValueButton.disabled = !state.selectedId || state.detailLoading;
}

async function copyText(value, label = "Value") {
  const text = String(value ?? "");
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
    } else {
      const area = document.createElement("textarea");
      area.value = text;
      area.style.position = "fixed";
      area.style.opacity = "0";
      document.body.append(area);
      area.select();
      document.execCommand("copy");
      area.remove();
    }
    toast(`${label} copied.`, "success", 3000);
  } catch {
    toast(`Could not copy ${label.toLowerCase()}.`, "error");
  }
}

/* Screenshot and pick-at-point */

function releaseCapture() {
  if (state.capture?.url) URL.revokeObjectURL(state.capture.url);
  state.capture = null;
  state.captureError = null;
  state.captureLoading = false;
  state.pickBusy = false;
}

async function captureWindow() {
  if (state.captureLoading || state.pickBusy) return;
  if (!state.caps.capture || !state.targetId) {
    toast("This target does not advertise capture.", "error");
    return;
  }
  const targetId = state.targetId;
  const epoch = state.targetEpoch;
  state.captureAttempt = "window";
  state.captureLoading = true;
  state.captureError = null;
  updateNodeButtons();
  renderScreenshot();
  try {
    const result = await rpc("capture_ui", { target: targetId });
    if (epoch !== state.targetEpoch || targetId !== state.targetId) return;
    await applyCaptureResult(result, "window", null, targetId, epoch);
  } catch (error) {
    if (epoch !== state.targetEpoch || targetId !== state.targetId) return;
    state.captureError = error;
    toast(errorMessage(error), "error", 9000);
  } finally {
    if (epoch === state.targetEpoch && targetId === state.targetId) {
      state.captureLoading = false;
      updateNodeButtons();
      renderScreenshot();
    }
  }
}

async function captureNode() {
  if (state.captureLoading || state.pickBusy) return;
  if (!state.caps.inspectNodes) {
    toast("This runtime does not advertise inspect_nodes.", "error", 8000);
    return;
  }
  if (!state.caps.capture) {
    toast("This runtime does not advertise frame capture.", "error", 8000);
    return;
  }
  if (!state.selectedId) {
    toast("Select a component before taking a node screenshot.", "info");
    return;
  }
  const targetId = state.targetId;
  const epoch = state.targetEpoch;
  const nodeId = state.selectedId;
  state.captureAttempt = "node";
  state.captureLoading = true;
  state.captureError = null;
  updateNodeButtons();
  renderScreenshot();
  try {
    const result = await rpc("capture_ui_node", { target: targetId, node_id: nodeId });
    if (epoch !== state.targetEpoch || targetId !== state.targetId) return;
    await applyCaptureResult(result, "node", result?.node_id || result?.nodeID || nodeId, targetId, epoch);
  } catch (error) {
    if (epoch !== state.targetEpoch || targetId !== state.targetId) return;
    state.captureError = error;
    toast(errorMessage(error), "error", 9000);
  } finally {
    if (epoch === state.targetEpoch && targetId === state.targetId) {
      state.captureLoading = false;
      updateNodeButtons();
      renderScreenshot();
    }
  }
}

async function applyCaptureResult(result, kind, nodeId, targetId, epoch) {
  if (!result || typeof result !== "object") throw new RpcError("The controller returned no capture.", "capture_response");
  const resource = typeof result.resource === "string" ? result.resource : "";
  if (!resource) throw new RpcError("The controller returned no capture resource.", "capture_response");
  let response;
  try {
    response = await fetch(resource, { headers: authHeaders() });
  } catch {
    throw new RpcError("Could not load the capture image from the controller.", "capture_resource");
  }
  if (!response.ok) throw new RpcError(`The capture resource request failed (HTTP ${response.status}).`, "capture_resource");
  const blob = await response.blob();
  if (!blob.size) throw new RpcError("The controller returned an empty capture image.", "capture_resource");
  if (epoch !== state.targetEpoch || targetId !== state.targetId) return false;
  if (state.capture?.url) URL.revokeObjectURL(state.capture.url);
  state.capture = {
    ...result,
    url: URL.createObjectURL(blob),
    kind,
    nodeId: nodeId || result.node_id || result.nodeID || null,
    contentFrame: result.contentFrame || result.content_frame || null,
    capturedAt: Date.now(),
    zoom: "fit",
  };
  state.captureError = null;
  state.captureLoading = false;
  updateNodeButtons();
  renderScreenshot();
  return true;
}

function captureDimensions(capture) {
  const width = numberValue(capture?.width) ?? numberValue(capture?.naturalWidth) ?? 0;
  const height = numberValue(capture?.height) ?? numberValue(capture?.naturalHeight) ?? 0;
  return { width, height };
}

function renderScreenshot() {
  const body = dom.screenshotBody;
  body.replaceChildren();
  body.classList.toggle("is-pick", state.mode === "pick" && state.capture?.kind === "window");
  body.classList.toggle("is-interact", state.mode === "interact" && state.capture?.kind === "window");
  body.classList.toggle("is-zoom", state.capture?.zoom === "actual");
  body.classList.toggle("is-busy", state.captureLoading || state.pickBusy);
  dom.screenshotZoom.hidden = !state.capture;
  dom.screenshotClear.hidden = !state.capture;
  dom.saveButton.disabled = !state.capture;
  dom.captureButton.disabled = !state.caps.capture || state.captureLoading;
  dom.nodeShotButton.disabled = !(state.caps.inspectNodes && state.caps.capture) || !state.selectedId || state.captureLoading;
  dom.screenshotZoom.textContent = state.capture?.zoom === "actual" ? "Fit" : "100%";

  if (state.captureLoading && !state.capture) {
    dom.screenshotMeta.textContent = state.captureAttempt === "node" ? "Capturing node…" : "Capturing window…";
    showState(body, { kind: "loading", title: "Capturing", compact: true });
    return;
  }
  if (state.captureError) {
    showState(body, {
      kind: "error",
      title: "Capture failed",
      message: errorMessage(state.captureError),
      compact: true,
      action: "Retry",
      onAction: retryCapture,
    });
    if (!state.capture) dom.screenshotMeta.textContent = "Error";
    return;
  }
  if (!state.capture) {
    const caps = state.caps;
    let message = "Capture the window to inspect it visually and pick components.";
    if (!caps.capture) message = "Select a runtime with frame capture.";
    showState(body, {
      icon: "camera",
      title: "No screenshot",
      message,
      compact: true,
      action: caps.capture ? "Capture" : "",
      onAction: caps.capture ? captureWindow : null,
    });
    dom.screenshotMeta.textContent = "";
    return;
  }

  const capture = state.capture;
  const image = document.createElement("img");
  image.src = capture.url;
  image.draggable = false;
  image.alt = capture.kind === "node"
    ? `Screenshot of ${nodeLabel(currentNode())}`
    : `Screenshot of ${targetById()?.title || "target"}`;
  image.addEventListener("load", () => {
    const width = numberValue(capture.width) ?? image.naturalWidth;
    const height = numberValue(capture.height) ?? image.naturalHeight;
    if (capture.width !== width || capture.height !== height) {
      capture.width = width;
      capture.height = height;
      updateScreenshotMeta();
    }
  });
  if (state.mode === "pick" && capture.kind === "window") {
    image.addEventListener("click", event => handlePickClick(event, image));
    image.title = "Click to select the component at this point";
  }
  if (state.mode === "interact" && capture.kind === "window") bindFrameInput(image, capture);
  body.append(image);
  updateScreenshotMeta();
}

function updateScreenshotMeta() {
  const capture = state.capture;
  if (!capture) {
    dom.screenshotMeta.textContent = "";
    return;
  }
  const { width, height } = captureDimensions(capture);
  const pieces = [];
  pieces.push(capture.kind === "node" ? "Node" : "Window");
  if (width && height) pieces.push(`${width}×${height}`);
  if (capture.source) pieces.push(String(capture.source));
  if (capture.kind === "node" && capture.nodeId) pieces.push(`node ${shortId(capture.nodeId)}`);
  if (capture.capture_id || capture.captureId) pieces.push(shortId(capture.capture_id || capture.captureId));
  dom.screenshotMeta.textContent = pieces.join(" · ");
  dom.screenshotMeta.title = pieces.join(" · ");
}

function handlePickClick(event, image) {
  if (state.mode !== "pick" || state.pickBusy || !state.capture) return;
  const capture = state.capture;
  if (capture.kind !== "window") return;
  const point = imagePoint(event, image, capture);
  if (point && capture.capture_id) pickAt(point.x, point.y, capture.capture_id);
}

function imagePoint(event, image, capture) {
  const rect = image.getBoundingClientRect();
  const { width, height } = captureDimensions(capture);
  if (!rect.width || !rect.height || !width || !height) return null;
  const x = (event.clientX - rect.left) / rect.width;
  const y = (event.clientY - rect.top) / rect.height;
  if (x < 0 || x >= 1 || y < 0 || y >= 1) return null;
  return { x: Math.floor(x * width), y: Math.floor(y * height) };
}

function bindFrameInput(image, capture) {
  image.draggable = false;
  let press = null;
  image.addEventListener("pointerdown", event => {
    if (state.inputBusy || event.button > 2 || !event.isPrimary) return;
    const point = imagePoint(event, image, capture);
    if (!point) return;
    event.preventDefault();
    image.setPointerCapture(event.pointerId);
    press = { point, button: event.button, id: event.pointerId };
  });
  image.addEventListener("pointercancel", () => { press = null; });
  image.addEventListener("pointerup", event => {
    const start = press;
    press = null;
    if (!start || start.id !== event.pointerId) return;
    const end = imagePoint(event, image, capture);
    if (!end) return;
    const moved = Math.hypot(end.x - start.point.x, end.y - start.point.y) > 4;
    if (moved && start.button === 0) {
      sendFrameInput(capture, { kind: "drag", ...start.point, to_x: end.x, to_y: end.y });
    } else if (!moved) {
      sendFrameInput(capture, { kind: "click", ...end, button: start.button });
    }
  });
  image.addEventListener("contextmenu", event => event.preventDefault());
  let scroll;
  let timer;
  image.addEventListener("wheel", event => {
    const point = imagePoint(event, image, capture);
    if (!point || state.inputBusy) return;
    event.preventDefault();
    const divisor = event.deltaMode === 0 ? 40 : event.deltaMode === 2 ? 0.1 : 1;
    scroll ??= { kind: "scroll", ...point, dx: 0, dy: 0 };
    scroll.dx += event.deltaX / divisor;
    scroll.dy += event.deltaY / divisor;
    clearTimeout(timer);
    timer = setTimeout(() => {
      const action = scroll;
      scroll = null;
      const delta = value => Math.max(-10000, Math.min(10000, Math.round(value) || Math.sign(value)));
      sendFrameInput(capture, { ...action, dx: delta(action.dx), dy: delta(action.dy) });
    }, 100);
  }, { passive: false });
}

async function sendFrameInput(capture, action) {
  if (state.mode !== "interact" || state.capture !== capture || state.inputBusy) return;
  const target = state.targetId;
  const epoch = state.targetEpoch;
  setInputBusy(true);
  try {
    await rpc("input_ui", { target, capture_id: capture.capture_id, action });
    if (epoch === state.targetEpoch) await captureWindow();
  } catch (error) {
    if (epoch === state.targetEpoch) toast(errorMessage(error), "error");
  } finally {
    if (epoch === state.targetEpoch) setInputBusy(false);
  }
}

async function pickAt(x, y, captureId) {
  const targetId = state.targetId;
  const epoch = state.targetEpoch;
  state.pickBusy = true;
  renderScreenshot();
  try {
    const result = await rpc("select_ui_at", { target: targetId, capture_id: captureId, x, y });
    if (epoch !== state.targetEpoch || targetId !== state.targetId) return;
    if (result?.capture_id && state.capture) state.capture.capture_id = result.capture_id;
    const hit = result?.hit || null;
    const nodeId = hit?.nodeID || hit?.nodeId || hit?.node_id || null;
    if (!nodeId) {
      state.selectedId = null;
      state.detail = null;
      state.detailError = null;
      state.detailRequest += 1;
      clearTimeout(state.detailTimer);
      if (state.capture?.kind === "node") releaseCapture();
      renderTreeArea();
      renderDetails();
      updateNodeButtons();
      renderScreenshot();
      toast("No component was hit at that point.", "info");
      return;
    }
    if (!state.entries.has(nodeId)) {
      await loadTree({ preserveExpansion: true });
      if (epoch !== state.targetEpoch || targetId !== state.targetId) return;
    }
    if (!state.entries.has(nodeId)) {
      toast("The runtime hit a node that is not in the current tree. Refresh and try again.", "error", 8000);
      return;
    }
    for (const pathId of Array.isArray(hit.path) ? hit.path : []) {
      if (state.entries.has(pathId)) state.expanded.add(pathId);
    }
    state.mode = "pick";
    updateTargetUi();
    if (state.selectedId === nodeId) {
      highlightNode(nodeId);
      loadNodeDetail(nodeId);
      renderDetails();
    } else {
      selectNode(nodeId, { reveal: true, reason: "pick" });
    }
    const contentPoint = result.contentPoint || result.content_point;
    const pointText = contentPoint && numberValue(contentPoint.x) !== null
      ? ` at content ${fmtNumber(contentPoint.x)}×${fmtNumber(contentPoint.y)}`
      : "";
    toast(`Picked ${nodeLabel(currentNode())}${pointText}.`, "success", 4000);
  } catch (error) {
    if (epoch !== state.targetEpoch || targetId !== state.targetId) return;
    if (error?.code === "stale_capture") {
      releaseCapture();
      renderScreenshot();
    }
    toast(`Pick failed: ${errorMessage(error)}`, "error", 9000);
  } finally {
    if (epoch === state.targetEpoch && targetId === state.targetId) {
      state.pickBusy = false;
      renderScreenshot();
    }
  }
}

function saveCapture() {
  const capture = state.capture;
  if (!capture?.url) return;
  const target = targetById();
  const node = currentNode();
  const kind = capture.kind === "node" && node ? `node-${slug(nodeLabel(node))}` : "window";
  const stamp = new Date().toISOString().replace(/[:.]/g, "-");
  const name = `cove-${slug(target?.title || "target")}-${kind}-${stamp}.png`;
  const link = document.createElement("a");
  link.href = capture.url;
  link.download = name;
  document.body.append(link);
  link.click();
  link.remove();
  toast(`Saved ${name}.`, "success", 4000);
}

function retryCapture() {
  if (isDisconnectedError(state.captureError)) refreshCurrent();
  else if (state.captureAttempt === "node") captureNode();
  else captureWindow();
}

function toggleScreenshotZoom() {
  if (!state.capture) return;
  state.capture.zoom = state.capture.zoom === "actual" ? "fit" : "actual";
  renderScreenshot();
}

function clearScreenshot() {
  releaseCapture();
  updateTargetUi();
  renderScreenshot();
}

function enterPickMode() {
  if (!state.caps.inspectNodes) {
    state.mode = "select";
    updateModeUi();
    toast("This runtime does not advertise inspect_nodes.", "error", 8000);
    return;
  }
  if (!state.caps.capture) {
    state.mode = "select";
    updateModeUi();
    toast("This target does not advertise capture; pick-at-point needs a screenshot.", "error", 8000);
    return;
  }
  state.mode = "pick";
  updateModeUi();
  if (!state.capture || state.capture.kind !== "window") {
    captureWindow();
  } else {
    renderScreenshot();
  }
  dom.pickHintText.textContent = "Click a point in the screenshot. The runtime resolves it to a component through a real hit test.";
}

function exitPickMode() {
  state.mode = "select";
  updateModeUi();
  renderScreenshot();
  renderDetails();
}

function updateModeUi() {
  dom.modeSelect.setAttribute("aria-pressed", String(state.mode === "select"));
  dom.modePick.setAttribute("aria-pressed", String(state.mode === "pick"));
  dom.modePick.disabled = !(state.caps.inspectNodes && state.caps.capture);
  dom.modeInteract.disabled = !(state.caps.input && state.caps.capture);
  dom.modeInteract.setAttribute("aria-pressed", String(state.mode === "interact"));
  dom.pickHint.hidden = state.mode !== "pick";
  dom.screenshotBody.classList.toggle("is-pick", state.mode === "pick" && state.capture?.kind === "window");
  if (!state.selectedId && state.mode === "pick") renderDetails();
}

/* Keyboard and text input */

function renderInputUi() {
  const caps = state.caps;
  const target = targetById();
  const disabled = !caps.input || state.inputBusy;
  dom.keyForm.querySelectorAll("input, button").forEach(control => { control.disabled = disabled; });
  dom.textForm.querySelectorAll("input, button").forEach(control => { control.disabled = disabled; });
  dom.inputMeta.textContent = caps.input
    ? `App input · ${target?.title || "target"}`
    : "This target does not advertise input.";
  updateInputButtons();
}

function updateInputButtons() {
  const disabled = !state.caps.input || state.inputBusy;
  dom.sendKey.disabled = disabled || !dom.keyValue.value.trim();
  dom.sendText.disabled = disabled || !dom.textValue.value.trim();
}

function setInputBusy(busy) {
  state.inputBusy = busy;
  renderInputUi();
}

async function sendKey(event) {
  event?.preventDefault();
  if (!state.caps.input || state.inputBusy) return;
  const key = dom.keyValue.value.trim();
  if (!key) return;
  const modifiers = [...dom.keyForm.querySelectorAll('input[name="modifier"]:checked')].map(input => input.value);
  const targetId = state.targetId;
  setInputBusy(true);
  try {
    const result = await rpc("press_ui_key", { target: targetId, key, modifiers });
    if (targetId !== state.targetId) return;
    toast(`Key delivered${result?.source ? ` via ${result.source}` : ""}. Verify the resulting UI state.`, "success", 5000);
  } catch (error) {
    toast(errorMessage(error), "error", 9000);
  } finally {
    setInputBusy(false);
  }
}

async function sendText(event) {
  event?.preventDefault();
  if (!state.caps.input || state.inputBusy) return;
  const text = dom.textValue.value;
  if (!text) return;
  const targetId = state.targetId;
  setInputBusy(true);
  try {
    const result = await rpc("type_ui_text", { target: targetId, text });
    if (targetId !== state.targetId) return;
    toast(`Text delivered${result?.source ? ` via ${result.source}` : ""}. Verify the resulting UI state.`, "success", 5000);
  } catch (error) {
    toast(errorMessage(error), "error", 9000);
  } finally {
    setInputBusy(false);
  }
}

function toggleInputDrawer() {
  const expanded = dom.inputToggle.getAttribute("aria-expanded") === "true";
  dom.inputToggle.setAttribute("aria-expanded", String(!expanded));
  dom.inputToggle.textContent = expanded ? "Show" : "Hide";
  dom.inputBody.hidden = expanded;
}

/* Network */

function stopTimer(key) {
  const timer = state[key]?.timer;
  if (timer) clearInterval(timer);
  if (state[key]) state[key].timer = null;
}

function stopAllFollowTimers() {
  stopTimer("network");
  stopTimer("logs");
  stopTimer("performance");
  stopTimer("debugger");
  stopTimer("memory");
  stopTimer("registeredState");
}

function syncFollowTimers() {
  stopAllFollowTimers();
  if (document.hidden || !state.connected || !state.targetId) return;
  if (state.view === "network" && state.network.follow && state.caps.network) {
    state.network.timer = setInterval(() => loadNetwork({ quiet: true }), 2000);
  }
  if (state.view === "logs" && state.logs.follow && state.caps.logs) {
    state.logs.timer = setInterval(() => loadLogs({ quiet: true }), 2000);
  }
  if (state.view === "performance" && state.performance.tab === "frames" && state.performance.follow && state.caps.timing && !state.performance.snapshot) {
    state.performance.timer = setInterval(() => loadFrames({ quiet: true }), 1000);
  }
  if (state.view === "memory" && state.memory.follow && state.caps.runtime) {
    state.memory.timer = setInterval(() => loadMemory({ quiet: true }), state.memory.intervalMs);
  }
  if (state.view === "state" && state.registeredState.follow && state.caps.state) {
    state.registeredState.timer = setInterval(() => loadRegisteredState(state.registeredState.offset, { quiet: true }), 2000);
  }
  syncDebuggerTimer();
}

function syncDebuggerTimer() {
  stopTimer("debugger");
  if (document.hidden || !state.connected || !state.token || !state.targetId || state.view !== "debugger") return;
  const dbg = state.debugger;
  if (!dbg.attached || dbg.unavailable || !state.caps.runtime) return;
  dbg.timer = setInterval(() => refreshDebugger({ quiet: true, poll: true }), DEBUGGER_POLL_MS);
}

function resetNetwork() {
  stopTimer("network");
  state.network = {
    loaded: false, loading: false, error: null, records: [], limit: 0, follow: dom.networkFollow.checked, timer: null,
    recording: null, recordingPending: false, recordingRequest: null, clearPending: false,
    controlError: null, controlUnavailable: false, cleared: false, generation: 0, recordingEpoch: 0,
    readAt: 0, waterfall: true,
  };
  dom.networkMeta.textContent = "Recorded in-process HTTP activity.";
  dom.networkBadge.hidden = true;
  if (state.view === "network") renderNetwork();
}

function ensureNetwork() {
  if (!state.token || !state.targetId) {
    renderNetwork();
    return;
  }
  if (!state.caps.network) {
    renderNetwork();
    return;
  }
  if (!state.network.loaded && !state.network.loading) loadNetwork();
  else renderNetwork();
}

async function loadNetwork(options = {}) {
  const quiet = options.quiet === true;
  const net = state.network;
  if (!state.caps.network || !state.targetId || net.loading || net.clearPending) return;
  const targetId = state.targetId;
  const epoch = state.targetEpoch;
  const generation = net.generation;
  const recordingEpoch = net.recordingEpoch;
  net.loading = true;
  net.error = null;
  renderNetwork();
  try {
    const result = await rpc("query_ui_network", { target: targetId });
    if (epoch !== state.targetEpoch || targetId !== state.targetId || generation !== net.generation) return;
    const data = unwrapData(result);
    const records = Array.isArray(data?.records) ? data.records : [];
    net.records = records;
    net.limit = numberValue(data?.limit) ?? 0;
    net.loaded = true;
    net.error = null;
    // The wall clock at which the runtime's snapshot was read. A pending
    // request has no recorded end, so this is the only honest right edge for
    // its bar: the request was still in flight at this instant.
    net.readAt = Date.now();
    if (records.length) net.cleared = false;
    // A read that started before a control call carries the pre-call state; it
    // may merge records but must not overwrite the confirmed recording state.
    if (!net.recordingPending && recordingEpoch === net.recordingEpoch) {
      const recording = booleanOf(data?.recording) ?? booleanOf(data?.enabled);
      if (recording !== null) net.recording = recording;
    }
    dom.networkBadge.hidden = records.length === 0;
    dom.networkBadge.textContent = String(records.length);
  } catch (error) {
    if (epoch !== state.targetEpoch || targetId !== state.targetId || generation !== net.generation) return;
    net.error = error;
    if (!quiet) toast(errorMessage(error), "error", 8000);
  } finally {
    if (epoch === state.targetEpoch && targetId === state.targetId && generation === net.generation) {
      net.loading = false;
      renderNetwork();
    }
  }
}

function booleanOf(value) {
  return typeof value === "boolean" ? value : null;
}

function networkControlUnavailable(error) {
  if (!error) return false;
  if (["unknown_command", "unknown_tool", "method_not_found", "unsupported", "not_supported", "not-supported"].includes(error.code)) return true;
  return /does not (?:advertise|expose|support)|not supported|unsupported/i.test(error.message || "");
}

function networkControlsAvailable() {
  return Boolean(state.targetId && state.caps.network && state.caps.networkControl && !state.network.controlUnavailable);
}

async function setNetworkRecording(enabled) {
  const net = state.network;
  if (!networkControlsAvailable() || !state.token || !state.connected) return;
  if (net.recordingPending || net.clearPending) return;
  if (net.recording === enabled) return;
  const targetId = state.targetId;
  const epoch = state.targetEpoch;
  net.recordingPending = true;
  net.recordingRequest = enabled;
  net.controlError = null;
  renderNetwork();
  try {
    const data = unwrapData(await rpc("set_ui_network_recording", { target: targetId, enabled }));
    if (epoch !== state.targetEpoch || targetId !== state.targetId || state.network !== net) return;
    // The reply carries the state the recorder actually holds; never assume the
    // requested value won.
    const confirmed = booleanOf(data?.enabled);
    net.recording = confirmed === null ? enabled : confirmed;
    net.recordingEpoch += 1;
    net.controlError = null;
    toast(net.recording ? "Network recording resumed." : "Network recording paused. Existing requests are retained.", "success", 4000);
  } catch (error) {
    if (epoch !== state.targetEpoch || targetId !== state.targetId || state.network !== net) return;
    net.controlError = error;
    if (networkControlUnavailable(error)) net.controlUnavailable = true;
    toast(`${enabled ? "Resume" : "Pause"} failed: ${errorMessage(error)}`, "error", 9000);
  } finally {
    if (epoch === state.targetEpoch && targetId === state.targetId && state.network === net) {
      net.recordingPending = false;
      net.recordingRequest = null;
      renderNetwork();
    }
  }
}

async function clearNetwork() {
  const net = state.network;
  if (!networkControlsAvailable() || !state.token || !state.connected) return;
  if (net.clearPending || net.recordingPending) return;
  const targetId = state.targetId;
  const epoch = state.targetEpoch;
  // Reads that started before the clear describe the retired window, so they
  // must not merge records back into the cleared table.
  net.generation += 1;
  net.clearPending = true;
  net.loading = false;
  net.clearError = null;
  net.error = null;
  net.controlError = null;
  renderNetwork();
  let cleared = false;
  try {
    const data = unwrapData(await rpc("clear_ui_network", { target: targetId }));
    if (epoch !== state.targetEpoch || targetId !== state.targetId || state.network !== net) return;
    const count = numberValue(data?.cleared);
    net.records = [];
    net.limit = numberValue(data?.limit) ?? net.limit;
    net.loaded = true;
    net.error = null;
    net.clearError = null;
    net.cleared = true;
    dom.networkBadge.hidden = true;
    dom.networkBadge.textContent = "0";
    cleared = true;
    toast(count !== null && count > 0
      ? `${count} request${count === 1 ? "" : "s"} cleared.`
      : "Network history cleared.", "success", 4000);
  } catch (error) {
    if (epoch !== state.targetEpoch || targetId !== state.targetId || state.network !== net) return;
    net.clearError = error;
    if (networkControlUnavailable(error)) net.controlUnavailable = true;
    net.loaded = true;
    toast(`Clear failed: ${errorMessage(error)}`, "error", 9000);
  } finally {
    if (epoch === state.targetEpoch && targetId === state.targetId && state.network === net) {
      net.clearPending = false;
      renderNetwork();
      if (cleared && net.follow) loadNetwork({ quiet: true });
    }
  }
}

function setNetworkChrome() {
  const net = state.network;
  const target = targetById();
  const offline = !state.token || !state.connected;
  const unavailable = !target || !state.caps.network || !state.caps.networkControl || net.controlUnavailable;
  const pending = net.recordingPending || net.clearPending;
  dom.networkRecordingToggle.disabled = offline || unavailable || pending;
  dom.networkClear.disabled = offline || unavailable || pending;
  dom.networkRefresh.disabled = pending;
  dom.networkRecordingToggle.setAttribute("aria-busy", String(net.recordingPending));
  dom.networkClear.setAttribute("aria-busy", String(net.clearPending));
  dom.networkContent.setAttribute("aria-busy", String(net.clearPending));
  let recordingTitle;
  if (offline) recordingTitle = "Reconnect to the controller to control network recording";
  else if (!target) recordingTitle = "Select a connected target to control network recording";
  else if (!state.caps.network) recordingTitle = "Network records come from a Cove runtime. Select a runtime target";
  else if (!state.caps.networkControl || net.controlUnavailable) recordingTitle = "This runtime does not expose network recording controls";
  else if (net.recordingPending) recordingTitle = net.recordingRequest ? "Resuming network recording" : "Pausing network recording";
  else recordingTitle = net.recording === false ? "Resume recording new HTTP requests" : "Pause recording new HTTP requests";
  dom.networkRecordingToggle.title = recordingTitle;
  const paused = net.recording === false;
  const recordingLabel = net.recordingPending
    ? (net.recordingRequest ? "Resuming…" : "Pausing…")
    : (paused ? "Resume" : "Pause");
  const label = dom.networkRecordingToggle.querySelector("span");
  if (label) label.textContent = recordingLabel;
  const use = dom.networkRecordingToggle.querySelector("use");
  if (use) use.setAttribute("href", paused ? "#i-play" : "#i-pause");
  let clearTitle;
  if (offline) clearTitle = "Reconnect to the controller to clear network records";
  else if (!target) clearTitle = "Select a connected target to clear network records";
  else if (!state.caps.network) clearTitle = "Network records come from a Cove runtime. Select a runtime target";
  else if (!state.caps.networkControl || net.controlUnavailable) clearTitle = "This runtime does not expose network recording controls";
  else if (net.clearPending) clearTitle = "Clearing the runtime's retained network records";
  else clearTitle = "Clear the runtime's retained network records";
  dom.networkClear.title = clearTitle;
  const clearLabel = dom.networkClear.querySelector("span");
  if (clearLabel) clearLabel.textContent = net.clearPending ? "Clearing…" : "Clear";
}

function networkMatches(record) {
  const query = dom.networkSearch.value.trim().toLowerCase();
  const stateFilter = dom.networkStateFilter.value;
  if (stateFilter !== "all" && record.state !== stateFilter) return false;
  if (!query) return true;
  return [record.method, record.url, record.status, record.state, record.error]
    .filter(value => value !== undefined && value !== null)
    .join(" ").toLowerCase().includes(query);
}

function networkWaterfallRows(records, readAt) {
  const rows = [];
  let unplaced = 0;
  for (const record of records) {
    const start = numberValue(record.startedAt);
    if (start === null) {
      unplaced += 1;
      continue;
    }
    const ttfb = numberValue(record.ttfbMs);
    const duration = numberValue(record.durationMs);
    const pending = record.state === "pending";
    // A pending request has no recorded end. The read instant is the only
    // honest right edge: it was still in flight then, so its duration is at
    // least this much, never an invented completion.
    const elapsed = pending
      ? Math.max(0, readAt - start, ttfb ?? 0)
      : Math.max(0, duration ?? 0, ttfb ?? 0);
    rows.push({ record, start, ttfb, duration, pending, elapsed, end: start + elapsed });
  }
  rows.sort((left, right) => left.start - right.start || Number(left.record.id ?? 0) - Number(right.record.id ?? 0));
  return { rows, unplaced };
}

function renderNetworkWaterfall(records) {
  const net = state.network;
  const readAt = net.readAt || Date.now();
  const { rows, unplaced } = networkWaterfallRows(records, readAt);
  if (!rows.length) {
    return el("div", { class: "panel-block" },
      el("div", { class: "panel-block-head" }, el("h2", { text: "Waterfall" })),
      el("div", { class: "panel-block-body" },
        emptyInline("No request carries a start timestamp, so nothing can be placed on a time axis."),
      ),
    );
  }
  const origin = rows[0].start;
  const latest = rows.reduce((maximum, row) => Math.max(maximum, row.end), origin);
  const span = Math.max(latest - origin, 1);
  const percent = value => Math.max(0, Math.min(100, (value / span) * 100));
  const list = el("div", {
    class: "waterfall-rows",
    attrs: {
      role: "listbox",
      "aria-label": `Request waterfall for ${rows.length} requests over ${fmtMs(span, 1)}.`,
    },
  });
  for (const [index, row] of rows.entries()) {
    const { record, start, ttfb, pending, elapsed } = row;
    const offset = start - origin;
    const status = numberValue(record.status);
    const failed = record.state === "failed" || (status !== null && status >= 400);
    const segments = [];
    const head = ttfb === null ? null : Math.min(ttfb, elapsed);
    if (head !== null && head > 0) {
      segments.push({ kind: "waiting", from: 0, to: head, label: `Waiting for response head ${fmtMs(head)}` });
    }
    if (pending) {
      segments.push({
        kind: "inflight",
        from: head ?? 0,
        to: elapsed,
        label: head === null
          ? `In flight ${fmtMs(elapsed)}; the response head had not arrived when the runtime was read`
          : `Receiving body, still in flight after ${fmtMs(elapsed)}`,
      });
    } else if (elapsed > (head ?? 0)) {
      segments.push({
        kind: head === null ? "unsplit" : "receiving",
        from: head ?? 0,
        to: elapsed,
        label: head === null
          ? `Total ${fmtMs(elapsed)}; the recorder did not capture a response-head arrival, so no wait/receive split is drawn`
          : `Receiving body ${fmtMs(elapsed - head)}`,
      });
    }
    if (!segments.length) {
      segments.push({ kind: failed ? "failed" : "receiving", from: 0, to: elapsed, label: `Total ${fmtMs(elapsed)}` });
    }
    const describe = `${textOf(record.method, "?")} ${textOf(record.url, "")}: started ${fmtEpochMs(start)}, `
      + `${pending ? "in flight" : textOf(record.state, "unknown")}, `
      + `${ttfb === null ? "head arrival not recorded" : `head ${fmtMs(ttfb)}`}, `
      + `${pending ? `at least ${fmtMs(elapsed)}` : `total ${fmtMs(elapsed)}`}`
      + `${failed ? ", failed" : ""}`;
    const bar = el("div", {
      class: `waterfall-bar${failed ? " is-failed" : ""}${pending ? " is-pending" : ""}`,
      attrs: { style: `left:${percent(offset)}%;width:${percent(elapsed)}%` },
    });
    for (const segment of segments) {
      const width = segment.to - segment.from;
      bar.append(el("span", {
        class: `waterfall-seg is-${segment.kind}`,
        attrs: { style: `flex:${Math.max(width, 0.001)} 1 0`, title: segment.label },
      }));
    }
    const option = el("div", {
      class: `waterfall-row${failed ? " is-failed" : ""}`,
      attrs: {
        role: "option",
        "aria-selected": "false",
        tabindex: index === 0 ? "0" : "-1",
        "aria-label": describe,
        title: describe,
        "data-request": String(record.id ?? index),
      },
      on: { click: () => showNetworkRecord(record) },
    },
      el("span", { class: "waterfall-label" },
        el("span", { class: "waterfall-method", text: textOf(record.method, "?") }),
        el("span", { class: "waterfall-url", text: textOf(record.url, "(no URL)") }),
      ),
      el("span", { class: "waterfall-track" }, bar),
      el("span", { class: "waterfall-duration", text: pending ? `≥ ${fmtMs(elapsed, 1)}` : fmtMs(elapsed, 1) }),
    );
    list.append(option);
  }
  list.addEventListener("keydown", event => {
    const current = event.target.closest?.(".waterfall-row");
    if (!current) return;
    const options = [...list.children];
    const index = options.indexOf(current);
    let next = null;
    if (event.key === "ArrowDown") next = options[Math.min(options.length - 1, index + 1)];
    else if (event.key === "ArrowUp") next = options[Math.max(0, index - 1)];
    else if (event.key === "Home") next = options[0];
    else if (event.key === "End") next = options[options.length - 1];
    else if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      current.click();
      return;
    }
    if (!next) return;
    event.preventDefault();
    for (const option of options) option.tabIndex = -1;
    next.tabIndex = 0;
    next.focus();
  });
  const ticks = [0, 0.25, 0.5, 0.75, 1].map(fraction => el("span", {
    class: "waterfall-tick",
    attrs: { style: `left:${fraction * 100}%` },
    text: fmtMs(span * fraction, span < 10 ? 2 : 0),
  }));
  const notes = [
    "Cove's recorder timestamps the request start, the response head and the body end. It captures no DNS, connect, TLS or queueing phases, so none are drawn here.",
  ];
  if (rows.some(row => row.pending)) {
    notes.push(`In-flight requests end at the read instant ${fmtEpochMs(readAt)}. That is a lower bound on their duration, not a recorded completion.`);
  }
  if (rows.some(row => row.ttfb === null && !row.pending)) {
    notes.push("Requests without a recorded head arrival show one undivided bar; the wait/receive split is not estimated.");
  }
  if (unplaced) {
    notes.push(`${unplaced} request${unplaced === 1 ? "" : "s"} carried no start timestamp and ${unplaced === 1 ? "is" : "are"} omitted from the axis.`);
  }
  return el("div", { class: "panel-block" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Waterfall" }),
      el("span", { class: "pane-meta", text: `${rows.length} request${rows.length === 1 ? "" : "s"} · ${fmtMs(span, 1)} span · from ${fmtEpochMs(origin)}` }),
    ),
    el("div", { class: "panel-block-body" },
      el("div", { class: "waterfall" },
        el("div", { class: "waterfall-axis" },
          el("span", { class: "waterfall-axis-spacer" }),
          el("span", { class: "waterfall-axis-track" }, ...ticks),
          el("span", { class: "waterfall-axis-spacer is-duration" }),
        ),
        list,
      ),
      el("div", { class: "perf-legend" },
        el("span", { class: "is-waiting", text: "Waiting for head" }),
        el("span", { class: "is-receiving", text: "Receiving body" }),
        el("span", { class: "is-inflight", text: "In flight" }),
        el("span", { class: "is-unsplit", text: "Head not recorded" }),
        el("span", { class: "is-failed", text: "Failed" }),
      ),
      ...notes.map(note => el("div", { class: "perf-note", text: note })),
    ),
  );
}

function renderNetwork() {
  const host = dom.networkContent;
  clear(host);
  setNetworkChrome();
  const net = state.network;
  const target = targetById();
  const paused = net.recording === false;
  dom.networkRecordingStatus.hidden = !paused;
  if (!target) {
    dom.networkMeta.textContent = "No target selected.";
    showState(host, { icon: "device", title: "No target", message: "Select a connected target to read network activity." });
    return;
  }
  if (!state.caps.network) {
    dom.networkMeta.textContent = "Unavailable for this target.";
    showState(host, {
      icon: "network",
      title: "Network is unavailable",
      message: state.caps.runtime ? "This runtime does not advertise network recording." : "Network records come from a Cove runtime. Select a runtime target.",
    });
    return;
  }
  const controlsMissing = !state.caps.networkControl || net.controlUnavailable;
  const chrome = () => {
    const nodes = [];
    if (controlsMissing) {
      nodes.push(el("div", {
        class: "warn-inline network-note",
        attrs: { role: "note" },
        text: state.caps.networkControl
          ? "This runtime does not expose network recording controls."
          : "This runtime does not advertise network recording controls.",
      }));
    }
    return nodes;
  };
  if (net.clearPending && !net.records.length) {
    dom.networkMeta.textContent = paused ? "Clearing… · recording paused" : "Clearing…";
    showState(host, { kind: "loading", title: "Clearing network history" });
    host.prepend(...chrome());
    return;
  }
  if (net.loading && !net.loaded) {
    dom.networkMeta.textContent = "Loading…";
    showState(host, { kind: "loading", title: "Reading network activity" });
    host.prepend(...chrome());
    return;
  }
  if (net.error && !net.records.length) {
    dom.networkMeta.textContent = "Error";
    showState(host, {
      kind: "error",
      title: "Network read failed",
      message: errorMessage(net.error),
      action: "Retry",
      onAction: () => isDisconnectedError(net.error) ? refreshCurrent() : loadNetwork(),
    });
    host.prepend(...chrome());
    return;
  }
  if (net.clearError && !net.records.length) {
    dom.networkMeta.textContent = "Clear failed";
    showState(host, {
      kind: "error",
      title: "Clear failed",
      message: errorMessage(net.clearError),
      action: "Retry",
      onAction: () => clearNetwork(),
    });
    host.prepend(...chrome());
    return;
  }
  if (!net.records.length) {
    if (paused) {
      dom.networkMeta.textContent = "0 requests · recording paused";
      showState(host, {
        icon: "pause",
        title: "Recording paused",
        message: "No requests are being recorded. Existing requests stay in the table; resume to capture new ones.",
        action: networkControlsAvailable() && state.connected ? "Resume" : null,
        onAction: () => setNetworkRecording(true),
      });
      host.prepend(...chrome());
      return;
    }
    dom.networkMeta.textContent = net.cleared ? "Cleared" : "0 requests";
    showState(host, {
      icon: "network",
      title: net.cleared ? "Network history cleared" : "No HTTP activity",
      message: net.cleared
        ? (net.follow ? "Waiting for requests issued after the clear." : "Requests issued after the clear will appear here.")
        : "Requests issued by this runtime appear here. Recording may be disabled in this build.",
      action: "Refresh",
      onAction: () => loadNetwork(),
    });
    host.prepend(...chrome());
    return;
  }
  const filtered = net.records.filter(networkMatches);
  const parts = [];
  if (net.clearPending) parts.push("Clearing…");
  parts.push(`${net.records.length} request${net.records.length === 1 ? "" : "s"}`);
  if (filtered.length !== net.records.length) parts.push(`${filtered.length} shown`);
  if (paused) parts.push("recording paused");
  if (controlsMissing) parts.push("controls unavailable");
  if (net.limit) parts.push(`buffer ${net.limit}`);
  dom.networkMeta.textContent = parts.join(" · ");
  if (!filtered.length) {
    showState(host, { icon: "search", title: "No matching requests", message: "Change the URL filter or request state." });
    host.prepend(...chrome());
    return;
  }
  host.append(...chrome());
  if (net.clearError) host.append(errorInline(`Clear failed: ${errorMessage(net.clearError)}`));
  if (net.controlError) host.append(errorInline(`Recording control failed: ${errorMessage(net.controlError)}`));
  if (net.error) host.append(errorInline(errorMessage(net.error)));
  if (net.waterfall) host.append(renderNetworkWaterfall(filtered));
  const table = el("table", { class: "data-table" },
    el("thead", {}, el("tr", {},
      el("th", { text: "Time" }),
      el("th", { text: "Method" }),
      el("th", { text: "Status" }),
      el("th", { text: "URL" }),
      el("th", { text: "State" }),
      el("th", { class: "num", text: "TTFB" }),
      el("th", { class: "num", text: "Duration" }),
      el("th", { class: "num", text: "Bytes" }),
    )),
    el("tbody", {}, ...filtered.map(record => {
      const status = numberValue(record.status);
      const statusClass = status === null ? "" : status >= 500 ? "bad" : status >= 400 ? "warn" : "ok";
      return el("tr", {},
        el("td", { class: "mono-cell", text: fmtEpochMs(record.startedAt), attrs: { title: record.startedAt ? new Date(record.startedAt).toLocaleString() : "" } }),
        el("td", { class: "method", text: textOf(record.method, "?") }),
        el("td", {}, el("span", { class: `status-code ${statusClass}`, text: status === null ? "—" : String(status) })),
        el("td", { class: "url-cell" }, el("button", {
          class: "link-button",
          props: { type: "button", title: record.url || "" },
          text: textOf(record.url, "(no URL)"),
          on: { click: () => showNetworkRecord(record) },
        })),
        el("td", {}, el("span", { class: `state-badge ${record.state || ""}`, text: textOf(record.state, "unknown") })),
        el("td", { class: "num", text: fmtMs(record.ttfbMs) }),
        el("td", { class: "num", text: fmtMs(record.durationMs) }),
        el("td", { class: "num", text: fmtBytes(record.responseBytes), attrs: { title: `request ${fmtBytes(record.requestBytes)} · response ${fmtBytes(record.responseBytes)}` } }),
      );
    })),
  );
  host.append(el("div", { class: "table-wrap" }, table));
}

function headerPairs(headers) {
  if (!headers) return [];
  if (Array.isArray(headers)) {
    return headers.map(entry => Array.isArray(entry) ? [String(entry[0]), String(entry[1])] : [String(entry?.name ?? ""), String(entry?.value ?? "")]);
  }
  if (typeof headers === "object") return Object.entries(headers).map(([name, value]) => [name, String(value)]);
  return [];
}

function renderHeaderPairs(pairs) {
  if (!pairs.length) return emptyInline("No headers were captured.");
  return el("div", { class: "header-pairs" }, ...pairs.flatMap(([name, value]) => [
    el("span", { class: "name", text: name }),
    el("span", { class: "value", text: value }),
  ]));
}

function showNetworkRecord(record) {
  dom.networkDialogTitle.textContent = `${textOf(record.method, "?")} ${textOf(record.url, "")}`;
  const body = dom.networkDialogBody;
  clear(body);
  body.append(
    sectionBlock("Request",
      kvList([
        ["ID", String(record.id ?? "—")],
        ["URL", el("span", { class: "mono", text: textOf(record.url, "—") })],
        ["Status", record.status ?? "—"],
        ["State", record.state || "—"],
        ["Error", record.error || "—"],
        ["Started", record.startedAt ? `${fmtEpochMs(record.startedAt)} · ${new Date(record.startedAt).toLocaleString()}` : "—"],
        ["TTFB", fmtMs(record.ttfbMs)],
        ["Duration", fmtMs(record.durationMs)],
        ["Request bytes", fmtBytes(record.requestBytes)],
        ["Response bytes", fmtBytes(record.responseBytes)],
      ]),
      el("div", { class: "source-actions" },
        el("button", { class: "tool-button", props: { type: "button" }, text: "Copy URL", on: { click: () => copyText(record.url, "URL") } }),
      ),
    ),
    sectionBlock("Request headers", renderHeaderPairs(headerPairs(record.requestHeaders))),
    sectionBlock("Response headers", renderHeaderPairs(headerPairs(record.responseHeaders))),
    sectionBlock("Request body",
      record.requestBody ? el("pre", { class: "body-block", text: record.requestBody }) : emptyInline("No request body was captured."),
      record.requestBodyTruncated ? el("div", { class: "truncation", text: "The request body was truncated at the recorder's byte limit." }) : null,
    ),
    sectionBlock("Response body",
      record.responseBody ? el("pre", { class: "body-block", text: record.responseBody }) : emptyInline("No response body was captured."),
      record.responseBodyTruncated ? el("div", { class: "truncation", text: "The response body was truncated at the recorder's byte limit." }) : null,
    ),
  );
  dom.networkDialog.showModal();
  dom.networkDialogClose.focus();
}

/* Logs */

function resetLogs() {
  stopTimer("logs");
  state.logs = {
    loaded: false, loading: false, error: null, records: [], dropped: 0, firstAvailable: null,
    follow: dom.logsFollow.checked, timer: null, clearing: false, clearError: null, generation: 0,
    cleared: false,
  };
  dom.logsMeta.textContent = "Recent runtime log records.";
  dom.logsLabel.value = "";
  dom.logsBadge.hidden = true;
  if (state.view === "logs") renderLogs();
}

function ensureLogs() {
  if (!state.token || !state.targetId || !state.caps.logs) {
    renderLogs();
    return;
  }
  if (!state.logs.loaded && !state.logs.loading) loadLogs({ reset: true });
  else renderLogs();
}

async function loadLogs(options = {}) {
  const quiet = options.quiet === true;
  const logs = state.logs;
  if (!state.caps.logs || !state.targetId || logs.loading || logs.clearing) return;
  const targetId = state.targetId;
  const epoch = state.targetEpoch;
  const generation = logs.generation;
  if (options.reset) {
    logs.records = [];
    logs.dropped = 0;
    logs.firstAvailable = null;
    logs.loaded = false;
    logs.cleared = false;
    logs.clearError = null;
  }
  logs.loading = true;
  logs.error = null;
  renderLogs();
  try {
    const after = logs.records.reduce((maximum, record) => Math.max(maximum, numberValue(record.sequence) ?? 0), 0);
    const params = { target: targetId };
    if (after > 0) params.after = after;
    const data = unwrapData(await rpc("query_ui_logs", params));
    if (epoch !== state.targetEpoch || targetId !== state.targetId || generation !== logs.generation) return;
    const incoming = Array.isArray(data?.records) ? data.records : [];
    const merged = new Map(logs.records.map(record => [record.sequence, record]));
    for (const record of incoming) merged.set(record.sequence, record);
    let records = [...merged.values()].sort((left, right) => (numberValue(left.sequence) ?? 0) - (numberValue(right.sequence) ?? 0));
    const capped = records.length > 500;
    if (capped) records = records.slice(-500);
    logs.records = records;
    logs.capped = capped;
    logs.dropped = numberValue(data?.dropped) ?? logs.dropped;
    logs.firstAvailable = data?.firstAvailableSequence ?? logs.firstAvailable;
    logs.loaded = true;
    logs.error = null;
    if (records.length) logs.cleared = false;
    dom.logsBadge.hidden = records.length === 0;
    dom.logsBadge.textContent = String(records.length);
  } catch (error) {
    if (epoch !== state.targetEpoch || targetId !== state.targetId || generation !== logs.generation) return;
    logs.error = error;
    if (!quiet) toast(errorMessage(error), "error", 8000);
  } finally {
    if (epoch === state.targetEpoch && targetId === state.targetId && generation === logs.generation) {
      logs.loading = false;
      renderLogs();
    }
  }
}

async function clearLogs() {
  const logs = state.logs;
  if (!state.caps.logs || !state.targetId || !state.token || !state.connected || logs.clearing) return;
  const targetId = state.targetId;
  const epoch = state.targetEpoch;
  // Responses from reads that started before the clear describe the old replay
  // window, so they must not merge records back into the cleared table.
  logs.generation += 1;
  logs.clearing = true;
  logs.loading = false;
  logs.clearError = null;
  logs.error = null;
  renderLogs();
  let cleared = false;
  try {
    const data = unwrapData(await rpc("clear_ui_logs", { target: targetId }));
    if (epoch !== state.targetEpoch || targetId !== state.targetId || state.logs !== logs) return;
    const count = numberValue(data?.cleared);
    logs.records = [];
    logs.capped = false;
    logs.dropped = numberValue(data?.dropped) ?? 0;
    logs.firstAvailable = data?.firstAvailableSequence ?? null;
    logs.loaded = true;
    logs.error = null;
    logs.clearError = null;
    logs.cleared = true;
    dom.logsBadge.hidden = true;
    dom.logsBadge.textContent = "0";
    cleared = true;
    toast(count !== null && count > 0
      ? `${count} log record${count === 1 ? "" : "s"} cleared.`
      : "Log history cleared.", "success", 4000);
  } catch (error) {
    if (epoch !== state.targetEpoch || targetId !== state.targetId || state.logs !== logs) return;
    logs.clearError = error;
    logs.loaded = true;
    toast(`Clear failed: ${errorMessage(error)}`, "error", 9000);
  } finally {
    if (epoch === state.targetEpoch && targetId === state.targetId && state.logs === logs) {
      logs.clearing = false;
      renderLogs();
      syncFollowTimers();
      if (cleared && logs.follow) loadLogs({ quiet: true });
    }
  }
}

function setLogsChrome() {
  const logs = state.logs;
  const target = targetById();
  const offline = !state.token || !state.connected;
  const unavailable = !target || !state.caps.logs;
  dom.logsClear.disabled = offline || unavailable || logs.clearing;
  dom.logsRefresh.disabled = logs.clearing;
  dom.logsClear.setAttribute("aria-busy", String(logs.clearing));
  dom.logsContent.setAttribute("aria-busy", String(logs.clearing));
  let title = "Clear the runtime's replay log history";
  if (offline) title = "Reconnect to the controller to clear logs";
  else if (!target) title = "Select a connected target to clear logs";
  else if (!state.caps.logs) title = state.caps.runtime
    ? "This runtime does not advertise diagnostics"
    : "Logs come from a Cove runtime. Select a runtime target";
  else if (logs.clearing) title = "Clearing the runtime's replay log history";
  dom.logsClear.title = title;
  const label = dom.logsClear.querySelector("span");
  if (label) label.textContent = logs.clearing ? "Clearing…" : "Clear";
}

function logMatches(record) {
  if (dom.logsLabel.value && record.label !== dom.logsLabel.value) return false;
  const minimum = LEVELS.indexOf(dom.logsLevel.value);
  const levelIndex = LEVELS.indexOf(record.level);
  if (minimum >= 0 && levelIndex >= 0 && levelIndex < minimum) return false;
  const query = dom.logsSearch.value.trim().toLowerCase();
  if (!query) return true;
  const location = record.location || {};
  const process = record.process || {};
  return [record.level, record.label, record.message, location.file, location.function, location.source, process.name, process.runID]
    .filter(Boolean).join(" ").toLowerCase().includes(query);
}

function renderLogs() {
  const host = dom.logsContent;
  clear(host);
  setLogsChrome();
  const selectedLabel = dom.logsLabel.value;
  const labels = [...new Set(state.logs.records.map(record => record.label).filter(Boolean))].sort();
  if (selectedLabel && !labels.includes(selectedLabel)) labels.push(selectedLabel);
  dom.logsLabel.replaceChildren(el("option", { text: "All labels", attrs: { value: "" } }),
    ...labels.map(label => el("option", { text: label, attrs: { value: label } })));
  dom.logsLabel.value = selectedLabel;
  dom.logsLabel.disabled = !state.caps.logs || !labels.length;
  const target = targetById();
  if (!target) {
    dom.logsMeta.textContent = "No target selected.";
    showState(host, { icon: "device", title: "No target", message: "Select a connected target to read logs." });
    return;
  }
  if (!state.caps.logs) {
    dom.logsMeta.textContent = "Unavailable for this target.";
    showState(host, {
      icon: "logs",
      title: "Logs are unavailable",
      message: state.caps.runtime ? "This runtime does not advertise diagnostics." : "Logs come from a Cove runtime. Select a runtime target.",
    });
    return;
  }
  if (state.logs.clearing && !state.logs.records.length) {
    dom.logsMeta.textContent = "Clearing…";
    showState(host, { kind: "loading", title: "Clearing log history" });
    return;
  }
  if (state.logs.loading && !state.logs.loaded) {
    dom.logsMeta.textContent = "Loading…";
    showState(host, { kind: "loading", title: "Reading logs" });
    return;
  }
  if (state.logs.error && !state.logs.records.length) {
    dom.logsMeta.textContent = "Error";
    showState(host, {
      kind: "error",
      title: "Log read failed",
      message: errorMessage(state.logs.error),
      action: "Retry",
      onAction: () => isDisconnectedError(state.logs.error) ? refreshCurrent() : loadLogs({ reset: true }),
    });
    return;
  }
  if (state.logs.clearError && !state.logs.records.length) {
    dom.logsMeta.textContent = "Clear failed";
    showState(host, {
      kind: "error",
      title: "Clear failed",
      message: errorMessage(state.logs.clearError),
      action: "Retry",
      onAction: () => clearLogs(),
    });
    return;
  }
  if (!state.logs.records.length) {
    const wasCleared = state.logs.cleared;
    dom.logsMeta.textContent = wasCleared ? "Cleared" : "0 records";
    showState(host, {
      icon: "logs",
      title: wasCleared ? "Log history cleared" : "No log records",
      message: wasCleared
        ? (state.logs.follow ? "Waiting for records logged after the clear." : "Records logged after the clear will appear here.")
        : "The runtime has not published log records yet.",
      action: "Refresh",
      onAction: () => loadLogs({ reset: true }),
    });
    return;
  }
  const filtered = state.logs.records.filter(logMatches).slice().reverse();
  const parts = [];
  if (state.logs.clearing) parts.push("Clearing…");
  parts.push(`${state.logs.records.length} record${state.logs.records.length === 1 ? "" : "s"}`);
  if (filtered.length !== state.logs.records.length) parts.push(`${filtered.length} shown`);
  if (state.logs.dropped) parts.push(`${state.logs.dropped} dropped`);
  if (state.logs.capped) parts.push("retaining last 500");
  if (state.logs.firstAvailable) parts.push(`first available #${state.logs.firstAvailable}`);
  dom.logsMeta.textContent = parts.join(" · ");
  if (state.logs.clearError) host.append(errorInline(`Clear failed: ${errorMessage(state.logs.clearError)}`));
  if (state.logs.error) host.append(errorInline(errorMessage(state.logs.error)));
  if (!filtered.length) {
    showState(host, { icon: "search", title: "No matching logs", message: "Change the level or text filter." });
    return;
  }
  const list = el("div", { class: "log-list" });
  for (const record of filtered) {
    const level = LEVELS.includes(record.level) ? record.level : "info";
    const location = record.location || {};
    const source = location.file ? `${basename(location.file)}:${location.line ?? "?"}` : (location.source || location.function || "");
    const summary = el("summary", {},
      el("span", { class: "log-time", text: fmtClock(record.timestamp), attrs: { title: fmtDate(record.timestamp) } }),
      el("span", { class: `log-level ${level}`, text: level }),
      el("span", { class: "log-label", text: textOf(record.label, "—"), attrs: { title: record.label || "" } }),
      el("span", { class: "log-message", text: textOf(record.message, "") }),
      el("span", { class: "log-source", text: source, attrs: { title: [location.file, location.function, location.line].filter(Boolean).join(":") } }),
    );
    const detail = el("div", { class: "log-detail" },
      kvList([
        ["Sequence", String(record.sequence ?? "—")],
        ["Timestamp", fmtDate(record.timestamp)],
        ["Label", record.label || "—"],
        ["Location", [location.file, location.function, location.line, location.column].filter(value => value !== undefined && value !== null && value !== "").join(":") || "—"],
        ["Process", record.process ? `${record.process.name || "?"} · pid ${record.process.pid ?? "?"}` : "—"],
        ["Run", record.process?.runID || "—"],
      ]),
    );
    if (record.metadata && Object.keys(record.metadata).length) {
      detail.append(el("div", { class: "section" }, el("h3", { text: "Metadata" }), el("pre", { text: JSON.stringify(record.metadata, null, 2) })));
    }
    const entry = el("details", { class: "log-entry" }, summary, detail);
    list.append(entry);
  }
  host.append(list);
}

/* Performance */

function readBudgetPreference() {
  try {
    const stored = Number(localStorage.getItem(BUDGET_KEY));
    return BUDGET_RATES.includes(stored) ? stored : 60;
  } catch {
    return 60;
  }
}

function writeBudgetPreference(rate) {
  try { localStorage.setItem(BUDGET_KEY, String(rate)); } catch {}
}

/// Adopts the display rate the runtime measured. An absent or non-positive
/// rate leaves the user's preference in charge rather than assuming 60 Hz.
function applyRuntimeRefreshRate(perf, rate) {
  const hz = rate !== null && Number.isFinite(rate) && rate > 0 ? rate : null;
  perf.refreshHz = hz;
  if (hz !== null && !perf.budgetManual) perf.budgetHz = hz;
  syncBudgetOptions(perf);
}

/// Keeps the budget selector honest about where its rate comes from: the
/// runtime's measured display rate, or a rate the user chose by hand.
function syncBudgetOptions(perf) {
  const select = dom.performanceBudget;
  if (!select) return;
  const rates = [...BUDGET_RATES];
  if (perf.refreshHz !== null && !rates.includes(perf.refreshHz)) rates.push(perf.refreshHz);
  rates.sort((left, right) => left - right);
  const existing = [...select.options].map(option => Number(option.value));
  if (existing.length !== rates.length || existing.some((rate, index) => rate !== rates[index])) {
    clear(select);
    for (const rate of rates) {
      select.append(el("option", {
        attrs: { value: String(rate) },
        text: `${fmtNumber(rate, rate % 1 === 0 ? 0 : 1)} Hz · ${fmtNumber(1000 / rate, 1)} ms`,
      }));
    }
  }
  select.value = String(perf.budgetHz);
  const measured = perf.refreshHz !== null && !perf.budgetManual;
  select.title = measured
    ? `The runtime measured this display's refresh rate: ${fmtNumber(perf.refreshHz, 1)} Hz. Choose another rate to override it.`
    : perf.refreshHz === null
      ? "The runtime does not publish its display refresh rate. The budget is the rate you select here."
      : `Overriding the runtime's measured ${fmtNumber(perf.refreshHz, 1)} Hz display rate.`;
}

function emptyAllocationSites() {
  return {
    loaded: false,
    loading: false,
    polling: false,
    action: null,
    error: null,
    status: "idle",
    supported: null,
    reason: "",
    rate: ALLOCATION_DEFAULT_RATE,
    report: null,
    queriedAt: 0,
    startedAt: 0,
    stoppedAt: 0,
    request: 0,
    timer: null,
    expanded: new Set(),
    sessionSeq: 0,
    baseline: null,
    baselineSource: "",
    baselineError: "",
    diffExpanded: new Set(),
  };
}

function readMemoryIntervalPreference() {
  try {
    const value = Number(localStorage.getItem(MEMORY_INTERVAL_KEY));
    if (MEMORY_INTERVALS.includes(value)) return value;
  } catch {}
  return 2000;
}

function writeMemoryIntervalPreference(interval) {
  try {
    localStorage.setItem(MEMORY_INTERVAL_KEY, String(interval));
  } catch {}
}

function emptyMemory() {
  return {
    loaded: false,
    loading: false,
    error: null,
    data: null,
    refreshedAt: 0,
    request: 0,
    history: [],
    follow: dom.memoryFollow?.checked === true,
    timer: null,
    intervalMs: readMemoryIntervalPreference(),
    series: { rss: true, allocated: true, caches: true, targets: true, virtual: false },
    allocation: emptyAllocationSites(),
  };
}

function emptyAppSize() {
  return {
    loaded: false,
    loading: false,
    error: null,
    data: null,
    refreshedAt: 0,
    request: 0,
    treemapMode: "groups",
    treemapFocus: null,
    containmentOpen: {},
    baseline: null,
    baselineError: "",
  };
}

function emptyDeepLinks() {
  return {
    root: readDeepLinksRootPreference(),
    validatedRoot: "",
    loaded: false,
    loading: false,
    error: null,
    data: null,
    refreshedAt: 0,
    request: 0,
  };
}

function readDeepLinksRootPreference() {
  try {
    return localStorage.getItem(DEEP_LINKS_ROOT_KEY) || "";
  } catch {
    return "";
  }
}

function writeDeepLinksRootPreference(root) {
  try {
    if (root) localStorage.setItem(DEEP_LINKS_ROOT_KEY, root);
    else localStorage.removeItem(DEEP_LINKS_ROOT_KEY);
  } catch {}
}

function emptyProfile() {
  return {
    status: "idle",
    supported: null,
    reason: "",
    error: null,
    loaded: false,
    loading: false,
    polling: false,
    request: 0,
    loadRequest: 0,
    hz: PROFILE_DEFAULT_HZ,
    durationMs: PROFILE_DEFAULT_DURATION,
    startedAt: 0,
    completedAt: 0,
    elapsedMs: 0,
    thread: "",
    platform: "",
    intervalMs: null,
    samplesTaken: 0,
    samplesDropped: 0,
    reportedDurationMs: null,
    samplesPerSecond: null,
    samples: [],
    samplesTruncated: false,
    retainedSamples: 0,
    stacks: [],
    stackTotalSamples: 0,
    stacksTruncated: false,
    hotspots: { loading: false, error: null, data: null },
    view: "call-tree",
    filter: "",
    selection: null,
    expanded: new Set(),
    methodSort: { key: "selfSamples", dir: -1 },
    pollTimer: null,
    stopTimer: null,
  };
}

function emptyPerformance() {
  return {
    tab: "frames",
    budgetHz: readBudgetPreference(),
    /// The display rate the runtime published, or null when it cannot observe
    /// one. A published rate drives the budget until the user overrides it.
    refreshHz: null,
    budgetManual: false,
    frames: [],
    dropped: 0,
    loaded: false,
    loading: false,
    error: null,
    follow: dom.performanceFollow?.checked === true,
    timer: null,
    selectedFrame: null,
    analysis: { frame: null, loading: false, error: null, data: null, request: 0 },
    analysisCache: new Map(),
    recording: false,
    startedAt: 0,
    recordTimer: null,
    traceLoading: false,
    traceError: null,
    captures: [],
    activeCaptureId: "",
    eventLoading: false,
    eventError: null,
    baseline: "",
    candidate: "",
    comparison: null,
    comparing: false,
    selectedSpan: null,
    zoom: 1,
    snapshot: null,
    importError: null,
    profile: emptyProfile(),
  };
}

function resetPerformance(previousTarget = state.targetId) {
  const previous = state.performance;
  if (previous?.recordTimer) clearInterval(previous.recordTimer);
  if (previous?.recording && previousTarget) {
    rpc("stop_ui_trace", { target: previousTarget }).catch(() => {});
  }
  if (previous?.profile?.status === "sampling" && previousTarget) {
    rpc("stop_ui_profile", { target: previousTarget }).catch(() => {});
  }
  clearProfileTimers(previous?.profile);
  stopTimer("performance");
  const budget = previous?.budgetHz ?? readBudgetPreference();
  const tab = previous?.tab ?? "frames";
  const hz = PROFILE_HZ_RATES.includes(previous?.profile?.hz) ? previous.profile.hz : PROFILE_DEFAULT_HZ;
  const durationMs = PROFILE_DURATIONS.includes(previous?.profile?.durationMs) ? previous.profile.durationMs : PROFILE_DEFAULT_DURATION;
  state.performance = emptyPerformance();
  state.performance.budgetHz = BUDGET_RATES.includes(budget) ? budget : 60;
  state.performance.tab = PERFORMANCE_TABS.includes(tab) ? tab : "frames";
  state.performance.profile.hz = hz;
  state.performance.profile.durationMs = durationMs;
  if (dom.performanceFollow) state.performance.follow = dom.performanceFollow.checked === true;
  dom.performanceMeta.textContent = "Frame timings, CPU spans and sampled CPU profiles published by the runtime.";
  if (state.view === "performance") renderPerformance();
}

function ensurePerformance() {
  const perf = state.performance;
  if (!state.token || perf.snapshot || !state.targetId) {
    renderPerformance();
    return;
  }
  if (perf.tab === "frames" && state.caps.timing && !perf.loaded && !perf.loading && !perf.error) {
    loadFrames();
    return;
  }
  renderPerformance();
  if (perf.tab === "profile") ensureProfile();
}

function activeCaptureRecord() {
  const perf = state.performance;
  return perf.captures.find(record => record.summary?.id === perf.activeCaptureId) || null;
}

function captureRecordById(id) {
  return state.performance.captures.find(record => record.summary?.id === id) || null;
}

function setPerformanceTab(tab, options = {}) {
  if (!PERFORMANCE_TABS.includes(tab)) return;
  if (state.performance.tab === tab && options.force !== true) return;
  state.performance.tab = tab;
  syncFollowTimers();
  renderPerformance();
  dom.performanceContent.scrollTop = 0;
  if (tab === "profile") ensureProfile();
  if (options.focusTab) $(`tab-performance-${tab}`)?.focus();
}

function setPerformanceChrome() {
  const perf = state.performance;
  const caps = state.caps;
  const snapshot = Boolean(perf.snapshot);
  const framesTab = perf.tab === "frames";
  const profileTab = perf.tab === "profile";
  dom.performanceBudget.value = String(perf.budgetHz);
  dom.performanceBudget.disabled = snapshot || !framesTab;
  dom.performanceFollow.checked = perf.follow;
  dom.performanceFollow.disabled = snapshot || !caps.timing || !framesTab;
  dom.performanceRefresh.disabled = snapshot || (profileTab ? !caps.profile || perf.profile.loading : !caps.timing || perf.loading);
  dom.performanceRefresh.title = profileTab ? "Refresh CPU profile and hot spots" : "Refresh frame timings";
  dom.performanceStart.disabled = snapshot || !caps.trace || perf.recording || perf.traceLoading;
  dom.performanceStop.disabled = snapshot || !perf.recording || perf.traceLoading;
  dom.performanceStart.title = snapshot
    ? "Return to live data before recording a trace."
    : caps.trace
      ? "Record CPU spans around a real operation"
      : "This target does not advertise trace capture.";
  dom.performanceStop.title = perf.recording ? "Stop the CPU trace" : "No trace is recording.";
  dom.performanceExport.disabled = perf.loading || perf.recording || perf.traceLoading || perf.eventLoading || !(perf.frames.length || perf.captures.length);
  dom.performanceElapsed.hidden = !perf.recording;
  dom.performanceRecordingDot.hidden = !perf.recording;
  dom.performanceSnapshotBadge.hidden = !snapshot;
  if (snapshot) dom.performanceSnapshotBadge.textContent = "Imported snapshot";
  if (perf.recording) updateTraceElapsed();
}

function updatePerformanceMeta() {
  const perf = state.performance;
  const target = targetById();
  if (perf.snapshot) {
    const spans = perf.captures.reduce((total, record) => total + (numberValue(record.summary?.eventCount) ?? record.events.length), 0);
    const parts = [`Imported ${perf.snapshot.name}`, `${perf.frames.length} frames`, `${spans} spans`];
    if (perf.snapshot.exportedAt) parts.push(`exported ${perf.snapshot.exportedAt}`);
    dom.performanceMeta.textContent = parts.join(" · ");
    return;
  }
  if (!target) {
    dom.performanceMeta.textContent = "No target selected.";
    return;
  }
  if (!state.caps.runtime) {
    dom.performanceMeta.textContent = "Performance data comes from a Cove runtime. Select a runtime target.";
    return;
  }
  if (perf.tab === "frames") {
    if (!state.caps.timing) {
      dom.performanceMeta.textContent = "This runtime does not advertise frame timing.";
      return;
    }
    if (perf.loading && !perf.loaded) {
      dom.performanceMeta.textContent = "Reading frame timings…";
      return;
    }
    if (perf.error && !perf.frames.length) {
      dom.performanceMeta.textContent = "Frame timing read failed.";
      return;
    }
    if (!perf.frames.length) {
      dom.performanceMeta.textContent = "0 frames";
      return;
    }
    const recent = perf.frames.slice(-60);
    const budget = 1000 / perf.budgetHz;
    const janks = recent.filter(frame => frameJank(frame, budget)).length;
    const parts = [`${perf.frames.length} frames`, `${janks} jank in last ${recent.length}`, `budget ${fmtNumber(budget, 1)} ms`];
    if (perf.dropped) parts.push(`${perf.dropped} dropped`);
    if (perf.loading) parts.push("updating…");
    dom.performanceMeta.textContent = parts.join(" · ");
    return;
  }
  if (perf.tab === "profile") {
    const profile = perf.profile;
    const platform = profilePlatformName(target);
    if (profile.status === "unsupported" || platform === "linux" || platform === "windows") {
      dom.performanceMeta.textContent = `CPU sampling is not available on ${platform === "windows" ? "Windows" : platform === "linux" ? "Linux" : platform || "this platform"}.`;
      return;
    }
    if (!state.caps.profile) {
      dom.performanceMeta.textContent = "This runtime does not advertise CPU profiling.";
      return;
    }
    if (profile.status === "sampling" || profile.status === "starting") {
      const parts = [
        `Sampling ${profile.thread || "the UI thread"}`,
        fmtSeconds(profileElapsedMs(profile)),
        `${formatCount(profile.samplesTaken)} samples`,
      ];
      if (profile.samplesDropped) parts.push(`${formatCount(profile.samplesDropped)} overwritten`);
      dom.performanceMeta.textContent = parts.join(" · ");
      return;
    }
    if (profile.loading && !profile.loaded) {
      dom.performanceMeta.textContent = "Reading CPU profile…";
      return;
    }
    if (profile.error && !profile.samplesTaken && !profile.samples.length) {
      dom.performanceMeta.textContent = "CPU profile read failed.";
      return;
    }
    if (profile.status === "complete" || profile.samples.length || profile.samplesTaken) {
      const parts = [
        `${formatCount(profile.samplesTaken)} samples`,
        fmtSeconds(profile.elapsedMs || profile.reportedDurationMs),
        profile.thread || "unknown thread",
        `${formatCount(profile.stacks.length)} hot stacks`,
      ];
      if (profile.samplesDropped) parts.push(`${formatCount(profile.samplesDropped)} overwritten`);
      dom.performanceMeta.textContent = parts.join(" · ");
      return;
    }
    dom.performanceMeta.textContent = "No CPU profile.";
    return;
  }
  if (!state.caps.trace) {
    dom.performanceMeta.textContent = "This runtime does not advertise trace capture.";
    return;
  }
  if (perf.recording) {
    dom.performanceMeta.textContent = perf.traceLoading ? "Stopping trace…" : "Recording CPU spans…";
    return;
  }
  if (perf.traceLoading) {
    dom.performanceMeta.textContent = "Starting trace…";
    return;
  }
  const record = activeCaptureRecord();
  if (!record) {
    dom.performanceMeta.textContent = "No trace capture.";
    return;
  }
  const summary = record.summary || {};
  const parts = [String(summary.id || "capture"), `${numberValue(summary.eventCount) ?? record.events.length} spans`, fmtMs(summary.durationMs, 1)];
  if (record.imported) parts.push("imported");
  else if (!record.complete) parts.push(`${record.events.length} loaded`);
  dom.performanceMeta.textContent = parts.join(" · ");
}

function renderPerformance() {
  const scrollTop = dom.performanceContent.scrollTop;
  const spansScroll = state.performance.tab === "timeline"
    ? dom.performanceTimeline.querySelector(".spans-table-wrap")?.scrollTop ?? 0
    : 0;
  setPerformanceChrome();
  const perf = state.performance;
  for (const name of PERFORMANCE_TABS) {
    const button = $(`tab-performance-${name}`);
    const panel = $(`performance-${name}`);
    if (!button || !panel) continue;
    const selected = name === perf.tab;
    button.setAttribute("aria-selected", String(selected));
    button.tabIndex = selected ? 0 : -1;
    panel.hidden = !selected;
  }
  if (perf.tab === "frames") renderFramesPanel();
  else if (perf.tab === "timeline") renderTimelinePanel();
  else renderProfilePanel();
  updatePerformanceMeta();
  dom.performanceContent.scrollTop = scrollTop;
  const spans = dom.performanceTimeline.querySelector(".spans-table-wrap");
  if (spans) spans.scrollTop = spansScroll;
}

/* Frames */

function frameParts(frame) {
  return {
    recomposition: numberValue(frame?.recompositionMs),
    layout: numberValue(frame?.layoutMs),
    draw: numberValue(frame?.drawMs),
    encode: numberValue(frame?.renderThreadMs),
    // GPU execution overlaps the CPU phases, so it is reported beside them and
    // never summed into UI, total or the jank decision.
    gpu: numberValue(frame?.gpuMs),
    total: numberValue(frame?.totalMs),
  };
}

function frameUiMs(frame) {
  const parts = frameParts(frame);
  if (parts.recomposition === null && parts.layout === null && parts.draw === null) return null;
  return (parts.recomposition ?? 0) + (parts.layout ?? 0) + (parts.draw ?? 0);
}

function frameJank(frame, budget) {
  const ui = frameUiMs(frame);
  const encode = frameParts(frame).encode;
  return (ui !== null && ui > budget) || (encode !== null && encode > budget);
}

async function loadFrames(options = {}) {
  const quiet = options.quiet === true;
  const perf = state.performance;
  if (perf.snapshot || !state.caps.timing || !state.targetId || perf.loading) return;
  const targetId = state.targetId;
  const epoch = state.targetEpoch;
  if (options.reset) {
    perf.frames = [];
    perf.dropped = 0;
    perf.loaded = false;
  }
  perf.loading = true;
  perf.error = null;
  renderPerformance();
  try {
    const after = perf.frames.reduce((maximum, frame) => Math.max(maximum, numberValue(frame.frame) ?? 0), 0);
    const params = { target: targetId };
    if (after > 0) params.after = after;
    const data = await rpc("record_ui_timings", params);
    if (perf.snapshot || epoch !== state.targetEpoch || targetId !== state.targetId) return;
    const incoming = Array.isArray(data?.frames) ? data.frames : [];
    const merged = new Map(perf.frames.map(frame => [frame.frame, frame]));
    for (const frame of incoming) merged.set(frame.frame, frame);
    let frames = [...merged.values()].sort((left, right) => (numberValue(left.frame) ?? 0) - (numberValue(right.frame) ?? 0));
    if (frames.length > MAX_FRAME_RECORDS) frames = frames.slice(-MAX_FRAME_RECORDS);
    perf.frames = frames;
    perf.dropped = Math.max(perf.dropped, numberValue(data?.dropped) ?? 0);
    applyRuntimeRefreshRate(perf, numberValue(data?.refreshRate));
    perf.loaded = true;
    perf.error = null;
    if (perf.selectedFrame !== null && !frames.some(frame => frame.frame === perf.selectedFrame)) {
      perf.selectedFrame = null;
      perf.analysis = { frame: null, loading: false, error: null, data: null, request: perf.analysis.request + 1 };
    }
  } catch (error) {
    if (perf.snapshot || epoch !== state.targetEpoch || targetId !== state.targetId) return;
    perf.error = error;
    if (!quiet) toast(errorMessage(error), "error", 8000);
  } finally {
    if (epoch === state.targetEpoch && targetId === state.targetId) {
      perf.loading = false;
      renderPerformance();
    }
  }
}

function renderFramesPanel() {
  const host = dom.performanceFrames;
  clear(host);
  const perf = state.performance;
  const target = targetById();
  if (perf.snapshot) {
    host.append(snapshotBanner());
    if (!perf.frames.length) {
      const block = el("div");
      showState(block, {
        icon: "timer",
        title: "No frames in this snapshot",
        message: "The imported file contains no TimingFrame records. Its trace captures are available on the Timeline tab.",
        action: "Open timeline",
        onAction: () => setPerformanceTab("timeline"),
      });
      host.append(block);
      return;
    }
  } else if (!target) {
    showState(host, { icon: "device", title: "No target", message: "Select a connected target to read frame timings." });
    return;
  } else if (!state.caps.runtime) {
    showState(host, {
      icon: "timer",
      title: "Frame timings are unavailable",
      message: "Frame timing comes from a Cove runtime. Select a runtime target.",
    });
    return;
  } else if (!state.caps.timing) {
    showState(host, {
      icon: "timer",
      title: "Frame timings are unavailable",
      message: "This runtime does not advertise frame timing. CPU spans may still be available on the Timeline tab.",
    });
    return;
  }
  if (perf.loading && !perf.loaded) {
    showState(host, { kind: "loading", title: "Reading frame timings", message: "Waiting for the runtime's TimingFrame records." });
    return;
  }
  if (perf.error && !perf.frames.length) {
    showState(host, {
      kind: "error",
      title: "Frame timing read failed",
      message: errorMessage(perf.error),
      action: "Retry",
      onAction: () => isDisconnectedError(perf.error) ? refreshCurrent() : loadFrames({ reset: true }),
    });
    return;
  }
  if (!perf.frames.length) {
    showState(host, {
      icon: "timer",
      title: "No frame samples",
      message: "The runtime has not submitted frames with timing data yet. Frame timings come from the runtime probe and do not require Screen Recording.",
      action: "Refresh",
      onAction: () => loadFrames({ reset: true }),
    });
    return;
  }
  if (perf.error) host.append(errorInline(errorMessage(perf.error)));
  if (perf.recording) {
    host.append(el("div", { class: "perf-banner", attrs: { role: "status" } },
      el("span", { class: "recording-label", text: "Live" }),
      el("span", { text: "Recording CPU spans. Stop the trace in the header when the operation is complete; the result opens on the Timeline tab." }),
    ));
  }
  const budget = 1000 / perf.budgetHz;
  host.append(renderFrameStats(perf.frames, budget));
  host.append(renderFrameChart(perf.frames, budget));
  host.append(renderFrameTable(perf.frames, budget));
  host.append(renderFrameAnalysisBlock());
  renderFrameAnalysis();
  if (perf.selectedFrame !== null && !perf.snapshot && state.caps.frameAnalysis && perf.analysis.frame !== perf.selectedFrame && !perf.analysis.loading) {
    loadFrameAnalysis(perf.selectedFrame);
  }
}

function snapshotBanner() {
  const perf = state.performance;
  const snapshot = perf.snapshot;
  const source = snapshot?.target?.title || "";
  return el("div", { class: "snapshot-banner", attrs: { role: "status" } },
    icon("info"),
    el("span", {},
      el("strong", { text: snapshot.name }),
      ` · ${perf.frames.length} frames · ${perf.captures.length} capture${perf.captures.length === 1 ? "" : "s"}`,
      source ? ` from ${source}` : "",
      snapshot.exportedAt ? ` · exported ${snapshot.exportedAt}` : "",
    ),
    el("span", { class: "muted", text: "Imported files are read-only; frame records are shown exactly as exported." }),
    el("button", {
      class: "tool-button",
      props: { type: "button" },
      text: "Return to live",
      on: { click: returnToLive },
    }),
  );
}

function averageOf(values) {
  return values.length ? values.reduce((total, value) => total + value, 0) / values.length : null;
}

function percentileOf(values, fraction) {
  if (!values.length) return null;
  const sorted = [...values].sort((left, right) => left - right);
  const index = Math.min(sorted.length - 1, Math.max(0, Math.ceil(fraction * sorted.length) - 1));
  return sorted[index];
}

function renderFrameStats(frames, budget) {
  const recent = frames.slice(-60);
  const janks = recent.filter(frame => frameJank(frame, budget)).length;
  const uiValues = recent.map(frameUiMs).filter(value => value !== null);
  const totals = recent.map(frame => numberValue(frame.totalMs)).filter(value => value !== null);
  const latest = frames[frames.length - 1];
  const stats = [
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: `Jank · last ${recent.length}` }),
      el("div", { class: "stat-value" }, String(janks), el("small", { text: ` / ${recent.length} frames` }))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Average UI" }),
      el("div", { class: "stat-value" }, fmtNumber(averageOf(uiValues), 2), el("small", { text: " ms" }))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Average total" }),
      el("div", { class: "stat-value" }, fmtNumber(averageOf(totals), 2), el("small", { text: " ms" }))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "P95 total" }),
      el("div", { class: "stat-value" }, fmtNumber(percentileOf(totals, 0.95), 2), el("small", { text: " ms" }))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Latest" }),
      el("div", { class: "stat-value" }, fmtNumber(numberValue(latest.totalMs), 2), el("small", { text: " ms" }))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: `Budget · ${state.performance.budgetHz} Hz` }),
      el("div", { class: "stat-value" }, fmtNumber(budget, 1), el("small", { text: " ms" }))),
  ];
  return el("div", { class: "stat-row" }, ...stats);
}

function barPercent(value, scale) {
  return Math.max(1.2, Math.min(100, (value / scale) * 100));
}

function frameUiBar(parts, ui, scale) {
  const bar = el("div", { class: "frame-bar is-ui", attrs: { style: `height:${barPercent(ui, scale)}%` } });
  for (const [name, value] of [["recomposition", parts.recomposition], ["layout", parts.layout], ["draw", parts.draw]]) {
    if (value === null || value <= 0) continue;
    bar.append(el("span", { class: `frame-seg is-${name}`, attrs: { style: `flex:${value} 1 0` } }));
  }
  return bar;
}

function renderFrameChart(frames, budget) {
  const recent = frames.slice(-FRAME_WINDOW);
  const values = [];
  for (const frame of recent) {
    const parts = frameParts(frame);
    const ui = frameUiMs(frame);
    if (ui !== null) values.push(ui);
    if (parts.encode !== null) values.push(parts.encode);
    if (parts.total !== null) values.push(parts.total);
  }
  const scale = Math.max(values.reduce((maximum, value) => Math.max(maximum, value), 0), budget * 1.25, 1);
  const threshold = Math.max(0, Math.min(100, (budget / scale) * 100));
  const selected = state.performance.selectedFrame;
  const columns = el("div", { class: "frame-columns" });
  let focusIndex = selected === null ? recent.length - 1 : recent.findIndex(frame => frame.frame === selected);
  if (focusIndex < 0) focusIndex = recent.length - 1;
  for (const [index, frame] of recent.entries()) {
    const parts = frameParts(frame);
    const ui = frameUiMs(frame);
    const jank = frameJank(frame, budget);
    const isSelected = frame.frame === selected;
    const describe = `Frame ${frame.frame}: total ${fmtNumber(parts.total, 2)} ms, UI ${ui === null ? "not reported" : `${fmtNumber(ui, 2)} ms`}, encode ${parts.encode === null ? "not reported" : `${fmtNumber(parts.encode, 2)} ms`}${jank ? ", over budget" : ""}`;
    const column = el("div", {
      class: `frame-column${jank ? " is-jank" : ""}${isSelected ? " is-selected" : ""}`,
      attrs: {
        role: "option",
        "aria-selected": String(isSelected),
        tabindex: isSelected || index === focusIndex ? "0" : "-1",
        "aria-label": describe,
        title: describe,
        "data-frame": String(frame.frame),
      },
      on: { click: () => selectFrame(frame.frame) },
    }, el("div", { class: "frame-bars" }));
    const bars = column.firstElementChild;
    if (ui !== null) bars.append(frameUiBar(parts, ui, scale));
    if (parts.encode !== null) bars.append(el("div", { class: "frame-bar is-encode", attrs: { style: `height:${barPercent(parts.encode, scale)}%` } }));
    if (parts.total !== null) bars.append(el("div", { class: "frame-bar is-total", attrs: { style: `height:${barPercent(parts.total, scale)}%` } }));
    columns.append(column);
  }
  const plot = el("div", {
    class: "frame-plot",
    attrs: {
      role: "listbox",
      "aria-label": `Frame timings for the last ${recent.length} frames. Budget ${fmtNumber(budget, 1)} milliseconds.`,
    },
  }, columns);
  plot.addEventListener("keydown", event => {
    const current = event.target.closest?.(".frame-column");
    if (!current) return;
    const options = [...columns.children];
    const index = options.indexOf(current);
    let next = null;
    if (event.key === "ArrowRight") next = options[Math.min(options.length - 1, index + 1)];
    else if (event.key === "ArrowLeft") next = options[Math.max(0, index - 1)];
    else if (event.key === "Home") next = options[0];
    else if (event.key === "End") next = options[options.length - 1];
    else if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      selectFrame(Number(current.dataset.frame), { focus: true });
      return;
    }
    if (!next) return;
    event.preventDefault();
    selectFrame(Number(next.dataset.frame), { focus: true });
  });
  const missingRecomposition = recent.some(frame => frameParts(frame).recomposition === null);
  const missingEncode = recent.some(frame => frameParts(frame).encode === null);
  const notes = [];
  if (missingRecomposition) notes.push("Recomposition is not reported by this runtime; UI bars contain layout and draw only.");
  if (missingEncode) notes.push("Encode bars appear only for frames whose presentation the host observed.");
  if (recent.some(frame => frameParts(frame).gpu !== null)) {
    notes.push("GPU is the backend's measured execution time for the frame's submission. It overlaps the CPU phases, so it is not part of the UI or Encode bars and is not added to Total.");
  } else {
    notes.push("GPU execution is not reported for these frames; no estimate is substituted.");
  }
  return el("div", { class: "panel-block" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Frame chart" }),
      el("span", { class: "pane-meta", text: `last ${recent.length} of ${frames.length} · ${fmtNumber(scale, 1)} ms scale` }),
    ),
    el("div", { class: "panel-block-body" },
      el("div", { class: "frame-chart" },
        el("div", { class: "frame-chart-plot" },
          el("div", { class: "frame-threshold", attrs: { style: `bottom:${threshold}%` } }, el("span", { text: `${fmtNumber(budget, 1)} ms` })),
          plot,
        ),
      ),
      el("div", { class: "frame-axis" }, el("span", { text: "0 ms" }), el("span", { text: `${fmtNumber(scale, 1)} ms` })),
      el("div", { class: "perf-legend" },
        el("span", { class: "is-budget", text: `${fmtNumber(budget, 1)} ms budget` }),
        el("span", { class: "is-ui", text: "UI" }),
        el("span", { class: "is-encode", text: "Encode" }),
        el("span", { class: "is-total", text: "Total" }),
        el("span", { class: "is-jank", text: "Over budget" }),
      ),
      ...notes.map(note => el("div", { class: "perf-note", text: note })),
    ),
  );
}

function selectFrame(frameNumber, options = {}) {
  const perf = state.performance;
  if (!Number.isFinite(frameNumber)) return;
  perf.selectedFrame = frameNumber;
  updateFrameSelection();
  if (options.focus) {
    dom.performanceFrames.querySelector(`.frame-column[data-frame="${frameNumber}"]`)?.focus();
  }
  renderFrameAnalysis();
  if (!perf.snapshot && state.caps.frameAnalysis) loadFrameAnalysis(frameNumber);
}

function updateFrameSelection() {
  const selected = state.performance.selectedFrame;
  for (const node of dom.performanceFrames.querySelectorAll("[data-frame]")) {
    const isSelected = Number(node.dataset.frame) === selected;
    node.classList.toggle("is-selected", isSelected);
    if (node.getAttribute("role") === "option") {
      node.setAttribute("aria-selected", String(isSelected));
      node.tabIndex = isSelected ? 0 : -1;
    }
  }
}

function renderFrameTable(frames, budget) {
  const rows = frames.slice().reverse().slice(0, FRAME_TABLE_LIMIT);
  const selected = state.performance.selectedFrame;
  const header = el("tr", {},
    el("th", { class: "num", text: "Frame" }),
    el("th", { class: "num", text: "UI" }),
    el("th", { class: "num", text: "Encode" }),
    el("th", { class: "num", text: "GPU" }),
    el("th", { class: "num", text: "Total" }),
    el("th", { class: "num", text: "Recomp" }),
    el("th", { class: "num", text: "Layout" }),
    el("th", { class: "num", text: "Draw" }),
    el("th", { class: "num", text: "Nodes" }),
    el("th", { class: "num", text: "Batches" }),
    el("th", { text: "Submitted" }),
    el("th", { text: "Presented" }),
  );
  const body = el("tbody");
  for (const frame of rows) {
    const parts = frameParts(frame);
    const ui = frameUiMs(frame);
    const jank = frameJank(frame, budget);
    const isSelected = frame.frame === selected;
    body.append(el("tr", {
      class: `frame-row${isSelected ? " is-selected" : ""}${jank ? " is-jank" : ""}`,
      attrs: { "data-frame": String(frame.frame), title: `Frame ${frame.frame}` },
      on: { click: () => selectFrame(frame.frame) },
    },
      el("td", { class: "num mono-cell", text: `#${frame.frame}` }),
      el("td", { class: "num", text: ui === null ? "not reported" : fmtNumber(ui, 2) }),
      el("td", { class: "num", text: parts.encode === null ? "not reported" : fmtNumber(parts.encode, 2) }),
      el("td", { class: "num", text: parts.gpu === null ? "not reported" : fmtNumber(parts.gpu, 2) }),
      el("td", { class: "num", text: fmtNumber(parts.total, 2) }),
      el("td", { class: "num", text: parts.recomposition === null ? "not reported" : fmtNumber(parts.recomposition, 2) }),
      el("td", { class: "num", text: fmtNumber(parts.layout, 2) }),
      el("td", { class: "num", text: fmtNumber(parts.draw, 2) }),
      el("td", { class: "num", text: fmtNumber(numberValue(frame.nodeCount), 0) }),
      el("td", { class: "num", text: fmtNumber(numberValue(frame.batchCount), 0) }),
      el("td", { text: frame.submitted ? "yes" : "no" }),
      el("td", { text: frame.presented === null || frame.presented === undefined ? "not observed" : frame.presented ? "yes" : "no" }),
    ));
  }
  return el("div", { class: "panel-block" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Frames" }),
      el("span", { class: "pane-meta", text: `newest first · ${rows.length} of ${frames.length}` }),
    ),
    el("div", { class: "table-wrap" }, el("table", { class: "data-table frame-table" }, el("thead", {}, header), body)),
  );
}

function renderFrameAnalysisBlock() {
  return el("div", { class: "panel-block" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Frame analysis" }),
      el("span", { class: "pane-meta", attrs: { id: "frame-analysis-meta" } }),
    ),
    el("div", { class: "panel-block-body", attrs: { id: "frame-analysis-body" } }),
  );
}

function perfChip(text, tone) {
  return el("span", { class: `perf-chip is-${tone}`, text });
}

function frameAnalysisAttribution() {
  const perf = state.performance;
  const wrap = el("div", { class: "perf-attribution" });
  if (perf.snapshot) {
    wrap.append(emptyInline("Imported snapshot: per-span attribution was not part of this file. The exported TimingFrame records are shown above."));
    return wrap;
  }
  if (!state.caps.frameAnalysis) {
    wrap.append(emptyInline("Per-span attribution is unavailable: this target does not advertise frame_analysis. The runtime publishes TimingFrame fields without a span breakdown, so this panel can show only measured frame totals."));
    return wrap;
  }
  const analysis = perf.analysis;
  if (analysis.frame !== perf.selectedFrame) {
    wrap.append(emptyInline("Waiting for the runtime's frame analysis."));
    return wrap;
  }
  if (analysis.loading) {
    wrap.append(el("div", { class: "inline-spinner" }, el("span", { class: "spinner" }), "Reading frame attribution…"));
    return wrap;
  }
  if (analysis.error) {
    wrap.append(errorInline(errorMessage(analysis.error)));
    wrap.append(el("button", {
      class: "tool-button",
      props: { type: "button" },
      text: "Retry",
      on: { click: () => loadFrameAnalysis(perf.selectedFrame) },
    }));
    return wrap;
  }
  if (!analysis.data || typeof analysis.data !== "object") {
    wrap.append(emptyInline("The runtime returned no frame analysis."));
    return wrap;
  }
  const data = analysis.data;
  const status = data.status === "unavailable" ? "unavailable" : data.status === "ok" ? "ok" : null;
  const reason = typeof data.reason === "string" ? data.reason : "";
  const phases = Array.isArray(data.phases) ? data.phases : [];
  const spans = Array.isArray(data.spans) ? data.spans : [];
  const window = data.window && typeof data.window === "object" ? data.window : null;
  const chips = el("div", { class: "perf-verdicts" },
    status === "unavailable" ? perfChip("Attribution unavailable", "bad") : status === "ok" ? perfChip("Attributed from retained spans", "good") : null,
    data.dominantPhase ? perfChip(`Dominant: ${data.dominantPhase}`, "accent") : null,
    window ? perfChip(`Sequences ${textOf(window.startSequence)}–${textOf(window.endSequence)}`, "muted") : null,
  );
  if (chips.childElementCount) wrap.append(chips);
  if (status === "unavailable" || reason) {
    wrap.append(emptyInline(reason || "The runtime reported that this frame cannot be attributed to retained spans."));
  }
  if (phases.length) {
    const table = el("table", { class: "data-table" },
      el("thead", {}, el("tr", {},
        el("th", { text: "Phase" }),
        el("th", { class: "num", text: "Spans" }),
        el("th", { class: "num", text: "CPU" }),
        el("th", { class: "num", text: "Maximum" }),
        el("th", { class: "num", text: "Share" }),
      )),
      el("tbody", {}, ...phases.map(phase => {
        const share = numberValue(phase.share);
        return el("tr", {},
          el("td", { class: "mono-cell", text: textOf(phase.phase) }),
          el("td", { class: "num", text: fmtNumber(numberValue(phase.spanCount), 0) }),
          el("td", { class: "num", text: fmtMs(numberValue(phase.cpuMs), 2) }),
          el("td", { class: "num", text: fmtMs(numberValue(phase.maximumMs), 2) }),
          el("td", { class: "num", text: share === null ? "—" : `${(share * 100).toFixed(1)}%` }),
        );
      })),
    );
    wrap.append(el("div", { class: "table-wrap" }, table));
    wrap.append(el("div", { class: "perf-note", text: "Phase CPU time is summed from the retained spans in this frame's window; the chart's Encode bar is the host's render-thread measurement." }));
  }
  if (spans.length) {
    const table = el("table", { class: "data-table", attrs: { style: "margin-top:8px" } },
      el("thead", {}, el("tr", {},
        el("th", { class: "num", text: "Seq" }),
        el("th", { text: "Phase" }),
        el("th", { text: "Name" }),
        el("th", { class: "num", text: "Start" }),
        el("th", { class: "num", text: "Duration" }),
        el("th", { text: "Scope" }),
      )),
      el("tbody", {}, ...spans.map(span => {
        const scope = span.scopeID ?? span.scope_id ?? "";
        return el("tr", {},
          el("td", { class: "num mono-cell", text: String(numberValue(span.sequence) ?? "—") }),
          el("td", { class: "mono-cell", text: textOf(span.phase) }),
          el("td", { text: textOf(span.name) }),
          el("td", { class: "num", text: fmtMs(numberValue(span.startMs ?? span.start_ms), 2) }),
          el("td", { class: "num", text: fmtMs(numberValue(span.durationMs ?? span.duration_ms), 2) }),
          el("td", { class: "mono-cell", text: scope ? shortId(scope) : "—", attrs: { title: scope } }),
        );
      })),
    );
    wrap.append(el("div", { class: "table-wrap" }, table));
  }
  const notes = [];
  const spanCount = numberValue(data.spanCount);
  if (data.truncated) notes.push(`Only the longest ${spans.length} of ${spanCount ?? "?"} retained spans in this frame's window are listed.`);
  const unclassifiedCount = numberValue(data.unclassifiedSpanCount);
  if (unclassifiedCount) notes.push(`Frame-level work outside the five attributed phases: ${unclassifiedCount} span${unclassifiedCount === 1 ? "" : "s"}, ${fmtNumber(numberValue(data.unclassifiedCpuMs), 2)} ms.`);
  if (!spans.length && !phases.length && status !== "unavailable" && !reason) notes.push("The runtime returned no retained spans for this frame window.");
  if (notes.length) wrap.append(el("ul", { class: "perf-notes" }, ...notes.map(note => el("li", { text: note }))));
  wrap.append(rawDetails("Runtime payload", data));
  return wrap;
}

function renderFrameAnalysis() {
  const body = $("frame-analysis-body");
  const meta = $("frame-analysis-meta");
  if (!body || !meta) return;
  clear(body);
  const perf = state.performance;
  const frame = perf.frames.find(item => item.frame === perf.selectedFrame) || null;
  if (!frame) {
    meta.textContent = "";
    body.append(emptyInline("Select a frame bar or row to inspect its TimingFrame record."));
    return;
  }
  meta.textContent = `frame #${frame.frame}`;
  const budget = 1000 / perf.budgetHz;
  const parts = frameParts(frame);
  const ui = frameUiMs(frame);
  const jank = frameJank(frame, budget);
  const verdicts = el("div", { class: "perf-verdicts" },
    perfChip(jank ? "Over budget" : "Within budget", jank ? "bad" : "good"),
    parts.recomposition === null ? perfChip("Recomposition not reported", "muted") : null,
    parts.encode === null ? perfChip("Encode not observed", "muted") : null,
  );
  body.append(verdicts);
  body.append(el("div", { class: "metrics" },
    metric("Total", fmtNumber(parts.total, 2), "ms"),
    metric("UI", ui === null ? "Not reported" : fmtNumber(ui, 2), ui === null ? "" : "ms"),
    metric("Recomposition", parts.recomposition === null ? "Not reported" : fmtNumber(parts.recomposition, 2), parts.recomposition === null ? "" : "ms"),
    metric("Layout", fmtNumber(parts.layout, 2), "ms"),
    metric("Draw", fmtNumber(parts.draw, 2), "ms"),
    metric("Encode", parts.encode === null ? "Not reported" : fmtNumber(parts.encode, 2), parts.encode === null ? "" : "ms"),
    metric("GPU", parts.gpu === null ? "Not reported" : fmtNumber(parts.gpu, 2), parts.gpu === null ? "" : "ms"),
    metric("Nodes", fmtNumber(numberValue(frame.nodeCount), 0)),
    metric("Batches", fmtNumber(numberValue(frame.batchCount), 0)),
  ));
  const windowStart = numberValue(frame.timelineStart);
  const windowEnd = numberValue(frame.timelineEnd);
  const timelineWindow = windowStart === null || windowEnd === null || (windowStart === 0 && windowEnd === 0)
    ? "not reported"
    : `${windowStart}–${windowEnd}`;
  body.append(kvList([
    ["Submitted", frame.submitted ? "yes" : "no"],
    ["Presented", frame.presented === null || frame.presented === undefined ? "not observed by the host" : frame.presented ? "yes" : "no"],
    ["Frame sequence", String(frame.frame)],
    ["Timeline window (seq)", timelineWindow],
  ]));
  const verdict = el("div", { class: "perf-note" });
  if (jank) {
    const reasons = [];
    if (ui !== null && ui > budget) reasons.push(`UI ${fmtNumber(ui, 2)} ms exceeds ${fmtNumber(budget, 1)} ms by ${fmtNumber(ui - budget, 2)} ms`);
    if (parts.encode !== null && parts.encode > budget) reasons.push(`encode ${fmtNumber(parts.encode, 2)} ms exceeds ${fmtNumber(budget, 1)} ms by ${fmtNumber(parts.encode - budget, 2)} ms`);
    verdict.textContent = `${reasons.join("; ")}.`;
  } else if (parts.encode === null) {
    verdict.textContent = `UI is within the ${fmtNumber(budget, 1)} ms budget. Encode time was not observed, so raster jank cannot be assessed.`;
  } else if (numberValue(parts.total) !== null && parts.total > budget) {
    verdict.textContent = `UI and encode are within the ${fmtNumber(budget, 1)} ms budget. Total is ${fmtNumber(parts.total, 2)} ms; the runtime measures it separately, so it also covers work outside the UI and encode sums.`;
  } else {
    verdict.textContent = `UI and encode are within the ${fmtNumber(budget, 1)} ms budget.`;
  }
  body.append(verdict);
  body.append(el("div", { class: "section", attrs: { style: "margin-top:10px" } },
    el("h3", { text: "Attribution" }),
    frameAnalysisAttribution(),
  ));
  const hasWindow = windowStart !== null && windowEnd !== null && !(windowStart === 0 && windowEnd === 0);
  let correlation;
  if (perf.snapshot) {
    correlation = hasWindow
      ? `This snapshot reports timeline sequences ${windowStart}–${windowEnd}; imported frames have no live timeline to attribute.`
      : "The imported frame has no timeline window, so it cannot be attributed to spans.";
  } else if (!state.caps.frameAnalysis) {
    correlation = hasWindow
      ? `This frame reports timeline sequences ${windowStart}–${windowEnd}, but this target does not advertise frame_analysis.`
      : "This TimingFrame has no timeline window or wall-clock timestamp, so it cannot be aligned with trace spans.";
  } else {
    correlation = hasWindow
      ? `Attribution uses the retained spans in timeline sequences ${windowStart}–${windowEnd}. TimingFrame records carry no wall-clock timestamp, so they are not aligned to wall-clock time.`
      : "The runtime reports no timeline window for this frame, so retained spans cannot be attributed to it.";
  }
  body.append(el("div", { class: "perf-note", text: correlation }));
}

async function loadFrameAnalysis(frameNumber) {
  const perf = state.performance;
  if (perf.snapshot || !state.caps.frameAnalysis || !state.targetId) return;
  const frame = perf.frames.find(item => item.frame === frameNumber);
  if (!frame) return;
  const key = `${frameNumber}:${numberValue(frame.totalMs) ?? "?"}`;
  const cached = perf.analysisCache.get(key);
  if (cached) {
    perf.analysis = { frame: frameNumber, loading: false, error: null, data: cached, request: perf.analysis.request };
    renderFrameAnalysis();
    return;
  }
  const targetId = state.targetId;
  const request = perf.analysis.request + 1;
  perf.analysis = { frame: frameNumber, loading: true, error: null, data: null, request };
  renderFrameAnalysis();
  try {
    const data = await rpc("analyze_ui_frame", { target: targetId, frame: frameNumber });
    if (request !== state.performance.analysis.request || targetId !== state.targetId) return;
    state.performance.analysis = { frame: frameNumber, loading: false, error: null, data, request };
    state.performance.analysisCache.set(key, data);
  } catch (error) {
    if (request !== state.performance.analysis.request || targetId !== state.targetId) return;
    state.performance.analysis = { frame: frameNumber, loading: false, error, data: null, request };
  } finally {
    renderFrameAnalysis();
  }
}

/* Timeline */

function updateTraceElapsed() {
  const perf = state.performance;
  if (!perf.recording) {
    dom.performanceElapsed.hidden = true;
    return;
  }
  const seconds = Math.max(0, (Date.now() - perf.startedAt) / 1000);
  dom.performanceElapsed.textContent = `Recording ${seconds.toFixed(1)} s`;
  dom.performanceElapsed.hidden = false;
}

async function startTrace() {
  const perf = state.performance;
  if (perf.snapshot) {
    toast("Return to live data before recording a trace.", "info");
    return;
  }
  if (!state.caps.trace) {
    toast("This runtime does not advertise trace capture.", "error");
    return;
  }
  if (perf.recording || perf.traceLoading) return;
  const targetId = state.targetId;
  perf.traceLoading = true;
  perf.traceError = null;
  renderPerformance();
  try {
    await rpc("start_ui_trace", { target: targetId });
    if (targetId !== state.targetId) return;
    perf.recording = true;
    perf.startedAt = Date.now();
    perf.selectedSpan = null;
    perf.analysisCache.clear();
    perf.analysis = { frame: null, loading: false, error: null, data: null, request: perf.analysis.request + 1 };
    perf.recordTimer = setInterval(updateTraceElapsed, 250);
    toast("Recording CPU spans. Stop when the operation is complete.", "success", 5000);
  } catch (error) {
    if (targetId !== state.targetId) return;
    perf.traceError = error;
    toast(errorMessage(error), "error", 9000);
  } finally {
    if (targetId === state.targetId) {
      perf.traceLoading = false;
      renderPerformance();
    }
  }
}

async function stopTrace(options = {}) {
  const quiet = options.quiet === true;
  const perf = state.performance;
  if (!perf.recording) {
    if (!quiet) toast("No trace is recording.", "info");
    return;
  }
  const targetId = state.targetId;
  perf.traceLoading = true;
  renderPerformance();
  try {
    const summary = await rpc("stop_ui_trace", { target: targetId });
    if (targetId !== state.targetId) return;
    perf.recording = false;
    if (perf.recordTimer) clearInterval(perf.recordTimer);
    perf.recordTimer = null;
    if (!summary || typeof summary !== "object" || !summary.id) {
      throw new RpcError("The runtime returned no capture summary.", "invalid_response");
    }
    perf.traceError = null;
    perf.comparison = null;
    perf.tab = "timeline";
    perf.analysisCache.clear();
    perf.analysis = { frame: null, loading: false, error: null, data: null, request: perf.analysis.request + 1 };
    addTraceCapture(summary);
    const count = numberValue(summary.eventCount) ?? 0;
    if (!quiet) {
      toast(count ? `Trace ${summary.id} stopped with ${count} spans.` : `Trace ${summary.id} stopped with no spans.`, count ? "success" : "info", 6000);
    }
    await loadTraceEvents({ captureId: summary.id, quiet: true });
  } catch (error) {
    if (targetId !== state.targetId) return;
    perf.traceError = error;
    if (!quiet) toast(errorMessage(error), "error", 9000);
  } finally {
    if (targetId === state.targetId) {
      perf.traceLoading = false;
      renderPerformance();
    }
  }
}

function addTraceCapture(summary) {
  if (!summary?.id) return;
  const perf = state.performance;
  let record = captureRecordById(summary.id);
  if (!record) {
    record = {
      summary,
      events: [],
      total: numberValue(summary.eventCount) ?? 0,
      nextOffset: null,
      complete: false,
      imported: false,
      loading: false,
    };
    perf.captures.push(record);
  } else {
    record.summary = summary;
  }
  if (perf.captures.length > 8) {
    const index = perf.captures.findIndex(item => item !== record);
    if (index >= 0) perf.captures.splice(index, 1);
  }
  perf.activeCaptureId = summary.id;
  const ids = perf.captures.map(item => item.summary?.id).filter(Boolean);
  if (!perf.baseline || !ids.includes(perf.baseline)) perf.baseline = ids[0] || "";
  perf.candidate = summary.id;
}

function updateCaptureSummary(summary) {
  if (!summary?.id) return;
  const record = captureRecordById(summary.id);
  if (record) record.summary = summary;
}

async function fetchTracePage(record, options = {}) {
  const perf = state.performance;
  if (!record || !record.summary?.id || record.imported || record.loading || record.complete || !state.targetId) return false;
  const targetId = state.targetId;
  const offset = numberValue(record.nextOffset) ?? 0;
  record.loading = true;
  perf.eventLoading = true;
  perf.eventError = null;
  if (options.silent !== true) renderPerformance();
  try {
    const data = await rpc("get_ui_trace", { target: targetId, capture_id: record.summary?.id, offset, limit: 100 });
    if (targetId !== state.targetId) return false;
    if (data?.capture) updateCaptureSummary(data.capture);
    const page = data?.events || {};
    const items = Array.isArray(page.items) ? page.items : [];
    record.events = offset === 0 ? items : [...record.events, ...items];
    record.total = numberValue(page.total) ?? record.events.length;
    record.nextOffset = numberValue(page.nextOffset);
    record.complete = record.nextOffset === null || record.events.length >= record.total || record.events.length >= MAX_SNAPSHOT_EVENTS;
    return true;
  } catch (error) {
    if (targetId !== state.targetId) return false;
    perf.eventError = error;
    if (!options.quiet) toast(errorMessage(error), "error", 8000);
    return false;
  } finally {
    record.loading = false;
    if (targetId === state.targetId) {
      perf.eventLoading = false;
      if (options.silent !== true) renderPerformance();
    }
  }
}

async function loadTraceEvents(options = {}) {
  const record = options.captureId ? captureRecordById(options.captureId) : activeCaptureRecord();
  if (!record) return;
  if (record.loading || record.complete || record.imported) {
    renderPerformance();
    return;
  }
  await fetchTracePage(record, options);
}

function selectCapture(id) {
  const perf = state.performance;
  const record = captureRecordById(id);
  if (!record) return;
  if (perf.activeCaptureId !== id) {
    perf.activeCaptureId = id;
    perf.selectedSpan = null;
    perf.comparison = null;
    perf.zoom = 1;
    perf.eventError = null;
  }
  renderPerformance();
  if (!record.imported && !record.complete && !record.events.length) loadTraceEvents({ captureId: id, quiet: true });
}

function tracePhaseValue(phase, key) {
  return numberValue(phase?.[key]) ?? numberValue(phase?.[key.replace(/Ms$/, "_ms")]);
}

function localTraceComparison(baseline, candidate) {
  const phases = new Map();
  for (const phase of Array.isArray(baseline?.phases) ? baseline.phases : []) {
    if (phase?.phase) phases.set(phase.phase, { phase: phase.phase, baseline: phase, candidate: null });
  }
  for (const phase of Array.isArray(candidate?.phases) ? candidate.phases : []) {
    if (!phase?.phase) continue;
    const entry = phases.get(phase.phase) || { phase: phase.phase, baseline: null, candidate: null };
    entry.candidate = phase;
    phases.set(phase.phase, entry);
  }
  const changes = [...phases.values()].sort((left, right) => left.phase.localeCompare(right.phase)).map(entry => {
    const baselineAverage = tracePhaseValue(entry.baseline, "averageMs");
    const candidateAverage = tracePhaseValue(entry.candidate, "averageMs");
    const baselineP95 = tracePhaseValue(entry.baseline, "p95Ms");
    const candidateP95 = tracePhaseValue(entry.candidate, "p95Ms");
    return {
      phase: entry.phase,
      baseline: entry.baseline,
      candidate: entry.candidate,
      averageDeltaMs: baselineAverage === null || candidateAverage === null ? null : candidateAverage - baselineAverage,
      p95DeltaMs: baselineP95 === null || candidateP95 === null ? null : candidateP95 - baselineP95,
    };
  });
  return { baseline, candidate, changes, source: "summaries" };
}

async function compareTraces() {
  const perf = state.performance;
  if (perf.comparing) return;
  const baselineId = $("trace-baseline")?.value || perf.baseline;
  const candidateId = $("trace-candidate")?.value || perf.candidate;
  if (!baselineId || !candidateId) {
    toast("Record two traces to compare them.", "info");
    return;
  }
  if (baselineId === candidateId) {
    toast("Choose two different traces.", "info");
    return;
  }
  const baseline = captureRecordById(baselineId);
  const candidate = captureRecordById(candidateId);
  if (!baseline || !candidate) return;
  perf.baseline = baselineId;
  perf.candidate = candidateId;
  if (!baseline.imported && !candidate.imported && state.targetId) {
    const targetId = state.targetId;
    perf.comparing = true;
    perf.comparison = null;
    perf.traceError = null;
    renderPerformance();
    try {
      const data = await rpc("compare_ui_traces", { target: targetId, baseline_id: baselineId, candidate_id: candidateId });
      if (targetId !== state.targetId) return;
      perf.comparison = data && typeof data === "object" ? { ...data, source: "runtime" } : null;
    } catch (error) {
      if (targetId !== state.targetId) return;
      perf.traceError = error;
      toast(errorMessage(error), "error", 9000);
    } finally {
      if (targetId === state.targetId) {
        perf.comparing = false;
        renderPerformance();
      }
    }
    return;
  }
  perf.comparison = localTraceComparison(baseline.summary, candidate.summary);
  renderPerformance();
}

function renderTimelinePanel() {
  const host = dom.performanceTimeline;
  clear(host);
  const perf = state.performance;
  const target = targetById();
  if (perf.snapshot) {
    host.append(snapshotBanner());
    if (!perf.captures.length) {
      const block = el("div");
      showState(block, {
        icon: "trace",
        title: "No trace captures in this snapshot",
        message: "The imported file contains no trace captures. Frame records are available on the Frames tab.",
        action: "Open frames",
        onAction: () => setPerformanceTab("frames"),
      });
      host.append(block);
      return;
    }
  } else if (!target) {
    showState(host, { icon: "device", title: "No target", message: "Select a connected runtime to record CPU spans." });
    return;
  } else if (!state.caps.runtime) {
    showState(host, {
      icon: "trace",
      title: "Trace is unavailable",
      message: "Trace capture comes from a Cove runtime. Select a runtime target.",
    });
    return;
  } else if (!state.caps.trace) {
    showState(host, {
      icon: "trace",
      title: "Trace is unavailable",
      message: "This runtime does not advertise trace capture.",
    });
    return;
  }
  if (perf.recording) {
    host.append(el("div", { class: "panel-block" },
      el("div", { class: "panel-block-head" },
        el("h2", { text: "Recording" }),
        el("span", { class: "recording-label", text: perf.traceLoading ? "Stopping…" : "Live" }),
      ),
      el("div", { class: "panel-block-body" },
        el("p", { text: "Perform the real operation in the app, then stop the trace to read its spans. Spans come from the runtime probe and do not require Screen Recording." }),
      ),
    ));
    if (perf.traceError) host.append(errorInline(errorMessage(perf.traceError)));
    return;
  }
  if (perf.traceLoading && !perf.captures.length) {
    showState(host, { kind: "loading", title: "Reading trace", message: "Waiting for the runtime to finish the capture." });
    return;
  }
  const record = activeCaptureRecord();
  if (!record) {
    const block = el("div");
    showState(block, {
      icon: "trace",
      title: "No trace capture",
      message: "Start a trace, perform a real operation, then stop it to read CPU spans. Spans come from the runtime probe and do not require Screen Recording.",
      action: "Start trace",
      onAction: startTrace,
    });
    host.append(block);
    if (perf.traceError) host.append(errorInline(errorMessage(perf.traceError)));
    return;
  }
  if (perf.traceError) host.append(errorInline(errorMessage(perf.traceError)));
  host.append(renderCaptureToolbar(record));
  host.append(el("div", { class: "stat-row" },
    el("div", { class: "stat" }, el("div", { class: "stat-label", text: "Duration" }), el("div", { class: "stat-value" }, fmtNumber(numberValue(record.summary?.durationMs), 1), el("small", { text: " ms" }))),
    el("div", { class: "stat" }, el("div", { class: "stat-label", text: "Spans" }), el("div", { class: "stat-value" }, String(numberValue(record.summary?.eventCount) ?? record.events.length))),
    el("div", { class: "stat" }, el("div", { class: "stat-label", text: "Loaded" }), el("div", { class: "stat-value" }, String(record.events.length))),
    el("div", { class: "stat" }, el("div", { class: "stat-label", text: "Dropped" }), el("div", { class: "stat-value" }, String(numberValue(record.summary?.dropped) ?? 0))),
    el("div", { class: "stat" }, el("div", { class: "stat-label", text: "Phases" }), el("div", { class: "stat-value" }, String(Array.isArray(record.summary?.phases) ? record.summary.phases.length : 0))),
  ));
  host.append(el("div", { class: "panel-block" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Phases" }),
      el("span", { class: "pane-meta", text: record.summary?.id || "" }),
    ),
    el("div", { class: "panel-block-body" }, renderTracePhases(record.summary)),
  ));
  host.append(renderFlameBlock(record));
  host.append(renderTraceEventsBlock(record));
  host.append(renderTraceCompareBlock());
  renderSpanDetail();
}

function renderCaptureToolbar(record) {
  const perf = state.performance;
  const select = el("select", {
    props: { id: "performance-capture" },
    attrs: { "aria-label": "Trace capture" },
    on: { change: () => selectCapture(select.value) },
  }, ...perf.captures.map(item => new Option(
    `${item.summary?.id || "capture"} · ${numberValue(item.summary?.eventCount) ?? item.events.length} spans${item.imported ? " · imported" : ""}`,
    item.summary?.id || "",
  )));
  select.value = perf.activeCaptureId;
  const toolbar = el("div", { class: "perf-toolbar" },
    el("label", { class: "inline-field", attrs: { for: "performance-capture" }, text: "Capture" }),
    select,
    el("span", { class: "pane-meta", text: record.imported ? "Imported snapshot capture" : record.complete ? "All spans loaded" : `${record.events.length} of ${record.total || "?"} spans loaded` }),
    record.imported ? null : el("button", {
      class: "tool-button",
      props: { type: "button", disabled: record.loading },
      text: "Reload capture",
      on: { click: () => reloadCapture(record) },
    }),
  );
  return toolbar;
}

function reloadCapture(record) {
  if (!record || record.imported || record.loading) return;
  record.events = [];
  record.nextOffset = null;
  record.complete = false;
  record.total = numberValue(record.summary?.eventCount) ?? 0;
  renderPerformance();
  loadTraceEvents({ captureId: record.summary?.id, quiet: true });
}

function renderTracePhases(summary) {
  const phases = Array.isArray(summary?.phases) ? summary.phases : [];
  if (!phases.length) return emptyInline("No phase summaries were published for this capture.");
  return el("div", { class: "phases" },
    el("div", { class: "phase-row is-head" },
      el("span", { text: "Phase" }), el("span", { text: "Count" }), el("span", { text: "Average" }),
      el("span", { text: "P95" }), el("span", { text: "Maximum" }),
    ),
    ...phases.map(phase => el("div", { class: "phase-row" },
      el("span", { class: "phase-name", text: phase.phase || "—" }),
      el("span", { text: String(phase.count ?? "—") }),
      el("span", { text: fmtMs(tracePhaseValue(phase, "averageMs"), 3) }),
      el("span", { text: fmtMs(tracePhaseValue(phase, "p95Ms"), 3) }),
      el("span", { text: fmtMs(tracePhaseValue(phase, "maximumMs"), 3) }),
    )),
  );
}

function emptySpanMessage(record) {
  if (record?.imported) return "The imported file contains no spans for this capture.";
  return record?.summary?.eventCount ? "No events were returned for this page." : "The capture contains no CPU spans. The runtime may have been idle.";
}

function buildFlameModel(events) {
  const parsed = [];
  for (const event of events) {
    const start = numberValue(event.start_ms ?? event.startMs);
    if (start === null) continue;
    const duration = Math.max(0, numberValue(event.duration_ms ?? event.durationMs) ?? 0);
    parsed.push({
      sequence: numberValue(event.sequence) ?? 0,
      phase: String(event.phase || "unphased"),
      name: String(event.name || ""),
      scope: event.scope_id ?? event.scopeID ?? "",
      start,
      duration,
      end: start + duration,
    });
  }
  if (!parsed.length) return null;
  const start = parsed.reduce((minimum, item) => Math.min(minimum, item.start), parsed[0].start);
  const end = parsed.reduce((maximum, item) => Math.max(maximum, item.end), start + 0.001);
  const span = Math.max(end - start, 0.001);
  const tracks = new Map();
  const ordered = [...parsed].sort((left, right) => left.start - right.start || right.duration - left.duration || left.sequence - right.sequence);
  for (const item of ordered) {
    let track = tracks.get(item.phase);
    if (!track) {
      track = { phase: item.phase, lanes: [], items: [] };
      tracks.set(item.phase, track);
    }
    track.items.push(item);
    let lane = track.lanes.find(candidate => candidate.end <= item.start + 0.0005);
    if (!lane) {
      lane = { end: item.end, items: [] };
      track.lanes.push(lane);
    }
    lane.end = Math.max(lane.end, item.end);
    lane.items.push(item);
  }
  return { start, end, span, tracks: [...tracks.values()], count: parsed.length };
}

function renderFlameBlock(record) {
  const perf = state.performance;
  const events = record.events;
  const head = el("div", { class: "panel-block-head" },
    el("h2", { text: "Timeline flame chart" }),
    el("span", { class: "pane-meta", text: `${events.length} spans · ${record.complete ? "complete" : "partial"}` }),
  );
  if (perf.eventLoading && !events.length) {
    return el("div", { class: "panel-block" }, head,
      el("div", { class: "panel-block-body" }, el("div", { class: "inline-spinner" }, el("span", { class: "spinner" }), "Reading trace events…")));
  }
  if (perf.eventError && !events.length) {
    return el("div", { class: "panel-block" }, head,
      el("div", { class: "panel-block-body" },
        errorInline(errorMessage(perf.eventError)),
        el("button", { class: "tool-button", props: { type: "button" }, text: "Retry", on: { click: () => loadTraceEvents({ captureId: record.summary?.id }) } })));
  }
  if (!events.length) {
    return el("div", { class: "panel-block" }, head,
      el("div", { class: "panel-block-body" }, emptyInline(emptySpanMessage(record))));
  }
  const model = buildFlameModel(events);
  if (!model) {
    return el("div", { class: "panel-block" }, head,
      el("div", { class: "panel-block-body" }, emptyInline("The returned spans have no finite start times to plot.")));
  }
  const zoom = perf.zoom;
  const controls = el("div", { class: "flame-controls" },
    el("span", { class: "inline-field", text: "Zoom" }),
    ...[1, 2, 4, 8].map(level => el("button", {
      class: `tool-button small${zoom === level ? " is-active" : ""}`,
      props: { type: "button", "aria-pressed": zoom === level },
      text: `${level}×`,
      on: { click: () => { perf.zoom = level; renderPerformance(); } },
    })),
    el("span", { class: "pane-meta", text: `${fmtNumber(model.span, 1)} ms window · ${model.tracks.length} phases` }),
  );
  const plot = el("div", { class: "flame-plot", attrs: { style: `width:${zoom * 100}%` } });
  const axisRow = el("div", { class: "flame-row is-axis" },
    el("div", { class: "flame-track-label", text: "Time" }),
    el("div", { class: "flame-track-lanes" }),
  );
  const axisLanes = axisRow.lastElementChild;
  for (let index = 0; index <= 4; index += 1) {
    const fraction = index / 4;
    const label = index === 0 ? "0 ms" : index === 4 ? `${fmtNumber(model.span, 1)} ms` : fmtNumber(model.span * fraction, 1);
    axisLanes.append(el("span", { class: "flame-tick", attrs: { style: `left:${fraction * 100}%` }, text: label }));
  }
  plot.append(axisRow);
  let rendered = 0;
  let hidden = 0;
  let hiddenLanes = 0;
  for (const track of model.tracks) {
    const lanesToDraw = track.lanes.slice(0, 16);
    hiddenLanes += Math.max(0, track.lanes.length - lanesToDraw.length);
    const laneHeight = 16;
    const row = el("div", { class: "flame-row" },
      el("div", { class: "flame-track-label" },
        el("span", { class: "flame-phase", text: track.phase }),
        el("span", { class: "flame-count", text: `${track.items.length} span${track.items.length === 1 ? "" : "s"}` }),
      ),
      el("div", { class: "flame-track-lanes", attrs: { style: `height:${Math.max(1, lanesToDraw.length) * laneHeight}px` } }),
    );
    const laneHost = row.lastElementChild;
    for (const [laneIndex, lane] of lanesToDraw.entries()) {
      const laneNode = el("div", { class: "flame-lane", attrs: { style: `top:${laneIndex * laneHeight}px;height:${laneHeight}px` } });
      for (const item of lane.items) {
        if (rendered >= MAX_FLAME_BARS) {
          hidden += 1;
          continue;
        }
        rendered += 1;
        const left = ((item.start - model.start) / model.span) * 100;
        const width = Math.max(0.15, (item.duration / model.span) * 100);
        const isSelected = perf.selectedSpan === item.sequence;
        const describe = `${item.phase}${item.name ? ` · ${item.name}` : ""} · ${fmtNumber(item.duration, 2)} ms · starts +${fmtNumber(item.start - model.start, 2)} ms${item.scope ? ` · scope ${item.scope}` : ""}`;
        laneNode.append(el("div", {
          class: `flame-bar${isSelected ? " is-selected" : ""}`,
          attrs: {
            style: `left:${left}%;width:${width}%`,
            title: describe,
            "data-sequence": String(item.sequence),
          },
          on: { click: () => selectSpan(item.sequence) },
        }));
      }
      laneHost.append(laneNode);
    }
    plot.append(row);
  }
  const notes = [];
  if (hiddenLanes) notes.push(`${hiddenLanes} lane${hiddenLanes === 1 ? " is" : "s are"} not drawn because a track has more than 16 overlapping spans.`);
  if (hidden) notes.push(`${hidden} more span${hidden === 1 ? "" : "s"} are not drawn; the spans table below still lists every loaded span.`);
  if (!record.complete) {
    notes.push(record.imported
      ? `The imported file contains ${events.length} of ${record.total || "?"} spans for this capture; the rest were not exported.`
      : `The timeline plots the ${events.length} loaded spans of ${record.total || "?"} in this capture. Load more spans to extend it.`);
  }
  return el("div", { class: "panel-block" },
    head,
    el("div", { class: "panel-block-body" },
      controls,
      el("div", {
        class: "flame-scroll",
        attrs: {
          role: "img",
          "aria-label": `Timeline flame chart with ${events.length} spans across ${model.tracks.length} phases. The spans table below lists every loaded span.`,
        },
      }, plot),
      notes.length ? el("div", { class: "perf-note" }, notes.join(" ")) : null,
      el("div", { class: "perf-note", text: "Bars are pointer-selectable; the spans table provides keyboard selection. The chart window starts at the earliest loaded span; the table's Start column is relative to the capture's timeline origin." }),
      el("div", { class: "span-detail-host", attrs: { id: "span-detail" } }),
    ),
  );
}

function renderTraceEventsBlock(record) {
  const perf = state.performance;
  const events = record.events;
  const body = el("div", { class: "panel-block-body" });
  const selected = perf.selectedSpan;
  if (perf.eventLoading && !events.length) {
    body.append(el("div", { class: "inline-spinner" }, el("span", { class: "spinner" }), "Reading trace events…"));
  } else if (perf.eventError && !events.length) {
    body.append(errorInline(errorMessage(perf.eventError)));
  } else if (!events.length) {
    body.append(emptyInline(emptySpanMessage(record)));
  } else {
    const rows = events.slice(0, SPAN_TABLE_LIMIT).map((event, index) => {
      const sequence = numberValue(event.sequence) ?? 0;
      const scope = event.scope_id ?? event.scopeID ?? "";
      const isSelected = sequence === selected;
      return el("tr", {
        class: isSelected ? "is-selected" : "",
        attrs: { "data-sequence": String(sequence), tabindex: isSelected || (selected === null && index === 0) ? "0" : "-1" },
        on: { click: () => selectSpan(sequence) },
      },
        el("td", { class: "num mono-cell", text: String(sequence) }),
        el("td", { class: "mono-cell", text: event.phase || "—" }),
        el("td", { text: event.name || "—" }),
        el("td", { class: "num", text: fmtMs(event.start_ms ?? event.startMs, 2) }),
        el("td", { class: "num", text: fmtMs(event.duration_ms ?? event.durationMs, 2) }),
        el("td", { class: "mono-cell", text: scope ? shortId(scope) : "—", attrs: { title: scope } }),
      );
    });
    const tbody = el("tbody", {}, ...rows);
    tbody.addEventListener("keydown", event => {
      const row = event.target.closest("tr[data-sequence]");
      if (!row) return;
      const list = [...tbody.querySelectorAll("tr[data-sequence]")];
      const index = list.indexOf(row);
      let next = null;
      if (event.key === "ArrowDown") next = list[Math.min(list.length - 1, index + 1)];
      else if (event.key === "ArrowUp") next = list[Math.max(0, index - 1)];
      else if (event.key === "Home") next = list[0];
      else if (event.key === "End") next = list[list.length - 1];
      else if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        selectSpan(Number(row.dataset.sequence), { focus: true });
        return;
      }
      if (!next) return;
      event.preventDefault();
      selectSpan(Number(next.dataset.sequence), { focus: true });
    });
    body.append(el("div", { class: "table-wrap spans-table-wrap" }, el("table", { class: "data-table" },
      el("thead", {}, el("tr", {},
        el("th", { class: "num", text: "Seq" }),
        el("th", { text: "Phase" }),
        el("th", { text: "Name" }),
        el("th", { class: "num", text: "Start" }),
        el("th", { class: "num", text: "Duration" }),
        el("th", { text: "Scope" }),
      )),
      tbody,
    )));
  }
  const pager = el("div", { class: "trace-pager" },
    el("span", { text: `${events.length} of ${record.total || events.length} spans` }),
  );
  if (events.length > SPAN_TABLE_LIMIT) pager.append(el("span", { class: "muted", text: `table shows the first ${SPAN_TABLE_LIMIT} of ${events.length} loaded spans` }));
  if (perf.eventLoading) pager.append(el("span", { class: "inline-spinner" }, el("span", { class: "spinner" }), "Loading…"));
  if (!record.complete && !record.imported && !perf.eventLoading) {
    pager.append(el("button", {
      class: "tool-button",
      props: { type: "button" },
      text: "Load more",
      on: { click: () => loadTraceEvents({ captureId: record.summary?.id, quiet: true }) },
    }));
  } else if (record.imported && !record.complete) {
    pager.append(el("span", { class: "muted", text: "The imported file contains only the loaded spans." }));
  } else if (record.complete && events.length > 0) {
    pager.append(el("span", { text: "End of capture" }));
  }
  if (perf.eventError) pager.append(el("span", { class: "muted", text: errorMessage(perf.eventError) }));
  body.append(pager);
  const selectedEvent = events.find(event => (numberValue(event.sequence) ?? 0) === perf.selectedSpan) || null;
  const meta = selectedEvent
    ? `#${selectedEvent.sequence} · ${selectedEvent.phase || "span"}${selectedEvent.name ? ` · ${selectedEvent.name}` : ""}`
    : record.summary?.id || "";
  return el("div", { class: "panel-block" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Spans" }),
      el("span", { class: "pane-meta", text: meta }),
    ),
    body,
  );
}

function selectSpan(sequence, options = {}) {
  const perf = state.performance;
  if (!Number.isFinite(sequence)) return;
  perf.selectedSpan = sequence;
  updateSpanSelection();
  renderSpanDetail();
  if (options.focus) {
    dom.performanceTimeline.querySelector(`tr[data-sequence="${sequence}"]`)?.focus();
  }
  const bar = dom.performanceTimeline.querySelector(`.flame-bar[data-sequence="${perf.selectedSpan}"]`);
  const scroller = bar?.closest(".flame-scroll");
  if (bar && scroller) {
    const barRect = bar.getBoundingClientRect();
    const scrollRect = scroller.getBoundingClientRect();
    if (barRect.left < scrollRect.left) scroller.scrollLeft += barRect.left - scrollRect.left - 12;
    else if (barRect.right > scrollRect.right) scroller.scrollLeft += barRect.right - scrollRect.right + 12;
    if (barRect.top < scrollRect.top) scroller.scrollTop += barRect.top - scrollRect.top - 12;
    else if (barRect.bottom > scrollRect.bottom) scroller.scrollTop += barRect.bottom - scrollRect.bottom + 12;
  }
}

function updateSpanSelection() {
  const selected = state.performance.selectedSpan;
  for (const node of dom.performanceTimeline.querySelectorAll("[data-sequence]")) {
    const isSelected = Number(node.dataset.sequence) === selected;
    node.classList.toggle("is-selected", isSelected);
    if (node.tagName === "TR") node.tabIndex = isSelected ? 0 : -1;
  }
}

function renderSpanDetail() {
  const host = $("span-detail");
  if (!host) return;
  clear(host);
  const perf = state.performance;
  const record = activeCaptureRecord();
  const event = record?.events.find(item => (numberValue(item.sequence) ?? 0) === perf.selectedSpan) || null;
  if (!event) return;
  const scope = event.scope_id ?? event.scopeID ?? "";
  host.append(el("div", { class: "section", attrs: { style: "margin:10px 0 0" } },
    el("h3", { text: `Span #${event.sequence}` }),
    kvList([
      ["Phase", event.phase || "—"],
      ["Name", event.name || "—"],
      ["Start", fmtMs(event.start_ms ?? event.startMs, 3)],
      ["Duration", fmtMs(event.duration_ms ?? event.durationMs, 3)],
      ["Scope", scope || "—"],
    ]),
    Array.isArray(event.reasons) && event.reasons.length ? rawDetails("Reasons", event.reasons) : null,
  ));
}

function renderTraceCompareBlock() {
  const perf = state.performance;
  const captures = perf.captures;
  const block = el("div", { class: "panel-block" },
    el("div", { class: "panel-block-head" }, el("h2", { text: "Compare captures" })),
    el("div", { class: "panel-block-body" }),
  );
  const body = block.lastElementChild;
  if (captures.length < 2) {
    body.append(emptyInline("Record at least two traces in this session to compare phases."));
    return block;
  }
  const baselineSelect = el("select", { props: { id: "trace-baseline" }, attrs: { "aria-label": "Baseline trace" } },
    ...captures.map(record => new Option(`${record.summary?.id || "capture"} · ${numberValue(record.summary?.eventCount) ?? record.events.length} spans${record.imported ? " · imported" : ""}`, record.summary?.id || "")));
  const candidateSelect = el("select", { props: { id: "trace-candidate" }, attrs: { "aria-label": "Candidate trace" } },
    ...captures.map(record => new Option(`${record.summary?.id || "capture"} · ${numberValue(record.summary?.eventCount) ?? record.events.length} spans${record.imported ? " · imported" : ""}`, record.summary?.id || "")));
  baselineSelect.value = captures.some(record => record.summary?.id === perf.baseline) ? perf.baseline : captures[0].summary?.id || "";
  candidateSelect.value = captures.some(record => record.summary?.id === perf.candidate) ? perf.candidate : captures[captures.length - 1].summary?.id || "";
  const reselect = () => {
    perf.baseline = baselineSelect.value;
    perf.candidate = candidateSelect.value;
    perf.comparison = null;
    renderPerformance();
  };
  baselineSelect.addEventListener("change", reselect);
  candidateSelect.addEventListener("change", reselect);
  body.append(el("div", { class: "trace-compare-controls" },
    el("label", { class: "inline-field", attrs: { for: "trace-baseline" }, text: "Baseline" }),
    baselineSelect,
    el("label", { class: "inline-field", attrs: { for: "trace-candidate" }, text: "Candidate" }),
    candidateSelect,
    el("button", {
      class: "tool-button",
      props: { type: "button" },
      text: perf.comparing ? "Comparing…" : "Compare",
      on: { click: compareTraces },
    }),
  ));
  const comparison = perf.comparison;
  if (comparison && Array.isArray(comparison.changes)) {
    if (!comparison.changes.length) {
      body.append(emptyInline("Both captures reported no phase summaries to compare."));
    } else {
      body.append(el("div", { class: "table-wrap", attrs: { style: "margin-top:8px" } }, el("table", { class: "data-table" },
        el("thead", {}, el("tr", {},
          el("th", { text: "Phase" }),
          el("th", { class: "num", text: "Baseline avg" }),
          el("th", { class: "num", text: "Candidate avg" }),
          el("th", { class: "num", text: "Δ avg" }),
          el("th", { class: "num", text: "Δ p95" }),
        )),
        el("tbody", {}, ...comparison.changes.map(change => {
          const baselineAverage = tracePhaseValue(change.baseline, "averageMs");
          const candidateAverage = tracePhaseValue(change.candidate, "averageMs");
          const averageDelta = numberValue(change.averageDeltaMs ?? change.average_delta_ms);
          const p95Delta = numberValue(change.p95DeltaMs ?? change.p95_delta_ms);
          return el("tr", {},
            el("td", { class: "mono-cell", text: change.phase || "—" }),
            el("td", { class: "num", text: fmtMs(baselineAverage, 3) }),
            el("td", { class: "num", text: fmtMs(candidateAverage, 3) }),
            el("td", { class: `num ${averageDelta === null ? "" : averageDelta > 0 ? "change-positive" : averageDelta < 0 ? "change-negative" : ""}`, text: averageDelta === null ? "—" : `${averageDelta > 0 ? "+" : ""}${averageDelta.toFixed(3)} ms` }),
            el("td", { class: `num ${p95Delta === null ? "" : p95Delta > 0 ? "change-positive" : p95Delta < 0 ? "change-negative" : ""}`, text: p95Delta === null ? "—" : `${p95Delta > 0 ? "+" : ""}${p95Delta.toFixed(3)} ms` }),
          );
        })),
      )));
      body.append(el("div", { class: "perf-note" },
        comparison.baseline?.id || comparison.candidate?.id
          ? `${comparison.baseline?.id || "baseline"} → ${comparison.candidate?.id || "candidate"}`
          : "",
        comparison.source === "summaries" ? " · computed from the captures' published phase summaries" : " · reported by the runtime"));
    }
  }
  return block;
}

/* Performance snapshots */

function downloadJson(filename, value) {
  const blob = new Blob([JSON.stringify(value, null, 2)], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  document.body.append(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

function snapshotFilename() {
  const target = targetById();
  const name = state.performance.snapshot?.target?.title || target?.title || "snapshot";
  const stamp = new Date().toISOString().replace(/[:.]/g, "-");
  return `cove-performance-${slug(name)}-${stamp}.json`;
}

function performanceSnapshotPayload() {
  const perf = state.performance;
  const target = targetById();
  const snapshotTarget = perf.snapshot?.target || (target
    ? { id: target.id, title: target.title, platform: target.platform, capabilities: Array.isArray(target.capabilities) ? target.capabilities : [] }
    : null);
  return {
    schema: SNAPSHOT_SCHEMA,
    version: SNAPSHOT_VERSION,
    exportedAt: new Date().toISOString(),
    budgetHz: perf.budgetHz,
    target: snapshotTarget,
    frames: perf.frames,
    captures: perf.captures.map(record => ({
      capture: record.summary,
      events: record.events,
      eventsComplete: record.complete === true,
    })),
    notes: [
      "TimingFrame records carry no wall-clock timestamp; frame-to-span alignment uses each frame's timeline sequence window, not wall-clock time.",
      "Only the active capture is paged to completion on export; other captures keep their summaries and already-loaded spans.",
    ],
  };
}

async function exportPerformance() {
  const perf = state.performance;
  if (perf.loading) {
    toast("Wait for the current frame read to finish before exporting.", "info");
    return;
  }
  if (perf.recording) {
    toast("Stop the trace before exporting.", "info");
    return;
  }
  if (!perf.frames.length && !perf.captures.length) {
    toast("Record frame timings or a trace before exporting.", "info");
    return;
  }
  const exportLabel = dom.performanceExport.querySelector("span");
  const exportText = exportLabel?.textContent || "Export";
  dom.performanceExport.disabled = true;
  dom.performanceExport.setAttribute("aria-busy", "true");
  if (exportLabel) exportLabel.textContent = "Exporting…";
  try {
    const active = activeCaptureRecord();
    if (active && !active.imported && !active.complete) {
      while (!active.complete && !active.imported && active.events.length < MAX_SNAPSHOT_EVENTS) {
        const before = active.events.length;
        const loaded = await fetchTracePage(active, { quiet: true, silent: true });
        if (!loaded) {
          throw new Error(`Could not read every span for ${active.summary?.id || "the active capture"}; export stopped. ${perf.eventError ? errorMessage(perf.eventError) : ""}`.trim());
        }
        if (active.events.length === before) break;
      }
    }
    const payload = performanceSnapshotPayload();
    downloadJson(snapshotFilename(), payload);
    toast(`Exported ${payload.frames.length} frames and ${payload.captures.length} capture${payload.captures.length === 1 ? "" : "s"}.`, "success");
  } catch (error) {
    toast(errorMessage(error), "error", 9000);
  } finally {
    dom.performanceExport.removeAttribute("aria-busy");
    if (exportLabel) exportLabel.textContent = exportText;
    dom.performanceExport.disabled = state.performance.loading || state.performance.recording || state.performance.traceLoading || state.performance.eventLoading || !(state.performance.frames.length || state.performance.captures.length);
    renderPerformance();
  }
}

function snapshotNumber(value, label, options = {}) {
  if (!Number.isFinite(value)) throw new Error(`${label} is not a finite number.`);
  if (value < 0) throw new Error(`${label} is negative.`);
  if (options.integer && !Number.isInteger(value)) throw new Error(`${label} is not an integer.`);
  return value;
}

function snapshotOptionalNumber(value, label) {
  if (value === null || value === undefined) return null;
  return snapshotNumber(value, label);
}

function validateSnapshotFrame(frame, index) {
  const label = `Frame ${index + 1}`;
  if (!frame || typeof frame !== "object" || Array.isArray(frame)) throw new Error(`${label} is not an object.`);
  const number = snapshotNumber(frame.frame, `${label} sequence`, { integer: true });
  if (typeof frame.submitted !== "boolean") throw new Error(`${label} has no submitted flag.`);
  if (frame.presented !== null && frame.presented !== undefined && typeof frame.presented !== "boolean") {
    throw new Error(`${label} has an invalid presented flag.`);
  }
  return {
    frame: number,
    recompositionMs: snapshotOptionalNumber(frame.recompositionMs, `${label} recompositionMs`),
    layoutMs: snapshotNumber(frame.layoutMs, `${label} layoutMs`),
    drawMs: snapshotNumber(frame.drawMs, `${label} drawMs`),
    renderThreadMs: snapshotOptionalNumber(frame.renderThreadMs, `${label} renderThreadMs`),
    gpuMs: snapshotOptionalNumber(frame.gpuMs, `${label} gpuMs`),
    submitted: frame.submitted,
    presented: frame.presented === true ? true : frame.presented === false ? false : null,
    totalMs: snapshotNumber(frame.totalMs, `${label} totalMs`),
    nodeCount: snapshotNumber(frame.nodeCount, `${label} nodeCount`, { integer: true }),
    batchCount: snapshotNumber(frame.batchCount, `${label} batchCount`, { integer: true }),
    timelineStart: frame.timelineStart === undefined ? 0 : snapshotNumber(frame.timelineStart, `${label} timelineStart`, { integer: true }),
    timelineEnd: frame.timelineEnd === undefined ? 0 : snapshotNumber(frame.timelineEnd, `${label} timelineEnd`, { integer: true }),
  };
}

function validateSnapshotEvent(event, label) {
  if (!event || typeof event !== "object" || Array.isArray(event)) throw new Error(`${label} is not an object.`);
  const sequence = numberValue(event.sequence);
  if (sequence === null) throw new Error(`${label} has no finite sequence.`);
  const phase = typeof event.phase === "string" ? event.phase : "";
  if (!phase) throw new Error(`${label} has no phase.`);
  const start = numberValue(event.start_ms ?? event.startMs);
  const duration = numberValue(event.duration_ms ?? event.durationMs);
  if (start === null) throw new Error(`${label} has no finite start_ms.`);
  if (duration === null) throw new Error(`${label} has no finite duration_ms.`);
  const scope = event.scope_id ?? event.scopeID ?? null;
  if (scope !== null && typeof scope !== "string") throw new Error(`${label} has an invalid scope_id.`);
  return {
    sequence,
    phase,
    name: typeof event.name === "string" ? event.name : "",
    scope_id: scope,
    start_ms: start,
    duration_ms: duration,
    reasons: Array.isArray(event.reasons) ? event.reasons : [],
  };
}

function validateSnapshotCapture(entry, index) {
  const label = `Capture ${index + 1}`;
  if (!entry || typeof entry !== "object" || Array.isArray(entry)) throw new Error(`${label} is not an object.`);
  const summary = entry.capture;
  if (!summary || typeof summary !== "object" || Array.isArray(summary)) throw new Error(`${label} has no capture summary.`);
  if (typeof summary.id !== "string" || !summary.id) throw new Error(`${label} has no capture id.`);
  const durationMs = snapshotNumber(numberValue(summary.durationMs), `${label} durationMs`);
  const eventCount = snapshotNumber(numberValue(summary.eventCount), `${label} eventCount`, { integer: true });
  const dropped = summary.dropped === undefined ? 0 : snapshotNumber(numberValue(summary.dropped), `${label} dropped`, { integer: true });
  const phases = [];
  if (summary.phases !== undefined && !Array.isArray(summary.phases)) throw new Error(`${label} has an invalid phases list.`);
  for (const [phaseIndex, phase] of (summary.phases || []).entries()) {
    if (!phase || typeof phase !== "object" || typeof phase.phase !== "string") throw new Error(`${label} phase ${phaseIndex + 1} has no name.`);
    phases.push({
      phase: phase.phase,
      count: snapshotNumber(numberValue(phase.count), `${label} phase ${phase.phase} count`, { integer: true }),
      averageMs: snapshotNumber(numberValue(phase.averageMs ?? phase.average_ms), `${label} phase ${phase.phase} averageMs`),
      p95Ms: snapshotNumber(numberValue(phase.p95Ms ?? phase.p95_ms), `${label} phase ${phase.phase} p95Ms`),
      maximumMs: snapshotNumber(numberValue(phase.maximumMs ?? phase.maximum_ms), `${label} phase ${phase.phase} maximumMs`),
    });
  }
  if (entry.events !== undefined && !Array.isArray(entry.events)) throw new Error(`${label} has an invalid events list.`);
  if ((entry.events || []).length > MAX_SNAPSHOT_EVENTS) {
    throw new Error(`${label} contains ${entry.events.length} spans; this client reads at most ${MAX_SNAPSHOT_EVENTS}. Export fewer spans.`);
  }
  const normalized = (entry.events || []).map((event, eventIndex) => validateSnapshotEvent(event, `${label} event ${eventIndex + 1}`));
  const eventsComplete = entry.eventsComplete === true;
  if (eventsComplete && normalized.length < eventCount) {
    throw new Error(`${label} declares complete events but contains ${normalized.length} of ${eventCount} spans.`);
  }
  return {
    summary: { id: summary.id, durationMs, eventCount, dropped, phases },
    events: normalized,
    total: eventCount,
    nextOffset: null,
    complete: eventsComplete && normalized.length >= eventCount,
    imported: true,
    loading: false,
  };
}

function parsePerformanceSnapshot(text) {
  let data;
  try {
    data = JSON.parse(text);
  } catch {
    throw new Error("The file is not valid JSON.");
  }
  if (!data || typeof data !== "object" || Array.isArray(data)) throw new Error("The snapshot must be a JSON object.");
  if (data.schema !== SNAPSHOT_SCHEMA) throw new Error(`Not a Cove performance snapshot (schema ${JSON.stringify(data.schema ?? null)}).`);
  if (data.version !== SNAPSHOT_VERSION) throw new Error(`Unsupported snapshot version ${JSON.stringify(data.version)}; this client reads version ${SNAPSHOT_VERSION}.`);
  if (!Array.isArray(data.frames)) throw new Error("The snapshot has no frames array.");
  if (!Array.isArray(data.captures)) throw new Error("The snapshot has no captures array.");
  if (data.frames.length > MAX_FRAME_RECORDS) {
    throw new Error(`The snapshot contains ${data.frames.length} frames; this client reads at most ${MAX_FRAME_RECORDS}.`);
  }
  const frames = data.frames.map(validateSnapshotFrame);
  if (new Set(frames.map(frame => frame.frame)).size !== frames.length) throw new Error("The snapshot contains duplicate frame sequences.");
  const captures = data.captures.map(validateSnapshotCapture);
  if (new Set(captures.map(record => record.summary.id)).size !== captures.length) throw new Error("The snapshot contains duplicate capture ids.");
  if (!frames.length && !captures.length) throw new Error("The snapshot contains no frames or trace captures.");
  return {
    frames,
    captures,
    budgetHz: BUDGET_RATES.includes(Number(data.budgetHz)) ? Number(data.budgetHz) : null,
    target: data.target && typeof data.target === "object" ? data.target : null,
    exportedAt: typeof data.exportedAt === "string" ? data.exportedAt : "",
  };
}

async function importPerformance(file) {
  if (!file) return;
  const perf = state.performance;
  if (perf.recording) {
    toast("Stop the active trace before importing a snapshot.", "error");
    return;
  }
  if (perf.profile.status === "sampling" || perf.profile.status === "starting" || perf.profile.status === "stopping") {
    toast("Stop the active CPU profile before importing a snapshot.", "error");
    return;
  }
  if (perf.loading) {
    toast("Wait for the current frame read to finish before importing.", "info");
    return;
  }
  if (perf.traceLoading || perf.eventLoading || perf.captures.some(record => record.loading)) {
    toast("Wait for the current trace operation to finish before importing.", "info");
    return;
  }
  const importLabel = dom.performanceImport.querySelector("span");
  const importText = importLabel?.textContent || "Import";
  dom.performanceImport.disabled = true;
  dom.performanceImport.setAttribute("aria-busy", "true");
  if (importLabel) importLabel.textContent = "Reading…";
  perf.importError = null;
  try {
    toast(`Reading ${file.name || "snapshot"}…`, "info", 2000);
    const snapshot = parsePerformanceSnapshot(await file.text());
    perf.frames = snapshot.frames;
    perf.dropped = 0;
    perf.loaded = true;
    perf.loading = false;
    perf.error = null;
    perf.selectedFrame = null;
    perf.analysis = { frame: null, loading: false, error: null, data: null, request: perf.analysis.request + 1 };
    perf.analysisCache.clear();
    perf.captures = snapshot.captures;
    perf.activeCaptureId = snapshot.captures[0]?.summary.id || "";
    perf.baseline = snapshot.captures[0]?.summary.id || "";
    perf.candidate = snapshot.captures[snapshot.captures.length - 1]?.summary.id || "";
    perf.comparison = null;
    perf.selectedSpan = null;
    perf.zoom = 1;
    perf.eventError = null;
    perf.eventLoading = false;
    perf.traceError = null;
    perf.traceLoading = false;
    perf.recording = false;
    clearProfileTimers(perf.profile);
    const profileHz = perf.profile.hz;
    const profileDuration = perf.profile.durationMs;
    perf.profile = emptyProfile();
    perf.profile.hz = profileHz;
    perf.profile.durationMs = profileDuration;
    if (snapshot.budgetHz) {
      perf.budgetHz = snapshot.budgetHz;
      writeBudgetPreference(snapshot.budgetHz);
    }
    perf.snapshot = {
      name: file.name || "snapshot.json",
      exportedAt: snapshot.exportedAt,
      target: snapshot.target,
    };
    perf.tab = "frames";
    stopTimer("performance");
    renderPerformance();
    dom.performanceContent.scrollTop = 0;
    toast(`Imported ${perf.frames.length} frame${perf.frames.length === 1 ? "" : "s"} and ${perf.captures.length} capture${perf.captures.length === 1 ? "" : "s"}.`, "success");
  } catch (error) {
    perf.importError = error;
    toast(`Import failed: ${error.message || error}`, "error", 10000);
    renderPerformance();
  } finally {
    dom.performanceImport.removeAttribute("aria-busy");
    dom.performanceImport.disabled = false;
    if (importLabel) importLabel.textContent = importText;
    dom.performanceFile.value = "";
  }
}

function returnToLive() {
  const perf = state.performance;
  const budget = perf.budgetHz;
  const tab = perf.tab;
  const hz = PROFILE_HZ_RATES.includes(perf.profile.hz) ? perf.profile.hz : PROFILE_DEFAULT_HZ;
  const durationMs = PROFILE_DURATIONS.includes(perf.profile.durationMs) ? perf.profile.durationMs : PROFILE_DEFAULT_DURATION;
  clearProfileTimers(perf.profile);
  stopTimer("performance");
  state.performance = emptyPerformance();
  state.performance.budgetHz = budget;
  state.performance.tab = tab;
  state.performance.profile.hz = hz;
  state.performance.profile.durationMs = durationMs;
  state.performance.follow = dom.performanceFollow.checked === true;
  renderPerformance();
  ensurePerformance();
  syncFollowTimers();
}


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

function renderProfileViewBody(profile, model) {
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

function renderProfilePanel() {
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

function releaseAllocationSampling(targetId) {
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

function stopAllocationPolling() {
  const timer = state.memory?.allocation?.timer;
  if (timer) clearInterval(timer);
  if (state.memory?.allocation) state.memory.allocation.timer = null;
}

function syncAllocationPolling() {
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

async function startAllocationSites() {
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

async function queryAllocationSites(options = {}) {
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

async function stopAllocationSites(options = {}) {
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

async function resetAllocationSites() {
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

function ensureAllocationSites() {
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

function abandonAllocationSites() {
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

function allocationSiteFailureKind(allocation) {
  if (!state.connected) return "offline";
  if (["unsupported", "unavailable", "error", "offline"].includes(allocation.status)) return allocation.status;
  return "";
}

function allocationSiteStatusLabel(allocation) {
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

function allocationSiteStatusClass(allocation) {
  if (!state.connected) return "failed";
  if (allocation.status === "running" || allocation.status === "starting") return "pending";
  if (allocation.status === "stopped") return "complete";
  if (["unsupported", "unavailable", "error", "offline"].includes(allocation.status)) return "failed";
  return "cancelled";
}

function allocationSiteMeta(allocation) {
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

function allocationSiteScopeNote() {
  return el("div", { class: "memory-scope" },
    icon("info"),
    el("span", {},
      el("strong", { text: "Cumulative sampled allocations, not live heap." }),
      " Start enables cove_alloc's opt-in sampler in the runtime: every Nth fresh allocation records a raw PC stack into a fixed ring. Sampled bytes and blocks are cumulative for the current session only; they are not live heap, object counts, object types, retaining paths or leaks. Frames are symbolized allocator return addresses; the runtime removes allocator-internal frames where it can identify them, and unresolved frames keep their raw address.",
    ),
  );
}

function allocationSiteState(kind, title, message, options = {}) {
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

function allocationSiteRateControl(allocation, disabled) {
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

function allocationSiteMetric(label, value, format, title) {
  return el("div", { class: "metric", attrs: { title } },
    el("span", { class: "metric-label", text: label }),
    el("span", { class: "metric-value", text: value === null ? "—" : format(value) }),
  );
}

function allocationSiteTopFrame(site) {
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

function renderAllocationSiteStack(site) {
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

function renderAllocationSiteTable(report, allocation) {
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

function allocationSiteTruncationNotices(report) {
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

/* Allocation-site baselines
 *
 * A baseline is the exact sampled report the client received, plus the target,
 * rate, session and timestamps that make the comparison interpretable. Sites
 * are matched by their full symbolized stack identity, never by array order.
 * Cumulative sampled counters from different sessions or rates are not
 * heap growth and are labelled as such instead of being converted.
 */

function allocationSnapshotRequireKeys(value, keys, label) {
  if (!isObjectValue(value)) throw new Error(`${label} is not an object`);
  const missing = keys.filter(key => !Object.prototype.hasOwnProperty.call(value, key));
  if (missing.length) throw new Error(`${label} is missing ${missing.join(", ")}`);
  const unknown = Object.keys(value).filter(key => !keys.includes(key));
  if (unknown.length) throw new Error(`${label} has an unknown field: ${unknown.join(", ")}`);
  return value;
}

function allocationSnapshotInteger(value, label, options = {}) {
  const { nullable = false, min = 0, max = Number.MAX_SAFE_INTEGER } = options;
  if (value === null && nullable) return null;
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < min || value > max) {
    throw new Error(`${label} is not ${nullable ? "null or " : ""}an integer from ${formatCount(min)} to ${formatCount(max)}`);
  }
  return value;
}

function allocationSnapshotString(value, label, max = 2048) {
  if (typeof value !== "string") throw new Error(`${label} is not a string`);
  if (value.length > max) throw new Error(`${label} is longer than ${formatCount(max)} characters`);
  return value;
}

function allocationSnapshotFrame(value, label) {
  allocationSnapshotRequireKeys(value, ALLOCATION_SNAPSHOT_FRAME_KEYS, label);
  const address = allocationSnapshotString(value.address, `${label}.address`, 64);
  if (address && !/^0x[0-9a-f]+$/.test(address)) {
    throw new Error(`${label}.address is not a lowercase hexadecimal address`);
  }
  const functionName = allocationSnapshotString(value.function, `${label}.function`);
  const file = allocationSnapshotString(value.file, `${label}.file`);
  const module = allocationSnapshotString(value.module, `${label}.module`);
  const line = value.line === null
    ? null
    : allocationSnapshotInteger(value.line, `${label}.line`, { min: 0, max: 0xffffffff });
  if (!address && !functionName && !file && line === null) {
    throw new Error(`${label} has no function, file, line or address`);
  }
  return { address, function: functionName, file, line, module };
}

function allocationSnapshotSite(value, label, index) {
  allocationSnapshotRequireKeys(value, ALLOCATION_SNAPSHOT_SITE_KEYS, label);
  if (!Array.isArray(value.frames)) throw new Error(`${label}.frames is not an array`);
  if (value.frames.length > MAX_ALLOCATION_SITE_FRAMES) {
    throw new Error(`${label}.frames has more than ${formatCount(MAX_ALLOCATION_SITE_FRAMES)} frames`);
  }
  const frames = value.frames.map((frame, frameIndex) => allocationSnapshotFrame(frame, `${label}.frames[${frameIndex}]`));
  const bytes = allocationSnapshotInteger(value.bytes, `${label}.bytes`, { nullable: true });
  const blocks = allocationSnapshotInteger(value.blocks, `${label}.blocks`, { nullable: true });
  const framesOmitted = allocationSnapshotInteger(value.framesOmitted, `${label}.framesOmitted`);
  if (typeof value.framesTruncated !== "boolean") throw new Error(`${label}.framesTruncated is not a boolean`);
  const stackKey = allocationSiteStackIdentity(frames);
  return {
    key: stackKey || `site-${index}`,
    stackKey,
    frames,
    bytes,
    blocks,
    framesOmitted,
    framesTruncated: value.framesTruncated,
  };
}

function allocationSnapshotReport(value) {
  allocationSnapshotRequireKeys(value, ALLOCATION_SNAPSHOT_REPORT_KEYS, "report");
  if (!ALLOCATION_SNAPSHOT_STATUSES.includes(value.status)) {
    throw new Error(`report.status is not one of ${ALLOCATION_SNAPSHOT_STATUSES.join(", ")}`);
  }
  if (typeof value.sitesTruncated !== "boolean") throw new Error("report.sitesTruncated is not a boolean");
  if (typeof value.framesTruncated !== "boolean") throw new Error("report.framesTruncated is not a boolean");
  if (!Array.isArray(value.sites)) throw new Error("report.sites is not an array");
  if (value.sites.length > MAX_ALLOCATION_SITES) {
    throw new Error(`report.sites has more than ${formatCount(MAX_ALLOCATION_SITES)} sites`);
  }
  const sites = value.sites.map((site, index) => allocationSnapshotSite(site, `report.sites[${index}]`, index));
  const seen = new Set();
  let knownBytes = 0;
  let knownBlocks = 0;
  for (const site of sites) {
    if (site.stackKey) {
      if (seen.has(site.stackKey)) throw new Error("report.sites repeats a stack identity; sites cannot be matched unambiguously");
      seen.add(site.stackKey);
    }
    if (site.bytes !== null) knownBytes += site.bytes;
    if (site.blocks !== null) knownBlocks += site.blocks;
  }
  const sampledBytes = allocationSnapshotInteger(value.sampledBytes, "report.sampledBytes", { nullable: true });
  const sampledBlocks = allocationSnapshotInteger(value.sampledBlocks, "report.sampledBlocks", { nullable: true });
  if (sampledBytes !== null && knownBytes > sampledBytes) {
    throw new Error("report.sampledBytes is smaller than the sum of its sites");
  }
  if (sampledBlocks !== null && knownBlocks > sampledBlocks) {
    throw new Error("report.sampledBlocks is smaller than the sum of its sites");
  }
  const totalSites = allocationSnapshotInteger(value.totalSites, "report.totalSites", { nullable: true });
  if (totalSites !== null && totalSites < sites.length) {
    throw new Error("report.totalSites is smaller than the number of listed sites");
  }
  return {
    status: value.status,
    sampledBytes,
    sampledBlocks,
    samplesAttempted: allocationSnapshotInteger(value.samplesAttempted, "report.samplesAttempted", { nullable: true }),
    samplesRetained: allocationSnapshotInteger(value.samplesRetained, "report.samplesRetained", { nullable: true }),
    samplesDropped: allocationSnapshotInteger(value.samplesDropped, "report.samplesDropped", { nullable: true }),
    captureFailures: allocationSnapshotInteger(value.captureFailures, "report.captureFailures", { nullable: true }),
    totalSites,
    sitesOmitted: allocationSnapshotInteger(value.sitesOmitted, "report.sitesOmitted"),
    framesOmitted: allocationSnapshotInteger(value.framesOmitted, "report.framesOmitted"),
    sitesTruncated: value.sitesTruncated,
    framesTruncated: value.framesTruncated,
    symbolizationReason: allocationSnapshotString(value.symbolizationReason, "report.symbolizationReason"),
    sites,
  };
}

function allocationSnapshotFromText(text, expectedTargetId) {
  let parsed;
  try {
    parsed = JSON.parse(text);
  } catch {
    throw new Error("the file is not valid JSON");
  }
  allocationSnapshotRequireKeys(parsed, ALLOCATION_SNAPSHOT_KEYS, "the snapshot");
  if (parsed.schema !== ALLOCATION_SNAPSHOT_SCHEMA) {
    throw new Error("the file is not a Cove allocation-site snapshot");
  }
  if (parsed.version !== ALLOCATION_SNAPSHOT_VERSION) {
    throw new Error(`unsupported snapshot version ${JSON.stringify(parsed.version)}`);
  }
  const capturedAt = allocationSnapshotInteger(parsed.capturedAt, "capturedAt", { min: 1 });
  const reportUpdatedAt = allocationSnapshotInteger(parsed.reportUpdatedAt, "reportUpdatedAt", { nullable: true, min: 1 });
  allocationSnapshotRequireKeys(parsed.target, ["id", "title"], "target");
  const targetId = allocationSnapshotString(parsed.target.id, "target.id", 256).trim();
  if (!targetId) throw new Error("target.id is empty");
  const title = allocationSnapshotString(parsed.target.title, "target.title", 512);
  if (expectedTargetId && targetId !== expectedTargetId) {
    throw new Error(`the snapshot was captured on target ${JSON.stringify(targetId)}, not the selected target`);
  }
  if (parsed.session !== null && (typeof parsed.session !== "string" || !/^[A-Za-z0-9:._-]{1,128}$/.test(parsed.session))) {
    throw new Error("session is not null or a session identifier");
  }
  const rate = allocationSnapshotInteger(parsed.rate, "rate", { min: 1, max: ALLOCATION_RATE_LIMIT });
  return {
    schema: ALLOCATION_SNAPSHOT_SCHEMA,
    version: ALLOCATION_SNAPSHOT_VERSION,
    capturedAt,
    reportUpdatedAt,
    target: { id: targetId, title },
    session: parsed.session,
    rate,
    report: allocationSnapshotReport(parsed.report),
  };
}

function allocationSessionId(allocation) {
  return allocation.sessionSeq > 0 ? `${ALLOCATION_SESSION_NONCE}:${allocation.sessionSeq}` : null;
}

function allocationSnapshotOfReport(report, allocation) {
  const target = targetById();
  const rate = allocationSiteRateValue(report.rate)
    ?? allocationSiteRateValue(allocation.rate)
    ?? ALLOCATION_DEFAULT_RATE;
  return {
    schema: ALLOCATION_SNAPSHOT_SCHEMA,
    version: ALLOCATION_SNAPSHOT_VERSION,
    capturedAt: Date.now(),
    reportUpdatedAt: allocation.queriedAt || report.refreshedAt || null,
    target: { id: state.targetId, title: target?.title || "" },
    session: allocationSessionId(allocation),
    rate,
    report: {
      status: allocation.status === "running" || report.status === "running" ? "running" : "stopped",
      sampledBytes: report.sampledBytes,
      sampledBlocks: report.sampledBlocks,
      samplesAttempted: report.samplesAttempted,
      samplesRetained: report.samplesRetained,
      samplesDropped: report.samplesDropped,
      captureFailures: report.captureFailures,
      totalSites: report.totalSites,
      sitesOmitted: report.sitesOmitted ?? 0,
      framesOmitted: report.framesOmitted ?? 0,
      sitesTruncated: Boolean(report.sitesTruncated),
      framesTruncated: Boolean(report.framesTruncated),
      symbolizationReason: report.symbolizationReason || "",
      sites: report.sites.slice(0, MAX_ALLOCATION_SITES).map(site => ({
        frames: site.frames.slice(0, MAX_ALLOCATION_SITE_FRAMES).map(frame => ({
          address: /^0x[0-9a-f]+$/.test(frame.address || "") ? frame.address : "",
          function: frame.function || "",
          file: frame.file || "",
          line: Number.isInteger(frame.line) && frame.line >= 0 && frame.line <= 0xffffffff ? frame.line : null,
          module: frame.module || "",
        })),
        bytes: site.bytes,
        blocks: site.blocks,
        framesOmitted: site.framesOmitted,
        framesTruncated: site.framesTruncated,
      })),
    },
  };
}

function allocationSnapshotSerializable(snapshot) {
  return {
    schema: snapshot.schema,
    version: snapshot.version,
    capturedAt: snapshot.capturedAt,
    reportUpdatedAt: snapshot.reportUpdatedAt,
    target: { id: snapshot.target.id, title: snapshot.target.title },
    session: snapshot.session,
    rate: snapshot.rate,
    report: {
      status: snapshot.report.status,
      sampledBytes: snapshot.report.sampledBytes,
      sampledBlocks: snapshot.report.sampledBlocks,
      samplesAttempted: snapshot.report.samplesAttempted,
      samplesRetained: snapshot.report.samplesRetained,
      samplesDropped: snapshot.report.samplesDropped,
      captureFailures: snapshot.report.captureFailures,
      totalSites: snapshot.report.totalSites,
      sitesOmitted: snapshot.report.sitesOmitted,
      framesOmitted: snapshot.report.framesOmitted,
      sitesTruncated: snapshot.report.sitesTruncated,
      framesTruncated: snapshot.report.framesTruncated,
      symbolizationReason: snapshot.report.symbolizationReason,
      sites: snapshot.report.sites.map(site => ({
        frames: site.frames.map(frame => ({
          address: frame.address,
          function: frame.function,
          file: frame.file,
          line: frame.line,
          module: frame.module,
        })),
        bytes: site.bytes,
        blocks: site.blocks,
        framesOmitted: site.framesOmitted,
        framesTruncated: site.framesTruncated,
      })),
    },
  };
}

function allocationCaptureBaseline() {
  const allocation = state.memory.allocation;
  const report = allocation.report;
  if (!report || !state.targetId) return;
  allocation.baseline = allocationSnapshotOfReport(report, allocation);
  allocation.baselineSource = "session snapshot";
  allocation.baselineError = "";
  allocation.diffExpanded.clear();
  renderAllocationSitesView();
  toast("Allocation-site baseline captured.", "success", 3000);
}

function allocationSnapshotFilename(snapshot) {
  const date = new Date(snapshot.capturedAt);
  const pad = value => String(value).padStart(2, "0");
  const stamp = `${date.getFullYear()}${pad(date.getMonth() + 1)}${pad(date.getDate())}-${pad(date.getHours())}${pad(date.getMinutes())}${pad(date.getSeconds())}`;
  return `cove-allocation-sites-${slug(snapshot.target.title || snapshot.target.id)}-${stamp}.json`;
}

function allocationExportBaseline() {
  const allocation = state.memory.allocation;
  const baseline = allocation.baseline;
  if (!baseline) return;
  try {
    downloadJson(allocationSnapshotFilename(baseline), allocationSnapshotSerializable(baseline));
  } catch (error) {
    allocation.baselineError = `Export failed: ${error?.message || error}`;
    renderAllocationSitesView();
  }
}

function allocationClearBaseline() {
  const allocation = state.memory.allocation;
  if (!allocation.baseline && !allocation.baselineError) return;
  const hadBaseline = Boolean(allocation.baseline);
  allocation.baseline = null;
  allocation.baselineSource = "";
  allocation.baselineError = "";
  allocation.diffExpanded.clear();
  renderAllocationSitesView();
  if (hadBaseline) toast("Allocation-site baseline cleared.", "info", 3000);
}

async function allocationImportBaseline(input) {
  const file = input.files && input.files[0];
  input.value = "";
  if (!file) return;
  const allocation = state.memory.allocation;
  const targetId = state.targetId;
  try {
    if (file.size > MAX_ALLOCATION_SNAPSHOT_BYTES) {
      throw new Error(`the file is larger than ${formatCount(MAX_ALLOCATION_SNAPSHOT_BYTES)} bytes`);
    }
    const text = await file.text();
    if (targetId !== state.targetId) return;
    allocation.baseline = allocationSnapshotFromText(text, targetId);
    allocation.baselineSource = "imported snapshot";
    allocation.baselineError = "";
    allocation.diffExpanded.clear();
    renderAllocationSitesView();
    toast("Allocation-site baseline imported.", "success", 3000);
  } catch (error) {
    if (targetId !== state.targetId) return;
    allocation.baselineError = `Snapshot rejected: ${error?.message || error}`;
    renderAllocationSitesView();
  }
}

function allocationSiteDiffIndex(sites) {
  const map = new Map();
  let duplicates = 0;
  let unmatchable = 0;
  for (const site of sites) {
    const key = site.stackKey ?? allocationSiteStackIdentity(site.frames || []);
    if (!key) {
      unmatchable += 1;
      continue;
    }
    const existing = map.get(key);
    if (existing) {
      duplicates += 1;
      existing.bytes = existing.bytes === null || site.bytes === null ? null : existing.bytes + site.bytes;
      existing.blocks = existing.blocks === null || site.blocks === null ? null : existing.blocks + site.blocks;
      existing.framesTruncated = existing.framesTruncated || site.framesTruncated;
      existing.framesOmitted = Math.max(existing.framesOmitted, site.framesOmitted);
      continue;
    }
    map.set(key, { ...site, key });
  }
  return { map, duplicates, unmatchable };
}

function allocationDiffPercent(before, after) {
  if (before === null || after === null || before <= 0) return null;
  return ((after - before) / before) * 100;
}

function allocationSiteDiff(baselineReport, currentReport) {
  const before = allocationSiteDiffIndex((baselineReport?.sites || []).slice(0, MAX_ALLOCATION_SITES));
  const after = allocationSiteDiffIndex((currentReport?.sites || []).slice(0, MAX_ALLOCATION_SITES));
  const keys = new Set([...before.map.keys(), ...after.map.keys()]);
  const rows = [];
  for (const key of keys) {
    const previous = before.map.get(key) || null;
    const next = after.map.get(key) || null;
    let status = "unchanged";
    if (!previous) status = "added";
    else if (!next) status = "removed";
    else if (previous.bytes !== next.bytes || previous.blocks !== next.blocks) status = "changed";
    let deltaBytes = null;
    let deltaBlocks = null;
    if (previous && next && previous.bytes !== null && next.bytes !== null) deltaBytes = next.bytes - previous.bytes;
    else if (!previous && next) deltaBytes = next.bytes;
    else if (previous && !next) deltaBytes = previous.bytes === null ? null : -previous.bytes;
    if (previous && next && previous.blocks !== null && next.blocks !== null) deltaBlocks = next.blocks - previous.blocks;
    else if (!previous && next) deltaBlocks = next.blocks;
    else if (previous && !next) deltaBlocks = previous.blocks === null ? null : -previous.blocks;
    const percentBytes = previous && next
      ? allocationDiffPercent(previous.bytes, next.bytes)
      : previous && previous.bytes !== null && previous.bytes > 0 ? -100 : null;
    const percentBlocks = previous && next
      ? allocationDiffPercent(previous.blocks, next.blocks)
      : previous && previous.blocks !== null && previous.blocks > 0 ? -100 : null;
    rows.push({
      key,
      status,
      before: previous,
      after: next,
      site: next || previous,
      deltaBytes,
      deltaBlocks,
      percentBytes,
      percentBlocks,
    });
  }
  const counts = { added: 0, removed: 0, changed: 0, unchanged: 0 };
  for (const row of rows) counts[row.status] += 1;
  const changed = rows
    .filter(row => row.status !== "unchanged")
    .sort((left, right) => Math.abs(right.deltaBytes ?? 0) - Math.abs(left.deltaBytes ?? 0)
      || Math.abs(right.deltaBlocks ?? 0) - Math.abs(left.deltaBlocks ?? 0)
      || left.key.localeCompare(right.key));
  const totalBytes = {
    before: baselineReport?.sampledBytes ?? null,
    after: currentReport?.sampledBytes ?? null,
  };
  totalBytes.delta = totalBytes.before !== null && totalBytes.after !== null
    ? totalBytes.after - totalBytes.before
    : null;
  totalBytes.percent = allocationDiffPercent(totalBytes.before, totalBytes.after);
  const totalBlocks = {
    before: baselineReport?.sampledBlocks ?? null,
    after: currentReport?.sampledBlocks ?? null,
  };
  totalBlocks.delta = totalBlocks.before !== null && totalBlocks.after !== null
    ? totalBlocks.after - totalBlocks.before
    : null;
  totalBlocks.percent = allocationDiffPercent(totalBlocks.before, totalBlocks.after);
  return {
    counts,
    changed,
    duplicates: before.duplicates + after.duplicates,
    unmatchableBefore: before.unmatchable,
    unmatchableAfter: after.unmatchable,
    totalBytes,
    totalBlocks,
  };
}

function allocationDeltaBytesText(delta) {
  if (delta === null) return "—";
  if (delta === 0) return "±0 B";
  return `${delta > 0 ? "+" : "−"}${fmtBytes(Math.abs(delta))}`;
}

function allocationDeltaCountText(delta) {
  if (delta === null) return "—";
  if (delta === 0) return "±0";
  return `${delta > 0 ? "+" : "−"}${formatCount(Math.abs(delta))}`;
}

function allocationDeltaPercentText(percent) {
  if (percent === null || !Number.isFinite(percent)) return "—";
  if (Math.abs(percent) < 0.05) return "±0.0%";
  return `${percent > 0 ? "+" : "−"}${Math.abs(percent).toFixed(1)}%`;
}

function allocationDeltaClass(delta) {
  if (delta === null) return "";
  if (delta > 0) return "is-positive";
  if (delta < 0) return "is-negative";
  return "is-zero";
}

function allocationDiffSampleCell(value, present, format) {
  if (!present) {
    return el("span", {
      class: "memory-diff-absent",
      text: "—",
      attrs: { title: "Absent from this report" },
    });
  }
  if (value === null) return el("span", { text: "Not reported" });
  return el("span", {
    text: format === "bytes" ? fmtBytes(value) : formatCount(value),
    attrs: { title: format === "bytes" ? `${formatCount(value)} bytes` : `${formatCount(value)} blocks` },
  });
}

function allocationDiffStatusLabel(status) {
  return status === "added" ? "Added" : status === "removed" ? "Removed" : "Changed";
}

function allocationBaselineMeta(baseline, source) {
  const report = baseline.report;
  const sites = formatCount(report.sites.length);
  const sampled = report.sampledBytes === null ? "bytes not reported" : `${fmtBytes(report.sampledBytes)} sampled`;
  return `${source || "baseline snapshot"} · captured ${formatWallTime(baseline.capturedAt)} · 1-in-${formatCount(baseline.rate)} · ${sites} site${report.sites.length === 1 ? "" : "s"} · ${sampled}`;
}

function allocationCurrentMeta(report, allocation) {
  if (!report) return "Current report · no allocation-site report";
  const rate = allocationSiteRateValue(report.rate) ?? allocationSiteRateValue(allocation.rate);
  const sites = formatCount(report.sites.length);
  const sampled = report.sampledBytes === null ? "bytes not reported" : `${fmtBytes(report.sampledBytes)} sampled`;
  const updated = report.refreshedAt ? formatWallTime(report.refreshedAt) : "update time not reported";
  return `Current report · updated ${updated} · 1-in-${formatCount(rate ?? ALLOCATION_DEFAULT_RATE)} · ${sites} site${report.sites.length === 1 ? "" : "s"} · ${sampled}`;
}

function allocationBaselineFullTime(milliseconds) {
  try {
    return new Date(milliseconds).toLocaleString();
  } catch {
    return "timestamp not reported";
  }
}

function renderAllocationBaseline(allocation, report) {
  const baseline = allocation.baseline;
  const block = el("section", {
    class: "memory-baseline",
    attrs: { id: "memory-allocation-baseline", "aria-labelledby": "alloc-baseline-title" },
  });
  block.append(el("div", { class: "memory-baseline-head" },
    el("h3", { attrs: { id: "alloc-baseline-title" }, text: "Baseline diff" }),
    el("span", {
      class: "pane-meta",
      attrs: { title: "Baseline capture time and sampling rate" },
      text: baseline ? `Baseline ${formatWallTime(baseline.capturedAt)}` : "No baseline",
    }),
  ));
  const actions = el("div", { class: "memory-baseline-actions" });
  actions.append(el("button", {
    class: "tool-button small",
    props: { type: "button", id: "alloc-baseline-capture", disabled: !report || allocation.loading },
    attrs: { title: report ? "Store the current allocation-site report as the baseline" : "Start sampling or query a report first" },
    text: "Capture baseline",
    on: { click: () => allocationCaptureBaseline() },
  }));
  actions.append(el("button", {
    class: "tool-button small",
    props: { type: "button", id: "alloc-baseline-export", disabled: !baseline },
    attrs: { title: "Download the baseline snapshot as JSON" },
    text: "Export JSON",
    on: { click: () => allocationExportBaseline() },
  }));
  actions.append(el("label", {
    class: "tool-button small",
    attrs: { for: "alloc-baseline-import", title: "Import an allocation-site baseline snapshot JSON" },
  }, icon("upload"), "Import JSON"));
  const importInput = el("input", {
    class: "sr-only",
    attrs: {
      type: "file",
      id: "alloc-baseline-import",
      accept: ".json,application/json",
      "aria-label": "Import an allocation-site baseline snapshot JSON",
    },
  });
  importInput.addEventListener("change", () => allocationImportBaseline(importInput));
  actions.append(importInput);
  actions.append(el("button", {
    class: "tool-button small",
    props: { type: "button", id: "alloc-baseline-clear", disabled: !baseline && !allocation.baselineError },
    attrs: { title: "Discard the allocation-site baseline" },
    text: "Clear baseline",
    on: { click: () => allocationClearBaseline() },
  }));
  block.append(actions);

  if (allocation.baselineError) block.append(errorInline(allocation.baselineError));
  block.append(el("p", {
    class: "memory-note memory-baseline-scope",
    text: "Baselines compare cumulative sampled allocations at the configured 1-in-N rate, not live heap, object counts or leaks.",
  }));

  if (!baseline) {
    block.append(el("p", {
      class: "memory-baseline-empty",
      text: report
        ? "No baseline snapshot. Capture the current report or import a snapshot JSON to compare sampled sites. No site is inferred."
        : "No baseline snapshot and no current report. Start sampling to capture a baseline, or import a snapshot JSON. No site is inferred.",
    }));
    return block;
  }

  block.append(el("p", {
    class: "memory-diff-baseline",
    text: allocationBaselineMeta(baseline, allocation.baselineSource),
    attrs: { title: allocationBaselineFullTime(baseline.capturedAt) },
  }));
  block.append(el("p", {
    class: "memory-diff-current",
    text: allocationCurrentMeta(report, allocation),
    attrs: report?.refreshedAt ? { title: allocationBaselineFullTime(report.refreshedAt) } : {},
  }));

  const notices = allocationSiteTruncationNotices(baseline.report).map(text => `Baseline: ${text}`);
  if (report) {
    notices.push(...allocationSiteTruncationNotices(report).map(text => `Current: ${text}`));
  }
  const eitherTruncated = baseline.report.sitesTruncated || baseline.report.sitesOmitted > 0
    || Boolean(report && (report.sitesTruncated || report.sitesOmitted > 0));
  if (eitherTruncated) {
    notices.push("Sites omitted from either report cannot be matched; an added or removed row can reflect a truncated list rather than an allocation change.");
  }
  if (notices.length) {
    block.append(el("div", { class: "warn-inline memory-site-truncation memory-diff-truncation" },
      notices.map(text => el("p", { class: "memory-note", text }))));
  }

  if (baseline.target.id !== state.targetId) {
    block.append(allocationSiteState("error", "Baseline belongs to another target",
      `The baseline was captured on ${baseline.target.id || "an unknown target"}; this view is comparing ${state.targetId || "no target"}. No sites are compared across targets.`));
    return block;
  }

  if (!report) {
    block.append(allocationSiteState("memory", "No current report",
      "The baseline is retained, but the runtime has not returned an allocation-site report for this target. Start or query sampling to compute a diff."));
    return block;
  }

  const currentRate = allocationSiteRateValue(report.rate) ?? allocationSiteRateValue(allocation.rate);
  if (currentRate !== null && currentRate !== baseline.rate) {
    block.append(allocationSiteState("memory", "Sampling rate changed",
      `The baseline sampled every ${formatCount(baseline.rate)} allocation${baseline.rate === 1 ? "" : "s"}; the current report samples every ${formatCount(currentRate)}. Cumulative sampled bytes and blocks at different rates are not comparable, so no site diff is computed. Capture a baseline at the current rate.`,
      { action: "Capture baseline", onAction: () => allocationCaptureBaseline() }));
    return block;
  }

  const sameSession = baseline.session !== null && baseline.session === allocationSessionId(allocation);
  if (!sameSession) {
    block.append(el("div", { class: "warn-inline memory-diff-session" },
      el("p", { class: "memory-note", text: "Different sampling session. The runtime restarts its cumulative sampled counters at each Start, so a negative delta is not freed memory and a positive delta is not live growth. Deltas describe cumulative sampled allocations at the configured rate." })));
  }

  const diff = allocationSiteDiff(baseline.report, report);
  block.append(el("div", { class: "memory-diff-summary" },
    el("span", { class: "is-added", text: `${formatCount(diff.counts.added)} added` }),
    el("span", { text: " · " }),
    el("span", { class: "is-removed", text: `${formatCount(diff.counts.removed)} removed` }),
    el("span", { text: " · " }),
    el("span", { class: "is-changed", text: `${formatCount(diff.counts.changed)} changed` }),
    el("span", { text: " · " }),
    el("span", { class: "is-unchanged", text: `${formatCount(diff.counts.unchanged)} unchanged` }),
  ));

  const totalLine = (label, total, format) => {
    if (total.before === null && total.after === null) {
      return el("p", { class: "memory-note", text: `${label} is not reported in either report; no total delta is computed.` });
    }
    if (total.before === null || total.after === null) {
      return el("p", { class: "memory-note", text: `${label} is only reported in the ${total.before === null ? "current" : "baseline"} report; no total delta is computed.` });
    }
    return el("p", { class: `memory-diff-total ${allocationDeltaClass(total.delta)}` },
      `${label} ${format(total.before)} → ${format(total.after)} · ${format === fmtBytes ? allocationDeltaBytesText(total.delta) : allocationDeltaCountText(total.delta)} · ${allocationDeltaPercentText(total.percent)}`);
  };
  block.append(totalLine("Sampled bytes", diff.totalBytes, fmtBytes));
  block.append(totalLine("Sampled blocks", diff.totalBlocks, formatCount));
  if (sameSession && ((diff.totalBytes.delta ?? 0) < 0 || (diff.totalBlocks.delta ?? 0) < 0)) {
    block.append(el("p", { class: "memory-note", text: "A decrease can come from the sampler's fixed ring dropping older samples; it is not freed memory or live heap." }));
  }

  if (diff.unmatchableBefore || diff.unmatchableAfter) {
    const sides = [];
    if (diff.unmatchableBefore) sides.push(`${formatCount(diff.unmatchableBefore)} in the baseline`);
    if (diff.unmatchableAfter) sides.push(`${formatCount(diff.unmatchableAfter)} in the current report`);
    block.append(el("p", { class: "memory-note", text: `${sides.join(" and ")} have no captured stack and cannot be matched; they are excluded from this diff.` }));
  }
  if (diff.duplicates > 0) {
    block.append(el("p", { class: "memory-note", text: `${formatCount(diff.duplicates)} duplicate stack identities were combined for this diff.` }));
  }
  if (sameSession && diff.counts.removed > 0) {
    block.append(el("p", { class: "memory-note", text: "Within one sampling session a removed row means the site is absent from this report's list (for example dropped from the retained ring), not that its allocations were freed." }));
  }

  if (!diff.changed.length) {
    block.append(el("p", { class: "memory-note", text: "No added, removed or changed sampled sites between the baseline and the current report." }));
  } else {
    const tableBody = el("tbody");
    diff.changed.forEach((row, index) => {
      const rank = index + 1;
      const detailsId = `alloc-diff-stack-${rank}`;
      const expanded = allocation.diffExpanded.has(row.key);
      const top = allocationSiteTopFrame(row.site);
      const toggle = el("button", {
        class: "link-button memory-site-toggle",
        props: { type: "button" },
        attrs: {
          "aria-expanded": String(expanded),
          "aria-controls": detailsId,
          "aria-label": `${expanded ? "Hide" : "Show"} stack for ${row.status} site ${rank}`,
        },
      },
        el("span", { class: "memory-site-top-name", text: top.name }),
        top.location ? el("span", { class: "memory-site-top-location", text: top.location }) : null,
        top.address && top.name !== top.address ? el("code", { class: "memory-site-top-address", text: top.address }) : null,
      );
      const details = el("tr", { class: "memory-site-stack-row memory-diff-stack-row", attrs: { id: detailsId } },
        el("td", { attrs: { colspan: "8" } }, renderAllocationSiteStack(row.site)),
      );
      details.hidden = !expanded;
      toggle.addEventListener("click", () => {
        const show = details.hidden;
        details.hidden = !show;
        toggle.setAttribute("aria-expanded", String(show));
        toggle.setAttribute("aria-label", `${show ? "Hide" : "Show"} stack for ${row.status} site ${rank}`);
        if (show) allocation.diffExpanded.add(row.key);
        else allocation.diffExpanded.delete(row.key);
      });
      tableBody.append(el("tr", {
        class: `memory-diff-row is-${row.status}`,
        attrs: { "data-allocation-diff-status": row.status },
      },
        el("td", {}, el("span", { class: `memory-diff-status is-${row.status}`, text: allocationDiffStatusLabel(row.status) })),
        el("td", { class: "num memory-bytes" },
          allocationDiffSampleCell(row.before?.bytes ?? null, Boolean(row.before), "bytes"),
          " → ",
          allocationDiffSampleCell(row.after?.bytes ?? null, Boolean(row.after), "bytes"),
        ),
        el("td", { class: `num memory-diff-delta ${allocationDeltaClass(row.deltaBytes)}`, text: allocationDeltaBytesText(row.deltaBytes) }),
        el("td", { class: `num memory-diff-delta ${allocationDeltaClass(row.deltaBytes)}`, text: allocationDeltaPercentText(row.percentBytes) }),
        el("td", { class: "num memory-bytes" },
          allocationDiffSampleCell(row.before?.blocks ?? null, Boolean(row.before), "blocks"),
          " → ",
          allocationDiffSampleCell(row.after?.blocks ?? null, Boolean(row.after), "blocks"),
        ),
        el("td", { class: `num memory-diff-delta ${allocationDeltaClass(row.deltaBlocks)}`, text: allocationDeltaCountText(row.deltaBlocks) }),
        el("td", { class: `num memory-diff-delta ${allocationDeltaClass(row.deltaBlocks)}`, text: allocationDeltaPercentText(row.percentBlocks) }),
        el("td", { class: "memory-site-stack-cell" }, toggle),
      ));
      tableBody.append(details);
    });
    block.append(el("div", { class: "table-wrap memory-site-table-wrap memory-diff-table-wrap" },
      el("table", { class: "data-table memory-site-table memory-diff-table" },
        el("thead", {},
          el("tr", {},
            el("th", { attrs: { scope: "col" }, text: "Change" }),
            el("th", { class: "num", attrs: { scope: "col" }, text: "Sampled bytes" }),
            el("th", { class: "num", attrs: { scope: "col" }, text: "Δ bytes" }),
            el("th", { class: "num", attrs: { scope: "col" }, text: "Δ %" }),
            el("th", { class: "num", attrs: { scope: "col" }, text: "Sampled blocks" }),
            el("th", { class: "num", attrs: { scope: "col" }, text: "Δ blocks" }),
            el("th", { class: "num", attrs: { scope: "col" }, text: "Δ %" }),
            el("th", { attrs: { scope: "col" }, text: "Sampling stack" }),
          ),
        ),
        tableBody,
      )));
    if (diff.counts.added || diff.counts.removed) {
      block.append(el("p", {
        class: "memory-note",
        text: "A dash means the site was absent from that report, not zero sampled bytes. Percentages are relative to the baseline sample; absent sites have no percentage.",
      }));
    }
  }
  return block;
}

function renderAllocationSitesBody(allocation, report) {
  const failure = allocationSiteFailureKind(allocation);
  const body = el("div", { class: "memory-site-body" });
  if (failure === "offline") {
    body.append(allocationSiteState("error", "Disconnected", allocation.reason || "The controller connection dropped. Sampling state is unknown until it reconnects."));
  } else if (failure === "unsupported") {
    body.append(allocationSiteState("memory", "Allocation sampling is unavailable", allocation.reason || "The runtime reports that stack capture is not available on this target. No sites are substituted."));
  } else if (failure === "unavailable") {
    body.append(allocationSiteState("memory", "Allocation sites are unavailable", allocation.reason || "This controller does not expose allocation-site sampling. No sites are substituted."));
  } else if (failure === "error") {
    body.append(allocationSiteState("error", "Allocation-site read failed", allocation.reason || errorMessage(allocation.error), {
      action: "Retry",
      onAction: () => queryAllocationSites({ force: true }),
    }));
  } else if (allocation.status === "starting") {
    body.append(allocationSiteState("loading", "Starting allocation sampling", "Waiting for the runtime to enable cove_alloc's sampler. No sites are shown until the runtime reports them."));
  } else if (allocation.status === "stopping") {
    body.append(allocationSiteState("loading", "Stopping allocation sampling", "Waiting for the runtime to aggregate the retained stacks."));
  } else if (allocation.status === "idle") {
    body.append(allocationSiteState("memory", "Not sampling", "Start enables the runtime's opt-in sampler. Sites appear only after allocations are sampled; nothing is inferred while it is off."));
  } else if (allocation.status === "running" && !report) {
    body.append(allocationSiteState("memory", "Sampling is running", "No counters have arrived yet. Query to read the sites collected so far; allocations made before Start are not sampled."));
  } else if (allocation.status === "running" && report && report.sites.length === 0) {
    body.append(allocationSiteState("memory", "No sampled allocations yet", "Sampling is running. Allocations made after Start appear here once the sampler captures a matching stack; no site is inferred."));
  } else if (allocation.status === "stopped" && !report) {
    body.append(allocationSiteState("memory", "No allocation-site report", allocation.reason || "Sampling stopped without returning a retained report. No sites are substituted."));
  } else if (allocation.status === "stopped" && report && report.sites.length === 0) {
    body.append(allocationSiteState("memory", "No retained allocation sites", "Sampling stopped without retaining a matching site. The counters below are the sampler's own attempts, drops and capture failures."));
  } else if (report && report.sites.length > 0) {
    body.append(renderAllocationSiteTable(report, allocation));
  }
  if (report) {
    const notices = allocationSiteTruncationNotices(report);
    if (notices.length) {
      body.prepend(el("div", { class: "warn-inline memory-site-truncation" },
        notices.map(text => el("p", { class: "memory-note", text }))));
    }
    if (failure || allocation.status === "running" || allocation.status === "stopped") {
      body.append(el("div", { class: "metrics memory-site-metrics" },
        allocationSiteMetric("Sampled bytes", report.sampledBytes, fmtBytes, "Cumulative bytes on sampled stacks for this session at the configured rate; not live heap."),
        allocationSiteMetric("Sampled blocks", report.sampledBlocks, formatCount, "Cumulative blocks on sampled stacks for this session at the configured rate; not live heap."),
        allocationSiteMetric("Attempted", report.samplesAttempted, formatCount, "Sampling triggers: every Nth successful fresh allocation."),
        allocationSiteMetric("Retained", report.samplesRetained, formatCount, "Samples retained in the sampler's fixed ring."),
        allocationSiteMetric("Dropped", report.samplesDropped, formatCount, "Samples overwritten, lost to a slot race or rejected as incomplete."),
        allocationSiteMetric("Capture failures", report.captureFailures, formatCount, "Sampling triggers where stack capture produced no caller frame."),
      ));
    }
  }
  return body;
}

function renderMemoryAllocationSites() {
  const allocation = state.memory.allocation;
  const report = allocation.report;
  const target = targetById();
  const offline = Boolean(state.token && target && state.caps.runtime && !state.connected);
  const busy = allocation.loading && !allocation.polling;
  const running = ["running", "starting", "stopping"].includes(allocation.status);
  const failure = allocationSiteFailureKind(allocation);
  const controlsBlocked = offline || busy || failure === "unsupported" || failure === "unavailable";
  const start = el("button", {
    class: "tool-button is-primary",
    props: { type: "button" },
    text: "Start",
    attrs: { title: "Enable cove_alloc's sampler in the runtime" },
    on: { click: () => startAllocationSites() },
  });
  start.disabled = controlsBlocked || running;
  const stopButton = el("button", {
    class: "tool-button",
    props: { type: "button" },
    text: "Stop",
    attrs: { title: "Stop sampling and collect the retained sites" },
    on: { click: () => stopAllocationSites() },
  });
  stopButton.disabled = offline || busy || !["running", "starting"].includes(allocation.status);
  const query = el("button", {
    class: "tool-button",
    props: { type: "button" },
    text: "Query",
    attrs: { title: "Read the sites collected so far without ending the session" },
    on: { click: () => queryAllocationSites({ force: true }) },
  });
  query.disabled = offline || busy;
  const reset = el("button", {
    class: "tool-button",
    props: { type: "button" },
    text: "Clear",
    attrs: { title: "Discard the stopped report from this view; while sampling, ask the controller to clear its retained sites" },
    on: { click: () => resetAllocationSites() },
  });
  reset.disabled = offline || busy || (!report && !running);
  const showBaseline = !["unsupported", "unavailable"].includes(allocation.status) || Boolean(allocation.baseline);
  return el("div", { class: "panel-block memory-allocation-sites", attrs: { id: "memory-allocation-sites" } },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Allocation sites" }),
      el("span", { class: `state-badge ${allocationSiteStatusClass(allocation)}`, text: allocationSiteStatusLabel(allocation) }),
      el("span", { class: "pane-meta", attrs: { title: "Sampling rate and the runtime's retained sample count" }, text: allocationSiteMeta(allocation) }),
    ),
    el("div", { class: "panel-block-body" },
      allocationSiteScopeNote(),
      el("div", { class: "memory-site-toolbar" },
        allocationSiteRateControl(allocation, controlsBlocked || running),
        el("div", { class: "profile-actions" }, start, stopButton, query, reset),
      ),
      allocation.error && failure === "error" ? errorInline(errorMessage(allocation.error)) : null,
      renderAllocationSitesBody(allocation, report),
      showBaseline ? renderAllocationBaseline(allocation, report) : null,
      el("p", { class: "memory-note", text: "Sampled bytes and blocks are cumulative for this session at the configured 1-in-N rate. They are not live heap, object counts, object types, retaining paths or leaks. The sampler counts fresh allocations only; reallocations are not sampled." }),
    ),
  );
}

function renderAllocationSitesView(options = {}) {
  if (state.view !== "memory") return;
  const host = dom.memoryContent;
  const current = host.querySelector("#memory-allocation-sites");
  if (!current) {
    renderMemory();
    return;
  }
  if (options.preserveFocus && current.contains(document.activeElement)) return;
  const scrollTop = host.scrollTop;
  current.replaceWith(renderMemoryAllocationSites());
  host.scrollTop = scrollTop;
}

function refreshMemoryView() {
  loadMemory({ reset: false });
  queryAllocationSites({ quiet: true });
}

function emptyMemoryState(message) {
  return el("div", { class: "state" },
    el("span", { class: "state-icon" }, icon("memory")),
    el("strong", { text: "No memory counters" }),
    el("p", { text: message }),
  );
}

function memoryScopeNote() {
  return el("div", { class: "memory-scope" },
    icon("info"),
    el("span", {},
      el("strong", { text: "Allocator tracking, not a heap object graph." }),
      " Process RSS and process virtual are real readings of the host process. Allocator counters, when the host installs a tracking allocator, are real totals from that allocator and cover allocated bytes and block counts only. Rust has no GC, so no GC generations, retaining paths or leak tracking are reported, and object types are not tracked. Cache and render-target values are host estimates; drawable pools, driver memory and external scene textures are excluded.",
    ),
  );
}

function renderMemoryAllocator(snapshot) {
  const allocation = snapshot.allocation;
  const block = el("div", { class: "panel-block memory-allocation" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Allocator tracking" }),
      el("span", {
        class: "pane-meta",
        text: allocation.reported ? `${allocation.reported} of ${MEMORY_ALLOCATION_ROWS.length * 2} reported` : "Not reported",
      }),
    ),
  );
  if (!allocation.reported) {
    block.append(el("div", { class: "panel-block-body" },
      el("p", { class: "memory-note", text: "The host installs no tracking allocator, so allocated bytes, block counts and size classes are not reported. RSS, cache and render values below are separate host readings and are not reused as allocation totals." }),
    ));
    return block;
  }
  const rows = allocation.rows.map(row => el("tr", {},
    el("td", { text: row.label, attrs: { title: row.description } }),
    el("td", { class: "num memory-bytes", text: row.bytes === null ? "Not reported" : fmtBytes(row.bytes) }),
    el("td", { class: "num memory-bytes", text: row.blocks === null ? "Not reported" : formatCount(row.blocks) }),
  ));
  block.append(el("div", { class: "table-wrap" }, el("table", { class: "data-table" },
    el("thead", {}, el("tr", {},
      el("th", { text: "Measure" }),
      el("th", { class: "num", text: "Bytes" }),
      el("th", { class: "num", text: "Blocks" }),
    )),
    el("tbody", {}, ...rows),
  )));
  return block;
}

function renderMemorySizeClasses(snapshot) {
  const entries = snapshot.sizeClasses;
  if (!entries.length) return null;
  const sorted = entries.slice().sort((left, right) => {
    const leftSize = memorySizeClassSortValue(left.label);
    const rightSize = memorySizeClassSortValue(right.label);
    if (leftSize !== rightSize) return leftSize < rightSize ? -1 : 1;
    return left.label.localeCompare(right.label);
  });
  const limit = 16;
  const shown = sorted.slice(0, limit);
  const showBlocks = shown.some(entry => entry.blocks !== null);
  const showBytes = shown.some(entry => entry.bytes !== null);
  return el("div", { class: "panel-block memory-size-classes" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Allocator size classes" }),
      el("span", { class: "pane-meta", text: `${sorted.length} class${sorted.length === 1 ? "" : "es"}` }),
    ),
    el("div", { class: "table-wrap" }, el("table", { class: "data-table" },
      el("thead", {}, el("tr", {},
        el("th", { text: "Size class" }),
        showBlocks ? el("th", { class: "num", text: "Live blocks" }) : null,
        showBytes ? el("th", { class: "num", text: "Live bytes" }) : null,
      )),
      el("tbody", {}, ...shown.map(entry => el("tr", {},
        el("td", { class: "memory-size-class", text: memorySizeClassLabel(entry.label) }),
        showBlocks ? el("td", { class: "num memory-bytes", text: entry.blocks === null ? "Not reported" : formatCount(entry.blocks) }) : null,
        showBytes ? el("td", { class: "num memory-bytes", text: entry.bytes === null ? "Not reported" : fmtBytes(entry.bytes) }) : null,
      ))),
    )),
    sorted.length > limit
      ? el("div", { class: "panel-block-body" },
        el("p", { class: "memory-note", text: `Showing ${limit} of ${sorted.length} size classes.` }))
      : null,
  );
}

function renderMemoryStats(snapshot) {
  const counters = snapshot.counters;
  const windowCount = snapshot.windows.size;
  const cacheBytes = sumMemoryCounters(counters, ["shapingBytes", "layoutBytes", "imageBytes"]);
  const targetBytes = memoryWindowTotals(snapshot.windows, "targetBytes");
  const msaaAllocated = memoryWindowTotals(snapshot.windows, "msaaAllocatedBytes");
  const msaaLogical = memoryWindowTotals(snapshot.windows, "msaaLogicalBytes");
  const rss = memoryCounterValue(counters, "processResidentBytes");
  const virtualBytes = memoryCounterValue(counters, "processVirtualBytes");
  return el("div", { class: "stat-row" },
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Process RSS" }),
      el("div", { class: "stat-value" }, rss === null ? "Not reported" : fmtBytes(rss))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Process virtual" }),
      el("div", { class: "stat-value" }, virtualBytes === null ? "Not reported" : fmtBytes(virtualBytes))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Logical caches" }),
      el("div", { class: "stat-value" }, fmtBytes(cacheBytes), el("small", { text: " estimated" }))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Image assets" }),
      el("div", { class: "stat-value" }, formatCount(memoryCounterValue(counters, "imageEntries")))) ,
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Render targets" }),
      el("div", { class: "stat-value" }, fmtBytes(targetBytes), el("small", { text: " estimated" }))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "MSAA allocated" }),
      el("div", { class: "stat-value" }, msaaAllocated === null ? "Not reported" : fmtBytes(msaaAllocated))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "MSAA logical" }),
      el("div", { class: "stat-value" }, fmtBytes(msaaLogical))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Windows" }),
      el("div", { class: "stat-value" }, String(windowCount))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Skipped parents" }),
      el("div", { class: "stat-value" }, formatCount(memoryCounterValue(counters, "skippedParentUpdates")))),
    el("div", { class: "stat" },
      el("div", { class: "stat-label", text: "Memory pressure" }),
      el("div", {
        class: "stat-value",
        attrs: {
          title: snapshot.pressure === null
            ? "The host reported no memory-pressure field."
            : snapshot.pressure.subscribed
              ? "The host subscribes to the platform memory-pressure signal."
              : "The host never wired the platform memory-pressure signal; this is not a measured normal.",
        },
      }, snapshot.pressure === null ? "Not reported" : snapshot.pressure.subscribed ? (snapshot.pressure.low ? "Low" : "Normal") : "Not observed")),
  );
}

function renderMemoryProcess(snapshot) {
  const counters = snapshot.counters;
  let reported = 0;
  const rows = MEMORY_PROCESS_COUNTERS.map(([key, label]) => {
    const bytes = memoryCounterValue(counters, key);
    if (bytes !== null) reported += 1;
    return el("tr", {},
      el("td", { text: label, attrs: { title: MEMORY_BYTE_LABELS[key][1] } }),
      el("td", { class: "num memory-bytes", text: bytes === null ? "Not reported" : fmtBytes(bytes) }),
    );
  });
  return el("div", { class: "panel-block" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Host process" }),
      el("span", { class: "pane-meta", text: `${reported} of ${MEMORY_PROCESS_COUNTERS.length} reported` }),
    ),
    el("div", { class: "table-wrap" }, el("table", { class: "data-table" },
      el("thead", {}, el("tr", {},
        el("th", { text: "Measure" }),
        el("th", { class: "num", text: "Bytes" }),
      )),
      el("tbody", {}, ...rows),
    )),
  );
}

function renderMemoryCounters(snapshot) {
  const counters = snapshot.counters;
  const rows = [];
  const addRow = (label, entries, bytes, description) => {
    rows.push(el("tr", {},
      el("td", { text: label, attrs: { title: description || "" } }),
      el("td", { class: "num", text: entries === null ? "—" : formatCount(entries) }),
      el("td", { class: "num memory-bytes", text: bytes === null ? "—" : fmtBytes(bytes) }),
    ));
  };
  addRow("Shaping cache", memoryCounterValue(counters, "shapingEntries"), memoryCounterValue(counters, "shapingBytes"), MEMORY_BYTE_LABELS.shapingBytes[1]);
  addRow("Text layout cache", memoryCounterValue(counters, "layoutEntries"), memoryCounterValue(counters, "layoutBytes"), MEMORY_BYTE_LABELS.layoutBytes[1]);
  addRow("Image cache", memoryCounterValue(counters, "imageEntries"), memoryCounterValue(counters, "imageBytes"), MEMORY_BYTE_LABELS.imageBytes[1]);
  const known = new Set(["shapingEntries", "shapingBytes", "layoutEntries", "layoutBytes", "imageEntries", "imageBytes", "skippedParentUpdates", "processResidentBytes", "processVirtualBytes"]);
  for (const name of snapshot.allocation.consumed) known.add(name);
  const other = Object.entries(counters).filter(([key]) => !known.has(key));
  if (other.length) {
    for (const [key, value] of other.slice(0, MAX_MEMORY_COUNTERS)) {
      const label = MEMORY_ENTRY_LABELS[key] || key;
      addRow(label, value, key.toLowerCase().includes("bytes") ? value : null, `Reported counter ${key}.`);
    }
  }
  return el("div", { class: "panel-block" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Cache and render counters" }),
      el("span", { class: "pane-meta", text: `${rows.length} counters` }),
    ),
    el("div", { class: "table-wrap" }, el("table", { class: "data-table" },
      el("thead", {}, el("tr", {},
        el("th", { text: "Counter" }),
        el("th", { class: "num", text: "Entries" }),
        el("th", { class: "num", text: "Bytes" }),
      )),
      el("tbody", {}, ...rows),
    )),
  );
}

function renderMemoryWindows(snapshot) {
  const ids = [...snapshot.windows.keys()];
  if (!ids.length) return null;
  return el("div", { class: "panel-block" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Per-window render targets" }),
      el("span", { class: "pane-meta", text: `${ids.length} window${ids.length === 1 ? "" : "s"}` }),
    ),
    el("div", { class: "table-wrap" }, el("table", { class: "data-table" },
      el("thead", {}, el("tr", {},
        el("th", { text: "Window" }),
        el("th", { class: "num", text: "Target estimate" }),
        el("th", { class: "num", text: "MSAA logical" }),
        el("th", { class: "num", text: "MSAA allocated" }),
        el("th", { class: "num", text: "Samples" }),
        el("th", { text: "State" }),
      )),
      el("tbody", {}, ...ids.map(id => {
        const counters = snapshot.windows.get(id);
        const suspended = memoryCounterValue(counters, "suspended");
        const swapchain = memoryCounterValue(counters, "hasSwapchain");
        const state = suspended === 1 ? "Suspended" : swapchain === 0 ? "No swapchain" : "Active";
        return el("tr", {},
          el("td", { class: "memory-window-id", text: id, attrs: { title: id } }),
          el("td", { class: "num memory-bytes", text: fmtBytes(memoryCounterValue(counters, "targetBytes")) }),
          el("td", { class: "num memory-bytes", text: fmtBytes(memoryCounterValue(counters, "msaaLogicalBytes")) }),
          el("td", { class: "num memory-bytes", text: fmtBytes(memoryCounterValue(counters, "msaaAllocatedBytes")) }),
          el("td", { class: "num", text: formatCount(memoryCounterValue(counters, "samples")) }),
          el("td", { text: state }),
        );
      })),
    )),
  );
}

function setMemoryChrome() {
  const target = targetById();
  const memory = state.memory;
  dom.memoryRefresh.disabled = !state.token || !state.caps.runtime || memory.loading;
  dom.memoryFollow.disabled = !state.token || !state.connected || !state.caps.runtime;
  dom.memoryInterval.disabled = dom.memoryFollow.disabled;
  dom.memoryInterval.value = String(memory.intervalMs);
  if (!state.token || !target) {
    dom.memoryMeta.textContent = "No target selected.";
    return;
  }
  if (!state.caps.runtime) {
    dom.memoryMeta.textContent = "Memory counters come from a Cove runtime. Select a runtime target.";
    return;
  }
  if (memory.loading) {
    dom.memoryMeta.textContent = "Reading memory counters…";
    return;
  }
  if (memory.error && !memory.loaded) {
    dom.memoryMeta.textContent = "Memory counter read failed.";
    return;
  }
  if (memory.data === null) {
    dom.memoryMeta.textContent = "This host has no memory provider. No process or cache readings are available.";
    return;
  }
  const snapshot = normalizeMemorySnapshot(memory.data);
  if (!snapshot) {
    dom.memoryMeta.textContent = "The host returned no counter object: no process or cache readings.";
    return;
  }
  const cacheBytes = sumMemoryCounters(snapshot.counters, ["shapingBytes", "layoutBytes", "imageBytes"]);
  const rss = memoryCounterValue(snapshot.counters, "processResidentBytes");
  const parts = [];
  if (rss !== null) parts.push(`${fmtBytes(rss)} RSS`);
  const allocation = snapshot.allocation;
  const currentBytes = allocation.rows.find(row => row.label === "Current")?.bytes ?? null;
  if (currentBytes !== null) parts.push(`${fmtBytes(currentBytes)} allocated`);
  parts.push(`${fmtBytes(cacheBytes)} in logical caches`);
  parts.push(`${snapshot.windows.size} window${snapshot.windows.size === 1 ? "" : "s"}`);
  parts.push(`updated ${formatWallTime(memory.refreshedAt)}`);
  dom.memoryMeta.textContent = allocation.reported
    ? `${parts.join(" · ")}. RSS and allocator counters are measured; caches are estimates.`
    : `${parts.join(" · ")}. RSS is measured; no allocator counters are reported; caches are estimates.`;
}

const MEMORY_SERIES = [
  { key: "rss", label: "Process RSS" },
  { key: "allocated", label: "Allocator current" },
  { key: "caches", label: "Logical caches" },
  { key: "targets", label: "Render targets" },
  { key: "virtual", label: "Process virtual" },
];

function memoryChartScale(values) {
  const max = values.reduce((maximum, value) => Math.max(maximum, value), 0);
  if (max <= 0) return 1024;
  const base = 10 ** Math.floor(Math.log10(max));
  for (const step of [1, 2, 5, 10]) {
    if (step * base >= max) return step * base;
  }
  return 10 * base;
}

function memorySeriesPoints(history, key, scale, origin, span) {
  const points = [];
  for (const sample of history) {
    const value = sample[key];
    if (value === null) continue;
    points.push({
      x: span > 0 ? ((sample.at - origin) / span) * 100 : 0,
      y: Math.max(0, Math.min(100, (value / scale) * 100)),
      sample,
    });
  }
  return points;
}

function renderMemoryChart(memory) {
  const history = memory.history;
  const reset = el("button", {
    class: "tool-button",
    attrs: { type: "button", title: "Discard the recorded samples and start a new series" },
    text: "Reset",
    on: {
      click: () => {
        memory.history = [];
        renderMemory();
      },
    },
  });
  const head = el("div", { class: "panel-block-head" }, el("h2", { text: "Memory overview" }));
  if (!history.length) {
    head.append(reset);
    return el("div", { class: "panel-block" }, head,
      el("div", { class: "panel-block-body" },
        emptyInline("No samples recorded yet. Refresh or enable Follow to append provider readings to the series."),
      ),
    );
  }
  const origin = history[0].at;
  const latest = history[history.length - 1].at;
  const span = latest - origin;
  const visible = MEMORY_SERIES.filter(series => memory.series[series.key]);
  const values = [];
  for (const series of visible) {
    for (const sample of history) {
      if (sample[series.key] !== null) values.push(sample[series.key]);
    }
  }
  const scale = memoryChartScale(values);
  head.append(
    el("span", { class: "pane-meta", text: `${history.length} sample${history.length === 1 ? "" : "s"} · ${fmtSeconds(span)} · ${fmtBytes(scale)} scale` }),
    reset,
  );
  const plot = el("div", {
    class: "memory-chart-plot",
    attrs: {
      role: "img",
      "aria-label": `Memory overview: ${history.length} samples over ${fmtSeconds(span)}, scale ${fmtBytes(scale)}.`,
    },
  });
  for (const fraction of [0.25, 0.5, 0.75, 1]) {
    plot.append(el("div", { class: "memory-grid", attrs: { style: `bottom:${fraction * 100}%` } }));
  }
  for (const series of visible) {
    const points = memorySeriesPoints(history, series.key, scale, origin, span);
    if (!points.length) continue;
    if (points.length === 1) {
      plot.append(el("div", {
        class: `memory-dot is-${series.key}`,
        attrs: {
          style: `left:${points[0].x}%;bottom:${points[0].y}%`,
          title: `${series.label} ${fmtBytes(points[0].sample[series.key])} at ${fmtEpochMs(points[0].sample.at)}`,
        },
      }));
      continue;
    }
    const forward = points.map(point => `${point.x}% ${100 - point.y}%`).join(", ");
    plot.append(el("div", {
      class: `memory-area is-${series.key}`,
      attrs: { style: `clip-path:polygon(${forward}, ${points[points.length - 1].x}% 100%, ${points[0].x}% 100%)` },
    }));
    const backward = points.slice().reverse().map(point => `${point.x}% calc(${100 - point.y}% - 2px)`).join(", ");
    plot.append(el("div", {
      class: `memory-line is-${series.key}`,
      attrs: { style: `clip-path:polygon(${forward}, ${backward})` },
    }));
  }
  let pressureMarks = 0;
  for (const sample of history) {
    if (!sample.pressure?.subscribed || !sample.pressure.low) continue;
    pressureMarks += 1;
    const x = span > 0 ? ((sample.at - origin) / span) * 100 : 0;
    plot.append(el("div", {
      class: "memory-pressure",
      attrs: { style: `left:${x}%`, title: `Memory-pressure signal at ${fmtEpochMs(sample.at)}` },
    }));
  }
  const cursor = el("div", { class: "memory-cursor", props: { hidden: true } });
  plot.append(cursor);
  const readout = el("div", { class: "memory-chart-readout", attrs: { role: "status" } });
  const describe = sample => `${fmtEpochMs(sample.at)} · ` + visible
    .map(series => `${series.label} ${sample[series.key] === null ? "not reported" : fmtBytes(sample[series.key])}`)
    .join(" · ");
  readout.textContent = describe(history[history.length - 1]);
  plot.addEventListener("pointermove", event => {
    const rect = plot.getBoundingClientRect();
    if (rect.width <= 0) return;
    const ratio = Math.max(0, Math.min(1, (event.clientX - rect.left) / rect.width));
    const at = origin + ratio * span;
    let nearest = history[0];
    for (const sample of history) {
      if (Math.abs(sample.at - at) < Math.abs(nearest.at - at)) nearest = sample;
    }
    cursor.hidden = false;
    cursor.style.left = `${span > 0 ? ((nearest.at - origin) / span) * 100 : 0}%`;
    readout.textContent = describe(nearest);
  });
  plot.addEventListener("pointerleave", () => {
    cursor.hidden = true;
    readout.textContent = describe(history[history.length - 1]);
  });
  const yaxis = el("div", { class: "memory-yaxis" },
    ...[0, 0.25, 0.5, 0.75, 1].map(fraction => el("span", {
      attrs: { style: `bottom:${fraction * 100}%` },
      text: fmtBytes(scale * fraction),
    })),
  );
  const legend = el("div", { class: "perf-legend memory-legend" },
    ...MEMORY_SERIES.map(series => el("button", {
      class: `memory-legend-toggle is-${series.key}${memory.series[series.key] ? " is-on" : ""}`,
      attrs: { type: "button", "aria-pressed": String(memory.series[series.key]), title: `Show or hide ${series.label} on the overview chart` },
      text: series.label,
      on: {
        click: () => {
          memory.series[series.key] = !memory.series[series.key];
          renderMemory();
        },
      },
    })),
    el("span", { class: "is-pressure", text: "Pressure signal" }),
  );
  const notes = [
    "Each point is one query_ui_memory reading. Lines connect reported readings; a gap is never filled with an invented sample.",
    "RSS covers the whole process. Allocator current, logical caches and render targets are subsets or estimates, drawn as separate series and never stacked into a total.",
  ];
  if (visible.some(series => history.some(sample => sample[series.key] === null))) {
    notes.push("A series with unreported readings plots only the readings the host returned.");
  }
  notes.push(pressureMarks
    ? "Vertical marks are the host's subscribed memory-pressure signal at the sample that observed it."
    : "No subscribed memory-pressure signal was observed in these samples, so no pressure marks are drawn.");
  if (history.length === 1) {
    notes.push("One sample: a trend needs at least two readings, so each series is drawn as a single point.");
  }
  return el("div", { class: "panel-block" }, head,
    el("div", { class: "panel-block-body" },
      el("div", { class: "memory-chart" },
        yaxis,
        plot,
        el("div", { class: "memory-xaxis" },
          el("span", { text: fmtEpochMs(origin) }),
          el("span", { text: fmtSeconds(span) }),
          el("span", { text: fmtEpochMs(latest) }),
        ),
      ),
      readout,
      legend,
      ...notes.map(note => el("div", { class: "perf-note", text: note })),
    ),
  );
}

function renderMemory() {
  const host = dom.memoryContent;
  clear(host);
  setMemoryChrome();
  const memory = state.memory;
  const target = targetById();
  if (!state.token) {
    showState(host, { icon: "device", title: "Not connected", message: "Connect to a controller to read memory counters." });
    return;
  }
  if (!target) {
    showState(host, { icon: "device", title: "No target", message: "Select a connected Cove runtime to read memory counters." });
    return;
  }
  if (!state.caps.runtime) {
    showState(host, { icon: "memory", title: "Memory counters are unavailable", message: "Memory counters come from a Cove runtime. Select a runtime target." });
    return;
  }
  if (memory.loading && !memory.loaded) {
    showState(host, { kind: "loading", title: "Reading memory counters", message: "Waiting for the runtime's query_ui_memory response." });
    return;
  }
  if (memory.error && !memory.loaded) {
    showState(host, {
      kind: "error",
      title: "Memory counter read failed",
      message: errorMessage(memory.error),
      action: "Retry",
      onAction: () => loadMemory({ reset: true }),
    });
    return;
  }
  if (memory.data === null) {
    showState(host, {
      icon: "memory",
      title: "This host has no memory provider",
      message: "The runtime returned null for query_ui_memory. Process RSS, cache counters and render-target estimates are all unavailable; nothing is substituted.",
      action: "Refresh",
      onAction: () => loadMemory({ reset: true }),
    });
    return;
  }
  const snapshot = normalizeMemorySnapshot(memory.data);
  if (!snapshot) {
    showState(host, {
      icon: "memory",
      title: "No memory counters",
      message: "The host returned a response without a counters object. No process or cache readings are shown.",
      action: "Refresh",
      onAction: () => loadMemory({ reset: true }),
    });
    return;
  }
  host.append(memoryScopeNote());
  if (memory.error) host.append(errorInline(errorMessage(memory.error)));
  host.append(renderMemoryChart(memory));
  host.append(renderMemoryStats(snapshot));
  host.append(renderMemoryAllocator(snapshot));
  host.append(renderMemoryAllocationSites());
  const sizeClasses = renderMemorySizeClasses(snapshot);
  if (sizeClasses) host.append(sizeClasses);
  host.append(renderMemoryProcess(snapshot));
  const counterCount = Object.keys(snapshot.counters).length;
  if (counterCount) host.append(renderMemoryCounters(snapshot));
  const windows = renderMemoryWindows(snapshot);
  if (windows) host.append(windows);
  if (!counterCount && !snapshot.windows.size) {
    const block = el("div");
    block.append(emptyMemoryState("The host returned empty counters and no windows. No process reading is substituted."));
    host.append(block);
  } else {
    host.append(el("div", { class: "perf-note", text: "Process RSS and process virtual are read from the operating system for the host process. They cover the whole process, not the UI scene, and an unreported reading is shown as Not reported rather than as zero." }));
    host.append(el("div", { class: "perf-note", text: "targetBytes is the host's own estimate for owned MSAA, backdrop and effect scratch. msaaAllocatedBytes is reported only when the backend exposes the allocation; absent values are shown as not reported, never estimated here." }));
    host.append(el("div", { class: "perf-note", text: "Refresh reads a fresh provider snapshot. Allocator counters are point-in-time allocation totals; RSS and virtual are process readings. None of these is a heap object graph, and Rust reports no GC generations or retaining paths." }));
  }
}

async function loadMemory(options = {}) {
  const memory = state.memory;
  if (!state.token || !state.targetId || !state.caps.runtime) {
    renderMemory();
    return;
  }
  if (memory.loading) return;
  const targetId = state.targetId;
  const request = memory.request + 1;
  memory.request = request;
  memory.loading = true;
  memory.error = null;
  if (options.reset) {
    memory.data = null;
    memory.loaded = false;
  }
  renderMemory();
  try {
    const data = unwrapData(await rpc("query_ui_memory", { target: targetId }));
    if (request !== memory.request || targetId !== state.targetId) return;
    state.memory.loaded = true;
    state.memory.data = data === null ? null : data;
    state.memory.refreshedAt = Date.now();
    recordMemorySample(data);
    if (options.quiet !== true) toast("Memory counters refreshed.", "success", 3000);
  } catch (error) {
    if (request !== memory.request || targetId !== state.targetId) return;
    state.memory.error = error;
    if (error?.code === "unknown_command") {
      state.memory.error = new RpcError("This controller does not expose query_ui_memory.", "unknown_command");
    }
    if (options.quiet !== true) toast(errorMessage(error), "error", 8000);
  } finally {
    if (request === memory.request && targetId === state.targetId) {
      state.memory.loading = false;
      renderMemory();
    }
  }
}

function ensureMemory() {
  if (!state.token || !state.targetId || !state.caps.runtime) {
    renderMemory();
    return;
  }
  if (!state.memory.loaded && !state.memory.loading) {
    loadMemory({ quiet: true });
  } else {
    renderMemory();
  }
  ensureAllocationSites();
}

function resetMemory(previousTarget = state.targetId) {
  const previousAllocation = state.memory?.allocation;
  stopAllocationPolling();
  stopTimer("memory");
  if (previousTarget && previousAllocation && ["running", "starting", "stopping"].includes(previousAllocation.status)) {
    releaseAllocationSampling(previousTarget).catch(() => {});
  }
  const request = state.memory.request + 1;
  state.memory = emptyMemory();
  state.memory.request = request;
  if (state.view === "memory") renderMemory();
}

/* Registered State
 *
 * query_ui_state lists only the state a component opted in to expose. Summaries
 * are read for explicitly watched ids, so private state is never requested. An
 * unmounted id is reported as missing rather than dropped silently.
 */

function emptyRegisteredState() {
  return {
    loaded: false, loading: false, error: null, request: 0,
    descriptors: [], total: 0, nextOffset: null, offset: 0, limit: 50,
    scope: "", watched: [], values: {}, missing: [],
    refreshedAt: 0, follow: dom.stateFollow?.checked === true, timer: null,
  };
}

async function loadRegisteredState(offset = 0, options = {}) {
  const registered = state.registeredState;
  if (!state.token || !state.targetId || !state.caps.state) {
    renderRegisteredState();
    return;
  }
  if (registered.loading) return;
  const targetId = state.targetId;
  const request = registered.request + 1;
  registered.request = request;
  registered.loading = true;
  registered.error = null;
  registered.offset = offset;
  renderRegisteredState();
  try {
    const params = { target: targetId, offset, limit: registered.limit };
    if (registered.scope) params.scope_id = registered.scope;
    if (registered.watched.length) params.ids = registered.watched;
    const data = unwrapData(await rpc("query_ui_state", params));
    if (request !== registered.request || targetId !== state.targetId) return;
    const metadata = data?.metadata ?? {};
    registered.descriptors = Array.isArray(metadata.items) ? metadata.items : [];
    registered.total = numberValue(metadata.total) ?? registered.descriptors.length;
    registered.nextOffset = numberValue(metadata.nextOffset);
    registered.values = {};
    for (const value of Array.isArray(data?.values) ? data.values : []) {
      if (typeof value?.id === "string") registered.values[value.id] = value;
    }
    registered.missing = Array.isArray(data?.missingIDs) ? data.missingIDs : [];
    registered.loaded = true;
    registered.refreshedAt = Date.now();
    if (options.quiet !== true) toast("Registered state refreshed.", "success", 3000);
  } catch (error) {
    if (request !== registered.request || targetId !== state.targetId) return;
    registered.error = error;
    if (error?.code === "unknown_command") {
      registered.error = new RpcError("This controller does not expose query_ui_state.", "unknown_command");
    }
    if (options.quiet !== true) toast(errorMessage(error), "error", 8000);
  } finally {
    if (request === registered.request && targetId === state.targetId) {
      registered.loading = false;
      renderRegisteredState();
    }
  }
}

function ensureRegisteredState() {
  if (!state.token || !state.targetId || !state.caps.state) {
    renderRegisteredState();
    return;
  }
  if (!state.registeredState.loaded && !state.registeredState.loading) {
    loadRegisteredState(0, { quiet: true });
  } else {
    renderRegisteredState();
  }
}

function resetRegisteredState() {
  stopTimer("registeredState");
  const request = state.registeredState.request + 1;
  state.registeredState = emptyRegisteredState();
  state.registeredState.request = request;
  if (state.view === "state") renderRegisteredState();
}

function setRegisteredStateChrome() {
  const registered = state.registeredState;
  const scopes = [...new Set(registered.descriptors.map(item => item.scope_id).filter(Boolean))].sort();
  const selected = registered.scope;
  if (selected && !scopes.includes(selected)) scopes.push(selected);
  dom.stateScope.replaceChildren(el("option", { text: "All scopes", attrs: { value: "" } }),
    ...scopes.map(scope => el("option", { text: scope, attrs: { value: scope } })));
  dom.stateScope.value = selected;
  dom.stateScope.disabled = !state.caps.state || !scopes.length;
  dom.stateFollow.checked = registered.follow;
  dom.stateFollow.disabled = !state.caps.state;
  dom.stateRefresh.disabled = !state.caps.state || registered.loading;
  if (!state.token || !state.targetId) {
    dom.stateMeta.textContent = "No target selected.";
  } else if (!state.caps.state) {
    dom.stateMeta.textContent = "Unavailable for this target.";
  } else if (registered.loading) {
    dom.stateMeta.textContent = "Reading…";
  } else if (!registered.loaded) {
    dom.stateMeta.textContent = "";
  } else {
    dom.stateMeta.textContent = `${registered.total} registered · ${registered.watched.length} watched`;
  }
}

function renderRegisteredStateRow(descriptor) {
  const registered = state.registeredState;
  const id = descriptor.id;
  const watched = registered.watched.includes(id);
  const value = registered.values[id];
  const missing = registered.missing.includes(id);
  const summary = missing
    ? "Unmounted"
    : value === undefined
      ? (watched ? "Not reported" : "Not watched")
      : value.summary;
  return el("tr", { class: watched ? "is-selected" : "" },
    el("td", {}, el("input", {
      attrs: { type: "checkbox", "aria-label": `Watch ${descriptor.name || id}` },
      props: { checked: watched, disabled: missing },
      on: {
        change: event => {
          registered.watched = event.target.checked
            ? [...registered.watched, id]
            : registered.watched.filter(item => item !== id);
          loadRegisteredState(registered.offset, { quiet: true });
        },
      },
    })),
    el("td", { text: textOf(descriptor.name), attrs: { title: id } }),
    el("td", { text: textOf(descriptor.value_type) }),
    el("td", { text: textOf(descriptor.scope_id) }),
    el("td", {},
      el("span", { text: textOf(summary) }),
      value?.truncated ? el("span", { class: "pane-meta", text: " truncated" }) : null),
  );
}

function renderRegisteredState() {
  const host = dom.stateContent;
  clear(host);
  setRegisteredStateChrome();
  const registered = state.registeredState;
  if (!state.token || !targetById()) {
    showState(host, { icon: "device", title: "No target", message: "Select a connected Cove runtime to read its registered state." });
    return;
  }
  if (!state.caps.state) {
    showState(host, {
      icon: "info",
      title: "State observation is unavailable",
      message: "This runtime does not advertise state observation. Only state a component opts in to expose can be read.",
    });
    return;
  }
  if (registered.loading && !registered.loaded) {
    showState(host, { kind: "loading", title: "Reading registered state" });
    return;
  }
  if (registered.error && !registered.loaded) {
    showState(host, {
      kind: "error",
      title: "State read failed",
      message: errorMessage(registered.error),
      action: "Retry",
      onAction: () => isDisconnectedError(registered.error) ? refreshCurrent() : loadRegisteredState(registered.offset),
    });
    return;
  }
  if (!registered.loaded) {
    showState(host, {
      icon: "info",
      title: "Registered state",
      message: "Read the state this runtime opted in to expose.",
      action: "Read state",
      primary: true,
      onAction: () => loadRegisteredState(0),
    });
    return;
  }
  if (registered.error) host.append(errorInline(errorMessage(registered.error)));
  if (!registered.descriptors.length) {
    host.append(emptyInline(registered.scope
      ? "No registered state in this scope."
      : "This runtime exposes no registered state. State is only listed when a component opts in."));
    return;
  }
  if (registered.missing.length) {
    host.append(errorInline(`Unmounted ids are not read: ${registered.missing.join(", ")}`));
  }
  const rows = registered.descriptors.map(renderRegisteredStateRow);
  const start = registered.offset + 1;
  const end = registered.offset + registered.descriptors.length;
  host.append(el("div", { class: "panel-block" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: "Registered state" }),
      el("span", { class: "pane-meta", text: `${start}–${end} of ${registered.total}` }),
    ),
    el("div", { class: "table-wrap" }, el("table", { class: "data-table" },
      el("thead", {}, el("tr", {},
        el("th", { text: "Watch" }),
        el("th", { text: "Name" }),
        el("th", { text: "Type" }),
        el("th", { text: "Scope" }),
        el("th", { text: "Value" }),
      )),
      el("tbody", {}, ...rows),
    )),
  ));
  host.append(el("div", { class: "panel-block-head" },
    el("button", {
      class: "tool-button",
      props: { type: "button", disabled: registered.offset === 0 || registered.loading },
      text: "Previous",
      on: { click: () => loadRegisteredState(Math.max(0, registered.offset - registered.limit), { quiet: true }) },
    }),
    el("button", {
      class: "tool-button",
      props: { type: "button", disabled: registered.nextOffset === null || registered.loading },
      text: "Next",
      on: { click: () => loadRegisteredState(registered.nextOffset, { quiet: true }) },
    }),
  ));
  host.append(el("div", { class: "perf-note", text: "Values are read only for watched ids. A summary is the component's own description, truncated by the runtime; unwatched state is never requested." }));
}

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

function renderAppSizeTreemap(executable) {
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

function appSizeSnapshotOptionalNumber(value, label) {
  if (value === undefined || value === null) return null;
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) {
    throw new Error(`${label} is not a non-negative number`);
  }
  return value;
}

function appSizeSnapshotText(value) {
  return typeof value === "string" ? value.trim() : "";
}

function appSizeSnapshotOf(executable, target) {
  const pid = executable.pid ?? appSizeFirstNumber(target?.pid, target?.processId, target?.process_id);
  const path = executable.path || "";
  return {
    schema: APP_SIZE_SNAPSHOT_SCHEMA,
    version: APP_SIZE_SNAPSHOT_VERSION,
    savedAt: Date.now(),
    source: "session snapshot",
    measurement: executable.measurement || "",
    label: executable.label || "",
    attribution: executable.attribution || "",
    target: {
      id: target?.id || "",
      pid: pid ?? null,
      platform: executable.platform || target?.platform || "",
    },
    pid: pid ?? null,
    path,
    format: executable.format || "",
    arch: executable.arch || "",
    totalBytes: executable.totalBytes,
    sections: executable.sections.map(section => ({
      name: section.name,
      kind: section.kind || "",
      sizeBytes: section.sizeBytes,
      address: section.address || "",
    })),
    symbols: executable.symbols.map(symbol => ({
      name: symbol.name,
      rawName: symbol.rawName || "",
      crateName: symbol.crateName || "",
      sizeBytes: symbol.sizeBytes,
    })),
    groups: executable.groups.map(group => ({
      name: group.name,
      symbolCount: group.symbolCount,
      sizeBytes: group.sizeBytes,
    })),
    truncation: executable.truncation,
    warnings: executable.warnings,
  };
}

function appSizeSnapshotEntries(value, kind) {
  if (value === undefined || value === null) return [];
  if (!Array.isArray(value)) throw new Error(`${kind} is not an array`);
  return value.map((entry, index) => {
    if (!isObjectValue(entry)) throw new Error(`${kind}[${index}] is not an object`);
    const name = appSizeSnapshotText(entry.name);
    if (!name) throw new Error(`${kind}[${index}] has no name`);
    const sizeBytes = appSizeSnapshotOptionalNumber(entry.sizeBytes, `${kind}[${index}].sizeBytes`);
    if (kind === "sections") {
      return { name, sizeBytes, kind: appSizeSnapshotText(entry.kind), address: appSizeSnapshotText(entry.address) };
    }
    if (kind === "symbols") {
      return {
        name,
        sizeBytes,
        rawName: appSizeSnapshotText(entry.rawName),
        crateName: appSizeSnapshotText(entry.crateName),
      };
    }
    return { name, sizeBytes, symbolCount: appSizeSnapshotOptionalNumber(entry.symbolCount, `${kind}[${index}].symbolCount`) };
  });
}

function appSizeSnapshotTruncation(value) {
  if (!isObjectValue(value)) return null;
  const result = {};
  for (const kind of ["groups", "sections", "symbols"]) {
    const info = value[kind];
    if (!isObjectValue(info)) continue;
    result[kind] = {
      shown: appSizeSnapshotOptionalNumber(info.shown, `truncation.${kind}.shown`),
      returned: appSizeSnapshotOptionalNumber(info.returned, `truncation.${kind}.returned`),
      total: appSizeSnapshotOptionalNumber(info.total, `truncation.${kind}.total`),
      truncated: info.truncated === true ? true : info.truncated === false ? false : null,
    };
  }
  return Object.keys(result).length ? result : null;
}

function appSizeBaselineFromSerialized(parsed, source) {
  const baseline = {
    schema: APP_SIZE_SNAPSHOT_SCHEMA,
    version: APP_SIZE_SNAPSHOT_VERSION,
    savedAt: appSizeSnapshotOptionalNumber(parsed.savedAt, "savedAt"),
    source,
    measurement: appSizeSnapshotText(parsed.measurement),
    label: appSizeSnapshotText(parsed.label),
    attribution: appSizeSnapshotText(parsed.attribution),
    target: isObjectValue(parsed.target) ? parsed.target : null,
    pid: appSizeSnapshotOptionalNumber(parsed.pid, "pid"),
    path: appSizeSnapshotText(parsed.path),
    format: appSizeSnapshotText(parsed.format),
    arch: appSizeSnapshotText(parsed.arch),
    totalBytes: appSizeSnapshotOptionalNumber(parsed.totalBytes, "totalBytes"),
    sections: appSizeSnapshotEntries(parsed.sections, "sections"),
    symbols: appSizeSnapshotEntries(parsed.symbols, "symbols"),
    groups: appSizeSnapshotEntries(parsed.groups, "groups"),
    truncation: appSizeSnapshotTruncation(parsed.truncation),
    warnings: Array.isArray(parsed.warnings) ? parsed.warnings.filter(warning => typeof warning === "string") : [],
  };
  if (!baseline.path && isObjectValue(baseline.target)) baseline.path = appSizeSnapshotText(baseline.target.path);
  if (!baseline.pid && isObjectValue(baseline.target)) baseline.pid = appSizeSnapshotOptionalNumber(baseline.target.pid, "target.pid");
  if (baseline.totalBytes === null && !baseline.sections.length && !baseline.symbols.length && !baseline.groups.length && !baseline.path) {
    throw new Error("the snapshot contains no total size, sections, symbols, groups or executable path");
  }
  return baseline;
}

function appSizeBaselineFromText(text) {
  let parsed;
  try {
    parsed = JSON.parse(text);
  } catch {
    throw new Error("the file is not valid JSON");
  }
  if (!isObjectValue(parsed)) throw new Error("the file does not contain a JSON object");
  if (parsed.schema === APP_SIZE_SNAPSHOT_SCHEMA) {
    if (parsed.version !== APP_SIZE_SNAPSHOT_VERSION) {
      throw new Error(`unsupported snapshot version ${JSON.stringify(parsed.version)}`);
    }
    return appSizeBaselineFromSerialized(parsed, "imported snapshot");
  }
  const executable = normalizeAppSize(parsed);
  if (!executable) throw new Error("the file has neither a Cove App Size snapshot nor a controller app-size payload");
  if (executable.unavailable) throw new Error(`the payload reports App Size unavailable: ${executable.reason}`);
  const baseline = appSizeSnapshotOf(executable, null);
  baseline.savedAt = appSizeFirstNumber(parsed.savedAt, parsed.saved_at, parsed.capturedAt, parsed.captured_at, parsed.timestamp);
  baseline.source = "imported controller payload";
  return baseline;
}

function appSizeDiffEntryKey(kind, entry) {
  return kind === "symbols" ? (entry.rawName || entry.name) : entry.name;
}

function appSizeDiffIndex(kind, entries) {
  const map = new Map();
  let duplicates = 0;
  for (const entry of entries || []) {
    const key = appSizeDiffEntryKey(kind, entry);
    const existing = map.get(key);
    if (existing) {
      duplicates += 1;
      if (existing.sizeBytes === null || entry.sizeBytes === null) existing.sizeBytes = null;
      else existing.sizeBytes += entry.sizeBytes;
      continue;
    }
    map.set(key, { key, name: entry.name, sizeBytes: entry.sizeBytes });
  }
  return { map, duplicates };
}

function appSizeDiff(baseline, current) {
  const kinds = [
    { key: "groups", label: "Group" },
    { key: "sections", label: "Section" },
    { key: "symbols", label: "Symbol" },
  ];
  const rows = [];
  let duplicates = 0;
  for (const kind of kinds) {
    const before = appSizeDiffIndex(kind.key, baseline?.[kind.key]);
    const after = appSizeDiffIndex(kind.key, current?.[kind.key]);
    duplicates += before.duplicates + after.duplicates;
    const keys = new Set([...before.map.keys(), ...after.map.keys()]);
    for (const key of keys) {
      const previous = before.map.get(key);
      const next = after.map.get(key);
      let status;
      let delta = null;
      if (!previous) {
        status = "added";
        delta = next.sizeBytes;
      } else if (!next) {
        status = "removed";
        delta = previous.sizeBytes === null ? null : -previous.sizeBytes;
      } else if (previous.sizeBytes === null || next.sizeBytes === null) {
        status = previous.sizeBytes === next.sizeBytes ? "unchanged" : "changed";
      } else {
        status = previous.sizeBytes === next.sizeBytes ? "unchanged" : "changed";
        delta = next.sizeBytes - previous.sizeBytes;
      }
      const denominator = previous?.sizeBytes ?? null;
      rows.push({
        kind: kind.key,
        kindLabel: kind.label,
        name: next?.name || previous?.name,
        status,
        beforeBytes: previous?.sizeBytes ?? null,
        afterBytes: next?.sizeBytes ?? null,
        beforePresent: Boolean(previous),
        afterPresent: Boolean(next),
        delta,
        percent: delta === null || denominator === null || denominator <= 0 ? null : (delta / denominator) * 100,
      });
    }
  }
  const counts = { added: 0, removed: 0, changed: 0, unchanged: 0 };
  for (const row of rows) counts[row.status] += 1;
  const order = { groups: 0, sections: 1, symbols: 2 };
  const changed = rows
    .filter(row => row.status !== "unchanged")
    .sort((left, right) => Math.abs(right.delta ?? 0) - Math.abs(left.delta ?? 0)
      || order[left.kind] - order[right.kind]
      || (left.name < right.name ? -1 : left.name > right.name ? 1 : 0));
  const totalBefore = appSizeSnapshotOptionalNumber(baseline?.totalBytes, "baseline total");
  const totalAfter = appSizeSnapshotOptionalNumber(current?.totalBytes, "current total");
  const totalDelta = totalBefore !== null && totalAfter !== null ? totalAfter - totalBefore : null;
  return {
    counts,
    rows,
    changed,
    duplicates,
    total: {
      before: totalBefore,
      after: totalAfter,
      delta: totalDelta,
      percent: totalDelta === null || totalBefore === null || totalBefore <= 0 ? null : (totalDelta / totalBefore) * 100,
    },
  };
}

function appSizeDeltaText(delta) {
  if (delta === null) return "Not reported";
  if (delta === 0) return "±0 B";
  return `${delta > 0 ? "+" : "−"}${fmtBytes(Math.abs(delta))}`;
}

function appSizePercentText(percent) {
  if (percent === null || !Number.isFinite(percent)) return "—";
  if (Math.abs(percent) < 0.05) return "±0.0%";
  return `${percent > 0 ? "+" : "−"}${Math.abs(percent).toFixed(1)}%`;
}

function appSizeDeltaClass(delta) {
  if (delta === null) return "";
  if (delta > 0) return "is-positive";
  if (delta < 0) return "is-negative";
  return "is-zero";
}

function appSizeDiffTruncationDetail(info) {
  if (!info) return "";
  const returned = info.returned;
  const total = info.total;
  if (total !== null && returned !== null && total > returned) return `${formatCount(returned)} of ${formatCount(total)}`;
  if (info.truncated === true) return returned !== null ? `${formatCount(returned)} returned, controller-capped` : "controller-capped";
  return "";
}

function appSizeDiffTruncation(baseline, current) {
  const labels = { groups: "groups", sections: "sections", symbols: "symbols" };
  const parts = [];
  for (const kind of ["groups", "sections", "symbols"]) {
    const before = appSizeDiffTruncationDetail(baseline?.truncation?.[kind]);
    const after = appSizeDiffTruncationDetail(current?.truncation?.[kind]);
    if (!before && !after) continue;
    const sides = [];
    if (before) sides.push(`baseline ${before}`);
    if (after) sides.push(`current ${after}`);
    parts.push(`${labels[kind]} (${sides.join(", ")})`);
  }
  return parts.length ? `Truncated lists: ${parts.join("; ")}. The diff compares only returned entries, so unlisted entries cannot be matched.` : "";
}

function appSizeDiffCellBytes(value, present) {
  if (!present) return el("span", { class: "app-size-diff-absent", text: "—" });
  if (value === null) return el("span", { text: "Not reported" });
  return el("span", { text: fmtBytes(value), attrs: { title: `${formatCount(value)} bytes` } });
}

function renderAppSizeDiff(executable) {
  const app = state.appSize;
  const baseline = app.baseline;
  const hasData = Boolean(executable) && !executable.unavailable;
  const meta = baseline
    ? `Baseline ${baseline.savedAt === null ? "not timestamped" : formatWallTime(baseline.savedAt)}`
    : "No baseline";
  const block = appSizePanel("Size diff", meta);
  block.id = "app-size-diff-panel";
  const body = el("div", { class: "panel-block-body" });
  const toolbar = el("div", { class: "app-size-diff-toolbar" });

  toolbar.append(el("button", {
    class: "tool-button small",
    props: { type: "button", disabled: !hasData || app.loading, id: "app-size-diff-snapshot" },
    attrs: { title: "Store the current report as the size baseline" },
    text: "Snapshot baseline",
    on: { click: () => appSizeCaptureBaseline() },
  }));
  toolbar.append(el("button", {
    class: "tool-button small",
    props: { type: "button", disabled: !baseline, id: "app-size-diff-export" },
    attrs: { title: "Download the baseline snapshot as JSON" },
    text: "Export JSON",
    on: { click: () => appSizeExportBaseline() },
  }));
  toolbar.append(el("label", {
    class: "tool-button small",
    attrs: { for: "app-size-diff-import", title: "Import an App Size snapshot JSON" },
  }, icon("upload"), "Import JSON"));
  const importInput = el("input", {
    class: "sr-only",
    attrs: {
      type: "file",
      id: "app-size-diff-import",
      accept: ".json,application/json",
      "aria-label": "Import an App Size snapshot JSON",
    },
  });
  importInput.addEventListener("change", () => appSizeImportSnapshot(importInput));
  toolbar.append(importInput);
  toolbar.append(el("button", {
    class: "tool-button small",
    props: { type: "button", disabled: !baseline, id: "app-size-diff-clear" },
    attrs: { title: "Discard the current baseline" },
    text: "Clear baseline",
    on: {
      click: () => {
        state.appSize.baseline = null;
        state.appSize.baselineError = "";
        renderAppSize();
      },
    },
  }));
  body.append(toolbar);
  if (app.baselineError) body.append(errorInline(app.baselineError));

  if (!baseline) {
    body.append(el("p", {
      class: "app-size-diff-empty",
      text: "No baseline snapshot. Snapshot the current report or import a snapshot JSON to compare later refreshes.",
    }));
    block.append(body);
    return block;
  }

  const stamp = baseline.savedAt === null ? "timestamp not reported" : formatWallTime(baseline.savedAt);
  body.append(el("p", {
    class: "app-size-diff-baseline",
    text: `${baseline.source || "snapshot"} · ${stamp} · ${baseline.totalBytes === null ? "total not reported" : fmtBytes(baseline.totalBytes)} · ${baseline.path || "path not reported"}`,
    attrs: baseline.path ? { title: baseline.path } : {},
  }));
  const currentStamp = app.refreshedAt ? formatWallTime(app.refreshedAt) : "timestamp not reported";
  body.append(el("p", {
    class: "app-size-diff-current",
    text: `Current report · ${currentStamp} · ${executable.totalBytes === null ? "total not reported" : fmtBytes(executable.totalBytes)} · ${executable.path || "path not reported"}`,
    attrs: executable.path ? { title: executable.path } : {},
  }));
  if (baseline.path && executable.path && baseline.path !== executable.path) {
    body.append(el("p", {
      class: "app-size-note",
      text: "The baseline comes from a different executable path. Entries are matched by name, so bytes may not be comparable.",
    }));
  }

  const diff = appSizeDiff(baseline, executable);
  const summary = el("div", { class: "app-size-diff-summary" });
  summary.append(
    el("span", { class: "is-added", text: `${formatCount(diff.counts.added)} added` }),
    el("span", { text: " · " }),
    el("span", { class: "is-removed", text: `${formatCount(diff.counts.removed)} removed` }),
    el("span", { text: " · " }),
    el("span", { class: "is-changed", text: `${formatCount(diff.counts.changed)} changed` }),
    el("span", { text: " · " }),
    el("span", { class: "is-unchanged", text: `${formatCount(diff.counts.unchanged)} unchanged` }),
  );
  body.append(summary);

  const total = diff.total;
  if (total.before !== null && total.after !== null) {
    body.append(el("p", {
      class: `app-size-diff-total ${appSizeDeltaClass(total.delta)}`,
      text: `Executable total ${fmtBytes(total.before)} → ${fmtBytes(total.after)} · ${appSizeDeltaText(total.delta)} · ${appSizePercentText(total.percent)}`,
    }));
  } else if (total.before === null && total.after === null) {
    body.append(el("p", { class: "app-size-note", text: "Executable total is not reported in either report; no total delta is computed." }));
  } else {
    body.append(el("p", {
      class: "app-size-note",
      text: `Executable total is only reported in the ${total.before === null ? "current" : "baseline"} report; no total delta is computed.`,
    }));
  }

  const truncation = appSizeDiffTruncation(baseline, executable);
  if (truncation) body.append(el("p", { class: "app-size-note", text: truncation }));
  if (diff.duplicates > 0) {
    body.append(el("p", {
      class: "app-size-note",
      text: `${formatCount(diff.duplicates)} duplicate names were combined by name for this diff.`,
    }));
  }

  if (!diff.changed.length) {
    body.append(el("p", {
      class: "app-size-note",
      text: "No added, removed or changed entries between the baseline and the current report.",
    }));
  } else {
    const shown = diff.changed.slice(0, MAX_APP_SIZE_DIFF_ROWS);
    body.append(appSizeTable(
      [
        { label: "Status" },
        { label: "Kind" },
        { label: "Name" },
        { label: "Baseline", num: true },
        { label: "Current", num: true },
        { label: "Δ bytes", num: true },
        { label: "Δ %", num: true },
      ],
      shown.map(row => el("tr", {
        class: `app-size-diff-row is-${row.status}`,
        attrs: { "data-app-size-diff-status": row.status, "data-app-size-diff-kind": row.kind },
      },
        el("td", { class: `app-size-diff-status is-${row.status}`, text: row.status === "added" ? "Added" : row.status === "removed" ? "Removed" : "Changed" }),
        el("td", { text: row.kindLabel }),
        el("td", { class: "app-size-name", text: row.name, attrs: { title: row.name } }),
        el("td", { class: "num app-size-bytes", attrs: { title: row.beforePresent ? "" : "Absent from the baseline" } }, appSizeDiffCellBytes(row.beforeBytes, row.beforePresent)),
        el("td", { class: "num app-size-bytes", attrs: { title: row.afterPresent ? "" : "Absent from the current report" } }, appSizeDiffCellBytes(row.afterBytes, row.afterPresent)),
        el("td", { class: `num app-size-delta ${appSizeDeltaClass(row.delta)}`, text: appSizeDeltaText(row.delta) }),
        el("td", { class: `num app-size-delta ${appSizeDeltaClass(row.delta)}`, text: appSizePercentText(row.percent) }),
      )),
    ));
    if (diff.changed.length > shown.length) {
      body.append(el("p", {
        class: "app-size-note",
        text: `Showing ${formatCount(shown.length)} of ${formatCount(diff.changed.length)} added, removed or changed entries.`,
      }));
    }
    if (diff.counts.added || diff.counts.removed) {
      body.append(el("p", {
        class: "app-size-note",
        text: "A dash means the entry was absent from that report, not zero bytes. Percentages are relative to the baseline entry; entries without baseline bytes have no percentage.",
      }));
    }
  }
  block.append(body);
  return block;
}

function appSizeCaptureBaseline() {
  const executable = normalizeAppSize(state.appSize.data);
  if (!executable || executable.unavailable) return;
  state.appSize.baseline = appSizeSnapshotOf(executable, targetById());
  state.appSize.baselineError = "";
  renderAppSize();
  toast("App Size baseline captured.", "success", 3000);
}

function appSizeFileStamp(milliseconds) {
  const date = new Date(numberValue(milliseconds) ?? Date.now());
  const pad = value => String(value).padStart(2, "0");
  return `${date.getFullYear()}${pad(date.getMonth() + 1)}${pad(date.getDate())}-${pad(date.getHours())}${pad(date.getMinutes())}${pad(date.getSeconds())}`;
}

function appSizeDownload(filename, text) {
  const blob = new Blob([text], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const anchor = el("a", { attrs: { href: url, download: filename, rel: "noopener" } });
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 0);
}

function appSizeExportBaseline() {
  const baseline = state.appSize.baseline;
  if (!baseline) return;
  try {
    const text = `${JSON.stringify(baseline, null, 2)}\n`;
    appSizeDownload(`cove-app-size-${appSizeFileStamp(baseline.savedAt)}.json`, text);
  } catch (error) {
    state.appSize.baselineError = `Export failed: ${error?.message || error}`;
    renderAppSize();
  }
}

async function appSizeImportSnapshot(input) {
  const file = input.files && input.files[0];
  input.value = "";
  if (!file) return;
  try {
    const text = await file.text();
    state.appSize.baseline = appSizeBaselineFromText(text);
    state.appSize.baselineError = "";
    toast("App Size baseline imported.", "success", 3000);
  } catch (error) {
    state.appSize.baselineError = `Snapshot rejected: ${error?.message || error}`;
  }
  renderAppSize();
}

function setAppSizeChrome() {
  const target = targetById();
  const app = state.appSize;
  const gate = target ? appSizeTargetUnavailable(target) : "";
  dom.appSizeRefresh.disabled = !state.token || !target || Boolean(gate) || app.loading;
  if (!state.token) {
    dom.appSizeMeta.textContent = "No controller connection.";
    return;
  }
  if (!target) {
    dom.appSizeMeta.textContent = "No target selected.";
    return;
  }
  if (gate) {
    dom.appSizeMeta.textContent = "App Size is unavailable for this target.";
    return;
  }
  if (app.loading && !app.loaded) {
    dom.appSizeMeta.textContent = "Reading the native executable…";
    return;
  }
  if (app.error && !app.loaded) {
    if (app.error.code === "unavailable") dom.appSizeMeta.textContent = "App Size unavailable.";
    else if (app.error.code === "unknown_command") dom.appSizeMeta.textContent = "This controller does not expose get_ui_app_size.";
    else dom.appSizeMeta.textContent = "Executable read failed.";
    return;
  }
  if (app.data === null) {
    dom.appSizeMeta.textContent = "No executable analysis returned.";
    return;
  }
  const executable = normalizeAppSize(app.data);
  if (!executable) {
    dom.appSizeMeta.textContent = "Unrecognized app size response.";
    return;
  }
  if (executable.unavailable) {
    dom.appSizeMeta.textContent = "App Size unavailable.";
    return;
  }
  const total = executable.totalBytes === null ? "total not reported" : `${fmtBytes(executable.totalBytes)} native executable`;
  dom.appSizeMeta.textContent = `${total} · ${formatCount(executable.sections.length)} sections · ${formatCount(executable.symbols.length)} symbols · ${formatCount(executable.groups.length)} crate groups · updated ${formatWallTime(app.refreshedAt)}`;
}

function renderAppSize() {
  const host = dom.appSizeContent;
  clear(host);
  setAppSizeChrome();
  const app = state.appSize;
  const target = targetById();
  if (!state.token) {
    showState(host, { icon: "device", title: "Not connected", message: "Connect to a controller to read the selected runtime's native executable." });
    return;
  }
  if (!target) {
    showState(host, { icon: "device", title: "No target", message: "Select a connected runtime to read its native executable." });
    return;
  }
  const gate = appSizeTargetUnavailable(target);
  if (gate) {
    showState(host, { icon: "package", title: "App Size unavailable", message: gate });
    return;
  }
  if (app.loading && !app.loaded) {
    showState(host, { kind: "loading", title: "Reading the native executable", message: "Waiting for the controller's get_ui_app_size response." });
    return;
  }
  if (app.error && !app.loaded) {
    if (app.error.code === "unavailable") {
      showState(host, {
        icon: "package",
        title: "App Size unavailable",
        message: errorMessage(app.error),
        action: "Retry",
        onAction: () => loadAppSize({ reset: true }),
      });
      return;
    }
    if (app.error.code === "unknown_command") {
      showState(host, {
        icon: "package",
        title: "App Size is not supported by this controller",
        message: errorMessage(app.error),
      });
      return;
    }
    showState(host, {
      kind: "error",
      title: "Executable read failed",
      message: errorMessage(app.error),
      action: "Retry",
      onAction: () => loadAppSize({ reset: true }),
    });
    return;
  }
  if (app.data === null) {
    showState(host, {
      icon: "package",
      title: "No executable analysis",
      message: "The controller returned no app size payload. Nothing is substituted.",
      action: "Refresh",
      onAction: () => loadAppSize({ reset: true }),
    });
    return;
  }
  const executable = normalizeAppSize(app.data);
  if (!executable) {
    showState(host, {
      kind: "error",
      title: "Unrecognized app size response",
      message: "The response contained no total size, sections, symbols, groups or executable path. No sizes are shown.",
      action: "Retry",
      onAction: () => loadAppSize({ reset: true }),
    });
    return;
  }
  if (executable.unavailable) {
    showState(host, {
      icon: "package",
      title: "App Size unavailable",
      message: executable.reason,
      action: "Retry",
      onAction: () => loadAppSize({ reset: true }),
    });
    return;
  }
  if (app.loading) {
    host.append(el("div", { class: "inline-spinner" }, el("span", { class: "spinner", attrs: { role: "progressbar", "aria-label": "Reading the native executable" } }), "Reading the native executable…"));
  }
  if (app.error) host.append(errorInline(errorMessage(app.error)));
  host.append(appSizeScope(executable, target));
  host.append(renderAppSizeStats(executable));
  host.append(appSizeSource(executable, target));
  for (const warning of executable.warnings) host.append(errorInline(warning));
  host.append(renderAppSizeTreemap(executable));
  host.append(renderAppSizeSections(executable));
  host.append(renderAppSizeSymbols(executable));
  host.append(renderAppSizeGroups(executable));
  host.append(renderAppSizeDiff(executable));
  if (executable.totalBytes === null) {
    host.append(el("div", { class: "app-size-note", text: "The controller reported no total executable size. A sum of section sizes is not substituted for it." }));
  }
  host.append(el("div", { class: "app-size-note", text: "Sections are object-file sections, not loaded memory mappings. Symbol sizes and crate groups are reported by the native analyzer; bytes outside those groups stay unassigned." }));
}

async function loadAppSize(options = {}) {
  const app = state.appSize;
  const target = targetById();
  if (!state.token || !target) {
    renderAppSize();
    return;
  }
  const gate = appSizeTargetUnavailable(target);
  if (gate) {
    app.loading = false;
    app.error = null;
    app.data = null;
    app.loaded = true;
    app.refreshedAt = Date.now();
    renderAppSize();
    return;
  }
  if (app.loading) return;
  const targetId = state.targetId;
  const request = app.request + 1;
  app.request = request;
  app.loading = true;
  app.error = null;
  if (options.reset) {
    app.data = null;
    app.loaded = false;
  }
  renderAppSize();
  try {
    const data = unwrapData(await rpc("get_ui_app_size", { target: targetId }));
    if (request !== app.request || targetId !== state.targetId) return;
    app.loaded = true;
    app.data = data === null || data === undefined ? null : data;
    app.refreshedAt = Date.now();
    if (options.quiet !== true) toast("App size refreshed.", "success", 3000);
  } catch (error) {
    if (request !== app.request || targetId !== state.targetId) return;
    app.error = error;
    if (error?.code === "unknown_command") {
      app.error = new RpcError("This controller does not expose get_ui_app_size.", "unknown_command");
    }
    if (options.quiet !== true && error?.code !== "unavailable") toast(errorMessage(error), "error", 8000);
  } finally {
    if (request === app.request && targetId === state.targetId) {
      app.loading = false;
      renderAppSize();
    }
  }
}

function ensureAppSize() {
  const target = targetById();
  if (!state.token || !target || appSizeTargetUnavailable(target)) {
    renderAppSize();
    return;
  }
  if (!state.appSize.loaded && !state.appSize.loading) {
    loadAppSize({ quiet: true });
    return;
  }
  renderAppSize();
}

function resetAppSize() {
  const request = state.appSize.request + 1;
  state.appSize = emptyAppSize();
  state.appSize.request = request;
  if (state.view === "app-size") renderAppSize();
}

/* Deep Links
 *
 * The controller validates a project's platform configuration on disk; the
 * client never reads files or infers links. `get_ui_deep_links {root}` returns
 * the discovered project files, Android schemes/hosts/issues, Apple
 * schemes/domains/issues and a summary. Only the absolute root is sent, so the
 * controller can reject traversal, symlink escapes and unknown files.
 */

function deepLinksAbsoluteRoot(root) {
  return root.startsWith("/") || /^[A-Za-z]:[\\/]/.test(root) || root.startsWith("\\\\");
}

function deepLinksRootError(root) {
  if (!root) return "Enter an absolute project root.";
  if (!deepLinksAbsoluteRoot(root)) return "The project root must be an absolute path. Relative paths and traversal are not sent to the controller.";
  if (root.split(/[\\/]+/).includes("..")) return "The project root must not contain .. path segments.";
  return "";
}

function deepLinksFirstArray(...values) {
  for (const value of values) {
    if (Array.isArray(value)) return value;
  }
  return [];
}

function deepLinksObject(...values) {
  for (const value of values) {
    if (isObjectValue(value)) return value;
  }
  return null;
}

function deepLinksFirstNumber(...values) {
  for (const value of values) {
    const number = numberValue(value);
    if (number !== null) return number;
  }
  return null;
}

function deepLinksRoot(data) {
  if (!isObjectValue(data)) return null;
  return isObjectValue(data.data) ? data.data : data;
}

function deepLinksUnavailableReason(root) {
  const unavailable = root.available === false
    || root.supported === false
    || root.unavailable === true
    || root.ok === false
    || root.status === "unavailable";
  if (!unavailable) return "";
  return firstStringValue(
    root.reason,
    root.unavailable_reason,
    root.unavailableReason,
    root.message,
    root.detail,
    typeof root.error === "string" ? root.error : "",
    root.error?.message,
    root.error?.reason,
  ) || "The controller reported no deep links validation for this root.";
}

function deepLinksPlatform(value) {
  const platform = String(value || "").toLowerCase();
  if (platform === "android") return "android";
  if (["ios", "ipados", "macos", "apple", "tvos", "watchos"].includes(platform)) return "apple";
  return platform;
}

function deepLinksPlatformLabel(platform) {
  if (platform === "android") return "Android";
  if (platform === "apple") return "Apple";
  return platform || "Project";
}

function deepLinksKindLabel(kind) {
  return {
    manifest: "Android manifest",
    android_manifest: "Android manifest",
    info_plist: "Info.plist",
    plist: "Info.plist",
    entitlements: "Entitlements",
    assetlinks: "Asset links",
    asset_links: "Asset links",
    assetlinks_json: "Asset links",
    associated_domains: "Associated domains",
    apple_app_site_association: "App site association",
    aasa: "App site association",
  }[String(kind || "").toLowerCase()] || kind || "";
}

function deepLinksFileStatus(status) {
  const raw = String(status || "").toLowerCase();
  if (["missing", "absent", "not_configured", "not-configured", "notconfigured", "none"].includes(raw)) {
    return { label: "Not configured", tone: "muted" };
  }
  if (["found", "present", "exists", "ok", "valid"].includes(raw)) return { label: "Found", tone: "good" };
  if (["invalid", "unreadable", "unparseable", "parse_error", "error"].includes(raw)) {
    return { label: raw === "unreadable" ? "Unreadable" : "Invalid", tone: "bad" };
  }
  if (raw) return { label: status, tone: "" };
  return { label: "—", tone: "" };
}

function deepLinksFileName(entry) {
  if (typeof entry === "string") return { path: entry, platform: "", kind: "", status: "", detail: "" };
  if (!isObjectValue(entry)) return null;
  const location = isObjectValue(entry.location) ? entry.location : {};
  const path = firstStringValue(entry.path, entry.file, entry.filePath, entry.file_path, entry.filename, entry.name, typeof entry.location === "string" ? entry.location : "", location.path, location.file);
  const platform = deepLinksPlatform(firstStringValue(entry.platform, entry.os, entry.target));
  const kind = firstStringValue(entry.kind, entry.type, entry.fileKind, entry.file_kind, entry.role);
  const status = firstStringValue(entry.status, entry.state, entry.presence, entry.result);
  const detail = firstStringValue(entry.detail, entry.message, entry.reason, entry.error, typeof entry.error === "string" ? entry.error : "");
  if (!path && !kind) return null;
  return { path, platform, kind, status, detail };
}

function deepLinksFiles(root, android, apple) {
  const entries = [
    ...deepLinksFirstArray(root.files, root.discoveredFiles, root.discovered_files, root.sources, root.checkedFiles, root.checked_files),
    ...deepLinksFirstArray(android?.files, android?.discoveredFiles, android?.discovered_files, android?.sources),
    ...deepLinksFirstArray(apple?.files, apple?.discoveredFiles, apple?.discovered_files, apple?.sources),
  ];
  const files = entries.map(deepLinksFileName).filter(Boolean);
  const nestedPaths = [
    ["android", "manifest", firstStringValue(android?.manifest, android?.manifestPath, android?.manifest_path)]
      .concat(typeof android?.manifest === "object" ? [android.manifest.path] : []),
    ["apple", "info_plist", firstStringValue(apple?.infoPlist, apple?.info_plist, apple?.plist, apple?.plistPath, apple?.plist_path)]
      .concat(typeof apple?.infoPlist === "object" ? [apple.infoPlist.path] : []),
    ["apple", "entitlements", firstStringValue(apple?.entitlements, apple?.entitlementsPath, apple?.entitlements_path)]
      .concat(typeof apple?.entitlements === "object" ? [apple.entitlements.path] : []),
  ];
  for (const [platform, kind, path] of nestedPaths) {
    if (!path) continue;
    if (files.some(file => file.path === path)) continue;
    files.push({ path, platform, kind, status: "", detail: "" });
  }
  const seen = new Set();
  return files.filter(file => {
    const key = `${file.path}\u0000${file.kind}\u0000${file.platform}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function deepLinksStringEntries(value, ...keys) {
  if (!Array.isArray(value)) return [];
  const entries = [];
  for (const entry of value) {
    if (typeof entry === "string" && entry.trim()) {
      entries.push({ name: entry.trim(), detail: "" });
      continue;
    }
    if (!isObjectValue(entry)) continue;
    const name = firstStringValue(...keys.map(key => entry[key]), entry.name, entry.value, entry.id);
    if (!name) continue;
    const detail = firstStringValue(entry.scheme, entry.schemeName, entry.scheme_name, entry.path, entry.pathPrefix, entry.path_prefix);
    entries.push({ name, detail: detail && detail !== name ? detail : "" });
  }
  return entries;
}

function deepLinksSchemes(source, ...names) {
  for (const name of names) {
    if (Array.isArray(source?.[name])) {
      return deepLinksStringEntries(source[name], "scheme", "scheme_name", "name");
    }
  }
  return [];
}

function deepLinksDomains(source, ...names) {
  for (const name of names) {
    if (Array.isArray(source?.[name])) {
      return deepLinksStringEntries(source[name], "domain", "host", "domain_name", "name");
    }
  }
  return [];
}

function deepLinksSeverity(value) {
  const raw = String(value || "").toLowerCase();
  if (["error", "fatal", "severe"].includes(raw)) return "error";
  if (["warning", "warn"].includes(raw)) return "warning";
  if (["note", "info", "information", "notice"].includes(raw)) return "note";
  return raw;
}

function deepLinksAssociatedDomains(entries) {
  return entries.map(entry => {
    const value = String(entry?.name ?? entry ?? "");
    const separator = value.indexOf(":");
    if (separator <= 0) return { name: value, detail: "—" };
    return { name: value.slice(separator + 1), detail: value.slice(0, separator) };
  }).filter(entry => entry.name);
}

function deepLinksIntentFilter(entry) {
  if (!isObjectValue(entry)) return null;
  const stringList = value => Array.isArray(value)
    ? value.map(item => typeof item === "string" ? item : firstStringValue(item?.name, item?.value)).filter(Boolean)
    : [];
  const activity = firstStringValue(entry.activity, entry.activityName, entry.activity_name, entry.name);
  const schemes = stringList(entry.schemes);
  const hosts = stringList(entry.hosts);
  const paths = stringList(entry.paths);
  const autoVerify = typeof entry.autoVerify === "boolean" ? entry.autoVerify : typeof entry.auto_verify === "boolean" ? entry.auto_verify : null;
  const file = firstStringValue(entry.file, entry.path);
  const line = deepLinksFirstNumber(entry.line, entry.lineNumber, entry.line_number);
  if (!activity && !schemes.length && !hosts.length && !paths.length) return null;
  return { activity, schemes, hosts, paths, autoVerify, file, line };
}

function deepLinksIssue(entry, platform) {
  if (typeof entry === "string") {
    const message = entry.trim();
    if (!message) return null;
    return { severity: "", message, path: "", line: null, column: null, code: "", platform };
  }
  if (!isObjectValue(entry)) return null;
  const location = isObjectValue(entry.location) ? entry.location : isObjectValue(entry.position) ? entry.position : {};
  const range = isObjectValue(entry.range) ? entry.range : {};
  const start = isObjectValue(range.start) ? range.start : {};
  const message = firstStringValue(entry.message, entry.detail, entry.description, entry.title, entry.reason, entry.text);
  const code = firstStringValue(entry.code, entry.rule, entry.id, entry.kind);
  if (!message && !code) return null;
  return {
    severity: deepLinksSeverity(firstStringValue(entry.severity, entry.level, entry.priority)),
    message: message || code,
    path: firstStringValue(entry.path, entry.file, entry.filePath, entry.file_path, entry.source, location.path, location.file),
    location: firstStringValue(typeof entry.location === "string" ? entry.location : "", location.label),
    line: deepLinksFirstNumber(entry.line, entry.lineNumber, entry.line_number, location.line, location.lineNumber, start.line),
    column: deepLinksFirstNumber(entry.column, entry.col, location.column, start.column),
    code: message ? code : "",
    fix: firstStringValue(entry.fix, entry.suggestion, entry.remediation, entry.hint),
    platform,
  };
}

function deepLinksIssues(source, platform) {
  if (!source) return [];
  return [
    ...deepLinksFirstArray(source.issues, source.errors, source.warnings, source.problems, source.diagnostics),
  ].map(entry => deepLinksIssue(entry, platform)).filter(Boolean);
}

function deepLinksSummary(root, files, android, apple, issues) {
  const summary = deepLinksObject(root.summary, root.totals, root.counts) || {};
  const androidSummary = deepLinksObject(summary.android, summary.android_links, summary.androidLinks) || {};
  const appleSummary = deepLinksObject(summary.apple, summary.apple_links, summary.appleLinks, summary.ios) || {};
  const errors = issues.filter(issue => issue.severity === "error").length;
  const warnings = issues.filter(issue => issue.severity === "warning").length;
  return {
    files: deepLinksFirstNumber(summary.files, summary.fileCount, summary.file_count, root.fileCount, root.file_count) ?? files.length,
    androidSchemes: deepLinksFirstNumber(summary.androidSchemes, summary.android_schemes, androidSummary.schemes, android.schemesCount, android.schemes_count) ?? android.schemes.length,
    androidHosts: deepLinksFirstNumber(summary.androidHosts, summary.android_hosts, androidSummary.hosts, android.hostsCount, android.hosts_count) ?? android.hosts.length,
    appleSchemes: deepLinksFirstNumber(summary.appleSchemes, summary.apple_schemes, summary.iosSchemes, summary.ios_schemes, appleSummary.schemes, apple.schemesCount, apple.schemes_count) ?? apple.schemes.length,
    appleDomains: deepLinksFirstNumber(summary.appleDomains, summary.apple_domains, summary.iosDomains, summary.ios_domains, appleSummary.domains, apple.domainsCount, apple.domains_count) ?? apple.domains.length,
    issues: deepLinksFirstNumber(summary.issues, summary.issueCount, summary.issue_count) ?? issues.length,
    errors: deepLinksFirstNumber(summary.errors, summary.errorCount, summary.error_count, androidSummary.errors, appleSummary.errors) ?? errors,
    warnings: deepLinksFirstNumber(summary.warnings, summary.warningCount, summary.warning_count, androidSummary.warnings, appleSummary.warnings) ?? warnings,
    text: firstStringValue(summary.text, summary.summary, summary.detail),
  };
}

function deepLinksConfigured(root, android, apple) {
  const summary = deepLinksObject(root.summary, root.totals, root.counts);
  for (const value of [root.configured, root.deepLinksConfigured, root.deep_links_configured, root.hasDeepLinks, root.has_deep_links, summary?.configured, summary?.deep_links_configured]) {
    if (typeof value === "boolean") return value;
  }
  const status = String(firstStringValue(root.status, root.state, root.result)).toLowerCase();
  if (["no_deep_links", "no-deep-links", "none", "not_configured", "not-configured", "unconfigured", "empty"].includes(status)) return false;
  if (["configured", "deep_links_configured", "deep-links-configured"].includes(status)) return true;
  return android.schemes.length > 0 || android.hosts.length > 0 || apple.schemes.length > 0 || apple.domains.length > 0;
}

function normalizeDeepLinks(data) {
  const root = deepLinksRoot(data);
  if (!root) return null;
  const reason = deepLinksUnavailableReason(root);
  if (reason) return { unavailable: true, reason };
  const projectRoot = firstStringValue(root.root, root.projectRoot, root.project_root, root.project, root.path);
  const androidSource = deepLinksObject(root.android, root.android_project, root.androidProject);
  const appleSource = deepLinksObject(root.apple, root.ios, root.apple_project, root.appleProject);
  const issuesSource = deepLinksFirstArray(root.issues, root.problems, root.diagnostics, root.errors, root.warnings);
  const hasShape = Boolean(androidSource || appleSource || Array.isArray(root.files) || Array.isArray(root.discovered_files)
    || Array.isArray(root.discoveredFiles) || isObjectValue(root.summary) || issuesSource.length
    || typeof root.configured === "boolean" || root.status !== undefined);
  if (!hasShape) return null;
  const android = {
    manifest: firstStringValue(
      typeof androidSource?.manifest === "string" ? androidSource.manifest : "",
      androidSource?.manifestPath,
      androidSource?.manifest_path,
      isObjectValue(androidSource?.manifest) ? androidSource.manifest.path : "",
    ),
    schemes: deepLinksSchemes(androidSource, "schemes", "urlSchemes", "url_schemes", "url_schemes"),
    hosts: deepLinksDomains(androidSource, "hosts", "domains", "deepLinkHosts", "deep_link_hosts"),
    links: deepLinksFirstArray(androidSource?.links, androidSource?.intentFilters, androidSource?.intent_filters)
      .map(deepLinksIntentFilter).filter(Boolean),
    configured: typeof androidSource?.configured === "boolean" ? androidSource.configured : null,
    applicationId: firstStringValue(androidSource?.applicationId, androidSource?.application_id, androidSource?.package),
  };
  const apple = {
    infoPlist: firstStringValue(
      typeof appleSource?.infoPlist === "string" ? appleSource.infoPlist : "",
      typeof appleSource?.info_plist === "string" ? appleSource.info_plist : "",
      typeof appleSource?.plist === "string" ? appleSource.plist : "",
      isObjectValue(appleSource?.infoPlist) ? appleSource.infoPlist.path : "",
      isObjectValue(appleSource?.info_plist) ? appleSource.info_plist.path : "",
    ),
    entitlements: firstStringValue(
      typeof appleSource?.entitlements === "string" ? appleSource.entitlements : "",
      appleSource?.entitlementsPath,
      appleSource?.entitlements_path,
      isObjectValue(appleSource?.entitlements) ? appleSource.entitlements.path : "",
    ),
    schemes: deepLinksSchemes(appleSource, "schemes", "urlSchemes", "url_schemes", "url_types", "urlTypes", "cfBundleURLSchemes"),
    domains: deepLinksDomains(appleSource, "domains", "associatedDomains", "associated_domains", "associatedDomainsList", "associated_domains_list"),
    associatedDomains: deepLinksAssociatedDomains(deepLinksStringEntries(
      deepLinksFirstArray(appleSource?.associatedDomains, appleSource?.associated_domains),
      "domain",
      "name",
    )),
    configured: typeof appleSource?.configured === "boolean" ? appleSource.configured : null,
    bundleId: firstStringValue(appleSource?.bundleId, appleSource?.bundle_id, appleSource?.bundleIdentifier, appleSource?.bundle_identifier),
  };
  const files = deepLinksFiles(root, androidSource, appleSource);
  const discoveredPath = (platform, kind) => files.find(file => (!platform || file.platform === platform) && file.kind === kind)?.path || "";
  if (!android.manifest) android.manifest = discoveredPath("android", "manifest");
  if (!apple.infoPlist) apple.infoPlist = discoveredPath("apple", "info_plist");
  if (!apple.entitlements) apple.entitlements = discoveredPath("apple", "entitlements");
  const androidIssues = deepLinksIssues(androidSource, "android");
  const appleIssues = deepLinksIssues(appleSource, "apple");
  const globalIssues = issuesSource.map(entry => deepLinksIssue(entry, "")).filter(Boolean);
  const seenIssues = new Set();
  const issues = [...androidIssues, ...appleIssues, ...globalIssues].filter(issue => {
    const key = [issue.platform, issue.path, issue.line, issue.column, issue.severity, issue.code, issue.message].join("\u0000");
    if (seenIssues.has(key)) return false;
    seenIssues.add(key);
    return true;
  });
  const summary = deepLinksSummary(root, files, android, apple, issues);
  const configured = deepLinksConfigured(root, android, apple);
  const displayPath = candidate => candidate && projectRoot && !deepLinksAbsoluteRoot(candidate)
    ? `${projectRoot.replace(/[\\/]+$/, "")}/${candidate.replace(/^[\\/]+/, "")}`
    : candidate;
  for (const issue of issues) issue.path = displayPath(issue.path);
  for (const link of android.links) link.file = displayPath(link.file);
  return {
    root: projectRoot,
    status: firstStringValue(root.status, root.state, root.result),
    statusMessage: firstStringValue(root.message, root.detail, root.reason, summary.text),
    configured,
    files,
    android: { ...android, issues: androidIssues },
    apple: { ...apple, issues: appleIssues },
    otherIssues: globalIssues,
    summary,
    warnings: deepLinksFirstArray(root.warnings).map(value => typeof value === "string" ? value : firstStringValue(value?.message, value?.detail)).filter(Boolean),
  };
}

function deepLinksPlural(count, singular, plural = `${singular}s`) {
  return `${formatCount(count)} ${count === 1 ? singular : plural}`;
}

function deepLinksPanel(title, meta, ...children) {
  return el("div", { class: "panel-block" },
    el("div", { class: "panel-block-head" },
      el("h2", { text: title }),
      meta ? el("span", { class: "pane-meta", text: meta }) : null,
    ),
    ...children,
  );
}

function deepLinksTable(headers, rows) {
  return el("div", { class: "table-wrap" }, el("table", { class: "data-table" },
    el("thead", {}, el("tr", {}, ...headers.map(header => el("th", { class: header.num ? "num" : "", text: header.label })))),
    el("tbody", {}, ...rows),
  ));
}

function deepLinksEmptyBody(message) {
  return el("div", { class: "panel-block-body" }, el("p", { class: "dl-note", text: message }));
}

function deepLinksStat(label, value, detail = "") {
  return el("div", { class: "stat" },
    el("div", { class: "stat-label", text: label }),
    el("div", { class: "stat-value" }, formatCount(value), detail ? el("small", { text: ` ${detail}` }) : null),
  );
}

function deepLinksLocation(issue) {
  const path = issue.path || issue.file;
  if (path) {
    let location = path;
    if (issue.line !== undefined && issue.line !== null) location += `:${issue.line}`;
    if (issue.column !== undefined && issue.column !== null) location += `:${issue.column}`;
    return location;
  }
  if (issue.location) return issue.location;
  if (issue.line !== undefined && issue.line !== null) return `line ${issue.line}${issue.column !== undefined && issue.column !== null ? `:${issue.column}` : ""}`;
  return "—";
}

function deepLinksIssueRows(issues, limit) {
  return issues.slice(0, limit).map(issue => el("tr", { class: `dl-issue-row is-${issue.severity || "neutral"}` },
    el("td", {}, el("span", { class: `dl-severity is-${issue.severity || "neutral"}`, text: issue.severity || "issue" })),
    el("td", { class: "dl-issue-message", text: issue.message }),
    el("td", { class: "dl-location", text: deepLinksLocation(issue) }),
    el("td", { class: "dl-code", text: issue.code || "—" }),
    el("td", { class: "dl-fix", text: issue.fix || "—" }),
  ));
}

function deepLinksIssuesBody(issues, limit) {
  if (!issues.length) return deepLinksEmptyBody("No issues reported.");
  const body = deepLinksTable([
    { label: "Severity" },
    { label: "Issue" },
    { label: "Location" },
    { label: "Code" },
    { label: "Fix" },
  ], deepLinksIssueRows(issues, limit));
  if (issues.length <= limit) return body;
  return el("div", {},
    body,
    el("div", { class: "panel-block-body" }, el("p", { class: "dl-note", text: `Showing ${formatCount(limit)} of ${formatCount(issues.length)} reported issues.` })),
  );
}

function deepLinksEntryTable(headers, entries) {
  const rows = entries.slice(0, MAX_DEEP_LINKS_ENTRIES).map(entry => el("tr", {},
    el("td", { class: "dl-entry", text: entry.name }),
    headers.length > 1 ? el("td", { class: "dl-entry-detail", text: entry.detail || "—" }) : null,
  ));
  const body = deepLinksTable(headers, rows);
  if (entries.length <= MAX_DEEP_LINKS_ENTRIES) return body;
  return el("div", {},
    body,
    el("div", { class: "panel-block-body" }, el("p", { class: "dl-note", text: `Showing ${formatCount(MAX_DEEP_LINKS_ENTRIES)} of ${formatCount(entries.length)} reported entries.` })),
  );
}

function deepLinksSchemesBlock(label, entries) {
  if (!entries.length) return deepLinksEmptyBody(`No ${label} reported.`);
  return deepLinksEntryTable([{ label: "Scheme" }], entries);
}

function deepLinksDomainsBlock(label, entries) {
  if (!entries.length) return deepLinksEmptyBody(`No ${label} reported.`);
  return deepLinksEntryTable([{ label: "Domain" }], entries);
}

function deepLinksHostsBlock(label, entries) {
  if (!entries.length) return deepLinksEmptyBody(`No ${label} reported.`);
  return deepLinksEntryTable([{ label: "Host" }], entries);
}

function deepLinksIntentFiltersBody(links) {
  if (!links.length) return null;
  const rows = links.slice(0, MAX_DEEP_LINKS_ENTRIES).map(link => el("tr", {},
    el("td", { class: "dl-entry", text: link.activity || "—" }),
    el("td", { class: "dl-entry", text: link.schemes.join(", ") || "—" }),
    el("td", { class: "dl-entry", text: link.hosts.join(", ") || "—" }),
    el("td", { class: "dl-entry", text: link.paths.join(", ") || "—" }),
    el("td", { text: link.autoVerify === null ? "—" : link.autoVerify ? "Yes" : "No" }),
    el("td", { class: "dl-location", text: deepLinksLocation(link) }),
  ));
  const body = deepLinksTable([
    { label: "Activity" },
    { label: "Schemes" },
    { label: "Hosts" },
    { label: "Paths" },
    { label: "Auto verify" },
    { label: "Location" },
  ], rows);
  if (links.length <= MAX_DEEP_LINKS_ENTRIES) return body;
  return el("div", {},
    body,
    el("div", { class: "panel-block-body" }, el("p", { class: "dl-note", text: `Showing ${formatCount(MAX_DEEP_LINKS_ENTRIES)} of ${formatCount(links.length)} reported intent filters.` })),
  );
}

function deepLinksAssociatedBody(entries) {
  if (!entries.length) return deepLinksEmptyBody("No associated domains reported.");
  return deepLinksEntryTable([{ label: "Domain" }, { label: "Service" }], entries);
}

function deepLinksFilesPanel(files) {
  const block = deepLinksPanel("Discovered files", `${formatCount(files.length)} reported`);
  if (!files.length) {
    block.append(deepLinksEmptyBody("No platform files were reported for this root."));
    return block;
  }
  const rows = files.slice(0, MAX_DEEP_LINKS_FILES).map(file => {
    const status = deepLinksFileStatus(file.status || "found");
    return el("tr", { class: "dl-file-row" },
      el("td", { class: "dl-location", text: file.path || "—", attrs: file.path ? { title: file.path } : {} }),
      el("td", { text: deepLinksPlatformLabel(file.platform) }),
      el("td", { text: deepLinksKindLabel(file.kind) || "—" }),
      el("td", {}, el("span", { class: `dl-file-status is-${status.tone || "neutral"}`, text: status.label })),
      el("td", { class: "dl-file-detail", text: file.detail || "—" }),
    );
  });
  block.append(deepLinksTable([
    { label: "Path" },
    { label: "Platform" },
    { label: "Kind" },
    { label: "Status" },
    { label: "Detail" },
  ], rows));
  if (files.length > MAX_DEEP_LINKS_FILES) {
    block.append(el("div", { class: "panel-block-body" }, el("p", { class: "dl-note", text: `Showing ${formatCount(MAX_DEEP_LINKS_FILES)} of ${formatCount(files.length)} discovered files.` })));
  }
  return block;
}

function deepLinksAndroidPanel(android) {
  const configured = android.configured === false ? "Not configured · " : "";
  const block = deepLinksPanel("Android", `${configured}${deepLinksPlural(android.schemes.length, "scheme")} · ${deepLinksPlural(android.hosts.length, "host")}`);
  if (android.manifest || android.applicationId) {
    block.append(el("div", { class: "panel-block-body dl-source" },
      el("span", { text: "Manifest" }),
      el("code", { text: android.manifest || "not reported" }),
      android.applicationId ? el("span", { class: "dl-source-meta", text: `applicationId ${android.applicationId}` }) : null,
    ));
  }
  block.append(el("div", { class: "dl-subhead" }, el("h3", { text: "Schemes" })));
  block.append(deepLinksSchemesBlock("Android schemes", android.schemes));
  block.append(el("div", { class: "dl-subhead" }, el("h3", { text: "Hosts" })));
  block.append(deepLinksHostsBlock("Android hosts", android.hosts));
  if (android.links.length) {
    block.append(el("div", { class: "dl-subhead" }, el("h3", { text: "Intent filters" }), el("span", { class: "pane-meta", text: formatCount(android.links.length) })));
    block.append(deepLinksIntentFiltersBody(android.links));
  }
  block.append(el("div", { class: "dl-subhead" }, el("h3", { text: "Issues" }), el("span", { class: "pane-meta", text: formatCount(android.issues.length) })));
  block.append(deepLinksIssuesBody(android.issues, MAX_DEEP_LINKS_ISSUES));
  return block;
}

function deepLinksApplePanel(apple) {
  const configured = apple.configured === false ? "Not configured · " : "";
  const block = deepLinksPanel("Apple", `${configured}${deepLinksPlural(apple.schemes.length, "scheme")} · ${deepLinksPlural(apple.domains.length, "domain")}`);
  if (apple.infoPlist || apple.entitlements || apple.bundleId) {
    block.append(el("div", { class: "panel-block-body dl-source" },
      el("span", { text: "Info.plist" }),
      el("code", { text: apple.infoPlist || "not reported" }),
      apple.bundleId ? el("span", { class: "dl-source-meta", text: apple.bundleId }) : null,
    ));
    if (apple.entitlements) {
      block.append(el("div", { class: "panel-block-body dl-source" },
        el("span", { text: "Entitlements" }),
        el("code", { text: apple.entitlements }),
      ));
    }
  }
  block.append(el("div", { class: "dl-subhead" }, el("h3", { text: "Schemes" })));
  block.append(deepLinksSchemesBlock("Apple URL schemes", apple.schemes));
  block.append(el("div", { class: "dl-subhead" }, el("h3", { text: "Associated domains" })));
  block.append(apple.associatedDomains.length
    ? deepLinksAssociatedBody(apple.associatedDomains)
    : deepLinksDomainsBlock("associated domains", apple.domains));
  block.append(el("div", { class: "dl-subhead" }, el("h3", { text: "Issues" }), el("span", { class: "pane-meta", text: formatCount(apple.issues.length) })));
  block.append(deepLinksIssuesBody(apple.issues, MAX_DEEP_LINKS_ISSUES));
  return block;
}

function deepLinksScope(model, root) {
  return el("div", { class: "dl-scope" },
    icon("link"),
    el("span", {},
      el("strong", { text: "Project root" }),
      " ",
      el("code", { text: model.root || root || "not reported" }),
    ),
    model.status ? el("span", { class: "dl-scope-meta", text: model.status }) : null,
  );
}

function renderDeepLinks() {
  const host = dom.deepLinksContent;
  clear(host);
  const app = state.deepLinks;
  setDeepLinksChrome();
  if (!state.token) {
    showState(host, { icon: "device", title: "Not connected", message: "Connect to a controller to validate a project's deep links." });
    return;
  }
  if (app.rootError) {
    showState(host, { kind: "error", icon: "link", title: "Invalid project root", message: app.rootError });
    return;
  }
  if (app.loading && !app.data) {
    showState(host, { kind: "loading", title: "Validating deep links", message: `Waiting for the controller's get_ui_deep_links response for ${app.root || "the project root"}.` });
    return;
  }
  if (app.error) {
    if (app.error.code === "unavailable") {
      showState(host, {
        icon: "link",
        title: "Deep Links unavailable",
        message: errorMessage(app.error),
        action: "Retry",
        onAction: () => loadDeepLinks({ root: app.root, quiet: false }),
      });
      return;
    }
    if (app.error.code === "unknown_command") {
      showState(host, {
        icon: "link",
        title: "Deep Links are not supported by this controller",
        message: errorMessage(app.error),
      });
      return;
    }
    showState(host, {
      kind: "error",
      title: "Deep links validation failed",
      message: errorMessage(app.error),
      action: "Retry",
      onAction: () => loadDeepLinks({ root: app.root, quiet: false }),
    });
    return;
  }
  if (app.data === null || app.data === undefined) {
    if (!app.validatedRoot) {
      showState(host, { icon: "link", title: "No project validated", message: "Enter an absolute project root and choose Validate." });
      return;
    }
    showState(host, {
      icon: "link",
      title: "No validation result",
      message: "The controller returned no deep links payload. Nothing is substituted.",
      action: "Retry",
      onAction: () => loadDeepLinks({ root: app.root, quiet: false }),
    });
    return;
  }
  const model = normalizeDeepLinks(app.data);
  if (!model) {
    showState(host, {
      kind: "error",
      title: "Unrecognized deep links response",
      message: "The response contained no project files, Android or Apple configuration, issues or summary. No links are shown.",
      action: "Retry",
      onAction: () => loadDeepLinks({ root: app.root, quiet: false }),
    });
    return;
  }
  if (model.unavailable) {
    showState(host, {
      icon: "link",
      title: "Deep Links unavailable",
      message: model.reason,
      action: "Retry",
      onAction: () => loadDeepLinks({ root: app.root, quiet: false }),
    });
    return;
  }
  if (app.loading) {
    host.append(el("div", { class: "inline-spinner" }, el("span", { class: "spinner", attrs: { role: "progressbar", "aria-label": "Validating deep links" } }), "Validating deep links…"));
  }
  host.append(deepLinksScope(model, app.root));
  if (!model.configured) {
    const reported = model.statusMessage && model.statusMessage !== "No deep links configured." && model.statusMessage !== "No deep links configured"
      ? model.statusMessage
      : "No Android intent filters or Apple URL types were reported for this project.";
    host.append(el("div", { class: "dl-status is-empty" },
      icon("link"),
      el("div", {},
        el("strong", { text: "No deep links configured" }),
        el("span", { text: reported }),
      ),
    ));
  }
  host.append(el("div", { class: "stat-row" },
    deepLinksStat("Android schemes", model.summary.androidSchemes),
    deepLinksStat("Android hosts", model.summary.androidHosts),
    deepLinksStat("Apple schemes", model.summary.appleSchemes),
    deepLinksStat("Apple domains", model.summary.appleDomains),
    deepLinksStat("Files", model.summary.files),
    deepLinksStat("Issues", model.summary.issues, model.summary.errors || model.summary.warnings ? `${deepLinksPlural(model.summary.errors, "error")} · ${deepLinksPlural(model.summary.warnings, "warning")}` : ""),
  ));
  host.append(deepLinksFilesPanel(model.files));
  host.append(deepLinksAndroidPanel(model.android));
  host.append(deepLinksApplePanel(model.apple));
  if (model.otherIssues.length) {
    const block = deepLinksPanel("Other issues", formatCount(model.otherIssues.length));
    block.append(deepLinksIssuesBody(model.otherIssues, MAX_DEEP_LINKS_ISSUES));
    host.append(block);
  }
  for (const warning of model.warnings) host.append(errorInline(warning));
  host.append(el("div", { class: "dl-note", text: "Validation reads known Android and Apple project files under the given root. Paths and locations come from the controller's report." }));
}

function setDeepLinksChrome() {
  const app = state.deepLinks;
  dom.deepLinksValidate.disabled = !state.token || app.loading;
  if (!state.token) {
    dom.deepLinksMeta.textContent = "Connect to a controller to validate deep links.";
    return;
  }
  if (app.rootError) {
    dom.deepLinksMeta.textContent = "Invalid project root.";
    return;
  }
  if (app.loading) {
    dom.deepLinksMeta.textContent = "Validating project configuration…";
    return;
  }
  if (app.error) {
    dom.deepLinksMeta.textContent = app.error.code === "unavailable" ? "Deep Links unavailable." : "Validation failed.";
    return;
  }
  if (app.data === null || app.data === undefined) {
    dom.deepLinksMeta.textContent = app.validatedRoot ? "No validation result returned." : "Validate Android intent filters and Apple URL types in a project root.";
    return;
  }
  const model = normalizeDeepLinks(app.data);
  if (!model) {
    dom.deepLinksMeta.textContent = "Unrecognized deep links response.";
    return;
  }
  if (model.unavailable) {
    dom.deepLinksMeta.textContent = "Deep Links unavailable.";
    return;
  }
  const stateText = model.configured ? "Deep links configured" : "No deep links configured";
  dom.deepLinksMeta.textContent = `${stateText} · ${deepLinksPlural(model.summary.files, "file")} · ${deepLinksPlural(model.summary.androidSchemes, "Android scheme")} · ${deepLinksPlural(model.summary.appleDomains, "Apple domain")} · ${deepLinksPlural(model.summary.issues, "issue")} · validated ${formatWallTime(app.refreshedAt)}`;
}

function showDeepLinksRootError(message) {
  dom.deepLinksRootError.hidden = !message;
  dom.deepLinksRootError.textContent = message || "";
  dom.deepLinksRoot.setAttribute("aria-invalid", message ? "true" : "false");
}

async function loadDeepLinks(options = {}) {
  const app = state.deepLinks;
  const root = String(options.root ?? app.root).trim();
  if (!state.token) {
    renderDeepLinks();
    return;
  }
  const invalid = deepLinksRootError(root);
  if (invalid) {
    app.root = root;
    app.rootError = invalid;
    app.loading = false;
    app.error = null;
    app.data = null;
    app.loaded = false;
    app.validatedRoot = "";
    showDeepLinksRootError(invalid);
    renderDeepLinks();
    return;
  }
  if (app.loading) return;
  app.root = root;
  app.rootError = "";
  writeDeepLinksRootPreference(root);
  showDeepLinksRootError("");
  const request = app.request + 1;
  app.request = request;
  app.loading = true;
  app.error = null;
  app.validatedRoot = root;
  if (options.reset !== false) {
    app.data = null;
    app.loaded = false;
  }
  renderDeepLinks();
  try {
    const data = unwrapData(await rpc("get_ui_deep_links", { root }));
    if (request !== app.request) return;
    app.loaded = true;
    app.data = data === null || data === undefined ? null : data;
    app.refreshedAt = Date.now();
  } catch (error) {
    if (request !== app.request) return;
    app.error = error;
    if (error?.code === "unknown_command") {
      app.error = new RpcError("This controller does not expose get_ui_deep_links.", "unknown_command");
    }
    app.data = null;
    app.loaded = false;
    if (options.quiet !== true && error?.code !== "unavailable") toast(errorMessage(error), "error", 8000);
  } finally {
    if (request === app.request) {
      app.loading = false;
      renderDeepLinks();
    }
  }
}

function submitDeepLinks(event) {
  event?.preventDefault();
  const root = dom.deepLinksRoot.value.trim();
  dom.deepLinksRoot.value = root;
  state.deepLinks.root = root;
  loadDeepLinks({ root, quiet: false });
}

function ensureDeepLinks() {
  renderDeepLinks();
}

/* Debugger
 *
 * A debugger session is controller-level and keyed by process. Status comes from
 * get_ui_debugger, controls from attach_ui_debugger, detach_ui_debugger and
 * control_ui_debugger, stopped state from get_ui_debug_stack, variables from
 * read_ui_debug_variables, expression results from evaluate_ui_debug and
 * breakpoints from set_ui_breakpoint and clear_ui_breakpoint. The controller
 * has no source-text tool yet; the source pane asks for a debugger source tool
 * and shows the captured reason instead of substituting lines when none exists.
 * Every paused state, frame, scope, variable, breakpoint and source line shown
 * here comes from a response.
 */

function emptyDebugger() {
  return {
    loaded: false,
    status: "idle",
    phase: "",
    reason: "",
    reasonCode: "",
    unavailable: false,
    adapterAvailable: true,
    warning: "",
    developerMode: null,
    error: null,
    adapter: null,
    pid: null,
    sessionId: "",
    attached: false,
    stop: null,
    statusLoading: false,
    request: 0,
    lastRefresh: 0,
    threads: [],
    threadsError: null,
    threadId: null,
    frames: [],
    stackLoaded: false,
    stackLoading: false,
    stackError: null,
    frameIndex: null,
    frameId: null,
    framesTotal: null,
    framesTruncated: false,
    stackRequest: 0,
    scopes: [],
    scopesLoaded: false,
    scopesLoading: false,
    scopesError: null,
    scopesTruncated: false,
    scopesRequest: 0,
    variableChildren: new Map(),
    variableLoading: new Set(),
    variableErrors: new Map(),
    variableRequests: new Map(),
    variableTruncated: new Set(),
    variableEpoch: 0,
    expandedScopes: new Set(),
    expandedVariables: new Set(),
    source: null,
    sourceLoaded: false,
    sourceLoading: false,
    sourceError: null,
    sourceRequest: 0,
    breakpoints: [],
    disabledBreakpoints: new Map(),
    busy: "",
    evaluateBusy: false,
    evaluateRequest: 0,
    console: [],
    timer: null,
    renderSig: {},
  };
}

function debuggerRoot(data) {
  if (!isObjectValue(data)) return {};
  return isObjectValue(data.debugger) ? data.debugger : data;
}

function debuggerSessionObject(root) {
  return isObjectValue(root.session) ? root.session : {};
}

function debuggerText(value) {
  if (value === null || value === undefined) return "";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return "";
}

function debuggerId(value) {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && value.trim()) {
    const text = value.trim();
    return /^-?\d+$/.test(text) ? Number(text) : text;
  }
  return null;
}

function debuggerVariablesReference(value) {
  const id = debuggerId(value);
  return id === 0 || id === "0" ? null : id;
}

function debuggerNumber(value) {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && /^-?\d+(\.\d+)?$/.test(value.trim())) return Number(value);
  return null;
}

function debuggerArray(...values) {
  for (const value of values) {
    if (Array.isArray(value)) return value;
  }
  return null;
}

function debuggerPathKey(value) {
  let path = String(value ?? "").trim();
  if (!path) return "";
  path = path.replace(/^file:\/\//i, "");
  try { path = decodeURIComponent(path); } catch {}
  path = path.replace(/\\/g, "/").replace(/\/+$/, "");
  return path;
}

function debuggerPathsMatch(left, right) {
  const a = debuggerPathKey(left);
  const b = debuggerPathKey(right);
  if (!a || !b) return false;
  const fold = /^[a-z]:\//i.test(a) || /^[a-z]:\//i.test(b) || String(state.platform).toLowerCase().includes("win");
  const x = fold ? a.toLowerCase() : a;
  const y = fold ? b.toLowerCase() : b;
  return x === y || x.endsWith(`/${y}`) || y.endsWith(`/${x}`);
}

function debuggerRawStatus(root) {
  const session = debuggerSessionObject(root);
  return firstStringValue(root.status, root.state, root.phase, session.status, session.state, session.phase)
    .trim()
    .toLowerCase()
    .replace(/[\s-]+/g, "_");
}

function debuggerUnavailableInfo(root) {
  const session = debuggerSessionObject(root);
  const raw = debuggerRawStatus(root);
  const marker = root.unavailable ?? root.debugger_unavailable ?? session.unavailable;
  const flagged = root.available === false
    || marker === true
    || (isObjectValue(marker) && marker.available !== true);
  const named = ["unavailable", "unsupported", "not_supported", "not_available", "denied", "disabled"].includes(raw);
  if (!flagged && !named) return null;
  const markerReason = typeof marker === "string"
    ? marker
    : isObjectValue(marker)
      ? firstStringValue(marker.reason, marker.message, marker.detail)
      : "";
  const error = root.error;
  const reason = firstStringValue(
    markerReason,
    root.reason,
    root.detail,
    typeof root.message === "string" && root.message !== root.status ? root.message : "",
    isObjectValue(error) ? firstStringValue(error.reason, error.message, error.detail) : debuggerText(error),
    session.reason,
    session.detail,
    root.warning,
  );
  const code = firstStringValue(
    root.reason_code,
    root.error_code,
    isObjectValue(error) ? error.code : "",
    isObjectValue(marker) ? marker.code : "",
    session.reason_code,
  );
  return { reason, code };
}

function debuggerStatusErrorInfo(root) {
  const session = debuggerSessionObject(root);
  const error = root.error;
  const reason = firstStringValue(
    isObjectValue(error) ? firstStringValue(error.message, error.reason, error.detail) : debuggerText(error),
    root.error_message,
    root.message,
    root.reason,
    session.error,
    session.message,
    isObjectValue(session.last_error) ? firstStringValue(session.last_error.message, session.last_error.reason) : "",
    isObjectValue(root.last_error) ? firstStringValue(root.last_error.message, root.last_error.reason) : "",
  );
  const code = firstStringValue(
    root.error_code,
    root.reason_code,
    isObjectValue(error) ? error.code : "",
    session.error_code,
    isObjectValue(session.last_error) ? session.last_error.code : "",
    isObjectValue(root.last_error) ? root.last_error.code : "",
  );
  return { reason, code };
}

function normalizeDebuggerStatus(data) {
  const root = debuggerRoot(data);
  const session = debuggerSessionObject(root);
  const raw = debuggerRawStatus(root);
  const unavailable = debuggerUnavailableInfo(root);
  if (unavailable) return { status: "unavailable", phase: raw, reason: unavailable.reason, code: unavailable.code };
  if (["failed", "error"].includes(raw)) {
    const info = debuggerStatusErrorInfo(root);
    const code = info.code;
    const refused = ["unavailable", "not_allowed", "attach_denied", "permission"].includes(code);
    return {
      status: refused ? "unavailable" : "error",
      phase: raw,
      reason: info.reason,
      code,
    };
  }
  const sessionPresent = Object.prototype.hasOwnProperty.call(root, "session") && isObjectValue(root.session);
  const sessionAbsent = Object.prototype.hasOwnProperty.call(root, "session") && root.session === null;
  const paused = root.paused === true
    || session.paused === true
    || ["paused", "suspended", "break", "stopped", "stopped_on_breakpoint"].includes(raw);
  const running = root.running === true
    || session.running === true
    || ["running", "resumed", "continued"].includes(raw);
  const attaching = root.attaching === true
    || ["attaching", "initializing", "launching", "starting", "connecting", "detaching"].includes(raw);
  const attachedFlag = root.attached === true
    || session.attached === true
    || ["attached", "running", "stopped"].includes(raw);
  const detachedFlag = root.detached === true
    || session.detached === true
    || root.attached === false
    || session.attached === false;
  const failed = ["error", "failed"].includes(raw) || (root.error !== undefined && root.error !== null);
  let status = "unknown";
  if (attaching) status = "attaching";
  else if (paused) status = "paused";
  else if (running) status = "running";
  else if (detachedFlag) status = "detached";
  else if (attachedFlag) status = "attached";
  else if (["detached", "not_attached", "not-attached", "disconnected", "inactive", "idle", "none"].includes(raw)) status = "detached";
  else if (["terminated", "exited", "ended", "gone", "closed"].includes(raw)) status = "terminated";
  else if (sessionAbsent && !attachedFlag) status = "detached";
  else if (sessionPresent && !raw && !attachedFlag) status = "detached";
  else if (failed) status = "error";
  if (status === "error") {
    const info = debuggerStatusErrorInfo(root);
    return { status, phase: raw, reason: info.reason, code: info.code };
  }
  return { status, phase: raw, reason: "", code: "" };
}

function normalizeDebuggerStop(raw) {
  if (!isObjectValue(raw)) return null;
  const hits = Array.isArray(raw.hit_breakpoint_ids)
    ? raw.hit_breakpoint_ids.map(debuggerId).filter(id => id !== null)
    : Array.isArray(raw.hitBreakpointIds)
      ? raw.hitBreakpointIds.map(debuggerId).filter(id => id !== null)
      : [];
  return {
    reason: firstStringValue(raw.reason),
    description: firstStringValue(raw.description),
    text: firstStringValue(raw.text),
    threadId: debuggerId(raw.thread_id ?? raw.threadId),
    allThreadsStopped: raw.all_threads_stopped === true || raw.allThreadsStopped === true,
    hitBreakpointIds: hits,
  };
}

function debuggerStatusIsExplicit(data) {
  const root = debuggerRoot(data);
  return ["status", "state", "phase", "session", "available", "unavailable", "attached", "detached", "paused", "running", "attaching", "terminated", "sessions", "warning", "developer_mode", "last_error"].some(
    key => Object.prototype.hasOwnProperty.call(root, key),
  );
}

function debuggerHasData(data) {
  const root = debuggerRoot(data);
  return ["status", "state", "phase", "session", "available", "unavailable", "attached", "detached", "paused", "running", "attaching", "terminated", "sessions", "warning", "developer_mode", "last_error", "pid", "adapter", "debug_adapter", "dap", "threads", "frames", "stack", "call_stack", "scopes", "source", "breakpoints", "thread_id", "frame_id", "stop", "total_frames", "truncated", "threads_error", "scopes_error"].some(
    key => Object.prototype.hasOwnProperty.call(root, key),
  );
}

function debuggerAdapterFrom(data, fallback = null) {
  const root = debuggerRoot(data);
  const session = debuggerSessionObject(root);
  const raw = root.adapter ?? root.debug_adapter ?? root.dap ?? session.adapter ?? session.debug_adapter;
  const name = firstStringValue(root.adapter_name, session.adapter_name);
  const version = firstStringValue(root.adapter_version, session.adapter_version);
  if (raw === undefined || raw === null) {
    if (!name && !version) return fallback;
    return { name: name || fallback?.name || "", version: version || fallback?.version || "", path: fallback?.path || "", protocol: fallback?.protocol || "" };
  }
  if (typeof raw === "string") return { name: raw, version: version || fallback?.version || "", path: fallback?.path || "", protocol: fallback?.protocol || "" };
  if (!isObjectValue(raw)) return fallback;
  const path = firstStringValue(raw.path, raw.executable, raw.binary, raw.command);
  const adapter = {
    name: firstStringValue(raw.name, raw.id, raw.adapter, name) || (path ? basename(path) : ""),
    version: firstStringValue(raw.version, raw.build, raw.llvm_version, version),
    path,
    protocol: firstStringValue(raw.protocol, raw.transport, raw.kind) || "dap",
  };
  if (!adapter.name && !adapter.version && !adapter.path && !adapter.protocol) return fallback;
  return adapter;
}

function debuggerAdapterLabel(adapter) {
  if (!adapter) return "";
  const clean = value => String(value || "").replace(/\s+/g, " ").trim();
  const name = clean(adapter.name);
  const version = clean(adapter.version);
  const path = clean(adapter.path);
  const llvm = /LLVM version ([0-9.]+)/i.exec(version);
  const lldb = /liblldb:\s*(lldb-[0-9.]+)/i.exec(version);
  const shortVersion = llvm
    ? [`LLVM ${llvm[1]}`, lldb?.[1]].filter(Boolean).join(" · ")
    : version;
  const parts = [name || path, shortVersion];
  const label = parts.filter(Boolean).join(" · ");
  if (path && path !== name) return `${label} · ${path}`;
  return label;
}

function debuggerFirstNumber(...values) {
  for (const value of values) {
    const number = debuggerNumber(value);
    if (number !== null) return number;
  }
  return null;
}

function debuggerPidFrom(data, fallback) {
  const root = debuggerRoot(data);
  const session = debuggerSessionObject(root);
  const pid = debuggerFirstNumber(root.pid, root.process_id, session.pid, session.process_id, root.process?.pid);
  return pid ?? fallback;
}

function debuggerSessionIdFrom(data, fallback) {
  const root = debuggerRoot(data);
  const session = debuggerSessionObject(root);
  return firstStringValue(root.session_id, root.sessionId, session.id, session.session_id, fallback);
}

function normalizeDebuggerThread(raw, index) {
  if (!isObjectValue(raw)) return null;
  const id = debuggerId(raw.id ?? raw.thread_id ?? raw.threadId);
  const name = firstStringValue(raw.name, raw.label, raw.title) || (id !== null ? `Thread ${id}` : `Thread ${index + 1}`);
  return {
    id,
    name,
    state: firstStringValue(raw.state, raw.status),
    current: raw.current === true || raw.selected === true || raw.stopped === true,
  };
}

function normalizeDebuggerThreads(raw) {
  if (!Array.isArray(raw)) return null;
  return raw.map(normalizeDebuggerThread).filter(Boolean);
}

function normalizeDebuggerSourceRef(raw, fallback = {}) {
  if (typeof raw === "string") return { path: raw, name: raw, sourceReference: fallback.sourceReference ?? null };
  if (!isObjectValue(raw)) return fallback;
  return {
    path: firstStringValue(raw.path, raw.file, raw.file_path, fallback.path),
    name: firstStringValue(raw.name, raw.path, raw.file, fallback.name),
    sourceReference: debuggerId(raw.sourceReference ?? raw.source_reference ?? raw.reference) ?? fallback.sourceReference ?? null,
  };
}

function normalizeDebuggerSource(raw, fallback = {}) {
  if (raw === null || raw === undefined) return null;
  if (typeof raw === "string") {
    return {
      path: raw,
      name: raw,
      sourceReference: fallback.sourceReference ?? null,
      lines: [],
      reason: "",
      unavailable: false,
      language: "",
      truncated: false,
      frameId: fallback.frameId ?? null,
    };
  }
  if (!isObjectValue(raw)) return null;
  const nested = isObjectValue(raw.source) ? raw.source : null;
  const path = firstStringValue(raw.path, raw.file, raw.file_path, nested?.path, nested?.file, fallback.path);
  const sourceReference = debuggerId(raw.sourceReference ?? raw.source_reference ?? raw.reference ?? nested?.sourceReference ?? nested?.source_reference) ?? fallback.sourceReference ?? null;
  const content = [
    typeof raw.content === "string" ? raw.content : "",
    typeof raw.text === "string" ? raw.text : "",
    typeof raw.code === "string" ? raw.code : "",
    typeof raw.source === "string" ? raw.source : "",
    typeof nested?.content === "string" ? nested.content : "",
    typeof nested?.text === "string" ? nested.text : "",
  ].find(value => value !== "") ?? "";
  const rawLines = debuggerArray(raw.lines, raw.code_lines, nested?.lines, Array.isArray(raw.content) ? raw.content : null);
  const sourceLines = rawLines ?? (content ? content.split(/\r?\n/) : []);
  const startLine = debuggerNumber(raw.start_line ?? raw.startLine ?? raw.line_start) ?? 1;
  const lines = sourceLines.map((line, index) => {
    if (isObjectValue(line)) {
      const number = debuggerNumber(line.number ?? line.line ?? line.line_number) ?? startLine + index;
      const text = typeof line.text === "string"
        ? line.text
        : typeof line.content === "string"
          ? line.content
          : typeof line.code === "string"
            ? line.code
            : "";
      return { number, text };
    }
    return { number: startLine + index, text: line === null || line === undefined ? "" : String(line) };
  });
  const truncated = lines.length > MAX_DEBUGGER_SOURCE_LINES;
  const currentLine = debuggerNumber(raw.currentLine ?? raw.current_line ?? raw.line ?? raw.lineNumber) ?? fallback.line ?? null;
  const currentColumn = debuggerNumber(raw.currentColumn ?? raw.current_column ?? raw.column ?? raw.columnNumber) ?? fallback.column ?? null;
  const unavailableMarker = raw.unavailable ?? raw.available;
  const reason = firstStringValue(
    raw.reason,
    raw.detail,
    isObjectValue(raw.unavailable) ? firstStringValue(raw.unavailable.reason, raw.unavailable.message) : "",
    unavailableMarker === false ? raw.message : "",
  );
  return {
    path,
    name: firstStringValue(raw.name, path),
    sourceReference,
    lines: truncated ? lines.slice(0, MAX_DEBUGGER_SOURCE_LINES) : lines,
    truncated,
    reason,
    unavailable: raw.available === false || raw.unavailable === true || isObjectValue(raw.unavailable),
    language: firstStringValue(raw.language, raw.language_id, nested?.language),
    frameId: fallback.frameId ?? null,
    currentLine,
    currentColumn,
  };
}

function normalizeDebuggerFrame(raw, index) {
  if (!isObjectValue(raw)) return null;
  const source = normalizeDebuggerSourceRef(
    isObjectValue(raw.source) ? raw.source : raw.location,
    { path: firstStringValue(raw.path, raw.file), name: firstStringValue(raw.file, raw.path) },
  );
  const line = debuggerNumber(raw.line ?? raw.lineNumber ?? raw.location?.line);
  const column = debuggerNumber(raw.column ?? raw.columnNumber ?? raw.location?.column);
  const id = debuggerId(raw.id ?? raw.frame_id ?? raw.frameId);
  const name = firstStringValue(raw.name, raw.function, raw.function_name, raw.method, raw.label);
  return {
    id,
    name,
    index,
    source,
    line,
    column,
    module: firstStringValue(raw.module, raw.module_id, raw.moduleId),
  };
}

function normalizeDebuggerFrames(raw) {
  if (!Array.isArray(raw)) return null;
  return raw.map(normalizeDebuggerFrame).filter(Boolean);
}

function debuggerScalarText(value) {
  if (value === null) return "null";
  if (value === undefined) return "";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean" || typeof value === "bigint") return String(value);
  try {
    return JSON.stringify(value);
  } catch {
    return "";
  }
}

function normalizeDebuggerVariable(raw, index) {
  if (!isObjectValue(raw)) return null;
  const name = firstStringValue(raw.name, raw.key, raw.label) || `[${index}]`;
  const reference = debuggerVariablesReference(raw.variablesReference ?? raw.variables_reference ?? raw.variables_ref);
  const childrenRaw = debuggerArray(raw.variables, raw.children, isObjectValue(raw.value) ? raw.value.variables : null);
  const children = childrenRaw ? normalizeDebuggerVariables(childrenRaw) : [];
  return {
    name,
    value: raw.value === undefined ? "" : debuggerScalarText(raw.value),
    valueTruncated: raw.value_truncated === true,
    type: firstStringValue(raw.type, raw.value_type, raw.class),
    reference,
    children,
    namedVariables: debuggerNumber(raw.namedVariables ?? raw.named_variables),
    indexedVariables: debuggerNumber(raw.indexedVariables ?? raw.indexed_variables),
    evaluateName: firstStringValue(raw.evaluateName, raw.evaluate_name),
  };
}

function normalizeDebuggerVariables(raw) {
  if (!Array.isArray(raw)) return [];
  return raw.map(normalizeDebuggerVariable).filter(Boolean);
}

function normalizeDebuggerScope(raw, index) {
  if (!isObjectValue(raw)) return null;
  return {
    name: firstStringValue(raw.name, raw.label) || `Scope ${index + 1}`,
    reference: debuggerVariablesReference(raw.variablesReference ?? raw.variables_reference ?? raw.variables_ref),
    expensive: raw.expensive === true,
    variables: normalizeDebuggerVariables(debuggerArray(raw.variables, raw.children) ?? []),
    truncated: raw.truncated === true,
    namedVariables: debuggerNumber(raw.namedVariables ?? raw.named_variables),
    indexedVariables: debuggerNumber(raw.indexedVariables ?? raw.indexed_variables),
  };
}

function normalizeDebuggerScopes(raw) {
  if (!Array.isArray(raw)) return null;
  return raw.map(normalizeDebuggerScope).filter(Boolean);
}

function normalizeDebuggerBreakpoint(raw, index, fallbackPath = "") {
  if (!isObjectValue(raw)) return null;
  const location = isObjectValue(raw.location) ? raw.location : {};
  const source = isObjectValue(raw.source) ? raw.source : {};
  const path = firstStringValue(raw.path, raw.file, raw.file_path, raw.source_path, source.path, source.file, location.path, fallbackPath);
  const line = debuggerNumber(raw.line ?? raw.line_number ?? location.line ?? location.line_number);
  const column = debuggerNumber(raw.column ?? raw.column_number ?? location.column);
  const id = debuggerId(raw.id ?? raw.breakpoint_id ?? raw.breakpointId);
  return {
    id,
    path,
    line,
    column,
    resolvedLine: debuggerNumber(raw.resolved_line ?? raw.resolvedLine),
    label: firstStringValue(raw.label),
    message: firstStringValue(raw.message),
    enabled: raw.enabled === false || raw.disabled === true ? false : true,
    verified: raw.verified === true ? true : raw.verified === false ? false : null,
    condition: firstStringValue(raw.condition),
    hitCondition: firstStringValue(raw.hitCondition, raw.hit_condition),
    logMessage: firstStringValue(raw.logMessage, raw.log_message),
  };
}

function normalizeDebuggerBreakpoints(raw, fallbackPath = "") {
  if (!Array.isArray(raw)) return null;
  return raw.map((breakpoint, index) => normalizeDebuggerBreakpoint(breakpoint, index, fallbackPath)).filter(Boolean);
}

function debuggerBreakpointList(data) {
  const root = debuggerRoot(data);
  const session = debuggerSessionObject(root);
  const source = isObjectValue(root.source) ? root.source : isObjectValue(root.selected_source) ? root.selected_source : null;
  return normalizeDebuggerBreakpoints(
    debuggerArray(root.breakpoints, root.breakpoint_list, session.breakpoints, source?.breakpoints),
    firstStringValue(source?.path),
  );
}

function sortDebuggerBreakpoints(breakpoints) {
  return [...breakpoints].sort((left, right) => {
    const path = debuggerPathKey(left.path).localeCompare(debuggerPathKey(right.path));
    if (path !== 0) return path;
    return (left.line ?? 0) - (right.line ?? 0);
  });
}

function debuggerBreakpointsAt(path, line) {
  return debuggerVisibleBreakpoints().filter(breakpoint => breakpoint.line === line && debuggerPathsMatch(breakpoint.path, path));
}

function debuggerBreakpointLabel(breakpoint) {
  const path = breakpoint.path || "unknown source";
  return breakpoint.line === null ? path : `${path}:${breakpoint.line}`;
}

function debuggerThreadList(data) {
  const root = debuggerRoot(data);
  const session = debuggerSessionObject(root);
  return normalizeDebuggerThreads(debuggerArray(root.threads, root.thread_list, session.threads));
}

function debuggerFrameList(data) {
  const root = debuggerRoot(data);
  const session = debuggerSessionObject(root);
  return normalizeDebuggerFrames(debuggerArray(root.frames, root.stack, root.call_stack, root.callStack, session.frames, session.stack));
}

function debuggerScopeList(data) {
  const root = debuggerRoot(data);
  const session = debuggerSessionObject(root);
  const frame = isObjectValue(root.frame) ? root.frame : null;
  return normalizeDebuggerScopes(debuggerArray(root.scopes, root.scope_list, session.scopes, frame?.scopes));
}

function debuggerVariablesFrom(data) {
  if (Array.isArray(data)) return normalizeDebuggerVariables(data);
  const root = debuggerRoot(data);
  const result = isObjectValue(root.result) ? root.result : null;
  const raw = debuggerArray(root.variables, root.children, result?.variables, root.variable?.variables);
  return raw ? normalizeDebuggerVariables(raw) : null;
}

function selectedDebuggerFrame() {
  const dbg = state.debugger;
  if (dbg.frameIndex === null) return null;
  return dbg.frames[dbg.frameIndex] || null;
}

function debuggerCurrentLocation() {
  const dbg = state.debugger;
  if (dbg.status !== "paused") return null;
  const frame = selectedDebuggerFrame();
  if (!frame || frame.line === null) return null;
  if (!dbg.source) return null;
  const sourcePath = dbg.source.path;
  const framePath = frame.source?.path;
  const sameFrame = dbg.source.frameId !== null && frame.id !== null && dbg.source.frameId === frame.id
    && (!sourcePath || !framePath || debuggerPathsMatch(sourcePath, framePath));
  if (!sameFrame && !(sourcePath && framePath && debuggerPathsMatch(sourcePath, framePath))) return null;
  return { line: frame.line, column: frame.column };
}

function debuggerFrameLabel(frame) {
  return frame.name || `Frame ${frame.index + 1}`;
}

function debuggerFrameLocation(frame) {
  const path = frame.source?.path || frame.source?.name || "";
  const file = path ? basename(path) : "";
  const line = frame.line !== null ? String(frame.line) : "";
  return [file, line].filter(Boolean).join(":");
}

function resetDebuggerVariables() {
  const dbg = state.debugger;
  dbg.variableEpoch += 1;
  dbg.variableChildren.clear();
  dbg.variableLoading.clear();
  dbg.variableErrors.clear();
  dbg.variableRequests.clear();
  dbg.variableTruncated.clear();
  dbg.expandedScopes.clear();
  dbg.expandedVariables.clear();
  dbg.renderSig.variables = "";
}

function clearDebuggerSessionData(options = {}) {
  const dbg = state.debugger;
  dbg.threads = [];
  dbg.threadsError = null;
  dbg.threadId = null;
  dbg.frames = [];
  dbg.stackLoaded = false;
  dbg.stackError = null;
  dbg.frameIndex = null;
  dbg.frameId = null;
  dbg.framesTotal = null;
  dbg.framesTruncated = false;
  dbg.stop = null;
  dbg.scopes = [];
  dbg.scopesLoaded = false;
  dbg.scopesError = null;
  dbg.scopesTruncated = false;
  resetDebuggerVariables();
  if (!options.keepSource) {
    dbg.source = null;
    dbg.sourceLoaded = false;
    dbg.sourceError = null;
  }
  if (!options.keepBreakpoints) {
    dbg.breakpoints = [];
  }
  dbg.renderSig = {};
}

function applyDebuggerSnapshot(data) {
  const dbg = state.debugger;
  const root = debuggerRoot(data);
  const session = debuggerSessionObject(root);
  const explicit = debuggerStatusIsExplicit(data);
  const previousStatus = dbg.status;
  let nextStatus = previousStatus;
  let nextPhase = dbg.phase;
  let nextReason = dbg.reason;
  let nextCode = dbg.reasonCode;
  let nextUnavailable = dbg.unavailable;
  let nextError = dbg.error;

  if (explicit) {
    const status = normalizeDebuggerStatus(data);
    nextStatus = status.status;
    nextPhase = status.phase || "";
    nextReason = status.reason || "";
    nextCode = status.code || "";
    nextUnavailable = status.status === "unavailable";
    nextError = status.status === "error" ? (status.reason || "The debugger reported an error without a message.") : null;
  }

  if (root.available !== undefined) dbg.adapterAvailable = root.available !== false;
  if (root.warning !== undefined) dbg.warning = firstStringValue(root.warning);
  if (root.developer_mode !== undefined) {
    dbg.developerMode = root.developer_mode === true ? true : root.developer_mode === false ? false : null;
  }
  const adapter = debuggerAdapterFrom(data, dbg.adapter);
  if (adapter) dbg.adapter = adapter;
  const pid = debuggerPidFrom(data, dbg.pid);
  if (pid !== null) dbg.pid = pid;
  const sessionId = debuggerSessionIdFrom(data, dbg.sessionId);
  if (sessionId) dbg.sessionId = sessionId;
  dbg.loaded = true;

  const threads = debuggerThreadList(data);
  const frames = debuggerFrameList(data);
  const scopes = debuggerScopeList(data);
  const breakpoints = debuggerBreakpointList(data);
  const stop = normalizeDebuggerStop(root.stop !== undefined ? root.stop : session.stop);
  const sourceRaw = root.source !== undefined ? root.source : root.selected_source !== undefined ? root.selected_source : undefined;
  const ended = ["detached", "terminated", "unavailable", "error"].includes(nextStatus);
  const paused = nextStatus === "paused";

  if (explicit) {
    dbg.status = nextStatus;
    dbg.phase = nextPhase;
    dbg.reason = nextReason;
    dbg.reasonCode = nextCode;
    dbg.unavailable = nextUnavailable;
    dbg.error = nextError;
    dbg.attached = ["attached", "running", "paused"].includes(nextStatus);
    dbg.stop = paused ? (stop || dbg.stop) : null;
    if (ended) {
      clearDebuggerSessionData({ keepSource: nextStatus !== "unavailable" && nextStatus !== "error", keepBreakpoints: false });
      if (["unavailable", "error", "detached", "terminated"].includes(nextStatus) && !isObjectValue(root.session)) {
        dbg.pid = null;
        dbg.sessionId = "";
      }
    } else if (!paused) {
      dbg.frames = [];
      dbg.stackLoaded = false;
      dbg.stackError = null;
      dbg.frameIndex = null;
      dbg.frameId = null;
      dbg.framesTotal = null;
      dbg.framesTruncated = false;
      dbg.scopes = [];
      dbg.scopesLoaded = false;
      dbg.scopesError = null;
      dbg.scopesTruncated = false;
      dbg.threads = [];
      dbg.threadsError = null;
      dbg.threadId = null;
      resetDebuggerVariables();
      dbg.sourceLoaded = false;
    } else if (previousStatus !== "paused") {
      dbg.frames = [];
      dbg.stackLoaded = false;
      dbg.stackError = null;
      dbg.frameIndex = null;
      dbg.frameId = null;
      dbg.framesTotal = null;
      dbg.framesTruncated = false;
      dbg.scopes = [];
      dbg.scopesLoaded = false;
      dbg.scopesError = null;
      dbg.scopesTruncated = false;
      resetDebuggerVariables();
      dbg.sourceLoaded = false;
    }
  }

  if (threads) {
    dbg.threads = threads;
    dbg.threadsError = null;
    const currentThread = debuggerId(root.thread_id ?? root.current_thread_id ?? root.threadId ?? session.thread_id ?? session.current_thread_id);
    if (currentThread !== null) dbg.threadId = currentThread;
    if (dbg.threadId !== null && !threads.some(thread => thread.id === dbg.threadId)) dbg.threadId = null;
    if (dbg.threadId === null) {
      const current = threads.find(thread => thread.current);
      if (current) dbg.threadId = current.id;
      else if (threads.length === 1) dbg.threadId = threads[0].id;
    }
  }
  const stopThread = debuggerId(root.thread_id ?? stop?.threadId);
  if (stopThread !== null) dbg.threadId = stopThread;

  if (frames) {
    dbg.frames = frames;
    dbg.stackLoaded = true;
    dbg.stackError = null;
    dbg.framesTotal = debuggerNumber(root.total_frames) ?? dbg.framesTotal;
    dbg.framesTruncated = root.truncated === true;
    const currentFrame = debuggerId(root.frame_id ?? root.current_frame_id ?? root.frameId ?? session.frame_id ?? session.current_frame_id);
    let index = currentFrame !== null ? frames.findIndex(frame => frame.id === currentFrame) : -1;
    if (index < 0 && frames.length) index = 0;
    dbg.frameIndex = index >= 0 ? index : null;
    dbg.frameId = index >= 0 ? frames[index].id : null;
  }

  if (scopes) {
    const signature = scopes.map(scope => `${scope.name}:${scope.reference ?? ""}`).join("|");
    const previousSignature = dbg.scopes.map(scope => `${scope.name}:${scope.reference ?? ""}`).join("|");
    if (signature !== previousSignature) resetDebuggerVariables();
    dbg.scopes = scopes;
    dbg.scopesLoaded = true;
    dbg.scopesError = null;
    dbg.scopesTruncated = root.scopes_truncated === true;
  }

  if (sourceRaw !== undefined) {
    if (sourceRaw === null) {
      dbg.source = null;
      dbg.sourceLoaded = true;
      dbg.sourceError = null;
    } else {
      const source = normalizeDebuggerSource(sourceRaw);
      if (source) {
        dbg.source = source;
        dbg.sourceLoaded = true;
        dbg.sourceError = null;
      }
    }
  }

  if (breakpoints) {
    dbg.breakpoints = sortDebuggerBreakpoints(breakpoints);
    for (const breakpoint of dbg.breakpoints) {
      dbg.disabledBreakpoints.delete(debuggerBreakpointKey(breakpoint));
    }
  }
}

function debuggerRequestParams(params = {}) {
  const body = {};
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== null) body[key] = value;
  }
  return body;
}

async function debuggerCall(method, params = {}, options = {}) {
  const target = options.target ?? state.targetId;
  if (!target) throw new RpcError("Select a runtime target first.", "no_target");
  const timeoutMs = options.timeout ?? DEBUGGER_RPC_TIMEOUT_MS;
  const externalSignal = options.rpcOptions?.signal;
  const controller = externalSignal ? null : new AbortController();
  const timer = controller ? setTimeout(() => controller.abort(), timeoutMs) : null;
  try {
    return await unwrapData(await rpc(method, { target, ...debuggerRequestParams(params) }, {
      ...options.rpcOptions,
      signal: externalSignal || controller.signal,
    }));
  } catch (error) {
    if (error?.name === "AbortError") {
      if (externalSignal?.aborted) throw error;
      throw new RpcError(`The debugger request timed out after ${Math.round(timeoutMs / 1000)} s.`, "timeout");
    }
    throw error;
  } finally {
    if (timer) clearTimeout(timer);
  }
}

async function debuggerSourceCall(params, options = {}) {
  const missing = [];
  for (const method of ["get_ui_debug_source", "read_ui_debug_source"]) {
    try {
      return { method, data: await debuggerCall(method, params, options) };
    } catch (error) {
      if (error?.code !== "unknown_command") throw error;
      missing.push(`${method} (${errorMessage(error)})`);
    }
  }
  throw new RpcError(`The controller exposes no debugger source tool. Tried ${missing.join(" and ")}.`, "unknown_command");
}

async function refreshDebugger(options = {}) {
  const dbg = state.debugger;
  if (!state.token) {
    renderDebugger();
    return;
  }
  const target = targetById();
  if (!target || !state.caps.runtime) {
    renderDebugger();
    return;
  }
  if (dbg.statusLoading) return;
  if (options.poll && (dbg.busy || !dbg.attached || dbg.unavailable || dbg.status === "attaching")) return;
  const targetId = state.targetId;
  const request = dbg.request + 1;
  dbg.request = request;
  dbg.statusLoading = true;
  if (!options.quiet) renderDebuggerChrome();
  try {
    const data = await debuggerCall("get_ui_debugger", {}, { target: targetId });
    if (request !== dbg.request || targetId !== state.targetId) return;
    if (!isObjectValue(data)) throw new RpcError("The controller returned an empty debugger status.", "invalid_response");
    applyDebuggerSnapshot(data);
    dbg.loaded = true;
    dbg.lastRefresh = Date.now();
    if (dbg.attached) await ensureDebuggerSessionData({ quiet: true });
  } catch (error) {
    if (request !== dbg.request || targetId !== state.targetId) return;
    const message = errorMessage(error);
    dbg.phase = "";
    dbg.attached = false;
    dbg.reason = message;
    dbg.reasonCode = error.code || "";
    dbg.error = message;
    dbg.unavailable = ["unavailable", "unsupported", "not_supported", "unknown_command", "not_available"].includes(error?.code);
    dbg.status = dbg.unavailable ? "unavailable" : "error";
    if (["unavailable", "unknown_command", "unsupported", "not_supported", "not_available"].includes(error?.code)) dbg.adapterAvailable = false;
    dbg.pid = null;
    dbg.sessionId = "";
    clearDebuggerSessionData({ keepSource: false, keepBreakpoints: false });
    if (!options.quiet) toast(message, "error", 9000);
  } finally {
    if (request === dbg.request && targetId === state.targetId) {
      dbg.statusLoading = false;
      renderDebugger();
      syncDebuggerTimer();
    }
  }
}

async function ensureDebuggerSessionData(options = {}) {
  const dbg = state.debugger;
  if (!dbg.attached || dbg.unavailable || dbg.status !== "paused") return;
  if (!dbg.stackLoaded && !dbg.stackLoading) await loadDebuggerStack(dbg.threadId, options);
  if (dbg.status !== "paused") return;
  const frame = selectedDebuggerFrame();
  if (!frame) return;
  if (!dbg.sourceLoaded && !dbg.sourceLoading) await loadDebuggerSourceForFrame(frame, options);
}

function ensureDebugger() {
  if (!state.token || !state.targetId || !state.caps.runtime) {
    renderDebugger();
    return;
  }
  if (!state.debugger.loaded && !state.debugger.statusLoading) {
    refreshDebugger({ quiet: true });
    return;
  }
  renderDebugger();
}

async function loadDebuggerStack(threadId, options = {}) {
  const dbg = state.debugger;
  if (!state.token || !state.targetId || !state.caps.runtime || !dbg.attached) return;
  if (dbg.stackLoading) return;
  const targetId = state.targetId;
  const requested = threadId !== null && threadId !== undefined ? threadId : null;
  const request = dbg.stackRequest + 1;
  dbg.stackRequest = request;
  dbg.stackLoading = true;
  dbg.stackError = null;
  dbg.threadsError = null;
  dbg.renderSig.stack = "";
  dbg.renderSig.threads = "";
  renderDebuggerStack();
  renderDebuggerThreads();
  try {
    const data = await debuggerCall("get_ui_debug_stack", { thread_id: requested, start_frame: 0, levels: 100 }, { target: targetId });
    if (request !== dbg.stackRequest || targetId !== state.targetId) return;
    if (requested !== null && dbg.threadId !== null && dbg.threadId !== requested && !debuggerThreadList(data)?.some(thread => thread.id === requested)) return;
    if (debuggerHasData(data)) applyDebuggerSnapshot(data);
    const root = debuggerRoot(data);
    const frames = debuggerFrameList(data);
    if (frames) {
      dbg.frames = frames;
      dbg.stackLoaded = true;
      dbg.stackError = null;
      dbg.framesTotal = debuggerNumber(root.total_frames) ?? null;
      dbg.framesTruncated = root.truncated === true;
    } else if (!dbg.stackLoaded) {
      dbg.stackError = new RpcError("The stack response contained no frames.", "invalid_response");
    }
    if (isObjectValue(root.threads_error)) {
      dbg.threadsError = new RpcError(
        firstStringValue(root.threads_error.message) || "The adapter could not read threads.",
        firstStringValue(root.threads_error.code) || "adapter_error",
      );
    }
    if (isObjectValue(root.scopes_error)) {
      dbg.scopesError = new RpcError(
        firstStringValue(root.scopes_error.message) || "The adapter could not read scopes.",
        firstStringValue(root.scopes_error.code) || "adapter_error",
      );
      dbg.scopesLoaded = true;
    }
    if (root.scopes_truncated === true) dbg.scopesTruncated = true;
    resetDebuggerVariables();
    if (dbg.frameIndex === null && dbg.frames.length) {
      dbg.frameIndex = 0;
      dbg.frameId = dbg.frames[0].id;
    }
    const frame = selectedDebuggerFrame();
    if (frame) {
      await selectDebuggerFrame(frame, dbg.frameIndex, {
        skipStackRender: true,
        skipScopes: true,
        quiet: options.quiet !== false,
      });
      await openDebuggerFirstScope({ quiet: options.quiet !== false });
    } else {
      dbg.scopes = [];
      dbg.scopesLoaded = true;
      dbg.scopesError = null;
    }
  } catch (error) {
    if (request !== dbg.stackRequest || targetId !== state.targetId) return;
    dbg.stackError = error;
    appendDebuggerConsole("error", `Call stack: ${errorMessage(error)}`);
  } finally {
    if (request === dbg.stackRequest && targetId === state.targetId) {
      dbg.stackLoading = false;
      dbg.renderSig.stack = "";
      dbg.renderSig.threads = "";
      renderDebuggerStack();
      renderDebuggerThreads();
      renderDebuggerVariables();
      updateDebuggerButtons();
    }
  }
}

function debuggerFrameSourceMatches(frame) {
  const source = state.debugger.source;
  if (!source || !frame) return false;
  const frameSource = frame.source || {};
  if (source.frameId !== null && frame.id !== null && source.frameId === frame.id
    && (!source.path || !frameSource.path || debuggerPathsMatch(frameSource.path, source.path))) return true;
  if (frameSource.path && source.path && debuggerPathsMatch(frameSource.path, source.path)) return true;
  if (frameSource.sourceReference !== null && frameSource.sourceReference !== undefined
    && source.sourceReference !== null && source.sourceReference !== undefined
    && String(frameSource.sourceReference) === String(source.sourceReference)) return true;
  return false;
}

async function selectDebuggerFrame(frame, index, options = {}) {
  const dbg = state.debugger;
  if (!frame) return;
  const sameFrame = dbg.frameIndex === index;
  dbg.frameIndex = index;
  dbg.frameId = frame.id;
  if (!sameFrame && !options.skipScopes) {
    dbg.scopes = [];
    dbg.scopesLoaded = false;
    dbg.scopesError = null;
    dbg.scopesTruncated = false;
    resetDebuggerVariables();
  }
  dbg.renderSig.stack = "";
  dbg.renderSig.variables = "";
  if (!options.skipStackRender) renderDebuggerStack();
  updateDebuggerButtons();
  const tasks = [];
  const sourceMatches = debuggerFrameSourceMatches(frame);
  if (sourceMatches) {
    if (dbg.source) dbg.source.frameId = frame.id;
    dbg.sourceLoaded = true;
    dbg.renderSig.source = "";
    renderDebuggerSource(true);
  } else {
    tasks.push(loadDebuggerSourceForFrame(frame, { quiet: options.quiet !== false }));
  }
  if (options.skipScopes) {
    // get_ui_debug_stack already returned the top frame's scopes.
  } else if (!dbg.scopesLoaded && !dbg.scopesLoading && frame.id !== null) {
    tasks.push(loadDebuggerScopes(frame, { quiet: options.quiet !== false }));
  } else if (frame.id === null) {
    dbg.scopes = [];
    dbg.scopesLoaded = true;
    dbg.scopesError = null;
  }
  await Promise.all(tasks);
}

async function selectDebuggerThread(thread) {
  if (!thread || thread.id === null) return;
  const dbg = state.debugger;
  if (dbg.threadId === thread.id) return;
  dbg.threadId = thread.id;
  dbg.frames = [];
  dbg.stackLoaded = false;
  dbg.stackError = null;
  dbg.frameIndex = null;
  dbg.frameId = null;
  dbg.framesTotal = null;
  dbg.framesTruncated = false;
  dbg.scopes = [];
  dbg.scopesLoaded = false;
  dbg.scopesError = null;
  dbg.scopesTruncated = false;
  resetDebuggerVariables();
  dbg.renderSig = { ...dbg.renderSig, threads: "", stack: "", variables: "", source: "" };
  renderDebuggerThreads();
  renderDebuggerStack();
  renderDebuggerVariables();
  if (dbg.status === "paused") await loadDebuggerStack(thread.id, { quiet: true });
}

async function openDebuggerFirstScope(options = {}) {
  const dbg = state.debugger;
  if (!dbg.scopes.length || dbg.expandedScopes.size) return;
  const first = dbg.scopes[0];
  const key = debuggerScopeKey(0);
  if (!first.variables.length && first.reference === null) return;
  dbg.expandedScopes.add(key);
  dbg.renderSig.variables = "";
  if (!first.variables.length && first.reference !== null) {
    await loadDebuggerVariableChildren(key, first.reference, { quiet: options.quiet !== false });
  }
}

async function loadDebuggerScopes(frame, options = {}) {
  const dbg = state.debugger;
  if (!state.token || !state.targetId || !state.caps.runtime || !frame || frame.id === null || !dbg.attached) return;
  const targetId = state.targetId;
  const frameIndex = dbg.frameIndex;
  const request = dbg.scopesRequest + 1;
  dbg.scopesRequest = request;
  dbg.scopesLoading = true;
  dbg.scopesError = null;
  dbg.renderSig.variables = "";
  renderDebuggerVariables();
  try {
    const data = await debuggerCall("read_ui_debug_variables", { frame_id: frame.id }, { target: targetId });
    if (request !== dbg.scopesRequest || targetId !== state.targetId || dbg.frameIndex !== frameIndex) return;
    if (debuggerHasData(data)) applyDebuggerSnapshot(data);
    const scopes = debuggerScopeList(data);
    if (scopes) {
      dbg.scopes = scopes;
      dbg.scopesLoaded = true;
      dbg.scopesError = null;
    } else if (!dbg.scopesLoaded) {
      dbg.scopesError = new RpcError("The scopes response contained no scopes.", "invalid_response");
    }
    resetDebuggerVariables();
    await openDebuggerFirstScope({ quiet: options.quiet !== false });
  } catch (error) {
    if (request !== dbg.scopesRequest || targetId !== state.targetId || dbg.frameIndex !== frameIndex) return;
    dbg.scopesError = error;
    appendDebuggerConsole("error", `Scopes: ${errorMessage(error)}`);
  } finally {
    if (request === dbg.scopesRequest && targetId === state.targetId) {
      dbg.scopesLoading = false;
      dbg.renderSig.variables = "";
      renderDebuggerVariables();
      updateDebuggerButtons();
    }
  }
}

async function loadDebuggerVariableChildren(key, reference, options = {}) {
  const dbg = state.debugger;
  if (!state.token || !state.targetId || !state.caps.runtime || reference === null || !dbg.attached) return;
  if (dbg.variableLoading.has(key)) return;
  const targetId = state.targetId;
  const epoch = dbg.variableEpoch;
  const request = (dbg.variableRequests.get(key) || 0) + 1;
  dbg.variableRequests.set(key, request);
  dbg.variableLoading.add(key);
  dbg.variableErrors.delete(key);
  dbg.renderSig.variables = "";
  renderDebuggerVariables();
  try {
    const data = await debuggerCall("read_ui_debug_variables", { variables_reference: reference, count: 100 }, { target: targetId });
    if (request !== dbg.variableRequests.get(key) || targetId !== state.targetId || epoch !== dbg.variableEpoch) return;
    const variables = debuggerVariablesFrom(data);
    if (!variables) throw new RpcError("The variables response contained no variables list.", "invalid_response");
    dbg.variableChildren.set(key, variables);
    if (debuggerRoot(data).truncated === true) dbg.variableTruncated.add(key);
    else dbg.variableTruncated.delete(key);
  } catch (error) {
    if (request !== dbg.variableRequests.get(key) || targetId !== state.targetId || epoch !== dbg.variableEpoch) return;
    dbg.variableErrors.set(key, error);
    appendDebuggerConsole("error", `Variables: ${errorMessage(error)}`);
  } finally {
    if (request === dbg.variableRequests.get(key) && targetId === state.targetId && epoch === dbg.variableEpoch) {
      dbg.variableLoading.delete(key);
      dbg.renderSig.variables = "";
      renderDebuggerVariables();
    }
  }
}

async function loadDebuggerSourceForFrame(frame, options = {}) {
  const dbg = state.debugger;
  if (!frame) return;
  const frameSource = frame.source || {};
  const frameIndex = dbg.frameIndex;
  if (!frameSource.path && frameSource.sourceReference === null) {
    dbg.source = {
      path: "",
      name: "",
      sourceReference: null,
      lines: [],
      truncated: false,
      reason: "The adapter returned no source location for this frame.",
      unavailable: false,
      language: "",
      frameId: frame.id,
    };
    dbg.sourceLoaded = true;
    dbg.sourceError = null;
    dbg.renderSig.source = "";
    renderDebuggerSource(true);
    return;
  }
  const params = frameSource.path ? { path: frameSource.path } : { source_reference: frameSource.sourceReference };
  if (frame.line !== null) params.line = frame.line;
  await loadDebuggerSource(params, {
    frameId: frame.id,
    frameIndex,
    line: frame.line,
    column: frame.column,
    quiet: options.quiet !== false,
  });
}

async function loadDebuggerSource(params = {}, options = {}) {
  const dbg = state.debugger;
  if (!state.token || !state.targetId || !state.caps.runtime || !dbg.attached) return;
  const targetId = state.targetId;
  const request = dbg.sourceRequest + 1;
  dbg.sourceRequest = request;
  const frameIndex = options.frameIndex ?? null;
  dbg.sourceLoading = true;
  dbg.sourceError = null;
  dbg.renderSig.source = "";
  updateDebuggerSourceHead();
  renderDebuggerSource(true);
  try {
    const response = await debuggerSourceCall(params, { target: targetId });
    const data = response.data;
    if (request !== dbg.sourceRequest || targetId !== state.targetId) return;
    if (frameIndex !== null && dbg.frameIndex !== frameIndex) return;
    const root = debuggerRoot(data);
    if (root.breakpoints !== undefined || root.source?.breakpoints !== undefined) applyDebuggerSnapshot(data);
    const raw = root.source !== undefined ? root.source : data;
    const unavailable = root.available === false || root.unavailable === true || isObjectValue(root.unavailable);
    if (unavailable) {
      const info = debuggerUnavailableInfo(root) || { reason: "", code: "" };
      dbg.source = {
        path: firstStringValue(params.path, raw?.path),
        name: firstStringValue(params.path, raw?.name),
        sourceReference: params.source_reference ?? null,
        lines: [],
        truncated: false,
        reason: info.reason || "The adapter reported that this source is unavailable without a reason.",
        unavailable: true,
        language: "",
        frameId: options.frameId ?? null,
      };
    } else {
      const source = normalizeDebuggerSource(raw, {
        path: params.path,
        sourceReference: params.source_reference ?? null,
        line: options.line,
        column: options.column,
        frameId: options.frameId ?? null,
      });
      if (!source) throw new RpcError("The source response contained no source.", "invalid_response");
      if (!source.path && params.path) source.path = params.path;
      source.frameId = options.frameId ?? null;
      dbg.source = source;
    }
    dbg.sourceLoaded = true;
    dbg.sourceError = null;
  } catch (error) {
    if (request !== dbg.sourceRequest || targetId !== state.targetId) return;
    if (error?.code === "unknown_command") {
      const path = params.path || "";
      const line = options.line !== null && options.line !== undefined ? `:${options.line}` : "";
      dbg.source = {
        path,
        name: path,
        sourceReference: params.source_reference ?? null,
        lines: [],
        truncated: false,
        reason: `${errorMessage(error)}${path ? ` The selected frame is at ${path}${line}.` : ""} No source text is substituted.`,
        unavailable: false,
        language: "",
        frameId: options.frameId ?? null,
      };
      dbg.sourceLoaded = true;
      dbg.sourceError = null;
      appendDebuggerConsole("error", `Source: ${errorMessage(error)}`);
    } else {
      dbg.sourceError = error;
      appendDebuggerConsole("error", `Source: ${errorMessage(error)}`);
      if (options.quiet !== true) toast(errorMessage(error), "error", 9000);
    }
  } finally {
    if (request === dbg.sourceRequest && targetId === state.targetId) {
      dbg.sourceLoading = false;
      dbg.renderSig.source = "";
      renderDebuggerSource(true);
    }
  }
}

function debuggerBreakpointKey(breakpoint) {
  return `${debuggerPathKey(breakpoint.path)}:${breakpoint.line ?? ""}`;
}

function debuggerVisibleBreakpoints() {
  const dbg = state.debugger;
  const active = dbg.breakpoints.map(breakpoint => ({ ...breakpoint, enabled: true }));
  const disabled = [...dbg.disabledBreakpoints.values()].map(breakpoint => ({ ...breakpoint, enabled: false }));
  return sortDebuggerBreakpoints([...active, ...disabled]);
}

function applyDebuggerBreakpointList(data, path) {
  const dbg = state.debugger;
  const list = debuggerBreakpointList(data);
  if (!list) return false;
  const others = dbg.breakpoints.filter(breakpoint => !debuggerPathsMatch(breakpoint.path, path));
  dbg.breakpoints = sortDebuggerBreakpoints([...others, ...list]);
  for (const breakpoint of dbg.breakpoints) {
    dbg.disabledBreakpoints.delete(debuggerBreakpointKey(breakpoint));
  }
  return true;
}

async function setDebuggerBreakpoint(path, line, enabled, options = {}) {
  const dbg = state.debugger;
  if (!debuggerCanAct(true) || !path || line === null) return;
  const targetId = state.targetId;
  const location = `${path}:${line}`;
  const visible = debuggerBreakpointsAt(path, line);
  const existing = visible[0] || null;
  const condition = firstStringValue(options.condition, existing?.condition);
  const key = debuggerBreakpointKey({ path, line });
  invalidateDebuggerStatusRequest();
  dbg.busy = enabled ? "Setting breakpoint" : "Clearing breakpoint";
  renderDebuggerChrome();
  try {
    let data;
    if (enabled) {
      data = await debuggerCall("set_ui_breakpoint", {
        path,
        line,
        condition: condition || undefined,
      }, { target: targetId });
    } else {
      data = await debuggerCall("clear_ui_breakpoint", { path, line }, { target: targetId });
    }
    if (targetId !== state.targetId) return;
    if (!applyDebuggerBreakpointList(data, path)) {
      throw new RpcError("The controller returned no breakpoint list.", "invalid_response");
    }
    if (enabled) {
      dbg.disabledBreakpoints.delete(key);
    } else if (existing) {
      dbg.disabledBreakpoints.set(key, {
        path: existing.path || path,
        line,
        column: existing.column,
        resolvedLine: null,
        condition: existing.condition,
        label: "",
        enabled: false,
        verified: null,
        id: null,
        resolvedLine: null,
        message: "",
      });
    }
    appendDebuggerConsole("event", `${enabled ? "Set" : "Cleared"} breakpoint at ${location}.`);
  } catch (error) {
    if (targetId !== state.targetId) return;
    appendDebuggerConsole("error", `Breakpoint: ${errorMessage(error)}`);
    toast(errorMessage(error), "error", 9000);
  } finally {
    if (targetId === state.targetId) {
      dbg.busy = "";
      dbg.renderSig.breakpoints = "";
      dbg.renderSig.source = "";
      renderDebuggerChrome();
      renderDebuggerBreakpoints(true);
      renderDebuggerSource(true);
    }
  }
}

function toggleDebuggerBreakpointAtLine(path, line) {
  const existing = debuggerBreakpointsAt(path, line);
  const active = existing.find(breakpoint => breakpoint.enabled);
  const disabled = existing.find(breakpoint => !breakpoint.enabled);
  if (active) setDebuggerBreakpoint(path, line, false, { condition: active.condition });
  else setDebuggerBreakpoint(path, line, true, { condition: disabled?.condition });
}

function toggleDebuggerBreakpoint(breakpoint, enabled) {
  if (!breakpoint.path || breakpoint.line === null) {
    toast("The adapter returned this breakpoint without a source location.", "error", 8000);
    return;
  }
  setDebuggerBreakpoint(breakpoint.path, breakpoint.line, enabled, { condition: breakpoint.condition });
}

function invalidateDebuggerStatusRequest() {
  state.debugger.request += 1;
  state.debugger.statusLoading = false;
}

async function attachDebugger() {
  const dbg = state.debugger;
  if (!debuggerCanAct(false) || dbg.attached) return;
  const targetId = state.targetId;
  invalidateDebuggerStatusRequest();
  dbg.busy = "Attaching";
  dbg.error = null;
  appendDebuggerConsole("event", "Attach requested.");
  renderDebuggerChrome();
  try {
    const data = await debuggerCall("attach_ui_debugger", { timeout_ms: 8000 }, { target: targetId, timeout: 15000 });
    if (targetId !== state.targetId) return;
    invalidateDebuggerStatusRequest();
    if (debuggerHasData(data)) applyDebuggerSnapshot(data);
    appendDebuggerConsole("event", "Attach completed.");
    if (dbg.attached) {
      await ensureDebuggerSessionData({ quiet: true });
    } else {
      await refreshDebugger({ quiet: true });
    }
  } catch (error) {
    if (targetId !== state.targetId) return;
    const message = errorMessage(error);
    dbg.phase = "";
    dbg.attached = false;
    dbg.reason = message;
    dbg.reasonCode = error.code || "";
    dbg.error = message;
    dbg.unavailable = ["unavailable", "unsupported", "not_supported", "not_available"].includes(error?.code);
    dbg.status = dbg.unavailable ? "unavailable" : "error";
    if (error?.code === "unknown_command") dbg.adapterAvailable = false;
    dbg.pid = null;
    dbg.sessionId = "";
    clearDebuggerSessionData({ keepSource: false, keepBreakpoints: false });
    appendDebuggerConsole("error", message);
    toast(message, "error", 10000);
  } finally {
    if (targetId === state.targetId) {
      dbg.busy = "";
      renderDebugger();
      syncDebuggerTimer();
    }
  }
}

async function detachDebugger() {
  const dbg = state.debugger;
  if (!debuggerCanAct(false) || dbg.busy) return;
  if (!dbg.attached && dbg.status !== "attaching") return;
  const targetId = state.targetId;
  invalidateDebuggerStatusRequest();
  dbg.busy = "Detaching";
  renderDebuggerChrome();
  try {
    const data = await debuggerCall("detach_ui_debugger", {}, { target: targetId });
    if (targetId !== state.targetId) return;
    invalidateDebuggerStatusRequest();
    if (debuggerHasData(data)) applyDebuggerSnapshot(data);
    appendDebuggerConsole("event", "Detach completed.");
    await refreshDebugger({ quiet: true });
  } catch (error) {
    if (targetId !== state.targetId) return;
    appendDebuggerConsole("error", `Detach: ${errorMessage(error)}`);
    toast(errorMessage(error), "error", 10000);
  } finally {
    if (targetId === state.targetId) {
      dbg.busy = "";
      renderDebugger();
      syncDebuggerTimer();
    }
  }
}

async function runDebuggerControl(action, label) {
  const dbg = state.debugger;
  if (!dbg.attached || dbg.busy) return;
  if (action !== "pause" && dbg.status !== "paused") return;
  if (action === "pause" && dbg.status === "paused") return;
  const targetId = state.targetId;
  invalidateDebuggerStatusRequest();
  dbg.busy = label;
  renderDebuggerChrome();
  try {
    const wireAction = { continue: "resume", step_into: "step_in" }[action] || action;
    const data = await debuggerCall("control_ui_debugger", { action: wireAction }, { target: targetId });
    if (targetId !== state.targetId) return;
    invalidateDebuggerStatusRequest();
    if (debuggerHasData(data)) applyDebuggerSnapshot(data);
    if (!debuggerFrameList(data)) {
      dbg.frames = [];
      dbg.stackLoaded = false;
      dbg.stackError = null;
      dbg.frameIndex = null;
      dbg.frameId = null;
      dbg.scopes = [];
      dbg.scopesLoaded = false;
      dbg.scopesError = null;
      resetDebuggerVariables();
      dbg.sourceLoaded = false;
    }
    appendDebuggerConsole("event", `${label} completed.`);
    await refreshDebugger({ quiet: true });
    if (state.debugger.status === "paused") await ensureDebuggerSessionData({ quiet: true });
  } catch (error) {
    if (targetId !== state.targetId) return;
    appendDebuggerConsole("error", `${label}: ${errorMessage(error)}`);
    toast(errorMessage(error), "error", 10000);
  } finally {
    if (targetId === state.targetId) {
      dbg.busy = "";
      renderDebugger();
      updateDebuggerButtons();
      syncDebuggerTimer();
    }
  }
}

function debuggerCanAct(requireAttached) {
  if (!state.connected || !state.token || !state.targetId || !state.caps.runtime) return false;
  const dbg = state.debugger;
  if (requireAttached) return dbg.attached && !dbg.busy;
  return dbg.adapterAvailable !== false;
}

function debuggerStatusLabel(status) {
  return {
    idle: "Not attached",
    unknown: "Unknown",
    unavailable: "Unavailable",
    error: "Error",
    detached: "Detached",
    attaching: "Attaching",
    attached: "Attached",
    running: "Running",
    paused: "Paused",
    terminated: "Terminated",
  }[status] || "Unknown";
}

function debuggerStatusTone(status) {
  if (status === "paused") return "paused";
  if (status === "attached" || status === "running") return "complete";
  if (status === "attaching") return "pending";
  if (status === "unavailable" || status === "error") return "failed";
  if (status === "detached" || status === "terminated" || status === "idle") return "cancelled";
  return "";
}

function updateDebuggerButtons() {
  const dbg = state.debugger;
  const targetOk = Boolean(state.connected && state.token && state.targetId && state.caps.runtime);
  const busy = Boolean(dbg.busy);
  const attaching = dbg.status === "attaching";
  const attached = dbg.attached;
  const paused = dbg.status === "paused";
  dom.debuggerAttach.disabled = !targetOk || dbg.adapterAvailable === false || attached || attaching || busy;
  dom.debuggerDetach.disabled = !targetOk || (!attached && !attaching) || busy;
  dom.debuggerPause.disabled = !targetOk || !attached || paused || busy;
  dom.debuggerContinue.disabled = !targetOk || !paused || busy;
  dom.debuggerStepOver.disabled = !targetOk || !paused || busy;
  dom.debuggerStepInto.disabled = !targetOk || !paused || busy;
  dom.debuggerStepOut.disabled = !targetOk || !paused || busy;
  dom.debuggerOpen.disabled = !targetOk || !attached || busy;
  dom.debuggerOpenPath.disabled = !targetOk || !attached || busy;
  dom.debuggerVariablesRefresh.disabled = !targetOk || !paused || dbg.frameId === null || dbg.scopesLoading || busy;
  dom.debuggerEvaluate.disabled = !targetOk || !paused || dbg.evaluateBusy || busy;
  dom.debuggerExpression.disabled = !targetOk || !paused || dbg.evaluateBusy || busy;
  dom.debuggerConsoleClear.disabled = dbg.console.length === 0;
  dom.debuggerRefresh.disabled = !targetOk || dbg.statusLoading;
  dom.debuggerAttach.textContent = dbg.status === "error" || dbg.status === "unavailable" ? "Retry attach" : "Attach";
  if (!targetOk) {
    dom.debuggerExpression.placeholder = "Select a runtime target";
  } else if (!paused) {
    dom.debuggerExpression.placeholder = "Pause the process to evaluate";
  } else {
    dom.debuggerExpression.placeholder = "Evaluate in the selected frame";
  }
}

function renderDebuggerChrome() {
  const dbg = state.debugger;
  const label = debuggerStatusLabel(dbg.status);
  dom.debuggerStatus.textContent = label;
  dom.debuggerStatus.className = `state-badge ${debuggerStatusTone(dbg.status)}`.trim();
  const stopTitle = dbg.stop?.reason ? ` · ${dbg.stop.reason}` : "";
  dom.debuggerStatus.title = `${dbg.phase ? `Adapter phase: ${dbg.phase}` : ""}${stopTitle}`.trim();
  dom.debuggerBadge.hidden = dbg.status !== "paused";
  dom.debuggerBadge.title = dbg.status === "paused" ? "Process paused" : "";

  const failed = dbg.status === "unavailable" || dbg.status === "error";
  const warning = !failed && !dbg.attached && Boolean(dbg.warning);
  const showBanner = failed || warning;
  dom.debuggerUnavailable.hidden = !showBanner;
  dom.debuggerUnavailable.classList.toggle("is-warning", warning);
  if (showBanner) {
    dom.debuggerUnavailableTitle.textContent = failed
      ? (dbg.status === "unavailable" ? "Debugger unavailable" : "Debugger error")
      : "Attach may be denied";
    const missing = dbg.status === "unavailable"
      ? "The controller reported the debugger as unavailable without a reason."
      : "The debugger request failed without a message.";
    let reason = failed ? (dbg.reason || missing) : dbg.warning;
    if (failed && dbg.reasonCode && !reason.includes(dbg.reasonCode)) reason = `${reason} (${dbg.reasonCode})`;
    if (dom.debuggerUnavailableReason.textContent !== reason) dom.debuggerUnavailableReason.textContent = reason;
    const adapter = failed ? debuggerAdapterLabel(dbg.adapter) : "";
    dom.debuggerAdapter.hidden = !adapter;
    const adapterLine = adapter ? `Adapter: ${adapter}` : "";
    if (dom.debuggerAdapter.textContent !== adapterLine) dom.debuggerAdapter.textContent = adapterLine;
    dom.debuggerAdapter.title = dbg.adapter?.version || "";
  }

  const adapter = debuggerAdapterLabel(dbg.adapter);
  dom.debuggerMeta.textContent = adapter
    ? `Adapter ${dbg.adapter?.name || adapter}. Sessions attach to one process.`
    : "Debug adapter sessions attach to one process at a time.";
  dom.debuggerMeta.title = adapter;
  const session = [];
  if (dbg.attached) {
    if (dbg.pid !== null) session.push(`PID ${dbg.pid}`);
    if (dbg.adapter?.name) session.push(dbg.adapter.name);
    if (dbg.sessionId) session.push(`session ${dbg.sessionId}`);
  }
  dom.debuggerSession.textContent = dbg.attached
    ? session.length ? session.join(" · ") : "Attached session"
    : "No session";
  dom.debuggerSession.title = dom.debuggerSession.textContent;
  dom.debuggerBusy.hidden = !dbg.busy;
  dom.debuggerBusyText.textContent = dbg.busy || "Working";
  updateDebuggerButtons();
}

function debuggerContextMessage(what) {
  if (!state.token) return { text: "Connect to a controller." };
  const target = targetById();
  if (!target) return { text: "Select a target." };
  if (!state.caps.runtime) return { text: "Select a Cove runtime; debugger sessions attach to a process." };
  const dbg = state.debugger;
  if (dbg.status === "unavailable") {
    return {
      text: dbg.adapterAvailable === false
        ? "The debugger is unavailable for this target. The reason is shown above."
        : "The last attach was refused. The adapter's reason is shown above.",
    };
  }
  if (dbg.status === "error") return { text: "The debugger reported an error. See the message above." };
  if (dbg.status === "attaching") return { text: "Attaching…", loading: true };
  if (!dbg.attached) return { text: `Attach to read ${what}.` };
  return null;
}

function appendDebuggerPaneMessage(host, message, options = {}) {
  const node = el("div", { class: `dbg-empty ${options.error ? "is-error" : ""}`.trim() });
  if (options.loading) {
    node.append(el("span", { class: "spinner", attrs: { role: "progressbar", "aria-label": options.loadingLabel || "Loading" } }), " ", message);
  } else {
    node.append(message);
  }
  host.append(node);
  if (options.retry) {
    host.append(el("div", { class: "dbg-empty" },
      el("button", {
        class: "text-button",
        props: { type: "button" },
        text: options.retryLabel || "Retry",
        on: { click: options.retry },
      })));
  }
}

function renderDebuggerThreads() {
  const dbg = state.debugger;
  const signature = JSON.stringify({
    threads: dbg.threads,
    threadId: dbg.threadId,
    status: dbg.status,
    loading: dbg.stackLoading,
    error: dbg.threadsError ? errorMessage(dbg.threadsError) : "",
  });
  if (signature === dbg.renderSig.threads) {
    updateDebuggerButtons();
    return;
  }
  dbg.renderSig.threads = signature;
  const host = dom.debuggerThreads;
  clear(host);
  dom.debuggerThreadsMeta.textContent = dbg.threads.length ? String(dbg.threads.length) : "";
  const context = debuggerContextMessage("threads");
  if (context && !dbg.threads.length) {
    appendDebuggerPaneMessage(host, context.text, { loading: context.loading, retry: (dbg.status === "detached" || dbg.status === "unknown") && debuggerCanAct(false) ? () => attachDebugger() : null, retryLabel: "Attach" });
    return;
  }
  if (dbg.stackLoading && dbg.status === "paused" && !dbg.threads.length) {
    appendDebuggerPaneMessage(host, "Reading threads…", { loading: true, loadingLabel: "Reading threads" });
    return;
  }
  if (dbg.threadsError && !dbg.threads.length) {
    appendDebuggerPaneMessage(host, errorMessage(dbg.threadsError), { error: true, retry: dbg.attached ? () => loadDebuggerStack(dbg.threadId) : null, retryLabel: "Retry" });
    return;
  }
  if (!dbg.threads.length) {
    host.append(el("div", { class: "dbg-empty", text: dbg.status === "paused" ? "No threads were reported." : "Pause the process to read threads." }));
    return;
  }
  for (const thread of dbg.threads) {
    const selected = thread.id !== null && thread.id === dbg.threadId;
    const row = el("button", {
      class: `dbg-row ${selected ? "is-selected" : ""} ${thread.current ? "is-current" : ""}`.trim(),
      attrs: {
        type: "button",
        "aria-pressed": String(selected),
        title: thread.id === null ? "The adapter returned no thread ID." : thread.name,
      },
      props: { disabled: thread.id === null },
    },
    el("span", { class: "dbg-row-mark", attrs: { "aria-hidden": "true" } }),
    el("span", { class: "dbg-row-main" },
      el("span", { class: "dbg-row-name", text: thread.name }),
      thread.state ? el("span", { class: "dbg-row-meta", text: thread.state }) : null));
    row.addEventListener("click", () => selectDebuggerThread(thread));
    host.append(row);
  }
}

function renderDebuggerStack() {
  const dbg = state.debugger;
  const signature = JSON.stringify({
    frames: dbg.frames,
    frameIndex: dbg.frameIndex,
    status: dbg.status,
    loading: dbg.stackLoading,
    error: dbg.stackError ? errorMessage(dbg.stackError) : "",
    threadId: dbg.threadId,
    stop: dbg.stop,
    total: dbg.framesTotal,
    truncated: dbg.framesTruncated,
  });
  if (signature === dbg.renderSig.stack) {
    updateDebuggerButtons();
    return;
  }
  dbg.renderSig.stack = signature;
  const host = dom.debuggerStack;
  clear(host);
  const stackMeta = [];
  if (dbg.frames.length) stackMeta.push(`${dbg.frames.length} frame${dbg.frames.length === 1 ? "" : "s"}`);
  if (dbg.framesTruncated) stackMeta.push(dbg.framesTotal ? `of ${dbg.framesTotal}` : "truncated");
  if (dbg.stop?.reason) stackMeta.push(dbg.stop.reason);
  dom.debuggerStackMeta.textContent = stackMeta.join(" · ");
  dom.debuggerStackMeta.title = dbg.stop?.description || dbg.stop?.text || "";
  if (dbg.status === "paused" && dbg.threadId === null && dbg.threads.length > 1) {
    appendDebuggerPaneMessage(host, "Select a thread.");
    return;
  }
  const context = debuggerContextMessage("the call stack");
  if (context && !dbg.frames.length) {
    appendDebuggerPaneMessage(host, context.text, { loading: context.loading });
    return;
  }
  if (dbg.status !== "paused" && !dbg.frames.length) {
    appendDebuggerPaneMessage(host, "Pause the process to read the call stack.");
    return;
  }
  if (dbg.stackLoading && !dbg.frames.length) {
    appendDebuggerPaneMessage(host, "Reading the call stack…", { loading: true, loadingLabel: "Reading the call stack" });
    return;
  }
  if (dbg.stackError && !dbg.frames.length) {
    appendDebuggerPaneMessage(host, errorMessage(dbg.stackError), { error: true, retry: dbg.attached ? () => loadDebuggerStack(dbg.threadId) : null, retryLabel: "Retry" });
    return;
  }
  if (!dbg.frames.length) {
    appendDebuggerPaneMessage(host, "The adapter returned no frames.");
    return;
  }
  dbg.frames.forEach((frame, index) => {
    const selected = index === dbg.frameIndex;
    const location = debuggerFrameLocation(frame);
    const row = el("button", {
      class: `dbg-row ${selected ? "is-selected is-current" : ""}`.trim(),
      attrs: {
        type: "button",
        "aria-pressed": String(selected),
        title: frame.source?.path ? `${frame.source.path}${frame.line !== null ? `:${frame.line}` : ""}` : debuggerFrameLabel(frame),
      },
    },
    el("span", { class: "dbg-row-mark", attrs: { "aria-hidden": "true" } }),
    el("span", { class: "dbg-row-main" },
      el("span", { class: "dbg-row-name", text: debuggerFrameLabel(frame) }),
      location || frame.module ? el("span", { class: "dbg-row-meta", text: [location, frame.module].filter(Boolean).join(" · ") }) : null));
    row.addEventListener("click", () => selectDebuggerFrame(frame, index, { quiet: true }));
    host.append(row);
  });
  if (dbg.framesTruncated) {
    host.append(el("div", { class: "dbg-empty", text: dbg.framesTotal ? `The adapter returned ${dbg.frames.length} of ${dbg.framesTotal} frames.` : "The adapter truncated this stack." }));
  }
}

function renderDebuggerBreakpoints(force = false) {
  const dbg = state.debugger;
  const visible = debuggerVisibleBreakpoints();
  const hitIds = new Set(dbg.stop?.hitBreakpointIds || []);
  const disabledCount = visible.filter(breakpoint => !breakpoint.enabled).length;
  const signature = JSON.stringify({
    breakpoints: visible,
    status: dbg.status,
    attached: dbg.attached,
    busy: dbg.busy,
    hit: [...hitIds],
  });
  if (!force && signature === dbg.renderSig.breakpoints) return;
  dbg.renderSig.breakpoints = signature;
  const host = dom.debuggerBreakpoints;
  clear(host);
  dom.debuggerBreakpointsMeta.textContent = visible.length
    ? `${visible.length}${disabledCount ? ` · ${disabledCount} disabled` : ""}`
    : "";
  const context = debuggerContextMessage("breakpoints");
  if (context && !visible.length) {
    appendDebuggerPaneMessage(host, context.text, { loading: context.loading });
    return;
  }
  if (!visible.length) {
    host.append(el("div", { class: "dbg-empty", text: dbg.status === "paused" ? "No breakpoints. Select a source line gutter to set one." : "No breakpoints." }));
    return;
  }
  for (const breakpoint of visible) {
    const label = debuggerBreakpointLabel(breakpoint);
    const canToggle = dbg.attached && !dbg.busy && Boolean(breakpoint.path) && breakpoint.line !== null;
    const hit = breakpoint.id !== null && hitIds.has(breakpoint.id);
    const meta = [];
    if (!breakpoint.enabled) meta.push("disabled");
    if (breakpoint.verified === false) meta.push("unverified");
    if (breakpoint.message) meta.push(breakpoint.message);
    if (breakpoint.resolvedLine !== null && breakpoint.resolvedLine !== undefined && breakpoint.resolvedLine !== breakpoint.line) {
      meta.push(`resolved ${breakpoint.resolvedLine}`);
    }
    if (breakpoint.condition) meta.push(`if ${breakpoint.condition}`);
    if (hit) meta.push("hit");
    const row = el("label", {
      class: `dbg-bp-row ${breakpoint.enabled ? "" : "is-disabled"} ${hit ? "is-hit" : ""}`.trim(),
      attrs: breakpoint.enabled ? null : { title: "Disabled breakpoints are cleared in lldb-dap and restored when enabled." },
    },
    el("input", {
      attrs: {
        type: "checkbox",
        "aria-label": `${breakpoint.enabled ? "Disable" : "Enable"} breakpoint at ${label}`,
      },
      props: { checked: breakpoint.enabled, disabled: !canToggle },
    }),
    el("span", { class: "dbg-bp-main" },
      el("span", { class: "dbg-bp-label", text: label }),
      meta.length ? el("span", { class: "dbg-bp-meta", text: meta.join(" · ") }) : null));
    const input = row.querySelector("input");
    input.addEventListener("change", () => toggleDebuggerBreakpoint(breakpoint, input.checked));
    host.append(row);
  }
}

function debuggerSourceSignature() {
  const dbg = state.debugger;
  const frame = selectedDebuggerFrame();
  return JSON.stringify({
    source: dbg.source,
    status: dbg.status,
    frameIndex: dbg.frameIndex,
    frameId: dbg.frameId,
    line: frame?.line ?? null,
    column: frame?.column ?? null,
    framePath: frame?.source?.path || "",
    loading: dbg.sourceLoading,
    error: dbg.sourceError ? errorMessage(dbg.sourceError) : "",
    breakpoints: debuggerVisibleBreakpoints(),
    attached: dbg.attached,
    busy: dbg.busy,
  });
}

function updateDebuggerSourceHead() {
  const dbg = state.debugger;
  const source = dbg.source;
  const current = debuggerCurrentLocation();
  const path = source?.path || source?.name || "No file";
  dom.debuggerSourcePath.textContent = path;
  dom.debuggerSourcePath.title = path;
  const meta = [];
  if (source?.lines?.length) meta.push(`${source.lines.length} lines`);
  if (source?.truncated) meta.push("truncated");
  if (source?.language) meta.push(source.language);
  if (current) meta.push(`${current.line}${current.column !== null ? `:${current.column}` : ""}`);
  if (dbg.sourceLoading) meta.push("loading");
  if (dbg.sourceError) meta.push("error");
  dom.debuggerSourceMeta.textContent = meta.join(" · ");
}

function renderDebuggerSource(force = false) {
  const dbg = state.debugger;
  const signature = debuggerSourceSignature();
  if (!force && signature === dbg.renderSig.source) {
    updateDebuggerSourceHead();
    return;
  }
  dbg.renderSig.source = signature;
  const host = dom.debuggerSource;
  const active = document.activeElement;
  const focusedLine = active && host.contains(active) && active.classList?.contains("dbg-gutter") ? active.dataset.line : null;
  const scrollTop = host.scrollTop;
  const scrollLeft = host.scrollLeft;
  clear(host);
  updateDebuggerSourceHead();
  const source = dbg.source;
  if (dbg.sourceError) host.append(el("div", { class: "dbg-empty is-error", text: errorMessage(dbg.sourceError) }));
  if (!source) {
    const context = debuggerContextMessage("source");
    showState(host, {
      compact: true,
      icon: "logs",
      title: dbg.attached ? "No source selected" : "Source",
      message: context?.text || "Select a frame or open a source path.",
    });
    return;
  }
  if (!source.lines.length) {
    const reason = source.reason
      || (source.unavailable ? "The adapter reported that this source is unavailable without a reason." : "")
      || (dbg.sourceLoading ? "Waiting for the adapter's source response." : "The adapter returned no source text for this location.");
    showState(host, {
      compact: true,
      kind: source.unavailable ? "error" : dbg.sourceLoading ? "loading" : "empty",
      icon: "logs",
      title: source.unavailable ? "Source unavailable" : "Source text unavailable",
      message: reason,
    });
    return;
  }
  if (source.truncated) {
    host.append(el("div", { class: "dbg-empty", text: `Showing the first ${MAX_DEBUGGER_SOURCE_LINES} lines returned by the adapter.` }));
  }
  const current = debuggerCurrentLocation();
  const canToggle = dbg.attached && !dbg.busy && Boolean(source.path);
  let currentRow = null;
  for (const line of source.lines) {
    const breakpoints = debuggerBreakpointsAt(source.path, line.number);
    const enabledBreakpoint = breakpoints.some(breakpoint => breakpoint.enabled);
    const isCurrent = current !== null && current.line === line.number;
    const row = el("div", {
      class: `dbg-line ${isCurrent ? "is-current" : ""} ${breakpoints.length ? "has-breakpoint" : ""}`.trim(),
      attrs: isCurrent ? { "aria-current": "location" } : null,
      dataset: { line: String(line.number) },
    });
    const action = breakpoints.length
      ? enabledBreakpoint ? "Disable" : "Enable"
      : "Set";
    const gutter = el("button", {
      class: "dbg-gutter",
      attrs: {
        type: "button",
        "aria-pressed": String(enabledBreakpoint),
        "aria-label": `${action} breakpoint at line ${line.number}`,
        title: `${action} breakpoint at line ${line.number}`,
        "data-line": String(line.number),
      },
      props: { disabled: !canToggle },
    });
    gutter.addEventListener("click", () => toggleDebuggerBreakpointAtLine(source.path, line.number));
    row.append(
      gutter,
      el("span", { class: "dbg-line-number", text: String(line.number) }),
      el("span", { class: "dbg-line-code", text: line.text }),
    );
    if (isCurrent) currentRow = row;
    host.append(row);
  }
  if (currentRow) {
    host.scrollTop = Math.max(0, currentRow.offsetTop - host.offsetTop - host.clientHeight / 2);
  } else {
    host.scrollTop = scrollTop;
  }
  host.scrollLeft = scrollLeft;
  if (focusedLine) {
    host.querySelector(`.dbg-gutter[data-line="${CSS.escape(focusedLine)}"]`)?.focus();
  }
}

function debuggerScopeKey(index) {
  return `scope:${index}`;
}

function debuggerVariableKey(parentKey, index) {
  return `${parentKey}/${index}`;
}

function debuggerVariablesSignature() {
  const dbg = state.debugger;
  return JSON.stringify({
    status: dbg.status,
    frameIndex: dbg.frameIndex,
    frameId: dbg.frameId,
    scopes: dbg.scopes,
    loaded: dbg.scopesLoaded,
    loading: dbg.scopesLoading,
    error: dbg.scopesError ? errorMessage(dbg.scopesError) : "",
    expandedScopes: [...dbg.expandedScopes],
    expandedVariables: [...dbg.expandedVariables],
    variableLoading: [...dbg.variableLoading],
    variableErrors: [...dbg.variableErrors].map(([key, error]) => [key, errorMessage(error)]),
    children: [...dbg.variableChildren],
  });
}

function renderDebuggerVariables(force = false) {
  const dbg = state.debugger;
  const signature = debuggerVariablesSignature();
  if (!force && signature === dbg.renderSig.variables) {
    updateDebuggerButtons();
    return;
  }
  dbg.renderSig.variables = signature;
  const host = dom.debuggerVariables;
  clear(host);
  dom.debuggerScopesMeta.textContent = dbg.scopes.length
    ? `${dbg.scopes.length}${dbg.scopesTruncated ? " · truncated" : ""}`
    : "";
  if (dbg.scopesError) host.append(el("div", { class: "dbg-empty is-error", text: errorMessage(dbg.scopesError) }));
  if (dbg.status !== "paused") {
    const context = debuggerContextMessage("scopes");
    appendDebuggerPaneMessage(host, context?.text || "Pause the process to read scopes.");
    return;
  }
  if (dbg.frameIndex === null) {
    appendDebuggerPaneMessage(host, "Select a frame to read scopes.");
    return;
  }
  if (dbg.frameId === null) {
    appendDebuggerPaneMessage(host, "The selected frame has no ID; scopes cannot be requested.");
    return;
  }
  if (dbg.scopesLoading && !dbg.scopes.length) {
    appendDebuggerPaneMessage(host, "Reading scopes…", { loading: true, loadingLabel: "Reading scopes" });
    return;
  }
  if (!dbg.scopes.length) {
    if (dbg.scopesLoaded) appendDebuggerPaneMessage(host, "The adapter returned no scopes for this frame.");
    else appendDebuggerPaneMessage(host, "No scopes loaded.", { loading: false });
    return;
  }
  dbg.scopes.forEach((scope, index) => {
    const key = debuggerScopeKey(index);
    const expanded = dbg.expandedScopes.has(key);
    const canExpand = scope.variables.length > 0 || scope.reference !== null;
    const counts = [];
    if (scope.namedVariables !== null) counts.push(`${scope.namedVariables} named`);
    if (scope.indexedVariables !== null) counts.push(`${scope.indexedVariables} indexed`);
    if (scope.expensive) counts.push("expensive");
    const row = el("div", { class: "dbg-var-row is-scope" },
      canExpand
        ? el("button", {
          class: "dbg-var-twisty",
          attrs: {
            type: "button",
            "aria-expanded": String(expanded),
            "aria-label": `${expanded ? "Collapse" : "Expand"} scope ${scope.name}`,
          },
          on: { click: () => toggleDebuggerScope(index) },
        }, icon("chevron"))
        : el("span", { class: "dbg-var-twisty empty", attrs: { "aria-hidden": "true" } }, icon("chevron")),
      el("span", { class: "dbg-var-body" },
        el("span", { class: "dbg-var-name", text: scope.name }),
        counts.length ? el("span", { class: "dbg-var-type", text: counts.join(" · ") }) : null));
    host.append(row);
    if (expanded) {
      const children = scope.variables.length ? scope.variables : dbg.variableChildren.get(key) || [];
      const box = el("div", { class: "dbg-var-children" });
      if (children.length) {
        renderDebuggerVariableRows(box, children, key, index);
      } else if (dbg.variableLoading.has(key)) {
        box.append(el("div", { class: "dbg-var-loading", text: "Reading variables…" }));
      } else if (dbg.variableErrors.has(key)) {
        box.append(el("div", { class: "dbg-empty is-error", text: errorMessage(dbg.variableErrors.get(key)) }));
      } else if (scope.reference === null) {
        box.append(el("div", { class: "dbg-var-loading", text: "No variables were returned for this scope." }));
      } else {
        box.append(el("div", { class: "dbg-var-loading", text: "No variables returned." }));
      }
      if (dbg.variableTruncated.has(key) || (scope.truncated && children.length)) {
        box.append(el("div", { class: "dbg-var-loading", text: "The adapter returned more variables than this page." }));
      }
      host.append(box);
    }
  });
}

function renderDebuggerVariableRows(host, variables, parentKey, scopeIndex) {
  const dbg = state.debugger;
  variables.forEach((variable, index) => {
    const key = debuggerVariableKey(parentKey, index);
    const expanded = dbg.expandedVariables.has(key);
    const children = variable.children.length ? variable.children : dbg.variableChildren.get(key) || [];
    const canExpand = children.length > 0 || variable.reference !== null;
    const meta = [];
    if (variable.type) meta.push(variable.type);
    if (variable.namedVariables !== null) meta.push(`${variable.namedVariables} named`);
    if (variable.indexedVariables !== null) meta.push(`${variable.indexedVariables} indexed`);
    const row = el("div", { class: "dbg-var-row" },
      canExpand
        ? el("button", {
          class: "dbg-var-twisty",
          attrs: {
            type: "button",
            "aria-expanded": String(expanded),
            "aria-label": `${expanded ? "Collapse" : "Expand"} ${variable.name}`,
          },
          on: { click: () => toggleDebuggerVariable(key, variable, scopeIndex) },
        }, icon("chevron"))
        : el("span", { class: "dbg-var-twisty empty", attrs: { "aria-hidden": "true" } }, icon("chevron")),
      el("span", { class: "dbg-var-body" },
        el("span", { class: "dbg-var-name", text: variable.name }),
        el("span", {
          class: "dbg-var-value",
          text: variable.value !== ""
            ? `${variable.value}${variable.valueTruncated ? "…" : ""}`
            : variable.reference !== null
              ? "…"
              : "—",
        }),
        meta.length ? el("span", { class: "dbg-var-type", text: meta.join(" · ") }) : null));
    host.append(row);
    if (expanded) {
      const box = el("div", { class: "dbg-var-children" });
      if (children.length) {
        renderDebuggerVariableRows(box, children, key, scopeIndex);
      } else if (dbg.variableLoading.has(key)) {
        box.append(el("div", { class: "dbg-var-loading", text: "Reading variables…" }));
      } else if (dbg.variableErrors.has(key)) {
        box.append(el("div", { class: "dbg-empty is-error", text: errorMessage(dbg.variableErrors.get(key)) }));
      } else if (variable.reference === null) {
        box.append(el("div", { class: "dbg-var-loading", text: "No variables returned." }));
      } else {
        box.append(el("div", { class: "dbg-var-loading", text: "No variables returned." }));
      }
      if (dbg.variableTruncated.has(key)) {
        box.append(el("div", { class: "dbg-var-loading", text: "The adapter returned more variables than this page." }));
      }
      host.append(box);
    }
  });
}

function toggleDebuggerScope(index) {
  const dbg = state.debugger;
  const key = debuggerScopeKey(index);
  if (dbg.expandedScopes.has(key)) {
    dbg.expandedScopes.delete(key);
  } else {
    dbg.expandedScopes.add(key);
    const scope = dbg.scopes[index];
    if (scope && !scope.variables.length && scope.reference !== null && !dbg.variableChildren.has(key)) {
      loadDebuggerVariableChildren(key, scope.reference, { quiet: true });
    }
  }
  dbg.renderSig.variables = "";
  renderDebuggerVariables();
}

function toggleDebuggerVariable(key, variable, scopeIndex) {
  const dbg = state.debugger;
  if (dbg.expandedVariables.has(key)) {
    dbg.expandedVariables.delete(key);
  } else {
    dbg.expandedVariables.add(key);
    if (!variable.children.length && variable.reference !== null && !dbg.variableChildren.has(key)) {
      loadDebuggerVariableChildren(key, variable.reference, { quiet: true });
    }
  }
  dbg.renderSig.variables = "";
  renderDebuggerVariables();
}

function debuggerConsoleEntryElement(entry) {
  return el("div", { class: `dbg-console-entry is-${entry.kind}` },
    el("span", { class: "dbg-console-kind", text: { command: "Eval", result: "Result", error: "Error", event: "Event" }[entry.kind] || "Info" }),
    el("span", { class: "dbg-console-text" },
      entry.text,
      entry.type ? el("span", { class: "dbg-console-type", text: ` ${entry.type}` }) : null));
}

function appendDebuggerConsole(kind, text, options = {}) {
  const dbg = state.debugger;
  const entry = { kind, text: String(text ?? ""), type: options.type || "" };
  dbg.console.push(entry);
  if (dbg.console.length > DEBUGGER_CONSOLE_LIMIT) dbg.console.splice(0, dbg.console.length - DEBUGGER_CONSOLE_LIMIT);
  const host = dom.debuggerConsole;
  host.querySelector(".dbg-empty")?.remove();
  const nearBottom = host.scrollHeight - host.scrollTop - host.clientHeight < 24;
  host.append(debuggerConsoleEntryElement(entry));
  while (host.children.length > DEBUGGER_CONSOLE_LIMIT) host.firstElementChild.remove();
  if (nearBottom) host.scrollTop = host.scrollHeight;
  dom.debuggerConsoleMeta.textContent = dbg.console.length ? `${dbg.console.length} entries` : "";
  updateDebuggerButtons();
}

function renderDebuggerConsole() {
  const dbg = state.debugger;
  const host = dom.debuggerConsole;
  clear(host);
  if (!dbg.console.length) {
    host.append(el("div", { class: "dbg-empty", text: "Evaluate an expression while the process is paused." }));
  } else {
    for (const entry of dbg.console) host.append(debuggerConsoleEntryElement(entry));
    host.scrollTop = host.scrollHeight;
  }
  dom.debuggerConsoleMeta.textContent = dbg.console.length ? `${dbg.console.length} entries` : "";
  updateDebuggerButtons();
}

function clearDebuggerConsole() {
  state.debugger.console = [];
  renderDebuggerConsole();
}

function debuggerEvaluation(data) {
  if (data === null || data === undefined) return { text: "(no value returned)", type: "" };
  if (!isObjectValue(data) && !Array.isArray(data)) return { text: debuggerScalarText(data), type: "" };
  const root = isObjectValue(data.result) ? data.result : data;
  if (isObjectValue(root)) {
    const value = root.value !== undefined
      ? root.value
      : root.result !== undefined
        ? root.result
        : root.display !== undefined
          ? root.display
          : root.text;
    const truncated = root.result_truncated === true || data.result_truncated === true;
    let type = firstStringValue(root.type, root.value_type, data.type);
    if (truncated) type = type ? `${type} · truncated` : "truncated";
    if (value !== undefined) {
      const text = typeof value === "string" ? (value === "" ? '""' : value) : debuggerScalarText(value);
      if (text !== "") return { text, type };
    }
  }
  return { text: debuggerScalarText(data), type: "" };
}

async function evaluateDebuggerExpression(event) {
  event.preventDefault();
  const dbg = state.debugger;
  if (!dbg.attached || dbg.status !== "paused" || dbg.evaluateBusy) return;
  const expression = dom.debuggerExpression.value.trim();
  if (!expression) return;
  const targetId = state.targetId;
  const frameId = dbg.frameId;
  const request = dbg.evaluateRequest + 1;
  dbg.evaluateRequest = request;
  dbg.evaluateBusy = true;
  appendDebuggerConsole("command", expression);
  dom.debuggerExpression.value = "";
  updateDebuggerButtons();
  try {
    const params = { expression, context: "repl" };
    if (frameId !== null) params.frame_id = frameId;
    const data = await debuggerCall("evaluate_ui_debug", params, { target: targetId });
    if (request !== dbg.evaluateRequest || targetId !== state.targetId) return;
    if (debuggerStatusIsExplicit(data)) {
      applyDebuggerSnapshot(data);
      renderDebugger();
    }
    const result = debuggerEvaluation(data);
    appendDebuggerConsole("result", result.text, { type: result.type });
  } catch (error) {
    if (request !== dbg.evaluateRequest || targetId !== state.targetId) return;
    appendDebuggerConsole("error", errorMessage(error));
  } finally {
    if (request === dbg.evaluateRequest && targetId === state.targetId) {
      dbg.evaluateBusy = false;
      updateDebuggerButtons();
      renderDebuggerChrome();
      dom.debuggerExpression.focus();
    }
  }
}

function openDebuggerSource(event) {
  event.preventDefault();
  const path = dom.debuggerOpenPath.value.trim();
  if (!path) return;
  loadDebuggerSource({ path }, { quiet: false });
}

function renderDebugger() {
  renderDebuggerChrome();
  renderDebuggerThreads();
  renderDebuggerStack();
  renderDebuggerBreakpoints();
  renderDebuggerSource();
  renderDebuggerVariables();
}

function resetDebugger(previousTarget = state.targetId) {
  const previous = state.debugger;
  stopTimer("debugger");
  if (previous?.attached && previousTarget && state.token) {
    debuggerCall("detach_ui_debugger", {}, { target: previousTarget }).catch(() => {});
  }
  const request = (previous?.request || 0) + 1;
  state.debugger = emptyDebugger();
  state.debugger.request = request;
  if (state.view === "debugger") {
    renderDebugger();
    renderDebuggerConsole();
  }
}

function debuggerUnloadDetach() {
  const dbg = state.debugger;
  if (!state.token || !state.targetId || !dbg.attached) return;
  try {
    fetch("/rpc", {
      method: "POST",
      keepalive: true,
      headers: { ...authHeaders(), "Content-Type": "application/json" },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: ++rpcSequence,
        method: "get_ui_debugger",
        params: { target: state.targetId, action: "detach" },
      }),
    }).catch(() => {});
  } catch {}
}

/* Views and refresh */

function switchView(view, options = {}) {
  if (!["inspector", "network", "logs", "performance", "memory", "app-size", "deep-links", "debugger", "state"].includes(view)) return;
  if (state.view === view && !options.force) return;
  state.view = view;
  const titles = { inspector: "Inspector", network: "Network", logs: "Logs", performance: "Performance", memory: "Memory", "app-size": "App Size", "deep-links": "Deep Links", debugger: "Debugger", state: "State" };
  document.title = `Cove DevTools · ${titles[view]}`;
  const tabs = [...document.querySelectorAll(".rail-tab")];
  for (const tab of tabs) {
    const selected = tab.dataset.view === view;
    tab.setAttribute("aria-selected", String(selected));
    tab.tabIndex = selected ? 0 : -1;
    const panel = $(`view-${tab.dataset.view}`);
    panel.hidden = !selected;
  }
  syncFollowTimers();
  syncAllocationPolling();
  ensureCurrentView();
  if (options.focusTab) {
    const tab = tabs.find(item => item.dataset.view === view);
    tab?.focus();
  }
}

async function refreshCurrent() {
  if (!state.token) {
    showSetup();
    return;
  }
  await refreshSession({ quiet: true });
  if (state.view === "inspector") {
    if (state.caps.tree) await loadTree({ preserveExpansion: true });
    updateTargetUi();
  } else if (state.view === "network") {
    if (state.caps.network) await loadNetwork();
    else renderNetwork();
  } else if (state.view === "logs") {
    if (state.caps.logs) await loadLogs({ reset: true });
    else renderLogs();
  } else if (state.view === "state") {
    if (state.caps.state) await loadRegisteredState(0);
    else renderRegisteredState();
  } else if (state.view === "performance") {
    if (state.performance.snapshot) {
      renderPerformance();
    } else if (state.performance.tab === "profile") {
      if (state.performance.profile.status === "sampling") await pollProfile();
      else if (state.caps.profile) await loadProfile({ quiet: true });
      else renderPerformance();
    } else {
      if (state.caps.timing) await loadFrames({ reset: true });
      else renderPerformance();
      const record = activeCaptureRecord();
      if (record && !record.imported && !record.complete) await loadTraceEvents({ captureId: record.summary?.id, quiet: true });
    }
  } else if (state.view === "memory") {
    if (state.caps.runtime) await loadMemory({ quiet: true });
    else renderMemory();
  } else if (state.view === "app-size") {
    await loadAppSize({ quiet: true });
  } else if (state.view === "deep-links") {
    if (state.deepLinks.root) await loadDeepLinks({ root: state.deepLinks.root, quiet: true });
    else renderDeepLinks();
  } else if (state.view === "debugger") {
    if (state.caps.runtime) await refreshDebugger({ quiet: true });
    else renderDebugger();
  }
}

/* Static event wiring */

function bindEvents() {
  const railTabs = [...document.querySelectorAll(".rail-tab")];
  for (const tab of railTabs) {
    tab.addEventListener("click", () => switchView(tab.dataset.view));
    tab.addEventListener("keydown", event => {
      const index = railTabs.indexOf(tab);
      let next = null;
      if (event.key === "ArrowDown" || event.key === "ArrowRight") next = railTabs[(index + 1) % railTabs.length];
      else if (event.key === "ArrowUp" || event.key === "ArrowLeft") next = railTabs[(index - 1 + railTabs.length) % railTabs.length];
      else if (event.key === "Home") next = railTabs[0];
      else if (event.key === "End") next = railTabs[railTabs.length - 1];
      else if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        switchView(tab.dataset.view, { focusTab: true });
        return;
      }
      if (next) {
        event.preventDefault();
        switchView(next.dataset.view, { focusTab: true });
      }
    });
  }

  dom.targets.addEventListener("change", () => {
    chooseTarget(dom.targets.value).catch(error => toast(errorMessage(error), "error", 8000));
  });
  dom.refresh.addEventListener("click", () => refreshCurrent());
  dom.treeRefresh.addEventListener("click", () => loadTree({ preserveExpansion: true }));
  dom.treeCollapse.addEventListener("click", () => {
    state.search = "";
    dom.treeSearch.value = "";
    state.expanded.clear();
    if (state.rootId) state.expanded.add(state.rootId);
    state.scrollSelection = false;
    renderTreeArea();
  });

  let searchTimer = null;
  dom.treeSearch.addEventListener("input", () => {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => {
      state.search = dom.treeSearch.value;
      state.scrollSelection = false;
      renderTreeArea();
    }, 100);
  });
  dom.filterAnchors.addEventListener("change", () => {
    state.filters.anchors = dom.filterAnchors.checked;
    state.scrollSelection = false;
    renderTreeArea();
  });
  dom.filterPrimitives.addEventListener("change", () => {
    state.filters.primitives = dom.filterPrimitives.checked;
    state.scrollSelection = false;
    renderTreeArea();
  });

  dom.tree.addEventListener("click", event => {
    const twisty = event.target.closest(".tree-twisty");
    const row = event.target.closest(".tree-row");
    if (!row) return;
    const id = row.dataset.id;
    if (twisty && !twisty.disabled) {
      event.preventDefault();
      if (state.expanded.has(id)) state.expanded.delete(id);
      else state.expanded.add(id);
      renderTreeArea();
      const current = [...dom.tree.children].find(child => child.dataset.id === id);
      current?.focus();
      return;
    }
    selectNode(id, { reveal: false });
    row.focus();
  });
  dom.tree.addEventListener("keydown", event => {
    const row = event.target.closest(".tree-row");
    if (!row || !state.visibleRows.length) return;
    const index = state.visibleRows.findIndex(item => item.id === row.dataset.id);
    if (index < 0) return;
    const current = state.visibleRows[index];
    let next = null;
    if (event.key === "ArrowDown") {
      next = state.visibleRows[Math.min(state.visibleRows.length - 1, index + 1)];
    } else if (event.key === "ArrowUp") {
      next = state.visibleRows[Math.max(0, index - 1)];
    } else if (event.key === "Home") {
      next = state.visibleRows[0];
    } else if (event.key === "End") {
      next = state.visibleRows[state.visibleRows.length - 1];
    } else if (event.key === "ArrowRight") {
      if (current.hasChildren && !current.expanded) {
        state.expanded.add(current.id);
        renderTreeArea();
      } else if (current.hasChildren) {
        const child = state.visibleRows[index + 1];
        if (child) next = child;
      }
    } else if (event.key === "ArrowLeft") {
      if (current.expanded && current.hasChildren) {
        state.expanded.delete(current.id);
        renderTreeArea();
      } else if (current.parentId) {
        next = state.visibleRows.find(item => item.id === current.parentId) || null;
      }
    } else if (event.key === "Enter" || event.key === " ") {
      selectNode(current.id, { reveal: false });
      event.preventDefault();
      return;
    } else {
      return;
    }
    event.preventDefault();
    if (next) {
      selectNode(next.id, { reveal: false });
      const nextRow = [...dom.tree.children].find(child => child.dataset.id === next.id);
      nextRow?.focus();
    } else {
      const currentRow = [...dom.tree.children].find(child => child.dataset.id === current.id);
      currentRow?.focus();
    }
  });

  dom.modeSelect.addEventListener("click", () => {
    state.mode = "select";
    updateModeUi();
    renderScreenshot();
    renderDetails();
  });
  dom.modePick.addEventListener("click", () => {
    if (state.mode === "pick") exitPickMode();
    else enterPickMode();
  });
  dom.modeInteract.addEventListener("click", () => {
    if (dom.modeInteract.disabled) return;
    state.mode = "interact";
    updateModeUi();
    if (state.capture?.kind === "window") renderScreenshot();
    else captureWindow();
  });
  dom.pickExit.addEventListener("click", () => exitPickMode());
  dom.captureButton.addEventListener("click", () => captureWindow());
  dom.nodeShotButton.addEventListener("click", () => captureNode());
  dom.saveButton.addEventListener("click", saveCapture);
  dom.screenshotZoom.addEventListener("click", toggleScreenshotZoom);
  dom.screenshotClear.addEventListener("click", clearScreenshot);
  dom.highlightButton.addEventListener("click", () => {
    if (state.selectedId) highlightNode(state.selectedId);
  });
  dom.clearHighlight.addEventListener("click", clearHighlight);
  dom.readValueButton.addEventListener("click", readLiveValue);
  dom.repaintHighlightButton.addEventListener("click", () => { toggleRepaintHighlight(); });
  dom.guidelinesButton.addEventListener("click", () => { toggleGuidelines(); });
  dom.baselinesButton.addEventListener("click", () => { toggleBaselines(); });
  dom.slowAnimationsButton.addEventListener("click", () => { toggleSlowAnimations(); });
  dom.oversizedImagesButton.addEventListener("click", () => { toggleOversizedImages(); });
  dom.copyNodeId.addEventListener("click", () => copyText(state.selectedId, "Node ID"));

  for (const tab of DETAIL_TABS) {
    const button = $(`tab-${tab}`);
    button.addEventListener("click", () => setDetailTab(tab));
    button.addEventListener("keydown", event => {
      const index = DETAIL_TABS.indexOf(tab);
      let next = null;
      if (event.key === "ArrowRight") next = DETAIL_TABS[(index + 1) % DETAIL_TABS.length];
      else if (event.key === "ArrowLeft") next = DETAIL_TABS[(index - 1 + DETAIL_TABS.length) % DETAIL_TABS.length];
      else if (event.key === "Home") next = DETAIL_TABS[0];
      else if (event.key === "End") next = DETAIL_TABS[DETAIL_TABS.length - 1];
      if (next) {
        event.preventDefault();
        setDetailTab(next, true);
      }
    });
  }

  dom.inputToggle.addEventListener("click", toggleInputDrawer);
  dom.keyForm.addEventListener("submit", sendKey);
  dom.textForm.addEventListener("submit", sendText);
  dom.keyValue.addEventListener("input", updateInputButtons);
  dom.textValue.addEventListener("input", updateInputButtons);

  dom.networkRecordingToggle.addEventListener("click", () => setNetworkRecording(state.network.recording === false));
  dom.networkClear.addEventListener("click", () => clearNetwork());
  dom.networkRefresh.addEventListener("click", () => loadNetwork());
  dom.networkFollow.addEventListener("change", () => {
    state.network.follow = dom.networkFollow.checked;
    syncFollowTimers();
    if (state.network.follow) loadNetwork({ quiet: true });
  });
  dom.networkSearch.addEventListener("input", renderNetwork);
  dom.networkStateFilter.addEventListener("change", renderNetwork);
  dom.networkWaterfall.addEventListener("change", () => {
    state.network.waterfall = dom.networkWaterfall.checked;
    renderNetwork();
  });
  dom.networkDialogClose.addEventListener("click", () => dom.networkDialog.close());
  dom.networkDialog.addEventListener("click", event => {
    if (event.target === dom.networkDialog) dom.networkDialog.close();
  });

  dom.logsClear.addEventListener("click", () => clearLogs());
  dom.logsRefresh.addEventListener("click", () => loadLogs({ reset: true }));
  dom.logsFollow.addEventListener("change", () => {
    state.logs.follow = dom.logsFollow.checked;
    syncFollowTimers();
    if (state.logs.follow) loadLogs({ quiet: true });
  });
  dom.logsSearch.addEventListener("input", renderLogs);
  dom.logsLevel.addEventListener("change", renderLogs);
  dom.logsLabel.addEventListener("change", renderLogs);
  dom.stateRefresh.addEventListener("click", () => loadRegisteredState(state.registeredState.offset));
  dom.stateScope.addEventListener("change", () => {
    state.registeredState.scope = dom.stateScope.value;
    loadRegisteredState(0, { quiet: true });
  });
  dom.stateFollow.addEventListener("change", () => {
    state.registeredState.follow = dom.stateFollow.checked;
    syncFollowTimers();
    if (state.registeredState.follow) loadRegisteredState(state.registeredState.offset, { quiet: true });
  });

  dom.performanceRefresh.addEventListener("click", () => {
    const perf = state.performance;
    if (perf.tab === "profile") {
      if (perf.profile.status === "sampling") pollProfile();
      else loadProfile({ quiet: true });
      return;
    }
    loadFrames({ reset: true });
  });
  dom.memoryRefresh.addEventListener("click", () => refreshMemoryView());
  dom.memoryFollow.addEventListener("change", () => {
    state.memory.follow = dom.memoryFollow.checked;
    syncFollowTimers();
    if (state.memory.follow) loadMemory({ quiet: true });
  });
  dom.memoryInterval.addEventListener("change", () => {
    const interval = Number(dom.memoryInterval.value);
    if (!MEMORY_INTERVALS.includes(interval)) return;
    state.memory.intervalMs = interval;
    writeMemoryIntervalPreference(interval);
    syncFollowTimers();
  });
  dom.appSizeRefresh.addEventListener("click", () => loadAppSize({ reset: false }));
  dom.deepLinksRootForm.addEventListener("submit", submitDeepLinks);
  dom.deepLinksRoot.addEventListener("input", () => {
    if (!dom.deepLinksRootError.hidden) {
      showDeepLinksRootError("");
      state.deepLinks.rootError = "";
      if (state.view === "deep-links") renderDeepLinks();
    }
  });
  dom.deepLinksRoot.addEventListener("change", () => {
    const root = dom.deepLinksRoot.value.trim();
    if (!root || !deepLinksRootError(root)) writeDeepLinksRootPreference(root);
  });
  dom.performanceFollow.addEventListener("change", () => {
    state.performance.follow = dom.performanceFollow.checked;
    syncFollowTimers();
    if (state.performance.follow) loadFrames({ quiet: true });
  });
  dom.performanceBudget.addEventListener("change", () => {
    const rate = Number(dom.performanceBudget.value);
    const perf = state.performance;
    if (!Number.isFinite(rate) || rate <= 0) return;
    if (!BUDGET_RATES.includes(rate) && rate !== perf.refreshHz) return;
    perf.budgetHz = rate;
    // Picking the measured rate by hand returns the budget to the runtime;
    // any other choice is an explicit override of it.
    perf.budgetManual = perf.refreshHz !== null && rate !== perf.refreshHz;
    writeBudgetPreference(rate);
    syncBudgetOptions(perf);
    renderPerformance();
  });
  dom.performanceStart.addEventListener("click", startTrace);
  dom.performanceStop.addEventListener("click", () => stopTrace());
  dom.performanceExport.addEventListener("click", exportPerformance);
  dom.performanceImport.addEventListener("click", () => dom.performanceFile.click());
  dom.performanceFile.addEventListener("change", () => importPerformance(dom.performanceFile.files?.[0]));

  const perfTabs = [...document.querySelectorAll(".perf-tab")];
  for (const tab of perfTabs) {
    tab.addEventListener("click", () => setPerformanceTab(tab.dataset.perfTab));
    tab.addEventListener("keydown", event => {
      const index = perfTabs.indexOf(tab);
      let next = null;
      if (event.key === "ArrowRight") next = perfTabs[(index + 1) % perfTabs.length];
      else if (event.key === "ArrowLeft") next = perfTabs[(index - 1 + perfTabs.length) % perfTabs.length];
      else if (event.key === "Home") next = perfTabs[0];
      else if (event.key === "End") next = perfTabs[perfTabs.length - 1];
      else if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        setPerformanceTab(tab.dataset.perfTab, { focusTab: true });
        return;
      }
      if (next) {
        event.preventDefault();
        setPerformanceTab(next.dataset.perfTab, { focusTab: true });
      }
    });
  }
  dom.debuggerRefresh.addEventListener("click", () => refreshDebugger());
  dom.debuggerAttach.addEventListener("click", attachDebugger);
  dom.debuggerDetach.addEventListener("click", detachDebugger);
  dom.debuggerPause.addEventListener("click", () => runDebuggerControl("pause", "Pause"));
  dom.debuggerContinue.addEventListener("click", () => runDebuggerControl("continue", "Continue"));
  dom.debuggerStepOver.addEventListener("click", () => runDebuggerControl("step_over", "Step over"));
  dom.debuggerStepInto.addEventListener("click", () => runDebuggerControl("step_into", "Step in"));
  dom.debuggerStepOut.addEventListener("click", () => runDebuggerControl("step_out", "Step out"));
  dom.debuggerVariablesRefresh.addEventListener("click", () => {
    const frame = selectedDebuggerFrame();
    if (frame) loadDebuggerScopes(frame, { quiet: false });
  });
  dom.debuggerOpenForm.addEventListener("submit", openDebuggerSource);
  dom.debuggerEvaluateForm.addEventListener("submit", evaluateDebuggerExpression);
  dom.debuggerConsoleClear.addEventListener("click", clearDebuggerConsole);

  dom.themeButton.addEventListener("click", () => {
    const order = ["auto", "light", "dark"];
    const next = order[(order.indexOf(state.theme) + 1) % order.length];
    setThemePreference(next);
  });
  const colorScheme = matchMedia("(prefers-color-scheme: dark)");
  const onColorScheme = () => {
    if (state.theme === "auto") applyTheme();
  };
  if (colorScheme.addEventListener) colorScheme.addEventListener("change", onColorScheme);
  else colorScheme.addListener?.(onColorScheme);

  dom.shortcutsButton.addEventListener("click", openShortcuts);
  dom.shortcutsClose.addEventListener("click", () => dom.shortcutsDialog.close());
  dom.shortcutsDialog.addEventListener("click", event => {
    if (event.target === dom.shortcutsDialog) dom.shortcutsDialog.close();
  });

  dom.toastClose.addEventListener("click", () => { dom.toast.hidden = true; });

  dom.tokenForm.addEventListener("submit", event => {
    event.preventDefault();
    const token = dom.tokenInput.value.trim();
    if (!/^[A-Za-z0-9_-]{1,256}$/.test(token)) {
      dom.setupError.hidden = false;
      dom.setupError.textContent = "Enter the token from the Inspector link. It contains letters, digits, hyphens and underscores.";
      return;
    }
    storeToken(token);
    showWorkspace();
    refreshSession();
  });
  dom.setupError.hidden = true;

  document.addEventListener("keydown", handleGlobalKeydown);
  document.addEventListener("visibilitychange", () => {
    syncFollowTimers();
    syncAllocationPolling();
    if (!document.hidden && state.connected) ensureCurrentView();
  });
  window.addEventListener("beforeunload", () => {
    stopAllFollowTimers();
    stopAllocationPolling();
    if (state.performance.recordTimer) clearInterval(state.performance.recordTimer);
    clearProfileTimers(state.performance.profile);
    debuggerUnloadDetach();
    if (state.capture?.url) URL.revokeObjectURL(state.capture.url);
  });
}

function openShortcuts() {
  if (!dom.shortcutsDialog.open) dom.shortcutsDialog.showModal();
  dom.shortcutsClose.focus();
}

function isTextEntry(target) {
  return Boolean(target?.closest?.("input, textarea, select, [contenteditable='true']"));
}

function handleGlobalKeydown(event) {
  const typing = isTextEntry(event.target);
  const dialogOpen = Boolean(document.querySelector("dialog[open]"));
  if (event.key === "Escape") {
    if (dialogOpen || typing) return;
    if (state.mode === "pick") {
      event.preventDefault();
      exitPickMode();
      return;
    }
    if (dom.treeSearch.value) {
      dom.treeSearch.value = "";
      state.search = "";
      renderTreeArea();
    }
    return;
  }
  if (typing || dialogOpen) return;
  if (event.altKey && !event.ctrlKey && !event.metaKey) {
    const index = Number(event.key);
    if (index >= 1 && index <= 9) {
      event.preventDefault();
      switchView(["inspector", "network", "logs", "performance", "memory", "app-size", "deep-links", "debugger", "state"][index - 1], { focusTab: true });
      return;
    }
  }
  if (state.view === "debugger") {
    let action = null;
    let label = "";
    if (event.key === "F5" && !dom.debuggerContinue.disabled) {
      action = "continue";
      label = "Continue";
    } else if (event.key === "F10" && !dom.debuggerStepOver.disabled) {
      action = "step_over";
      label = "Step over";
    } else if (event.key === "F11" && !dom.debuggerStepOut.disabled && event.shiftKey) {
      action = "step_out";
      label = "Step out";
    } else if (event.key === "F11" && !dom.debuggerStepInto.disabled) {
      action = "step_into";
      label = "Step in";
    }
    if (action) {
      event.preventDefault();
      runDebuggerControl(action, label);
      return;
    }
  }
  if (event.ctrlKey || event.metaKey) return;
  if (event.key === "?") {
    event.preventDefault();
    openShortcuts();
    return;
  }
  if (event.key === "/") {
    event.preventDefault();
    if (state.view !== "inspector") switchView("inspector");
    dom.treeSearch.focus();
    dom.treeSearch.select();
    return;
  }
  if (event.key.toLowerCase() === "p" && !event.altKey) {
    event.preventDefault();
    if (state.view !== "inspector") switchView("inspector");
    if (state.mode === "pick") exitPickMode();
    else enterPickMode();
    return;
  }
  if (event.key.toLowerCase() === "r" && !event.altKey) {
    event.preventDefault();
    refreshCurrent();
  }
}

/* Boot */

function init() {
  try {
    const storedTheme = localStorage.getItem(THEME_KEY);
    state.theme = ["auto", "light", "dark"].includes(storedTheme) ? storedTheme : "auto";
  } catch {
    state.theme = "auto";
  }
  applyTheme();
  bindEvents();
  document.title = "Cove DevTools · Inspector";
  setDetailTab(state.activeDetailTab);
  renderScreenshot();
  renderInputUi();
  renderTreeArea();
  renderDetails();
  renderRepaintHighlightUi();
  renderGuidelinesUi();
  renderBaselinesUi();
  renderSlowAnimationsUi();
  renderNetwork();
  renderLogs();
  renderPerformance();
  renderMemory();
  renderAppSize();
  dom.deepLinksRoot.value = state.deepLinks.root;
  renderDeepLinks();
  renderDebugger();
  renderDebuggerConsole();
  updateModeUi();
  const token = readToken();
  if (token) {
    state.token = token;
    showWorkspace();
    refreshSession();
  } else {
    showSetup();
  }
}

init();
