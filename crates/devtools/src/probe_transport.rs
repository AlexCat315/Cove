//! Network worker for native runtimes. Only the UI thread executes probe commands.
use crate::runtime_probe::RuntimeProbe;
use cove_devtools_protocol::control::{
    Completion, ConnectionInfo, Lease, Poll, Registration, Work,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::{Duration, Instant};

enum Message {
    Work(Work, SyncSender<Completion>, Instant),
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
                    let lease = agent
                        .post(&format!("{endpoint}/probe/register"))
                        .header("Authorization", &format!("Bearer {}", info.token))
                        .send_json(&registration)
                        .ok()
                        .and_then(|mut response| response.body_mut().read_json::<Lease>().ok());
                    if let Some(lease) = lease {
                        let mut completion = None;
                        while !worker_stopped.load(Ordering::Acquire) {
                            let mut response = match agent
                                .post(&format!("{endpoint}/probe/poll"))
                                .send_json(Poll {
                                    lease: lease.clone(),
                                    completion: completion.take(),
                                }) {
                                Ok(response) => response,
                                Err(_) => break,
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
                                        Ok(value) => completion = Some(value),
                                        Err(_) => break,
                                    }
                                }
                                Ok(None) => {}
                                Err(_) => break,
                            }
                        }
                        let _ = agent
                            .post(&format!("{endpoint}/probe/close"))
                            .send_json(&lease);
                        let _ = sender.try_send(Message::Reset);
                    }
                    std::thread::park_timeout(Duration::from_millis(400));
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Self { requests, stopped })
    }

    pub fn pump(&self, probe: &mut RuntimeProbe) {
        for message in self.requests.try_iter().take(4) {
            match message {
                Message::Work(work, reply, deadline) if Instant::now() < deadline => {
                    let _ = reply.try_send(Completion {
                        sequence: work.sequence,
                        result: probe.execute(work.command),
                    });
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
