# TextField

`TextField` 编辑绑定的 `TextBuffer`。普通输入、删除、清空和平台输入法提交都会更新同一个绑定；候选组合只显示预览，提交后才改变绑定。

```rust
use cove_ui::prelude::*;

fn project_name(cx: &Scope) -> impl View {
    let text = cx.state(|| TextBuffer::new("Cove"));
    TextField::new("Project name", text.binding())
        .behavior(TextFieldBehavior {
            clearable: true,
            ..Default::default()
        })
        .frame_width(240.0)
}
```

点击定位光标，拖动选择；双击选择词语，三击选择全部文字。非空选区两端显示错位的短竖线：起点贴下缘、终点贴上缘，采用细线与圆头，插入光标在此时暂停绘制，避免与选区端点重叠闪烁。Shift 配合方向键扩展选区。Home / End 移到当前显示行的首尾：软换行时停在行尾并保留行尾亲和，下一次上下移动仍回到该行末。输入和移动后光标重新开始闪烁计时。

macOS 使用 Command，其他桌面平台使用 Ctrl：A 全选、C 复制、X 剪切、V 粘贴。粘贴支持异步系统剪贴板。`secure: true` 隐藏显示内容并阻止复制和剪切；`read_only: true` 保留选择和复制，禁止修改；`disabled: true` 禁止交互。

## 撤销与重做

相邻的单字符输入合并成一步，连续删除同样合并；一次粘贴、IME 提交或选择替换是独立一步。撤销连选区一起恢复，新编辑丢弃重做分支，外部改写绑定的文本会清空两侧栈。

```rust
fn editor(cx: &Scope) -> impl View {
    let text = cx.state(|| TextBuffer::new(""));
    // Primary-Z 撤销，Primary-Shift-Z / Primary-Y 重做。
    TextField::new("Notes", text.binding()).multiline()
}
```

字段同时发布 `TextEditActions`，菜单栏的撤销/重做项和场景级快捷键读取同一份历史，不会把 Primary-Z 漏给外层快捷键宿主。

## 多行测量与滚动

垂直字段按行盒测量，随内容增高到 `max_visible_lines`，之后固定高度并滚动。显式换行也会让默认横向字段增高，直到同一个上限。滚动到上限后，滚轮以 `MULTILINE_WHEEL_STEP`（30pt）为一格移动视口；未溢出时滚轮继续传给外层滚动视图。

```rust
fn notes(cx: &Scope) -> impl View {
    let text = cx.state(|| TextBuffer::new(""));
    TextField::new("Notes", text.binding())
        .layout(TextFieldLayout {
            axis: TextFieldAxis::Vertical,
            max_visible_lines: 8,
            ..Default::default()
        })
}
```

## 词与行间导航

Option+Left/Right（Windows 为 Ctrl+Left/Right）按词移动，Option+Backspace / Option+Delete 按词删除；Command+Left/Right 到行首行尾，Command+Up/Down 到文档首尾，Command+Backspace / Command+Delete 删除到行首行尾。Up/Down 逐显示行移动并保持目标列，PageUp/PageDown 移动一个视口（保留七行可见区域中的一行作为上下文）。没有文字环境时按逻辑行计算，列位置同样保持。

## 逐段文字样式

`TextFieldCodeEditing::syntax_color_at_utf8_offset` 按 UTF-8 偏移返回颜色，文字按字形着色，未命中的字形使用字段的正文颜色。占位文本不参与着色。

```rust
let code = TextFieldCodeEditing {
    syntax_color_at_utf8_offset: Some(std::rc::Rc::new(|buffer: &TextBuffer, offset: usize| {
        (offset < 3).then(|| Color::hex(0xCC33E6))
    })),
    ..Default::default()
};
```

## 前后缀与长度限制

`prefix` / `suffix` 排在文字列内，`prepend` / `append` 是两端的独立色块；文本区宽度、光标位置和点击命中都会扣除这些占位。`show_word_limit` 配合 `max_length` 在尾部显示 `n / limit` 计数。

`max_length` 按完整字素计数，输入、粘贴与 IME 提交都受它约束。替换选区时先扣除被替换的字符；组合重音或 ZWJ 与原有文字合并且没有增加字符数时，即使已到上限也可以写入。

```rust
fn price(cx: &Scope) -> impl View {
    let text = cx.state(|| TextBuffer::new("12"));
    TextField::new("Amount", text.binding())
        .behavior(TextFieldBehavior {
            max_length: Some(6),
            ..Default::default()
        })
        .decoration(TextFieldDecoration {
            prefix: Some("¥".into()),
            suffix: Some("kg".into()),
            show_word_limit: true,
            ..Default::default()
        })
}
```

## 行号与横向滚动

`shows_line_numbers` 打开行号栏：三位数宽度的固定列加上 24pt 间距，`line_number_color` 是数字颜色，`line_number_gutter_color` 填充栏底。文档列不会因为行号位数变化而移动，行号也不随横向滚动移动。

长行溢出时，绘制阶段会把光标揭示进可见区域；横向滚轮按 30pt 步进（与纵向一致），同时支持触控板的精确增量。字段只要有一条轴溢出，就独占滚轮（包括滚到边界），不会把剩余增量转给外层滚动视图；聚焦且溢出的字段会声明 `WheelRoutingPriority::PreferFocused`，指针位于字段内时优先接收滚轮。hover 时在尾端绘制滚动条覆盖层。

```rust
let editor = TextField::new("", code.binding())
    .layout(TextFieldLayout { axis: TextFieldAxis::Vertical, ..Default::default() })
    .code_editing(TextFieldCodeEditing {
        shows_line_numbers: true,
        line_number_color: Some(Color::hex(0x9AA4B2)),
        ..Default::default()
    });
```

绘制时会把当前形状布局发布到 `__textfield_document_layout`，诊断和测试可以读取已排版行的边界。

## 回车与取消

单行字段的 Return 提交；Shift-Return 在任意字段插入换行；垂直字段的 Return 插入换行，Primary-Return 提交。设置了 `on_cancel` 时 Escape 会被消费并调用回调，随后把焦点交还最近的焦点祖先，编辑器的容器因此可以继续处理快捷键。

## 平台输入

平台输入接口使用 UTF-16 偏移。选区落入组合 emoji 或重音字母内部时，会回退到完整字素的边界；删除范围向完整字素扩展，避免留下半个字符。IME 组合提交与删除同样进入撤销历史，并遵守 `max_length`。

SDL / Android 的组合事件使用 Unicode 标量偏移，字段会转换到完整字素边界；组合预览缓存可在重绘时复用。失焦取消未提交的组合文字。

自定义宿主从 UTF-16 位置产生组合事件时，使用 `TextEditingEvent::from_utf16` 统一单位；Windows SDL 的入口也会先做此转换。浏览器当前只传入组合文本和零偏移。

样式的焦点/编辑接入见 [TextFieldStyle](/docs/text-field-style)，字符和协议索引约定见 [TextBuffer](/docs/text-buffer)。搜索和建议列表使用 [SearchField](/docs/search-field)；正文拖选使用[文本选择区域](/docs/text-selection)。
