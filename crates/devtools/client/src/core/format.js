function el(tag, options = {}, ...children) {
  const node = document.createElement(tag);
  if (options.class) node.className = options.class;
  if (options.text !== undefined && options.text !== null) node.textContent = String(options.text);
  if (options.attrs) {
    for (const [name, value] of Object.entries(options.attrs)) {
      if (value !== undefined && value !== null) node.setAttribute(name, String(value));
    }
  }
  if (options.props) {
    for (const [name, value] of Object.entries(options.props)) {
      if (value !== undefined) node[name] = value;
    }
  }
  if (options.dataset) {
    for (const [name, value] of Object.entries(options.dataset)) {
      if (value !== undefined) node.dataset[name] = String(value);
    }
  }
  if (options.on) {
    for (const [name, handler] of Object.entries(options.on)) node.addEventListener(name, handler);
  }
  appendChildren(node, children);
  return node;
}

function appendChildren(node, children) {
  for (const child of children.flat(Infinity)) {
    if (child === undefined || child === null || child === false) continue;
    node.append(child instanceof Node ? child : document.createTextNode(String(child)));
  }
}

function clearNode(node) {
  node.replaceChildren();
}

function icon(name, className = "icon") {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("class", className);
  svg.setAttribute("aria-hidden", "true");
  const use = document.createElementNS("http://www.w3.org/2000/svg", "use");
  use.setAttribute("href", `#i-${name}`);
  svg.append(use);
  return svg;
}

function clear(element) {
  element.replaceChildren();
}

function textOf(value, fallback = "—") {
  if (value === undefined || value === null || value === "") return fallback;
  return String(value);
}

function numberValue(value) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function fmtNumber(value, digits = 1) {
  const number = numberValue(value);
  return number === null ? "—" : number.toFixed(digits);
}

function fmtMs(value, digits = 2) {
  const number = numberValue(value);
  return number === null ? "—" : `${number.toFixed(digits)} ms`;
}

function fmtBytes(value) {
  const number = numberValue(value);
  if (number === null) return "—";
  if (number < 1024) return `${number} B`;
  if (number < 1024 * 1024) return `${(number / 1024).toFixed(1)} KB`;
  if (number < 1024 * 1024 * 1024) return `${(number / (1024 * 1024)).toFixed(1)} MB`;
  return `${(number / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function fmtClock(seconds) {
  const number = numberValue(seconds);
  if (number === null) return "—";
  const date = new Date(number * 1000);
  if (Number.isNaN(date.getTime())) return "—";
  try {
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit", fractionalSecondDigits: 3 });
  } catch {
    return date.toLocaleTimeString([], { hour12: false });
  }
}

function fmtDate(seconds) {
  const number = numberValue(seconds);
  if (number === null) return "—";
  const date = new Date(number * 1000);
  return Number.isNaN(date.getTime()) ? "—" : date.toLocaleString();
}

function fmtEpochMs(milliseconds) {
  const number = numberValue(milliseconds);
  if (number === null) return "—";
  const date = new Date(number);
  if (Number.isNaN(date.getTime())) return "—";
  try {
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit", fractionalSecondDigits: 3 });
  } catch {
    return date.toLocaleTimeString([], { hour12: false });
  }
}

function shortId(id) {
  const value = String(id ?? "");
  return value.length > 12 ? `${value.slice(0, 4)}…${value.slice(-5)}` : value;
}

function basename(path) {
  const value = String(path ?? "");
  return value.split(/[\\/]/).pop() || value;
}

function slug(value) {
  return String(value ?? "capture").trim().replace(/[^A-Za-z0-9._-]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 64) || "capture";
}

