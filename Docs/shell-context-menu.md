# 系统外壳文件右键菜单集成

更新：2026-11-15。面向在 Cove 之上构建解压/归档类应用的场景：把应用命令注入**操作系统外壳自己的文件右键菜单**（macOS 访达、Windows 文件资源管理器），而不是应用内部自绘的右键菜单。

> 区分：本文不讨论应用内的 `ContextMenu`（自己窗口里的文件行菜单）。那套原语见
> [`context_menu.rs`](../crates/compose/src/primitives/overlays/context_menu.rs)。本文讲的是 OS 外壳集成，落在 host/shell 原生层与打包清单/安装器，Compose 界面层不提供这类能力。

## 目标与边界

- 目标：在访达/资源管理器里右键文件或文件夹时，出现本应用的命令（如「解压到此处」「用本应用打开」）。
- 边界：macOS 与 Windows 的机制完全不同；macOS 没有 Windows 式「任意插一条顶层动词」的通用扩展点。
- 现状基线：框架内**不存在**任何 shell 扩展、文档类型、服务或注册表动词代码（全仓检索无 `IContextMenu`/`IExplorerCommand`/`NSServices`/`CFBundleDocumentTypes`/注册表写入）。Swift 参考版同样没有，属于新能力。

## 已存在的接收侧链路（可复用）

无论命令从哪个系统入口进来，最终都要把选中的文件路径交给应用。这条「被带文件启动 / 被带文件唤起」的接收通道已经打通：

| 环节 | 位置 | 行为 |
| --- | --- | --- |
| 原生入队（macOS/iOS） | [`apple.m`](../crates/shell/native/apple.m) `cove_open_documents_push`；`kAEOpenDocuments` / `kAEGetURL` 处理器 | 平台线程把路径/URL 推入线程安全收件箱并唤醒事件循环 |
| shell 排空 | [`open_documents.rs`](../crates/shell/src/open_documents.rs) `drain()` | macos/ios 走 `apple::drain()`；其余走 `argv::drain()` 取非 flag 启动参数 |
| host 投递 | [`runtime.rs`](../crates/hosts/src/desktop/application/runtime.rs) `deliver_open_documents`（tick 内调用） | 排空收件箱，交给核心 `OpenedDocuments::deliver` |
| 应用回调 | [`platform_input_context.rs`](../crates/core/src/scene/input/platform_input_context.rs) `OpenedDocuments::set_handler` | 回调签名 `Rc<dyn Fn(Vec<String>)>`；未装处理器时路径先入队，装处理器后回放 |

应用侧接入示例（路径以核心导出为准）：

```rust
cove_core::scene::input::platform_input_context::OpenedDocuments::set_handler(Some(Rc::new(
    |paths: Vec<String>| { /* 打开/解压这些归档 */ },
)));
```

Windows/Linux 由 `argv::drain()` 取启动参数；一个被系统动词调起的命令 `app.exe "<file>"`，文件路径即 `argv[1]`，会进入同一通道。

## macOS

第三方应用在访达右键菜单里可见，只有三条正规路径。

### 1. 「打开方式」文档关联（纯清单声明）

在 `Platforms/macOS/Info.plist` 声明 `CFBundleDocumentTypes`，把归档扩展名/UTI 关联到本应用，应用即出现在该类文件的「打开方式」子菜单。

| 键 | 取值说明 |
| --- | --- |
| `CFBundleTypeName` | 类型名，如 `Archive` |
| `CFBundleTypeRole` | `Viewer` / `Editor`；解压应用一般为 `Viewer` |
| `LSHandlerRank` | `Owner` / `Default` / `Alternate` / `None`；不抢占系统默认时用 `Alternate` |
| `LSItemContentTypes` | 系统/自定义 UTI 数组，如 `public.zip-archive`、`public.tar-archive`、`org.gnu.gzip-archive` |
| `CFBundleTypeExtensions` | 无内置 UTI 的格式（`.7z`、`.rar` 等）按扩展名补充 |

### 2. 服务（右键 → 服务 子菜单）

在 `Info.plist` 声明 `NSServices` 数组；用户在访达选中文件后，右键「服务」里出现本应用命令。

| 键 | 取值说明 |
| --- | --- |
| `NSMenuItem` | 字典，`default` 为服务项标题（如「解压到此处」） |
| `NSMessage` | 实例方法名，如 `coveExtractService:` |
| `NSSendTypes` | 服务可读取的类型，文件场景用 `public.file-url` |
| `NSRequiredContext` | 可选，限定适用上下文（如 `NSTextContent = public.content`） |
| `NSUserData` | 可选的服务侧字符串透传 |

回调签名（AppKit 按 `NSMessage` 拼出 `…:userData:error:` 形式）：

```objc
- (void)coveExtractService:(NSPasteboard *)pboard
                  userData:(NSString *)userData
                     error:(NSString **)error;
```

实现里从 `pboard` 读 `public.file-url` 项，解析出本地路径，推入现有 `cove_open_documents_push` 收件箱，复用上面的接收链路。

**现存阻塞点**：服务消息由 AppKit 发送给应用代理（app delegate）。当前应用代理归 SDL 持有（`SDL3AppDelegate`，`third-party/sdl3/src/video/cocoa/SDL_cocoaevents.m`），`cove-shell` 没有自定义代理钩子。要接收服务回调，需要在 shell 层做代理转发（保留 SDL 代理处理生命周期/激活，服务 selector 转发到 shell 目标），或以服务提供者方式注册目标。这一步是原生改动，需打包成 `.app` 后在访达「服务」菜单真机复核。

### 3. Action Extension（现代顶层项）

希望命令直接出现在访达右键顶层（而非「服务」子菜单），现代 macOS 的做法是独立的 **Action Extension**：独立 target，`NSExtension` + `NSExtensionPrincipalClass`，实现 `NSExtensionRequestHandling`。它运行在扩展进程内，不属于本壳进程内能力，需要单独的扩展工程与打包配置。

### 打包注意

macOS 清单由外部打包器 `cove-package`（git 依赖，tag `v0.3.0`，源码不在本仓库）拷进 `.app`。本仓库的 `Platforms/macOS/Info.plist` 是清单源；模板到产物的键保留/合并方式需在实际打包产物中核实，不能假设模板原样保留。

## Windows

### 1. 静态动词（注册表，无 COM）

简单命令（「解压到此处」）由安装期写注册表完成，被调起时以命令行带文件路径启动应用。

| 注册表位置 | 适用对象 |
| --- | --- |
| `HKCR\*\shell\<verb>` | 所有文件 |
| `HKCR\Directory\shell\<verb>` | 文件夹 |
| `HKCR\Directory\Background\shell\<verb>` | 目录空白处（可选） |
| 扩展名/ProgID 下 `…\shell\<verb>` | 特定类型 |

动词子键值：`MUIVerb`（显示名）、`Icon`（图标，`exe,0`）、`Position`（可选）；其下 `command` 子键默认值为命令行，如 `"C:\…\app.exe" "%1"`。卸载时清理这些键。文件路径经 `argv` 通道进入现有 opened-documents 链路。

### 2. 动态/复杂菜单（COM 处理器）

需要按文件类型动态显隐、勾选态、子菜单或自定义绘制时，实现 COM shell 扩展：旧版 `IContextMenu`，或 Win11 现代 `IExplorerCommand`；CLSID 注册到 `HKCR\*\shellex\ContextMenuHandlers`。这是独立的 COM 服务器，后续单列任务。

### 现存阻塞点

仓库当前**无安装器框架**（无 NSIS/Wix/`.reg`，也无注册表写入代码）。静态动词的写入/卸载需要先确定安装与卸载触发点（安装器脚本，或应用首次运行时自注册）。

## 职责分层现状

| 关注点 | 归属层 | 现状 |
| --- | --- | --- |
| 应用内文件列表、行操作菜单 | Compose `ContextMenu` | 已有 |
| 被带文件启动 / 唤起后的路径接收 | shell / host / core | 已有 |
| macOS 文档类型、服务清单声明 | 打包清单 `Info.plist` | 缺 |
| macOS 服务原生回调目标 | `cove-shell` Apple 层 | 缺（代理钩子阻塞） |
| Windows 静态动词注册/卸载 | 安装器 / host 注册表层 | 缺（无安装器框架） |
| Windows COM 上下文菜单处理器 | 独立 COM 服务器 | 缺 |

## 待办与验收

- 模板 `Info.plist` 补注释化的 `CFBundleDocumentTypes` / `NSServices` 参考块，供生成的归档应用拷贝填写；通用模板的生效段不硬编码具体压缩包类型。
- macOS：shell 层服务目标接入 opened-documents 通道，解决 SDL 代理持有问题。**原生可见性**（访达「服务」菜单出现该项、选中文件后回调到应用）须打包成 `.app` 在真机复核；DevTools 与结构化测试不覆盖外壳菜单可见性。
- Windows：静态动词注册表写入/卸载层；装机后在资源管理器右键复核动词出现与带参启动。
- 验收原则：键位构造、注册表路径、pasteboard/参数解析可用结构化测试覆盖；系统外壳里的实际可见性与交互属原生验收，单列记录，不与结构测试混同。

## 参考

- Apple：[NSServices](https://developer.apple.com/documentation/bundleresources/information-property-list/nsservices)、[Add functionality to Finder with action extensions](https://developer.apple.com/documentation/appkit/add-functionality-to-finder-with-action-extensions)
- Windows：Shell verbs（`HKCR\*\shell`、`Directory\shell`）、`IExplorerCommand` / `IContextMenu` COM 上下文菜单处理器
