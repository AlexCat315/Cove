# Sidebar / AdaptiveSidebar

`Sidebar` 使用稳定的条目标识保留行节点。`AdaptiveSidebar::try_new` 在侧栏、rail 和紧凑抽屉之间保留导航、正文和编辑状态；失去可见性的焦点交给选中目的地或导航按钮。

`SidebarItem::debug_name` 可为目的地指定检查标识；选择和展开分别由绑定持有。

共享 Rust 实现覆盖所有宿主平台。自动化场景验证与原生视觉、键盘及 IME 验收分别记录在 Gallery 验收表中。
