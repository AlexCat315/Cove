use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read};

use serde_json::{Value, json};

use super::{AgentError, AgentReply, MAX_RESPONSE_BYTES, decode_reply};

const MAX_WIRE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Default)]
struct Call {
    id: String,
    name: String,
    arguments: String,
}

pub(super) fn cancelled() -> AgentError {
    AgentError::Model("The model response was cancelled.".to_owned())
}

pub(super) fn read(
    body: impl Read,
    mut on_text: impl FnMut(&str) -> bool,
) -> Result<AgentReply, AgentError> {
    let mut body = BufReader::new(body.take(MAX_WIRE_BYTES + 1));
    let mut line = String::new();
    let mut event = String::new();
    let mut read = 0;
    let mut text = String::new();
    let mut reasoning = String::new();
    let mut calls = BTreeMap::<usize, Call>::new();
    let mut finish = None;
    let mut done = false;
    let mut payload_bytes = 0;
    loop {
        line.clear();
        let bytes = body.read_line(&mut line).map_err(|error| {
            AgentError::Model(format!("Could not read the model response: {error}"))
        })?;
        read += bytes as u64;
        if read > MAX_WIRE_BYTES {
            return Err(AgentError::InvalidResult);
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if let Some(data) = line.strip_prefix("data:") {
            if !event.is_empty() {
                event.push('\n');
            }
            event.push_str(data.strip_prefix(' ').unwrap_or(data));
            if event.len() > MAX_RESPONSE_BYTES {
                return Err(AgentError::InvalidResult);
            }
            continue;
        }
        if !line.is_empty() || event.is_empty() {
            if bytes == 0 {
                break;
            }
            continue;
        }
        if event.trim() == "[DONE]" {
            done = true;
            break;
        }
        let value: Value = serde_json::from_str(&event).map_err(|_| AgentError::InvalidResult)?;
        event.clear();
        if value.get("error").is_some() {
            return Err(AgentError::Model(
                "The model server interrupted its response.".to_owned(),
            ));
        }
        if let Some(choice) = value["choices"].as_array().and_then(|choices| {
            choices
                .iter()
                .find(|choice| choice["index"].as_u64().unwrap_or(0) == 0)
        }) {
            if let Some(reason) = choice["finish_reason"].as_str() {
                finish = Some(reason.to_owned());
            }
            let delta = &choice["delta"];
            let output = delta["content"].as_str().unwrap_or_default();
            let thoughts = delta["reasoning_content"].as_str().unwrap_or_default();
            payload_bytes += output.len() + thoughts.len();
            if let Some(incoming) = delta["tool_calls"].as_array() {
                for item in incoming {
                    let index = item["index"]
                        .as_u64()
                        .filter(|index| *index < 256)
                        .ok_or(AgentError::InvalidResult)? as usize;
                    let call = calls.entry(index).or_default();
                    let id = item["id"].as_str().unwrap_or_default();
                    let name = item["function"]["name"].as_str().unwrap_or_default();
                    let arguments = item["function"]["arguments"].as_str().unwrap_or_default();
                    payload_bytes += id.len() + name.len() + arguments.len();
                    call.id.push_str(id);
                    call.name.push_str(name);
                    call.arguments.push_str(arguments);
                }
            }
            if payload_bytes > MAX_RESPONSE_BYTES {
                return Err(AgentError::InvalidResult);
            }
            if !output.is_empty() && !on_text(output) {
                return Err(cancelled());
            }
            text.push_str(output);
            reasoning.push_str(thoughts);
        }
        if bytes == 0 {
            break;
        }
    }
    if !done && finish.is_none() {
        return Err(AgentError::Model(
            "The model stream ended before completion.".to_owned(),
        ));
    }
    let calls: Vec<_> = calls
        .into_values()
        .map(|call| {
            json!({
                "id": call.id,
                "function": {"name": call.name, "arguments": call.arguments}
            })
        })
        .collect();
    decode_reply(&json!({"choices": [{
        "finish_reason": finish,
        "message": {
            "content": text,
            "reasoning_content": (!reasoning.is_empty()).then_some(reasoning),
            "tool_calls": calls
        }
    }]}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AgentToolCall;

    #[test]
    fn reconstructs_tool_arguments_and_keeps_reasoning_off_the_visible_stream() {
        let wire = concat!(
            ": keepalive\r\n\r\n",
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"private\"}}]}\r\n\r\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"看\",\"tool_calls\":[{\"index\":0,\"id\":\"c0\",\"function\":{\"name\":\"read\",\"arguments\":\"{\\\"x\\\":\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"稿\",\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"1}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: [DONE]\n\n"
        );
        let mut visible = Vec::new();
        let reply = read(wire.as_bytes(), |part| {
            visible.push(part.to_owned());
            true
        })
        .unwrap();
        assert_eq!(visible, ["看", "稿"]);
        let AgentReply::Assistant(reply) = reply else {
            panic!("missing assistant message");
        };
        assert_eq!(reply.reasoning_content.as_deref(), Some("private"));
        assert_eq!(
            reply.calls,
            [AgentToolCall {
                id: "c0".into(),
                name: "read".into(),
                arguments: json!({"x":1})
            }]
        );
    }

    #[test]
    fn rejects_unfinished_output_and_stops_on_cancel() {
        let wire = "data: {\"choices\":[{\"delta\":{\"content\":\"draft\"}}]}\n\n";
        assert!(
            matches!(read(wire.as_bytes(), |_| true), Err(AgentError::Model(message)) if message.contains("before completion"))
        );
        assert!(
            matches!(read(wire.as_bytes(), |_| false), Err(AgentError::Model(message)) if message.contains("cancelled"))
        );
    }
}
