# Cove Agent

`cove-agent` 提供与界面无关的工具注册、参数验证、模型调用循环和 MCP。应用定义系统指令、可访问的数据及工具执行器。

启用 Cargo 特性 `openai-compatible` 后，可用 `ChatCompletionsModel` 连接支持 Chat Completions 的端点。该适配器支持普通答复和函数工具调用，并在后续工具回合保留提供方要求的推理上下文。API 地址可写为 `/v1` 基址或完整的 `/chat/completions` 地址。设备外的端点使用 HTTPS。

`ChatCompletionsModel::list_models(base, api_key)` 查询同一 API 基址下的 `/models` 并返回模型 ID。未实现该端点的服务仍可通过指定模型 ID 连接。列表请求和生成请求都不进入网络录制。

```rust
use std::sync::Arc;

use cove_agent::openai_compatible::ChatCompletionsModel;
use cove_agent::{AgentError, AgentMessage, AgentSession, AgentSkillLibrary, AgentToolbox};

fn run() -> Result<String, AgentError> {
    let model = ChatCompletionsModel::new(
        "http://127.0.0.1:11434/v1",
        "your-installed-model",
        None,
    )?;
    let toolbox = AgentToolbox::new(
        Vec::new(),
        Arc::new(|name, _| Err(AgentError::UnknownTool(name.to_owned()))),
    )?;
    let session = AgentSession::new(
        toolbox,
        &AgentSkillLibrary::default(),
        &[],
        6,
        model.into_agent_model(),
    )?;
    Ok(session.run(vec![AgentMessage::user("请写一段开场。")])?.answer)
}
```

`AgentSession::run` 会等待模型及工具返回，桌面和移动应用应在工作线程运行。使用工具时，以 `AgentTool` 声明参数并注册执行器。应用自行决定工具可以访问的数据，以及何时由用户确认写入。模型请求和答复不进入 Cove 的网络录制。

### 流式答复与修订差异

`ChatCompletionsModel::complete_stream` 在当前线程读取响应，并通过回调交付可见正文的增量。回调返回 `false` 时停止读取。完成后的 `AgentReply` 保留模型的工具请求和推理字段。应用可在工作线程调用该方法，再把增量发给 UI 线程。

`revision::change_span` 返回两版文本中变化部分的 UTF-8 字节范围，供修订核对或高亮使用。

## 持久化任务与恢复

`checkpoint::AgentRunner` 使用已有的 `AgentSession`。每次 `advance` 处理一个模型回合、一个工具调用或一个审批状态变化，并保存检查点。应用可以用 `load` 读取进度，用 `drive` 运行到下一处待确认状态或任务结束，也可以在步骤之间暂停。任务完成后，用 `continue_with` 追加用户消息。调用可能阻塞，桌面和移动应用应在工作线程执行 `advance`。

启用 `checkpoint-files` 后，可以用 `FileCheckpointStore` 在支持文件锁的系统中跨进程恢复。每个任务使用独立的文件锁，检查点文件仅当前用户可访问。也可使用进程内的 `MemoryCheckpointStore`，或实现 `CheckpointStore` 和 `CheckpointLease` 接入应用的存储。检查点包含会话消息和工具结果，应视作应用的私有数据。

```rust
use cove_agent::checkpoint::{AgentRunner, CheckpointError, CheckpointStatus, CheckpointStore, ToolPolicy};
use cove_agent::{AgentMessage, AgentSession};

fn next_inventory_step(
    session: &AgentSession,
    store: &dyn CheckpointStore,
) -> Result<(), CheckpointError> {
    let runner = AgentRunner::new(session, store, "warehouse-3:current-account")?;
    match runner.load("stock-audit") {
        Err(CheckpointError::Missing) => {
            runner.start(
                "stock-audit",
                vec![AgentMessage::user("核对库存和待发货数量。")],
                ToolPolicy::ConfirmWrites,
            )?;
        }
        Ok(_) => {}
        Err(error) => return Err(error),
    }

    let checkpoint = runner.advance("stock-audit")?;
    match checkpoint.status()? {
        CheckpointStatus::AwaitingApproval(call) => {
            // Present this call and checkpoint.revision() to the operator.
            println!("待确认的工具 {}", call.name);
        }
        CheckpointStatus::ToolInFlight { read_only: false, .. } => {
            // Reconcile the external system, then record a verified outcome.
            println!("写入结果待核对");
        }
        CheckpointStatus::Completed(answer) => println!("{answer}"),
        _ => {}
    }
    Ok(())
}
```

一个任务重启后，宿主用相同的作用域、任务 ID 与存储恢复。作用域由宿主绑定用户、工作区和工具执行权限。模型请求失败会保留进度，已保存的工具结果会随对话保留。中断的只读工具可以重试，已开始但结果不明的写操作由宿主核对，再用 `resolve` 记录结果。启用 `ConfirmWrites` 时，宿主也通过 `resolve` 审批或拒绝工具调用；决策须带上展示给操作者的检查点版本号。原有 `AgentSession::run` 可继续用于一次性请求。

工具实现可选用 `AgentToolbox::with_context`。它会收到稳定的调用标识，便于在外部系统查询写入回执，或实现幂等去重。恢复时可用 `checkpoint.tool_key()` 取得同一个标识，核对后再提交 `RecordOutcome`。

## 通用工作流

`workflow::WorkflowPlan` 声明步骤、依赖、输入与产物的 JSON Schema。Agent 步骤可分别绑定模型、技能和工具。每个步骤接收直接依赖的产物和它明确请求的原始输入。Cove 在接收计划时检查循环和缺失依赖，在放行下游前校验产物。

```rust
use std::{collections::BTreeMap, sync::Arc};
use cove_agent::{AgentSession, JsonSchema};
use cove_agent::checkpoint::{CheckpointStore, ToolPolicy};
use cove_agent::workflow::{
    ArtifactFormat, StepAction, WorkflowError, WorkflowPlan, WorkflowRunner,
    WorkflowStep, WorkflowStore,
};
use serde_json::json;

fn start_purchase(
    store: &dyn WorkflowStore,
    checkpoints: &dyn CheckpointStore,
    agents: &BTreeMap<String, Arc<AgentSession>>,
) -> Result<(), WorkflowError> {
    let text = JsonSchema::string(None, None);
    let plan = WorkflowPlan::new(text.clone(), vec![
        WorkflowStep {
            id: "verify".into(),
            dependencies: vec![],
            uses_input: true,
            action: StepAction::Agent {
                agent: "purchasing".into(),
                instructions: "核实采购数量与交付日期，给出审核摘要。".into(),
                format: ArtifactFormat::Text,
                tool_policy: ToolPolicy::ConfirmWrites,
            },
            output: text.clone(),
        },
        WorkflowStep {
            id: "approve".into(),
            dependencies: vec!["verify".into()],
            uses_input: false,
            action: StepAction::Review,
            output: text,
        },
    ])?;
    let runner = WorkflowRunner::new(&plan, store, checkpoints, agents, "office-a")?;
    runner.start("request-42", json!("采购两张办公桌，月底前交付。"))?;
    runner.advance("request-42", "verify")?;
    Ok(())
}
```

- 宿主通过 `ready_steps` 发现可推进的步骤，可在工作线程并行运行互不依赖的步骤。用 `advance` 推进单步，或用 `drive_step` 运行到检查点、工具确认或产物提交。遇到工具确认或中断写入后，用 `agent_checkpoint` 检查状态和版本，再用 `resolve_tool` 提交决定或回执。
- 审核前通过 `inputs_for` 展示待审材料。宿主用当前工作流版本调用 `review`，明确提交批准产物或拒绝理由。拒绝会阻塞依赖该审核的步骤，独立分支仍可继续。模型产物不符合声明时，宿主可以用 `repair_output` 提交有效产物，原始答复仍保存在子检查点。
- 启用 `workflow-files` 后，`FileWorkflowStore` 与 `FileCheckpointStore` 共同保存进度和子检查点。跨进程恢复时复用同一对存储、计划、作用域和 Agent 绑定。宿主应为这对存储分配唯一工作流 ID，并为工作流保留以 `wf-` 开头的检查点 ID。如果子 Agent 已完成而产物保存失败，下一次推进会补交产物，无需重新调用 Agent。

宿主负责调度、审核授权、模型与工具配置，以及归档这对存储。也可实现 `WorkflowStore` 和 `CheckpointStore` 对接应用自己的存储。

## 带证据的长期记忆

`memory::MemoryCatalog` 管理同一授权空间里的事实、偏好、决定与任务。宿主提供来源 ID、当前版本和能回查的位置，并提交候选记录。相同 `key` 对应一条当前结论，确认替换时需给出被替换记录的 ID。已替换的事实和决定可通过历史记录查询。

```rust
use cove_agent::memory::{
    EvidenceRef, InMemoryStore, MemoryCatalog, MemoryChange, MemoryError,
    MemoryKind, MemoryProposal, RecallQuery,
};

fn recall_custodian() -> Result<Option<String>, MemoryError> {
    let store = InMemoryStore::default();
    let catalog = MemoryCatalog::new(&store, "warehouse-a")?;
    let initial = catalog.snapshot()?;
    let snapshot = catalog.apply(initial.revision(), &[
        MemoryChange::ObserveSource {
            source: "handover".into(),
            revision: Some("v17".into()),
        },
        MemoryChange::Propose(MemoryProposal {
            id: "a42-handover-17".into(),
            key: "inventory.a42.custodian".into(),
            kind: MemoryKind::Fact,
            title: "A42 保管点".into(),
            content: "A42 由三号码头保管。".into(),
            evidence: vec![EvidenceRef {
                source: "handover".into(),
                revision: "v17".into(),
                locator: "entry:17".into(),
            }],
        }),
        MemoryChange::Confirm { id: "a42-handover-17".into(), replaces: None },
    ])?;
    Ok(snapshot.recall(RecallQuery {
        text: "A42 保管点",
        kinds: &[MemoryKind::Fact],
        limit: 4,
        max_bytes: 4096,
    }).first().map(|entry| entry.proposal.content.clone()))
}
```

- `ObserveSource` 记录版本变化或删除。引用过期版本的已确认事实进入 `NeedsReview`。即便来源后来回滚到原版本，也要明确复核才能再次生效。
- `recall` 在不可变快照上检索已确认的记录，按类型、条数及 JSON 字节预算返回证据。宿主可用 `recall_where` 在检索前附加记录级权限。下一轮可读取新快照，这一轮可以沿用已经选定的证据版本。
- `apply` 一次提交一批变更。证据过期、结论冲突或写入版本过时，都会使 Cove 拒绝整批变更。使用 `InMemoryStore`、启用 `memory-files` 的本地文件后端，或实现 `MemoryStore` 接入数据库。文件后端使用按空间隔离的原子 JSON 快照，适用于支持文件锁的系统。

宿主负责发现源资料的变更并传递给 Cove，也负责决定怎样生成候选、谁有权确认，以及允许 Agent 查阅哪个空间。来源可以是文件、数据库记录或业务事件。


## 本地资料检索

`cove_agent::retrieval::SearchIndex` 可以为只读工具提供中英文关键词检索。它在进程内建立倒排索引，用 BM25 排序并优先匹配标题。应用负责准备资料、切分片段、处理更新和限定访问范围；索引返回片段序号，应用可以用它关联原文及出处。

```rust
use cove_agent::retrieval::SearchIndex;

let mut index = SearchIndex::default();
let source = index.add("人物笔记", "鹰纹银匣由晏澄保管。");
assert_eq!(index.search("鹰纹银匣", 3), vec![source]);
```

## 检查生成文本的重复段落

`repetition::find_repetition` 可在预览阶段查找结果中重复或来自参考稿的长文本。它忽略大小写、空白和标点，返回结果原文中的 UTF-8 范围，便于界面定位。调用方决定参考稿范围以及如何提示作者。
