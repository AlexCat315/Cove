use std::collections::BTreeMap;
#[cfg(feature = "workflow-files")]
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};

#[cfg(feature = "workflow-files")]
use cove_agent::checkpoint::{CheckpointError, ToolDecision};
use cove_agent::checkpoint::{CheckpointStatus, MemoryCheckpointStore, ToolPolicy};
use cove_agent::workflow::{
    ArtifactFormat, InMemoryWorkflowStore, ReviewDecision, StepAction, StepState, WorkflowError,
    WorkflowPlan, WorkflowRunner, WorkflowStep,
};
#[cfg(feature = "workflow-files")]
use cove_agent::workflow::{WorkflowSnapshot, WorkflowStore};
use cove_agent::{
    AgentError, AgentModel, AgentReply, AgentSession, AgentSkillLibrary, AgentToolbox, JsonSchema,
};
#[cfg(feature = "workflow-files")]
use cove_agent::{AgentRole, AgentTool, AgentToolCall, AgentToolResult};
use serde_json::{Value, json};

fn session(model: AgentModel) -> Arc<AgentSession> {
    Arc::new(
        AgentSession::new(
            AgentToolbox::new(
                vec![],
                Arc::new(|name, _| Err(AgentError::UnknownTool(name.into()))),
            )
            .unwrap(),
            &AgentSkillLibrary::default(),
            &[],
            3,
            model,
        )
        .unwrap(),
    )
}

fn step(id: &str, dependencies: &[&str], action: StepAction, output: JsonSchema) -> WorkflowStep {
    WorkflowStep {
        id: id.into(),
        dependencies: dependencies.iter().map(|id| (*id).into()).collect(),
        uses_input: dependencies.is_empty(),
        action,
        output,
    }
}

fn agent(id: &str, format: ArtifactFormat) -> StepAction {
    StepAction::Agent {
        agent: id.into(),
        instructions: format!("Handle the {id} step."),
        format,
        tool_policy: ToolPolicy::ConfirmWrites,
    }
}

#[test]
fn invalid_graphs_and_invalid_serialized_plans_are_rejected() {
    let review = |id, deps: &[&str]| step(id, deps, StepAction::Review, JsonSchema::any());
    for steps in [
        vec![],
        vec![review("a", &[]), review("a", &[])],
        vec![review("a", &["missing"])],
        vec![review("a", &["a"])],
        vec![review("a", &["b"]), review("b", &["a"])],
        vec![review("a", &["b", "b"]), review("b", &[])],
        vec![review("../a", &[])],
    ] {
        assert!(WorkflowPlan::new(JsonSchema::any(), steps).is_err());
    }

    let plan = WorkflowPlan::new(
        JsonSchema::any(),
        vec![review("a", &["b"]), review("b", &[])],
    )
    .unwrap();
    let mut encoded = serde_json::to_value(&plan).unwrap();
    assert_eq!(
        serde_json::from_value::<WorkflowPlan>(encoded.clone()).unwrap(),
        plan
    );
    encoded["steps"][1]["dependencies"] = json!(["a"]);
    assert!(serde_json::from_value::<WorkflowPlan>(encoded).is_err());
}

#[test]
fn separate_agents_run_concurrently_and_only_approved_data_reaches_the_next_step() {
    let barrier = Arc::new(Barrier::new(2));
    let verify = Arc::clone(&barrier);
    let budget = Arc::clone(&barrier);
    let agents = BTreeMap::from([
        (
            "verify".into(),
            session(Arc::new(move |messages, _| {
                let input: Value = serde_json::from_str(&messages.last().unwrap().text).unwrap();
                assert_eq!(input, json!({"input":"Purchase two desks","artifacts":{}}));
                verify.wait();
                Ok(AgentReply::Answer("2".into()))
            })),
        ),
        (
            "budget".into(),
            session(Arc::new(move |_, _| {
                budget.wait();
                Ok(AgentReply::Answer("true".into()))
            })),
        ),
        (
            "archive".into(),
            session(Arc::new(|messages, _| {
                let input: Value = serde_json::from_str(&messages.last().unwrap().text).unwrap();
                assert_eq!(
                    input,
                    json!({"artifacts":{"review":"Approved for one desk"}})
                );
                Ok(AgentReply::Answer("Archived.".into()))
            })),
        ),
    ]);
    let plan = WorkflowPlan::new(
        JsonSchema::string(None, None),
        vec![
            step(
                "archive",
                &["review"],
                agent("archive", ArtifactFormat::Text),
                JsonSchema::string(None, None),
            ),
            step(
                "verify",
                &[],
                agent("verify", ArtifactFormat::Json),
                JsonSchema::integer(None, None),
            ),
            step(
                "budget",
                &[],
                agent("budget", ArtifactFormat::Json),
                JsonSchema::boolean(),
            ),
            step(
                "review",
                &["verify", "budget"],
                StepAction::Review,
                JsonSchema::string(None, None),
            ),
        ],
    )
    .unwrap();
    let workflows = InMemoryWorkflowStore::default();
    let checkpoints = MemoryCheckpointStore::default();
    let runner =
        WorkflowRunner::new(&plan, &workflows, &checkpoints, &agents, "purchasing").unwrap();
    let started = runner
        .start("request-42", json!("Purchase two desks"))
        .unwrap();
    assert_eq!(
        started
            .ready_steps()
            .map(|step| step.id.as_str())
            .collect::<Vec<_>>(),
        ["verify", "budget"]
    );
    assert!(matches!(
        runner.advance("request-42", "archive"),
        Err(WorkflowError::NotReady)
    ));

    std::thread::scope(|scope| {
        scope.spawn(|| runner.advance("request-42", "verify").unwrap());
        scope.spawn(|| runner.advance("request-42", "budget").unwrap());
    });

    let awaiting = runner.load("request-42").unwrap();
    let review = awaiting.inputs_for("review").unwrap();
    assert_eq!(
        review.artifacts,
        BTreeMap::from([("budget", &json!(true)), ("verify", &json!(2))])
    );
    assert!(matches!(
        runner.review(
            "request-42",
            "review",
            started.revision(),
            ReviewDecision::Approve(json!("Approved"))
        ),
        Err(WorkflowError::Stale { .. })
    ));
    assert!(matches!(
        runner.review(
            "request-42",
            "review",
            awaiting.revision(),
            ReviewDecision::Approve(json!(true))
        ),
        Err(WorkflowError::Schema(_))
    ));
    runner
        .review(
            "request-42",
            "review",
            awaiting.revision(),
            ReviewDecision::Approve(json!("Approved for one desk")),
        )
        .unwrap();
    let done = runner.advance("request-42", "archive").unwrap().workflow;
    assert!(done.is_completed());
    assert_eq!(
        done.state("archive").unwrap(),
        &StepState::Completed(json!("Archived."))
    );
}

#[test]
fn rejection_blocks_dependents_but_preserves_independent_steps() {
    let plan = WorkflowPlan::new(
        JsonSchema::any(),
        vec![
            step("review", &[], StepAction::Review, JsonSchema::any()),
            step(
                "publish",
                &["review"],
                StepAction::Review,
                JsonSchema::any(),
            ),
            step("notify", &[], StepAction::Review, JsonSchema::any()),
        ],
    )
    .unwrap();
    let workflows = InMemoryWorkflowStore::default();
    let checkpoints = MemoryCheckpointStore::default();
    let agents = BTreeMap::new();
    let runner =
        WorkflowRunner::new(&plan, &workflows, &checkpoints, &agents, "purchasing").unwrap();
    let started = runner.start("rejected", Value::Null).unwrap();
    let rejected = runner
        .review(
            "rejected",
            "review",
            started.revision(),
            ReviewDecision::Reject("Missing receipt.".into()),
        )
        .unwrap();
    assert_eq!(
        rejected.state("review").unwrap(),
        &StepState::Rejected("Missing receipt.".into())
    );
    assert_eq!(
        rejected
            .ready_steps()
            .map(|step| step.id.as_str())
            .collect::<Vec<_>>(),
        ["notify"]
    );
    assert!(matches!(
        rejected.inputs_for("publish"),
        Err(WorkflowError::NotReady)
    ));
    let notified = runner
        .review(
            "rejected",
            "notify",
            rejected.revision(),
            ReviewDecision::Approve(json!(true)),
        )
        .unwrap();
    assert_eq!(notified.ready_steps().count(), 0);
    assert!(!notified.is_completed());
}

#[test]
fn invalid_artifacts_can_be_repaired_without_rerunning_the_agent() {
    let calls = AtomicUsize::new(0);
    let calls = Arc::new(calls);
    let model_calls = Arc::clone(&calls);
    let agents = BTreeMap::from([(
        "extract".into(),
        session(Arc::new(move |_, _| {
            model_calls.fetch_add(1, Ordering::Relaxed);
            Ok(AgentReply::Answer(r#""invalid type""#.into()))
        })),
    )]);
    let plan = WorkflowPlan::new(
        JsonSchema::any(),
        vec![
            step(
                "extract",
                &[],
                agent("extract", ArtifactFormat::Json),
                JsonSchema::boolean(),
            ),
            step(
                "review",
                &["extract"],
                StepAction::Review,
                JsonSchema::any(),
            ),
        ],
    )
    .unwrap();
    let workflows = InMemoryWorkflowStore::default();
    let checkpoints = MemoryCheckpointStore::default();
    let runner =
        WorkflowRunner::new(&plan, &workflows, &checkpoints, &agents, "purchasing").unwrap();
    runner.start("invalid", Value::Null).unwrap();
    let invalid = runner.advance("invalid", "extract").unwrap().workflow;
    assert!(matches!(
        invalid.state("extract").unwrap(),
        StepState::InvalidOutput(_)
    ));
    assert_eq!(invalid.ready_steps().count(), 0);
    assert!(matches!(
        runner.repair_output("invalid", "extract", invalid.revision(), json!(1)),
        Err(WorkflowError::Schema(_))
    ));
    let repaired = runner
        .repair_output("invalid", "extract", invalid.revision(), json!(false))
        .unwrap();
    assert_eq!(
        repaired.state("extract").unwrap(),
        &StepState::Completed(json!(false))
    );
    assert_eq!(repaired.ready_steps().next().unwrap().id, "review");
    runner.advance("invalid", "extract").unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(
        runner
            .agent_checkpoint("invalid", "extract")
            .unwrap()
            .unwrap()
            .status()
            .unwrap(),
        CheckpointStatus::Completed(r#""invalid type""#)
    );
}

#[cfg(feature = "workflow-files")]
struct FailOnceOnCompletion<S> {
    inner: S,
    failed: AtomicBool,
}

#[cfg(feature = "workflow-files")]
impl<S: WorkflowStore> WorkflowStore for FailOnceOnCompletion<S> {
    fn load(&self, id: &str) -> Result<Option<Arc<WorkflowSnapshot>>, WorkflowError> {
        self.inner.load(id)
    }

    fn compare_exchange(
        &self,
        id: &str,
        expected: u64,
        next: Arc<WorkflowSnapshot>,
    ) -> Result<(), WorkflowError> {
        if next.is_completed() && !self.failed.swap(true, Ordering::Relaxed) {
            return Err(WorkflowError::Storage(std::io::Error::other(
                "simulated interruption",
            )));
        }
        self.inner.compare_exchange(id, expected, next)
    }
}

#[cfg(feature = "workflow-files")]
fn exercise_interruption(
    workflows: &dyn WorkflowStore,
    checkpoints: &dyn cove_agent::checkpoint::CheckpointStore,
    reopen: impl FnOnce() -> (
        Box<dyn WorkflowStore>,
        Box<dyn cove_agent::checkpoint::CheckpointStore>,
    ),
) {
    let writes = Arc::new(AtomicUsize::new(0));
    let executions = Arc::clone(&writes);
    let model_calls = Arc::new(AtomicUsize::new(0));
    let requests = Arc::clone(&model_calls);
    let toolbox = AgentToolbox::with_context(
        vec![AgentTool::new(
            "archive",
            "Archive the order",
            JsonSchema::any(),
            false,
        )],
        Arc::new(move |_, _, context| {
            assert!(context.checkpoint.is_some());
            executions.fetch_add(1, Ordering::Relaxed);
            Ok(AgentToolResult::encode(&json!({"record":42}), false))
        }),
    )
    .unwrap();
    let session = AgentSession::new(
        toolbox,
        &AgentSkillLibrary::default(),
        &[],
        3,
        Arc::new(move |messages, _| {
            requests.fetch_add(1, Ordering::Relaxed);
            if messages.last().unwrap().role == AgentRole::Tool {
                return Ok(AgentReply::Answer("42".into()));
            }
            Ok(AgentReply::Tools(vec![AgentToolCall {
                id: "write".into(),
                name: "archive".into(),
                arguments: json!({"order":1}),
            }]))
        }),
    )
    .unwrap();
    let agents = BTreeMap::from([("archive".into(), Arc::new(session))]);
    let plan = WorkflowPlan::new(
        JsonSchema::any(),
        vec![step(
            "archive",
            &[],
            agent("archive", ArtifactFormat::Json),
            JsonSchema::integer(None, None),
        )],
    )
    .unwrap();

    let runner = WorkflowRunner::new(&plan, workflows, checkpoints, &agents, "purchasing").unwrap();
    runner.start("order", json!(1)).unwrap();
    let paused = runner.drive_step("order", "archive", |_| false).unwrap();
    assert_eq!(
        paused.agent.unwrap().status().unwrap(),
        CheckpointStatus::Model
    );
    assert_eq!(model_calls.load(Ordering::Relaxed), 0);
    let approval = runner
        .drive_step("order", "archive", |_| true)
        .unwrap()
        .agent
        .unwrap();
    assert!(matches!(
        approval.status().unwrap(),
        CheckpointStatus::AwaitingApproval(_)
    ));
    assert_eq!(writes.load(Ordering::Relaxed), 0);
    let outsider = WorkflowRunner::new(&plan, workflows, checkpoints, &agents, "other").unwrap();
    assert!(matches!(
        outsider.load("order"),
        Err(WorkflowError::Invalid(_))
    ));
    assert!(matches!(
        runner.resolve_tool(
            "order",
            "archive",
            approval.revision() - 1,
            ToolDecision::Approve
        ),
        Err(WorkflowError::Agent(CheckpointError::Stale))
    ));
    runner
        .resolve_tool(
            "order",
            "archive",
            approval.revision(),
            ToolDecision::Approve,
        )
        .unwrap();
    assert!(matches!(
        runner.drive_step("order", "archive", |_| true),
        Err(WorkflowError::Storage(_))
    ));
    assert_eq!(
        runner.load("order").unwrap().state("archive").unwrap(),
        &StepState::Active
    );

    let missing_checkpoints = MemoryCheckpointStore::default();
    let missing = WorkflowRunner::new(
        &plan,
        workflows,
        &missing_checkpoints,
        &agents,
        "purchasing",
    )
    .unwrap();
    assert!(matches!(
        missing.advance("order", "archive"),
        Err(WorkflowError::Agent(CheckpointError::Missing))
    ));

    let (workflows, checkpoints) = reopen();
    let deserialized_plan = serde_json::from_value(serde_json::to_value(&plan).unwrap()).unwrap();
    let restarted = WorkflowRunner::new(
        &deserialized_plan,
        &*workflows,
        &*checkpoints,
        &agents,
        "purchasing",
    )
    .unwrap();
    let done = restarted.advance("order", "archive").unwrap().workflow;
    assert_eq!(
        done.state("archive").unwrap(),
        &StepState::Completed(json!(42))
    );
    assert_eq!(writes.load(Ordering::Relaxed), 1);
    assert_eq!(model_calls.load(Ordering::Relaxed), 2);
}

#[test]
#[cfg(feature = "workflow-files")]
fn file_backends_recover_a_completed_child_after_workflow_publish_fails() {
    use cove_agent::checkpoint::FileCheckpointStore;
    use cove_agent::workflow::FileWorkflowStore;

    let directory = tempfile::tempdir().unwrap();
    let workflows = FailOnceOnCompletion {
        inner: FileWorkflowStore::open(directory.path()).unwrap(),
        failed: AtomicBool::new(false),
    };
    let checkpoints = FileCheckpointStore::open(directory.path()).unwrap();
    exercise_interruption(&workflows, &checkpoints, || {
        (
            Box::new(FileWorkflowStore::open(directory.path()).unwrap()),
            Box::new(FileCheckpointStore::open(directory.path()).unwrap()),
        )
    });
}
