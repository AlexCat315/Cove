/* Keyboard and text input */

function renderInputUi() {
  const caps = state.caps;
  const target = targetById();
  const disabled = !caps.input || state.inputBusy;
  dom.keyForm.querySelectorAll("input, button").forEach(control => { control.disabled = disabled; });
  dom.textForm.querySelectorAll("input, button").forEach(control => { control.disabled = disabled; });
  dom.inputMeta.textContent = caps.input
    ? `App input · ${target?.title || "target"}`
    : "This target does not advertise input.";
  updateInputButtons();
}

function updateInputButtons() {
  const disabled = !state.caps.input || state.inputBusy;
  dom.sendKey.disabled = disabled || !dom.keyValue.value.trim();
  dom.sendText.disabled = disabled || !dom.textValue.value.trim();
}

function setInputBusy(busy) {
  state.inputBusy = busy;
  renderInputUi();
}

async function sendKey(event) {
  event?.preventDefault();
  if (!state.caps.input || state.inputBusy) return;
  const key = dom.keyValue.value.trim();
  if (!key) return;
  const modifiers = [...dom.keyForm.querySelectorAll('input[name="modifier"]:checked')].map(input => input.value);
  const targetId = state.targetId;
  setInputBusy(true);
  try {
    const result = await rpc("press_ui_key", { target: targetId, key, modifiers });
    if (targetId !== state.targetId) return;
    toast(`Key delivered${result?.source ? ` via ${result.source}` : ""}. Verify the resulting UI state.`, "success", 5000);
  } catch (error) {
    toast(errorMessage(error), "error", 9000);
  } finally {
    setInputBusy(false);
  }
}

async function sendText(event) {
  event?.preventDefault();
  if (!state.caps.input || state.inputBusy) return;
  const text = dom.textValue.value;
  if (!text) return;
  const targetId = state.targetId;
  setInputBusy(true);
  try {
    const result = await rpc("type_ui_text", { target: targetId, text });
    if (targetId !== state.targetId) return;
    toast(`Text delivered${result?.source ? ` via ${result.source}` : ""}. Verify the resulting UI state.`, "success", 5000);
  } catch (error) {
    toast(errorMessage(error), "error", 9000);
  } finally {
    setInputBusy(false);
  }
}

function toggleInputDrawer() {
  const expanded = dom.inputToggle.getAttribute("aria-expanded") === "true";
  dom.inputToggle.setAttribute("aria-expanded", String(!expanded));
  dom.inputToggle.textContent = expanded ? "Show" : "Hide";
  dom.inputBody.hidden = expanded;
}

