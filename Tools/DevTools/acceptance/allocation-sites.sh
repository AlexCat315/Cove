#!/usr/bin/env bash
# Allocation-site sampling acceptance for Cove DevTools.
#
# Starts an isolated controller and the real gallery, waits for the runtime,
# requires the `allocation_sites` capability, samples at 1-in-64, drives a real
# `resize_ui`, stops, and asserts the live `UiAllocationSites` contract:
# status/rate, positive sampled bytes and blocks, consistent
# attempted/retained/dropped samples, a non-empty bounded site list, real
# caller frames resolved to function and file:line, no allocator frame before a
# caller, truncation flags that match the returned cut, and a query limit that
# is respected.
#
# The whole start/stop/query cycle runs twice. The second start must reset the
# window counters to zero, a query immediately after it must not replay the
# first window's attempts, and the post-stop query must serve the second
# window's counters. That is the data a baseline/diff view needs: independent
# sampling windows, not a cumulative heap. This harness does not diff sites and
# does not claim type-aware object attribution, retaining paths or leaks; the
# counters are cumulative sampled allocations at the configured rate, never the
# live heap.
#
# Usage: Tools/DevTools/acceptance/allocation-sites.sh [--keep] [--no-build]
#
# Environment:
#   ACCEPTANCE_GALLERY_TIMEOUT   seconds to wait for a registered runtime (default 120)
#   ACCEPTANCE_ACTIVITY_TIMEOUT  seconds to wait for sampled sites after a resize (default 20)
#
# Exit codes: 0 pass, 1 functional assertion failure, 2 blocked (missing
# dependency, build/startup failure, missing tool, missing capability,
# unsupported sampler or missing symbolizer). Interrupt exits 130/143 after
# cleanup. See Docs/devtools-acceptance.md for the full contract.
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
RATE=64
# The site list is bounded by the protocol's MAX_ALLOCATION_SITES, read from the
# published query schema below. Frames are bounded by cove_alloc's capture
# depth (crates/alloc/src/sampling/ring.rs, MAX_FRAMES) and the ring capacity
# (SITE_RING_CAPACITY); both are part of the sampler the harness exercises.
MAX_FRAMES=8
RING_CAPACITY=1024
SITE_LIMIT=8
PREVIEW_LIMIT=4
BASE_WIDTH=1000
BASE_HEIGHT=760
SMALL_WIDTH=780
SMALL_HEIGHT=600
KEEP_ARTIFACTS=0
BUILD=1
for argument in "$@"; do
  case "$argument" in
    --keep) KEEP_ARTIFACTS=1 ;;
    --no-build) BUILD=0 ;;
    -h|--help)
      sed -n '2,29p' "$0"
      exit 0
      ;;
    *)
      printf 'Unknown argument: %s\n' "$argument" >&2
      exit 2
      ;;
  esac
done

for setting in "${ACCEPTANCE_GALLERY_TIMEOUT:-120}" "${ACCEPTANCE_ACTIVITY_TIMEOUT:-20}"; do
  case "$setting" in
    ''|*[!0-9]*)
      printf 'ACCEPTANCE_GALLERY_TIMEOUT and ACCEPTANCE_ACTIVITY_TIMEOUT must be whole numbers of seconds.\n' >&2
      exit 2
      ;;
  esac
done
GALLERY_TIMEOUT=${ACCEPTANCE_GALLERY_TIMEOUT:-120}
ACTIVITY_TIMEOUT=${ACCEPTANCE_ACTIVITY_TIMEOUT:-20}

RUN_DIR=""
CONTROLLER_PID=""
GALLERY_PID=""
PASS_COUNT=0

say() { printf '%s\n' "$*"; }
step() { printf '\n== %s ==\n' "$*"; }
pass() {
  PASS_COUNT=$((PASS_COUNT + 1))
  printf 'PASS  %s\n' "$*"
}
warn() { printf 'WARN  %s\n' "$*"; }

dump_logs() {
  [ -n "${RUN_DIR:-}" ] || return 0
  for file in controller gallery; do
    if [ -s "$RUN_DIR/logs/$file.log" ]; then
      say "--- $file.log (tail) ---"
      tail -n 20 "$RUN_DIR/logs/$file.log" || true
    fi
  done
}

blocked() {
  printf '\nBLOCKED  %s\nACTION   %s\n' "$1" "$2"
  dump_logs
  exit 2
}

failed() {
  printf '\nFAIL  %s\n' "$1"
  dump_logs
  exit 1
}

stop_pid() {
  pid=$1
  [ -n "$pid" ] || return 0
  kill -0 "$pid" 2>/dev/null || return 0
  kill -TERM "$pid" 2>/dev/null || true
  count=0
  while kill -0 "$pid" 2>/dev/null && [ "$count" -lt 50 ]; do
    sleep 0.1
    count=$((count + 1))
  done
  if kill -0 "$pid" 2>/dev/null; then
    kill -KILL "$pid" 2>/dev/null || true
  fi
  wait "$pid" 2>/dev/null || true
}

cleanup() {
  status=$?
  trap - EXIT INT TERM
  stop_pid "${GALLERY_PID:-}"
  GALLERY_PID=""
  stop_pid "${CONTROLLER_PID:-}"
  CONTROLLER_PID=""
  if [ -n "${RUN_DIR:-}" ] && [ -d "$RUN_DIR" ]; then
    if [ "$status" -eq 0 ] && [ "$KEEP_ARTIFACTS" -eq 0 ]; then
      rm -rf "$RUN_DIR" || true
    else
      printf 'Artifacts: %s\n' "$RUN_DIR"
    fi
  fi
  exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

is_uint() { [[ "$1" =~ ^[0-9]+$ ]]; }

step "Preflight"
for command in cargo python3 jq; do
  command -v "$command" >/dev/null 2>&1 || blocked \
    "Required command '$command' is not available." \
    "Install '$command' (for jq: brew install jq) and rerun."
done

TMP_ROOT=${TMPDIR:-/tmp}
TMP_ROOT=${TMP_ROOT%/}
[ -n "$TMP_ROOT" ] || TMP_ROOT=/tmp
RUN_DIR=$(mktemp -d "$TMP_ROOT/cove-devtools-allocation-sites.XXXXXX") || blocked \
  "Cannot create an isolated run directory." \
  "Check TMPDIR permissions and free space."
if ! mkdir -p "$RUN_DIR/logs" "$RUN_DIR/session" "$RUN_DIR/artifacts"; then
  blocked "Cannot create the acceptance workspace under $RUN_DIR." \
    "Check TMPDIR permissions and free space."
fi
say "Isolated run: $RUN_DIR"

if [ "$BUILD" -eq 1 ]; then
  step "Build controller and gallery"
  say "Building debug binaries; incremental builds are fast after the first run."
  if ! (cd "$ROOT" && cargo build --quiet -p cove-devtools-cli --bin cove-devtools) \
    >"$RUN_DIR/logs/build-controller.log" 2>&1; then
    tail -n 40 "$RUN_DIR/logs/build-controller.log" || true
    blocked "cargo build -p cove-devtools-cli --bin cove-devtools failed." \
      "Fix the build error above; acceptance never runs against stale binaries."
  fi
  if ! (cd "$ROOT" && cargo build --quiet -p cove-examples --bin gallery) \
    >"$RUN_DIR/logs/build-gallery.log" 2>&1; then
    tail -n 40 "$RUN_DIR/logs/build-gallery.log" || true
    blocked "cargo build -p cove-examples --bin gallery failed." \
      "Fix the build error above; acceptance never runs against stale binaries."
  fi
else
  step "Reuse existing binaries (--no-build)"
fi

if ! TARGET_DIR=$(cd "$ROOT" && cargo metadata --no-deps --format-version 1 \
  | jq -r '.target_directory'); then
  blocked "cargo metadata could not resolve the workspace target directory." \
    "Fix the workspace manifest and rerun."
fi
CLI="$TARGET_DIR/debug/cove-devtools"
GALLERY="$TARGET_DIR/debug/gallery"
[ -x "$CLI" ] || blocked "Controller binary not found at $CLI." \
  "Run cargo build -p cove-devtools-cli --bin cove-devtools in $ROOT."
[ -x "$GALLERY" ] || blocked "Gallery binary not found at $GALLERY." \
  "Run cargo build -p cove-examples --bin gallery in $ROOT."

if ! "$CLI" tools >"$RUN_DIR/tools.json" 2>"$RUN_DIR/logs/tools.err"; then
  tail -n 20 "$RUN_DIR/logs/tools.err" || true
  blocked "cove-devtools tools failed." \
    "Fix the controller CLI before running acceptance."
fi

tool_exists() {
  jq -e --arg name "$1" '.tools[]? | select(.name == $name)' "$RUN_DIR/tools.json" \
    >/dev/null 2>&1
}
tool_has_property() {
  jq -e --arg name "$1" --arg key "$2" \
    '.tools[]? | select(.name == $name) | (.inputSchema.properties // {}) | has($key)' \
    "$RUN_DIR/tools.json" >/dev/null 2>&1
}
tool_requires() {
  jq -e --arg name "$1" --arg key "$2" \
    '.tools[]? | select(.name == $name) | (.inputSchema.required // []) | index($key) != null' \
    "$RUN_DIR/tools.json" >/dev/null 2>&1
}

for name in start_ui_allocation_profile stop_ui_allocation_profile query_ui_allocation_sites; do
  tool_exists "$name" || blocked \
    "$name is not registered by the controller (allocation-site sampling tools are missing)." \
    "Restore the allocation-site tools in cove-agent/cove-devtools-cli; this harness cannot sample sites without them."
  tool_has_property "$name" target || blocked \
    "$name has no 'target' argument, so the harness cannot select the gallery runtime." \
    "Restore the target argument on the allocation-site tools and rerun."
done
tool_requires start_ui_allocation_profile rate || blocked \
  "start_ui_allocation_profile does not require a rate." \
  "Restore the {target?, rate} contract; sampling requires the 1-in-N divisor."
tool_has_property query_ui_allocation_sites limit || blocked \
  "query_ui_allocation_sites has no 'limit' argument, so a bounded query cannot be tested." \
  "Restore the {target?, limit} contract and rerun."

MAX_SITES=$(jq -r '
  .tools[]? | select(.name == "query_ui_allocation_sites")
  | (.inputSchema.properties.limit.maximum
     // ([.inputSchema.properties.limit.oneOf[]? | .maximum] | first)
     // empty)
  | if type == "number" then floor else empty end
' "$RUN_DIR/tools.json")
is_uint "$MAX_SITES" || blocked \
  "query_ui_allocation_sites publishes no integer limit bound ('$MAX_SITES')." \
  "Restore the MAX_ALLOCATION_SITES bound on the query schema and rerun."
if [ "$MAX_SITES" -lt 1 ] || [ "$MAX_SITES" -gt 1000000 ]; then
  blocked "query_ui_allocation_sites publishes an implausible limit bound ($MAX_SITES)." \
    "Align the schema with MAX_ALLOCATION_SITES in crates/devtools-protocol and rerun."
fi
[ "$SITE_LIMIT" -le "$MAX_SITES" ] || blocked \
  "The harness query limit $SITE_LIMIT exceeds the published bound $MAX_SITES." \
    "Lower SITE_LIMIT in this harness or restore the published bound."

pick_port() {
  python3 - <<'PY'
import socket
sock = socket.socket()
sock.bind(("127.0.0.1", 0))
print(sock.getsockname()[1])
sock.close()
PY
}
if ! PORT=$(pick_port); then
  blocked "Cannot allocate a loopback port." "Check local networking and rerun."
fi
case "$PORT" in
  ''|*[!0-9]*) blocked "The allocated port '$PORT' is not numeric." "Check the Python socket API and rerun." ;;
esac
if [ "$PORT" -lt 1024 ] || [ "$PORT" -gt 65535 ] || [ "$PORT" -eq 9229 ]; then
  PORT=$(pick_port)
fi
case "$PORT" in
  ''|*[!0-9]*) blocked "The allocated port '$PORT' is not numeric." "Check the Python socket API and rerun." ;;
esac
if [ "$PORT" -eq 9229 ]; then
  blocked "The temporary allocator returned the default port 9229." \
    "Rerun; the harness refuses to touch the user's live controller."
fi
if ! TOKEN=$(python3 -c 'import secrets; print(secrets.token_hex(24))'); then
  blocked "Cannot generate an isolated controller token." "Check the Python installation and rerun."
fi

export COVE_DEVTOOLS_SESSION_FILE="$RUN_DIR/session/controller.json"
export COVE_DEVTOOLS_TOKEN="$TOKEN"
export COVE_DEVTOOLS_ENDPOINT="http://127.0.0.1:$PORT"
if [ "$COVE_DEVTOOLS_SESSION_FILE" = "$HOME/Library/Application Support/Cove/devtools-controller.json" ]; then
  blocked "The isolated run would write the user's live discovery file." \
    "Report this as a harness bug; the run directory must own its session file."
fi

CALL_OUT=""
CALL_ERR="$RUN_DIR/logs/last-call.err"
CALL_STATUS=0

cove_call() {
  name=$1
  args=$2
  CALL_ERR="$RUN_DIR/logs/last-call.err"
  if CALL_OUT=$("$CLI" call "$name" "$args" 2>"$CALL_ERR"); then
    CALL_STATUS=0
  else
    CALL_STATUS=$?
  fi
  printf -- '--- %s %s (status %s)\n%s\n' "$name" "$args" "$CALL_STATUS" "$CALL_OUT" \
    >>"$RUN_DIR/logs/calls.log"
  return 0
}

call_message() {
  message=$(jq -r '.message // .error.message // .data.message // empty' \
    <<<"$CALL_OUT" 2>/dev/null || true)
  if [ -z "$message" ] && [ -s "$CALL_ERR" ]; then
    message=$(head -c 400 "$CALL_ERR" || true)
  fi
  [ -n "$message" ] || message="no controller message"
  printf '%s' "$message"
}

call_error_code() {
  jq -r '
    if (.data.code? | type) == "string" then .data.code
    elif (.error.code? | type) == "string" then .error.code
    elif (.code? | type) == "string" then .code
    else empty end
  ' <<<"$CALL_OUT" 2>/dev/null || true
}

# Common controller faults. `blocked` exits 2 for an unavailable sampler or a
# target that cannot be selected; `failed` exits 1 for everything else.
allocation_call_fault() {
  name=$1
  code=$(call_error_code)
  case "$code" in
    unavailable)
      blocked "$name reports allocation-site sampling unavailable: $(call_message)" \
        "The runtime advertises the capability only where cove_alloc can capture stacks. Run the native desktop gallery; a browser build and unsupported platforms must block instead of publishing sites."
      ;;
    target_gone|target_required|target_unbound)
      blocked "$name cannot select the runtime (code ${code:-unknown}): $(call_message)" \
        "Re-read get_ui_session; the gallery may have disconnected between calls. Acceptance never falls back to another runtime."
      ;;
    busy)
      failed "$name reports a session already running in this isolated gallery: $(call_message)"
      ;;
    invalid_request)
      failed "$name rejected the harness arguments: $(call_message)"
      ;;
    *)
      failed "$name failed (code ${code:-unknown}): $(call_message)"
      ;;
  esac
}

step "Start isolated controller on 127.0.0.1:$PORT"
(
  cd "$ROOT"
  exec "$CLI" serve --port "$PORT"
) >"$RUN_DIR/logs/controller.log" 2>&1 &
CONTROLLER_PID=$!
disown "$CONTROLLER_PID" 2>/dev/null || true

ready=0
deadline=$(( $(date +%s) + 30 ))
while [ "$(date +%s)" -lt "$deadline" ]; do
  if ! kill -0 "$CONTROLLER_PID" 2>/dev/null; then
    blocked "The controller exited before answering get_ui_session." \
      "Inspect controller.log; another process may hold the port or startup failed."
  fi
  if [ -f "$COVE_DEVTOOLS_SESSION_FILE" ]; then
    cove_call get_ui_session '{}'
    if [ "$CALL_STATUS" -eq 0 ] \
      && jq -e '.targets | type == "array"' >/dev/null 2>&1 <<<"$CALL_OUT"; then
      ready=1
      break
    fi
  fi
  sleep 0.25
done
[ "$ready" -eq 1 ] || blocked \
  "The controller did not answer get_ui_session within 30 seconds." \
  "Inspect controller.log and the discovery file $COVE_DEVTOOLS_SESSION_FILE."
DISCOVERY_ENDPOINT=$(jq -r '.endpoint // empty' "$COVE_DEVTOOLS_SESSION_FILE" 2>/dev/null || true)
[ "$DISCOVERY_ENDPOINT" = "$COVE_DEVTOOLS_ENDPOINT" ] || blocked \
  "The isolated controller wrote endpoint '$DISCOVERY_ENDPOINT' instead of '$COVE_DEVTOOLS_ENDPOINT'." \
  "Stop and inspect controller.log; every call must reach this run's controller, never the user's live one."
pass "controller ready at $COVE_DEVTOOLS_ENDPOINT (default port 9229 not used)"

step "Start gallery with COVE_DEVTOOLS=1 at ${BASE_WIDTH}x${BASE_HEIGHT}"
(
  cd "$ROOT"
  export COVE_DEVTOOLS=1
  exec "$GALLERY" --width "$BASE_WIDTH" --height "$BASE_HEIGHT"
) >"$RUN_DIR/logs/gallery.log" 2>&1 &
GALLERY_PID=$!
disown "$GALLERY_PID" 2>/dev/null || true
say "Gallery pid $GALLERY_PID"

RUNTIME_JSON=""
SESSION_JSON=""
deadline=$(( $(date +%s) + GALLERY_TIMEOUT ))
while [ "$(date +%s)" -lt "$deadline" ]; do
  if ! kill -0 "$GALLERY_PID" 2>/dev/null; then
    blocked "The gallery exited before registering a runtime." \
      "Open gallery.log. A GUI session and a working COVE_DEVTOOLS build are required."
  fi
  cove_call get_ui_session '{}'
  if [ "$CALL_STATUS" -eq 0 ]; then
    SESSION_JSON=$CALL_OUT
    RUNTIME_JSON=$(jq -c --argjson pid "$GALLERY_PID" '
      [ .targets[]? | select(.kind == "runtime" and (.pid // -1) == $pid) ] | .[0] // empty
    ' <<<"$SESSION_JSON")
    [ -n "$RUNTIME_JSON" ] && break
  fi
  sleep 0.5
done
[ -n "$RUNTIME_JSON" ] || blocked \
  "The gallery did not register with the isolated controller within $GALLERY_TIMEOUT seconds." \
  "Check gallery.log; verify the window opened on an unlocked display and COVE_DEVTOOLS_SESSION_FILE reached the app. The runtime is selected by the gallery PID only, never by title."

RUNTIME_ID=$(jq -r '.id' <<<"$RUNTIME_JSON")
RUNTIME_TITLE=$(jq -r '.title // empty' <<<"$RUNTIME_JSON")
RUNTIME_PID=$(jq -r '.pid // -1' <<<"$RUNTIME_JSON")
RUNTIME_PLATFORM=$(jq -r '.platform // empty' <<<"$RUNTIME_JSON")
RUNTIME_CAPS=$(jq -r '(.capabilities // []) | join(", ")' <<<"$RUNTIME_JSON")
pass "runtime selected: $RUNTIME_ID ($RUNTIME_TITLE, pid $RUNTIME_PID, platform ${RUNTIME_PLATFORM:-unknown})"

step "Require the allocation_sites capability"
if ! jq -e '(.capabilities // []) | index("allocation_sites") != null' \
  >/dev/null 2>&1 <<<"$RUNTIME_JSON"; then
  blocked "The gallery runtime does not advertise 'allocation_sites' (capabilities: ${RUNTIME_CAPS:-none})." \
    "The native sampler is unavailable to this build or platform. Run 'cargo test -p cove-devtools --test allocation_sites' and check that target/debug/gallery is a native desktop binary. Acceptance never invents sampled sites."
fi
pass "runtime advertises allocation_sites"

TARGET_ONLY=$(jq -cn --arg target "$RUNTIME_ID" '{target: $target}')
query_args() {
  jq -cn --arg target "$RUNTIME_ID" --argjson limit "$1" '{target: $target, limit: $limit}'
}

# Reads one u64 field into FIELD.
uint_field() {
  json=$1
  key=$2
  FIELD=$(jq -r --arg key "$key" 'if has($key) then .[$key] else empty end' <<<"$json")
  is_uint "$FIELD" || failed "$3: $key is '$FIELD', not a non-negative integer."
}

# The authoritative stack-frame filter from crates/devtools/src/allocation_sites.
# A frame is allocator-internal when it belongs to cove_alloc, a Rust allocation
# shim, or the unwinder. The probe drops leading internal frames so a site starts
# at the requester that allocated.
JQ_SITE_FILTER=$(cat <<'JQ'
def internal:
  (((.filePath // "") | gsub("\\\\"; "/")) | contains("crates/alloc/src/"))
  or
  ((.function // "") as $f
   | ["cove_alloc::", "TrackingAllocator", "__rust_alloc", "__rust_alloc_zeroed",
      "__rust_realloc", "__rg_alloc", "__rg_alloc_zeroed", "__rdl_alloc",
      "__rdl_alloc_zeroed", "__rdl_realloc", "alloc::alloc::alloc",
      "alloc::alloc::alloc_zeroed", "alloc::alloc::exchange_malloc",
      "libc::backtrace", "backtrace::"]
     | any(. as $name | $f | contains($name)));
def real:
  (((.function // "") | length) > 0)
  and (((.filePath // "") | length) > 0)
  and ((.line // 0) > 0);
JQ
)

validate_report() {
  local json=$1
  local expected_status=$2
  local expected_rate=$3
  local limit=$4
  local label=$5
  local nonempty=$6
  local evidence=$7
  local status sampled_bytes sampled_blocks attempted retained dropped
  local capture_failures site_count truncated sites_len expected_len
  local expected_truncated SUM_BYTES SUM_BLOCKS SITE_PROBLEMS INTERNAL
  local MIXED_SITES ALL_INTERNAL_SITES REAL_FRAMES

  status=$(jq -r '.status // empty' <<<"$json")
  [ "$status" = "$expected_status" ] || failed \
    "$label: status is '$status', expected '$expected_status'."
  uint_field "$json" rate "$label"
  [ "$FIELD" -eq "$expected_rate" ] || failed \
    "$label: rate is $FIELD, expected $expected_rate."
  uint_field "$json" sampledBytes "$label"
  sampled_bytes=$FIELD
  uint_field "$json" sampledBlocks "$label"
  sampled_blocks=$FIELD
  uint_field "$json" samplesAttempted "$label"
  attempted=$FIELD
  uint_field "$json" samplesRetained "$label"
  retained=$FIELD
  uint_field "$json" samplesDropped "$label"
  dropped=$FIELD
  uint_field "$json" captureFailures "$label"
  capture_failures=$FIELD
  uint_field "$json" siteCount "$label"
  site_count=$FIELD

  jq -e '(.sites | type) == "array"' >/dev/null 2>&1 <<<"$json" || failed \
    "$label: sites is missing or not an array."
  jq -e '(.sitesTruncated | type) == "boolean"' >/dev/null 2>&1 <<<"$json" || failed \
    "$label: sitesTruncated is missing or not a boolean."
  truncated=$(jq -r '.sitesTruncated' <<<"$json")
  sites_len=$(jq -r '.sites | length' <<<"$json")

  expected_len=$limit
  if [ "$site_count" -lt "$limit" ]; then
    expected_len=$site_count
  fi
  [ "$sites_len" -eq "$expected_len" ] || failed \
    "$label: returned $sites_len sites for limit $limit and siteCount $site_count; the bounded payload must carry min(limit, siteCount) sites."
  [ "$sites_len" -le "$MAX_SITES" ] || failed \
    "$label: returned $sites_len sites above the published bound $MAX_SITES."
  [ "$site_count" -ge "$sites_len" ] || failed \
    "$label: siteCount $site_count is below the returned site count $sites_len."

  expected_truncated=false
  if [ "$site_count" -gt "$sites_len" ]; then
    expected_truncated=true
  fi
  [ "$truncated" = "$expected_truncated" ] || failed \
    "$label: sitesTruncated=$truncated contradicts siteCount $site_count with $sites_len returned sites."

  [ "$retained" -le "$RING_CAPACITY" ] || failed \
    "$label: samplesRetained $retained exceeds the sampler ring capacity $RING_CAPACITY."
  [ "$attempted" -eq $((retained + dropped + capture_failures)) ] || failed \
    "$label: samplesAttempted $attempted != retained $retained + dropped $dropped + captureFailures $capture_failures."
  [ "$sampled_blocks" -eq "$retained" ] || failed \
    "$label: sampledBlocks $sampled_blocks != samplesRetained $retained; every retained sample contributes one sampled block."
  [ "$site_count" -le "$retained" ] || failed \
    "$label: siteCount $site_count exceeds samplesRetained $retained; each site needs at least one retained sample."

  if [ "$nonempty" -eq 1 ]; then
    [ "$sampled_bytes" -gt 0 ] || failed \
      "$label: sampledBytes is 0 after a real resize; cumulative sampled allocations cannot be empty."
    [ "$sampled_blocks" -gt 0 ] || failed \
      "$label: sampledBlocks is 0 after a real resize."
    [ "$retained" -gt 0 ] || failed \
      "$label: samplesRetained is 0 after a real resize."
    [ "$site_count" -gt 0 ] || failed \
      "$label: siteCount is 0 after a real resize."
    [ "$sites_len" -gt 0 ] || failed \
      "$label: sites is empty after a real resize."
  fi

  SUM_BYTES=$(jq -r '[.sites[]?.bytes // 0] | add // 0' <<<"$json")
  SUM_BLOCKS=$(jq -r '[.sites[]?.blocks // 0] | add // 0' <<<"$json")
  is_uint "$SUM_BYTES" || failed "$label: site bytes do not sum to a non-negative integer."
  is_uint "$SUM_BLOCKS" || failed "$label: site blocks do not sum to a non-negative integer."
  [ "$SUM_BYTES" -le "$sampled_bytes" ] || failed \
    "$label: the returned sites sum to $SUM_BYTES bytes, above sampledBytes $sampled_bytes."
  [ "$SUM_BLOCKS" -le "$sampled_blocks" ] || failed \
    "$label: the returned sites sum to $SUM_BLOCKS blocks, above sampledBlocks $sampled_blocks."
  if [ "$truncated" = "false" ]; then
    [ "$SUM_BYTES" -eq "$sampled_bytes" ] || failed \
      "$label: an untruncated answer lists $SUM_BYTES bytes but sampledBytes is $sampled_bytes."
    [ "$SUM_BLOCKS" -eq "$sampled_blocks" ] || failed \
      "$label: an untruncated answer lists $SUM_BLOCKS blocks but sampledBlocks is $sampled_blocks."
  fi

  SITE_PROBLEMS=$(jq -r --argjson max "$MAX_FRAMES" '
    .sites | to_entries[] | .key as $i | .value as $s
    | if (($s.bytes | type) != "number") then "site \($i): bytes is not a number"
      elif ($s.bytes <= 0) or ($s.bytes != ($s.bytes | floor)) then
        "site \($i): bytes \($s.bytes) is not a positive integer"
      elif (($s.blocks | type) != "number") then "site \($i): blocks is not a number"
      elif ($s.blocks <= 0) or ($s.blocks != ($s.blocks | floor)) then
        "site \($i): blocks \($s.blocks) is not a positive integer"
      elif (($s.framesTruncated | type) != "boolean") then
        "site \($i): framesTruncated is not a boolean"
      elif (($s.frames | type) != "array") then "site \($i): frames is not an array"
      elif (($s.frames | length) == 0) then "site \($i): frames is empty"
      elif (($s.frames | length) > $max) then
        "site \($i): \($s.frames | length) frames exceed the capture bound \($max)"
      elif ([$s.frames[] | select((.address | type) != "number" or .address <= 0 or .address != (.address | floor))] | length) > 0 then
        "site \($i): a frame address is not a positive integer"
      else empty end
  ' <<<"$json" | head -n 3 || true)
  [ -z "$SITE_PROBLEMS" ] || failed "$label: $SITE_PROBLEMS"

  INTERNAL=$(jq -r "$JQ_SITE_FILTER"'
    {
      internal_frames: ([.sites[].frames[] | select(internal)] | length),
      mixed_sites: ([.sites[]
        | (.frames | map(internal)) as $flags
        | select(($flags | any) and (($flags | all) | not))] | length),
      all_internal_sites: ([.sites[]
        | select((.frames | length) > 0 and (.frames | map(internal) | all))] | length),
      real_frames: ([.sites[].frames[] | select(real and (internal | not))] | length),
      first_real: ([.sites[].frames[] | select(real and (internal | not))][0] // null)
    }
  ' <<<"$json")
  MIXED_SITES=$(jq -r '.mixed_sites' <<<"$INTERNAL")
  ALL_INTERNAL_SITES=$(jq -r '.all_internal_sites' <<<"$INTERNAL")
  REAL_FRAMES=$(jq -r '.real_frames' <<<"$INTERNAL")
  [ "$MIXED_SITES" -eq 0 ] || failed \
    "$label: $MIXED_SITES site(s) mix allocator-internal frames with caller frames; the probe must drop allocator frames before the first caller."
  if [ "$ALL_INTERNAL_SITES" -gt 0 ]; then
    warn "$label: $ALL_INTERNAL_SITES site(s) resolve entirely to allocator internals; they are real samples but name no caller."
  fi
  if [ "$evidence" -eq 1 ]; then
    [ "$REAL_FRAMES" -gt 0 ] || failed \
      "$label: no frame resolves to a function with file:line; the sampled addresses cannot be attributed to caller code."
    REAL_FRAME_DESC=$(jq -r '.first_real | "\(.function) at \(.filePath):\(.line)"' <<<"$INTERNAL")
  fi
}

pause_for_activity() {
  sleep 0.25
}

run_window() {
  local label=$1
  local width=$2
  local height=$3
  local first_attempts=$4
  local start_args resize_args code status live_ok deadline
  local retained bytes count early_attempts
  local run_bytes run_blocks run_attempted run_retained run_count
  local symbolization_reason stop_bytes stop_blocks stop_attempted
  local stop_retained stop_dropped stop_failures stop_count stop_len
  local stop_truncated query_counters stop_counters stop_prefix query_sites
  local query_len query_truncated first_site query_one

  step "Window $label: start sampling at 1-in-$RATE"
  start_args=$(jq -cn --arg target "$RUNTIME_ID" --argjson rate "$RATE" \
    '{target: $target, rate: $rate}')
  cove_call start_ui_allocation_profile "$start_args"
  if [ "$CALL_STATUS" -ne 0 ]; then
    allocation_call_fault start_ui_allocation_profile
  fi
  W_START_JSON=$CALL_OUT
  printf '%s\n' "$W_START_JSON" >"$RUN_DIR/artifacts/window-$label-start.json"
  jq -e --argjson rate "$RATE" '
    .status == "running"
    and .rate == $rate
    and .sampledBytes == 0
    and .sampledBlocks == 0
    and .samplesAttempted == 0
    and .samplesRetained == 0
    and .samplesDropped == 0
    and .captureFailures == 0
    and .siteCount == 0
    and .sitesTruncated == false
    and (.sites | type) == "array"
    and (.sites | length) == 0
  ' >/dev/null 2>&1 <<<"$W_START_JSON" || failed \
    "window $label start did not return a fresh running window: $(jq -c '{status,rate,sampledBytes,sampledBlocks,samplesAttempted,samplesRetained,samplesDropped,captureFailures,siteCount,sitesTruncated,sites:(.sites|length)}' <<<"$W_START_JSON" 2>/dev/null || printf 'unparseable response')"
  pass "window $label start: status running at 1-in-$RATE, every window counter zeroed"

  step "Window $label: read the running session before activity"
  cove_call query_ui_allocation_sites "$(query_args "$PREVIEW_LIMIT")"
  if [ "$CALL_STATUS" -ne 0 ]; then
    allocation_call_fault query_ui_allocation_sites
  fi
  W_EARLY_JSON=$CALL_OUT
  printf '%s\n' "$W_EARLY_JSON" >"$RUN_DIR/artifacts/window-$label-early.json"
  validate_report "$W_EARLY_JSON" running "$RATE" "$PREVIEW_LIMIT" \
    "window $label running preview" 0 0
  early_attempts=$(jq -r '.samplesAttempted' <<<"$W_EARLY_JSON")
  if [ -n "$first_attempts" ]; then
    [ "$early_attempts" -lt "$first_attempts" ] || failed \
      "window $label is not a fresh sampling window: its post-start query already reports $early_attempts attempted samples, not below window 1's $first_attempts. A new start must clear the ring and counters instead of replaying the previous window."
    pass "window $label fresh window: post-start query reports $early_attempts attempts, below window 1's $first_attempts"
  fi

  step "Window $label: drive real activity with resize_ui to ${width}x${height}"
  resize_args=$(jq -cn --arg target "$RUNTIME_ID" --argjson width "$width" \
    --argjson height "$height" '{target: $target, width: $width, height: $height}')
  cove_call resize_ui "$resize_args"
  if [ "$CALL_STATUS" -ne 0 ]; then
    code=$(call_error_code)
    case "$code" in
      unavailable)
        blocked "resize_ui is unavailable on this runtime: $(call_message)" \
          "Sampled sites cannot be proven live without a real operation. Run the harness against the desktop gallery, which exposes a resize callback."
        ;;
      target_gone|target_required|target_unbound)
        blocked "resize_ui cannot select the runtime (code ${code:-unknown}): $(call_message)" \
          "The runtime disconnected between the start and the resize; rerun and inspect gallery.log if it repeats."
        ;;
      *)
        failed "resize_ui to ${width}x${height} failed (code ${code:-unknown}): $(call_message)"
        ;;
    esac
  fi
  jq -e 'if has("ok") then .ok == true else true end' >/dev/null 2>&1 <<<"$CALL_OUT" \
    || failed "resize_ui did not answer ok=true: $(jq -c . <<<"$CALL_OUT" 2>/dev/null || printf 'unparseable response')"
  if [ "$(jq -r 'if has("data") then (.data.requested // empty) else (.requested // empty) end' <<<"$CALL_OUT")" != "true" ]; then
    warn "resize_ui did not echo requested=true; the sampled sites below are the evidence that the operation allocated."
  fi
  pause_for_activity

  step "Window $label: poll the live session until a resize allocation is sampled"
  live_ok=0
  deadline=$(( $(date +%s) + ACTIVITY_TIMEOUT ))
  while [ "$(date +%s)" -lt "$deadline" ]; do
    if ! kill -0 "$CONTROLLER_PID" 2>/dev/null; then
      blocked "The controller exited while waiting for sampled allocation sites." \
        "Inspect controller.log; a dead controller cannot prove a live sampler."
    fi
    if ! kill -0 "$GALLERY_PID" 2>/dev/null; then
      blocked "The gallery exited while waiting for sampled allocation sites." \
        "Open gallery.log; the runtime must stay alive for the sampling window."
    fi
    cove_call query_ui_allocation_sites "$(query_args "$SITE_LIMIT")"
    if [ "$CALL_STATUS" -ne 0 ]; then
      allocation_call_fault query_ui_allocation_sites
    fi
    status=$(jq -r '.status // empty' <<<"$CALL_OUT")
    if [ "$status" = "ready" ]; then
      failed "window $label stopped sampling without stop_ui_allocation_profile; a running query must report status running."
    fi
    [ "$status" = "running" ] || failed \
      "window $label live query returned status '$status' instead of running."
    validate_report "$CALL_OUT" running "$RATE" "$SITE_LIMIT" \
      "window $label live query" 0 0
    retained=$(jq -r '.samplesRetained' <<<"$CALL_OUT")
    bytes=$(jq -r '.sampledBytes' <<<"$CALL_OUT")
    count=$(jq -r '.siteCount' <<<"$CALL_OUT")
    if [ "$retained" -gt 0 ] && [ "$bytes" -gt 0 ] && [ "$count" -gt 0 ]; then
      live_ok=1
      W_RUNNING_JSON=$CALL_OUT
      break
    fi
    sleep 0.25
  done
  [ "$live_ok" -eq 1 ] || failed \
    "window $label sampled no allocation sites within ${ACTIVITY_TIMEOUT}s of a real resize. A live sampler at 1-in-$RATE must observe the gallery's resize allocations."
  printf '%s\n' "$W_RUNNING_JSON" >"$RUN_DIR/artifacts/window-$label-running.json"
  run_bytes=$(jq -r '.sampledBytes' <<<"$W_RUNNING_JSON")
  run_blocks=$(jq -r '.sampledBlocks' <<<"$W_RUNNING_JSON")
  run_attempted=$(jq -r '.samplesAttempted' <<<"$W_RUNNING_JSON")
  run_retained=$(jq -r '.samplesRetained' <<<"$W_RUNNING_JSON")
  run_count=$(jq -r '.siteCount' <<<"$W_RUNNING_JSON")
  pass "window $label running: $run_bytes sampled bytes in $run_blocks blocks, $run_attempted attempted / $run_retained retained / $(jq -r '.samplesDropped' <<<"$W_RUNNING_JSON") dropped, $run_count sites"

  step "Window $label: stop sampling"
  cove_call stop_ui_allocation_profile "$TARGET_ONLY"
  if [ "$CALL_STATUS" -ne 0 ]; then
    code=$(call_error_code)
    case "$code" in
      unavailable)
        blocked "stop_ui_allocation_profile reports sampling unavailable: $(call_message)" \
          "The runtime lost the sampler between start and stop; rerun against the native gallery."
        ;;
      no_allocation_profile)
        failed "stop_ui_allocation_profile found no active session after a successful start: $(call_message)"
        ;;
      *)
        allocation_call_fault stop_ui_allocation_profile
        ;;
    esac
  fi
  W_STOP_JSON=$CALL_OUT
  printf '%s\n' "$W_STOP_JSON" >"$RUN_DIR/artifacts/window-$label-stop.json"

  symbolization_reason=$(jq -r '.symbolizationReason // empty' <<<"$W_STOP_JSON")
  if [ -n "$symbolization_reason" ]; then
    blocked "window $label captured sites but cannot resolve function/file:line: $symbolization_reason" \
      "Real caller frames are required for acceptance. macOS debug binaries resolve DWARF symbols; Linux capture has no symbolizer yet and must block instead of passing unresolved addresses."
  fi
  validate_report "$W_STOP_JSON" ready "$RATE" "$MAX_SITES" \
    "window $label stop" 1 1
  stop_bytes=$(jq -r '.sampledBytes' <<<"$W_STOP_JSON")
  stop_blocks=$(jq -r '.sampledBlocks' <<<"$W_STOP_JSON")
  stop_attempted=$(jq -r '.samplesAttempted' <<<"$W_STOP_JSON")
  stop_retained=$(jq -r '.samplesRetained' <<<"$W_STOP_JSON")
  stop_dropped=$(jq -r '.samplesDropped' <<<"$W_STOP_JSON")
  stop_failures=$(jq -r '.captureFailures' <<<"$W_STOP_JSON")
  stop_count=$(jq -r '.siteCount' <<<"$W_STOP_JSON")
  stop_len=$(jq -r '.sites | length' <<<"$W_STOP_JSON")
  stop_truncated=$(jq -r '.sitesTruncated' <<<"$W_STOP_JSON")
  pass "window $label stop: status ready, $stop_len of $stop_count sites (truncated=$stop_truncated), $stop_bytes bytes / $stop_blocks blocks, attempted=$stop_attempted retained=$stop_retained dropped=$stop_dropped failures=$stop_failures"
  pass "window $label frames: $REAL_FRAME_DESC"

  step "Window $label: query the stopped report with limit $SITE_LIMIT"
  cove_call query_ui_allocation_sites "$(query_args "$SITE_LIMIT")"
  if [ "$CALL_STATUS" -ne 0 ]; then
    allocation_call_fault query_ui_allocation_sites
  fi
  W_QUERY_JSON=$CALL_OUT
  printf '%s\n' "$W_QUERY_JSON" >"$RUN_DIR/artifacts/window-$label-query-limit$SITE_LIMIT.json"
  validate_report "$W_QUERY_JSON" ready "$RATE" "$SITE_LIMIT" \
    "window $label query limit $SITE_LIMIT" 1 0

  stop_counters=$(jq -c '{status,rate,sampledBytes,sampledBlocks,samplesAttempted,samplesRetained,samplesDropped,captureFailures,siteCount,sitesTruncated}' <<<"$W_STOP_JSON")
  query_counters=$(jq -c '{status,rate,sampledBytes,sampledBlocks,samplesAttempted,samplesRetained,samplesDropped,captureFailures,siteCount,sitesTruncated}' <<<"$W_QUERY_JSON")
  [ "$stop_counters" = "$query_counters" ] || failed \
    "window $label query after stop does not serve the stopped report: stop $stop_counters vs query $query_counters."
  stop_prefix=$(jq -c --argjson n "$SITE_LIMIT" '.sites[0:$n]' <<<"$W_STOP_JSON")
  query_sites=$(jq -c '.sites' <<<"$W_QUERY_JSON")
  [ "$stop_prefix" = "$query_sites" ] || failed \
    "window $label query limit $SITE_LIMIT did not return the first $SITE_LIMIT sites of the stopped report."
  query_len=$(jq -r '.sites | length' <<<"$W_QUERY_JSON")
  query_truncated=$(jq -r '.sitesTruncated' <<<"$W_QUERY_JSON")
  pass "window $label query: limit $SITE_LIMIT returned $query_len of $stop_count sites (truncated=$query_truncated) with identical counters"

  step "Window $label: query the stopped report with limit 1"
  cove_call query_ui_allocation_sites "$(query_args 1)"
  if [ "$CALL_STATUS" -ne 0 ]; then
    allocation_call_fault query_ui_allocation_sites
  fi
  printf '%s\n' "$CALL_OUT" >"$RUN_DIR/artifacts/window-$label-query-limit1.json"
  validate_report "$CALL_OUT" ready "$RATE" 1 \
    "window $label query limit 1" 1 0
  first_site=$(jq -c '.sites[0] // null' <<<"$W_STOP_JSON")
  query_one=$(jq -c '.sites[0] // null' <<<"$CALL_OUT")
  [ "$first_site" = "$query_one" ] || failed \
    "window $label query limit 1 did not return the first site of the stopped report."
  pass "window $label query: limit 1 returned $(jq -r '.sites | length' <<<"$CALL_OUT") site (truncated=$(jq -r '.sitesTruncated' <<<"$CALL_OUT"))"

  W_STOP_ATTEMPTED=$stop_attempted
}

step "Read the idle state before any sampling starts"
cove_call query_ui_allocation_sites "$(query_args "$SITE_LIMIT")"
if [ "$CALL_STATUS" -ne 0 ]; then
  allocation_call_fault query_ui_allocation_sites
fi
IDLE_JSON=$CALL_OUT
printf '%s\n' "$IDLE_JSON" >"$RUN_DIR/artifacts/allocation-idle.json"
validate_report "$IDLE_JSON" idle 0 "$SITE_LIMIT" "idle query" 0 0
jq -e '
  .sampledBytes == 0
  and .sampledBlocks == 0
  and .samplesAttempted == 0
  and .samplesRetained == 0
  and .samplesDropped == 0
  and .captureFailures == 0
  and .siteCount == 0
  and .sitesTruncated == false
  and (.sites | length) == 0
' >/dev/null 2>&1 <<<"$IDLE_JSON" || failed \
  "the idle query is not empty: $(jq -c '{status,rate,sampledBytes,sampledBlocks,samplesAttempted,samplesRetained,sites:(.sites|length)}' <<<"$IDLE_JSON" 2>/dev/null || printf 'unparseable response')"
pass "empty state: status idle, rate 0, no counters and no sites before the first start"

step "Sampling window 1 of 2"
run_window 1 "$SMALL_WIDTH" "$SMALL_HEIGHT" ""
FIRST_STOP_JSON=$W_STOP_JSON
FIRST_STOP_ATTEMPTED=$W_STOP_ATTEMPTED
FIRST_REAL_FRAME=$REAL_FRAME_DESC

step "Sampling window 2 of 2: a fresh window, not a continuation"
run_window 2 "$BASE_WIDTH" "$BASE_HEIGHT" "$FIRST_STOP_ATTEMPTED"
SECOND_STOP_JSON=$W_STOP_JSON
SECOND_REAL_FRAME=$REAL_FRAME_DESC

SECOND_ATTEMPTED=$(jq -r '.samplesAttempted' <<<"$SECOND_STOP_JSON")
SECOND_BYTES=$(jq -r '.sampledBytes' <<<"$SECOND_STOP_JSON")
SECOND_BLOCKS=$(jq -r '.sampledBlocks' <<<"$SECOND_STOP_JSON")
SECOND_RETAINED=$(jq -r '.samplesRetained' <<<"$SECOND_STOP_JSON")
SECOND_DROPPED=$(jq -r '.samplesDropped' <<<"$SECOND_STOP_JSON")
SECOND_COUNT=$(jq -r '.siteCount' <<<"$SECOND_STOP_JSON")
SECOND_TRUNCATED=$(jq -r '.sitesTruncated' <<<"$SECOND_STOP_JSON")
SECOND_QUERY_LEN=$(jq -r '.sites | length' <<<"$W_QUERY_JSON")
pass "window 2 is independent: start reported zeroed counters, the preview stayed below window 1's attempts, and the post-stop query matches only window 2"

step "Error state: stop with no active session"
cove_call stop_ui_allocation_profile "$TARGET_ONLY"
if [ "$CALL_STATUS" -eq 0 ]; then
  failed "stop_ui_allocation_profile succeeded with no active session: $(jq -c . <<<"$CALL_OUT" 2>/dev/null || printf 'unparseable response')"
fi
ERROR_CODE=$(call_error_code)
[ "$ERROR_CODE" = "no_allocation_profile" ] || failed \
  "a stop without a session returned code '${ERROR_CODE:-none}' instead of no_allocation_profile: $(call_message)"
printf '%s\n' "$CALL_OUT" >"$RUN_DIR/artifacts/stop-without-session.json"
pass "error state: a stop without a session reports no_allocation_profile"

printf '\nRESULT PASS  Allocation sites: sampled caller stacks and fresh sampling windows\n'
printf '  runtime:      %s (%s, pid %s, platform %s)\n' \
  "$RUNTIME_ID" "$RUNTIME_TITLE" "$RUNTIME_PID" "${RUNTIME_PLATFORM:-unknown}"
printf '  rate:         1-in-%s (cumulative sampled allocations, not live heap)\n' "$RATE"
printf '  window 1:     %s bytes / %s blocks, attempted %s, retained %s, dropped %s, %s sites (truncated=%s)\n' \
  "$(jq -r '.sampledBytes' <<<"$FIRST_STOP_JSON")" \
  "$(jq -r '.sampledBlocks' <<<"$FIRST_STOP_JSON")" \
  "$(jq -r '.samplesAttempted' <<<"$FIRST_STOP_JSON")" \
  "$(jq -r '.samplesRetained' <<<"$FIRST_STOP_JSON")" \
  "$(jq -r '.samplesDropped' <<<"$FIRST_STOP_JSON")" \
  "$(jq -r '.siteCount' <<<"$FIRST_STOP_JSON")" \
  "$(jq -r '.sitesTruncated' <<<"$FIRST_STOP_JSON")"
printf '  window 2:     %s bytes / %s blocks, attempted %s, retained %s, dropped %s, %s sites (truncated=%s)\n' \
  "$SECOND_BYTES" "$SECOND_BLOCKS" "$SECOND_ATTEMPTED" "$SECOND_RETAINED" \
  "$SECOND_DROPPED" "$SECOND_COUNT" "$SECOND_TRUNCATED"
printf '  query limits: %s -> %s sites (of %s), 1 -> 1 site\n' \
  "$SITE_LIMIT" "$SECOND_QUERY_LEN" "$SECOND_COUNT"
printf '  evidence:     %s\n' "$SECOND_REAL_FRAME"
printf '  window 1 frame: %s\n' "$FIRST_REAL_FRAME"
printf '  activity:     resize_ui %sx%s then %sx%s\n' \
  "$SMALL_WIDTH" "$SMALL_HEIGHT" "$BASE_WIDTH" "$BASE_HEIGHT"
printf '  checks:       %s\n' "$PASS_COUNT"
exit 0
