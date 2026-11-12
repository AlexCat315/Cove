# ScrollView

`ScrollView::vertical`、`horizontal` 接收内容；`scroll_offset` 绑定当前位置，`scroll_request` 接受程序滚动请求。

浏览器宿主会把画布内的滚轮和触控板位移交给 `ScrollView`，保留像素、行或页单位。可在 Gallery 的 `?component=scrollView` 验证。

导入 `cove_ui::compose::primitives::layout::ScrollAnchorViewExt` 后，对稳定标识的行使用 `.scroll_anchor()`，前插内容时保持其视口位置。指针进入和离开时滚动条渐入、渐出；减少动态效果时直接更新。

共享 Rust 实现覆盖所有宿主平台。自动化场景验证与原生视觉、键盘及 IME 验收分别记录在 Gallery 验收表中。

## 滚动条

`scrollbar_gutter` 决定滚动条的位置：`Overlay`（默认）浮在内容上；`Stable` 在尾边预留固定通道，尾部控件不会被遮挡；`Hidden` 不绘制滚动条、也不接收指针，内容仍可通过滚轮、触控板、键盘与 `ScrollReveal` 滚动。标签栏、工具栏这类自带溢出菜单的条带使用 `Hidden`。
