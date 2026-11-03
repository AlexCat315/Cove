# WorkspaceView

`WorkspaceView::new` 观察 `WorkspaceController` 的有效命令，保留既有视图树。区域的首选尺寸、最小值和最大值来自 `WorkspacePanelSizing`；测量不会改写文档，拖动、键盘和辅助功能使用同一约束。

通过 `WorkspaceThemeEnvironment::key()` 提供 `WorkspaceTheme`。`tab_bar_height` 跟随字号，`animates_layout` 控制区域关闭与折叠动画；离场保留绘制并禁用输入，反转保留面板身份。左右方向键按布局方向切换标签，Home / End 到两端。

共享 Rust 实现覆盖所有宿主平台。自动化场景验证与原生视觉、键盘及 IME 验收分别记录在 Gallery 验收表中。
