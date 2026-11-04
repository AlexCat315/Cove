# NumberField

数值绑定与编辑草稿分开。Return 或失焦提交，非法草稿回退；步进遵守边界。Shift 放大步长，Option 缩小步长。前缀可拖动调整，双击恢复 reset_value。

```rust
use cove_core::State;
use cove_ui_compose::primitives::controls::fields::numeric::{
    NumberField, NumberFieldOptions, NumberFieldPrefix, NumericTextFormat,
};

let amount = State::new(3.14_f32);
let field = NumberField::new(amount.binding()).range(Some(0.0), Some(10.0))
    .step(0.25).shows_stepper(true).prefix(NumberFieldPrefix::new("W"))
    .reset_value(1.0);
// 与 Vec3Field 共用同一份格式/边界/步长/启用配置。
let options = NumberFieldOptions::default().format(NumericTextFormat::new(',', 2));
let comma = NumberField::new(amount.binding()).options(options);
```

Gallery 与 Swift 对齐：3.14 的绑定、普通/拖动/混合/禁用示例，以及共享 1.25 绑定的逗号与小数点输入。分组符和尾随垃圾输入不会被宽松解析。

共享 Rust 实现适用于所有宿主。结构测试与 macOS 原生操作验收分别记录在 `Docs/gallery-audit.md`；输入事件注入不代表原生键盘或 IME 已验收。
