# Cove DevTools 协议

控制器持有目标和会话。`control.rs` 定义原生和 WASM 探针共用的注册、轮询、命令和错误类型。

| 端点 | 用途 |
| --- | --- |
| `POST /probe/register` | 使用配对凭据提交带协议版本的 `Registration`，返回目标 ID 和探针凭据。 |
| `POST /probe/poll` | 使用探针凭据提交上一条命令的结果，等候下一条带序号的 `Work`。 |
| `POST /probe/close` | 释放探针会话。未续租的会话自动过期。 |
| `POST /rpc` | Inspector 和 CLI/MCP 通过控制器凭据调用同一 JSON-RPC 命令。 |
| `GET /captures/{id}` | 读取控制器短期保存的 PNG 资源。 |

探针命令处理组件树、诊断读取、临时样式、环境、尺寸、日志、帧计时和 CPU trace。平台输入和截图由控制器调度，探针类型中没有这两类操作。已交付而未返回结果的命令报告结果未知。调用方重新检查运行状态后可决定下一步。

查看 [启动和工具用法](../devtools/README.md)。
