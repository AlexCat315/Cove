# NavigationDrawer

`NavigationDrawer::try_new` 接收展示绑定、选择绑定和分区。宽度为 `min(320, window.width - 56)`，高度填满宿主。选择目的地默认关闭抽屉；`NavigationDrawerOptions::closes_on_selection` 可更改此策略。

入口和离场使用 leading 位移动画；离场期间停止接收输入。

共享 Rust 实现覆盖所有宿主平台。自动化场景验证与原生视觉、键盘及 IME 验收分别记录在 Gallery 验收表中。
