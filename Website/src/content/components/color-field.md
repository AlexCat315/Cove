# ColorField

色块打开浮层：

- 色轮（环/盘）与透明度棋盘预览。
- RGBA 滑块；**R/G/B/A 数值可直接输入 0–255**（提交或失焦写入）。
- **HEX / HSL 文本切换**，输入按当前格式解析。
- **最近颜色**自动进色板：预设色板 + 最近提交的颜色（去重、上限 8）。
- 透明度滑块的轨道是棋盘上的透明→不透明渐变。

```rust
use cove_core::{Color, State};
use cove_ui_compose::primitives::controls::fields::color_field::ColorField;

let color = State::new(Color::from_rgba8(88, 126, 231, 255));
let field = ColorField::new(color.binding()).shows_inline_values(true);
let no_wheel = ColorField::new(color.binding()).shows_wheel(false);
let brand = ColorField::new(color.binding()).swatches(vec![
    Color::from_rgba8(88, 126, 231, 255),
    Color::from_rgba8(20, 24, 33, 255),
]);
// 最近列表可预置，也可在提交后读取。
let seeded = ColorField::new(color.binding())
    .recent_colors(vec![Color::from_rgba8(20, 24, 33, 255)]);
let recents = seeded.recent();
```

开关：`show_alpha(false)` 隐藏透明度，`shows_wheel(false)` 收起色轮，`shows_swatches(false)` 收起色板，`shows_inline_values/channels` 控制触发器上的文本。

## 吸管

`on_sample(handler)` 提供屏幕取色：设置后浮层出现吸管按钮，宿主在取色完成时回调采样颜色。控件本身不访问屏幕。

契约只在 `cove-platform`：`ScreenColor`、`ScreenColorSample`、`ScreenColorCompletion`、`ScreenColorBackend`。实现全部在 `cove-shell` 的宿主/后端层，按窗口系统分后端，并且都是异步回调——`Shell::sample_screen_color` 立即返回，结果回到平台线程再回调，恰好一次；取消、不支持、失败是三种不同的结果。

```rust
use cove_ui_compose::primitives::controls::fields::color_field::ColorField;
use cove_core::{Color, State};

let color = State::new(Color::from_rgba8(88, 126, 231, 255));
let field = ColorField::new(color.binding()).on_sample(|done| {
    // 宿主在这里启动原生取色；稍后带着选中的颜色调用 done，取消则不调用。
    done(Color::from_rgba8(88, 126, 231, 255));
});
```

| 平台 | 后端 |
| --- | --- |
| macOS | `NSColorSampler`（`objc2-app-kit`） |
| Windows | Win32 屏幕 DC 截屏采样 |
| Linux | X11 `XGetImage` 截屏采样；Wayland 用 XDG `PickColor` portal |
| iOS / Android / Wasm | 不支持，宿主不安装取色器，吸管隐藏 |

色轮是取色器的一部分，不属于通用 Slider。单独使用时见 [ColorWheel](/docs/color-wheel)。

共享 Rust 实现适用于所有宿主。结构测试与 macOS 原生操作验收分别记录在 `Docs/gallery-audit.md`；输入事件注入与 fake sampler 不代表原生拾色、原生键盘或 IME 已验收，Windows/Linux 的原生取色交互仍待实机验收。
