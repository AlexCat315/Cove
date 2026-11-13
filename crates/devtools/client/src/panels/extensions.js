/* Extensions
 *
 * A Cove package registers a DevTools panel at runtime through the extension
 * API. The runtime advertises the `extensions` capability when at least one
 * source is installed; this view lists the registered panels, lets the user
 * enable or disable each for the session, and opens one in a sandboxed frame.
 * This is the Cove equivalent of Flutter's DevTools extension framework.
 */

function emptyExtensions() {
  return {
    list: [], loaded: false, loading: false, error: null, request: 0,
    openedId: null,
  };
}

function ensureExtensions() {
  if (!state.token || !state.targetId || !state.caps.extensions) {
    renderExtensions();
    return;
  }
  if (!state.extensions.loaded && !state.extensions.loading) loadExtensions({ quiet: true });
  else renderExtensions();
}

async function loadExtensions(options = {}) {
  const extensions = state.extensions;
  if (!state.token || !state.targetId || !state.caps.extensions) {
    renderExtensions();
    return;
  }
  if (extensions.loading) return;
  const request = extensions.request + 1;
  const targetId = state.targetId;
  extensions.request = request;
  extensions.loading = true;
  extensions.error = null;
  if (options.reset) extensions.loaded = false;
  renderExtensions();
  try {
    const data = unwrapData(await rpc("list_extensions", { target: targetId }));
    if (request !== state.extensions.request || targetId !== state.targetId) return;
    state.extensions.list = Array.isArray(data?.extensions) ? data.extensions : [];
    state.extensions.loaded = true;
  } catch (error) {
    if (request !== state.extensions.request || targetId !== state.targetId) return;
    state.extensions.error = error;
    if (options.quiet !== true) toast(errorMessage(error), "error", 8000);
  } finally {
    if (request === state.extensions.request && targetId === state.targetId) {
      state.extensions.loading = false;
      renderExtensions();
    }
  }
}

async function setExtensionEnabled(id, enabled) {
  if (!state.token || !state.targetId) return;
  try {
    const data = unwrapData(await rpc("set_extension_enabled", {
      target: state.targetId, id, enabled,
    }));
    if (data?.ok !== true) {
      toast(`Extension ${id} is not registered in this runtime.`, "error", 5000);
    }
  } catch (error) {
    toast(errorMessage(error), "error", 8000);
  }
  await loadExtensions({ quiet: true });
}

// Only load entry points the runtime can legitimately publish. A `javascript:`
// URL (or any other scheme) is never turned into a frame source.
function extensionEntryUrl(entryPath) {
  const value = String(entryPath || "").trim();
  if (!value) return null;
  if (value.startsWith("data:text/html,")) return value;
  if (/^(https?:\/\/|\/|\.\/)/i.test(value)) return value;
  return null;
}

function setExtensionsChrome() {
  const extensions = state.extensions;
  dom.extensionsRefresh.disabled = !state.caps.extensions || extensions.loading;
  if (!state.token || !state.targetId) {
    dom.extensionsMeta.textContent = "No target selected.";
  } else if (!state.caps.extensions) {
    dom.extensionsMeta.textContent = "Unavailable for this target.";
  } else if (!extensions.loaded) {
    dom.extensionsMeta.textContent = "";
  } else {
    const enabled = extensions.list.filter(item => item.enabled === true).length;
    dom.extensionsMeta.textContent = `${extensions.list.length} registered · ${enabled} enabled`;
  }
}

function renderExtensions() {
  const host = dom.extensionsContent;
  clear(host);
  setExtensionsChrome();
  const extensions = state.extensions;
  if (!state.token) {
    showState(host, { icon: "device", title: "Not connected", message: "Connect to a controller to list DevTools extensions." });
    return;
  }
  if (!targetById()) {
    showState(host, { icon: "device", title: "No target", message: "Select a connected Cove runtime to list its DevTools extensions." });
    return;
  }
  if (!state.caps.extensions) {
    showState(host, {
      icon: "info",
      title: "Extensions are unavailable",
      message: "This runtime does not advertise the DevTools extension capability. A package opts in by registering an extension source.",
    });
    return;
  }
  if (extensions.loading && !extensions.loaded) {
    showState(host, { kind: "loading", title: "Reading extensions" });
    return;
  }
  if (extensions.error && !extensions.loaded) {
    showState(host, {
      kind: "error", title: "Extension read failed",
      message: errorMessage(extensions.error), action: "Retry",
      onAction: () => loadExtensions({ reset: true }),
    });
    return;
  }
  if (!extensions.loaded) {
    showState(host, {
      icon: "extension", title: "DevTools extensions",
      message: "Packages register DevTools panels through the extension API. Each appears here with an enable toggle and opens in a sandboxed frame.",
      action: "Read extensions", primary: true,
      onAction: () => loadExtensions({ reset: true }),
    });
    return;
  }
  if (extensions.error) host.append(errorInline(errorMessage(extensions.error)));
  if (!extensions.list.length) {
    host.append(emptyInline("This runtime exposes no DevTools extensions. A package opts in by registering an extension panel."));
    return;
  }
  for (const extension of extensions.list) host.append(renderExtensionCard(extension));
  const opened = extensions.list.find(item => item.id === extensions.openedId);
  if (opened) host.append(renderExtensionFrame(opened));
}

function renderExtensionCard(extension) {
  const entry = extensionEntryUrl(extension.entryPath);
  const enabled = extension.enabled === true;
  const actions = el("div", { class: "aux-actions" },
    el("label", { class: "check", attrs: { title: "Enable or disable this extension panel for the current target" } },
      el("input", {
        attrs: { type: "checkbox", "aria-label": `Enable ${textOf(extension.name, extension.id)}` },
        props: { checked: enabled, disabled: !state.caps.extensions },
        on: { change: event => setExtensionEnabled(extension.id, event.target.checked) },
      }),
      el("span", { text: enabled ? "Enabled" : "Disabled" }),
    ),
  );
  if (entry && enabled) {
    actions.append(el("button", {
      class: "tool-button",
      props: { type: "button" },
      text: state.extensions.openedId === extension.id ? "Close panel" : "Open panel",
      on: { click: () => {
        state.extensions.openedId = state.extensions.openedId === extension.id ? null : extension.id;
        renderExtensions();
      } },
    }));
  }
  return sectionBlock(textOf(extension.name, extension.id), actions,
    kvList([
      ["ID", extension.id],
      ["Version", extension.version],
      ["Description", extension.description],
      ["Homepage", extension.homepage],
      ["Panel", entry ? "available" : "no entry point"],
    ]),
  );
}

function renderExtensionFrame(extension) {
  const url = extensionEntryUrl(extension.entryPath);
  if (!url) return emptyInline("This extension has no loadable entry point.");
  return el("div", { class: "section extension-frame-section" },
    el("h3", { text: `${textOf(extension.name, extension.id)} panel` }),
    el("iframe", {
      attrs: {
        src: url,
        title: `${textOf(extension.name, extension.id)} extension panel`,
        sandbox: "allow-scripts allow-forms allow-popups",
        loading: "lazy",
        style: "width:100%;height:420px;border:1px solid var(--border, #333);border-radius:6px;background:#fff",
      },
    }),
  );
}

function resetExtensions() {
  const request = state.extensions.request + 1;
  state.extensions = emptyExtensions();
  state.extensions.request = request;
  if (state.view === "extensions") renderExtensions();
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

