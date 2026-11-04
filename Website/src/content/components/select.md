# Select

类型化值绑定到一组选项。Space / Return 打开，方向键移动，Return 提交，Escape 取消；禁用项跳过。长标签在触发器内截断，菜单可查看完整名称。

```rust
use cove_core::State;
use cove_ui_compose::primitives::controls::fields::select::{Select, SelectOption};

let quality = State::new("high".to_owned());
let field = Select::new(quality.binding(), vec![
    SelectOption::new("fast".to_owned(), "Fast", true),
    SelectOption::new("high".to_owned(), "High quality", true),
    SelectOption::new("offline".to_owned(), "Unavailable", false),
]);
```

Gallery 使用 Swift 的 fast / balanced / high / offline，默认 high，保留包含组合 emoji 的长标签和 360pt 最大宽度。

共享 Rust 实现适用于所有宿主。结构测试与 macOS 原生操作验收分别记录在 `Docs/gallery-audit.md`；输入事件注入不代表原生键盘或 IME 已验收。
