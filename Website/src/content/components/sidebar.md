# Sidebar / AdaptiveSidebar

`Sidebar` 使用稳定的条目标识保留行节点。`AdaptiveSidebar::try_new` 在侧栏、rail 和紧凑抽屉之间保留导航、正文和编辑状态；失去可见性的焦点交给选中目的地或导航按钮。

`SidebarItem::debug_name` 可为目的地指定检查标识；选择和展开分别由绑定持有。

共享 Rust 实现覆盖所有宿主平台。自动化场景验证与原生视觉、键盘及 IME 验收分别记录在 Gallery 验收表中。

## 外观

侧栏贴边、全高，使用 `surface_variant` 填充，与内容区之间一条发丝线；行高 30pt（触屏 44pt），图标使用强调色。选中项是比侧栏深一级的中性圆角底，切换选中时同一个底块以弹簧滑到新行（见 [动效](/docs/motion)）；悬停与按压由各行即时绘制。

## 半透明窗口

窗口以 `WindowBackdrop::Sidebar` 创建且平台应用成功时，`Environment::translucent_window` 为真，侧栏不画填充与发丝线，直接露出系统侧栏材质（macOS 为窗口后方模糊的 `NSVisualEffectView`）。正文应放在自带填充的容器里：

```rust
let mut configuration = ApplicationConfiguration::new(Some("App"));
configuration.window.backdrop = WindowBackdrop::Sidebar;
```

启用后材质铺到窗口顶边、标题栏透明，标题栏高度作为顶部安全区上报。原生外观跟随根主题而非系统设置：浅色主题配浅色材质与标题栏，深色主题配深色。宿主在根节点铺一层轻薄的主题色（`window_backdrop_tint`），保证非 vibrant 文字在任何背景上都可读，窗口标题隐藏。

Windows 与 Linux 当前回退为不透明：交换链以不透明方式创建，Mica 与合成器模糊无法透出，`translucent_window` 保持为假，侧栏照常填充。
