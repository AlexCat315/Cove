#!/usr/bin/env bash
# Allocation acceptance for Cove DevTools: live allocator counters.
#
# Starts an isolated controller and the real gallery, waits for the runtime,
# reads query_ui_memory and requires an allocator snapshot: a positive
# allocCurrentBytes, allocTotalBytes >= allocCurrentBytes, allocTotalBlocks > 0
# and, when reported, peak bytes/blocks >= the current values. It then asks the
# runtime to resize its window and requires the cumulative allocation counters
# to advance, so a static or cached snapshot cannot pass.
#
# Allocation tracking covers sizes and blocks. It does not name Rust object
# types, retaining paths, GC events or leaks, and this harness asserts none of
# those. Process resident/virtual memory and cache bytes are never accepted in
# place of the allocator counters.
#
# Usage: Tools/DevTools/acceptance/allocation.sh [--keep] [--no-build]
#
# Environment:
#   ACCEPTANCE_GALLERY_TIMEOUT   seconds to wait for a registered runtime (default 120)
#   ACCEPTANCE_ACTIVITY_TIMEOUT  seconds to wait for the counters to advance (default 20)
#
# Exit codes: 0 pass, 1 functional assertion failure, 2 blocked (missing
# dependency, missing product tool, no allocator counters, build or startup
# failure). Interrupt exits 130/143 after cleanup.
# See Docs/devtools-acceptance.md for the full contract.
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
KEEP_ARTIFACTS=0
BUILD=1
# The gallery is started at this size and then resized twice. The sizes are
# inside WindowResize::is_valid's 1..=16384 range and small enough for a laptop
# display; a host that clamps the second resize still allocates new targets.
BASE_WIDTH=1000
BASE_HEIGHT=760
SMALL_WIDTH=780
SMALL_HEIGHT=600
LARGE_WIDTH=1240
LARGE_HEIGHT=940
REQUIRED_ALLOC_COUNTERS=(allocCurrentBytes allocTotalBytes allocTotalBlocks)
for argument in "$@"; do
  case "$argument" in
    --keep) KEEP_ARTIFACTS=1 ;;
    --no-build) BUILD=0 ;;
    -h|--help)
      sed -n '2,25p' "$0"
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
RUN_DIR=$(mktemp -d "$TMP_ROOT/cove-devtools-allocation.XXXXXX") || blocked \
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
tool_required() {
  jq -r --arg name "$1" \
    '.tools[]? | select(.name == $name) | (.inputSchema.required // []) | join(" ")' \
    "$RUN_DIR/tools.json"
}

tool_exists query_ui_memory || blocked \
  "query_ui_memory is not registered by the controller." \
  "Restore the immediate-read memory tool in cove-agent/cove-devtools and rerun; there is no other way to read host allocation counters."
UNEXPECTED_REQUIRED=$(tool_required query_ui_memory \
  | tr ' ' '\n' | grep -v '^target$' | grep -v '^$' | tr '\n' ' ' || true)
[ -z "$UNEXPECTED_REQUIRED" ] || blocked \
  "query_ui_memory requires argument(s) this harness does not send: $UNEXPECTED_REQUIRED" \
  "Align the harness with the memory tool contract in Docs/devtools-acceptance.md."

tool_exists resize_ui || blocked \
  "resize_ui is not registered by the controller; the harness cannot prove the allocation counters are live." \
  "Restore resize_ui and run it against the desktop gallery. A static snapshot is never accepted as proof of allocation tracking."
UNEXPECTED_REQUIRED=$(tool_required resize_ui \
  | tr ' ' '\n' | grep -v '^target$' | grep -v '^width$' | grep -v '^height$' | grep -v '^$' \
  | tr '\n' ' ' || true)
[ -z "$UNEXPECTED_REQUIRED" ] || blocked \
  "resize_ui requires argument(s) this harness does not send: $UNEXPECTED_REQUIRED" \
  "Align the harness with the resize tool contract in Docs/devtools-acceptance.md."

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
pass "runtime selected: $RUNTIME_ID ($RUNTIME_TITLE, pid $RUNTIME_PID)"

step "Call query_ui_memory on the live runtime"
MEMORY_ARGS=$(jq -cn --arg target "$RUNTIME_ID" '{target: $target}')
read_memory() {
  attempt=0
  while :; do
    attempt=$((attempt + 1))
    cove_call query_ui_memory "$MEMORY_ARGS"
    if [ "$CALL_STATUS" -eq 0 ]; then
      return 0
    fi
    if ! kill -0 "$CONTROLLER_PID" 2>/dev/null; then
      blocked "The controller exited while query_ui_memory was running." \
        "Inspect controller.log; the run cannot read a live snapshot from a dead controller."
    fi
    code=$(call_error_code)
    case "$code" in
      unavailable)
        blocked "query_ui_memory reports the memory read unavailable: $(call_message)" \
          "The host must serve memory counters through its diagnostics probe; check the desktop diagnostics provider and rerun. No fallback reading is invented."
        ;;
      target_gone|target_required|target_unbound)
        blocked "query_ui_memory cannot select the runtime (code ${code:-unknown}): $(call_message)" \
          "Re-read get_ui_session and pass the gallery runtime id; the runtime may have disconnected."
        ;;
      *)
        if [ "$attempt" -ge 3 ]; then
          failed "query_ui_memory failed (code ${code:-unknown}): $(call_message)"
        fi
        sleep 0.3
        ;;
    esac
  done
}

read_memory
MEMORY_JSON=$CALL_OUT
printf '%s\n' "$MEMORY_JSON" >"$RUN_DIR/artifacts/memory-baseline.json"

jq -e '.ok == true' >/dev/null 2>&1 <<<"$MEMORY_JSON" || failed \
  "query_ui_memory did not answer ok=true: $(jq -c . <<<"$MEMORY_JSON" 2>/dev/null || printf 'unparseable response')"
if [ "$(jq -r '.data == null' <<<"$MEMORY_JSON")" = "true" ]; then
  blocked "query_ui_memory returned data: null for runtime $RUNTIME_ID; the host installed no memory provider." \
    "Install the desktop memory provider (crates/hosts/src/desktop/diagnostics/devtools.rs) backed by Application::diagnostic_memory_snapshot and rerun against the desktop gallery. Without a provider there is no allocator snapshot to assert."
fi
jq -e '(.data.counters | type) == "object"' >/dev/null 2>&1 <<<"$MEMORY_JSON" || failed \
  "query_ui_memory data.counters is missing or not an object."

step "Assert the live allocation counters"
COUNTER_KEYS=$(jq -r '.data.counters | keys | join(", ")' <<<"$MEMORY_JSON")
say "counters: $COUNTER_KEYS"
bad_counters=$(jq -r '
  .data.counters | to_entries[]
  | select((.value | type) != "number" or .value < 0 or .value != (.value | floor))
  | "\(.key)=\(.value | tojson)"
' <<<"$MEMORY_JSON" | head -n 1)
[ -z "$bad_counters" ] || failed \
  "data.counters carries a non-u64 entry ($bad_counters); counters must be non-negative integers."

MISSING_ALLOC=""
for key in "${REQUIRED_ALLOC_COUNTERS[@]}"; do
  jq -e --arg key "$key" '.data.counters | has($key)' >/dev/null 2>&1 <<<"$MEMORY_JSON" \
    || MISSING_ALLOC="$MISSING_ALLOC $key"
done
if [ -n "$MISSING_ALLOC" ]; then
  blocked "query_ui_memory has no allocator snapshot for runtime $RUNTIME_ID (missing:$MISSING_ALLOC). Present counters: $COUNTER_KEYS." \
    "Install the tracking allocator for the gallery process (the backend allocator crate as the global allocator, reporting allocCurrentBytes, allocTotalBytes and allocTotalBlocks in DiagnosticMemorySnapshot) and rerun. processResidentBytes, cache bytes and render-target bytes are NOT accepted as allocation tracking; without the allocator counters this run stays blocked."
fi

CURRENT_BYTES=$(jq -r '.data.counters.allocCurrentBytes' <<<"$MEMORY_JSON")
TOTAL_BYTES=$(jq -r '.data.counters.allocTotalBytes' <<<"$MEMORY_JSON")
TOTAL_BLOCKS=$(jq -r '.data.counters.allocTotalBlocks' <<<"$MEMORY_JSON")
for pair in "allocCurrentBytes=$CURRENT_BYTES" "allocTotalBytes=$TOTAL_BYTES" \
  "allocTotalBlocks=$TOTAL_BLOCKS"; do
  value=${pair#*=}
  is_uint "$value" || failed "${pair%%=*} is not a non-negative integer: '$value'."
done
[ "$CURRENT_BYTES" -gt 0 ] || failed \
  "allocCurrentBytes is 0; a live gallery process holds allocated bytes, so zero means the allocator snapshot is not tracking (or must be omitted as unavailable, never published as 0)."
[ "$TOTAL_BLOCKS" -gt 0 ] || failed \
  "allocTotalBlocks is 0; the allocator reports no allocation calls at all."
[ "$TOTAL_BYTES" -ge "$CURRENT_BYTES" ] || failed \
  "allocTotalBytes ($TOTAL_BYTES) is below allocCurrentBytes ($CURRENT_BYTES); cumulative allocated bytes cannot be less than the live bytes."
pass "allocator snapshot: current $CURRENT_BYTES bytes, total $TOTAL_BYTES bytes in $TOTAL_BLOCKS blocks"

CURRENT_BLOCKS=$(jq -r '.data.counters.allocLiveBlocks // .data.counters.allocCurrentBlocks // empty' <<<"$MEMORY_JSON")
if [ -n "$CURRENT_BLOCKS" ]; then
  is_uint "$CURRENT_BLOCKS" || failed \
    "allocLiveBlocks is not a non-negative integer: '$CURRENT_BLOCKS'."
  [ "$CURRENT_BLOCKS" -le "$TOTAL_BLOCKS" ] || failed \
    "allocLiveBlocks ($CURRENT_BLOCKS) exceeds allocTotalBlocks ($TOTAL_BLOCKS); live blocks cannot exceed the cumulative count."
  say "live blocks: $CURRENT_BLOCKS (of $TOTAL_BLOCKS cumulative)"
else
  say "allocLiveBlocks is not reported; the live block count is not asserted."
fi

# Every reported peak counter must be at least the current reading. Peak names
# are matched by prefix so a host may report bytes, blocks or both.
PEAK_KEYS=$(jq -r '.data.counters | keys[] | select(test("^allocPeak.*Bytes$"))' <<<"$MEMORY_JSON")
for key in $PEAK_KEYS; do
  value=$(jq -r --arg key "$key" '.data.counters[$key]' <<<"$MEMORY_JSON")
  is_uint "$value" || failed "$key is not a non-negative integer: '$value'."
  [ "$value" -ge "$CURRENT_BYTES" ] || failed \
    "$key ($value) is below allocCurrentBytes ($CURRENT_BYTES); a peak cannot be lower than the current reading."
  [ "$value" -le "$TOTAL_BYTES" ] || warn \
    "$key ($value) is above allocTotalBytes ($TOTAL_BYTES); verify the peak and total counters measure the same allocator events."
done
PEAK_BLOCK_KEYS=$(jq -r '.data.counters | keys[] | select(test("^allocPeak.*Blocks$"))' <<<"$MEMORY_JSON")
for key in $PEAK_BLOCK_KEYS; do
  value=$(jq -r --arg key "$key" '.data.counters[$key]' <<<"$MEMORY_JSON")
  is_uint "$value" || failed "$key is not a non-negative integer: '$value'."
  if [ -n "$CURRENT_BLOCKS" ]; then
    [ "$value" -ge "$CURRENT_BLOCKS" ] || failed \
      "$key ($value) is below allocLiveBlocks ($CURRENT_BLOCKS); a peak cannot be lower than the current reading."
  fi
done
if [ -n "$PEAK_KEYS$PEAK_BLOCK_KEYS" ]; then
  pass "peak counters reported: $(printf '%s ' $PEAK_KEYS $PEAK_BLOCK_KEYS)(>= current)"
else
  say "No allocPeak*Bytes/Blocks counter is reported; the peak check is skipped."
fi

step "Drive real activity: resize the gallery window to ${SMALL_WIDTH}x${SMALL_HEIGHT}"
# Probe tools answer the raw payload (`{"requested":true}`), while immediate
# tools such as query_ui_memory wrap it in `{ok,data}`. Accept both shapes.
probe_ok() {
  jq -e 'if has("ok") then .ok == true else true end' >/dev/null 2>&1 <<<"$CALL_OUT"
}
probe_data() {
  jq -c 'if has("ok") then (.data // {}) else . end' <<<"$CALL_OUT" 2>/dev/null || printf '{}'
}
RESIZE_ARGS=$(jq -cn --arg target "$RUNTIME_ID" --argjson width "$SMALL_WIDTH" \
  --argjson height "$SMALL_HEIGHT" '{target: $target, width: $width, height: $height}')
cove_call resize_ui "$RESIZE_ARGS"
if [ "$CALL_STATUS" -ne 0 ]; then
  code=$(call_error_code)
  case "$code" in
    unavailable)
      blocked "resize_ui is unavailable on this runtime: $(call_message)" \
        "The allocation counters cannot be proven live without a real operation. Run the harness against the desktop gallery, which exposes a resize callback."
      ;;
    target_gone|target_required|target_unbound)
      blocked "resize_ui cannot select the runtime (code ${code:-unknown}): $(call_message)" \
        "The runtime disconnected between the memory read and the resize; rerun and inspect gallery.log if it repeats."
      ;;
    *)
      failed "resize_ui to ${SMALL_WIDTH}x${SMALL_HEIGHT} failed (code ${code:-unknown}): $(call_message)"
      ;;
  esac
fi
probe_ok || failed \
  "resize_ui did not answer ok=true: $(jq -c . <<<"$CALL_OUT" 2>/dev/null || printf 'unparseable response')"
if [ "$(jq -r '.requested // empty' <<<"$(probe_data)")" != "true" ]; then
  warn "resize_ui did not echo requested=true; the resize may still have been queued, so the counter growth below is the evidence."
fi

# Poll until both cumulative counters advance. A single static answer cannot
# pass; querying alone allocates on the host, so a live counter always moves.
GROWTH_JSON=""
GROWTH_TOTAL_BYTES=$TOTAL_BYTES
GROWTH_TOTAL_BLOCKS=$TOTAL_BLOCKS
deadline=$(( $(date +%s) + ACTIVITY_TIMEOUT ))
while [ "$(date +%s)" -lt "$deadline" ]; do
  if ! kill -0 "$CONTROLLER_PID" 2>/dev/null; then
    blocked "The controller exited while waiting for the allocation counters to advance." \
      "Inspect controller.log; a dead controller cannot prove live counters."
  fi
  read_memory
  bytes=$(jq -r '.data.counters.allocTotalBytes // empty' <<<"$CALL_OUT")
  blocks=$(jq -r '.data.counters.allocTotalBlocks // empty' <<<"$CALL_OUT")
  is_uint "$bytes" && is_uint "$blocks" || failed \
    "A follow-up query_ui_memory lost the allocation counters (allocTotalBytes='$bytes', allocTotalBlocks='$blocks')."
  GROWTH_TOTAL_BYTES=$bytes
  GROWTH_TOTAL_BLOCKS=$blocks
  if [ "$bytes" -gt "$TOTAL_BYTES" ] && [ "$blocks" -gt "$TOTAL_BLOCKS" ]; then
    GROWTH_JSON=$CALL_OUT
    break
  fi
  sleep 0.25
done

if [ -z "$GROWTH_JSON" ]; then
  failed "allocTotalBytes/allocTotalBlocks did not advance within ${ACTIVITY_TIMEOUT}s after a real resize (bytes $TOTAL_BYTES -> $GROWTH_TOTAL_BYTES, blocks $TOTAL_BLOCKS -> $GROWTH_TOTAL_BLOCKS). Allocation counters that never move are static snapshots, not live allocator readings."
fi
printf '%s\n' "$GROWTH_JSON" >"$RUN_DIR/artifacts/memory-after-resize.json"
AFTER_BYTES=$(jq -r '.data.counters.allocTotalBytes' <<<"$GROWTH_JSON")
AFTER_BLOCKS=$(jq -r '.data.counters.allocTotalBlocks' <<<"$GROWTH_JSON")
AFTER_CURRENT=$(jq -r '.data.counters.allocCurrentBytes' <<<"$GROWTH_JSON")
is_uint "$AFTER_CURRENT" || failed \
  "The post-resize read has no integer allocCurrentBytes: '$AFTER_CURRENT'."
[ "$AFTER_CURRENT" -gt 0 ] || failed \
  "The post-resize read reports allocCurrentBytes=0; a live runtime cannot have no allocated bytes."
[ "$AFTER_BYTES" -ge "$AFTER_CURRENT" ] || failed \
  "The post-resize read has allocTotalBytes ($AFTER_BYTES) below allocCurrentBytes ($AFTER_CURRENT)."
pass "allocation counters advanced after resize_ui: bytes $TOTAL_BYTES -> $AFTER_BYTES (+$((AFTER_BYTES - TOTAL_BYTES))), blocks $TOTAL_BLOCKS -> $AFTER_BLOCKS (+$((AFTER_BLOCKS - TOTAL_BLOCKS)))"

step "Drive a second resize to ${LARGE_WIDTH}x${LARGE_HEIGHT} and require monotonic counters"
RESIZE_ARGS=$(jq -cn --arg target "$RUNTIME_ID" --argjson width "$LARGE_WIDTH" \
  --argjson height "$LARGE_HEIGHT" '{target: $target, width: $width, height: $height}')
cove_call resize_ui "$RESIZE_ARGS"
if [ "$CALL_STATUS" -ne 0 ]; then
  code=$(call_error_code)
  failed "The second resize_ui failed (code ${code:-unknown}): $(call_message)"
fi
probe_ok || failed \
  "The second resize_ui did not answer ok=true: $(jq -c . <<<"$CALL_OUT" 2>/dev/null || printf 'unparseable response')"
sleep 0.5
read_memory
FINAL_JSON=$CALL_OUT
printf '%s\n' "$FINAL_JSON" >"$RUN_DIR/artifacts/memory-final.json"
FINAL_CURRENT=$(jq -r '.data.counters.allocCurrentBytes // empty' <<<"$FINAL_JSON")
FINAL_BYTES=$(jq -r '.data.counters.allocTotalBytes // empty' <<<"$FINAL_JSON")
FINAL_BLOCKS=$(jq -r '.data.counters.allocTotalBlocks // empty' <<<"$FINAL_JSON")
is_uint "$FINAL_CURRENT" && [ "$FINAL_CURRENT" -gt 0 ] || failed \
  "The final read has no positive allocCurrentBytes: '$FINAL_CURRENT'."
is_uint "$FINAL_BYTES" || failed "The final read has no integer allocTotalBytes: '$FINAL_BYTES'."
is_uint "$FINAL_BLOCKS" || failed "The final read has no integer allocTotalBlocks: '$FINAL_BLOCKS'."
[ "$FINAL_BYTES" -ge "$AFTER_BYTES" ] || failed \
  "allocTotalBytes decreased ($AFTER_BYTES -> $FINAL_BYTES); cumulative allocation totals must be monotonic."
[ "$FINAL_BLOCKS" -ge "$AFTER_BLOCKS" ] || failed \
  "allocTotalBlocks decreased ($AFTER_BLOCKS -> $FINAL_BLOCKS); cumulative allocation totals must be monotonic."
[ "$FINAL_BYTES" -ge "$FINAL_CURRENT" ] || failed \
  "The final read has allocTotalBytes ($FINAL_BYTES) below allocCurrentBytes ($FINAL_CURRENT)."
pass "counters stayed cumulative after the second resize: total $FINAL_BYTES bytes, $FINAL_BLOCKS blocks"

printf '\nRESULT PASS  Allocation: live allocator size and block tracking\n'
printf '  runtime:      %s (%s, pid %s)\n' "$RUNTIME_ID" "$RUNTIME_TITLE" "$RUNTIME_PID"
printf '  current:      %s bytes after the second resize (baseline %s)\n' \
  "$FINAL_CURRENT" "$CURRENT_BYTES"
printf '  totals:       %s bytes in %s blocks (baseline %s / %s)\n' \
  "$FINAL_BYTES" "$FINAL_BLOCKS" "$TOTAL_BYTES" "$TOTAL_BLOCKS"
printf '  peak counters: %s\n' "${PEAK_KEYS:-not reported}${PEAK_BLOCK_KEYS:+ $PEAK_BLOCK_KEYS}"
printf '  activity:     resize_ui %sx%s then %sx%s\n' \
  "$SMALL_WIDTH" "$SMALL_HEIGHT" "$LARGE_WIDTH" "$LARGE_HEIGHT"
printf '  checks:       %s\n' "$PASS_COUNT"
exit 0
