# ScrollView

`ScrollView::vertical`、`horizontal` 接收内容；`scroll_offset` 绑定当前位置，`scroll_request` 接受程序滚动请求。

导入 `cove_ui::compose::primitives::layout::ScrollAnchorViewExt` 后，对稳定标识的行使用 `.scroll_anchor()`，前插内容时保持其视口位置。指针进入和离开时滚动条渐入、渐出；减少动态效果时直接更新。

共享 Rust 实现覆盖所有宿主平台。自动化场景验证与原生视觉、键盘及 IME 验收分别记录在 Gallery 验收表中。
