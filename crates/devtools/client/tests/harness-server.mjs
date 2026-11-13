// Shared static asset serving for the fake-controller harnesses.
//
// client/manifest.json is the single source of truth for the served bundles:
// /index.html, /inspector.css and /inspector.js are each the byte-for-byte
// concatenation of their ordered parts. The production controller embeds those
// same parts at compile time (crates/devtools-cli/build.rs); this helper
// concatenates them the same way, and serves any other client file from disk by
// path. Each harness only supplies its own fake /rpc controller and scenarios.
import { readFile } from "node:fs/promises";
import { dirname, extname, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const clientDir = resolve(here, "..");
const manifestPath = resolve(clientDir, "manifest.json");

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

let bundles = null;

async function servedBundles() {
  if (!bundles) {
    const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
    bundles = new Map(await Promise.all(manifest.bundles.map(async bundle => {
      const parts = await Promise.all(
        bundle.parts.map(part => readFile(resolve(clientDir, bundle.root, part), "utf8")),
      );
      return [bundle.path, { media: bundle.media, body: parts.join("") }];
    })));
  }
  return bundles;
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
  const file = resolve(clientDir, `.${path}`);
  const inside = relative(clientDir, file);
  if (!inside) return null;
  if (inside.startsWith("..")) return null;
  const segments = inside.split(sep);
  if (segments[0] === "tests") return null;
  if (segments.some(segment => segment.startsWith("."))) return null;
  return file;
}

// Writes the asset when pathname is served and returns true. A false return lets
// the harness fall through to its own 404 handling.
export async function serveClientAsset(response, pathname) {
  const bundle = (await servedBundles()).get(pathname === "/" ? "/index.html" : pathname);
  if (bundle) {
    response.writeHead(200, { "content-type": bundle.media, "cache-control": "no-store" });
    response.end(bundle.body);
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
