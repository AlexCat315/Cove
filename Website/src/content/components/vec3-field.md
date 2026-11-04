# Vec3Field

三个轴共用数值格式、步长和约束。宽度足够时等宽并排，否则三个轴一起堆叠。拖轴标签可调整值，双击复位；右键菜单复制、粘贴或复位整个向量。

```rust
use cove_core::State;
use cove_ui_compose::primitives::controls::fields::{Vec3, Vec3Constraints};
use cove_ui_compose::primitives::controls::fields::vec3_field::Vec3Field;

let (x,y,z) = (State::new(1.0), State::new(2.0), State::new(3.0));
let field = Vec3Field::new(x.binding(), y.binding(), z.binding()).step(0.1)
    .constraints(Vec3Constraints {reset:Some(Vec3::new(0.0,0.0,0.0)), ..Default::default()});
```

Gallery 对应 Swift 的全宽 Transform 与 240pt 混合 X/Z 示例，共享初始向量 (1,2,3)。默认连接平台剪贴板，异步读取后严格验证三个有限数值；也可提供自定义 on_copy / on_paste。with_format 可配置所有轴的小数格式。

共享 Rust 实现适用于所有宿主。结构测试与 macOS 原生操作验收分别记录在 `Docs/gallery-audit.md`；输入事件注入不代表原生键盘或 IME 已验收。
