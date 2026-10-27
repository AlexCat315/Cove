# Changelog

每个 Cove SDK 版本对应一个 `vX.Y.Z` 标签，统一包含 CLI、框架源码、模板与各目标平台的原生预编译库。

## Unreleased

## 0.2.8 — 2026-10-27

- 增加 `Device.prepareResources` 和四类资源的异步创建 API，提供阶段/步骤/编译进度、Task 取消和失败清理；Metal、Vulkan、DX12 在昂贵编译期间释放设备资源锁，已预热的 UI 可继续渲染。
- 着色器按内容共享并计数引用，取消或失败只释放本次持有的资源；并发创建同一管线时回收重复对象，保护编译期间的着色器输入生命周期。
- 内置 UI 打包 macOS、iOS 与模拟器的预编译 Metal 库，按源指纹失效回退；新增跨平台着色器产物生成流程。GPU 管线仍需在目标设备上创建，本版本未提供跨进程驱动缓存。
- 包含未成功发布的 0.2.7 的全部内存与效果修复：跨后端 16 MiB MSAA 工作区、磨砂前缀重绘、同通道揭幕、Vulkan 临时附件与空块释放、DX12 分配报告和离线 UI DXIL。
- 修复 DX12 多采样颜色纹理的无效资源标记，以及发布说明中的旧 CI 跳过标记阻断后续版本工作流的问题；支持手动恢复不可变标签的发布。

## 0.2.7 — 2026-10-27

- 统一窗口渲染的 MSAA 生命周期：磨砂重绘已解析的前缀，揭幕在最终通道合成，跨通道只保留单采样图像；取消效果帧保留整屏多采样附件的路径。
- 为需要显存的 Metal、Vulkan 和 DX12 后端提供 16 MiB MSAA 工作区预算，通过保持原始视口与样本位置的分条渲染限制大窗口分配；支持真正 memoryless 的硬件继续使用片上附件。
- Vulkan 临时附件使用 TRANSIENT_ATTACHMENT 并优先选择 lazy 内存；空内存块在资源退役后释放。DX12 增加纹理分配报告。
- 修复安装版 Windows 测试诊断脚本查找 CLI 的路径。
- 补齐 UI 与磨砂/揭幕的 DXIL 离线产物及固定 Slang 工具链生成入口；修正着色器编译与缓存生命周期文档。
- 磨砂捕获按实际尺寸复用纹理，修复较小的后续面板沿用较大纹理时的采样范围错误。

## 0.2.6 — 2026-10-27

- 保持默认 4x MSAA，普通窗口在支持的 Apple GPU 上改用临时附件；磨砂与揭幕自动保留持久样本。固定场景的输出逐字节一致，M1 的 4K 附件分配由 129.5 MiB 降至 0 MiB 持久 backing，2x 场景 GPU 耗时下降约 32%；DevTools 分开显示逻辑容量和后端 MSAA 分配，其他后端保留安全回退。

- iOS 构建由 destination 选择应用 SDK，避免全局 `-sdk` 把宿主宏插件误编译为模拟器程序；补齐字段追踪对延迟绘制、布局回调和未读取状态失效信号的保守处理。

## 0.2.5 — 2026-10-27

- 新增 `@TrackStateReads` 视图宏，自动推导普通值类型 `@State` 的字段读取依赖；按实际读取字段刷新，重建时替换依赖，整值读取、引用与不透明值保守失效，并在 DevTools 中记录字段名称。
- Metal 接入可选的命令缓冲 GPU 时间戳，完成 1x 解析覆盖率与 4x MSAA 的固定场景评估：M1 上 1x 降低 UI GPU 耗时，但 4x 显著改善自定义三角形边缘，因此桌面默认继续保留 4x；报告包含像素误差、目标内存与截图，测量入口只在显式启用时执行。

## 0.2.4 — 2026-10-27

- 修复 Windows 安装器读取 GitHub 校验清单的方式：下载后明确按 UTF-8 文本逐行解析，兼容二进制响应类型，并避免覆盖 PowerShell 自动变量。
- 包含 Cove 0.2 系列的完整改名、子树复用、抗锯齿、mip／LOD、跨平台内存压力和 DevTools 修复。

## 0.2.3 — 2026-10-27

- 完整发布 Cove 0.2 系列 SDK：包含改名、子树复用、覆盖率抗锯齿、mip／LOD、内存压力与 DevTools 接线修复。
- 修复解析抗锯齿在 Android 上的浮点数学重载与类型推导，采用 Swift 原生开方和取整，并在整数转换前限制细分数量。

## 0.2.2 — 2026-10-27

- 完整发布 Cove 改名后的 SDK，包含 0.2 系列的子树复用、抗锯齿、mip／LOD、内存压力和 DevTools 修复。
- Android 发布明确选择已安装的 NDK r30，避免 runner 继承的旧 NDK 环境变量使原生库打包失败。
- 发行脚本等待真实的 GitHub workflow ID，不再把空列表显示为 `null`。

## 0.2.1 — 2026-10-27

- 首次发布完整 Cove SDK：同步模块、CLI、清单、安装目录与发布包名称，包含值状态子树复用、覆盖率抗锯齿、mip／LOD 和跨平台内存压力修复。
- 修复 Linux／Android 原生构建中 SVG、WebP 静态依赖的链接顺序，CLI 自动配置所需选项。
- 浏览器 DevTools 与内置 Agent 共用内存数据源，Android 提供文本／图片缓存统计；断开连接后清除旧快照。
- 深层 UI 交互回归在应用主 Actor 上执行，避免 macOS CI 后台工作线程栈溢出；改名后的 CLI 使用物理路径，防止旧路径链接产生重复编译缓存。

## 0.2.0 — 2026-10-27

- 项目迁移为 Cove：框架模块采用 `Cove*`，CLI 为 `cove`，清单与本地目录采用 `cove.json`／`.cove`；安装器、发布包、Android 标识与仓库链接同步改名。此版本需要更新旧项目的导入与配置。

- `@State` 父视图更新时自动跳过值输入相等的子 scope，保留原状态和回调；闭包、可变引用及不透明输入保守重组，环境变化仍强制刷新。
- 补齐描边、路径、圆环与圆角遮罩的覆盖率抗锯齿；`geometryAntialiasing = .analytic` 可按窗口使用 1x 目标，默认仍保留 4x MSAA。离屏 HUD 与 Android 使用解析遮罩。
- 图像生成带 alpha 权重的 mip 金字塔，计入图片缓存预算，并按实际显示像素选择 LOD；GPU 图像可携带由生产者生成的独立分辨率层。
- 增加 Windows 低内存通知、Linux 主机／cgroup 压力监听和 Android `onTrimMemory` 清理；DevTools 提供 `query_ui_memory` 与性能页内存估算。
- 显式本地 SDK 覆盖可跨版本调试；SDK 包补齐 Inspector 客户端及资源文档。Android 发布不再请求已下架的 `tools` 包。

## 0.1.14 — 2026-10-26

- 修复发布引导：源码仓库的旧原生库清单不再阻止新版本从源码构建；已安装 SDK 缺少库时明确报错，避免回退到不存在的依赖源码。

- 窗口隐藏、最小化或被遮挡时停止呈现，等待在途工作完成后释放该窗口的 swapchain、MSAA 与 backdrop 目标；恢复时按最新尺寸和 VSync 设置重建，辅助窗口互不影响。
- 重组性能统计不再触发重画；接入 Swift Observation，按实际读取属性的视图失效，父视图不会因子视图的读取而订阅同一属性。
- 文本 shaping/layout 缓存增加 LRU 淘汰、条目数与估算字节双限；提供应用内存配置、窗口资源快照与主动清理，接入 macOS 内存压力及 iOS 内存警告。
- 主窗口与辅助窗口可分别覆盖采样数和清屏色；未覆盖时继承应用配置，默认画质保持不变。
- 多窗口分别持有 reveal/history 与 blur 临时状态，共享字体、图片驻留和已预热管线；隐藏窗口同时释放特效缓冲，避免窗口之间的呈现状态相互覆盖。
- 离屏视图在内容、尺寸和像素密度不变时复用几何快照；图集补丁只发送一次，主动清理后可重新生成。

## 0.1.13 — 2026-10-26

- SDK CLI 增加 `guava task` 和 `guava.workspace.json`：项目提供 Swift 任务库，SDK 负责依赖排序、按需构建、参数传递及构建前钩子，不需要项目包装脚本或独立 CLI。
- 工作区可声明 `dependencyUpdates: "branches"`，本地 build/run 自动检查并刷新分支依赖；CI 保持锁文件，离线检查保留已解析版本。

## 0.1.12 — 2026-10-26

- 安装包包含完整 SDK 与主机原生库，按版本安装到全局 `versions/VERSION`；项目、示例和 IDE 引用同一份源码与库，不再回退到各项目单独检出 GuavaUI。
- CLI 自动调用项目对应 SDK 的工具；安装 SDK 不再改写项目版本，`sdk use` 负责选择版本。
- Guava 构建、测试与清理使用 SwiftPM 标准 `.build`，移除 `.guava/build/swift` 这套独立编译缓存。
- 将原生构建与 SDK 发布收拢为一个工作流、一个 Release；原生库按目标拆包并由 SDK 自动准备，覆盖桌面、Android ARM64、iOS 真机和 ARM64 模拟器。
- Android 构建自动安装匹配且校验过的 Swift Android SDK；平台工具由 setup/doctor 检查。
- 并发项目通过系统文件锁复用同一份 SDK 安装与原生库准备；修复旧安装记录路径末尾斜杠导致升级被误判为其他安装的问题。

## 0.1.11 — 2026-10-26

- `guava.json` 的 `toolchain.sdkVersion` 支持写 `latest`：构建时浮动到本地已安装的最高版本 SDK（离线可用，不触发网络），用 `guava sdk install latest` 拉取新发布并刷新；`guava sdk install latest` 在精确钉版项目里仍会把解析后的具体版本写回清单（npm 语义），无 `GUAVA_SDK_ROOT` 的 SwiftPM 回退路径改用 `from: 0.0.0` 解析最高发布标签，Android Gradle 回退同样支持 latest。
- `guava sdk install latest` 解析 GitHub Latest 指针时去掉仓库地址末尾的 `.git`，此前拼接出 `.git/releases/latest` 会 404。
- `guava self update` 不再调用 `api.github.com`（未认证每小时 60 次配额，超限即失败），改为跟随 `github.com/repos/latest` 的重定向解析最新版本；下载类错误现在打印完整的 HTTP 状态而不是通用的 `ToolError` 提示。
- `guava setup --ios` / `--android` 显式检查 CMake 与 Ninja（移动端 C 依赖从源码交叉编译）；移除 iOS 提示中并不存在的 Rust 工具链要求。
- `Tools/Release/release.sh` 推送后只确认 release workflow 已触发并输出 run 链接，不再轮询等待 CI 结果；移除 `--no-ci`、`--ci-reruns`、`--ci-timeout` 选项。

## 0.1.10 — 2026-10-26

## 0.1.9 — 2026-10-26

- DevTools Inspector 自动打开时先打开会话目录内的 `inspector.html` 跳转页：`open`、`xdg-open` 与 `rundll32` 会丢弃 `file://` URL 的查询参数，客户端此前只能回退到默认 9229 端口并停留在 Disconnected；跳转页在浏览器内完成导航，动态端口与一次性 token 完整保留，打开即自动连接。`guava devtools` 附加到现存会话时同样生成该跳转页。

## 0.1.8 — 2026-10-26

- Windows 子进程以反斜杠路径启动，参数只在需要时加引号：`cmd.exe` 会扫描整条命令行，曾把 `C:/…/cmd.exe` 里的 `/cmd.exe` 当作 `/c` 开关，现在能执行命令并返回真实退出码。
- 文件与管道错误改用 `NSPOSIXErrorDomain`：Android 的 Foundation 没有 `POSIXErrorCode`，`System` 包恢复在 Android 上编译。

### 触摸与移动端

- `NavigationStack` 在触摸表面支持从前缘拖动返回：顶层页面跟随手指移开，下层页面以视差露出；松手时超过宽度三分之一或快速轻扫则返回，否则回弹。从右到左布局从右缘开始。
- `Dialog` 在手机上改为底部弹层：带拖动柄、贴合内容高度、避开 Home 指示条与键盘，去掉关闭按钮和分隔线；`AlertDialog` 仍居中显示。
- `HoverCard` 在无法悬停的触摸表面改为点按标签打开底部弹层，内容不再只能靠悬停到达。
- 标签栏的“更多标签”与列可见性菜单在触摸表面改用底部弹层，与 `Popover`、上下文菜单共用 `AdaptiveOverlayHost`。
- `SectionedList` 在触摸表面点按行即激活（与 `List` 一致），分组标题与行内容对齐并贴近所属行。
- 修复减弱动态效果时 `NavigationStack` 推入页面后无法再返回：没有过渡动画时不再等待过渡结束。
- `Examples/Mobile` 的根视图放入 `LayerRoot`，底部弹层、对话框与菜单在移动端可以正常呈现；项目详情的“分享”改为打开对话框。

## 0.1.7 — 2026-10-25

- 新增可独立构建的 `System` 包，提供 `GuavaFileSystem` 与 `GuavaProcess`，不依赖 SDL、窗口或渲染器；桌面与移动端共享文件、路径、权限语义，不支持的进程能力明确报错。
- 文件写入统一使用同目录暂存与替换，失败时保留旧内容；Windows 仅对临时共享锁进行有限重试，私密文件使用当前用户 ACL。
- 进程服务统一流式读取、输出限额、超时与取消。超时覆盖子进程退出后的管道排空，Windows 使用 Job Object 回收后代，读取句柄由读取线程关闭。
- CLI、应用存储、DevTools 与技能目录使用共享系统能力，移除重复的 Windows 文件写入与进程读取实现。
- 下载文件先复制到目标目录暂存，再校验和发布，避免 CI 临时目录与工作区跨卷时替换失败；下载失败保留原文件。
- `guava test` 支持仅构建、跳过构建、分开运行 XCTest/Swift Testing、测试事件文件与总超时；CI 增加系统契约测试、构建缓存和 Windows 超时进程转储。
- 上下文菜单保留编辑器的弹性高度，触摸滑动列表与长按菜单协作处理指针捕获，允许垂直滚动接管手势。

## 0.1.6 — 2026-10-25

## 0.1.5 — 2026-10-25

### 触摸与移动端

- 菜单在触摸表面改用独立呈现：`MenuPresentationHost` 决定菜单怎样呈现，`TouchMenu` 负责触摸下的导航与项激活，`TouchOverlay` 承担模态呈现；`Popover` 在触摸输入下改走 `TouchOverlay`，不再套用指针式弹出层。
- 新增 `LongPress` 手势与 `MobileBar` 标签栏。标签栏去掉顶部的分隔线，由自身表面标出边缘，背景延伸到 Home 指示条下方。
- 列表行区分指针与触摸：触摸行铺满容器宽度、最小高度 44pt；`ListRowStyle` 随之调整内边距与选中样式。
- 按钮按触摸输入调整尺寸与内边距，触摸表面上的拖动柄按 `minimumControlExtent` 取宽度。

### Android 与表面度量

- `UIEnvironment` 新增 `idiom`、`textScale` 与 `keyboardHeight`，与安全区分开；新增 `TextScale` 读取系统辅助功能中的文字缩放。
- `AndroidApplication` 新增 `updateScale` 与 `surfaceMetrics`，`SurfaceState` 统一持有逻辑尺寸、密度、惯用语、安全区、键盘高度与系统栏图标样式；`AndroidContentSession` 改用 `SurfaceState`。
- Android 的 `windowSoftInputMode` 改为 `adjustResize`，键盘弹出时内容按 `keyboardHeight` 收缩，模板与示例同步更新。

### 控件与表面

- 边框改用半透明墨色（`border`、`borderStrong`、`divider`），不再使用不透明灰；分隔线改为 0.5pt 发丝线，用来确认已看到的边界，而不是画出实线。浅色与深色主题都按此调整。
- `.secondary` 按钮、`StatusButtonStyle` 与开关改用 `onSurface` 的半透明墨色填充，不再叠加表面色与描边；开关滑块在开关两态下都是白色圆片，在浮起表面上不再与卡片同色而消失。
- 新增 `ListGroup`：可带标题与脚注，行位于同一圆角表面上，分隔线从行内容处起始，触摸表面使用更大的圆角。
- 侧栏可折叠：`AdaptiveSidebar` 新增 `isCollapsed` 绑定与 `collapseLabel` / `expandLabel`。宽度足够时仍可折成 72pt 轨道，被窗口强制成轨道时按钮改为展开抽屉，两种情形不再混淆。
- 侧栏拖动柄静止时不绘制，只在悬停或聚焦时出现，边界交给表面差异表达。
- 按钮的 `tooltip` 只重复可见标题时不再弹出，文本仍作为辅助技术的帮助；`AccessibilityTree.visibleText` 提供控件实际显示的文本。
- 示例库的页面改为窗口表面上的单块软面板（`surfaceSunken` 加圆角），去掉头尾与工具栏之间的分隔线。

### 工具与示例

- 新增 `GUAVA_PREVIEW_DEVICE=phone`：桌面构建以 390×844 窗口、触摸输入与刘海屏安全区运行，无需模拟器即可检查和调整移动端布局。
- 新增 `Tools/Release/release.sh`：升版本、提交、打标签、推送并等待发布工作流；网络步骤带退避重试，失败的工作流先重跑失败任务。
- 新增 `Examples/Mobile` 示例，共享业务代码位于 `Sources/AppContent`，覆盖 iOS、Android 与 macOS 目标。

## 0.1.4 — 2026-10-25

- 图形管线改用 `PipelineStore` 缓存：图形、计算与网格管线按描述复用，`Device` 通过它创建与销毁管线，并统计创建耗时与缓存命中。`NativeDrawListRenderer` 在配置变化后保留管线状态。
- 修复 Windows 上的原子安装写入：替换文件时按类型选择替换标志，`guava self update` 不再留下半写的可执行文件。
- CLI 子进程的标准错误改用专用读取器排空至 EOF，安装器与终端不再因管道未读完而卡住。
- 跨平台安装器修复后重新发布 SDK 与 CLI。

## 0.1.3 — 2026-10-24

- 图表支持堆叠与缩放，新增 `ChartOrientation`；视口可拖动与刷选，数据变化使用可中断的过渡动画。
- 修复 Android 的数学库与 SDL 链接，修正 Vulkan 动态描述符布局。
- 保留层滚动时，画家局部的裁剪保持与内容对齐。
- 恢复 DevTools 的独立依赖，并校验开发者客户端可独立启动。

## 0.1.2 — 2026-10-24

- 图表新增缩放与堆叠，补齐对应测试。
- 修复 CLI 在 Windows 上查找工具链的问题。

## 0.1.1 — 2026-10-24

- 修复 Windows 上读取 `PATH` 时区分大小写的问题，CLI 能正确找到已安装的工具。

## 0.1.0 — 2026-10-24

- 首个发布：预编译 `guava` CLI 与运行库，提供 macOS arm64、Windows x86_64 与 Linux x86_64 三个平台。
- `install.sh` 与 `install.ps1` 安装器写入用户 `PATH`；CLI 自带运行库，`guava --version` 不需要先装 Swift。
- 提供 `guava create`、`run`、`build`、`test`、`clean`、`devtools`、`sdk install` 与 `self update`。
