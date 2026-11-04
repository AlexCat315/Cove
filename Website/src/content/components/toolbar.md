# Toolbar / AppBar

`ToolbarCommands::try_new` 按可用空间测量命令，把低优先级项移入 More actions；Always 保留，Overflow 固定在菜单中。横向按宽度，纵向按高度处理溢出。

`ToolbarGroup` 共享轴、按钮样式与导航。水平使用 Left/Right，垂直使用 Up/Down；Home/End 到两端，搜索输入框保留自己的光标按键。禁用整栏时阻止子控件输入并释放焦点与捕获。

Gallery 对齐 Swift `ToolbarStory.swift`：自适应项目命令、播放组、带边框格式组、搜索、50…200% 缩放、Apply 栏、纵向工具组和八项纵向溢出命令。紧凑宽度下完整保留控件并换行。

```rust
use cove_ui_compose::primitives::controls::toolbar::{Toolbar, ToolbarCommands, ToolbarAxis, ToolbarItem, ToolbarItemPlacement};
use cove_ui_compose::primitives::overlays::menu::MenuItem;

let items = vec![ToolbarItem::new(MenuItem::new(1, "Run", || {}))
    .placement(ToolbarItemPlacement::Always)];
let commands = ToolbarCommands::try_new(items, true, ToolbarAxis::Horizontal).unwrap();
let bar = Toolbar::new(commands);
```

共享 Rust 实现覆盖所有宿主。结构与场景回归、原生视觉与操作验收分别记录在 `Docs/gallery-audit.md`。
