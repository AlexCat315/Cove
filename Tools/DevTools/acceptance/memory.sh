#!/usr/bin/env bash
# Memory acceptance for Cove DevTools: process resident memory on the live host.
#
# Starts an isolated controller and the real gallery, waits for the runtime,
# calls query_ui_memory and asserts the existing logical cache counters plus a
# real processResidentBytes reading (and processVirtualBytes >= resident when the
# host also reports virtual memory). A missing provider or missing process
# counters blocks with an actionable message; cache bytes are never accepted in
# place of RSS. Dart heap, GC events and leak tracking are not applicable to a
# Rust host and are not asserted here.
#
# Usage: Tools/DevTools/acceptance/memory.sh [--keep] [--no-build]
#
# Environment:
#   ACCEPTANCE_GALLERY_TIMEOUT  seconds to wait for a registered runtime (default 90)
#
# Exit codes: 0 pass, 1 functional assertion failure, 2 blocked (platform,
# missing product tool, no memory provider, no process counters, build or
# startup failure). Interrupt exits 130/143 after cleanup.
# See Docs/devtools-acceptance.md for the full contract.
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
TITLE="CoveUI — Component Showcase"
KEEP_ARTIFACTS=0
BUILD=1
# Any live process exceeds 1 MiB resident; a smaller "reading" would be a cache
# byte total mislabelled as RSS, so the harness refuses it.
MIN_RESIDENT_BYTES=1048576
CACHE_COUNTERS=(shapingEntries shapingBytes layoutEntries layoutBytes imageEntries imageBytes)
for argument in "$@"; do
  case "$argument" in
    --keep) KEEP_ARTIFACTS=1 ;;
    --no-build) BUILD=0 ;;
    -h|--help)
      sed -n '2,20p' "$0"
      exit 0
      ;;
    *)
      printf 'Unknown argument: %s\n' "$argument" >&2
      exit 2
      ;;
  esac
done

case "${ACCEPTANCE_GALLERY_TIMEOUT:-90}" in
  ''|*[!0-9]*)
    printf 'ACCEPTANCE_GALLERY_TIMEOUT must be a whole number of seconds.\n' >&2
    exit 2
    ;;
esac
GALLERY_TIMEOUT=${ACCEPTANCE_GALLERY_TIMEOUT:-90}

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
[ "$(uname -s)" = "Darwin" ] || blocked \
  "This harness exercises the macOS desktop host's process memory reading." \
  "Run it on macOS. Other hosts need their own platform acceptance run and must report process counters from their own kernel interfaces."

for command in cargo python3 jq; do
  command -v "$command" >/dev/null 2>&1 || blocked \
    "Required command '$command' is not available." \
    "Install '$command' (for jq: brew install jq) and rerun."
done

TMP_ROOT=${TMPDIR:-/tmp}
TMP_ROOT=${TMP_ROOT%/}
[ -n "$TMP_ROOT" ] || TMP_ROOT=/tmp
RUN_DIR=$(mktemp -d "$TMP_ROOT/cove-devtools-memory.XXXXXX") || blocked \
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

tool_exists query_ui_memory || blocked \
  "query_ui_memory is not registered by the controller." \
  "Restore the immediate-read memory tool in cove-agent/cove-devtools and rerun; there is no other way to read host memory counters."
tool_has_property query_ui_memory target || warn \
  "query_ui_memory has no 'target' argument; the harness still selects the runtime through get_ui_session."
if ! unexpected=$(jq -r --arg name "query_ui_memory" \
  '.tools[]? | select(.name == $name) | (.inputSchema.required // [])[] | select(. != "target")' \
  "$RUN_DIR/tools.json"); then
  blocked "Cannot read the query_ui_memory schema." "Inspect tools.json and rerun."
fi
[ -z "$unexpected" ] || blocked \
  "query_ui_memory requires argument(s) this harness does not send: $(printf '%s ' $unexpected)" \
  "Align the harness with the memory tool contract in Docs/devtools-acceptance.md."

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
if [ "$PORT" -eq 9229 ] || [ "$PORT" -lt 1024 ] || [ "$PORT" -gt 65535 ]; then
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

step "Start gallery with COVE_DEVTOOLS=1"
(
  cd "$ROOT"
  export COVE_DEVTOOLS=1
  exec "$GALLERY"
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
      "Open gallery.log. A GUI session, an unlocked display and a working COVE_DEVTOOLS build are required."
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
if [ -z "$RUNTIME_JSON" ]; then
  cove_call get_ui_session '{}'
  if [ "$CALL_STATUS" -eq 0 ]; then
    SESSION_JSON=$CALL_OUT
    RUNTIME_JSON=$(jq -c --arg title "$TITLE" '
      [ .targets[]? | select(.kind == "runtime" and .title == $title) ] | .[0] // empty
    ' <<<"$SESSION_JSON")
  fi
fi
[ -n "$RUNTIME_JSON" ] || blocked \
  "The gallery did not register with the isolated controller within $GALLERY_TIMEOUT seconds." \
  "Check gallery.log; verify the window opened on an unlocked display and COVE_DEVTOOLS_SESSION_FILE reached the app."

RUNTIME_ID=$(jq -r '.id' <<<"$RUNTIME_JSON")
RUNTIME_TITLE=$(jq -r '.title // empty' <<<"$RUNTIME_JSON")
RUNTIME_PID=$(jq -r '.pid // -1' <<<"$RUNTIME_JSON")
RUNTIME_PLATFORM=$(jq -r '.platform // empty' <<<"$RUNTIME_JSON")
[ "$RUNTIME_PLATFORM" = "macos" ] || blocked \
  "The gallery registered as platform '$RUNTIME_PLATFORM' instead of macos." \
  "Run the acceptance harness on the macOS host; another platform needs its own RSS source and run."
pass "runtime selected: $RUNTIME_ID ($RUNTIME_TITLE, pid $RUNTIME_PID)"

step "Call query_ui_memory on the live runtime"
MEMORY_ARGS=$(jq -cn --arg target "$RUNTIME_ID" '{target: $target}')
attempt=0
while :; do
  attempt=$((attempt + 1))
  cove_call query_ui_memory "$MEMORY_ARGS"
  if [ "$CALL_STATUS" -eq 0 ]; then
    break
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
MEMORY_JSON=$CALL_OUT
printf '%s\n' "$MEMORY_JSON" >"$RUN_DIR/artifacts/memory.json"

jq -e '.ok == true' >/dev/null 2>&1 <<<"$MEMORY_JSON" || failed \
  "query_ui_memory did not answer ok=true: $(jq -c . <<<"$MEMORY_JSON" 2>/dev/null || printf 'unparseable response')"
if [ "$(jq -r '.data == null' <<<"$MEMORY_JSON")" = "true" ]; then
  blocked "query_ui_memory returned data: null for runtime $RUNTIME_ID; the host installed no memory provider." \
    "Install the desktop memory provider (crates/hosts/src/desktop/diagnostics/devtools.rs) backed by Application::diagnostic_memory_snapshot and rerun against the macOS gallery debug build. A browser/WASM runtime answers null by design; cache bytes are not a substitute for a provider."
fi
jq -e '(.data.counters | type) == "object"' >/dev/null 2>&1 <<<"$MEMORY_JSON" || failed \
  "query_ui_memory data.counters is missing or not an object."

step "Assert the existing cache and composition counters"
COUNTER_KEYS=$(jq -r '.data.counters | keys | join(", ")' <<<"$MEMORY_JSON")
say "counters: $COUNTER_KEYS"
bad_counters=$(jq -r '
  .data.counters | to_entries[]
  | select((.value | type) != "number" or .value < 0 or .value != (.value | floor))
  | "\(.key)=\(.value | tojson)"
' <<<"$MEMORY_JSON" | head -n 1)
[ -z "$bad_counters" ] || failed \
  "data.counters carries a non-u64 entry ($bad_counters); counters must be non-negative integers."

MISSING_CACHE=""
for key in "${CACHE_COUNTERS[@]}"; do
  jq -e --arg key "$key" '.data.counters | has($key)' >/dev/null 2>&1 <<<"$MEMORY_JSON" \
    || MISSING_CACHE="$MISSING_CACHE $key"
done
[ -z "$MISSING_CACHE" ] || failed \
  "data.counters is missing cache counter(s):$MISSING_CACHE. Present: $COUNTER_KEYS."
jq -e '.data.counters | has("skippedParentUpdates")' >/dev/null 2>&1 <<<"$MEMORY_JSON" \
  || failed "data.counters is missing skippedParentUpdates. Present: $COUNTER_KEYS."
SHAPING_BYTES=$(jq -r '.data.counters.shapingBytes' <<<"$MEMORY_JSON")
LAYOUT_BYTES=$(jq -r '.data.counters.layoutBytes' <<<"$MEMORY_JSON")
IMAGE_BYTES=$(jq -r '.data.counters.imageBytes' <<<"$MEMORY_JSON")
IMAGE_ENTRIES=$(jq -r '.data.counters.imageEntries' <<<"$MEMORY_JSON")
pass "cache counters present: shapingBytes=$SHAPING_BYTES layoutBytes=$LAYOUT_BYTES imageBytes=$IMAGE_BYTES imageEntries=$IMAGE_ENTRIES skippedParentUpdates=$(jq -r '.data.counters.skippedParentUpdates' <<<"$MEMORY_JSON")"

step "Assert render-target estimates"
WINDOW_ENTRIES=$(jq -c '
  def wire_number:
    if type == "number" then .
    elif type == "object" then
      (.bytes // .value // .count // .amount)
      | if type == "number" then . else null end
    else null end;
  def metadata: ["id", "window", "windowID", "window_id", "name", "counters"];
  if (.data.windows | type) == "object" then
    [ .data.windows | to_entries[] as $entry
      | ($entry.value | wire_number) as $number
      | if $number != null then
          ($entry.key | split(".")) as $parts
          | if ($parts | length) > 1 then
              {window: ($parts[0:-1] | join(".")), counter: $parts[-1], value: $number}
            else
              {window: $entry.key, counter: "value", value: $number}
            end
        elif ($entry.value | type) == "object" then
          ($entry.value | to_entries[]
            | select((.key as $key | metadata | index($key)) | not)
            | {window: $entry.key, counter: .key, value: (.value | wire_number)})
        else empty end ]
  elif (.data.windows | type) == "array" then
    [ .data.windows[] | select(type == "object") as $item
      | (($item.counters // $item) | to_entries[]
          | select((.key as $key | metadata | index($key)) | not)
          | {window: ($item.id // $item.window // $item.windowID // $item.window_id // $item.name // "window"),
             counter: .key, value: (.value | wire_number)}) ]
  else [] end
' <<<"$MEMORY_JSON")
if [ "$(jq -r '.data | has("windows")' <<<"$MEMORY_JSON")" != "true" ]; then
  failed "query_ui_memory data has no windows map; render-target estimates are unreported."
fi
WINDOWS_TYPE=$(jq -r '.data.windows | type' <<<"$MEMORY_JSON")
case "$WINDOWS_TYPE" in
  object|array) ;;
  *) failed "data.windows is $WINDOWS_TYPE, not a window counter map or array." ;;
esac
bad_windows=$(jq -r '
  .[] | select(.value == null or .value < 0 or .value != (.value | floor))
  | "\(.window).\(.counter)=\(.value | tojson)"
' <<<"$WINDOW_ENTRIES" | head -n 1)
[ -z "$bad_windows" ] || failed \
  "data.windows carries a non-u64 entry ($bad_windows); window counters must be non-negative integers."
WINDOW_COUNT=$(jq -r 'length' <<<"$WINDOW_ENTRIES")
TARGET_COUNT=$(jq -r '[.[] | select(.counter == "targetBytes")] | length' <<<"$WINDOW_ENTRIES")
if [ "$WINDOW_COUNT" -gt 0 ]; then
  TARGET_TOTAL=$(jq -r '[.[] | select(.counter == "targetBytes") | .value] | add // 0' <<<"$WINDOW_ENTRIES")
  [ "$TARGET_COUNT" -gt 0 ] || failed \
    "data.windows reports $WINDOW_COUNT counter(s) but no targetBytes render-target estimate."
  pass "render-target estimates: $WINDOW_COUNT window counters, targetBytes total $TARGET_TOTAL"
else
  warn "data.windows is empty; this run cannot assert render-target estimates."
fi

step "Assert a real process resident reading"
RESIDENT=$(jq -r '(.data.counters.processResidentBytes // empty)' <<<"$MEMORY_JSON")
if [ -z "$RESIDENT" ] || [ "$RESIDENT" = "null" ]; then
  blocked "query_ui_memory omits counters.processResidentBytes for runtime $RUNTIME_ID. Present counters: $COUNTER_KEYS." \
    "Report processResidentBytes as a real u64 process RSS/physical-footprint reading from the host (macOS proc_pidinfo(PROC_PIDTASKINFO); Linux and Android /proc/self/status VmRSS; Windows GetProcessMemoryInfo WorkingSetSize) in the diagnostic memory snapshot. Do not return 0, do not omit it for a desktop runtime, and never substitute cache bytes. Rerun after the host change."
fi
is_uint "$RESIDENT" || failed \
  "processResidentBytes is not a non-negative integer: '$RESIDENT'."
[ "$RESIDENT" -gt 0 ] || failed \
  "processResidentBytes is 0; zero is not a real resident reading and must be reported as unavailable instead."
if [ "$RESIDENT" -lt "$MIN_RESIDENT_BYTES" ]; then
  failed "processResidentBytes=$RESIDENT is below the $MIN_RESIDENT_BYTES-byte floor every live process exceeds; this looks like a cache-byte total relabelled as RSS."
fi
# Estimated cache bytes live inside the process; a resident reading below them is
# implausible. Estimates can overstate, so this is a warning, not a failure.
CACHE_SUM=$(jq -r '
  [(.data.counters.shapingBytes // 0), (.data.counters.layoutBytes // 0),
   (.data.counters.imageBytes // 0)] | add // 0
' <<<"$MEMORY_JSON")
if [ "$RESIDENT" -le "$CACHE_SUM" ]; then
  warn "processResidentBytes=$RESIDENT is not above the estimated cache bytes ($CACHE_SUM); verify the host measures process memory rather than republishing a cache total."
fi
pass "process resident: $RESIDENT bytes on pid $RUNTIME_PID"

VIRTUAL=$(jq -r '(.data.counters.processVirtualBytes // empty)' <<<"$MEMORY_JSON")
if [ -n "$VIRTUAL" ] && [ "$VIRTUAL" != "null" ]; then
  is_uint "$VIRTUAL" || failed \
    "processVirtualBytes is not a non-negative integer: '$VIRTUAL'."
  [ "$VIRTUAL" -gt 0 ] || failed \
    "processVirtualBytes is 0; zero is not a real virtual-memory reading."
  [ "$VIRTUAL" -ge "$RESIDENT" ] || failed \
    "processVirtualBytes ($VIRTUAL) is below processResidentBytes ($RESIDENT); resident memory cannot exceed the process's virtual address space."
  pass "process virtual: $VIRTUAL bytes (>= resident $RESIDENT)"
else
  say "processVirtualBytes is not reported; the virtual >= resident check is skipped."
fi

# A second read proves the counters are live process readings, not a fixed literal.
sleep 0.2
kill -0 "$CONTROLLER_PID" 2>/dev/null || blocked \
  "The controller exited before the second memory read." \
  "Inspect controller.log; a single answer is not enough to accept a process reading."
cove_call query_ui_memory "$MEMORY_ARGS"
[ "$CALL_STATUS" -eq 0 ] || failed \
  "The second query_ui_memory call failed: $(call_message)"
SECOND_RESIDENT=$(jq -r '(.data.counters.processResidentBytes // empty)' <<<"$CALL_OUT")
is_uint "$SECOND_RESIDENT" && [ "$SECOND_RESIDENT" -gt 0 ] || failed \
  "The second query_ui_memory read has no positive processResidentBytes: '$SECOND_RESIDENT'."
jq -e '.data.counters | has("shapingBytes") and has("layoutBytes") and has("imageBytes")' \
  >/dev/null 2>&1 <<<"$CALL_OUT" || failed \
  "The second query_ui_memory read lost the cache counters."
pass "second read still reports a positive total: resident $SECOND_RESIDENT bytes (first read $RESIDENT bytes)"

printf '\nRESULT PASS  Memory: cache counters, render-target estimates and process RSS\n'
printf '  runtime:     %s (%s, pid %s)\n' "$RUNTIME_ID" "$RUNTIME_TITLE" "$RUNTIME_PID"
printf '  resident:    %s bytes\n' "$RESIDENT"
printf '  virtual:     %s\n' "${VIRTUAL:-not reported}"
printf '  cache bytes: shaping %s, layout %s, image %s\n' \
  "$SHAPING_BYTES" "$LAYOUT_BYTES" "$IMAGE_BYTES"
printf '  windows:     %s counters\n' "$WINDOW_COUNT"
printf '  checks:      %s\n' "$PASS_COUNT"
exit 0
