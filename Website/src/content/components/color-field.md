# ColorField

色块打开 RGB、透明度及十六进制编辑浮层。shows_inline_values 显示十六进制；shows_inline_channels 控制通道读数。show_alpha(false) 隐藏透明度编辑。颜色值由调用方绑定持有。

```rust
use cove_core::{Color, State};
use cove_ui_compose::primitives::controls::fields::color_field::ColorField;

let color = State::new(Color::from_rgba8(88, 126, 231, 255));
let field = ColorField::new(color.binding()).shows_inline_values(true);
let rgb_only = ColorField::new(color.binding()).show_alpha(false);
```

Gallery 对齐 Swift：同一颜色绑定的两个触发器，以及 200×60 的实时色块。浮层宽度随视口限制，输入草稿跨重组保留，浅/深主题读取当前主题。

共享 Rust 实现适用于所有宿主。结构测试与 macOS 原生操作验收分别记录在 `Docs/gallery-audit.md`；输入事件注入不代表原生键盘或 IME 已验收。
