# Badge

`Badge` 是单行状态徽标，适合简短的状态名称。它按文字、前导标记和内边距确定宽度，文字随主题的文字缩放变化。窄栏可以让整个徽标换到下一行。

```rust
use cove_ui_compose::primitives::feedback::{Badge, StatusTone};

let badge = Badge::new("Running")
    .tone(StatusTone::Info)
    .shows_dot(true);
```

`BadgeVariant` 提供 Soft、Solid 和 Outline 外观。自定义图标优先于状态圆点。徽标向辅助技术提供完整的状态文字。
