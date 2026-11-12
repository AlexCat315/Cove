//! Runtime registration, bounded command delivery and target lifetime.
use cove_devtools_protocol::control::{
    Completion, Fault, Lease, Poll, ProbeCommand, Registration, VERSION, Work,
};
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

const LEASE_TIMEOUT: Duration = Duration::from_secs(15);
const CALL_TIMEOUT: Duration = Duration::from_secs(6);

#[derive(Default)]
pub struct Registry {
    state: Mutex<State>,
    changed: Condvar,
}

#[derive(Default)]
struct State {
    targets: HashMap<String, Target>,
    sequence: u64,
}

struct Target {
    registration: Registration,
    key: String,
    last_seen: Instant,
    pending: VecDeque<Work>,
    in_flight: Option<u64>,
    calls: HashMap<u64, Option<Result<Value, Fault>>>,
}

pub fn random_id() -> Result<String, Fault> {
    let mut bytes = [0u8; 24];
    getrandom::fill(&mut bytes).map_err(|error| Fault::new("internal", error.to_string()))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn stale() -> Fault {
    Fault::new(
        "target_gone",
        "The runtime target is disconnected. Select a current target.",
    )
}

impl Registry {
    pub fn register(&self, registration: Registration) -> Result<Lease, Fault> {
        if registration.version != VERSION {
            return Err(Fault::new(
                "protocol_version",
                "Controller and runtime must use the same protocol version.",
            ));
        }
        if registration.title.len() > 512
            || registration.platform.len() > 32
            || registration.capabilities.len() > 32
            || registration.capabilities.iter().any(|name| name.len() > 32)
        {
            return Err(Fault::new(
                "invalid_request",
                "Invalid target registration.",
            ));
        }
        let mut state = self.state.lock().unwrap();
        Self::prune(&mut state);
        if state.targets.len() >= 32 {
            return Err(Fault::new(
                "busy",
                "The controller has too many registered targets.",
            ));
        }
        let lease = Lease {
            id: random_id()?,
            key: random_id()?,
        };
        state.targets.insert(
            lease.id.clone(),
            Target {
                registration,
                key: lease.key.clone(),
                last_seen: Instant::now(),
                pending: VecDeque::new(),
                in_flight: None,
                calls: HashMap::new(),
            },
        );
        Ok(lease)
    }

    fn prune(state: &mut State) {
        state
            .targets
            .retain(|_, target| target.last_seen.elapsed() < LEASE_TIMEOUT);
    }

    pub fn registrations(&self) -> Vec<(String, Registration)> {
        let mut state = self.state.lock().unwrap();
        Self::prune(&mut state);
        let mut result: Vec<_> = state
            .targets
            .iter()
            .map(|(id, target)| (id.clone(), target.registration.clone()))
            .collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }

    pub fn list(&self) -> Vec<Value> {
        self.registrations()
            .into_iter()
            .map(|(id, target)| {
                json!({
                    "id": id, "title": target.title, "platform": target.platform, "pid": target.pid,
                    "kind": "runtime", "capabilities": target.capabilities,
                })
            })
            .collect()
    }

    pub fn poll(&self, poll: Poll) -> Result<Option<Work>, Fault> {
        let until = Instant::now() + Duration::from_millis(400);
        let mut state = self.state.lock().unwrap();
        let target = state
            .targets
            .get_mut(&poll.lease.id)
            .filter(|target| target.key == poll.lease.key)
            .ok_or_else(stale)?;
        target.last_seen = Instant::now();
        if let Some(Completion { sequence, result }) = poll.completion {
            if target.in_flight != Some(sequence) {
                return Err(Fault::new(
                    "invalid_sequence",
                    "Completion does not match the delivered command.",
                ));
            }
            target.in_flight = None;
            if let Some(reply) = target.calls.get_mut(&sequence) {
                *reply = Some(result);
            }
            self.changed.notify_all();
        }
        loop {
            let target = state.targets.get_mut(&poll.lease.id).ok_or_else(stale)?;
            if target.in_flight.is_none()
                && let Some(work) = target.pending.pop_front()
            {
                target.in_flight = Some(work.sequence);
                return Ok(Some(work));
            }
            let remaining = until.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(None);
            }
            state = self.changed.wait_timeout(state, remaining).unwrap().0;
        }
    }

    pub fn close(&self, lease: Lease) -> Result<(), Fault> {
        let mut state = self.state.lock().unwrap();
        if !state
            .targets
            .get(&lease.id)
            .is_some_and(|target| target.key == lease.key)
        {
            return Err(stale());
        }
        state.targets.remove(&lease.id);
        self.changed.notify_all();
        Ok(())
    }

    pub fn invoke(&self, target_id: &str, command: ProbeCommand) -> Result<Value, Fault> {
        let until = Instant::now() + CALL_TIMEOUT;
        let mut state = self.state.lock().unwrap();
        Self::prune(&mut state);
        state.sequence += 1;
        let sequence = state.sequence;
        let target = state.targets.get_mut(target_id).ok_or_else(stale)?;
        if target.calls.len() >= 8 {
            return Err(Fault::new("busy", "The runtime command queue is full."));
        }
        target.calls.insert(sequence, None);
        target.pending.push_back(Work { sequence, command });
        self.changed.notify_all();
        loop {
            let target = state.targets.get_mut(target_id).ok_or_else(stale)?;
            if let Some(Some(result)) = target.calls.get_mut(&sequence).map(Option::take) {
                target.calls.remove(&sequence);
                return result;
            }
            let remaining = until.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                let delivered = target.in_flight == Some(sequence);
                target.pending.retain(|work| work.sequence != sequence);
                target.calls.remove(&sequence);
                return Err(if delivered {
                    Fault::new(
                        "outcome_unknown",
                        "The runtime received the command, but no result arrived.",
                    )
                } else {
                    Fault::new("timeout", "The runtime did not receive the command.")
                });
            }
            state = self.changed.wait_timeout(state, remaining).unwrap().0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn register(registry: &Registry, title: &str) -> Lease {
        registry
            .register(Registration {
                version: VERSION,
                title: title.into(),
                platform: "test".into(),
                pid: None,
                window: None,
                capabilities: vec!["tree".into()],
            })
            .unwrap()
    }

    #[test]
    fn dispatch_is_scoped_to_the_registered_target() {
        let registry = Arc::new(Registry::default());
        let one = register(&registry, "one");
        let two = register(&registry, "two");
        let caller = {
            let registry = registry.clone();
            let target = two.id.clone();
            std::thread::spawn(move || registry.invoke(&target, ProbeCommand::Tree))
        };
        assert!(
            registry
                .poll(Poll {
                    lease: one,
                    completion: None
                })
                .unwrap()
                .is_none()
        );
        let work = registry
            .poll(Poll {
                lease: two.clone(),
                completion: None,
            })
            .unwrap()
            .unwrap();
        assert!(matches!(work.command, ProbeCommand::Tree));
        let fault = registry
            .poll(Poll {
                lease: two.clone(),
                completion: Some(Completion {
                    sequence: work.sequence + 1,
                    result: Ok(json!("incorrect completion")),
                }),
            })
            .unwrap_err();
        assert_eq!(fault.code, "invalid_sequence");
        registry
            .poll(Poll {
                lease: two.clone(),
                completion: Some(Completion {
                    sequence: work.sequence,
                    result: Ok(json!({"root": "two"})),
                }),
            })
            .unwrap();
        assert_eq!(caller.join().unwrap().unwrap()["root"], "two");
        registry.close(two.clone()).unwrap();
        assert_eq!(
            registry
                .invoke(&two.id, ProbeCommand::Tree)
                .unwrap_err()
                .code,
            "target_gone"
        );
    }
}
