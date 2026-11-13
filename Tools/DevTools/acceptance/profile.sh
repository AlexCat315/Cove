#!/usr/bin/env bash
# Milestone 3 CPU profiler acceptance for Cove DevTools on macOS.
#
# Starts an isolated controller and the real gallery, then drives the real
# in-process sampler through the public controller tools: a natural-length
# profile and an early-stopped longer profile. Every number asserted here comes
# from the live UI thread; nothing is synthesized or replayed.
#
# Builds with cargo, then launches the resulting binaries directly. They are
# the same targets `cargo run` would launch; direct PIDs make cleanup exact.
#
# Usage: Tools/DevTools/acceptance/profile.sh [--keep] [--no-build]
#
# Environment:
#   ACCEPTANCE_PROFILE_TIMEOUT  seconds to wait for a ready profile (default 30)
#   ACCEPTANCE_UI_THREAD        expected gallery UI thread name (default cove-ui)
#
# Exit codes: 0 pass, 1 functional assertion failure, 2 blocked (platform,
# missing product tool, unsupported sampler, build or startup failure).
# Interrupt exits 130/143 after cleanup. See Docs/devtools-acceptance.md.
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
TITLE="CoveUI — Component Showcase"
HZ=500
DURATION_MS=1500
EARLY_DURATION_MS=5000
UI_THREAD=${ACCEPTANCE_UI_THREAD:-cove-ui}
PROFILE_TIMEOUT=${ACCEPTANCE_PROFILE_TIMEOUT:-30}
KEEP_ARTIFACTS=0
BUILD=1
for argument in "$@"; do
  case "$argument" in
    --keep) KEEP_ARTIFACTS=1 ;;
    --no-build) BUILD=0 ;;
    -h|--help)
      sed -n '2,19p' "$0"
      exit 0
      ;;
    *)
      printf 'Unknown argument: %s\n' "$argument" >&2
      exit 2
      ;;
  esac
done

case "$PROFILE_TIMEOUT" in
  ''|*[!0-9]*)
    printf 'ACCEPTANCE_PROFILE_TIMEOUT must be a whole number of seconds.\n' >&2
    exit 2
    ;;
esac
if [ "$PROFILE_TIMEOUT" -lt 5 ]; then
  printf 'ACCEPTANCE_PROFILE_TIMEOUT must be at least 5 seconds.\n' >&2
  exit 2
fi
case "${ACCEPTANCE_GALLERY_TIMEOUT:-90}" in
  ''|*[!0-9]*)
    printf 'ACCEPTANCE_GALLERY_TIMEOUT must be a whole number of seconds.\n' >&2
    exit 2
    ;;
esac

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

is_number() {
  [[ "$1" =~ ^-?([0-9]+([.][0-9]*)?|[.][0-9]+)([eE][-+]?[0-9]+)?$ ]]
}
is_uint() { [[ "$1" =~ ^[0-9]+$ ]]; }

step "Preflight"
[ "$(uname -s)" = "Darwin" ] || blocked \
  "This harness exercises the macOS in-process CPU sampler." \
  "Run it on macOS. Other hosts must report cpu_profile as unavailable instead of samples."

ARCH=$(uname -m)
case "$ARCH" in
  arm64|x86_64) ;;
  *)
    blocked "The macOS sampler is compiled for arm64 and x86_64, not '$ARCH'." \
      "Run the acceptance harness on a supported macOS architecture."
    ;;
esac

for command in cargo python3 jq; do
  command -v "$command" >/dev/null 2>&1 || blocked \
    "Required command '$command' is not available." \
    "Install '$command' (for jq: brew install jq) and rerun."
done

TMP_ROOT=${TMPDIR:-/tmp}
TMP_ROOT=${TMP_ROOT%/}
[ -n "$TMP_ROOT" ] || TMP_ROOT=/tmp
RUN_DIR=$(mktemp -d "$TMP_ROOT/cove-devtools-m3.XXXXXX") || blocked \
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

for name in start_ui_profile stop_ui_profile get_ui_profile query_ui_hotspots; do
  tool_exists "$name" || blocked \
    "$name is not registered by the controller (CPU profiler backend tool missing)." \
    "Implement the profiler tools in cove-agent/cove-devtools-cli and rerun. The sampler cannot be exercised through this harness without them."
  tool_has_property "$name" target || blocked \
    "$name has no 'target' argument, so the harness cannot select the gallery runtime." \
    "Restore the target argument on the profiling tools and rerun."
done
tool_requires start_ui_profile hz || blocked \
  "start_ui_profile does not require hz." \
  "Restore the {target?, hz, duration_ms} contract and rerun."
tool_requires start_ui_profile duration_ms || blocked \
  "start_ui_profile does not require duration_ms." \
  "Restore the {target?, hz, duration_ms} contract and rerun."

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
# The harness stops both processes by PID; disowning keeps bash from printing
# a job-control termination notice while the EXIT trap cleans up.
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
deadline=$(( $(date +%s) + ${ACCEPTANCE_GALLERY_TIMEOUT:-90} ))
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
  "The gallery did not register with the isolated controller within ${ACCEPTANCE_GALLERY_TIMEOUT:-90} seconds." \
  "Check gallery.log; verify the window opened on an unlocked display and COVE_DEVTOOLS_SESSION_FILE reached the app."

RUNTIME_ID=$(jq -r '.id' <<<"$RUNTIME_JSON")
RUNTIME_TITLE=$(jq -r '.title // empty' <<<"$RUNTIME_JSON")
RUNTIME_PID=$(jq -r '.pid // -1' <<<"$RUNTIME_JSON")
RUNTIME_PLATFORM=$(jq -r '.platform // empty' <<<"$RUNTIME_JSON")
RUNTIME_CAPS=$(jq -r '(.capabilities // []) | join(",")' <<<"$RUNTIME_JSON")
pass "runtime selected: $RUNTIME_ID ($RUNTIME_TITLE, pid $RUNTIME_PID)"

step "Require the runtime's real CPU sampler"
[ "$RUNTIME_PLATFORM" = "macos" ] || blocked \
  "The gallery registered as platform '$RUNTIME_PLATFORM' instead of macos." \
  "Run the acceptance harness on the macOS host; other platforms must report the sampler as unavailable."
case ",$RUNTIME_CAPS," in
  *,cpu_profile,*) ;;
  *)
    blocked "The gallery runtime does not advertise 'cpu_profile' (capabilities: ${RUNTIME_CAPS:-none})." \
      "The macOS sampler is unavailable to this build. Run 'cargo test -p cove-devtools --test profile_tests' and check that target/debug/gallery is a native $ARCH binary (not a browser build). Acceptance never passes without a real sampler."
    ;;
esac
cove_call get_ui_session '{}'
[ "$CALL_STATUS" -eq 0 ] || blocked \
  "get_ui_session failed after the runtime registered: $(call_message)" \
  "Inspect controller.log; the controller may have shut down."
pass "runtime advertises cpu_profile (platform $RUNTIME_PLATFORM, $ARCH)"

PROFILE_ARGS=$(jq -cn --arg target "$RUNTIME_ID" '{target: $target}')

step "Start a $DURATION_MS ms profile at $HZ Hz"
start_args=$(jq -cn --arg target "$RUNTIME_ID" --argjson hz "$HZ" --argjson duration "$DURATION_MS" \
  '{target: $target, hz: $hz, duration_ms: $duration}')
cove_call start_ui_profile "$start_args"
if [ "$CALL_STATUS" -ne 0 ]; then
  code=$(call_error_code)
  case "$code" in
    unavailable)
      blocked "start_ui_profile reports the sampler unavailable: $(call_message)" \
        "This macOS build has no usable sampler for the gallery UI thread. Check crates/profiling/src/platform and the target architecture, then rerun."
      ;;
    busy)
      failed "start_ui_profile is busy before any profile ran in this isolated gallery: $(call_message)"
      ;;
    invalid_request)
      failed "start_ui_profile rejected hz=$HZ duration_ms=$DURATION_MS: $(call_message)"
      ;;
    *)
      failed "start_ui_profile failed (code ${code:-unknown}): $(call_message)"
      ;;
  esac
fi
START_JSON=$CALL_OUT
START_STATUS=$(jq -r '.status // empty' <<<"$START_JSON")
START_THREAD=$(jq -r '.thread // empty' <<<"$START_JSON")
START_HZ=$(jq -r '.hz // empty' <<<"$START_JSON")
[ "$START_STATUS" = "running" ] || failed \
  "start_ui_profile returned status '$START_STATUS' instead of running."
[ "$START_HZ" = "$HZ" ] || failed \
  "start_ui_profile echoed rate '$START_HZ' instead of $HZ."
[ -n "$START_THREAD" ] || failed "start_ui_profile returned an empty thread name."
[ "$START_THREAD" = "$UI_THREAD" ] || failed \
  "The sampler follows thread '$START_THREAD', not the gallery UI thread '$UI_THREAD'."
jq -e '(.stacks | type) == "array" and (.stacks | length) == 0' >/dev/null 2>&1 <<<"$START_JSON" \
  || failed "While running, start_ui_profile must not publish stacks."
jq -e '(.samplesDropped | type) == "number"' >/dev/null 2>&1 <<<"$START_JSON" \
  || failed "start_ui_profile returned a non-numeric samplesDropped."
pass "sampling started: thread=$START_THREAD hz=$START_HZ, stacks empty while running"

step "Poll get_ui_profile until status ready"
profile_ready=0
seen_running=0
deadline=$(( $(date +%s) + PROFILE_TIMEOUT ))
while [ "$(date +%s)" -lt "$deadline" ]; do
  cove_call get_ui_profile "$PROFILE_ARGS"
  if [ "$CALL_STATUS" -ne 0 ]; then
    code=$(call_error_code)
    case "$code" in
      unavailable)
        blocked "get_ui_profile reports the sampler unavailable: $(call_message)" \
          "Check the macOS sampler and rerun."
        ;;
      *)
        failed "get_ui_profile failed (code ${code:-unknown}): $(call_message)"
        ;;
    esac
  fi
  status=$(jq -r '.status // empty' <<<"$CALL_OUT")
  case "$status" in
    ready)
      profile_ready=1
      break
      ;;
    running)
      seen_running=1
      ;;
    idle)
      failed "get_ui_profile returned to idle before publishing a ready profile."
      ;;
    *)
      failed "get_ui_profile returned an unexpected status '$status'."
      ;;
  esac
  sleep 0.1
done
[ "$profile_ready" -eq 1 ] || failed \
  "get_ui_profile did not reach status ready within $PROFILE_TIMEOUT seconds."
[ "$seen_running" -eq 1 ] || warn \
  "The profile was already ready on the first read; no running status was observed."
PROFILE_JSON=$CALL_OUT
printf '%s\n' "$PROFILE_JSON" >"$RUN_DIR/artifacts/profile-${DURATION_MS}ms.json"

step "Assert the natural-length profile carries real samples"
require_num() {
  key=$1
  jq -e --arg key "$key" 'has($key) and (.[$key] | type) == "number"' \
    >/dev/null 2>&1 <<<"$PROFILE_JSON" || failed "Profile field '$key' is absent or not numeric."
}
require_uint() {
  key=$1
  value=$(jq -r --arg key "$key" '.[$key] // empty' <<<"$PROFILE_JSON")
  is_uint "$value" || failed "Profile field '$key' is not a non-negative integer: '$value'."
}
jq -e '(.thread | type) == "string" and (.thread | length) > 0' \
  >/dev/null 2>&1 <<<"$PROFILE_JSON" || failed "Profile field 'thread' is absent or empty."
require_num hz
require_num durationMs
require_num sampleIntervalMs
require_num samplesTaken
require_num samplesDropped
require_num retainedSamples
require_num samplesPerSecond
require_uint samplesTaken
require_uint samplesDropped
require_uint retainedSamples

P_STATUS=$(jq -r '.status // empty' <<<"$PROFILE_JSON")
P_THREAD=$(jq -r '.thread // empty' <<<"$PROFILE_JSON")
P_HZ=$(jq -r '.hz // empty' <<<"$PROFILE_JSON")
P_DURATION=$(jq -r '.durationMs // empty' <<<"$PROFILE_JSON")
P_INTERVAL=$(jq -r '.sampleIntervalMs // empty' <<<"$PROFILE_JSON")
P_TAKEN=$(jq -r '.samplesTaken // empty' <<<"$PROFILE_JSON")
P_DROPPED=$(jq -r '.samplesDropped // empty' <<<"$PROFILE_JSON")
P_RETAINED=$(jq -r '.retainedSamples // empty' <<<"$PROFILE_JSON")
P_PER_SECOND=$(jq -r '.samplesPerSecond // empty' <<<"$PROFILE_JSON")
P_STACKS=$(jq -r '.stacks | length' <<<"$PROFILE_JSON")
P_TRUNCATED=$(jq -r '.stacksTruncated // false' <<<"$PROFILE_JSON")

[ "$P_STATUS" = "ready" ] || failed "The finished profile reports status '$P_STATUS' instead of ready."
[ "$P_THREAD" = "$UI_THREAD" ] || failed \
  "The finished profile follows thread '$P_THREAD', not the gallery UI thread '$UI_THREAD'."
[ "$P_HZ" = "$HZ" ] || failed "The finished profile reports hz=$P_HZ instead of $HZ."
[ "$P_TAKEN" -gt 0 ] || failed \
  "The sampler took zero samples; this is not a profile. Inspect the UI thread name and the sampler."
[ "$P_RETAINED" -gt 0 ] || failed "The sampler retained zero samples."
[ "$P_RETAINED" -le "$P_TAKEN" ] || failed \
  "retainedSamples $P_RETAINED exceeds samplesTaken $P_TAKEN."
is_number "$P_PER_SECOND" && awk -v v="$P_PER_SECOND" 'BEGIN { exit !(v > 0) }' \
  || failed "samplesPerSecond is not positive: '$P_PER_SECOND'."
is_number "$P_DURATION" && awk -v v="$P_DURATION" 'BEGIN { exit !(v > 0) }' \
  || failed "durationMs is not positive: '$P_DURATION'."
is_number "$P_INTERVAL" && awk -v v="$P_INTERVAL" 'BEGIN { exit !(v > 0) }' \
  || failed "sampleIntervalMs is not positive: '$P_INTERVAL'."
is_uint "$P_STACKS" || failed "The profile's stack count is not numeric."
[ "$P_STACKS" -gt 0 ] || failed "The ready profile has no aggregated stacks."

jq -e '
  all(.stacks[];
    ((.samples | type) == "number" and .samples > 0)
    and ((.percent | type) == "number" and .percent >= 0 and .percent <= 100.0001)
    and ((.frames | type) == "array" and (.frames | length) > 0)
    and all(.frames[]; ((.label | type) == "string" and (.label | length) > 0))
  )
' >/dev/null 2>&1 <<<"$PROFILE_JSON" || failed \
  "Aggregated stacks are malformed (need samples>0, percent 0..100, non-empty labelled frames)."
STACK_SAMPLES=$(jq '[.stacks[].samples] | add' <<<"$PROFILE_JSON")
[ "$STACK_SAMPLES" -le "$P_RETAINED" ] || failed \
  "Stack sample counts ($STACK_SAMPLES) exceed retained samples ($P_RETAINED)."
TOP_FRAME=$(jq -r '.stacks[0].frames[0].label // empty' <<<"$PROFILE_JSON")
pass "profile ready: $P_TAKEN samples in ${P_DURATION} ms (${P_PER_SECOND}/s, $P_DROPPED dropped, $P_RETAINED retained, $P_STACKS stacks)"
pass "real UI thread '$P_THREAD' sampled at $P_HZ Hz (interval ${P_INTERVAL} ms); top frame '$TOP_FRAME'"
if [ "$P_TRUNCATED" = "true" ]; then
  warn "The wire truncated the stack list; totals still cover every retained sample."
fi

step "Query hotspots"
hotspot_args=$(jq -cn --arg target "$RUNTIME_ID" '{target: $target, limit: 200}')
cove_call query_ui_hotspots "$hotspot_args"
if [ "$CALL_STATUS" -ne 0 ]; then
  code=$(call_error_code)
  case "$code" in
    unavailable)
      blocked "query_ui_hotspots reports the sampler unavailable: $(call_message)" \
        "Check the macOS sampler and rerun."
      ;;
    *)
      failed "query_ui_hotspots failed (code ${code:-unknown}): $(call_message)"
      ;;
  esac
fi
HOTSPOT_JSON=$CALL_OUT
printf '%s\n' "$HOTSPOT_JSON" >"$RUN_DIR/artifacts/hotspots.json"
H_STATUS=$(jq -r '.status // empty' <<<"$HOTSPOT_JSON")
H_THREAD=$(jq -r '.thread // empty' <<<"$HOTSPOT_JSON")
H_TAKEN=$(jq -r '.samplesTaken // empty' <<<"$HOTSPOT_JSON")
H_RETAINED=$(jq -r '.retainedSamples // empty' <<<"$HOTSPOT_JSON")
H_COUNT=$(jq -r '.hotspots | length' <<<"$HOTSPOT_JSON")
[ "$H_STATUS" = "ready" ] || failed "Hotspots report status '$H_STATUS' instead of ready."
[ "$H_THREAD" = "$UI_THREAD" ] || failed \
  "Hotspots report thread '$H_THREAD' instead of '$UI_THREAD'."
[ "$H_TAKEN" = "$P_TAKEN" ] || failed \
  "Hotspots report $H_TAKEN samples but the profile carries $P_TAKEN."
[ "$H_RETAINED" = "$P_RETAINED" ] || failed \
  "Hotspots report $H_RETAINED retained samples but the profile carries $P_RETAINED."
is_uint "$H_COUNT" || failed "The hotspot count is not numeric."
[ "$H_COUNT" -gt 0 ] || failed "query_ui_hotspots returned no hotspot for a sampled profile."

jq -e '
  all(.hotspots[];
    ((.label | type) == "string" and (.label | length) > 0)
    and ((.selfSamples | type) == "number" and .selfSamples >= 0)
    and ((.inclusiveSamples | type) == "number" and .inclusiveSamples >= .selfSamples)
    and ((.inclusiveSamples | type) == "number" and .inclusiveSamples <= ($retained))
    and ((.selfMs | type) == "number" and .selfMs >= 0)
    and ((.inclusiveMs | type) == "number" and .inclusiveMs >= .selfMs)
    and ((.selfPercent | type) == "number" and .selfPercent >= 0 and .selfPercent <= 100.0001)
    and ((.inclusivePercent | type) == "number" and .inclusivePercent >= 0 and .inclusivePercent <= 100.0001)
  )
' --argjson retained "$H_RETAINED" >/dev/null 2>&1 <<<"$HOTSPOT_JSON" || failed \
  "Hotspots are malformed (self must not exceed inclusive, inclusive must not exceed retained samples)."

TOP_HOTSPOT=$(jq -r '.hotspots[0].label // empty' <<<"$HOTSPOT_JSON")
TOP_SELF=$(jq -r '.hotspots[0].selfSamples // empty' <<<"$HOTSPOT_JSON")
ATTRIBUTED=$(jq -c '
  [ .hotspots[]
    | select((.function // "") != "" and (.filePath // "") != "" and (.line // 0) > 0) ]
  | .[0] // empty
' <<<"$HOTSPOT_JSON")
FUNCTIONED=$(jq -r '[.hotspots[] | select((.function // "") != "")] | length' <<<"$HOTSPOT_JSON")
pass "hotspots: $H_COUNT entries; top '$TOP_HOTSPOT' with $TOP_SELF self samples"
if [ -n "$ATTRIBUTED" ]; then
  HOT_FUNCTION=$(jq -r '.function' <<<"$ATTRIBUTED")
  HOT_FILE=$(jq -r '.filePath' <<<"$ATTRIBUTED")
  HOT_LINE=$(jq -r '.line' <<<"$ATTRIBUTED")
  HOT_SELF=$(jq -r '.selfSamples' <<<"$ATTRIBUTED")
  pass "source-attributed hotspot: $HOT_FUNCTION at $HOT_FILE:$HOT_LINE ($HOT_SELF self samples)"
elif [ "$FUNCTIONED" -gt 0 ]; then
  warn "No hotspot resolved both a function and file:line; $FUNCTIONED hotspot(s) carry a function name without a source line."
else
  warn "No hotspot resolved a function name; labels retain sampled addresses instead of inventing symbols."
fi

step "Start a longer profile and stop it early"
early_args=$(jq -cn --arg target "$RUNTIME_ID" --argjson hz "$HZ" --argjson duration "$EARLY_DURATION_MS" \
  '{target: $target, hz: $hz, duration_ms: $duration}')
cove_call start_ui_profile "$early_args"
if [ "$CALL_STATUS" -ne 0 ]; then
  code=$(call_error_code)
  case "$code" in
    unavailable)
      blocked "The longer profile reports the sampler unavailable: $(call_message)" \
        "Check the macOS sampler and rerun."
      ;;
    busy)
      failed "start_ui_profile is busy after the first profile finished: $(call_message)"
      ;;
    *)
      failed "start_ui_profile for the longer run failed (code ${code:-unknown}): $(call_message)"
      ;;
  esac
fi
EARLY_RUNNING=$(jq -r '.status // empty' <<<"$CALL_OUT")
[ "$EARLY_RUNNING" = "running" ] || failed \
  "The longer profile returned status '$EARLY_RUNNING' instead of running."

deadline=$(( $(date +%s) + 3 ))
early_live=0
while [ "$(date +%s)" -lt "$deadline" ]; do
  cove_call get_ui_profile "$PROFILE_ARGS"
  [ "$CALL_STATUS" -eq 0 ] || failed \
    "get_ui_profile failed while the longer run was active: $(call_message)"
  status=$(jq -r '.status // empty' <<<"$CALL_OUT")
  live=$(jq -r '.samplesTaken // 0' <<<"$CALL_OUT")
  if [ "$status" = "running" ] && [ "$live" -ge 10 ]; then
    early_live=1
    break
  fi
  if [ "$status" = "ready" ]; then
    failed "The longer profile finished before it could be stopped early; the sampler ignored its 5 s bound."
  fi
  sleep 0.05
done
if [ "$early_live" -eq 1 ]; then
  sleep 0.2
else
  warn "The live counter did not reach 10 samples within 3 s; stopping at the bound check anyway."
fi

cove_call stop_ui_profile "$PROFILE_ARGS"
if [ "$CALL_STATUS" -ne 0 ]; then
  code=$(call_error_code)
  case "$code" in
    unavailable)
      blocked "stop_ui_profile reports the sampler unavailable: $(call_message)" \
        "Check the macOS sampler and rerun."
      ;;
    *)
      failed "stop_ui_profile failed during early stop (code ${code:-unknown}): $(call_message)"
      ;;
  esac
fi
EARLY_JSON=$CALL_OUT
printf '%s\n' "$EARLY_JSON" >"$RUN_DIR/artifacts/profile-early-stop.json"
E_STATUS=$(jq -r '.status // empty' <<<"$EARLY_JSON")
E_THREAD=$(jq -r '.thread // empty' <<<"$EARLY_JSON")
E_DURATION=$(jq -r '.durationMs // empty' <<<"$EARLY_JSON")
E_TAKEN=$(jq -r '.samplesTaken // empty' <<<"$EARLY_JSON")
E_RETAINED=$(jq -r '.retainedSamples // empty' <<<"$EARLY_JSON")
E_STACKS=$(jq -r '.stacks | length' <<<"$EARLY_JSON")
[ "$E_STATUS" = "ready" ] || failed "The early-stopped profile reports status '$E_STATUS' instead of ready."
[ "$E_THREAD" = "$UI_THREAD" ] || failed \
  "The early-stopped profile follows thread '$E_THREAD' instead of '$UI_THREAD'."
is_uint "$E_TAKEN" && [ "$E_TAKEN" -gt 0 ] || failed \
  "The early-stopped profile took no samples."
is_uint "$E_RETAINED" && [ "$E_RETAINED" -gt 0 ] || failed \
  "The early-stopped profile retained no samples."
[ "$E_STACKS" -gt 0 ] || failed "The early-stopped profile has no aggregated stacks."
is_number "$E_DURATION" && awk -v v="$E_DURATION" -v bound="$EARLY_DURATION_MS" \
  'BEGIN { exit !(v > 0 && v < bound) }' || failed \
  "stop_ui_profile waited for the ${EARLY_DURATION_MS} ms bound instead of stopping early (durationMs=$E_DURATION)."
pass "early stop: $E_TAKEN samples in ${E_DURATION} ms (bound ${EARLY_DURATION_MS} ms, $E_RETAINED retained, $E_STACKS stacks)"

cove_call get_ui_profile "$PROFILE_ARGS"
[ "$CALL_STATUS" -eq 0 ] || failed \
  "get_ui_profile failed after the early stop: $(call_message)"
R_STATUS=$(jq -r '.status // empty' <<<"$CALL_OUT")
R_TAKEN=$(jq -r '.samplesTaken // empty' <<<"$CALL_OUT")
R_STACKS=$(jq -r '.stacks | length' <<<"$CALL_OUT")
[ "$R_STATUS" = "ready" ] || failed \
  "After the early stop the last profile reports status '$R_STATUS' instead of ready."
[ "$R_TAKEN" = "$E_TAKEN" ] || failed \
  "After the early stop get_ui_profile reports $R_TAKEN samples instead of $E_TAKEN."
[ "$R_STACKS" -gt 0 ] || failed "The retained profile lost its stacks after the early stop."
pass "the stopped profile stays readable: status ready with $R_TAKEN samples"

step "Require an explicit no_profile for a second stop"
cove_call stop_ui_profile "$PROFILE_ARGS"
if [ "$CALL_STATUS" -eq 0 ]; then
  failed "A second stop_ui_profile succeeded; it must report that no profile is active."
fi
SECOND_CODE=$(call_error_code)
SECOND_MESSAGE=$(call_message)
[ "$SECOND_CODE" = "no_profile" ] || failed \
  "A second stop returned code '${SECOND_CODE:-unknown}' instead of no_profile: $SECOND_MESSAGE"
pass "second stop returned explicit no_profile: $SECOND_MESSAGE"

cove_call get_ui_profile "$PROFILE_ARGS"
[ "$CALL_STATUS" -eq 0 ] || failed \
  "get_ui_profile failed after the rejected second stop: $(call_message)"
R2_STATUS=$(jq -r '.status // empty' <<<"$CALL_OUT")
R2_TAKEN=$(jq -r '.samplesTaken // empty' <<<"$CALL_OUT")
[ "$R2_STATUS" = "ready" ] || failed \
  "The rejected second stop changed the retained profile status to '$R2_STATUS'."
[ "$R2_TAKEN" = "$E_TAKEN" ] || failed \
  "The rejected second stop changed the retained profile sample count from $E_TAKEN to $R2_TAKEN."

printf '\nRESULT PASS  Milestone 3 CPU profiler: live samples, stacks, hotspots and early stop\n'
printf '  runtime:        %s (%s, pid %s)\n' "$RUNTIME_ID" "$RUNTIME_TITLE" "$RUNTIME_PID"
printf '  ui thread:      %s\n' "$P_THREAD"
printf '  profile:        %s samples in %s ms at %s Hz (%s/s, %s dropped, %s stacks)\n' \
  "$P_TAKEN" "$P_DURATION" "$P_HZ" "$P_PER_SECOND" "$P_DROPPED" "$P_STACKS"
printf '  hotspots:       %s entries; top %s\n' "$H_COUNT" "$TOP_HOTSPOT"
printf '  early stop:     %s samples in %s ms (bound %s ms)\n' \
  "$E_TAKEN" "$E_DURATION" "$EARLY_DURATION_MS"
printf '  second stop:    %s\n' "$SECOND_CODE"
printf '  checks:         %s\n' "$PASS_COUNT"
exit 0
