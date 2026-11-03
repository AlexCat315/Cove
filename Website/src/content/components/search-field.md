# SearchField

`SearchField` 将 `TextBuffer` 绑定到搜索输入框。数据筛选由调用方决定，组件不执行网络请求。

```rust
use cove_ui::prelude::*;

fn search(cx: &Scope) -> impl View {
    let query = cx.state(TextBuffer::default);
    SearchField::new(query.binding())
        .placeholder("Search components…")
}
```

绑定在文本提交时更新。输入法的候选组合不提前提交到绑定；焦点、候选位置与光标闪烁由共享文本输入组件和宿主协同处理。点击输入框后即可使用原生键盘与输入法，无需先通过调试工具填值。

提供 `suggestions(Vec<String>)` 后，输入框下方显示匹配项。上下键移动高亮，Enter 选择建议或调用 `on_submit`；`on_suggestion_selected` 接收选中的文字。Escape 默认清空查询，可用 `clears_on_escape(false)` 关闭。清空按钮更新同一个绑定，清空后不会在重新布局时恢复旧值。

`enabled(false)` 禁止输入。前导搜索图标是装饰，不单独进入焦点顺序。输入框的文字区域避开搜索图标和清空按钮。

Gallery 侧栏按名称、分类和简介筛选，英文大小写不敏感，多个词按“全部包含”匹配；筛选结果只改变导航列表，不自动切换当前示例。无结果时显示空状态，清空后恢复列表。
