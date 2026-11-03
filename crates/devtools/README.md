# Cove DevTools

Rust 2024 运行时诊断、Inspector 和 AI 工具。应用端使用 WebSocket；AI 通过独立的 `cove-devtools` MCP 服务连接，同一会话复用一条应用连接。

## 启动与 MCP 接入

```sh
COVE_DEVTOOLS=1 cargo run --package cove-examples
cargo run --quiet --package cove-devtools-cli -- mcp
```

仓库根目录的 `.mcp.json` 提供项目配置。支持该配置的 MCP 客户端可直接加载；其他客户端注册同一 stdio 命令：

```json
{
  "mcpServers": {
    "cove-devtools": {
      "command": "cargo",
      "args": ["run", "--quiet", "--manifest-path", "/absolute/path/to/Cove/Cargo.toml", "--package", "cove-devtools-cli", "--", "mcp"]
    }
  }
}
```

服务可在应用启动前完成 `initialize`、`tools/list` 和任务指南发现。第一次操作时连接应用；连接失效后，下次调用重新连接。写操作失败不会自动重放。

默认地址为 `ws://127.0.0.1:9229/`。通过 `--endpoint 9231`、`--endpoint ws://127.0.0.1:9231/` 或 `--endpoint @session.json` 指定目标，也可设置 `COVE_DEVTOOLS_ENDPOINT`。没有显式目标时依次读取 `COVE_DEVTOOLS_SESSION_FILE`、`COVE_DEVTOOLS_PORT`，最后使用 9229。

应用端设置 `COVE_DEVTOOLS_SESSION_ID`、`COVE_DEVTOOLS_TOKEN` 和 `COVE_DEVTOOLS_SESSION_FILE` 后，将含认证端点的会话文件写入指定路径。MCP 服务重新连接时重读文件；连接 token 不会写入工具结果或连接错误。Android 使用 `adb forward tcp:9229 tcp:9229` 后连接本地端口。

MCP 服务在 macOS、Linux 和 Windows 使用同一 Rust 实现，不依赖 UI/GPU 库。设备上的应用通过可达的 WebSocket 或端口转发接入；浏览器内的 MessageChannel Inspector 仍由浏览器宿主连接。具体工具是否可用取决于 `get_ui_session` 返回的实际 host capabilities。

## 直接使用工具

| 任务 | 工具 |
| --- | --- |
| 连接与能力发现 | `get_ui_session`、`get_diagnostics` |
| 查找节点、布局与源位置 | `query_ui_nodes` |
| 点击、填值、键盘、拖拽与滚动 | `click_ui`、`fill_ui`、`press_ui_key`、`input_ui` |
| 可访问性动作与滚动到节点 | `activate_ui` |
| 等待可见、隐藏、启用或字段值 | `wait_for_ui` |
| 截图与窗口尺寸 | `capture_ui`、`resize_ui` |
| 选择与临时样式 | `select_ui`、`set_ui_style`、`edit_ui_style_history` |
| 日志、重组、公开状态与内存 | `query_ui_logs`、`query_ui_recomposition`、`query_ui_state`、`query_ui_memory` |
| 帧耗时 | `record_ui_timings` |
| 围绕操作录制 CPU 时间线 | `start_ui_trace`、`stop_ui_trace` |
| 固定时段录制、分页与比较 | `capture_ui_trace`、`get_ui_trace`、`compare_ui_traces` |
| 使用指南 | `list_skills`、`read_skill`，或 MCP `prompts/list`、`prompts/get` |

`query_ui_nodes` 在 MCP/CLI 中默认 `compact: true`，返回节点 ID、语义、可见边界和源位置。设 `compact: false` 可读取完整布局与样式。角色筛选兼容 `button` / `Button`、`textField` / `TextField`；组合控件使用与原生无障碍一致的可见文字。普通发现不读取懒加载字段值。读取字段需显式指定 `node_id` 和 `include_value: true`，上限 4096 个字符并报告 `valueTruncated`；安全字段返回 `<redacted>`，不执行值 provider。

交互工具的 `selector` 接受 `node_id`、`label`、`text`、`role` 和 `source`。`label` 精确匹配，`text` 为子串；点击和填值只接受唯一、可见、启用且未被模态作用域阻挡的目标。歧义会返回候选 ID。节点卸载后 ID 失效。

常用调用参数：

```json
{"selector":{"label":"Save","role":"button"},"capture":true}
{"selector":{"label":"Name","role":"textField"},"text":"Cove"}
{"selector":{"node_id":"177"},"state":"value","expected":"Cove","timeout_ms":3000}
{"key":"Enter"}
{"scale":2,"settle_ms":400}
```

截图通过 MCP `image` content 返回，同时提供像素和逻辑尺寸；不会把 base64 放进文字上下文。`click_ui` / `fill_ui` 的 `capture: true` 在同一连接上立即截图。截图不包含 OS 窗口装饰；`settle_ms` 是 0–2000 ms 的采样延迟，不是异步完成断言。请使用 `wait_for_ui` 验证实际状态。

`input_ui` 接受最多 256 个有类型的事件。指针坐标为逻辑点，滚轮位移使用点单位；键盘使用 `KeyboardEvent.code`。修饰键位为 Shift=1、Ctrl=2、Alt=4、Meta=8。`fill_ui` 根据远端平台选择全选快捷键，支持空字符串清空。事件在全部校验后才交付；每个 wait 最多 2000 ms，总等待最多 10000 ms。输入成功回执只确认交付。

CPU trace 可先 start，再执行真实交互，最后 stop；后台连接线程会持续读取，避免录制期间空闲的 MCP 客户端阻塞宿主。事件与结果有上限，超出部分报告损失，最多保留八个 capture。停止时会读取最后一批 span。空闲采集可能为空；父子 span 重叠，CPU present 不代表 GPU 执行时间，帧样本也不代表显示 FPS。

## 无 MCP 客户端时的 CLI

相同工具可以直接调用，无需编写 WebSocket 脚本：

```sh
cargo run --quiet -p cove-devtools-cli -- tools
cargo run --quiet -p cove-devtools-cli -- --endpoint 9231 call query_ui_nodes '{"text":"Save","role":"button"}'
cargo run --quiet -p cove-devtools-cli -- call click_ui '{"selector":{"label":"Save","role":"button"}}'
cargo run --quiet -p cove-devtools-cli -- call capture_ui '{"scale":2}' --image /tmp/cove.jpg
```

一次 CLI 调用拥有一个连接。需要跨调用保持选择、样式历史或 start/stop trace 时使用 MCP 服务。JSON 输出不含截图 base64；`--image` 保存 JPEG。

## Inspector 与宿主集成

浏览器打开 `crates/devtools/client/index.html`，连接运行时端点。Inspector 提供树、镜像、布局、临时样式、重组统计、公开状态、日志、性能与宿主已安装的 checkpoint / 输入录制。流按连接显式订阅，断开后释放所属临时检查状态。

框架宿主创建 `DevTools`，通过 `set_input_delivery` 安装输入回调，按需安装帧读取、状态、窗口和内存 provider，然后启动；每轮 UI 事件泵调用 `pump`，布局或帧完成后调用对应通知。`input`、`accessibility` 和 `mirror` 分别描述原始输入、语义动作和图片读取，截图缺失不妨碍已安装的输入工具。没有 provider 的能力不会被假定存在。

协议见 [Inspection](../devtools-protocol/Inspection.md)、[Analysis](../devtools-protocol/Analysis.md)、[Observation](../devtools-protocol/Observation.md) 和 [Logs](../devtools-protocol/Logs.md)。

```sh
cargo test -p cove-agent -p cove-devtools-protocol -p cove-devtools -p cove-devtools-cli --locked
```
