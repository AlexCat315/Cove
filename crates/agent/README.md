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
