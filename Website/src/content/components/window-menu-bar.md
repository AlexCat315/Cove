# 窗口菜单栏

先定义菜单和动作，再选择呈现方式。

```rust
use cove_ui::{ApplicationLaunch, NativeMenu, WindowBackdrop};

let menus = vec![NativeMenu::named("View")
    .action("Refresh", refresh)
    .separator()
    .action("Reset", reset)];

let launch = ApplicationLaunch::new(content)
    .configure(|app| app.window.title = "Editor".into())
    .window_backdrop(WindowBackdrop::Sidebar)
    .immersive_menu_bar(menus);
```

`.immersive_menu_bar(menus)` 将菜单放进 Cove 自绘标题区，并显示 `WindowConfiguration::title`。标题在菜单和窗口按钮之间截断，桌面上可从标题区域拖动窗口。配合 `WindowBackdrop::Sidebar`，标题区和正文共用窗口材质。

| 平台 | 沉浸式菜单栏 |
| --- | --- |
| macOS | Cove 菜单和标题，左侧保留系统交通灯。 |
| Windows | Cove 菜单、标题和最小化、最大化、关闭按钮。最大化按钮尚未接入系统 Snap Layouts 浮层。 |
| Linux | Cove 菜单、标题和窗口按钮，拖动及缩放由 SDL 对接 X11 或 Wayland。 |
| iOS、Android、Web | 显式配置时显示应用内菜单和标题，窗口控制交给宿主。 |

`.native_menu_bar(menus)` 目前由 macOS AppKit 提供，可通过 `NativeMenuBar::is_supported()` 查询。Windows 和 Linux 使用 `.immersive_menu_bar(menus)` 在窗口内呈现菜单。

顶部栏与正文之间是否画分割线，由 `WindowConfiguration::title_bar_divider` 控制：`TitleBarDivider::Seamless`（默认）让窗口材质贯穿标题区，`TitleBarDivider::Hairline` 额外加一层淡色底和 1px 发丝线。两者布局和窗口按钮完全相同，偏好不依赖 `WindowBackdrop`。

用 `.submenu(NativeMenu::named("…"))` 添加子菜单。需要快捷键、选中或禁用状态时，用 `.item(NativeMenuAction::new(…))` 添加动作。原生菜单可通过 `AppDisplayHandle::install_native_menu_bar` 替换，刷新标题和可用状态。

直接嵌入时使用 `ImmersiveMenuBar::new(menus).title("Editor").leading(view)`。在桌面宿主中同时将窗口标题栏设为 `AppWindowTitleBarStyle::HiddenInset`。空字符串可隐藏自绘标题。

`WindowBackdrop::Sidebar` 让未被不透明内容覆盖的像素透出系统材质。macOS 使用 AppKit 侧栏材质，Windows 的 DX12 路径使用 DirectComposition 和 DWM。平台没有可用的合成支持时使用不透明背景。
