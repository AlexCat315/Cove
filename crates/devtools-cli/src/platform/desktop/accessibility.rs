use cove_devtools_protocol::control::Fault;
use serde::Serialize;
use serde_json::{Value, json};
use std::hash::{DefaultHasher, Hash, Hasher};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
use linux as backend;
#[cfg(target_os = "macos")]
use macos as backend;
#[cfg(target_os = "windows")]
use windows as backend;

const LIMIT: usize = 256;
const MAX_DEPTH: usize = 20;

pub enum Request<'a> {
    List,
    Find { using: &'a str, value: &'a str },
    Activate(&'a str),
}

pub fn execute(pid: u32, window: u32, request: Request<'_>) -> Result<Value, Fault> {
    backend::execute(pid, window, request)
}

#[derive(Serialize)]
struct Entry {
    id: String,
    parent: Option<String>,
    role: String,
    name: String,
    automation_id: String,
}

impl Entry {
    fn new(
        path: &[usize],
        parent: Option<&str>,
        role: String,
        name: String,
        automation_id: String,
    ) -> Self {
        let mut hash = DefaultHasher::new();
        (path, &role, &name, &automation_id).hash(&mut hash);
        Self {
            id: format!("native-{:016x}", hash.finish()),
            parent: parent.map(str::to_owned),
            role: role.chars().take(64).collect(),
            name: name.chars().take(160).collect(),
            automation_id: automation_id.chars().take(160).collect(),
        }
    }
}

fn respond<H>(
    request: Request<'_>,
    nodes: Vec<(Entry, H)>,
    truncated: bool,
    invoke: impl FnOnce(&H) -> Result<(), Fault>,
) -> Result<Value, Fault> {
    match request {
        Request::List => Ok(json!({
            "backend": "system_accessibility",
            "elements": nodes.iter().map(|(node, _)| node).collect::<Vec<_>>(),
            "truncated": truncated,
        })),
        Request::Find { using, value } => {
            if !matches!(using, "name" | "role" | "id") {
                return Err(Fault::new(
                    "invalid_selector",
                    "Use name, role or id for a desktop accessibility selector.",
                ));
            }
            if truncated {
                return Err(Fault::new(
                    "result_too_large",
                    "The platform tree exceeds the discovery limit.",
                ));
            }
            let mut matches = nodes.iter().filter(|(node, _)| match using {
                "name" => node.name == value,
                "role" => node.role == value,
                _ => node.automation_id == value,
            });
            let Some((element, _)) = matches.next() else {
                return Err(Fault::new(
                    "element_not_found",
                    "The native selector matched no elements.",
                ));
            };
            if matches.next().is_some() {
                return Err(Fault::new(
                    "ambiguous_target",
                    "The native selector matched multiple elements.",
                ));
            }
            Ok(json!({"element":element.id,"name":element.name,"role":element.role}))
        }
        Request::Activate(id) => {
            let (_, handle) = nodes
                .iter()
                .find(|(node, _)| node.id == id)
                .ok_or_else(|| {
                    Fault::new(
                        "stale_element",
                        "The native element is no longer available.",
                    )
                })?;
            invoke(handle)?;
            Ok(json!({"delivered":true}))
        }
    }
}
