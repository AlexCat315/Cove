# Cove DevTools

Inspector、MCP 和 CLI 共用一个 Rust 控制服务。原生宿主与 WASM 应用通过同一个 Rust 探针提供 Cove 组件、源码、日志、样式和 CPU 时间线。截图、鼠标键盘、触摸和原生控件操作由平台适配器执行。

## 启动

先启动控制器，再在另一个终端启动 Gallery。

```sh
cargo run -p cove-devtools-cli -- serve
COVE_DEVTOOLS=1 cargo run -p cove-examples --bin gallery
```

PowerShell 使用 `$env:COVE_DEVTOOLS = '1'` 设置环境变量。控制器默认监听 `127.0.0.1:9229`，终端打印 Inspector 链接。链接内含配对凭据，仅供当前开发会话使用。控制器把端点和凭据写入当前用户的本地配置目录。用 `COVE_DEVTOOLS_SESSION_FILE` 可指定发现文件，用 `COVE_DEVTOOLS_ENDPOINT` 和 `COVE_DEVTOOLS_TOKEN` 可显式指定配对。

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
| Windows | Windows Graphics Capture 读取系统合成画面，UI Automation 查原生控件。系统键鼠受前台窗口、完整性级别和安全桌面约束。 |
| macOS | ScreenCaptureKit 读取合成画面，Accessibility API 查原生控件，Quartz 发送键鼠事件。在系统设置中给运行控制器的终端或应用授予屏幕录制和辅助功能权限。 |
| Linux / X11 | 从 X11 读取显示器画面并发送 XTest 输入，应用控件通过 AT-SPI 查询。 |
| Linux / Wayland | 选择 Wayland shared screen，首次截图会请求 RemoteDesktop 与 ScreenCast 授权。PipeWire 画面与绝对坐标输入使用同一个流。绑定 Cove 运行时可继续看组件，AT-SPI 按应用查原生控件。发送键盘前先在共享显示器上聚焦目标。 |
| Web | 用 `serve --web-root crates/browser/dist` 提供 WASM 页面。将 Inspector 链接的路径改为 `/app/` 并保留片段，打开即可注册探针。为页面截图和浏览器输入配置 WebDriver。 |
| iOS / iPadOS | 使用 Appium XCUITest / WebDriverAgent 会话。通过 Xcode 或 Appium 启动开发版，设置 `COVE_DEVTOOLS=1`。模拟器可使用配套 Mac 的本地控制器，真机需通过隧道让设备侧的本地端口连到控制器。 |
| Android | 使用 Appium UiAutomator2 会话。Debug APK 可用 `adb reverse tcp:9229 tcp:9229` 转发探针连接。启动 `com.cove.probe.MainActivity` 时将含 `endpoint`、`token` 和可选 `appTitle` 的 UTF-8 JSON 做 Base64 编码，传给字符串 extra `cove_devtools`。 |

Linux 构建控制器需要 PipeWire、XCB、Clang、D-Bus、EGL 与 GBM 开发依赖。Debian/Ubuntu 包名见仓库 CI 配置。

配置已有 WebDriver 会话时，启动控制器传入 `--webdriver web,http://127.0.0.1:9515,SESSION_ID`。移动平台用 `ios` 或 `android`。控制器接入会话，设备启动、签名与会话创建由对应驱动负责。远端驱动通过本地端口隧道接入。在 Inspector 或 `bind_ui_target` 中关联运行时与对应会话。桌面探针有匹配的系统窗口标识时自动关联。

## 操作与验收

1. 用 `get_ui_session` 取得目标 ID。系统文件面板出现后，重新查询并选择面板目标。
2. `capture_ui` 返回 PNG 资源、像素宽高、坐标范围和来源。桌面默认裁剪到窗口范围，`scope: "display"` 取窗口所在的显示器。桌面合成画面会保留实际遮挡。浏览器截图范围为页面视口。
3. 用 `click_ui` 或 `input_ui` 传入截图 ID 和 `coordinates` 坐标。图片内点按宽高比例换算，并加上坐标原点。拖动和滚动同样使用截图坐标。窗口、显示器或浏览器上下文变化时重新截图。
4. 用 `wait_for_ui` 检查 Cove 节点是否可见、隐藏、启用或取得预期值。值断言需要 `node_id` 与 `expected`。用 `query_native_ui` 或 `find_native_ui` 查原生控件，`activate_ui` 执行其系统动作。操作后重新查询控件、目标或画面，确认结果。

`query_ui_nodes` 可筛选角色、文字和源文件，默认返回紧凑信息。单个字段可用 `node_id` 和 `include_value: true` 读取，安全字段保留脱敏。完整组件树在 Inspector 的 Components 中查看。

`press_ui_key` 发送按键及修饰键，`type_ui_text` 使用平台的文本输入能力。输入回执表示交付。IME 候选、软键盘、物理触控板及系统拖入需要记录实际设备、输入路径与后置条件。浏览器视口外的系统界面可以选择浏览器所在桌面的显示器目标。

`start_ui_trace` 与 `stop_ui_trace` 可包围实际操作，`get_ui_trace` 分页读取，`compare_ui_traces` 比较 CPU 阶段。帧计时区分 CPU 提交、渲染线程完成和宿主能观察到的呈现操作。GPU 执行和屏幕扫描输出没有在这些 CPU 样本中计量。

`start_ui_profile hz=1000 duration_ms=5000` 用 `cove-profiling` 的真实采样器跟随 UI 线程，`stop_ui_profile` 停止、join 并在后台线程完成符号化，`get_ui_profile` 返回有界聚合调用栈（不逐样本返回），`query_ui_hotspots` 按函数和 `file:line` 返回 self/inclusive 热点。单次采样上限 10 秒；self 时间是独占时间，inclusive 时间在同一采样内对每帧只计一次，时间按相邻采样的实际间隔累计。仅 macOS 提供真实采样；其他平台返回 `unavailable` 及原因，不合成样本。

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
