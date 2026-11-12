# Tooltip

`TooltipExt::tooltip(description)` 给任意视图附加一条非交互说明。指针停留到 `delay`（默认 0.6s）后显示；提示经 portal 渲染，不占布局、不拦截命中，指针按下或 Escape 关闭，离开目标后重新计时。

```rust
let save = Button::title("Save", save).tooltip("Save the current workspace");
let row = Text::new(long_title).truncated().tooltip_when_truncated(long_title);
```

## 窗口级仲裁

同一时刻只显示一个提示：新提示让旧提示立即退场，嵌套目标中内层优先。一个提示出现后 0.4s 内（warm window）进入的另一个目标跳过停留直接显示，所以沿着工具栏移动不会每格重新等一次。

仲裁实例挂在窗口的输入上下文上，两个窗口互不抢占，新窗口从没有提示开始。

## 键盘与截断

可见焦点落在目标上时提示随焦点出现；`tooltip_when_truncated` 只在该节点真的截断文字后才显示，完整可见的标签保持安静。焦点出现不享受 warm window 的免等待，避免菜单关闭后立刻盖住用户刚用过的工具条。

## 位置

提示以目标为中心放在上方，顶部会超出窗口时翻到下方；实际高度参与落点计算，长文案按 `max_width`（默认 320pt）换行。

## 显式锚点

`Tooltip::anchored(TooltipAnchor::range(rect), content)` 给 LSP 悬停这类自己已经做了防抖的生产者使用：呈现直接跟随 `is_presented`，不再自行计时，锚点可以是点或矩形。

共享 Rust 实现覆盖所有宿主平台。自动化场景验证与原生视觉、键盘及 IME 验收分别记录在 Gallery 验收表中。
