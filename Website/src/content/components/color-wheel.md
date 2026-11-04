# ColorWheel

取色轮，两种样式：

- `ColorWheelStyle::Ring`（默认）：色相环包围一个内嵌的饱和度/明度方块。
- `ColorWheelStyle::Disk`：整块圆盘，角度映射色相、半径映射饱和度（明度固定为满值，由通道滑块或方向键调整）。

红色都在正上方，色相顺时针增大。拖动实时写回绑定；方向键步进色相与明度；透明度保持不变。

```rust
use cove_core::{Color, State};
use cove_ui_compose::primitives::controls::fields::color_wheel::{ColorWheel, ColorWheelStyle};

let color = State::new(Color::from_rgba8(200, 64, 64, 255));
let ring = ColorWheel::new(color.binding()).diameter(160.0);
let disk = ColorWheel::new(color.binding()).style(ColorWheelStyle::Disk);
let ring_only = ColorWheel::new(color.binding()).shows_value_square(false);
```

HSV 转换与角度/方块/圆盘映射在 `color_wheel_model`（`hsv_from_color`、`color_from_hsv`、`hue_for_angle`、`angle_for_hue`、`sv_for_point`、`point_for_sv`、`disk_saturation_for_radius`、`disk_radius_for_saturation`）。圆盘的平滑逐顶点着色用 `cove_core` 的 `DrawList::add_disk_shaded(center, radius, |x, y| color)`；环用 `add_shaded_annulus(center, inner, outer, shade)`。两者都按位置逐顶点求色，并按半径做一像素羽化。

色轮是取色器家族的一部分，不是通用的轮盘型 Slider。若需要角度/旋钮类通用控件，应单独立组件，而不是把颜色语义塞进 Slider。`ColorField` 默认在浮层顶部展示环样式；`shows_wheel(false)` 可关闭。

共享 Rust 实现适用于所有宿主。结构测试与 macOS 原生操作验收分别记录在 `Docs/gallery-audit.md`；输入事件注入不代表原生键盘或 IME 已验收。
