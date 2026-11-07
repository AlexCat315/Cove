# Sidebar / AdaptiveSidebar

`Sidebar` 使用稳定的条目标识保留行节点。`AdaptiveSidebar::try_new` 在侧栏、rail 和紧凑抽屉之间保留导航、正文和编辑状态；失去可见性的焦点交给选中目的地或导航按钮。

`SidebarItem::debug_name` 可为目的地指定检查标识；选择和展开分别由绑定持有。

共享 Rust 实现覆盖所有宿主平台。自动化场景验证与原生视觉、键盘及 IME 验收分别记录在 Gallery 验收表中。

## 外观

侧栏贴边、全高，使用 `surface_variant` 填充，与内容区之间一条发丝线；行高 30pt（触屏 44pt），图标使用强调色。选中项是比侧栏深一级的中性圆角底，切换选中时同一个底块以弹簧滑到新行（见 [动效](/docs/motion)）；悬停与按压由各行即时绘制。
