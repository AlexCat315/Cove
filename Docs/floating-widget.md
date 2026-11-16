# 桌面悬浮组件（Floating widget）

本文件记录 Cove 为「桌面悬浮球 / 常驻小组件」提供的一等公民能力：异形（点击穿透）窗口、窗口初始位置与多屏、系统托盘、全局热键。这些是框架层原语，不含任何具体业务（例如拖文件解压由应用的 `FileDropZone` 完成，见 [拖放上传文档](../Website/src/content/components/drag-upload.md)）。

## 参考落地：RichZip / MugenZip 悬浮球

`../RichZip/src/widget.rs` 是一份完整的产品级示例：`mugenzip --widget` 用四个原语拼出一个「拖文件进来即解压」的圆形悬浮球。

- **窗口**：132×132 正方形 + `WindowShape::Ellipse` 成正圆；`shows_dock_icon = false` 去掉任务栏；`clear_color = [0,0,0,0]` 透明背景；启动后用 `display_bounds(0, true)` + `set_window_position` 停在主屏右上角。
- **拖放**：`FileDropZone` 铺满球面，只收压缩包扩展名；回调直接调 `extract_service::run(path, IntoFolder)`（与资源管理器「解压到同名文件夹」动词同一条 headless 链路），解压后 reveal 目录，全程不弹浏览窗口。
- **托盘**：`status_item::install` 提供「打开主界面 / 显示隐藏 / 退出」；点击球用 `std::process::Command` 另起一个无 `--widget` 的进程打开完整浏览器。
- **热键**：`Ctrl+Shift+K` 经 `register_hotkey` 切换 `hide_window` / `show_window`。

入口分发在 `../RichZip/src/lib.rs`：自定义 `run()` 检查 `--widget` 参数，在悬浮球声明和常规浏览器声明之间切换。

构建（Windows，需要 Windows SDK 的 `mt.exe` 在 PATH）：

```bash
cd C:/dev/RichZip
export PATH="/c/Program Files (x86)/Windows Kits/10/bin/10.0.26100.0/x64:$PATH"
cargo build --bin mugenzip
./target/debug/mugenzip.exe --widget
```

三平台覆盖以 `cargo check`/结构化测试为界；原生交互的最终验收仍需真机。

## 1. 异形窗口与点击穿透

在 `WindowConfiguration` 上设置 `shape` 即可让窗口呈现为圆形或圆角矩形，形状之外的像素既不绘制也不接收鼠标事件：

```rust
app.window.shape = Some(WindowShape::Ellipse);              // 正方形窗口即正圆
app.window.shape = Some(WindowShape::RoundedRect { radius: 18.0 });
```

配套的透明渲染：把 `app.window.rendering.clear_color` 设为 `[0,0,0,0]`，并让内容本身按同一形状绘制 alpha 通道（例如 `corner_radius = 边长/2` 的圆形视图），否则视觉矩形与点击区域不一致。

Windows 实现要点（否则圆外会是一块浅灰方块，而不是透明）：`SDL_SetWindowShape` 用 `SetWindowRgn` 裁出椭圆的命中/绘制区域，但 DX12 的 DirectComposition swapchain 会绕过 Win32 region。异形窗口必须走一条独立的呈现路径——`SurfaceDescriptor.shaped = true` 时，`dx12/composition.rs` 跳过 `DwmExtendFrameIntoClientArea(-1…)` 和 `DWMWA_SYSTEMBACKDROP_TYPE=TRANSIENTWINDOW`（它们会在整个客户区铺 DWM 磨砂灰，填满被裁掉的圆角）。该标志由 `AppWindowRenderSettings.shaped` 从 `WindowConfiguration::shape.is_some()` 一路传到合成层。跳过 backdrop 后，clear 像素在 `AlphaMode=PREMULTIPLIED` 的 DirectComposition swapchain 上真正透明，圆外直接合成到桌面。

视觉层注意（实测）：在 shaped 透明窗口里，裸 `Text` / `Icon` 视图不会绘制；内容要用 `Button::title` / `Button::icon` 这类内置 label 路径，并放在被裁剪的圆盘**之外**的 ZStack 层（放进被 `clipped()` 的 zone 里 label 会消失）。

实现走 vendored SDL3 的 `SDL_SetWindowShape`，它按平台分派到各自的原生机制，因此一套配置覆盖三平台：

| 平台 | 原生机制 | 说明 |
|---|---|---|
| Windows | `SetWindowRgn`（layered window 区域） | `SDL_WINDOW_TRANSPARENT` 创建时置位 |
| Linux (X11) | XShape extension | 同上 |
| macOS | `Cocoa_UpdateWindowShape` | 透明像素穿透 |
| Wayland | 无形状协议 | 请求被安全忽略，退化为透明矩形窗口，功能不受影响 |

`shows_dock_icon = false` 会让窗口成为 utility window：Windows 上 SDL 映射为 `WS_EX_TOOLWINDOW`（不占任务栏按钮），X11 上为 `_NET_WM_WINDOW_TYPE_UTILITY`，macOS 上应用切换为 Accessory（不显示 Dock 图标）。这与 `AppDisplayHandle::hide_window` 的常驻语义配套。

底层 C 入口 `cove_shell_set_window_shape(kind, radius)`：`kind=0` 椭圆、`kind=1` 圆角矩形；alpha 蒙版由 SDL 复制并按后端要求拉伸。

## 2. 窗口初始位置与多屏

### 启动时定位

`WindowConfiguration::placement: Option<AppWindowPlacement>`：

```rust
app.window.placement = Some(AppWindowPlacement::at(100.0, 200.0));  // 绝对窗口单位
app.window.placement = Some(AppWindowPlacement::on_display(1));     // 居中于 1 号屏
```

同时给 `position` 与 `display` 时，坐标相对该显示器原点。`None` 保持系统默认。

### 运行时查询与移动

`AppDisplayHandle` 新增：

```rust
display.display_count() -> usize
display.display_bounds(index, usable) -> Option<DisplayBounds> // x/y/width/height + usable 标志
display.set_window_position(x, y)   // 主窗口左上角，窗口单位
display.window_position() -> Option<(f32, f32)>
```

`usable = true` 返回排除任务栏 / 菜单栏后的可用区（平台有报告时）。`DisplayBounds` 来自 `cove_shell`，`AppDisplayHandle` 在桌面平台可用。

显示器顺序为 SDL display order；`index` 越界返回 `None` / 0。

## 3. 系统托盘（状态栏）

`status_item` 模块此前仅 macOS 可用，现已扩展到 Windows / Linux。公开入口不变（`cove_ui::hosts::status_item`）：

```rust
use cove_ui::hosts::{status_item, status_item::StatusEntry};

status_item::install("图标路径或 SF Symbol", "悬浮助手", &[
    StatusEntry { id: "toggle", title: "显示 / 隐藏" },
    StatusEntry { id: "",      title: "" },            // 空标题 = 分隔线
    StatusEntry { id: "quit",  title: "退出" },
], move |id| { /* UI 线程执行 */ })?;
```

- **macOS**：`NSStatusItem` + `NSMenu`；`quit` 由原生直接 terminate（隐藏态仍能响应菜单栏）。
- **Windows / X11**：vendored SDL3 的 `SDL_CreateTray` 托盘与菜单（图标 / tooltip / 点击回调），经 `cove_shell_tray_install` 接入；点击把条目索引入队并 `cove_shell_wake()`，UI 线程经 `status_item::poll()` 消费。
- **iOS / Android**：`install` 返回 `Err(Unsupported)`；`status_item::SUPPORTED` 常量可在构建 UI 前判断。
- **icon**：macOS 传 SF Symbol 名；Windows / Linux 传 BMP 文件路径（空串用平台默认图标）。

菜单在平台主线程构建一次（`flush_pending` 每 tick 调用，幂等），handler 属于调用 `install` 的 UI 线程。

## 4. 全局热键

在 `AppDisplayHandle` 上注册，回调在 UI 线程执行：

```rust
use cove_ui::hosts::{HotKey, HotKeyModifiers};

display.register_hotkey(
    HotKey::new(1, HotKeyModifiers { control: true, shift: true, ..Default::default() }, 'K' as i32),
    std::rc::Rc::new(move || { /* 呼出 / 隐藏悬浮球 */ }),
);
// 退出前
display.unregister_hotkey(1);
```

`key` 为 ASCII 大写字母 / 数字，或功能键（Windows `VK_F1..VK_F12`，Linux 同值映射到 XK_F1..F12，macOS 走 SDL 键名 → 虚拟键码）。`id` 是应用自定正整数，触发时回调只拿到 id。

| 平台 | 机制 | 备注 |
|---|---|---|
| Windows | `GetAsyncKeyState` 每帧轮询 + 边沿检测 | 不依赖消息循环，修饰键需精确匹配（按下组合之外的修饰键不触发） |
| macOS | Carbon `RegisterEventHotKey` | 不需辅助功能授权；修饰键常量 shift/control/option/cmd |
| Linux X11 | `XQueryKeymap` 每帧轮询（dlopen `libX11.so.6`） | 无链接期 X11 依赖；Wayland 无 X11 连接时返回 `false` |
| Wayland | — | 无全局热键协议（需 portal），注册返回 `false`，应用应提供降级入口 |

内部触发经 SDL user event（kind 30，携带 id）进入既有输入流，宿主的输入拦截器把它派发给登记的 handler。注册返回 `false` 表示平台不支持或 id 非法，应用应据此禁用相关 UI。

## 平台覆盖一览

| 能力 | Windows | macOS | Linux (X11) | Wayland |
|---|---|---|---|---|
| 异形 + 点击穿透 | ✅ | ✅ | ✅ | ⚠️ 退化为透明矩形 |
| 跳过任务栏 / Dock | ✅ | ✅ | ✅ | ✅（由合成器决定） |
| 初始位置 / 多屏 | ✅ | ✅ | ✅ | ✅ |
| 系统托盘 | ✅ SDL | ✅ NSStatusItem | ✅ SDL | ✅ SDL（状态项协议） |
| 全局热键 | ✅ | ✅ | ✅ | ❌ 需 portal |

## 仍然需要的真机验收

- 圆形窗口四角的点击穿透、任务栏隐藏、拖动与置顶的实际交互。
- 多显示器不同 DPI 缩放下的坐标换算（`set_window_position` 使用窗口单位）。
- 托盘图标在各桌面环境的显示、菜单点击回投；macOS `quit` 在隐藏态的 terminate。
- 全局热键被其他应用占用时的失败处理与组合键语义。
