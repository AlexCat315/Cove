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

点击定位光标，拖动选择；双击选择词语，三击选择全部文字。Shift 配合方向键扩展选区。Home / End 移到当前逻辑行的首尾。光标在焦点内闪烁，输入和移动后重新开始计时。

macOS 使用 Command，其他桌面平台使用 Ctrl：A 全选、C 复制、X 剪切、V 粘贴。粘贴支持异步系统剪贴板。`secure: true` 隐藏显示内容并阻止复制和剪切；`read_only: true` 保留选择和复制，禁止修改；`disabled: true` 禁止交互。

平台输入接口使用 UTF-16 偏移。选区落入组合 emoji 或重音字母内部时，会回退到完整字素的边界；删除范围向完整字素扩展，避免留下半个字符。

当前完成的是基础输入与选择。撤销/重做、多行自动测量与滚轮滚动、完整文字样式及前后缀装饰仍在迁移验收中，暂不作为已完成能力列出。搜索和建议列表使用 [SearchField](/docs/search-field)；正文拖选使用[文本选择区域](/docs/text-selection)。
