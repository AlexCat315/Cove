# WorkspaceView

`WorkspaceView::new` 观察 `WorkspaceController` 的有效命令，保留既有视图树。区域的首选尺寸、最小值和最大值来自 `WorkspacePanelSizing`；测量不会改写文档，拖动、键盘和辅助功能使用同一约束。

通过 `WorkspaceThemeEnvironment::key()` 提供 `WorkspaceTheme`。`tab_bar_height` 跟随字号，`animates_layout` 控制区域关闭与折叠动画；离场保留绘制并禁用输入，反转保留面板身份。左右方向键按布局方向切换标签，Home / End 到两端。

## 外观

工作区自带画布：根节点使用 `surface_variant` 填充，面板是 `surface` 圆角块，面板之间由 `gutter`（默认 6pt）露出的画布分隔，不画分割线；折叠后的侧轨与外壳槽位不再自绘底色。标签栏中的选中标签是胶囊，切换标签时同一个胶囊以 `SlidingSelection` 滑到新标签，与 TabView、分段控件一致。

槽位内的拆分（`WorkspaceLayoutNode::split`）按 `fraction` 分配两侧空间，中间保留一个 `gutter` 宽的分隔条：

```rust
use cove_ui_addons::workspace::{WorkspaceLayoutNode, WorkspaceSplitAxis, WorkspaceTabGroupID};

fn editor_over_preview() -> WorkspaceLayoutNode {
    WorkspaceLayoutNode::split(
        WorkspaceSplitAxis::Vertical,
        0.58,
        WorkspaceLayoutNode::group(WorkspaceTabGroupID::new("main")),
        WorkspaceLayoutNode::group(WorkspaceTabGroupID::new("preview")),
    )
}
```

共享 Rust 实现覆盖所有宿主平台。自动化场景验证与原生视觉、键盘及 IME 验收分别记录在 Gallery 验收表中。
