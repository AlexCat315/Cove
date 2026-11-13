#!/usr/bin/env bash
# App Size acceptance for Cove DevTools: native executable analysis.
#
# Starts an isolated controller and the real gallery, waits for the runtime,
# calls get_ui_app_size on it and asserts the response against the executable
# actually running: available=true, file_bytes equal to the file on disk,
# non-empty sections/symbols/groups, real section sizes and addresses,
# demangled Rust group names, and truncation flags consistent with the returned
# counts at two different limits. It then calls the same tool on a display or
# other platform target and requires the explicit "unavailable" fault with no
# size fields, so no executable is invented for a target that has none.
#
# This measures the selected runtime's native executable. It is not Flutter or
# Dart AOT attribution; no package, asset or snapshot sizes are asserted.
#
# Usage: Tools/DevTools/acceptance/app-size.sh [--keep] [--no-build]
#
# Environment:
#   ACCEPTANCE_GALLERY_TIMEOUT  seconds to wait for a registered runtime (default 120)
#
# Exit codes: 0 pass, 1 functional assertion failure, 2 blocked (non-macOS
# host, missing dependency, missing product tool, build or startup failure,
# executable analysis unavailable). Interrupt exits 130/143 after cleanup.
# See Docs/devtools-acceptance.md for the full contract.
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
KEEP_ARTIFACTS=0
BUILD=1
# Rows requested for the truncation run. A debug gallery has far more sections,
# symbols and groups than this, so at least one list must report truncation.
SMALL_SECTIONS=3
SMALL_SYMBOLS=5
SMALL_GROUPS=3
# Rows requested for the full run: the controller's own maxima.
MAX_SECTIONS=100
MAX_SYMBOLS=200
MAX_GROUPS=100
for argument in "$@"; do
  case "$argument" in
    --keep) KEEP_ARTIFACTS=1 ;;
    --no-build) BUILD=0 ;;
    -h|--help)
      sed -n '2,24p' "$0"
      exit 0
      ;;
    *)
      printf 'Unknown argument: %s\n' "$argument" >&2
      exit 2
      ;;
  esac
done

case "${ACCEPTANCE_GALLERY_TIMEOUT:-120}" in
  ''|*[!0-9]*)
    printf 'ACCEPTANCE_GALLERY_TIMEOUT must be a whole number of seconds.\n' >&2
    exit 2
    ;;
esac
GALLERY_TIMEOUT=${ACCEPTANCE_GALLERY_TIMEOUT:-120}

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
  "This harness resolves the selected runtime's local PID to its executable through the macOS process API." \
  "Run it on macOS. Linux and Windows hosts have their own executable resolver and need their own acceptance run."

for command in cargo python3 jq; do
  command -v "$command" >/dev/null 2>&1 || blocked \
    "Required command '$command' is not available." \
    "Install '$command' (for jq: brew install jq) and rerun."
done

TMP_ROOT=${TMPDIR:-/tmp}
TMP_ROOT=${TMP_ROOT%/}
[ -n "$TMP_ROOT" ] || TMP_ROOT=/tmp
RUN_DIR=$(mktemp -d "$TMP_ROOT/cove-devtools-app-size.XXXXXX") || blocked \
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

tool_exists get_ui_app_size || blocked \
  "get_ui_app_size is not registered by the controller." \
  "Restore the App Size tool in the controller before running acceptance; there is no other way to measure the runtime executable."
if ! unexpected=$(jq -r --arg name "get_ui_app_size" \
  '.tools[]? | select(.name == $name) | (.inputSchema.required // [])[] | select(. != "target")' \
  "$RUN_DIR/tools.json"); then
  blocked "Cannot read the get_ui_app_size schema." "Inspect tools.json and rerun."
fi
[ -z "$unexpected" ] || blocked \
  "get_ui_app_size requires argument(s) this harness does not send: $(printf '%s ' $unexpected)" \
  "Align the harness with the App Size tool contract in Docs/devtools-acceptance.md."
for property in sections symbols groups; do
  tool_has_property get_ui_app_size "$property" || blocked \
    "get_ui_app_size has no '$property' limit argument; the truncation assertions cannot run." \
    "Restore the bounded sections/symbols/groups arguments or update the harness with the documented contract."
done

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
    elif (.error.data.code? | type) == "string" then .error.data.code
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
[ -n "$RUNTIME_JSON" ] || blocked \
  "The gallery did not register with the isolated controller within $GALLERY_TIMEOUT seconds." \
  "Check gallery.log; verify the window opened on an unlocked display and COVE_DEVTOOLS_SESSION_FILE reached the app. The runtime is selected by the gallery PID, never by title."

RUNTIME_ID=$(jq -r '.id' <<<"$RUNTIME_JSON")
RUNTIME_TITLE=$(jq -r '.title // empty' <<<"$RUNTIME_JSON")
RUNTIME_PID=$(jq -r '.pid // -1' <<<"$RUNTIME_JSON")
RUNTIME_PLATFORM=$(jq -r '.platform // empty' <<<"$RUNTIME_JSON")
[ "$RUNTIME_PLATFORM" = "macos" ] || blocked \
  "The gallery registered as platform '$RUNTIME_PLATFORM' instead of macos." \
  "Run the acceptance harness on the macOS host; another platform needs its own executable resolver and run."
pass "runtime selected: $RUNTIME_ID ($RUNTIME_TITLE, pid $RUNTIME_PID)"

measure_runtime() {
  limits=$1
  cove_call get_ui_app_size "$limits"
  attempt=0
  while [ "$CALL_STATUS" -ne 0 ]; do
    attempt=$((attempt + 1))
    if ! kill -0 "$CONTROLLER_PID" 2>/dev/null; then
      blocked "The controller exited while get_ui_app_size was running." \
        "Inspect controller.log; App Size cannot be read from a dead controller."
    fi
    code=$(call_error_code)
    case "$code" in
      unavailable)
        blocked "get_ui_app_size reports the executable analysis unavailable: $(call_message)" \
          "The selected runtime's local PID must resolve to a readable object file; check the executable and the platform resolver, then rerun. No size is estimated or substituted."
        ;;
      target_gone|target_required|target_unbound)
        blocked "get_ui_app_size cannot select the runtime (code ${code:-unknown}): $(call_message)" \
          "Re-read get_ui_session and pass the gallery runtime id; the runtime may have disconnected."
        ;;
      *)
        if [ "$attempt" -ge 3 ]; then
          failed "get_ui_app_size failed (code ${code:-unknown}): $(call_message)"
        fi
        sleep 0.3
        cove_call get_ui_app_size "$limits"
        ;;
    esac
  done
}

SMALL_ARGS=$(jq -cn \
  --arg target "$RUNTIME_ID" \
  --argjson sections "$SMALL_SECTIONS" \
  --argjson symbols "$SMALL_SYMBOLS" \
  --argjson groups "$SMALL_GROUPS" \
  '{target: $target, sections: $sections, symbols: $symbols, groups: $groups}')

step "Measure the gallery runtime (limits ${SMALL_SECTIONS}/${SMALL_SYMBOLS}/${SMALL_GROUPS})"
measure_runtime "$SMALL_ARGS"
SMALL_JSON=$CALL_OUT
printf '%s\n' "$SMALL_JSON" >"$RUN_DIR/artifacts/app-size-small.json"

jq -e 'type == "object" and (.available | type) == "boolean"' >/dev/null 2>&1 <<<"$SMALL_JSON" \
  || failed "get_ui_app_size did not answer an object with available: $(jq -c . <<<"$SMALL_JSON" 2>/dev/null || printf 'unparseable response')"
[ "$(jq -r '.available' <<<"$SMALL_JSON")" = "true" ] || failed \
  "get_ui_app_size answered available=$(jq -r '.available' <<<"$SMALL_JSON") for a local gallery runtime: $(jq -c '.reason // .message // .error // empty' <<<"$SMALL_JSON")"
jq -e '(.measurement == "native_executable") and ((.label | type) == "string") and ((.label | length) > 0) and ((.attribution | type) == "string") and ((.attribution | length) > 0)' \
  >/dev/null 2>&1 <<<"$SMALL_JSON" || failed \
  "the measurement is not labelled as a native executable: measurement=$(jq -r '.measurement // "missing"' <<<"$SMALL_JSON") label=$(jq -r '.label // "missing"' <<<"$SMALL_JSON")"
pass "native executable measurement: $(jq -r '.format' <<<"$SMALL_JSON") $(jq -r '.architecture' <<<"$SMALL_JSON") $(jq -r '.kind' <<<"$SMALL_JSON")"

[ "$(jq -r '.pid' <<<"$SMALL_JSON")" = "$RUNTIME_PID" ] || failed \
  "App Size reports pid $(jq -r '.pid' <<<"$SMALL_JSON") instead of the gallery runtime pid $RUNTIME_PID."
[ "$(jq -r '.target' <<<"$SMALL_JSON")" = "$RUNTIME_ID" ] || failed \
  "App Size reports target $(jq -r '.target // "missing"' <<<"$SMALL_JSON") instead of $RUNTIME_ID."

EXECUTABLE=$(jq -r '.executable // empty' <<<"$SMALL_JSON")
case "$EXECUTABLE" in
  /*) ;;
  *) failed "App Size returned a non-absolute executable path: '${EXECUTABLE:-missing}'." ;;
esac
[ -f "$EXECUTABLE" ] || failed \
  "App Size named $EXECUTABLE but no such file exists; the measurement is not of a live executable."
FILE_BYTES=$(jq -r '.file_bytes' <<<"$SMALL_JSON")
is_uint "$FILE_BYTES" || failed "file_bytes is not a whole number: $FILE_BYTES"
[ "$FILE_BYTES" -gt 0 ] || failed "file_bytes is 0; a measured executable cannot be empty."
ACTUAL_BYTES=$(python3 -c 'import os,sys; print(os.path.getsize(sys.argv[1]))' "$EXECUTABLE") \
  || failed "Cannot stat $EXECUTABLE."
[ "$FILE_BYTES" -eq "$ACTUAL_BYTES" ] || failed \
  "file_bytes $FILE_BYTES does not match the $ACTUAL_BYTES bytes on disk at $EXECUTABLE."
EXECUTABLE_REAL=$(python3 -c 'import os,sys; print(os.path.realpath(sys.argv[1]))' "$EXECUTABLE")
GALLERY_REAL=$(python3 -c 'import os,sys; print(os.path.realpath(sys.argv[1]))' "$GALLERY")
[ "$EXECUTABLE_REAL" = "$GALLERY_REAL" ] || failed \
  "App Size measured '$EXECUTABLE_REAL' instead of the launched gallery binary '$GALLERY_REAL'; the runtime PID resolved to a different process."
pass "executable: $EXECUTABLE ($FILE_BYTES bytes on disk, matches the launched gallery)"

FORMAT=$(jq -r '.format // empty' <<<"$SMALL_JSON")
[ "$FORMAT" = "MachO" ] || failed \
  "The macOS gallery executable parsed as format '$FORMAT' instead of MachO."
KIND=$(jq -r '.kind // empty' <<<"$SMALL_JSON")
[ "$KIND" = "Executable" ] || failed \
  "The gallery executable parsed as kind '$KIND' instead of Executable."
case "$(uname -m)" in
  arm64) EXPECTED_ARCH=Aarch64 ;;
  x86_64) EXPECTED_ARCH=X86_64 ;;
  *) EXPECTED_ARCH="" ;;
esac
ARCH=$(jq -r '.architecture // empty' <<<"$SMALL_JSON")
if [ -n "$EXPECTED_ARCH" ]; then
  [ "$ARCH" = "$EXPECTED_ARCH" ] || failed \
    "The gallery executable parsed as architecture '$ARCH' but the host is '$(uname -m)' ($EXPECTED_ARCH)."
fi
[ -n "$(jq -r '.symbol_size_basis // empty' <<<"$SMALL_JSON")" ] || failed \
  "symbol_size_basis is missing; inferred symbol sizes would be unexplained."
pass "object file: MachO Executable $ARCH with a documented symbol size basis"

bad_section=$(jq -r '
  .sections | to_entries[]
  | select(
      ((.value.name | type) != "string") or ((.value.name | length) == 0)
      or ((.value.kind | type) != "string") or ((.value.kind | length) == 0)
      or ((.value.size | type) != "number") or (.value.size < 0) or (.value.size != (.value.size | floor))
      or ((.value.address | type) != "number") or (.value.address < 0) or (.value.address != (.value.address | floor))
    )
  | "\(.key): \(.value | tojson)"
' <<<"$SMALL_JSON" | head -n 1)
[ -z "$bad_section" ] || failed "A section row is malformed: $bad_section"
SECTION_ROWS=$(jq -r '.sections | length' <<<"$SMALL_JSON")
[ "$SECTION_ROWS" -gt 0 ] || failed "App Size returned no sections for the native executable."
jq -e '[.sections[] | select(.segment == "__TEXT")] | length > 0' >/dev/null 2>&1 <<<"$SMALL_JSON" \
  || failed "No returned section belongs to the __TEXT segment; the rows are not Mach-O sections: $(jq -c '[.sections[].segment] | unique' <<<"$SMALL_JSON")"
jq -e '[.sections[] | select(.size > 0 and .address > 0)] | length > 0' >/dev/null 2>&1 <<<"$SMALL_JSON" \
  || failed "Every returned section has size 0 or address 0; no real section extent was reported."
jq -e '[.sections[].size] as $sizes | $sizes == ($sizes | sort | reverse)' >/dev/null 2>&1 <<<"$SMALL_JSON" \
  || failed "Sections are not ordered by descending size: $(jq -c '[.sections[].size]' <<<"$SMALL_JSON")"
pass "sections: $SECTION_ROWS returned, largest $(jq -r '.sections[0].name' <<<"$SMALL_JSON") $(jq -r '.sections[0].size' <<<"$SMALL_JSON") bytes at $(printf '0x%x' "$(jq -r '.sections[0].address' <<<"$SMALL_JSON")")"

bad_symbol=$(jq -r '
  .symbols | to_entries[]
  | select(
      ((.value.name | type) != "string") or ((.value.name | length) == 0)
      or ((.value.raw | type) != "string") or ((.value.raw | length) == 0)
      or ((.value.kind | type) != "string") or ((.value.kind | length) == 0)
      or ((.value.size | type) != "number") or (.value.size < 0) or (.value.size != (.value.size | floor))
      or ((.value.address | type) != "number") or (.value.address < 0) or (.value.address != (.value.address | floor))
      or ((.value.inferred_size | type) != "boolean")
    )
  | "\(.key): \(.value | tojson)"
' <<<"$SMALL_JSON" | head -n 1)
[ -z "$bad_symbol" ] || failed "A symbol row is malformed: $bad_symbol"
SYMBOL_ROWS=$(jq -r '.symbols | length' <<<"$SMALL_JSON")
[ "$SYMBOL_ROWS" -gt 0 ] || failed "App Size returned no symbols for the native executable."
jq -e '[.symbols[].size] as $sizes | $sizes == ($sizes | sort | reverse)' >/dev/null 2>&1 <<<"$SMALL_JSON" \
  || failed "Symbols are not ordered by descending size: $(jq -c '[.symbols[].size]' <<<"$SMALL_JSON")"
[ "$(jq -r '.symbol_bytes' <<<"$SMALL_JSON")" -gt 0 ] || failed \
  "symbol_bytes is 0; the returned symbols carry no size."
pass "symbols: $SYMBOL_ROWS returned of $(jq -r '.symbol_count' <<<"$SMALL_JSON"), $(jq -r '.symbol_bytes' <<<"$SMALL_JSON") bytes in the symbol table, largest $(jq -r '.symbols[0].name[0:96]' <<<"$SMALL_JSON") $(jq -r '.symbols[0].size' <<<"$SMALL_JSON") bytes"

bad_group=$(jq -r '
  .groups | to_entries[]
  | select(
      ((.value.name | type) != "string") or ((.value.name | length) == 0)
      or ((.value.bytes | type) != "number") or (.value.bytes < 0) or (.value.bytes != (.value.bytes | floor))
      or ((.value.symbols | type) != "number") or (.value.symbols <= 0) or (.value.symbols != (.value.symbols | floor))
    )
  | "\(.key): \(.value | tojson)"
' <<<"$SMALL_JSON" | head -n 1)
[ -z "$bad_group" ] || failed "A group row is malformed: $bad_group"
GROUP_ROWS=$(jq -r '.groups | length' <<<"$SMALL_JSON")
[ "$GROUP_ROWS" -gt 0 ] || failed "App Size returned no crate groups for the native executable."
jq -e '[.groups[].bytes] as $bytes | $bytes == ($bytes | sort | reverse)' >/dev/null 2>&1 <<<"$SMALL_JSON" \
  || failed "Groups are not ordered by descending bytes: $(jq -c '[.groups[].bytes]' <<<"$SMALL_JSON")"
jq -e '([.groups[].bytes] | add // 0) <= .symbol_bytes and ([.groups[].symbols] | add // 0) <= .symbol_count' \
  >/dev/null 2>&1 <<<"$SMALL_JSON" || failed \
  "Group sums exceed the symbol totals: $(jq -c '{group_bytes: ([.groups[].bytes] | add), symbol_bytes, group_symbols: ([.groups[].symbols] | add), symbol_count}' <<<"$SMALL_JSON")"
bad_group_name=$(jq -r '.groups[] | select(.name | test("^__?(ZN|R)|^<")) | .name' <<<"$SMALL_JSON" | head -n 1)
[ -z "$bad_group_name" ] || failed \
  "Group name '$bad_group_name' is still mangled; groups must use demangled crate names."
pass "groups: $GROUP_ROWS returned of $(jq -r '.group_count' <<<"$SMALL_JSON"), largest $(jq -r '.groups[0].name' <<<"$SMALL_JSON") $(jq -r '.groups[0].bytes' <<<"$SMALL_JSON") bytes across $(jq -r '.groups[0].symbols' <<<"$SMALL_JSON") symbols"

truncation_summary() {
  jq -r '"sections \(.sections_returned)/\(.section_count) truncated=\(.sections_truncated); symbols \(.symbols_returned)/\(.symbol_count) truncated=\(.symbols_truncated); groups \(.groups_returned)/\(.group_count) truncated=\(.groups_truncated)"' <<<"$1"
}

check_truncation() {
  payload=$1
  sections_limit=$2
  symbols_limit=$3
  groups_limit=$4
  jq -e --argjson ls "$sections_limit" --argjson ly "$symbols_limit" --argjson lg "$groups_limit" '
    (.sections | length) == .sections_returned
    and (.symbols | length) == .symbols_returned
    and (.groups | length) == .groups_returned
    and (.sections | length) <= $ls
    and (.symbols | length) <= $ly
    and (.groups | length) <= $lg
    and .section_count >= .sections_returned
    and .symbol_count >= .symbols_returned
    and .group_count >= .groups_returned
    and (.sections_truncated == (.section_count > .sections_returned))
    and (.symbols_truncated == (.symbol_count > .symbols_returned))
    and (.groups_truncated == (.group_count > .groups_returned))
  ' >/dev/null 2>&1 <<<"$payload"
}

check_truncation "$SMALL_JSON" "$SMALL_SECTIONS" "$SMALL_SYMBOLS" "$SMALL_GROUPS" || failed \
  "Truncation flags and counts disagree at limits ${SMALL_SECTIONS}/${SMALL_SYMBOLS}/${SMALL_GROUPS}: $(truncation_summary "$SMALL_JSON")"
TRUNCATED_LISTS=$(jq -r '[.sections_truncated, .symbols_truncated, .groups_truncated] | map(select(. == true)) | length' <<<"$SMALL_JSON")
[ "$TRUNCATED_LISTS" -gt 0 ] || failed \
  "No list was truncated at limits ${SMALL_SECTIONS}/${SMALL_SYMBOLS}/${SMALL_GROUPS}, so bounded output is unproven: $(truncation_summary "$SMALL_JSON")"
pass "bounded rows at ${SMALL_SECTIONS}/${SMALL_SYMBOLS}/${SMALL_GROUPS}: $(truncation_summary "$SMALL_JSON")"

step "Read the same executable at the controller maxima (${MAX_SECTIONS}/${MAX_SYMBOLS}/${MAX_GROUPS})"
MAX_ARGS=$(jq -cn \
  --arg target "$RUNTIME_ID" \
  --argjson sections "$MAX_SECTIONS" \
  --argjson symbols "$MAX_SYMBOLS" \
  --argjson groups "$MAX_GROUPS" \
  '{target: $target, sections: $sections, symbols: $symbols, groups: $groups}')
measure_runtime "$MAX_ARGS"
MAX_JSON=$CALL_OUT
printf '%s\n' "$MAX_JSON" >"$RUN_DIR/artifacts/app-size-max.json"

jq -e --argjson small "$SMALL_JSON" '
  .available == true
  and .pid == $small.pid
  and .target == $small.target
  and .executable == $small.executable
  and .file_bytes == $small.file_bytes
  and .section_bytes == $small.section_bytes
  and .section_count == $small.section_count
  and .symbol_bytes == $small.symbol_bytes
  and .symbol_count == $small.symbol_count
  and .group_count == $small.group_count
' >/dev/null 2>&1 <<<"$MAX_JSON" || failed \
  "The two reads disagree on the measured executable or totals. small=$(jq -c '{pid, executable, file_bytes, section_bytes, section_count, symbol_bytes, symbol_count, group_count}' <<<"$SMALL_JSON") max=$(jq -c '{pid, executable, file_bytes, section_bytes, section_count, symbol_bytes, symbol_count, group_count}' <<<"$MAX_JSON")"
pass "totals are stable across limits: $(jq -r '.file_bytes' <<<"$MAX_JSON") file bytes, $(jq -r '.section_count' <<<"$MAX_JSON") sections, $(jq -r '.symbol_count' <<<"$MAX_JSON") symbols, $(jq -r '.group_count' <<<"$MAX_JSON") groups"

check_truncation "$MAX_JSON" "$MAX_SECTIONS" "$MAX_SYMBOLS" "$MAX_GROUPS" || failed \
  "Truncation flags and counts disagree at limits ${MAX_SECTIONS}/${MAX_SYMBOLS}/${MAX_GROUPS}: $(truncation_summary "$MAX_JSON")"

jq -e 'if .sections_truncated then ([.sections[].size] | add // 0) <= .section_bytes else ([.sections[].size] | add // 0) == .section_bytes end' \
  >/dev/null 2>&1 <<<"$MAX_JSON" || failed \
  "Returned section sizes do not add up to section_bytes: $(jq -c '{sum: ([.sections[].size] | add), section_bytes, truncated: .sections_truncated}' <<<"$MAX_JSON")"
jq -e 'if .symbols_truncated then ([.symbols[].size] | add // 0) <= .symbol_bytes else ([.symbols[].size] | add // 0) == .symbol_bytes end' \
  >/dev/null 2>&1 <<<"$MAX_JSON" || failed \
  "Returned symbol sizes do not agree with symbol_bytes: $(jq -c '{sum: ([.symbols[].size] | add), symbol_bytes, truncated: .symbols_truncated}' <<<"$MAX_JSON")"
jq -e 'if .groups_truncated then ([.groups[].bytes] | add // 0) <= .symbol_bytes and ([.groups[].symbols] | add // 0) <= .symbol_count else ([.groups[].bytes] | add // 0) == .symbol_bytes and ([.groups[].symbols] | add // 0) == .symbol_count end' \
  >/dev/null 2>&1 <<<"$MAX_JSON" || failed \
  "Group sums do not agree with the symbol totals: $(jq -c '{group_bytes: ([.groups[].bytes] | add), symbol_bytes, group_symbols: ([.groups[].symbols] | add), symbol_count, truncated: .groups_truncated}' <<<"$MAX_JSON")"

jq -n --argjson small "$SMALL_JSON" --argjson max "$MAX_JSON" '
  ($small.sections[0] == $max.sections[0])
  and ($small.symbols[0] == $max.symbols[0])
  and ($small.groups[0] == $max.groups[0])
' >/dev/null 2>&1 || failed \
  "The same executable returned different top rows at different limits: $(jq -c '.' <<<"$MAX_JSON" >/dev/null 2>&1 && printf 'small=%s max=%s' "$(jq -c '{section: .sections[0], symbol: .symbols[0], group: .groups[0]}' <<<"$SMALL_JSON")" "$(jq -c '{section: .sections[0], symbol: .symbols[0], group: .groups[0]}' <<<"$MAX_JSON")")"
pass "top rows are deterministic across limits: $(jq -r '.sections[0].name' <<<"$MAX_JSON") / $(jq -r '.symbols[0].name' <<<"$MAX_JSON") / $(jq -r '.groups[0].name' <<<"$MAX_JSON")"

# Mach-O prefixes every symbol with an underscore, so legacy Rust names appear
# as __ZN... and Rust v0 names as __R... (ELF keeps _ZN.../_R...).
DEMANGLED=$(jq -r '[.symbols[] | select((.raw | test("^__?(ZN|R)")) and (.name != .raw) and (.name | test("^_Z|^_R") | not) and (.name | test("::")))] | length' <<<"$MAX_JSON")
[ "$DEMANGLED" -gt 0 ] || failed \
  "None of the $(jq -r '.symbols | length' <<<"$MAX_JSON") returned symbols demangled a Rust symbol: $(jq -c '[.symbols[] | select(.raw | test("^__?(ZN|R)")) | {raw, name}] | .[0:3]' <<<"$MAX_JSON")"
RUST_GROUP=$(jq -r '[.groups[] | select(.name | test("^cove_|^gallery$"))] | .[0].name // empty' <<<"$MAX_JSON")
[ -n "$RUST_GROUP" ] || failed \
  "No returned group is a real gallery/cove crate; group names are $(jq -c '[.groups[].name] | .[0:10]' <<<"$MAX_JSON")"
pass "demangled names: $(jq -r '[.symbols[] | select((.raw | test("^__?(ZN|R)")) and (.name != .raw))] | .[0] | "\(.raw[0:72]) -> \(.name[0:96])"' <<<"$MAX_JSON")"
pass "crate groups name real Rust crates: $RUST_GROUP"

step "Assert a non-runtime target is explicitly unavailable"
cove_call get_ui_session '{}'
[ "$CALL_STATUS" -eq 0 ] || blocked \
  "get_ui_session failed after runtime selection (code $(call_error_code)): $(call_message)" \
  "Inspect controller.log; the session must list platform targets for the unavailable assertion."
SESSION_JSON=$CALL_OUT
TARGET_ID=$(jq -r '[.targets[]? | select(.kind == "display")][0].id // empty' <<<"$SESSION_JSON")
TARGET_KIND=display
if [ -z "$TARGET_ID" ]; then
  TARGET_ID=$(jq -r '[.targets[]? | select(.kind == "window" or .kind == "webdriver" or .kind == "portal" or .kind == "process")][0].id // empty' <<<"$SESSION_JSON")
  TARGET_KIND=$(jq -r --arg id "$TARGET_ID" '[.targets[]? | select(.id == $id)][0].kind // empty' <<<"$SESSION_JSON")
fi
[ -n "$TARGET_ID" ] || blocked \
  "The controller lists no display, window, webdriver, portal or process target to prove the unavailable path." \
  "Run this harness on a macOS GUI session so the platform adapter lists a display or window target."
say "Non-runtime target: $TARGET_ID ($TARGET_KIND)"
NON_RUNTIME_ARGS=$(jq -cn --arg target "$TARGET_ID" '{target: $target}')
cove_call get_ui_app_size "$NON_RUNTIME_ARGS"
printf '%s\n' "$CALL_OUT" >"$RUN_DIR/artifacts/app-size-non-runtime.json"
[ "$CALL_STATUS" -ne 0 ] || failed \
  "get_ui_app_size returned data for $TARGET_KIND target $TARGET_ID; a platform target has no runtime executable and must be unavailable."
NON_RUNTIME_CODE=$(call_error_code)
[ "$NON_RUNTIME_CODE" = "unavailable" ] || failed \
  "get_ui_app_size on $TARGET_KIND target $TARGET_ID answered code '${NON_RUNTIME_CODE:-none}' instead of unavailable: $(call_message)"
jq -e '
  ((.result? // null) == null)
  and ((.data.code? == "unavailable") or (.error.data.code? == "unavailable") or (.code? == "unavailable"))
' >/dev/null 2>&1 <<<"$CALL_OUT" \
  || failed "The unavailable answer is not an error carrying code=unavailable: $(jq -c . <<<"$CALL_OUT" 2>/dev/null || printf 'unparseable response')"
jq -e '((.error.message? // .message? // "") | test("native executable of a running Cove runtime")) and ((.error.message? // .message? // "") | test("platform window, display or browser session"))' \
  >/dev/null 2>&1 <<<"$CALL_OUT" || failed \
  "The unavailable message does not name the missing runtime executable: $(jq -r '.error.message? // .message? // empty' <<<"$CALL_OUT")"
jq -e '
  def size_fields: has("file_bytes") or has("sections") or has("symbols") or has("groups") or has("available");
  ((size_fields) | not) and (((.error? // {}) | size_fields) | not) and (((.data? // {}) | size_fields) | not)
' >/dev/null 2>&1 <<<"$CALL_OUT" || failed \
  "The unavailable answer carries App Size fields: $(jq -c . <<<"$CALL_OUT")"
pass "non-runtime $TARGET_KIND target $TARGET_ID -> unavailable: $(jq -r '.error.message? // .message? // empty' <<<"$CALL_OUT")"

say ""
say "Checks: $PASS_COUNT"
say "RESULT PASS  App Size: native executable sections, symbols and crate groups"
exit 0
