import { $, state } from "../core/globals.js";
import { clear, el, fmtNumber } from "../core/format.js";
import { errorMessage, rpc, unwrapData } from "../core/session.js";
import { emptyInline, errorInline } from "../core/ui-helpers.js";
import { loadTree, selectNode } from "../inspector/tree.js";
import { switchView } from "../shell/views.js";

const pageSize = 50;
const panels = {
  components: { hostId: "component-activity-content", capability: "componentActivity", method: "query_ui_component_activity" },
  semantics: { hostId: "semantics-content", capability: "semanticsTree", method: "query_ui_semantics" },
};

export function resetSceneDiagnostics() {
  for (const panel of Object.values(panels)) {
    panel.request = 0;
    panel.loaded = false;
    panel.loading = false;
    clear($(panel.hostId));
  }
}

export function ensureSceneDiagnostics(name) {
  const panel = panels[name];
  if (name === "components" && state.performance.snapshot) {
    panel.request = (panel.request ?? 0) + 1;
    panel.loaded = false;
    panel.loading = false;
    $(panel.hostId).replaceChildren(emptyInline("Open a live session to read component activity."));
    return;
  }
  if (!panel.loaded && !panel.loading) loadSceneDiagnostics(name);
}

async function revealNode(id) {
  const epoch = state.targetEpoch;
  switchView("inspector");
  if (!state.entries.has(id)) await loadTree({ preserveExpansion: true });
  if (epoch === state.targetEpoch) selectNode(id);
}

export async function loadSceneDiagnostics(name, offset = 0) {
  const panel = panels[name];
  const host = $(panel.hostId);
  clear(host);
  panel.loaded = false;
  if (!state.targetId || !state.caps[panel.capability]) {
    host.append(emptyInline("Unavailable for this runtime."));
    return;
  }
  if (name === "components" && state.performance.snapshot) {
    host.append(emptyInline("Open a live session to read component activity."));
    return;
  }
  const epoch = state.targetEpoch;
  const request = (panel.request ?? 0) + 1;
  panel.request = request;
  panel.loading = true;
  host.append(emptyInline("Reading…"));
  const params = { target: state.targetId, offset, limit: pageSize };
  if (name === "components") {
    const filters = new FormData($("component-activity-filter"));
    const text = filters.get("text")?.trim();
    if (text) params.text = text;
    params.sort_by = filters.get("sort");
  }
  try {
    const page = unwrapData(await rpc(panel.method, params));
    if (epoch !== state.targetEpoch || request !== panel.request) return;
    panel.loaded = true;
    renderPage(name, host, page, offset);
  } catch (error) {
    if (epoch !== state.targetEpoch || request !== panel.request) return;
    clear(host);
    host.append(errorInline(errorMessage(error)));
  } finally {
    if (epoch === state.targetEpoch && request === panel.request) panel.loading = false;
  }
}

function renderPage(name, host, page, offset) {
  clear(host);
  const components = name === "components";
  const headings = components
    ? ["Component", "Rebuilds", "Layouts", "Paints", "Last invalidation"]
    : ["Role", "Label", "State", "Input"];
  const rows = page.items.map(item => {
    const semantics = item.semantics || {};
    const name = components ? item.name : semantics.role;
    const link = el("button", {
      class: "tool-button", text: name,
      attrs: { title: item.parentID ? `Semantic parent #${item.parentID}` : `Node #${item.id}` },
      on: { click: () => revealNode(item.id) },
    });
    const values = components
      ? [item.buildCount, item.layoutCount, item.paintCount, item.reasons?.map(reason => [reason.kind, reason.detail].filter(Boolean).join(" · ")).join(", ")]
      : [semantics.label, Object.entries(semantics.state || {}).flatMap(([key, value]) =>
          value === true && key !== "is_enabled" ? [key.replace(/^is_/, "").replaceAll("_", " ")]
            : value === false && key === "is_enabled" ? ["disabled"] : []).join(", "),
        item.acceptsInput ? "Enabled" : "Disabled"];
    return el("tr", {},
      el("td", { attrs: components ? {} : { style: `padding-inline-start:${8 + Math.min(item.depth, 12) * 14}px` } }, link),
      values.map(value => el("td", { text: typeof value === "number" ? fmtNumber(value, 0) : value || "—" })),
    );
  });
  if (rows.length) {
    host.append(el("div", { class: "table-wrap" }, el("table", { class: "data-table" },
      el("thead", {}, el("tr", {}, headings.map(text => el("th", { text })))),
      el("tbody", {}, rows),
    )));
  } else {
    host.append(emptyInline(components ? "No matching components." : "No semantics in the viewport."));
  }
  host.append(el("div", { class: "aux-actions" },
    el("span", { class: "muted", text: rows.length ? `${offset + 1}–${offset + rows.length} of ${page.total}` : `${page.total} results` }),
    offset > 0 ? pageButton("Previous", name, Math.max(0, offset - pageSize)) : null,
    page.nextOffset != null ? pageButton("Next", name, page.nextOffset) : null,
  ));
}

function pageButton(label, name, offset) {
  return el("button", { class: "tool-button", text: label, on: { click: () => loadSceneDiagnostics(name, offset) } });
}
