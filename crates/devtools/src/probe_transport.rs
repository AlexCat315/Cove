//! Network worker for native runtimes. Only the UI thread executes probe commands.
use crate::runtime_probe::RuntimeProbe;
use cove_devtools_protocol::control::{
    Completion, ConnectionInfo, Fault, Lease, Poll, ProbeCommand, Registration, Work,
};
use serde::Serialize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::{Duration, Instant};

/// Sends one JSON body without ureq's pretty printer.
///
/// `RequestBuilder::send_json` uses `serde_json::to_vec_pretty`, which inflates
/// a deep tree by an order of magnitude and makes an otherwise bounded probe
/// result exceed [`cove_devtools_protocol::control::MAX_MESSAGE`] on the wire.
fn post_json<T: Serialize>(
    agent: &ureq::Agent,
    url: &str,
    token: Option<&str>,
    value: &T,
) -> Option<ureq::http::Response<ureq::Body>> {
    let body = serde_json::to_vec(value).ok()?;
    let mut request = agent.post(url).header("Content-Type", "application/json");
    if let Some(token) = token {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }
    request.send(body).ok()
}

pub type DeferredCapture = Box<dyn FnOnce() -> Result<serde_json::Value, Fault> + Send>;
pub type PrepareCapture = dyn Fn() -> Result<DeferredCapture, Fault>;

enum Reply {
    Ready(Completion),
    Capture {
        sequence: u64,
        finish: DeferredCapture,
    },
}

enum Message {
    Work(Work, SyncSender<Reply>, Instant),
    Reset,
}

pub struct ProbeTransport {
    requests: Receiver<Message>,
    stopped: Arc<AtomicBool>,
}

impl ProbeTransport {
    pub fn start(
        registration: Registration,
        connection: Option<(String, String)>,
    ) -> Result<Self, String> {
        let stopped = Arc::new(AtomicBool::new(false));
        let (sender, requests) = mpsc::sync_channel(4);
        let worker_stopped = stopped.clone();
        std::thread::Builder::new()
            .name("cove.devtools.probe".into())
            .spawn(move || {
                let agent: ureq::Agent = ureq::Agent::config_builder()
                    .timeout_global(Some(Duration::from_secs(4)))
                    .max_redirects(0)
                    .proxy(None)
                    .build()
                    .into();
                while !worker_stopped.load(Ordering::Acquire) {
                    let info = match &connection {
                        Some((endpoint, token)) => Ok(ConnectionInfo {
                            endpoint: endpoint.clone(),
                            token: token.clone(),
                        }),
                        None => ConnectionInfo::resolve(None, None),
                    };
                    let Ok(info) = info else {
                        std::thread::park_timeout(Duration::from_millis(400));
                        continue;
                    };
                    let local = info
                        .endpoint
                        .parse::<ureq::http::Uri>()
                        .ok()
                        .is_some_and(|uri| {
                            uri.scheme_str() == Some("http")
                                && matches!(uri.host(), Some("127.0.0.1" | "localhost" | "[::1]"))
                                && uri.query().is_none()
                        });
                    if !local {
                        std::thread::park_timeout(Duration::from_millis(400));
                        continue;
                    }
                    let endpoint = info.endpoint.trim_end_matches('/');
                    let lease = post_json(
                        &agent,
                        &format!("{endpoint}/probe/register"),
                        Some(&info.token),
                        &registration,
                    )
                    .and_then(|mut response| response.body_mut().read_json::<Lease>().ok());
                    if let Some(lease) = lease {
                        let mut completion = None;
                        while !worker_stopped.load(Ordering::Acquire) {
                            let poll = Poll {
                                lease: lease.clone(),
                                completion: completion.take(),
                            };
                            let mut response = match post_json(
                                &agent,
                                &format!("{endpoint}/probe/poll"),
                                None,
                                &poll,
                            ) {
                                Some(response) => response,
                                None => break,
                            };
                            match response.body_mut().read_json::<Option<Work>>() {
                                Ok(Some(work)) => {
                                    let (reply, result) = mpsc::sync_channel(1);
                                    let timeout = Duration::from_secs(4);
                                    if sender
                                        .try_send(Message::Work(
                                            work,
                                            reply,
                                            Instant::now() + timeout,
                                        ))
                                        .is_err()
                                    {
                                        break;
                                    }
                                    match result.recv_timeout(timeout) {
                                        Ok(Reply::Ready(value)) => completion = Some(value),
                                        Ok(Reply::Capture { sequence, finish }) => {
                                            completion = Some(Completion {
                                                sequence,
                                                result: finish(),
                                            });
                                        }
                                        Err(_) => break,
                                    }
                                }
                                Ok(None) => {}
                                Err(_) => break,
                            }
                        }
                        let _ = post_json(
                            &agent,
                            &format!("{endpoint}/probe/close"),
                            Some(&info.token),
                            &lease,
                        );
                        let _ = sender.try_send(Message::Reset);
                    }
                    std::thread::park_timeout(Duration::from_millis(400));
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Self { requests, stopped })
    }

    pub fn pump(&self, probe: &mut RuntimeProbe, capture: Option<&PrepareCapture>) {
        for message in self.requests.try_iter().take(4) {
            match message {
                Message::Work(work, reply, deadline) if Instant::now() < deadline => {
                    let sequence = work.sequence;
                    let response = if matches!(work.command, ProbeCommand::Capture)
                        && let Some(prepare) = capture
                    {
                        match prepare() {
                            Ok(finish) => Reply::Capture { sequence, finish },
                            Err(error) => Reply::Ready(Completion {
                                sequence,
                                result: Err(error),
                            }),
                        }
                    } else {
                        Reply::Ready(Completion {
                            sequence,
                            result: probe.execute(work.command),
                        })
                    };
                    let _ = reply.try_send(response);
                }
                Message::Work(..) => {}
                Message::Reset => probe.disconnect(),
            }
        }
    }
}

impl Drop for ProbeTransport {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Serves one request and returns the body the client actually wrote.
    fn capture_request(listener: TcpListener) -> std::thread::JoinHandle<Vec<u8>> {
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("one connection");
            let mut request = Vec::new();
            let mut buffer = [0u8; 8192];
            let (body_start, length) = loop {
                let count = stream.read(&mut buffer).expect("read request");
                assert!(count > 0, "client closed before sending a body");
                request.extend_from_slice(&buffer[..count]);
                let Some(end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
                    continue;
                };
                let headers = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
                let length = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length:"))
                    .and_then(|value| value.trim().parse::<usize>().ok())
                    .expect("the body declares its length");
                if request.len() >= end + 4 + length {
                    break (end + 4, length);
                }
            };
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
                .expect("respond");
            request[body_start..body_start + length].to_vec()
        })
    }

    #[test]
    fn probe_bodies_are_compact_on_the_wire() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let address = listener.local_addr().expect("address");
        let server = capture_request(listener);
        let value = serde_json::json!({
            "result": (0..200)
                .map(|index| serde_json::json!({
                    "id": index,
                    "children": [{"id": index + 1, "children": []}],
                }))
                .collect::<Vec<_>>(),
        });
        let compact = serde_json::to_vec(&value).expect("compact");
        let pretty = serde_json::to_vec_pretty(&value).expect("pretty");
        assert!(
            compact.len() < pretty.len(),
            "pretty printing inflates JSON"
        );

        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(4)))
            .build()
            .into();
        assert!(
            post_json(
                &agent,
                &format!("http://{address}/probe/poll"),
                None,
                &value
            )
            .is_some()
        );
        assert_eq!(
            server.join().expect("server"),
            compact,
            "the transport must not use ureq's pretty send_json"
        );
    }
}
