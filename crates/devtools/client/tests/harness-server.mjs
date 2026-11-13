// Shared static asset serving for the fake-controller harnesses.
//
// The Inspector client is delivered as one /inspector.js ES module. Its source
// lives in client/src/ as ordered parts; src/manifest.json is the single source
// of truth for the order, and both the production controller (generated at build
// time by crates/devtools-cli/build.rs) and this helper concatenate them
// byte-for-byte. Every other client file is served straight from disk by path,
// so adding an asset needs no edit here. Each harness only supplies its own fake
// /rpc controller and its scenarios.
import { readFile } from "node:fs/promises";
import { dirname, extname, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const clientDir = resolve(here, "..");
const sourceDir = resolve(clientDir, "src");
const manifestPath = resolve(sourceDir, "manifest.json");

// Endpoint contract with index.html: the whole module arrives as one file.
const BUNDLE_PATH = "/inspector.js";
const BUNDLE_MIME = "text/javascript; charset=utf-8";

const MIME_BY_EXTENSION = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".webp": "image/webp",
  ".ico": "image/x-icon",
  ".woff": "font/woff",
  ".woff2": "font/woff2",
  ".ttf": "font/ttf",
  ".wasm": "application/wasm",
  ".txt": "text/plain; charset=utf-8",
};

let bundle = null;

async function inspectorBundle() {
  if (!bundle) {
    const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
    const parts = await Promise.all(
      manifest.parts.map(part => readFile(resolve(sourceDir, part), "utf8")),
    );
    bundle = parts.join("");
  }
  return bundle;
}

// Maps a request path to a file inside the client directory, refusing anything
// that escapes it. The tests/ tree and dotfiles are never served.
function clientFile(pathname) {
  let path;
  try {
    path = decodeURIComponent(pathname);
  } catch {
    return null;
  }
  if (path === "/") path = "/index.html";
  const file = resolve(clientDir, `.${path}`);
  const inside = relative(clientDir, file);
  if (!inside) return null;
  if (inside.startsWith("..")) return null;
  const segments = inside.split(sep);
  if (segments[0] === "tests") return null;
  if (segments.some(segment => segment.startsWith("."))) return null;
  return file;
}

// Writes the asset when pathname is a client file and returns true. A false
// return lets the harness fall through to its own 404 handling.
export async function serveClientAsset(response, pathname) {
  if (pathname === BUNDLE_PATH) {
    response.writeHead(200, { "content-type": BUNDLE_MIME, "cache-control": "no-store" });
    response.end(await inspectorBundle());
    return true;
  }
  const file = clientFile(pathname);
  if (!file) return false;
  let body;
  try {
    body = await readFile(file);
  } catch {
    return false;
  }
  response.writeHead(200, {
    "content-type": MIME_BY_EXTENSION[extname(file).toLowerCase()] ?? "application/octet-stream",
    "cache-control": "no-store",
  });
  response.end(body);
  return true;
}
