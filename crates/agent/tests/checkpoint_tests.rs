use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};

use cove_agent::checkpoint::{
    AgentCheckpoint, AgentRunner, CheckpointError, CheckpointLease, CheckpointStatus,
    CheckpointStore, MemoryCheckpointStore, ToolDecision, ToolPolicy,
};
use cove_agent::{
    AgentError, AgentMessage, AgentModel, AgentReply, AgentRole, AgentSession, AgentSkillLibrary,
    AgentTool, AgentToolCall, AgentToolResult, AgentToolbox, JsonSchema, ToolExecutor,
};
use serde_json::json;

fn job_session(tools: Vec<AgentTool>, execute: ToolExecutor, model: AgentModel) -> AgentSession {
    AgentSession::new(
        AgentToolbox::new(tools, execute).unwrap(),
        &AgentSkillLibrary::default(),
        &[],
        4,
        model,
    )
    .unwrap()
}

fn tool(name: &str, read_only: bool) -> AgentTool {
    AgentTool::new(name, name, JsonSchema::any(), read_only)
}

fn request(id: &str, name: &str) -> AgentToolCall {
    AgentToolCall {
        id: id.to_owned(),
        name: name.to_owned(),
        arguments: json!({"sku":"A-42"}),
    }
}

fn stats_model() -> AgentModel {
    Arc::new(|messages, _| {
        if messages.last().is_some_and(|m| m.role == AgentRole::Tool) {
            return Ok(AgentReply::Answer("Inventory verified.".to_owned()));
        }
        Ok(AgentReply::Tools(vec![request("read", "inventory")]))
    })
}

#[test]
fn a_user_can_continue_a_completed_inventory_turn() {
    let store = MemoryCheckpointStore::default();
    let session = job_session(
        vec![tool("inventory", true)],
        Arc::new(|_, _| Ok(AgentToolResult::encode(&json!({"count":17}), false))),
        stats_model(),
    );
    let runner = AgentRunner::new(&session, &store, "inventory:me").unwrap();
    let created = runner
        .start(
            "audit",
            vec![AgentMessage::user("Check A-42")],
            ToolPolicy::Automatic,
        )
        .unwrap();
    assert_eq!(created.status().unwrap(), CheckpointStatus::Model);
    assert!(matches!(
        runner.advance("audit").unwrap().status().unwrap(),
        CheckpointStatus::ToolReady(_)
    ));
    assert_eq!(
        runner.advance("audit").unwrap().status().unwrap(),
        CheckpointStatus::Model
    );
    let done = runner.advance("audit").unwrap();
    assert_eq!(done.completed_run().unwrap().unwrap().tool_rounds, 1);
    assert_eq!(runner.advance("audit").unwrap().revision(), done.revision());
    assert!(matches!(
        runner.continue_with("audit", 0, "Recount"),
        Err(CheckpointError::Stale)
    ));
    let follow_up = runner
        .continue_with("audit", done.revision(), "Recount")
        .unwrap();
    assert_eq!(follow_up.status().unwrap(), CheckpointStatus::Model);
    assert_eq!(follow_up.messages().len(), done.messages().len() + 1);
    assert_eq!(follow_up.messages().last().unwrap().text, "Recount");
    let paused = runner
        .drive("audit", |checkpoint| {
            !matches!(checkpoint.status().unwrap(), CheckpointStatus::ToolReady(_))
        })
        .unwrap();
    assert!(matches!(
        paused.status().unwrap(),
        CheckpointStatus::ToolReady(_)
    ));
    let completed = runner.drive("audit", |_| true).unwrap();
    assert_eq!(
        completed.status().unwrap(),
        CheckpointStatus::Completed("Inventory verified.")
    );

    let outsider = AgentRunner::new(&session, &store, "inventory:other").unwrap();
    assert!(matches!(
        outsider.load("audit"),
        Err(CheckpointError::Invalid(_))
    ));
    assert!(matches!(
        runner.start("audit", vec![], ToolPolicy::Automatic),
        Err(CheckpointError::Exists)
    ));
}

#[test]
fn a_failed_model_request_keeps_the_previous_checkpoint() {
    let attempted = AtomicBool::new(false);
    let session = job_session(
        vec![],
        Arc::new(|name, _| Err(AgentError::UnknownTool(name.to_owned()))),
        Arc::new(move |_, _| {
            if attempted.swap(true, Ordering::Relaxed) {
                Ok(AgentReply::Answer("Received.".to_owned()))
            } else {
                Err(AgentError::HttpStatus(503))
            }
        }),
    );
    let store = MemoryCheckpointStore::default();
    let runner = AgentRunner::new(&session, &store, "inbox:me").unwrap();
    let before = runner
        .start(
            "receipt",
            vec![AgentMessage::user("Check the receipt")],
            ToolPolicy::Automatic,
        )
        .unwrap();
    assert!(matches!(
        runner.advance("receipt"),
        Err(CheckpointError::Agent(AgentError::HttpStatus(503)))
    ));
    assert_eq!(runner.load("receipt").unwrap(), before);
    assert_eq!(
        runner.advance("receipt").unwrap().status().unwrap(),
        CheckpointStatus::Completed("Received.")
    );
}

#[test]
fn two_workers_advance_a_tool_boundary_only_once() {
    let invocations = AtomicUsize::new(0);
    let observed = Arc::new(invocations);
    let execution = observed.clone();
    let store = MemoryCheckpointStore::default();
    let session = job_session(
        vec![tool("inventory", true)],
        Arc::new(move |_, _| {
            execution.fetch_add(1, Ordering::Relaxed);
            Ok(AgentToolResult::encode(&json!({"count":17}), false))
        }),
        stats_model(),
    );
    let runner = AgentRunner::new(&session, &store, "inventory:me").unwrap();
    runner
        .start(
            "audit",
            vec![AgentMessage::user("Check A-42")],
            ToolPolicy::Automatic,
        )
        .unwrap();
    runner.advance("audit").unwrap();
    let start = Barrier::new(3);
    std::thread::scope(|scope| {
        for _ in 0..2 {
            scope.spawn(|| {
                start.wait();
                runner.advance("audit").unwrap();
            });
        }
        start.wait();
    });
    assert_eq!(observed.load(Ordering::Relaxed), 1);
    assert_eq!(
        runner.load("audit").unwrap().status().unwrap(),
        CheckpointStatus::Completed("Inventory verified.")
    );
}

#[derive(Default)]
struct FailSaveStore {
    backend: MemoryCheckpointStore,
    fail_next_save: AtomicBool,
}

struct FailSaveLease<'a> {
    delegate: Box<dyn CheckpointLease + 'a>,
    failed: &'a AtomicBool,
}

impl CheckpointStore for FailSaveStore {
    fn load(&self, id: &str) -> Result<Option<AgentCheckpoint>, CheckpointError> {
        self.backend.load(id)
    }

    fn lease(&self, id: &str) -> Result<Box<dyn CheckpointLease + '_>, CheckpointError> {
        Ok(Box::new(FailSaveLease {
            delegate: self.backend.lease(id)?,
            failed: &self.fail_next_save,
        }))
    }
}

impl CheckpointLease for FailSaveLease<'_> {
    fn load(&self) -> Result<Option<AgentCheckpoint>, CheckpointError> {
        self.delegate.load()
    }

    fn save(&mut self, checkpoint: &AgentCheckpoint) -> Result<(), CheckpointError> {
        if self.failed.swap(false, Ordering::Relaxed) {
            return Err(std::io::Error::other("simulated crash before publishing outcome").into());
        }
        self.delegate.save(checkpoint)
    }
}

#[test]
fn an_interrupt_after_a_write_requires_a_verified_outcome() {
    let store = Arc::new(FailSaveStore::default());
    let outcome_store = store.clone();
    let invocations = Arc::new(AtomicUsize::new(0));
    let execution = invocations.clone();
    let toolbox = AgentToolbox::with_context(
        vec![tool("archive_report", false)],
        Arc::new(move |_, _, context| {
            assert_eq!(context.call_id, Some("archive"));
            let key = context.checkpoint.unwrap();
            assert_eq!(key.scope, "reports:me");
            assert_eq!(key.run_id, "report");
            assert_eq!((key.assistant_message, key.call_index), (1, 0));
            execution.fetch_add(1, Ordering::Relaxed);
            outcome_store.fail_next_save.store(true, Ordering::Relaxed);
            Ok(AgentToolResult::encode(&json!({"archived":true}), false))
        }),
    )
    .unwrap();
    let session = AgentSession::new(
        toolbox,
        &AgentSkillLibrary::default(),
        &[],
        4,
        Arc::new(|messages, _| {
            if let Some(last) = messages.last()
                && last.role == AgentRole::Tool
            {
                assert_eq!(last.result_for.as_deref(), Some("archive"));
                assert_eq!(last.text, r#"{"archived":true}"#);
                return Ok(AgentReply::Answer("Report archived.".to_owned()));
            }
            Ok(AgentReply::Tools(vec![request(
                "archive",
                "archive_report",
            )]))
        }),
    )
    .unwrap();
    let runner = AgentRunner::new(&session, &*store, "reports:me").unwrap();
    runner
        .start(
            "report",
            vec![AgentMessage::user("Archive the report")],
            ToolPolicy::ConfirmWrites,
        )
        .unwrap();
    runner.advance("report").unwrap();
    let approval = runner.advance("report").unwrap();
    assert!(matches!(
        approval.status().unwrap(),
        CheckpointStatus::AwaitingApproval(_)
    ));
    assert_eq!(invocations.load(Ordering::Relaxed), 0);
    assert!(matches!(
        runner.resolve("report", 0, ToolDecision::Approve),
        Err(CheckpointError::Stale)
    ));
    runner
        .resolve("report", approval.revision(), ToolDecision::Approve)
        .unwrap();
    assert!(matches!(
        runner.advance("report"),
        Err(CheckpointError::Storage(_))
    ));

    let ambiguous = runner.drive("report", |_| true).unwrap();
    assert_eq!(ambiguous.tool_key().unwrap(), approval.tool_key().unwrap());
    assert!(matches!(
        ambiguous.status().unwrap(),
        CheckpointStatus::ToolInFlight {
            read_only: false,
            ..
        }
    ));
    assert_eq!(invocations.load(Ordering::Relaxed), 1);
    runner
        .resolve(
            "report",
            ambiguous.revision(),
            ToolDecision::RecordOutcome(AgentToolResult::encode(&json!({"archived":true}), false)),
        )
        .unwrap();
    assert_eq!(
        runner.advance("report").unwrap().status().unwrap(),
        CheckpointStatus::Completed("Report archived.")
    );
}

#[test]
fn an_interrupted_read_can_be_repeated() {
    let store = Arc::new(FailSaveStore::default());
    let outcome_store = store.clone();
    let invocations = Arc::new(AtomicUsize::new(0));
    let execution = invocations.clone();
    let session = job_session(
        vec![tool("inventory", true)],
        Arc::new(move |_, _| {
            if execution.fetch_add(1, Ordering::Relaxed) == 0 {
                outcome_store.fail_next_save.store(true, Ordering::Relaxed);
            }
            Ok(AgentToolResult::encode(&json!({"count":17}), false))
        }),
        stats_model(),
    );
    let runner = AgentRunner::new(&session, &*store, "inventory:me").unwrap();
    runner
        .start("audit", vec![], ToolPolicy::ConfirmWrites)
        .unwrap();
    runner.advance("audit").unwrap();
    assert!(matches!(
        runner.advance("audit"),
        Err(CheckpointError::Storage(_))
    ));
    assert!(matches!(
        runner.load("audit").unwrap().status().unwrap(),
        CheckpointStatus::ToolInFlight {
            read_only: true,
            ..
        }
    ));
    assert_eq!(
        runner.advance("audit").unwrap().status().unwrap(),
        CheckpointStatus::Model
    );
    assert_eq!(invocations.load(Ordering::Relaxed), 2);
    assert!(
        runner
            .advance("audit")
            .unwrap()
            .completed_run()
            .unwrap()
            .is_some()
    );
}

#[cfg(all(feature = "checkpoint-files", any(unix, windows)))]
#[test]
fn a_file_checkpoint_reopens_between_tools_without_repeating_completed_calls() {
    use cove_agent::checkpoint::FileCheckpointStore;

    let directory = tempfile::tempdir().unwrap();
    {
        let session = job_session(
            vec![tool("inventory", true), tool("shipments", true)],
            Arc::new(|name, _| {
                assert_eq!(name, "inventory");
                Ok(AgentToolResult::encode(&json!({"on_hand":17}), false))
            }),
            Arc::new(|_, _| {
                let mut message = AgentMessage::new(AgentRole::Assistant);
                message.reasoning_content = Some("provider continuation".to_owned());
                message.calls = vec![
                    request("on-hand", "inventory"),
                    request("pending", "shipments"),
                ];
                Ok(AgentReply::Assistant(message))
            }),
        );
        let store = FileCheckpointStore::open(directory.path()).unwrap();
        let runner = AgentRunner::new(&session, &store, "inventory:me").unwrap();
        assert!(
            runner
                .start("../invalid", vec![], ToolPolicy::Automatic)
                .is_err()
        );
        runner
            .start(
                "audit",
                vec![AgentMessage::user("Prepare report")],
                ToolPolicy::Automatic,
            )
            .unwrap();
        runner.advance("audit").unwrap();
        runner.advance("audit").unwrap();
    }
    let store = FileCheckpointStore::open(directory.path()).unwrap();
    let resumed = job_session(
        vec![tool("inventory", true), tool("shipments", true)],
        Arc::new(|name, _| {
            assert_eq!(name, "shipments");
            Ok(AgentToolResult::encode(&json!({"pending":3}), false))
        }),
        Arc::new(|messages, _| {
            assert_eq!(
                messages[1].reasoning_content.as_deref(),
                Some("provider continuation")
            );
            assert_eq!(messages[2].result_for.as_deref(), Some("on-hand"));
            assert_eq!(messages[3].result_for.as_deref(), Some("pending"));
            Ok(AgentReply::Answer("17 on hand; 3 pending.".to_owned()))
        }),
    );
    let runner = AgentRunner::new(&resumed, &store, "inventory:me").unwrap();
    let before = runner.load("audit").unwrap();
    assert!(
        matches!(before.status().unwrap(), CheckpointStatus::ToolReady(call) if call.name == "shipments")
    );
    runner.advance("audit").unwrap();
    let done = runner.advance("audit").unwrap();
    assert_eq!(
        done.completed_run().unwrap().unwrap().answer,
        "17 on hand; 3 pending."
    );
}
