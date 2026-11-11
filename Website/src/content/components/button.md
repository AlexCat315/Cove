# Button

```rust
use cove_ui::prelude::*;

fn save_button(on_save: impl Fn() + 'static) -> Button {
    Button::new(Text::new("Save"), on_save)
}
```

内置按钮样式使用主题的标签字体。标签沿用 `Text` 的浮点字距和四分之一设备像素横向取样，适用于非整数的文字缩放比例。
