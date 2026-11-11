# Popover

`Popover::new(label, content)` 由带标签的触发按钮打开浮层。默认 `placement = .bottomLeading`、`surface = .bubble`、`overflow = .constrain`，与 Swift 版一致。

```rust
use cove_ui_compose::primitives::overlays::{Popover, PopoverPlacement, PopoverSurface};

let popover = Popover::new("Project settings", form)
    .is_presented(is_open.binding())
    .width(300.0)
    .placement(PopoverPlacement::BottomLeading);
```

指针环境下，`.bubble` 表面绘制卡片背景、描边、阴影和指向触发器的小三角；`.content` 表面只保留内容盒，供菜单、选择器等自绘容器使用。`.constrain` 把气泡约束在窗口内，`.flip` 在空间不足时翻到对侧。触摸环境改用同一内容的底部 sheet。

气泡会沿交叉轴微调位置，让小三角始终指向触发器中心——即使触发器很窄、靠 leading/trailing 对齐时也是如此；靠近视口边缘时优先保证气泡完整可见，小三角退到最近的可及点。

气泡内控件属于触发器的焦点作用域；Escape 或点击外部关闭气泡并还原焦点。`sizing(.anchor)` 让气泡宽度跟随触发器；`trigger_description` 为触发器补充无障碍标签、值与帮助文本；`trigger_style` 用自定义按钮样式替换默认触发器外观；`on_key` 可拦截气泡内的按键事件。

共享 Rust 实现覆盖所有宿主平台。自动化场景验证与原生视觉、键盘及 IME 验收分别记录在 Gallery 验收表中。