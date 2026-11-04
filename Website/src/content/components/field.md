# Field

单个具名字段的外壳：标签、控件、帮助或错误文案。`Form` 里的一格和独立使用的 `Field` 共用同一套布局，所以单独展示一个控件时也能得到真实的标签语义、必填标记、可点击聚焦和可访问性描述，而不是页面各自拼一个标签加控件。

```rust
use cove_ui_compose::primitives::controls::{Field, FormLayout, FieldLabelPlacement};

let field = Field::new("width", "Width", control)
    .configure(|presentation| {
        presentation.help = "Points".to_owned();
    });
// 需要固定标签位置时覆盖默认的自适应放置。
let stacked = field.layout(FormLayout { label_placement: FieldLabelPlacement::Stacked, ..Default::default() });
```

标签在宽度足够时移到控件左侧，否则在控件上方堆叠；`is_required`、`help` 和 `error` 来自同一份 `FormFieldPresentation`。放进 `Form` 时由表单控制器提供校验错误，独立使用时由调用方在 `presentation.error` 上给出。

## 共享输入框外壳

所有可编辑控件从同一份 `InputAppearance` 解析填充、边框和圆角。`InputChromeState`（启用、聚焦、悬停、错误）是唯一的交互快照，`TextField`、数值前缀、`Select`、`TimeInput` 和 JSON 编辑器都据此取色，不再各自手写背景与边框；主题换一套 `InputAppearance` 就整体生效。

共享 Rust 实现适用于所有宿主。结构测试与 macOS 原生操作验收分别记录在 `Docs/gallery-audit.md`。
