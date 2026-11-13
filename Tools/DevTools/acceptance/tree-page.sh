#!/usr/bin/env bash
# Paged tree acceptance for Cove DevTools: query_ui_tree under the agent budget.
#
# The browser client can read the full `inspect_ui_tree` summary, but the agent
# and MCP toolbox refuses any tool result over 512 KiB, and a live gallery tree
# is ~1.1 MB. This harness proves the bounded query is the agent's way in:
#
#   1. Start an isolated controller and the real gallery on a non-default port.
#   2. Fetch the live full tree through the same controller HTTP route the CLI
#      uses (not through the bounded tool path), and record that it exceeds the
#      512 KiB agent result budget.
#   3. Call the unpaged `inspect_ui_tree` through `cove-devtools call` and
#      require the explicit budget refusal.
#   4. Call `query_ui_tree` at depth 2 / limit 200 and depth 1 / limit 50.
#      Require valid JSON, a non-empty root and node count, real node IDs that
#      exist in the live full tree, a depth-bounded result, a node count at most
#      the requested limit, truncation exactly when nodes were omitted, and a
#      compact result under 512 KiB.
#   5. Call `query_ui_tree` with one live node ID and require that subtree root.
#   6. Call it with a nonexistent node ID, an out-of-range depth and an
#      out-of-range limit; require an explicit error for each, never a page of
#      invented nodes. The CLI schema also rejects out-of-range bounds before
#      the controller, so the harness calls the controller route directly to
#      pin its `invalid_request` answer.
#
# No nodes are synthesized: every ID and every child edge asserted here came
# from the live gallery through the controller.
#
# Usage: Tools/DevTools/acceptance/tree-page.sh [--keep] [--no-build]
#
# Environment:
#   ACCEPTANCE_GALLERY_TIMEOUT  seconds to wait for a registered runtime (default 120)
#
# Exit codes: 0 pass, 1 functional assertion failure, 2 blocked (missing
# dependency, missing product tool/schema, build or startup failure, no live
# runtime). Interrupt exits 130/143 after cleanup. See
# Docs/devtools-acceptance.md for the full contract.
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
KEEP_ARTIFACTS=0
BUILD=1
# The agent/MCP toolbox result limit from crates/agent/src/tool.rs.
AGENT_RESULT_LIMIT=524288
DEPTH_PAGE=2
LIMIT_PAGE=200
DEPTH_SHALLOW=1
LIMIT_SHALLOW=50
# A deeper page that the node limit, not the depth, cuts: the live gallery has
# 18 nodes within the maximum depth 16, so limit 10 must return exactly 10 and
# truncate. Depth stays inside the tool's 1..=16 contract.
DEPTH_LIMIT=16
LIMIT_CAP=10

for argument in "$@"; do
  case "$argument" in
    --keep) KEEP_ARTIFACTS=1 ;;
    --no-build) BUILD=0 ;;
    -h|--help)
      sed -n '2,37p' "$0"
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

step "Preflight"
for command in cargo python3 jq curl; do
  command -v "$command" >/dev/null 2>&1 || blocked \
    "Required command '$command' is not available." \
    "Install '$command' and rerun."
done

TMP_ROOT=${TMPDIR:-/tmp}
TMP_ROOT=${TMP_ROOT%/}
[ -n "$TMP_ROOT" ] || TMP_ROOT=/tmp
RUN_DIR=$(mktemp -d "$TMP_ROOT/cove-devtools-tree-page.XXXXXX") || blocked \
  "Cannot create an isolated run directory." \
  "Check TMPDIR permissions and free space."
if ! mkdir -p "$RUN_DIR/logs" "$RUN_DIR/session" "$RUN_DIR/pages"; then
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

tool_exists query_ui_tree || blocked \
  "query_ui_tree is not registered by the controller." \
  "Restore the paged tree tool in the controller; inspect_ui_tree alone cannot reach an agent under the 512 KiB result limit."
tool_exists inspect_ui_tree || blocked \
  "inspect_ui_tree is not registered; the overflow comparison cannot run." \
  "Restore the existing full-tree tool before running acceptance."
for property in depth limit; do
  tool_has_property query_ui_tree "$property" || blocked \
    "query_ui_tree has no '$property' argument; the paging assertions cannot run." \
    "Restore the depth/limit arguments or update the harness with the documented contract."
done
tool_has_property query_ui_tree node_id || blocked \
  "query_ui_tree has no node_id argument; the bounded-subtree and stale-node assertions cannot run." \
  "Restore the node_id argument or update the harness with the documented contract."

if ! unexpected=$(jq -r --arg name "query_ui_tree" \
  '.tools[]? | select(.name == $name) | (.inputSchema.required // [])[] | select(. != "target")' \
  "$RUN_DIR/tools.json"); then
  blocked "Cannot read the query_ui_tree schema." "Inspect tools.json and rerun."
fi
[ -z "$unexpected" ] || blocked \
  "query_ui_tree requires argument(s) this harness does not send: $(printf '%s ' $unexpected)" \
  "Align the harness with the tool contract in Docs/devtools-acceptance.md."

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
LIVE_DISCOVERY="$HOME/Library/Application Support/Cove/devtools-controller.json"
if [ "$COVE_DEVTOOLS_SESSION_FILE" = "$LIVE_DISCOVERY" ]; then
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
    message=$(head -c 600 "$CALL_ERR" || true)
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
[ "$RUNTIME_PID" = "$GALLERY_PID" ] || failed \
  "The selected runtime pid $RUNTIME_PID is not this run's gallery pid $GALLERY_PID."
pass "runtime selected: $RUNTIME_ID ($RUNTIME_TITLE, pid $RUNTIME_PID)"

# The controller's own JSON-RPC route, used only as live ground truth for the
# node IDs and subtree shape the paged tool must return. It is not the agent
# tool path: the size assertion below shows why the agent cannot use it.
rpc_full_tree() {
  curl --silent --show-error --max-time 60 \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    --data '{"jsonrpc":"2.0","id":1,"method":"inspect_ui_tree","params":{}}' \
    "$COVE_DEVTOOLS_ENDPOINT/rpc"
}

step "Live full tree and the unpaged tool"
if ! rpc_full_tree >"$RUN_DIR/pages/full-tree.json" 2>"$RUN_DIR/logs/full-tree.err"; then
  tail -n 20 "$RUN_DIR/logs/full-tree.err" || true
  failed "The controller did not return a full tree over its JSON-RPC route."
fi
if ! jq -e '.result.root.id | type == "string" and length > 0' \
  "$RUN_DIR/pages/full-tree.json" >/dev/null 2>&1; then
  failed "The live full tree has no root node: $(jq -r '.error.message // "no result"' "$RUN_DIR/pages/full-tree.json" 2>/dev/null || true)"
fi
FULL_ROOT_ID=$(jq -r '.result.root.id' "$RUN_DIR/pages/full-tree.json")
FULL_COMPACT_BYTES=$(jq -c '.result' "$RUN_DIR/pages/full-tree.json" | wc -c | tr -d ' ')
# Flatten the live tree once: id, depth, absolute frame and real child IDs per
# node.
jq -c '
  def flat($depth):
    {id: .id, depth: $depth, frame: .absoluteFrame, kids: [.children[]?.id]},
    (.children[]? | flat($depth + 1));
  [.result.root | flat(0)]
' "$RUN_DIR/pages/full-tree.json" >"$RUN_DIR/pages/full-nodes.json"
FULL_NODES=$(jq 'length' "$RUN_DIR/pages/full-nodes.json")
[ "$FULL_NODES" -gt 1 ] || failed \
  "The live tree has $FULL_NODES node(s); the gallery tree must have real depth and breadth for paging."
FULL_MAX_DEPTH=$(jq '[.[].depth] | max' "$RUN_DIR/pages/full-nodes.json")
jq -r '.[].id' "$RUN_DIR/pages/full-nodes.json" | sort >"$RUN_DIR/pages/full.ids"
say "Live tree: $FULL_NODES nodes, max depth $FULL_MAX_DEPTH, root $FULL_ROOT_ID, compact JSON $FULL_COMPACT_BYTES bytes"

if [ "$FULL_COMPACT_BYTES" -le "$AGENT_RESULT_LIMIT" ]; then
  blocked "The live full tree is $FULL_COMPACT_BYTES bytes, under the $AGENT_RESULT_LIMIT-byte agent budget; this run cannot demonstrate the overflow the paged tool exists for." \
    "Run the harness against the gallery, whose tree is ~1.1 MB, or document the smaller app's own paging need."
fi
pass "live full tree exceeds the agent budget: $FULL_COMPACT_BYTES bytes > $AGENT_RESULT_LIMIT"

cove_call inspect_ui_tree '{}'
if [ "$CALL_STATUS" -eq 0 ]; then
  failed "inspect_ui_tree succeeded through cove-devtools call ($(wc -c <<<"$CALL_OUT" | tr -d ' ') bytes printed); the unpaged tool is expected to exceed the 512 KiB agent result limit."
fi
UNPAGED_MESSAGE=$(call_message)
case "$UNPAGED_MESSAGE" in
  *"512 KiB"*|*"result limit"*|*"InvalidResult"*|*"invalid result"*) ;;
  *) failed "inspect_ui_tree failed for an unexpected reason: $UNPAGED_MESSAGE" ;;
esac
pass "unpaged inspect_ui_tree refused by the agent budget: $UNPAGED_MESSAGE"

# Helpers over one query_ui_tree response.
page_count() {
  jq -r '
    if (.nodeCount? | type) == "number" then .nodeCount
    elif (.node_count? | type) == "number" then .node_count
    else empty end
  ' "$1"
}
page_truncated() {
  jq -r 'if (.truncated? | type) == "boolean" then .truncated else empty end' "$1"
}
page_has_root() {
  jq -e '(.root.id? | type) == "string" and (.root.id | length) > 0' "$1" >/dev/null 2>&1
}
page_flatten() {
  jq -c '
    def flat($depth):
      {id: .id, depth: $depth, frame: .absoluteFrame, kids: [.children[]?.id]},
      (.children[]? | flat($depth + 1));
    [.root | flat(0)]
  ' "$1"
}

# Assert one page against the live full tree.
# $1 label, $2 response file, $3 depth, $4 limit
assert_page() {
  label=$1
  file=$2
  depth=$3
  limit=$4
  jq -e . "$file" >/dev/null 2>&1 || failed "$label returned invalid JSON."
  page_has_root "$file" || failed "$label has no live root node."
  count=$(page_count "$file")
  [ -n "$count" ] || failed "$label has no explicit nodeCount/node_count."
  page_truncated "$file" >/dev/null 2>&1 || true
  truncated=$(page_truncated "$file")
  [ -n "$truncated" ] || failed "$label has no explicit boolean truncated flag."

  page_flatten "$file" >"$RUN_DIR/pages/$label.nodes.json"
  returned=$(jq 'length' "$RUN_DIR/pages/$label.nodes.json")
  [ "$returned" -eq "$count" ] || failed \
    "$label reports nodeCount $count but the root subtree holds $returned nodes."
  [ "$returned" -gt 0 ] || failed "$label returned an empty tree."
  [ "$returned" -le "$limit" ] || failed \
    "$label returned $returned nodes, over its limit $limit."
  max_depth=$(jq '[.[].depth] | max' "$RUN_DIR/pages/$label.nodes.json")
  [ "$max_depth" -le $((depth - 1)) ] || failed \
    "$label returned $((max_depth + 1)) node levels, over the requested depth $depth (the root is level 1)."
  duplicates=$(jq '[.[].id] | length - (unique | length)' "$RUN_DIR/pages/$label.nodes.json")
  [ "$duplicates" -eq 0 ] || failed "$label contains $duplicates duplicate node ID(s)."

  # Every returned ID must exist in the live full tree.
  jq -r '.[].id' "$RUN_DIR/pages/$label.nodes.json" | sort -u >"$RUN_DIR/pages/$label.ids"
  if [ -s "$RUN_DIR/pages/$label.ids" ] \
    && [ -n "$(comm -23 "$RUN_DIR/pages/$label.ids" "$RUN_DIR/pages/full.ids")" ]; then
    failed "$label returned node ID(s) that are not in the live tree: $(comm -23 "$RUN_DIR/pages/$label.ids" "$RUN_DIR/pages/full.ids" | head -n 3 | tr '\n' ' ')"
  fi

  # Geometry and edges must be the live node's, not a stand-in.
  if ! jq -e -n \
    --slurpfile page "$RUN_DIR/pages/$label.nodes.json" \
    --slurpfile full "$RUN_DIR/pages/full-nodes.json" '
      ($full[0] | map({key: .id, value: {frame: .frame, kids: .kids}}) | from_entries) as $live
      | [ $page[0][]
          | select(.frame != $live[.id].frame
              or .kids != ($live[.id].kids[: (.kids | length)]) ) ]
      | length == 0
    ' >/dev/null; then
    failed "$label changed a live node's absolute frame or child order."
  fi

  within=$(jq --argjson depth "$depth" '[.[] | select(.depth < $depth)] | length' \
    "$RUN_DIR/pages/full-nodes.json")
  deeper=$(jq --argjson depth "$depth" '[.[] | select(.depth >= $depth)] | length' \
    "$RUN_DIR/pages/full-nodes.json")
  if [ "$within" -gt "$limit" ]; then
    limit_cut=1
  else
    limit_cut=0
  fi
  if [ "$deeper" -gt 0 ]; then
    depth_cut=1
  else
    depth_cut=0
  fi
  if [ "$limit_cut" -eq 1 ]; then
    expected_count=$limit
  else
    expected_count=$within
  fi
  [ "$count" -eq "$expected_count" ] || failed \
    "$label reports $count nodes; the live tree has $within nodes within depth $depth and the limit is $limit, so the prefix is $expected_count."
  if [ "$limit_cut" -eq 1 ] || [ "$depth_cut" -eq 1 ]; then
    expected_truncated=true
  else
    expected_truncated=false
  fi
  [ "$truncated" = "$expected_truncated" ] || failed \
    "$label reports truncated=$truncated; live nodes omitted: within-depth $within (limit $limit), deeper $deeper."
  bytes=$(jq -c . "$file" | wc -c | tr -d ' ')
  [ "$bytes" -lt "$AGENT_RESULT_LIMIT" ] || failed \
    "$label result is $bytes compact bytes, over the $AGENT_RESULT_LIMIT-byte agent budget."
  pass "$label: $count nodes, $((max_depth + 1)) of $depth levels, truncated=$truncated, $bytes bytes (< $AGENT_RESULT_LIMIT), root $FULL_ROOT_ID"
}

step "query_ui_tree depth $DEPTH_PAGE / limit $LIMIT_PAGE"
ARGS=$(jq -cn --arg target "$RUNTIME_ID" \
  --argjson depth "$DEPTH_PAGE" --argjson limit "$LIMIT_PAGE" \
  '{target: $target, depth: $depth, limit: $limit}')
cove_call query_ui_tree "$ARGS"
[ "$CALL_STATUS" -eq 0 ] || failed \
  "query_ui_tree depth $DEPTH_PAGE limit $LIMIT_PAGE failed (code $(call_error_code)): $(call_message)"
printf '%s\n' "$CALL_OUT" >"$RUN_DIR/pages/page-depth$DEPTH_PAGE.json"
ROOT_ID=$(jq -r '.root.id' "$RUN_DIR/pages/page-depth$DEPTH_PAGE.json")
[ "$ROOT_ID" = "$FULL_ROOT_ID" ] || failed \
  "The unpaged-root page starts at $ROOT_ID instead of the live root $FULL_ROOT_ID."
assert_page "depth-$DEPTH_PAGE-limit-$LIMIT_PAGE" "$RUN_DIR/pages/page-depth$DEPTH_PAGE.json" \
  "$DEPTH_PAGE" "$LIMIT_PAGE"

step "query_ui_tree depth $DEPTH_SHALLOW / limit $LIMIT_SHALLOW"
ARGS=$(jq -cn --arg target "$RUNTIME_ID" \
  --argjson depth "$DEPTH_SHALLOW" --argjson limit "$LIMIT_SHALLOW" \
  '{target: $target, depth: $depth, limit: $limit}')
cove_call query_ui_tree "$ARGS"
[ "$CALL_STATUS" -eq 0 ] || failed \
  "query_ui_tree depth $DEPTH_SHALLOW limit $LIMIT_SHALLOW failed (code $(call_error_code)): $(call_message)"
printf '%s\n' "$CALL_OUT" >"$RUN_DIR/pages/page-depth$DEPTH_SHALLOW.json"
assert_page "depth-$DEPTH_SHALLOW-limit-$LIMIT_SHALLOW" \
  "$RUN_DIR/pages/page-depth$DEPTH_SHALLOW.json" "$DEPTH_SHALLOW" "$LIMIT_SHALLOW"

step "query_ui_tree depth $DEPTH_LIMIT / limit $LIMIT_CAP (limit cap)"
ARGS=$(jq -cn --arg target "$RUNTIME_ID" \
  --argjson depth "$DEPTH_LIMIT" --argjson limit "$LIMIT_CAP" \
  '{target: $target, depth: $depth, limit: $limit}')
cove_call query_ui_tree "$ARGS"
[ "$CALL_STATUS" -eq 0 ] || failed \
  "query_ui_tree depth $DEPTH_LIMIT limit $LIMIT_CAP failed (code $(call_error_code)): $(call_message)"
printf '%s\n' "$CALL_OUT" >"$RUN_DIR/pages/page-depth$DEPTH_LIMIT.json"
assert_page "depth-$DEPTH_LIMIT-limit-$LIMIT_CAP" \
  "$RUN_DIR/pages/page-depth$DEPTH_LIMIT.json" "$DEPTH_LIMIT" "$LIMIT_CAP"
[ "$(page_truncated "$RUN_DIR/pages/page-depth$DEPTH_LIMIT.json")" = true ] || failed \
  "The depth $DEPTH_LIMIT / limit $LIMIT_CAP page is not truncated even though the limit cut live nodes."
[ "$(page_count "$RUN_DIR/pages/page-depth$DEPTH_LIMIT.json")" -eq "$LIMIT_CAP" ] || failed \
  "The limit $LIMIT_CAP page did not return exactly $LIMIT_CAP nodes."

step "Bound a subtree by live node ID"
SUBTREE_ID=$(jq -r --arg root "$FULL_ROOT_ID" \
  'map(select(.depth == 1)) | .[0].id // empty' "$RUN_DIR/pages/full-nodes.json")
if [ -z "$SUBTREE_ID" ]; then
  warn "The gallery root has no child; skipping the positive subtree call."
else
  ARGS=$(jq -cn --arg target "$RUNTIME_ID" --arg node_id "$SUBTREE_ID" \
    '{target: $target, node_id: $node_id, depth: 2, limit: 20}')
  cove_call query_ui_tree "$ARGS"
  [ "$CALL_STATUS" -eq 0 ] || failed \
    "query_ui_tree on live node $SUBTREE_ID failed (code $(call_error_code)): $(call_message)"
  printf '%s\n' "$CALL_OUT" >"$RUN_DIR/pages/subtree.json"
  SUBTREE_ROOT=$(jq -r '.root.id // empty' "$RUN_DIR/pages/subtree.json")
  [ "$SUBTREE_ROOT" = "$SUBTREE_ID" ] || failed \
    "The subtree query rooted at $SUBTREE_ID returned root '$SUBTREE_ROOT'."
  SUBTREE_COUNT=$(page_count "$RUN_DIR/pages/subtree.json")
  [ -n "$SUBTREE_COUNT" ] && [ "$SUBTREE_COUNT" -gt 0 ] || failed \
    "The subtree query rooted at $SUBTREE_ID returned no nodes."
  jq -r '[.root | recurse(.children[]?)] | .[].id' "$RUN_DIR/pages/subtree.json" \
    | sort >"$RUN_DIR/pages/subtree.ids"
  if [ -n "$(comm -23 "$RUN_DIR/pages/subtree.ids" "$RUN_DIR/pages/full.ids")" ]; then
    failed "The subtree query invented node ID(s) not in the live tree."
  fi
  page_flatten "$RUN_DIR/pages/subtree.json" >"$RUN_DIR/pages/subtree.nodes.json"
  if ! jq -e -n \
    --slurpfile page "$RUN_DIR/pages/subtree.nodes.json" \
    --slurpfile full "$RUN_DIR/pages/full-nodes.json" '
      ($full[0] | map({key: .id, value: {frame: .frame, kids: .kids}}) | from_entries) as $live
      | [ $page[0][]
          | select(.frame != $live[.id].frame
              or .kids != ($live[.id].kids[: (.kids | length)]) ) ]
      | length == 0
    ' >/dev/null; then
    failed "The subtree query changed a live node's absolute frame or child order."
  fi
  pass "subtree query rooted at real node $SUBTREE_ID: $SUBTREE_COUNT nodes"
fi

step "Explicit errors for invalid node, depth and limit"
rpc_tool() {
  name=$1
  params=$2
  curl --silent --show-error --max-time 30 \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    --data "$(jq -cn --arg method "$name" --argjson params "$params" \
      '{jsonrpc: "2.0", id: 1, method: $method, params: $params}')" \
    "$COVE_DEVTOOLS_ENDPOINT/rpc"
}

# The agent tool path must refuse an invalid page with an explicit message.
assert_cli_error() {
  label=$1
  args=$2
  cove_call query_ui_tree "$args"
  [ "$CALL_STATUS" -ne 0 ] || failed "$label returned a page instead of an explicit error."
  message=$(call_message)
  [ -n "$message" ] && [ "$message" != "no controller message" ] || failed \
    "$label failed without an error message (stderr: $(head -c 300 "$CALL_ERR" 2>/dev/null || true))."
  case "$message" in
    *[Nn]ode*|*[Dd]epth*|*[Ll]imit*|*range*|*bounds*|*invalid*|*Invalid*) ;;
    *) failed "$label error does not name the invalid field: $message" ;;
  esac
  code=$(call_error_code)
  if [ -n "$code" ]; then
    say "CLI   $label -> [$code] $message"
  else
    say "CLI   $label -> schema refusal: $message"
  fi
}

# The controller itself must answer the documented fault code.
assert_rpc_error() {
  label=$1
  params=$2
  expected=$3
  field=$4
  response=$(rpc_tool query_ui_tree "$params")
  printf '%s\n' "$response" >"$RUN_DIR/pages/error-$(printf '%s' "$label" | tr ' /' '--').json"
  if jq -e '.result' >/dev/null 2>&1 <<<"$response"; then
    failed "$label returned a controller result instead of an error."
  fi
  code=$(jq -r '.error.data.code // .error.code // empty' <<<"$response" 2>/dev/null || true)
  message=$(jq -r '.error.message // empty' <<<"$response" 2>/dev/null || true)
  [ "$code" = "$expected" ] || failed \
    "$label answered code '$code' instead of '$expected': ${message:-no message}"
  case "$message" in
    *"$field"*) ;;
    *) failed "$label error message does not name '$field': ${message:-no message}" ;;
  esac
  pass "$label -> controller [$code]: $message"
}

STALE_ID="stale-node-00000000-0000-0000-0000-000000000000"
ARGS=$(jq -cn --arg target "$RUNTIME_ID" --arg node_id "$STALE_ID" \
  '{target: $target, node_id: $node_id, depth: 1, limit: 20}')
assert_cli_error "nonexistent node $STALE_ID" "$ARGS"
assert_rpc_error "nonexistent node $STALE_ID" "$ARGS" stale_node node

ARGS=$(jq -cn --arg target "$RUNTIME_ID" \
  '{target: $target, depth: 10000, limit: 20}')
assert_cli_error "depth 10000" "$ARGS"
assert_rpc_error "depth 10000" "$ARGS" invalid_request depth

ARGS=$(jq -cn --arg target "$RUNTIME_ID" \
  '{target: $target, depth: 2, limit: 0}')
assert_cli_error "limit 0" "$ARGS"
assert_rpc_error "limit 0" "$ARGS" invalid_request limit

step "Cleanup check"
if ! kill -0 "$GALLERY_PID" 2>/dev/null; then
  failed "The gallery exited during the run; the assertions above may describe a dead runtime."
fi
if ! kill -0 "$CONTROLLER_PID" 2>/dev/null; then
  failed "The controller exited during the run."
fi

printf '\nRESULT PASS  Paged tree: query_ui_tree under the agent budget, %s checks\n' "$PASS_COUNT"
exit 0
