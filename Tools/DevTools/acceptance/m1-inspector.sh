#!/usr/bin/env bash
# Milestone 1 Inspector acceptance for Cove DevTools on macOS.
#
# Starts an isolated controller and the real gallery, then verifies that a
# visible node can be selected by a point through the runtime hit tester and
# captured as a PNG. Every step uses live runtime and screen-capture data.
#
# Builds with cargo, then launches the resulting binaries directly. They are
# the same targets `cargo run` would launch; direct PIDs make cleanup exact.
#
# Usage: Tools/DevTools/acceptance/m1-inspector.sh [--keep]
#
# Exit codes: 0 pass, 1 functional assertion failure, 2 blocked (platform,
# permissions, capture or missing product tool). Interrupt exits 130/143 after
# cleanup. See Docs/devtools-acceptance.md for the full contract.
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
TITLE="CoveUI — Component Showcase"
SEP=$'\x1f'
KEEP_ARTIFACTS=0
GALLERY_TIMEOUT=${ACCEPTANCE_GALLERY_TIMEOUT:-90}
for argument in "$@"; do
  case "$argument" in
    --keep) KEEP_ARTIFACTS=1 ;;
    -h|--help)
      sed -n '2,15p' "$0"
      exit 0
      ;;
    *)
      printf 'Unknown argument: %s\n' "$argument" >&2
      exit 2
      ;;
  esac
done

case "$GALLERY_TIMEOUT" in
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

step "Preflight"
[ "$(uname -s)" = "Darwin" ] || blocked \
  "This harness exercises the macOS controller and gallery." \
  "Run it on macOS. Other hosts need their own platform acceptance run."

for command in cargo python3 jq; do
  command -v "$command" >/dev/null 2>&1 || blocked \
    "Required command '$command' is not available." \
    "Install '$command' (for jq: brew install jq) and rerun."
done

TMP_ROOT=${TMPDIR:-/tmp}
TMP_ROOT=${TMP_ROOT%/}
[ -n "$TMP_ROOT" ] || TMP_ROOT=/tmp
RUN_DIR=$(mktemp -d "$TMP_ROOT/cove-devtools-m1.XXXXXX") || blocked \
  "Cannot create an isolated run directory." \
  "Check TMPDIR permissions and free space."
if ! mkdir -p "$RUN_DIR/logs" "$RUN_DIR/session" "$RUN_DIR/artifacts" "$RUN_DIR/pages"; then
  blocked "Cannot create the acceptance workspace under $RUN_DIR." \
    "Check TMPDIR permissions and free space."
fi
say "Isolated run: $RUN_DIR"

step "Build controller and gallery"
say "Building debug binaries; the first run can take several minutes."
if ! (cd "$ROOT" && cargo build --quiet -p cove-devtools-cli --bin cove-devtools) \
  >"$RUN_DIR/logs/build-controller.log" 2>&1; then
  tail -n 40 "$RUN_DIR/logs/build-controller.log" || true
  blocked "cargo build -p cove-devtools-cli --bin cove-devtools failed." \
    "Fix the build error above; the acceptance harness will not run against stale binaries."
fi
if ! (cd "$ROOT" && cargo build --quiet -p cove-examples --bin gallery) \
  >"$RUN_DIR/logs/build-gallery.log" 2>&1; then
  tail -n 40 "$RUN_DIR/logs/build-gallery.log" || true
  blocked "cargo build -p cove-examples --bin gallery failed." \
    "Fix the build error above; the acceptance harness will not run against stale binaries."
fi

if ! TARGET_DIR=$(cd "$ROOT" && cargo metadata --no-deps --format-version 1 \
  | jq -r '.target_directory'); then
  blocked "cargo metadata could not resolve the workspace target directory." \
    "Fix the workspace manifest and rerun."
fi
CLI="$TARGET_DIR/debug/cove-devtools"
GALLERY="$TARGET_DIR/debug/gallery"
[ -x "$CLI" ] || blocked "Controller binary not found at $CLI." "Run cargo build -p cove-devtools-cli --bin cove-devtools in $ROOT."
[ -x "$GALLERY" ] || blocked "Gallery binary not found at $GALLERY." "Run cargo build -p cove-examples --bin gallery in $ROOT."

if ! "$CLI" tools >"$RUN_DIR/tools.json" 2>"$RUN_DIR/logs/tools.err"; then
  tail -n 20 "$RUN_DIR/logs/tools.err" || true
  blocked "cove-devtools tools failed." "Fix the controller CLI before running acceptance."
fi

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
  printf -- '--- %s %s\n%s\n' "$name" "$args" "$CALL_OUT" >>"$RUN_DIR/logs/calls.log"
  return 0
}

cove_call_image() {
  name=$1
  args=$2
  image=$3
  CALL_ERR="$RUN_DIR/logs/last-call.err"
  if CALL_OUT=$("$CLI" call "$name" "$args" --image "$image" 2>"$CALL_ERR"); then
    CALL_STATUS=0
  else
    CALL_STATUS=$?
  fi
  printf -- '--- %s %s (image %s)\n%s\n' "$name" "$args" "$image" "$CALL_OUT" \
    >>"$RUN_DIR/logs/calls.log"
  return 0
}

call_message() {
  code=$(jq -r '.code // empty' <<<"$CALL_OUT" 2>/dev/null || true)
  message=$(jq -r '.message // empty' <<<"$CALL_OUT" 2>/dev/null || true)
  if [ -z "$message" ] && [ -s "$CALL_ERR" ]; then
    message=$(head -c 400 "$CALL_ERR" || true)
  fi
  [ -n "$code" ] || code="unknown"
  printf '%s: %s' "$code" "$message"
}

tool_exists() {
  jq -e --arg name "$1" '.tools[]? | select(.name == $name)' "$RUN_DIR/tools.json" \
    >/dev/null 2>&1
}

tool_required_has() {
  jq -e --arg name "$1" --arg key "$2" \
    '.tools[]? | select(.name == $name) | (.inputSchema.required // []) | index($key) != null' \
    "$RUN_DIR/tools.json" >/dev/null 2>&1
}

tool_has_property() {
  jq -e --arg name "$1" --arg key "$2" \
    '.tools[]? | select(.name == $name) | (.inputSchema.properties // {}) | has($key)' \
    "$RUN_DIR/tools.json" >/dev/null 2>&1
}

is_number() {
  [[ "$1" =~ ^-?([0-9]+([.][0-9]*)?|[.][0-9]+)([eE][-+]?[0-9]+)?$ ]]
}

unsupported_required() {
  name=$1
  allowed=$2
  jq -r --arg name "$name" \
    '.tools[]? | select(.name == $name) | (.inputSchema.required // [])[]' \
    "$RUN_DIR/tools.json" |
    while IFS= read -r key; do
      case " $allowed " in
        *" $key "*) ;;
        *) printf '%s\n' "$key" ;;
      esac
    done
}

step "Start isolated controller on 127.0.0.1:$PORT"
(
  cd "$ROOT"
  exec "$CLI" serve --port "$PORT"
) >"$RUN_DIR/logs/controller.log" 2>&1 &
CONTROLLER_PID=$!

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
say "Gallery pid $GALLERY_PID"

RUNTIME_JSON=""
deadline=$(( $(date +%s) + GALLERY_TIMEOUT ))
while [ "$(date +%s)" -lt "$deadline" ]; do
  if ! kill -0 "$GALLERY_PID" 2>/dev/null; then
    blocked "The gallery exited before registering a runtime." \
      "Open gallery.log. A GUI session, an unlocked display and a working COVE_DEVTOOLS build are required."
  fi
  cove_call get_ui_session '{}'
  if [ "$CALL_STATUS" -eq 0 ]; then
    SESSION_JSON=$CALL_OUT
    RUNTIME_JSON=$(jq -c --argjson pid "$GALLERY_PID" --arg title "$TITLE" '
      [ .targets[]? | select(.kind == "runtime")
        | select((.pid // -1) == $pid or .title == $title) ] | .[0] // empty
    ' <<<"$SESSION_JSON")
    [ -n "$RUNTIME_JSON" ] && break
  fi
  sleep 0.5
done
[ -n "$RUNTIME_JSON" ] || blocked \
  "The gallery did not register with the isolated controller within $GALLERY_TIMEOUT seconds." \
  "Check gallery.log; verify the window opened on an unlocked display and COVE_DEVTOOLS_SESSION_FILE reached the app."

RUNTIME_ID=$(jq -r '.id' <<<"$RUNTIME_JSON")
RUNTIME_TITLE=$(jq -r '.title // empty' <<<"$RUNTIME_JSON")
RUNTIME_PID=$(jq -r '.pid // -1' <<<"$RUNTIME_JSON")
CAPABILITIES=$(jq -r '(.capabilities // []) | join(",")' <<<"$RUNTIME_JSON")
case ",$CAPABILITIES," in
  *,tree,*) ;;
  *) failed "Runtime $RUNTIME_ID registered without the 'tree' capability ($CAPABILITIES)." ;;
esac
pass "runtime selected: $RUNTIME_ID ($RUNTIME_TITLE, pid $RUNTIME_PID)"

step "Check macOS capture and accessibility permissions"
cove_call get_ui_session '{}'
[ "$CALL_STATUS" -eq 0 ] || blocked "get_ui_session failed after the runtime registered: $(call_message)" \
  "Inspect controller.log; the controller may have shut down."
SESSION_JSON=$CALL_OUT
PLATFORM=$(jq -r '.platform // empty' <<<"$SESSION_JSON")
[ "$PLATFORM" = "macos" ] || blocked "The controller reports platform '$PLATFORM' instead of macos." \
  "Run the acceptance harness on the macOS host."

SCREEN=$(jq -r '.permissions.screen_capture // false' <<<"$SESSION_JSON")
ACCESS=$(jq -r '.permissions.accessibility // false' <<<"$SESSION_JSON")
MISSING_PERMISSIONS=""
if [ "$SCREEN" != "true" ]; then
  MISSING_PERMISSIONS="${MISSING_PERMISSIONS}Screen Recording (permissions.screen_capture=false). "
fi
if [ "$ACCESS" != "true" ]; then
  MISSING_PERMISSIONS="${MISSING_PERMISSIONS}Accessibility (permissions.accessibility=false). "
fi
MISSING_PERMISSIONS=${MISSING_PERMISSIONS% }
if [ -n "$MISSING_PERMISSIONS" ]; then
  blocked "Missing macOS permission(s): $MISSING_PERMISSIONS" \
    "System Settings > Privacy & Security > Screen & System Audio Recording (Screen Recording on older macOS) and > Accessibility: enable the terminal/app that launches this script, quit and reopen it, then rerun. This run cannot pass without live pixels; a stale controller process keeps the old TCC decision."
fi
pass "macOS permissions: screen_capture=true accessibility=true"

step "Select runtime window and bind it"
PLATFORM_TARGET=$(jq -r '.platform_target // empty' <<<"$RUNTIME_JSON")
window_deadline=$(( $(date +%s) + 15 ))
while :; do
  WINDOW_ID=""
  if [ -n "$PLATFORM_TARGET" ] && jq -e --arg id "$PLATFORM_TARGET" \
    '.targets[]? | select(.kind == "window" and .id == $id)' >/dev/null 2>&1 <<<"$SESSION_JSON"; then
    WINDOW_ID=$PLATFORM_TARGET
  fi
  if [ -z "$WINDOW_ID" ]; then
    WINDOW_ID=$(jq -r --argjson pid "$GALLERY_PID" --arg title "$RUNTIME_TITLE" '
      ([ .targets[]? | select(.kind == "window" and .title == $title)
          | select(.id | startswith("window:" + ($pid | tostring) + ":"))
          | .id ] | .[0])
      // ([ .targets[]? | select(.kind == "window")
            | select(.id | startswith("window:" + ($pid | tostring) + ":"))
            | .id ] | .[0])
      // empty
    ' <<<"$SESSION_JSON")
  fi
  [ -n "$WINDOW_ID" ] && break
  [ "$(date +%s)" -lt "$window_deadline" ] || break
  sleep 0.5
  cove_call get_ui_session '{}'
  [ "$CALL_STATUS" -eq 0 ] || break
  SESSION_JSON=$CALL_OUT
  RUNTIME_JSON=$(jq -c --arg id "$RUNTIME_ID" '[ .targets[]? | select(.id == $id) ] | .[0] // empty' <<<"$SESSION_JSON")
  [ -n "$RUNTIME_JSON" ] || break
  PLATFORM_TARGET=$(jq -r '.platform_target // empty' <<<"$RUNTIME_JSON")
done
[ -n "$WINDOW_ID" ] || blocked \
  "No macOS window target is associated with gallery pid $GALLERY_PID." \
  "Check gallery.log and that the window is not minimized; get_ui_session lists window targets only for visible windows."

if [ "$PLATFORM_TARGET" != "$WINDOW_ID" ]; then
  bind_args=$(jq -cn --arg target "$RUNTIME_ID" --arg platform "$WINDOW_ID" \
    '{target: $target, platform_target: $platform}')
  cove_call bind_ui_target "$bind_args"
  [ "$CALL_STATUS" -eq 0 ] || failed \
    "bind_ui_target failed for runtime $RUNTIME_ID and window $WINDOW_ID: $(call_message)"
  jq -e '.bound == true' >/dev/null 2>&1 <<<"$CALL_OUT" || failed \
    "bind_ui_target did not return bound=true for runtime $RUNTIME_ID."
  cove_call get_ui_session '{}'
  [ "$CALL_STATUS" -eq 0 ] || failed "get_ui_session failed after binding: $(call_message)"
  SESSION_JSON=$CALL_OUT
  RUNTIME_JSON=$(jq -c --arg id "$RUNTIME_ID" '[ .targets[]? | select(.id == $id) ] | .[0] // empty' <<<"$SESSION_JSON")
  [ -n "$RUNTIME_JSON" ] || failed "Runtime $RUNTIME_ID disappeared after binding."
  PLATFORM_TARGET=$(jq -r '.platform_target // empty' <<<"$RUNTIME_JSON")
  [ "$PLATFORM_TARGET" = "$WINDOW_ID" ] || failed \
    "Runtime $RUNTIME_ID reports platform_target='$PLATFORM_TARGET' after binding to '$WINDOW_ID'."
fi
pass "runtime $RUNTIME_ID bound to window $WINDOW_ID"

step "Capture the window with capture_ui"
capture_args=$(jq -cn --arg target "$WINDOW_ID" '{target: $target, scope: "window"}')
cove_call_image capture_ui "$capture_args" "$RUN_DIR/artifacts/window.png"
if [ "$CALL_STATUS" -ne 0 ]; then
  blocked "capture_ui failed: $(call_message)" \
    "Verify Screen Recording, keep the gallery window on an unlocked display, and rerun. This is not a pass; no pixels were captured."
fi
CAPTURE_JSON=$CALL_OUT
CAPTURE_ID=$(jq -r '.capture_id // empty' <<<"$CAPTURE_JSON")
CAP_W=$(jq -r '.width // empty' <<<"$CAPTURE_JSON")
CAP_H=$(jq -r '.height // empty' <<<"$CAPTURE_JSON")
COORD_X=$(jq -r '.coordinates.x // empty' <<<"$CAPTURE_JSON")
COORD_Y=$(jq -r '.coordinates.y // empty' <<<"$CAPTURE_JSON")
COORD_W=$(jq -r '.coordinates.width // empty' <<<"$CAPTURE_JSON")
COORD_H=$(jq -r '.coordinates.height // empty' <<<"$CAPTURE_JSON")
CAPTURE_SOURCE=$(jq -r '.source // empty' <<<"$CAPTURE_JSON")
[ -n "$CAPTURE_ID" ] || failed "capture_ui returned no capture_id."
[ -n "$CAPTURE_SOURCE" ] || failed "capture_ui returned no source provenance."
for value in "$CAP_W" "$CAP_H" "$COORD_X" "$COORD_Y" "$COORD_W" "$COORD_H"; do
  if ! [[ "$value" =~ ^-?[0-9]+$ ]]; then
    failed "capture_ui returned a non-integer dimension or coordinate: '$value'."
  fi
done
if [ "$CAP_W" -le 0 ] || [ "$CAP_H" -le 0 ]; then
  failed "capture_ui returned an empty image (${CAP_W}x${CAP_H})."
fi
if [ "$COORD_W" -le 0 ] || [ "$COORD_H" -le 0 ]; then
  failed "capture_ui returned an empty coordinate space."
fi

if ! window_stats=$(python3 "$ROOT/Tools/DevTools/acceptance/png_probe.py" \
  "$RUN_DIR/artifacts/window.png" 2>"$RUN_DIR/logs/png-window.err"); then
  cat "$RUN_DIR/logs/png-window.err" || true
  failed "The window capture is not a supported 8-bit RGB/RGBA PNG."
fi
W_STATS_W=$(jq -r '.width' <<<"$window_stats")
W_STATS_H=$(jq -r '.height' <<<"$window_stats")
W_UNIQUE=$(jq -r '.uniqueColors' <<<"$window_stats")
W_OPAQUE=$(jq -r '.opaquePixels' <<<"$window_stats")
[ "$W_STATS_W" -eq "$CAP_W" ] && [ "$W_STATS_H" -eq "$CAP_H" ] || failed \
  "capture_ui reported ${CAP_W}x${CAP_H} but the PNG is ${W_STATS_W}x${W_STATS_H}."
if [ "$W_UNIQUE" -lt 2 ] || [ "$W_OPAQUE" -eq 0 ]; then
  blocked "The window capture is uniform (${W_UNIQUE} colors, ${W_OPAQUE} opaque pixels); ScreenCaptureKit produced no usable content." \
    "Confirm Screen Recording, unlock the display, keep the gallery window visible (not minimized or fully covered by an opaque window), and rerun."
fi
pass "window capture ${CAP_W}x${CAP_H} pixels from '$CAPTURE_SOURCE' (${W_UNIQUE} colors, ${W_OPAQUE} opaque pixels)"

step "Query visible nodes through the runtime"
query_deadline=$(( $(date +%s) + 15 ))
last_query_error=""
nodes_ready=0
while [ "$(date +%s)" -lt "$query_deadline" ]; do
  rm -f "$RUN_DIR/pages"/page-*.json
  offset=0
  page=0
  pages_ok=1
  while :; do
    page=$((page + 1))
    page_args=$(jq -cn --arg target "$RUNTIME_ID" --argjson offset "$offset" \
      '{target: $target, visible_only: true, compact: true, limit: 100, offset: $offset}')
    cove_call query_ui_nodes "$page_args"
    if [ "$CALL_STATUS" -ne 0 ]; then
      last_query_error=$(call_message)
      pages_ok=0
      break
    fi
    if ! jq -e '.ok == true and (.data.items | type == "array")' \
      >/dev/null 2>&1 <<<"$CALL_OUT"; then
      last_query_error="query_ui_nodes returned an unexpected shape"
      pages_ok=0
      break
    fi
    printf '%s\n' "$CALL_OUT" >"$RUN_DIR/pages/page-$page.json"
    next_offset=$(jq -r '.data.nextOffset // .data.next_offset // empty' <<<"$CALL_OUT")
    [ -n "$next_offset" ] || break
    offset=$next_offset
    [ "$page" -lt 5 ] || break
  done
  if [ "$pages_ok" -eq 1 ]; then
    jq -s '[.[].data.items[]]' "$RUN_DIR/pages"/page-*.json >"$RUN_DIR/artifacts/nodes.json"
    NODE_TOTAL=$(jq 'length' "$RUN_DIR/artifacts/nodes.json")
    if [ "$NODE_TOTAL" -gt 0 ]; then
      nodes_ready=1
      break
    fi
    last_query_error="the runtime tree contains no nodes"
  fi
  sleep 0.3
done
[ "$nodes_ready" -eq 1 ] || failed \
  "query_ui_nodes did not return live nodes within 15 seconds: $last_query_error"

logical_w=$COORD_W
logical_h=$COORD_H
jq -r --argjson ww "$logical_w" --argjson wh "$logical_h" '
  .[]
  | select(.frame != null and (.frame | type) == "object")
  | select((.frame.x | type) == "number" and (.frame.y | type) == "number")
  | select((.frame.w | type) == "number" and (.frame.h | type) == "number")
  | select(.frame.w >= 16 and .frame.h >= 16)
  | select(.frame.x >= -2 and .frame.y >= -2)
  | select((.frame.x + .frame.w) <= ($ww + 4))
  | select((.frame.y + .frame.h) <= ($wh + 4))
  | select((.frame.w * .frame.h) <= ($ww * $wh * 0.8))
  | [ .id, .frame.x, .frame.y, .frame.w, .frame.h, (.role // ""),
      ((.label // "") | gsub("[\\x00-\\x1f]"; " ")),
      (.frame.w * .frame.h),
      (if (.label // "") != "" then 1 else 0 end) ] | join("\u001f")
' "$RUN_DIR/artifacts/nodes.json" | sort -t "$SEP" -k9,9nr -k8,8n >"$RUN_DIR/candidates.tsv"
CANDIDATE_COUNT=$(wc -l <"$RUN_DIR/candidates.tsv" | tr -d ' ')
[ "$CANDIDATE_COUNT" -gt 0 ] || failed \
  "No visible node with a positive frame inside the captured window was found among $NODE_TOTAL nodes."
pass "runtime returned $NODE_TOTAL nodes; $CANDIDATE_COUNT have a usable visible frame"

if ! tool_exists select_ui_at; then
  blocked "select_ui_at is not registered by the controller (Milestone 1 backend tool missing)." \
    "Implement select-at-point in the runtime/controller and rerun. capture_ui and query_ui_nodes passed; artifacts show the live data."
fi

unexpected=$(unsupported_required select_ui_at "target x y capture_id")
[ -z "$unexpected" ] || blocked \
  "select_ui_at requires unsupported argument(s): $(printf '%s ' $unexpected)" \
  "Align the harness with the final tool contract in Docs/devtools-acceptance.md: capture_id plus capture-pixel x/y."
USE_TARGET=0
if tool_has_property select_ui_at target; then
  USE_TARGET=1
fi
USE_CAPTURE_SELECT=0
if tool_has_property select_ui_at capture_id; then
  USE_CAPTURE_SELECT=1
fi
[ "$USE_CAPTURE_SELECT" -eq 1 ] || blocked \
  "select_ui_at does not accept capture_id, so capture pixels cannot be mapped to content points." \
  "The final contract is {target?, capture_id, x, y} with x/y in capture pixels; update the tool and rerun."
if ! tool_has_property select_ui_at x || ! tool_has_property select_ui_at y; then
  blocked "select_ui_at has no x/y capture-pixel arguments." \
    "Restore the {capture_id, x, y} contract and rerun."
fi

step "Calibrate capture pixels to content logical points"
if [ "$CAP_W" -lt 4 ] || [ "$CAP_H" -lt 4 ]; then
  failed "The window capture is too small for point calibration (${CAP_W}x${CAP_H})."
fi
AX=1
AY=1
BX=$((CAP_W - 2))
BY=$((CAP_H - 2))
UX=$(awk -v coordinates="$COORD_W" -v pixels="$CAP_W" 'BEGIN { printf "%.9f", coordinates / pixels }')
UY=$(awk -v coordinates="$COORD_H" -v pixels="$CAP_H" 'BEGIN { printf "%.9f", coordinates / pixels }')

cal_args=$(jq -cn --arg target "$RUNTIME_ID" --arg capture "$CAPTURE_ID" \
  --argjson x "$AX" --argjson y "$AY" --argjson use_target "$USE_TARGET" '
  {} + (if $use_target then {target: $target} else {} end)
     + {capture_id: $capture, x: $x, y: $y}
')
cove_call select_ui_at "$cal_args"
if [ "$CALL_STATUS" -ne 0 ]; then
  code=$(jq -r '.code // empty' <<<"$CALL_OUT" 2>/dev/null || true)
  case "$code" in
    unavailable|unknown_command)
      blocked "select_ui_at cannot map capture pixels: $(call_message)" \
        "Check the runtime's content geometry (macOS must report the AppKit content origin) and restart the controller."
      ;;
    *)
      failed "select_ui_at calibration at (${AX}, ${AY}) failed: $(call_message)"
      ;;
  esac
fi
CP_A_X=$(jq -r '.contentPoint.x // .content_point.x // empty' <<<"$CALL_OUT")
CP_A_Y=$(jq -r '.contentPoint.y // .content_point.y // empty' <<<"$CALL_OUT")
is_number "$CP_A_X" && is_number "$CP_A_Y" || failed \
  "select_ui_at calibration returned no numeric contentPoint."

cal_args=$(jq -cn --arg target "$RUNTIME_ID" --arg capture "$CAPTURE_ID" \
  --argjson x "$BX" --argjson y "$BY" --argjson use_target "$USE_TARGET" '
  {} + (if $use_target then {target: $target} else {} end)
     + {capture_id: $capture, x: $x, y: $y}
')
cove_call select_ui_at "$cal_args"
if [ "$CALL_STATUS" -ne 0 ]; then
  failed "select_ui_at calibration at (${BX}, ${BY}) failed: $(call_message)"
fi
CP_B_X=$(jq -r '.contentPoint.x // .content_point.x // empty' <<<"$CALL_OUT")
CP_B_Y=$(jq -r '.contentPoint.y // .content_point.y // empty' <<<"$CALL_OUT")
is_number "$CP_B_X" && is_number "$CP_B_Y" || failed \
  "select_ui_at calibration returned no numeric contentPoint."

if ! calibration=$(awk \
  -v cx="$COORD_X" -v cy="$COORD_Y" -v ux="$UX" -v uy="$UY" \
  -v ax="$AX" -v ay="$AY" -v bx="$BX" -v by="$BY" \
  -v apx="$CP_A_X" -v apy="$CP_A_Y" -v bpx="$CP_B_X" -v bpy="$CP_B_Y" '
  BEGIN {
    sax = cx + (ax + 0.5) * ux;
    say = cy + (ay + 0.5) * uy;
    sbx = cx + (bx + 0.5) * ux;
    sby = cy + (by + 0.5) * uy;
    dx = bpx - apx;
    dy = bpy - apy;
    if (dx == 0 || dy == 0) exit 1;
    sx = (sbx - sax) / dx;
    sy = (sby - say) / dy;
    if (sx <= 0 || sy <= 0) exit 1;
    if ((sx - sy < 0 ? sy - sx : sx - sy) / ((sx + sy) / 2) > 0.01) exit 1;
    ox = sax - apx * sx;
    oy = say - apy * sy;
    printf "%.9f %.9f %.9f %.9f", sx, sy, ox, oy;
  }'); then
  failed "select_ui_at calibration is not a positive, uniform affine mapping."
fi
read -r CAL_SCALE_X CAL_SCALE_Y CAL_ORIGIN_X CAL_ORIGIN_Y <<<"$calibration"
is_number "$CAL_SCALE_X" && is_number "$CAL_SCALE_Y" \
  && is_number "$CAL_ORIGIN_X" && is_number "$CAL_ORIGIN_Y" || failed \
  "select_ui_at calibration produced non-numeric geometry."
pass "capture mapping: ${UX} x ${UY} server units per pixel, ${CAL_SCALE_X} server units per content point, content origin (${CAL_ORIGIN_X}, ${CAL_ORIGIN_Y})"

step "Build eligible nodes (hit-testable, visible, unclipped)"
: >"$RUN_DIR/eligible.tsv"
checked=0
last_node_error=""
while IFS="$SEP" read -r id fx fy fw fh role label area candidate_labeled; do
  [ -n "$id" ] || continue
  checked=$((checked + 1))
  node_args=$(jq -cn --arg target "$RUNTIME_ID" --arg id "$id" \
    '{target: $target, node_id: $id, compact: false, limit: 1}')
  cove_call query_ui_nodes "$node_args"
  if [ "$CALL_STATUS" -ne 0 ]; then
    last_node_error=$(call_message)
    continue
  fi
  eligible=$(jq -r '
    .data.items[0] as $item
    | select($item != null)
    | select($item.presentation.isVisible == true)
    | select($item.presentation.inputEnabled == true)
    | select($item.node.flags.hitTestable == true)
    | ($item.node.absoluteFrame // empty) as $absolute
    | select(($absolute | type) == "object")
    | [ ($item.node.flags.hasBackground == true),
        (($item.semantics.label // "") | gsub("[\\x00-\\x1f]"; " ")),
        $absolute.x, $absolute.y, $absolute.w, $absolute.h ] | join("\u001f")
  ' <<<"$CALL_OUT" 2>/dev/null || true)
  [ -n "$eligible" ] || continue
  hasbg=""
  semantic_label=""
  absolute_x=""
  absolute_y=""
  absolute_w=""
  absolute_h=""
  IFS="$SEP" read -r hasbg semantic_label absolute_x absolute_y absolute_w absolute_h <<<"$eligible"
  if ! is_number "$absolute_x" || ! is_number "$absolute_y" \
    || ! is_number "$absolute_w" || ! is_number "$absolute_h"; then
    continue
  fi
  if ! awk -v vx="$fx" -v vy="$fy" -v vw="$fw" -v vh="$fh" \
    -v ax="$absolute_x" -v ay="$absolute_y" -v aw="$absolute_w" -v ah="$absolute_h" '
    BEGIN {
      dx = ax - vx; if (dx < 0) dx = -dx;
      dy = ay - vy; if (dy < 0) dy = -dy;
      dw = aw - vw; if (dw < 0) dw = -dw;
      dh = ah - vh; if (dh < 0) dh = -dh;
      if (aw <= 0 || ah <= 0) exit 1;
      exit !(dx <= 2 && dy <= 2 && dw <= 2 && dh <= 2)
    }'; then
    continue
  fi
  if [ -z "$label" ]; then
    label=$semantic_label
  fi
  labeled=0
  if [ -n "$label" ]; then
    labeled=1
  fi
  eligible_line="$id$SEP$fx$SEP$fy$SEP$fw$SEP$fh$SEP$absolute_x$SEP$absolute_y$SEP$absolute_w$SEP$absolute_h$SEP$role$SEP$label$SEP$hasbg$SEP$area$SEP$labeled"
  printf '%s\n' "$eligible_line" >>"$RUN_DIR/eligible.tsv"
  if [ "$checked" -ge 150 ]; then
    break
  fi
done <"$RUN_DIR/candidates.tsv"

sort -t "$SEP" -k14,14nr -k12,12r -k13,13n "$RUN_DIR/eligible.tsv" >"$RUN_DIR/eligible-sorted.tsv"
ELIGIBLE_COUNT=$(wc -l <"$RUN_DIR/eligible-sorted.tsv" | tr -d ' ')
[ "$ELIGIBLE_COUNT" -gt 0 ] || failed \
  "None of the $checked framed nodes is visible, input-enabled, hit-testable and unclipped (last query error: ${last_node_error:-none})."
say "$ELIGIBLE_COUNT eligible nodes (checked $checked candidates)"

step "Select a node by point with select_ui_at"
CHOSEN_ID=""
CHOSEN_LABEL=""
CHOSEN_ROLE=""
CHOSEN_BG=""
CHOSEN_VX=""
CHOSEN_VY=""
CHOSEN_VW=""
CHOSEN_VH=""
CHOSEN_AX=""
CHOSEN_AY=""
CHOSEN_AW=""
CHOSEN_AH=""
CHOSEN_PX=""
CHOSEN_PY=""
CHOSEN_CX=""
CHOSEN_CY=""
attempts=0
mismatches=""
while IFS="$SEP" read -r id vx vy vw vh ax ay aw ah role label hasbg area labeled; do
  [ -n "$id" ] || continue
  attempts=$((attempts + 1))
  if [ "$attempts" -gt 20 ]; then
    break
  fi
  center_x=$(awk -v x="$vx" -v w="$vw" 'BEGIN { printf "%.6f", x + w / 2 }')
  center_y=$(awk -v y="$vy" -v h="$vh" 'BEGIN { printf "%.6f", y + h / 2 }')
  if ! mapped=$(awk \
    -v ox="$CAL_ORIGIN_X" -v oy="$CAL_ORIGIN_Y" -v sx="$CAL_SCALE_X" -v sy="$CAL_SCALE_Y" \
    -v cx="$COORD_X" -v cy="$COORD_Y" -v ux="$UX" -v uy="$UY" \
    -v nx="$center_x" -v ny="$center_y" -v iw="$CAP_W" -v ih="$CAP_H" '
    BEGIN {
      server_x = ox + nx * sx;
      server_y = oy + ny * sy;
      pixel_x = (server_x - cx) / ux - 0.5;
      pixel_y = (server_y - cy) / uy - 0.5;
      px = int(pixel_x + 0.5);
      py = int(pixel_y + 0.5);
      if (px < 0 || py < 0 || px >= iw || py >= ih) exit 1;
      printf "%d %d", px, py;
    }'); then
    continue
  fi
  read -r pixel_x pixel_y <<<"$mapped"
  select_args=$(jq -cn \
    --arg target "$RUNTIME_ID" --arg capture "$CAPTURE_ID" \
    --argjson x "$pixel_x" --argjson y "$pixel_y" --argjson use_target "$USE_TARGET" '
    {} + (if $use_target then {target: $target} else {} end)
       + {capture_id: $capture, x: $x, y: $y}
  ')
  cove_call select_ui_at "$select_args"
  if [ "$CALL_STATUS" -ne 0 ]; then
    code=$(jq -r '.code // empty' <<<"$CALL_OUT" 2>/dev/null || true)
    case "$code" in
      stale_node) continue ;;
      unknown_command|unavailable)
        blocked "select_ui_at is listed but cannot run: $(call_message)" \
          "Wire the runtime hit-test command through the controller and probe, then rerun."
        ;;
      *)
        failed "select_ui_at failed at capture pixel (${pixel_x}, ${pixel_y}) for node $id: $(call_message)"
        ;;
    esac
  fi
  response_capture=$(jq -r '.capture_id // empty' <<<"$CALL_OUT")
  [ "$response_capture" = "$CAPTURE_ID" ] || failed \
    "select_ui_at answered for capture '$response_capture' instead of '$CAPTURE_ID'."
  selected=$(jq -r '.hit.nodeID // .hit.node_id // .hit.nodeId // empty' <<<"$CALL_OUT")
  path_last=$(jq -r '(.hit.path // []) | last // empty' <<<"$CALL_OUT")
  content_x=$(jq -r '.contentPoint.x // .content_point.x // empty' <<<"$CALL_OUT")
  content_y=$(jq -r '.contentPoint.y // .content_point.y // empty' <<<"$CALL_OUT")
  if [ -z "$selected" ] || [ "$selected" = "null" ]; then
    mismatches="${mismatches}${id}@(${pixel_x},${pixel_y}) -> (no hit)\n"
    continue
  fi
  if [ "$selected" != "$id" ]; then
    mismatches="${mismatches}${id}@(${pixel_x},${pixel_y}) -> ${selected}\n"
    continue
  fi
  if [ -n "$path_last" ] && [ "$path_last" != "$id" ]; then
    failed "select_ui_at hit $selected but hit.path ends at '$path_last'."
  fi
  is_number "$content_x" && is_number "$content_y" || failed \
    "select_ui_at hit $id but returned no numeric contentPoint."
  if ! awk -v x="$content_x" -v y="$content_y" -v nx="$center_x" -v ny="$center_y" '
    BEGIN {
      dx = x - nx; if (dx < 0) dx = -dx;
      dy = y - ny; if (dy < 0) dy = -dy;
      exit !(dx <= 2 && dy <= 2)
    }'; then
    failed "select_ui_at hit $id at content ($content_x, $content_y) instead of the node center (${center_x}, ${center_y})."
  fi
  hit_frame=$(jq -r '.hit.absoluteFrame // empty' <<<"$CALL_OUT")
  if [ -n "$hit_frame" ]; then
    hit_x=$(jq -r '.x // empty' <<<"$hit_frame")
    hit_y=$(jq -r '.y // empty' <<<"$hit_frame")
    hit_w=$(jq -r '.w // empty' <<<"$hit_frame")
    hit_h=$(jq -r '.h // empty' <<<"$hit_frame")
    if is_number "$hit_x" && is_number "$hit_y" && is_number "$hit_w" && is_number "$hit_h"; then
      awk -v ax="$hit_x" -v ay="$hit_y" -v aw="$hit_w" -v ah="$hit_h" \
        -v bx="$ax" -v by="$ay" -v bw="$aw" -v bh="$ah" '
        BEGIN {
          dx = ax - bx; if (dx < 0) dx = -dx;
          dy = ay - by; if (dy < 0) dy = -dy;
          dw = aw - bw; if (dw < 0) dw = -dw;
          dh = ah - bh; if (dh < 0) dh = -dh;
          exit !(dx <= 2 && dy <= 2 && dw <= 2 && dh <= 2)
        }' || failed "select_ui_at hit $id but its absolute frame differs from the queried node."
    fi
  fi
  CHOSEN_ID=$id
  CHOSEN_LABEL=$label
  CHOSEN_ROLE=$role
  CHOSEN_BG=$hasbg
  CHOSEN_VX=$vx
  CHOSEN_VY=$vy
  CHOSEN_VW=$vw
  CHOSEN_VH=$vh
  CHOSEN_AX=$ax
  CHOSEN_AY=$ay
  CHOSEN_AW=$aw
  CHOSEN_AH=$ah
  CHOSEN_PX=$pixel_x
  CHOSEN_PY=$pixel_y
  CHOSEN_CX=$center_x
  CHOSEN_CY=$center_y
  break
done <"$RUN_DIR/eligible-sorted.tsv"

if [ -z "$CHOSEN_ID" ]; then
  printf 'Tried %s nodes. Mismatches:\n%b' "$attempts" "$mismatches"
  failed "select_ui_at never returned the ID of the node whose center was hit."
fi
pass "select_ui_at at capture pixel (${CHOSEN_PX}, ${CHOSEN_PY}) -> hit.nodeID $CHOSEN_ID (content center ${CHOSEN_CX}, ${CHOSEN_CY}; frame ${CHOSEN_AX}, ${CHOSEN_AY} ${CHOSEN_AW}x${CHOSEN_AH})"

step "Capture the selected node with capture_ui_node"
if ! tool_exists capture_ui_node; then
  blocked "capture_ui_node is not registered by the controller (Milestone 1 backend tool missing)." \
    "Implement node screenshots in the runtime/controller and rerun. select_ui_at already selected $CHOSEN_ID."
fi
unexpected=$(unsupported_required capture_ui_node "target node_id capture_id")
[ -z "$unexpected" ] || blocked \
  "capture_ui_node requires unsupported argument(s): $(printf '%s ' $unexpected)" \
  "Align the harness with the tool contract in Docs/devtools-acceptance.md."
if ! tool_required_has capture_ui_node node_id; then
  blocked "capture_ui_node does not require node_id." \
    "Restore the {target?, node_id} contract and rerun."
fi
USE_TARGET_NODE=0
if tool_has_property capture_ui_node target; then
  USE_TARGET_NODE=1
fi
USE_CAPTURE_NODE=0
if tool_required_has capture_ui_node capture_id; then
  USE_CAPTURE_NODE=1
fi

node_args=$(jq -cn \
  --arg target "$RUNTIME_ID" --arg node "$CHOSEN_ID" --arg capture "$CAPTURE_ID" \
  --argjson use_target "$USE_TARGET_NODE" --argjson use_capture "$USE_CAPTURE_NODE" '
  {} + (if $use_target then {target: $target} else {} end)
     + {node_id: $node}
     + (if $use_capture then {capture_id: $capture} else {} end)
')
cove_call_image capture_ui_node "$node_args" "$RUN_DIR/artifacts/node.png"
if [ "$CALL_STATUS" -ne 0 ]; then
  code=$(jq -r '.code // empty' <<<"$CALL_OUT" 2>/dev/null || true)
  case "$code" in
    capture_denied|capture_failed|not_visible|unavailable|target_gone|platform_error)
      blocked "capture_ui_node failed: $(call_message)" \
        "Verify Screen Recording, keep the gallery window and selected node visible, and rerun."
      ;;
    *)
      failed "capture_ui_node failed: $(call_message)"
      ;;
  esac
fi
NODE_JSON=$CALL_OUT
NODE_W=$(jq -r '.width // empty' <<<"$NODE_JSON")
NODE_H=$(jq -r '.height // empty' <<<"$NODE_JSON")
NODE_SOURCE=$(jq -r '.source // empty' <<<"$NODE_JSON")
NODE_CAPTURE_ID=$(jq -r '.capture_id // empty' <<<"$NODE_JSON")
NODE_RESOURCE=$(jq -r '.resource // empty' <<<"$NODE_JSON")
NODE_REPORTED_ID=$(jq -r '.node_id // .nodeID // .nodeId // empty' <<<"$NODE_JSON")
CONTENT_X=$(jq -r '.contentFrame.x // .content_frame.x // empty' <<<"$NODE_JSON")
CONTENT_Y=$(jq -r '.contentFrame.y // .content_frame.y // empty' <<<"$NODE_JSON")
CONTENT_W=$(jq -r '.contentFrame.w // .content_frame.w // empty' <<<"$NODE_JSON")
CONTENT_H=$(jq -r '.contentFrame.h // .content_frame.h // empty' <<<"$NODE_JSON")
NODE_COORD_X=$(jq -r '.coordinates.x // empty' <<<"$NODE_JSON")
NODE_COORD_Y=$(jq -r '.coordinates.y // empty' <<<"$NODE_JSON")
NODE_COORD_W=$(jq -r '.coordinates.width // empty' <<<"$NODE_JSON")
NODE_COORD_H=$(jq -r '.coordinates.height // empty' <<<"$NODE_JSON")
[ -n "$NODE_SOURCE" ] || failed "capture_ui_node returned no source provenance."
[ -n "$NODE_CAPTURE_ID" ] || failed "capture_ui_node returned no capture_id."
[ -n "$NODE_RESOURCE" ] || failed "capture_ui_node returned no capture resource."
[ -n "$NODE_REPORTED_ID" ] || failed "capture_ui_node returned no node_id."
[ "$NODE_REPORTED_ID" = "$CHOSEN_ID" ] || failed \
  "capture_ui_node reports node '$NODE_REPORTED_ID' but selected node is '$CHOSEN_ID'."
case "$NODE_W" in ''|*[!0-9]*) failed "capture_ui_node returned a non-numeric width: '$NODE_W'." ;; esac
case "$NODE_H" in ''|*[!0-9]*) failed "capture_ui_node returned a non-numeric height: '$NODE_H'." ;; esac
if [ "$NODE_W" -le 0 ] || [ "$NODE_H" -le 0 ]; then
  failed "capture_ui_node returned an empty image."
fi
if ! is_number "$CONTENT_X" || ! is_number "$CONTENT_Y" \
  || ! is_number "$CONTENT_W" || ! is_number "$CONTENT_H"; then
  failed "capture_ui_node returned no valid contentFrame."
fi
for value in "$NODE_COORD_X" "$NODE_COORD_Y" "$NODE_COORD_W" "$NODE_COORD_H"; do
  is_number "$value" || failed "capture_ui_node returned a non-numeric crop coordinate: '$value'."
done
if ! awk -v ax="$CONTENT_X" -v ay="$CONTENT_Y" -v aw="$CONTENT_W" -v ah="$CONTENT_H" \
  -v bx="$CHOSEN_AX" -v by="$CHOSEN_AY" -v bw="$CHOSEN_AW" -v bh="$CHOSEN_AH" '
  BEGIN {
    dx = ax - bx; if (dx < 0) dx = -dx;
    dy = ay - by; if (dy < 0) dy = -dy;
    dw = aw - bw; if (dw < 0) dw = -dw;
    dh = ah - bh; if (dh < 0) dh = -dh;
    exit !(dx <= 2 && dy <= 2 && dw <= 2 && dh <= 2)
  }'; then
  failed "capture_ui_node contentFrame (${CONTENT_X}, ${CONTENT_Y}, ${CONTENT_W}x${CONTENT_H}) does not match the selected node frame (${CHOSEN_AX}, ${CHOSEN_AY}, ${CHOSEN_AW}x${CHOSEN_AH})."
fi

if ! node_stats=$(python3 "$ROOT/Tools/DevTools/acceptance/png_probe.py" \
  "$RUN_DIR/artifacts/node.png" 2>"$RUN_DIR/logs/png-node.err"); then
  cat "$RUN_DIR/logs/png-node.err" || true
  failed "capture_ui_node did not produce a supported 8-bit RGB/RGBA PNG."
fi
S_W=$(jq -r '.width' <<<"$node_stats")
S_H=$(jq -r '.height' <<<"$node_stats")
S_UNIQUE=$(jq -r '.uniqueColors' <<<"$node_stats")
S_OPAQUE=$(jq -r '.opaquePixels' <<<"$node_stats")
S_OPAQUE_RATIO=$(jq -r '.opaqueRatio' <<<"$node_stats")
[ "$S_W" -eq "$NODE_W" ] && [ "$S_H" -eq "$NODE_H" ] || failed \
  "capture_ui_node reported ${NODE_W}x${NODE_H} but the PNG is ${S_W}x${S_H}."
[ "$S_W" -le "$CAP_W" ] && [ "$S_H" -le "$CAP_H" ] || failed \
  "The node PNG (${S_W}x${S_H}) is larger than the window capture (${CAP_W}x${CAP_H})."

expected_px_w=$(awk -v frame="$CONTENT_W" -v scale="$CAL_SCALE_X" -v units="$UX" \
  'BEGIN { printf "%d", int(frame * scale / units + 0.5) }')
expected_px_h=$(awk -v frame="$CONTENT_H" -v scale="$CAL_SCALE_Y" -v units="$UY" \
  'BEGIN { printf "%d", int(frame * scale / units + 0.5) }')
tol_w=$(awk -v a="$CONTENT_W" -v b="$expected_px_w" 'BEGIN { m = (a > b ? a : b); t = int(m * 0.02) + 4; if (t < 4) t = 4; print t }')
tol_h=$(awk -v a="$CONTENT_H" -v b="$expected_px_h" 'BEGIN { m = (a > b ? a : b); t = int(m * 0.02) + 4; if (t < 4) t = 4; print t }')
awk -v w="$S_W" -v h="$S_H" -v ew="$expected_px_w" -v eh="$expected_px_h" -v tw="$tol_w" -v th="$tol_h" '
  BEGIN { exit !((w - ew <= tw && ew - w <= tw) && (h - eh <= th && eh - h <= th)) }' || failed \
  "Node PNG ${S_W}x${S_H} does not match the contentFrame crop (${expected_px_w}x${expected_px_h})."
if ! awk -v w="$NODE_COORD_W" -v h="$NODE_COORD_H" -v fw="$CONTENT_W" -v fh="$CONTENT_H" \
  -v sx="$CAL_SCALE_X" -v sy="$CAL_SCALE_Y" '
  BEGIN {
    expected_w = fw * sx;
    expected_h = fh * sy;
    dw = w - expected_w; if (dw < 0) dw = -dw;
    dh = h - expected_h; if (dh < 0) dh = -dh;
    exit !(dw <= 2 && dh <= 2)
  }'; then
  failed "Node capture coordinates ${NODE_COORD_W}x${NODE_COORD_H} do not match contentFrame ${CONTENT_W}x${CONTENT_H} at the calibrated scale."
fi
if ! awk -v x="$NODE_COORD_X" -v y="$NODE_COORD_Y" \
  -v ox="$CAL_ORIGIN_X" -v oy="$CAL_ORIGIN_Y" -v sx="$CAL_SCALE_X" -v sy="$CAL_SCALE_Y" \
  -v fx="$CONTENT_X" -v fy="$CONTENT_Y" '
  BEGIN {
    expected_x = ox + fx * sx;
    expected_y = oy + fy * sy;
    dx = x - expected_x; if (dx < 0) dx = -dx;
    dy = y - expected_y; if (dy < 0) dy = -dy;
    exit !(dx <= 2 && dy <= 2)
  }'; then
  failed "Node crop origin (${NODE_COORD_X}, ${NODE_COORD_Y}) does not match contentFrame origin (${CONTENT_X}, ${CONTENT_Y}) at the calibrated scale."
fi
awk -v w="$S_W" -v h="$S_H" -v fw="$CONTENT_W" -v fh="$CONTENT_H" '
  BEGIN {
    if (fw <= 0 || fh <= 0 || h <= 0) exit 1;
    expected = fw / fh;
    actual = w / h;
    difference = (actual - expected) / expected;
    if (difference < 0) difference = -difference;
    exit !(difference <= 0.03)
  }' || failed "Node PNG aspect ratio ${S_W}x${S_H} does not match contentFrame ${CONTENT_W}x${CONTENT_H}."

if [ "$S_UNIQUE" -lt 2 ] && [ "$CHOSEN_BG" != "true" ]; then
  failed "Node PNG is uniform (${S_UNIQUE} color) and the node reports no background; cannot prove node pixels."
fi
[ "$S_OPAQUE" -gt 0 ] || failed "Node PNG is fully transparent; no node pixels were captured."
if [ "$CHOSEN_BG" = "true" ]; then
  awk -v ratio="$S_OPAQUE_RATIO" 'BEGIN { exit !(ratio >= 0.5) }' || failed \
    "Node PNG is mostly transparent (opaque ratio ${S_OPAQUE_RATIO}) despite a node background."
fi

pass "capture_ui_node produced ${S_W}x${S_H} PNG from '$NODE_SOURCE' (contentFrame ${CONTENT_W}x${CONTENT_H}, ${S_UNIQUE} colors, opaque ratio ${S_OPAQUE_RATIO})"

printf '\nRESULT PASS  Milestone 1 Inspector: select-at-point and node screenshot\n'
printf '  runtime:      %s\n' "$RUNTIME_ID"
printf '  window:       %s\n' "$WINDOW_ID"
printf '  window PNG:   %sx%s, %s colors\n' "$CAP_W" "$CAP_H" "$W_UNIQUE"
printf '  node:         %s (%s, label=%s)\n' "$CHOSEN_ID" "$CHOSEN_ROLE" "$CHOSEN_LABEL"
printf '  select pixel: %s,%s -> content %s,%s\n' "$CHOSEN_PX" "$CHOSEN_PY" "$CHOSEN_CX" "$CHOSEN_CY"
printf '  node frame:   %s,%s %sx%s (visible %s,%s %sx%s)\n' \
  "$CHOSEN_AX" "$CHOSEN_AY" "$CHOSEN_AW" "$CHOSEN_AH" \
  "$CHOSEN_VX" "$CHOSEN_VY" "$CHOSEN_VW" "$CHOSEN_VH"
printf '  node PNG:     %sx%s, %s colors, opaque ratio %s\n' "$S_W" "$S_H" "$S_UNIQUE" "$S_OPAQUE_RATIO"
printf '  checks:       %s\n' "$PASS_COUNT"
exit 0
