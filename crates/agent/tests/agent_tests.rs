//! Translation of CoveUIAgentCoreTests/AgentTests.swift.
//!
//! Behaviour checklist:
//! - a tool's input is schema-validated before its executor runs
//! - an unregistered tool is an unknown-tool error, not a schema error
//! - a result that is not JSON is rejected after execution
//! - the model sees real tool results and the enabled skill instructions
//! - MCP negotiates notifications, calls tools, and serves skills as prompts
//! - malformed JSON-RPC ids are rejected while a valid id is echoed back
//! - the tool-round limit stops a model that never answers
//! - a skill file reached through a symlink outside the root is rejected
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

use cove_agent::{
    AgentError, AgentMessage, AgentReply, AgentRole, AgentSession, AgentSkill, AgentSkillLibrary,
    AgentTool, AgentToolCall, AgentToolResult, AgentToolbox, JsonSchema, McpServer,
};

fn echo_tool() -> AgentTool {
    AgentTool::new(
        "echo",
        "Return supplied text",
        JsonSchema::object(
            BTreeMap::from([("text".to_string(), JsonSchema::string(None, None))]),
            vec!["text".to_string()],
        ),
        true,
    )
}

fn toolbox() -> AgentToolbox {
    AgentToolbox::new(
        vec![echo_tool()],
        std::sync::Arc::new(|_, input| Ok(AgentToolResult::encode(input, false))),
    )
    .expect("toolbox")
}

#[test]
fn validates_before_executing_and_bounds_results() {
    let toolbox = toolbox();
    // Wrong type for a required string is caught by the schema before the
    // executor is reached. Swift throws `JSONSchemaViolation`; Rust folds it
    // into `InvalidConfiguration`, so the violation text carries the path.
    match toolbox.execute("echo", &serde_json::json!({ "text": 12 })) {
        Err(AgentError::InvalidConfiguration(message)) => {
            assert!(
                message.contains("text"),
                "violation should name the field: {message}"
            );
        }
        other => panic!("expected a schema violation, got {other:?}"),
    }
    assert!(matches!(
        toolbox.execute("unregistered", &serde_json::json!({})),
        Err(AgentError::UnknownTool(_))
    ));
    // An executor that emits non-JSON is rejected after it runs.
    let invalid = AgentToolbox::new(
        vec![echo_tool()],
        std::sync::Arc::new(|_, _| Ok(AgentToolResult::new(b"invalid".to_vec(), false))),
    )
    .expect("toolbox");
    assert!(matches!(
        invalid.execute("echo", &serde_json::json!({ "text": "ok" })),
        Err(AgentError::InvalidResult)
    ));
}

#[test]
fn model_receives_real_tool_results_and_explicit_skill_context() {
    let library = AgentSkillLibrary::new(vec![
        AgentSkill::new(
            "inspect",
            "Inspect app",
            "Read state before forming a diagnosis.",
        )
        .expect("skill"),
    ])
    .expect("library");
    let saw_tool_names = std::sync::Arc::new(AtomicBool::new(false));
    let saw_instructions = std::sync::Arc::new(AtomicBool::new(false));
    let names_flag = saw_tool_names.clone();
    let instructions_flag = saw_instructions.clone();
    let session = AgentSession::new(
        toolbox(),
        &library,
        &["inspect".to_string()],
        8,
        std::sync::Arc::new(move |messages: &[AgentMessage], tools: &[AgentTool]| {
            names_flag.store(
                tools.iter().map(|tool| tool.name.as_str()).eq(["echo"]),
                Ordering::SeqCst,
            );
            instructions_flag.store(
                messages
                    .first()
                    .is_some_and(|message| message.text.contains("Read state")),
                Ordering::SeqCst,
            );
            if let Some(result) = messages.last()
                && result.role == AgentRole::Tool
            {
                assert_eq!(result.result_for.as_deref(), Some("call1"));
                assert!(!result.is_error);
                assert_eq!(result.text, r#"{"text":"measured"}"#);
                return Ok(AgentReply::Answer("Verified".into()));
            }
            Ok(AgentReply::Tools(vec![AgentToolCall {
                id: "call1".into(),
                name: "echo".into(),
                arguments: serde_json::json!({ "text": "measured" }),
            }]))
        }),
    )
    .expect("session");
    let run = session
        .run(vec![AgentMessage::user("Inspect")])
        .expect("run");
    assert_eq!(run.answer, "Verified");
    assert_eq!(run.tool_rounds, 1);
    assert!(
        saw_tool_names.load(Ordering::SeqCst),
        "the model must see the registered tools"
    );
    assert!(
        saw_instructions.load(Ordering::SeqCst),
        "enabled skill instructions lead the messages"
    );
}

fn json_body(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).expect("response is JSON")
}

#[test]
fn mcp_negotiates_lists_calls_and_exposes_skills_as_prompts() {
    let library = AgentSkillLibrary::new(vec![
        AgentSkill::new("inspect", "Inspect", "Observe first").expect("skill"),
    ])
    .expect("library");
    let server = McpServer::new("fixture", "1", toolbox()).with_skills(library);

    let notification = server.respond(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
    assert!(notification.is_none(), "a notification is never answered");

    let request = br#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"echo","arguments":{"text":"ok"}}}"#;
    let response = server.respond(request).expect("response");
    let json = json_body(&response);
    let result = &json["result"];
    assert_eq!(result["structuredContent"]["text"], "ok");
    assert_eq!(result["isError"], false);

    let skill = server
        .respond(br#"{"jsonrpc":"2.0","id":2,"method":"prompts/get","params":{"name":"inspect"}}"#)
        .expect("response");
    assert!(String::from_utf8_lossy(&skill).contains("Observe first"));

    // A boolean is not a JSON-RPC id.
    let bad_id = server
        .respond(br#"{"jsonrpc":"2.0","id":true,"method":"ping"}"#)
        .expect("response");
    assert!(String::from_utf8_lossy(&bad_id).contains("-32600"));

    // A request without `jsonrpc` is invalid but still echoes its id.
    let invalid = server
        .respond(br#"{"id":42,"method":"ping"}"#)
        .expect("response");
    assert_eq!(json_body(&invalid)["id"], 42);
}

#[test]
fn tool_round_limit_is_enforced() {
    let looping = AgentSession::new(
        toolbox(),
        &AgentSkillLibrary::default(),
        &[],
        1,
        std::sync::Arc::new(|_: &[AgentMessage], _: &[AgentTool]| {
            Ok(AgentReply::Tools(vec![AgentToolCall {
                id: "loop".into(),
                name: "echo".into(),
                arguments: serde_json::json!({ "text": "ok" }),
            }]))
        }),
    )
    .expect("session");
    assert!(matches!(
        looping.run(Vec::new()),
        Err(AgentError::StepLimit)
    ));
}

#[cfg(unix)]
#[test]
fn skill_directory_rejects_escaping_symlinks() {
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!("cove-agent-skills-{}", std::process::id()));
    let outside =
        std::env::temp_dir().join(format!("cove-agent-skill-outside-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_file(&outside);
    std::fs::create_dir_all(root.join("inspect")).expect("skill dir");
    std::fs::write(
        &outside,
        "---\nname: inspect\ndescription: Inspect\n---\nObserve first",
    )
    .expect("outside file");
    let link = root.join("inspect/SKILL.md");
    symlink(&outside, &link).expect("symlink");

    assert!(
        AgentSkillLibrary::load(&root).is_err(),
        "a SKILL.md resolving outside the root must be rejected"
    );

    std::fs::remove_file(&link).expect("unlink");
    std::fs::copy(&outside, &link).expect("copy");
    let library = AgentSkillLibrary::load(&root).expect("library loads");
    assert_eq!(
        library.read("inspect").expect("skill").instructions,
        "Observe first"
    );

    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_file(&outside);
}
