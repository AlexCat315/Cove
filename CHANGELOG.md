# Changelog

GuavaUI SDK 与 `guava` CLI 的发布记录。每个版本对应 git 标签 `vX.Y.Z`；版本号写在 `Tools/Guava/Sources/GuavaCLI/SDK.json`，并与 `Artifacts/native.json`、各应用的 `toolchain.sdkVersion` 保持一致。

## Unreleased

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
