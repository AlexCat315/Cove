# 系统外壳文件右键菜单集成

更新：2026-11-15。面向在 Cove 之上构建解压/归档类应用的场景：把应用命令注入**操作系统外壳自己的文件右键菜单**（macOS 访达、Windows 文件资源管理器、Linux 各文件管理器），而不是应用内部自绘的右键菜单。

> 区分：本文不讨论应用内的 `ContextMenu`（自己窗口里的文件行菜单）。那套原语见
> [`context_menu.rs`](../crates/compose/src/primitives/overlays/context_menu.rs)。本文讲的是 OS 外壳集成，落在 host/shell 原生层与打包清单/安装器，Compose 界面层不提供这类能力。

## 目标与边界

- 目标：在访达/资源管理器/文件管理器里右键文件或文件夹时，出现本应用的命令（如「解压到此处」「用本应用打开」）。
- 三个平台的机制互不相同，**没有可用的统一抽象**；框架能提供的是「注册动词 → 把路径交给应用」，不是「一个跨平台右键菜单 API」。
- 现状基线：框架内**不存在**任何 shell 扩展、文档类型、服务或注册表动词代码（全仓检索无 `IContextMenu`/`IExplorerCommand`/`NSServices`/`CFBundleDocumentTypes`/注册表写入）。Swift 参考版同样没有，属于新能力。

### 各平台能力上限

| 平台 | 入口 | 能做到 | 硬限制 |
| --- | --- | --- | --- |
| macOS | 「打开方式」 | 纯清单声明，零代码 | 只在子菜单里，不是右键顶层 |
| macOS | 「服务」（Services） | 右键 → 服务 子菜单里的自定义命令 | 不在顶层；需应用被 LaunchServices 注册 |
| macOS | Action Extension | 进「快速操作」/访达动作 | 独立 `.appex`、沙箱、独立进程，需回传通道 |
| macOS | Finder Sync | 可进右键上下文菜单 | **只在被监视目录内生效**，不是全局 |
| Windows | 静态动词（注册表） | 任意文件/文件夹/空白处加命令 | Win11 折叠进「显示更多选项」，非一级菜单；`HKCR` 需提权 |
| Windows | `IExplorerCommand`（COM） | Win11 一级右键菜单、动态显隐/子菜单 | 需 COM 服务器 + 稀疏包/MSIX + 签名 |
| Linux | 文件管理器专有机制 | 各文件管理器各自的右键项 | 无统一 API，需按管理器分别安装 |

## 已存在的接收侧链路（可复用）

无论命令从哪个系统入口进来，最终都要把选中的文件路径交给应用。这条「被带文件启动 / 被带文件唤起」的接收通道已经打通：

| 环节 | 位置 | 行为 |
| --- | --- | --- |
| 原生入队（macOS/iOS） | [`apple.m`](../crates/shell/native/apple.m) `cove_open_documents_push`；`kAEOpenDocuments` / `kAEGetURL` 处理器 | 经 `NSAppleEventManager` 注册 Apple Event 处理器（不依赖 app delegate），平台线程把路径/URL 推入线程安全收件箱并唤醒事件循环 |
| shell 排空 | [`open_documents.rs`](../crates/shell/src/open_documents.rs) `drain()` | macos/ios 走 `apple::drain()`；其余走 `argv::drain()`，一次性取走全部非 flag 启动参数后清空 |
| host 投递 | [`runtime.rs`](../crates/hosts/src/desktop/application/runtime.rs) `deliver_open_documents`（tick 内调用） | 排空收件箱，交给核心 `OpenedDocuments::deliver` |
| 应用回调 | [`platform_input_context.rs`](../crates/core/src/scene/input/platform_input_context.rs) `OpenedDocuments::set_handler` | 回调签名 `Rc<dyn Fn(Vec<String>)>`；未装处理器时路径先入队，装处理器后按到达顺序回放 |

应用侧接入示例（路径以核心导出为准）：

```rust
cove_core::scene::OpenedDocuments::set_handler(Some(Rc::new(
    |paths: Vec<String>| { /* 打开/解压这些归档 */ },
)));
```

Windows 与 Linux 的动词在启动时把文件路径写进命令行（`%1` / `%f`），因此进入 `argv` 通道；`argv::drain()` 返回的是**全部**参数，多选时 `argv[1..]` 是多个路径，调用方不要只读 `argv[1]`。macOS 的「打开方式」与「服务」都走 Apple Event，**不会**出现在 `argv` 里。

## macOS

### 1. 「打开方式」文档关联（纯清单声明）

在 `Info.plist` 声明 `CFBundleDocumentTypes`，把归档扩展名/UTI 关联到本应用，应用即出现在该类文件的「打开方式」子菜单。

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
| `NSSendFileTypes` | **文件型服务首选**：UTI 数组，如 `public.item` 或归档 UTI；Finder 选中文件时按此匹配 |
| `NSSendTypes` | 粘贴板类型数组。仅当要匹配非文件数据（如 `public.utf8-plain-text`）时使用 |
| `NSRequiredContext` | 可选，限定适用上下文（如 `NSTextContent = public.content`） |
| `NSUserData` | 可选的服务侧字符串透传 |

回调签名（AppKit 按 `NSMessage` 拼出 `…:userData:error:` 形式）：

```objc
- (void)coveExtractService:(NSPasteboard *)pboard
                  userData:(NSString *)userData
                     error:(NSString **)error;
```

**目标对象不需要自有 app delegate**。服务消息不进 Apple Event，走的是响应链/服务提供者：

```objc
// 在应用启动完成后注册一次；provider 需被强引用。
[NSApp setServicesProvider:provider];
[NSApp registerServicesMenuSendTypes:@[NSPasteboardTypeFileURL]
                         returnTypes:@[]];
```

provider 实现里从 `pboard` 读 `NSURL`（`NSFilenamesPboardType` 仅为兼容旧系统），解析成本地路径后推入现有 `cove_open_documents_push` 收件箱，复用上面的接收链路。

**可见性前提**：应用必须被 LaunchServices 注册（`cargo run` 出的裸二进制不会被注册，服务一定不出现），改完清单需重新注册并让 Finder 刷新缓存：

```bash
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f /Applications/YourApp.app
killall Finder          # 菜单缓存刷新
```

### 3. Action Extension（「快速操作」）

希望命令出现在访达右键的「快速操作」里，现代 macOS 的做法是独立 **Action Extension**：独立 `.appex` target，`NSExtension` + `NSExtensionPrincipalClass`，实现 `NSExtensionRequestHandling`。

限制：扩展跑在**独立沙箱进程**里，不属于本壳进程内能力；拿到文件 URL 后要么自己完成解压（受沙箱可写范围限制），要么经 App Group 容器 / URL scheme / NSXPC 把任务交回主应用。扩展点标识与 Finder 承载方式以 Apple 文档为准，且需在打包产物里真实嵌入并签名。

### 4. Finder Sync 扩展（可进右键上下文菜单）

`FIFinderSync` 子类可在 `menu(for:)` 返回自定义 `NSMenu`、`menuKind` 为上下文菜单。**它只在 `directoryURLs` 监视的目录内出现**——适合「监视下载目录 / 用户指定的解压目录」，不能当全局右键项使用；对存档工具是可选增强，不是替代方案。

### 打包注意

- 清单源是 **`Templates/app/Platforms/macOS/Info.plist`**（不是 `Platforms/macOS/Info.plist`；后者若存在也属组件自己的模板，如 `crates/terminal/`）。该模板已带**注释化**的 `CFBundleDocumentTypes` / `NSServices` 参考块，默认不生效；应用按自己的扩展名/UTI 解注释并改写。
- macOS 清单由外部打包器 `cove-package`（`xtask/`，git 依赖 tag `v0.3.0`）拷进 `.app`。模板到产物的键保留/合并方式需在实际打包产物里核实，不能假设模板原样保留。

## Windows

### 1. 静态动词（注册表，无 COM）

简单命令（「解压到此处」）由注册表完成，被调起时以命令行带文件路径启动应用。

| 注册表位置 | 适用对象 | 提权 |
| --- | --- | --- |
| `HKCU\Software\Classes\*\shell\<verb>` | 所有文件 | 否（推荐：单用户安装） |
| `HKCU\Software\Classes\Directory\shell\<verb>` | 文件夹 | 否 |
| `HKCU\Software\Classes\Directory\Background\shell\<verb>` | 目录空白处 | 否 |
| `HKCU\Software\Classes\SystemFileAssociations\.zip\shell\<verb>` | 特定扩展名 | 否 |
| `HKCR\...`（同上路径） | 同上 | **是**（机器级安装才需要） |

动词子键值：`MUIVerb`（显示名，可为 `@dll,-id` 本地化资源）、`Icon`（`exe,0`）、`Position`（可选，`Top` / `Bottom`）；其下 `command` 子键默认值为命令行，如 `"C:\…\app.exe" "%1"`。多选语义由 `MultiSelectModel` 控制（`Single` / `Document` / `Player`）：默认语义为逐项调用，一次拿到全部选中项需显式设为 `Player`，实际行为以资源管理器实测为准。

**Win11 现实约束**：静态动词属于 legacy，会被折叠进右键「显示更多选项」；「一眼可见的一级菜单」只有 `IExplorerCommand` 方案能做到。若产品验收标准是「右键一级可见」，静态动词方案不达标——这一点必须在排期时写明。

### 2. 动态/复杂菜单（COM 处理器）

需要按文件类型动态显隐、勾选态、子菜单或自定义绘制，或要出现在 Win11 一级右键菜单时，实现 COM shell 扩展：旧版 `IContextMenu`，Win11 现代 `IExplorerCommand`。后者需要稀疏包/MSIX 打包（AppxManifest 中声明 `windows.fileExplorerContextMenus` 扩展）并签名，是独立的 COM 服务器与发布流程。

### 3. 写入与卸载的归属

注册/卸载应由**安装器**（NSIS / WiX / MSIX）生成，而不是应用首次运行自注册：自注册会留下孤儿键、需要处理提权、且用户可能先删除目录再卸装。安装器需同时处理升级（同 verb 键覆盖）与卸载（删键 + 通知 `SHChangeNotify(SHCNE_ASSOCCHANGED)` 刷新外壳缓存）。

## Linux

Linux 没有 Windows 式的统一注册表动词，也没有 macOS 的服务清单；右键菜单由**各文件管理器**自己的机制提供。框架只能抽象成「动词名 + 一组路径」，安装期按用户所用的管理器分别落文件。

| 文件管理器 | 机制 | 安装位置（用户级） | 备注 |
| --- | --- | --- | --- |
| GNOME Files (Nautilus) | scripts | `~/.local/share/nautilus/scripts/<name>` | 脚本须可执行；选中路径经环境变量 `NAUTILUS_SCRIPT_SELECTED_FILE_PATHS`（换行分隔）传入；缺失时 `NAUTILUS_SCRIPT_SELECTED_URIS` 为 file URI |
| KDE Dolphin | service menu | `~/.local/share/kio/servicemenus/<name>.desktop`（旧版为 `~/.local/share/kservices5/ServiceMenus/`） | `X-KDE-ServiceTypes=KonqPopupMenu/Plugin`、`MimeType=application/zip;`、`Actions=extract;`、`Action.extract.Name/Exec`；命令占位符 `%f` 单文件、`%F` 多文件 |
| Thunar (XFCE) | 自定义动作 | `~/.config/Thunar/uca.xml` | 图形界面有限，通常直接写 xml；`%f` / `%F` 同义 |
| Nemo (Cinnamon) | actions | `~/.local/share/nemo/actions/<name>.nemo_action` | `.nemo_action` 的 `Exec` / `Selection` 键 |
| 任意（「打开方式」） | MIME 关联 | `~/.local/share/applications/<name>.desktop` | `MimeType=application/zip;application/x-7z-compressed;…`、`Exec=app %F`；生效用 `xdg-mime default <name>.desktop application/zip` |

注意：

- 这些机制都只在**图形会话**里可见，且没有「一次注册全平台生效」的路径；安装器要探测桌面环境，或按需安装多个 action 文件。
- 沙箱分发（Flatpak / Snap）下，宿主机脚本与 portal 权限会限制可见性与可访问路径，不能假设与裸装行为一致；`XDG Desktop Portal` 的 `OpenURI` 只解决「打开」，不提供右键动词。
- 与 Windows 一样，卸载要删净上述文件，否则菜单残留。

## 职责分层现状

| 关注点 | 归属层 | 现状 |
| --- | --- | --- |
| 应用内文件列表、行操作菜单 | Compose `ContextMenu` | 已有 |
| 被带文件启动 / 唤起后的路径接收 | shell / host / core | 已有 |
| 平台无关的「动词注册」抽象（动词名、扩展名/UTI、目标命令、目标回调） | shell（薄抽象） | 缺 |
| macOS 文档类型、服务清单声明 | 打包清单 `Templates/app/…/Info.plist` | 模板已有注释块，应用侧未启用 |
| macOS 服务回调目标（provider） | shell 的 Apple 层 | 缺（不需要 app delegate，加 provider 即可） |
| macOS Finder Sync / Action Extension | 独立 target + 打包器 | 缺 |
| Windows 静态动词注册/卸载 | 安装器 | 缺（无安装器框架） |
| Windows `IExplorerCommand` 处理器 | 独立 COM 服务器 + MSIX | 缺 |
| Linux 各文件管理器 action 文件 | 安装器 | 缺 |

分层原则：动词的**文案与语义**（「解压到此处」、适用扩展名）属于应用；壳层只做「把命令和回调登记进平台机制、把路径送进 opened-documents」的通用工作，不在框架里硬编码压缩包语义。

## 待办与验收

- 壳层：新增平台无关的动词注册 API（动词 id、显示名、适用扩展名/UTI、激活命令、回调），内部按平台落到 Info.plist 片段、注册表键或 Linux action 文件；先支持 macOS 服务与 Windows 静态动词两条最小路径。
- macOS：`NSApp setServicesProvider:` 接入，provider 把路径推入 opened-documents。**原生可见性**（访达「服务」菜单出现该项、选中文件后回调到应用）须打包成 `.app` 真机复核。
- Windows：安装器写入/卸载动词；`MultiSelectModel` 与多选语义实测；Win11「显示更多选项」下的可见性实测。
- Linux：至少覆盖 Nautilus 与 Dolphin 两种机制，其余按需。
- 验收原则：键位构造、注册表路径、pasteboard/参数解析可用结构化测试覆盖；外壳里的实际可见性与交互属原生验收，单列记录，不与结构测试混同。DevTools 不覆盖外壳菜单可见性。
- 「改了没效果」排查顺序：是否真的注册（macOS `lsregister -dump | grep -i <app>`；Windows `reg query "HKCU\Software\Classes\*\shell\<verb>"`；Linux `xdg-mime query default <mime>`）、是否刷新了外壳缓存（Finder 重启 / `SHChangeNotify` / 注销重登）、是否为沙箱或开发构建。

## 参考

- Apple：[NSServices](https://developer.apple.com/documentation/bundleresources/information-property-list/nsservices)、[NSApplication setServicesProvider:](https://developer.apple.com/documentation/appkit/nsapplication/1428471-setservicesprovider)、[Add functionality to Finder with action extensions](https://developer.apple.com/documentation/appkit/add-functionality-to-finder-with-action-extensions)、[Finder Sync](https://developer.apple.com/documentation/findersync)
- Windows：Shell verbs（`*\shell`、`Directory\shell`、`MultiSelectModel`）、`IExplorerCommand` / `IContextMenu` COM 上下文菜单处理器、MSIX 外壳扩展打包
- Linux：[Nautilus scripts](https://help.gnome.org/users/gnome-help/stable/nautilus-scripts.html)、KDE service menus（`X-KDE-ServiceTypes`）、Thunar 自定义动作、[xdg-mime](https://portland.freedesktop.org/doc/xdg-mime.html)
