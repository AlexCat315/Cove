# Cove DevTools 协议

控制器持有目标和会话。`control.rs` 定义原生和 WASM 探针共用的注册、轮询、命令和错误类型。

| 端点 | 用途 |
| --- | --- |
| `POST /probe/register` | 使用配对凭据提交带协议版本的 `Registration`，返回目标 ID 和探针凭据。 |
| `POST /probe/poll` | 使用探针凭据提交上一条命令的结果，等候下一条带序号的 `Work`。 |
| `POST /probe/close` | 释放探针会话。未续租的会话自动过期。 |
| `POST /rpc` | Inspector 和 CLI/MCP 通过控制器凭据调用同一 JSON-RPC 命令。 |
| `GET /captures/{id}` | 读取控制器短期保存的 PNG 资源。 |

探针命令处理组件树、诊断读取、临时样式、环境、尺寸、日志、帧计时、CPU trace、UI 线程 CPU 采样（`profile_start`、`profile_stop`、`profile_read`、`profile_hotspots`）、调试绘制（`debug_paint` 的 guidelines 逐帧用真实节点 `absolute_frame` 画 1px 边框，根视口与裁剪链生效并设有上限；`baselines` 用节点 `text_measure()` 的真实 `TextLayoutResult` 逐行画 1px 基线，同样按根视口和裁剪链裁剪并设上限，没有文本布局的节点不画；`oversized_images` 读取 `Image` 每次绘制记录的真实解码像素尺寸与显示像素尺寸，源面积超过显示面积 4 倍时画 2px 边框，跳过矢量来源、未知尺寸和非有限/零显示尺寸，同样按根视口和裁剪链裁剪并设上限；`repaint_highlight` 读取节点在真实绘制路径中累加的 repaint 代数，为已绘制节点画 2px 循环配色边框，命中图层缓存的复用不算重绘、颜色保持不变，从未绘制的节点不画，同样按根视口和裁剪链裁剪并设上限）和全局动画时间缩放（`animation_scale`：将动画增量乘以缩放值，限制在 0.05–10 并返回实际应用值；帧计时、输入和诊断仍按墙钟时间）。采样与符号化仅在 macOS 可用，其他平台返回 `unavailable` 及原因。平台输入和截图由控制器调度，探针类型中没有这两类操作。已交付而未返回结果的命令报告结果未知。调用方重新检查运行状态后可决定下一步。

查看 [启动和工具用法](../devtools/README.md)。
