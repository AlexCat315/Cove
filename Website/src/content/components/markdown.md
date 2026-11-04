# MarkdownView

`MarkdownView::new(source)` 渲染标题、段落、列表、表格与围栏代码块。通过 `.on_open_link(...)` 接收链接激活。

围栏代码保留不换行的固有宽度，超出容器后横向滚动；正文段落按容器宽度换行。代码块提供语言标签与复制操作。
