import { emptyDebugger } from "../panels/debugger/model.js";
import { emptyExtensions } from "../panels/extensions.js";
import { emptyAppSize, emptyDeepLinks, emptyMemory, emptyPerformance } from "../panels/performance/overview.js";
import { emptyRegisteredState } from "../panels/state.js";
import { sendKey, sendText } from "../inspector/input.js";
import { clearHighlight } from "../inspector/tree.js";
import { deepLinksRoot, deepLinksRootError } from "../panels/deep-links/model.js";
import { toast } from "./session.js";
export const $ = id => document.getElementById(id);

export const TOKEN_KEY = "cove.devtools.token";
export const THEME_KEY = "cove.devtools.theme";
export const DETAIL_TABS = ["properties", "render", "layout", "source"];
// The flex editor sends the LayoutNode variant names the layout record
// publishes (`Justify` and `Align`), so the client never invents a value the
// runtime cannot parse.
export const FLEX_JUSTIFY_VALUES = ["FlexStart", "Center", "FlexEnd", "SpaceBetween", "SpaceAround", "SpaceEvenly"];
export const FLEX_ALIGN_VALUES = ["Auto", "FlexStart", "Center", "FlexEnd", "Stretch", "Baseline", "SpaceBetween", "SpaceAround", "SpaceEvenly"];
export const FLEX_PROPERTY_NAMES = ["flexGrow", "flexShrink", "justifyContent", "alignItems"];
export const FLEX_FACTOR_LIMIT = 1000;
// A temporary override can reach the layout record one or two frames later.
// The editor reads the record back instead of echoing what it sent.
export const FLEX_RECORD_ATTEMPTS = 5;
export const FLEX_RECORD_DELAY_MS = 100;
export const LEVELS = ["trace", "debug", "info", "notice", "warning", "error", "critical"];
export const PERFORMANCE_TABS = ["frames", "timeline", "profile", "components"];
export const BUDGET_RATES = [60, 90, 120];
export const BUDGET_KEY = "cove.devtools.budgetHz";
export const PROFILE_CAPABILITIES = ["cpu_profile", "profiling", "cpu_sampling", "profile"];
export const PROFILE_HZ_RATES = [100, 250, 500, 1000];
export const PROFILE_DURATIONS = [1000, 3000, 5000, 10000];
export const PROFILE_DEFAULT_HZ = 1000;
export const PROFILE_DEFAULT_DURATION = 5000;
export const PROFILE_POLL_MS = 500;
export const MAX_PROFILE_SAMPLES = 20000;
export const MAX_PROFILE_STACKS = 200;
export const MAX_PROFILE_FRAMES = 64;
export const MAX_PROFILE_STACK_FRAMES = 16;
export const MAX_HOTSPOTS = 200;
export const PROFILE_VIEWS = ["call-tree", "bottom-up", "flame-chart", "methods", "stacks"];
export const PROFILE_VIEW_LABELS = {
  "call-tree": "Call tree",
  "bottom-up": "Bottom-up",
  "flame-chart": "Flame chart",
  methods: "Methods",
  stacks: "Stacks",
};
export const MAX_PROFILE_TREE_NODES = 3000;
export const MAX_PROFILE_TREE_DEPTH = 48;
export const MAX_PROFILE_TREE_CHILDREN = 64;
export const MAX_PROFILE_TREE_ROWS = 1200;
export const MAX_PROFILE_BOTTOM_GROUPS = 200;
export const MAX_PROFILE_BOTTOM_CALLERS = 32;
export const MAX_PROFILE_FLAME_BARS = 320;
export const MAX_PROFILE_FLAME_DEPTH = 40;
export const PROFILE_FLAME_MIN_PERCENT = 0.15;
export const PROFILE_METHOD_SORT_KEYS = ["label", "location", "selfMs", "inclusiveMs", "selfPercent", "inclusivePercent", "selfSamples", "inclusiveSamples"];
export const MAX_MEMORY_COUNTERS = 256;
// The overview chart keeps a bounded ring of provider readings. Each point is
// one real query_ui_memory response; nothing is interpolated into a sample.
export const MEMORY_SAMPLE_LIMIT = 720;
export const MEMORY_INTERVALS = [1000, 2000, 5000];
export const MEMORY_INTERVAL_KEY = "cove.devtools.memoryIntervalMs";
// Allocation sites are opt-in sampled stacks from cove_alloc, not heap objects.
// The controller owns the real ring, symbolization and bounds; this view only
// caps what it draws and says when something was cut.
export const ALLOCATION_SITE_CAPABILITIES = [
  "allocation_sites",
  "allocation_sampling",
  "alloc_sites",
  "alloc_sampling",
];
export const ALLOCATION_DEFAULT_RATE = 1024;
export const ALLOCATION_RATE_LIMIT = 1_000_000;
export const ALLOCATION_POLL_MS = 1000;
export const MAX_ALLOCATION_SITES = 64;
export const MAX_ALLOCATION_SITE_FRAMES = 32;
// Baseline snapshots are this view's own JSON files. Import is strict: the
// schema, every field's type and the site stack identities are checked before
// a snapshot can replace the baseline.
export const ALLOCATION_SNAPSHOT_SCHEMA = "cove.devtools.allocation-sites";
export const ALLOCATION_SNAPSHOT_VERSION = 1;
export const MAX_ALLOCATION_SNAPSHOT_BYTES = 2 * 1024 * 1024;
export const ALLOCATION_SESSION_NONCE = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
export const ALLOCATION_SNAPSHOT_STATUSES = ["running", "stopped"];
export const ALLOCATION_SNAPSHOT_KEYS = [
  "schema",
  "version",
  "capturedAt",
  "reportUpdatedAt",
  "target",
  "session",
  "rate",
  "report",
];
export const ALLOCATION_SNAPSHOT_REPORT_KEYS = [
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
export const ALLOCATION_SNAPSHOT_SITE_KEYS = ["frames", "bytes", "blocks", "framesOmitted", "framesTruncated"];
export const ALLOCATION_SNAPSHOT_FRAME_KEYS = ["address", "function", "file", "line", "module"];
// The controller may call these tools under a different allocation-sampling
// name; only an unknown-command answer moves to the next alias.
export const ALLOCATION_SITE_COMMANDS = {
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
export const MAX_APP_SIZE_SECTIONS = 200;
export const MAX_APP_SIZE_SYMBOLS = 200;
export const MAX_APP_SIZE_GROUPS = 100;
export const MAX_APP_SIZE_SEGMENTS = 100;
export const MAX_APP_SIZE_CONTAINMENT_GROUPS = 25;
export const APP_SIZE_LOCAL_PLATFORMS = ["macos", "windows", "linux"];
// The treemap draws only entries with reported bytes and a deterministic
// squarified layout over a fixed 1000x300 grid; percentages keep it responsive.
export const APP_SIZE_TREEMAP_WIDTH = 1000;
export const APP_SIZE_TREEMAP_HEIGHT = 300;
export const APP_SIZE_TREEMAP_LEGEND = 16;
export const APP_SIZE_TREEMAP_TONES = 12;
export const APP_SIZE_SNAPSHOT_SCHEMA = "cove.devtools.app-size";
export const APP_SIZE_SNAPSHOT_VERSION = 1;
export const MAX_APP_SIZE_DIFF_ROWS = 400;
// Deep Links validates project files, not a runtime. The root is the only
// input, kept verbatim so the controller can enforce its own canonical path.
export const DEEP_LINKS_ROOT_KEY = "cove.devtools.deepLinksRoot";
export const MAX_DEEP_LINKS_FILES = 200;
export const MAX_DEEP_LINKS_ENTRIES = 200;
export const MAX_DEEP_LINKS_ISSUES = 400;
export const DEBUGGER_POLL_MS = 1500;
export const DEBUGGER_CONSOLE_LIMIT = 200;
export const MAX_DEBUGGER_SOURCE_LINES = 20000;
export const DEBUGGER_RPC_TIMEOUT_MS = 10000;
// Slow animations are a real runtime time scale: the host multiplies animation
// deltas by the scale without touching frame timings, input or diagnostics.
export const SLOW_ANIMATION_SCALE = 0.2;
export const NORMAL_ANIMATION_SCALE = 1;
// Per-frame span attribution is optional: hosts advertise it before the client
// calls `analyze_ui_frame`. Without it, only TimingFrame records are available.
export const FRAME_ANALYSIS_CAPABILITIES = ["frame_analysis", "analyze_frame"];
export const SNAPSHOT_SCHEMA = "cove.devtools.performance";
export const SNAPSHOT_VERSION = 1;
export const FRAME_WINDOW = 120;
export const FRAME_TABLE_LIMIT = 100;
export const MAX_FRAME_RECORDS = 240;
export const MAX_FLAME_BARS = 1200;
export const SPAN_TABLE_LIMIT = 1000;
export const MAX_SNAPSHOT_EVENTS = 5000;

export const dom = {
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
  filterChains: $("filter-chains"),
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
  extensionsContent: $("extensions-content"),
  extensionsMeta: $("extensions-meta"),
  extensionsRefresh: $("extensions-refresh"),
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

export const state = {
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
  filters: { anchors: true, primitives: true, chains: true },
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
  extensions: emptyExtensions(),
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

// Imported bindings are read-only, so the counter is advanced by the owner of
// the module rather than by each caller that needs a request id.
export function nextRpcSequence() {
  rpcSequence += 1;
  return rpcSequence;
}

export function emptyCaps() {
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
    componentActivity: false,
    semanticsTree: false,
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

export class RpcError extends Error {
  constructor(message, code = "error", data = null) {
    super(message);
    this.name = "RpcError";
    this.code = code;
    this.data = data;
  }
}

