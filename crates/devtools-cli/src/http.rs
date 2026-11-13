use crate::controller::Controller;
use cove_devtools_protocol::control::{
    ConnectionInfo, Fault, Lease, MAX_MESSAGE, Poll, Registration,
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
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
    drivers: Vec<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let controller = Arc::new(Controller::new(drivers)?);
    let permissions = controller.permissions();
    if permissions["screen_capture"] == false {
        eprintln!(
            "Screen Recording is off. Enable it for the terminal running this controller, then restart."
        );
    }
    if permissions["accessibility"] == false {
        eprintln!(
            "Accessibility is off. Enable it for the terminal running this controller to inspect native controls."
        );
    }
    let web_root = web_root
        .map(std::fs::canonicalize)
        .transpose()?
        .map(Arc::new);
    let server = Server::http(("127.0.0.1", port))?;
    let address = server.server_addr();
    let session_file =
        ConnectionInfo::path().ok_or("Set COVE_DEVTOOLS_SESSION_FILE for local discovery.")?;
    let folder = session_file
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(folder)?;
    let mut file = tempfile::NamedTempFile::new_in(folder)?;
    serde_json::to_writer(
        &mut file,
        &ConnectionInfo {
            endpoint: format!("http://{address}"),
            token: token.clone(),
        },
    )?;
    file.flush()?;
    file.persist(session_file)?;
    eprintln!("Inspector: http://{address}/#{token}");
    let authorization = Arc::new(format!("Bearer {token}"));
    let workers = Arc::new(AtomicUsize::new(0));
    for request in server.incoming_requests() {
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
        (&Method::Get, "/") => asset(
            request,
            "text/html; charset=utf-8",
            include_str!("../../devtools/client/index.html"),
        ),
        (&Method::Get, "/inspector.js") => asset(
            request,
            "text/javascript; charset=utf-8",
            include_str!("../../devtools/client/inspector.js"),
        ),
        (&Method::Get, "/inspector.css") => asset(
            request,
            "text/css; charset=utf-8",
            include_str!("../../devtools/client/inspector.css"),
        ),
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
        (_, "/rpc" | "/probe/register") => {
            json_response(
                request,
                401,
                json!({"code":"unauthorized","message":"Set the controller token."}),
            );
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

fn asset(request: Request, mime: &str, body: &'static str) {
    let _ = request.respond(
        Response::from_string(body)
            .with_header(header("Content-Type", mime))
            .with_header(header("Cache-Control", "no-store"))
            .with_header(header("Referrer-Policy", "no-referrer")),
    );
}
