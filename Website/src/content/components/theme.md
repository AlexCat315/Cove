# 默认主题

`Theme::default_light()` 与 `Theme::default_dark()` 采用 macOS 窗口结构：内容区为白色（深色 `#1E1E1E`），侧栏与分组区高一级填充（`surface_variant`），区域靠填充而非描边或阴影区分；强调色为系统蓝 `#0071E3`，白字对比度不低于 4.5:1。

```rust
use cove_ui::prelude::*;
use cove_ui::compose::theme::{Theme, ThemeViewExt};

fn content() -> impl View {
    Text::new("Cove").theme(Theme::default_light())
}
```

## 形状与层次

圆角为 0 / 6 / 8 / 12 / 16pt（`radius.none` … `radius.xl`）。按钮、分段控件、搜索框、弹出按钮与滑块拇指为胶囊；纯图标按钮无边框。未指定样式的 `Button` 是 macOS 推按钮（白底、发丝边、接触阴影），默认动作需显式使用 `PrimaryButtonStyle`。

文字沿用 SF 的字距规律：字号越大字距越紧（`display` 34pt / -0.9，`title` 26pt / -0.6，`headline` 17pt / -0.4）。

## 材质

`theme.materials` 提供三种磨砂材质，均为半透明底色 + 背景模糊 + 1pt 边缘：

| 材质 | 用途 |
| --- | --- |
| `chrome` | 悬浮侧栏、工具栏 |
| `content` | 需要透出背景的分组内容 |
| `floating` | 菜单、弹出层、对话框（`floating_surface` 默认使用） |

```rust
use cove_ui::prelude::*;
use cove_ui::compose::modifiers::style::{GlassExt, GlassKind};

fn toolbar(content: impl View + 'static) -> impl View {
    content.glass(GlassKind::Chrome).corner_radius(16.0)
}
```

## 动效

`theme.motion` 以弹簧为主：`fast` 0.12s、`standard` 0.28s、`slow` 0.45s，曲线为 `Easing::RESPONSIVE`（强 ease-out）与 `Easing::FLUID`（iOS 面板曲线）。浮层进入用欠阻尼弹簧（`surface.entrance` 0.38s，从 `reveal_scale` 0.9 弹出），关闭用 0.14s ease-out。`press_scale` 为 0.96：按下即时压缩，松手按 `button_release_spring` 回弹。详见 [动效](/docs/motion)。

## 自定义

应用可以复制主题修改对应 token：

```rust
use cove_ui::prelude::*;
use cove_ui::compose::theme::{Theme, ThemeViewExt};

fn branded_content() -> impl View {
    let mut theme = Theme::default_light();
    theme.colors.accent = Color::from_rgba8(49, 97, 127, 255);
    theme.colors.accent_hover = Color::from_rgba8(36, 77, 103, 255);
    theme.colors.accent_pressed = Color::from_rgba8(27, 60, 81, 255);
    theme.colors.selection = Color::from_rgba8(91, 121, 151, 72);
    // InputAppearance 在构造主题时初始化；修改强调色后同步输入框焦点色。
    theme.inputs.border_focused = theme.colors.accent;
    Text::new("Cove").theme(theme)
}
```

输入框使用 `surface_sunken` 底色、1pt 边框，聚焦时改变边框颜色；统一调整时修改 `theme.inputs.background` / `theme.inputs.radius`。自定义 [TextFieldStyle](/docs/text-field-style) 可以消费同一组 token。

正文与输入框选区共用 `colors.selection`，`colors.focus_ring` 控制键盘焦点提示。

`ThemeReader` 闭包中读取的可观察状态属于外层视图的读取依赖；重建后会重新订阅，移除视图时释放依赖。
