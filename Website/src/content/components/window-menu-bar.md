# 窗口菜单栏

先定义菜单和动作，再选择呈现方式。

```rust
use cove_ui::{ApplicationLaunch, NativeMenu, WindowBackdrop};

let menus = vec![NativeMenu::named("View")
    .action("Refresh", refresh)
    .separator()
    .action("Reset", reset)];

let launch = ApplicationLaunch::new(content)
    .window_backdrop(WindowBackdrop::Sidebar)
    .immersive_menu_bar(menus);
```

将 `.immersive_menu_bar(menus)` 换成 `.native_menu_bar(menus)` 即可使用系统菜单。macOS 使用应用菜单栏，Windows 使用窗口的原生菜单栏。在没有统一原生菜单栏的桌面环境中，启动配置使用沉浸式菜单。

用 `.submenu(NativeMenu::named("…"))` 添加子菜单。需要快捷键、选中或禁用状态时，用 `.item(NativeMenuAction::new(…))` 添加动作。原生菜单可通过 `AppDisplayHandle::install_native_menu_bar` 替换，刷新标题和可用状态。

需要在标题栏放置自己的视图时，可以直接使用 `ImmersiveMenuBar::new(menus).leading(view)`，并将窗口的标题栏设为 `AppWindowTitleBarStyle::HiddenInset`。

`WindowBackdrop::Sidebar` 让未被不透明内容覆盖的像素透出系统材质。macOS 使用 AppKit 侧栏材质，Windows 的 DX12 路径使用 DirectComposition 和 DWM。平台没有可用的合成支持时使用不透明背景。
