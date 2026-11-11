# 窗口菜单栏

用一份 `NativeMenu` 定义菜单、动作和快捷键，交给宿主呈现。

```rust
use cove_ui::{ApplicationLaunch, NativeMenu, WindowBackdrop};

let menus = vec![NativeMenu::named("View")
    .action("Refresh", refresh)
    .separator()
    .action("Reset", reset)];

let launch = ApplicationLaunch::new(content)
    .configure(|app| app.window.title = "Editor".into())
    .window_backdrop(WindowBackdrop::Sidebar)
    .menu_bar(menus);
```

`.menu_bar(menus)` 根据宿主选择菜单栏。

| 平台 | 菜单呈现 |
| --- | --- |
| macOS | AppKit 将菜单项安装到屏幕顶部的系统应用菜单栏，主快捷键使用 Command。 |
| Windows | Cove 在窗口顶部绘制同一份菜单、窗口标题和窗口按钮，主快捷键使用 Ctrl。 |
| Linux、iOS、Android、Web | 保留宿主框架。当前宿主没有应用菜单栏后端。 |

`.native_menu_bar(menus)` 直接请求系统菜单栏。用 `NativeMenuBar::is_supported()` 查询宿主能力。

Windows 自绘栏读取 `WindowConfiguration::title`，窄窗口中会截断标题。顶层菜单项在悬停时显示小圆角底色，展开后保留更明显的底色。用 `WindowConfiguration::title_bar_divider` 切换 `Seamless` 或 `Hairline`。

用 `.submenu(NativeMenu::named("…"))` 添加子菜单。需要快捷键、选中或禁用状态时，用 `.item(NativeMenuAction::new(…))` 添加动作。原生菜单可通过 `AppDisplayHandle::install_native_menu_bar` 替换，刷新标题和可用状态。

Windows 需要把自绘栏放进视图树时，用 `ImmersiveMenuBar::new(menus).title("Editor").leading(view)`，并将窗口标题栏设为 `AppWindowTitleBarStyle::HiddenInset`。

`WindowBackdrop::Sidebar` 让未被不透明内容覆盖的像素透出系统材质。macOS 使用 AppKit 侧栏材质，Windows 的 DX12 路径使用 DirectComposition 和 DWM。平台没有可用的合成支持时使用不透明背景。
