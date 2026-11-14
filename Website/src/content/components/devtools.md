# DevTools

调试版自带控制器，一条命令启动带 DevTools 的调试版。

```sh
cargo run -p cove-examples --bin gallery --features media-ffmpeg
```

终端打印 Inspector 链接，CLI 与 MCP 直接可用。已有控制器在运行时自动配对；`COVE_DEVTOOLS=0` 关闭 DevTools。

从仓库根目录加载 `.mcp.json` 即可启动 MCP 服务。若客户端找不到 Cargo，在客户端配置中把 `command` 设为本机 Cargo 的绝对路径。

在 Inspector 选中应用，点击 **Capture** 查看 Cove 渲染画面。**Pick** 查找像素下的组件，**Interact** 在应用内点击、拖动或滚动。键盘和文本输入发往选中运行时内的控件。

```sh
cargo run --quiet -p cove-devtools-cli -- call get_ui_session '{}'
cargo run --quiet -p cove-devtools-cli -- call capture_ui '{"target":"RUNTIME_ID"}' --image frame.png
cargo run --quiet -p cove-devtools-cli -- call click_ui '{"target":"RUNTIME_ID","capture_id":"CAPTURE_ID","x":120,"y":84}'
```

`x` 和 `y` 是图片左上角起算的像素坐标。窗口尺寸改变后重新获取画面。用 `wait_for_ui` 检查操作后的控件状态。输入回执表示事件已交付，原生键盘和 IME 需要独立验收。

画面与输入范围为 Cove 场景。系统菜单、原生文件面板和输入法候选窗由操作系统管理。

需要暂停应用并查看源码时，用 `cargo cove run --devtools` 一条命令启动 Gallery 和独立控制器。

## 布局与性能

在 **Inspector → Layout** 查看选中节点的布局链。每个轴显示 Taffy 收到的固定尺寸、可用空间和百分比基准，另列出求解尺寸与最终发布的尺寸。子节点越界按各边逻辑点数显示。`Text` 绘制时报告截断和内容溢出。

**Performance → Components** 按当前挂载组件的重建、布局或绘制计数排序。重建从挂载后开始计数，布局计入完整求解，绘制计入实际生成内容的节点。**Frames** 可按最慢阶段排序。GPU 时间单独比较帧预算。帧槽内发生的着色器和管线创建会显示次数及 CPU 时间。

在 **Performance → Frames** 选中一帧可查看 **GPU passes**，包括 GPU 实测时间、绘制次数和关联的合成层。时间采样取决于后端和设备。合成层显示缓存与裁剪状态。展开帧详情后导出可把对应的 pass 数据保存到性能快照。

在 **Performance → CPU Profile** 选择时长，开始记录后操作要分析的控件，再停止并查看调用树和热点。面板由运行时的 `cpu_profile` 能力启用。原生宿主显示 UI 线程采样，Windows 调试版可从匹配的 PDB 解析函数和源码位置。WebAssembly 显示记录的帧数和各阶段耗时。

**Semantics** 查看 Cove 场景中的语义层级和控件状态，点击行可定位到 Inspector。文本输入草稿和安全字段值从语义树中排除。

```sh
cove-devtools call query_ui_layout '{"node_id":"NODE_ID"}'
cove-devtools call query_ui_component_activity '{"sort_by":"layout","limit":20}'
cove-devtools call analyze_ui_frame '{"frame":42}'
cove-devtools call query_ui_semantics '{"limit":20}'
```
