# Cove DevTools 运行结构

调试构建的应用会启动内嵌控制器，也可加入已有控制器。运行 Gallery 只需一条命令。

```sh
cargo run -p cove-examples --bin gallery --features media-ffmpeg
```

控制器为每个 Cove 运行时分配独立目标 ID。Inspector、MCP 与 CLI 共用 JSON-RPC 工具。探针通过短期租约接收命令，在对应运行时的 UI 线程读取组件树、状态、布局、语义和性能数据。

## 应用内画面和输入

| 平台 | 画面来源 | 输入目标 |
| --- | --- | --- |
| macOS、iOS / iPadOS | Cove 的 Metal 渲染目标 | Cove 场景事件分发 |
| Windows | Cove 的 DX12 或 Vulkan 渲染目标 | Cove 场景事件分发 |
| Linux（X11 与 Wayland）、Android | Cove 的 Vulkan 渲染目标 | Cove 场景事件分发 |
| Web | Cove 自己的 WebGPU 或 Canvas2D 画布 | 页面内的 Cove 运行时 |

画面读取使用应用持有的渲染资源。点击、拖动和滚动携带画面 ID 与相对像素坐标，运行时复核视口再分发事件。发送后用 `wait_for_ui` 或重新查询节点确认结果。语义面板由 Cove 场景树生成。系统菜单、原生文件面板和输入法候选窗位于运行时边界之外。

帧列表报告 UI、提交与 GPU 耗时。选中帧可查看 [GPU pass 与合成层](devtools-gpu-breakdown.md)，CPU trace 可以按帧查看或比较。平台与操作验收见 [Gallery 审计](gallery-audit.md)。[DevTools 工具说明](../crates/devtools/README.md)
