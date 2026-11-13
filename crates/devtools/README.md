# Cove DevTools

Inspector、MCP 和 CLI 共用一个 Rust 控制服务。原生宿主与 WASM 应用通过同一个 Rust 探针提供 Cove 组件、源码、日志、样式和 CPU 时间线。画面由应用自己的渲染器提供，调试输入在所选 Cove 运行时内分发。

## 启动

调试版自带控制器，一条命令即可：

```sh
cargo run -p cove-examples --bin gallery --features media-ffmpeg
```

终端打印 Inspector 链接，CLI 与 MCP 直接可用，无需第二个命令。调试版默认开启 DevTools，`COVE_DEVTOOLS=0` 关闭；发布版默认关闭。

已有控制器在运行时（例如单独执行 `cargo run -p cove-devtools-cli -- serve`），应用直接配对，不再另起控制器。显式设置 `COVE_DEVTOOLS_ENDPOINT` 或 `COVE_DEVTOOLS_TOKEN` 时沿用该端点，即使它暂时不可达，也不会被进程内控制器覆盖。

控制器默认监听 `127.0.0.1:9229`，端口被占用时改用临时端口并打印实际链接。链接内含配对凭据，仅供当前开发会话使用。控制器把端点和凭据写入当前用户的本地配置目录，用 `COVE_DEVTOOLS_SESSION_FILE` 可指定发现文件。

MCP 客户端加载仓库的 `.mcp.json`，它调用 `Tools/DevTools/mcp.sh`：GUI 启动的客户端 PATH 不含 Rust 工具链和 sccache，脚本补齐后 exec `cargo run --quiet -p cove-devtools-cli -- mcp`。MCP 可以先启动，工具调用时读取控制器发现文件。CLI 使用同一组命令：

```sh
cargo run --quiet -p cove-devtools-cli -- tools
cargo run --quiet -p cove-devtools-cli -- call get_ui_session '{}'
cargo run --quiet -p cove-devtools-cli -- call query_ui_nodes '{"target":"RUNTIME_ID","text":"Actions:"}'
cargo run --quiet -p cove-devtools-cli -- call capture_ui '{"target":"WINDOW_ID"}' --image capture.png
```

## 平台接入

| 平台 | 接入方式 |
| --- | --- |
| macOS、Windows、Linux | 调试版用 `COVE_DEVTOOLS=1` 注册。控制器读取该窗口的 GPU 渲染目标。窗口被遮挡时，调试宿主继续渲染到自己的纹理。 |
| iOS / iPadOS | 用 Xcode 启动设置了 `COVE_DEVTOOLS=1` 的调试版。模拟器可连接配套 Mac 的控制器。真机通过隧道接入控制器。 |
| Android | Debug APK 可用 `adb reverse tcp:9229 tcp:9229` 接入。启动 `com.cove.probe.MainActivity` 时，把含 `endpoint`、`token` 和可选 `appTitle` 的 UTF-8 JSON 进行 Base64 编码，传给字符串 extra `cove_devtools`。 |
| Web | 用 `serve --web-root crates/browser/dist` 提供 WASM 页面。将 Inspector 链接的路径改为 `/app/` 并保留片段，打开即可注册。画面来自页面自己的 Canvas。 |

macOS 自动化可用 `SDL_WINDOW_ACTIVATE_WHEN_SHOWN=0 SDL_WINDOW_ACTIVATE_WHEN_RAISED=0 COVE_DEVTOOLS=1` 启动 Gallery，保留当前应用的键盘焦点。

## 操作与验收

1. 用 `get_ui_session` 选择 Cove 运行时。各窗口有独立目标 ID。
2. `capture_ui` 获取应用画面的 PNG、尺寸和 capture ID。 `capture_ui_node` 裁出节点的可见画面区域。高分辨率画面超过传输上限时按比例缩小。
3. `select_ui_at` 用画面像素命中组件。在 Inspector 选择 **Interact**，可以在画面上点击、拖动或滚动。CLI/MCP 的 `click_ui` 与 `input_ui` 使用同一个 capture ID，坐标原点是返回图片的左上角。视口尺寸变化后重新获取画面。
4. `press_ui_key` 和 `type_ui_text` 在运行时内分发按键和已提交文本。用 `wait_for_ui` 验证节点可见性或预期值。值断言需要 `node_id` 与 `expected`。安全字段保留脱敏。

DevTools 的画面范围为 Cove 渲染内容。系统标题栏、原生文件面板、软键盘和输入法候选窗由操作系统管理。应用内输入验收覆盖 Cove 控件的交互与状态变化，物理设备和原生输入法验收需要实际操作。

`start_ui_trace` 与 `stop_ui_trace` 可包围实际操作，`get_ui_trace` 分页读取，`compare_ui_traces` 比较 CPU 阶段。帧计时区分 CPU 提交、渲染线程完成和宿主能观察到的呈现操作。GPU 执行和屏幕扫描输出没有在这些 CPU 样本中计量。

`start_ui_profile hz=1000 duration_ms=5000` 用 `cove-profiling` 的真实采样器跟随 UI 线程，`stop_ui_profile` 停止、join 并在后台线程完成符号化，`get_ui_profile` 返回有界聚合调用栈（不逐样本返回），`query_ui_hotspots` 按函数和 `file:line` 返回 self/inclusive 热点。单次采样上限 10 秒；self 时间是独占时间，inclusive 时间在同一采样内对每帧只计一次，时间按相邻采样的实际间隔累计。仅 macOS 提供真实采样；其他平台返回 `unavailable` 及原因，不合成样本。

## 客户端源码结构

客户端源码分四棵目录树，全部由 `client/manifest.json` 声明拼装顺序：

- `client/markup/`：`head.html`、`shell.html`（页头、rail、`<main>` 起始）、`views/`（每个面板一段 `<section>`）、`dialogs.html`、`tail.html`，按序拼成 `/index.html`。
- `client/css/`：`base.css`，`shell/`（页头、布局、setup、modal、toast、滚动条、响应式），`inspector/`（toolbar、grid、states、details、screenshot、detail tabs、flex explorer、input drawer），`panels/` 与 `panels/performance/` 对应各面板样式，按序拼成 `/inspector.css`。
- `client/src/`：`core/` 放全局引用与共享状态（`globals.js`、`format.js`、`session.js`、`ui-helpers.js`、`debug-paint.js`），`inspector/` 放节点树（`tree.js`）、详情编辑器（`editors.js`）、画面捕获与输入，`shell/` 放视图切换、事件绑定与启动。多文件面板各自成目录：`panels/memory/`（快照、采样、基线、视图）、`panels/debugger/`（模型、会话、视图）、`panels/app-size/`（模型、分区、treemap、diff）、`panels/deep-links/`（模型、视图）、`panels/performance/profile/`（模型、控制、树视图、面板视图）；单文件面板（`network`、`logs`、`state`、`extensions`、`overview`、`frames`、`timeline`、`snapshots`）留在 `panels/` 或 `panels/performance/`。按序拼成 `/inspector.js`。
- `client/tests/`：harness 与共享静态服务。

三个 bundle 都由服务端把分片**字节拼接**成单份资源：JS 分片共享同一顶层作用域，不是独立 ES module，`import`/`export` 不会跨分片生效；HTML 与 CSS 拼接后仍是同一份文档与样式表。因此新增、拆分或改名分片只需改 `manifest.json`；生产端表由 `crates/devtools-cli/build.rs` 从 manifest 生成，漏登记的同类文件会让构建直接失败，不依赖任何手写列表。`client/tests/harness-server.mjs` 以同一种方式拼接并提供静态文件，各 harness 只保留自己的 fake `/rpc` 控制器与场景。

## 调试器

`get_ui_session` 的 `debugger` 字段报告 `lldb-dap` 路径、版本、Developer mode 和已有会话。`attach_ui_debugger` 仅对本机桌面运行时按 PID 启动异步 attach，默认最多等待 8 秒；被 macOS 的 task_for_pid 授权拒绝时返回 `unavailable` 并保留适配器原文，等待超时返回 `timeout` 并清理适配器与 debugserver。`get_ui_debugger` 读取会话状态、停止原因和断点，`set_ui_breakpoint`/`clear_ui_breakpoint` 按绝对源码路径设置或清除断点，`control_ui_debugger` 执行 pause、resume、step_over、step_in、step_out。`get_ui_debug_stack` 返回线程、帧和顶层作用域，`read_ui_debug_variables` 分页读取变量，`evaluate_ui_debug` 在已停止的帧中求值。运行中的进程返回 `not_stopped`，不返回旧帧或伪造的暂停状态；`detach_ui_debugger` 要求适配器不终止目标。

## 深度链接

`get_ui_deep_links root=/绝对项目路径` 只读取项目根目录内固定的平台文件：Android `Platforms/Android/app/src/main/AndroidManifest.xml`、macOS `Info.plist`、`Platforms/{macOS,iOS}` 下的 `Info.plist`/`*.entitlements`，以及已知位置的 `assetlinks.json`/`apple-app-site-association`。根路径必须是绝对路径且不含 `..`；任何解析到根目录之外的符号链接都会被拒绝，单个文件上限 4 MiB。

Android 用真实 XML 解析器读取 intent filter 的 VIEW、BROWSABLE、scheme/host/path、`autoVerify` 和行号；Apple 用 plist 解析器读取 `CFBundleURLTypes` 与 `com.apple.developer.associated-domains`。关联文件按真实 JSON 校验。返回 `files`、`android`、`apple` 与 `summary`；问题包含 `code`、`severity`、`file`、`location`（有行号时为 `file:line`）和修复建议。缺少平台文件表示未配置，不是错误；空项目返回 `No deep links configured.`。该工具只校验本地配置，不访问线上 `/.well-known`，也不校验包名、签名或运行时 URL 投递。

## 宿主

桌面、Android 和 Web 宿主使用 `RuntimeProbe`。原生探针由后台线程连接控制器，命令在所属窗口的 UI 线程执行。WASM 由页面传输相同消息。协议见 [DevTools 协议](../devtools-protocol/README.md)。

```sh
cargo test -p cove-devtools-protocol -p cove-devtools -p cove-devtools-cli
```
