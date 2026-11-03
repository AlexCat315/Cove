`Text` 是普通文字与样式片段共用的 Rust 文本组件。

```rust
use cove_ui::prelude::*;

let plain = Text::new("Cove");
let styled = Text::runs(vec![TextRun::with_traits("Cove", TextTraits::BOLD)]);
```

完整用法、选择交互和迁移说明见 [Text](/docs/text)。
