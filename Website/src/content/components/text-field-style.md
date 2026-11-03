# TextFieldStyle

`TextFieldStyle` 接收输入内容和交互状态，负责背景、边框与焦点外观。文字、光标、选区、输入法和撤销历史由 `configuration.content` 保留；自定义样式应包裹这个内容。

```rust
use cove_ui::prelude::*;
use cove_ui::compose::modifiers::style::AppearanceExt;
use cove_ui::compose::theme::styles::components::{
    TextFieldStyle, TextFieldStyleConfiguration, TextFieldStyleViewExt,
};

struct EditingHighlight;

impl TextFieldStyle for EditingHighlight {
    fn make_body(&self, configuration: TextFieldStyleConfiguration) -> AnyView {
        let inputs = configuration.theme.inputs;
        let background = if !configuration.is_enabled {
            inputs.background_disabled
        } else if configuration.is_editing {
            configuration.theme.colors.focus_ring.multiplied_alpha(0.08)
        } else {
            inputs.background
        };
        let border = if configuration.is_focused {
            inputs.border_focused
        } else {
            inputs.border_color
        };
        AnyView::new(configuration.content
            .background_color(background)
            .border_color(border, 1.0)
            .corner_radius(inputs.radius))
    }
}

fn name(cx: &Scope) -> impl View {
    let text = cx.state(|| TextBuffer::new("Cove"));
    TextField::new("Name", text.binding())
        .text_field_style(EditingHighlight)
        .frame_width(240.0)
}
```

`is_focused` 表示当前键盘焦点；`is_editing` 在持有焦点或存在输入法组合时为真，它不表示值已改变。焦点、组合开始、取消和提交会触发样式重建，无需等待文字绘制。重建保留同一个输入节点、光标、选区及历史。

`is_hovered` 遵循设备的 hover 能力，触摸表面不模拟悬停。`is_enabled` 为假时焦点和编辑状态复位，控件不接受输入。`is_error` 是样式配置中的错误状态，字段当前默认传入 `false`。

`text_field_style` 可以应用到容器，让子字段继承样式；字段自身的 `.style(AnyTextFieldStyle::new(...))` 优先于继承值。默认使用 `DefaultTextFieldStyle`；`EditorTextFieldStyle` 提供平整的编辑器表面，通过光标和选区显示焦点。

Gallery 的 TextField 页面展示编辑状态高亮和 Editor 样式，可配合 Size、Disabled 和主题切换检查外观。
