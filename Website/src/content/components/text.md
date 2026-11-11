# Text

普通文字和带样式片段都使用 `Text`，共用同一节点、排版配置和选择交互。

```rust
use cove_ui::prelude::*;

fn plain() -> Text {
    Text::new("Hello, Cove")
}

fn styled() -> Text {
    Text::runs(vec![
        TextRun::new("Read the "),
        TextRun::with_traits("Cove guide", TextTraits::BOLD)
            .link("guide://start"),
        TextRun::new(" to get started."),
    ])
    .on_open_link(|destination| println!("{destination}"))
}
```

两种构造方式都支持 `alignment`、`color`、`line_limit`、`truncated`、`display_only` 和环境字体。`TextTraits` 提供粗体、斜体、行内代码与删除线，可用 `|` 组合；`plain_text()` 读取拼接后的正文。

## 浮点字号

```rust
use cove_ui::compose::modifiers::style::AppearanceExt;
use cove_ui::core::{Font, FontWeight};
use cove_ui::prelude::*;

fn caption() -> impl View {
    Text::new("Fractional type").font(Font::system(13.5, FontWeight::Regular))
}
```

排版以浮点坐标计算字距、行宽和光标位置。绘制时，轮廓字形按最接近的四分之一设备像素横向取样，纵向基线对齐到设备像素。

## 选择与复制

在一段或多段正文外使用 `text_selection(true)`。普通文字和样式片段参加同一选区；按钮、菜单行等控件继续拥有自身手势。

```rust
use cove_ui::prelude::*;

fn selectable_text() -> impl View {
    Text::runs(vec![
        TextRun::new("Select "),
        TextRun::with_traits("this text", TextTraits::ITALIC),
    ])
    .text_selection(true)
}
```

拖选、反向拖选、双击选词、三击选段落、系统复制快捷键与 Shift 扩选使用同一套交互。跨链接拖选不打开链接，单击才交给 `on_open_link`。触控长按在释放时选词，快速点击和滑动保留给链接及滚动。

非空选区两端显示错位的短竖线：起点贴下缘、终点贴上缘，采用细线与圆头。跨行或跨段落时，只标记整个选区的两个外端点。切换 `Text::new` 与 `Text::runs` 保留节点身份与仍有效的选区。`display_only(true)` 让该文本退出外层阅读选区。

短竖线是边界标记；调整范围使用拖选或 Shift 快捷键。触控圆形手柄与放大镜尚未接入。输入框仍由 `TextField` 编辑，使用同样的端点外观。

## 行数与截断

`line_limit(Some(2))` 限制显示行数；`truncated()` 以省略号拟合一行。带样式片段在截断时保留样式，并按完整字素切分。

```rust
use cove_ui::prelude::*;

fn title() -> impl View {
    Text::runs(vec![TextRun::with_traits(
        "A title with é and 👩🏽‍💻",
        TextTraits::BOLD,
    )])
    .truncated()
    .frame_width(160.0)
}
```

本次为破坏性整理：删除 `RichText`，不提供兼容别名。原 `RichText::new(runs)` 改为 `Text::runs(runs)`，原 `RichTextRun` / `RichTextTraits` 改为 `TextRun` / `TextTraits`。`with_alignment` / `with_color` 改为 `alignment` / `color`。

CJK 回退按字体家族匹配当前字重和斜体；同一段内容切换粗体时重新选择对应字面。

回退字体继承主字体实际解析出的样式。Apple 使用主字体的 CoreText 字重及 italic trait；其他平台读取 OpenType OS/2、head 和当前 variation 坐标。命名字体本身已是 Bold 时，即使请求参数为 Regular，CJK 回退也保留其实际字重。
