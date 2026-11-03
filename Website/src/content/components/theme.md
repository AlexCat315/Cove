# 默认主题

`Theme::default_light()` 与 `Theme::default_dark()` 使用中性桌面配色：浅色为暖白与石墨色控件，深色为分层炭灰与浅灰色控件。状态色只用于成功、警告、错误和信息。

```rust
use cove_ui::prelude::*;
use cove_ui::compose::theme::{Theme, ThemeViewExt};

fn content() -> impl View {
    Text::new("Cove").theme(Theme::default_light())
}
```

输入框使用 `surface_sunken` 底色、1pt 边框，聚焦时改变边框颜色。默认圆角为 0 / 4 / 6 / 8 / 12pt；胶囊类控件仍可使用 `radius.pill`。

正文与输入框选区共用 `colors.selection`，使用低饱和灰蓝色与轻微圆角，两端以短竖线标记边界。选区直接使用主题透明度，不额外淡化文字。`colors.focus_ring` 控制键盘焦点提示，`inputs.border_focused` 控制输入框焦点边框。

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

需要统一调整输入底色或圆角时，修改 `theme.inputs.background` / `theme.inputs.radius`。自定义 [TextFieldStyle](/docs/text-field-style) 可以消费同一组 token，或为业务场景提供专属外观。
