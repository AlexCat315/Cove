# Cove Agent

`cove-agent` 提供与界面无关的工具注册、参数验证、模型调用循环和 MCP。应用定义系统指令、可访问的数据及工具执行器。

启用 Cargo 特性 `openai-compatible` 后，可用 `ChatCompletionsModel` 连接支持 Chat Completions 的端点。该适配器支持普通答复和函数工具调用，并在后续工具回合保留提供方要求的推理上下文。API 地址可写为 `/v1` 基址或完整的 `/chat/completions` 地址。设备外的端点使用 HTTPS。

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

## 本地资料检索

`cove_agent::retrieval::SearchIndex` 可以为只读工具提供中英文关键词检索。它在进程内建立倒排索引，用 BM25 排序并优先匹配标题。应用负责准备资料、切分片段、处理更新和限定访问范围；索引返回片段序号，应用可以用它关联原文及出处。

```rust
use cove_agent::retrieval::SearchIndex;

let mut index = SearchIndex::default();
let source = index.add("人物笔记", "鹰纹银匣由晏澄保管。");
assert_eq!(index.search("鹰纹银匣", 3), vec![source]);
```
