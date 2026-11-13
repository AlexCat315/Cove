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

