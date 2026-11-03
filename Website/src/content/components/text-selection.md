# 文本选择

在阅读区域上使用 `text_selection(true)`，可以跨多个普通 `Text` 拖动选择并复制。该区域共用一份选择状态。

```rust
use cove_ui::prelude::*;

fn paragraph() -> impl View {
    Text::new("Select this paragraph and copy it.")
        .text_selection(true)
}
```

拖动选择文字，双击选择词，三击选择段落；Shift 扩展选择。使用系统复制快捷键复制选区。区域内的多个段落按屏幕阅读顺序拼接，滚动和重新布局会更新选择几何。

按钮标题、可点击列表行等由控件拥有的文字保留控件手势，不会被外层阅读选择抢走。可编辑输入框由自身处理光标与选区。需要阻止子区域参加外层选择时，在子区域使用 `text_selection(false)`。

Gallery 的示例内容启用阅读选择，侧栏导航仍保留点击操作。只有在启用了选择且没有被控件手势拥有的正文上，才应看到文字选择光标；可见文字不等于所有位置都应该可拖选。

右键菜单以及 Look Up / Share 取决于宿主安装的操作，不由 `text_selection(true)` 自动提供。应用可使用 `text_selection_actions` 传入对应回调。
