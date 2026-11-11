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

`.immersive_menu_bar(menus)` 将菜单放进 Cove 自绘标题区。配合 `WindowBackdrop::Sidebar`，标题区和正文共用窗口材质。

`.native_menu_bar(menus)` 使用平台菜单，目前仅 macOS 由 AppKit 显示应用菜单；其它桌面平台没有系统应用菜单，可通过 `NativeMenuBar::is_supported()` 查询。桌面窗口统一使用上面的自绘单行标题区。

顶部栏与正文之间是否画分割线，由 `WindowConfiguration::title_bar_divider` 控制：`TitleBarDivider::Seamless`（默认）让窗口材质贯穿标题区，`TitleBarDivider::Hairline` 额外加一层淡色底和 1px 发丝线。两者布局和窗口按钮完全相同，偏好不依赖 `WindowBackdrop`。

用 `.submenu(NativeMenu::named("…"))` 添加子菜单。需要快捷键、选中或禁用状态时，用 `.item(NativeMenuAction::new(…))` 添加动作。原生菜单可通过 `AppDisplayHandle::install_native_menu_bar` 替换，刷新标题和可用状态。

需要在标题栏放置自己的视图时，可以直接使用 `ImmersiveMenuBar::new(menus).leading(view)`，并将窗口的标题栏设为 `AppWindowTitleBarStyle::HiddenInset`。

`WindowBackdrop::Sidebar` 让未被不透明内容覆盖的像素透出系统材质。macOS 使用 AppKit 侧栏材质，Windows 的 DX12 路径使用 DirectComposition 和 DWM。平台没有可用的合成支持时使用不透明背景。
