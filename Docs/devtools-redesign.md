# Cove DevTools 重写

旧 DevTools 从 DrawList 重绘画面，等待 GPU 回读并编码 JPEG。重绘看不到系统合成的透明材质和原生浮层。旧输入经应用主窗口分发，浏览器还有独立协议和帧内检查树。原有问题的验收记录见 [Gallery 审计](gallery-audit.md)。

## 运行结构

| 层 | 职责 |
| --- | --- |
| 控制器 | 持有目标、会话、命令和短期 PNG 资源。Inspector、MCP 和 CLI 都调用这一个服务。 |
| Cove 探针 | 在所属 UI 线程读取组件树、布局、源码、状态和 CPU 数据，应用调试样式和环境。桌面、Android 与 Web 使用同一个 Rust 执行器。 |
| 平台适配器 | 从系统合成器、设备会话或浏览器取像，执行系统键鼠、触摸和原生无障碍操作。 |

每个注册的 Cove 运行时获得独立目标 ID。桌面用进程与系统窗口身份自动关联，浏览器和移动设备通过配置的 WebDriver 会话关联。操作系统发现的窗口可以直接选择，例如 Windows 文件面板。Wayland 的屏幕与输入共享同一个 portal 会话，Cove 组件继续从对应运行时读取。

控制器和客户端使用 JSON-RPC。探针主动连接，带租约和序号轮询类型化命令。截图是带采集来源、像素尺寸和输入坐标范围的 PNG 资源。资源和命令有大小上限。已交付且结果未知的操作由调用者重新观察状态。

## 平台路径

| 平台 | 画面与操作 |
| --- | --- |
| macOS | ScreenCaptureKit 读取实际显示区域。Accessibility 查询与操作原生元素，Quartz 发送键鼠。 |
| Windows | Windows Graphics Capture 读取显示器。UI Automation 查询和激活控件，系统键鼠按目标窗口与坐标派送。 |
| Linux / X11 | X11 显示画面和 XTest 输入，AT-SPI 查原生控件。 |
| Linux / Wayland | XDG RemoteDesktop 与 ScreenCast 选定一个显示器，PipeWire 提供画面。绝对坐标来自同一流的逻辑尺寸，AT-SPI 查询应用级控件。 |
| iOS / iPadOS | Appium XCUITest / WebDriverAgent 提供设备截图、触摸和原生元素。 |
| Android | Appium UiAutomator2 提供设备截图、触摸和原生元素。ADB 反向转发供调试探针连接。 |
| Web | 标准 WebDriver 提供页面视口、文档元素和浏览器输入。Cove 探针提供 canvas 内的组件。 |

## 画面与证据

桌面以显示器合成画面为源，可裁剪到窗口范围或保留整个显示器。遮挡、透明区域下的内容及显示器上的原生浮层来自同一份画面。点击、拖动和滚动带截图 ID。控制器复核目标、坐标范围及能观察到的窗口或浏览器上下文，发送输入后由调用者验证实际结果。

树和截图按需读取。帧样本区分 CPU 布局、绘制、提交以及可观察的渲染线程结果。GPU 执行与物理显示刷新需要系统性能工具的证据。可按起止操作记录 CPU trace，再分页读取事件或比较阶段。

浏览器页面截图只覆盖视口。物理触控板、系统软键盘和 IME 候选需要在运行浏览器的设备上观察。覆盖按平台、窗口类型与动作分别登记，见 [Gallery 审计](gallery-audit.md)。启动及命令见 [Cove DevTools](../crates/devtools/README.md)。
