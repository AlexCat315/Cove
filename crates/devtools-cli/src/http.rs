use crate::controller::Controller;
use cove_devtools_protocol::control::{
    ConnectionInfo, Fault, Lease, MAX_MESSAGE, Poll, Registration,
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;
use tiny_http::{Header, Method, Request, Response, Server};

#[derive(Deserialize)]
struct Command {
    jsonrpc: String,
    id: Value,
    method: String,
    #[serde(default = "empty")]
    params: Value,
}

fn empty() -> Value {
    json!({})
}

pub fn serve(
    port: u16,
    token: String,
    web_root: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (server, discovery) = bind(port, token)?;
    eprintln!("Inspector: {}", inspector_url(&discovery.info));
    run(&server, &discovery.info.token, web_root, &AtomicBool::new(false))
}

pub fn inspector_url(info: &ConnectionInfo) -> String {
    format!("{}/#{}", info.endpoint, info.token)
}

pub struct RunningController {
    discovery: Discovery,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl RunningController {
    pub fn start(
        port: u16,
        web_root: Option<PathBuf>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let web_root = web_root.map(std::fs::canonicalize).transpose()?;
        let token = crate::random_id().map_err(|error| error.message)?;
        let (server, discovery) = bind(port, token.clone())?;
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let worker = std::thread::Builder::new()
            .name("cove.devtools.controller".into())
            .spawn(move || {
                if let Err(error) = run(&server, &token, web_root, &worker_stop) {
                    eprintln!("Cove DevTools: {error}");
                }
            })?;
        Ok(Self { discovery, stop, worker: Some(worker) })
    }

    pub fn connection(&self) -> &ConnectionInfo {
        &self.discovery.info
    }
}

impl Drop for RunningController {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// The controller a runtime pairs with: one already running, or one started
/// inside this process so launching the app needs no second command.
pub enum AttachedController {
    External(ConnectionInfo),
    Embedded(RunningController),
}

impl AttachedController {
    pub fn connection(&self) -> &ConnectionInfo {
        match self {
            Self::External(info) => info,
            Self::Embedded(controller) => controller.connection(),
        }
    }

    pub fn is_embedded(&self) -> bool {
        matches!(self, Self::Embedded(_))
    }

    /// A live controller keeps ownership of the discovery file. An explicitly
    /// configured endpoint is left alone even while it is unreachable, so a
    /// controller started later still wins over an in-process one.
    pub fn attach(port: u16) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let configured = std::env::var_os("COVE_DEVTOOLS_ENDPOINT").is_some()
            || std::env::var_os("COVE_DEVTOOLS_TOKEN").is_some();
        if let Ok(info) = ConnectionInfo::resolve(None, None)
            && (configured || controller_is_live(&info))
        {
            return Ok(Self::External(info));
        }
        // A stale process can hold the default port; an ephemeral one keeps the
        // Inspector reachable instead of failing the launch.
        let controller =
            RunningController::start(port, None).or_else(|_| RunningController::start(0, None))?;
        Ok(Self::Embedded(controller))
    }
}

struct Discovery {
    path: PathBuf,
    info: ConnectionInfo,
    previous: Option<ConnectionInfo>,
}

impl Drop for Discovery {
    fn drop(&mut self) {
        if !ConnectionInfo::local().is_some_and(|info| info.token == self.info.token) {
            return;
        }
        if let Some(info) = self.previous.as_ref().filter(|info| controller_is_live(info)) {
            let _ = publish(&self.path, info);
        } else {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn controller_is_live(info: &ConnectionInfo) -> bool {
    if !crate::local_http(&info.endpoint) {
        return false;
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(250)))
        .proxy(None)
        .max_redirects(0)
        .build()
        .into();
    agent.get(&format!("{}/health", info.endpoint.trim_end_matches('/')))
        .header("Authorization", &format!("Bearer {}", info.token))
        .call()
        .is_ok_and(|response| response.status().as_u16() == 204)
}

fn bind(
    port: u16,
    token: String,
) -> Result<(Server, Discovery), Box<dyn std::error::Error + Send + Sync>> {
    let server = Server::http(("127.0.0.1", port))?;
    let info = ConnectionInfo { endpoint: format!("http://{}", server.server_addr()), token };
    let path = ConnectionInfo::path().ok_or("Set COVE_DEVTOOLS_SESSION_FILE for local discovery.")?;
    let previous = ConnectionInfo::local();
    publish(&path, &info)?;
    Ok((server, Discovery { path, info, previous }))
}

fn publish(path: &Path, info: &ConnectionInfo) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let folder = path.parent().filter(|path| !path.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(folder)?;
    let mut file = tempfile::NamedTempFile::new_in(folder)?;
    serde_json::to_writer(&mut file, info)?;
    file.flush()?;
    file.persist(path)?;
    Ok(())
}

fn run(
    server: &Server,
    token: &str,
    web_root: Option<PathBuf>,
    stop: &AtomicBool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let web_root = web_root
        .map(std::fs::canonicalize)
        .transpose()?
        .map(Arc::new);
    let controller = Arc::new(Controller::new());
    let authorization = Arc::new(format!("Bearer {token}"));
    let workers = Arc::new(AtomicUsize::new(0));
    while !stop.load(Ordering::Acquire) {
        let Some(request) = server.recv_timeout(Duration::from_millis(100))? else {
            continue;
        };
        if workers.fetch_add(1, Ordering::AcqRel) >= 64 {
            workers.fetch_sub(1, Ordering::AcqRel);
            let _ = request.respond(Response::empty(503));
            continue;
        }
        let controller = controller.clone();
        let web_root = web_root.clone();
        let authorization = authorization.clone();
        let active = workers.clone();
        if std::thread::Builder::new()
            .name("cove.devtools.http".into())
            .spawn(move || {
                let _worker = Worker(active);
                handle(
                    request,
                    &controller,
                    &authorization,
                    web_root.as_deref().map(PathBuf::as_path),
                );
            })
            .is_err()
        {
            workers.fetch_sub(1, Ordering::AcqRel);
        }
    }
    Ok(())
}

struct Worker(Arc<AtomicUsize>);
impl Drop for Worker {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("valid HTTP header")
}

fn read<T: DeserializeOwned>(request: &mut Request) -> Result<T, Fault> {
    let invalid = || {
        Fault::new(
            "invalid_request",
            "The request is malformed or exceeds the size limit.",
        )
    };
    if let Some(length) = request.body_length().filter(|length| *length > MAX_MESSAGE) {
        // A probe result that outgrows the wire disconnects its target unless
        // the cause is visible; report the route and the size.
        eprintln!(
            "cove-devtools: rejecting a {length}-byte request to {}",
            request.url()
        );
        return Err(invalid());
    }
    let mut bytes = Vec::new();
    request
        .as_reader()
        .take(MAX_MESSAGE as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid())?;
    if bytes.len() > MAX_MESSAGE {
        return Err(invalid());
    }
    serde_json::from_slice(&bytes).map_err(|_| invalid())
}

fn json_response(request: Request, status: u16, value: Value) {
    let body = serde_json::to_vec(&value).unwrap_or_default();
    let _ = request.respond(
        Response::from_data(body)
            .with_status_code(status)
            .with_header(header("Content-Type", "application/json"))
            .with_header(header("Cache-Control", "no-store")),
    );
}

fn handle(
    mut request: Request,
    controller: &Controller,
    authorization: &str,
    web_root: Option<&Path>,
) {
    let path = request
        .url()
        .split('?')
        .next()
        .unwrap_or_default()
        .to_owned();
    let allowed = request.headers().iter().any(|header| {
        header.field.equiv("Authorization") && header.value.as_str() == authorization
    });
    match (request.method(), path.as_str()) {
        (&Method::Get, "/health") if allowed => {
            let _ = request.respond(Response::empty(204));
        }
        (&Method::Post, "/probe/register") if allowed => {
            let result = read::<Registration>(&mut request)
                .and_then(|probe| controller.registry.register(probe));
            match result {
                Ok(lease) => json_response(request, 200, json!(lease)),
                Err(error) => json_response(request, 400, json!(error)),
            }
        }
        (&Method::Post, "/probe/poll") => {
            let result = read::<Poll>(&mut request).and_then(|poll| controller.registry.poll(poll));
            match result {
                Ok(work) => json_response(request, 200, json!(work)),
                Err(error) => json_response(request, 400, json!(error)),
            }
        }
        (&Method::Post, "/probe/close") => {
            let result =
                read::<Lease>(&mut request).and_then(|lease| controller.registry.close(lease));
            match result {
                Ok(()) => json_response(request, 200, Value::Null),
                Err(error) => json_response(request, 400, json!(error)),
            }
        }
        (&Method::Post, "/rpc") if allowed => {
            let command = match read::<Command>(&mut request) {
                Ok(command)
                    if command.jsonrpc == "2.0"
                        && (command.id.is_number() || command.id.is_string()) =>
                {
                    command
                }
                _ => {
                    json_response(
                        request,
                        400,
                        json!({"jsonrpc":"2.0","id":null,
                        "error":{"code":-32600,"message":"Invalid JSON-RPC request."}}),
                    );
                    return;
                }
            };
            let result = match controller.execute(&command.method, command.params) {
                Ok(result) => json!({"jsonrpc":"2.0", "id":command.id, "result":result}),
                Err(error) => json!({"jsonrpc":"2.0", "id":command.id,
                    "error":{"code":-32000,"message":error.message,"data":{"code":error.code}}}),
            };
            json_response(request, 200, result);
        }
        (&Method::Get, path) if allowed && path.starts_with("/captures/") => {
            if let Some(image) = controller.image(&path[10..]) {
                let length = image.len();
                let _ = request.respond(Response::new(
                    200.into(),
                    vec![
                        header("Content-Type", "image/png"),
                        header("Cache-Control", "no-store"),
                    ],
                    Cursor::new(SharedImage(image)),
                    Some(length),
                    None,
                ));
            } else {
                let _ = request.respond(Response::empty(404));
            }
        }
        (_, "/rpc" | "/probe/register") => {
            json_response(
                request,
                401,
                json!({"code":"unauthorized","message":"Set the controller token."}),
            );
        }
        (&Method::Get, path) if path.starts_with("/app/") => {
            let path = path.strip_prefix("/app/").unwrap();
            let path = if path.is_empty() { "index.html" } else { path };
            let resolved = web_root.and_then(|root| {
                let path = Path::new(path);
                if path
                    .components()
                    .any(|part| !matches!(part, Component::Normal(_)))
                {
                    return None;
                }
                let path = root.join(path).canonicalize().ok()?;
                path.starts_with(root).then_some(path)
            });
            if let Some(path) = resolved
                && let Ok(file) = std::fs::File::open(&path)
            {
                let mime = match path.extension().and_then(|extension| extension.to_str()) {
                    Some("html") => "text/html; charset=utf-8",
                    Some("js") => "text/javascript; charset=utf-8",
                    Some("css") => "text/css; charset=utf-8",
                    Some("wasm") => "application/wasm",
                    Some("json") => "application/json",
                    Some("ttf") => "font/ttf",
                    Some("otf") => "font/otf",
                    _ => "application/octet-stream",
                };
                let _ = request
                    .respond(Response::from_file(file).with_header(header("Content-Type", mime)));
            } else {
                let _ = request.respond(Response::empty(404));
            }
        }
        // Every served client asset comes from the manifest: a bundled route is
        // one concatenation, a module route is one part of the module graph.
        (&Method::Get, path) => {
            let path = if path == "/" { "/index.html" } else { path };
            match client_asset(path) {
                Some((media, body)) => asset(request, media, body),
                None => {
                    let _ = request.respond(Response::empty(404));
                }
            }
        }
        _ => {
            let _ = request.respond(Response::empty(404));
        }
    }
}

struct SharedImage(Arc<Vec<u8>>);
impl AsRef<[u8]> for SharedImage {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

// Generated by build.rs from client/manifest.json: the bundled routes are
// embedded as ordered parts, and the Inspector module set is embedded one file
// per route so the browser can resolve `import`. Adding, splitting or reordering
// a part is a manifest edit, never a Rust edit.
include!(concat!(env!("OUT_DIR"), "/client_bundles.rs"));

/// Resolves a served route to its media type and concatenated body. The parts
/// are joined once and reused, so repeated requests do not rebuild the asset.
fn client_bundle(path: &str) -> Option<(&'static str, &'static str)> {
    static BODIES: OnceLock<Vec<(&'static str, &'static str, String)>> = OnceLock::new();
    BODIES
        .get_or_init(|| {
            CLIENT_BUNDLES
                .iter()
                .map(|(route, media, parts)| {
                    let body: String = parts.iter().map(|(_, body)| *body).collect();
                    (*route, *media, body)
                })
                .collect()
        })
        .iter()
        .find(|(route, _, _)| *route == path)
        .map(|(_, media, body)| (*media, body.as_str()))
}

/// Resolves a served route to its media type and body: concatenated bundles
/// first, then the ES module set behind the module entry route.
fn client_asset(path: &str) -> Option<(&'static str, &'static str)> {
    client_bundle(path).or_else(|| client_module(path))
}

/// Resolves a module route. The parts are served as they are; the entry route
/// is synthesized from the manifest's module list, so import order lives in
/// exactly one place and every part is reachable by the browser.
fn client_module(path: &str) -> Option<(&'static str, &'static str)> {
    let (route, root, media, parts) = CLIENT_MODULES;
    static ENTRY: OnceLock<String> = OnceLock::new();
    if path == route {
        let entry = ENTRY.get_or_init(|| {
            let mut body = String::new();
            for (part, _) in parts {
                body.push_str("import \"/");
                body.push_str(root);
                body.push('/');
                body.push_str(part);
                body.push_str("\";\n");
            }
            body
        });
        return Some((media, entry.as_str()));
    }
    let relative = path.strip_prefix('/')?.strip_prefix(root)?.strip_prefix('/')?;
    parts
        .iter()
        .find(|(part, _)| *part == relative)
        .map(|(_, body)| (media, *body))
}

fn asset(request: Request, mime: &str, body: impl Into<String>) {
    let _ = request.respond(
        Response::from_string(body.into())
            .with_header(header("Content-Type", mime))
            .with_header(header("Cache-Control", "no-store"))
            .with_header(header("Referrer-Policy", "no-referrer")),
    );
}

#[cfg(test)]
mod tests {
    use super::{CLIENT_BUNDLES, CLIENT_MODULES, client_bundle, client_module};

    #[test]
    fn every_bundle_is_ordered_unique_and_nonempty() {
        assert!(
            !CLIENT_BUNDLES.is_empty(),
            "build.rs must embed the client bundles"
        );
        let mut routes = std::collections::BTreeSet::new();
        for (route, media, parts) in CLIENT_BUNDLES {
            assert!(routes.insert(*route), "{route} is served twice");
            assert!(!media.is_empty(), "{route} has no media type");
            assert!(!parts.is_empty(), "{route} has no parts");
            let mut names = std::collections::BTreeSet::new();
            for (name, body) in *parts {
                assert!(names.insert(*name), "{route} lists {name} twice");
                assert!(!body.trim().is_empty(), "{route} part {name} is empty");
            }
        }
    }

    #[test]
    fn bundles_concatenate_parts_in_manifest_order() {
        for (route, _, parts) in CLIENT_BUNDLES {
            let (_, body) = client_bundle(route).expect("the route resolves");
            let total: usize = parts.iter().map(|(_, body)| body.len()).sum();
            assert_eq!(body.len(), total, "{route} dropped or duplicated bytes");
            let first = parts.first().expect("at least one part").1;
            let last = parts.last().expect("at least one part").1;
            assert!(body.starts_with(first), "{route} must start with its first part");
            assert!(body.ends_with(last), "{route} must end with its last part");
        }
        assert!(client_bundle("/missing").is_none());
        assert_eq!(
            client_bundle("/").is_none(),
            true,
            "/ is normalized before lookup, never a bundle route"
        );
    }

    #[test]
    fn module_entry_imports_every_part_in_order() {
        let (route, root, media, parts) = CLIENT_MODULES;
        assert!(!media.is_empty(), "{route} has no media type");
        assert!(!parts.is_empty(), "{route} has no modules");
        let mut names = std::collections::BTreeSet::new();
        for (name, body) in parts {
            assert!(names.insert(*name), "{route} lists {name} twice");
            assert!(!body.trim().is_empty(), "{route} module {name} is empty");
        }
        let (_, entry) = client_module(route).expect("the module entry resolves");
        let expected: String = parts
            .iter()
            .map(|(name, _)| format!("import \"/{root}/{name}\";\n"))
            .collect();
        assert_eq!(entry, expected, "{route} must import every module in order");
    }

    #[test]
    fn every_module_part_is_served_at_its_root_route() {
        let (route, root, _, parts) = CLIENT_MODULES;
        for (name, body) in parts {
            let path = format!("/{root}/{name}");
            let (media, served) = client_module(&path)
                .unwrap_or_else(|| panic!("{path} must resolve; {route} lists {name}"));
            assert!(!media.is_empty());
            assert_eq!(served, *body, "{path} must serve the module bytes");
        }
        assert!(client_module("/src/missing.js").is_none());
        assert!(client_module("/inspector").is_none());
        assert!(client_module("/src/../Cargo.toml").is_none());
    }
}
